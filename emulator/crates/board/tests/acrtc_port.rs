//! port 1（ACRTC の DMA）の試験（仕様書 §12 段階 2、DR-016、DR-022）。
//! 期待値は仕様書 §06、DR-022、`docs/parts/host-acrtc-interface.md` §2.3・§7 と、`fc85_board::acrtc` の
//! 模型の前提（ピンの遅れ）からの手計算で取る。

use fc85_board::dram::DramEnv;
use fc85_board::fabric::{Decision, Fabric, HostReply, Phase, PortReply, SlotRecord};
use fc85_board::ports::FabricPorts;
use fc85_core::time::{PERIOD_2CLK, PERIOD_16M, SLOT, Time};

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
    /// スロットごとの ACRTC の答え（P1A が同期化した DTACK を見た 16 MHz の縁。0〜3 が q0〜q3）。
    answers: Vec<Option<u8>>,
}

impl Rig {
    /// ACRTC：DMA を許し、画面 0 の MW を 0x40、STR＝1。
    fn new() -> Self {
        let mut r = Self {
            f: Fabric::new(),
            p: FabricPorts::new(),
            trace: Vec::new(),
            answers: Vec::new(),
        };
        r.acrtc_reg(0x02, CCR_DMA);
        r.acrtc_reg(0xC2, 0x40);
        r.acrtc_reg(0x04, 0xC000);
        r
    }

    fn step(&mut self) -> SlotRecord {
        let rec = self.f.step_slot(&mut self.p);
        self.trace.push(rec);
        let answered = self.p.acrtc.port_reply() != PortReply::Wait;
        self.answers
            .push(answered.then(|| self.p.acrtc.reply_edge()));
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
        self.port1_answers().iter().map(|a| a.0).collect()
    }

    /// port 1 のアクセスごとに（長さ、ACRTC が答えたのは何スロット目か、その縁、始めたスロット）。答えの縁が
    /// q0 ならそのスロットに、q1〜q3 なら P1A が預かって次のスロットに数える（fabric.md §03、DR-091）ので、
    /// 長さは答えたスロット＋（縁が q0 でなければ 1）。
    fn port1_answers(&self) -> Vec<(u64, u64, u8, u64)> {
        let mut out = Vec::new();
        let (mut n, mut at, mut start) = (0, None, 0);
        for (r, a) in self.trace.iter().zip(&self.answers) {
            if r.access.is_some_and(|a| a.port == 1) {
                if n == 0 {
                    start = r.slot;
                }
                n += 1;
                if let Some(q) = a {
                    assert!(at.is_none(), "1 回のアクセスに答えが 2 つ");
                    at = Some((n, *q));
                }
                if r.access_done {
                    let (k, q) = at.take().expect("答えのない port 1 のアクセスが終わった");
                    assert_eq!(n, k + (q > 0) as u64, "答えの縁 q{q}");
                    out.push((n, k, q, start));
                    n = 0;
                }
            }
        }
        out
    }

