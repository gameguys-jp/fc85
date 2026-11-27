//! 番地とレジスタの表（`regmap`、`docs/spec/registers.toml` から生成。DR-100）と、本体のバスの解読
//! （`console::System`）の突き合わせ。68000 のコアを使わず、試験がバスサイクルを直接出す。
//!
//! 表のレジスタは、表のアクセスの向きと幅でバスエラーにならない。読出しの向きのないレジスタは読んでも、
//! 書込みの向きのないレジスタは書いても、バスエラーにならない（DR-052、DR-053）。読むと状態の変わる
//! レジスタ（`read_effect`）は書くだけ。16・32 bit のレジスタのバイトのアクセスと、8 bit の装置の奇数番地は
//! バスエラー。窓の中で表にない番地、窓の外の番地、条件（`when`）を満たさない窓はバスエラー。

use fc85_board::console::{BERR_CYCLE_CLOCKS, System};
use fc85_board::regmap::{
    self, Access, REGIONS, REGISTERS, Register, Unlisted, WINDOWS, When, Width, Window,
};
use fc85_chips::hd68000::{Bus, BusReply, Size, fc};

/// 試験の CPU：S0 の時刻を進めながらバスサイクルを出す。
struct Cpu {
    sys: System,
    clock: u64,
}

impl Cpu {
    /// 条件 `when` を満たした本体。
    fn new(when: When) -> Self {
        let mut c = Self {
            sys: System::new(),
            clock: 0,
        };
        match when {
            When::Always | When::NoDoc => {}
            When::Doc => c.attach_doc(),
            // 電源投入で RESET＝1。PRGBUS は数サイクルの後に上がる
            When::DspPrgbus => c.idle(100),
            When::Cart => c.sys.cart = Some(vec![0; 16]),
        }
        c
    }

    fn attach_doc(&mut self) {
        self.sys.ports.doc = true;
        let now = self.sys.now();
        self.sys.ports.doc_board.set_phase(now, 0);
    }

    fn done(&mut self, r: BusReply) -> BusReply {
        self.clock += match r {
            BusReply::Ok { waits, .. } => 4 + u64::from(waits),
            BusReply::BusError { clocks } => u64::from(clocks),
        };
        r
    }

    fn rd(&mut self, addr: u32, size: Size) -> BusReply {
        let r = self.sys.read(self.clock, addr, fc::SUPERVISOR_DATA, size);
        self.done(r)
    }

    fn wr(&mut self, addr: u32, size: Size, v: u16) -> BusReply {
        let r = self
            .sys
            .write(self.clock, addr, fc::SUPERVISOR_DATA, size, v);
        self.done(r)
    }

    fn idle(&mut self, n: u64) {
        self.clock += n;
        self.sys.sync(self.clock);
    }
}

const BERR: BusReply = BusReply::BusError {
    clocks: BERR_CYCLE_CLOCKS,
};

fn is_ok(r: BusReply) -> bool {
    matches!(r, BusReply::Ok { .. })
}

/// レジスタの 1 要素の語（32 bit は上位語と下位語）。
fn words(r: &Register, a: u32) -> Vec<u32> {
    if r.bits == 32 {
        vec![a, a + 2]
    } else {
        vec![a]
    }
}

fn size_of(r: &Register) -> Size {
    if r.bits == 8 { Size::Byte } else { Size::Word }
}

#[test]
fn every_register_takes_its_access_without_a_bus_error() {
    for r in REGISTERS {
        let mut c = Cpu::new(r.when);
        for a in r.addrs() {
            for w in words(r, a) {
                // 読出し（読むと状態の変わるレジスタは読まない）
                if r.read_effect.is_none() {
                    let got = c.rd(w, size_of(r));
                    assert!(is_ok(got), "{} の読出し {w:#08x}：{got:?}", r.name);
                }
                // 書込み（読出し専用のレジスタへの書込みは捨てる）
                let got = c.wr(w, size_of(r), 0);
                assert!(is_ok(got), "{} の書込み {w:#08x}：{got:?}", r.name);
            }
        }
    }
}

