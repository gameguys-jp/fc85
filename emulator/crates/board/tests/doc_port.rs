//! DOC 子基板の port 2（P2A と波形 RAM）の試験。
//! 期待値は仕様書 §06（port 2）、DR-035、DR-046 (9)、DR-050、台帳 ES5503.md §3 と、P2A の模型
//! （emulator.md §03）からの手計算で取る。

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
    // P2A は書込みを預かってすぐ答える（DR-050）。Fabric の要素は、port 0 の S のスロットで読み、
    // port 2 に 1 スロットで書き、次のスロットの頭で次を決めるので 4 スロット。DOC のサイクルも 4 スロット。
    // 預かりが空くのは、窓の後の同期化した E の立下り。E の位相が 4 クロック（250 ns）なら、次の書込みが
    // その縁より前に来て、1 byte が 4 スロット（1 µs、1.0 MB/s）。位相 0 では縁の後に来るので、1 回おきに
    // 1 サイクル待ち（4 スロット）、4 スロットと 8 スロットが交互（平均 6 スロット、0.67 MB/s）
    for (phase, lens_cycle, gap_cycle) in [(4 * PERIOD_16M, [1, 1], [4, 4]), (0, [1, 4], [4, 8])] {
        let mut r = Rig::new(phase);
        staging_to_wave(&mut r, 64);
        r.run(1000);
        assert_eq!(r.f.status(0), DONE);
        assert!((0..64).all(|i| r.p.doc_board.wave[0x1000 + i] == i as u8 ^ 0x5A));
        assert_eq!(r.p.doc_board.windows, 64);
        // 1・2 byte 目は、設定の書込みの終わりで始まる位置が変わる
        let lens = r.port2_lengths();
        assert!(
            lens[2..]
                .iter()
                .enumerate()
                .all(|(i, &x)| x == lens_cycle[i % 2]),
            "{lens:?}"
        );
        let g = gaps(&r.starts());
        assert!(
            g[2..]
                .iter()
                .enumerate()
                .all(|(i, &x)| x == gap_cycle[i % 2]),
            "{g:?}"
        );
    }
}

#[test]
fn posted_writes_keep_pace_with_the_doc_in_6_of_16_phases() {
    // E の位相 16 通り（16 MHz の 1 クロックずつ）で 64 byte を書く。最初の要素から最後の要素までが
    // 252〜254 スロット（1 byte 4 スロット）の位相が 6 通り、374〜376 スロット（4 と 8 の交互）が 10 通り
    let mut fast = 0;
    let mut slow = 0;
    for k in 0..16 {
        let mut r = Rig::new(k * PERIOD_16M);
        staging_to_wave(&mut r, 64);
        r.run(1000);
        assert_eq!(r.f.status(0), DONE, "位相 {k}");
        let s = r.starts();
        match s[63] - s[0] {
            252..=254 => fast += 1,
            374..=376 => slow += 1,
            x => panic!("位相 {k}：{x}"),
        }
    }
    assert_eq!((fast, slow), (6, 10));
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
            // リセットの後の走査は 22 番から 31 番を回ってから 0 番に来る（台帳 ES5503 追補 2026-11-26）
            if let Some(o) = r.p.doc_board.last_osc.filter(|o| o.n == 0) {
                assert_eq!(o.addr >> 8, 0x30);
                assert_eq!(o.wave, (o.addr & 0xFF) as u8 % 255 + 1, "{o:?}");
                reads += 1;
            }
        }
    }
    assert!(reads > 400);
}

