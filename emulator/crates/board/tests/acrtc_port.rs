//! port 1（ACRTC の DMA）の試験（仕様書 §12 段階 2、DR-016、DR-022）。
//! 期待値は仕様書 §06、DR-022、`docs/parts/host-acrtc-interface.md` §2.3・§7 と、`fc85_board::acrtc` の
//! 模型の前提（ピンの遅れ）からの手計算で取る。

use fc85_board::acrtc::UNITS_PER_2CLK;
use fc85_board::fabric::{Decision, Fabric, HostReply, Phase, SlotRecord};
use fc85_board::ports::FabricPorts;

// Data Fabric のチャネルのオフセット（仕様書 §06）。
const SRC: u16 = 0x00;
const DST: u16 = 0x02;
const SRC_ADDR: u16 = 0x06;
const DST_ADDR: u16 = 0x0A;
const COUNT: u16 = 0x0C;
const SRC_STEP: u16 = 0x0E;
const DST_STEP: u16 = 0x10;
const FORMAT: u16 = 0x12;
const TRIGGER: u16 = 0x1A;
const CTRL: u16 = 0x1C;

const BUSY: u16 = 0x01;
const DONE: u16 = 0x02;
const STALL: u16 = 0x04;
const ERROR: u16 = 0x08;

// ACRTC のコマンド（U85 p.173）と CCR。
const WPR: u16 = 0x0800;
const RWPH: u16 = 0x0C;
const RWPL: u16 = 0x0D;
const DRD: u16 = 0x2400;
const DWT: u16 = 0x2800;
/// DDM＝1、DRC＝1（サイクルスチール。DR-016）、GBM＝010。
const CCR_DMA: u16 = 0x2A00;
const CCR_ABT: u16 = 0x8000;

struct Rig {
    f: Fabric,
    p: FabricPorts,
    trace: Vec<SlotRecord>,
}

impl Rig {
    /// ACRTC：DMA を許し、画面 0 の MW を 0x40、STR＝1。
    fn new() -> Self {
        let mut r = Self {
            f: Fabric::new(),
            p: FabricPorts::new(),
            trace: Vec::new(),
        };
        r.acrtc_reg(0x02, CCR_DMA);
        r.acrtc_reg(0xC2, 0x40);
        r.acrtc_reg(0x04, 0xC000);
        r
    }

    fn step(&mut self) -> SlotRecord {
        let rec = self.f.step_slot(&mut self.p);
        self.trace.push(rec);
        rec
    }

