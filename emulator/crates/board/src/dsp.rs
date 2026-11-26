//! DSP のアダプタ：TMS32010 と、port 3・4 の FIFO、外部プログラム RAM、DSP bridge のつなぎ。
//!
//! DR-017、DR-029、DR-035、台帳 TMS32010.md §2.1・§2.2、仕様書 §05・§07。
//!
//! 模型：
//! - DSP は命令サイクル（CLKOUT、200 ns。core 層の `PERIOD_DSP`）で進める。ベンド点のジャンパで外部
//!   クロックに替えられる（DR-035。`set_input_period`）。
//! - CTRL.RESET＝1 で、次の CLKOUT から RS を Low にし、8 サイクル後に PRGBUS を 1 にする。0 に戻すと、
//!   次の CLKOUT で PRGBUS を下げ、その次で RS を解く（DR-029 (1)）。電源投入時は RESET＝1 [P]。
//! - 68000 からプログラム RAM（0x280000〜）を読み書きできるのは PRGBUS＝1 の間だけ。ほかはバスエラー
//!   （`prog_read`・`prog_write` が `None`・`false` を返す）。
//! - I/O（台帳 §2.2）：IN 0 は入力 FIFO（port 3）から 1 語取り出す。空なら前の値を返して UNDERRUN。
//!   OUT 1 は出力 FIFO（port 4）へ 1 語入れる。満杯なら捨てて OVERRUN（DR-017 (2)）。IN 2 は状態、
//!   IN 3 はメールボックス（読むとコマンド到着を下げる）、OUT 2 は応答（68000 への要求を上げる）。
//!   ほかのポートの IN は 0 [V]、OUT は捨てる。
//! - プログラム RAM の /WE は 8〜4095 番地だけ（DR-029 (2)）。TBLW で 0〜7 番地に書くと OUT と同じに
//!   なる。回数を `io.port_tblw` に数える（エミュレータの警告）。
//! - BIO は CTRL.BIOSEL で選んだ条件を、CLKOUT で 1 段の FF に取ってから入れる [P]（DR-029 (3)）。
//! - 音声クロックの拍はラッチに取る（DR-049）。拍の立上りで 1 にし、DSP が IN 2 を読むと 0 にする。
//!   読んだ値の `STATUS_TICK` に、消す前の値が出る。CTRL.RESET＝1 の間は 0。拍の立上りは、次の
//!   CLKOUT の頭でまとめて取り込む [P]。IN 2 の読出しと同じ CLKOUT の中に来た拍の競合は扱わない [V]。
//! - INT はコマンド到着の間 Low [P]。
//! - 状態ポートのビットと、CPU から見た bridge のレジスタの番地とビットは仕様にない。ここでは
//!   `STATUS_*` の配置を仮に置く [P]（OI-053）。
//! - スロットの中の DSP のサイクルは、そのスロットの Fabric のアクセスより前に済ませる（`audio` と
//!   同じ）[P]。
//! - プログラム RAM の読出しの間に合わなさ（仕様書 §11 のベンド点「DSPの速さ」、DR-066）：番地から
//!   データまでに使える時間は tc − td1 − tsu＝tc − 100 ns（台帳 §2.1。20 MHz で 100 ns）。RAM
//!   （HM6168H-55）と切替回路で 75 ns かかる（仕様書 §05。余裕 25 ns）。CLKIN を上げて使える時間が
//!   足りなくなると、間に合わないデータの線は前の語の値のまま取り込まれる。線ごとの遅れの差を
//!   0〜12 ns に散らし、約 22.9 MHz から 1 本ずつ崩れ、約 24.5 MHz で全部の線が前の語になる [P][V]。
//!   命令のフェッチと TBLR の両方に効く。基板の発振器（20 MHz）では起きない。

use crate::audio::AudioClock;
use crate::fifo::Fifo;
use fc85_chips::tms32010::{self, Tms32010};
use fc85_core::time::{Clock, PERIOD_DSP, Time, UNITS_PER_SECOND};
use serde::{Deserialize, Serialize};

