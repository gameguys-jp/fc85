//! HD68000 の単体試験（仕様書 §12 段階 3）。ダウンロードしたデータなしで走る。
//! 期待値は台帳 `docs/parts/HD68000.md` の §2.3〜§2.9（M68 表7-3〜7-16、DB85 表39）から手で数える。
//! 台帳で [V] の動作（例外の読み書きの数、I/N の極性など）は `docs/impl/emulator.md` §03 の扱いに
//! 合わせる。SingleStepTests との照合は `hd68000_sst.rs`。

use fc85_chips::hd68000::{Bus, BusReply, Hd68000, IackReply, RunState, Size, fc};

/// 試験の基板の RAM（000000–00FFFF）。ほかの番地はバスエラーになる。
const RAM_SIZE: u32 = 0x1_0000;
/// バスエラーのサイクルを打ち切るまでのクロック数（試験の基板の値）。
const BERR_CLOCKS: u32 = 8;
/// 番地を割り当てていない領域。
const UNMAPPED: u32 = 0x00f0_0000;
const SSP: u32 = 0x8000;
const START: u32 = 0x1000;

/// 例外 `vec` の処理ルーチンの番地。どれも NOP を 8 つ置く。
const fn handler(vec: u32) -> u32 {
    0x4000 + vec * 0x10
}

const NOP: u16 = 0x4e71;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ev {
    Read {
        at: u64,
        addr: u32,
        fc: u8,
    },
    Write {
        at: u64,
        addr: u32,
        fc: u8,
        value: u16,
    },
    Iack {
        at: u64,
        level: u8,
    },
    Reset {
        at: u64,
    },
}

struct Board {
    ram: Vec<u8>,
    log: Vec<Ev>,
    /// どのサイクルにも入れる待ち。
    waits: u32,
    ipl: u8,
    iack: IackReply,
}

impl Board {
    fn new() -> Self {
        Self {
            ram: vec![0; RAM_SIZE as usize],
            log: Vec::new(),
            waits: 0,
            ipl: 0,
            iack: IackReply::Autovector { waits: 0 },
        }
    }

    fn w16(&mut self, addr: u32, v: u16) {
        let a = addr as usize;
        self.ram[a] = (v >> 8) as u8;
        self.ram[a + 1] = v as u8;
    }

    fn w32(&mut self, addr: u32, v: u32) {
        self.w16(addr, (v >> 16) as u16);
        self.w16(addr + 2, v as u16);
    }

    fn r16(&self, addr: u32) -> u16 {
        let a = addr as usize;
        u16::from(self.ram[a]) << 8 | u16::from(self.ram[a + 1])
    }

    fn r32(&self, addr: u32) -> u32 {
        u32::from(self.r16(addr)) << 16 | u32::from(self.r16(addr + 2))
    }

    fn reads(&self) -> usize {
        self.log
            .iter()
            .filter(|e| matches!(e, Ev::Read { .. }))
            .count()
    }

    fn writes(&self) -> usize {
        self.log
            .iter()
            .filter(|e| matches!(e, Ev::Write { .. }))
            .count()
    }
}

impl Bus for Board {
    fn read(&mut self, at: u64, addr: u32, fc: u8, size: Size) -> BusReply {
        self.log.push(Ev::Read { at, addr, fc });
        if addr >= RAM_SIZE {
            return BusReply::BusError {
                clocks: BERR_CLOCKS,
            };
        }
        let data = match size {
            Size::Byte => u16::from(self.ram[addr as usize]),
            Size::Word => self.r16(addr),
        };
        BusReply::Ok {
            data,
            waits: self.waits,
        }
    }

    fn write(&mut self, at: u64, addr: u32, fc: u8, size: Size, value: u16) -> BusReply {
        self.log.push(Ev::Write {
            at,
            addr,
            fc,
            value,
        });
        if addr >= RAM_SIZE {
            return BusReply::BusError {
                clocks: BERR_CLOCKS,
            };
        }
        match size {
            Size::Byte => self.ram[addr as usize] = value as u8,
            Size::Word => self.w16(addr, value),
        }
        BusReply::Ok {
            data: 0,
            waits: self.waits,
        }
    }

    fn iack(&mut self, at: u64, level: u8) -> IackReply {
        self.log.push(Ev::Iack { at, level });
        self.iack
    }

    fn ipl(&mut self, _at: u64) -> u8 {
        self.ipl
    }

    fn reset_out(&mut self, at: u64) {
        self.log.push(Ev::Reset { at });
    }
}

