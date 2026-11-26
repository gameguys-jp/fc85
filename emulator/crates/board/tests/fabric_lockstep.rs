//! Data Fabric の門の模型（`fabric_gate`。fabric.md §06 の PAL の式から生成した関数をつないだもの）と
//! 動作の模型（`fabric`）を並べて動かし、スロットごとに突き合わせる（DR-085）。
//!
//! 比べるもの：スロットの記録（`SlotRecord`）、STATUS、段階、割込み、CPU への応答、ポートへの働きかけ
//! （模型のポートは呼ばれた順の記録、`FabricPorts` は中身の全部）。
//! CPU の 2 回のアクセスの間には 1 スロット以上おく（門の模型の前提。fabric_gate/mod.rs の冒頭）。

mod fabric_gate;

use fabric_gate::GateFabric;
use fc85_board::fabric::{Access, Dir, Fabric, HostReply, PortReply, Ports};
use fc85_board::ports::FabricPorts;

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

/// 決まった擬似乱数（xorshift64*）。
#[derive(Clone, Debug, PartialEq, Eq)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// n 回に 1 回。
    fn one_in(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u64) as usize]
    }
}

// ---------------------------------------------------------------- 突き合わせの台

/// 2 つの模型の、ポートの側の比べ方。
trait Compare: Ports + Clone {
    /// スロットの後に比べる。`slot` は進めたスロットの番号。
    fn compare(a: &mut Self, b: &mut Self, slot: u64);
}

impl Compare for FabricPorts {
    fn compare(a: &mut Self, b: &mut Self, slot: u64) {
        // 中身が大きいので 16 スロットごと。
        if slot.is_multiple_of(16) {
            assert!(a == b, "スロット {slot}：ポートの中身が違う");
        }
    }
}

/// 網羅の目安：突き合わせたスロットのうち、それぞれのことが起きた数。
const COVER: [&str; 20] = [
    "スロット",
    "アクセス",
    "Start",
    "Write",
    "CountAfterWrite",
    "Host",
    "SrcStep",
    "DstStep",
    "Count",
    "Reload",
    "2 スロットのアクセス",
    "延びたアクセス",
    "DONE が立つ",
    "STALL が立つ",
    "ERROR が立つ",
    "MISS が立つ",
    "WAIT",
    "動作中の STOP",
    "2 チャネルが動作",
    "割込み",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Cover([u64; COVER.len()]);

impl Cover {
    fn add(&mut self, o: &Cover) {
        for (a, b) in self.0.iter_mut().zip(o.0) {
            *a += b;
        }
    }
    fn get(&self, name: &str) -> u64 {
        self.0[COVER.iter().position(|n| *n == name).unwrap()]
    }
}

impl std::fmt::Display for Cover {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for (n, v) in COVER.iter().zip(self.0) {
            write!(f, "{n} {v}、")?;
        }
        Ok(())
    }
}

struct Lock<P: Compare> {
    f: Fabric,
    g: GateFabric,
    pf: P,
    pg: P,
    cover: Cover,
    prev_status: [u16; 2],
}

impl<P: Compare> Lock<P> {
    fn new(p: P) -> Self {
        Self {
            f: Fabric::new(),
            g: GateFabric::new(),
            pf: p.clone(),
            pg: p,
            cover: Cover::default(),
            prev_status: [0; 2],
        }
    }

    fn slots(&self) -> u64 {
        self.cover.0[0]
    }

    fn count(&mut self, name: &str, yes: bool) {
        let i = COVER.iter().position(|n| *n == name).unwrap();
        self.cover.0[i] += yes as u64;
    }

