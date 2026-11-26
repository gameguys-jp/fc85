//! 機械の外のテレビの模型（仕様書 §11 のベンド点「映像のクロック」の「同期と色が崩れる。テレビが追従
//! できないことがある」）。
//!
//! 機械はコンポジット（NTSC 互換、240p。仕様書 §07・§10、DR-002）を出す。映像系の 21.47727 MHz を替えると、
//! HSYNC（2CLK×342）、VSYNC（262 ライン）、色副搬送波（÷6）が一緒に動く。テレビは、水平の AFC、垂直の
//! 同期、色の発振（3.579545 MHz の水晶）の 3 つでこれに合わせ、それぞれの引込みの範囲を外れると絵が崩れる。
//! テレビは機械の一部ではないので、模型の値はすべて [P][V]（`Tv` の既定値）。状態を持たず、機械の状態
//! （ACRTC のレジスタ、クロックの周期、画面の配列、時刻）から毎回絵を作る。
//!
//! - 水平：ラインの周波数が中心（この機械の工場出荷の 15.6998 kHz に H-HOLD を合わせた [P]）から ±500 Hz の
//!   内なら引き込む。差に比例して絵が横にずれ、端で 4 µs [P][V]。外れると水平の発振は自走し、テレビの
//!   1 ラインに機械の信号の別の時刻が入る：絵は斜めの帯に裂け、帯は流れる。
//! - 垂直：フィールドの周波数が 55〜66 Hz の内なら引き込む（垂直の発振は 55 Hz で自走し、同期はそれより速い
//!   信号だけを引き込む [P][V]）。外れると絵が縦に流れる（帰線の黒い帯が動く）。
//! - 色：色副搬送波と水晶の差が ±500 Hz の内なら引き込み、差に応じて色相がずれる（1 次の PLL の定常の位相差
//!   asin(差/範囲)。端で 90°）[P][V]。外れると位相が唸りの周波数 √(差²−範囲²) で回り、色相が縦に回る縞に
//!   なる。唸りが 300 Hz を超えるとカラーキラーが色を消す [P][V]。水平が外れているときもバーストを取れず
//!   色を消す。
//! - 同期がない（クロックが止まった、STR＝0 で HSYNC が L のまま、ラインがテレビの自走の 10 倍より長い）：
//!   両方の発振が自走し、入力の今の値（出力段が保っている画素。帰線なら黒）の輝度が一様に出る。色は消す [P][V]。
//!
//! 引込みと外れの間の履歴（ヒステリシス）、PLL の過渡、偏向の振幅の変化は作らない。工場出荷のクロックで
//! 案B（HC 171、VC 262）なら、絵は画面の配列（`Video::frame`）をそのまま `rgb888` にしたものと同じ。

use fc85_core::time::{PERIOD_2CLK, PERIOD_21M, UNITS_PER_SECOND};

use crate::console::System;
use crate::video::{RASTER_H, RASTER_W, rgb888};

const UPS: f64 = UNITS_PER_SECOND as f64;

/// テレビの設定（すべて [P][V]）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tv {
    /// 水平の発振の中心（Hz）。
    pub h_center: f64,
    /// 水平の AFC の引込みの範囲（±Hz）。
    pub h_range: f64,
    /// 引込みの端での絵の横のずれ（秒）。
    pub h_shift: f64,
    /// これより長いライン（自走の周期の倍数）は、同期として分けられない。
    pub h_sync_max: f64,
    /// 垂直の発振の自走の周波数と、引き込める上限（Hz）。
    pub v_free: f64,
    pub v_max: f64,
    /// 色の発振の中心（Hz）、引込みの範囲（±Hz）、色を消す唸り（Hz）。
    pub sc_center: f64,
    pub sc_range: f64,
    pub sc_kill: f64,
}

impl Default for Tv {
    fn default() -> Self {
        Self {
            h_center: UPS / (342 * PERIOD_2CLK) as f64,
            h_range: 500.0,
            h_shift: 4e-6,
            h_sync_max: 10.0,
            v_free: 55.0,
            v_max: 66.0,
            sc_center: UPS / (6 * PERIOD_21M) as f64,
            sc_range: 500.0,
            sc_kill: 300.0,
        }
    }
}

/// 水平の同期の状態。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hsync {
    /// 同期がない。
    None,
    /// 外れて自走している。
    Free,
    /// 引き込んだ。絵の横のずれ（ドット、右が正）。
    Locked { shift: i32 },
}

