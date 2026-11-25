//! DOC 子基板（ES5503 と波形 RAM 64 KiB）と、port 2 のアダプタ P2A。
//!
//! DR-011、DR-019 (3)、DR-031 (4)、DR-035、仕様書 §06（port 2）、fabric.md §06・§08、台帳 ES5503.md §2・§3。
//!
//! 模型：
//! - DOC のサイクルは E の 1 周期（CLK 8 クロック。8 MHz で 1 µs＝15120 単位）。E の立下りでサイクルが
//!   始まり、E Low の間に DOC がオシレータを 1 つ訪れて波形 RAM を読む（`tick`）。E High はホストの窓
//!   （台帳 §3.3）。E の立下りは `set_phase` で決める（既定は 0）。CLK を外部のクロックに替えるジャンパは
//!   `set_clock_period`（DR-035）。
//! - P2A（fabric.md §08）：Fabric が port 2 のアクセスを始めると、番地・データ・向きをラッチする
//!   （スロットの頭の 20 ns 後の、16 MHz の次の立上り）。E を 16 MHz の 2 段の FF で同期化し、ラッチの後に
//!   同期化した E の立下りを見たら、次の E High の窓を取る。窓では CS̄ を出し、書込みはその窓で波形 RAM に
//!   書き、読出しは窓の終わり（E の立下り）の値を取る。同期化した E の次の立下りで、Fabric に終わり
//!   （OPDN）を返す [P]。窓の前に番地を出しておくのは、台帳 §2 の注意 2（アドレスは直前の E Low の間に
//!   出す）による。
//! - 打ち切り（時間切れ、STOP）：窓を取る前なら取り消す。E が上がった後なら、窓のアクセスは行い、
//!   答えだけを捨てる [V]。
//! - 波形 RAM は 64 KiB。DOC の BS（番地の bit 16）はつながず、上下のバンクは同じ RAM（台帳 §2）。
//! - 68000 から DOC のレジスタへのアクセス（0x210000〜。同じ窓を取り合う）は段階 3 でつなぐ。今は試験と
//!   上の層が `doc` を直接読み書きする。
//! - DOC の出力（SIG±、CA、CSTRB）のサンプルホールドとミキサは段階 4。今は最後の値を記録するだけ。

use crate::acrtc::{T_OUT, T_SETUP};
use crate::fabric::{Access, Dir, PortReply};
use fc85_chips::es5503::{Es5503, Slot, WaveBus};
use fc85_core::time::{Clock, PERIOD_16M, PERIOD_DOC_E, SLOT, Time};
use serde::{Deserialize, Serialize};

/// 波形 RAM の大きさ（HM4864A×8。DR-031 (4)）。
pub const WAVE_BYTES: usize = 0x10000;

/// 時刻つきの線のレベル。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Level {
    level: bool,
    prev: bool,
    at: Time,
}

impl Level {
    fn value(&self, t: Time) -> bool {
        if t >= self.at { self.level } else { self.prev }
    }

    fn set(&mut self, level: bool, t: Time) {
        self.prev = self.value(t);
        self.level = level;
        self.at = t;
    }
}

/// P2A の段。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Stage {
    Idle,
    /// ラッチした。同期化した E の立下りを待つ。
    Wait,
    /// 次の E High の窓を取った。
    Claimed,
}

/// P2A がラッチしたアクセス。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Request {
    write: bool,
    addr: u16,
    data: u8,
    /// ラッチの時刻。
    at: Time,
}

/// DOC が訪れたオシレータの読出し（記録）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OscRead {
    pub n: u8,
    pub addr: u32,
    pub wave: u8,
}

/// P2A が使った窓（記録）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    /// P2A がアクセスをラッチした時刻。
    pub latched: Time,
    /// E の立上りと立下りの時刻。
    pub rise: Time,
    pub fall: Time,
    pub write: bool,
    pub addr: u16,
    pub data: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocBoard {
    pub doc: Es5503,
    pub wave: Vec<u8>,
    /// E の立下り（DOC サイクルの頭）。
    fall: Clock,
    /// 今のサイクルの E の立上りがまだか。
    rise_pending: bool,
    e: Level,
    /// E の同期化の FF（16 MHz）。
    sync: [bool; 2],
    stage: Stage,
    /// Fabric が始めた、まだラッチしていないアクセス。
    queued: Option<Request>,
    req: Option<Request>,
    /// 取った窓に E が上がった。
    owned: bool,
    /// 窓のアクセスが済んだ（読んだ値）。
    op_done: Option<u8>,
    /// 打ち切った後で、答えを捨てる。
    discard: bool,
    reply: Option<u16>,
    /// DOC のサイクルの数。
    pub cycles: u64,
    /// 最後に訪れたオシレータの読出しと、最後の SIG±。
    pub last_osc: Option<OscRead>,
    pub last_sig: i32,
    /// P2A が最後に使った窓。
    pub last_window: Option<Window>,
    /// P2A が使った窓の数。
    pub windows: u64,
}

impl Default for DocBoard {
    fn default() -> Self {
        Self::new()
    }
}

struct WaveRam<'a>(&'a [u8]);

impl WaveBus for WaveRam<'_> {
    fn read_wave(&mut self, addr: u32) -> u8 {
        self.0[(addr as usize) % WAVE_BYTES]
    }
}

