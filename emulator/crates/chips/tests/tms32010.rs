//! TMS32010 の単体試験（仕様書 §12 段階 1）。
//! 期待値は台帳 `docs/parts/TMS32010.md` の記述（§3.3〜§3.7、§4.1〜§4.3、追補 A）と、
//! そこに引いた TI の資料（T1）の命令の説明から取る。

use fc85_chips::tms32010::{Bus, Cycle, Tms32010};

// ---- 命令の符号（台帳 §4.3 の上位バイトの表） ----

/// 直接アドレス。
const fn d(hi: u16, dma: u16) -> u16 {
    (hi << 8) | (dma & 0x7F)
}
/// 間接アドレス。`*`、`*+`、`*-`。ARP は変えない。
const IND: u16 = 0x88;
const IND_INC: u16 = 0xA8;
const IND_DEC: u16 = 0x98;
const fn ind(hi: u16, mode: u16) -> u16 {
    (hi << 8) | mode
}

const LAC: u16 = 0x20;
const ADD: u16 = 0x00;
const SUB: u16 = 0x10;
const SAR0: u16 = 0x30;
const LAR0: u16 = 0x38;
const IN: u16 = 0x40;
const OUT: u16 = 0x48;
const SACL: u16 = 0x50;
const SACH: u16 = 0x58;
const ADDH: u16 = 0x60;
const ADDS: u16 = 0x61;
const SUBH: u16 = 0x62;
const SUBC: u16 = 0x64;
const ZALH: u16 = 0x65;
const ZALS: u16 = 0x66;
const TBLR: u16 = 0x67;
const MAR: u16 = 0x68;
const DMOV: u16 = 0x69;
const LT: u16 = 0x6A;
const LTD: u16 = 0x6B;
const LTA: u16 = 0x6C;
const MPY: u16 = 0x6D;
const XOR: u16 = 0x78;
const AND: u16 = 0x79;
const OR: u16 = 0x7A;
const LST: u16 = 0x7B;
const SST: u16 = 0x7C;
const TBLW: u16 = 0x7D;

const fn lack(k: u16) -> u16 {
    0x7E00 | (k & 0xFF)
}
const fn lark(ar: u16, k: u16) -> u16 {
    0x7000 | (ar << 8) | (k & 0xFF)
}
const fn ldpk(k: u16) -> u16 {
    0x6E00 | (k & 1)
}
const fn larp(k: u16) -> u16 {
    0x6880 | (k & 1)
}
const fn mpyk(k: i16) -> u16 {
    0x8000 | (k as u16 & 0x1FFF)
}

const NOP: u16 = 0x7F80;
const DINT: u16 = 0x7F81;
const EINT: u16 = 0x7F82;
const ABS: u16 = 0x7F88;
const ZAC: u16 = 0x7F89;
const SOVM: u16 = 0x7F8B;
const CALA: u16 = 0x7F8C;
const RET: u16 = 0x7F8D;
const PAC: u16 = 0x7F8E;
const APAC: u16 = 0x7F8F;
const SPAC: u16 = 0x7F90;
const PUSH: u16 = 0x7F9C;
const POP: u16 = 0x7F9D;

const BANZ: u16 = 0xF400;
const BV: u16 = 0xF500;
const BIOZ: u16 = 0xF600;
const CALL: u16 = 0xF800;
const B: u16 = 0xF900;
const BZ: u16 = 0xFF00;

// ---- 試験用のバス ----

struct TestBus {
    prog: Vec<u16>,
    ports: [u16; 8],
    outs: Vec<(u8, u16)>,
    prog_writes: Vec<(u16, u16)>,
    bio_low: bool,
}

impl TestBus {
    fn new(prog: &[(u16, &[u16])]) -> Self {
        let mut p = vec![NOP; 4096];
        for &(at, words) in prog {
            for (i, &w) in words.iter().enumerate() {
                p[usize::from(at) + i] = w;
            }
        }
        Self {
            prog: p,
            ports: [0; 8],
            outs: Vec::new(),
            prog_writes: Vec::new(),
            bio_low: false,
        }
    }
}

