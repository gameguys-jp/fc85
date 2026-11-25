//! 本体のバス（段階 3）の試験。68000 のコアを使わず、試験がバスサイクルを直接出す。
//! 期待値は仕様書 §05・§09、DR-016、DR-024、DR-026、DR-027、DR-028、DR-052、DR-053 と、
//! 台帳 HD68000.md §2.5、host-acrtc-interface.md §5、HD63484.md §3.4・§4.5 からの手計算で取る。

use fc85_board::console::{BERR_CYCLE_CLOCKS, System, acrtc_timing};
use fc85_board::video::{RASTER_W, VIRQ_VBL, VIRQ_VBL_EN};
use fc85_chips::hd68000::{Bus, BusReply, IackReply, Size, fc};
use fc85_core::time::{PERIOD_2CLK, PERIOD_8M};

/// 試験の CPU：S0 の時刻を進めながらバスサイクルを出す。
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

    fn rd(&mut self, addr: u32, size: Size) -> BusReply {
        let r = self.sys.read(self.clock, addr, fc::SUPERVISOR_DATA, size);
        self.done(r)
    }

    fn wr(&mut self, addr: u32, size: Size, v: u16) -> BusReply {
        let r = self
            .sys
            .write(self.clock, addr, fc::SUPERVISOR_DATA, size, v);
        self.done(r)
    }

    fn rw(&mut self, addr: u32) -> u16 {
        match self.rd(addr, Size::Word) {
            BusReply::Ok { data, .. } => data,
            r => panic!("{addr:#x}: {r:?}"),
        }
    }

    fn ww(&mut self, addr: u32, v: u16) {
        match self.wr(addr, Size::Word, v) {
            BusReply::Ok { .. } => {}
            r => panic!("{addr:#x}: {r:?}"),
        }
    }

    /// ACRTC のレジスタに書く（AR、データ）。
    fn acrtc(&mut self, reg: u16, v: u16) {
        self.ww(0x20_0000, reg);
        self.ww(0x20_0002, v);
    }

    /// 台帳 HD63484.md §3.4 の案B（256×192、4bpp、262 ライン）。
    fn acrtc_case_b(&mut self) {
        for (r, v) in [
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
        ] {
            self.acrtc(r, v);
        }
    }

    /// `n` クロック何もしない。
    fn idle(&mut self, n: u64) {
        self.clock += n;
        self.sys.sync(self.clock);
    }
}

const BERR: BusReply = BusReply::BusError {
    clocks: BERR_CYCLE_CLOCKS,
};

/// 1 フレーム（262 ライン×342 ドット）の CPU クロック数（切上げ）。
fn frame_clocks() -> u64 {
    (262 * 342 * PERIOD_2CLK).div_ceil(PERIOD_8M)
}

