//! 仕様書 §11 のベンド点「映像のクロック」（DR-035 (1)・(5)）の試験：2CLK と HSYNC、ACRTC のアクセスの
//! 時間切れ（DR-052）、Main RAM（DR-026）と VRAM の保持（DR-072）、機械の外のテレビの模型、保存と復元、
//! ホストの 1 フレーム。期待値は時刻の単位（1/15.12 GHz）での手計算で、式は各試験の注釈に書く。

use fc85_board::console::{BERR_CYCLE_CLOCKS, BERR_HALF_CLOCKS, Console, System, acrtc_cycle};
use fc85_board::doc::host_recognize;
use fc85_board::tv::{Chroma, Hsync, Tv};
use fc85_board::vclock::{discharged, main_row, vram_row};
use fc85_board::video::{RASTER_H, RASTER_W, rgb888};
use fc85_chips::hd68000::{Bus, BusReply, Size, fc};
use fc85_core::time::{PERIOD_2CLK, PERIOD_8M, PERIOD_21M};

/// 試験の CPU：S0 の時刻を進めながらバスサイクルを出す（console.rs の試験と同じ）。
struct Cpu {
    sys: System,
    clock: u64,
}

impl Cpu {
    fn new() -> Self {
        Self {
            sys: System::new(),
            clock: 0,
        }
    }

    fn done(&mut self, r: BusReply) -> BusReply {
        self.clock += match r {
            BusReply::Ok { waits, .. } => 4 + u64::from(waits),
            BusReply::BusError { clocks } => u64::from(clocks),
        };
        r
    }

    fn rd(&mut self, addr: u32) -> BusReply {
        let r = self
            .sys
            .read(self.clock, addr, fc::SUPERVISOR_DATA, Size::Word);
        self.done(r)
    }

    fn wr(&mut self, addr: u32, v: u16) -> BusReply {
        let r = self
            .sys
            .write(self.clock, addr, fc::SUPERVISOR_DATA, Size::Word, v);
        self.done(r)
    }

    fn rw(&mut self, addr: u32) -> u16 {
        match self.rd(addr) {
            BusReply::Ok { data, .. } => data,
            r => panic!("{addr:#x}: {r:?}"),
        }
    }

    fn ww(&mut self, addr: u32, v: u16) {
        match self.wr(addr, v) {
            BusReply::Ok { .. } => {}
            r => panic!("{addr:#x}: {r:?}"),
        }
    }

    fn acrtc(&mut self, reg: u16, v: u16) {
        self.ww(0x20_0000, reg);
        self.ww(0x20_0002, v);
    }

    /// 台帳 HD63484.md §3.4 の案B（256×192、4bpp、262 ライン）。
    fn acrtc_case_b(&mut self) {
        for (r, v) in CASE_B {
            self.acrtc(r, v);
        }
    }

    fn idle(&mut self, n: u64) {
        self.clock += n;
        self.sys.sync(self.clock);
    }

    fn bend(&mut self, cmd: &str) {
        self.sys.bend(cmd).unwrap();
    }
}

const CASE_B: [(u16, u16); 10] = [
    (0x82, 0xAA0D),
    (0x84, 0x127F),
    (0x86, 0x0106),
    (0x88, 0x2703),
    (0x8A, 0x00C0),
    (0xCA, 0x0040),
    (0xCC, 0x0000),
    (0xCE, 0x0000),
    (0x02, 0x2200),
    (0x04, 0xC008),
];

const BERR: BusReply = BusReply::BusError {
    clocks: BERR_CYCLE_CLOCKS,
};

/// 2CLK の周期 `p` で、1 フレーム（262 ライン×342 ドット）の CPU クロック数（切上げ）。
fn frame_clocks(p: u64) -> u64 {
    (262 * 342 * p).div_ceil(PERIOD_8M)
}

/// 1 ラインと 1 フレームの長さ（最後の 2 つの HSYNC と VSYNC の間）。
fn periods(c: &Cpu) -> (u64, u64) {
    let v = &c.sys.ports.acrtc.video;
    (v.hsync_at - v.hsync_prev, v.frame_at - v.frame_prev)
}