impl Bus for TestBus {
    fn read_program(&mut self, addr: u16) -> u16 {
        assert!(addr < 0x1000, "番地は 12 bit");
        self.prog[usize::from(addr)]
    }
    fn write_program(&mut self, addr: u16, data: u16) {
        assert!(addr < 0x1000, "番地は 12 bit");
        self.prog[usize::from(addr)] = data;
        self.prog_writes.push((addr, data));
    }
    fn port_in(&mut self, port: u8) -> u16 {
        self.ports[usize::from(port)]
    }
    fn port_out(&mut self, port: u8, data: u16) {
        self.outs.push((port, data));
    }
    fn bio_low(&mut self) -> bool {
        self.bio_low
    }
}

/// 0 番地から置いたプログラムで、リセットを外した直後の状態を作る。
fn boot(prog: &[u16]) -> (Tms32010, TestBus) {
    boot_at(&[(0, prog)])
}

fn boot_at(prog: &[(u16, &[u16])]) -> (Tms32010, TestBus) {
    let mut bus = TestBus::new(prog);
    let mut cpu = Tms32010::new();
    assert_eq!(cpu.tick(&mut bus), Cycle::Reset);
    cpu.set_rs(false);
    assert_eq!(cpu.tick(&mut bus), Cycle::Restart);
    (cpu, bus)
}

/// 命令 1 つ（または割込みの受付）を終えるまで進め、かかったサイクル数を返す。
fn step(cpu: &mut Tms32010, bus: &mut TestBus) -> u32 {
    let first = cpu.tick(bus);
    assert!(
        matches!(first, Cycle::Exec { .. } | Cycle::Interrupt),
        "命令の途中から進めた: {first:?}"
    );
    let mut n = 1;
    while cpu.mid_instruction() {
        assert_eq!(cpu.tick(bus), Cycle::Busy);
        n += 1;
    }
    n
}

fn run(cpu: &mut Tms32010, bus: &mut TestBus, count: usize) {
    for _ in 0..count {
        step(cpu, bus);
    }
}

// ---- リセット（台帳 §3.5） ----

#[test]
fn reset_sets_pc_and_intm_but_keeps_ovm() {
    let (mut cpu, mut bus) = boot(&[SOVM, EINT, NOP, NOP]);
    run(&mut cpu, &mut bus, 3);
    assert!(cpu.ovm());
    assert!(!cpu.intm());
    assert_eq!(cpu.pc(), 3);

    cpu.set_rs(true);
    assert_eq!(cpu.tick(&mut bus), Cycle::Reset);
    assert_eq!(cpu.tick(&mut bus), Cycle::Reset);
    assert_eq!(cpu.pc(), 0);
    assert!(cpu.intm(), "リセットで INTM＝1");
    assert!(cpu.ovm(), "OVM はリセットで変わらない");

    cpu.set_rs(false);
    assert_eq!(
        cpu.tick(&mut bus),
        Cycle::Restart,
        "解除から 1 周期後に再開"
    );
    assert_eq!(
        cpu.tick(&mut bus),
        Cycle::Exec {
            pc: 0,
            opcode: SOVM
        }
    );
}

// ---- サイクル数（台帳 §3.7、追補 A、DR-032） ----

#[test]
fn cycle_counts_follow_ti_table() {
    let (mut cpu, mut bus) = boot(&[
        lack(1),    // 0: 1
        PUSH,       // 1: 2
        POP,        // 2: 2
        d(IN, 0),   // 3: 2
        d(OUT, 0),  // 4: 2
        B,          // 5: 2（2 語）
        8,          //
        NOP,        // 7: 飛ばす
        BZ,         // 8: 2（不成立でも 2）
        20,         //
        d(TBLR, 0), // 10: 3
        d(TBLW, 1), // 11: 3
        lack(15),   // 12: 1
        CALA,       // 13: 2
        NOP,        // 14: 1（RET の戻り先）
        RET,        // 15: 2
    ]);
    let mut got = Vec::new();
    for _ in 0..13 {
        let pc = cpu.pc();
        got.push((pc, step(&mut cpu, &mut bus)));
    }
    assert_eq!(
        got,
        vec![
            (0, 1),
            (1, 2),
            (2, 2),
            (3, 2),
            (4, 2),
            (5, 2),
            (8, 2),
            (10, 3),
            (11, 3),
            (12, 1),
            (13, 2),
            (15, 2),
            (14, 1),
        ]
    );
}

