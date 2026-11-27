//! Data Fabric の試験（仕様書 §12 段階 2）。
//! 期待値は仕様書 §06、DR-046、`docs/spec/fabric.md` の記述と、そこからの手計算で取る。

use fc85_board::fabric::{
    Access, Aop, Decision, Dir, Fabric, HostReply, HostStart, Phase, PortReply, Ports, SlotRecord,
    TIMEOUT_SLOTS, fd_value, port_attr, port_ok, transform,
};
use fc85_board::ports::FabricPorts;

// チャネルの中のオフセット（仕様書 §06）。
const SRC: u16 = 0x00;
const DST: u16 = 0x02;
const SRC_ADDR: u16 = 0x06;
const DST_ADDR: u16 = 0x0A;
const COUNT: u16 = 0x0C;
const SRC_STEP: u16 = 0x0E;
const DST_STEP: u16 = 0x10;
const FORMAT: u16 = 0x12;
const MASK: u16 = 0x14;
const XOR: u16 = 0x16;
const SHIFT: u16 = 0x18;
const TRIGGER: u16 = 0x1A;
const CTRL: u16 = 0x1C;
const STATUS: u16 = 0x1E;

const BUSY: u16 = 0x01;
const DONE: u16 = 0x02;
const STALL: u16 = 0x04;
const ERROR: u16 = 0x08;
const MISS: u16 = 0x10;
const WAIT: u16 = 0x20;

/// TRIGGER の発生源 6 は予約で、トリガは来ない（DR-046 (8)）。転送を止めておくのに使う。
const NEVER: u16 = 6;

#[derive(Clone, Copy)]
struct Cfg {
    src: u16,
    dst: u16,
    src_addr: u16,
    dst_addr: u16,
    count: u16,
    src_step: u16,
    dst_step: u16,
    format: u16,
    trigger: u16,
}

impl Default for Cfg {
    fn default() -> Self {
        Self {
            src: 0,
            dst: 6,
            src_addr: 0,
            dst_addr: 0,
            count: 1,
            src_step: 1,
            dst_step: 1,
            format: 0,
            trigger: 0,
        }
    }
}

struct Rig<P: Ports> {
    f: Fabric,
    p: P,
    trace: Vec<SlotRecord>,
}

impl Rig<FabricPorts> {
    fn new() -> Self {
        Self::with(FabricPorts::new())
    }
}

impl<P: Ports> Rig<P> {
    fn with(p: P) -> Self {
        Self {
            f: Fabric::new(),
            p,
            trace: Vec::new(),
        }
    }

    fn step(&mut self) -> SlotRecord {
        let r = self.f.step_slot(&mut self.p);
        self.trace.push(r);
        r
    }