#[test]
fn word_registers_reject_byte_access_and_byte_devices_reject_odd_addresses() {
    for r in REGISTERS {
        let mut c = Cpu::new(r.when);
        let a = r.addr;
        if r.bits == 8 {
            // 8 bit の装置は上位バイトレーン（偶数番地）だけ
            assert_eq!(
                c.rd(a + 1, Size::Byte),
                BERR,
                "{} の奇数番地の読出し",
                r.name
            );
            assert_eq!(
                c.wr(a + 1, Size::Byte, 0),
                BERR,
                "{} の奇数番地の書込み",
                r.name
            );
        } else {
            for b in [a, a + 1] {
                if r.read_effect.is_none() {
                    assert_eq!(
                        c.rd(b, Size::Byte),
                        BERR,
                        "{} のバイトの読出し {b:#08x}",
                        r.name
                    );
                }
                assert_eq!(
                    c.wr(b, Size::Byte, 0),
                    BERR,
                    "{} のバイトの書込み {b:#08x}",
                    r.name
                );
            }
        }
    }
}

#[test]
fn memories_take_their_access() {
    for m in REGIONS {
        let mut c = Cpu::new(m.when);
        for a in [m.first, m.last - 1] {
            assert!(
                is_ok(c.rd(a, Size::Word)),
                "{} の語の読出し {a:#08x}",
                m.name
            );
            assert!(
                is_ok(c.rd(a, Size::Byte)),
                "{} のバイトの読出し {a:#08x}",
                m.name
            );
            if m.width == Width::Any {
                assert!(
                    is_ok(c.rd(a + 1, Size::Byte)),
                    "{} のバイトの読出し {:#08x}",
                    m.name,
                    a + 1
                );
            }
            // ROM への書込みは捨てる（DR-052）
            assert!(
                is_ok(c.wr(a, Size::Word, 0)),
                "{} の書込み {a:#08x}",
                m.name
            );
        }
        if m.access == Access::Rw {
            let a = m.last - 1;
            c.wr(a, Size::Word, 0x5AA5);
            assert!(
                matches!(c.rd(a, Size::Word), BusReply::Ok { data: 0x5AA5, .. }),
                "{} の読み書き",
                m.name
            );
        }
    }
}

/// 窓の中で、どのレジスタにも当たらない番地。
fn unlisted(w: &Window) -> Vec<u32> {
    // 窓は偶数番地から奇数番地まで
    let mut taken = vec![false; ((w.last - w.first) / 2 + 1) as usize];
    for r in REGISTERS {
        for a in r.addrs() {
            for b in (a..a + r.span()).step_by(2) {
                if (w.first..=w.last).contains(&b) {
                    taken[((b - w.first) / 2) as usize] = true;
                }
            }
        }
    }
    for m in REGIONS {
        for b in (m.first.max(w.first)..=m.last.min(w.last)).step_by(2) {
            taken[((b - w.first) / 2) as usize] = true;
        }
    }
    (w.first..=w.last)
        .step_by(2)
        .filter(|a| !taken[((a - w.first) / 2) as usize])
        .collect()
}

#[test]
fn unlisted_addresses_inside_windows_are_bus_errors() {
    let mut checked = 0;
    for w in WINDOWS {
        let free = unlisted(w);
        let mut c = Cpu::new(w.when);
        match w.unlisted {
            Unlisted::BusError => {
                for &a in &free {
                    assert_eq!(c.rd(a, Size::Word), BERR, "{} の {a:#08x} の読出し", w.name);
                    assert_eq!(
                        c.wr(a, Size::Word, 0),
                        BERR,
                        "{} の {a:#08x} の書込み",
                        w.name
                    );
                    checked += 1;
                }
            }
            // 窓の中の番地はすべて IC のレジスタ（DOC のネイティブの 256 個）
            Unlisted::Chip => {
                for a in (w.first..=w.last).step_by(2) {
                    assert!(
                        is_ok(c.wr(a, Size::Byte, 0)),
                        "{} の {a:#08x} の書込み",
                        w.name
                    );
                }
            }
        }
    }
    // DSP・ADPCM・Temporal・Data Fabric・入力・映像の窓の空き
    assert!(checked > 1000, "{checked}");
}

