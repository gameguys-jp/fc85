//! 同期化の FF とメタステーブル（DR-088）。
//!
//! 別のクロックで変わる線を FF が縁で取るとき、線の変わり目が FF の判定点のすぐ近くに来ると、FF の出力は
//! しばらく決まらない（メタステーブル）。模型：
//! - 判定点：FF は縁の `setup` 前の値を取る。`setup` は系列の標準の値（`Family`）を、個体と温度で
//!   ずらす（入力が上がるときと下がるときで別。`Reg::set_env`）。個体 0・25 °C は標準の値 [P][V]。
//! - 判定点から 1 単位（約 66 ps）以内に線が変わったら、単位より細かい位置を擬似乱数で決めて差 δ を作り、
//!   δ < 0 なら新しい値、そうでなければ前の値を取る。出力が落ち着くまでの縁からの時間は、
//!   τ・ln(T0／2|δ|)（τ＝1/T。T と T0 は TI SDYA006 の表 1 の系列の定数。式 (6) の MTBF の逆）と
//!   普通の遅れの長い方 [D][P]。
//! - 落ち着く前の線を FF が判定点で取ると、値は擬似乱数で、その FF もメタステーブルになる（δ を 1 単位の
//!   中で擬似乱数で決め直す）[P]。読む FF ごとに値が違いうる。
//! - 1 単位の外は普通の FF：判定点の値を取り、普通の遅れの後に出力が変わる。
//!
//! 1 単位より遠い変わり目でも、式の上では普通の遅れより長くかかることがある（LS で約 0.9 ns まで）。
//! それを捨てても、後ろの FF の判定点までの余り（16 MHz の 2 段で 40 ns 以上）を超える確率は変わらない
//! （超えるのは |δ| が 1 単位よりずっと小さいときだけ）。

use crate::dram::{DramEnv, splitmix64};
use fc85_core::time::{Time, UNITS_PER_SECOND};
use serde::{Deserialize, Serialize};

/// 1 ns の時刻の単位の数（15.12）。
const UNITS_PER_NS: f64 = UNITS_PER_SECOND as f64 / 1e9;

/// 判定点の温度の係数（1 °C あたり。TTL の遅れと同じ向きに延びる）[P][V]。
const PER_DEGREE: f64 = 0.003;

/// 判定点の、入力が上がるときと下がるときの違い（標準の値に対する ±）[P][V]。
const EDGE_SKEW: f64 = 0.1;

/// どの判定点よりも長い時間（最も遠い PAL で 20 ns×1.4×1.225（100 °C）＝約 519 単位）。縁のこれより前に
/// 落ち着いた線は、どの FF でも落ち着いた値で取る（速くするための近道）。
pub const QUIET: Time = 1024;

/// 16 MHz の 2 段の同期化で、入力が変わってから、どの段も入力と同じ値に落ち着き、どの判定点からも離れる
/// までの時間（16 MHz の 6 周期）。1 段目が 16 MHz の 1 周期と少し後に取り、LS・PAL のメタステーブル
/// （この模型で最長約 73 ns）を 2 段目が擬似乱数で取っても、その次の縁で取り直して落ち着く（約 4200 単位）。
/// 基板はこの間だけ FF を 1 つずつ見る（速くするための近道）。
pub const SETTLE: Time = 5670;

/// 時刻つきの線。メタステーブルの出力は `from` から `at` まで値が決まらない。保存の形に `from` がなければ
/// `at` と同じ（DR-088 の前の形）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "LineRepr")]
pub struct Line {
    /// 落ち着いた後の値。
    pub level: bool,
    /// `from` より前の値。
    pub prev: bool,
    /// 変わり始めた時刻（メタステーブルでなければ `at` と同じ）。
    pub from: Time,
    /// 落ち着いた時刻。
    pub at: Time,
}

