//! HD63484 ACRTC の単体試験（仕様書 §12 段階 1：描画・COPY・FIFO）。
//! 期待値は台帳 `docs/parts/HD63484.md` の追補 2・追補 3（U85 の記述と実行例）から取る。「(n)」は追補 2 の節。

use std::collections::BTreeSet;

use fc85_chips::hd63484::{
    DisplayOut, Hd63484, SR_ARD, SR_CED, SR_CER, SR_RFF, SR_RFR, SR_WFE, SR_WFR, Vram,
};

// ---- コマンド（(9) の表、U85 p.173） ----

const ORG: u16 = 0x0400;
const WPR: u16 = 0x0800;
const RPR: u16 = 0x0C00;
const WPTN: u16 = 0x1800;
const RPTN: u16 = 0x1C00;
const DRD: u16 = 0x2400;
const DWT: u16 = 0x2800;
const DMOD: u16 = 0x2C00;
const RD: u16 = 0x4400;
const WT: u16 = 0x4800;
const MOD: u16 = 0x4C00;
const CLR: u16 = 0x5800;
const SCLR: u16 = 0x5C00;
const CPY: u16 = 0x6000;
const AMOVE: u16 = 0x8000;
const RMOVE: u16 = 0x8400;
const ALINE: u16 = 0x8800;
const ARCT: u16 = 0x9000;
const APLL: u16 = 0x9800;
const RPLL: u16 = 0x9C00;
const APLG: u16 = 0xA000;
const CRCL: u16 = 0xA800;
const ELPS: u16 = 0xAC00;
const AARC: u16 = 0xB000;
const RARC: u16 = 0xB400;
const AEARC: u16 = 0xB800;
const REARC: u16 = 0xBC00;
const AFRCT: u16 = 0xC000;
const RFRCT: u16 = 0xC400;
const PAINT: u16 = 0xC800;
const DOT: u16 = 0xCC00;
const PTN: u16 = 0xD000;
const AGCPY: u16 = 0xE000;
const RGCPY: u16 = 0xF000;

// 描画パラメータレジスタの番号（(2)）
const CL0: u16 = 0x00;
const CL1: u16 = 0x01;
const CCMP: u16 = 0x02;
const MASK: u16 = 0x04;
const PRC0: u16 = 0x05;
const PRC2: u16 = 0x07;
const XMIN: u16 = 0x08;
const RWPH: u16 = 0x0C;
const RWPL: u16 = 0x0D;

// コマンド語の下位 8 bit（AREA、COL、OPM）
/// 円・楕円・弧の C＝1（時計回り）
const CW: u16 = 0x0100;
const COL01: u16 = 0x08;
const COL10: u16 = 0x10;
const COL11: u16 = 0x18;

fn neg(v: i16) -> u16 {
    v as u16
}

/// 1 M 語のフレームバッファ。
struct Mem(Vec<u16>);

impl Vram for Mem {
    fn read(&mut self, addr: u32) -> u16 {
        self.0[addr as usize]
    }
    fn write(&mut self, addr: u32, data: u16) {
        self.0[addr as usize] = data;
    }
}

struct Rig {
    a: Hd63484,
    m: Mem,
    mw: u32,
    org: u32,
}

impl Rig {
    /// 4 bit/画素、画面 0 の MW、STR＝1。
    fn new(mw: u16) -> Rig {
        let mut r = Rig {
            a: Hd63484::new(),
            m: Mem(vec![0; 1 << 20]),
            mw: u32::from(mw),
            org: 0,
        };
        r.reg(0x02, 0x0200); // CCR：ABT＝0、GBM＝010（4 bit/画素）
        r.reg(0xC2, mw); // MWR0（上画面）
        r.reg(0x04, 0xC000); // OMR：M/S＝1、STR＝1
        r
    }

    /// 原点を語 `org` の画素 0 に置いた描画用の構成。
    fn drawing(mw: u16, org: u32) -> Rig {
        let mut r = Rig::new(mw);
        r.run(&[ORG, (org >> 12) as u16, ((org & 0xFFF) << 4) as u16]);
        r.org = org;
        r
    }

    fn reg(&mut self, ar: u16, v: u16) {
        self.a.write(false, ar);
        self.a.write(true, v);
    }

    /// 書込み FIFO に書く。満杯なら空くまで進める。
    fn put(&mut self, words: &[u16]) {
        self.a.write(false, 0);
        for &w in words {
            while self.a.status() & SR_WFR == 0 {
                self.a.tick(&mut self.m);
            }
            self.a.write(true, w);
        }
    }

    fn finish(&mut self) {
        for _ in 0..10_000_000 {
            if self.a.idle() {
                return;
            }
            self.a.tick(&mut self.m);
        }
        panic!("コマンドが終わらない");
    }

    /// コマンドを書き、終わるまで進めて、かかった 2CLK の数を返す。
    fn run(&mut self, words: &[u16]) -> u64 {
        let start = self.a.cycles();
        self.put(words);
        self.finish();
        self.a.cycles() - start
    }

    fn wpr(&mut self, rn: u16, v: u16) {
        self.run(&[WPR | rn, v]);
    }

    fn set_rwp(&mut self, dn: u16, addr: u32) {
        self.wpr(RWPH, (dn << 14) | (addr >> 12) as u16);
        self.wpr(RWPL, ((addr & 0xFFF) << 4) as u16);
    }

    fn fifo(&mut self) -> u16 {
        self.a.write(false, 0);
        self.a.read(true)
    }

    /// 論理座標の画素（原点は語 `org` の画素 0、4 bit/画素）。
    fn pix(&self, x: i32, y: i32) -> u16 {
        let addr = self.org as i64 + i64::from(x.div_euclid(4)) - i64::from(y) * i64::from(self.mw);
        (self.m.0[addr as usize] >> (x.rem_euclid(4) * 4)) & 0xF
    }

    fn set_pix(&mut self, x: i32, y: i32, v: u16) {
        let addr = (self.org as i64 + i64::from(x.div_euclid(4))
            - i64::from(y) * i64::from(self.mw)) as usize;
        let sh = x.rem_euclid(4) * 4;
        self.m.0[addr] = (self.m.0[addr] & !(0xF << sh)) | (v << sh);
    }

    /// 0 でない画素の座標を数える。
    fn lit(&self, xs: std::ops::RangeInclusive<i32>, ys: std::ops::RangeInclusive<i32>) -> usize {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| self.pix(x, y) != 0)
            .count()
    }
}

// ---- ホストとのやり取り（(1)） ----

#[test]
fn reset_state() {
    let mut a = Hd63484::new();
    assert_eq!(a.status(), SR_CED | SR_WFR | SR_WFE, "RES の後は $23");
    assert_eq!(a.read(false), 0xFF23);
    a.write(false, 0x02);
    assert_eq!(a.read(true), 0x8000, "CCR は ABT＝1");
}

#[test]
fn address_register_increments_only_from_r80() {
    let mut r = Rig::new(0x10);
    r.a.write(false, 0x82);
    for v in [0xAA0D, 0x127F, 0x0106] {
        r.a.write(true, v);
    }
    r.a.write(false, 0x82);
    let back: Vec<u16> = (0..3).map(|_| r.a.read(true)).collect();
    assert_eq!(back, [0xAA0D, 0x127F, 0x0106]);
    // r00〜r7F では進まない
    r.a.write(false, 0x06);
    r.a.write(true, 0x1234);
    r.a.write(true, 0x5678);
    assert_eq!(r.a.read(true), 0x5678);
    assert_eq!(r.a.read(true), 0x5678);
}

#[test]
fn fifo_flags_follow_the_contents() {
    let mut r = Rig::new(0x10);
    r.reg(0x04, 0x8000); // STR＝0：コマンドを実行しない
    r.a.write(false, 0);
    r.a.write(true, WPR);
    assert_eq!(r.a.status(), SR_WFR, "書込みで WFE と CED が 0");
    for _ in 0..7 {
        r.a.write(true, 0);
    }
    assert_eq!(r.a.status(), 0, "8 語で WFR も 0");
    r.a.write(true, 0);
    assert_eq!(r.a.error_counts().0, 1, "満杯の FIFO に書いた");
}

#[test]
fn abort_clears_the_fifos_and_sets_sr_23() {
    let mut r = Rig::new(0x10);
    r.reg(0x04, 0x8000);
    r.put(&[WPR | CL0, 0x1234, RPR]);
    r.reg(0x02, 0x8200);
    assert_eq!(r.a.status(), 0x23);
    r.reg(0x02, 0x0200);
    r.reg(0x04, 0xC000);
    r.finish();
    assert_eq!(r.a.param(0), 0, "打ち切ったコマンドは実行しない");
}

#[test]
fn str_zero_and_pause_hold_the_command() {
    let mut r = Rig::new(0x10);
    r.reg(0x04, 0x8000);
    r.put(&[WPR | CL0, 0x1234]);
    for _ in 0..50 {
        r.a.tick(&mut r.m);
    }
    assert_eq!(r.a.param(0), 0, "STR＝0 では描画も止まる（U85 p.70）");
    r.reg(0x02, 0x4200); // PSE＝1
    r.reg(0x04, 0xC000);
    for _ in 0..50 {
        r.a.tick(&mut r.m);
    }
    assert_eq!(r.a.param(0), 0, "PSE＝1 の間は止まる（U85 p.66）");
    r.reg(0x02, 0x0200);
    r.finish();
    assert_eq!(r.a.param(0), 0x1234);
}

#[test]
fn undefined_command_sets_cer_until_abort() {
    let mut r = Rig::new(0x10);
    r.reg(0x02, 0x0280); // CRE＝1
    r.put(&[0x0000, WPR | CL0, 0x1234]);
    for _ in 0..50 {
        r.a.tick(&mut r.m);
    }
    assert_eq!(r.a.status() & SR_CER, SR_CER);
    assert!(r.a.irq());
    assert_eq!(r.a.param(0), 0, "CER の後は実行しない [V]");
    r.reg(0x02, 0x8200);
    assert_eq!(r.a.status(), 0x23, "CER は ABT で 0");
    assert!(!r.a.irq());
}

#[test]
fn command_end_interrupt() {
    let mut r = Rig::new(0x10);
    r.reg(0x02, 0x0220); // CEE＝1
    assert!(r.a.irq(), "RES の後は CED＝1");
    r.put(&[WPR | CL0, 0x1234]);
    assert!(!r.a.irq(), "コマンドを書くと CED＝0");
    r.finish();
    assert!(r.a.irq());
}

// ---- レジスタのコマンド（(2)、(4)） ----

#[test]
fn wpr_and_rpr_take_6_cycles() {
    // U85 p.178、p.180 の例：CL1 に $1111
    let mut r = Rig::new(0x10);
    assert_eq!(r.run(&[WPR | CL1, 0x1111]), 6);
    assert_eq!(r.run(&[RPR | CL1]), 6);
    assert_eq!(r.a.status() & SR_RFR, SR_RFR);
    assert_eq!(r.fifo(), 0x1111);
    assert_eq!(r.a.status() & SR_RFR, 0);
}

