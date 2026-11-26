//! メモリのアクセス時間の模型：クロックのジャンパで上げすぎると、外付けのメモリの読出しが間に合わない
//! （仕様書 §11 のベンド点、DR-035、DR-083）。
//!
//! 対象は、クロックのジャンパの先にある 3 つのメモリ：DOC の波形 RAM（HM4864A-15×8。1 bit が 1 個）、
//! ACRTC の VRAM（HM4864A-12×16。1 bit が 1 個）、DSP のプログラム RAM（HM6168H-55×4。チップ k が
//! bit 4k〜4k＋3）。Main RAM、Staging RAM、Temporal RAM は 16 MHz 系でジャンパがないので入れない。
//!
//! 模型 [P]：
//! - 時間は、メモリの制御がアクセスを始めてから、受け手がデータを取り込むまで。使える時間は「k×周期 −
//!   決まった ns」の形（`Path::windows`。式の出どころは DR-083 と emulator.md §03）。決まった ns は、受け手の
//!   セットアップとつなぎの遅れで、クロックによらない。
//! - チップのアクセス時間は資料の最大×倍率。倍率は（個体、メモリ、チップ）の hash から、25 °C で中央 0.72、
//!   σ 0.06 の対数正規。70 °C で 0.97 を超えないように切る（資料の最大は 0〜70 °C の保証なので）。
//!   温度が 1 °C 上がるごとに 0.3 % 遅くなる [P][V]。個体と温度は DR-072 の `dram unit`・`dram temp` と同じ。
//! - 使える時間から要る時間を引いた余りが −1.5 ns より小さいチップの bit は間に合わない（`fail`）。±1.5 ns の
//!   中は際（`marg`）で、読むたびに擬似乱数で、余りに比例した確率で間に合わない [P]。
//! - 間に合わない bit は「残り」の値になる。何が残るかはメモリごとに使う側が決める（DOC は番地の上位 byte、
//!   VRAM の読出しは 1、VRAM の書込みは前の値、DSP は前のデータバスの値）。
//! - `fail` と `marg` は、周期・個体・温度が変わったときだけ計算する。どちらも 0 なら、1 回のアクセスの手間は
//!   分岐 1 つ。工場出荷のクロックでは、どの個体・温度（0〜70 °C）でも 0。

use crate::dram::{DramEnv, splitmix64};
use fc85_core::time::{Time, UNITS_PER_SECOND};
use serde::{Deserialize, Serialize};

/// 25 °C の倍率の中央と σ（自然対数）[P][V]。
pub const FACTOR_MEDIAN: f64 = 0.72;
pub const FACTOR_SIGMA: f64 = 0.06;
/// 70 °C の倍率の上限（資料の最大は 0〜70 °C で保証する）[P]。
pub const FACTOR_MAX_70: f64 = 0.97;
/// 温度 1 °C あたりの遅れ（MOS）[P][V]。
pub const PER_DEGREE: f64 = 0.003;
/// 際の幅（±ns）[P]。
pub const MARGIN_NS: f64 = 1.5;

/// メモリの読み書きの道。周期は、DOC は CLK、VRAM は 2CLK、DSP は命令サイクル（CLKIN の 4 周期）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Path {
    /// DOC が E Low に波形 RAM を読む。
    Wave,
    /// ACRTC が VRAM を読む（描画、DMA、表示）。
    VramRead,
    /// ACRTC が VRAM に書く。
    VramWrite,
    /// TMS32010 がプログラム RAM を読む（命令のフェッチと TBLR）。
    Prog,
}

/// 使える時間の 1 つの式：k×周期 − offset（ns）と、それに当たる資料の最大（ns）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub name: &'static str,
    pub k: f64,
    pub offset_ns: f64,
    pub max_ns: f64,
}

const fn w(name: &'static str, k: f64, offset_ns: f64, max_ns: f64) -> Window {
    Window {
        name,
        k,
        offset_ns,
        max_ns,
    }
}