impl DocBoard {
    /// 電源投入時。波形 RAM は 0 [V]。
    pub fn new() -> Self {
        Self {
            doc: Es5503::new(),
            wave: vec![0; WAVE_BYTES],
            fall: Clock::new(PERIOD_DOC_E, 0),
            rise_pending: false,
            e: Level::default(),
            sync: [false; 2],
            stage: Stage::Idle,
            queued: None,
            req: None,
            owned: false,
            op_done: None,
            discard: false,
            reply: None,
            cycles: 0,
            last_osc: None,
            last_sig: 0,
            last_window: None,
            windows: 0,
        }
    }

    /// E の立下りの位相（既定は 0）。`now` は次に進めるスロットの頭。
    pub fn set_phase(&mut self, now: Time, units: Time) {
        let p = self.fall.period();
        self.fall.set_next(now + units % p);
        self.rise_pending = false;
    }

    /// DOC の CLK の周期（既定は 8 MHz。外部クロックのジャンパ。DR-035）。E の周期はその 8 倍。
    pub fn set_clock_period(&mut self, clk: Time) {
        self.fall.set_period(8 * clk);
    }

    /// Fabric への答え。
    pub fn port_reply(&mut self) -> PortReply {
        match self.reply.take() {
            Some(v) => PortReply::Done(v),
            None => PortReply::Wait,
        }
    }

    /// 打ち切り（時間切れ、STOP）。
    pub fn abort(&mut self) {
        self.reply = None;
        self.queued = None;
        if self.owned {
            self.discard = true;
        } else {
            self.stage = Stage::Idle;
            self.req = None;
        }
    }

    /// `t0` から 1 スロット進める。`starting` は、このスロットで始まる port 2 のアクセス。
    pub fn run_slot(&mut self, t0: Time, starting: Option<&Access>) {
        let end = t0 + SLOT;
        // 子基板を後から挿した（試験の設定）：位相を保って今のスロットまで飛ばす
        let p = self.fall.period();
        if self.fall.next() + p < t0 {
            let skip = (t0 - self.fall.next()) / p * p;
            self.fall.set_next(self.fall.next() + skip);
            self.rise_pending = false;
        }
        if let Some(a) = starting {
            self.queued = Some(Request {
                write: a.dir == Dir::Write,
                addr: a.addr,
                data: a.data as u8,
                at: t0 + T_OUT,
            });
        }
        let mut q = 0;
        loop {
            let ff = (q < 4).then(|| t0 + q * PERIOD_16M);
            let rise = self
                .rise_pending
                .then(|| self.fall.next() - self.fall.period() / 2)
                .filter(|&r| r < end);
            let fall = (self.fall.next() < end).then(|| self.fall.next());
            // 同じ時刻なら E の縁を先にする（FF はセットアップの分だけ前の値を取る）
            let e_edge = match (rise, fall) {
                (Some(r), Some(f)) => Some(r.min(f)),
                (r, f) => r.or(f),
            };
            match (ff, e_edge) {
                (None, None) => break,
                (Some(f), Some(e)) if e <= f => self.e_edge(e, rise == Some(e)),
                (Some(f), _) => {
                    self.ff_edge(f);
                    q += 1;
                }
                (None, Some(e)) => self.e_edge(e, rise == Some(e)),
            }
        }
    }

    /// E の縁。
    fn e_edge(&mut self, t: Time, rising: bool) {
        if rising {
            self.rise_pending = false;
            self.e.set(true, t);
            if self.stage == Stage::Claimed {
                self.owned = true;
                if let Some(r) = self.req.as_mut().filter(|r| r.write) {
                    self.wave[usize::from(r.addr)] = r.data;
                }
            }
            return;
        }
        let t = self.fall.take_before(Time::MAX).unwrap_or(t);
        // 窓の終わり：読出しの値を取る
        if self.owned {
            self.owned = false;
            if let Some(r) = self.req {
                let v = self.wave[usize::from(r.addr)];
                self.op_done = Some(if r.write { 0 } else { v });
                self.last_window = Some(Window {
                    latched: r.at,
                    rise: t - self.fall.period() / 2,
                    fall: t,
                    write: r.write,
                    addr: r.addr,
                    data: if r.write { r.data } else { v },
                });
                self.windows += 1;
            }
        }
        self.e.set(false, t);
        self.rise_pending = true;
        // DOC のサイクル：E Low で波形 RAM を読む
        self.cycles += 1;
        let tick = self.doc.tick(&mut WaveRam(&self.wave));
        if let Slot::Osc { n, addr, wave } = tick.slot {
            self.last_osc = Some(OscRead { n, addr, wave });
        }
        self.last_sig = tick.sig;
    }

    /// 16 MHz の立上り：E の同期化と P2A の段。
    fn ff_edge(&mut self, t: Time) {
        // ラッチ：Fabric の出力が来ていて、前のアクセスが済んでいれば
        if self.stage == Stage::Idle
            && let Some(r) = self.queued.filter(|r| r.at <= t)
        {
            self.queued = None;
            self.req = Some(Request { at: t, ..r });
            self.stage = Stage::Wait;
        }
        let s2_old = self.sync[1];
        self.sync[1] = self.sync[0];
        self.sync[0] = self.e.value(t.saturating_sub(T_SETUP));
        let fell = s2_old && !self.sync[1];
        if !fell {
            return;
        }
        match self.stage {
            Stage::Wait if self.req.is_some_and(|r| r.at < t) => self.stage = Stage::Claimed,
            Stage::Claimed => {
                if let Some(v) = self.op_done.take() {
                    if !self.discard {
                        self.reply = Some(u16::from(v));
                    }
                    self.discard = false;
                    self.stage = Stage::Idle;
                    self.req = None;
                }
            }
            _ => {}
        }
    }
}
