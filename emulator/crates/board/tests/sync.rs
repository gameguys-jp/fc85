//! 同期化の FF とメタステーブル（DR-088、`fc85_board::sync`）の基板の試験：DOC の E の同期化。
//! ACRTC のブリッジの段の数は acrtc_port.rs。

use fc85_board::doc::DocBoard;
use fc85_board::dram::DramEnv;
use fc85_core::time::{PERIOD_8M, SLOT, Time};

/// DOC の CLK を `clk`（時刻の単位）にして `slots` スロット進める。
fn run_doc(env: DramEnv, skip_first: bool, clk: Time, slots: u64) -> DocBoard {
    let mut d = DocBoard::new();
    d.set_clock_period(clk);
    d.set_sync(env, skip_first);
    for k in 0..slots {
        d.run_slot(k * SLOT, None);
    }
    d
}

const UNIT3: DramEnv = DramEnv {
    unit: 3,
    temp_c: 25,
};

/// 基板のクロック（16 MHz の 2 分周）では、E の縁は 16 MHz の縁に揃い、判定点から離れている。
#[test]
fn the_board_clock_keeps_e_away_from_the_decision_points() {
    for skip in [false, true] {
        let d = run_doc(UNIT3, skip, PERIOD_8M, 100_000);
        assert_eq!(d.sync_splits, 0, "1 段目を飛ばす {skip}");
        assert_eq!(d.sync_meta(), [0, 0], "1 段目を飛ばす {skip}");
    }
}

/// 外部のクロック（約 8.07 MHz。16 MHz と非同期）でも、2 段なら E の立下りを使う 3 つの登録は食い違わない。
/// 74ALS74 の 1 段目を飛ばすと、P2A の登録が E をそのまま取り、判定点の個体差の間に E が落ちると食い違う。
#[test]
fn skipping_the_74als74_lets_the_p2a_registers_disagree_on_an_external_clock() {
    let clk = 1873;
    let slots = 200_000;
    for env in [DramEnv::default(), UNIT3] {
        let d = run_doc(env, false, clk, slots);
        assert_eq!(d.sync_splits, 0, "{env:?}");
        assert_eq!(d.sync_meta()[1], 0, "{env:?}");
    }
    let unit0 = run_doc(DramEnv::default(), true, clk, slots).sync_splits;
    let unit3 = run_doc(UNIT3, true, clk, slots).sync_splits;
    // 個体 0 は 3 つの判定点が同じで、食い違うのは判定点ちょうどに縁が来たとき（E の立下り約 5 万回の
    // 1/945 ほど）だけ。個体 3 は判定点が数 ns ずれ、その間に落ちた分が食い違う（約 7200 回）
    assert!(unit0 < 100, "{unit0}");
    assert!(unit3 > 10 * unit0.max(1), "{unit0} {unit3}");
}