/// DOC：CAS̄↓ は取り込みの CLK 2 つ前、RAS̄↓ は 3 つ前。データのセットアップ 100 ns（台帳 ES5503.md §3.5）。
/// HM4864A-15 の tCAC 75、tRAC 150 ns（MEM8505 p.192）。
const WAVE: [Window; 2] = [w("tCAC", 2.0, 100.0, 75.0), w("tRAC", 3.0, 100.0, 150.0)];
/// VRAM の読出し：読みの CAS̄↓ は中央＋40 ns、RAS̄↓ は始まり＋100 ns。データは終わりの tMRDS 40 ns と
/// 読出しの道 10 ns の前（review-digital.md §5.1）。HM4864A-12 の tCAC 60、tRAC 120 ns。
const VRAM_READ: [Window; 2] = [w("tCAC", 1.0, 90.0, 60.0), w("tRAC", 2.0, 150.0, 120.0)];
/// VRAM の書込み：書きの CAS̄↓ は中央＋90 ns、RAS̄↑ はサイクルの終わり。HM4864A-12 の tRSH 60 ns。
const VRAM_WRITE: [Window; 1] = [w("tRSH", 1.0, 90.0, 60.0)];
/// DSP：tc − td1(max) 50 − tsu(D) 50 − 切替 20 ns（台帳 TMS32010.md §3.2）。HM6168H-55 の tAA 55 ns
/// （MEM8505 p.112）。
const PROG: [Window; 1] = [w("tAA", 1.0, 120.0, 55.0)];

impl Path {
    /// 使える時間の式。
    pub fn windows(self) -> &'static [Window] {
        match self {
            Path::Wave => &WAVE,
            Path::VramRead => &VRAM_READ,
            Path::VramWrite => &VRAM_WRITE,
            Path::Prog => &PROG,
        }
    }

    /// チップの数。
    pub fn chips(self) -> u32 {
        match self {
            Path::Wave => 8,
            Path::VramRead | Path::VramWrite => 16,
            Path::Prog => 4,
        }
    }

    /// チップ `chip` がつながる bit。
    pub fn chip_bits(self, chip: u32) -> u16 {
        match self {
            Path::Prog => 0xF << (4 * chip),
            _ => 1 << chip,
        }
    }

    /// 倍率の hash のメモリの番号（VRAM の読みと書きは同じチップ）。
    fn memory(self) -> u64 {
        match self {
            Path::Wave => 1,
            Path::VramRead | Path::VramWrite => 2,
            Path::Prog => 3,
        }
    }
}

/// 標準正規分布の分位点（Abramowitz–Stegun 26.2.23、誤差 4.5e-4）。0 < p < 1。
fn inv_phi(p: f64) -> f64 {
    let q = if p < 0.5 { p } else { 1.0 - p };
    let t = (-2.0 * q.ln()).sqrt();
    let x = t
        - (2.515_517 + t * (0.802_853 + t * 0.010_328))
            / (1.0 + t * (1.432_788 + t * (0.189_269 + t * 0.001_308)));
    if p < 0.5 { -x } else { x }
}

fn temp_scale(temp_c: i32) -> f64 {
    1.0 + PER_DEGREE * f64::from(temp_c - 25)
}

