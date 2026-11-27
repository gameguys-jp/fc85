//! DOC 子基板の port 2（P2A と波形 RAM）の試験。
//! 期待値は仕様書 §06（port 2）、DR-035、DR-046 (9)、DR-050、台帳 ES5503.md §3 と、P2A の模型
//! （emulator.md §03）からの手計算で取る。

use fc85_board::fabric::{Decision, Fabric, HostReply, SlotRecord};
use fc85_board::ports::FabricPorts;
use fc85_core::time::{PERIOD_8M, PERIOD_16M, PERIOD_DOC_E, SLOT, Time};

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

    /// port 2 のアクセスを始めたスロット。
    fn port2_starts(&self) -> Vec<u64> {
        let mut v = Vec::new();
        let mut inside = false;
        for r in &self.trace {
            let p2 = r.access.is_some_and(|a| a.port == 2);
            if p2 && !inside {
                v.push(r.slot);
            }
            inside = p2 && !r.access_done;
        }
        v
    }

    /// port 2 の書込みのアクセスの長さが、手計算（`write_answers_by_hand`）の答えの縁から決まる長さ
    /// （答えたスロット＋（縁が q0 でなければ 1）。fabric.md §03、DR-091）と同じか。手計算の答えを返す。
    fn check_writes_by_hand(&self, clk: Time, phase: Time) -> Vec<(u64, u8)> {
        let by_hand = write_answers_by_hand(&self.port2_starts(), clk, phase);
        let want: Vec<usize> = by_hand
            .iter()
            .map(|&(k, q)| (k + u64::from(q > 0)).min(16) as usize)
            .collect();
        assert_eq!(self.port2_lengths(), want, "CLK {clk} 位相 {phase}");
        by_hand
    }
}

/// 同期化した E の立下り：E が `f` に下がると、74ALS74（SYN1）が 7 ns（106 単位）前の値を取る最初の 16 MHz の縁で
/// Low になり、その次の縁で P2A が SYN1 の Low と SYN2 の High を見る（emulator.md §03、DR-088）。
fn sync_fall(f: Time) -> Time {
    (f + 106).div_ceil(PERIOD_16M) * PERIOD_16M + PERIOD_16M
}

/// port 2 の書込みのアクセスを始めたスロット `starts` ごとに、P2A がラッチして答える 16 MHz の縁の手計算
/// （emulator.md §03 の「P2A の預かりと窓の取り方」）。返す値は（アクセスの何スロット目か、縁 q0〜q3）。E の
/// 立下りは `phase`＋j×8×`clk`。書込みは、スロットの頭の 20 ns（302 単位）後の最初の縁（q1）か、預かりが空く縁の
/// 遅い方でラッチし、その縁で答える。預かったものは、ラッチの縁かその後の最初の同期化した E の立下りで次の E High
/// の窓を取り、その次の同期化した立下りで預かりを空ける。
fn write_answers_by_hand(starts: &[u64], clk: Time, phase: Time) -> Vec<(u64, u8)> {
    let te = 8 * clk;
    let mut free = 0;
    starts
        .iter()
        .map(|&w| {
            let t0 = w * SLOT;
            let latch = (t0 + PERIOD_16M).max(free);
            let mut f = phase;
            while sync_fall(f) < latch {
                f += te;
            }
            free = sync_fall(f + te);
            let d = latch - t0;
            (d / SLOT + 1, (d % SLOT / PERIOD_16M) as u8)
        })
        .collect()
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
    // P2A は書込みを預かってすぐ答える（DR-050）。ラッチと答えはスロットの頭の 20 ns 後の縁（q1）なので、
    // 答えは次のスロットに数え、port 2 は 2 スロット（DR-091）。Fabric の要素は、START、port 0 の S のスロットの
    // 読出し、port 2 の 2 スロット、延びるアクセスの後の空き（満杯・空の信号の規則）、読出しが S のスロットに
    // 来るまでの 1 スロットで 6 スロット（1.5 µs、0.67 MB/s）。DOC のサイクル（4 スロット）より長い。預かりが
    // 空くのは、窓の後の同期化した E の立下り。E の位相が 4 クロック（250 ns）なら、次の書込みはいつも
    // その縁より後に来て 2 スロット。位相 0 では 1 回おきに 2 スロット目の q2 まで待って 3 スロットになるが、
    // その分は S のスロットを待つ 1 スロットに入り、要素は 6 スロットのまま。どの書込みの答えの縁も手計算
    // （`write_answers_by_hand`）と同じ
    for (phase, lens_cycle) in [(4 * PERIOD_16M, [2, 2]), (0, [3, 2])] {
        let mut r = Rig::new(phase);
        staging_to_wave(&mut r, 64);
        r.run(1000);
        assert_eq!(r.f.status(0), DONE);
        assert!((0..64).all(|i| r.p.doc_board.wave[0x1000 + i] == i as u8 ^ 0x5A));
        assert_eq!(r.p.doc_board.windows, 64);
        r.check_writes_by_hand(PERIOD_8M, phase);
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
        assert!(g.iter().all(|&x| x == 6), "{g:?}");
    }
}