/// ベクタ表（SSP、PC、例外 2〜255）と処理ルーチンを置き、`START` に `prog` を置いてリセットする。
/// リセットのバスサイクルは記録から消す。
fn boot(prog: &[u16]) -> (Hd68000, Board) {
    let mut b = Board::new();
    b.w32(0, SSP);
    b.w32(4, START);
    for vec in 2..256 {
        b.w32(vec * 4, handler(vec));
        for i in 0..8 {
            b.w16(handler(vec) + i * 2, NOP);
        }
    }
    for (i, &w) in prog.iter().enumerate() {
        b.w16(START + 2 * i as u32, w);
    }
    let mut cpu = Hd68000::new();
    cpu.reset(&mut b);
    b.log.clear();
    (cpu, b)
}

/// 1 命令を実行し、クロック数を返す。バスサイクルの記録は命令ごとに消す。
fn step(cpu: &mut Hd68000, b: &mut Board) -> u32 {
    b.log.clear();
    cpu.step(b)
}

/// グループ 1・2 の例外の 3 語のフレーム（SR、PC）を SSP から読む。
fn frame(cpu: &Hd68000, b: &Board) -> (u16, u32) {
    (b.r16(cpu.ssp), b.r32(cpu.ssp + 2))
}

// ------------------------------------------------------------------ リセット

/// 台帳 §2.7・§2.8：SSP を 0 番地、PC を 4 番地からスーパーバイザのプログラム空間で読み、最初の 2 語を
/// 読む。M68 表7-16 の 40(6/0)（DB85 表39 の 34 とは食い違う [V]。§03）。
#[test]
fn reset_reads_ssp_pc_and_prefetch_in_40_clocks() {
    let mut b = Board::new();
    b.w32(0, 0x0000_7ff0);
    b.w32(4, 0x0000_2000);
    b.w16(0x2000, 0x1234);
    b.w16(0x2002, 0x5678);
    let mut cpu = Hd68000::new();
    cpu.reset(&mut b);
    let p = fc::SUPERVISOR_PROGRAM;
    assert_eq!(
        b.log,
        vec![
            Ev::Read {
                at: 14,
                addr: 0,
                fc: p
            },
            Ev::Read {
                at: 18,
                addr: 2,
                fc: p
            },
            Ev::Read {
                at: 22,
                addr: 4,
                fc: p
            },
            Ev::Read {
                at: 26,
                addr: 6,
                fc: p
            },
            Ev::Read {
                at: 30,
                addr: 0x2000,
                fc: p
            },
            Ev::Read {
                at: 36,
                addr: 0x2002,
                fc: p
            },
        ]
    );
    assert_eq!(cpu.clock, 40);
    assert_eq!(cpu.ssp, 0x7ff0);
    assert_eq!(cpu.pc, 0x2000);
    assert_eq!(cpu.prefetch, [0x1234, 0x5678]);
    assert_eq!(cpu.sr, 0x2700);
    assert_eq!(cpu.state, RunState::Running);
}

// ------------------------------------------------------------------ 命令の時間

struct Case {
    name: &'static str,
    prog: &'static [u16],
    setup: fn(&mut Hd68000, &mut Board),
    clocks: u32,
    reads: usize,
    writes: usize,
}

fn no_setup(_: &mut Hd68000, _: &mut Board) {}