/// プログラム RAM の語数（4K×16）。
pub const PROG_WORDS: usize = 4096;
/// RESET から PRGBUS までの CLKOUT の数（台帳 §2.1 の 2.）。
pub const PRGBUS_DELAY: u8 = 8;

/// IN 2 の状態のビット [P]（OI-053）。
pub const STATUS_INPUT_READY: u16 = 0x0001;
pub const STATUS_OUTPUT_SPACE: u16 = 0x0002;
pub const STATUS_COMMAND: u16 = 0x0004;
pub const STATUS_RESPONSE: u16 = 0x0008;
pub const STATUS_UNDERRUN: u16 = 0x0010;
pub const STATUS_OVERRUN: u16 = 0x0020;
/// 拍のラッチ（DR-049）。IN 2 を読むと 0 になる。
pub const STATUS_TICK: u16 = 0x0040;
/// bit 15〜8：入力 FIFO の語数（0〜64。アダプタのカウンタ）。
pub const STATUS_COUNT_SHIFT: u32 = 8;

/// BIO の発生源（CTRL.BIOSEL[1:0]。DR-029 (3)）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BioSel {
    /// 入力 FIFO が空でない（既定）。
    #[default]
    InputReady,
    /// 出力 FIFO に空きがある。
    OutputSpace,
    /// メールボックスに着信。
    Mailbox,
    /// 音声クロックの拍のラッチ（DR-049）。
    AudioTick,
}

/// DSP bridge の CTRL。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DspCtrl {
    pub reset: bool,
    pub bio: BioSel,
}

impl Default for DspCtrl {
    fn default() -> Self {
        Self {
            reset: true,
            bio: BioSel::InputReady,
        }
    }
}

/// アダプタのラッチと記録。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DspIo {
    /// 68000 → DSP のメールボックス（IN 3）。
    pub mailbox: u16,
    /// コマンド到着（INT）。
    pub command: bool,
    /// DSP → 68000 の応答（OUT 2）。
    pub response: u16,
    /// 応答あり（68000 への要求）。
    pub responded: bool,
    /// UNDERRUN・OVERRUN（STATUS、W1C）。
    pub underrun: bool,
    pub overrun: bool,
    pub underruns: u64,
    pub overruns: u64,
    /// TBLW で 0〜7 番地に書いた回数（OUT と同じになる。DR-029 (2)）。
    pub port_tblw: u64,
    /// 拍のラッチ（DR-049）。
    pub tick: bool,
}

/// 68000 から見た状態（STATUS）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DspStatus {
    pub prgbus: bool,
    pub input_ready: bool,
    pub output_space: bool,
    pub command: bool,
    pub response: bool,
    pub underrun: bool,
    pub overrun: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DspPort {
    pub dsp: Tms32010,
    pub prog: Vec<u16>,
    pub io: DspIo,
    clk: Clock,
    ctrl: DspCtrl,
    /// RS を Low にしているか。
    rs_low: bool,
    /// RESET からの CLKOUT の数（`PRGBUS_DELAY` で止める）。
    reset_cycles: u8,
    prgbus: bool,
    /// BIO の同期化の FF（true で BIO が Low）。
    bio_ff: bool,
    /// ラッチに取り込んだ最後の拍の時刻。
    tick_seen: Option<Time>,
    /// DSP のサイクルの数（RESET の間も数える）。
    pub cycles: u64,
    /// プログラム RAM のデータ線に残っている前の語（間に合わない線はこの値になる）。
    #[serde(default)]
    data_bus: u16,
    /// 間に合わずに前の語の値で取り込んだ読出しの数。
    #[serde(default)]
    pub late_fetches: u64,
}

/// ナノ秒を時刻の単位（1/15.12 GHz）に。
const fn ns(n: Time) -> Time {
    n * UNITS_PER_SECOND / 1_000_000_000
}
/// 番地からデータまでに使えない時間：td1(max)＋tsu(D)＝50＋50 ns（台帳 TMS32010.md §2.1）。
const FETCH_OVERHEAD: Time = ns(100);
/// RAM と切替回路の番地からデータまでの時間（仕様書 §05、DR-029）[P]。
const RAM_ACCESS: Time = ns(75);
/// 線ごとの遅れの差の最大 [P]。
const LINE_SKEW: Time = ns(12);

