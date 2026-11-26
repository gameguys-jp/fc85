//! DSP のアダプタ（port 3・4、プログラム RAM、DSP bridge）の試験。
//! 期待値は DR-017、DR-029、台帳 TMS32010.md §2.1・§2.2 と、そこからの手計算で取る。
//! 状態ポートのビットは仮の配置（OI-053）。拍のラッチは DR-049、Fabric の候補の入れ替えは DR-048。

use fc85_board::dsp::{
    BioSel, DspStatus, STATUS_COMMAND, STATUS_INPUT_READY, STATUS_OUTPUT_SPACE, STATUS_TICK,
    late_lines,
};
use fc85_board::fabric::{Fabric, HostReply};
use fc85_board::ports::FabricPorts;

// ---- Data Fabric のレジスタ（仕様書 §06） ----

const SRC: u16 = 0x00;
const DST: u16 = 0x02;
const SRC_STEP: u16 = 0x0E;
const DST_ADDR: u16 = 0x0A;
const COUNT: u16 = 0x0C;
const DST_STEP: u16 = 0x10;
const FORMAT: u16 = 0x12;
const CTRL: u16 = 0x1C;
const AUDIO_N: u16 = 0x102;
const DONE: u16 = 0x02;

// ---- TMS32010 の命令（台帳 TMS32010.md §2・付録の符号） ----

const fn d(hi: u16, dma: u16) -> u16 {
    (hi << 8) | (dma & 0x7F)
}
const fn lack(k: u16) -> u16 {
    0x7E00 | (k & 0xFF)
}
const fn lark(ar: u16, k: u16) -> u16 {
    0x7000 | (ar << 8) | (k & 0xFF)
}
const fn larp(k: u16) -> u16 {
    0x6880 | (k & 1)
}
const ADD: u16 = 0x00;
const LAC: u16 = 0x20;
const IN: u16 = 0x40;
const OUT: u16 = 0x48;
const SACL: u16 = 0x50;
const TBLW: u16 = 0x7D;
const AND: u16 = 0x79;
const NOP: u16 = 0x7F80;
const EINT: u16 = 0x7F82;
const ZAC: u16 = 0x7F89;
const RET: u16 = 0x7F8D;
const BANZ: u16 = 0xF400;
const BIOZ: u16 = 0xF600;
const B: u16 = 0xF900;
const BZ: u16 = 0xFF00;

struct Rig {
    f: Fabric,
    p: FabricPorts,
}

impl Rig {
    fn new() -> Self {
        Self {
            f: Fabric::new(),
            p: FabricPorts::new(),
        }
    }

