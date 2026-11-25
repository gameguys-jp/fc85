//! ACRTC（HD63484）と Data Fabric の port 1 のつなぎ。
//!
//! 回路は、A 基板のブリッジの同期化の FF（DR-016）と、B 基板の P1A（fabric.md §07）。時刻の計算は
//! `docs/parts/host-acrtc-interface.md` §2.3・§7、DMA の手順は DR-022 と台帳 HD63484.md。
//!
//! 模型：
//! - 時刻は core 層の単位（1 スロットが 3780）。16 MHz の立上り（q0〜q3）は 945 ごと、ACRTC の 2CLK
//!   （21.47727 MHz÷4＝5.369318 MHz）の立上りは 2816 ごと。2 つの発振源の位相は `set_phase` で決める
//!   （既定は 0）。実機の水晶のずれは扱わない。
//! - Fabric の出力（DACK、R/W、データ）は、アクセスを始めるスロットの頭（P1A のクロック）の 20 ns 後に
//!   変わる [P]。DACK を戻すのも、P1A のクロックの 20 ns 後。
//! - ACRTC は、立上りの 40 ns 前の DACK を見る（tDAKS）。DREQ は立上りの 50 ns 後（tDRQD2）、DTACK は
//!   70 ns 後（tDDTZL）、DONE は 50 ns 後（tDND）に変わる。どれも表の値 [D U85 p.314–315]。DTACK と
//!   DONE が DACK＝H から戻る時刻（tDDTLH、tDNLZ）は扱わず、Tn の立上りで戻す（次のアクセスまでに
//!   同期化の FF を抜けるので、結果は変わらない。host-acrtc-interface.md §7.3）。
//! - ブリッジの FF は 16 MHz の立上りで、10 ns 前の値を取る [P]。DREQ と DTACK は 2 段、DONE は 1 段（DR-016）。
//! - P1A は 16 MHz の立上りで、2 段目の DTACK（その立上りの前の値）を見たら DACK を戻し、データバスの語と
//!   DONE の FF を取り込む。アクセスはそのスロットで終わる（Fabric は次のスロットの頭で次を決める）。
//! - RDY1（Fabric の準備の信号）は、スロットの中ほど（q2 の立上り、MCLK↑）に取り込む 2 段目の DREQ。

use crate::fabric::{Access, Dir, PortReply};
use crate::video::Video;
use fc85_chips::hd63484::{Hd63484, Vram};
use fc85_core::time::{Clock, PERIOD_2CLK, PERIOD_16M, SLOT, Time};
use serde::{Deserialize, Serialize};

/// Fabric の出力の遅れ（20 ns）。
pub(crate) const T_OUT: Time = 302;
/// tDAKS（40 ns）。
const T_DAKS: Time = 605;
/// tDRQD2（50 ns）。
const T_DRQD: Time = 756;
/// tDDTZL（70 ns）。
const T_DDTZL: Time = 1058;
/// tDND（50 ns）。
const T_DND: Time = 756;
/// 同期化の FF のセットアップ（10 ns）。
pub(crate) const T_SETUP: Time = 151;

/// ACRTC の VRAM：128 KiB（64K 語。HM4864A×16、仕様書 §05）。ACRTC の 20 bit の語番地のうち
/// 下位 16 bit を使い、上は折り返す [V]。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VramMem(pub Vec<u16>);

impl VramMem {
    pub fn new() -> Self {
        Self(vec![0; 0x10000])
    }
}

impl Default for VramMem {
    fn default() -> Self {
        Self::new()
    }
}

impl Vram for VramMem {
    fn read(&mut self, addr: u32) -> u16 {
        self.0[(addr & 0xFFFF) as usize]
    }
    fn write(&mut self, addr: u32, data: u16) {
        self.0[(addr & 0xFFFF) as usize] = data;
    }
}

/// 1 本の信号線（true が L：アクティブ）。最後に変わった時刻を持つ。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Pin {
    level: bool,
    prev: bool,
    at: Time,
}

impl Pin {
    fn value(&self, t: Time) -> bool {
        if t >= self.at { self.level } else { self.prev }
    }

    fn set(&mut self, level: bool, t: Time) {
        if level != self.level {
            self.prev = self.level;
            self.level = level;
            self.at = t;
        }
    }
}

/// ブリッジと P1A。ACRTC と VRAM を持つ。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcrtcBridge {
    pub acrtc: Hd63484,
    pub vram: VramMem,
    /// 次のスロットの頭の時刻。
    now: Time,
    /// 2CLK。
    clk: Clock,
    dack: Pin,
    /// DMA の R/W（H が ACRTC への書込み）とデータバス。DACK を戻した後も保つ。
    bus_write: bool,
    bus_data: u16,
    dreq: Pin,
    dtack: Pin,
    done: Pin,
    dreq_s: [bool; 2],
    dtack_s: [bool; 2],
    done_ff: bool,
    rdy1: bool,
    /// P1A が DACK を出している。
    active: bool,
    /// このスロットで終わったアクセス（読んだ語、DONE）。
    reply: Option<(u16, bool)>,
    /// DACK を出したスロットの数（統計）。
    pub dack_slots: u64,
    /// 映像の出力段（パレット、画面、映像の割込み）。
    pub video: Video,
}

