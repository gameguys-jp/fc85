//! 本体の音声クロック（TRIGGER 3）と、DOC 子基板がないときの本体 DAC。
//!
//! DR-018、DR-019、DR-035、仕様書 §06（port 2）・§07。
//!
//! 模型：
//! - 音声クロックは、8 MHz を N で割る 12 bit の分周器。入力はベンド点のジャンパで外部に替えられる
//!   （DR-035。`set_input_period`）。N は AUDIO_N（0x250102）の値で、0 は 4096 として数える [V]。
//!   N を書き換えると、次の拍から効く（分周器が一回りしたときに読み込む）[P]。
//! - 拍（TRIGGER 3 の立上り）は、分周器が一回りした入力クロックの立上り。出力は入力の 2 クロック
//!   （8 MHz で 250 ns）High にする（DR-018 の最小パルス幅）[P]。N が 2 以下では High のままで、
//!   トリガの立上りは出ない [V]。
//! - 本体 DAC は、拍ごとに port 2 の FIFO から 1 語を取り込む（上位 byte が L、下位 byte が R）。空なら
//!   直前の値を保ち、UNDERRUN を記録する（DR-019 (2)）。DOC 子基板があるときは FIFO を読まない（DR-019 (3)）。
//! - FIFO の受け渡しは、スロットの中の時刻の順ではなく、スロットの頭（Fabric のアクセスの前）にまとめる。
//!   ずれは 1 スロット（250 ns）以内 [P]。
//! - DAC の CTRL・STATUS・DATA（0x230010〜）の番地とビットは段階 3 で決める。

use crate::fifo::Fifo;
use fc85_core::time::{Clock, PERIOD_8M, Time};
use serde::{Deserialize, Serialize};

/// 音声クロックの分周器（TRIGGER 3、BIO の拍）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioClock {
    clk: Clock,
    /// 今の周期の N。
    n: u16,
    /// 次の周期から使う N（AUDIO_N に書いた値）。
    n_next: u16,
    count: u32,
    /// 出力が High の残りの入力クロック数。
    high: u8,
    /// 最後の拍の時刻。
    beat_at: Option<Time>,
    /// 拍の数。
    pub ticks: u64,
}

impl Default for AudioClock {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioClock {
    /// 電源投入時。N は 0（4096 として数える）[V]。
    pub fn new() -> Self {
        Self {
            clk: Clock::new(PERIOD_8M, 0),
            n: 0,
            n_next: 0,
            count: 0,
            high: 0,
            beat_at: None,
            ticks: 0,
        }
    }

    /// AUDIO_N の書込み（12 bit）。
    pub fn set_n(&mut self, n: u16) {
        self.n_next = n & 0xFFF;
    }

    /// 今の周期の分周比。
    pub fn divisor(&self) -> u32 {
        if self.n == 0 { 4096 } else { u32::from(self.n) }
    }

    /// 入力クロックの周期（既定は 8 MHz）。
    pub fn set_input_period(&mut self, period: Time) {
        self.clk.set_period(period);
    }

    /// TRIGGER 3 のレベル。
    pub fn level(&self) -> bool {
        self.high > 0
    }

    /// 時刻 `t` の TRIGGER 3 のレベル。`run` で進めた範囲の中の時刻に使う（DSP の BIO。DR-029 (3)）。
    pub fn level_at(&self, t: Time) -> bool {
        let pulse = self
            .beat_at
            .is_some_and(|b| t >= b && t < b + 2 * self.clk.period());
        pulse || (self.level() && self.divisor() <= 2)
    }

    /// `end` より前の入力クロックの立上りを進め、拍の数を返す。
    pub fn run(&mut self, end: Time) -> u32 {
        let mut beats = 0;
        while let Some(t) = self.clk.take_before(end) {
            self.high = self.high.saturating_sub(1);
            self.count += 1;
            if self.count >= self.divisor() {
                self.count = 0;
                self.n = self.n_next;
                self.high = 2;
                self.beat_at = Some(t);
                self.ticks += 1;
                beats += 1;
            }
        }
        beats
    }
}

/// 本体 DAC（HA17408×2、8 bit ステレオ。DR-019）の入口のラッチ。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainDac {
    /// DAC に出している語（上位 byte が L、下位 byte が R）。電源投入時は 0 [V]。
    pub latch: u16,
    /// UNDERRUN（STATUS、W1C）。
    pub underrun: bool,
    /// 空の FIFO で拍が来た回数。
    pub underruns: u64,
    /// 拍の数（取り込んだか保ったか）。
    pub beats: u64,
}

impl MainDac {
    pub fn new() -> Self {
        Self::default()
    }

    /// 拍：FIFO から 1 語を取り込む。空なら保つ。
    pub fn beat(&mut self, fifo: &mut Fifo) {
        self.beats += 1;
        if fifo.is_empty() {
            self.underrun = true;
            self.underruns += 1;
        } else {
            self.latch = fifo.shift_out();
        }
    }

    pub fn left(&self) -> u8 {
        (self.latch >> 8) as u8
    }

    pub fn right(&self) -> u8 {
        self.latch as u8
    }
}
