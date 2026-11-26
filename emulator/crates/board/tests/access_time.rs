//! クロックを上げすぎたときの、メモリのアクセス時間の模型（DR-083、`access.rs`）の試験：DOC の波形 RAM、
//! ACRTC の VRAM、DSP のプログラム RAM。期待値は DR-083 の式（台帳 ES5503.md §3.5、review-digital.md §5.1、
//! TMS32010.md §3.2 と、MEM8505 p.112・p.192 の最大）と、チップの倍率（25 °C で中央 0.72、σ 0.06）から取る。

use fc85_board::access::{AccessTime, Path, slack_ns};
use fc85_board::acrtc::AcrtcBridge;
use fc85_board::console::System;
use fc85_board::doc::DocBoard;
use fc85_board::dram::{Dram, DramEnv};
use fc85_board::dsp::DspPort;
use fc85_board::vclock::TrackedVram;
use fc85_board::video::RASTER_W;
use fc85_chips::hd63484::Vram;
use fc85_chips::hd68000::{Bus, BusReply, Size, fc};
use fc85_core::time::{PERIOD_8M, PERIOD_21M, SLOT, Time, UNITS_PER_SECOND};

/// 周波数（Hz）の周期を時刻の単位で。
fn period(hz: f64) -> Time {
    (UNITS_PER_SECOND as f64 / hz).round() as Time
}

fn env(unit: u32, temp_c: i32) -> DramEnv {
    DramEnv { unit, temp_c }
}

/// クロック `hz` での間に合わない bit と際の bit。DOC は CLK、VRAM は映像系（2CLK はその 4 倍の周期）、DSP は
/// CLKIN（命令サイクルはその 4 倍）。
fn masks(path: Path, hz: f64, e: DramEnv) -> (u16, u16) {
    match path {
        Path::Wave => {
            let mut b = DocBoard::new();
            b.set_bends(Some((&Default::default(), e)), 0);
            b.set_clock_period(period(hz));
            b.wave_time().masks()
        }
        Path::VramRead | Path::VramWrite => {
            let mut a = AcrtcBridge::new();
            a.set_access_env(e);
            a.set_video_period(period(hz));
            let (r, w) = a.vram_time();
            if path == Path::VramRead {
                r.masks()
            } else {
                w.masks()
            }
        }
        Path::Prog => {
            let mut d = DspPort::new();
            d.set_access_env(e);
            d.set_input_period(4 * period(hz));
            d.prog_time().masks()
        }
    }
}

/// 最初に間に合わない bit が出る周波数（`lo` と `hi` の間で、1 kHz まで二分する）。
fn onset(path: Path, e: DramEnv, mut lo: f64, mut hi: f64) -> f64 {
    assert_eq!(masks(path, lo, e).0, 0);
    assert_ne!(masks(path, hi, e).0, 0);
    while hi - lo > 1e3 {
        let m = 0.5 * (lo + hi);
        if masks(path, m, e).0 == 0 {
            lo = m;
        } else {
            hi = m;
        }
    }
    hi
}

#[test]
fn factory_clocks_have_no_failing_bits_for_any_unit_or_temperature() {
    // 工場出荷の余り：DOC 150−75f、VRAM 96.2−60f、DSP 80−55f ns。f は 70 °C でも 0.97 以下
    for unit in 0..64 {
        for temp in [0, 70] {
            let mut s = System::new();
            s.bend("mode bend").unwrap();
            s.bend(&format!("dram unit {unit}")).unwrap();
            s.bend(&format!("dram temp {temp}")).unwrap();
            let p = &s.ports;
            let (r, w) = p.acrtc.vram_time();
            for (name, a) in [
                ("wave", p.doc_board.wave_time()),
                ("vram read", r),
                ("vram write", w),
                ("prog", p.dsp.prog_time()),
            ] {
                assert_eq!(a.masks(), (0, 0), "{name} 個体 {unit} {temp} °C");
            }
        }
    }
    // 通常モードではクロックのジャンパが入らないので、上げても何もない
    let mut s = System::new();
    for cmd in ["clock doc 20e6", "clock dsp 40e6", "clock video 50e6"] {
        s.bend(cmd).unwrap();
    }
    assert_eq!(s.ports.doc_board.wave_time().masks(), (0, 0));
    assert_eq!(s.ports.dsp.prog_time().masks(), (0, 0));
    assert_eq!(s.ports.acrtc.vram_time().0.masks(), (0, 0));
    // Bend モードに入れると効く
    s.bend("mode bend").unwrap();
    assert_eq!(s.ports.doc_board.wave_time().masks(), (0xFF, 0));
    assert_eq!(s.ports.dsp.prog_time().masks(), (0xFFFF, 0));
    assert_eq!(s.ports.acrtc.vram_time().0.masks(), (0xFFFF, 0));
}