#[test]
fn the_video_clock_sets_the_2clk_and_the_hsync_period() {
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(2 * frame_clocks(PERIOD_2CLK));
    // 工場出荷：1 ライン＝342×2816＝963072、1 フレーム＝262×963072＝252324864（59.92 Hz）
    assert_eq!(periods(&c), (963_072, 252_324_864));
    // 外部クロック 10.738635 MHz：周期は 15.12e9÷10738635＝1408.0002 → 1408、2CLK はその 4 倍の 5632、
    // 1 ライン＝342×5632＝1926144、1 フレーム＝262×1926144＝504649728
    c.bend("mode bend");
    c.bend("clock video 10.738635e6");
    assert_eq!(c.sys.ports.acrtc.video_period(), 1408);
    assert_eq!(c.sys.ports.acrtc.period_2clk(), Some(5632));
    c.idle(3 * frame_clocks(5632));
    assert_eq!(periods(&c), (1_926_144, 504_649_728));
    // 通常モードではジャンパをつながない：基板の発振器に戻る
    c.bend("mode normal");
    assert_eq!(c.sys.ports.acrtc.period_2clk(), Some(PERIOD_2CLK));
    c.idle(3 * frame_clocks(PERIOD_2CLK));
    assert_eq!(periods(&c), (963_072, 252_324_864));
    // 止める（0）と 2CLK も HSYNC も来ない
    c.bend("mode bend");
    c.bend("clock video 0");
    assert_eq!(c.sys.ports.acrtc.period_2clk(), None);
    let lines = c.sys.ports.acrtc.video.lines;
    c.idle(frame_clocks(PERIOD_2CLK));
    assert_eq!(c.sys.ports.acrtc.video.lines, lines);
    // 戻すと続きから動く
    c.bend("clock video off");
    c.idle(3 * frame_clocks(PERIOD_2CLK));
    assert_eq!(periods(&c), (963_072, 252_324_864));
}

/// 2CLK の周期 `p` のとき、2CLK の位相を 1 単位ずつ変えた ACRTC のアクセスの待ちの最小・最大と、時間切れの数。
fn acrtc_waits(p: u64) -> (u32, u32, u64) {
    let berr = BERR_HALF_CLOCKS * PERIOD_8M / 2;
    let (mut lo, mut hi, mut out) = (u32::MAX, 0, 0);
    for edge in 0..p {
        let (_, dtack) = acrtc_cycle(0, edge, p);
        let (f, w) = host_recognize(0, dtack);
        if f >= berr {
            out += 1;
        } else {
            lo = lo.min(w);
            hi = hi.max(w);
        }
    }
    (lo, hi, out)
}

#[test]
fn acrtc_accesses_time_out_when_the_video_clock_is_slow() {
    // DR-052：S0 から 65.5 クロック（S0＋123795）より前の立下りで DTACK を取り込めなければバスエラー。
    // T1 は S0＋4687（2 クロック＋FF の 302＋CS のセットアップ 605）以後の最初の 2CLK の立上りで、DTACK を
    // 取り込めるのは T1＋p＋830（p は 2CLK の周期）から。取り込むのはその後の最初の立下り（S0＋945＋1890n）。
    // 最後に間に合う立下りは 64.5 クロック（S0＋121905）で、待ちは (121905−4725)÷1890＝62。
    // - いちばん早い T1 でも 4687＋p＋830 ≥ 121905、つまり p ≥ 116388（入力 ≤ 519.64 kHz）なら必ず時間切れ
    // - いちばん遅い T1（4687＋p−1）で 4687＋2p−1＋830 < 121905、つまり p ≤ 58194（入力 ≥ 1.0393 MHz）なら
    //   時間切れはない
    assert_eq!(acrtc_waits(PERIOD_2CLK), (2, 4, 0));
    assert_eq!(acrtc_waits(116_388).2, 116_388);
    // p＝116384：T1 がいちばん早い 4 通り（4687〜4690）だけ間に合い、待ちは 62
    assert_eq!(acrtc_waits(116_384), (62, 62, 116_384 - 4));
    assert_eq!(acrtc_waits(58_192).2, 0);
    assert!(acrtc_waits(58_196).2 > 0);
    // 1.1 MHz：周期 15.12e9÷1.1e6＝13745.45 → 13745、p＝54980。待ちは T1＝S0＋4687 で DTACK S0＋60497、
    // 立下り S0＋61425 で (61425−4725)÷1890＝30、T1＝S0＋59666 で DTACK S0＋115476、立下り S0＋116235 で 59
    assert_eq!(acrtc_waits(54_980), (30, 59, 0));

    // バスの上で
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(1000);
    c.bend("mode bend");
    c.bend("clock video 1.1e6");
    assert_eq!(c.sys.ports.acrtc.period_2clk(), Some(54_980));
    for _ in 0..40 {
        match c.rd(0x20_0000) {
            BusReply::Ok { waits, .. } => assert!((30..=59).contains(&waits), "{waits}"),
            r => panic!("{r:?}"),
        }
        c.clock += 1;
    }
    // 0.5 MHz（p＝120960 ≥ 116388）、1 Hz、止めた：どのアクセスもバスエラー（67 クロック）
    for hz in ["0.5e6", "1", "0"] {
        c.bend(&format!("clock video {hz}"));
        for _ in 0..8 {
            assert_eq!(c.rd(0x20_0000), BERR, "{hz}");
            assert_eq!(c.wr(0x20_0002, 0), BERR, "{hz}");
            c.clock += 3;
        }
    }
    // 戻すと 2〜4 ウェイト
    c.bend("clock video off");
    c.idle(10);
    for _ in 0..20 {
        match c.rd(0x20_0000) {
            BusReply::Ok { waits, .. } => assert!((2..=4).contains(&waits)),
            r => panic!("{r:?}"),
        }
    }
}

