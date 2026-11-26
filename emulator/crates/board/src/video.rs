//! 映像の出力段：ACRTC が読んだ表示の語を画素にし、パレットで色にする（段階 3）。
//!
//! 仕様書 §05・§07・§10、DR-027（パレット）、DR-053（映像の割込み）、台帳 HD63484.md §3.4・§4.5・§4.8。
//!
//! 模型：
//! - ACRTC の 2CLK（ドットクロック、5.369318 MHz）ごとに `tick` を呼ぶ。表示の語は、ACRTC が読んだ 2CLK に
//!   ラッチし、その 4−HSD 後の 2CLK にシフトレジスタへ入れ、LSB の側から 1 画素ずつ出す（台帳 §4.5）。
//!   ACRTC は背景を DISP1 の 1 表示サイクル前から読むので（前読み）、HSD＝0 なら DISP1 の頭から語の画素 0
//!   が出る。HSD は属性（HSYNC の最後のメモリサイクル）でラッチした下位 2 bit で、左へ 0〜3 画素ずらす
//!   （横のスムーズスクロール）[P]。ウィンドウは前読みがない（WSS＝0）ので、DISP2 より 1 表示サイクル
//!   遅れて出る（U85 p.71 から導いた結果）[V]。
//! - 横のズーム：属性の HZ＋1＝z。z 個の表示サイクルのうち最初の語だけをシフトレジスタに入れ、1 画素を
//!   z ドット出す [P]。
//! - 語を読まない表示サイクル（ブランクのウィンドウ、単一アクセスの ACP＝1 で描画が取ったサイクル）は、
//!   0 の語をラッチする。描画のない描画可能サイクルの MAD は 0（U85 p.22 図 2.1 の注）。描画がそのサイクルを
//!   使ったときに MAD に出る番地やデータは扱わない [P]。
//! - ブランキング：DISP1 か DISP2 が出ている間だけ画素を出し、ほかは黒（0）[P]。DSK のずれもそのまま出る。
//! - カーソル：CUD1 か CUD2 が出ている画素は、パレットの番号を反転（XOR $F）する [P]。属性の BLINK1 が
//!   出ている間は、番号の bit 3 を 0 にする（8〜15 の色が 0〜7 になる点滅）[P]。BLINK2・SPL・ATR は使わない。
//! - 画面は、ラインの頭（HSYNC の始まり）からの 2CLK を横、VSYNC の始まりからのラスタを縦にした
//!   `RASTER_W`×`RASTER_H` の $0RGB の配列。
//! - パレット（DR-027）：裏と表の 2 面。HSYNC の始まりから 2CLK に 1 語ずつ、番号の順に裏から表へ写す
//!   （16 語×186 ns＝3.0 µs）[P]。写しの間の CPU の書込みは待たされる（待ちは console 層が数える）。
//!   PAL_CTRL の DIRECT では、CPU は表へ直接書く。書込みのストローブ（68000 の S4〜S7、250 ns）の間は
//!   パレットの番地が CPU の側に切り替わり、表示中の画素は書いている語の新しい色になる（点状のノイズ）。
//!   74LS189 が書込みの間に書いているデータを出すとした [V]。
//! - 映像の割込み（DR-053）：VBlank（背景の縦の範囲の外）の立上りと、HBlank（背景の横の範囲の外）の
//!   立上りで保留を立てる。HBlank は LINE_CMP を使うと 1 ラインだけ。レベル 6（DR-028）。
//! - Fabric のトリガ 1（VBlank）と 2（HBlank）は、同じレベルを渡す（DR-018）。

use fc85_chips::hd63484::Hd63484;
use fc85_core::time::Time;
use serde::{Deserialize, Serialize};

use crate::access::AccessTime;
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

/// 属性の BLINK1（MA18。U85 p.55）。
const ATTR_BLINK1: u32 = 1 << 18;

/// 表示の語を、シフトレジスタに入れるまで持つ。`at` は入れる 2CLK（`Video::clock` の値）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Load {
    at: u32,
    value: u16,
}

