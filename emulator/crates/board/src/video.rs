//! 映像の出力段：ACRTC が読んだ表示の語を画素にし、パレットで色にする（段階 3）。
//!
//! 仕様書 §05・§07・§10、DR-027（パレット）、DR-053（映像の割込み）、台帳 HD63484.md §3.4・§4.5。
//!
//! 模型：
//! - ACRTC の 2CLK（ドットクロック、5.369318 MHz）ごとに `tick` を呼ぶ。ACRTC が表示の語を読んだ 2CLK
//!   から、その語の 4 画素を LSB の側から 1 画素ずつ出す（台帳 §4.5）。シフトレジスタと DISP の遅れ（DSK）は
//!   扱わない [P]。
//! - 画面は、ラインの頭（HSYNC の始まり）からの 2CLK を横、VSYNC の始まりからのラスタを縦にした
//!   `RASTER_W`×`RASTER_H` の $0RGB の配列。表示期間の外（ブランキング）は黒（0）にする [P]。
//! - パレット（DR-027）：裏と表の 2 面。HSYNC の始まりで裏から表へ 16 語を写す。写しには 3.0 µs かかり、
//!   その間の CPU の書込みは待たされる（待ちは console 層が数える）。写しは HSYNC の始まりで一度に済ませる
//!   [P]。PAL_CTRL の DIRECT では、CPU は表へ直接書く。書込みの間の点状のノイズは作っていない [V]。
//! - 映像の割込み（DR-053）：VBlank（表示のラスタの外）の立上りと、HBlank（表示の横の範囲の外）の
//!   立上りで保留を立てる。HBlank は LINE_CMP を使うと 1 ラインだけ。レベル 6（DR-028）。
//! - Fabric のトリガ 1（VBlank）と 2（HBlank）は、同じレベルを渡す（DR-018）。

use fc85_chips::hd63484::Hd63484;
use fc85_core::time::Time;
use serde::{Deserialize, Serialize};

use crate::acrtc::VramMem;

/// 画面の横（2CLK）。案B の 1 ライン（HC 171×2＝342）を入れる。
pub const RASTER_W: usize = 342;
/// 画面の縦（ラスタ）。案B の 1 フレーム（VC 262）。
pub const RASTER_H: usize = 262;

/// VIDEO_IRQ のビット（DR-053）。
pub const VIRQ_VBL_EN: u16 = 0x0001;
pub const VIRQ_HBL_EN: u16 = 0x0002;
pub const VIRQ_LINE_CMP: u16 = 0x0004;
pub const VIRQ_VBL: u16 = 0x0100;
pub const VIRQ_HBL: u16 = 0x0200;
pub const VIRQ_HBLANK: u16 = 0x4000;
pub const VIRQ_VBLANK: u16 = 0x8000;