#[test]
fn wptn_and_rptn_example() {
    // U85 p.182、p.184 の例：番地 $B から $2314、$5713
    let mut r = Rig::new(0x10);
    assert_eq!(r.run(&[WPTN | 0xB, 2, 0x2314, 0x5713]), 4 * 2 + 8);
    assert_eq!((r.a.pattern(11), r.a.pattern(12)), (0x2314, 0x5713));
    assert_eq!(r.run(&[RPTN | 0xB, 2]), 4 * 2 + 10);
    assert_eq!((r.fifo(), r.fifo()), (0x2314, 0x5713));
}

#[test]
fn read_fifo_full_makes_the_command_wait() {
    let mut r = Rig::new(0x10);
    r.wpr(CL0, 0x0A0A);
    for _ in 0..8 {
        r.run(&[RPR | CL0]);
    }
    assert_eq!(r.a.status() & SR_RFF, SR_RFF);
    r.put(&[RPR | CL0]);
    for _ in 0..100 {
        r.a.tick(&mut r.m);
    }
    assert!(!r.a.idle(), "読出し FIFO が空くまで待つ");
    r.fifo();
    r.finish();
    let rest: Vec<u16> = (0..8).map(|_| r.fifo()).collect();
    assert_eq!(rest, [0x0A0A; 8]);
}

#[test]
fn org_example_sets_the_origin_and_clears_cp() {
    // U85 p.176 の例：語 $25 の bit 7–4（4 bit/画素、MW＝$10）
    let mut r = Rig::new(0x10);
    r.run(&[AMOVE, 5, 5]);
    assert_eq!(r.run(&[ORG, 0x0000, 0x0254]), 8);
    assert_eq!(r.a.cp(), (0, 0));
    assert_eq!((r.a.param(0x10), r.a.param(0x11)), (0x0000, 0x0254));
    r.wpr(CL0, 0xFFFF);
    r.run(&[DOT]);
    assert_eq!(r.m.0[0x25], 0x00F0);
    // 1 画素右は同じ語の bit 11–8、1 画素上は MW だけ前の語
    r.run(&[AMOVE, 1, 1]);
    assert_eq!((r.a.param(0x10), r.a.param(0x11)), (0x0000, 0x0158));
}

// ---- データ転送（(4)、(7)） ----

#[test]
fn rd_wt_mod_step_rwp() {
    // U85 p.192 の例：上画面の番地 $56 の $5555
    let mut r = Rig::new(0x10);
    r.m.0[0x56] = 0x5555;
    r.set_rwp(0, 0x56);
    assert_eq!(r.run(&[RD]), 12);
    assert_eq!(r.fifo(), 0x5555);
    assert_eq!(r.a.rwp(), (0, 0x57));
    assert_eq!(r.run(&[WT, 0x1234]), 8);
    assert_eq!(r.m.0[0x57], 0x1234);
    r.wpr(MASK, 0xF0F0);
    r.m.0[0x58] = 0x2222;
    assert_eq!(r.run(&[MOD | 1, 0x1111]), 8, "MM＝01（OR）");
    assert_eq!(r.m.0[0x58], 0x3232, "MASK が 0 のビットは変えない");
    assert_eq!(r.a.rwp(), (0, 0x59));
}

#[test]
fn clr_example() {
    // U85 p.203–204 の例：$52〜$56 の 5 語×7 行（行 5〜B）
    let mut r = Rig::new(0x10);
    r.set_rwp(0, 0x56);
    let cycles = r.run(&[CLR, 0x1111, 0xFFFC, 0xFFFA]);
    assert_eq!(cycles, (2 * 5 + 8) * 7 + 12);
    for row in 0..16u32 {
        for col in 0..16u32 {
            let inside = (5..=0xB).contains(&row) && (2..=6).contains(&col);
            let want = if inside { 0x1111 } else { 0 };
            assert_eq!(
                r.m.0[(row * 16 + col) as usize],
                want,
                "行 {row:X}、列 {col:X}"
            );
        }
    }
    // 終了後の RWP は最後の行の 1 つ先（図の位置。ラベルの $B6 との食い違いは [V]）
    assert_eq!(r.a.rwp(), (0, 0xC6));
}

#[test]
fn sclr_modifies_only_mask_bits() {
    // U85 p.207 の例と同じ MASK＝$F0F0
    let mut r = Rig::new(0x10);
    r.wpr(MASK, 0xF0F0);
    for a in 0x50..0x60 {
        r.m.0[a] = 0x2222;
    }
    r.set_rwp(0, 0x50);
    let cycles = r.run(&[SCLR | 3, 0xFFFF, 3, 0]);
    assert_eq!(cycles, (4 * 4 + 6) + 12);
    assert_eq!(
        r.m.0[0x50..0x55],
        [0xD2D2, 0xD2D2, 0xD2D2, 0xD2D2, 0x2222],
        "EOR"
    );
    assert_eq!(r.a.rwp(), (0, 0x40), "AY＝0 の 1 行。+Y の向きへ 1 行先");
}

#[test]
fn cpy_example_rotates_columns_into_rows() {
    // U85 p.213–215 の例：Pss＝$89、RWP＝$B0、S＝1、DSD＝000、AX＝3、AY＝6
    let (dark, light) = (0xEEEE, 0x1111);
    let mut r = Rig::new(0x10);
    for (a, v) in [
        (0x48, dark),
        (0x49, dark),
        (0x4A, light),
        (0x58, dark),
        (0x59, dark),
        (0x5A, light),
        (0x5B, light),
        (0x68, dark),
        (0x69, dark),
        (0x6A, light),
    ] {
        r.m.0[a] = v;
    }
    r.set_rwp(0, 0xB0);
    let cycles = r.run(&[CPY | 0x0800, 0x0000, 0x0890, 3, 6]);
    assert_eq!(cycles, (6 * 4 + 10) * 7 + 12);
    assert_eq!(r.a.rwp(), (0, 0x70), "RWPe");
    let row = |r: &Rig, base: usize| r.m.0[base..base + 7].to_vec();
    assert_eq!(row(&r, 0xB0), [0, 0, dark, dark, dark, 0, 0]);
    assert_eq!(row(&r, 0xA0), [0, 0, light, light, light, 0, 0]);
    assert_eq!(row(&r, 0x90), [0, 0, 0, light, 0, 0, 0]);
    assert_eq!(row(&r, 0x80), [0; 7]);
    assert_eq!(r.m.0[0x59], dark, "転送元は変わらない");
}

#[test]
fn dwt_program_transfer_and_rwpe() {
    let mut r = Rig::new(0x40);
    r.set_rwp(0, 0x100);
    // AX＝1、AY＝−1：2 語×2 行、下へ
    let cycles = r.run(&[DWT, 1, neg(-1), 0xA, 0xB, 0xC, 0xD]);
    assert_eq!(cycles, (4 * 2 + 8) * 2 + 16 + 34);
    assert_eq!(r.m.0[0x100..0x102], [0xA, 0xB]);
    assert_eq!(r.m.0[0x140..0x142], [0xC, 0xD]);
    assert_eq!(r.a.rwp(), (0, 0x180));
}

#[test]
fn dmod_applies_mm_and_mask() {
    let mut r = Rig::new(0x40);
    r.wpr(MASK, 0x00FF);
    r.m.0[0x200] = 0x1234;
    r.m.0[0x1FF] = 0x5678;
    r.set_rwp(0, 0x200);
    // AX＝−1：左へ 2 語。MM＝10（AND）
    r.run(&[DMOD | 2, neg(-1), 0, 0x0F0F, 0x0F0F]);
    assert_eq!((r.m.0[0x200], r.m.0[0x1FF]), (0x1204, 0x5608));
}

#[test]
fn drd_hangs_after_the_last_word_until_abort() {
    let mut r = Rig::new(0x40);
    r.m.0[0x300] = 0x1111;
    r.m.0[0x301] = 0x2222;
    r.set_rwp(0, 0x300);
    r.put(&[DRD, 1, 0]);
    for _ in 0..1000 {
        r.a.tick(&mut r.m);
    }
    assert_eq!((r.fifo(), r.fifo()), (0x1111, 0x2222));
    assert!(!r.a.idle(), "最後の語の後も待ち続ける（U85 p.128）");
    assert_eq!(r.a.status() & SR_CED, 0);
    r.reg(0x02, 0x8200);
    r.reg(0x02, 0x0200);
    assert_eq!(
        r.run(&[WPR | CL0, 0x5A5A]),
        6,
        "ABT の後は次のコマンドを実行する"
    );
}

#[test]
fn parameters_arriving_late_add_wait_cycles() {
    let mut r = Rig::new(0x10);
    let start = r.a.cycles();
    r.put(&[WPR | CL0]);
    for _ in 0..10 {
        r.a.tick(&mut r.m);
    }
    r.put(&[0x4321]);
    r.finish();
    // 10 サイクルのうち最初の 1 つはコマンド語の取り出しで、6 サイクルに含まれる。待ちは 9
    assert_eq!(r.a.cycles() - start, 6 + 9);
    assert_eq!(r.a.param(0), 0x4321);
}

// ---- 演算モードと色モード（(4)） ----

#[test]
fn operation_modes() {
    // 語 $7654 の画素 1（値 5）に色データ 3 または 9 を描く
    let cases = [
        (0, 0x3333, 0x0000, 0x7634),
        (1, 0x3333, 0x0000, 0x7674),
        (2, 0x3333, 0x0000, 0x7614),
        (3, 0x3333, 0x0000, 0x7664),
        (4, 0x3333, 0x0050, 0x7634), // P＝CCMP：同じ位置の画素と比べる
        (4, 0x3333, 0x5505, 0x7654),
        (5, 0x3333, 0x0050, 0x7654),
        (5, 0x3333, 0x5505, 0x7634),
        (6, 0x3333, 0x0000, 0x7654), // P＜CL：5＜3 ではない
        (6, 0x9999, 0x0000, 0x7694),
        (7, 0x3333, 0x0000, 0x7634), // P＞CL：5＞3
        (7, 0x9999, 0x0000, 0x7654),
    ];
    for (opm, color, ccmp, want) in cases {
        let mut r = Rig::drawing(0x10, 0x100);
        r.m.0[0x100] = 0x7654;
        r.wpr(CL0, color);
        r.wpr(CCMP, ccmp);
        r.run(&[AMOVE, 1, 0]);
        assert_eq!(r.run(&[DOT | opm]), 8);
        assert_eq!(
            r.m.0[0x100], want,
            "OPM＝{opm:03b}、色 {color:04X}、CCMP {ccmp:04X}"
        );
    }
}

#[test]
fn color_modes() {
    let mut r = Rig::drawing(0x10, 0x100);
    r.wpr(CL0, 0x2222);
    r.wpr(CL1, 0x7777);
    // パターンの点 (0, 0) が 1、(1, 0) が 0
    r.run(&[WPTN, 1, 0x0001]);
    r.wpr(PRC2, 0x0010); // PEX＝1
    for (op, want) in [(0, [7, 2]), (COL01, [7, 0]), (COL10, [0, 2])] {
        r.m.0[0x100] = 0;
        r.run(&[ORG, 0, 0x1000]);
        r.run(&[ALINE | op, 2, 0]);
        assert_eq!([r.pix(0, 0), r.pix(1, 0)], want, "COL＝{:02b}", op >> 3);
    }
    // COL＝11：パターン RAM の語 4×PPY＋PPX を色データにする
    r.run(&[WPTN | 5, 1, 0xABCD]);
    r.wpr(PRC0, 0x1010); // PPY＝1、PPX＝1
    r.run(&[ORG, 0, 0x1000]);
    r.run(&[DOT | COL11]);
    assert_eq!(r.pix(0, 0), 0xD);
}

