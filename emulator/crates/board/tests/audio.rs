//! 音声クロック（TRIGGER 3）、本体 DAC（port 2）、ADPCM のアダプタ（port 5、TRIGGER 5）の試験。
//! 期待値は DR-017、DR-018、DR-019、台帳 MSM5205.md §3（V̄C̄K̄ の周期）と、そこからの手計算で取る。

use fc85_board::adpcm::AdpcmCtrl;
use fc85_board::fabric::{Decision, Fabric, HostReply, Phase, SlotRecord};
use fc85_board::ports::FabricPorts;
use fc85_core::time::PERIOD_8M;

const SRC: u16 = 0x00;
const DST: u16 = 0x02;
const SRC_ADDR: u16 = 0x06;
const COUNT: u16 = 0x0C;
const SRC_STEP: u16 = 0x0E;
const FORMAT: u16 = 0x12;
const TRIGGER: u16 = 0x1A;
const CTRL: u16 = 0x1C;
const AUDIO_N: u16 = 0x102;

const DONE: u16 = 0x02;
const EACH: u16 = 0x08;

struct Rig {
    f: Fabric,
    p: FabricPorts,
    trace: Vec<SlotRecord>,
}

impl Rig {
    fn new() -> Self {
        Self {
            f: Fabric::new(),
            p: FabricPorts::new(),
            trace: Vec::new(),
        }
    }