/// Main RAM（0x100000〜）を、各行が放電した値の逆になるように埋める（崩れたセルはどれも値が変わる）。
fn fill_main_ram(sys: &mut System) {
    for (i, b) in sys.main_ram.iter_mut().enumerate() {
        *b = !discharged(main_row(i / 2)) as u8;
    }
}

/// 行 `row` の 512 byte の 1 の数。
fn ones(ram: &[u8], row: usize) -> u32 {
    ram[row * 512..(row + 1) * 512]
        .iter()
        .map(|b| b.count_ones())
        .sum()
}

#[test]
fn main_ram_keeps_its_data_in_normal_mode_and_at_the_factory_clock() {
    // 通常モード：表示を始めず（HSYNC がなく、リフレッシュがない）、温度とクロックの設定もつながない。何も崩れない
    let mut c = Cpu::new();
    c.bend("dram temp 85");
    c.bend("clock video 0");
    assert!(!c.sys.main_dram.dram.tracking());
    fill_main_ram(&mut c.sys);
    let before = c.sys.main_ram.clone();
    c.idle(800_000);
    for row in 0..256u32 {
        c.rw(0x10_0000 + row * 512);
    }
    assert!(c.sys.main_ram == before);

    // Bend モード、工場出荷のクロック、70 °C：同じ行のリフレッシュの間は最大 26 ライン＝26×963072＝
    // 25039872（1.656 ms）で、70 °C の最短の保持 50 ms÷2^4.5＝2.21 ms より短い。崩れない
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(1000);
    c.bend("mode bend");
    c.bend("dram temp 70");
    assert!(c.sys.main_dram.dram.tracking());
    fill_main_ram(&mut c.sys);
    let before = c.sys.main_ram.clone();
    c.idle(400_000);
    assert!(c.sys.main_ram == before);
}