// ---- 線と図形（(5)、(8)） ----

#[test]
fn aline_example() {
    // U85 p.229 の例：CP＝(−13, −10) から (10, 2) へ
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, neg(-13), neg(-10)]);
    assert_eq!(r.run(&[ALINE, 10, 2]), 4 * 23 + 18, "L＝23");
    assert_eq!(r.a.cp(), (10, 2));
    assert_eq!(r.lit(-20..=20, -20..=20), 23);
    assert_eq!(r.pix(-13, -10), 0xF, "始点は描く");
    assert_eq!(r.pix(10, 2), 0, "終点は描かない");
    for x in -13..=9 {
        assert_eq!(r.lit(x..=x, -10..=2), 1, "X ごとに 1 画素");
    }
}

#[test]
fn arct_example() {
    // U85 p.233 の例：CP＝(6, −6)、対角 (−16, 10)
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, 6, neg(-6)]);
    assert_eq!(r.run(&[ARCT, neg(-16), 10]), 2 * 4 * (22 + 16) + 54);
    assert_eq!(r.a.cp(), (6, -6));
    assert_eq!(r.lit(-30..=30, -30..=30), 2 * (22 + 16));
    for (x, y) in [(6, -6), (-16, -6), (-16, 10), (6, 10)] {
        assert_eq!(r.pix(x, y), 0xF, "角 ({x}, {y})");
    }
}

#[test]
fn apll_example() {
    // U85 p.238 の例：CP＝(−8, −6)、(−4, 4)、(8, 6)、(16, −8)
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, neg(-8), neg(-6)]);
    let cycles = r.run(&[APLL, 3, neg(-4), 4, 8, 6, 16, neg(-8)]);
    assert_eq!(cycles, (4 * 10 + 16) + (4 * 12 + 16) + (4 * 14 + 16) + 8);
    assert_eq!(r.a.cp(), (16, -8));
    assert_eq!(r.pix(16, -8), 0);
    for (x, y) in [(-8, -6), (-4, 4), (8, 6)] {
        assert_eq!(r.pix(x, y), 0xF);
    }
}

#[test]
fn relative_polyline_matches_absolute() {
    let mut a = Rig::drawing(0x40, 0x8020);
    let mut b = Rig::drawing(0x40, 0x8020);
    for r in [&mut a, &mut b] {
        r.wpr(CL0, 0xFFFF);
        r.run(&[AMOVE, neg(-8), neg(-6)]);
    }
    a.run(&[APLL, 3, neg(-4), 4, 8, 6, 16, neg(-8)]);
    b.run(&[RPLL, 3, 4, 10, 12, 2, 8, neg(-14)]);
    assert_eq!(a.a.cp(), b.a.cp());
    assert!(a.m.0 == b.m.0);
}

#[test]
fn aplg_closes_back_to_cp() {
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    let cycles = r.run(&[APLG, 2, 4, 0, 4, 4]);
    assert_eq!(cycles, (4 * 4 + 16) * 2 + 4 * 4 + 20, "閉じる線 Lo＝4");
    assert_eq!(r.a.cp(), (0, 0));
    assert_eq!(r.lit(-5..=10, -5..=10), 12);
    assert_eq!(r.pix(2, 2), 0xF, "斜めの辺");
}

#[test]
fn line_pattern_with_zoom() {
    // U85 p.168：'1111101010' を PZX＝1 で '11111111110011001100'
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL1, 0xFFFF);
    r.run(&[WPTN, 1, 0b01_0101_1111]);
    r.wpr(PRC2, 0x0091); // PEX＝9、PZX＝1
    r.run(&[ALINE, 20, 0]);
    let got: String = (0..20)
        .map(|x| if r.pix(x, 0) != 0 { '1' } else { '0' })
        .collect();
    assert_eq!(got, "11111111110011001100");
    assert_eq!(r.a.param(5), 0x0000, "20 画素で PPX が PSX に戻る");
}

#[test]
fn filled_rectangle_example() {
    // U85 p.170 の図 6.17(b)：パターン 6×7、PPX＝2、PPY＝1、11×10 画素
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL1, 0xFFFF);
    r.run(&[WPTN, 7, 0x00, 0x04, 0x0A, 0x11, 0x11, 0x0A, 0x04]);
    r.wpr(PRC0, 0x1020); // PPY＝1、PPX＝2
    r.wpr(PRC2, 0x6050); // PEY＝6、PEX＝5
    let cycles = r.run(&[AFRCT, 10, 9]);
    assert_eq!(cycles, (4 * 11 + 8) * 10 + 18);
    let want = [
        "10000010000",
        "01000101000",
        "00101000101",
        "00101000101",
        "01000101000",
        "10000010000",
        "00000000000",
        "10000010000",
        "01000101000",
        "00101000101",
    ];
    for (y, row) in want.iter().enumerate() {
        let got: String = (0..11)
            .map(|x| if r.pix(x, y as i32) != 0 { '1' } else { '0' })
            .collect();
        assert_eq!(&got, row, "下から {y} 行目");
    }
    assert_eq!(r.a.cp(), (0, 10), "Pe は CP の列で最後の行の 1 つ先");
}

#[test]
fn rfrct_paints_toward_negative_corner() {
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0x3333);
    r.run(&[AMOVE, 5, 5]);
    r.run(&[RFRCT, neg(-2), neg(-3)]);
    assert_eq!(r.lit(0..=10, 0..=10), 3 * 4);
    assert_eq!((r.pix(3, 2), r.pix(5, 5)), (3, 3));
    assert_eq!(r.a.cp(), (5, 1));
}

#[test]
fn moves_take_56_cycles() {
    let mut r = Rig::drawing(0x40, 0x8020);
    assert_eq!(r.run(&[AMOVE, 3, 4]), 56);
    assert_eq!(r.run(&[RMOVE, neg(-5), 2]), 56);
    assert_eq!(r.a.cp(), (-2, 6));
}

// ---- 領域モード（(6)） ----

/// (AREA, 線の始点の X, 終点の X, 描いた画素, 終了後の CP, ARD)
type AreaCase = (u16, i16, i16, &'static str, (i16, i16), bool);

#[test]
fn area_modes() {
    // 領域は (0, 0)〜(4, 4)。y＝2 の横線
    let cases: [AreaCase; 7] = [
        (0x80, 0, 10, "1111111111", (10, 2), false),
        (0x20, 0, 10, "1111100000", (5, 2), true),
        (0x40, 0, 10, "1111100000", (10, 2), false),
        (0x60, 0, 10, "1111100000", (10, 2), true),
        (0xA0, 9, -1, "0000011111", (4, 2), true),
        (0xC0, 0, 10, "0000011111", (10, 2), false),
        (0xE0, 0, 10, "0000011111", (10, 2), true),
    ];
    for (area, from, to, want, cp, ard) in cases {
        let mut r = Rig::drawing(0x40, 0x8020);
        r.wpr(CL0, 0xFFFF);
        for (i, v) in [0, 0, 4, 4].into_iter().enumerate() {
            r.wpr(XMIN + i as u16, v);
        }
        r.run(&[AMOVE, neg(from), 2]);
        r.run(&[ALINE | area, neg(to), 2]);
        let got: String = (0..10)
            .map(|x| if r.pix(x, 2) != 0 { '1' } else { '0' })
            .collect();
        assert_eq!(got, want, "AREA＝{:03b}", area >> 5);
        assert_eq!(r.a.cp(), cp, "AREA＝{:03b}", area >> 5);
        assert_eq!(r.a.status() & SR_ARD != 0, ard, "AREA＝{:03b}", area >> 5);
        assert_eq!(r.a.status() & SR_CED, SR_CED);
        if ard {
            r.run(&[RPR | CL0]);
            assert_eq!(r.a.status() & SR_ARD, 0, "RPR で ARD は 0");
        }
    }
}

// ---- 図形コピー（(8)） ----

#[test]
fn agcpy_example() {
    // U85 p.296–297 の例：CP＝(4, 2)、Pss＝(18, 2)、DX＝13、DY＝7、S＝1、DSD＝000
    let mut r = Rig::drawing(0x40, 0x8020);
    let src = |x: i32, y: i32| ((x * 3 + y * 5) & 0xF) as u16 | 1;
    for y in 2..=9 {
        for x in 18..=31 {
            r.set_pix(x, y, src(x, y));
        }
    }
    r.run(&[AMOVE, 4, 2]);
    let cycles = r.run(&[AGCPY | 0x0800, 18, 2, 13, 7]);
    assert_eq!(cycles, ((4 + 2) * 8 + 10) * 14 + 70, "A＝8、B＝14");
    assert_eq!(r.a.cp(), (4, 16));
    for j in 0..14 {
        for i in 0..8 {
            assert_eq!(
                r.pix(4 + i, 2 + j),
                src(18 + j, 2 + i),
                "転送先 ({}, {})",
                4 + i,
                2 + j
            );
        }
    }
    assert_eq!(r.pix(12, 2), 0);
    assert_eq!(r.pix(4, 16), 0);
}

#[test]
fn rgcpy_mirrors_with_dsd() {
    let mut r = Rig::drawing(0x40, 0x8020);
    for x in 0..4 {
        r.set_pix(10 + x, 0, x as u16 + 1);
    }
    r.run(&[AMOVE, 3, 5]);
    // S＝0、DSD＝010（X は負の向き）：左右を反転して写す
    r.run(&[RGCPY | 0x0200, 7, neg(-5), 3, 0]);
    let got: Vec<u16> = (0..4).map(|x| r.pix(x, 5)).collect();
    assert_eq!(got, [4, 3, 2, 1]);
    assert_eq!(r.a.cp(), (3, 6));
}

// ---- 円・楕円・弧（追補 3 の (1)） ----

/// (x, y) を中心とした相対座標の、0 でない画素の集合。
fn lit_set(r: &Rig, c: (i32, i32), half: i32) -> BTreeSet<(i32, i32)> {
    let mut s = BTreeSet::new();
    for y in -half..=half {
        for x in -half..=half {
            if r.pix(c.0 + x, c.1 + y) != 0 {
                s.insert((x, y));
            }
        }
    }
    s
}

fn circle(r: i64) -> BTreeSet<(i32, i32)> {
    let mut rig = Rig::drawing(0x40, 0x8020);
    rig.wpr(CL0, 0xFFFF);
    rig.run(&[CRCL, r as u16]);
    lit_set(&rig, (0, 0), (r & 0x1FFF) as i32 + 2)
}

fn ellipse(a: u16, b: u16, dx: u16) -> BTreeSet<(i32, i32)> {
    let mut rig = Rig::drawing(0x40, 0x8020);
    rig.wpr(CL0, 0xFFFF);
    rig.run(&[ELPS, a, b, dx]);
    lit_set(&rig, (0, 0), 20)
}

/// 8 近傍でつながった閉じた線か：どの点にも、隣の点が 2 つ以上ある。
fn closed(s: &BTreeSet<(i32, i32)>) -> bool {
    s.iter().all(|&(x, y)| {
        let n = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ]
        .iter()
        .filter(|(dx, dy)| s.contains(&(x + dx, y + dy)))
        .count();
        n >= 2
    })
}