#[test]
fn branch_taken_or_not_is_two_cycles() {
    let (mut cpu, mut bus) = boot(&[BZ, 10, lack(1), BZ, 10]);
    assert_eq!(step(&mut cpu, &mut bus), 2);
    assert_eq!(cpu.pc(), 10, "ACC＝0 で成立");

    let (mut cpu, mut bus) = boot(&[lack(1), BZ, 10, NOP]);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(step(&mut cpu, &mut bus), 2);
    assert_eq!(cpu.pc(), 3, "不成立は 2 語を飛ばす");
}

// ---- 演算（台帳 §4.3、T1 の各命令の説明） ----

#[test]
fn mpy_of_8000_squared_gives_c0000000() {
    let (mut cpu, mut bus) = boot(&[d(LT, 0), d(MPY, 0), d(LT, 1), d(MPY, 2)]);
    cpu.set_data(0, 0x8000);
    cpu.set_data(1, (-2i16) as u16);
    cpu.set_data(2, 3);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(
        cpu.p(),
        0xC000_0000,
        "台帳 §4.3：正しい値（0x40000000）は出ない"
    );
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.p(), (-6i32) as u32);
}

#[test]
fn mpyk_takes_a_signed_13_bit_constant() {
    let (mut cpu, mut bus) = boot(&[d(LT, 0), mpyk(-4096), PAC, mpyk(4095), PAC]);
    cpu.set_data(0, 2);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.acc(), (-8192i32) as u32);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.acc(), 8190);
}

#[test]
fn pac_apac_spac_lta_use_the_product() {
    let (mut cpu, mut bus) = boot(&[d(LT, 0), mpyk(5), PAC, APAC, SPAC, d(LTA, 1)]);
    cpu.set_data(0, 3);
    cpu.set_data(1, 9);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.acc(), 15);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 30);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 15);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 30, "LTA は前の積を足す");
    assert_eq!(cpu.t(), 9);
}

#[test]
fn abs_of_80000000_depends_on_ovm() {
    let prog = [d(ZALH, 0), ABS, d(LAC, 1), ABS, SOVM, d(ZALH, 0), ABS];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(0, 0x8000);
    cpu.set_data(1, (-5i16) as u16);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.acc(), 0x8000_0000, "OVM＝0 ではそのまま");
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.acc(), 5);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.acc(), 0x7FFF_FFFF, "OVM＝1 では最大の正の値");
}

#[test]
fn overflow_sets_ov_and_ovm_saturates() {
    // 0x7FFF0000 + 0x7FFF0000 は正の側に、0x80000000 − 0x00010000 は負の側にあふれる。
    let prog = [
        d(ZALH, 0),
        d(ADDH, 0),
        SOVM,
        d(ZALH, 0),
        d(ADDH, 0),
        d(ZALH, 1),
        d(SUBH, 2),
    ];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(0, 0x7FFF);
    cpu.set_data(1, 0x8000);
    cpu.set_data(2, 1);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.acc(), 0xFFFE_0000, "OVM＝0 では回り込む");
    assert!(cpu.ov());
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.acc(), 0x7FFF_FFFF);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.acc(), 0x8000_0000);
}

#[test]
fn ov_stays_set_until_bv() {
    // T1 §2.2.2.1：OV は BV か状態語の書き換えでしか消えない。
    let prog = [
        d(ZALH, 0),
        d(ADDH, 0),
        lack(1),
        BV,
        10,
        NOP,
        NOP,
        NOP,
        NOP,
        NOP,
        BV,
        20,
    ];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(0, 0x7FFF);
    run(&mut cpu, &mut bus, 3);
    assert!(cpu.ov(), "あふれない演算では消えない");
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.pc(), 10);
    assert!(!cpu.ov(), "BV で分岐したら消える");
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.pc(), 12, "2 回目の BV は分岐しない");
}