#[test]
fn main_ram_decays_when_the_video_clock_stops() {
    // 85 °C（保持は 25 °C の 1/64。最短 50 ms÷64＝0.78 ms）で映像のクロックを止め、HSYNC ごとのリフレッシュを
    // 止める。0.1 s は 25 °C の 6.4 s で、崩れる割合は 0.999×Φ(ln 6.4÷0.5)＋0.001×Φ(ln 32÷0.5)＝0.999897
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(1000);
    c.bend("mode bend");
    c.bend("dram temp 85");
    c.bend("clock video 0");
    fill_main_ram(&mut c.sys);
    let before = c.sys.main_ram.clone();
    // 行 2・3 は 0.5 ms ごとに CPU が読む（作業領域）。0.5 ms＜0.78 ms なので残る
    for _ in 0..200 {
        c.idle(4000);
        assert_eq!(c.rw(0x10_0000 + 2 * 512), 0xFFFF);
        assert_eq!(c.rw(0x10_0000 + 3 * 512), 0x0000);
    }
    // 行 4〜7 を読むと、行のまとまり（行 g と g＋128）が崩れる。RA0＝1 の行（補のセル）は 1、RA0＝0 の行
    // （真のセル）は 0 になる。残るのは 4096×0.000103≒0.4 bit
    for row in 4..8u32 {
        c.rd(0x10_0000 + row * 512);
    }
    let ram = &c.sys.main_ram;
    for row in [4, 6, 132, 134] {
        assert!(ones(ram, row) <= 6, "行 {row}: {}", ones(ram, row));
    }
    for row in [5, 7, 133, 135] {
        assert!(ones(ram, row) >= 4090, "行 {row}: {}", ones(ram, row));
    }
    for row in [2, 3, 130, 131] {
        assert_eq!(ones(ram, row), ones(&before, row), "行 {row}");
    }
    // 読み書きしていない行は、次に活性化されるまで値が変わらない（模型の約束）
    assert_eq!(ones(ram, 50), ones(&before, 50));

    // クロックを戻すと、HSYNC ごとのリフレッシュ（5 行）で全部の行が活性化され、崩れが現れる
    c.bend("clock video off");
    c.idle(2 * frame_clocks(PERIOD_2CLK));
    let changed: u32 = c
        .sys
        .main_ram
        .iter()
        .zip(&before)
        .map(|(a, b)| (a ^ b).count_ones())
        .sum();
    // 行 2・3・130・131 の 4×4096 bit を除く 1048576−16384＝1032192 bit の 99.99 %
    assert!((1_030_000..=1_032_192).contains(&changed), "{changed}");
}

#[test]
fn main_ram_loses_a_few_weak_cells_at_15_mhz_and_70_c() {
    // 15 MHz：周期 1008、1 ライン 342×4032＝1378944（91.2 µs）。同じ行のリフレッシュの間は 25 か 26 ライン、
    // 26 ラインは 35852544（2.371 ms）で、70 °C の最短 2.21 ms を超える。25 °C に直すと ×2^4.5 で 53.65 ms、
    // 崩れる割合は 0.999×Φ(ln 0.05365÷0.5)＋0.001×Φ(ln 0.2683÷0.5)＝4.2527e-6（25 ラインは 3.366e-6）。
    // 1048576 bit のうち崩れるのは平均 4.5 bit（弱いセルの裾だけ）。個体 1 では、分位点（dram.rs の hash）が
    // 4.2527e-6 より小さいセルは 5 つ（6.2e-7、2.7e-6、3.1e-6、3.5e-6、3.8e-6。次は 4.26e-6）。
    // 個体 0 は最小が 6.1e-6 で、1 つも崩れない
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(1000);
    c.bend("mode bend");
    c.bend("dram temp 70");
    c.bend("dram unit 1");
    c.bend("clock video 15e6");
    fill_main_ram(&mut c.sys);
    let before = c.sys.main_ram.clone();
    c.idle(160_000);
    let changed: u32 = c
        .sys
        .main_ram
        .iter()
        .zip(&before)
        .map(|(a, b)| (a ^ b).count_ones())
        .sum();
    assert_eq!(changed, 5);
}

