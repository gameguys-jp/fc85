//! DOC 子基板のベンド点の試験：バスのテストピン（仕様書 §11「DOC波形RAMの線」、DR-034 (1)）、
//! DOC のレジスタの保持（台帳 ES5503.md §3.1・§4.7、E2 p.15）、波形 RAM の保持（DR-072）。
//! 期待値は台帳 §2（ピン）・§3.2（走査）と、子基板のバスの模型（emulator.md §03）と、保持時間の分布
//! （dram.rs、es5503.rs の REG_*）からの手計算で取る。

use fc85_board::bend::Wire;
use fc85_board::console::{BERR_CYCLE_CLOCKS, System};
use fc85_board::doc::{DocBoard, DocLines, HostDone};
use fc85_board::dram::DramEnv;
use fc85_board::fabric::{Fabric, HostReply};
use fc85_board::ports::FabricPorts;
use fc85_chips::hd68000::{Bus, BusReply, Size, fc};
use fc85_core::time::{SLOT, Time, UNITS_PER_SECOND};

fn lines(spec: &str) -> DocLines {
    let mut l = DocLines::default();
    l.parse(spec).unwrap();
    l
}

/// Bend モードの DocBoard（時刻 0 で波形 RAM の追跡を始める）。
fn bent_board(spec: &str, env: DramEnv) -> DocBoard {
    let mut b = DocBoard::new();
    b.set_bends(Some((&lines(spec), env)), 0);
    b
}

fn run(b: &mut DocBoard, t: &mut Time, slots: u64) {
    for _ in 0..slots {
        b.run_slot(*t, None);
        *t += SLOT;
    }
}

/// オシレータ `n` を、頁 `ptr` の 256 byte の表を 1 標本 1 byte で自走させる（台帳 §4.1〜§4.3）。
fn play(b: &mut DocBoard, n: u8, ptr: u8) {
    for (reg, v) in [
        (0x00, 0x00),
        (0x20, 0x02),
        (0x40, 0xFF),
        (0x80, ptr),
        (0xC0, 0x00),
        (0xA0, 0x00),
    ] {
        b.doc.write(reg + n, v);
    }
}

/// 68000 のブリッジで 1 回アクセスする（DTACK まで）。
fn cpu(b: &mut DocBoard, t: &mut Time, reg: u8, write: Option<u8>) -> HostDone {
    b.host_begin(*t, reg, write);
    loop {
        b.run_slot(*t, None);
        *t += SLOT;
        if let Some(d) = b.host_poll() {
            run(b, t, 4);
            return d;
        }
    }
}

#[test]
fn docbus_names_the_two_buses() {
    // A0〜A7 は A のバス、A8〜A15 と D0〜D7 は AD のバス（A8 と D0 は同じピン 30。台帳 §2）
    let l = lines("A0=0, A3=A4, A4=A3, A9=1, D7=Z, A8=D1");
    assert_eq!(l.a[0], Wire::Low);
    assert_eq!((l.a[3], l.a[4]), (Wire::Line(4), Wire::Line(3)));
    assert_eq!(l.ad[1], Wire::High);
    assert_eq!(l.ad[7], Wire::Float);
    assert_eq!(l.ad[0], Wire::Line(1));
    assert!(!l.is_normal());
    let mut back = l.clone();
    back.parse("normal").unwrap();
    assert!(back.is_normal());
    for bad in ["A3=D3", "D0=A1", "A16=0", "D8=1", "B1=0", "A1", "A1=Q"] {
        assert!(DocLines::default().parse(bad).is_err(), "{bad}");
    }
    // Bend の命令 `docbus`
    let mut s = System::new();
    s.bend("docbus A0=0").unwrap();
    assert!(!s.bends.is_factory());
    assert!(s.bends.summary().contains("DOC"));
    assert!(
        s.ports.doc_board.lines().is_none(),
        "通常モードでは回路に入れない"
    );
    s.bend("mode bend").unwrap();
    assert_eq!(s.ports.doc_board.lines().unwrap().a[0], Wire::Low);
    s.bend("docbus normal").unwrap();
    assert!(s.bends.is_factory());
    assert!(s.ports.doc_board.lines().is_none());
    assert!(s.bend("docbus A3=D1").is_err());
}