/// 色の状態。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chroma {
    /// 引き込んだ。色相のずれ（ラジアン）。
    Locked { hue: f64 },
    /// 外れて、色相が唸りの周波数（Hz、符号つき）で回る。
    Beat { hz: f64 },
    /// カラーキラーで色を消した。
    Killed,
}

/// テレビが見た信号と、引込みの状態。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    pub h: Hsync,
    pub v_locked: bool,
    pub chroma: Chroma,
    /// 機械のラインとフィールドの周波数、色副搬送波（Hz。止まっていれば 0）。
    pub line_hz: f64,
    pub field_hz: f64,
    pub sc_hz: f64,
}

impl Status {
    /// 1 行で（ホストの表示）。
    pub fn summary(&self) -> String {
        let h = match self.h {
            Hsync::None if self.line_hz == 0.0 => {
                return "テレビ：同期なし（信号が止まった）".into();
            }
            Hsync::None => {
                return format!(
                    "テレビ：同期なし（ライン {:.1} Hz は同期として分けられない）",
                    self.line_hz
                );
            }
            Hsync::Free => "水平が外れた".to_string(),
            Hsync::Locked { shift: 0 } => "水平は同期".to_string(),
            Hsync::Locked { shift } => format!("水平は同期（{shift:+} ドット）"),
        };
        let v = if self.v_locked {
            "垂直は同期"
        } else {
            "垂直が流れる"
        };
        let c = match self.chroma {
            Chroma::Locked { hue: 0.0 } => "色は同期".to_string(),
            Chroma::Locked { hue } => format!("色相 {:+.1}°", hue.to_degrees()),
            Chroma::Beat { hz } => format!("色相が {hz:+.0} Hz で回る"),
            Chroma::Killed => "色が消えた".to_string(),
        };
        format!(
            "テレビ：{h}、{v}、{c}（ライン {:.1} Hz、フィールド {:.2} Hz）",
            self.line_hz, self.field_hz
        )
    }

    /// 工場出荷のクロックと同じに映っているか。
    pub fn normal(&self) -> bool {
        self.h == Hsync::Locked { shift: 0 }
            && self.v_locked
            && self.chroma == Chroma::Locked { hue: 0.0 }
    }
}

/// `$0RGB` を、色相を回し（`cos`、`sin`）色の強さを `gain` 倍して 0RGB の u32 にする（YIQ）。
fn tint(c: u16, cos: f64, sin: f64, gain: f64) -> u32 {
    let [r, g, b] = rgb888(c).map(|v| f64::from(v) / 255.0);
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let i = 0.596 * r - 0.274 * g - 0.322 * b;
    let q = 0.211 * r - 0.523 * g + 0.312 * b;
    let (i, q) = (gain * (i * cos - q * sin), gain * (i * sin + q * cos));
    let px = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (px(y + 0.956 * i + 0.621 * q) << 16)
        | (px(y - 0.272 * i - 0.647 * q) << 8)
        | px(y - 1.106 * i + 1.703 * q)
}