#[test]
fn crcl_example_draws_a_closed_circle_around_cp() {
    // U85 p.249 の例：CP＝(0, 0)、r＝7
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    let cycles = r.run(&[CRCL, 7]);
    let s = lit_set(&r, (0, 0), 10);
    assert_eq!(cycles, 8 * s.len() as u64 + 66, "d は描いた点の数");
    assert_eq!(r.a.cp(), (0, 0), "CP は中心に戻る");
    for p in [(7, 0), (0, 7), (-7, 0), (0, -7)] {
        assert!(s.contains(&p), "{p:?}");
    }
    for &(x, y) in &s {
        assert!(
            (x * x + y * y - 49).abs() <= 7,
            "({x}, {y}) が円から離れている"
        );
        for q in [(-x, y), (x, -y), (y, x)] {
            assert!(s.contains(&q), "({x}, {y}) と対称な {q:?} がない");
        }
    }
    assert!(closed(&s));
    // 時計回りでも同じ画素
    let mut c = Rig::drawing(0x40, 0x8020);
    c.wpr(CL0, 0xFFFF);
    assert_eq!(c.run(&[CRCL | CW, 7]), cycles);
    assert!(c.m.0 == r.m.0);
    // r は下位 13 bit
    assert_eq!(circle(0x2003), circle(3));
}

#[test]
fn crcl_of_radius_zero_draws_nothing() {
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    assert_eq!(r.run(&[CRCL, 0]), 66);
    assert_eq!(r.lit(-3..=3, -3..=3), 0);
}

#[test]
fn elps_example() {
    // U85 p.252 の例：CP＝(16, 10)、a＝9、b＝4、dX＝9（dY＝6）
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, 16, 10]);
    let cycles = r.run(&[ELPS, 9, 4, 9]);
    let s = lit_set(&r, (16, 10), 12);
    assert_eq!(cycles, 10 * s.len() as u64 + 90);
    assert_eq!(r.a.cp(), (16, 10));
    for p in [(9, 0), (0, 6), (-9, 0), (0, -6)] {
        assert!(s.contains(&p), "{p:?}");
    }
    for &(x, y) in &s {
        // 4x² ＋ 9y² ＝ 324 から、半画素ぶん以内
        let f = 4 * x * x + 9 * y * y - 324;
        assert!(f.abs() <= 60, "({x}, {y}) が楕円から離れている（{f}）");
        assert!(s.contains(&(-x, y)) && s.contains(&(x, -y)));
    }
    assert!(closed(&s));
    assert_eq!(s, ellipse(9, 4, 9), "CP の位置によらない");
}

#[test]
fn aarc_example_draws_three_quarters() {
    // U85 p.254–255 の例：CP＝(12, 4)、中心 (12, 10)、Pe＝(6, 10)、C＝0（反時計回り）
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, 12, 4]);
    let cycles = r.run(&[AARC, 12, 10, 6, 10]);
    let s = lit_set(&r, (12, 10), 8);
    assert_eq!(cycles, 8 * s.len() as u64 + 18);
    assert_eq!(r.a.cp(), (6, 10));
    // 円の右・上を回り、左下の 4 分の 1 と終点を描かない
    let want: BTreeSet<_> = circle(6)
        .into_iter()
        .filter(|&(x, y)| !(x < 0 && y < 0) && (x, y) != (-6, 0))
        .collect();
    assert_eq!(s, want);
}

#[test]
fn aarc_clockwise_takes_the_short_way() {
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, 12, 4]);
    r.run(&[AARC | CW, 12, 10, 6, 10]);
    let want: BTreeSet<_> = circle(6)
        .into_iter()
        .filter(|&(x, y)| (x < 0 && y < 0) || (x, y) == (0, -6))
        .collect();
    assert_eq!(lit_set(&r, (12, 10), 8), want);
    assert_eq!(r.a.cp(), (6, 10));
}

#[test]
fn rarc_example() {
    // U85 p.257–258 の例：CP＝(6, 10)、dXc＝6、dYc＝0、dXe＝6、dYe＝6、C＝0
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, 6, 10]);
    r.run(&[RARC, 6, 0, 6, 6]);
    assert_eq!(r.a.cp(), (12, 16));
    let want: BTreeSet<_> = circle(6)
        .into_iter()
        .filter(|&(x, y)| !(x < 0 && y > 0) && (x, y) != (0, 6))
        .collect();
    assert_eq!(lit_set(&r, (12, 10), 8), want);
}

#[test]
fn arc_with_pe_equal_to_cp_draws_the_whole_circle() {
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, 6, 0]);
    r.run(&[AARC, 0, 0, 6, 0]);
    assert_eq!(lit_set(&r, (0, 0), 8), circle(6));
    assert_eq!(r.a.cp(), (6, 0));
}

#[test]
fn aearc_quarter_and_rearc_agree() {
    // a＝9、b＝4 の楕円（dX＝9、dY＝6）を、(25, 10) から (16, 16) まで反時計回りに
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0xFFFF);
    r.run(&[AMOVE, 25, 10]);
    let cycles = r.run(&[AEARC, 9, 4, 16, 10, 16, 16]);
    let s = lit_set(&r, (16, 10), 12);
    assert_eq!(cycles, 10 * s.len() as u64 + 96);
    assert_eq!(r.a.cp(), (16, 16));
    let want: BTreeSet<_> = ellipse(9, 4, 9)
        .into_iter()
        .filter(|&(x, y)| x > 0 && y >= 0)
        .collect();
    assert_eq!(s, want);

    let mut q = Rig::drawing(0x40, 0x8020);
    q.wpr(CL0, 0xFFFF);
    q.run(&[AMOVE, 25, 10]);
    q.run(&[REARC, 9, 4, neg(-9), 0, neg(-9), 6]);
    assert_eq!(q.a.cp(), (16, 16));
    assert!(q.m.0 == r.m.0);
}

#[test]
fn curves_advance_the_line_pattern() {
    // 線のコマンドとして、描いた点ごとにパターンの X が進む（U85 p.168）
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0x0000);
    r.wpr(CL1, 0xFFFF);
    r.run(&[WPTN, 1, 0b01]);
    r.wpr(PRC2, 0x0010); // PEX＝1：1 点おき
    r.run(&[CRCL, 7]);
    let n = lit_set(&r, (0, 0), 10).len();
    let d = circle(7).len();
    assert_eq!(n, d.div_ceil(2));
}

// ---- パターン（追補 3 の (2)） ----

/// 6×8 の、左右・上下とも対称でない模様（語 y の bit x が点 (x, y)）。
const PTN6X8: [u16; 8] = [
    0b000001, 0b000011, 0b000101, 0b001001, 0b010001, 0b100001, 0b111111, 0b100100,
];

fn ptn_rig() -> Rig {
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0x2222);
    r.wpr(CL1, 0xFFFF);
    let mut w = vec![WPTN, 8];
    w.extend(PTN6X8);
    r.run(&w);
    r.wpr(PRC2, 0x7050); // PEY＝7、PEX＝5
    r.run(&[AMOVE, 20, 10]);
    r
}

fn ptn_bit(i: i32, j: i32) -> u16 {
    if (PTN6X8[(j % 8) as usize] >> (i % 6)) & 1 != 0 {
        0xF
    } else {
        0x2
    }
}

#[test]
fn ptn_example_copies_the_pattern() {
    // U85 p.286 の例 (1)：PP＝PS、PZ＝0、SZ＝PE−PS（$0705）、SL＝0、SD＝000
    let mut r = ptn_rig();
    let cycles = r.run(&[PTN, 0x0705]);
    assert_eq!(cycles, (4 * 6 + 10) * 8 + 20);
    for j in 0..8 {
        for i in 0..6 {
            assert_eq!(r.pix(20 + i, 10 + j), ptn_bit(i, j), "({i}, {j})");
        }
    }
    assert_eq!(r.lit(14..=30, 4..=24), 48);
    assert_eq!(r.a.cp(), (20, 18), "Pe は CP の上、SZy＋1 行先（表 C36-1）");
}

#[test]
fn ptn_smaller_and_larger_than_the_pattern() {
    // U85 p.288 の例 (3)（SZ＝$0403）と p.289 の例 (4)（SZ＝$0F0B）
    for (sz, w, h) in [(0x0403u16, 4, 5), (0x0F0B, 12, 16)] {
        let mut r = ptn_rig();
        r.run(&[PTN, sz]);
        for j in 0..h {
            for i in 0..w {
                assert_eq!(
                    r.pix(20 + i, 10 + j),
                    ptn_bit(i, j),
                    "SZ＝{sz:04X} ({i}, {j})"
                );
            }
        }
        assert_eq!(r.lit(14..=40, 4..=34), (w * h) as usize);
    }
}

#[test]
fn ptn_zoom_doubles_each_dot() {
    // U85 p.290 の例 (5)：PZX＝1、PZY＝1、SZ＝$0F0B
    let mut r = ptn_rig();
    r.wpr(PRC2, 0x7151); // PEY＝7、PZY＝1、PEX＝5、PZX＝1
    r.run(&[PTN, 0x0F0B]);
    for j in 0..16 {
        for i in 0..12 {
            assert_eq!(r.pix(20 + i, 10 + j), ptn_bit(i / 2, j / 2), "({i}, {j})");
        }
    }
}

#[test]
fn ptn_scan_directions() {
    // 表 C36-1：行の向きは SD×45°、行を送る向きは SL＝0 で＋90°、SL＝1 で＋45°
    let dirs = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    for sl in 0..2u16 {
        for sd in 0..8u16 {
            let mut r = ptn_rig();
            r.run(&[PTN | (sl << 11) | (sd << 8), 0x0705]);
            let row = dirs[sd as usize];
            let adv = dirs[((sd + 2 - sl) & 7) as usize];
            for j in 0..8 {
                for i in 0..6 {
                    let (x, y) = (20 + i * row.0 + j * adv.0, 10 + i * row.1 + j * adv.1);
                    assert_eq!(
                        r.pix(x, y),
                        ptn_bit(i, j),
                        "SL＝{sl}、SD＝{sd:03b}、({i}, {j})"
                    );
                }
            }
            assert_eq!(r.lit(0..=40, -10..=30), 48, "SL＝{sl}、SD＝{sd:03b}");
            assert_eq!(r.a.cp(), ((20 + 8 * adv.0) as i16, (10 + 8 * adv.1) as i16));
        }
    }
}

// ---- 塗りつぶし（追補 3 の (3)） ----

const EDG: u16 = 0x03;

/// 縁の色 5 の矩形の枠を (0, 0)〜(w, h) に描いた構成。CL0＝A、CL1＝B。
fn framed(w: u16, h: u16) -> Rig {
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0x5555);
    r.wpr(EDG, 0x5555);
    r.run(&[ARCT, w, h]);
    r.wpr(CL0, 0xAAAA);
    r.wpr(CL1, 0xBBBB);
    r
}

#[test]
fn paint_fills_a_rectangle_inside_the_edge() {
    let mut r = framed(11, 8);
    r.run(&[AMOVE, 3, 3]);
    let cycles = r.run(&[PAINT]);
    assert_eq!(
        cycles,
        (18 * 10 + 102) * 7 - 58,
        "U85 p.173 の矩形の式（A＝10、B＝7）"
    );
    for y in -2..=10 {
        for x in -2..=13 {
            let inside = (1..=10).contains(&x) && (1..=7).contains(&y);
            let frame = (0..=11).contains(&x) && (0..=8).contains(&y) && !inside;
            let want = if inside {
                0xA
            } else if frame {
                0x5
            } else {
                0
            };
            assert_eq!(r.pix(x, y), want, "({x}, {y})");
        }
    }
    assert_eq!(r.a.status() & SR_RFR, 0, "塗り残しはない");
}

