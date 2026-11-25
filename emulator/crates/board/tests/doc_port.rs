//! DOC 子基板の port 2（P2A と波形 RAM）の試験。
//! 期待値は仕様書 §06（port 2）、DR-035、DR-046 (9)、台帳 ES5503.md §3 と、P2A の模型（emulator.md §03）からの
//! 手計算で取る。

use fc85_board::fabric::{Decision, Fabric, HostReply, SlotRecord};
use fc85_board::ports::FabricPorts;
use fc85_core::time::{PERIOD_16M, PERIOD_DOC_E};

const SRC: u16 = 0x00;
const DST: u16 = 0x02;
const SRC_ADDR: u16 = 0x06;
const DST_ADDR: u16 = 0x0A;
const COUNT: u16 = 0x0C;
const SRC_STEP: u16 = 0x0E;
const DST_STEP: u16 = 0x10;
const FORMAT: u16 = 0x12;
const CTRL: u16 = 0x1C;
const DONE: u16 = 0x02;
const ERROR: u16 = 0x08;

struct Rig {
    f: Fabric,
    p: FabricPorts,
    trace: Vec<SlotRecord>,
}

impl Rig {
    fn new(phase: u64) -> Self {
        let mut p = FabricPorts::new();
        p.doc = true;
        p.doc_board.set_phase(0, phase);
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

    fn write(&mut self, off: u16, v: u16) {
        match self.f.host_write(off, v, &mut self.p) {
            HostReply::Ack => {}
            HostReply::Wait => loop {
                self.step();
                if let Some(r) = self.f.host_poll() {
                    assert_eq!(r, HostReply::Ack);
                    break;
                }
            },
            r => panic!("書込み {off:#x}: {r:?}"),
        }
    }

    /// port 2 のアクセスの長さ（スロット）。
    fn port2_lengths(&self) -> Vec<usize> {
        let mut v = Vec::new();
        let mut n = 0;
        for r in &self.trace {
            match r.access {
                Some(a) if a.port == 2 => n += 1,
                _ if n > 0 => {
                    v.push(n);
                    n = 0;
                }
                _ => {}
            }
            if r.access_done && n > 0 {
                v.push(n);
                n = 0;
            }
        }
        v
    }

    fn starts(&self) -> Vec<u64> {
        self.trace
            .iter()
            .filter(|r| matches!(r.decision, Decision::Start(_)))
            .map(|r| r.slot)
            .collect()
    }
}

fn channel(r: &mut Rig, regs: &[(u16, u16)]) {
    for &(off, v) in regs {
        r.write(off, v);
    }
    r.write(CTRL, 1);
}

/// Staging RAM → 波形 RAM の 0x1000 へ `n` byte。
fn staging_to_wave(r: &mut Rig, n: usize) {
    for i in 0..n {
        r.p.staging[i] = i as u8 ^ 0x5A;
    }
    channel(
        r,
        &[
            (SRC, 0),
            (DST, 2),
            (DST_ADDR, 0x1000),
            (COUNT, n as u16),
            (SRC_STEP, 1),
            (DST_STEP, 1),
            (FORMAT, 0),
        ],
    );
}

fn gaps(v: &[u64]) -> Vec<u64> {
    v.windows(2).map(|w| w[1] - w[0]).collect()
}

#[test]
fn fabric_writes_wave_ram_through_p2a() {
    // E の位相が 0：E の立下りは 4 スロットごとのスロット 4m の頭、同期化した立下りは 2 クロック
    // （16 MHz）後。書込みのアクセスは奇数のスロット w＝4m＋1 で始まり、ラッチの後の最初の立下りで窓を
    // 取り、その次の立下りで終わる：8 スロット。次の要素は FLAG の規則と S のスロットで 4 スロット後に
    // 始まり、1 byte が 12 スロット（3 µs）。位相が 4 クロック（250 ns）なら、ラッチの直後の立下りに
    // 間に合い、5 スロットと 8 スロット（2 µs）
    for (phase, len, gap) in [(0, 8, 12), (4 * PERIOD_16M, 5, 8)] {
        let mut r = Rig::new(phase);
        staging_to_wave(&mut r, 64);
        r.run(1000);
        assert_eq!(r.f.status(0), DONE);
        assert!((0..64).all(|i| r.p.doc_board.wave[0x1000 + i] == i as u8 ^ 0x5A));
        assert_eq!(r.p.doc_board.windows, 64);
        let lens = r.port2_lengths();
        // 1 byte 目は、設定の書込みの終わりで始まる位置が変わる
        assert!(lens[1..].iter().all(|&x| x == len), "{lens:?}");
        let g = gaps(&r.starts());
        assert!(g[2..].iter().all(|&x| x == gap), "{g:?}");
    }
}

#[test]
fn the_window_comes_after_the_address_and_lasts_half_a_cycle() {
    // 台帳 ES5503.md §3.4 の Tas（350 ns）が E のどちらの縁を基準にしても足りるよう、番地をラッチしてから
    // E が上がるまで 350 ns（5292 単位）以上あける。窓は E High の 500 ns（7560 単位）
    for k in 0..16 {
        let mut r = Rig::new(k * PERIOD_16M);
        staging_to_wave(&mut r, 8);
        let mut seen = 0;
        for _ in 0..200 {
            r.step();
            if r.p.doc_board.windows != seen {
                seen = r.p.doc_board.windows;
                let w = r.p.doc_board.last_window.unwrap();
                assert!(w.rise - w.latched >= 5292, "位相 {k}：{w:?}");
                assert_eq!(w.fall - w.rise, PERIOD_DOC_E / 2);
                assert!(w.write);
            }
        }
        assert_eq!((seen, r.f.status(0)), (8, DONE), "位相 {k}");
    }
}

#[test]
fn fabric_reads_wave_ram() {
    // 読出しは S のスロットを待たないので、位相 0 でも 4 クロックでも 1 byte が 8 スロット（2 µs）
    for phase in [0, 4 * PERIOD_16M] {
        let mut r = Rig::new(phase);
        for i in 0..32 {
            r.p.doc_board.wave[0x2000 + i] = (i as u8).wrapping_mul(7);
        }
        channel(
            &mut r,
            &[
                (SRC, 2),
                (DST, 6),
                (SRC_ADDR, 0x2000),
                (COUNT, 32),
                (SRC_STEP, 1),
                (FORMAT, 0),
            ],
        );
        r.run(400);
        assert_eq!(r.f.status(0), DONE);
        assert!((0..32).all(|i| r.p.temporal.mem[i] == (i as u8).wrapping_mul(7)));
        let g = gaps(&r.starts());
        assert!(g[2..].iter().all(|&x| x == 8), "{g:?}");
    }
}

#[test]
fn the_doc_plays_what_the_fabric_wrote() {
    let mut r = Rig::new(0);
    // 0x3000 からの 256 byte に 1〜255 の傾き（0 はオシレータを止めるので使わない。台帳 §4.4）
    for i in 0..256 {
        r.p.staging[i] = (i % 255) as u8 + 1;
    }
    channel(
        &mut r,
        &[
            (SRC, 0),
            (DST, 2),
            (DST_ADDR, 0x3000),
            (COUNT, 256),
            (SRC_STEP, 1),
            (DST_STEP, 1),
            (FORMAT, 0),
        ],
    );
    r.run(3500);
    assert_eq!(r.f.status(0), DONE);
    // オシレータ 0：テーブル 256 byte、分解能 0、ポインタ 0x30、フリーラン（台帳 §4.1〜§4.3）
    let doc = &mut r.p.doc_board.doc;
    for (reg, v) in [
        (0x00, 0x00),
        (0x20, 0x02),
        (0x40, 0xFF),
        (0x80, 0x30),
        (0xC0, 0x00),
        (0xA0, 0x00),
        (0xE1, 0x00),
    ] {
        doc.write(reg, v);
    }
    let mut reads = 0;
    let mut cycles = r.p.doc_board.cycles;
    for _ in 0..2000 {
        r.step();
        if r.p.doc_board.cycles != cycles {
            cycles = r.p.doc_board.cycles;
            if let Some(o) = r.p.doc_board.last_osc {
                assert_eq!(o.addr >> 8, 0x30);
                assert_eq!(o.wave, (o.addr & 0xFF) as u8 % 255 + 1, "{o:?}");
                reads += 1;
            }
        }
    }
    assert!(reads > 400);
}

#[test]
fn a_slow_doc_clock_times_out() {
    // DR-035 の外部クロック：E の周期が長いと、窓を待つ間に 32 クロック（16 スロット）を超え、ERROR で
    // 止まる（DR-046 (9)）。3 MHz では 2 byte の後に、2 MHz では 1 byte 目で止まる。打ち切った分は書かない。
    // 4 MHz なら 1 回のアクセスは最長 14 スロットで、時間切れにならない
    for (clk, done, written) in [(3780, DONE, 4), (5040, ERROR, 2), (7560, ERROR, 0)] {
        let mut r = Rig::new(0);
        r.p.doc_board.set_clock_period(clk);
        r.p.staging[..4].copy_from_slice(&[1, 2, 3, 4]);
        channel(
            &mut r,
            &[
                (SRC, 0),
                (DST, 2),
                (DST_ADDR, 0x100),
                (COUNT, 4),
                (SRC_STEP, 1),
                (DST_STEP, 1),
                (FORMAT, 0),
            ],
        );
        r.run(400);
        assert_eq!(r.f.status(0), done, "CLK {clk}");
        let w = &r.p.doc_board.wave[0x100..0x104];
        let want: Vec<u8> = (1..=4).map(|x| if x <= written { x } else { 0 }).collect();
        assert_eq!(w, &want[..], "CLK {clk}");
        assert!(r.port2_lengths().iter().all(|&x| x <= 16));
    }
}

#[test]
fn stop_cancels_a_waiting_port2_access() {
    let mut r = Rig::new(0);
    r.p.staging[0] = 0x77;
    channel(
        &mut r,
        &[
            (SRC, 0),
            (DST, 2),
            (DST_ADDR, 0x40),
            (COUNT, 1),
            (SRC_STEP, 1),
            (FORMAT, 0),
        ],
    );
    // port 2 のアクセスが始まるまで進め、窓を取る前に止める
    loop {
        let r2 = r.step();
        if r2.access.is_some_and(|a| a.port == 2) {
            break;
        }
    }
    r.write(CTRL, 2);
    r.run(40);
    assert_eq!(r.p.doc_board.wave[0x40], 0);
    assert_eq!(r.p.doc_board.windows, 0);
    // P2A は空いていて、次の転送は通る
    r.write(CTRL, 1);
    r.run(40);
    assert_eq!(r.p.doc_board.wave[0x40], 0x77);
}

#[test]
fn without_the_daughterboard_the_doc_does_not_run() {
    let mut r = Rig::new(0);
    r.p.doc = false;
    r.run(100);
    assert_eq!(r.p.doc_board.cycles, 0);
    // 子基板を挿すと、位相を保ったまま今から動く
    r.p.doc = true;
    r.run(8);
    assert_eq!(r.p.doc_board.cycles, 2);
}

#[test]
fn snapshot_round_trip_mid_port2_transfer() {
    let mut r = Rig::new(4 * PERIOD_16M);
    staging_to_wave(&mut r, 32);
    r.run(53);
    let saved = serde_json::to_string(&(&r.f, &r.p)).unwrap();
    let (f2, p2): (Fabric, FabricPorts) = serde_json::from_str(&saved).unwrap();
    let mut b = Rig {
        f: f2,
        p: p2,
        trace: Vec::new(),
    };
    for _ in 0..400 {
        assert_eq!(r.step(), b.step());
    }
    assert_eq!(r.p, b.p);
    assert_eq!(r.f.status(0), DONE);
}