#[test]
fn vram_decays_while_the_clock_is_stopped_and_the_picture_breaks() {
    let mut c = Cpu::new();
    // パレット：色 i は灰色 i×$111
    for i in 0..16u32 {
        c.ww(0x27_0000 + 2 * i, (i * 0x111) as u16);
    }
    // 各語を、行（語の番地の bit 0〜7）の放電した値の逆にする：偶数の語は $FFFF、奇数の語は $0000
    let pattern = |w: usize| !discharged(vram_row(w));
    for (w, v) in c.sys.ports.acrtc.vram.0.iter_mut().enumerate() {
        *v = pattern(w);
    }
    c.acrtc_case_b();
    c.idle(2 * frame_clocks(PERIOD_2CLK));
    let shown = c.sys.ports.acrtc.video.frame.clone();
    // 表示の左上（ライン 43、ドット 64〜）：語 0 は色 15 が 4 画素、語 1 は色 0
    assert_eq!(
        shown[43 * RASTER_W + 64..43 * RASTER_W + 72],
        [0xFFF, 0xFFF, 0xFFF, 0xFFF, 0, 0, 0, 0]
    );
    // 工場出荷のクロック、Bend モード、85 °C でも、リフレッシュ（1 ラインに 12 行、128 行を 10.7 ラインの
    // 0.68 ms）は最短 0.78 ms より速い。崩れない
    c.bend("mode bend");
    c.bend("dram temp 85");
    c.idle(2 * frame_clocks(PERIOD_2CLK));
    assert!((0..0x1_0000).all(|w| c.sys.ports.acrtc.vram.0[w] == pattern(w)));
    // 止めて 50 ms（25 °C の 3.2 s）：崩れる割合は 0.999×Φ(ln 3.2÷0.5)＋0.001×Φ(ln 16÷0.5)＝0.99000
    c.bend("clock video 0");
    c.idle(400_000);
    // 止めているあいだは、どの行も活性化されないので値は変わらない（模型の約束）
    assert!((0..0x1_0000).all(|w| c.sys.ports.acrtc.vram.0[w] == pattern(w)));
    // テレビ：同期がない。入力の今の値（画素）の輝度が一様に出る
    let tv = Tv::default();
    assert_eq!(tv.status(&c.sys).h, Hsync::None);
    let mut pic = Vec::new();
    tv.picture(&c.sys, &mut pic);
    assert!(pic.iter().all(|&p| p == pic[0]));
    // 戻すと、リフレッシュと表示の読出しが行を活性化し、ほぼ全部のセルが放電した値になる
    c.bend("clock video off");
    c.idle(2 * frame_clocks(PERIOD_2CLK));
    let vram = &c.sys.ports.acrtc.vram.0;
    let same: u32 = (0..0x1_0000)
        .map(|w| (!(vram[w] ^ discharged(vram_row(w)))).count_ones())
        .sum();
    let frac = f64::from(same) / f64::from(0x1_0000 * 16);
    assert!((0.985..=0.995).contains(&frac), "{frac}");
    // 画面は縞が反転する（表示の 256×192 のほとんどの画素が変わる）
    let now = &c.sys.ports.acrtc.video.frame;
    let mut differ = 0;
    for y in 43..235 {
        for x in 64..320 {
            differ += u32::from(now[y * RASTER_W + x] != shown[y * RASTER_W + x]);
        }
    }
    assert!(differ > 256 * 192 * 9 / 10, "{differ}");
}

#[test]
fn the_acrtc_refreshes_vram_while_the_display_is_stopped() {
    // STR＝0（表示を止めた）でも ACRTC はリフレッシュを続ける [V]：85 °C で 0.1 s 止めても崩れない
    let mut c = Cpu::new();
    for (w, v) in c.sys.ports.acrtc.vram.0.iter_mut().enumerate() {
        *v = !discharged(vram_row(w));
    }
    c.acrtc_case_b();
    c.idle(1000);
    c.bend("mode bend");
    c.bend("dram temp 85");
    c.acrtc(0x04, 0x8008);
    c.idle(800_000);
    assert!(!c.sys.ports.acrtc.acrtc.display_running());
    assert!((0..0x1_0000).all(|w| c.sys.ports.acrtc.vram.0[w] == !discharged(vram_row(w))));
}