#[test]
fn addresses_outside_the_windows_are_bus_errors() {
    let mut probes = vec![0xFF_FFFE];
    for pair in WINDOWS.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if b.first > a.last + 1 {
            let mid = (a.last + 1 + (b.first - a.last - 1) / 2) & !1;
            probes.extend([a.last + 1, mid, b.first - 2]);
        }
    }
    let last = WINDOWS.last().unwrap();
    probes.push(last.last + 1);
    let mut c = Cpu::new(When::Cart);
    c.attach_doc();
    for a in probes {
        assert!(
            !WINDOWS.iter().any(|w| (w.first..=w.last).contains(&a)),
            "{a:#08x}"
        );
        assert_eq!(c.rd(a, Size::Word), BERR, "{a:#08x} の語の読出し");
        assert_eq!(c.rd(a, Size::Byte), BERR, "{a:#08x} のバイトの読出し");
        assert_eq!(c.wr(a, Size::Word, 0), BERR, "{a:#08x} の書込み");
    }
}

#[test]
fn windows_without_their_condition_are_bus_errors() {
    // DOC の子基板がない
    let mut c = Cpu::new(When::NoDoc);
    assert_eq!(c.rd(regmap::DOC_IRQ, Size::Byte), BERR);
    assert_eq!(c.wr(regmap::DOC_FREQ_LO, Size::Byte, 0), BERR);
    // DOC の子基板があると本体 DAC はない
    let mut c = Cpu::new(When::Doc);
    for a in [regmap::DAC_CTRL, regmap::DAC_STATUS, regmap::DAC_DATA] {
        assert_eq!(c.rd(a, Size::Word), BERR, "{a:#08x}");
        assert_eq!(c.wr(a, Size::Word, 0), BERR, "{a:#08x}");
    }
    // DSP の RESET を外すと、プログラム RAM の窓はない
    let mut c = Cpu::new(When::DspPrgbus);
    assert!(is_ok(c.rd(regmap::DSP_PROGRAM, Size::Word)));
    assert!(is_ok(c.wr(regmap::DSP_CTRL, Size::Word, 0)));
    c.idle(100);
    assert_eq!(
        c.rd(regmap::DSP_STATUS, Size::Word).data_bit(0),
        Some(false)
    );
    assert_eq!(c.rd(regmap::DSP_PROGRAM, Size::Word), BERR);
    assert_eq!(c.wr(regmap::DSP_PROGRAM + 0x1FFE, Size::Word, 0), BERR);
    // カートリッジがない
    let mut c = Cpu::new(When::Always);
    assert_eq!(c.rd(regmap::CART, Size::Word), BERR);
    assert_eq!(c.rd(regmap::CART_END - 2, Size::Word), BERR);
}

trait DataBit {
    fn data_bit(self, n: u32) -> Option<bool>;
}

impl DataBit for BusReply {
    fn data_bit(self, n: u32) -> Option<bool> {
        match self {
            BusReply::Ok { data, .. } => Some(data >> n & 1 != 0),
            BusReply::BusError { .. } => None,
        }
    }
}

#[test]
fn the_table_agrees_with_the_constants_the_emulator_uses() {
    use fc85_board::video;
    assert_eq!(regmap::VIRQ_VBL_EN, video::VIRQ_VBL_EN);
    assert_eq!(regmap::VIRQ_HBL_EN, video::VIRQ_HBL_EN);
    assert_eq!(regmap::VIRQ_LINE_CMP, video::VIRQ_LINE_CMP);
    assert_eq!(regmap::VIRQ_VBL, video::VIRQ_VBL);
    assert_eq!(regmap::VIRQ_HBL, video::VIRQ_HBL);
    assert_eq!(regmap::VIRQ_HBLANK, video::VIRQ_HBLANK);
    assert_eq!(regmap::VIRQ_VBLANK, video::VIRQ_VBLANK);
    // 表の中身の形：窓は番地の順で重ならず、レジスタは窓の中
    for pair in WINDOWS.windows(2) {
        assert!(
            pair[0].last < pair[1].first,
            "{} と {}",
            pair[0].name,
            pair[1].name
        );
    }
    for r in REGISTERS {
        for a in r.addrs() {
            assert!(
                WINDOWS
                    .iter()
                    .any(|w| w.first <= a && a + r.span() - 1 <= w.last),
                "{} {a:#08x}",
                r.name
            );
        }
        assert!(r.access.reads() || r.access.writes());
    }
    assert_eq!(
        REGISTERS
            .iter()
            .find(|r| r.name == "PALETTE")
            .map(|r| r.count),
        Some(16)
    );
}