#[derive(Deserialize)]
struct LineRepr {
    level: bool,
    prev: bool,
    #[serde(default)]
    from: Option<Time>,
    at: Time,
}

impl From<LineRepr> for Line {
    fn from(r: LineRepr) -> Self {
        Self {
            level: r.level,
            prev: r.prev,
            from: r.from.unwrap_or(r.at),
            at: r.at,
        }
    }
}

impl Line {
    /// `t` から `level` のまま。
    pub const fn steady(level: bool, t: Time) -> Self {
        Self {
            level,
            prev: level,
            from: t,
            at: t,
        }
    }

    /// 時刻 `t` に `level` へ変わる（同じ値なら何もしない）。変わったら `true`。
    #[inline]
    pub fn set(&mut self, level: bool, t: Time) -> bool {
        if level != self.level {
            self.prev = self.level;
            self.level = level;
            self.from = t;
            self.at = t;
            true
        } else {
            false
        }
    }

    /// 縁 `t` から見て、`QUIET` より前に `level` に落ち着いている（どの FF もそのまま `level` を取る）。
    #[inline]
    pub fn quiet(&self, level: bool, t: Time) -> bool {
        self.level == level && self.at + QUIET <= t
    }

    /// 時刻 `t` の値。落ち着く前なら `None`。
    #[inline]
    pub fn value(&self, t: Time) -> Option<bool> {
        if t >= self.at {
            Some(self.level)
        } else if t < self.from {
            Some(self.prev)
        } else {
            None
        }
    }
}

/// FF の系列。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Family {
    /// 74LS74・74LS175。
    Ls,
    /// 74ALS74。
    Als,
    /// バイポーラの PAL の登録（PAL16R4A-2、PAL20R4A など）。メタステーブルの定数は資料が見つからず、
    /// LS で代わりにする [V]。
    Pal,
}

/// 系列の定数。
struct Consts {
    /// T（1/ns。TI SDYA006 表 1）[D]。
    t_per_ns: f64,
    /// T0（s。同じ表）[D]。
    t0_s: f64,
    /// 判定点の標準の値（縁の前、ns）。資料の最大のセットアップの半分から 6 割ほど [P][V]。
    setup_ns: f64,
    /// 個体のばらつき（標準の値に対する ±）[P][V]。
    spread: f64,
    /// 普通の CLK→Q の遅れ（標準、ns）[P]。
    tco_ns: f64,
}

impl Family {
    fn consts(self) -> Consts {
        match self {
            // 74LS74A・74LS175 の tsu は最大 20 ns。DR-016 の時刻の計算は 10 ns の判定点（host-acrtc-interface.md §7）
            Family::Ls => Consts {
                t_per_ns: 0.74,
                t0_s: 4.8e-3,
                setup_ns: 10.0,
                spread: 0.4,
                tco_ns: 20.0,
            },
            // 74ALS74A の tsu は最大 15 ns
            Family::Als => Consts {
                t_per_ns: 1.0,
                t0_s: 8.7e-6,
                setup_ns: 7.0,
                spread: 0.4,
                tco_ns: 8.0,
            },
            // PAL16R4A-2 の tsu は最大 35 ns、tCO は最大 25 ns
            Family::Pal => Consts {
                t_per_ns: 0.74,
                t0_s: 4.8e-3,
                setup_ns: 20.0,
                spread: 0.3,
                tco_ns: 15.0,
            },
        }
    }

    /// 普通の CLK→Q の遅れ（時刻の単位）。
    pub fn tco(self) -> Time {
        (self.consts().tco_ns * UNITS_PER_NS).round() as Time
    }

    /// 判定点からの差 `delta`（時刻の単位）で入力が変わったとき、縁から出力が落ち着くまでの時間（単位）。
    /// 普通の遅れより短いことがある（呼ぶ側で長い方を取る）。
    pub fn resolution(self, delta: f64) -> Time {
        let c = self.consts();
        let tau = UNITS_PER_NS / c.t_per_ns;
        let t0 = c.t0_s * UNITS_PER_SECOND as f64;
        let d = delta.abs().max(1e-30);
        (tau * (t0 / (2.0 * d)).ln()).max(0.0).round() as Time
    }
}