    fn run(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    fn acrtc_reg(&mut self, ar: u16, v: u16) {
        self.p.acrtc.acrtc.write(false, ar);
        self.p.acrtc.acrtc.write(true, v);
    }

    /// ACRTC の書込み FIFO にコマンドとパラメータを書く（CPU のプログラム転送）。満杯ならスロットを進める。
    fn acrtc_cmd(&mut self, words: &[u16]) {
        self.p.acrtc.acrtc.write(false, 0);
        for &w in words {
            while self.p.acrtc.acrtc.status() & 0x02 == 0 {
                self.step();
            }
            self.p.acrtc.acrtc.write(true, w);
        }
    }

    fn set_rwp(&mut self, addr: u32) {
        self.acrtc_cmd(&[
            WPR | RWPH,
            (addr >> 12) as u16,
            WPR | RWPL,
            ((addr & 0xFFF) << 4) as u16,
        ]);
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
        dst_addr: u16,
        count: u16,
        step: u16,
        trigger: u16,
    ) {
        // 16 bit の入出力（ACRTC の語）
        self.channel_format(src, dst, src_addr, dst_addr, count, step, trigger, 1);
    }

    #[allow(clippy::too_many_arguments)]
    fn channel_format(
        &mut self,
        src: u16,
        dst: u16,
        src_addr: u16,
        dst_addr: u16,
        count: u16,
        step: u16,
        trigger: u16,
        format: u16,
    ) {
        self.write(SRC, src);
        self.write(DST, dst);
        self.write(SRC_ADDR, src_addr);
        self.write(DST_ADDR, dst_addr);
        self.write(COUNT, count);
        self.write(SRC_STEP, step);
        self.write(DST_STEP, step);
        self.write(FORMAT, format);
        self.write(TRIGGER, trigger);
        self.write(CTRL, 1);
    }

    fn run_until_idle(&mut self, limit: usize) {
        for _ in 0..limit {
            if self.f.phase(0) == Phase::Idle {
                return;
            }
            self.step();
        }
        panic!(
            "{limit} スロットで終わらない（STATUS {:#x}）",
            self.f.status(0)
        );
    }

    fn acrtc_finish(&mut self) {
        for _ in 0..10_000 {
            if self.p.acrtc.acrtc.idle() {
                return;
            }
            self.step();
        }
        panic!("ACRTC のコマンドが終わらない");
    }

    /// port 1 のアクセスごとの長さ（スロット）。
    fn port1_lengths(&self) -> Vec<u64> {
        let mut out = Vec::new();
        let mut n = 0;
        for r in &self.trace {
            if r.access.is_some_and(|a| a.port == 1) {
                n += 1;
                if r.access_done {
                    out.push(n);
                    n = 0;
                }
            }
        }
        out
    }

    fn start_gaps(&self) -> Vec<u64> {
        let s: Vec<u64> = self
            .trace
            .iter()
            .filter(|r| matches!(r.decision, Decision::Start(_)))
            .map(|r| r.slot)
            .collect();
        s.windows(2).map(|w| w[1] - w[0]).collect()
    }
}

fn staging_words(p: &mut FabricPorts, at: usize, words: &[u16]) {
    for (i, w) in words.iter().enumerate() {
        p.staging[at + 2 * i..at + 2 * i + 2].copy_from_slice(&w.to_be_bytes());
    }
}

#[test]
fn dwt_from_staging_ram_fills_the_rectangle() {
    let mut r = Rig::new();
    let words: Vec<u16> = (1..=8).map(|i| 0x1100 * i).collect();
    staging_words(&mut r.p, 0x1000, &words);
    r.set_rwp(0x100);
    // AX＝3、AY＝−1：4 語×2 行、下へ。Fabric の COUNT は x·y＝8（DR-022）
    r.acrtc_cmd(&[DWT, 3, (-1i16) as u16]);
    r.channel(0, 1, 0x1000, 0, 8, 2, 0);
    r.run_until_idle(500);
    assert_eq!(r.f.status(0), DONE);
    assert_eq!(r.p.acrtc.acrtc.dma_progress(), (8, 8));
    r.acrtc_finish();
    assert_eq!(r.p.acrtc.vram.0[0x100..0x104], words[..4]);
    assert_eq!(r.p.acrtc.vram.0[0x140..0x144], words[4..]);
    assert_eq!(r.p.acrtc.acrtc.error_counts(), (0, 0));
    // DACK を出してから DTACK を同期化して戻すまで、いつも 3 スロット（host-acrtc-interface.md §7.2 の計算を
    // 本機のスロットに当てはめたもの。emulator.md §03）
    assert_eq!(r.port1_lengths(), vec![3; 8]);
    // Staging RAM は S のスロットでしか読めないので、要素は 6 か 8 スロットに 1 つ
    let gaps = r.start_gaps();
    assert!(gaps.iter().all(|g| *g == 6 || *g == 8), "{gaps:?}");
}

#[test]
fn drd_to_staging_ram_reads_the_rectangle() {
    let mut r = Rig::new();
    for i in 0..4 {
        r.p.acrtc.vram.0[0x300 + i] = 0xA0A0 + i as u16;
    }
    r.set_rwp(0x300);
    r.acrtc_cmd(&[DRD, 3, 0]);
    r.channel(1, 0, 0, 0x2000, 4, 2, 0);
    r.run_until_idle(500);
    assert_eq!(r.f.status(0), DONE);
    let got: Vec<u16> = (0..4)
        .map(|i| u16::from_be_bytes([r.p.staging[0x2000 + 2 * i], r.p.staging[0x2001 + 2 * i]]))
        .collect();
    assert_eq!(got, vec![0xA0A0, 0xA0A1, 0xA0A2, 0xA0A3]);
    r.acrtc_finish();
    assert_eq!(r.p.acrtc.acrtc.error_counts(), (0, 0));
}

#[test]
fn one_word_takes_six_or_seven_slots_from_temporal_ram() {
    // Temporal RAM（逐次、S のスロットを待たない）から。位相をずらしても、port 1 のアクセスは 3 スロットで、
    // 要素は 6 か 7 スロットに 1 つ（次の DREQ の同期化が、中ほどの取込みに間に合うかで決まる）
    for phase in (0..UNITS_PER_2CLK).step_by(400) {
        let mut r = Rig::new();
        r.p.acrtc.set_phase(phase);
        for i in 0..16 {
            r.p.temporal.mem[i] = i as u8;
        }
        r.set_rwp(0x400);
        r.acrtc_cmd(&[DWT, 15, 0]);
        // Temporal RAM は 8 bit。FORMAT 2 でゼロ拡張して 16 bit の語にする
        r.channel_format(7, 1, 0, 0, 16, 1, 0, 2);
        r.run_until_idle(500);
        assert_eq!(r.f.status(0), DONE, "位相 {phase}");
        assert_eq!(r.port1_lengths(), vec![3; 16], "位相 {phase}");
        let gaps = r.start_gaps();
        assert!(
            gaps.iter().all(|g| (6..=7).contains(g)),
            "位相 {phase}：{gaps:?}"
        );
        r.acrtc_finish();
        assert!(
            (0..16).all(|i| r.p.acrtc.vram.0[0x400 + i] == i as u16),
            "位相 {phase}"
        );
    }
}

#[test]
fn count_larger_than_the_rectangle_is_an_error() {
    let mut r = Rig::new();
    staging_words(&mut r.p, 0x1000, &[1, 2, 3, 4, 5, 6]);
    r.set_rwp(0x100);
    r.acrtc_cmd(&[DWT, 3, 0]);
    // COUNT＝6 に対し、ACRTC は 4 語目で DONE を出す。IRQ_EN＝1
    r.channel(0, 1, 0x1000, 0, 6, 2, 0x20);
    r.run_until_idle(500);
    assert_eq!(r.f.status(0), ERROR);
    assert!(r.f.irq());
    assert_eq!(r.port1_lengths().len(), 4);
    r.acrtc_finish();
    assert_eq!(r.p.acrtc.vram.0[0x100..0x105], [1, 2, 3, 4, 0]);
}

#[test]
fn count_smaller_than_the_rectangle_is_an_error_and_abt_clears_the_acrtc() {
    let mut r = Rig::new();
    staging_words(&mut r.p, 0x1000, &[1, 2, 3]);
    r.set_rwp(0x100);
    r.acrtc_cmd(&[DWT, 3, 0]);
    // COUNT＝3：3 語目が最後の要素なのに DONE が来ない
    r.channel(0, 1, 0x1000, 0, 3, 2, 0);
    r.run_until_idle(500);
    assert_eq!(r.f.status(0), ERROR);
    assert_eq!(r.p.acrtc.acrtc.dma_progress(), (3, 4));
    r.run(100);
    assert!(!r.p.acrtc.acrtc.idle(), "ACRTC は 4 語目を待ち続ける");
    // CPU が CCR の ABT で止める（DR-022）
    r.acrtc_reg(0x02, CCR_DMA | CCR_ABT);
    r.acrtc_reg(0x02, CCR_DMA);
    r.run(10);
    assert!(r.p.acrtc.acrtc.idle());
    assert!(!r.p.acrtc.acrtc.dreq() && !r.p.acrtc.rdy1());
}

#[test]
fn the_channel_stalls_until_the_acrtc_requests() {
    let mut r = Rig::new();
    staging_words(&mut r.p, 0x1000, &[7, 8]);
    r.channel(0, 1, 0x1000, 0, 2, 2, 0);
    r.run(30);
    assert_eq!(r.f.status(0), BUSY | STALL);
    assert!(r.trace.iter().all(|x| x.access.is_none()));
    r.set_rwp(0x200);
    r.acrtc_cmd(&[DWT, 1, 0]);
    r.run_until_idle(200);
    assert_eq!(r.f.status(0), DONE);
    r.acrtc_finish();
    assert_eq!(r.p.acrtc.vram.0[0x200..0x202], [7, 8]);
}

#[test]
fn stop_in_the_middle_of_a_port1_access_releases_dack() {
    let mut r = Rig::new();
    staging_words(&mut r.p, 0x1000, &[1, 2, 3, 4]);
    r.set_rwp(0x100);
    r.acrtc_cmd(&[DWT, 3, 0]);
    r.channel(0, 1, 0x1000, 0, 4, 2, 0);
    while !r.p.acrtc.dack() {
        r.step();
    }
    r.write(CTRL, 2);
    r.run(2);
    assert!(!r.p.acrtc.dack());
    assert_eq!(r.f.status(0), 0);
    // ACRTC は DACK＝H を見てサイクルを終える。FIFO に入った語は、DWT の準備（34 サイクル）の後に描く
    r.run(60);
    assert!(!r.p.acrtc.acrtc.dma_dtack());
    assert_eq!(r.p.acrtc.acrtc.dma_progress(), (1, 4));
    assert_eq!(r.p.acrtc.vram.0[0x100], 1);
}

#[test]
fn snapshot_round_trip_mid_port1_transfer() {
    let mut r = Rig::new();
    let words: Vec<u16> = (0..12).map(|i| 0x0F0F ^ i).collect();
    staging_words(&mut r.p, 0x1000, &words);
    r.p.acrtc.set_phase(1234);
    r.set_rwp(0x100);
    r.acrtc_cmd(&[DWT, 11, 0]);
    r.channel(0, 1, 0x1000, 0, 12, 2, 0);
    r.run(37);
    let saved = serde_json::to_string(&(&r.f, &r.p)).unwrap();
    let (f2, p2): (Fabric, FabricPorts) = serde_json::from_str(&saved).unwrap();
    let mut b = Rig {
        f: f2,
        p: p2,
        trace: Vec::new(),
    };
    for _ in 0..80 {
        assert_eq!(r.step(), b.step());
    }
    assert_eq!(r.f, b.f);
    assert_eq!(r.p, b.p);
    assert_eq!(r.f.status(0), DONE);
}