/// 命令サイクル `tc` のとき、間に合わないデータの線（1 が間に合わない）。
pub fn late_lines(tc: Time) -> u16 {
    let avail = tc.saturating_sub(FETCH_OVERHEAD);
    let mut mask = 0;
    for k in 0..16u16 {
        // 線 k の遅れ：RAM_ACCESS から最大 LINE_SKEW まで早い。並びは線の番号を 5 倍して散らす
        let early = LINE_SKEW * Time::from((k * 5) % 16) / 15;
        if avail < RAM_ACCESS - early {
            mask |= 1 << k;
        }
    }
    mask
}

impl Default for DspPort {
    fn default() -> Self {
        Self::new()
    }
}

impl DspPort {
    /// 電源投入時。RESET＝1、プログラム RAM の中身は 0 [V]。
    pub fn new() -> Self {
        Self {
            dsp: Tms32010::new(),
            prog: vec![0; PROG_WORDS],
            io: DspIo::default(),
            clk: Clock::new(PERIOD_DSP, 0),
            ctrl: DspCtrl::default(),
            rs_low: false,
            reset_cycles: 0,
            prgbus: false,
            bio_ff: false,
            tick_seen: None,
            cycles: 0,
            data_bus: 0,
            late_fetches: 0,
        }
    }

    pub fn ctrl(&self) -> DspCtrl {
        self.ctrl
    }

    /// CTRL の書込み。RS と PRGBUS は次の CLKOUT から変わる。
    pub fn set_ctrl(&mut self, ctrl: DspCtrl) {
        self.ctrl = ctrl;
    }

    /// CLKIN の 4 倍の周期（既定は 20 MHz の 4 倍＝200 ns）。
    pub fn set_input_period(&mut self, period: Time) {
        self.clk.set_period(period);
    }

    pub fn status(&self, input: &Fifo, output: &Fifo) -> DspStatus {
        DspStatus {
            prgbus: self.prgbus,
            input_ready: !input.is_empty(),
            output_space: output.ir(),
            command: self.io.command,
            response: self.io.responded,
            underrun: self.io.underrun,
            overrun: self.io.overrun,
        }
    }

    /// STATUS の UNDERRUN・OVERRUN を消す（W1C）。
    pub fn clear_flags(&mut self, underrun: bool, overrun: bool) {
        self.io.underrun &= !underrun;
        self.io.overrun &= !overrun;
    }

    /// 68000 がメールボックスに書く（コマンド到着）。
    pub fn write_mailbox(&mut self, v: u16) {
        self.io.mailbox = v;
        self.io.command = true;
    }

    /// 68000 が応答を読む（要求を下げる）。
    pub fn read_response(&mut self) -> u16 {
        self.io.responded = false;
        self.io.response
    }

    /// 68000 への要求（応答あり）。割込みのレベルは段階 3 で決める。
    pub fn irq(&self) -> bool {
        self.io.responded
    }

    /// 68000 からプログラム RAM を読む（語の番号）。PRGBUS＝0 ならバスエラーで `None`。
    pub fn prog_read(&self, word: u16) -> Option<u16> {
        self.prgbus
            .then(|| self.prog[usize::from(word) % PROG_WORDS])
    }

    /// 68000 からプログラム RAM に書く。PRGBUS＝0 ならバスエラーで `false`。
    pub fn prog_write(&mut self, word: u16, v: u16) -> bool {
        if self.prgbus {
            self.prog[usize::from(word) % PROG_WORDS] = v;
        }
        self.prgbus
    }