    fn step(&mut self) {
        use fc85_board::fabric::{Aop, Decision, Phase};
        let a = self.f.step_slot(&mut self.pf);
        let b = self.g.step_slot(&mut self.pg);
        assert_eq!(a, b, "スロット {} の記録", a.slot);
        let mut st = [0; 2];
        for (c, v) in st.iter_mut().enumerate() {
            *v = self.f.status(c);
            assert_eq!(
                *v,
                self.g.status(c),
                "スロット {} の ch{c} の STATUS",
                a.slot
            );
            assert_eq!(self.f.phase(c), self.g.phase(c), "スロット {}", a.slot);
        }
        assert_eq!(self.f.irq(), self.g.irq(), "スロット {} の割込み", a.slot);
        P::compare(&mut self.pf, &mut self.pg, a.slot);

        self.count("スロット", true);
        self.count("アクセス", a.access.is_some());
        self.count("Start", matches!(a.decision, Decision::Start(_)));
        self.count("Write", matches!(a.decision, Decision::Write(_)));
        self.count(
            "CountAfterWrite",
            matches!(a.decision, Decision::CountAfterWrite(_)),
        );
        self.count("Host", a.aop == Aop::Host);
        self.count("SrcStep", matches!(a.aop, Aop::SrcStep(_)));
        self.count("DstStep", matches!(a.aop, Aop::DstStep(_)));
        self.count("Count", matches!(a.aop, Aop::Count(_)));
        self.count("Reload", matches!(a.aop, Aop::Reload(..)));
        self.count(
            "2 スロットのアクセス",
            a.access.is_some_and(|x| x.port == 9) && a.access_done,
        );
        self.count(
            "延びたアクセス",
            a.access
                .is_some_and(|x| x.port == 1 || (x.port == 2 && self.pf.doc_present()))
                && !a.access_done,
        );
        for (bit, name) in [
            (2, "DONE が立つ"),
            (4, "STALL が立つ"),
            (8, "ERROR が立つ"),
            (16, "MISS が立つ"),
        ] {
            let rise = (st[0] & !self.prev_status[0] | st[1] & !self.prev_status[1]) & bit;
            self.count(name, rise != 0);
        }
        self.count("WAIT", (st[0] | st[1]) & 0x20 != 0);
        self.count(
            "2 チャネルが動作",
            self.f.phase(0) == Phase::Run && self.f.phase(1) == Phase::Run,
        );
        self.count("割込み", self.f.irq());
        self.prev_status = st;
    }