#[test]
fn a_slow_doc_clock_can_time_out_on_the_second_write() {
    // DR-035 の外部クロック：E の周期が長いと、2 byte 目は、預かった 1 byte 目の窓が終わるまで待つ。
    // 待ちは「1 byte 目のラッチから次の同期化した E の立下りまで」＋「E の 1 周期」−「要素の間隔」で、
    // 32 クロック（16 スロット）を超えると ERROR で止まる（DR-046 (9)）。その後は窓の直後にラッチして
    // すぐ答えるので、待ちは E の 1 周期より短い。E の位相（16 MHz の 1 クロックずつ）をすべて試し、
    // 終わる位相と止まる位相を数える。止まったときも、預かった 1 byte 目は書く
    for (clk, done, error) in [
        (3780, 32, 0),
        (5040, 36, 6),
        (7560, 15, 49),
        (15120, 0, 128),
    ] {
        let mut n = (0, 0);
        for k in 0..8 * clk / PERIOD_16M {
            let mut r = Rig::new(0);
            r.p.doc_board.set_clock_period(clk);
            r.p.doc_board.set_phase(0, k * PERIOD_16M);
            staging_to_wave(&mut r, 16);
            r.run(1500);
            let written = (0..16)
                .filter(|&i| r.p.doc_board.wave[0x1000 + i] == i as u8 ^ 0x5A)
                .count();
            match r.f.status(0) {
                DONE => {
                    assert_eq!(written, 16);
                    n.0 += 1;
                }
                ERROR => {
                    assert_eq!(written, 1, "CLK {clk} 位相 {k}");
                    n.1 += 1;
                }
                x => panic!("CLK {clk} 位相 {k}：{x:#x}"),
            }
            assert!(r.port2_lengths().iter().all(|&x| x <= 16));
            assert!(!r.p.doc_board.busy());
        }
        assert_eq!(n, (done, error), "CLK {clk}");
    }
}

#[test]
fn stop_cancels_a_waiting_read() {
    let mut r = Rig::new(0);
    r.p.doc_board.wave[0x2000] = 0x33;
    r.p.temporal.mem[0] = 0xEE;
    let regs = [
        (SRC, 2),
        (DST, 6),
        (SRC_ADDR, 0x2000),
        (COUNT, 1),
        (FORMAT, 0),
    ];
    channel(&mut r, &regs);
    // port 2 の読出しが始まったら、窓を取る前に止める
    while r.step().access.is_none_or(|a| a.port != 2) {}
    r.write(CTRL, 2);
    r.run(40);
    assert_eq!(r.p.temporal.mem[0], 0xEE);
    assert_eq!(r.p.doc_board.windows, 0);
    assert!(!r.p.doc_board.busy());
    // P2A は空いていて、次の転送は通る
    channel(&mut r, &regs);
    r.run(40);
    assert_eq!(r.p.temporal.mem[0], 0x33);
}

#[test]
fn stop_keeps_a_posted_write_and_cancels_the_next() {
    // DOC の CLK が 2 MHz なら、2 byte 目は 1 byte 目の窓を長く待つ。そこで止めると、2 byte 目は
    // 取り消し、預かった 1 byte 目は窓で書く
    let mut r = Rig::new(0);
    r.p.doc_board.set_clock_period(7560);
    staging_to_wave(&mut r, 4);
    let mut n = 0;
    while n < 2 {
        if r.step().access.is_some_and(|a| a.port == 2) {
            n += 1;
        }
    }
    assert!(r.p.doc_board.busy());
    assert_eq!(r.p.doc_board.windows, 0);
    r.write(CTRL, 2);
    r.run(100);
    assert_eq!(&r.p.doc_board.wave[0x1000..0x1004], &[0x5A, 0, 0, 0]);
    assert_eq!(r.p.doc_board.windows, 1);
    assert!(!r.p.doc_board.busy());
}

#[test]
fn a_read_waits_for_the_posted_write_before_it() {
    // 波形 RAM の中で 1 byte ずらして写す。読出しは預かった書込みの窓の後になるので、各要素は直前に
    // 書いた値を読み、先頭の値が広がる
    let mut r = Rig::new(0);
    r.p.doc_board.wave[0x500..0x505].copy_from_slice(&[9, 1, 2, 3, 4]);
    channel(
        &mut r,
        &[
            (SRC, 2),
            (DST, 2),
            (SRC_ADDR, 0x500),
            (DST_ADDR, 0x501),
            (COUNT, 4),
            (SRC_STEP, 1),
            (DST_STEP, 1),
            (FORMAT, 0),
        ],
    );
    r.run(200);
    assert_eq!(r.f.status(0), DONE);
    assert_eq!(&r.p.doc_board.wave[0x500..0x505], &[9; 5]);
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