/// いくつかの FF の判定点の範囲（縁の前の単位。近いものと遠いもの）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub lo: Time,
    pub hi: Time,
}

impl Span {
    pub fn of<'a>(regs: impl IntoIterator<Item = &'a Reg>) -> Self {
        let mut s = Self {
            lo: Time::MAX,
            hi: 0,
        };
        for r in regs {
            s.lo = s.lo.min(r.setup[0].min(r.setup[1]));
            s.hi = s.hi.max(r.max_setup());
        }
        s
    }

    /// 縁 `t` で、範囲のどの判定点でも `d` を落ち着いた値で取る（変わり目が判定点の 1 単位の内になく、
    /// 決まらない間にもない）なら、その値。
    #[inline]
    pub fn clean(&self, t: Time, d: &Line) -> Option<bool> {
        if d.at + self.hi + 1 < t {
            Some(d.level)
        } else if d.from + self.lo > t + 1 {
            Some(d.prev)
        } else {
            None
        }
    }
}

/// 1 つの FF の判定点。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reg {
    family: Family,
    /// FF の番号（擬似乱数と個体のばらつきの鍵）。
    id: u32,
    /// 擬似乱数の鍵（FF の番号と個体）。
    key: u64,
    /// 判定点：縁の何単位前の値を取るか（[入力が下がるとき, 上がるとき]）。
    setup: [Time; 2],
    /// 普通の CLK→Q の遅れ（`Family::tco`）。
    tco: Time,
}

impl Reg {
    /// 個体 0・25 °C の FF。
    pub fn new(family: Family, id: u32) -> Self {
        let mut r = Self {
            family,
            id,
            key: 0,
            setup: [0; 2],
            tco: family.tco(),
        };
        r.set_env(DramEnv::default());
        r
    }

    /// 判定点のうち遠い方（縁の前の単位）。
    pub fn max_setup(&self) -> Time {
        self.setup[0].max(self.setup[1])
    }

    pub fn family(&self) -> Family {
        self.family
    }

    /// 判定点（[入力が下がるとき, 上がるとき]、縁の前の単位）。
    pub fn setup(&self) -> [Time; 2] {
        self.setup
    }

    /// 個体と温度（DR-072 の `dram unit`・`dram temp` と同じ値）から判定点を決める。
    pub fn set_env(&mut self, env: DramEnv) {
        let c = self.family.consts();
        let unit = u64::from(env.unit);
        let id = u64::from(self.id);
        self.key = splitmix64(0x5EC0_0000_0000_0000 ^ (unit << 32) ^ id);
        // [-1, 1) の一様
        let z = |salt: u64| {
            let h = splitmix64(self.key ^ salt);
            (h >> 11) as f64 / (1u64 << 52) as f64 - 1.0
        };
        let (zu, zd) = if env.unit == 0 {
            (0.0, [0.0; 2])
        } else {
            (z(1), [z(2), z(3)])
        };
        let temp = 1.0 + PER_DEGREE * f64::from(env.temp_c - 25);
        for (s, zd) in self.setup.iter_mut().zip(zd) {
            let ns = c.setup_ns * (1.0 + c.spread * zu + EDGE_SKEW * zd) * temp;
            *s = ((ns * UNITS_PER_NS).round() as Time).clamp(1, QUIET - 2);
        }
    }

    /// 縁 `t` で線 `d` を取る。取った値と、縁から出力が落ち着くまでの時間（単位）を返す。
    #[inline]
    pub fn sample(&self, t: Time, d: &Line) -> (bool, Time) {
        if d.quiet(d.level, t) {
            return (d.level, self.tco);
        }
        let dp = t.saturating_sub(self.setup[usize::from(d.level)]);
        let uncertain = d.from <= dp && dp < d.at;
        let near = d.level != d.prev && dp + 1 >= d.at && dp <= d.at + 1;
        if uncertain || near {
            return self.meta(t, d, dp, uncertain);
        }
        (if dp >= d.at { d.level } else { d.prev }, self.tco)
    }