fn blank_frame() -> Vec<u16> {
    vec![0; RASTER_W * RASTER_H]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Video {
    /// 裏のパレット（CPU が書く）と、表のパレット（表示が使う）。`$0RGB`。
    pub back: [u16; 16],
    pub front: [u16; 16],
    /// PAL_CTRL bit 0：CPU が表へ直接書く（Bend）。
    pub direct: bool,
    /// シフトレジスタ（LSB の側から出す）と、残りの画素の数。
    shift: u16,
    left: u8,
    /// 画面（`RASTER_W`×`RASTER_H`、$0RGB）。保存には入れない。
    #[serde(skip, default = "blank_frame")]
    pub frame: Vec<u16>,
    /// 終わったフレームの数（ラスタ 0 に戻った回数）。
    pub frames: u64,
    /// VIDEO_IRQ の許可（bit 2–0）と保留（bit 9–8）。
    pub irq_ctrl: u16,
    pub pending: u16,
    pub line_cmp: u16,
    hblank: bool,
    vblank: bool,
    /// 直前の HSYNC の始まりの時刻（パレットの写し、Main RAM のリフレッシュ）。
    pub hsync_at: Time,
    /// 今のラスタ（VSYNC の始まりから）。
    pub raster: u16,
}

impl Default for Video {
    fn default() -> Self {
        Self::new()
    }
}

impl Video {
    pub fn new() -> Self {
        Self {
            back: [0; 16],
            front: [0; 16],
            direct: false,
            shift: 0,
            left: 0,
            frame: blank_frame(),
            frames: 0,
            irq_ctrl: 0,
            pending: 0,
            line_cmp: 0,
            hblank: false,
            vblank: false,
            hsync_at: 0,
            raster: 0,
        }
    }

    /// ACRTC の 2CLK の後（`t` はその立上り）：画素を 1 つ出し、ブランキングと割込みを更新する。
    pub fn tick(&mut self, acrtc: &Hd63484, vram: &VramMem, t: Time) {
        if !acrtc.display_running() {
            // STR＝0：同期も表示も止まる。トリガと割込みのレベルは 0 にする [P]
            self.hblank = false;
            self.vblank = false;
            return;
        }
        let (dot, raster) = acrtc.beam();
        let f = acrtc.frame();
        self.raster = raster;
        if dot == 0 {
            self.hsync_at = t;
            if !self.direct {
                self.front = self.back;
            }
            if raster == 0 {
                self.frames += 1;
            }
        }
        if let Some(a) = acrtc.display_fetch() {
            self.shift = vram.0[(a & 0xFFFF) as usize];
            self.left = 4;
        }
        let hdisp = dot >= f.h_start && dot < f.h_end;
        let vdisp = raster >= f.v_start && raster < f.v_end;
        let color = if hdisp && vdisp {
            let i = if self.left > 0 {
                let i = self.shift & 15;
                self.shift >>= 4;
                self.left -= 1;
                i
            } else {
                0
            };
            self.front[usize::from(i)]
        } else {
            0
        };
        if usize::from(dot) < RASTER_W && usize::from(raster) < RASTER_H {
            self.frame[usize::from(raster) * RASTER_W + usize::from(dot)] = color;
        }
        let hb = !hdisp;
        let vb = !vdisp;
        if hb && !self.hblank {
            let cmp = self.irq_ctrl & VIRQ_LINE_CMP == 0 || raster == self.line_cmp;
            if self.irq_ctrl & VIRQ_HBL_EN != 0 && cmp {
                self.pending |= VIRQ_HBL;
            }
        }
        if vb && !self.vblank && self.irq_ctrl & VIRQ_VBL_EN != 0 {
            self.pending |= VIRQ_VBL;
        }
        self.hblank = hb;
        self.vblank = vb;
    }

    /// VBlank と HBlank のレベル（Fabric のトリガ 1・2）。
    pub fn blank_levels(&self) -> (bool, bool) {
        (self.vblank, self.hblank)
    }

    /// 映像の割込みの要求（ACRTC の IRQ を除く）。
    pub fn irq(&self) -> bool {
        self.pending != 0
    }

    /// CPU のパレットの書込み（語の番号 0〜15）。
    pub fn write_palette(&mut self, n: usize, v: u16) {
        let v = v & 0x0FFF;
        if self.direct {
            self.front[n] = v;
        } else {
            self.back[n] = v;
        }
    }

    /// VIDEO_IRQ の読出し。
    pub fn read_irq(&self) -> u16 {
        (self.irq_ctrl & 7)
            | self.pending
            | if self.hblank { VIRQ_HBLANK } else { 0 }
            | if self.vblank { VIRQ_VBLANK } else { 0 }
    }

    /// VIDEO_IRQ の書込み：bit 2–0 が許可、bit 9–8 は 1 を書くと保留を消す（W1C）。許可を下げると保留も消す。
    pub fn write_irq(&mut self, v: u16) {
        self.irq_ctrl = v & 7;
        self.pending &= !(v & (VIRQ_VBL | VIRQ_HBL));
        if self.irq_ctrl & VIRQ_VBL_EN == 0 {
            self.pending &= !VIRQ_VBL;
        }
        if self.irq_ctrl & VIRQ_HBL_EN == 0 {
            self.pending &= !VIRQ_HBL;
        }
    }
}

/// `$0RGB` を 8 bit の RGB にする（各 4 bit を 17 倍）。DAC とアナログの段は扱わない。
pub fn rgb888(c: u16) -> [u8; 3] {
    let n = |s: u16| ((c >> s) & 15) as u8 * 17;
    [n(8), n(4), n(0)]
}