#[test]
fn failing_bits_never_pass_again_as_the_clock_rises() {
    for unit in 0..8 {
        for (path, lo, step) in [
            (Path::Wave, 8e6, 25e3),
            (Path::VramRead, 21.47727e6, 100e3),
            (Path::VramWrite, 21.47727e6, 100e3),
            (Path::Prog, 20e6, 50e3),
        ] {
            let mut prev = 0u16;
            for k in 0..400 {
                let (fail, marg) = masks(path, lo + step * f64::from(k), env(unit, 25));
                assert_eq!(fail & prev, prev, "{path:?} 個体 {unit} {k}");
                assert_eq!(fail & marg, 0);
                prev = fail;
            }
            assert_ne!(prev, 0, "{path:?} 個体 {unit}");
        }
    }
}

#[test]
fn onsets_follow_the_datasheet_windows() {
    // 資料の最大の部品（倍率 1）と典型（0.72）で、余りが 0 になる周波数：
    // DOC 2T−100＝75f → 11.43／12.99 MHz、VRAM T2−90＝60f → 2CLK×4 で 26.67／30.03 MHz、
    // DSP tc−120＝55f → 22.86／25.06 MHz。個体 0・25 °C は、いちばん遅いチップで決まる
    let e = DramEnv::default();
    let doc = onset(Path::Wave, e, 8e6, 20e6);
    let vram = onset(Path::VramRead, e, 21.47727e6, 50e6);
    let dsp = onset(Path::Prog, e, 20e6, 40e6);
    println!("個体 0・25 °C：DOC {doc:.0} Hz、映像 {vram:.0} Hz、DSP {dsp:.0} Hz");
    assert!((10.5e6..15e6).contains(&doc), "{doc}");
    assert!((25e6..35e6).contains(&vram), "{vram}");
    assert!((21e6..28e6).contains(&dsp), "{dsp}");
    // 読みと書きの VRAM は同じ式（T2−90 ns に tCAC と tRSH がともに 60 ns）
    assert_eq!(onset(Path::VramWrite, e, 21.47727e6, 50e6), vram);
    // 個体で変わり、熱いほど下がる
    let others: Vec<f64> = (1..6)
        .map(|u| onset(Path::Wave, env(u, 25), 8e6, 20e6))
        .collect();
    assert!(others.iter().any(|&f| (f - doc).abs() > 50e3), "{others:?}");
    let hot = onset(Path::Wave, env(0, 70), 8e6, 20e6);
    assert!(hot < doc, "{hot} {doc}");
    // 余りの式：DOC のチップは tCAC で決まり、8 MHz で 150−75f、10 MHz で 100−75f
    let f = |p: f64| slack_ns(Path::Wave, period(p), 0, e);
    assert!((f(8e6) - f(10e6) - 50.0).abs() < 0.01);
}

/// 1 つのオシレータ（0 番）だけを、頁 `page` の 256 byte の表で、1 標本 1 byte で自走させる。
fn play(b: &mut DocBoard, page: u8) {
    for (reg, v) in [
        (0x00, 0x00),
        (0x20, 0x02),
        (0x40, 0xFF),
        (0x80, page),
        (0xC0, 0x00),
        (0xA0, 0x00),
        (0xE1, 0x00),
    ] {
        b.doc.write(reg, v);
    }
}

