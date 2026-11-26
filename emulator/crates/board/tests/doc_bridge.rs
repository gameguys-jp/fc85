//! 68000 から DOC のレジスタへのブリッジ（段階 3）の試験。
//! 期待値は仕様書 §05、DR-016、DR-019、DR-035、台帳 HD68000.md §2.2、ES5503.md §2・§3 と、ブリッジの
//! 模型（emulator.md §03）からの手計算で取る。

use fc85_board::console::BERR_HALF_CLOCKS;
use fc85_board::doc::{HostDone, host_register};
use fc85_board::fabric::{Decision, Fabric, HostReply};
use fc85_board::ports::FabricPorts;
use fc85_core::time::{PERIOD_8M, PERIOD_16M, PERIOD_DOC_E, Time};

const SRC: u16 = 0x00;
const DST: u16 = 0x02;
const DST_ADDR: u16 = 0x0A;
const COUNT: u16 = 0x0C;
const SRC_STEP: u16 = 0x0E;
const DST_STEP: u16 = 0x10;
const FORMAT: u16 = 0x12;
const CTRL: u16 = 0x1C;
const DONE: u16 = 0x02;

/// reg n の番地。
fn reg(n: u8) -> u32 {
    0x21_0000 + 2 * u32::from(n)
}

/// S0 から S4 の立上り（CS の登録）まで。
const TO_S4: Time = 2 * PERIOD_8M;
/// バスの時間切れ（DR-052）：AS↓ の後の立上りを 64 数えて BERR を出し、S0 から 65.5 クロックの立下りで
/// 認識する。同じ立下りで DTACK を認識してもバスエラー。
const BUS_TIMEOUT: Time = BERR_HALF_CLOCKS * PERIOD_8M / 2;

struct Rig {
    f: Fabric,
    p: FabricPorts,
    starts: Vec<u64>,
}

impl Rig {
    fn new(phase: Time) -> Self {
        let mut p = FabricPorts::new();
        p.doc = true;
        p.doc_board.set_phase(0, phase);
        Self {
            f: Fabric::new(),
            p,
            starts: Vec::new(),
        }
    }