    /// 答えのスロットと縁が手計算（`answer_by_hand`）と同じか。`clk0` は 2CLK の最初の立上り、`stages` は
    /// 同期化の段。
    fn check_answers_by_hand(&self, clk0: Time, stages: u64) {
        for (_, k, q, start) in self.port1_answers() {
            assert_eq!(
                (k, q),
                answer_by_hand(start * SLOT, clk0, stages),
                "スロット {start} に始めたアクセス（{stages} 段）"
            );
        }
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

/// port 1 のアクセスを始めたスロットの頭 `t0` から、P1A が ACRTC の答え（同期化した DTACK）を見る 16 MHz の縁
/// までの手計算（emulator.md §03 の port 1 の時刻）。DACK は頭の 20 ns（302 単位）後に出る。ACRTC は 40 ns
/// （605）前の DACK を見る 2CLK の立上り（T1）の次の立上り（T3）の 70 ns（1058）後に DTACK を出す。ブリッジの
/// FF（74LS175）は 16 MHz の縁の 10 ns（151）前の値を取り、P1A は `stages` 段目の出力を次の縁で見る（0 段は
/// DTACK のピンを P1A の判定点 20 ns（302）前で見る）。2CLK の立上りは `clk0`＋k×PERIOD_2CLK。返す値は
/// （アクセスの何スロット目か、縁 q0〜q3）。
///
/// 2 段では、T1 は頭の 60 ns 後から 2CLK の 1 周期（186 ns）の間に来て、P1A が答えを見るのは 3 スロット目の
/// q0（T1 が頭から 109 ns まで）、q1（171 ns まで）、q2（234 ns まで）、q3（それより後）。
fn answer_by_hand(t0: Time, clk0: Time, stages: u64) -> (u64, u8) {
    let after = |t: Time, first: Time, p: Time| first + (t.saturating_sub(first)).div_ceil(p) * p;
    let t1 = after(t0 + 302 + 605, clk0, PERIOD_2CLK);
    let dtack = t1 + PERIOD_2CLK + 1058;
    let seen = if stages == 0 {
        after(dtack + 302, 0, PERIOD_16M)
    } else {
        after(dtack + 151, 0, PERIOD_16M) + stages * PERIOD_16M
    } - t0;
    (seen / SLOT + 1, (seen % SLOT / PERIOD_16M) as u8)
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
    // DACK を出してから同期化した DTACK を見るのは、いつも 3 スロット目（host-acrtc-interface.md §7.2 の計算を
    // 本機のスロットに当てはめたもの。emulator.md §03）。その縁が q0 なら 3 スロット、q1〜q3 なら次のスロットに
    // 数えて 4 スロット（DR-091）。6 回目（スロット 43 に始めた）だけ、T1 が頭から 238 ns で q3 になる
    r.check_answers_by_hand(0, 2);
    assert_eq!(r.port1_lengths(), vec![3, 3, 3, 3, 3, 4, 3, 3]);
    // Staging RAM は S のスロットでしか読めないので、要素は 6 か 8 スロットに 1 つ（4 スロットのアクセスは
    // S のスロットを待つ 8 スロットの間に入り、DR-091 の前と同じ）
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
    // Temporal RAM（逐次、S のスロットを待たない）から。位相をずらしても、要素は 6 か 7 スロットに 1 つ（次の
    // DREQ の同期化が、中ほどの取込みに間に合うかで決まる）。port 1 のアクセスは、答えの縁が 3 スロット目の q0 なら
    // 3 スロット、q1〜q3 なら 4 スロット（DR-091）。答えが遅い語は次の DREQ も遅く、DR-091 の前も 7 スロットだった
    // ので、要素の間隔はほぼ変わらない（位相 400 の初めの 3 語だけ 6 から 7 になる）
    for phase in (0..PERIOD_2CLK).step_by(400) {
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
        r.check_answers_by_hand(phase, 2);
        let l = r.port1_lengths();
        assert_eq!(l.len(), 16, "位相 {phase}");
        assert!(l.iter().all(|n| (3..=4).contains(n)), "位相 {phase}：{l:?}");
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

/// ベンド点「同期化の段」（DR-088）：段を飛ばすと、DACK から DTACK を見るまでが短くなる。語は正しく届く。
#[test]
fn fewer_sync_stages_shorten_port1_accesses() {
    let mut total = Vec::new();
    for stages in [2, 1, 0] {
        let mut r = Rig::new();
        r.p.acrtc.set_sync(DramEnv::default(), stages);
        for i in 0..16 {
            r.p.temporal.mem[i] = 0x30 + i as u8;
        }
        r.set_rwp(0x500);
        r.acrtc_cmd(&[DWT, 15, 0]);
        r.channel_format(7, 1, 0, 0, 16, 1, 0, 2);
        r.run_until_idle(500);
        assert_eq!(r.f.status(0), DONE, "{stages} 段");
        r.acrtc_finish();
        assert!(
            (0..16).all(|i| r.p.acrtc.vram.0[0x500 + i] == 0x30 + i as u16),
            "{stages} 段"
        );
        r.check_answers_by_hand(0, stages.into());
        let l = r.port1_lengths();
        assert_eq!(l.len(), 16);
        total.push(l.iter().sum::<u64>());
    }
    // 16 回のアクセスの計：2 段は 52 スロット（3 スロット目の q0 の答えが 12 回、q1・q3 が 4 回）、1 段と 0 段は
    // 49（答えが 2 スロット目の q2・q3 に早まっても、P1A が預かって 3 スロット目に数える。4 スロットは 1 回）。
    // DR-091 の前は、2 スロット目の答えをそのスロットに数え、48・43・45 だった
    assert_eq!(total, vec![52, 49, 49]);
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
        answers: Vec::new(),
    };
    for _ in 0..80 {
        assert_eq!(r.step(), b.step());
    }
    assert_eq!(r.f, b.f);
    assert_eq!(r.p, b.p);
    assert_eq!(r.f.status(0), DONE);
}