#[test]
fn memories_decode_to_the_address_map() {
    let mut c = Cpu::new();
    c.sys.boot_rom[0..4].copy_from_slice(&[0x12, 0x34, 0x56, 0x78]);
    c.sys.ports.char_rom[1] = 0xA5;
    // Boot ROM：1 ウェイト（DR-026）。書込みは捨てて DTACK を返す
    assert_eq!(
        c.rd(0, Size::Word),
        BusReply::Ok {
            data: 0x1234,
            waits: 1
        }
    );
    assert_eq!(
        c.rd(3, Size::Byte),
        BusReply::Ok {
            data: 0x78,
            waits: 1
        }
    );
    assert!(matches!(c.wr(0, Size::Word, 0), BusReply::Ok { .. }));
    assert_eq!(c.sys.boot_rom[0], 0x12);
    // 文字 ROM：偶数番地の上位レーンに 32 KiB。1 ウェイト（DR-031）
    assert_eq!(
        c.rd(0x01_0002, Size::Byte),
        BusReply::Ok {
            data: 0xA5,
            waits: 1
        }
    );
    assert_eq!(
        c.rd(0x01_0002, Size::Word),
        BusReply::Ok {
            data: 0xA5FF,
            waits: 1
        }
    );
    assert_eq!(
        c.rd(0x01_0003, Size::Byte),
        BusReply::Ok {
            data: 0xFF,
            waits: 1
        }
    );
    // Main RAM：big-endian、バイトとワード
    c.ww(0x10_0000, 0xBEEF);
    assert!(matches!(
        c.wr(0x10_0003, Size::Byte, 0x42),
        BusReply::Ok { .. }
    ));
    assert_eq!(c.rw(0x10_0000), 0xBEEF);
    assert_eq!(c.rw(0x10_0002), 0x0042);
    assert_eq!(c.sys.main_ram[0..4], [0xBE, 0xEF, 0x00, 0x42]);
    // Staging RAM は Fabric の port 0 と同じ配列
    c.ww(0x12_0010, 0x1357);
    assert_eq!(c.sys.ports.staging[0x10..0x12], [0x13, 0x57]);
    // 割り当てのない番地、カートリッジがないときの窓、16 bit のレジスタのバイト幅はバスエラー
    for a in [
        0x02_0000, 0x13_0000, 0x29_0000, 0x40_0000, 0x50_0000, 0xFF_FFFE,
    ] {
        assert_eq!(c.rd(a, Size::Word), BERR, "{a:#x}");
    }
    for a in [
        0x20_0000, 0x22_0000, 0x23_0000, 0x24_0000, 0x25_0000, 0x26_0000, 0x27_0000,
    ] {
        assert_eq!(c.rd(a, Size::Byte), BERR, "{a:#x}");
    }
    assert_eq!(c.sys.log.bus_errors, 13);
    // カートリッジ：1 ウェイト
    c.sys.cart = Some(vec![0xCA, 0xFE, 0xBA, 0xBE]);
    assert_eq!(
        c.rd(0x40_0002, Size::Word),
        BusReply::Ok {
            data: 0xBABE,
            waits: 1
        }
    );
    assert_eq!(
        c.rd(0x40_0004, Size::Word),
        BusReply::Ok {
            data: 0xFFFF,
            waits: 1
        }
    );
}

#[test]
fn staging_waits_follow_the_500ns_frame() {
    // DR-024：S0 の 4 クロックの枠の中の位置で 0、0、2、1（平均 0.75）
    let mut c = Cpu::new();
    let mut seen = Vec::new();
    for _ in 0..8 {
        let at = c.clock;
        match c.rd(0x12_0000, Size::Word) {
            BusReply::Ok { waits, .. } => seen.push((at % 4, waits)),
            r => panic!("{r:?}"),
        }
        c.clock += 1;
    }
    for (p, w) in seen {
        assert_eq!(w, [0, 0, 2, 1][p as usize], "位相 {p}");
    }
}

#[test]
fn vpa_cycles_take_10_to_19_clocks() {
    // 台帳 HD68000.md §2.5：E の位相 10 通りで 10〜19 クロックが 1 回ずつ
    let mut totals: Vec<u32> = (0..10).map(|at| 4 + System::vpa_waits(at)).collect();
    totals.sort();
    assert_eq!(totals, (10..=19).collect::<Vec<_>>());
    // IACK もオートベクタの VPA サイクル
    let mut s = System::new();
    assert_eq!(
        s.iack(3, 6),
        IackReply::Autovector {
            waits: System::vpa_waits(3)
        }
    );
}

#[test]
fn acrtc_accesses_take_2_to_4_waits() {
    // host-acrtc-interface.md §5.2：S4 で CS を登録し DTACK を直結すると 2〜4 ウェイト。中央の遅れの
    // 条件で 2：5%、3：67%、4：28%（2CLK の位相が一様のとき）。模型は FF の遅れを 20 ns としたので
    // 2 が 6%、4 が 27% になる
    let mut hist = [0u32; 8];
    for phase in 0..PERIOD_2CLK {
        let (_, w) = acrtc_timing(0, phase);
        hist[w as usize] += 1;
    }
    assert_eq!(hist, [0, 0, 172, 1890, 754, 0, 0, 0]);
    // バスの上でも 2〜4
    let mut c = Cpu::new();
    for _ in 0..50 {
        match c.rd(0x20_0000, Size::Word) {
            BusReply::Ok { waits, .. } => assert!((2..=4).contains(&waits)),
            r => panic!("{r:?}"),
        }
        c.clock += 1;
    }
    // SR（上位バイトは 1）。初期は CED・WFR・WFE
    assert_eq!(c.rw(0x20_0000), 0xFF23);
}