/// チップのアクセス時間の、資料の最大に対する倍率。
pub fn chip_factor(path: Path, chip: u32, env: DramEnv) -> f64 {
    let key = splitmix64(0xACCE_55ED_0000_0000 ^ u64::from(env.unit))
        ^ (path.memory() << 8)
        ^ u64::from(chip);
    let u = ((splitmix64(key) >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let f25 =
        (FACTOR_MEDIAN * (FACTOR_SIGMA * inv_phi(u)).exp()).min(FACTOR_MAX_70 / temp_scale(70));
    f25 * temp_scale(env.temp_c)
}

/// 周期 `period`（時刻の単位）のときの、チップ `chip` の余り（ns。負なら間に合わない）。
pub fn slack_ns(path: Path, period: Time, chip: u32, env: DramEnv) -> f64 {
    let t = period as f64 * 1e9 / UNITS_PER_SECOND as f64;
    let f = chip_factor(path, chip, env);
    path.windows()
        .iter()
        .map(|w| w.k * t - w.offset_ns - w.max_ns * f)
        .fold(f64::INFINITY, f64::min)
}

/// 1 つの道の、間に合わない bit と際の bit。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessTime {
    /// 間に合わない bit。
    fail: u16,
    /// 際の bit。
    marg: u16,
    /// 際の bit k が残りの値になる確率（1/256）。
    odds: [u8; 16],
    /// 際の bit を含むアクセスの数（擬似乱数の列の番号）。
    count: u64,
    /// 擬似乱数の種（個体と道）。
    seed: u64,
}

impl AccessTime {
    /// 周期 `period`（`Path` の周期、時刻の単位。0 は止まっている）と、個体と温度 `env` から、間に合わない
    /// bit と際の bit を決める。擬似乱数の列は続ける。
    pub fn set(&mut self, path: Path, period: Time, env: DramEnv) {
        self.fail = 0;
        self.marg = 0;
        self.odds = [0; 16];
        self.seed = splitmix64(
            (u64::from(env.unit) << 8) ^ (path.memory() << 4) ^ (path == Path::VramWrite) as u64,
        );
        if period == 0 {
            return;
        }
        for chip in 0..path.chips() {
            let s = slack_ns(path, period, chip, env);
            let bits = path.chip_bits(chip);
            if s < -MARGIN_NS {
                self.fail |= bits;
            } else if s <= MARGIN_NS {
                self.marg |= bits;
                let p = (MARGIN_NS - s) / (2.0 * MARGIN_NS);
                let odds = (p * 256.0).round().clamp(1.0, 255.0) as u8;
                for k in 0..16 {
                    if bits & (1 << k) != 0 {
                        self.odds[k] = odds;
                    }
                }
            }
        }
    }

    /// 間に合わない bit と際の bit。
    pub fn masks(&self) -> (u16, u16) {
        (self.fail, self.marg)
    }

    /// 間に合わない bit か際の bit がある。
    #[inline]
    pub fn bits(&self) -> u16 {
        self.fail | self.marg
    }

    /// メモリの出す値 `v` を、間に合わない bit だけ残り（`rest`）にして返す。書込みでは `v` が書く値、
    /// `rest` が前の値。
    #[inline]
    pub fn read(&mut self, v: u16, rest: impl FnOnce() -> u16) -> u16 {
        if self.fail | self.marg == 0 {
            return v;
        }
        self.bend(v, rest())
    }

    /// 8 bit のメモリの `read`。
    #[inline]
    pub fn read8(&mut self, v: u8, rest: impl FnOnce() -> u8) -> u8 {
        if self.fail | self.marg == 0 {
            return v;
        }
        self.bend(u16::from(v), u16::from(rest())) as u8
    }

    #[cold]
    #[inline(never)]
    fn bend(&mut self, v: u16, rest: u16) -> u16 {
        let mut m = self.fail;
        if self.marg != 0 {
            let n = self.count.wrapping_mul(2);
            let h = [
                splitmix64(self.seed ^ n),
                splitmix64(self.seed ^ n.wrapping_add(1)),
            ];
            self.count = self.count.wrapping_add(1);
            let mut bits = self.marg;
            while bits != 0 {
                let k = bits.trailing_zeros() as usize;
                let r = (h[k / 8] >> (8 * (k % 8))) as u8;
                if r < self.odds[k] {
                    m |= 1 << k;
                }
                bits &= bits - 1;
            }
        }
        (v & !m) | (rest & m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fc85_core::time::{PERIOD_2CLK, PERIOD_8M, PERIOD_DSP};

    fn hz(path: Path, f: f64) -> Time {
        // 周期は DOC が CLK、VRAM が 2CLK（映像系の 1/4）、DSP が CLKIN の 4 周期
        let k = match path {
            Path::Wave => 1.0,
            Path::VramRead | Path::VramWrite | Path::Prog => 4.0,
        };
        (k * UNITS_PER_SECOND as f64 / f).round() as Time
    }

    #[test]
    fn inv_phi_matches_known_values() {
        assert!(inv_phi(0.5).abs() < 1e-3);
        assert!((inv_phi(0.841_344_7) - 1.0).abs() < 1e-3);
        assert!((inv_phi(0.022_750_1) + 2.0).abs() < 1e-3);
    }

    #[test]
    fn factors_have_the_median_and_keep_the_datasheet_at_70_c() {
        let mut v: Vec<f64> = (0..2000)
            .map(|u| {
                chip_factor(
                    Path::Wave,
                    u % 8,
                    DramEnv {
                        unit: u / 8,
                        temp_c: 25,
                    },
                )
            })
            .collect();
        v.sort_by(f64::total_cmp);
        assert!((v[1000] - FACTOR_MEDIAN).abs() < 0.01, "{}", v[1000]);
        // σ 0.06：84 % 点は中央×e^0.06
        assert!((v[1682] / v[1000] - 0.06f64.exp()).abs() < 0.01);
        for unit in 0..256 {
            for chip in 0..16 {
                let f = chip_factor(Path::VramRead, chip, DramEnv { unit, temp_c: 70 });
                assert!(f <= FACTOR_MAX_70 + 1e-12);
            }
        }
        // 温度：25 °C から 70 °C で 13.5 % 遅く
        let e = DramEnv::default();
        let hot = DramEnv { temp_c: 70, ..e };
        let r = chip_factor(Path::Prog, 1, hot) / chip_factor(Path::Prog, 1, e);
        assert!((r - 1.135).abs() < 1e-9);
    }

    #[test]
    fn factory_periods_have_no_failing_bits() {
        for unit in 0..64 {
            for temp_c in [0, 25, 70] {
                let env = DramEnv { unit, temp_c };
                for (path, p) in [
                    (Path::Wave, PERIOD_8M),
                    (Path::VramRead, PERIOD_2CLK),
                    (Path::VramWrite, PERIOD_2CLK),
                    (Path::Prog, PERIOD_DSP),
                ] {
                    let mut a = AccessTime::default();
                    a.set(path, p, env);
                    assert_eq!(a.bits(), 0, "{path:?} {unit} {temp_c}");
                }
            }
        }
    }

    #[test]
    fn failing_bits_grow_with_the_clock() {
        let env = DramEnv::default();
        for path in [Path::Wave, Path::VramRead, Path::VramWrite, Path::Prog] {
            let mut prev = 0u16;
            for k in 0..400 {
                let f = 5e6 + 0.25e6 * f64::from(k);
                let mut a = AccessTime::default();
                a.set(path, hz(path, f), env);
                let (fail, _) = a.masks();
                assert_eq!(fail & prev, prev, "{path:?} {f}");
                prev = fail;
            }
            // 約 105 MHz ではすべての bit が間に合わない
            let all = (0..path.chips()).fold(0, |m, c| m | path.chip_bits(c));
            assert_eq!(prev, all, "{path:?}");
        }
    }

    #[test]
    fn residual_values_replace_the_failing_bits() {
        let mut a = AccessTime::default();
        a.set(Path::Wave, hz(Path::Wave, 40e6), DramEnv::default());
        assert_eq!(a.masks(), (0xFF, 0));
        assert_eq!(a.read8(0x80, || 0x12), 0x12);
        let mut b = AccessTime::default();
        b.set(Path::Prog, 0, DramEnv::default());
        assert_eq!(b.read(0x1234, || unreachable!()), 0x1234);
    }
}
