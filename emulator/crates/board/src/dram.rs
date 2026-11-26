//! DRAM の保持の模型：リフレッシュが遅れたときの崩れ（仕様書 §11 のベンド点、DR-035 (5)、DR-072）。
//!
//! DRAM のセルは電荷で値を持ち、行を活性化する（リフレッシュ、読出し、書込み）たびに書き戻される。
//! 活性化の間が保持時間より長いと、そのセルは放電した値になる。保持時間は資料では最短の保証
//! （4164 は 2 ms、70 °C）しかなく、実際の値は個体・セル・温度でばらつく [V]。
//!
//! 模型 [P]：行のまとまり（リフレッシュの 1 回で活性化される単位）ごとに、最後に活性化した時刻を持つ。
//! 活性化のたびに経過 dt を見て、保持時間が dt より短いセルを放電した値にする。セルの保持時間は分布の
//! 分位点 u（セルの番号と個体の番号の hash）で決め、u が分布の F(dt) より小さければ崩れる。
//! 分布は 25 °C で、主の対数正規（中央 1 s、σ 0.5）と弱いセルの裾（1/1000、中央 200 ms、σ 0.5）の
//! 混合。選別で最短 50 ms（70 °C で 2.2 ms、規格の 2 ms を満たす）を保証する。保持時間は 10 °C ごとに
//! 半分になる。経過が最短より短いとき（通常の動作）は何もしないので、速さに効かない。
//!
//! 追跡は使う側が始める（`Dram::start`）。止めているあいだの活性化は数えない。

use fc85_core::time::{Time, UNITS_PER_SECOND};
use serde::{Deserialize, Serialize};

/// 選別で保証する最短の保持時間（25 °C、秒）[P]。
const T_MIN_S: f64 = 0.05;
/// 主の分布の中央（25 °C、秒）と σ（自然対数）[P][V]。
const MAIN_MEDIAN_S: f64 = 1.0;
const MAIN_SIGMA: f64 = 0.5;
/// 弱いセルの割合、中央（25 °C、秒）、σ [P][V]。
const TAIL_WEIGHT: f64 = 1e-3;
const TAIL_MEDIAN_S: f64 = 0.2;
const TAIL_SIGMA: f64 = 0.5;

/// 個体と温度（Bend の設定。`dram unit N`・`dram temp C`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DramEnv {
    /// 個体の番号。セルの保持時間の並びが変わる。
    pub unit: u32,
    /// 温度（°C）。
    pub temp_c: i32,
}

impl Default for DramEnv {
    fn default() -> Self {
        Self {
            unit: 0,
            temp_c: 25,
        }
    }
}

impl DramEnv {
    /// 経過を 25 °C の経過に直す倍率（保持時間は 10 °C ごとに半分）。
    fn scale(&self) -> f64 {
        2f64.powf(f64::from(self.temp_c - 25) / 10.0)
    }

    /// これより短い経過では、どのセルも崩れない（選別の最短の保持時間）。
    pub fn safe(&self) -> Time {
        (T_MIN_S / self.scale() * UNITS_PER_SECOND as f64) as Time
    }

    /// 経過 dt で崩れるセルの割合（0〜1）。
    pub fn fraction(&self, dt: Time) -> f64 {
        let t = dt as f64 / UNITS_PER_SECOND as f64 * self.scale();
        if t < T_MIN_S {
            return 0.0;
        }
        let main = phi((t / MAIN_MEDIAN_S).ln() / MAIN_SIGMA);
        let tail = phi((t / TAIL_MEDIAN_S).ln() / TAIL_SIGMA);
        (1.0 - TAIL_WEIGHT) * main + TAIL_WEIGHT * tail
    }

    /// チップ `chip` のセル `cell` が、崩れる割合 `p` のときに崩れるか。
    pub fn decays(&self, chip: u32, cell: u32, p: f64) -> bool {
        let key = (u64::from(self.unit) << 40) ^ (u64::from(chip) << 32) ^ u64::from(cell);
        let u = (splitmix64(key) >> 11) as f64 / (1u64 << 53) as f64;
        u < p
    }

    /// 1 byte の各 bit（bit i はチップ `chip0 + i` のセル `cell`）のうち、崩れるものを
    /// `discharged` の同じ bit にする。
    pub fn decay_byte(&self, v: u8, chip0: u32, cell: u32, p: f64, discharged: u8) -> u8 {
        let mut out = v;
        for i in 0..8 {
            if self.decays(chip0 + i, cell, p) {
                let m = 1u8 << i;
                out = (out & !m) | (discharged & m);
            }
        }
        out
    }
}

/// 標準正規分布の累積分布（Abramowitz–Stegun 7.1.26 の erf、誤差 1.5e-7）。
fn phi(x: f64) -> f64 {
    let z = x / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * z.abs());
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-z * z).exp();
    0.5 * (1.0 + if z >= 0.0 { erf } else { -erf })
}

