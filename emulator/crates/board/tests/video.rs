//! 映像の出力段（`video.rs`）と HD63484 の表示の試験。68000 のコアを使わず、試験がバスサイクルを直接出す。
//! 期待値は台帳 HD63484.md §3.4（案B）・§4.5（画素の順）・§4.8 と U85（台帳の出典）の手計算で取る。
//! 案B：表示は 64〜319 ドット、ラスタ 43〜234。背景は 60 から 4 ドットごとに読む（1 サイクルの前読み）。

use fc85_board::console::System;
use fc85_board::video::RASTER_W;
use fc85_chips::hd68000::{Bus, BusReply, Size, fc};
use fc85_core::time::{PERIOD_2CLK, PERIOD_8M, Time};

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

    fn ww(&mut self, addr: u32, v: u16) {
        match self
            .sys
            .write(self.clock, addr, fc::SUPERVISOR_DATA, Size::Word, v)
        {
            BusReply::Ok { waits, .. } => self.clock += 4 + u64::from(waits),
            r => panic!("{addr:#x}: {r:?}"),
        }
    }

    fn acrtc(&mut self, reg: u16, v: u16) {
        self.ww(0x20_0000, reg);
        self.ww(0x20_0002, v);
    }

    /// 案B（台帳 §3.4）。パレットは番号 i を $0100＋i（見分けられる色）、0 は $0F0F にする。
    fn case_b() -> Self {
        let mut c = Self::new();
        for i in 0..16u16 {
            c.ww(0x27_0000 + 2 * u32::from(i), pal(i));
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

    fn idle(&mut self, n: u64) {
        self.clock += n;
        self.sys.sync(self.clock);
    }

    fn beam(&self) -> (u16, u16) {
        self.sys.ports.acrtc.acrtc.beam()
    }

    /// ラスタ `raster` のドット `dot` 以降（同じラスタ）まで進める。
    fn until(&mut self, raster: u16, dot: u16) {
        for _ in 0..200_000 {
            let (d, r) = self.beam();
            if r == raster && d >= dot {
                return;
            }
            self.idle(1);
        }
        panic!("ラスタ {raster} に来ない");
    }

    /// 次のフレームを全部描かせる。
    fn frame(&mut self) {
        self.until(0, 0);
        self.until(1, 0);
        self.until(0, 0);
    }

    fn px(&self, raster: usize, dot: usize) -> u16 {
        self.sys.ports.acrtc.video.frame[raster * RASTER_W + dot]
    }

    fn row(&self, raster: usize, dots: std::ops::Range<usize>) -> Vec<u16> {
        dots.map(|d| self.px(raster, d)).collect()
    }
}

fn pal(i: u16) -> u16 {
    if i == 0 { 0x0F0F } else { 0x100 + i }
}

#[test]
fn background_starts_at_disp1_after_the_prefetch() {
    let mut c = Cpu::case_b();
    c.sys.ports.acrtc.vram.0[0] = 0x4321;
    c.sys.ports.acrtc.vram.0[1] = 0x8765;
    c.frame();
    // 画素 0 は DISP1 の頭（64）。左端の画素が LSB の側（台帳 §4.5）
    assert_eq!(c.px(43, 63), 0, "DISP1 の外は黒");
    assert_eq!(c.row(43, 64..73), [1, 2, 3, 4, 5, 6, 7, 8, 0].map(pal));
    assert_eq!(c.px(43, 319), pal(0));
    assert_eq!(c.px(43, 320), 0);
}

#[test]
fn hsd_scrolls_the_picture_left_by_dots() {
    // SAR1 の SDA（rCC bit 11–8）＝1：属性の HSD でシフトレジスタを 1 ドット早く入れる [P]（U85 p.40–42、p.55）
    let mut c = Cpu::case_b();
    c.sys.ports.acrtc.vram.0[0] = 0x4321;
    c.sys.ports.acrtc.vram.0[1] = 0x8765;
    c.acrtc(0xCC, 0x0100);
    c.frame();
    assert_eq!(c.row(43, 64..72), [2, 3, 4, 5, 6, 7, 8, 0].map(pal));
    // SDA＝3：3 ドット
    c.acrtc(0xCC, 0x0300);
    c.frame();
    assert_eq!(c.row(43, 64..70), [4, 5, 6, 7, 8, 0].map(pal));
}

#[test]
fn horizontal_zoom_widens_each_pixel() {
    // ZFR（rEA）の HZF＝1：同じ番地を 2 表示サイクル出し、1 画素を 2 ドット（U85 p.110）
    let mut c = Cpu::case_b();
    c.sys.ports.acrtc.vram.0[0] = 0x4321;
    c.sys.ports.acrtc.vram.0[1] = 0x8765;
    c.acrtc(0xEA, 0x1000);
    c.frame();
    assert_eq!(
        c.row(43, 64..82),
        [1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 0, 0].map(pal)
    );
}

#[test]
fn window_shows_one_display_cycle_after_disp2() {
    // ウィンドウ：96〜159 ドット（DISP2）、ラスタ 53〜62、SAR3＝$3000（U85 p.83–84、p.89–90）。前読みがないので、
    // 語は 100〜163 に出る [V]。背景はラスタ 53 が基本画面の行 10
    let mut c = Cpu::case_b();
    for a in 0x3000..0x3100 {
        c.sys.ports.acrtc.vram.0[a] = 0xFFFF;
    }
    for a in 640..704 {
        c.sys.ports.acrtc.vram.0[a] = 0x1111;
    }
    for (r, v) in [
        (0x92, 0x221F),
        (0x94, 49),
        (0x96, 10),
        (0xDA, 0x10),
        (0xDE, 0x3000),
    ] {
        c.acrtc(r, v);
    }
    c.acrtc(0x06, 0x4300);
    c.frame();
    assert_eq!(c.row(53, 92..101), [1, 1, 1, 1, 1, 1, 1, 1, 15].map(pal));
    assert_eq!(c.row(53, 162..166), [15, 15, 1, 1].map(pal));
    assert_eq!(c.px(52, 120), pal(0));
    assert_eq!(c.px(63, 120), pal(0));
    // ブランク（SE3＝10）：穴は 0 の語
    c.acrtc(0x06, 0x4200);
    c.frame();
    assert_eq!(c.row(53, 96..101), [1, 1, 1, 1, 0].map(pal));
    assert_eq!(c.px(53, 163), pal(0));
    assert_eq!(c.px(53, 164), pal(1));
}

#[test]
fn blanked_split_is_black_and_the_base_moves_down() {
    // 上画面（SP0＝16）をブランク（SE0＝10）：DISP1 が出ないので黒。基本画面はラスタ 59 から（U85 p.75–78）
    let mut c = Cpu::case_b();
    c.sys.ports.acrtc.vram.0[0] = 0x4321;
    c.acrtc(0x8C, 16);
    c.acrtc(0x06, 0x6000);
    c.frame();
    assert!(c.row(50, 0..RASTER_W).iter().all(|&p| p == 0));
    assert_eq!(c.row(59, 64..68), [1, 2, 3, 4].map(pal));
}

#[test]
fn cursor_inverts_the_palette_index() {
    // グラフィックカーソル（CDR＝$8800：CM＝10、常に表示）。CXS 40、CXE 41 → 80〜83 ドット、
    // CYS＝CYE＝50 → ラスタ 53（U85 p.94、p.107–109）。番号を XOR $F [P]
    let mut c = Cpu::case_b();
    c.sys.ports.acrtc.vram.0[10 * 64 + 4] = 0x0003;
    for (r, v) in [(0x98, 0x2928), (0x9A, 50), (0x9C, 50), (0xE8, 0x8800)] {
        c.acrtc(r, v);
    }
    c.frame();
    assert_eq!(c.row(53, 78..86), [0, 0, 12, 15, 15, 15, 0, 0].map(pal));
    assert_eq!(c.px(54, 81), pal(0));
}

#[test]
fn blink1_drops_bit_3_of_the_index() {
    // BCR（r90）の BON1＝1、BOFF1＝0：BLINK1 は常に High（U85 p.92–93）。番号の bit 3 を 0 にする [P]
    let mut c = Cpu::case_b();
    c.sys.ports.acrtc.vram.0[0] = 0x00A9;
    c.acrtc(0x90, 0x1000);
    c.frame();
    assert_eq!(c.row(43, 64..66), [1, 2].map(pal));
    c.acrtc(0x90, 0x0000);
    c.frame();
    assert_eq!(c.row(43, 64..66), [9, 10].map(pal));
}

#[test]
fn palette_copy_moves_one_entry_per_2clk() {
    // HSYNC の始まりから 2CLK に 1 語ずつ、番号の順に裏から表へ（16×186 ns＝3.0 µs、DR-027）[P]
    let mut c = Cpu::case_b();
    c.until(100, 200);
    for i in 0..16u16 {
        c.ww(0x27_0000 + 2 * u32::from(i), 0x200 + i);
    }
    let (_, r) = c.beam();
    c.until(r + 1, 3);
    let (dot, _) = c.beam();
    assert!((3..15).contains(&dot), "{dot}");
    let v = &c.sys.ports.acrtc.video;
    for i in 0..16u16 {
        let want = if i <= dot { 0x200 + i } else { pal(i) };
        assert_eq!(v.front[usize::from(i)], want, "ドット {dot} の表の {i}");
    }
}

#[test]
fn direct_writes_flash_the_written_colour_during_the_strobe() {
    // DIRECT（DR-027）：書込みのストローブ（S4〜S7＝at＋2〜at＋4 クロック）の間、表示中の画素は書いている語の
    // 新しい色になる [P][V]。ほかの画素は番号 0 のまま
    let mut c = Cpu::case_b();
    c.frame();
    c.ww(0x27_0020, 1);
    c.until(100, 150);
    let at = c.clock;
    c.ww(0x27_000A, 0x0F00);
    c.until(100, 250);
    let v = &c.sys.ports.acrtc.video;
    assert_eq!(v.front[5], 0x0F00);
    let (from, to) = ((at + 2) * PERIOD_8M, (at + 4) * PERIOD_8M);
    let hs: Time = v.hsync_at;
    let noisy: Vec<usize> = (64..240)
        .filter(|&d| {
            let t = hs + d as Time * PERIOD_2CLK;
            t >= from && t < to
        })
        .collect();
    assert!((1..=2).contains(&noisy.len()), "{noisy:?}");
    for d in 64..240 {
        let want = if noisy.contains(&d) { 0x0F00 } else { pal(0) };
        assert_eq!(c.px(100, d), want, "ドット {d}");
    }
}

#[test]
fn drawing_races_the_beam() {
    // ラスタ 60（基本画面の行 17）で画面全体（64 語×192 行）の CLR を始める。描画は表示サイクルの合間に進み
    // （表示のラスタで約 91 語＝1.4 行）、語はその時刻に VRAM に入る。このフレームでは、ビームが先に通った行は
    // 古いまま、CLR がビームを追い越した後（ラスタ 100 ごろ）の行は新しい
    let mut c = Cpu::case_b();
    c.until(60, 0);
    c.acrtc(0x00, 0x5800); // CLR
    c.ww(0x20_0002, 0xFFFF);
    c.ww(0x20_0002, 63);
    c.ww(0x20_0002, (-191i16) as u16);
    c.until(236, 0);
    assert_eq!(c.px(43, 100), pal(0), "行 0 はこのフレームでは古い");
    assert_eq!(c.px(80, 100), pal(0), "行 37 はビームが先");
    assert_eq!(c.px(200, 100), pal(15), "行 157 は追い越した後");
    c.frame();
    assert!((43..235).all(|r| c.px(r, 64) == pal(15) && c.px(r, 319) == pal(15)));
}