/// M68 表7-4〜7-15 の値（括弧内は読出し数／書込み数）。
#[test]
fn instruction_times_follow_the_tables() {
    let cases = [
        // 表7-14
        Case {
            name: "NOP",
            prog: &[NOP],
            setup: no_setup,
            clocks: 4,
            reads: 1,
            writes: 0,
        },
        // 表7-4・7-5
        Case {
            name: "MOVE.w D0,D1",
            prog: &[0x3200],
            setup: no_setup,
            clocks: 4,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "MOVE.l #,D1",
            prog: &[0x223c, 0x1234, 0x5678],
            setup: no_setup,
            clocks: 12,
            reads: 3,
            writes: 0,
        },
        Case {
            name: "MOVE.w (A0),(A1)",
            prog: &[0x3290],
            setup: |c, _| {
                c.a[0] = 0x2000;
                c.a[1] = 0x3000;
            },
            clocks: 12,
            reads: 2,
            writes: 1,
        },
        Case {
            name: "MOVE.l (A0)+,-(A1)",
            prog: &[0x2318],
            setup: |c, _| {
                c.a[0] = 0x2000;
                c.a[1] = 0x3000;
            },
            clocks: 20,
            reads: 3,
            writes: 2,
        },
        // 表7-6：ロングで源がレジスタなら 6 が 8 になる。
        Case {
            name: "ADD.l D0,D1",
            prog: &[0xd280],
            setup: no_setup,
            clocks: 8,
            reads: 1,
            writes: 0,
        },
        // 表7-7
        Case {
            name: "ADDQ.w #1,D0",
            prog: &[0x5240],
            setup: no_setup,
            clocks: 4,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "ADDQ.l #1,D0",
            prog: &[0x5280],
            setup: no_setup,
            clocks: 8,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "ADDQ.l #1,A0",
            prog: &[0x5288],
            setup: no_setup,
            clocks: 8,
            reads: 1,
            writes: 0,
        },
        // 表7-12
        Case {
            name: "LEA (d16,A0),A1",
            prog: &[0x43e8, 0x0010],
            setup: no_setup,
            clocks: 8,
            reads: 2,
            writes: 0,
        },
        Case {
            name: "JMP (A0)",
            prog: &[0x4ed0],
            setup: |c, _| c.a[0] = 0x2000,
            clocks: 8,
            reads: 2,
            writes: 0,
        },
        Case {
            name: "JSR (xxx).L",
            prog: &[0x4eb9, 0x0000, 0x2000],
            setup: no_setup,
            clocks: 20,
            reads: 3,
            writes: 2,
        },
        Case {
            name: "MOVEM.l D0-D1/A0,-(A7)",
            prog: &[0x48e7, 0xc080],
            setup: no_setup,
            clocks: 8 + 8 * 3,
            reads: 2,
            writes: 6,
        },
        Case {
            name: "MOVEM.l (A7)+,D0-D1/A0",
            prog: &[0x4cdf, 0x0103],
            setup: |c, _| c.ssp = 0x7ff4,
            clocks: 12 + 8 * 3,
            reads: 3 + 6,
            writes: 0,
        },
        Case {
            name: "MOVEM.w (A0),D0/A1",
            prog: &[0x4c90, 0x0201],
            setup: |c, _| c.a[0] = 0x2000,
            clocks: 12 + 4 * 2,
            reads: 3 + 2,
            writes: 0,
        },
        // 表7-11
        Case {
            name: "BRA.s",
            prog: &[0x6004],
            setup: no_setup,
            clocks: 10,
            reads: 2,
            writes: 0,
        },
        Case {
            name: "BNE.s（分岐しない）",
            prog: &[0x6604],
            setup: |c, _| c.sr = 0x2704,
            clocks: 8,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "BNE.w（分岐しない）",
            prog: &[0x6600, 0x0010],
            setup: |c, _| c.sr = 0x2704,
            clocks: 12,
            reads: 2,
            writes: 0,
        },
        Case {
            name: "DBF D0（分岐する）",
            prog: &[0x51c8, 0xfffe],
            setup: |c, _| c.d[0] = 1,
            clocks: 10,
            reads: 2,
            writes: 0,
        },
        Case {
            name: "DBF D0（回数が尽きる）",
            prog: &[0x51c8, 0xfffe],
            setup: |c, _| c.d[0] = 0,
            clocks: 14,
            reads: 3,
            writes: 0,
        },
        // 表7-9：6＋2n（バイト・ワード）、8＋2n（ロング）
        Case {
            name: "LSL.w #3,D0",
            prog: &[0xe748],
            setup: no_setup,
            clocks: 6 + 2 * 3,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "ASL.l D1,D0（D1＝5）",
            prog: &[0xe3a0],
            setup: |c, _| c.d[1] = 5,
            clocks: 8 + 2 * 5,
            reads: 1,
            writes: 0,
        },
        // 表7-6 の注：38＋2n
        Case {
            name: "MULU D1,D0（D1＝0）",
            prog: &[0xc0c1],
            setup: no_setup,
            clocks: 38,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "MULU D1,D0（D1＝FFFF）",
            prog: &[0xc0c1],
            setup: |c, _| c.d[1] = 0xffff,
            clocks: 38 + 2 * 16,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "MULS D1,D0（D1＝5555）",
            prog: &[0xc1c1],
            setup: |c, _| c.d[1] = 0x5555,
            clocks: 38 + 2 * 16,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "MULS D1,D0（D1＝0）",
            prog: &[0xc1c1],
            setup: no_setup,
            clocks: 38,
            reads: 1,
            writes: 0,
        },
        // 表7-14
        Case {
            name: "RTS",
            prog: &[0x4e75],
            setup: |c, b| {
                c.ssp = 0x7ffc;
                b.w32(0x7ffc, 0x2000);
            },
            clocks: 16,
            reads: 4,
            writes: 0,
        },
        Case {
            name: "RTE",
            prog: &[0x4e73],
            setup: |c, b| {
                c.ssp = 0x7ffa;
                b.w16(0x7ffa, 0x2700);
                b.w32(0x7ffc, 0x2000);
            },
            clocks: 20,
            reads: 5,
            writes: 0,
        },
        Case {
            name: "RESET",
            prog: &[0x4e70],
            setup: no_setup,
            clocks: 132,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "STOP #2700",
            prog: &[0x4e72, 0x2700],
            setup: no_setup,
            clocks: 4,
            reads: 0,
            writes: 0,
        },
        Case {
            name: "TRAPV（V＝0）",
            prog: &[0x4e76],
            setup: no_setup,
            clocks: 4,
            reads: 1,
            writes: 0,
        },
        Case {
            name: "CHK D1,D0（範囲内）",
            prog: &[0x4181],
            setup: |c, _| {
                c.d[0] = 5;
                c.d[1] = 10;
            },
            clocks: 10,
            reads: 1,
            writes: 0,
        },
    ];
    for c in &cases {
        let (mut cpu, mut b) = boot(c.prog);
        (c.setup)(&mut cpu, &mut b);
        let clocks = step(&mut cpu, &mut b);
        assert_eq!(
            (clocks, b.reads(), b.writes()),
            (c.clocks, c.reads, c.writes),
            "{}",
            c.name
        );
    }
}