pub(crate) fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 行のまとまりごとの、最後に活性化した時刻。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dram {
    /// 空なら追跡していない。
    last: Vec<Time>,
    groups: usize,
    env: DramEnv,
    /// `env.safe()` の写し（活性化のたびに計算しない）。
    safe: Time,
}

impl Dram {
    /// 行のまとまりが `groups` 個の DRAM。追跡はしていない。
    pub fn new(groups: usize) -> Self {
        let env = DramEnv::default();
        Self {
            last: Vec::new(),
            groups,
            env,
            safe: env.safe(),
        }
    }

    pub fn tracking(&self) -> bool {
        !self.last.is_empty()
    }

    pub fn env(&self) -> DramEnv {
        self.env
    }

    /// 個体と温度を変える。
    pub fn set_env(&mut self, env: DramEnv) {
        self.env = env;
        self.safe = env.safe();
    }

    /// 追跡を始める。どの行も今リフレッシュしたとみなす。追跡中なら何もしない。
    pub fn start(&mut self, now: Time) {
        if !self.tracking() {
            self.last = vec![now; self.groups];
        }
    }

    /// 追跡をやめる。
    pub fn stop(&mut self) {
        self.last = Vec::new();
    }

    /// 行のまとまり `g` を活性化する（リフレッシュ、読出し、書込み）。崩れるセルがあれば、その割合を
    /// 返す。使う側は、そのまとまりのセルを `DramEnv::decay_byte` などで放電した値にしてから読み書きする。
    #[inline]
    pub fn activate(&mut self, g: usize, now: Time) -> Option<f64> {
        let last = self.last.get_mut(g)?;
        let dt = now.saturating_sub(*last);
        *last = now;
        if dt < self.safe {
            return None;
        }
        let p = self.env.fraction(dt);
        (p > 0.0).then_some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: f64) -> Time {
        (x * UNITS_PER_SECOND as f64) as Time
    }

    #[test]
    fn phi_matches_known_values() {
        assert!((phi(0.0) - 0.5).abs() < 1e-7);
        assert!((phi(1.0) - 0.841_344_7).abs() < 1e-6);
        assert!((phi(-2.0) - 0.022_750_1).abs() < 1e-6);
    }

    #[test]
    fn the_distribution_keeps_the_spec_and_decays_when_refresh_stops() {
        let e = DramEnv::default();
        // 25 °C で 50 ms までは崩れない。70 °C でも規格の 2 ms は崩れない
        assert_eq!(e.fraction(s(0.049)), 0.0);
        let hot = DramEnv { temp_c: 70, ..e };
        assert_eq!(hot.fraction(s(0.002)), 0.0);
        assert!(hot.fraction(s(0.003)) > 0.0);
        // 弱いセルの裾から崩れ、1 s で約半分、3 s でほとんど
        let p100 = e.fraction(s(0.1));
        assert!(p100 > 1e-5 && p100 < 1e-3, "{p100}");
        assert!((e.fraction(s(1.0)) - 0.5).abs() < 0.01);
        assert!(e.fraction(s(3.0)) > 0.98);
        // 単調
        let mut prev = 0.0;
        for k in 1..200 {
            let p = e.fraction(s(0.02 * f64::from(k)));
            assert!(p >= prev);
            prev = p;
        }
    }

    #[test]
    fn cells_decay_in_the_given_fraction_and_are_fixed_per_unit() {
        let e = DramEnv::default();
        let n = (0..100_000).filter(|&c| e.decays(0, c, 0.3)).count();
        assert!((n as f64 / 100_000.0 - 0.3).abs() < 0.01, "{n}");
        // 割合が増えると、前に崩れたセルは崩れたまま（保持時間の順）
        for c in 0..10_000 {
            if e.decays(3, c, 0.1) {
                assert!(e.decays(3, c, 0.2));
            }
        }
        // 個体が変わると並びが変わる
        let other = DramEnv { unit: 1, ..e };
        let same = (0..10_000)
            .filter(|&c| e.decays(0, c, 0.5) == other.decays(0, c, 0.5))
            .count();
        assert!(same < 6_000, "{same}");
        assert_eq!(e.decay_byte(0xFF, 0, 7, 1.0, 0x0F), 0x0F);
        assert_eq!(e.decay_byte(0xA5, 0, 7, 0.0, 0x0F), 0xA5);
    }

    #[test]
    fn activation_reports_only_long_gaps_while_tracking() {
        let mut d = Dram::new(4);
        assert_eq!(d.activate(0, s(10.0)), None);
        d.start(s(1.0));
        assert_eq!(d.activate(0, s(1.001)), None);
        assert_eq!(d.activate(1, s(1.040)), None);
        let p = d.activate(2, s(2.0)).unwrap();
        assert!((p - 0.5).abs() < 0.01);
        // 活性化したので、次はそこからの経過
        assert_eq!(d.activate(2, s(2.01)), None);
        d.set_env(DramEnv {
            temp_c: 85,
            ..DramEnv::default()
        });
        assert!(d.activate(3, s(2.1)).is_some());
        d.stop();
        assert!(!d.tracking());
        assert_eq!(d.activate(3, s(9.0)), None);
    }
}