#[test]
fn paint_pattern_follows_the_offset_from_cp() {
    // 2×2 の市松模様。CP の左と下でも、CP からのずれで模様が決まる（U85 p.272）
    let mut r = framed(11, 8);
    r.run(&[WPTN, 2, 0b01, 0b10]);
    r.wpr(PRC2, 0x1010); // PEY＝1、PEX＝1
    r.run(&[AMOVE, 4, 3]);
    r.run(&[PAINT]);
    for y in 1..=7 {
        for x in 1..=10 {
            let one = ((x - 4i32).rem_euclid(2) + (y - 3i32).rem_euclid(2)) % 2 == 0;
            assert_eq!(r.pix(x, y), if one { 0xB } else { 0xA }, "({x}, {y})");
        }
    }
    assert_eq!(r.a.param(5), 0x0000, "パターンの位置は戻る");
}

#[test]
fn paint_inside_a_circle() {
    // U85 p.278–279 の例（E＝0）：縁の色の円の中を塗る
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0x5555);
    r.wpr(EDG, 0x5555);
    r.run(&[CRCL, 7]);
    r.wpr(CL0, 0xAAAA);
    // CL0・CL1 の色も縁とみなすので、CL1 を背景（0）と違う色にする（U85 p.271）
    r.wpr(CL1, 0xBBBB);
    r.run(&[PAINT]);
    for y in -6..=6 {
        let xs: Vec<i32> = (-8..=8).filter(|&x| r.pix(x, y) == 5).collect();
        let (l, h) = (xs[0], xs[xs.len() - 1]);
        for x in -9..=9 {
            let want = if r.pix(x, y) == 5 {
                5
            } else if x > l && x < h {
                0xA
            } else {
                0
            };
            assert_eq!(r.pix(x, y), want, "({x}, {y})");
        }
    }
    assert_eq!(r.lit(-9..=9, 8..=9), 0, "円の外は塗らない");
    assert_eq!(r.a.status() & SR_RFR, 0);
}

#[test]
fn paint_treats_cl0_and_cl1_as_edges() {
    let mut r = framed(11, 8);
    r.wpr(CL1, 0x0000); // 背景と同じ色
    r.run(&[AMOVE, 3, 3]);
    r.run(&[PAINT]);
    assert_eq!(
        r.lit(1..=10, 1..=7),
        0,
        "背景の色が CL1 なので、縁とみなして塗らない"
    );
}

#[test]
fn paint_e1_fills_the_region_of_the_edg_color() {
    // E＝1：EDG 以外の色が縁（U85 p.271–272）
    let mut r = Rig::drawing(0x40, 0x8020);
    r.wpr(CL0, 0x5555);
    r.run(&[AFRCT, 5, 3]);
    r.wpr(EDG, 0x5555);
    r.wpr(CL0, 0xAAAA);
    r.run(&[AMOVE, 2, 1]);
    r.run(&[PAINT | 0x0100]);
    assert_eq!(r.lit(-5..=10, -5..=10), 6 * 4);
    for y in 0..=3 {
        for x in 0..=5 {
            assert_eq!(r.pix(x, y), 0xA);
        }
    }
}

/// U85 p.275 の図 C34-6 の手順：PAINT の後、読出し FIFO に座標があれば、それぞれから PAINT をやり直す。
/// 最初の PAINT で FIFO へ出た種の数を返す。
fn paint_flow(r: &mut Rig) -> usize {
    let mut first = None;
    let mut todo = vec![None];
    while let Some(seed) = todo.pop() {
        if let Some((x, y, pp)) = seed {
            r.wpr(PRC0, pp);
            r.run(&[AMOVE, x, y]);
        }
        r.put(&[PAINT]);
        let mut words = Vec::new();
        loop {
            if r.a.status() & SR_RFR != 0 {
                words.push(r.fifo());
            } else if r.a.idle() {
                break;
            } else {
                r.a.tick(&mut r.m);
            }
        }
        assert_eq!(words.len() % 3, 0);
        first.get_or_insert(words.len() / 3);
        for w in words.chunks(3) {
            todo.push(Some((w[0], w[1], w[2])));
        }
    }
    first.unwrap()
}

#[test]
fn paint_overflow_seeds_go_to_the_read_fifo() {
    // 上の通路（y＝9）から、仕切り（x＝3, 6, …, 27）の間の 10 個の部屋が下へ伸びる
    let mut r = framed(30, 10);
    r.wpr(CL0, 0x5555);
    for x in (3..=27).step_by(3) {
        r.run(&[AMOVE, x, 1]);
        r.run(&[ALINE, x, 9]);
    }
    r.wpr(CL0, 0xAAAA);
    r.run(&[AMOVE, 1, 9]);
    let overflow = paint_flow(&mut r);
    // 通路の行で 10 個の種を見つけ、スタック（4 点）に入らない 6 個を FIFO へ出す。
    // 18 語は FIFO（8 語）に入りきらず、ホストが読むまで待つ（U85 p.274）
    assert_eq!(overflow, 6);
    for y in 1..=9 {
        for x in 1..=29 {
            let want = if y < 9 && x % 3 == 0 { 0x5 } else { 0xA };
            assert_eq!(r.pix(x, y), want, "({x}, {y})");
        }
    }
}

#[test]
fn paint_with_data_in_the_read_fifo_has_a_two_point_stack() {
    let mut r = framed(30, 10);
    r.wpr(CL0, 0x5555);
    for x in (3..=27).step_by(3) {
        r.run(&[AMOVE, x, 1]);
        r.run(&[ALINE, x, 9]);
    }
    r.wpr(CL0, 0xAAAA);
    r.run(&[RPR | CL0]); // 読出し FIFO に 1 語
    r.run(&[AMOVE, 1, 9]);
    r.put(&[PAINT]);
    r.a.tick(&mut r.m); // PAINT を取り出して実行する
    let mut words = Vec::new();
    loop {
        if r.a.status() & SR_RFR != 0 {
            words.push(r.fifo());
        } else if r.a.idle() {
            break;
        } else {
            r.a.tick(&mut r.m);
        }
    }
    assert_eq!(words[0], 0xAAAA);
    assert_eq!(
        (words.len() - 1) / 3,
        8,
        "10 個の種のうち 2 個だけスタックに入る [V]"
    );
}

#[test]
fn paint_area_modes() {
    // 縁の矩形 (0, 0)〜(11, 8) の中で、領域を (3, 2)〜(6, 5) にする
    for (area, painted, ard) in [
        (0x40u16, 16, false),
        (0x60, 16, true),
        (0xC0, 70 - 16, false),
    ] {
        let mut r = framed(11, 8);
        for (i, v) in [3, 2, 6, 5].into_iter().enumerate() {
            r.wpr(XMIN + i as u16, v);
        }
        let start = if area & 0x80 != 0 { (1, 1) } else { (4, 3) };
        r.run(&[AMOVE, start.0, start.1]);
        r.run(&[PAINT | area]);
        let n = (1..=7)
            .flat_map(|y| (1..=10).map(move |x| (x, y)))
            .filter(|&(x, y)| r.pix(x, y) == 0xA)
            .count();
        assert_eq!(n, painted, "AREA＝{:03b}", area >> 5);
        assert_eq!(r.a.status() & SR_ARD != 0, ard, "AREA＝{:03b}", area >> 5);
    }
    // 001：外へ出ようとしたところで止まり、ARD と CED を 1 にする
    let mut r = framed(11, 8);
    for (i, v) in [3, 2, 6, 5].into_iter().enumerate() {
        r.wpr(XMIN + i as u16, v);
    }
    r.run(&[AMOVE, 4, 3]);
    r.run(&[PAINT | 0x20]);
    assert_eq!(r.a.status() & (SR_ARD | SR_CED), SR_ARD | SR_CED);
    let n = r.lit(1..=10, 1..=7);
    assert!(n < 16, "塗り終える前に止まる（{n} 画素）");
}

// ---- 保存 ----

#[test]
fn snapshot_round_trip_mid_transfer() {
    let mut r = Rig::new(0x40);
    r.set_rwp(0, 0x100);
    r.put(&[DWT, 3, 0, 1, 2]);
    for _ in 0..40 {
        r.a.tick(&mut r.m);
    }
    let json = serde_json::to_string(&r.a).unwrap();
    let back: Hd63484 = serde_json::from_str(&json).unwrap();
    assert_eq!(back, r.a);
    let mut s = Rig {
        a: back,
        m: Mem(r.m.0.clone()),
        mw: r.mw,
        org: r.org,
    };
    for rig in [&mut r, &mut s] {
        rig.put(&[3, 4]);
        rig.finish();
    }
    assert_eq!(r.a, s.a);
    assert_eq!(r.m.0[0x100..0x104], [1, 2, 3, 4]);
    assert!(r.m.0 == s.m.0);
}

// ---- データの DMA（DREQ・DACK・DTACK・DONE。U85 p.49–50、p.66–67、p.320–321） ----

const CCR_DMA_STEAL: u16 = 0x2A00; // DDM＝1、DRC＝1（サイクルスチール。DR-016）、GBM＝010
const CCR_DMA_BURST: u16 = 0x2200; // DDM＝1、DRC＝0

impl Rig {
    /// DREQ が出るまで進める。
    fn until_dreq(&mut self, limit: u32) {
        for _ in 0..limit {
            if self.a.dreq() {
                return;
            }
            self.a.tick(&mut self.m);
        }
        panic!("DREQ が出ない");
    }

    /// DMA の 1 語。DACK＝L を 1 回目の立上りで T1、2 回目で T3（DTACK）、DACK＝H を 3 回目で Tn。
    /// 読んだ語と、DONE が出ていたかを返す。
    fn dma_word(&mut self, to_acrtc: bool, data: u16) -> (u16, bool) {
        assert!(self.a.dreq());
        self.a.set_dack(true, to_acrtc, data);
        self.a.tick(&mut self.m);
        assert!(!self.a.dma_dtack(), "T1 では DTACK はまだ");
        let done = self.a.done();
        self.a.tick(&mut self.m);
        assert!(self.a.dma_dtack(), "T3 で DTACK");
        assert_eq!(self.a.done(), done);
        let v = self.a.dma_data();
        self.a.set_dack(false, to_acrtc, data);
        self.a.tick(&mut self.m);
        assert!(!self.a.dma_dtack() && !self.a.done());
        (v, done)
    }
}