#[test]
fn the_doc_reads_through_the_bent_lines() {
    // E Low：DOC は A のバスに列、AD のバスに行を出し、波形 RAM のデータを AD のバスで受ける。
    // A0 を GND、A1 と A2 を入れ替え、A9/D1 を +5 V にすると、頁 0x30 の列 c を読むはずが、
    // 頁 0x32 の列 c' を読み、受ける値の bit 1 が 1 になる
    let mut b = bent_board("A0=0, A1=A2, A2=A1, A9=1", DramEnv::default());
    for (i, w) in b.wave.iter_mut().enumerate() {
        *w = (i ^ (i >> 8) ^ 0x5A) as u8 | 1;
    }
    play(&mut b, 0, 0x30);
    b.doc.write(0xE1, 0);
    let mut t = 0;
    let mut seen = std::collections::BTreeSet::new();
    let mut cycles = b.cycles;
    for _ in 0..4000 {
        run(&mut b, &mut t, 1);
        if b.cycles == cycles {
            continue;
        }
        cycles = b.cycles;
        let o = b.last_osc.unwrap();
        assert_eq!(o.addr >> 8, 0x30, "DOC の中の番地は頁 0x30");
        let c = o.addr as usize & 0xFE;
        let c = (c & !6) | ((c >> 1) & 2) | ((c << 1) & 4);
        let want = b.wave[0x3200 | c] | 2;
        assert_eq!(o.wave, want, "{o:?}");
        seen.insert(c);
    }
    assert_eq!(seen.len(), 128, "偶数の列だけを読む");
}

#[test]
fn floating_lines_are_noise_that_resumes_after_a_snapshot() {
    // 浮かせた線は、通るたびに LFSR の値（DR-063 と同じ）[P]
    let mut b = bent_board("D3=Z", DramEnv::default());
    b.wave.fill(0x01);
    play(&mut b, 0, 0x00);
    b.doc.write(0xE1, 0);
    let mut t = 0;
    run(&mut b, &mut t, 40);
    let mut copy: DocBoard = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
    let mut t2 = t;
    let mut a = Vec::new();
    let mut c = Vec::new();
    for _ in 0..400 {
        run(&mut b, &mut t, 1);
        run(&mut copy, &mut t2, 1);
        a.push(b.last_osc.unwrap().wave);
        c.push(copy.last_osc.unwrap().wave);
    }
    assert_eq!(a, c);
    assert!(a.contains(&0x01) && a.contains(&0x09));
    assert!(a.iter().all(|&v| v == 0x01 || v == 0x09));
    assert_eq!(b, copy);
}

#[test]
fn the_bridge_sees_the_register_number_and_data_through_the_lines() {
    // E High の 68000 のアクセス：レジスタの番号は A のバス、データは AD のバスを通る。
    // A7 を GND にすると 0x80〜0xFF の番号は 0x00〜0x7F になり、D1 を +5 V にすると値の bit 1 が 1
    let mut b = bent_board("A7=0, D1=1", DramEnv::default());
    let mut t = 0;
    // 0xC0（オシレータ 0 の SIZE）に 0x04 を書くと、0x40（音量）に 0x06 が入る
    cpu(&mut b, &mut t, 0xC0, Some(0x04));
    assert_eq!(b.doc.volume(0), 0x06);
    // 読出しも同じ線を通る：0xC1 を読むと 0x41（音量）の 0x10 が 0x12 で返る
    b.doc.write(0x41, 0x10);
    assert_eq!(cpu(&mut b, &mut t, 0xC1, None).read, Some(0x12));
    // 0xE0（OIR）は 0x60（波形データ。電源投入時は 0xFF）になり、割込みを受け取れない
    assert_eq!(cpu(&mut b, &mut t, 0xE0, None).read, Some(0xFF));
    // 線を戻すと元どおり
    b.set_bends(Some((&DocLines::default(), DramEnv::default())), t);
    cpu(&mut b, &mut t, 0x40, Some(0x04));
    assert_eq!(b.doc.volume(0), 0x04);
}

// ---- P2A（Data Fabric の port 2） ----

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

struct Rig {
    f: Fabric,
    p: FabricPorts,
}