impl Default for AcrtcBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AcrtcBridge {
    pub fn new() -> Self {
        Self {
            acrtc: Hd63484::new(),
            vram: VramMem::new(),
            now: 0,
            clk: Clock::new(PERIOD_2CLK, 0),
            dack: Pin::default(),
            bus_write: false,
            bus_data: 0,
            dreq: Pin::default(),
            dtack: Pin::default(),
            done: Pin::default(),
            dreq_s: [false; 2],
            dtack_s: [false; 2],
            done_ff: false,
            rdy1: false,
            active: false,
            reply: None,
            dack_slots: 0,
            video: Video::new(),
        }
    }

    /// 次の 2CLK の立上りを、次のスロットの頭から `units` 後にする（0〜2815）。
    pub fn set_phase(&mut self, units: Time) {
        self.clk.set_next(self.now + units % PERIOD_2CLK);
    }

    /// `t` 以降の最初の 2CLK の立上り。
    pub fn edge_at_or_after(&self, t: Time) -> Time {
        let n = self.clk.next();
        if t <= n {
            n
        } else {
            n + (t - n).div_ceil(PERIOD_2CLK) * PERIOD_2CLK
        }
    }

    /// RDY1：中ほどで取り込んだ、同期化した DREQ。
    pub fn rdy1(&self) -> bool {
        self.rdy1
    }

    /// DACK を出しているか。
    pub fn dack(&self) -> bool {
        self.dack.level
    }

    /// Fabric への応答：このスロットで終わっていれば語（DONE が出ていれば `Last`）。
    pub fn port_reply(&self) -> PortReply {
        match self.reply {
            None => PortReply::Wait,
            Some((v, false)) => PortReply::Done(v),
            Some((v, true)) => PortReply::Last(v),
        }
    }

    /// Fabric がアクセスを打ち切った（時間切れ、STOP）。次のスロットの頭で DACK を戻す。
    pub fn abort(&mut self) {
        if self.active {
            self.active = false;
            self.dack.set(false, self.now + T_OUT);
        }
    }

    /// `t0` から 1 スロット進める。`starting` は、このスロットで始まる port 1 のアクセス。
    pub fn run_slot(&mut self, t0: Time, starting: Option<&Access>) {
        let end = t0 + SLOT;
        self.reply = None;
        if let Some(a) = starting {
            self.active = true;
            self.bus_write = a.dir == Dir::Write;
            self.bus_data = a.data;
            self.dack.set(true, t0 + T_OUT);
        }
        if self.active {
            self.dack_slots += 1;
        }
        let mut q = 0;
        loop {
            let ff = (q < 4).then(|| t0 + q * PERIOD_16M);
            let edge = (self.clk.next() < end).then(|| self.clk.next());
            match (ff, edge) {
                (None, None) => break,
                (Some(f), Some(e)) if e < f => self.acrtc_edge(),
                (Some(f), _) => {
                    self.ff_edge(f, q);
                    q += 1;
                }
                (None, Some(_)) => self.acrtc_edge(),
            }
        }
        self.now = end;
    }

    /// 16 MHz の立上り：同期化の FF と P1A。
    fn ff_edge(&mut self, t: Time, q: Time) {
        let s = t.saturating_sub(T_SETUP);
        let dtack2 = self.dtack_s[1];
        let done = self.done_ff;
        if q == 2 {
            self.rdy1 = self.dreq_s[1];
        }
        self.dreq_s = [self.dreq.value(s), self.dreq_s[0]];
        self.dtack_s = [self.dtack.value(s), self.dtack_s[0]];
        self.done_ff = self.done.value(s);
        if self.active && dtack2 {
            self.active = false;
            let v = if self.bus_write {
                0
            } else {
                self.acrtc.dma_data()
            };
            self.reply = Some((v, done));
            self.dack.set(false, t + T_OUT);
        }
    }

    /// 2CLK の立上り：ACRTC を 1 周期進める。
    fn acrtc_edge(&mut self) {
        let Some(e) = self.clk.take_before(Time::MAX) else {
            return;
        };
        let dack = self.dack.value(e.saturating_sub(T_DAKS));
        self.acrtc.set_dack(dack, self.bus_write, self.bus_data);
        self.acrtc.tick(&mut self.vram);
        self.video.tick(&self.acrtc, &self.vram, e);
        self.dreq.set(self.acrtc.dreq(), e + T_DRQD);
        self.dtack.set(self.acrtc.dma_dtack(), e + T_DDTZL);
        self.done.set(self.acrtc.done(), e + T_DND);
    }
}