    fn run(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    /// スロット `slot` の前まで進める（`slot` はまだ進めない）。
    fn run_to(&mut self, slot: u64) {
        while self.f.slot() < slot {
            self.step();
        }
    }

    fn wait_poll(&mut self) -> HostReply {
        for _ in 0..4 {
            self.step();
            if let Some(r) = self.f.host_poll() {
                return r;
            }
        }
        panic!("A ファイルの許可が来ない");
    }

    fn write(&mut self, off: u16, v: u16) {
        match self.f.host_write(off, v, &mut self.p) {
            HostReply::Ack => {}
            HostReply::Wait => assert_eq!(self.wait_poll(), HostReply::Ack),
            r => panic!("書込み {off:#x}: {r:?}"),
        }
    }

    fn read(&mut self, off: u16) -> u16 {
        match self.f.host_read(off, &self.p) {
            HostReply::Data(v) => v,
            HostReply::Wait => match self.wait_poll() {
                HostReply::Data(v) => v,
                r => panic!("読出し {off:#x}: {r:?}"),
            },
            r => panic!("読出し {off:#x}: {r:?}"),
        }
    }

    fn setup(&mut self, ch: u16, c: Cfg) {
        let b = ch * 0x40;
        self.write(b + SRC, c.src);
        self.write(b + DST, c.dst);
        self.write(b + SRC_ADDR, c.src_addr);
        self.write(b + DST_ADDR, c.dst_addr);
        self.write(b + COUNT, c.count);
        self.write(b + SRC_STEP, c.src_step);
        self.write(b + DST_STEP, c.dst_step);
        self.write(b + FORMAT, c.format);
        self.write(b + TRIGGER, c.trigger);
    }

    fn start(&mut self, ch: u16) {
        self.write(ch * 0x40 + CTRL, 1);
    }

    fn status(&self, ch: usize) -> u16 {
        self.f.status(ch)
    }

    /// チャネルが止まるまで進める。進めたスロットの数を返す。
    fn run_until_idle(&mut self, ch: usize, limit: usize) -> usize {
        for n in 0..limit {
            if self.f.phase(ch) == Phase::Idle {
                return n;
            }
            self.step();
        }
        panic!(
            "{limit} スロットで終わらない（STATUS {:#x}）",
            self.status(ch)
        );
    }

    /// 記録の中の、新しい要素を始めた判断（スロット、チャネル）。
    fn starts(&self) -> Vec<(u64, u8)> {
        self.trace
            .iter()
            .filter_map(|r| match r.decision {
                Decision::Start(c) => Some((r.slot, c)),
                _ => None,
            })
            .collect()
    }

    /// 記録の中の、始めたスロットのアクセス（2 スロット以上のアクセスは最初のスロットだけ）。
    fn accesses(&self) -> Vec<(u64, bool, Access)> {
        let mut out: Vec<(u64, bool, Access)> = Vec::new();
        let mut prev: Option<Access> = None;
        let mut prev_done = true;
        for r in &self.trace {
            if let Some(a) = r.access {
                let same = !prev_done && prev.is_some_and(|p| p.port == a.port && p.dir == a.dir);
                if !same {
                    out.push((r.slot, r.s, a));
                }
                prev = Some(a);
                prev_done = r.access_done;
            } else {
                prev = None;
                prev_done = true;
            }
        }
        out
    }
}

fn gaps(v: &[(u64, u8)]) -> Vec<u64> {
    v.windows(2).map(|w| w[1].0 - w[0].0).collect()
}

// ---------------------------------------------------------------- 変換とポートの表

#[test]
fn transform_follows_dr046() {
    let t = |x, f, s, m, k| transform(x, f, s, m, k);
    // FORMAT 5：SHIFT を回転の量に。SHIFT 8 でバイトの入れ替え、−4 は右へ 4。
    assert_eq!(t(0x1234, 5, 8, 0xFFFF, 0), 0x3412);
    assert_eq!(t(0x1234, 5, 0x1C, 0xFFFF, 0), 0x4123);
    // FORMAT 4：上位バイトを下位へ移してからシフトし、8 bit で切り捨てる。
    assert_eq!(t(0xAB12, 4, 0, 0xFFFF, 0), 0xAB);
    assert_eq!(t(0xAB00, 4, 2, 0xFFFF, 0), 0xAC);
    // FORMAT 3：下位バイト。FORMAT 2：8 bit の入力（上位は見ない）をゼロ拡張。
    assert_eq!(t(0xAB12, 3, 0, 0xFFFF, 0), 0x12);
    assert_eq!(t(0x00F3, 2, 4, 0xFFFF, 0), 0x0F30);
    assert_eq!(t(0xFF34, 2, 0, 0xFFFF, 0), 0x0034);
    // 論理シフト：負は右、空いたビットは 0。−16 ですべて消える。
    assert_eq!(t(0x8001, 1, 0x1C, 0xFFFF, 0), 0x0800);
    assert_eq!(t(0xFFFF, 1, 0x10, 0xFFFF, 0), 0x0000);
    assert_eq!(t(0x0081, 0, 1, 0xFFFF, 0), 0x02);
    assert_eq!(t(0x0081, 0, 0x1F, 0xFFFF, 0), 0x40);
    // AND MASK の後に XOR。
    assert_eq!(t(0x1234, 1, 0, 0x0FF0, 0xFFFF), 0xFDCF);
    // 8 bit の出力では XOR の上位は見えない。FD の上位には出る（fabric.md §04）。
    assert_eq!(t(0x005A, 0, 0, 0xFFFF, 0xFF00), 0x5A);
    assert_eq!(fd_value(0x005A, 0, 0, 0xFFFF, 0xFF00), 0xFF5A);
    // FORMAT 6・7 はマスクが 0 で、XOR だけが残る（動作中の書換えのとき）。
    assert_eq!(t(0x1234, 6, 0, 0xFFFF, 0xA55A), 0x5A);
}

#[test]
fn port_table_matches_fabric_md() {
    use Dir::*;
    // 番地を持つポートと、満杯・空の信号を持つポート（fabric.md §06 の ATR）。
    let addr: Vec<u8> = (0..16).filter(|&p| port_attr(p, false).addr).collect();
    let flag: Vec<u8> = (0..16).filter(|&p| port_attr(p, false).flag).collect();
    assert_eq!(addr, vec![0, 2, 8, 9]);
    assert_eq!(flag, vec![1, 2, 3, 4, 5, 10, 11, 12, 13, 14, 15]);
    // 向きと幅。
    assert!(
        port_ok(3, Write, 1, false) && !port_ok(3, Read, 1, false) && !port_ok(3, Write, 0, false)
    );
    assert!(port_ok(5, Write, 0, false) && !port_ok(5, Write, 2, false));
    assert!(port_ok(4, Read, 3, false) && !port_ok(4, Read, 0, false));
    assert!(port_ok(9, Read, 0, false) && !port_ok(9, Read, 1, false));
    // port 2：DOC 子基板があれば 8 bit の R/W、なければ本体 DAC の FIFO（W、16 bit）。
    assert!(port_ok(2, Write, 1, false) && !port_ok(2, Read, 0, false));
    assert!(port_ok(2, Read, 0, true) && port_ok(2, Write, 0, true) && !port_ok(2, Write, 1, true));
    // port 0 はどの幅でも。FORMAT 6・7 はどこでも使えない。
    for f in 0..6 {
        assert!(port_ok(0, Read, f, false) && port_ok(0, Write, f, false));
    }
    assert!(!port_ok(0, Read, 6, false) && !port_ok(0, Write, 7, false));
}

// ---------------------------------------------------------------- レジスタ

#[test]
fn registers_read_zero_and_bad_addresses_get_no_dtack() {
    let mut r = Rig::new();
    r.p.doc = true;
    r.p.modules = 1 << 8;
    r.write(SRC, 3);
    r.write(MASK, 0x1234);
    // 書込み専用のレジスタと 32 bit の上位の語は 0（DR-046 (3)(4)）。
    for off in [
        SRC, DST, 0x04, 0x08, SRC_STEP, MASK, XOR, SHIFT, TRIGGER, CTRL,
    ] {
        assert_eq!(r.f.host_read(off, &r.p), HostReply::Data(0), "{off:#x}");
    }
    assert_eq!(r.f.regs(0).src, 3);
    assert_eq!(r.f.regs(0).mask, 0x1234);
    // MODULES：bit 2 DOC 子基板、bit 8 映像入力。PORT_RESET・AUDIO_N は読むと 0。
    assert_eq!(r.f.host_read(0x104, &r.p), HostReply::Data(0x0104));
    assert_eq!(r.f.host_read(0x100, &r.p), HostReply::Data(0));
    // バスエラー：奇数番地、各チャネルの 0x20〜0x3E、チャネル 2・3、0x106 以降（DR-046 (10)）。
    for off in [
        0x01, 0x20, 0x3E, 0x60, 0x80, 0xC0, 0x105, 0x106, 0x200, 0xFFE,
    ] {
        assert_eq!(r.f.host_read(off, &r.p), HostReply::BusError, "{off:#x}");
        assert_eq!(
            r.f.host_write(off, 0, &mut r.p),
            HostReply::BusError,
            "{off:#x}"
        );
    }
    // AUDIO_N は 12 bit で分周器へ。
    r.write(0x102, 0xF123);
    assert_eq!(r.p.audio_n, 0x123);
}

#[test]
fn register_file_access_waits_for_the_next_slot() {
    let mut r = Rig::new();
    assert_eq!(r.f.host_write(SRC_ADDR, 0x1234, &mut r.p), HostReply::Wait);
    assert_eq!(r.f.host_poll(), None);
    let rec = r.step();
    assert_eq!(rec.aop, Aop::Host);
    assert_eq!(r.f.host_poll(), Some(HostReply::Ack));
    // 読むと作業値。START の前は電源投入時の 0。
    assert_eq!(r.read(SRC_ADDR), 0);
    assert_eq!(r.read(0x04), 0);
}

#[test]
fn start_copies_the_settings_in_three_slots() {
    let mut r = Rig::new();
    r.setup(
        0,
        Cfg {
            src_addr: 0x1000,
            dst_addr: 0x2000,
            count: 5,
            trigger: NEVER,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run(4);
    let aops: Vec<Aop> = r.trace.iter().rev().take(4).rev().map(|x| x.aop).collect();
    assert_eq!(
        aops,
        vec![
            Aop::Reload(0, 0),
            Aop::Reload(0, 1),
            Aop::Reload(0, 2),
            Aop::None
        ]
    );
    // 動いていて、トリガを待つ。作業値は設定値、COUNT は「残りの数−1」。
    assert_eq!(r.status(0), BUSY | WAIT);
    assert_eq!(r.read(SRC_ADDR), 0x1000);
    assert_eq!(r.read(DST_ADDR), 0x2000);
    assert_eq!(r.read(COUNT), 4);
}

#[test]
fn count_zero_is_done_without_a_transfer() {
    let mut r = Rig::new();
    r.setup(
        0,
        Cfg {
            count: 0,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 10);
    assert_eq!(r.status(0), DONE);
    assert_eq!(r.read(COUNT), 0xFFFF);
    assert!(r.trace.iter().all(|x| x.access.is_none()));
}

#[test]
fn start_checks_the_ports_and_the_format() {
    let bad = [
        // 書込み専用の port 3 から読む。
        Cfg {
            src: 3,
            format: 1,
            ..Cfg::default()
        },
        // 16 bit の出力を 8 bit の port 6 へ。
        Cfg {
            format: 1,
            ..Cfg::default()
        },
        // FORMAT 6。
        Cfg {
            format: 6,
            ..Cfg::default()
        },
        // 映像入力基板のない port 8。
        Cfg {
            src: 8,
            ..Cfg::default()
        },
        // DOC 子基板がないときの port 2 から読む。
        Cfg {
            src: 2,
            ..Cfg::default()
        },
    ];
    for (i, c) in bad.into_iter().enumerate() {
        let mut r = Rig::new();
        r.setup(0, Cfg { trigger: 0x20, ..c });
        r.start(0);
        r.run(3);
        assert_eq!(r.status(0), ERROR, "{i}");
        assert!(r.f.irq(), "{i}");
        assert!(r.trace.iter().all(|x| x.access.is_none()), "{i}");
    }
    let mut r = Rig::new();
    r.setup(0, Cfg::default());
    r.start(0);
    r.run_until_idle(0, 20);
    assert_eq!(r.status(0), DONE);
}

// ---------------------------------------------------------------- 転送の時間

#[test]
fn staging_to_temporal_moves_one_byte_every_two_slots() {
    let mut r = Rig::new();
    for i in 0..8 {
        r.p.staging[0x1000 + i] = 0x11 * (i as u8 + 1);
    }
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 6,
            src_addr: 0x1000,
            count: 8,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 100);
    assert_eq!(
        &r.p.temporal.mem[..8],
        &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
    );
    assert_eq!(r.p.temporal.wptr, 8);
    // 2 スロットに 1 要素（4 MB/s）。port 0 は S のスロットだけで読む。
    assert_eq!(gaps(&r.starts()), vec![2; 7]);
    for (_, s, a) in r.accesses() {
        if a.port == 0 {
            assert!(s);
        }
    }
    assert_eq!(r.status(0), DONE);
    assert_eq!(r.read(SRC_ADDR), 0x1008);
    assert_eq!(r.read(COUNT), 0xFFFF);
}

#[test]
fn staging_to_staging_takes_four_slots_per_element() {
    let mut r = Rig::new();
    for i in 0..8 {
        r.p.staging[0x2000 + i] = i as u8 + 1;
    }
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 0,
            src_addr: 0x2000,
            dst_addr: 0x3000,
            count: 4,
            src_step: 2,
            dst_step: 2,
            format: 1,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 100);
    assert_eq!(&r.p.staging[0x3000..0x3008], &[1, 2, 3, 4, 5, 6, 7, 8]);
    // S のスロットを 2 回使うので 4 スロットに 1 要素（仕様書 §06）。
    assert_eq!(gaps(&r.starts()), vec![4; 3]);
    assert!(r.accesses().iter().all(|(_, s, _)| *s));
}

#[test]
fn count_update_goes_where_the_address_file_is_free() {
    // (読出し元, 書込み先, FORMAT, GO の操作, W の判断の操作, W の後の COUNT)：fabric.md §05。
    let cases = [
        (7, 6, 0, Aop::Count(0), Aop::None, false),
        (0, 6, 0, Aop::SrcStep(0), Aop::Count(0), false),
        (0, 0, 0, Aop::SrcStep(0), Aop::DstStep(0), true),
        (7, 0, 0, Aop::Count(0), Aop::DstStep(0), false),
    ];
    for (src, dst, format, go, w, after) in cases {
        let mut r = Rig::new();
        r.setup(
            0,
            Cfg {
                src,
                dst,
                format,
                count: 1,
                dst_addr: 0x4000,
                ..Cfg::default()
            },
        );
        r.start(0);
        r.run_until_idle(0, 40);
        let find = |d: Decision| r.trace.iter().find(|x| x.decision == d).copied();
        assert_eq!(find(Decision::Start(0)).unwrap().aop, go, "{src}→{dst}");
        let wr = find(Decision::Write(0)).unwrap();
        assert_eq!(wr.aop, w, "{src}→{dst}");
        match find(Decision::CountAfterWrite(0)) {
            Some(c) => {
                assert!(after, "{src}→{dst}");
                assert_eq!(c.aop, Aop::Count(0));
                // W のスロットで COUNT を更新する。
                assert_eq!(c.slot, wr.slot + 1);
                assert_eq!(c.access.unwrap().dir, Dir::Write);
            }
            None => assert!(!after, "{src}→{dst}"),
        }
        assert_eq!(r.status(0), DONE);
    }
}

#[test]
fn fifo_reads_take_three_slots_and_fifo_writes_four() {
    // DSP の出力の FIFO（port 4）から Temporal RAM へ：上位バイト（FORMAT 4）。
    let mut r = Rig::new();
    for i in 0..6 {
        r.p.dsp_out.shift_in(0x0100 * (i + 1) + 0x55);
    }
    r.setup(
        0,
        Cfg {
            src: 4,
            dst: 6,
            count: 6,
            format: 4,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 100);
    assert_eq!(&r.p.temporal.mem[..6], &[1, 2, 3, 4, 5, 6]);
    assert_eq!(gaps(&r.starts()), vec![3; 5]);

    // Temporal RAM から DSP の入力の FIFO（port 3）へ：ゼロ拡張（FORMAT 2）。
    let mut r = Rig::new();
    r.p.temporal.mem[..5].copy_from_slice(&[0x81, 0x82, 0x83, 0x84, 0x85]);
    r.setup(
        0,
        Cfg {
            src: 7,
            dst: 3,
            count: 5,
            format: 2,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 100);
    let got: Vec<u16> = (0..5).map(|_| r.p.dsp_in.shift_out()).collect();
    assert_eq!(got, vec![0x81, 0x82, 0x83, 0x84, 0x85]);
    assert_eq!(gaps(&r.starts()), vec![4; 4]);
}

#[test]
fn char_rom_reads_take_two_slots_and_ignore_bit_15() {
    let mut r = Rig::new();
    r.p.char_rom[0x10..0x13].copy_from_slice(&[0xA1, 0xB2, 0xC3]);
    r.setup(
        0,
        Cfg {
            src: 9,
            dst: 6,
            src_addr: 0x8010,
            count: 3,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 60);
    assert_eq!(&r.p.temporal.mem[..3], &[0xA1, 0xB2, 0xC3]);
    // 読出しは 1 回 2 スロットで、要素は 3 スロットに 1 つ。番地はそのまま渡り、ROM が bit 15 を無視する。
    let reads: Vec<&SlotRecord> = r
        .trace
        .iter()
        .filter(|x| x.access.is_some_and(|a| a.port == 9))
        .collect();
    assert_eq!(reads.len(), 6);
    for pair in reads.chunks(2) {
        assert_eq!(pair[1].slot, pair[0].slot + 1);
        assert!(!pair[0].access_done && pair[1].access_done);
    }
    let addrs: Vec<u16> = reads
        .iter()
        .step_by(2)
        .map(|x| x.access.unwrap().addr)
        .collect();
    assert_eq!(addrs, vec![0x8010, 0x8011, 0x8012]);
    assert_eq!(gaps(&r.starts()), vec![3; 2]);
}

#[test]
fn candidate_alternates_while_both_channels_run() {
    // 読出し元が port 0 でなければ、再設定（3 スロットずつ、ch0 の後に ch1）の後は 1 スロットごとに
    // 入れ替わる。port 0 から読むチャネルは S の頭で入れ替えない（DR-048）ので、2 チャネルとも port 0
    // から読むと、S と X の 2 スロットずつ候補になる。
    for src in [9, 0] {
        let mut r = Rig::new();
        for ch in 0..2 {
            r.setup(
                ch,
                Cfg {
                    src,
                    trigger: NEVER,
                    ..Cfg::default()
                },
            );
        }
        r.start(0);
        r.start(1);
        r.run(20);
        let tail: Vec<&SlotRecord> = r.trace.iter().rev().take(10).rev().collect();
        for w in tail.windows(2) {
            let same = w[0].candidate == w[1].candidate;
            assert_eq!(
                same,
                src == 0 && w[0].s,
                "src {src}：{:?}",
                tail.iter().map(|x| (x.s, x.candidate)).collect::<Vec<_>>()
            );
        }
    }

    // 2 チャネルが交互に要素を始める（Temporal RAM へ、Temporal RAM と文字 ROM から）。
    let mut r = Rig::new();
    r.p.temporal.mem[0x8000..0x8003].copy_from_slice(&[1, 2, 3]);
    r.p.temporal.rptr = 0x8000;
    r.p.char_rom[..3].copy_from_slice(&[0xF1, 0xF2, 0xF3]);
    r.setup(
        0,
        Cfg {
            src: 7,
            dst: 6,
            count: 3,
            ..Cfg::default()
        },
    );
    r.setup(
        1,
        Cfg {
            src: 9,
            dst: 6,
            count: 3,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.start(1);
    r.run(80);
    let chans: Vec<u8> = r.starts().iter().map(|s| s.1).collect();
    assert_eq!(chans.len(), 6);
    assert!(chans.windows(2).all(|w| w[0] != w[1]), "{chans:?}");
    let mut got = r.p.temporal.mem[..6].to_vec();
    got.sort();
    assert_eq!(got, vec![1, 2, 3, 0xF1, 0xF2, 0xF3]);
    assert_eq!(r.status(0) | r.status(1), DONE);
}

// ---------------------------------------------------------------- 待ちとトリガ

#[test]
fn empty_source_fifo_stalls_and_resumes() {
    let mut r = Rig::new();
    r.p.dsp_out.shift_in(0x1100);
    r.p.dsp_out.shift_in(0x2200);
    r.setup(
        0,
        Cfg {
            src: 4,
            dst: 6,
            count: 4,
            format: 4,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run(40);
    assert_eq!(r.p.temporal.wptr, 2);
    assert_eq!(r.status(0), BUSY | STALL);
    r.p.dsp_out.shift_in(0x3300);
    r.p.dsp_out.shift_in(0x4400);
    r.run_until_idle(0, 40);
    assert_eq!(&r.p.temporal.mem[..4], &[0x11, 0x22, 0x33, 0x44]);
    assert_eq!(r.status(0), DONE);
}

/// 発生源 1 に 1 スロットの幅のパルスを出す。
fn pulse<P: Ports>(r: &mut Rig<P>) {
    r.f.set_trigger_inputs(1 << 1);
    r.step();
    r.f.set_trigger_inputs(0);
    r.step();
}

#[test]
fn each_trigger_moves_one_element() {
    let mut r = Rig::new();
    r.setup(
        0,
        Cfg {
            src: 7,
            dst: 6,
            count: 3,
            trigger: 1 | 0x08,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run(10);
    assert_eq!(r.status(0), BUSY | WAIT);
    for n in 1..=3 {
        pulse(&mut r);
        r.run(10);
        assert_eq!(r.p.temporal.wptr, n);
    }
    assert_eq!(r.status(0), DONE);
}

#[test]
fn block_trigger_with_loop_keeps_one_pending_and_counts_a_miss() {
    let mut r = Rig::new();
    r.setup(
        0,
        Cfg {
            src: 7,
            dst: 6,
            count: 4,
            trigger: 1 | 0x10,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run(6);
    assert_eq!(r.status(0), BUSY | WAIT);
    // 1 回目でブロックを始め、ブロックの途中の 2 回目は保留、3 回目は取りこぼし。
    pulse(&mut r);
    while r.starts().is_empty() {
        r.step();
    }
    pulse(&mut r);
    pulse(&mut r);
    r.run(40);
    // 2 ブロック（8 要素）で止まり、次のトリガを待つ。
    assert_eq!(r.p.temporal.wptr, 8);
    assert_eq!(r.status(0), BUSY | WAIT | DONE | MISS);
    r.write(STATUS, DONE | MISS);
    assert_eq!(r.status(0), BUSY | WAIT);
}

#[test]
fn loop_restarts_from_the_settings() {
    let mut r = Rig::new();
    r.p.staging[0x10..0x13].copy_from_slice(&[7, 8, 9]);
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 6,
            src_addr: 0x10,
            count: 3,
            trigger: 0x10,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run(60);
    r.write(CTRL, 2);
    assert_eq!(&r.p.temporal.mem[..9], &[7, 8, 9, 7, 8, 9, 7, 8, 9]);
    assert_eq!(r.status(0) & BUSY, 0);
    assert_ne!(r.status(0) & DONE, 0);
}

#[test]
fn stop_discards_the_element_not_yet_written() {
    let mut r = Rig::new();
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 6,
            count: 100,
            ..Cfg::default()
        },
    );
    r.start(0);
    while r
        .trace
        .iter()
        .filter(|x| x.decision == Decision::Write(0))
        .count()
        < 3
    {
        r.step();
    }
    // 3 つ目の要素の書込みを決めた直後に止める。
    r.write(CTRL, 2);
    r.run(10);
    assert_eq!(r.p.temporal.wptr, 2);
    assert_eq!(r.status(0), 0);
    assert!(!r.f.irq());
}

// ---------------------------------------------------------------- 延びるアクセス

/// port 1 と DOC の port 2 を、決めた数のスロットの後に答えるポートにする。ほかは即答。
struct SlowPorts {
    doc: bool,
    /// `continue_access` を何回呼ばれたら答えるか。`None` は答えない。
    answer_after: Option<u32>,
    continues: u32,
    aborts: u32,
    writes: Vec<(u8, u16)>,
}

impl SlowPorts {
    fn new(doc: bool, answer_after: Option<u32>) -> Self {
        Self {
            doc,
            answer_after,
            continues: 0,
            aborts: 0,
            writes: Vec::new(),
        }
    }
}

impl Ports for SlowPorts {
    fn ready(&self, _port: u8, _dir: Dir) -> bool {
        true
    }
    fn access(&mut self, a: &Access) -> PortReply {
        if a.dir == Dir::Write {
            self.writes.push((a.port, a.data));
        }
        self.continues = 0;
        if a.port == 1 || (a.port == 2 && self.doc) {
            PortReply::Wait
        } else {
            PortReply::Done(0x55)
        }
    }
    fn continue_access(&mut self, _port: u8) -> PortReply {
        self.continues += 1;
        match self.answer_after {
            Some(n) if self.continues >= n => PortReply::Done(0x1234),
            _ => PortReply::Wait,
        }
    }
    fn abort_access(&mut self, _port: u8) {
        self.aborts += 1;
    }
    fn doc_present(&self) -> bool {
        self.doc
    }
    fn module_present(&self, _port: u8) -> bool {
        false
    }
    fn port_reset(&mut self, _mask: u16) {}
    fn write_audio_n(&mut self, _n: u16) {}
}

#[test]
fn a_long_access_times_out_after_32_clocks() {
    let mut r = Rig::with(SlowPorts::new(false, None));
    r.setup(
        0,
        Cfg {
            src: 1,
            dst: 6,
            format: 3,
            count: 4,
            trigger: 0x20,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run(40);
    assert_eq!(r.status(0), ERROR);
    assert!(r.f.irq());
    assert_eq!(r.p.aborts, 1);
    // 時間切れまで 16 スロット（32 クロック）、アクセスは port 1 の 1 回だけ。
    let held = r
        .trace
        .iter()
        .filter(|x| x.access.is_some_and(|a| a.port == 1))
        .count();
    assert_eq!(held as u32, TIMEOUT_SLOTS);
    assert!(r.p.writes.is_empty());
}

#[test]
fn doc_writes_wait_for_the_answer() {
    let mut r = Rig::with(SlowPorts::new(true, Some(3)));
    r.setup(
        0,
        Cfg {
            src: 7,
            dst: 2,
            format: 0,
            count: 2,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 60);
    assert_eq!(r.status(0), DONE);
    assert_eq!(r.p.writes, vec![(2, 0x55), (2, 0x55)]);
    // 書込みは 4 スロット（始めたスロットと、3 回目の続きで答える）。port 2 は満杯・空の信号を持つ
    // ポートなので、終わったスロットの次の頭でも新しい要素を始めず、その次の頭で始める（DR-046 (2)）。
    let w: Vec<&SlotRecord> = r
        .trace
        .iter()
        .filter(|x| x.access.is_some_and(|a| a.port == 2))
        .collect();
    assert_eq!(w.len(), 8);
    let first_end = w[3].slot;
    let starts = r.starts();
    assert_eq!(starts[1].0, first_end + 2);
}

// ---------------------------------------------------------------- CPU と割込み

#[test]
fn cpu_access_to_the_register_file_takes_a_slot_from_the_transfer() {
    let mut r = Rig::new();
    for i in 0..16 {
        r.p.staging[0x100 + i] = i as u8;
    }
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 0,
            src_addr: 0x100,
            dst_addr: 0x200,
            count: 8,
            src_step: 2,
            dst_step: 2,
            format: 1,
            ..Cfg::default()
        },
    );
    r.start(0);
    while r.starts().len() < 3 {
        r.step();
    }
    let remaining = r.read(COUNT);
    let host = r.trace.iter().rev().find(|x| x.aop == Aop::Host).unwrap();
    assert!(host.slot > r.starts()[2].0);
    assert!(remaining < 8);
    r.run_until_idle(0, 100);
    assert_eq!(
        &r.p.staging[0x200..0x210],
        &r.p.staging[0x100..0x110].to_vec()[..]
    );
    assert_eq!(r.status(0), DONE);
}

#[test]
fn irq_follows_done_and_irq_en() {
    let mut r = Rig::new();
    r.setup(
        0,
        Cfg {
            trigger: 0x20,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 20);
    assert!(r.f.irq());
    r.write(STATUS, DONE);
    assert!(!r.f.irq());

    let mut r = Rig::new();
    r.setup(1, Cfg::default());
    r.start(1);
    r.run_until_idle(1, 20);
    assert_eq!(r.status(1), DONE);
    assert!(!r.f.irq());
}

// ---------------------------------------------------------------- ポートの細部

#[test]
fn port0_byte_lanes_and_word_alignment() {
    let mut r = Rig::new();
    r.p.staging[0x1000] = 0xAA;
    r.p.staging[0x1001] = 0xBB;
    // 8 bit：奇数番地は下位バイト、書く先の偶数番地は上位バイト。
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 0,
            src_addr: 0x1001,
            dst_addr: 0x2000,
            count: 1,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 20);
    assert_eq!(&r.p.staging[0x2000..0x2002], &[0xBB, 0x00]);
    // 16 bit：bit 0 を無視する。
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 0,
            src_addr: 0x1001,
            dst_addr: 0x3001,
            count: 1,
            format: 1,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run_until_idle(0, 20);
    assert_eq!(&r.p.staging[0x3000..0x3002], &[0xAA, 0xBB]);
}

#[test]
fn mask_xor_and_shift_apply_on_the_way() {
    let mut r = Rig::new();
    r.p.staging[0x10..0x12].copy_from_slice(&[0x12, 0x34]);
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 0,
            src_addr: 0x10,
            dst_addr: 0x20,
            count: 1,
            format: 5,
            ..Cfg::default()
        },
    );
    r.write(SHIFT, 8);
    r.write(MASK, 0xFF0F);
    r.write(XOR, 0x0001);
    r.start(0);
    r.run_until_idle(0, 20);
    // 0x1234 を 8 回して 0x3412、MASK で 0x3402、XOR で 0x3403。
    assert_eq!(&r.p.staging[0x20..0x22], &[0x34, 0x03]);
}

#[test]
fn port_reset_clears_fifos_and_pointers() {
    let mut r = Rig::new();
    r.p.dsp_in.shift_in(1);
    r.p.dsp_out.shift_in(2);
    r.p.temporal.wptr = 5;
    r.p.temporal.rptr = 9;
    r.write(0x100, (1 << 3) | (1 << 7));
    assert!(r.p.dsp_in.is_empty());
    assert_eq!(r.p.dsp_out.len(), 1);
    assert_eq!((r.p.temporal.wptr, r.p.temporal.rptr), (5, 0));
}

// ---------------------------------------------------------------- 保存

#[test]
fn snapshot_round_trip_mid_transfer() {
    let mut r = Rig::new();
    for i in 0..32 {
        r.p.staging[i] = i as u8 ^ 0x5A;
    }
    r.setup(
        0,
        Cfg {
            src: 0,
            dst: 6,
            count: 32,
            ..Cfg::default()
        },
    );
    r.setup(
        1,
        Cfg {
            src: 7,
            dst: 3,
            count: 8,
            format: 2,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.start(1);
    r.run(23);
    let saved = serde_json::to_string(&(&r.f, &r.p)).unwrap();
    let (f2, p2): (Fabric, FabricPorts) = serde_json::from_str(&saved).unwrap();
    let mut b = Rig::with(p2);
    b.f = f2;
    for _ in 0..60 {
        assert_eq!(r.step(), b.step());
    }
    assert_eq!(r.f, b.f);
    assert_eq!(r.p, b.p);
}

#[test]
fn a_staging_channel_keeps_its_rate_while_the_other_channel_waits() {
    // DR-048（OI-052）：port 0 から読むチャネルは S の頭で候補を手放さないので、相手がトリガを待って
    // 動いているだけなら、1 チャネルのときと同じ 2 スロットに 1 要素で進む。DR-046 (2) のままでは、
    // 候補が S の頭にだけ来て 2 要素で止まっていた。
    let mut got = Vec::new();
    for other in [false, true] {
        let mut r = Rig::new();
        r.setup(
            0,
            Cfg {
                count: 100,
                ..Cfg::default()
            },
        );
        if other {
            // ch1 は文字 ROM → Temporal RAM で、トリガ（発生源 6）を待つだけ
            r.setup(
                1,
                Cfg {
                    src: 9,
                    trigger: NEVER,
                    ..Cfg::default()
                },
            );
            r.start(1);
            // ch1 の再設定（3 スロット）を先に済ませる
            r.run(5);
        }
        r.start(0);
        r.run(400);
        let starts: Vec<(u64, u8)> = r.starts().into_iter().filter(|x| x.1 == 0).collect();
        let g = gaps(&starts);
        assert!(g.iter().all(|&x| x == 2), "{g:?}");
        got.push((starts.len(), r.status(0)));
    }
    assert_eq!(got, vec![(100, DONE), (100, DONE)]);
}

// ---------------------------------------------------------------- CPU のバスサイクルの規則（DR-098）
//
// 仕様書 §06 の規則を手で計算した値と比べる。S0 のクロックを at、S0 を含むスロットを n（at ＝ 2n＋odd。odd は
// S0 が q2 の縁）とする。HACK の Y がストローブ（読出しは S2 ＝ at＋1、書込みは S4 ＝ at＋2 の立上り）を必ず
// 見るのは、その 1 クロック後の 16 MHz の縁。CAP・DTK はその後の最初の q1 の縁で立つので、取込みのスロットは
// c ＝ (at＋3＋書込み)÷2。DTACK は q1(r) の縁で出て q3(r) の立下りで認識され（r は c、A ファイルは許可の
// スロット g ＝ c＋1）、S4 の終わりは at＋2.5 クロックなので、待ちは (2r＋1.5)−(at＋2.5) ＝ 2r−at−1。

/// スロット `n`、位相 `odd` に S0 を置いたバスサイクル。答えと待ちの数。
fn cycle(
    r: &mut Rig<FabricPorts>,
    n: u64,
    odd: bool,
    off: u16,
    w: Option<u16>,
) -> (HostReply, u32) {
    while r.f.slot() < n {
        r.step();
    }
    match r.f.host_begin(n, odd, off, w, &r.p) {
        HostStart::Done(a, waits) => (a, waits),
        HostStart::Pending => loop {
            r.step();
            if let Some(a) = r.f.host_finish() {
                break a;
            }
        },
        HostStart::BusError => panic!("{off:#x}"),
    }
}

#[test]
fn bus_cycle_waits_follow_the_s0_phase() {
    // （オフセット、書込み、S0 が q0 の待ち、q2 の待ち、許可のスロット − n）
    // STATUS の読出し：q0 は c ＝ n＋1、待ち 2(n＋1)−2n−1 ＝ 1。q2 は c ＝ n＋2、待ち 2(n＋2)−(2n＋1)−1 ＝ 2。
    // CTRL の書込み：c ＝ n＋2（どちらも）。待ち q0 は 3、q2 は 2。
    // COUNT の読出し：g ＝ c＋1。q0 は g ＝ n＋2 で待ち 3、q2 は g ＝ n＋3 で待ち 4。
    // COUNT の書込み：g ＝ n＋3。待ち q0 は 5、q2 は 4。
    for (off, w, waits, grant) in [
        (STATUS, None, [1, 2], None),
        (CTRL, Some(0), [3, 2], None),
        (COUNT, None, [3, 4], Some([2, 3])),
        (COUNT, Some(7), [5, 4], Some([3, 3])),
    ] {
        for odd in [false, true] {
            let mut r = Rig::new();
            let n = 10;
            let (_, got) = cycle(&mut r, n, odd, off, w);
            assert_eq!(got, waits[odd as usize], "{off:#x} {w:?} {odd}");
            let hosts: Vec<u64> = r
                .trace
                .iter()
                .filter(|x| x.aop == Aop::Host)
                .map(|x| x.slot)
                .collect();
            assert_eq!(
                hosts,
                grant.map(|g| vec![n + g[odd as usize]]).unwrap_or_default(),
                "{off:#x}"
            );
        }
    }
}

#[test]
fn ctrl_and_w1c_take_effect_three_slots_after_s0() {
    // REQ が LCLK(c)（c ＝ n＋2）で取り込み、c＋1 ＝ n＋3 のスロットの頭の判断と LCLK で効く。STATUS は n＋3 の
    // スロットの後に変わり、n＋2 の後はまだ変わらない。
    for odd in [false, true] {
        // START
        let mut r = Rig::new();
        r.setup(
            0,
            Cfg {
                trigger: NEVER,
                ..Cfg::default()
            },
        );
        r.run(2);
        let n = r.f.slot() + 1;
        cycle(&mut r, n, odd, CTRL, Some(1));
        r.run_to(n + 3);
        assert_eq!(r.status(0) & BUSY, 0, "START {odd}");
        r.step();
        assert_eq!(r.status(0) & BUSY, BUSY, "START {odd}");
        // STOP（トリガを待って動いている）
        r.run(10);
        let n = r.f.slot() + 1;
        cycle(&mut r, n, odd, CTRL, Some(2));
        r.run_to(n + 3);
        assert_eq!(r.status(0) & BUSY, BUSY, "STOP {odd}");
        r.step();
        assert_eq!(r.status(0) & BUSY, 0, "STOP {odd}");
        // W1C（DONE）
        r.setup(0, Cfg::default());
        r.start(0);
        r.run_until_idle(0, 40);
        assert_eq!(r.status(0), DONE);
        let n = r.f.slot() + 1;
        cycle(&mut r, n, odd, STATUS, Some(DONE));
        r.run_to(n + 3);
        assert_eq!(r.status(0), DONE, "W1C {odd}");
        r.step();
        assert_eq!(r.status(0), 0, "W1C {odd}");
    }
}

#[test]
fn status_reads_see_the_state_after_the_capture_slot() {
    // DONE が立つスロット k（そのスロットの LCLK の後に STATUS が変わる）を先に調べ、S0 を k の前後に置いて
    // 読む。値は c ＝ n＋1（q0）か n＋2（q2）のスロットの後のもので、c ≥ k なら DONE が見える。
    let mut base = Rig::new();
    base.setup(
        0,
        Cfg {
            count: 3,
            ..Cfg::default()
        },
    );
    base.start(0);
    let mut probe = Rig {
        f: base.f.clone(),
        p: base.p.clone(),
        trace: Vec::new(),
    };
    while probe.status(0) & DONE == 0 {
        probe.step();
    }
    let k = probe.f.slot() - 1;
    for n in k - 3..=k {
        for odd in [false, true] {
            let mut r = Rig {
                f: base.f.clone(),
                p: base.p.clone(),
                trace: Vec::new(),
            };
            let (got, _) = cycle(&mut r, n, odd, STATUS, None);
            let c = n + 1 + odd as u64;
            let done = if c >= k { DONE } else { BUSY };
            assert_eq!(got, HostReply::Data(done), "k {k} n {n} {odd}");
        }
    }
}

#[test]
fn start_is_taken_only_if_the_channel_was_stopped_at_the_capture_slot() {
    // START は LCLK(c)（c ＝ n＋2）の BUSY、つまりスロット c の頭の段階を見る。転送が終わって段階が止まる
    // スロットを k（その LCLK の後に BUSY が消える）とすると、c ≥ k＋1（n ≥ k−1）なら受け付けて n＋3 で
    // 再設定を始め、c ≤ k なら動作中の START として捨てる。
    let mut base = Rig::new();
    base.setup(
        0,
        Cfg {
            count: 3,
            ..Cfg::default()
        },
    );
    base.start(0);
    let mut probe = Rig {
        f: base.f.clone(),
        p: base.p.clone(),
        trace: Vec::new(),
    };
    while probe.status(0) & BUSY != 0 {
        probe.step();
    }
    let k = probe.f.slot() - 1;
    for n in k - 3..=k {
        for odd in [false, true] {
            let mut r = Rig {
                f: base.f.clone(),
                p: base.p.clone(),
                trace: Vec::new(),
            };
            cycle(&mut r, n, odd, CTRL, Some(1));
            let check = k.max(n + 3) + 2;
            r.run_to(check);
            let want = if n + 1 >= k { BUSY } else { 0 };
            assert_eq!(r.status(0) & BUSY, want, "k {k} n {n} {odd}");
        }
    }
}

#[test]
fn stop_then_start_back_to_back_restarts_the_channel() {
    // STOP（S0 が n の q0、待ち 3）に続けて START を書くと、START の S0 は n＋3 の q2 で、c ＝ n＋5。STOP は
    // n＋3 で効くので、START は止まったチャネルとして受け付けられ、n＋6 で再設定を始める。
    let mut r = Rig::new();
    r.setup(
        0,
        Cfg {
            trigger: NEVER,
            ..Cfg::default()
        },
    );
    r.start(0);
    r.run(10);
    assert_eq!(r.status(0), BUSY | WAIT);
    let n = r.f.slot() + 1;
    let (_, waits) = cycle(&mut r, n, false, CTRL, Some(2));
    assert_eq!(waits, 3);
    // 7 クロックのサイクルの次の S0 は 4n＋14 の縁（n＋3 の q2）
    cycle(&mut r, n + 3, true, CTRL, Some(1));
    r.run_to(n + 4);
    assert_eq!(r.status(0) & BUSY, 0);
    r.run_to(n + 6);
    assert_eq!(r.status(0) & BUSY, 0);
    r.step();
    assert_eq!(r.status(0) & BUSY, BUSY);
}

#[test]
fn config_writes_take_effect_after_the_strobe() {
    // 設定のレジスタは、ストローブの終わり（S7 ＝ q1(c＋1) の後）に変わる。実機では c＋1 の LCLK から c＋2 の
    // SCLK までのどこかなので、エミュレータは c＋2 ＝ n＋4 のスロットの頭に置く。
    for odd in [false, true] {
        let mut r = Rig::new();
        r.run(4);
        let n = 5;
        cycle(&mut r, n, odd, FORMAT, Some(3));
        r.run_to(n + 4);
        assert_eq!(r.f.regs(0).format, 0, "{odd}");
        r.step();
        assert_eq!(r.f.regs(0).format, 3, "{odd}");
    }
}

#[test]
fn trigger_needs_four_slots_before_start() {
    // TRIGGER の発生源を、入力が High のもの（1：VBlank）へ替えると、選択器の後の同期化に立上りが伝わる。
    // TRIGGER（S0 のスロット nT）は nT＋4 の頭で変わり、U36 が LCLK(nT＋4)、STS が LCLK(nT＋5) で取り込み、
    // 立上りを LCLK(nT＋6) で数える。止まっている間は数えない。START（S0 のスロット nS）は nS＋3 で段階を
    // 変えるので、nS ≥ nT＋4 なら立上りは数えられない。続けて書くと（TRIGGER の待ちの後の S0 は nT＋3 の q2）
    // 数えられ、転送がトリガなしで進む。
    for (d, odd, want) in [
        (3, true, DONE),
        (4, false, BUSY | WAIT),
        (4, true, BUSY | WAIT),
    ] {
        let mut r = Rig::new();
        r.setup(
            0,
            Cfg {
                trigger: 0,
                ..Cfg::default()
            },
        );
        r.f.set_trigger_inputs(1 << 1);
        r.run(4);
        let nt = r.f.slot() + 1;
        cycle(&mut r, nt, false, TRIGGER, Some(1));
        cycle(&mut r, nt + d, odd, CTRL, Some(1));
        r.run(40);
        assert_eq!(r.status(0), want, "d {d} {odd}");
    }
}