impl Rig {
    fn new() -> Self {
        let mut p = FabricPorts::new();
        p.doc = true;
        Self {
            f: Fabric::new(),
            p,
        }
    }

    fn run(&mut self, n: usize) {
        for _ in 0..n {
            self.f.step_slot(&mut self.p);
        }
    }

    fn channel(&mut self, regs: &[(u16, u16)]) {
        for &(off, v) in regs.iter().chain(&[(CTRL, 1)]) {
            match self.f.host_write(off, v, &mut self.p) {
                HostReply::Ack => {}
                HostReply::Wait => loop {
                    self.f.step_slot(&mut self.p);
                    if self.f.host_poll().is_some() {
                        break;
                    }
                },
                r => panic!("{off:#x}: {r:?}"),
            }
        }
    }
}

#[test]
fn p2a_uses_the_column_and_data_lines_but_not_the_row() {
    // P2A の窓：列の番地（U48）は A のバス、データ（U50・U58）は AD のバスを通る。行（U49）は通らない。
    // A0 を +5 V、A13/D5 を +5 V にして 0x1000〜0x100F に i^0x5A を書くと、奇数の番地にだけ、bit 5 を
    // 1 にした値が入る。行が線を通るなら頁 0x30 に入るはずだが、頁 0x10 のまま
    let mut r = Rig::new();
    r.p.doc_board
        .set_bends(Some((&lines("A0=1, A13=1"), DramEnv::default())), 0);
    for i in 0..16 {
        r.p.staging[i] = i as u8 ^ 0x5A;
    }
    r.channel(&[
        (SRC, 0),
        (DST, 2),
        (DST_ADDR, 0x1000),
        (COUNT, 16),
        (SRC_STEP, 1),
        (DST_STEP, 1),
        (FORMAT, 0),
    ]);
    r.run(200);
    assert_eq!(r.f.status(0), DONE);
    for i in 0..16usize {
        let want = if i & 1 == 1 {
            (i as u8 ^ 0x5A) | 0x20
        } else {
            0
        };
        assert_eq!(r.p.doc_board.wave[0x1000 + i], want, "{i}");
    }
    assert!(r.p.doc_board.wave[0x3000..0x3010].iter().all(|&v| v == 0));
    // 読出し：列の bit 0 は 1、受ける値の bit 5 は 1
    r.p.doc_board.wave[0x1000..0x1010].copy_from_slice(&[0x40; 16]);
    r.channel(&[
        (SRC, 2),
        (DST, 6),
        (SRC_ADDR, 0x1000),
        (COUNT, 16),
        (SRC_STEP, 1),
        (FORMAT, 0),
    ]);
    r.p.doc_board.wave[0x1001] = 0x11;
    r.run(400);
    assert_eq!(r.f.status(0), DONE);
    assert_eq!(&r.p.temporal.mem[0..2], &[0x31, 0x31]);
    assert!(r.p.temporal.mem[2..16].iter().all(|&v| v == 0x60));
}

// ---- 波形 RAM の保持（DR-072） ----

/// 偶数の行（RA0＝0、放電すると 0）を 0xFF、奇数の行を 0x00 にする。崩れたビットはみな反転する。
fn fill_against_discharge(wave: &mut [u8]) {
    for (i, w) in wave.iter_mut().enumerate() {
        *w = if (i >> 8) & 1 == 0 { 0xFF } else { 0x00 };
    }
}

/// 行 `rows` のうち、放電した値になったビットの数（`fill_against_discharge` の値から反転したビット）。
fn flipped_in(wave: &[u8], rows: impl IntoIterator<Item = usize>) -> u32 {
    let mut n = 0;
    for r in rows {
        for &w in &wave[r << 8..(r + 1) << 8] {
            n += if r & 1 == 0 {
                (!w).count_ones()
            } else {
                w.count_ones()
            };
        }
    }
    n
}

fn flipped(wave: &[u8]) -> u32 {
    flipped_in(wave, 0..256)
}