    fn run(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    fn set_trigger_inputs(&mut self, v: u8) {
        self.f.set_trigger_inputs(v);
        self.g.set_trigger_inputs(v);
    }

    /// A ファイルの許可を待つ。
    fn wait_poll(&mut self) -> HostReply {
        for _ in 0..4 {
            self.step();
            let (a, b) = (self.f.host_poll(), self.g.host_poll());
            assert_eq!(a, b, "A ファイルの応答");
            if let Some(r) = a {
                return r;
            }
        }
        panic!("A ファイルの許可が来ない");
    }

    /// CPU の書込み。応答を突き合わせ、次のアクセスまで 1 スロット進める。
    fn write(&mut self, off: u16, v: u16) -> HostReply {
        let a = self.f.host_write(off, v, &mut self.pf);
        let b = self.g.host_write(off, v, &mut self.pg);
        assert_eq!(a, b, "書込み {off:#x}");
        let r = if a == HostReply::Wait {
            self.wait_poll()
        } else {
            a
        };
        self.step();
        r
    }

    fn read(&mut self, off: u16) -> HostReply {
        let a = self.f.host_read(off, &self.pf);
        let b = self.g.host_read(off, &self.pg);
        assert_eq!(a, b, "読出し {off:#x}");
        let r = if a == HostReply::Wait {
            self.wait_poll()
        } else {
            a
        };
        self.step();
        r
    }

    fn setup(&mut self, ch: u16, c: [u16; 9]) {
        let b = ch * 0x40;
        for (off, v) in [
            SRC, DST, SRC_ADDR, DST_ADDR, COUNT, SRC_STEP, DST_STEP, FORMAT, TRIGGER,
        ]
        .into_iter()
        .zip(c)
        {
            self.write(b + off, v);
        }
    }

    fn start(&mut self, ch: u16) {
        self.write(ch * 0x40 + CTRL, 1);
    }

    fn stop(&mut self, ch: u16) {
        self.write(ch * 0x40 + CTRL, 2);
    }

    /// CPU のアクセス（`write` か `read`）。動作中のチャネルへの STOP を数える。
    fn host(&mut self, off: u16, w: Option<u16>) -> HostReply {
        if let Some(v) = w
            && off < 0x80
            && off & 0x3F == CTRL
            && v & 2 != 0
        {
            let busy = self.f.status((off >> 6) as usize) & 1 != 0;
            self.count("動作中の STOP", busy);
        }
        match w {
            Some(v) => self.write(off, v),
            None => self.read(off),
        }
    }

    fn run_until_idle(&mut self, ch: usize, limit: usize) {
        for _ in 0..limit {
            if self.f.status(ch) & 1 == 0 {
                return;
            }
            self.step();
        }
        panic!("{limit} スロットで終わらない");
    }

    /// 終わりに、ポートの中身をもう一度比べる。
    fn finish(mut self) -> Cover {
        P::compare(&mut self.pf, &mut self.pg, 0);
        self.cover
    }
}

/// 設定の 9 語（SRC、DST、SRC_ADDR、DST_ADDR、COUNT、SRC_STEP、DST_STEP、FORMAT、TRIGGER）。
fn cfg(src: u16, dst: u16, src_addr: u16, dst_addr: u16, count: u16, format: u16) -> [u16; 9] {
    [src, dst, src_addr, dst_addr, count, 1, 1, format, 0]
}

fn with_trigger(mut c: [u16; 9], t: u16) -> [u16; 9] {
    c[8] = t;
    c
}

fn with_steps(mut c: [u16; 9], s: u16, d: u16) -> [u16; 9] {
    c[5] = s;
    c[6] = d;
    c
}

fn pulse<P: Compare>(r: &mut Lock<P>) {
    r.set_trigger_inputs(1 << 1);
    r.step();
    r.set_trigger_inputs(0);
    r.step();
}

// ---------------------------------------------------------------- 決めた筋書き（fabric.rs の試験と同じ転送）

#[test]
fn scenarios_of_the_fabric_tests_match() {
    // staging → temporal、staging → staging、COUNT の更新の 4 通り、FIFO、文字 ROM。
    let mut r = Lock::new(FabricPorts::new());
    for i in 0..16 {
        r.pf.staging[0x1000 + i] = 0x11u8.wrapping_mul(i as u8 + 1);
        r.pg.staging[0x1000 + i] = 0x11u8.wrapping_mul(i as u8 + 1);
    }
    r.setup(0, cfg(0, 6, 0x1000, 0, 8, 0));
    r.start(0);
    r.run_until_idle(0, 100);
    r.setup(0, with_steps(cfg(0, 0, 0x1000, 0x3000, 4, 1), 2, 2));
    r.start(0);
    r.run_until_idle(0, 100);
    for (src, dst) in [(7, 6), (0, 6), (0, 0), (7, 0)] {
        r.setup(0, cfg(src, dst, 0, 0x4000, 1, 0));
        r.start(0);
        r.run_until_idle(0, 40);
    }
    for p in [&mut r.pf, &mut r.pg] {
        for i in 0..6 {
            p.dsp_out.shift_in(0x0100 * (i + 1) + 0x55);
        }
        p.char_rom[0x10..0x13].copy_from_slice(&[0xA1, 0xB2, 0xC3]);
    }
    r.setup(0, cfg(4, 6, 0, 0, 6, 4));
    r.start(0);
    r.run_until_idle(0, 100);
    r.setup(0, cfg(7, 3, 0, 0, 5, 2));
    r.start(0);
    r.run_until_idle(0, 100);
    r.setup(0, cfg(9, 6, 0x8010, 0, 3, 0));
    r.start(0);
    r.run_until_idle(0, 60);

    // 2 チャネル：候補の入れ替え（port 9 と port 0 から）、交互の要素。
    for src in [9, 0] {
        for ch in 0..2 {
            r.setup(ch, with_trigger(cfg(src, 6, 0, 0, 1, 0), 6));
        }
        r.start(0);
        r.start(1);
        r.run(20);
        r.stop(0);
        r.stop(1);
    }
    r.setup(0, cfg(7, 6, 0, 0, 3, 0));
    r.setup(1, cfg(9, 6, 0, 0, 3, 0));
    r.start(0);
    r.start(1);
    r.run(80);

    // 空の FIFO で STALL、FIFO を足して再開。
    r.setup(0, cfg(4, 6, 0, 0, 4, 4));
    r.start(0);
    r.run(40);
    for p in [&mut r.pf, &mut r.pg] {
        p.dsp_out.shift_in(0x3300);
        p.dsp_out.shift_in(0x4400);
        p.dsp_out.shift_in(0x5500);
        p.dsp_out.shift_in(0x6600);
    }
    r.run_until_idle(0, 60);

    // EACH のトリガ、LOOP のブロックのトリガ（保留と MISS）、W1C。
    r.setup(0, with_trigger(cfg(7, 6, 0, 0, 3, 0), 1 | 0x08));
    r.start(0);
    r.run(10);
    for _ in 0..3 {
        pulse(&mut r);
        r.run(10);
    }
    r.setup(0, with_trigger(cfg(7, 6, 0, 0, 4, 0), 1 | 0x10));
    r.start(0);
    r.run(6);
    pulse(&mut r);
    r.run(3);
    pulse(&mut r);
    pulse(&mut r);
    r.run(40);
    r.write(STATUS, 0x12);
    r.read(STATUS);
    r.stop(0);

    // LOOP で設定値から再開、STOP。
    r.setup(0, with_trigger(cfg(0, 6, 0x10, 0, 3, 0), 0x10));
    r.start(0);
    r.run(60);
    r.stop(0);

    // 3 つ目の要素の W を決めた直後の STOP。
    r.setup(0, cfg(0, 6, 0, 0, 100, 0));
    r.start(0);
    r.run(9);
    r.stop(0);
    r.run(10);

    // 転送中の A ファイルの読み書き、IRQ、port 0 のバイトレーン、MASK・XOR・SHIFT、PORT_RESET。
    r.setup(0, with_trigger(cfg(0, 6, 0x1000, 0, 12, 0), 0x20));
    r.start(0);
    r.run(3);
    r.read(COUNT);
    r.read(SRC_ADDR);
    r.write(DST_ADDR, 0x1234);
    r.run_until_idle(0, 100);
    r.write(STATUS, 0x02);
    for f in 0..8 {
        r.setup(1, cfg(0, 0, 0x1001, 0x2001, 3, f));
        r.write(0x40 + MASK, 0x0FF0);
        r.write(0x40 + XOR, 0x5A5A);
        r.write(0x40 + SHIFT, f * 5);
        r.start(1);
        r.run_until_idle(1, 100);
    }
    r.write(0x100, 0xFFFF);
    r.write(0x102, 0x0123);
    r.read(0x104);

    // staging のチャネルは、もう一方が FIFO で待っていても速さが変わらない。
    r.setup(0, cfg(4, 6, 0, 0, 4, 4));
    r.setup(1, cfg(0, 6, 0x1000, 0, 8, 0));
    r.start(0);
    r.start(1);
    r.run(60);
    r.stop(0);
    r.run(4);
    let c = r.finish();
    for name in [
        "Start",
        "Write",
        "CountAfterWrite",
        "Reload",
        "STALL が立つ",
        "MISS が立つ",
    ] {
        assert!(c.get(name) > 0, "{c}");
    }
}

#[test]
fn doc_board_long_accesses_match() {
    // DOC 子基板の port 2：書込み・読出しは窓まで延びる。
    let mut p = FabricPorts::new();
    p.doc = true;
    let mut r = Lock::new(p);
    for (src, dst) in [(0, 2), (2, 0), (7, 2), (2, 6)] {
        r.setup(0, cfg(src, dst, 0x40, 0x80, 6, 0));
        r.start(0);
        r.run_until_idle(0, 400);
    }
    r.setup(0, cfg(0, 2, 0x40, 0x80, 40, 0));
    r.setup(1, cfg(9, 6, 0, 0, 20, 0));
    r.start(0);
    r.start(1);
    r.run(30);
    r.stop(0);
    r.run_until_idle(1, 400);
    r.finish();
}

// ---------------------------------------------------------------- 擬似乱数のポートと手順

/// ポートの模型。呼ばれた順に記録する。FIFO の満杯・空、準備の線、延びたアクセスの遅れと
/// port 1 の DONE の印、トリガのレベルを擬似乱数で決める。
#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Run(Option<Access>),
    Access(Access),
    Continue(u8),
    Abort(u8),
    Reset(u16),
    AudioN(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MockPorts {
    rng: Rng,
    log: Vec<Call>,
    doc: bool,
    modules: u16,
    /// FIFO の中の語の数（port 2、3、5 は書込み、4、10 は読出しの側）。
    level: [u8; 16],
    cap: u8,
    /// 延びたアクセス：ポート、残りのスロット、最後の語の印。
    long: Option<(u8, u32, bool)>,
    rdy1: bool,
    xrdy: bool,
    trig: u8,
    /// 延びたアクセスの遅れの上限（時間切れ 16 を超えることがある）。
    max_wait: u64,
    /// port 1 が最後の語の印を出す割合（n 回に 1 回）。
    mark_one_in: u64,
}

impl MockPorts {
    fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let doc = rng.one_in(2);
        let modules = rng.next() as u16 & 0xFD00;
        let cap = rng.pick(&[1u8, 2, 4, 16]);
        let max_wait = rng.pick(&[3u64, 8, 20]);
        let mark_one_in = rng.pick(&[2u64, 4, 64]);
        Self {
            rng,
            log: Vec::new(),
            doc,
            modules,
            level: [0; 16],
            cap,
            long: None,
            rdy1: true,
            xrdy: true,
            trig: 0,
            max_wait,
            mark_one_in,
        }
    }

    fn reply(&mut self, port: u8, v: u16) -> PortReply {
        if port == 1 && self.rng.one_in(self.mark_one_in) {
            PortReply::Last(v)
        } else {
            PortReply::Done(v)
        }
    }
}

impl Compare for MockPorts {
    fn compare(a: &mut Self, b: &mut Self, slot: u64) {
        assert_eq!(a.log, b.log, "スロット {slot} のポートへの働きかけ");
        a.log.clear();
        b.log.clear();
        assert!(a == b, "スロット {slot}：ポートの模型の中身が違う");
    }
}

impl Ports for MockPorts {
    fn run_slot(&mut self, starting: Option<&Access>) {
        self.log.push(Call::Run(starting.copied()));
        let r = &mut self.rng;
        for p in [2usize, 3, 5] {
            if self.level[p] > 0 && r.one_in(3) {
                self.level[p] -= 1;
            }
        }
        for p in [4usize, 10] {
            if self.level[p] < self.cap && r.one_in(3) {
                self.level[p] += 1;
            }
        }
        for b in 1..=5 {
            if r.one_in(12) {
                self.trig ^= 1 << b;
            }
        }
        self.rdy1 = !r.one_in(4);
        self.xrdy = !r.one_in(3);
    }
    fn trigger_levels(&self) -> u8 {
        self.trig
    }
    fn ready(&self, port: u8, dir: Dir) -> bool {
        let full = |p: usize| self.level[p] >= self.cap;
        match (port, dir) {
            (1, _) => self.rdy1,
            (2, _) if self.doc => true,
            (2, Dir::Write) => !full(2),
            (3, Dir::Write) => !full(3),
            (4, Dir::Read) => self.level[4] > 0,
            (5, Dir::Write) => !full(5),
            (10, Dir::Read) => self.modules & 1 << 10 != 0 && self.level[10] > 0,
            (11..=15, _) => self.xrdy,
            (0 | 6 | 7 | 9, _) => true,
            _ => false,
        }
    }
    fn access(&mut self, a: &Access) -> PortReply {
        self.log.push(Call::Access(*a));
        let v = self.rng.next() as u16;
        let p = a.port as usize;
        match (a.port, a.dir) {
            (1, _) | (2, _) if a.port == 1 || self.doc => {
                let wait = self.rng.below(self.max_wait) as u32;
                if wait == 0 {
                    return self.reply(a.port, v);
                }
                let mark = a.port == 1 && self.rng.one_in(self.mark_one_in);
                self.long = Some((a.port, wait, mark));
                return PortReply::Wait;
            }
            (2 | 3 | 5, Dir::Write) => self.level[p] = (self.level[p] + 1).min(self.cap),
            (4 | 10, Dir::Read) => self.level[p] = self.level[p].saturating_sub(1),
            _ => {}
        }
        PortReply::Done(if a.dir == Dir::Read { v } else { 0 })
    }
    fn continue_access(&mut self, port: u8) -> PortReply {
        self.log.push(Call::Continue(port));
        match &mut self.long {
            Some((p, n, mark)) if *p == port => {
                *n -= 1;
                if *n == 0 {
                    let m = *mark;
                    self.long = None;
                    let v = self.rng.next() as u16;
                    return if m {
                        PortReply::Last(v)
                    } else {
                        PortReply::Done(v)
                    };
                }
                PortReply::Wait
            }
            _ => PortReply::Wait,
        }
    }
    fn abort_access(&mut self, port: u8) {
        self.log.push(Call::Abort(port));
        self.long = None;
    }
    fn doc_present(&self) -> bool {
        self.doc
    }
    fn module_present(&self, port: u8) -> bool {
        self.modules & 1 << port != 0
    }
    fn port_reset(&mut self, mask: u16) {
        self.log.push(Call::Reset(mask));
        for p in 0..16 {
            if mask & 1 << p != 0 {
                self.level[p] = 0;
            }
        }
    }
    fn write_audio_n(&mut self, n: u16) {
        self.log.push(Call::AudioN(n));
    }
}

/// 擬似乱数の CPU の手順：設定と START、転送中の STOP・設定の書換え・A ファイルの読み書き、
/// STATUS の読出しと W1C、PORT_RESET、トリガの入力。
fn random_program(seed: u64, slots: u64) -> Cover {
    let mut r = Lock::new(MockPorts::new(seed));
    let mut rng = Rng::new(seed ^ 0xF00D);
    // ポートの選び方：使えるものを多めに、使えないもの（START の検査で ERROR）も時々。
    let srcs = [0u16, 0, 0, 1, 2, 4, 4, 7, 7, 8, 9, 9, 10, 11, 3];
    let dsts = [0u16, 0, 0, 1, 2, 2, 3, 3, 5, 6, 6, 6, 12, 4];
    let formats = [0u16, 0, 0, 1, 1, 2, 3, 4, 5, 6];
    let counts = [0u16, 1, 2, 3, 5, 8, 16, 40, 100];
    let steps = [0u16, 1, 1, 2, 0xFFFF, 0x8000];
    let mut queue: Vec<(u16, Option<u16>)> = Vec::new();
    while r.slots() < slots {
        if rng.one_in(40) {
            r.set_trigger_inputs(rng.next() as u8 & 0x3E);
        }
        if queue.is_empty() && rng.one_in(12) {
            let ch = rng.below(2) as u16 * 0x40;
            match rng.below(20) {
                0..=7 => {
                    let src = rng.pick(&srcs);
                    let dst = rng.pick(&dsts);
                    let trig = match rng.below(5) {
                        0..=2 => 0,
                        3 => rng.below(8) as u16,
                        _ => rng.below(64) as u16,
                    } | (rng.one_in(3) as u16) << 5;
                    for (off, v) in [
                        (SRC, src),
                        (DST, dst),
                        (SRC_ADDR, rng.next() as u16),
                        (DST_ADDR, rng.next() as u16),
                        (COUNT, rng.pick(&counts)),
                        (SRC_STEP, rng.pick(&steps)),
                        (DST_STEP, rng.pick(&steps)),
                        (FORMAT, rng.pick(&formats)),
                        (
                            MASK,
                            if rng.one_in(2) {
                                0xFFFF
                            } else {
                                rng.next() as u16
                            },
                        ),
                        (XOR, if rng.one_in(2) { 0 } else { rng.next() as u16 }),
                        (SHIFT, rng.below(32) as u16),
                        (TRIGGER, trig),
                        (CTRL, 1),
                    ] {
                        queue.push((ch + off, Some(v)));
                    }
                }
                8 | 9 => queue.push((ch + CTRL, Some(2))),
                10 => queue.push((ch + CTRL, Some(rng.below(4) as u16))),
                11 | 12 => queue.push((ch + STATUS, None)),
                13 => queue.push((ch + STATUS, Some(rng.next() as u16 & 0x1A))),
                14 => queue.push((ch + rng.pick(&[SRC_ADDR, DST_ADDR, COUNT]), None)),
                15 => queue.push((
                    ch + rng.pick(&[SRC_ADDR, DST_ADDR, COUNT]),
                    Some(rng.next() as u16),
                )),
                16 => queue.push((
                    ch + rng.pick(&[SRC, DST, FORMAT, TRIGGER, SRC_STEP, DST_STEP]),
                    Some(rng.next() as u16 & 0x3F),
                )),
                17 => queue.push((rng.pick(&[0x100u16, 0x102, 0x104]), Some(rng.next() as u16))),
                18 => queue.push((rng.pick(&[0x104u16, 0x0E, 0x20, 0x3E, 0x200, 0x07]), None)),
                _ => queue.push((ch + CTRL, Some(1))),
            }
        }
        if !queue.is_empty() && !rng.one_in(3) {
            let (off, w) = queue.remove(0);
            r.host(off, w);
        } else {
            r.step();
        }
    }
    r.finish()
}

#[test]
fn random_programs_match() {
    let mut total = Cover::default();
    for seed in 0..64 {
        total.add(&random_program(seed, 8000));
    }
    println!("{total}");
    for name in COVER {
        assert!(total.get(name) > 0, "{name} が起きない：{total}");
    }
    assert!(
        total.get("アクセス") > total.get("スロット") / 16,
        "{total}"
    );
}