fn plain(c: u16) -> u32 {
    let [r, g, b] = rgb888(c);
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

impl Tv {
    /// 機械の今の信号から、引込みの状態を求める。
    pub fn status(&self, sys: &System) -> Status {
        let a = &sys.ports.acrtc;
        let f21 = sys.video_clock_hz();
        let fr = a.acrtc.frame();
        let dots = u64::from(fr.line).max(1);
        let rasters = u64::from(fr.rasters).max(1);
        let none = Status {
            h: Hsync::None,
            v_locked: false,
            chroma: Chroma::Killed,
            line_hz: 0.0,
            field_hz: 0.0,
            sc_hz: 0.0,
        };
        if f21 == 0.0 || !a.acrtc.display_running() {
            return none;
        }
        let line = (dots * 4) as f64 / f21;
        let line_hz = 1.0 / line;
        let field_hz = line_hz / rasters as f64;
        let sc_hz = f21 / 6.0;
        if line > self.h_sync_max / self.h_center {
            return Status {
                line_hz,
                field_hz,
                sc_hz,
                ..none
            };
        }
        let dh = line_hz - self.h_center;
        let h = if dh.abs() <= self.h_range {
            let dot = 4.0 / f21;
            Hsync::Locked {
                shift: (dh / self.h_range * self.h_shift / dot).round() as i32,
            }
        } else {
            Hsync::Free
        };
        let v_locked = (self.v_free..=self.v_max).contains(&field_hz);
        // 1 mHz より小さい差（浮動小数の誤差）は 0
        let ds = sc_hz - self.sc_center;
        let ds = if ds.abs() < 1e-3 { 0.0 } else { ds };
        let chroma = if !matches!(h, Hsync::Locked { .. }) {
            Chroma::Killed
        } else if ds.abs() <= self.sc_range {
            Chroma::Locked {
                hue: (ds / self.sc_range).asin(),
            }
        } else {
            let beat = (ds * ds - self.sc_range * self.sc_range).sqrt();
            if beat <= self.sc_kill {
                Chroma::Beat {
                    hz: beat.copysign(ds),
                }
            } else {
                Chroma::Killed
            }
        };
        Status {
            h,
            v_locked,
            chroma,
            line_hz,
            field_hz,
            sc_hz,
        }
    }

    /// テレビに映る絵（`RASTER_W`×`RASTER_H`、0RGB の u32）を `out` に書く。並びは `Video::frame` と同じ
    /// （ラインの頭からのドット、VSYNC の始まりからのライン）。
    pub fn picture(&self, sys: &System, out: &mut Vec<u32>) {
        let st = self.status(sys);
        out.clear();
        out.resize(RASTER_W * RASTER_H, 0);
        let a = &sys.ports.acrtc;
        let v = &a.video;
        let f = &v.frame;
        if st.normal() {
            for (o, &c) in out.iter_mut().zip(f.iter()) {
                *o = plain(c);
            }
            return;
        }
        let pixel = |r: usize, d: usize| -> u16 {
            if r < RASTER_H && d < RASTER_W {
                f[r * RASTER_W + d]
            } else {
                0
            }
        };
        let gray = |c: u16| tint(c, 1.0, 0.0, 0.0);
        let p21 = a.video_period();
        let fr = a.acrtc.frame();
        let dots = usize::from(fr.line).max(1);
        let rasters = usize::from(fr.rasters).max(1);
        let now = sys.now() as f64 / UPS;
        let tf = v.frame_at as f64 / UPS;
        match st.h {
            Hsync::None => {
                // 入力の今の値（出力段が保つ画素）の輝度
                let held = if a.acrtc.display_running() {
                    let (d, r) = a.acrtc.beam();
                    pixel(usize::from(r), usize::from(d))
                } else {
                    0
                };
                out.fill(gray(held));
            }
            Hsync::Locked { shift } => {
                let line = (dots as u64 * 4 * p21) as f64 / UPS;
                // テレビの 1 行目の時刻：垂直が同期していればフレームの頭、流れていれば自走の最後のフィールドの頭
                let t0 = if st.v_locked {
                    tf
                } else {
                    let tv0 = 1.0 / self.v_free;
                    ((now / tv0).floor() - 1.0) * tv0
                };
                let vs = ((t0 - tf) / line).floor() as i64;
                for y in 0..RASTER_H {
                    let r = (y as i64 + vs).rem_euclid(rasters as i64) as usize;
                    let (cos, sin, gain) = match st.chroma {
                        Chroma::Locked { hue } => (hue.cos(), hue.sin(), 1.0),
                        Chroma::Beat { hz } => {
                            let t = t0 + y as f64 * line;
                            let ph = (hz * t).fract() * std::f64::consts::TAU;
                            (ph.cos(), ph.sin(), 1.0)
                        }
                        Chroma::Killed => (1.0, 0.0, 0.0),
                    };
                    let plain_row = matches!(st.chroma, Chroma::Locked { hue: 0.0 });
                    for x in 0..RASTER_W {
                        let d = x as i64 - i64::from(shift);
                        let c = if d >= 0 { pixel(r, d as usize) } else { 0 };
                        out[y * RASTER_W + x] = if plain_row {
                            plain(c)
                        } else {
                            tint(c, cos, sin, gain)
                        };
                    }
                }
            }
            Hsync::Free => {
                // 水平の発振は自走：テレビのライン y、横 x の時刻の信号を映す。ラインの頭は自走の周期の格子
                let dot = 4.0 * p21 as f64 / UPS;
                let line = dots as f64 * dot;
                let frame = rasters as f64 * line;
                let th0 = 1.0 / self.h_center;
                let t0 = if st.v_locked {
                    tf
                } else {
                    let tv0 = 1.0 / self.v_free;
                    ((now / tv0).floor() - 1.0) * tv0
                };
                let first = (t0 / th0).floor();
                for y in 0..RASTER_H {
                    let row = (first + y as f64) * th0;
                    for x in 0..RASTER_W {
                        let t = row + x as f64 * th0 / RASTER_W as f64;
                        let u = (t - tf).rem_euclid(frame);
                        let r = (u / line) as usize;
                        let d = ((u - r as f64 * line) / dot) as usize;
                        out[y * RASTER_W + x] = gray(pixel(r, d));
                    }
                }
            }
        }
    }
}