/// DIRECT のパレットの書込みのストローブ（時刻は core 層の単位）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Strobe {
    from: Time,
    to: Time,
    n: u8,
    value: u16,
    done: bool,
}

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
    /// 今の画素の残りのドット（横のズーム）。
    #[serde(default)]
    sub: u8,
    /// 読んだ表示の語の、シフトレジスタに入れるまでの列（2 つの輪）と、その頭と数。`clock` は表示が動いている
    /// 間の 2CLK の数。
    #[serde(default)]
    pipe: [Load; 2],
    #[serde(default)]
    head: u8,
    #[serde(default)]
    loads: u8,
    #[serde(default)]
    clock: u32,
    /// 次にシフトレジスタに入れる語までの表示サイクルの数（横のズーム）。
    #[serde(default)]
    group: u8,
    /// ラッチした ACRTC の属性（20 bit）。
    #[serde(default)]
    pub attr: u32,
    /// DIRECT の書込みのストローブ。
    #[serde(default)]
    strobes: Vec<Strobe>,
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
    /// HSYNC の数（Main RAM のリフレッシュ。DR-026）と、その前の HSYNC の始まりの時刻（テレビの模型）。
    #[serde(default)]
    pub lines: u64,
    #[serde(default)]
    pub hsync_prev: Time,
    /// 今のフレームと前のフレームの始まり（ラスタ 0 の HSYNC）の時刻（テレビの模型）。
    #[serde(default)]
    pub frame_at: Time,
    #[serde(default)]
    pub frame_prev: Time,
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
            sub: 0,
            pipe: [Load::default(); 2],
            head: 0,
            loads: 0,
            clock: 0,
            group: 0,
            attr: 0,
            strobes: Vec::new(),
            frame: blank_frame(),
            frames: 0,
            irq_ctrl: 0,
            pending: 0,
            line_cmp: 0,
            hblank: false,
            vblank: false,
            hsync_at: 0,
            raster: 0,
            lines: 0,
            hsync_prev: 0,
            frame_at: 0,
            frame_prev: 0,
        }
    }

    /// ACRTC の 2CLK の後（`t` はその立上り）：画素を 1 つ出し、ブランキングと割込みを更新する。表示の読出しは
    /// VRAM の読出しの間に合わない bit（`read`、DR-083）を通す。
    pub fn tick(&mut self, acrtc: &Hd63484, vram: &VramMem, read: &mut AccessTime, t: Time) {
        let noise = if self.strobes.is_empty() {
            None
        } else {
            self.strobe(t)
        };
        if !acrtc.display_running() {
            // STR＝0：同期も表示も止まる。トリガと割込みのレベルは 0 にする [P]
            self.hblank = false;
            self.vblank = false;
            return;
        }
        let (dot, raster) = acrtc.beam();
        let o = acrtc.display_out();
        self.raster = raster;
        self.clock = self.clock.wrapping_add(1);
        if dot < 16 {
            if dot == 0 {
                self.hsync_prev = self.hsync_at;
                self.hsync_at = t;
                self.lines += 1;
                self.group = 0;
                if raster == 0 {
                    self.frames += 1;
                    self.frame_prev = self.frame_at;
                    self.frame_at = t;
                }
            }
            if !self.direct {
                // 写し：HSYNC の始まりから 2CLK に 1 語ずつ [P]
                self.front[usize::from(dot)] = self.back[usize::from(dot)];
            }
        }
        if let Some(a) = o.attr {
            self.attr = a;
        }
        let fetched = o.fetch.is_some() || o.hole;
        let mut index = 0;
        if self.left | self.loads != 0 || fetched {
            let zoom = ((self.attr >> 12) & 15) as u8 + 1;
            if self.loads > 0 {
                let l = self.pipe[usize::from(self.head)];
                if l.at == self.clock {
                    self.shift = l.value;
                    self.left = 4;
                    self.sub = zoom;
                    self.head ^= 1;
                    self.loads -= 1;
                }
            }
            if self.left > 0 {
                index = self.shift & 15;
                self.sub = self.sub.saturating_sub(1);
                if self.sub == 0 {
                    self.shift >>= 4;
                    self.left -= 1;
                    self.sub = zoom;
                }
            }
            if fetched {
                if self.group == 0 {
                    // 穴（語を読まない表示サイクル）は 0 の語とする [P]
                    let value = o
                        .fetch
                        .map_or(0, |a| read.read(vram.0[(a & 0xFFFF) as usize], || 0xFFFF));
                    let due = 4 - ((self.attr >> 8) & 3);
                    if self.loads < 2 {
                        let n = usize::from((self.head + self.loads) & 1);
                        self.pipe[n] = Load {
                            at: self.clock.wrapping_add(due),
                            value,
                        };
                        self.loads += 1;
                    }
                    self.group = zoom - 1;
                } else {
                    self.group -= 1;
                }
            }
        }
        let color = if o.disp1 || o.disp2 {
            match noise {
                Some(c) => c,
                None => {
                    let mut i = index;
                    if o.cud1 || o.cud2 {
                        i ^= 15;
                    }
                    if self.attr & ATTR_BLINK1 != 0 {
                        i &= 7;
                    }
                    self.front[usize::from(i)]
                }
            }
        } else {
            0
        };
        if usize::from(dot) < RASTER_W && usize::from(raster) < RASTER_H {
            self.frame[usize::from(raster) * RASTER_W + usize::from(dot)] = color;
        }
        let (hdisp, vdisp) = acrtc.display_area();
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

    /// DIRECT の書込み：ストローブの始まりで表に入れ、ストローブの間はその色を返す。
    fn strobe(&mut self, t: Time) -> Option<u16> {
        let mut noise = None;
        for s in &mut self.strobes {
            if t >= s.from {
                if !s.done {
                    self.front[usize::from(s.n)] = s.value;
                    s.done = true;
                }
                if t < s.to {
                    noise = Some(s.value);
                }
            }
        }
        self.strobes.retain(|s| t < s.to);
        noise
    }

    /// VBlank と HBlank のレベル（Fabric のトリガ 1・2）。
    pub fn blank_levels(&self) -> (bool, bool) {
        (self.vblank, self.hblank)
    }

    /// 映像の割込みの要求（ACRTC の IRQ を除く）。
    pub fn irq(&self) -> bool {
        self.pending != 0
    }

    /// CPU のパレットの書込み（語の番号 0〜15）。すぐに入れる（時刻のない書込み）。
    pub fn write_palette(&mut self, n: usize, v: u16) {
        let v = v & 0x0FFF;
        if self.direct {
            self.front[n] = v;
        } else {
            self.back[n] = v;
        }
    }

    /// CPU のパレットの書込みで、書込みのストローブが `from`〜`to` のもの。DIRECT では、表への書込みと
    /// 点状のノイズをストローブの時刻に出す。裏への書込みはすぐに入れる。
    pub fn write_palette_at(&mut self, n: usize, v: u16, from: Time, to: Time) {
        let v = v & 0x0FFF;
        if self.direct {
            self.strobes.push(Strobe {
                from,
                to,
                n: n as u8,
                value: v,
                done: false,
            });
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