#[test]
fn a_slow_doc_clock_lets_the_wave_ram_decay() {
    // CLK 50 kHz（E 160 µs）、N＝16：走査は 18 サイクル＝2.88 ms、リフレッシュは 1 走査に 2 行で、4164 の
    // 128 のまとまりを 64 走査（184.32 ms）で 1 巡する。DR-072 の分布で 25 °C、184.32 ms の割合は
    // 0.999×Φ(ln 0.18432/0.5)＋0.001×Φ(ln 0.9216/0.5)＝3.6e-4＋4.35e-4＝7.95e-4。524,288 ビットで
    // 約 417 ビット（±20）。オシレータ（みな止まって頁 0x80 を読む）が読むまとまり 0 は崩れない
    let mut b = bent_board("", DramEnv::default());
    fill_against_discharge(&mut b.wave);
    b.doc.set_decay(false);
    b.set_clock_period(UNITS_PER_SECOND / 50_000);
    b.doc.write(0xE1, 0x1E);
    let mut t = 0;
    // 8 MHz のままなら崩れない
    let mut fast = bent_board("", DramEnv::default());
    fill_against_discharge(&mut fast.wave);
    fast.doc.write(0xE1, 0x1E);
    let mut tf = 0;
    run(&mut fast, &mut tf, 1_600_000);
    assert_eq!(flipped(&fast.wave), 0);
    // 2 巡と少し（400 ms）
    run(&mut b, &mut t, 1_600_000);
    let n = flipped(&b.wave);
    assert!((330..=500).contains(&n), "{n}");
    assert_eq!(flipped_in(&b.wave, [0x00, 0x80]), 0);
}

/// 試験の CPU：S0 の時刻を進めながらバスサイクルを出す。
struct Cpu {
    sys: System,
    clock: u64,
}

impl Cpu {
    fn new() -> Self {
        let mut sys = System::new();
        sys.ports.doc = true;
        Self { sys, clock: 0 }
    }

    fn rd(&mut self, addr: u32) -> BusReply {
        let r = self
            .sys
            .read(self.clock, addr, fc::SUPERVISOR_DATA, Size::Byte);
        self.done(r)
    }

    fn wr(&mut self, addr: u32, v: u8) -> BusReply {
        let r = self
            .sys
            .write(self.clock, addr, fc::SUPERVISOR_DATA, Size::Byte, v.into());
        self.done(r)
    }

    fn done(&mut self, r: BusReply) -> BusReply {
        self.clock += match r {
            BusReply::Ok { waits, .. } => 4 + u64::from(waits),
            BusReply::BusError { clocks } => u64::from(clocks),
        };
        r
    }

    fn reg(&mut self, n: u8) -> u8 {
        match self.rd(0x21_0000 + 2 * u32::from(n)) {
            BusReply::Ok { data, .. } => data as u8,
            r => panic!("{n:#x}: {r:?}"),
        }
    }

    fn set(&mut self, n: u8, v: u8) {
        assert!(matches!(
            self.wr(0x21_0000 + 2 * u32::from(n), v),
            BusReply::Ok { .. }
        ));
    }

    /// `s` 秒何もしない。
    fn idle_s(&mut self, s: f64) {
        self.clock += (s * 8e6) as u64;
        self.sys.sync(self.clock);
    }
}

#[test]
fn a_bent_row_line_leaves_rows_unrefreshed_only_in_bend_mode() {
    // A8/D0（RA0）を +5 V にすると、リフレッシュの行も DOC の読む行も奇数になり、偶数のまとまりは
    // 活性化されない。100 °C（保持時間は 2^7.5＝181 分の 1、最短 0.28 ms）で 20 ms 置くと、25 °C の
    // 3.6 秒に当たり、Φ(ln 3.6/0.5)＝0.995 が放電する。線を戻すとリフレッシュで崩れが見える。
    // 奇数のまとまりは 64 走査（N＝1 で 192 µs）ごとにリフレッシュされ、崩れない
    for bend in [false, true] {
        let mut c = Cpu::new();
        fill_against_discharge(&mut c.sys.ports.doc_board.wave);
        c.set(0xE1, 0);
        c.sys.bend("dram temp 100").unwrap();
        c.sys.bend("docbus A8=1").unwrap();
        if bend {
            c.sys.bend("mode bend").unwrap();
        }
        assert_eq!(c.sys.ports.doc_board.tracking(), bend);
        c.idle_s(0.02);
        c.sys.bend("docbus normal").unwrap();
        c.idle_s(0.001);
        let wave = &c.sys.ports.doc_board.wave;
        let even = flipped_in(wave, (0..256).step_by(2));
        let odd = flipped_in(wave, (1..256).step_by(2));
        if bend {
            // 偶数の行は 128 行×2048 ビット＝262,144 ビットのうち 99% 以上が放電した
            assert!(even > 259_522, "{even}");
            assert_eq!(odd, 0);
        } else {
            // 通常モードでは追跡しないので崩れない
            assert_eq!((even, odd), (0, 0));
        }
    }
}