#[test]
fn shifts_and_sign_extension() {
    let prog = [
        d(LAC + 4, 0), // 0xFFFF を 4 bit 左へ、符号拡張
        ZAC,
        d(ADDS, 0), // 符号拡張なし
        lack(0xFF),
        d(ZALS, 0),
        d(ZALH, 1),
        d(ADDS, 2),
        d(SACH, 3),
        d(SACH + 1, 4),
        d(SACH + 4, 5),
        d(SACL, 6),
        d(ADD + 15, 7),
        d(SUB + 15, 7),
    ];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(0, 0xFFFF);
    cpu.set_data(1, 0x1234);
    cpu.set_data(2, 0x5678);
    cpu.set_data(7, 1);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 0xFFFF_FFF0);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.acc(), 0x0000_FFFF);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 0xFF, "LACK は符号拡張しない");
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 0x0000_FFFF);
    run(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.acc(), 0x1234_5678);
    assert_eq!(cpu.data(3), 0x1234, "SACH シフト 0");
    assert_eq!(cpu.data(4), 0x2468, "SACH シフト 1");
    assert_eq!(cpu.data(5), 0x2345, "SACH シフト 4");
    assert_eq!(cpu.data(6), 0x5678);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 0x1234_5678 + 0x8000);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 0x1234_5678);
}

#[test]
fn logic_ops_touch_only_the_low_half() {
    let prog = [d(ZALH, 0), d(ADDS, 1), d(OR, 2), d(XOR, 3), d(AND, 4)];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(0, 0x1234);
    cpu.set_data(1, 0x5678);
    cpu.set_data(2, 0x000F);
    cpu.set_data(3, 0xFFFF);
    cpu.set_data(4, 0x0FF0);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.acc(), 0x1234_567F);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 0x1234_A980);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.acc(), 0x0000_0980, "AND は上位を 0 にする");
}

#[test]
fn sixteen_subc_divide() {
    // T1 の SUBC：正の被除数を ACC に置き、16 回で下位に商、上位に余りが残る。
    let mut prog = vec![d(ZALS, 0)];
    prog.extend([d(SUBC, 1); 16]);
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(0, 1000);
    cpu.set_data(1, 7);
    run(&mut cpu, &mut bus, 17);
    assert_eq!(cpu.acc() & 0xFFFF, 142);
    assert_eq!(cpu.acc() >> 16, 6);
}

#[test]
fn three_tap_fir_with_ltd_and_mpy() {
    // y[n] = h0·x[n] + h1·x[n−1] + h2·x[n−2]。係数と入出力は Q15。
    // 遅延線は 10〜12 番地、係数は 20〜22 番地。LTD が遅延線を 1 つずらす（台帳 §4.3）。
    let prog = [
        d(IN, 10), // x[n] をポート 0 から
        ZAC,
        d(LT, 12),
        d(MPY, 22),
        d(LTD, 11),
        d(MPY, 21),
        d(LTD, 10),
        d(MPY, 20),
        APAC,
        d(SACH + 1, 30), // Q30 → Q15
        d(OUT + 1, 30),
        B,
        0,
    ];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(20, 0x4000); // 0.5
    cpu.set_data(21, 0x2000); // 0.25
    cpu.set_data(22, 0x1000); // 0.125
    for x in [0x4000, 0, 0, 0] {
        bus.ports[0] = x;
        let cycles: u32 = (0..12).map(|_| step(&mut cpu, &mut bus)).sum();
        assert_eq!(cycles, 15, "1 標本 15 サイクル（§3.7 の表から計算）");
        assert_eq!(cpu.pc(), 0);
    }
    // 0.5 のインパルスへの応答：0.25、0.125、0.0625、0
    assert_eq!(
        bus.outs,
        vec![(1, 0x2000), (1, 0x1000), (1, 0x0800), (1, 0)]
    );
}

// ---- アドレスとレジスタ（台帳 §4.1、§4.2） ----