fn run_doc(b: &mut DocBoard, slots: u64) {
    for k in 0..slots {
        b.run_slot(k * SLOT, None);
    }
}

#[test]
fn an_overclocked_doc_reads_the_high_address_byte() {
    // 20 MHz（T 50 ns）では tCAC に使える時間が 2×50−100＝0 ns で、8 bit とも間に合わない。読んだ値は、
    // 同じアクセスで DOC が AD のバスに出した A8–A15（頁の番号）
    let mut b = DocBoard::new();
    b.wave[0x1200..0x1300].fill(0x80);
    b.set_clock_period(period(20e6));
    assert_eq!(b.wave_time().masks(), (0xFF, 0));
    play(&mut b, 0x12);
    run_doc(&mut b, 40);
    let o = b.last_osc.unwrap();
    assert_eq!((o.addr >> 8, o.wave), (0x12, 0x12));
    assert_eq!(b.doc.control(0) & 1, 0, "止まらない");
    // 頁 0 では 0x00 を読み、オシレータは止まる（0 は停止の印。台帳 §4.4）
    let mut z = DocBoard::new();
    z.wave[0..0x100].fill(0x80);
    z.set_clock_period(period(20e6));
    play(&mut z, 0x00);
    run_doc(&mut z, 40);
    assert_eq!(z.doc.control(0) & 1, 1, "H が立つ");
    // 一部の bit が間に合わない周波数では、間に合わない bit だけが頁の番号の bit（際の bit は読むたびに
    // 変わるので見ない）。頁 0xFF ならその bit は 1 で、0 にはならない
    let hz = (0..400)
        .map(|k| 11e6 + 10e3 * f64::from(k))
        .find(|&h| {
            let (fail, marg) = masks(Path::Wave, h, DramEnv::default());
            fail != 0 && fail | marg != 0xFF
        })
        .unwrap();
    let (fail, marg) = masks(Path::Wave, hz, DramEnv::default());
    let mut p = DocBoard::new();
    p.wave[0xFF00..].fill(0x80);
    p.set_clock_period(period(hz));
    play(&mut p, 0xFF);
    for k in 0..400 {
        p.run_slot(k * SLOT, None);
        // リセットの後の走査は 22 番から 31 番を回ってから 0 番に来る（台帳 ES5503 追補 2026-11-26）
        let Some(v) = p.last_osc.filter(|o| o.n == 0).map(|o| o.wave) else {
            assert!(k < 100, "0 番に来ない");
            continue;
        };
        assert_eq!(
            v & !marg as u8,
            (0x80 | fail as u8) & !marg as u8,
            "{hz} {fail:#x} {marg:#x}"
        );
    }
}

#[test]
fn vram_reads_float_high_and_writes_keep_the_old_bits() {
    let mut dram = Dram::new(128);
    let mut mem = vec![0x1234u16; 0x1_0000];
    let (mut r, mut w) = (AccessTime::default(), AccessTime::default());
    // 50 MHz（2CLK 80 ns）では T2−90 が負で、16 bit とも間に合わない
    r.set(Path::VramRead, 4 * period(50e6), DramEnv::default());
    w.set(Path::VramWrite, 4 * period(50e6), DramEnv::default());
    let mut v = TrackedVram {
        mem: &mut mem,
        dram: &mut dram,
        read: &mut r,
        write: &mut w,
        at: 0,
    };
    assert_eq!(v.read(5), 0xFFFF);
    v.write(5, 0xABCD);
    assert_eq!(mem[5], 0x1234);
    // 工場出荷の 2CLK ではそのまま
    let (mut r, mut w) = (AccessTime::default(), AccessTime::default());
    r.set(Path::VramRead, 4 * PERIOD_21M, DramEnv::default());
    w.set(Path::VramWrite, 4 * PERIOD_21M, DramEnv::default());
    let mut v = TrackedVram {
        mem: &mut mem,
        dram: &mut dram,
        read: &mut r,
        write: &mut w,
        at: 0,
    };
    v.write(5, 0xABCD);
    assert_eq!(v.read(5), 0xABCD);
}