    /// `end` より前の CLKOUT を進める。
    pub fn run(&mut self, end: Time, input: &mut Fifo, output: &mut Fifo, audio: &AudioClock) {
        while let Some(t) = self.clk.take_before(end) {
            self.cycles += 1;
            self.pins();
            if let Some(b) = audio.beat_at()
                && b <= t
                && self.tick_seen != Some(b)
            {
                self.tick_seen = Some(b);
                self.io.tick = true;
            }
            if self.ctrl.reset {
                self.io.tick = false;
            }
            self.bio_ff = match self.ctrl.bio {
                BioSel::InputReady => !input.is_empty(),
                BioSel::OutputSpace => output.ir(),
                BioSel::Mailbox => self.io.command,
                BioSel::AudioTick => self.io.tick,
            };
            let mut bus = Bus {
                prog: &mut self.prog,
                late: late_lines(self.clk.period()),
                data_bus: &mut self.data_bus,
                late_fetches: &mut self.late_fetches,
                input,
                output,
                io: &mut self.io,
                bio_low: self.bio_ff,
            };
            self.dsp.tick(&mut bus);
        }
    }

    /// CLKOUT ごとの RS・PRGBUS・INT。
    fn pins(&mut self) {
        if self.ctrl.reset {
            if !self.rs_low {
                self.rs_low = true;
                self.reset_cycles = 0;
                self.dsp.set_rs(true);
            }
            if self.reset_cycles < PRGBUS_DELAY {
                self.reset_cycles += 1;
                self.prgbus = self.reset_cycles == PRGBUS_DELAY;
            }
        } else if self.prgbus {
            self.prgbus = false;
        } else if self.rs_low {
            self.rs_low = false;
            self.dsp.set_rs(false);
        }
        self.dsp.set_int(!self.io.command);
    }
}

struct Bus<'a> {
    prog: &'a mut [u16],
    /// 間に合わないデータの線。
    late: u16,
    data_bus: &'a mut u16,
    late_fetches: &'a mut u64,
    input: &'a mut Fifo,
    output: &'a mut Fifo,
    io: &'a mut DspIo,
    bio_low: bool,
}

impl Bus<'_> {
    fn status(&self) -> u16 {
        let mut s = (self.input.len() as u16) << STATUS_COUNT_SHIFT;
        for (on, bit) in [
            (!self.input.is_empty(), STATUS_INPUT_READY),
            (self.output.ir(), STATUS_OUTPUT_SPACE),
            (self.io.command, STATUS_COMMAND),
            (self.io.responded, STATUS_RESPONSE),
            (self.io.underrun, STATUS_UNDERRUN),
            (self.io.overrun, STATUS_OVERRUN),
            (self.io.tick, STATUS_TICK),
        ] {
            if on {
                s |= bit;
            }
        }
        s
    }
}

impl tms32010::Bus for Bus<'_> {
    fn read_program(&mut self, addr: u16) -> u16 {
        let v = self.prog[usize::from(addr) % PROG_WORDS];
        let got = (v & !self.late) | (*self.data_bus & self.late);
        if got != v {
            *self.late_fetches += 1;
        }
        // 取り込んだ後、線は RAM の値に落ち着く
        *self.data_bus = v;
        got
    }

    fn write_program(&mut self, addr: u16, data: u16) {
        let a = addr & 0x0FFF;
        if a < 8 {
            self.io.port_tblw += 1;
            self.port_out(a as u8, data);
        } else {
            self.prog[usize::from(a)] = data;
        }
    }

    fn port_in(&mut self, port: u8) -> u16 {
        match port {
            0 => {
                if self.input.is_empty() {
                    self.io.underrun = true;
                    self.io.underruns += 1;
                }
                self.input.shift_out()
            }
            2 => {
                let s = self.status();
                self.io.tick = false;
                s
            }
            3 => {
                self.io.command = false;
                self.io.mailbox
            }
            _ => 0,
        }
    }

    fn port_out(&mut self, port: u8, data: u16) {
        match port {
            1 => {
                if !self.output.shift_in(data) {
                    self.io.overrun = true;
                    self.io.overruns += 1;
                }
            }
            2 => {
                self.io.response = data;
                self.io.responded = true;
            }
            _ => {}
        }
    }

    fn bio_low(&mut self) -> bool {
        self.bio_low
    }
}