#[test]
fn the_display_shows_vram_through_the_palette() {
    let mut c = Cpu::new();
    // 語 0 の画素は LSB の側から 1、2、3、4（台帳 §4.5）
    c.sys.ports.acrtc.vram.0[0] = 0x4321;
    for (i, rgb) in [(0, 0x000), (1, 0xF00), (2, 0x0F0), (3, 0x00F), (4, 0xFFF)] {
        c.ww(0x27_0000 + 2 * i, rgb);
    }
    c.acrtc_case_b();
    c.idle(2 * frame_clocks());
    let v = &c.sys.ports.acrtc.video;
    assert!(v.frames >= 1);
    // 表示の始まり：HSW 13＋HDS 19＝32 メモリサイクル＝64 ドット、VSW 3＋VDS 40＝43 ライン（台帳 §3.4）
    let f = c.sys.ports.acrtc.acrtc.frame();
    assert_eq!((f.h_start, f.h_end, f.v_start, f.v_end), (64, 320, 43, 235));
    let row = 43 * RASTER_W;
    assert_eq!(v.frame[row + 63], 0, "表示の左はブランキング");
    assert_eq!(v.frame[row + 64..row + 68], [0xF00, 0x0F0, 0x00F, 0xFFF]);
    assert_eq!(v.frame[row + 68], 0x000);
    assert_eq!(v.frame[42 * RASTER_W + 64], 0, "表示の上はブランキング");
}

#[test]
fn vblank_raises_level_6_and_the_cpu_clears_it() {
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.ww(0x27_0022, VIRQ_VBL_EN);
    c.idle(frame_clocks());
    assert_eq!(c.sys.ipl(c.clock), 6);
    assert_ne!(c.rw(0x27_0022) & VIRQ_VBL, 0);
    c.ww(0x27_0022, VIRQ_VBL | VIRQ_VBL_EN);
    assert_eq!(c.sys.ipl(c.clock), 0);
    // 次の VBlank で、また立つ
    c.idle(frame_clocks());
    assert_eq!(c.sys.ipl(c.clock), 6);
    // LINE は今のラスタ
    let line = c.rw(0x27_0024);
    assert!(line < 262);
}

#[test]
fn palette_writes_wait_for_the_copy_in_hsync() {
    // DR-027：HSYNC の間の写し（3.0 µs）と重なった書込みは、写しが終わるまで待つ
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(1000);
    let mut max = 0;
    for _ in 0..600 {
        if let BusReply::Ok { waits, .. } = c.wr(0x27_0002, Size::Word, 0x123) {
            max = max.max(waits);
        }
    }
    assert!((20..=24).contains(&max), "{max}");
    // DIRECT では待たない
    c.ww(0x27_0020, 1);
    for _ in 0..600 {
        assert_eq!(
            c.wr(0x27_0002, Size::Word, 0x123),
            BusReply::Ok { data: 0, waits: 0 }
        );
    }
}

#[test]
fn main_ram_waits_for_the_refresh() {
    // DR-026：HSYNC ごとの 5 行のリフレッシュ（12.5 クロック）。重なると最大約 12 クロック待つ
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.idle(1000);
    let mut max = 0;
    let mut waited = 0;
    for _ in 0..2000 {
        if let BusReply::Ok { waits, .. } = c.rd(0x10_0000, Size::Word) {
            max = max.max(waits);
            waited += u32::from(waits > 0);
        }
    }
    assert!((10..=12).contains(&max), "{max}");
    assert!(waited > 0);
}