#[test]
fn registers_of_disabled_oscillators_decay_in_normal_mode() {
    // 有効でないオシレータはリフレッシュされず、レジスタを保たない（E2 p.15）。チップの性質なので通常
    // モードでも起きる。25 ms は 25 °C の保持時間の分布（中央 20 ms、β 3.6）で 1/(1＋1.25^−3.6)＝0.69
    let mut c = Cpu::new();
    c.set(0xE1, (16 - 1) * 2);
    for n in 0..32 {
        c.set(0x40 + n, 0);
    }
    c.idle_s(0.025);
    let ones = |c: &mut Cpu, r: std::ops::Range<u8>| -> u32 {
        r.map(|n| c.reg(0x40 + n).count_ones()).sum()
    };
    assert_eq!(ones(&mut c, 0..16), 0, "走査するオシレータは保つ");
    let s = f64::from(ones(&mut c, 16..32)) / 128.0;
    assert!((s - 0.69).abs() < 0.15, "{s}");
}

#[test]
fn a_1_hz_doc_clock_times_out_on_the_bus_and_does_not_overflow() {
    // CLK 1 Hz：E の 1 周期が 8 秒。68000 の DOC のアクセスは DR-052 の時間切れでバスエラー
    // （S0 から 67 クロック）。Fabric の port 2 の 2 byte 目は 16 スロットで ERROR（DR-046 (9)）
    let mut c = Cpu::new();
    c.sys.bend("clock doc 1").unwrap();
    c.sys.bend("mode bend").unwrap();
    for _ in 0..4 {
        assert_eq!(
            c.rd(0x21_00E0),
            BusReply::BusError {
                clocks: BERR_CYCLE_CLOCKS
            }
        );
        c.idle_s(0.001);
    }
    // 極端に遅い値も切り詰めて動く（時刻があふれない）
    c.sys.bend("clock doc 1e-30").unwrap();
    c.idle_s(0.01);
    assert!(matches!(c.rd(0x21_00E0), BusReply::BusError { .. }));
    c.sys.bend("clock doc off").unwrap();
    // 基板の発振器に戻しても、次の E の立下りまでは前の周期のまま（DR-035。位相は続く）
    assert!(c.sys.ports.doc_board.cycles < 2);
}

#[test]
fn snapshot_round_trip_with_bent_lines_and_decay() {
    // テストピン（浮かせを含む）、遅い CLK、波形 RAM の追跡が入った途中で保存して戻すと、同じ状態で続く
    let mut c = Cpu::new();
    for (i, w) in c.sys.ports.doc_board.wave.iter_mut().enumerate() {
        *w = (i as u8) | 1;
    }
    c.set(0xE1, (4 - 1) * 2);
    for n in 0..4 {
        c.set(n, 0x00);
        c.set(0x20 + n, 0x01 + n);
        c.set(0x40 + n, 0x80);
        c.set(0x80 + n, 0x10 * n);
        c.set(0xC0 + n, 0x00);
        c.set(0xA0 + n, n << 4);
    }
    for cmd in [
        "docbus A3=Z, A4=A5, A5=A4, D2=0",
        "clock doc 200e3",
        "dram temp 90",
        "mode bend",
    ] {
        c.sys.bend(cmd).unwrap();
    }
    c.idle_s(0.03);
    let json = serde_json::to_string(&c.sys).unwrap();
    let mut back: System = serde_json::from_str(&json).unwrap();
    let to = c.clock + 80_000;
    c.sys.sync(to);
    back.sync(to);
    assert_eq!(c.sys.ports.doc_board, back.ports.doc_board);
    assert!(c.sys.ports.doc_board.tracking());
}