#[test]
fn indirect_uses_low_8_bits_and_steps_in_9_bits() {
    let prog = [
        d(LAR0, 0),
        ind(LAC, IND),
        ind(MAR, IND_INC),
        d(LAR0, 1),
        ind(MAR, IND_INC),
        ind(MAR, IND_DEC),
        d(LAR0, 2),
        ind(MAR, IND_INC),
    ];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(0, 0x0105);
    cpu.set_data(1, 0x01FF);
    cpu.set_data(2, 0xFFFF);
    cpu.set_data(5, 0x0ABC);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.acc(), 0x0ABC, "番地は AR の下位 8 bit");
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.ar(0), 0x0106);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.ar(0), 0x0000, "下位 9 bit で回る");
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.ar(0), 0x01FF);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.ar(0), 0xFE00, "上位 7 bit は変わらない");
}

#[test]
fn indirect_can_load_the_next_arp() {
    // bit3＝0 のとき bit0 が次の ARP。LARP は MAR の形。
    let prog = [
        lark(0, 10),
        lark(1, 20),
        larp(1),
        ind(SAR0, IND_INC & !0x08), // AR1 の番地へ AR0 を書き、AR1 を増やし、ARP＝0（bit0）
        larp(1),
        ind(MAR, IND),
    ];
    let (mut cpu, mut bus) = boot(&prog);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.arp(), 1);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.data(20), 10);
    assert_eq!(cpu.ar(1), 21);
    assert_eq!(cpu.arp(), 0);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.arp(), 1, "bit3＝1 なら ARP は変わらない");
}

#[test]
fn sar_of_the_stepped_ar_stores_the_new_value() {
    // T3 3-55〜3-56 の WARNING の例：AR0＝10 で SAR AR0,*+ は 10 番地に 11、SAR AR0,*- は 9 を書く
    let prog = [
        larp(0),
        lark(0, 10),
        ind(SAR0, IND_INC),
        lark(0, 10),
        ind(SAR0, IND_DEC),
    ];
    let (mut cpu, mut bus) = boot(&prog);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.data(10), 11);
    assert_eq!(cpu.ar(0), 11);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.data(10), 9);
    assert_eq!(cpu.ar(0), 9);
}

#[test]
fn banz_tests_9_bits_and_always_decrements() {
    let prog = [d(LAR0, 0), BANZ, 10, d(LAR0, 1), BANZ, 20];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(0, 0x0200);
    cpu.set_data(1, 1);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc(), 3, "下位 9 bit が 0 なら分岐しない");
    assert_eq!(cpu.ar(0), 0x03FF, "不成立でも減らす");
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.pc(), 20);
    assert_eq!(cpu.ar(0), 0);
}

#[test]
fn dp_selects_page_and_sst_always_uses_page_1() {
    let prog = [
        lack(0x5A),
        SOVM,
        larp(1),
        ldpk(1),
        d(SACL, 3),
        d(SST, 5),
        ldpk(0),
        d(SST, 6),
    ];
    let (mut cpu, mut bus) = boot(&prog);
    run(&mut cpu, &mut bus, 6);
    assert_eq!(cpu.data(0x83), 0x5A, "DP＝1 で 128 番地から");
    assert_eq!(cpu.data(3), 0);
    // OV＝0、OVM＝1、INTM＝1、ARP＝1、DP＝1、ほかの bit は 1
    assert_eq!(cpu.data(0x85), 0x7FFF);
    assert_eq!(cpu.status(), 0x7FFF);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.data(0x86), 0x7FFE, "DP＝0 でもページ 1");
    assert_eq!(cpu.data(6), 0);
}

#[test]
fn lst_restores_status_but_not_intm() {
    let (mut cpu, mut bus) = boot(&[EINT, d(LST, 0), NOP]);
    cpu.set_data(0, 0xA101); // OV＝1、OVM＝0、INTM＝1、ARP＝1、DP＝1
    run(&mut cpu, &mut bus, 2);
    assert!(cpu.ov());
    assert!(!cpu.ovm());
    assert!(!cpu.intm(), "LST は INTM を変えない");
    assert_eq!(cpu.arp(), 1);
    assert_eq!(cpu.dp(), 1);
}

#[test]
fn dmov_and_ltd_shift_the_delay_line() {
    let prog = [d(DMOV, 10), d(LT, 30), mpyk(2), d(LTD, 20)];
    let (mut cpu, mut bus) = boot(&prog);
    cpu.set_data(10, 0x1234);
    cpu.set_data(20, 7);
    cpu.set_data(30, 4);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.data(11), 0x1234);
    assert_eq!(cpu.data(10), 0x1234);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.t(), 7);
    assert_eq!(cpu.acc(), 8, "LTD は前の積を足す");
    assert_eq!(cpu.data(21), 7);
}