    fn step(&mut self) {
        self.f.step_slot(&mut self.p);
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

    fn status(&self) -> DspStatus {
        self.p.dsp.status(&self.p.dsp_in, &self.p.dsp_out)
    }

    /// PRGBUS を待ってプログラムを書き、RESET を解く。
    fn boot(&mut self, prog: &[u16], bio: BioSel) {
        while !self.status().prgbus {
            self.step();
        }
        for (i, &w) in prog.iter().enumerate() {
            assert!(self.p.dsp.prog_write(i as u16, w));
        }
        let mut c = self.p.dsp.ctrl();
        c.reset = false;
        c.bio = bio;
        self.p.dsp.set_ctrl(c);
    }
}

#[test]
fn prgbus_follows_reset_and_gates_the_program_window() {
    let mut r = Rig::new();
    assert!(
        !r.p.dsp.prog_write(8, 1),
        "電源投入の直後は PRGBUS＝0 でバスエラー"
    );
    let mut slots = 0;
    while !r.status().prgbus {
        r.step();
        slots += 1;
    }
    // RESET から 8 CLKOUT（1.6 µs）で PRGBUS（台帳 §2.1 の 2.）
    assert_eq!((r.p.dsp.cycles, slots), (8, 6));
    r.boot(
        &[lack(0x5A), d(SACL, 0), d(OUT + 2, 0), B, 3],
        BioSel::InputReady,
    );
    assert!(
        r.p.dsp.prog_read(1).is_some(),
        "PRGBUS は次の CLKOUT まで 1"
    );
    r.step();
    assert_eq!(r.p.dsp.prog_read(1), None, "解除の後は PRGBUS を先に下げる");
    r.run(20);
    assert!(r.p.dsp.irq());
    assert_eq!(r.p.dsp.read_response(), 0x5A);
    assert!(!r.p.dsp.irq());
    // もう一度 RESET：止まって、8 CLKOUT の後に窓が開く。解くと 0 番地から始め直す
    let c = r.p.dsp.ctrl();
    r.p.dsp
        .set_ctrl(fc85_board::dsp::DspCtrl { reset: true, ..c });
    r.step();
    assert!(!r.p.dsp.prog_write(0, lack(0x33)));
    r.run(6);
    assert!(r.p.dsp.prog_write(0, lack(0x33)));
    r.p.dsp.set_ctrl(c);
    r.run(20);
    assert_eq!(r.p.dsp.read_response(), 0x33);
}

/// 入力 FIFO が空でない間（BIO）、1 語読んで 2 倍にして出す。
const DOUBLER: [u16; 11] = [
    BIOZ,
    4,
    B,
    0,
    d(IN, 0),
    d(LAC, 0),
    d(ADD, 0),
    d(SACL, 1),
    d(OUT + 1, 1),
    B,
    0,
];

/// ch0：Temporal RAM（port 7）→ port 3、ch1：port 4 → Temporal RAM（port 6）。
fn block_channels(r: &mut Rig, input: &[u8], out_at: u16) {
    r.p.temporal.mem[..input.len()].copy_from_slice(input);
    r.p.temporal.rptr = 0;
    r.p.temporal.wptr = out_at;
    let n = input.len() as u16;
    for (off, v) in [(SRC, 7), (DST, 3), (COUNT, n), (FORMAT, 2)] {
        r.write(off, v);
    }
    for (off, v) in [(SRC, 4), (DST, 6), (COUNT, n), (FORMAT, 3)] {
        r.write(0x40 + off, v);
    }
    r.write(CTRL, 1);
    r.write(0x40 + CTRL, 1);
}

#[test]
fn dsp_processes_a_block_through_the_fabric() {
    let mut r = Rig::new();
    let input: Vec<u8> = (0..16).map(|i| i * 3 + 1).collect();
    r.boot(&DOUBLER, BioSel::InputReady);
    block_channels(&mut r, &input, 0x100);
    r.run(400);
    assert_eq!((r.f.status(0), r.f.status(1)), (DONE, DONE));
    let want: Vec<u8> = input.iter().map(|x| x * 2).collect();
    assert_eq!(&r.p.temporal.mem[0x100..0x110], &want[..]);
    let s = r.status();
    assert!(!s.underrun && !s.overrun);
}

#[test]
fn dsp_processes_a_block_from_and_to_staging_ram() {
    // ch0 は Staging RAM から読み、ch1 は DSP の出力を待つ。DR-046 (2) のままでは ch0 の候補が S の頭に
    // だけ来て両方が止まった（OI-052）。DR-048 で、候補を S の頭で手放さないようにした。
    let mut r = Rig::new();
    let input: Vec<u16> = (0..16).map(|i| 0x1000 + i * 3).collect();
    for (i, w) in input.iter().enumerate() {
        r.p.staging[2 * i..2 * i + 2].copy_from_slice(&w.to_be_bytes());
    }
    r.boot(&DOUBLER, BioSel::InputReady);
    for (off, v) in [(SRC, 0), (DST, 3), (COUNT, 16), (SRC_STEP, 2), (FORMAT, 1)] {
        r.write(off, v);
    }
    for (off, v) in [
        (SRC, 4),
        (DST, 0),
        (DST_ADDR, 0x100),
        (COUNT, 16),
        (DST_STEP, 2),
        (FORMAT, 1),
    ] {
        r.write(0x40 + off, v);
    }
    r.write(CTRL, 1);
    r.write(0x40 + CTRL, 1);
    r.run(400);
    assert_eq!((r.f.status(0), r.f.status(1)), (DONE, DONE));
    let got: Vec<u16> = (0..16)
        .map(|i| u16::from_be_bytes([r.p.staging[0x100 + 2 * i], r.p.staging[0x101 + 2 * i]]))
        .collect();
    assert_eq!(got, input.iter().map(|x| x * 2).collect::<Vec<_>>());
}

#[test]
fn empty_input_returns_the_last_word_and_full_output_drops() {
    let mut r = Rig::new();
    r.p.dsp_in.shift_in(0x1234);
    // IN 0 を 2 回、OUT 1 を 100 回（BANZ は 0 でなければ分岐してから 1 減らす）
    r.boot(
        &[
            d(IN, 0),
            d(IN, 1),
            lark(0, 99),
            larp(0),
            d(OUT + 1, 1),
            BANZ,
            4,
            B,
            7,
        ],
        BioSel::InputReady,
    );
    r.run(400);
    assert_eq!((r.p.dsp.dsp.data(0), r.p.dsp.dsp.data(1)), (0x1234, 0x1234));
    assert_eq!((r.p.dsp.io.underruns, r.p.dsp.io.overruns), (1, 36));
    assert_eq!(r.p.dsp_out.len(), 64);
    let s = r.status();
    assert!(s.underrun && s.overrun);
    r.p.dsp.clear_flags(true, false);
    let s = r.status();
    assert!(!s.underrun && s.overrun, "W1C は書いたビットだけ");
}

#[test]
fn status_port_and_mailbox() {
    let mut r = Rig::new();
    for w in [1, 2, 3] {
        r.p.dsp_in.shift_in(w);
    }
    r.p.dsp.write_mailbox(0xBEEF);
    r.boot(
        &[
            d(IN + 2, 0),
            d(IN + 3, 1),
            d(IN + 2, 2),
            d(OUT + 2, 1),
            B,
            4,
        ],
        BioSel::InputReady,
    );
    r.run(20);
    let base = STATUS_INPUT_READY | STATUS_OUTPUT_SPACE | (3 << 8);
    assert_eq!(r.p.dsp.dsp.data(0), base | STATUS_COMMAND);
    assert_eq!(r.p.dsp.dsp.data(2), base, "IN 3 で着信が下がる");
    assert_eq!(r.p.dsp.read_response(), 0xBEEF);
}

#[test]
fn a_command_interrupts_the_dsp() {
    let mut r = Rig::new();
    let mut prog = vec![NOP; 24];
    prog[..6].copy_from_slice(&[B, 4, B, 20, EINT, B]);
    prog[6] = 5;
    prog[20..24].copy_from_slice(&[d(IN + 3, 0), d(OUT + 2, 0), EINT, RET]);
    r.boot(&prog, BioSel::InputReady);
    r.run(20);
    assert!(!r.p.dsp.irq());
    for v in [0x1111, 0x2222] {
        r.p.dsp.write_mailbox(v);
        r.run(20);
        assert!(!r.status().command);
        assert_eq!(r.p.dsp.read_response(), v);
    }
    assert!((5..=6).contains(&r.p.dsp.dsp.pc()), "待ちの B に戻る");
}

#[test]
fn tblw_to_addresses_0_to_7_acts_as_out() {
    let mut r = Rig::new();
    let prog = [
        lack(0x77),
        d(SACL, 0),
        lack(2),
        d(TBLW, 0),
        lack(0x80),
        d(TBLW, 0),
        B,
        6,
    ];
    r.boot(&prog, BioSel::InputReady);
    r.run(20);
    // 2 番地へは /WE が出ず、OUT 2 と同じになる（DR-029 (2)）
    assert_eq!(r.p.dsp.prog[2], lack(2));
    assert_eq!(r.p.dsp.read_response(), 0x77);
    assert_eq!(r.p.dsp.io.port_tblw, 1);
    assert_eq!(r.p.dsp.prog[0x80], 0x77);
}

#[test]
fn soft_synthesis_paced_by_output_space() {
    let mut r = Rig::new();
    // 出力 FIFO に空きがある間（BIO）、0、1、2… を出す（DR-029 (3)）
    r.boot(
        &[
            lack(1),
            d(SACL, 1),
            ZAC,
            d(SACL, 0),
            BIOZ,
            8,
            B,
            4,
            d(OUT + 1, 0),
            d(LAC, 0),
            d(ADD, 1),
            d(SACL, 0),
            B,
            4,
        ],
        BioSel::OutputSpace,
    );
    r.write(AUDIO_N, 40);
    // port 4 → port 2（本体 DAC）。DAC の FIFO が満杯の間は STALL で待つ
    for (off, v) in [(SRC, 4), (DST, 2), (COUNT, 1000), (FORMAT, 1)] {
        r.write(off, v);
    }
    r.write(CTRL, 1);
    let mut got = Vec::new();
    let mut beats = r.p.main_dac.beats;
    while got.len() < 150 {
        r.step();
        if r.p.main_dac.beats != beats {
            beats = r.p.main_dac.beats;
            got.push(r.p.main_dac.latch);
        }
    }
    assert_eq!(got, (0..150).collect::<Vec<u16>>());
    assert!(!r.p.main_dac.underrun);
    assert_eq!(r.p.dsp.io.overruns, 0);
}

/// 拍を数えるプログラム（DR-049）。データ RAM の 0 番地が数。
/// BIO 版：BIO（拍のラッチ）が Low になるのを待ち、IN 2 を読んで消してから 1 足す。
const TICK_BY_BIO: [u16; 14] = [
    lack(1),
    d(SACL, 1),
    ZAC,
    d(SACL, 0),
    BIOZ,
    8,
    B,
    4,
    d(IN + 2, 2),
    d(LAC, 0),
    d(ADD, 1),
    d(SACL, 0),
    B,
    4,
];
/// 状態版：IN 2 を読み続け、拍のビットが 1 なら 1 足す（読めば消える）。
const TICK_BY_STATUS: [u16; 16] = [
    lack(1),
    d(SACL, 1),
    lack(0x40),
    d(SACL, 3),
    ZAC,
    d(SACL, 0),
    d(IN + 2, 2),
    d(LAC, 2),
    d(AND, 3),
    BZ,
    6,
    d(LAC, 0),
    d(ADD, 1),
    d(SACL, 0),
    B,
    6,
];

#[test]
fn the_tick_latch_counts_every_audio_tick() {
    assert_eq!(STATUS_TICK, 0x40);
    let mut got = Vec::new();
    for (prog, bio) in [
        (&TICK_BY_BIO[..], BioSel::AudioTick),
        (&TICK_BY_STATUS[..], BioSel::InputReady),
    ] {
        for n in [40, 41, 48, 100] {
            let mut r = Rig::new();
            r.boot(prog, bio);
            r.write(AUDIO_N, n);
            r.run(2100);
            let t0 = r.p.audio.ticks;
            let c0 = r.p.dsp.dsp.data(0);
            r.run(8000);
            let counted = u64::from(r.p.dsp.dsp.data(0) - c0);
            let beats = r.p.audio.ticks - t0;
            // 区切りの前後の拍が 1 つずれることがある
            assert!(counted.abs_diff(beats) <= 1, "N={n}：{counted}/{beats}");
            got.push(beats);
        }
    }
    // 250 ns のパルスのままでは、BIO の待ちループが N＝41 で 146/391、N＝48 で 0/333 だった（OI-053）
    assert_eq!(&got[..4], &[400, 391, 333, 160]);
}

#[test]
fn reset_clears_the_tick_latch() {
    let mut r = Rig::new();
    r.write(AUDIO_N, 40);
    r.run(2100);
    assert!(r.p.audio.ticks > 0);
    assert!(!r.p.dsp.io.tick, "RESET の間は 0");
    r.boot(&TICK_BY_BIO, BioSel::AudioTick);
    r.run(20 * 10);
    assert!(r.p.dsp.dsp.data(0) >= 9);
}

#[test]
fn snapshot_round_trip_while_the_dsp_runs() {
    let mut r = Rig::new();
    let input: Vec<u8> = (0..40).map(|i| 0x10 + i).collect();
    r.boot(&DOUBLER, BioSel::InputReady);
    block_channels(&mut r, &input, 0x200);
    r.run(60);
    let saved = serde_json::to_string(&(&r.f, &r.p)).unwrap();
    let (f2, p2): (Fabric, FabricPorts) = serde_json::from_str(&saved).unwrap();
    let mut b = Rig { f: f2, p: p2 };
    for _ in 0..800 {
        assert_eq!(r.f.step_slot(&mut r.p), b.f.step_slot(&mut b.p));
    }
    assert_eq!(r.p, b.p);
    let want: Vec<u8> = input.iter().map(|x| x * 2).collect();
    assert_eq!(&r.p.temporal.mem[0x200..0x228], &want[..]);
}

/// CLKIN の周波数から、命令サイクル（CLKIN の 4 周期）を時刻の単位で。
fn tc(hz: f64) -> u64 {
    4 * (fc85_core::time::UNITS_PER_SECOND as f64 / hz).round() as u64
}

#[test]
fn an_overclocked_dsp_latches_late_program_lines_from_the_previous_word() {
    // 使える時間 tc − 100 ns と、RAM と切替回路の 75 ns（台帳 §2.1、仕様書 §05。DR-066）
    assert_eq!(late_lines(tc(20e6)), 0, "基板の 20 MHz：余裕 25 ns");
    assert_eq!(late_lines(tc(20.5e6)), 0, "資料の上限 20.5 MHz");
    assert_eq!(late_lines(tc(22.8e6)), 0, "tc 175.4 ns：ちょうど間に合う");
    let n23 = late_lines(tc(23e6)).count_ones();
    assert!(
        (1..16).contains(&n23),
        "23 MHz では一部の線だけ遅れる: {n23}"
    );
    assert_eq!(
        late_lines(tc(24.6e6)),
        0xFFFF,
        "tc 162.6 ns：全部の線が前の語"
    );
    let mut last = 0;
    for mhz in 20..=26 {
        let n = late_lines(tc(f64::from(mhz) * 1e6)).count_ones();
        assert!(n >= last, "速くするほど崩れる線は増える");
        last = n;
    }

    // 同じプログラムを 20 MHz と 25 MHz で動かす：25 MHz では前の語の値で命令を取り込み、暴走する
    let prog = [lack(0x5A), d(SACL, 0), d(OUT + 2, 0), B, 3];
    let mut normal = Rig::new();
    normal.boot(&prog, BioSel::InputReady);
    normal.run(200);
    assert_eq!(normal.p.dsp.late_fetches, 0);
    assert_eq!(normal.p.dsp.read_response(), 0x5A);
    let mut fast = Rig::new();
    fast.p.dsp.set_input_period(tc(25e6));
    fast.boot(&prog, BioSel::InputReady);
    fast.run(200);
    assert!(fast.p.dsp.late_fetches > 0);
}