#[test]
fn posted_writes_take_six_slots_in_all_16_phases() {
    // E の位相 16 通り（16 MHz の 1 クロックずつ）で 64 byte を書く。最初の要素から最後の要素までは、どの位相も
    // 378 スロット（1 byte 6 スロット）。DR-091 の前は、port 2 が 1 スロットで要素が 4 スロットになり、252〜254
    // スロット（1 byte 4 スロット）の位相が 6 通り、374〜376 スロット（4 と 8 の交互）が 10 通りだった
    for k in 0..16 {
        let mut r = Rig::new(k * PERIOD_16M);
        staging_to_wave(&mut r, 64);
        r.run(1000);
        assert_eq!(r.f.status(0), DONE, "位相 {k}");
        r.check_writes_by_hand(PERIOD_8M, k * PERIOD_16M);
        let s = r.starts();
        assert_eq!(s[63] - s[0], 63 * 6, "位相 {k}");
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
    // 答えが 16 スロット目の q0 より遅いと（q1〜q3 の答えは 17 スロット目に数える。DR-091）、32 クロック
    // （16 スロット）を超えて ERROR で止まる（DR-046 (9)）。その後は窓の直後にラッチしてすぐ答えるので、
    // 待ちは E の 1 周期より短い。E の位相（16 MHz の 1 クロックずつ）をすべて試し、終わる位相と止まる位相を
    // 数える。2 byte 目の答えの縁は手計算（`write_answers_by_hand`）と同じ。止まったときも、預かった 1 byte 目は
    // 書く。2 byte 目を 16 スロット目の q1〜q3 にラッチした位相（3 MHz の 1 通り、2 MHz の 3 通り）では、
    // Fabric は数えないが、P2A が預かったので窓で書く
    //
    // DR-091 の前は 3 MHz が 36 通りと 6 通り、2 MHz が 15 通りと 49 通り。要素の間隔が 4 から 6 スロットに
    // 延びて、2 byte 目の待ちが 2 スロット短くなった（16 スロット目の q1〜q3 を失う分より大きい）
    for (clk, done, error) in [
        (3780, 32, 0),
        (5040, 41, 1),
        (7560, 20, 44),
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
            let h = r.check_writes_by_hand(clk, k * PERIOD_16M);
            let (k2, q2) = h[1];
            let timed_out = k2 + u64::from(q2 > 0) > 16;
            match r.f.status(0) {
                DONE => {
                    assert!(!timed_out, "CLK {clk} 位相 {k}");
                    assert_eq!(written, 16);
                    n.0 += 1;
                }
                ERROR => {
                    assert!(timed_out, "CLK {clk} 位相 {k}");
                    let latched = k2 == 16;
                    assert_eq!(written, 1 + usize::from(latched), "CLK {clk} 位相 {k}");
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
fn stop_after_the_window_is_claimed_discards_the_read() {
    // P2A の式（DR-089）：窓を取った（S1・S0＝11）後の読出しを打ち切ると、E が上がる前でも「捨てる」段
    // （10）に入り、窓を 1 つ使ってから空く。答えは Fabric に届かない。P2A は E の立上りを知らない
    // （DR-110、OI-089 (1)）。E の位相を 16 MHz の 1 周期ずつ変えて、窓を取った直後のスロットで止める
    let regs = [
        (SRC, 2),
        (DST, 6),
        (SRC_ADDR, 0x2000),
        (COUNT, 1),
        (FORMAT, 0),
    ];
    for k in 0..PERIOD_DOC_E / PERIOD_16M {
        let mut r = Rig::new(k * PERIOD_16M);
        r.p.doc_board.wave[0x2000] = 0x33;
        r.p.temporal.mem[0] = 0xEE;
        channel(&mut r, &regs);
        while r.p.doc_board.p2a_state() != 0b11 {
            r.step();
        }
        assert_eq!(r.p.doc_board.windows, 0);
        r.write(CTRL, 2);
        r.run(40);
        assert_eq!(r.p.temporal.mem[0], 0xEE, "位相 {k}");
        assert_eq!(r.p.doc_board.windows, 1, "位相 {k}：取った窓は使う");
        assert_eq!(r.p.doc_board.p2a_state(), 0b00);
        // P2A は空いていて、次の転送は通る
        channel(&mut r, &regs);
        r.run(40);
        assert_eq!(r.p.temporal.mem[0], 0x33, "位相 {k}");
        assert_eq!(r.p.doc_board.windows, 2);
    }
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