#[test]
fn dma_dwt_one_word_per_request_and_done_on_the_last() {
    let mut r = Rig::new(0x40);
    r.set_rwp(0, 0x100);
    r.reg(0x02, CCR_DMA_STEAL);
    // AX＝1、AY＝−1：2 語×2 行。データは DMA で渡す
    r.put(&[DWT, 1, neg(-1)]);
    let mut dones = Vec::new();
    for w in [0xA, 0xB, 0xC, 0xD] {
        r.until_dreq(200);
        r.a.set_dack(true, true, w);
        r.a.tick(&mut r.m);
        // サイクルスチール：T1 で DREQ を戻す
        assert!(!r.a.dreq());
        dones.push(r.a.done());
        r.a.tick(&mut r.m);
        assert!(r.a.dma_dtack());
        r.a.set_dack(false, true, w);
        r.a.tick(&mut r.m);
    }
    assert_eq!(dones, vec![false, false, false, true]);
    assert_eq!(r.a.dma_progress(), (4, 4));
    assert!(!r.a.dreq(), "語が残らなければ要求しない");
    r.finish();
    assert_eq!(r.m.0[0x100..0x102], [0xA, 0xB]);
    assert_eq!(r.m.0[0x140..0x142], [0xC, 0xD]);
    assert_eq!(r.a.rwp(), (0, 0x180));
    assert_ne!(r.a.status() & SR_CED, 0);
}

#[test]
fn dma_cycle_takes_t1_t3_and_waits_for_dack_high() {
    let mut r = Rig::new(0x40);
    r.set_rwp(0, 0x100);
    r.reg(0x02, CCR_DMA_STEAL);
    r.put(&[DWT, 0, 0]);
    r.until_dreq(200);
    r.a.set_dack(true, true, 0x1234);
    r.a.tick(&mut r.m); // T1
    assert!(r.a.done(), "1 語だけなので最初の語で DONE");
    r.a.tick(&mut r.m); // T3
    assert!(r.a.dma_dtack());
    // DACK を L のまま保てば、DTACK も保つ（Tw）
    for _ in 0..5 {
        r.a.tick(&mut r.m);
        assert!(r.a.dma_dtack() && r.a.done());
    }
    r.a.set_dack(false, true, 0x1234);
    r.a.tick(&mut r.m); // Tn
    assert!(!r.a.dma_dtack() && !r.a.done());
    r.finish();
    assert_eq!(r.m.0[0x100], 0x1234);
}

#[test]
fn dma_drd_hands_out_the_read_fifo_and_does_not_hang() {
    let mut r = Rig::new(0x40);
    for i in 0..4 {
        r.m.0[0x300 + i] = 0x1111 * (i as u16 + 1);
    }
    r.set_rwp(0, 0x300);
    r.reg(0x02, CCR_DMA_STEAL);
    r.put(&[DRD, 3, 0]);
    let mut got = Vec::new();
    for _ in 0..4 {
        r.until_dreq(500);
        got.push(r.dma_word(false, 0));
    }
    assert_eq!(
        got,
        vec![
            (0x1111, false),
            (0x2222, false),
            (0x3333, false),
            (0x4444, true)
        ]
    );
    assert!(!r.a.dreq());
    // プログラム転送と違い、最後の語の後で止まらない
    r.finish();
    assert_ne!(r.a.status() & SR_CED, 0);
    assert_eq!(r.a.error_counts(), (0, 0));
}

#[test]
fn dma_drd_requests_only_while_the_read_fifo_has_data() {
    let mut r = Rig::new(0x40);
    r.set_rwp(0, 0x300);
    r.reg(0x02, CCR_DMA_STEAL);
    r.put(&[DRD, 1, 0]);
    // 実行を始めて最初の語を読むまでは要求しない
    for _ in 0..40 {
        r.a.tick(&mut r.m);
        assert!(!r.a.dreq());
    }
    r.until_dreq(100);
    assert_ne!(r.a.status() & SR_RFR, 0);
}

#[test]
fn dma_burst_keeps_dreq_through_the_cycle() {
    let mut r = Rig::new(0x40);
    r.set_rwp(0, 0x100);
    r.reg(0x02, CCR_DMA_BURST);
    r.put(&[DWT, 3, 0]);
    r.until_dreq(200);
    r.a.set_dack(true, true, 1);
    r.a.tick(&mut r.m);
    assert!(r.a.dreq(), "バーストでは語が残る間 DREQ を保つ");
    r.a.tick(&mut r.m);
    r.a.set_dack(false, true, 1);
    r.a.tick(&mut r.m);
    for w in 2..=4 {
        r.dma_word(true, w);
    }
    assert!(!r.a.dreq());
    r.finish();
    assert_eq!(r.m.0[0x100..0x104], [1, 2, 3, 4]);
}

#[test]
fn dma_write_fifo_full_holds_dreq() {
    let mut r = Rig::new(0x100);
    r.set_rwp(0, 0x1000);
    r.reg(0x02, CCR_DMA_STEAL);
    // 64 語×1 行。DMA は 3 サイクルで 1 語、描画は 4〜6 サイクルで 1 語なので、FIFO がいずれ満ちる
    r.put(&[DWT, 63, 0]);
    let mut held = false;
    let mut sent = 0u16;
    for _ in 0..5000 {
        if sent == 64 {
            break;
        }
        if r.a.dreq() {
            r.dma_word(true, sent);
            sent += 1;
        } else {
            if sent > 0 && r.a.status() & SR_WFR == 0 {
                held = true;
            }
            r.a.tick(&mut r.m);
        }
    }
    assert_eq!(sent, 64);
    assert!(held, "書込み FIFO が満杯の間は要求しない");
    r.finish();
    assert!((0..64).all(|i| r.m.0[0x1000 + i] == i as u16));
    assert_eq!(r.a.error_counts(), (0, 0));
}

#[test]
fn dma_needs_ddm_and_stops_on_pause_and_abort() {
    let mut r = Rig::new(0x40);
    r.set_rwp(0, 0x100);
    // DDM＝0：プログラム転送。DREQ は出ない
    r.put(&[DWT, 0, 0]);
    for _ in 0..200 {
        r.a.tick(&mut r.m);
        assert!(!r.a.dreq());
    }
    r.put(&[0x5555]);
    r.finish();
    assert_eq!(r.m.0[0x100], 0x5555);

    r.reg(0x02, CCR_DMA_STEAL);
    r.put(&[DWT, 3, 0]);
    r.until_dreq(200);
    r.reg(0x02, CCR_DMA_STEAL | 0x4000); // PSE
    assert!(!r.a.dreq());
    r.reg(0x02, CCR_DMA_STEAL);
    assert!(r.a.dreq());
    r.dma_word(true, 1);
    r.reg(0x02, CCR_DMA_STEAL | 0x8000); // ABT
    r.reg(0x02, CCR_DMA_STEAL);
    assert!(!r.a.dreq());
    assert_eq!(r.a.dma_progress(), (0, 0));
}

#[test]
fn dma_state_survives_a_snapshot() {
    let mut r = Rig::new(0x40);
    r.set_rwp(0, 0x100);
    r.reg(0x02, CCR_DMA_STEAL);
    r.put(&[DWT, 1, 0]);
    r.until_dreq(200);
    r.a.set_dack(true, true, 0x77);
    r.a.tick(&mut r.m);
    let saved = serde_json::to_string(&r.a).unwrap();
    let b: Hd63484 = serde_json::from_str(&saved).unwrap();
    assert_eq!(b, r.a);
    assert_eq!(b.dma_progress(), (0, 2));
}

// ---- コマンドの DMA（CDM＝1。U85 p.66、台帳 §4.7） ----

/// CDM＝1、DRC＝0、GBM＝010。DRC でバーストを選んでも、コマンドの DMA はサイクルスチール（U85 p.66 の注 [V]）。
const CCR_CMD_DMA: u16 = 0x1200;

#[test]
fn command_dma_feeds_the_command_fifo_one_word_per_request_without_done() {
    let mut r = Rig::new(0x40);
    r.reg(0x02, CCR_CMD_DMA);
    // RWP を 0x100 にして、WT で 2 語を書く
    let cmds = [WPR | RWPH, 0, WPR | RWPL, 0x1000, WT, 0xAAAA, WT, 0xBBBB];
    let mut i = 0;
    for _ in 0..10_000 {
        if i == cmds.len() {
            break;
        }
        if !r.a.dreq() {
            r.a.tick(&mut r.m);
            continue;
        }
        r.a.set_dack(true, true, cmds[i]);
        r.a.tick(&mut r.m);
        assert!(!r.a.dreq(), "T1 で戻す（サイクルスチール）");
        assert!(!r.a.done(), "コマンドの DMA は DONE を出さない");
        r.a.tick(&mut r.m);
        assert!(r.a.dma_dtack());
        r.a.set_dack(false, true, cmds[i]);
        r.a.tick(&mut r.m);
        i += 1;
    }
    assert_eq!(i, cmds.len());
    r.finish();
    assert_eq!(r.m.0[0x100..0x102], [0xAAAA, 0xBBBB]);
    // データの DMA の語の数には入らない
    assert_eq!(r.a.dma_progress(), (0, 0));
    assert_eq!(r.a.error_counts(), (0, 0));
    // 語の数は決まっていないので、CDM の間は要求を続ける。DONE の入力で CDM が 0 になり、やめる
    r.until_dreq(100);
    r.a.done_input();
    for _ in 0..100 {
        r.a.tick(&mut r.m);
        assert!(!r.a.dreq());
    }
}

#[test]
fn command_dma_holds_dreq_while_the_command_fifo_is_full() {
    let mut r = Rig::new(0x40);
    r.reg(0x02, CCR_CMD_DMA);
    // 2,048 語の CLR の後に WPR を続ける。CLR を描く間に書込み FIFO（16 語）が満ちる
    let mut cmds = vec![CLR, 0x5A5A, 2047, 0];
    for k in 0..12 {
        cmds.extend([WPR | CL0, k]);
    }
    let (mut i, mut held) = (0, false);
    for _ in 0..1_000_000 {
        if i == cmds.len() {
            break;
        }
        if r.a.dreq() {
            assert!(!r.dma_word(true, cmds[i]).1);
            i += 1;
        } else {
            if r.a.status() & SR_WFR == 0 {
                held = true;
            }
            r.a.tick(&mut r.m);
        }
    }
    assert_eq!(i, cmds.len());
    assert!(held, "書込み FIFO が満杯の間は要求しない");
    r.finish();
    assert!(r.m.0[..2048].iter().all(|&w| w == 0x5A5A));
    assert_eq!(r.a.error_counts(), (0, 0));
    // PSE・ABT の間は、コマンドの DMA も要求しない
    r.until_dreq(100);
    r.reg(0x02, CCR_CMD_DMA | 0x4000);
    assert!(!r.a.dreq());
    r.reg(0x02, CCR_CMD_DMA);
    assert!(r.a.dreq());
}

// ---- 表示（台帳 §3.4 の案B、U85 p.22・p.30・p.42・p.55・p.69–111） ----
//
// 案B：HSW 13、HDS 19、HDW 128、HC 171（342 ドット）、VSW 3、VDS 40、SP1 192、VC 262（台帳 §3.4）。
// 表示の始まりは 2×(13＋19)＝64 ドット、ラスタ 3＋40＝43。インタリーブ（ACM＝10）の表示サイクルは
// 2CLK×4 で、最初のメモリサイクルが表示の読出し（台帳 §3.2・§3.3）。

const LINE: usize = 342;
const RASTERS: usize = 262;

/// 書込みを数える VRAM（1 M 語）。
struct Counted {
    m: Vec<u16>,
    writes: u64,
}

impl Vram for Counted {
    fn read(&mut self, addr: u32) -> u16 {
        self.m[addr as usize]
    }
    fn write(&mut self, addr: u32, data: u16) {
        self.writes += 1;
        self.m[addr as usize] = data;
    }
}

struct Screen {
    a: Hd63484,
    m: Counted,
}