/// DIVU・DIVS の結果と時間。M68 表7-6 は最大（DIVU 140、DIVS 158）だけを書く。データで変わる時間は
/// 台帳 §2.9 の注の解析による。上位の語の比較で分かる桁あふれは早く終わる。
#[test]
fn divide_results_and_times() {
    // 100 ÷ 7 ＝ 14 余り 2
    let (mut cpu, mut b) = boot(&[0x80c1]);
    cpu.d[0] = 100;
    cpu.d[1] = 7;
    let t = step(&mut cpu, &mut b);
    assert_eq!(cpu.d[0], 0x0002_000e);
    assert!((76..=140).contains(&t), "DIVU {t}");
    // −100 ÷ 7 ＝ −14 余り −2（余りは被除数の符号）
    let (mut cpu, mut b) = boot(&[0x81c1]);
    cpu.d[0] = (-100i32) as u32;
    cpu.d[1] = 7;
    let t = step(&mut cpu, &mut b);
    assert_eq!(cpu.d[0], 0xfffe_fff2);
    assert_eq!(cpu.sr & 0x0f, 0x08);
    assert!((120..=158).contains(&t), "DIVS {t}");
    // 桁あふれ：V を立て、Dn は変えない。10 クロック。
    let (mut cpu, mut b) = boot(&[0x80c1]);
    cpu.d[0] = 0x0001_0000;
    cpu.d[1] = 1;
    assert_eq!(step(&mut cpu, &mut b), 10);
    assert_eq!(cpu.d[0], 0x0001_0000);
    assert_eq!(cpu.sr & 0x02, 0x02);
}