#[test]
fn stack_is_four_deep_and_loses_the_oldest() {
    let mut prog = Vec::new();
    for k in 1..=5 {
        prog.extend([lack(k), PUSH]);
    }
    prog.extend([POP; 5]);
    let (mut cpu, mut bus) = boot(&prog);
    run(&mut cpu, &mut bus, 10);
    assert_eq!(cpu.stack(), [5, 4, 3, 2]);
    let mut popped = Vec::new();
    for _ in 0..5 {
        run(&mut cpu, &mut bus, 1);
        popped.push(cpu.acc());
    }
    assert_eq!(popped, vec![5, 4, 3, 2, 2]);
}

#[test]
fn push_keeps_the_low_12_bits() {
    let (mut cpu, mut bus) = boot(&[d(LAC, 0), PUSH, ZAC, POP]);
    cpu.set_data(0, 0xF123);
    run(&mut cpu, &mut bus, 4);
    assert_eq!(cpu.acc(), 0x0123);
}

#[test]
fn call_and_ret() {
    let (mut cpu, mut bus) = boot_at(&[(0, &[CALL, 10, NOP]), (10, &[RET])]);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.pc(), 10);
    assert_eq!(cpu.stack()[0], 2);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.pc(), 2);
}

// ---- I/O と表（台帳 §2.2、§3.3、§3.4） ----

#[test]
fn in_and_out_use_the_port_field() {
    let (mut cpu, mut bus) = boot(&[d(IN + 3, 0), d(OUT + 1, 0), d(OUT + 2, 1)]);
    bus.ports[3] = 0xABCD;
    cpu.set_data(1, 0x0042);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.data(0), 0xABCD);
    assert_eq!(bus.outs, vec![(1, 0xABCD), (2, 0x0042)]);
}

#[test]
fn tblr_and_tblw_use_acc_low_12_bits_and_one_stack_level() {
    let prog = [d(LAC, 0), d(TBLR, 4), d(TBLW, 5), NOP];
    let (mut cpu, mut bus) = boot_at(&[(0, &prog), (0x123, &[0x5A5A])]);
    cpu.set_data(0, 0xF123);
    cpu.set_data(5, 0x1111);
    run(&mut cpu, &mut bus, 2);
    assert_eq!(cpu.data(4), 0x5A5A);
    assert_eq!(cpu.pc(), 2, "表の後は次の命令へ戻る");
    run(&mut cpu, &mut bus, 1);
    assert_eq!(bus.prog_writes, vec![(0x123, 0x1111)]);
    assert_eq!(cpu.pc(), 3);
}

#[test]
fn tblr_with_a_full_stack_loses_the_oldest() {
    let mut prog = Vec::new();
    for k in 1..=4 {
        prog.extend([lack(k), PUSH]);
    }
    prog.extend([d(TBLR, 0), POP, POP, POP, POP]);
    let (mut cpu, mut bus) = boot(&prog);
    run(&mut cpu, &mut bus, 9);
    assert_eq!(cpu.stack(), [4, 3, 2, 2]);
}

#[test]
fn bioz_branches_when_bio_is_low() {
    let (mut cpu, mut bus) = boot(&[BIOZ, 10, BIOZ, 20]);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.pc(), 2);
    bus.bio_low = true;
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.pc(), 20);
}

// ---- 割込み（台帳 §3.6、追補 A） ----

/// 0: B 4、2: 割込みの入口（B 30）、4 以降は `body`。
fn boot_with_vector(body: &[u16]) -> (Tms32010, TestBus) {
    boot_at(&[(0, &[B, 4, B, 30]), (4, body)])
}