#[test]
fn the_acia_receives_midi_and_raises_level_5() {
    // DR-028：0x03（マスタリセット）、0x95（÷16、8N1、受信割込み）
    let mut c = Cpu::new();
    assert!(matches!(
        c.wr(0x26_0014, Size::Byte, 0x03),
        BusReply::Ok { .. }
    ));
    assert!(matches!(
        c.wr(0x26_0014, Size::Byte, 0x95),
        BusReply::Ok { .. }
    ));
    c.sys.acia.receive(0x90);
    // 1 byte は 320 µs＝2560 クロック
    c.idle(2500);
    assert_eq!(c.sys.ipl(c.clock), 0);
    c.idle(100);
    assert_eq!(c.sys.ipl(c.clock), 5);
    // INPUT_IRQ_STATUS の MIDI ビットは IRQ̄ の写し
    assert_eq!(c.rw(0x26_0018) & 4, 4);
    match c.rd(0x26_0014, Size::Byte) {
        BusReply::Ok { data, waits } => {
            assert_eq!(data & 0x81, 0x81);
            assert!((6..=15).contains(&waits));
        }
        r => panic!("{r:?}"),
    }
    assert_eq!(c.rd(0x26_0016, Size::Byte).data(), Some(0x90));
    assert_eq!(c.sys.ipl(c.clock), 0);
    // 送信
    assert!(matches!(
        c.wr(0x26_0016, Size::Byte, 0xF8),
        BusReply::Ok { .. }
    ));
    c.idle(2600);
    assert_eq!(c.sys.acia.take_sent(), Some(0xF8));
    // 下位レーンの ACIA はない
    assert_eq!(c.rd(0x26_0015, Size::Byte), BERR);
}

trait Data {
    fn data(&self) -> Option<u16>;
}

impl Data for BusReply {
    fn data(&self) -> Option<u16> {
        match self {
            BusReply::Ok { data, .. } => Some(*data),
            _ => None,
        }
    }
}

#[test]
fn input_changes_raise_level_1() {
    let mut c = Cpu::new();
    c.ww(0x26_001A, 1);
    c.sys.set_ports(0x0010, 0);
    assert_eq!(c.rw(0x26_0000), 0x0010);
    assert_eq!(c.sys.ipl(c.clock), 1);
    c.ww(0x26_0018, 1);
    assert_eq!(c.sys.ipl(c.clock), 0);
    // PORT_B は許可していない
    c.sys.set_ports(0x0010, 1);
    assert_eq!(c.sys.ipl(c.clock), 0);
    assert_eq!(c.rw(0x26_0018), 2);
}

#[test]
fn levels_are_prioritized_by_the_74ls148() {
    let mut c = Cpu::new();
    c.ww(0x26_001A, 1);
    c.sys.set_ports(1, 0);
    c.acrtc_case_b();
    c.ww(0x27_0022, VIRQ_VBL_EN);
    c.idle(frame_clocks());
    assert_eq!(c.sys.irq_levels(), (1 << 6) | (1 << 1));
    assert_eq!(c.sys.ipl(c.clock), 6);
}

#[test]
fn dsp_bridge_registers() {
    // DR-053：CTRL（RESET、BIOSEL、IRQ_EN）、STATUS、MAILBOX、RESPONSE。PRGBUS の間だけプログラム窓
    let mut c = Cpu::new();
    assert_eq!(c.rw(0x22_0000), 1);
    c.idle(100);
    assert_eq!(c.rw(0x22_0002) & 1, 1, "RESET の 8 周期後に PRGBUS");
    c.ww(0x28_0010, 0x7F00);
    assert_eq!(c.rw(0x28_0010), 0x7F00);
    c.ww(0x22_0000, 0x0008 | (2 << 1));
    c.idle(100);
    assert_eq!(c.rd(0x28_0010, Size::Word), BERR);
    assert_eq!(c.rw(0x22_0000), 0x000C);
    c.ww(0x22_0004, 0x1234);
    assert_eq!(c.rw(0x22_0002) & 0x08, 0x08, "COMMAND");
    assert_eq!(c.rd(0x22_0008, Size::Word), BERR);
}

