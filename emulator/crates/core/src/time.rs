//! 仮想機の時刻（仕様書 §12「各クロック領域は整数または有理数の時刻で独立に管理する」）。
//!
//! 4 つの発振源（16 MHz、21.47727 MHz、20 MHz、384 kHz。DR-003）の周期が、すべて整数になる単位を使う。
//! 1 単位は 1/15.12 GHz（約 66.1 ps）。21.47727 MHz は 6fsc＝6 × 315/88 MHz＝945/44 MHz として扱う。
//! Data Fabric のスロット（8 MHz の 2 クロック、250 ns）は 3780 単位。
//! u64 で数えるので、あふれるまで約 1.2×10⁹ 秒ある。

use serde::{Deserialize, Serialize};

/// 時刻（単位の数）。
pub type Time = u64;

/// 1 秒の単位の数。
pub const UNITS_PER_SECOND: Time = 15_120_000_000;
/// Data Fabric のスロット（250 ns）。
pub const SLOT: Time = 3780;
/// 16 MHz（62.5 ns）。
pub const PERIOD_16M: Time = 945;
/// 8 MHz（125 ns）：CPU、Data Fabric、音声クロックの分周器の入力、DOC の CLK。
pub const PERIOD_8M: Time = 1890;
/// 21.47727 MHz（46.56 ns）。
pub const PERIOD_21M: Time = 704;
/// ACRTC の 2CLK とドットクロック（21.47727 MHz÷4＝5.369318 MHz、186.24 ns）。
pub const PERIOD_2CLK: Time = 2816;
/// 20 MHz（50 ns）：DSP の CLKIN。
pub const PERIOD_20M: Time = 756;
/// DSP の命令サイクル（CLKOUT＝20 MHz÷4、200 ns）。
pub const PERIOD_DSP: Time = 3024;
/// 384 kHz（2.604 µs）：MSM5205 の発振。
pub const PERIOD_384K: Time = 39375;
/// DOC のサイクル（E＝8 MHz÷8＝1 MHz、1 µs）。
pub const PERIOD_DOC_E: Time = 15120;

/// 1 つのクロックの立上りの列。周期は途中で変えられる（外部クロックのジャンパ。DR-035）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    period: Time,
    next: Time,
}

impl Clock {
    /// 最初の立上りを `first` に置く。
    pub fn new(period: Time, first: Time) -> Self {
        assert!(period > 0);
        Self {
            period,
            next: first,
        }
    }

    pub fn period(&self) -> Time {
        self.period
    }

    /// 次の立上りの時刻。
    pub fn next(&self) -> Time {
        self.next
    }

    /// 周期を変える。次の立上りの時刻はそのままで、その後から新しい周期で数える。
    pub fn set_period(&mut self, period: Time) {
        assert!(period > 0);
        self.period = period;
    }

    /// 次の立上りの時刻を置き直す（位相を変える）。
    pub fn set_next(&mut self, t: Time) {
        self.next = t;
    }

    /// 次の立上りが `end` より前なら、その時刻を返して次へ進める。
    pub fn take_before(&mut self, end: Time) -> Option<Time> {
        if self.next < end {
            let t = self.next;
            self.next += self.period;
            Some(t)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_are_exact() {
        assert_eq!(UNITS_PER_SECOND / 16_000_000, PERIOD_16M);
        assert_eq!(UNITS_PER_SECOND % 16_000_000, 0);
        assert_eq!(UNITS_PER_SECOND / 20_000_000, PERIOD_20M);
        assert_eq!(UNITS_PER_SECOND / 384_000, PERIOD_384K);
        assert_eq!(UNITS_PER_SECOND % 384_000, 0);
        // 945/44 MHz
        assert_eq!(UNITS_PER_SECOND * 44 / 945_000_000, PERIOD_21M);
        assert_eq!((UNITS_PER_SECOND * 44) % 945_000_000, 0);
        assert_eq!(PERIOD_2CLK, 4 * PERIOD_21M);
        assert_eq!(PERIOD_DSP, 4 * PERIOD_20M);
        assert_eq!(SLOT, 2 * PERIOD_8M);
        assert_eq!(PERIOD_DOC_E, 8 * PERIOD_8M);
        // 1 秒は 4,000,000 スロット
        assert_eq!(UNITS_PER_SECOND / SLOT, 4_000_000);
    }

    #[test]
    fn clock_edges_within_a_slot() {
        let mut c = Clock::new(PERIOD_2CLK, 100);
        let mut edges = Vec::new();
        for k in 0..3 {
            while let Some(t) = c.take_before((k + 1) * SLOT) {
                edges.push((k, t));
            }
        }
        assert_eq!(edges, vec![(0, 100), (0, 2916), (1, 5732), (2, 8548)]);
        c.set_period(PERIOD_384K);
        assert_eq!(c.take_before(u64::MAX), Some(11364));
        assert_eq!(c.next(), 11364 + PERIOD_384K);
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Clock>(&s).unwrap(), c);
    }
}