/// どのバスサイクルにも待ちが入ると、待ちの合計だけ長くなり、次のサイクルの S0 も遅れる。
#[test]
fn bus_waits_add_to_the_clocks() {
    let (mut cpu, mut b) = boot(&[0x223c, 0x1234, 0x5678, NOP]);
    b.waits = 3;
    let start = cpu.clock;
    assert_eq!(step(&mut cpu, &mut b), 12 + 3 * 3);
    let ats: Vec<u64> = b
        .log
        .iter()
        .map(|e| match e {
            Ev::Read { at, .. } => *at - start,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(ats, vec![0, 7, 14]);
    assert_eq!(cpu.d[1], 0x1234_5678);
    assert_eq!(step(&mut cpu, &mut b), 4 + 3);
}

// ------------------------------------------------------------------ MOVEM

/// -(An) は最後のレジスタから積む（ロングは下位の語を先に書く）。(An)+ は D0 から取り出す。
#[test]
fn movem_saves_and_restores_registers() {
    let (mut cpu, mut b) = boot(&[0x48e7, 0xc080, 0x4cdf, 0x020c]);
    cpu.d[0] = 0x1111_2222;
    cpu.d[1] = 0x3333_4444;
    cpu.a[0] = 0x5555_6666;
    step(&mut cpu, &mut b);
    assert_eq!(cpu.ssp, SSP - 12);
    assert_eq!(b.r32(SSP - 12), 0x1111_2222);
    assert_eq!(b.r32(SSP - 8), 0x3333_4444);
    assert_eq!(b.r32(SSP - 4), 0x5555_6666);
    let written: Vec<(u32, u16)> = b
        .log
        .iter()
        .filter_map(|e| match e {
            Ev::Write { addr, value, .. } => Some((*addr, *value)),
            _ => None,
        })
        .collect();
    assert_eq!(
        written,
        vec![
            (SSP - 2, 0x6666),
            (SSP - 4, 0x5555),
            (SSP - 6, 0x4444),
            (SSP - 8, 0x3333),
            (SSP - 10, 0x2222),
            (SSP - 12, 0x1111),
        ]
    );
    // D2・D3・A1 に取り出す。
    assert_eq!(step(&mut cpu, &mut b), 12 + 8 * 3);
    assert_eq!(cpu.ssp, SSP);
    assert_eq!(
        (cpu.d[2], cpu.d[3], cpu.a[1]),
        (0x1111_2222, 0x3333_4444, 0x5555_6666)
    );
}

/// MOVEM.w でレジスタへは 32 bit に符号拡張する。
#[test]
fn movem_word_sign_extends() {
    let (mut cpu, mut b) = boot(&[0x4c90, 0x0201]);
    cpu.a[0] = 0x2000;
    cpu.d[0] = 0x1234_5678;
    b.w16(0x2000, 0x8001);
    b.w16(0x2002, 0x7000);
    step(&mut cpu, &mut b);
    assert_eq!(cpu.d[0], 0xffff_8001);
    assert_eq!(cpu.a[1], 0x0000_7000);
}

// ------------------------------------------------------------------ 例外（グループ 1・2）

/// TRAP #n：次の命令の番地と SR を積み、ベクタ 32＋n へ。34 クロック（§2.8 の [V]：M68 表7-16 は
/// 38(4/4) と書くが、フレームは 3 語なので書込みは 3 回。§03）。
#[test]
fn trap_stacks_next_pc_and_sr() {
    let (mut cpu, mut b) = boot(&[0x4e43]);
    cpu.sr = 0x2715;
    assert_eq!(step(&mut cpu, &mut b), 34);
    assert_eq!(cpu.ssp, SSP - 6);
    assert_eq!(frame(&cpu, &b), (0x2715, START + 2));
    assert_eq!(cpu.pc, handler(35));
    assert_eq!(cpu.prefetch, [NOP, NOP]);
    assert_eq!((b.reads(), b.writes()), (4, 3));
}

/// 不正命令・line A・line F：命令の番地を積む。34(4/3)。
#[test]
fn illegal_and_line_a_f_stack_their_own_pc() {
    for (op, vec) in [(0x4afc, 4), (0xa000, 10), (0xf000, 11)] {
        let (mut cpu, mut b) = boot(&[op]);
        assert_eq!(step(&mut cpu, &mut b), 34, "{op:04x}");
        assert_eq!(frame(&cpu, &b), (0x2700, START), "{op:04x}");
        assert_eq!(cpu.pc, handler(vec), "{op:04x}");
    }
}

/// 特権違反：ユーザーモードの MOVE to SR。フレームはスーパーバイザのデータ空間に積む。
#[test]
fn privilege_violation_from_user_mode() {
    let (mut cpu, mut b) = boot(&[0x46c0]);
    cpu.sr = 0x0000;
    cpu.usp = 0x6000;
    assert_eq!(step(&mut cpu, &mut b), 34);
    assert_eq!(cpu.sr, 0x2000);
    assert_eq!(frame(&cpu, &b), (0x0000, START));
    assert_eq!(cpu.pc, handler(8));
    assert!(b.log.iter().any(|e| matches!(
        e,
        Ev::Write {
            fc: fc::SUPERVISOR_DATA,
            ..
        }
    )));
    assert_eq!(cpu.usp, 0x6000);
}

/// CHK：Dn が負なら 40、上限より大きければ 38（比較の順。§03）。積む PC は次の命令。
/// 例外のときの N は Dn の符号。
#[test]
fn chk_traps_below_zero_and_above_bound() {
    let (mut cpu, mut b) = boot(&[0x4181]);
    cpu.d[0] = 0xffff;
    cpu.d[1] = 10;
    assert_eq!(step(&mut cpu, &mut b), 40);
    let (sr, pc) = frame(&cpu, &b);
    assert_eq!((sr & 0x08, pc), (0x08, START + 2));
    assert_eq!(cpu.pc, handler(6));

    let (mut cpu, mut b) = boot(&[0x4181]);
    cpu.d[0] = 20;
    cpu.d[1] = 10;
    assert_eq!(step(&mut cpu, &mut b), 38);
    let (sr, pc) = frame(&cpu, &b);
    assert_eq!((sr & 0x08, pc), (0, START + 2));
}

/// 0 除算：38 クロック（M68 表7-16 は 42(5/4)。§03）。N・Z・V・C を 0 にして積む。
#[test]
fn divide_by_zero_traps() {
    let (mut cpu, mut b) = boot(&[0x80c1]);
    cpu.sr = 0x271f;
    cpu.d[0] = 1234;
    assert_eq!(step(&mut cpu, &mut b), 38);
    assert_eq!(frame(&cpu, &b), (0x2710, START + 2));
    assert_eq!(cpu.pc, handler(5));
    assert_eq!(cpu.d[0], 1234);
}

/// TRAPV：V が立っていれば 34。
#[test]
fn trapv_traps_on_overflow() {
    let (mut cpu, mut b) = boot(&[0x4e76]);
    cpu.sr = 0x2702;
    assert_eq!(step(&mut cpu, &mut b), 34);
    assert_eq!(frame(&cpu, &b), (0x2702, START + 2));
    assert_eq!(cpu.pc, handler(7));
}

/// トレース：命令の後に 34 クロックの例外処理（M68 表7-16）。T を外して積む。
#[test]
fn trace_runs_after_each_instruction() {
    let (mut cpu, mut b) = boot(&[NOP, NOP]);
    cpu.sr = 0xa700;
    assert_eq!(step(&mut cpu, &mut b), 4 + 34);
    assert_eq!(frame(&cpu, &b), (0xa700, START + 2));
    assert_eq!(cpu.sr, 0x2700);
    assert_eq!(cpu.pc, handler(9));
}

// ------------------------------------------------------------------ 割込み

/// オートベクタの割込み：44(5/3)（IACK を 4 クロックとして。台帳 §2.4）。バスサイクルの順は
/// PC の下位を積む、IACK、SR、PC の上位、ベクタ、処理ルーチンの 2 語。
#[test]
fn autovector_interrupt_takes_44_clocks() {
    let (mut cpu, mut b) = boot(&[NOP]);
    cpu.sr = 0x2000;
    b.ipl = 3;
    let t0 = cpu.clock;
    assert_eq!(step(&mut cpu, &mut b), 44);
    assert_eq!(cpu.sr, 0x2300);
    assert_eq!(frame(&cpu, &b), (0x2000, START));
    assert_eq!(cpu.pc, handler(27));
    let d = fc::SUPERVISOR_DATA;
    let p = fc::SUPERVISOR_PROGRAM;
    let rel: Vec<Ev> = b
        .log
        .iter()
        .map(|e| match *e {
            Ev::Read { at, addr, fc } => Ev::Read {
                at: at - t0,
                addr,
                fc,
            },
            Ev::Write {
                at,
                addr,
                fc,
                value,
            } => Ev::Write {
                at: at - t0,
                addr,
                fc,
                value,
            },
            Ev::Iack { at, level } => Ev::Iack { at: at - t0, level },
            Ev::Reset { at } => Ev::Reset { at: at - t0 },
        })
        .collect();
    assert_eq!(
        rel,
        vec![
            Ev::Write {
                at: 6,
                addr: SSP - 2,
                fc: d,
                value: START as u16
            },
            Ev::Iack { at: 10, level: 3 },
            Ev::Write {
                at: 18,
                addr: SSP - 6,
                fc: d,
                value: 0x2000
            },
            Ev::Write {
                at: 22,
                addr: SSP - 4,
                fc: d,
                value: 0
            },
            Ev::Read {
                at: 26,
                addr: 27 * 4,
                fc: d
            },
            Ev::Read {
                at: 30,
                addr: 27 * 4 + 2,
                fc: d
            },
            Ev::Read {
                at: 34,
                addr: handler(27),
                fc: p
            },
            Ev::Read {
                at: 40,
                addr: handler(27) + 2,
                fc: p
            },
        ]
    );
}

/// 装置がベクタ番号を返す割込みと、BERR で答えるスプリアス割込み（ベクタ 24）。
#[test]
fn vectored_and_spurious_interrupts() {
    let (mut cpu, mut b) = boot(&[NOP]);
    cpu.sr = 0x2000;
    b.ipl = 5;
    b.iack = IackReply::Vector {
        vector: 0x40,
        waits: 2,
    };
    assert_eq!(step(&mut cpu, &mut b), 44 + 2);
    assert_eq!(cpu.pc, handler(0x40));

    let (mut cpu, mut b) = boot(&[NOP]);
    cpu.sr = 0x2000;
    b.ipl = 2;
    b.iack = IackReply::Spurious { clocks: 12 };
    assert_eq!(step(&mut cpu, &mut b), 40 + 12);
    assert_eq!(cpu.pc, handler(24));
}

/// マスク以下のレベルは受け付けない。
#[test]
fn masked_interrupt_waits() {
    let (mut cpu, mut b) = boot(&[NOP, NOP]);
    cpu.sr = 0x2300;
    b.ipl = 3;
    assert_eq!(step(&mut cpu, &mut b), 4);
    assert_eq!(cpu.pc, START + 2);
    b.ipl = 4;
    assert_eq!(step(&mut cpu, &mut b), 44);
    assert_eq!(cpu.pc, handler(28));
}

/// レベル 7 はマスクできず、7 に変わったときに受け付ける（台帳 §2.4）。出したままなら 1 回だけ。
#[test]
fn level_7_is_taken_on_the_edge() {
    let (mut cpu, mut b) = boot(&[NOP]);
    assert_eq!(cpu.sr & 0x0700, 0x0700);
    b.ipl = 7;
    assert_eq!(step(&mut cpu, &mut b), 44);
    assert_eq!(cpu.pc, handler(31));
    // 出したまま：処理ルーチンの NOP を実行する。
    assert_eq!(step(&mut cpu, &mut b), 4);
    assert_eq!(step(&mut cpu, &mut b), 4);
    // いったん外してから出し直す。
    b.ipl = 0;
    assert_eq!(step(&mut cpu, &mut b), 4);
    b.ipl = 7;
    assert_eq!(step(&mut cpu, &mut b), 44);
    assert_eq!(frame(&cpu, &b).1, handler(31) + 6);
}

/// STOP は SR を書き換えて止まり、割込みで動き出す。積む PC は STOP の次の命令。
#[test]
fn stop_waits_for_an_interrupt() {
    let (mut cpu, mut b) = boot(&[0x4e72, 0x2000, NOP]);
    assert_eq!(step(&mut cpu, &mut b), 4);
    assert_eq!(cpu.state, RunState::Stopped);
    assert_eq!(cpu.sr, 0x2000);
    // 要求がなければ 4 クロックずつ進むだけ。
    assert_eq!(step(&mut cpu, &mut b), 4);
    assert!(b.log.is_empty());
    b.ipl = 2;
    assert_eq!(step(&mut cpu, &mut b), 44);
    assert_eq!(cpu.state, RunState::Running);
    assert_eq!(frame(&cpu, &b), (0x2000, START + 4));
    assert_eq!(cpu.pc, handler(26));
}

/// ユーザーモードの STOP は特権違反で、止まらない。
#[test]
fn stop_in_user_mode_is_a_privilege_violation() {
    let (mut cpu, mut b) = boot(&[0x4e72, 0x2000]);
    cpu.sr = 0x0000;
    assert_eq!(step(&mut cpu, &mut b), 34);
    assert_eq!(cpu.state, RunState::Running);
    assert_eq!(cpu.pc, handler(8));
}

/// RESET 命令は周辺のリセットを 1 回出す（4 クロック目から 124 クロック）。CPU は止まらない。
#[test]
fn reset_instruction_resets_the_peripherals() {
    let (mut cpu, mut b) = boot(&[0x4e70, NOP]);
    let t0 = cpu.clock;
    assert_eq!(step(&mut cpu, &mut b), 132);
    assert_eq!(b.log[0], Ev::Reset { at: t0 + 4 });
    assert_eq!(cpu.pc, START + 2);
}

// ------------------------------------------------------------------ グループ 0

/// バスエラー：7 語のフレーム（状態語、アクセスの番地、命令、SR、PC）で、50(4/7)（M68 表7-16）。
/// 状態語は R/W（bit 4）、I/N（bit 3）、FC。データの読出しなので R/W＝1、I/N＝0。
#[test]
fn bus_error_frame() {
    let (mut cpu, mut b) = boot(&[0x3010]);
    cpu.a[0] = UNMAPPED;
    assert_eq!(step(&mut cpu, &mut b), BERR_CLOCKS + 50);
    let sp = cpu.ssp;
    assert_eq!(sp, SSP - 14);
    assert_eq!(b.r16(sp) & 0x1f, 0x10 | u16::from(fc::SUPERVISOR_DATA));
    assert_eq!(b.r32(sp + 2), UNMAPPED);
    assert_eq!(b.r16(sp + 6), 0x3010);
    assert_eq!(b.r16(sp + 8), 0x2700);
    assert_eq!(b.r32(sp + 10), START);
    assert_eq!(cpu.pc, handler(2));
    assert_eq!((b.reads(), b.writes()), (1 + 4, 7));
}

/// アドレスエラー：奇数番地へのワードのアクセスはバスに出ず、すぐにグループ 0 の処理に入る。
#[test]
fn address_error_on_data_read() {
    let (mut cpu, mut b) = boot(&[0x3010]);
    cpu.a[0] = 0x2001;
    assert_eq!(step(&mut cpu, &mut b), 50);
    let sp = cpu.ssp;
    assert_eq!(b.r16(sp) & 0x1f, 0x10 | u16::from(fc::SUPERVISOR_DATA));
    assert_eq!(b.r32(sp + 2), 0x2001);
    assert_eq!(cpu.pc, handler(3));
    assert!(
        !b.log
            .iter()
            .any(|e| matches!(e, Ev::Read { addr: 0x2000, .. }))
    );
}

/// 奇数番地への分岐：プログラムの読出しのアドレスエラー。I/N は SST に合わせて 1（M68 の説明と逆。
/// §03）。積む PC は分岐先 − 4（§03）。
#[test]
fn address_error_on_branch_target() {
    let (mut cpu, mut b) = boot(&[0x4ed0]);
    cpu.a[0] = 0x2001;
    assert_eq!(step(&mut cpu, &mut b), 50);
    let sp = cpu.ssp;
    assert_eq!(
        b.r16(sp) & 0x1f,
        0x10 | 0x08 | u16::from(fc::SUPERVISOR_PROGRAM)
    );
    assert_eq!(b.r32(sp + 2), 0x2001);
    assert_eq!(b.r32(sp + 10), 0x2001 - 4);
    assert_eq!(cpu.pc, handler(3));
}

/// 二重バスフォルト：フレームを積む途中のバスエラーで止まる（HALT）。リセットでだけ戻る。
#[test]
fn double_bus_fault_halts() {
    let (mut cpu, mut b) = boot(&[0x3010]);
    cpu.a[0] = UNMAPPED;
    cpu.ssp = UNMAPPED + 0x100;
    let t = step(&mut cpu, &mut b);
    assert_eq!(t, BERR_CLOCKS + 4 + BERR_CLOCKS);
    assert_eq!(cpu.state, RunState::Halted);
    assert_eq!(step(&mut cpu, &mut b), 4);
    assert!(b.log.is_empty());
    cpu.reset(&mut b);
    assert_eq!(cpu.state, RunState::Running);
    assert_eq!((cpu.ssp, cpu.pc), (SSP, START));
}

// ------------------------------------------------------------------ 速さ

/// 実時間に対する速さの目安（`cargo test --release -p fc85-chips --test hd68000 -- --ignored
/// --nocapture`）。転送・演算・分岐の混ざったループを 8 MHz の 10 秒分回す。
#[test]
#[ignore]
fn speed_against_real_time() {
    // loop: MOVE.l (A0)+,(A1)+ / ADD.l D0,D1 / MULU D2,D3 / LSL.l #4,D4 / DBF D5,loop
    //       MOVEA.l #2000,A0 / MOVEA.l #3000,A1 / MOVE.w #FF,D5 / BRA loop
    let prog = [
        0x22d8, 0xd280, 0xc6c2, 0xe98c, 0x51cd, 0xfff6, 0x207c, 0x0000, 0x2000, 0x227c, 0x0000,
        0x3000, 0x3a3c, 0x00ff, 0x60e2,
    ];
    let (mut cpu, mut b) = boot(&prog);
    cpu.a[0] = 0x2000;
    cpu.a[1] = 0x3000;
    cpu.d[5] = 0xff;
    let clocks = 80_000_000u64;
    let start = std::time::Instant::now();
    let end = cpu.clock + clocks;
    let mut n = 0u64;
    while cpu.clock < end {
        cpu.step(&mut b);
        n += 1;
        if b.log.len() > 4096 {
            b.log.clear();
        }
    }
    let secs = start.elapsed().as_secs_f64();
    println!(
        "{n} 命令、{clocks} クロックを {secs:.3} 秒（実時間の {:.1} 倍）",
        10.0 / secs
    );
    assert_eq!(cpu.state, RunState::Running);
    assert!(cpu.pc >= START && cpu.pc < START + 0x20);
}