    fn step(&mut self) {
        let r = self.f.step_slot(&mut self.p);
        if matches!(r.decision, Decision::Start(_)) {
            self.starts.push(r.slot);
        }
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

    /// 68000 のバスサイクルを始める。S0 は次に進めるスロットの頭。
    fn begin(&mut self, addr: u32, write: Option<u8>) -> Time {
        let s0 = self.p.now();
        let w = write.map(|v| u16::from(v) << 8);
        assert_eq!(
            self.p.doc_host_begin(s0, addr, true, false, w),
            HostReply::Wait
        );
        s0
    }

    /// 68000 のバスサイクルを 1 回。DTACK まで進め、サイクルの終わり（S7 の終わり）まで進める。
    fn cpu(&mut self, addr: u32, write: Option<u8>) -> HostDone {
        self.begin(addr, write);
        let d = loop {
            self.step();
            if let Some(d) = self.p.doc_host_poll() {
                break d;
            }
        };
        while self.p.now() < d.recognized + 3 * PERIOD_8M / 2 {
            self.step();
        }
        d
    }
}

/// E の立下りの位相 `phase` で、S0 が `s0` のアクセスの待ちの数（手計算）。
/// 同期化した E の立下りは E の立下りの 2 クロック（16 MHz）後。S4 の立上り L 以降の最初のもの S で窓を
/// 取り、その窓の後の同期化した立下り（S＋E の周期）で DTACK を出す。S−L を 16 MHz の j クロックとすると、
/// DTACK が 68000 のクロックの立上りに来れば次の立下り、立下りに来ればその次の立下りで認識する。
fn expected_waits(phase: Time, period: Time, s0: Time) -> u32 {
    let l = s0 + TO_S4;
    let mut s = phase + 2 * PERIOD_16M;
    while s < l {
        s += period;
    }
    let dtack = s + period;
    let half = PERIOD_8M / 2;
    let recognized = if dtack.is_multiple_of(PERIOD_8M) {
        dtack + half
    } else {
        dtack + PERIOD_8M
    };
    ((recognized - (s0 + TO_S4 + half)) / PERIOD_8M) as u32
}

#[test]
fn the_window_decodes_upper_byte_accesses_only() {
    assert_eq!(host_register(0x21_0000, true, false), Some(0x00));
    assert_eq!(host_register(0x21_01C2, true, false), Some(0xE1));
    assert_eq!(host_register(0x21_01FE, true, false), Some(0xFF));
    // 下位バイトレーン、ワード、奇数番地、窓の外
    assert_eq!(host_register(0x21_0001, false, true), None);
    assert_eq!(host_register(0x21_0000, true, true), None);
    assert_eq!(host_register(0x21_0001, true, false), None);
    assert_eq!(host_register(0x21_0200, true, false), None);
    assert_eq!(host_register(0x20_FFFE, true, false), None);

    let mut r = Rig::new(0);
    assert_eq!(
        r.p.doc_host_begin(0, 0x21_0001, false, true, None),
        HostReply::BusError
    );
    assert_eq!(
        r.p.doc_host_begin(0, 0x21_0000, true, true, Some(0x1234)),
        HostReply::BusError
    );
    // 子基板がない（DR-019）
    r.p.doc = false;
    assert_eq!(
        r.p.doc_host_begin(0, 0x21_0000, true, false, None),
        HostReply::BusError
    );
    assert!(!r.p.doc_board.host_busy());
}

#[test]
fn the_cpu_writes_and_reads_doc_registers() {
    let mut r = Rig::new(0);
    let w = r.cpu(reg(0x41), Some(0xA5));
    assert_eq!((w.reg, w.write, w.read), (0x41, Some(0xA5), None));
    assert_eq!(r.p.doc_board.doc.read(0x41), 0xA5);
    let d = r.cpu(reg(0x41), None);
    assert_eq!(d.read, Some(0xA5));
    assert_eq!(r.p.doc_board.host_windows, 2);
    assert_eq!(r.p.doc_board.windows, 0);
}

#[test]
fn waits_follow_the_e_phase() {
    // E の位相 16 通り。待ちは 8＋⌈j/2⌉（j＝0〜15）で 8〜16、バスサイクルは 12〜20 クロック
    // （1.5〜2.5 µs）。番地（S4 で登録）から E の立上りまでは 375 ns 以上、窓は 500 ns
    let mut seen = [0; 17];
    for k in 0..16 {
        let mut r = Rig::new(k * PERIOD_16M);
        r.run(10);
        let s0 = r.p.now();
        let d = r.cpu(reg(0x20), Some(k as u8));
        assert_eq!(d.s0, s0);
        assert_eq!(
            d.waits,
            expected_waits(k * PERIOD_16M, PERIOD_DOC_E, s0),
            "位相 {k}"
        );
        assert!(d.rise - (s0 + TO_S4) >= 5670, "位相 {k}：{d:?}");
        assert_eq!(d.fall - d.rise, PERIOD_DOC_E / 2);
        seen[d.waits as usize] += 1;
    }
    assert_eq!(seen[8..], [1, 2, 2, 2, 2, 2, 2, 2, 1]);
}

#[test]
fn the_cpu_goes_before_p2a_on_the_same_edge() {
    // E の位相 4 クロックで、Fabric が Staging RAM から波形 RAM へ 64 byte を書く（1 byte 4 スロットで
    // 窓を毎回使う。DR-050）。途中で 68000 が DOC に書くと、68000 の待ちは Fabric がないときと同じで、
    // P2A は窓を 1 つ譲る：最初から最後の要素までが 4 スロット延びる
    let phase = 4 * PERIOD_16M;
    let mut spans = Vec::new();
    for with_cpu in [false, true] {
        let mut r = Rig::new(phase);
        for i in 0..64 {
            r.p.staging[i] = i as u8 ^ 0x5A;
        }
        for (off, v) in [
            (SRC, 0),
            (DST, 2),
            (DST_ADDR, 0x1000),
            (COUNT, 64),
            (SRC_STEP, 1),
            (DST_STEP, 1),
            (FORMAT, 0),
        ] {
            r.write(off, v);
        }
        r.write(CTRL, 1);
        r.run(100);
        if with_cpu {
            let s0 = r.p.now();
            let d = r.cpu(reg(0x40), Some(0x77));
            assert_eq!(d.waits, expected_waits(phase, PERIOD_DOC_E, s0));
            assert_eq!(r.p.doc_board.doc.read(0x40), 0x77);
        }
        r.run(400);
        assert_eq!(r.f.status(0), DONE);
        assert!((0..64).all(|i| r.p.doc_board.wave[0x1000 + i] == i as u8 ^ 0x5A));
        assert_eq!(r.p.doc_board.windows, 64);
        spans.push(r.starts[63] - r.starts[0]);
    }
    assert_eq!(spans[1], spans[0] + 4, "{spans:?}");
}

#[test]
fn the_cpu_starts_an_oscillator_through_the_bridge() {
    // 波形 RAM 0x3000〜0x30FF に 1〜255 の傾きを置き、オシレータ 0 を 68000 から設定して走らせる
    // （台帳 §4.1〜§4.3。0x00 は止まるので使わない。§4.4）
    let mut r = Rig::new(0);
    for i in 0..256 {
        r.p.doc_board.wave[0x3000 + i] = (i % 255) as u8 + 1;
    }
    for (n, v) in [
        (0x00, 0x00),
        (0x20, 0x02),
        (0x40, 0xFF),
        (0x80, 0x30),
        (0xC0, 0x00),
        (0xA0, 0x00),
        (0xE1, 0x00),
    ] {
        r.cpu(reg(n), Some(v));
    }
    let mut reads = 0;
    let mut cycles = r.p.doc_board.cycles;
    for _ in 0..800 {
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
    assert!(reads > 150);
}

#[test]
fn a_slow_doc_clock_can_run_past_the_bus_timeout() {
    // DR-035 の外部クロック。DOC の CLK が 2 MHz（E は 64 クロック＝16 スロット）なら待ちは 32＋⌈j/2⌉
    // （j＝0〜63）。DR-052 の時間切れ（S0 から 65.5 クロックの立下り）は待ち 63 以上で、j＝61〜63 の
    // 3 通り。窓はそれより前に済むので、書込みは DOC に届く。1 MHz（E は 32 スロット）ではすべての位相で
    // 時間切れになる。打ち切りは BERR の後のスロットの頭（S0 から 66 クロック）で、それより前に窓が始まる
    // 位相（128 通りのうち 66 通り）だけ書込みが届く
    for (clk, errors, landed) in [(7560, 3, 64), (15120, 128, 66)] {
        let period = 8 * clk;
        let (mut n_err, mut n_landed) = (0, 0);
        for k in 0..period / PERIOD_16M {
            let mut r = Rig::new(0);
            r.p.doc_board.set_clock_period(clk);
            r.p.doc_board.set_phase(0, k * PERIOD_16M);
            r.p.doc_board.doc.write(0x41, 0);
            r.run(10);
            let s0 = r.begin(reg(0x41), Some(0x99));
            let mut ok = false;
            while r.p.now() <= s0 + BUS_TIMEOUT {
                r.step();
                if let Some(d) = r.p.doc_host_poll() {
                    assert_eq!(d.waits, expected_waits(k * PERIOD_16M, period, s0));
                    ok = d.recognized < s0 + BUS_TIMEOUT;
                    break;
                }
            }
            if !ok {
                r.p.doc_board.host_abort();
                n_err += 1;
            }
            r.run(70);
            assert!(!r.p.doc_board.host_busy());
            assert!(r.p.doc_host_poll().is_none());
            if r.p.doc_board.doc.read(0x41) == 0x99 {
                n_landed += 1;
            }
        }
        assert_eq!((n_err, n_landed), (errors, landed), "CLK {clk}");
    }
}

#[test]
fn snapshot_round_trip_with_a_cpu_access_in_flight() {
    let mut r = Rig::new(7 * PERIOD_16M);
    r.run(5);
    r.begin(reg(0x10), Some(0x3C));
    r.run(3);
    let saved = serde_json::to_string(&(&r.f, &r.p)).unwrap();
    let (f2, p2): (Fabric, FabricPorts) = serde_json::from_str(&saved).unwrap();
    let mut b = Rig {
        f: f2,
        p: p2,
        starts: Vec::new(),
    };
    let mut done = (None, None);
    for _ in 0..20 {
        r.step();
        b.step();
        done.0 = done.0.or(r.p.doc_host_poll());
        done.1 = done.1.or(b.p.doc_host_poll());
    }
    assert!(done.0.is_some());
    assert_eq!(done.0, done.1);
    assert_eq!(r.p, b.p);
    assert_eq!(r.p.doc_board.doc.read(0x10), 0x3C);
}