/// 試験の CPU（video_clock.rs と同じ）：S0 の時刻を進めながらバスサイクルを出す。
struct Cpu {
    sys: System,
    clock: u64,
}

impl Cpu {
    fn ww(&mut self, addr: u32, v: u16) {
        let r = self
            .sys
            .write(self.clock, addr, fc::SUPERVISOR_DATA, Size::Word, v);
        match r {
            BusReply::Ok { waits, .. } => self.clock += 4 + u64::from(waits),
            r => panic!("{addr:#x}: {r:?}"),
        }
    }

    fn acrtc(&mut self, reg: u16, v: u16) {
        self.ww(0x20_0000, reg);
        self.ww(0x20_0002, v);
    }

    fn idle(&mut self, n: u64) {
        self.clock += n;
        self.sys.sync(self.clock);
    }

    /// 案B（台帳 HD63484.md §3.4）と描画の幅 64 語、表示優先（video.rs の試験と同じ）。パレットの色 i は
    /// 灰色 i×$111。
    fn case_b() -> Self {
        let mut c = Self {
            sys: System::new(),
            clock: 0,
        };
        for i in 0..16u32 {
            c.ww(0x27_0000 + 2 * i, (i * 0x111) as u16);
        }
        for (r, v) in [
            (0x82, 0xAA0D),
            (0x84, 0x127F),
            (0x86, 0x0106),
            (0x88, 0x2703),
            (0x8A, 0x00C0),
            (0xC2, 0x0040),
            (0xCA, 0x0040),
            (0xCC, 0x0000),
            (0xCE, 0x0000),
            (0x02, 0x0200),
            (0x04, 0xC008),
        ] {
            c.acrtc(r, v);
        }
        c
    }

    /// 1 フレーム（映像系 `hz` の 2CLK で 262×342）より長く待つ。
    fn frames(&mut self, n: u64, hz: f64) {
        let p2 = 4 * period(hz);
        self.idle(n * (262 * 342 * p2).div_ceil(PERIOD_8M));
    }

    /// 表示の画素（ライン 43〜234、ドット 64〜319）の色の番号（灰色 i×$111 の i）。
    fn shown(&self) -> Vec<u16> {
        let f = &self.sys.ports.acrtc.video.frame;
        let mut v = Vec::new();
        for y in 43..235 {
            for x in 64..320 {
                v.push(f[y * RASTER_W + x] / 0x111);
            }
        }
        v
    }
}