impl Screen {
    /// 案B の同期と、基本画面（SAR1＝0、MWR1＝$40）。DCR は電源投入時の値、OMR は $C008。
    fn plan_b() -> Screen {
        let mut s = Screen {
            a: Hd63484::new(),
            m: Counted {
                m: vec![0; 1 << 20],
                writes: 0,
            },
        };
        s.reg(0x02, 0x0200);
        for (ar, v) in [
            (0x82, 0xAA0D),
            (0x84, 0x127F),
            (0x86, 0x0106),
            (0x88, 0x2703),
            (0x8A, 0x00C0),
            (0xC2, 0x0040),
            (0xCA, 0x0040),
        ] {
            s.reg(ar, v);
        }
        s.reg(0x04, 0xC008);
        s
    }

    fn reg(&mut self, ar: u16, v: u16) {
        self.a.write(false, ar);
        self.a.write(true, v);
    }

    fn put(&mut self, words: &[u16]) {
        self.a.write(false, 0);
        for &w in words {
            self.a.write(true, w);
        }
    }

    fn tick(&mut self) -> DisplayOut {
        self.a.tick(&mut self.m);
        self.a.display_out()
    }

    /// ラスタ `raster` の 2CLK `dot` まで進める。
    fn until(&mut self, raster: u16, dot: u16) {
        for _ in 0..LINE * RASTERS + 1 {
            if self.a.beam() == (dot, raster) {
                return;
            }
            self.tick();
        }
        panic!("({raster}, {dot}) に来ない");
    }

    /// 次のフレームの頭（ラスタの段取りをレジスタから求め直す）からの 1 フレーム分のピンの出力（ラスタ×342＋ドット）。
    fn frame(&mut self) -> Vec<DisplayOut> {
        self.tick();
        self.until(0, 0);
        let mut v = vec![DisplayOut::default(); LINE * RASTERS];
        for _ in 0..LINE * RASTERS {
            let o = self.tick();
            let (dot, raster) = self.a.beam();
            v[usize::from(raster) * LINE + usize::from(dot)] = o;
        }
        v
    }
}

/// ラスタ `raster` で読んだ語番地を、読んだ 2CLK の順に。
fn fetches(f: &[DisplayOut], raster: usize) -> Vec<(usize, u32)> {
    (0..LINE)
        .filter_map(|d| f[raster * LINE + d].fetch.map(|a| (d, a)))
        .collect()
}

fn span(f: &[DisplayOut], raster: usize, pin: fn(&DisplayOut) -> bool) -> Option<(usize, usize)> {
    let on: Vec<usize> = (0..LINE).filter(|&d| pin(&f[raster * LINE + d])).collect();
    Some((*on.first()?, *on.last()? + 1))
}

#[test]
fn display_reads_one_cycle_ahead_of_disp1() {
    let mut s = Screen::plan_b();
    let f = s.frame();
    // 前読み（U85 p.22 の図 2.1(d)、p.42 の図 3.11）：DISP1 の 1 表示サイクル前の 60 から 4 ドットごとに 65 語
    let r43 = fetches(&f, 43);
    assert_eq!(r43.len(), 65);
    assert_eq!(r43[0], (60, 0));
    assert_eq!(r43[64], (316, 64));
    assert!(
        r43.iter()
            .enumerate()
            .all(|(k, &(d, a))| d == 60 + 4 * k && a == k as u32)
    );
    assert_eq!(fetches(&f, 234)[0], (60, 191 * 0x40));
    assert!(fetches(&f, 42).is_empty() && fetches(&f, 235).is_empty());
    // DISP1 は 64〜319、ラスタ 43〜234（台帳 §3.4）。DISP2 は出ない
    assert_eq!(span(&f, 43, |o| o.disp1), Some((64, 320)));
    assert_eq!(span(&f, 234, |o| o.disp1), Some((64, 320)));
    assert_eq!(span(&f, 42, |o| o.disp1), None);
    assert!(f.iter().all(|o| !o.disp2 && !o.cud1 && !o.cud2 && !o.hole));
    // 属性は HSYNC（26 ドット）の最後のメモリサイクルの終わり（U85 p.55）。どのラスタにも出る
    assert!((0..RASTERS).all(|r| span(&f, r, |o| o.attr.is_some()) == Some((25, 26))));
    assert_eq!(s.a.frame().v_end, 235);
}

#[test]
fn dsk_delays_disp_by_memory_cycles() {
    // DSK（OMR bit 9–8）：DISP をメモリサイクル単位に遅らせる（U85 p.71）。読出しは変わらない
    let mut s = Screen::plan_b();
    s.reg(0x04, 0xC208);
    let f = s.frame();
    assert_eq!(span(&f, 43, |o| o.disp1), Some((68, 324)));
    assert_eq!(fetches(&f, 43)[0], (60, 0));
}

#[test]
fn splits_stack_upper_base_and_lower() {
    // 上画面（SP0＝16、SAR0＝$1000、MW $20）、基本画面（SP1＝32）、下画面（SP2＝8、SAR2＝$2000、MW $10）。
    // DCR：SE1＝1、SE0＝11、SE2＝11（U85 p.75–78）。SSW は r8C が SP0、r8A が SP1、r8E が SP2（U85 p.91）
    let mut s = Screen::plan_b();
    for (ar, v) in [
        (0x8C, 16),
        (0x8A, 32),
        (0x8E, 8),
        (0xC2, 0x20),
        (0xC4, 0x0000),
        (0xC6, 0x1000),
        (0xD2, 0x10),
        (0xD6, 0x2000),
        (0x06, 0x7C00),
    ] {
        s.reg(ar, v);
    }
    let f = s.frame();
    let first = |r: usize| fetches(&f, r).first().map(|&(_, a)| a);
    assert_eq!(first(43), Some(0x1000));
    assert_eq!(first(58), Some(0x1000 + 15 * 0x20));
    assert_eq!(first(59), Some(0));
    assert_eq!(first(90), Some(31 * 0x40));
    assert_eq!(first(91), Some(0x2000));
    assert_eq!(first(98), Some(0x2000 + 7 * 0x10));
    assert_eq!(first(99), None);
    assert_eq!(s.a.frame().v_end, 99);
    // 属性の SPL（MA17–16）は分割の番号 [V]
    let spl = |r: usize| (f[r * LINE + 25].attr.unwrap() >> 16) & 3;
    assert_eq!((spl(43), spl(59), spl(91)), (0, 1, 2));

    // SE0＝10（ブランク）：上画面は場所を取るが、読まず DISP1 も出さない
    s.reg(0x06, 0x6C00);
    let f = s.frame();
    assert!((43..59).all(|r| fetches(&f, r).is_empty() && span(&f, r, |o| o.disp1).is_none()));
    assert_eq!(fetches(&f, 59)[0], (60, 0));
    // SE0＝00（使わない）：基本画面が上へ詰まる
    s.reg(0x06, 0x4C00);
    let f = s.frame();
    assert_eq!(fetches(&f, 43)[0], (60, 0));
    assert_eq!(fetches(&f, 75)[0], (60, 0x2000));
    // SE1＝0：基本画面もブランク
    s.reg(0x06, 0x0000);
    let f = s.frame();
    assert!(f.iter().all(|o| o.fetch.is_none() && !o.disp1));
}

/// ウィンドウ：メモリサイクル 48 から 32 メモリサイクル（96〜159 ドット）、ラスタ 53 から 10 ラスタ、
/// SAR3＝$3000、MW3＝$10。HWR の HWS は HSYNC の終わりから（48−13＝35、書く値は 34）、VWR の VWS は
/// VSYNC の終わりから（53−3＝50、書く値は 49）（U85 p.83–84、p.89–90）。
fn windowed(dcr: u16) -> Screen {
    let mut s = Screen::plan_b();
    for (ar, v) in [
        (0x92, 0x221F),
        (0x94, 49),
        (0x96, 10),
        (0xDA, 0x10),
        (0xDE, 0x3000),
        (0x06, dcr),
    ] {
        s.reg(ar, v);
    }
    s
}

#[test]
fn window_takes_the_display_cycles_in_its_area() {
    let mut s = windowed(0x4300);
    let f = s.frame();
    let r53 = fetches(&f, 53);
    assert_eq!(r53.len(), 65);
    // 96〜156 の 16 サイクルはウィンドウ。背景の番地は、その間も進む [V]
    assert_eq!(r53[8], (92, 10 * 0x40 + 8));
    assert_eq!(r53[9], (96, 0x3000));
    assert_eq!(r53[24], (156, 0x300F));
    assert_eq!(r53[25], (160, 10 * 0x40 + 25));
    assert_eq!(fetches(&f, 62)[9], (96, 0x3000 + 9 * 0x10));
    assert_eq!(fetches(&f, 63)[9], (96, 20 * 0x40 + 9));
    assert_eq!(span(&f, 53, |o| o.disp2), Some((96, 160)));
    assert_eq!(span(&f, 52, |o| o.disp2), None);
    assert_eq!(span(&f, 63, |o| o.disp2), None);
    assert_eq!(span(&f, 53, |o| o.disp1), Some((64, 320)));

    // SE3＝10（ブランク）：ウィンドウの場所は穴になり、読まない（U85 p.78）
    let mut s = windowed(0x4200);
    let f = s.frame();
    let holes: Vec<usize> = (0..LINE).filter(|&d| f[53 * LINE + d].hole).collect();
    assert_eq!(holes, (96..160).step_by(4).collect::<Vec<_>>());
    assert_eq!(fetches(&f, 53).len(), 65 - 16);
    assert!(f.iter().all(|o| !o.disp2));
}

#[test]
fn zoom_repeats_addresses_and_rasters() {
    // ZFR（rEA）：HZF＝1（横 2 倍）、VZF＝2（縦 3 倍）。基本画面だけ（U85 p.110–111）
    let mut s = Screen::plan_b();
    s.reg(0xEA, 0x1200);
    let f = s.frame();
    let r43: Vec<u32> = fetches(&f, 43).iter().map(|&(_, a)| a).collect();
    assert_eq!(r43.len(), 65);
    assert!(r43.iter().enumerate().all(|(k, &a)| a == k as u32 / 2));
    assert_eq!(fetches(&f, 45)[0].1, 0);
    assert_eq!(fetches(&f, 46)[0].1, 0x40);
    assert_eq!(fetches(&f, 234)[0].1, 63 * 0x40);
    // 属性の HZ（MAD15–12）
    assert_eq!((f[43 * LINE + 25].attr.unwrap() >> 12) & 15, 1);
}

#[test]
fn attribute_carries_sda_hz_blink_and_atr() {
    // U85 p.55：MA19 BLINK2、MA18 BLINK1、MA17–16 SPL、MAD15–12 HZ、MAD11–8 HSD（SAR の SDA）、MAD7–0 ATR
    let mut s = Screen::plan_b();
    s.reg(0xCC, 0x0200); // SDA＝2
    s.reg(0xEA, 0x3000); // HZF＝3
    s.reg(0x06, 0x405A); // ATR＝$5A
    s.reg(0x90, 0x1000); // BON1＝1、BOFF1＝0：BLINK1 は常に High。BON2＝0：BLINK2 は常に Low（U85 p.92–93）
    let f = s.frame();
    assert_eq!(
        f[50 * LINE + 25].attr,
        Some(0x4_0000 | 0x1_0000 | 0x3000 | 0x0200 | 0x5A)
    );
    // 表示の外のラスタは分割がなく、SDA と HZ は 0 [V]
    assert_eq!(f[10 * LINE + 25].attr, Some(0x4_0000 | 0x5A));
}

