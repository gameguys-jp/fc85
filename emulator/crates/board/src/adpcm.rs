//! ADPCM のアダプタ：MSM5205 と port 5 の FIFO のつなぎ（DR-017、DR-018、DR-035、仕様書 §06・§07）。
//!
//! 模型：
//! - MSM5205 は 384 kHz の発振で進める（core 層の `PERIOD_384K`）。ベンド点のジャンパで外部クロックに
//!   替えられる（DR-035。`set_input_period`）。
//! - V̄C̄K̄ の立上りで、次の取込み（立下りの 6 クロック後。台帳 MSM5205.md §3.2）に使うニブルを D3〜D0 に
//!   置く [P]。byte の 1 つ目のニブルのときに FIFO から 1 byte を取り出す。既定は上位のニブルから、CTRL で
//!   下位からにできる（DR-017 (3)）。
//! - FIFO が空なら 0x08 を使い（ニブル 0000 と 1000 が交互になる）、UNDERRUN を記録する（DR-017 (2)）。
//!   CTRL の「置換しない」（Bend）では、FIFO の出力に残った前の byte を使う（67401 の空の読出し）。
//! - RESET（CTRL の RESET。電源投入時は 1 [P]）の間は FIFO から取り出さず、解除の後は byte の 1 つ目の
//!   ニブルから始める [P]。
//! - 半分空きの割込みは、FIFO の残り（7 bit のカウンタ）が 32 byte 以下の間のレベルとする [P]（DR-017 (4)）。
//! - TRIGGER 5 は V̄C̄K̄ のピンのレベル（DR-018）。
//! - S1＝S2＝H（V̄C̄K̄ が入力になる Bend のモード）では V̄C̄K̄ を駆動しないので、取込みは起きない [V]。
//! - CPU から見たレジスタ（0x230000〜）の番地とビットは段階 3 で決める。ここでは CTRL を構造体の値で持つ。
//! - FIFO の受け渡しは、スロットの頭（Fabric のアクセスの前）にまとめる（`audio` と同じ）[P]。

use crate::fifo::Fifo;
use fc85_chips::msm5205::{Edge, Msm5205};
use fc85_core::time::{Clock, PERIOD_384K, Time};
use serde::{Deserialize, Serialize};

/// FIFO が空のときに使う byte（DR-017 (2)）。
pub const UNDERRUN_BYTE: u8 = 0x08;

/// ADPCM ブリッジの CTRL（台帳 MSM5205.md §4.6 の案）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdpcmCtrl {
    /// MSM5205 の RESET。
    pub reset: bool,
    /// S1・S2（既定は H・L で 8 kHz）。
    pub s1: bool,
    pub s2: bool,
    /// ニブルを下位から使う。
    pub low_first: bool,
    /// FIFO が空のとき 0x08 に置き換えない（Bend）。
    pub no_replace: bool,
    /// 半分空きの割込みを許す。
    pub half_irq: bool,
}

impl Default for AdpcmCtrl {
    fn default() -> Self {
        Self {
            reset: true,
            s1: true,
            s2: false,
            low_first: false,
            no_replace: false,
            half_irq: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdpcmPort {
    pub msm: Msm5205,
    clk: Clock,
    ctrl: AdpcmCtrl,
    /// 今の byte。
    byte: u8,
    /// 次に使うのが byte の 2 つ目のニブルか。
    second: bool,
    /// UNDERRUN（STATUS、W1C）。
    pub underrun: bool,
    /// 空の FIFO から byte を取ろうとした回数。
    pub underruns: u64,
    /// 取込みの数と、最後に取り込んだニブル。
    pub captures: u64,
    pub last_nibble: u8,
}

impl Default for AdpcmPort {
    fn default() -> Self {
        Self::new()
    }
}

impl AdpcmPort {
    pub fn new() -> Self {
        let mut a = Self {
            msm: Msm5205::new(),
            clk: Clock::new(PERIOD_384K, 0),
            ctrl: AdpcmCtrl::default(),
            byte: 0,
            second: false,
            underrun: false,
            underruns: 0,
            captures: 0,
            last_nibble: 0,
        };
        a.set_ctrl(AdpcmCtrl::default());
        a
    }

    pub fn ctrl(&self) -> AdpcmCtrl {
        self.ctrl
    }

    /// CTRL の書込み。
    pub fn set_ctrl(&mut self, ctrl: AdpcmCtrl) {
        self.ctrl = ctrl;
        self.msm.set_reset(ctrl.reset);
        self.msm.set_rate_pins(ctrl.s1, ctrl.s2);
        if ctrl.reset {
            self.second = false;
        }
    }

    /// 発振の周期（既定は 384 kHz）。
    pub fn set_input_period(&mut self, period: Time) {
        self.clk.set_period(period);
    }

    /// PORT_RESET の bit 5：byte の 1 つ目のニブルから始め直す [P]。
    pub fn restart_byte(&mut self) {
        self.second = false;
    }

    /// TRIGGER 5 のレベル（V̄C̄K̄ のピン）。
    pub fn vck(&self) -> bool {
        self.msm.vck()
    }

    /// 半分空きの割込み。
    pub fn irq(&self, fifo: &Fifo) -> bool {
        self.ctrl.half_irq && fifo.len() <= 32
    }

    /// `end` より前の発振の立上りを進める。
    pub fn run(&mut self, end: Time, fifo: &mut Fifo) {
        while self.clk.take_before(end).is_some() {
            let t = self.msm.tick();
            if let Some(c) = t.capture {
                self.captures += 1;
                self.last_nibble = c.nibble;
            }
            if t.edge == Some(Edge::Rise) {
                self.feed(fifo);
            }
        }
    }

    /// V̄C̄K̄ の立上り：次のニブルを置く。
    fn feed(&mut self, fifo: &mut Fifo) {
        if self.ctrl.reset {
            self.second = false;
            return;
        }
        let first = !self.second;
        if first {
            self.byte = if fifo.is_empty() {
                self.underrun = true;
                self.underruns += 1;
                if self.ctrl.no_replace {
                    fifo.shift_out() as u8
                } else {
                    UNDERRUN_BYTE
                }
            } else {
                fifo.shift_out() as u8
            };
        }
        let nibble = if first != self.ctrl.low_first {
            self.byte >> 4
        } else {
            self.byte & 0x0F
        };
        self.msm.set_data(nibble);
        self.second = !self.second;
    }
}