/// 0RGB の u32（`rgb888` そのまま）。
fn plain(c: u16) -> u32 {
    let [r, g, b] = rgb888(c);
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

/// 色のついた画面（VRAM の語 i は画素 4 つ、パレットは 16 色）。
fn colorful() -> Cpu {
    let mut c = Cpu::new();
    for (i, rgb) in [
        0x000, 0xF00, 0x0F0, 0x00F, 0xFF0, 0x0FF, 0xF0F, 0xFFF, 0x800, 0x080, 0x008, 0x888, 0xF80,
        0x8F0, 0x08F, 0x444,
    ]
    .into_iter()
    .enumerate()
    {
        c.ww(0x27_0000 + 2 * i as u32, rgb);
    }
    for (w, v) in c.sys.ports.acrtc.vram.0.iter_mut().enumerate() {
        *v = (w as u16).wrapping_mul(0x1357);
    }
    c.acrtc_case_b();
    c.idle(2 * frame_clocks(PERIOD_2CLK));
    c
}

#[test]
fn the_tv_shows_the_frame_as_is_at_the_factory_clock() {
    let c = colorful();
    let tv = Tv::default();
    let st = tv.status(&c.sys);
    assert!(st.normal(), "{st:?}");
    // 工場出荷：ライン 15.12e9÷(342×2816)＝15699.76 Hz、フィールド÷262＝59.923 Hz
    assert!((st.line_hz - 15_699.76).abs() < 0.01);
    assert!((st.field_hz - 59.923).abs() < 0.001);
    let mut pic = Vec::new();
    tv.picture(&c.sys, &mut pic);
    let f = &c.sys.ports.acrtc.video.frame;
    assert_eq!(pic.len(), RASTER_W * RASTER_H);
    assert!(pic.iter().zip(f).all(|(&p, &v)| p == plain(v)));
}

/// 映像のクロックを `hz` にしたテレビの状態。
fn status_at(c: &mut Cpu, hz: &str) -> fc85_board::tv::Status {
    c.bend("mode bend");
    c.bend(&format!("clock video {hz}"));
    c.idle(2000);
    Tv::default().status(&c.sys)
}

#[test]
fn the_tv_pulls_in_and_loses_lock() {
    // テレビ [P][V]：水平は 15699.76 Hz の ±500 Hz、垂直は 55〜66 Hz、色は 3579545.45 Hz の ±500 Hz で
    // 引き込み、唸りが 300 Hz を超えると色を消す。ライン＝入力÷1368（342 ドット×4）、フィールド＝÷262、
    // 色副搬送波＝÷6
    let mut c = colorful();
    // 20.7943 MHz：ライン 15200.51 Hz（−499.25 Hz）で水平は引き込む。横のずれは −499.25÷500×4 µs を
    // ドット（4÷20.7943e6）で割り −20.77 → −21。色副搬送波の差は −112 kHz で色を消す
    let st = status_at(&mut c, "20.7943e6");
    assert_eq!(st.h, Hsync::Locked { shift: -21 });
    assert!(st.v_locked);
    assert_eq!(st.chroma, Chroma::Killed);
    // 20.79 MHz：ライン 15197.37 Hz（−502.39 Hz）で外れる
    assert_eq!(status_at(&mut c, "20.79e6").h, Hsync::Free);
    // 21 MHz：ライン 15350.88 Hz（−348.9 Hz）、ずれ −348.9÷500×4e-6×5.25e6＝−14.65 → −15
    let st = status_at(&mut c, "21e6");
    assert_eq!(st.h, Hsync::Locked { shift: -15 });
    // 垂直：55 Hz はライン 14410 Hz、入力 19.71288 MHz。19.72 MHz は 55.02 Hz で引き込み、19.70 MHz は
    // 54.96 Hz で流れる。66 Hz は入力 23.6556 MHz。23.65 MHz は 65.98 Hz、23.67 MHz は 66.04 Hz
    for (hz, locked) in [
        ("19.72e6", true),
        ("19.70e6", false),
        ("23.65e6", true),
        ("23.67e6", false),
    ] {
        let st = status_at(&mut c, hz);
        assert_eq!(st.v_locked, locked, "{hz}: {st:?}");
        assert_eq!(st.h, Hsync::Free, "{hz}");
    }
    // 色：21.4757 MHz は副搬送波 3579283.33 Hz（−262.12 Hz）。色相は asin(−262.12÷500)＝−31.62°
    let st = status_at(&mut c, "21.4757e6");
    assert_eq!(st.h, Hsync::Locked { shift: 0 });
    match st.chroma {
        Chroma::Locked { hue } => assert!((hue.to_degrees() + 31.62).abs() < 0.01, "{hue}"),
        x => panic!("{x:?}"),
    }
    // 21.4743 MHz：3579050 Hz（−495.45 Hz）、asin(−0.99091)＝−82.27°
    match status_at(&mut c, "21.4743e6").chroma {
        Chroma::Locked { hue } => assert!((hue.to_degrees() + 82.27).abs() < 0.01, "{hue}"),
        x => panic!("{x:?}"),
    }
    // 21.4742 MHz：3579033.33 Hz（−512.12 Hz）で外れ、唸りは √(512.12²−500²)＝110.76 Hz
    match status_at(&mut c, "21.4742e6").chroma {
        Chroma::Beat { hz } => assert!((hz + 110.76).abs() < 0.01, "{hz}"),
        x => panic!("{x:?}"),
    }
    // 21.474 MHz：−545.45 Hz、唸り √(545.45²−500²)＝217.99 Hz
    match status_at(&mut c, "21.474e6").chroma {
        Chroma::Beat { hz } => assert!((hz + 217.99).abs() < 0.01, "{hz}"),
        x => panic!("{x:?}"),
    }
    // 21.47 MHz：−1212.1 Hz、唸り 1104 Hz＞300 Hz で色を消す
    assert_eq!(status_at(&mut c, "21.47e6").chroma, Chroma::Killed);
    // 2 MHz：ライン 1461.99 Hz（684 µs）は自走の 10 倍（636.9 µs）より長く、同期として分けられない
    let st = status_at(&mut c, "2e6");
    assert_eq!(st.h, Hsync::None);
    assert!((st.line_hz - 1461.99).abs() < 0.01);
    // 止めた
    let st = status_at(&mut c, "0");
    assert_eq!((st.h, st.line_hz), (Hsync::None, 0.0));
    // 戻すと元どおり
    assert!(status_at(&mut c, "off").normal());
}

#[test]
fn the_tv_pictures_follow_the_lock() {
    let tv = Tv::default();
    let mut pic = Vec::new();
    let gray = |p: u32| (p >> 16) & 0xFF == p & 0xFF && (p >> 8) & 0xFF == p & 0xFF;
    // 21 MHz：水平は −15 ドットずれて引き込み、色は消える。テレビの横 x はフレームのドット x＋15
    let mut c = colorful();
    status_at(&mut c, "21e6");
    c.idle(2 * frame_clocks(4 * 720));
    tv.picture(&c.sys, &mut pic);
    let f = &c.sys.ports.acrtc.video.frame;
    assert!(pic.iter().all(|&p| gray(p)));
    for y in 0..RASTER_H {
        for x in 0..RASTER_W - 15 {
            assert_eq!(
                pic[y * RASTER_W + x] != 0,
                f[y * RASTER_W + x + 15] != 0,
                "({x}, {y})"
            );
        }
    }
    // 21.4757 MHz：色相だけが −31.6° 回る。灰色は灰色のまま、色のある画素は変わる
    status_at(&mut c, "21.4757e6");
    tv.picture(&c.sys, &mut pic);
    let f = &c.sys.ports.acrtc.video.frame;
    let mut turned = 0;
    for (&p, &v) in pic.iter().zip(f) {
        if gray(plain(v)) {
            assert_eq!(p, plain(v));
        } else {
            turned += u32::from(p != plain(v));
        }
    }
    assert!(turned > 10_000, "{turned}");
    // 20.5 MHz：水平が外れ、色が消え、絵は斜めに裂ける（同じ行に複数のラインが入る）
    status_at(&mut c, "20.5e6");
    tv.picture(&c.sys, &mut pic);
    assert!(pic.iter().all(|&p| gray(p)));
    assert_ne!(
        pic,
        c.sys
            .ports
            .acrtc
            .video
            .frame
            .iter()
            .map(|&v| plain(v))
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_tv_rolls_when_the_field_is_too_slow() {
    // 工場出荷のクロックで VC を 300 ラインにする：フィールド 15699.76÷300＝52.33 Hz＜55 Hz で垂直が流れる。
    // 水平と色はそのまま
    let mut c = colorful();
    c.acrtc(0x86, 300);
    c.idle(2 * frame_clocks(PERIOD_2CLK) * 300 / 262);
    assert_eq!(c.sys.ports.acrtc.acrtc.frame().rasters, 300);
    let st = Tv::default().status(&c.sys);
    assert_eq!(st.h, Hsync::Locked { shift: 0 });
    assert!(!st.v_locked);
    assert_eq!(st.chroma, Chroma::Locked { hue: 0.0 });
    // 絵はフレームを縦に回したもの：ある `vs` について、テレビの行 y はフレームの行 (y＋vs) mod 300
    let mut pic = Vec::new();
    Tv::default().picture(&c.sys, &mut pic);
    let f = &c.sys.ports.acrtc.video.frame;
    let row = |r: usize| -> Vec<u32> {
        if r < RASTER_H {
            f[r * RASTER_W..(r + 1) * RASTER_W]
                .iter()
                .map(|&v| plain(v))
                .collect()
        } else {
            vec![0; RASTER_W]
        }
    };
    let fits = |vs: usize| {
        (0..RASTER_H).all(|y| pic[y * RASTER_W..(y + 1) * RASTER_W] == row((y + vs) % 300)[..])
    };
    let vs: Vec<usize> = (0..300).filter(|&vs| fits(vs)).collect();
    assert!(!vs.is_empty());
}

#[test]
fn snapshots_keep_the_video_clock_and_the_decay_state() {
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(1000);
    c.bend("mode bend");
    c.bend("dram temp 70");
    c.bend("clock video 15e6");
    fill_main_ram(&mut c.sys);
    c.idle(30_000);
    let json = serde_json::to_string(&c.sys).unwrap();
    let mut back: System = serde_json::from_str(&json).unwrap();
    assert_eq!(back.ports.acrtc.video_period(), 1008);
    let t = c.clock + 60_000;
    c.sys.sync(t);
    back.sync(t);
    // 保存に入る状態がすべて同じ（画面の配列と音声の出力は保存に入れない）
    assert!(serde_json::to_value(&c.sys).unwrap() == serde_json::to_value(&back).unwrap());
    assert!(c.sys.main_dram == back.main_dram && c.sys.ports.acrtc.dram == back.ports.acrtc.dram);
    assert!(c.sys.main_ram == back.main_ram);

    // 映像のクロックの前の保存（新しい欄がない）も読める：基板の発振器、追跡なし
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(5000);
    let mut v: serde_json::Value = serde_json::to_value(&c.sys).unwrap();
    v.as_object_mut().unwrap().remove("main_dram");
    let a = v["ports"]["acrtc"].as_object_mut().unwrap();
    a.remove("period21");
    a.remove("dram");
    let vid = a["video"].as_object_mut().unwrap();
    for k in ["lines", "hsync_prev", "frame_at", "frame_prev"] {
        vid.remove(k);
    }
    v["bends"]["clocks"]
        .as_object_mut()
        .unwrap()
        .remove("video");
    let mut old: System = serde_json::from_value(v).unwrap();
    assert_eq!(old.ports.acrtc.video_period(), PERIOD_21M);
    assert!(!old.main_dram.dram.tracking() && !old.ports.acrtc.dram.tracking());
    let t = c.clock + 20_000;
    c.sys.sync(t);
    old.sync(t);
    assert_eq!(old.ports.acrtc.video.raster, c.sys.ports.acrtc.video.raster);
}

/// Boot ROM：案B を ACRTC に書き（MOVE.W #imm,(abs).L＝$33FC）、その場で回る（BRA.S *）。
fn tiny_console() -> Console {
    let mut w: Vec<u16> = vec![0x0012, 0x0000, 0x0000, 0x0400];
    w.resize(0x200, 0);
    for (r, v) in CASE_B {
        w.extend([0x33FC, r, 0x0020, 0x0000, 0x33FC, v, 0x0020, 0x0002]);
    }
    w.push(0x60FE);
    let mut c = Console::new();
    for (i, x) in w.iter().enumerate() {
        c.sys.boot_rom[2 * i..2 * i + 2].copy_from_slice(&x.to_be_bytes());
    }
    c.power_on();
    c
}

#[test]
fn run_frame_keeps_the_host_going_when_the_video_clock_is_slow_or_stopped() {
    // `run_frame` は ACRTC のフレームの終わりまで。上限は CPU の 2 フレーム分（8e6÷60×2）だが、映像の
    // クロックが基板の半分より遅いか止まっていれば 1 フレーム分（133333 クロック）で返る
    const FRAME: u64 = 8_000_000 / 60;
    let mut c = tiny_console();
    for _ in 0..3 {
        c.run_frame();
    }
    assert!(c.sys.ports.acrtc.acrtc.display_running());
    let step = |c: &mut Console| {
        let (f, t) = (c.sys.ports.acrtc.video.frames, c.cpu.clock);
        c.run_frame();
        (c.sys.ports.acrtc.video.frames - f, c.cpu.clock - t)
    };
    let (n, dt) = step(&mut c);
    assert_eq!(n, 1);
    assert!(dt <= frame_clocks(PERIOD_2CLK) + 80, "{dt}");
    c.sys.bend("mode bend").unwrap();
    // 2 MHz（1 フレーム 0.179 s）、1 Hz、止めた：どれも約 1/60 秒で返る
    for hz in ["2e6", "1", "0"] {
        c.sys.bend(&format!("clock video {hz}")).unwrap();
        let (n, dt) = step(&mut c);
        assert_eq!(n, 0, "{hz}");
        assert!((FRAME..FRAME + 20).contains(&dt), "{hz}: {dt}");
    }
    c.sys.bend("clock video off").unwrap();
    let (n, _) = step(&mut c);
    assert_eq!(n, 1);
}