#[test]
fn graphic_cursor_covers_cxs_to_cxe_and_cys_to_cye() {
    // CDR（rE8）：CM＝10（グラフィック）、CON1＝1、COFF1＝0（常に表示）。GCR：CXS 40、CXE 43（HSYNC の
    // 始まりからのメモリサイクル）、CYS 50、CYE 52（VSYNC の終わりからのラスタ）。両端を含む [V]（U85 p.94、p.107–109）
    let mut s = Screen::plan_b();
    for (ar, v) in [(0x98, 0x2B28), (0x9A, 50), (0x9C, 52), (0xE8, 0x8800)] {
        s.reg(ar, v);
    }
    let f = s.frame();
    for r in 53..=55 {
        assert_eq!(span(&f, r, |o| o.cud1), Some((80, 88)), "ラスタ {r}");
    }
    assert_eq!(span(&f, 52, |o| o.cud1), None);
    assert_eq!(span(&f, 56, |o| o.cud1), None);
    assert!(f.iter().all(|o| !o.cud2));
    // CSK＝01：1 メモリサイクル遅れる（U85 p.71）
    s.reg(0x04, 0xC408);
    let f = s.frame();
    assert_eq!(span(&f, 53, |o| o.cud1), Some((82, 90)));
    // 十字（CM＝11）：横の線が CUD1、縦の線が CUD2 [V]
    s.reg(0xE8, 0xC800);
    let f = s.frame();
    assert_eq!(span(&f, 54, |o| o.cud1), Some((0, LINE)));
    assert_eq!(span(&f, 100, |o| o.cud1), None);
    assert_eq!(span(&f, 100, |o| o.cud2), Some((82, 90)));
    // CSK＝00 では CUD2 は出ない
    s.reg(0x04, 0xC008);
    let f = s.frame();
    assert!(f.iter().all(|o| !o.cud2));
    // CON1＝0：出ない [V]
    s.reg(0xE8, 0x8000);
    let f = s.frame();
    assert!(f.iter().all(|o| !o.cud1));
}

/// フィールドごとに、ラスタ 53 の CUD1 と属性の BLINK1 を集める。
fn blink_pattern(s: &mut Screen, fields: usize) -> Vec<(bool, bool)> {
    (0..fields)
        .map(|_| {
            s.until(53, 25);
            let attr = s.a.display_out().attr.unwrap();
            s.until(53, 80);
            let cud = s.a.display_out().cud1;
            (cud, attr & 0x4_0000 != 0)
        })
        .collect()
}

#[test]
fn cursor_and_attribute_blink_in_units_of_four_fields() {
    // CON1＝1（8 フィールド）、COFF1＝2（12 フィールド）。BON1＝2（12）、BOFF1＝1（8）（U85 p.92–93、p.109）
    let mut s = Screen::plan_b();
    for (ar, v) in [
        (0x98, 0x2B28),
        (0x9A, 50),
        (0x9C, 52),
        (0xE8, 0x8A00),
        (0x90, 0x2100),
    ] {
        s.reg(ar, v);
    }
    let p = blink_pattern(&mut s, 40);
    for (n, &(cud, b1)) in p.iter().enumerate() {
        assert_eq!(cud, n % 20 < 8, "フィールド {n} の CUD1");
        assert_eq!(b1, n % 20 < 12, "フィールド {n} の BLINK1");
    }
}

/// ラスタ `from` の頭で `words` を書込み FIFO に入れ、終わるまでの書込みの位置（ラスタ、ドット）を集める。
fn timed_writes(s: &mut Screen, from: u16, words: &[u16]) -> Vec<(usize, usize)> {
    s.until(from, 0);
    s.put(words);
    let mut log = Vec::new();
    while !s.a.idle() {
        let n = s.m.writes;
        s.tick();
        if s.m.writes > n {
            let (dot, raster) = s.a.beam();
            log.push((usize::from(raster), usize::from(dot)));
        }
        assert!(log.len() < 1 << 20);
    }
    log
}

#[test]
fn drawing_waits_for_display_refresh_and_attribute_cycles() {
    // 4096 語の CLR（1 行）。式は (2×4096＋8)×1＋12＝8212 2CLK（U85 p.173）。メモリサイクルの前の内部の
    // 2CLK（20）は均等に配る [P]
    let mut s = Screen::plan_b();
    s.reg(0x02, 0x0200);
    let clr = [CLR, 0x1234, 4095, 0];
    let start = s.a.cycles();
    let log = timed_writes(&mut s, 100, &clr);
    let took = s.a.cycles() - start;
    assert_eq!(log.len(), 4096);
    assert!(took > 8212);
    // HSYNC の 26 ドットはリフレッシュ（OMR の RAM＝0、U85 p.72）で、描画のメモリサイクルは 26 から
    assert!(log.iter().all(|&(_, d)| d >= 27));
    // 表示の間は、表示サイクルの 2 つ目のメモリサイクル（60＋4k＋2、＋3）だけ（台帳 §3.2）
    assert!(
        log.iter()
            .all(|&(_, d)| !(60..320).contains(&d) || (d - 60) % 4 == 3)
    );
    // 表示のラスタの 1 本に入る語：HSYNC の後の 26〜59 で 17、表示の 65 サイクルで 65、320〜341 で 11。
    // 内部の 2CLK が 1 つ入ると、そのラスタは 1 語減る
    let per = |r: usize| log.iter().filter(|&&(rr, _)| rr == r).count();
    assert!(
        (101..140).all(|r| (92..=93).contains(&per(r))),
        "{}",
        per(110)
    );

    // 表示の外（ラスタ 236 から）：リフレッシュのほかは毎メモリサイクル。171−13＝158 語
    let log = timed_writes(&mut s, 236, &clr);
    assert!((237..255).all(|r| (157..=158).contains(&per2(&log, r))));
}

fn per2(log: &[(usize, usize)], r: usize) -> usize {
    log.iter().filter(|&&(rr, _)| rr == r).count()
}

#[test]
fn words_land_in_vram_at_their_memory_cycles() {
    // 描いている途中の VRAM は、済んだメモリサイクルの分だけ書き換わっている
    let mut s = Screen::plan_b();
    s.until(100, 0);
    s.put(&[CLR, 0xFFFF, 4095, 0]);
    s.until(110, 0);
    let done = s.m.m[..4096].iter().filter(|&&w| w == 0xFFFF).count();
    assert!(done > 0 && done < 4096);
    assert_eq!(done as u64, s.m.writes);
    assert_eq!(s.a.pending_writes(), 4096 - done);
    // 書いた語は番地の順に続く
    assert!(s.m.m[..done].iter().all(|&w| w == 0xFFFF));
}

#[test]
fn single_access_display_priority_and_drawing_priority() {
    // 単一アクセス（ACM＝00）：表示サイクルは 1 メモリサイクル。ACP＝0 では描画は帰線の間だけ、
    // ACP＝1 では描画が表示サイクルを取る（U85 p.70）
    let mut s = Screen::plan_b();
    s.reg(0x04, 0xC000);
    let clr = [CLR, 0x1234, 1023, 0];
    let log = timed_writes(&mut s, 100, &clr);
    assert!(log.iter().all(|&(_, d)| !(62..320).contains(&d)));
    let mut s = Screen::plan_b();
    s.reg(0x04, 0xE000);
    s.until(100, 0);
    s.put(&clr);
    let mut holes = 0;
    while !s.a.idle() {
        holes += usize::from(s.tick().hole);
    }
    assert!(holes > 0);
}

#[test]
fn str_off_resets_the_time_base() {
    // STR＝0：表示と描画を止め、内部の時間の基準を戻す（U85 p.70）
    let mut s = Screen::plan_b();
    s.until(100, 50);
    s.reg(0x04, 0x8008);
    s.tick();
    assert_eq!(s.a.beam(), (0, 0));
    // STR＝0 の間はリフレッシュの番地だけが出る（下の試験）
    let o = s.tick();
    assert_eq!(DisplayOut { refresh: None, ..o }, DisplayOut::default());
    s.reg(0x04, 0xC008);
    s.tick();
    assert_eq!(s.a.beam(), (1, 0));
}

#[test]
fn refresh_addresses_count_down_in_hsync_and_while_stopped() {
    // RAM＝0：HSYNC の L の間のメモリサイクルにリフレッシュの番地を出し、1 ずつ減らす。最後のサイクルは
    // 属性（U85 p.53、p.55、p.72）。案B の HSW＝13 なら、ドット 0、2、…、22 の 12 回（台帳 §3.4 の HSW−1）
    let mut s = Screen::plan_b();
    s.tick();
    s.until(0, 0);
    let mut seq = Vec::new();
    let mut dots = vec![Vec::new(); RASTERS];
    for _ in 0..LINE * RASTERS {
        let o = s.tick();
        let (d, r) = s.a.beam();
        if let Some(a) = o.refresh {
            dots[usize::from(r)].push(usize::from(d));
            seq.push(a);
        }
    }
    for (r, d) in dots.iter_mut().enumerate() {
        d.sort_unstable();
        assert_eq!(*d, (0..12).map(|k| 2 * k).collect::<Vec<_>>(), "{r}");
    }
    assert_eq!(seq.len(), 262 * 12);
    assert!(seq.windows(2).all(|w| w[1] == w[0].wrapping_sub(1)));
    // 128 行（HM4864A の A0〜A6）は 11 ラインで一巡する
    let rows: BTreeSet<u8> = seq[..11 * 12].iter().map(|a| a & 0x7F).collect();
    assert_eq!(rows.len(), 128);
    // STR＝0：毎メモリサイクル（2CLK×2）に出し続け、番地は止める前から続けて減る [V]
    let last = *seq.last().unwrap();
    s.reg(0x04, 0x8008);
    let stopped: Vec<Option<u8>> = (0..8).map(|_| s.tick().refresh).collect();
    assert!(stopped.chunks(2).all(|c| c[0].is_some() && c[1].is_none()));
    let got: Vec<u8> = stopped.iter().flatten().copied().collect();
    let want: Vec<u8> = (1..=4).map(|k| last.wrapping_sub(k)).collect();
    assert_eq!(got, want);
    // RAM＝1（SRAM）：リフレッシュしない
    let mut s = Screen::plan_b();
    s.reg(0x04, 0xC088);
    let f = s.frame();
    assert!(f.iter().all(|o| o.refresh.is_none()));
}

#[test]
fn display_and_pending_writes_survive_a_snapshot() {
    let mut s = windowed(0x4300);
    s.reg(0xEA, 0x1100);
    s.until(100, 0);
    s.put(&[CLR, 0x5A5A, 2047, 0]);
    s.until(105, 123);
    let saved = serde_json::to_string(&s.a).unwrap();
    let mut b: Hd63484 = serde_json::from_str(&saved).unwrap();
    assert_eq!(b, s.a);
    let mut m = Counted {
        m: s.m.m.clone(),
        writes: s.m.writes,
    };
    for _ in 0..LINE * RASTERS {
        s.tick();
        b.tick(&mut m);
        assert_eq!(b.display_out(), s.a.display_out());
    }
    assert_eq!(m.m, s.m.m);
    assert!(b.idle());
}