#[test]
fn adpcm_and_dac_registers() {
    let mut c = Cpu::new();
    // 電源投入時の CTRL は RESET＝1、S1＝1（8 kHz）
    assert_eq!(c.rw(0x23_0000), 0x0003);
    c.ww(0x23_0000, 0x0022);
    assert_eq!(c.rw(0x23_0000), 0x0022);
    // FIFO が空なので、半分空きの割込み（レベル 3）
    assert_eq!(c.sys.ipl(c.clock), 3);
    assert_eq!(c.rw(0x23_0002) & 0x200, 0x200);
    // 本体 DAC の DIRECT
    c.ww(0x23_0010, 1);
    c.ww(0x23_0014, 0x80FF);
    assert_eq!(c.sys.ports.main_dac.latch, 0x80FF);
    // DOC の子基板があると本体 DAC の窓はない
    c.sys.ports.doc = true;
    let now = c.sys.now();
    c.sys.ports.doc_board.set_phase(now, 0);
    assert_eq!(c.rd(0x23_0010, Size::Word), BERR);
}

#[test]
fn temporal_pointers() {
    let mut c = Cpu::new();
    c.ww(0x24_0000, 0x0100);
    c.ww(0x24_0002, 0x0040);
    assert_eq!(c.rw(0x24_0006), 0x00C0);
    c.ww(0x24_0004, 1);
    assert_eq!((c.rw(0x24_0000), c.rw(0x24_0002)), (0, 0));
    assert_eq!(c.rd(0x24_0020, Size::Word), BERR, "Capture RAM は [O]");
}

#[test]
fn doc_bus_timeout_counts_from_as() {
    // DR-052：DOC の CLK を 1 MHz に下げると、ブリッジは S0 から 65.5 クロックまでに DTACK を返せない
    let mut c = Cpu::new();
    c.sys.ports.doc = true;
    let now = c.sys.now();
    c.sys.ports.doc_board.set_phase(now, 0);
    assert!(matches!(c.rd(0x21_00E0, Size::Byte), BusReply::Ok { .. }));
    c.sys.ports.doc_board.set_clock_period(8 * PERIOD_8M);
    c.idle(200);
    let mut errors = 0;
    for _ in 0..8 {
        if c.rd(0x21_00E0, Size::Byte) == BERR {
            errors += 1;
        }
        c.idle(150);
    }
    assert_eq!(errors, 8);
}

#[test]
fn snapshot_round_trip() {
    let mut c = Cpu::new();
    c.acrtc_case_b();
    c.ww(0x27_0022, VIRQ_VBL_EN);
    c.idle(5000);
    let json = serde_json::to_string(&c.sys).unwrap();
    let mut back: System = serde_json::from_str(&json).unwrap();
    let a = c.clock + 20_000;
    c.sys.sync(a);
    back.sync(a);
    assert_eq!(c.sys.ipl_level(), back.ipl_level());
    assert_eq!(
        c.sys.ports.acrtc.video.raster,
        back.ports.acrtc.video.raster
    );
    assert_eq!(c.sys.fabric, back.fabric);
}

#[test]
fn bent_char_rom_lines_are_seen_from_the_cpu_and_port_9_alike() {
    // 仕様書 §11 のベンド点「文字ROMの線」（DR-063）：ROM の A0 は 68000 の A1
    let mut c = Cpu::new();
    c.sys.ports.char_rom[0] = 0x81;
    c.sys.ports.char_rom[1] = 0x42;
    c.sys.ports.char_lines.parse("A0=0, D7=1").unwrap();
    // 0x010002 は ROM の 1 番地だが、A0 を GND に固定したので 0 番地を読む。D7 は +5 V
    assert_eq!(
        c.rd(0x01_0002, Size::Byte),
        BusReply::Ok {
            data: 0x81,
            waits: 1
        }
    );
    c.sys.ports.char_lines.parse("normal, D0=D1").unwrap();
    assert_eq!(
        c.rd(0x01_0002, Size::Byte),
        BusReply::Ok {
            data: 0x43,
            waits: 1
        }
    );
}