    #[cold]
    #[inline(never)]
    fn meta(&self, t: Time, d: &Line, dp: Time, uncertain: bool) -> (bool, Time) {
        let h = splitmix64(self.key ^ t.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        // (0, 1) の一様
        let u = ((h >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
        let (v, delta) = if uncertain {
            (h & 1 != 0, u - 0.5)
        } else {
            let delta = (d.at as f64 - dp as f64) + u - 0.5;
            (if delta < 0.0 { d.level } else { d.prev }, delta)
        };
        (v, self.family.resolution(delta).max(self.tco))
    }
}

/// 同期化の段：判定点と出力（Q）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncFf {
    pub reg: Reg,
    q: Line,
    /// 普通の遅れより長くかかった回数（メタステーブル）。
    pub meta: u64,
}

impl SyncFf {
    pub fn new(family: Family, id: u32) -> Self {
        Self {
            reg: Reg::new(family, id),
            q: Line::default(),
            meta: 0,
        }
    }

    /// 出力。
    #[inline]
    pub fn q(&self) -> Line {
        self.q
    }

    /// `t` から出力を `level` に落ち着かせる（位相を決め直したとき）。
    pub fn settle(&mut self, level: bool, t: Time) {
        self.q = Line::steady(level, t);
    }

    /// 縁 `t` で、落ち着いた値 `v` を取る（`Span::clean` で確かめた後）。
    #[inline]
    pub fn take(&mut self, t: Time, v: bool) {
        self.q.set(v, t + self.reg.tco);
    }

    /// 縁 `t` で `d` を取る。
    #[inline]
    pub fn clock(&mut self, t: Time, d: &Line) {
        if d.quiet(self.q.level, t) {
            return;
        }
        let (v, out) = self.reg.sample(t, d);
        let tco = self.reg.tco;
        if out > tco {
            self.meta += 1;
            self.q = Line {
                level: v,
                prev: self.q.level,
                from: t,
                at: t + out,
            };
        } else {
            self.q.set(v, t + tco);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_saved_form_without_from_reads_as_a_clean_edge() {
        let l: Line = serde_json::from_str(r#"{"level":true,"prev":false,"at":5000}"#).unwrap();
        assert_eq!(l, {
            let mut n = Line::steady(false, 0);
            n.set(true, 5000);
            n
        });
        let back: Line = serde_json::from_str(&serde_json::to_string(&l).unwrap()).unwrap();
        assert_eq!(back, l);
    }

    #[test]
    fn unit_zero_at_25c_uses_the_nominal_decision_point() {
        // 10 ns＝151.2 単位、7 ns＝105.84、20 ns＝302.4
        assert_eq!(Reg::new(Family::Ls, 1).setup(), [151, 151]);
        assert_eq!(Reg::new(Family::Als, 1).setup(), [106, 106]);
        assert_eq!(Reg::new(Family::Pal, 1).setup(), [302, 302]);
        assert_eq!(Family::Ls.tco(), 302);
        assert_eq!(Family::Als.tco(), 121);
    }

    #[test]
    fn other_units_and_temperatures_move_the_decision_point() {
        let mut seen = std::collections::BTreeSet::new();
        for unit in 1..50 {
            for id in 0..4 {
                let mut r = Reg::new(Family::Ls, id);
                r.set_env(DramEnv { unit, temp_c: 25 });
                for s in r.setup() {
                    // 10 ns × (1 ± 0.4 ± 0.1)
                    assert!((75..=227).contains(&s), "{s}");
                    seen.insert(s);
                }
                let mut hot = r;
                hot.set_env(DramEnv { unit, temp_c: 70 });
                assert!(hot.setup()[0] > r.setup()[0]);
            }
        }
        assert!(seen.len() > 50);
    }

    #[test]
    fn edges_away_from_the_decision_point_are_taken_like_a_plain_ff() {
        let r = Reg::new(Family::Ls, 7);
        let t = 100_000;
        let dp = t - 151;
        for at in [dp - 500, dp - 2, dp + 2, dp + 500] {
            let mut d = Line::steady(false, 0);
            d.set(true, at);
            let (v, out) = r.sample(t, &d);
            assert_eq!(v, at <= dp);
            assert_eq!(out, Family::Ls.tco());
        }
    }

    /// 判定点ちょうどに来た縁：値は半々で、落ち着くまでの時間が 26 ns を超える割合は、窓
    /// T0・e^(−T・26 ns) の 1 単位に対する比（式 (6)）。
    #[test]
    fn coincident_edges_split_evenly_and_resolve_by_the_ti_constants() {
        let r = Reg::new(Family::Ls, 9);
        let n = 200_000;
        let b = (26.0 * UNITS_PER_NS).round() as Time;
        let (mut new, mut late) = (0, 0);
        for k in 0..n {
            let t = 1_000_000 + k * 945;
            let mut d = Line::steady(false, 0);
            d.set(true, t - 151);
            let (v, out) = r.sample(t, &d);
            new += v as u32;
            late += (out > b) as u32;
        }
        let half = f64::from(new) / n as f64;
        assert!((half - 0.5).abs() < 0.01, "{half}");
        let w = 4.8e-3 * (-0.74f64 * 26.0).exp() * UNITS_PER_SECOND as f64;
        let got = f64::from(late) / n as f64;
        assert!((got - w).abs() < 0.01, "{got} {w}");
        // 74ALS74 は判定点ちょうどでも、次の縁の PAL の判定点（42.5 ns 後）より前に落ち着く。超えるのは
        // |δ| が約 2e-14 単位より小さいとき（1 回に約 5e-14）
        let a = Reg::new(Family::Als, 9);
        let pal_budget = 945 - 302;
        let mut worst = 0;
        for k in 0..10_000 {
            let t = 1_000_000 + k * 945;
            let mut d = Line::steady(false, 0);
            d.set(true, t - 106);
            worst = worst.max(a.sample(t, &d).1);
        }
        assert!(worst < pal_budget, "{worst}");
        assert!(Family::Als.resolution(2e-14) > pal_budget);
    }

    #[test]
    fn a_late_first_stage_gives_the_next_stage_a_random_value() {
        let mut a = SyncFf::new(Family::Ls, 1);
        let b = Reg::new(Family::Ls, 2);
        let mut seen = [0; 2];
        for k in 0..2000u64 {
            let t = 10_000 + k * 3000;
            // 落ち着くのが次の縁の判定点より後
            a.q = Line {
                level: true,
                prev: false,
                from: t - 945,
                at: t,
            };
            let (v, out) = b.sample(t, &a.q());
            seen[usize::from(v)] += 1;
            assert!(out >= Family::Ls.tco());
        }
        assert!(seen[0] > 800 && seen[1] > 800, "{seen:?}");
        // 判定点ちょうどの縁は、LS では半単位の差でも約 24.5 ns かかり、普通の遅れ（20 ns）より長い。
        // メタステーブルとして数え、出力は落ち着くまで決まらない
        let mut a = SyncFf::new(Family::Ls, 3);
        for k in 0..1000u64 {
            let t = 50_000 + k * 3000;
            let mut d = Line::steady(k % 2 == 1, 0);
            d.set(k % 2 == 0, t - 151);
            a.clock(t, &d);
            assert_eq!(a.q().value(t + Family::Ls.tco()), None);
        }
        assert_eq!(a.meta, 1000);
    }
}