    fn step(&mut self) -> SlotRecord {
        let r = self.f.step_slot(&mut self.p);
        self.trace.push(r);
        r
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

    #[allow(clippy::too_many_arguments)]
    fn channel(
        &mut self,
        src: u16,
        dst: u16,
        src_addr: u16,
        count: u16,
        step: u16,
        format: u16,
        trigger: u16,
    ) {
        self.write(SRC, src);
        self.write(DST, dst);
        self.write(SRC_ADDR, src_addr);
        self.write(COUNT, count);
        self.write(SRC_STEP, step);
        self.write(FORMAT, format);
        self.write(TRIGGER, trigger);
        // 発生源の選択器は同期化の前にある。選んだ入力が High だと立上りとして伝わるので、
        // 同期化を抜けるまで待ってから START する（emulator.md §03）
        for _ in 0..4 {
            self.step();
        }
        self.write(CTRL, 1);
    }

    /// 本体 DAC の拍が来るまで進め、そのスロットを返す。
    fn next_beat(&mut self, limit: usize) -> u64 {
        let b = self.p.main_dac.beats;
        for _ in 0..limit {
            let r = self.step();
            if self.p.main_dac.beats != b {
                return r.slot;
            }
        }
        panic!("拍が来ない");
    }

    /// ADPCM の取込みが来るまで進め、取り込んだニブルを返す。
    fn next_capture(&mut self, limit: usize) -> u8 {
        let c = self.p.adpcm_port.captures;
        for _ in 0..limit {
            self.step();
            if self.p.adpcm_port.captures != c {
                return self.p.adpcm_port.last_nibble;
            }
        }
        panic!("取込みが来ない");
    }

    fn starts(&self) -> Vec<u64> {
        self.trace
            .iter()
            .filter(|r| matches!(r.decision, Decision::Start(_)))
            .map(|r| r.slot)
            .collect()
    }
}

fn gaps(v: &[u64]) -> Vec<u64> {
    v.windows(2).map(|w| w[1] - w[0]).collect()
}

fn adpcm_on(r: &mut Rig, f: impl FnOnce(&mut AdpcmCtrl)) {
    let mut c = r.p.adpcm_port.ctrl();
    c.reset = false;
    f(&mut c);
    r.p.adpcm_port.set_ctrl(c);
}

// ---------------------------------------------------------------- 音声クロックと本体 DAC

#[test]
fn audio_clock_divides_8mhz_by_n_from_the_next_beat() {
    let mut r = Rig::new();
    // 電源投入時の N は 0（4096 として数える）。最初の拍は 4096 クロック＝2048 スロットの終わりのスロット
    r.write(AUDIO_N, 100);
    let first = r.next_beat(3000);
    assert_eq!(first, 2047);
    // 書いた N は次の拍から：100 クロック＝12.5 µs＝50 スロットごと
    let beats: Vec<u64> = (0..4).map(|_| r.next_beat(200)).collect();
    assert_eq!(gaps(&[vec![first], beats].concat()), vec![50; 4]);
    // ベンド点：入力を 4 MHz の外部クロックにすると、拍の間隔が倍になる（DR-035）
    r.p.audio.set_input_period(2 * PERIOD_8M);
    let a = r.next_beat(200);
    let b = r.next_beat(200);
    let c = r.next_beat(200);
    assert_eq!((b - a, c - b), (100, 100));
}

#[test]
fn main_dac_takes_one_word_per_beat_and_holds_when_empty() {
    let mut r = Rig::new();
    r.write(AUDIO_N, 40);
    r.next_beat(3000);
    assert!(r.p.main_dac.underrun, "空の FIFO の拍");
    r.p.main_dac.underrun = false;
    for w in [0x1020, 0x3040, 0x5060] {
        r.p.dac.shift_in(w);
    }
    let mut got = Vec::new();
    for _ in 0..4 {
        r.next_beat(100);
        got.push((
            r.p.main_dac.left(),
            r.p.main_dac.right(),
            r.p.main_dac.underrun,
        ));
    }
    assert_eq!(
        got,
        vec![
            (0x10, 0x20, false),
            (0x30, 0x40, false),
            (0x50, 0x60, false),
            (0x50, 0x60, true)
        ]
    );
}

#[test]
fn the_fabric_keeps_the_dac_fifo_filled_from_staging_ram() {
    let mut r = Rig::new();
    let words: Vec<u16> = (0..100).map(|i| 0x0101 * i + 0x8000).collect();
    for (i, w) in words.iter().enumerate() {
        r.p.staging[2 * i..2 * i + 2].copy_from_slice(&w.to_be_bytes());
    }
    r.write(AUDIO_N, 100);
    // Staging RAM → port 2（本体 DAC の FIFO）。満杯なら STALL で待つ（DR-017 (2)）
    r.channel(0, 2, 0, 100, 2, 1, 0);
    r.next_beat(3000);
    let mut got = vec![r.p.main_dac.latch];
    while got.len() < 100 {
        r.next_beat(100);
        got.push(r.p.main_dac.latch);
        assert!(!r.p.main_dac.underrun, "{} 語目", got.len());
    }
    assert_eq!(got, words);
    assert_eq!(r.f.status(0), DONE);
}

#[test]
fn trigger3_paces_one_element_per_beat() {
    let mut r = Rig::new();
    r.p.char_rom[..5].copy_from_slice(&[1, 2, 3, 4, 5]);
    r.write(AUDIO_N, 40);
    r.next_beat(3000);
    // 文字 ROM → Temporal RAM を、拍（20 スロットごと）に 1 要素ずつ
    r.channel(9, 6, 0, 5, 1, 0, 3 | EACH);
    for _ in 0..200 {
        r.step();
        if r.f.phase(0) == Phase::Idle {
            break;
        }
    }
    assert_eq!(r.f.status(0), DONE);
    assert_eq!(&r.p.temporal.mem[..5], &[1, 2, 3, 4, 5]);
    assert_eq!(gaps(&r.starts()), vec![20; 4]);
}

// ---------------------------------------------------------------- ADPCM

#[test]
fn adpcm_uses_the_high_nibble_first_and_0x08_when_empty() {
    let mut r = Rig::new();
    r.p.adpcm.shift_in(0x12);
    r.p.adpcm.shift_in(0x34);
    adpcm_on(&mut r, |_| {});
    // 8 kHz（384 kHz÷48）：125 µs＝500 スロットごとに 1 つ
    let got: Vec<u8> = (0..9).map(|_| r.next_capture(600)).collect();
    // 解除の前に置いたデータ（電源投入時の 0）を、最初の 1 回で取り込むことがある
    let s = got.iter().position(|&n| n == 1).unwrap();
    assert!(s <= 1, "{got:?}");
    assert_eq!(&got[s..s + 8], &[1, 2, 3, 4, 0, 8, 0, 8]);
    assert!(r.p.adpcm_port.underrun);
}

#[test]
fn adpcm_low_first_and_no_replace() {
    let mut r = Rig::new();
    r.p.adpcm.shift_in(0x12);
    r.p.adpcm.shift_in(0x34);
    adpcm_on(&mut r, |c| {
        c.low_first = true;
        c.no_replace = true;
    });
    let got: Vec<u8> = (0..9).map(|_| r.next_capture(600)).collect();
    let s = got.iter().position(|&n| n == 2).unwrap();
    // 空になった後は、FIFO の出力に残った 0x34 を繰り返す（Bend）
    assert_eq!(&got[s..s + 8], &[2, 1, 4, 3, 4, 3, 4, 3]);
}

#[test]
fn adpcm_capture_period_follows_s1_s2() {
    for (s1, s2, slots) in [(true, false, 500), (false, true, 667), (false, false, 1000)] {
        let mut r = Rig::new();
        adpcm_on(&mut r, |c| {
            c.s1 = s1;
            c.s2 = s2;
        });
        r.next_capture(1200);
        let a = r.trace.len();
        r.next_capture(1200);
        let b = r.trace.len();
        // 384 kHz÷48・÷64・÷96 の周期（スロットに丸める）
        assert!((b - a).abs_diff(slots) <= 1, "S1={s1} S2={s2}：{}", b - a);
    }
}

#[test]
fn adpcm_reset_leaves_the_fifo_alone() {
    let mut r = Rig::new();
    for b in 0..10 {
        r.p.adpcm.shift_in(b);
    }
    // 電源投入時は RESET（CTRL の既定）
    for _ in 0..3000 {
        r.step();
    }
    assert_eq!(r.p.adpcm.len(), 10);
    assert!(!r.p.adpcm_port.underrun);
}

#[test]
fn adpcm_half_empty_irq_is_a_level() {
    let mut r = Rig::new();
    for b in 0..34 {
        r.p.adpcm.shift_in(b);
    }
    adpcm_on(&mut r, |c| c.half_irq = true);
    assert!(!r.p.adpcm_port.irq(&r.p.adpcm));
    // 2 つのニブル（1 byte）ごとに 1 つ減る。32 byte 以下になれば割込み
    while r.p.adpcm.len() > 32 {
        r.step();
    }
    assert!(r.p.adpcm_port.irq(&r.p.adpcm));
    r.p.adpcm.shift_in(0x77);
    assert!(!r.p.adpcm_port.irq(&r.p.adpcm), "書き足せば下がる");
}

#[test]
fn trigger5_follows_vck() {
    let mut r = Rig::new();
    r.p.char_rom[..3].copy_from_slice(&[9, 8, 7]);
    adpcm_on(&mut r, |_| {});
    r.channel(9, 6, 0, 3, 1, 0, 5 | EACH);
    for _ in 0..2000 {
        r.step();
        if r.f.phase(0) == Phase::Idle {
            break;
        }
    }
    assert_eq!(r.f.status(0), DONE);
    assert_eq!(&r.p.temporal.mem[..3], &[9, 8, 7]);
    let g = gaps(&r.starts());
    assert!(g.iter().all(|x| x.abs_diff(500) <= 1), "{g:?}");
}

#[test]
fn selecting_a_high_source_right_before_start_counts_as_an_edge() {
    let mut r = Rig::new();
    r.p.char_rom[..2].copy_from_slice(&[1, 2]);
    adpcm_on(&mut r, |_| {});
    // V̄C̄K̄ が High になった直後まで進める
    while r.p.adpcm_port.vck() {
        r.step();
    }
    while !r.p.adpcm_port.vck() {
        r.step();
    }
    r.write(SRC, 9);
    r.write(DST, 6);
    r.write(COUNT, 2);
    r.write(SRC_STEP, 1);
    r.write(TRIGGER, 5 | EACH);
    r.write(CTRL, 1);
    let t0 = r.trace.len() as u64;
    for _ in 0..1200 {
        r.step();
        if r.f.phase(0) == Phase::Idle {
            break;
        }
    }
    let s = r.starts();
    // 1 つ目は選んだときの立上り（74LS151 → U36 → STS。fabric.md §06）、2 つ目は次の本当の立上り
    assert!(s[0] - t0 <= 8, "{s:?}");
    assert!(s[1] - s[0] > 450, "{s:?}");
}

#[test]
fn snapshot_round_trip_with_audio_running() {
    let mut r = Rig::new();
    for b in 0..20 {
        r.p.adpcm.shift_in(0x90 + b);
    }
    adpcm_on(&mut r, |_| {});
    r.write(AUDIO_N, 30);
    for i in 0..20u16 {
        r.p.staging[2 * i as usize..2 * i as usize + 2].copy_from_slice(&(i * 3).to_be_bytes());
    }
    r.channel(0, 2, 0, 20, 2, 1, 0);
    for _ in 0..2500 {
        r.step();
    }
    let saved = serde_json::to_string(&(&r.f, &r.p)).unwrap();
    let (f2, p2): (Fabric, FabricPorts) = serde_json::from_str(&saved).unwrap();
    let mut b = Rig {
        f: f2,
        p: p2,
        trace: Vec::new(),
    };
    for _ in 0..1500 {
        assert_eq!(r.step(), b.step());
    }
    assert_eq!(r.p, b.p);
}