#[test]
fn interrupt_jumps_to_2_with_one_dummy_cycle() {
    let (mut cpu, mut bus) = boot_with_vector(&[EINT, NOP, NOP, NOP]);
    run(&mut cpu, &mut bus, 2);
    cpu.set_int(false);
    run(&mut cpu, &mut bus, 1); // EINT の次の命令は待たせる
    assert_eq!(cpu.pc(), 6);
    assert_eq!(cpu.tick(&mut bus), Cycle::Interrupt);
    assert!(!cpu.mid_instruction(), "受付は 1 サイクル");
    assert_eq!(cpu.pc(), 2);
    assert_eq!(cpu.stack()[0], 6);
    assert!(cpu.intm());
    assert_eq!(
        cpu.tick(&mut bus),
        Cycle::Exec { pc: 2, opcode: B },
        "次のサイクルで 2 番地を実行"
    );
}

#[test]
fn interrupt_waits_one_instruction_after_mpy() {
    let (mut cpu, mut bus) = boot_with_vector(&[EINT, NOP, d(MPY, 0), NOP, NOP]);
    run(&mut cpu, &mut bus, 4);
    cpu.set_int(false);
    run(&mut cpu, &mut bus, 1);
    assert_eq!(cpu.pc(), 8, "MPY の次の命令は実行する");
    assert_eq!(cpu.tick(&mut bus), Cycle::Interrupt);
    assert_eq!(cpu.stack()[0], 8);
}

#[test]
fn interrupt_waits_for_a_multi_cycle_instruction() {
    let (mut cpu, mut bus) = boot_with_vector(&[EINT, NOP, PUSH, NOP]);
    run(&mut cpu, &mut bus, 3);
    assert!(matches!(cpu.tick(&mut bus), Cycle::Exec { pc: 6, .. }));
    cpu.set_int(false);
    assert_eq!(cpu.tick(&mut bus), Cycle::Busy);
    assert_eq!(cpu.tick(&mut bus), Cycle::Interrupt);
    assert_eq!(cpu.stack()[0], 7);
}

#[test]
fn int_edge_is_latched_while_disabled() {
    let (mut cpu, mut bus) = boot_with_vector(&[DINT, NOP, EINT, NOP, NOP]);
    run(&mut cpu, &mut bus, 2);
    cpu.set_int(false);
    cpu.set_int(true);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.pc(), 8);
    assert_eq!(cpu.tick(&mut bus), Cycle::Interrupt, "INTF に残っていた");
    assert_eq!(cpu.stack()[0], 8);
}

#[test]
fn int_held_low_requests_again() {
    let (mut cpu, mut bus) = boot_at(&[
        (0, &[B, 4, B, 8]),
        (4, &[EINT, NOP]),
        (8, &[EINT, NOP, NOP]),
    ]);
    cpu.set_int(false);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(cpu.tick(&mut bus), Cycle::Interrupt);
    run(&mut cpu, &mut bus, 3);
    assert_eq!(
        cpu.tick(&mut bus),
        Cycle::Interrupt,
        "Low のままなら再び要求"
    );
    assert_eq!(cpu.stack()[..2], [10, 6]);
}

// ---- その他 ----

#[test]
fn undefined_opcode_runs_as_one_cycle_nop() {
    let (mut cpu, mut bus) = boot(&[0xA000, NOP]);
    assert_eq!(step(&mut cpu, &mut bus), 1);
    assert_eq!(cpu.pc(), 1);
    assert_eq!(cpu.illegal_count(), 1);
}

#[test]
fn snapshot_round_trip() {
    let (mut cpu, mut bus) = boot(&[d(LT, 0), mpyk(5), d(TBLR, 1), PAC, NOP]);
    cpu.set_data(0, 3);
    run(&mut cpu, &mut bus, 2);
    assert!(matches!(cpu.tick(&mut bus), Cycle::Exec { .. }));
    assert!(cpu.mid_instruction(), "TBLR の途中で保存する");
    let json = serde_json::to_string(&cpu).unwrap();
    let mut back: Tms32010 = serde_json::from_str(&json).unwrap();
    assert_eq!(back, cpu);
    let mut bus2 = TestBus::new(&[(0, &[d(LT, 0), mpyk(5), d(TBLR, 1), PAC, NOP])]);
    for _ in 0..4 {
        assert_eq!(cpu.tick(&mut bus), back.tick(&mut bus2));
    }
    assert_eq!(back, cpu);
}