#[test]
fn an_overclocked_video_clock_shows_the_failing_vram_bits() {
    // VRAM が全部 0 の画面：工場出荷では色 0。50 MHz では表示の読出しの 16 bit とも間に合わず $FFFF で、
    // 色 15
    let mut c = Cpu::case_b();
    c.frames(2, 21.47727e6);
    assert!(c.shown().iter().all(|&i| i == 0));
    c.sys.bend("mode bend").unwrap();
    c.sys.bend("clock video 50e6").unwrap();
    c.frames(2, 50e6);
    assert!(c.shown().iter().all(|&i| i == 15));
    // 一部の bit が間に合わない周波数：語の bit k が 1、画素 j（左から）は bit 4j〜4j＋3。際の bit は読むたびに
    // 変わるので見ない
    let hz = (0..400)
        .map(|k| 25e6 + 25e3 * f64::from(k))
        .find(|&h| {
            let (fail, marg) = masks(Path::VramRead, h, DramEnv::default());
            fail != 0 && (fail | marg).count_ones() < 12
        })
        .unwrap();
    let (fail, marg) = masks(Path::VramRead, hz, DramEnv::default());
    println!("映像 {hz} Hz：fail {fail:#06x}、marg {marg:#06x}");
    let nib = |v: u16, j: usize| (v >> (4 * j)) & 15;
    c.sys.bend(&format!("clock video {hz}")).unwrap();
    c.frames(2, hz);
    for (n, &i) in c.shown().iter().enumerate() {
        let j = n % 4;
        assert_eq!(i & !nib(marg, j), nib(fail, j), "{hz} 画素 {n}");
    }
    // 書込み：CLR で $FFFF を画面に書くと、間に合わない bit は前の 0 のまま（読みと書きは同じ式 T2−90 ns で、
    // 同じ bit が間に合わない）
    assert_eq!(masks(Path::VramWrite, hz, DramEnv::default()), (fail, marg));
    c.acrtc(0x00, 0x5800); // CLR
    c.ww(0x20_0002, 0xFFFF);
    c.ww(0x20_0002, 63);
    c.ww(0x20_0002, (-191i16) as u16);
    c.frames(2, hz);
    let vram = &c.sys.ports.acrtc.vram.0;
    let bad = vram[..64 * 192]
        .iter()
        .position(|&v| v & !marg != !fail & !marg);
    assert_eq!(bad, None, "{:#x?}", bad.map(|i| vram[i]));
    assert!(vram[64 * 192..].iter().all(|&v| v == 0));
    // 戻すと、書けた bit だけが 1 で読める
    let written = vram[..64 * 192].to_vec();
    c.sys.bend("clock video off").unwrap();
    c.frames(2, 21.47727e6);
    for (n, &i) in c.shown().iter().enumerate() {
        let (y, x) = (n / 256, n % 256);
        assert_eq!(i, nib(written[y * 64 + x / 4], x % 4), "画素 {n}");
    }
}

#[test]
fn the_marginal_band_fizzes_and_survives_a_snapshot() {
    // DOC の個体 0 で、際の bit がある周波数を探す（余りが ±1.5 ns の中。ずっと間に合う bit もある）
    let e = DramEnv::default();
    let hz = (0..4000)
        .map(|k| 10e6 + 1e3 * f64::from(k))
        .find(|&h| masks(Path::Wave, h, e).1 != 0)
        .unwrap();
    let (fail, marg) = masks(Path::Wave, hz, e);
    println!("際：{hz} Hz で fail {fail:#04x}、marg {marg:#04x}");
    let mut a = AccessTime::default();
    a.set(Path::Wave, period(hz), e);
    // 読む値は 0、残りは 0xFF：際の bit は 0 と 1 の両方が出て、ほかの bit は決まった値
    let reads: Vec<u8> = (0..1000).map(|_| a.read8(0, || 0xFF)).collect();
    assert!(reads.iter().all(|&v| v & !(fail | marg) as u8 == 0));
    assert!(reads.iter().all(|&v| v & fail as u8 == fail as u8));
    let ones = reads.iter().filter(|&&v| v & marg as u8 != 0).count();
    let zeros = reads.iter().filter(|&&v| v & marg as u8 == 0).count();
    assert!(ones > 0 && zeros > 0, "{ones} {zeros}");
    // 保存して戻すと、同じ列が続く
    let saved = serde_json::to_string(&a).unwrap();
    let mut b: AccessTime = serde_json::from_str(&saved).unwrap();
    let x: Vec<u8> = (0..200).map(|_| a.read8(0, || 0xFF)).collect();
    let y: Vec<u8> = (0..200).map(|_| b.read8(0, || 0xFF)).collect();
    assert_eq!(x, y);
    // 子基板ごと：際の周波数で鳴らし、途中で保存して戻すと、同じ値を読み続ける
    let mut d = DocBoard::new();
    d.wave.fill(0x80);
    d.set_clock_period(period(hz));
    play(&mut d, 0xFF);
    run_doc(&mut d, 200);
    let saved = serde_json::to_string(&d).unwrap();
    let mut d2: DocBoard = serde_json::from_str(&saved).unwrap();
    for k in 200..2000 {
        d.run_slot(k * SLOT, None);
        d2.run_slot(k * SLOT, None);
        assert_eq!(d.last_osc, d2.last_osc);
    }
    assert_eq!(d, d2);
}
