//! 映像のクロックのベンド点（仕様書 §11、DR-035 (1)・(5)）：Main RAM と VRAM の行の割当てと、リフレッシュが
//! 遅れたときの崩れ（DR-072 の `dram`）。
//!
//! 映像系の 21.47727 MHz を外部のクロックに替えると、ACRTC の 2CLK（÷4）と HSYNC が一緒に動く。Main RAM の
//! リフレッシュは HSYNC ごとに 5 行（DR-026）、VRAM のリフレッシュは ACRTC が HSYNC の間に行う（台帳 HD63484.md
//! §3.1）ので、クロックを下げるか止めると、どちらもリフレッシュの間が延びる。
//!
//! 模型 [P]（追跡は Bend モードのときだけ。通常モードでは何もしない）：
//! - 部品はどちらも 64K×1 の DRAM×16（Main RAM は 4164-15、VRAM は HM4864A-12）で、128 行のリフレッシュ
//!   （行の番地の下位 7 bit。RA7 の違う 2 行を一度に活性化する。HM4864A は 128 行／2 ms [D MEM8505 p.192]）。
//!   行のまとまりは 128 個、1 つが 2 行×256 列＝512 語。
//! - Main RAM の行は 68000 の A9〜A16（語の番地の bit 8〜15）、列は A1〜A8 [P]。同じ 512 byte の中を使う
//!   プログラムの作業領域（スタック、変数）は、CPU のアクセスで行が活性化されて残り、ほかが崩れる。
//!   リフレッシュは 7 bit のカウンタで、HSYNC ごとに 5 行（DR-026）。CPU の Main RAM へのアクセスも、
//!   その語の行を活性化する。
//! - VRAM の行は ACRTC の語の番地の bit 0〜7、列は bit 8〜15 [P]。ACRTC はリフレッシュの番地を MAD に出す
//!   （8 bit、台帳 §3.1）ので、それを行の番地の線に通すにはこの割当てになる（MAD7–0 に出るとした [V]）。
//!   表示の読出し（1 ラインに連続した 65 語）、描画の読み書き、リフレッシュが行を活性化する。表示の読出しは
//!   2 ラインで 128 行の全部を通る。
//! - 放電した値：行の番地の bit 0（RA0）が 0 の行は真のセルで 0、1 の行は補のセルで 1 [P]（折り返しの
//!   ビット線で、隣の行が BL と /BL に交互につながるとした。実際の配置は資料にない [V]）。崩れた Main RAM は
//!   512 byte ごとに $00 と $FF、VRAM は 1 語（4 画素）ごとにパレットの 0 番と 15 番の縦の縞に近づく。
//! - どのセルが先に崩れるかは `DramEnv::decays`（DR-072）。チップの番号は Main RAM が `MAIN_RAM_CHIP0`、
//!   VRAM が `VRAM_CHIP0` から（データの bit i がチップ CHIP0＋i）、セルの番号は語の番地。

use fc85_chips::hd63484::Vram;
use fc85_core::time::{PERIOD_21M, Time, UNITS_PER_SECOND};
use serde::{Deserialize, Serialize};

use crate::bend::{Mode, video_period_of};
use crate::console::System;
use crate::dram::Dram;

/// 行のまとまり（128 行のリフレッシュ）の数。
pub const GROUPS: usize = 128;
/// Main RAM の HSYNC ごとのリフレッシュの行の数（DR-026）。
pub const MAIN_ROWS_PER_HSYNC: u64 = 5;
/// `DramEnv::decays` のチップの番号の始まり。
pub const MAIN_RAM_CHIP0: u32 = 0x100;
pub const VRAM_CHIP0: u32 = 0x200;

/// Main RAM の語（0〜0xFFFF）の行の番地（A9〜A16）。
#[inline]
pub fn main_row(word: usize) -> usize {
    word >> 8
}

/// VRAM の語（0〜0xFFFF）の行の番地（語の番地の bit 0〜7）。
#[inline]
pub fn vram_row(word: usize) -> usize {
    word & 0xFF
}

/// 行の放電した値（16 bit）：RA0 が 1 なら補のセルで 1。
#[inline]
pub fn discharged(row: usize) -> u16 {
    if row & 1 != 0 { 0xFFFF } else { 0 }
}

/// 語の 16 bit のうち崩れるものを放電した値にする。
fn decay_word(d: &Dram, chip0: u32, word: usize, v: u16, p: f64, dis: u16) -> u16 {
    let env = d.env();
    let cell = word as u32;
    let lo = env.decay_byte(v as u8, chip0, cell, p, dis as u8);
    let hi = env.decay_byte((v >> 8) as u8, chip0 + 8, cell, p, (dis >> 8) as u8);
    (u16::from(hi) << 8) | u16::from(lo)
}

impl System {
    /// 映像系の 21.47727 MHz の周波数（Hz。0 は止まっている）。外部のクロックは、時刻の単位に丸める前の
    /// 周波数（テレビの模型が見る。色副搬送波の引込みは時刻の 1 単位より細かい [P]）。
    pub fn video_clock_hz(&self) -> f64 {
        let p = self.ports.acrtc.video_period();
        if p == 0 {
            return 0.0;
        }
        match self.bends.clocks.video {
            Some(m) if self.bends.mode == Mode::Bend && video_period_of(Some(m)) == p => {
                m as f64 / 1000.0
            }
            _ => UNITS_PER_SECOND as f64 / p as f64,
        }
    }

    /// 映像のクロックのジャンパと、DRAM（Main RAM・VRAM）の保持の追跡を回路に入れる（`apply_bends` から）。
    /// 通常モードは基板の発振器で、追跡しない。
    pub(crate) fn apply_video_bends(&mut self, on: bool) {
        let now = self.now();
        let env = self.bends.dram;
        let a = &mut self.ports.acrtc;
        a.set_video_period(if on {
            self.bends.clocks.video_period()
        } else {
            PERIOD_21M
        });
        a.dram.set_env(env);
        self.main_dram.dram.set_env(env);
        if on {
            a.dram.start(now);
            self.main_dram.start(now, a.video.lines);
        } else {
            a.dram.stop();
            self.main_dram.dram.stop();
        }
    }
}

/// Main RAM のリフレッシュの回路（DR-026）と保持の模型。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainRamRefresh {
    pub dram: Dram,
    /// 次にリフレッシュする行（7 bit のカウンタ）。
    pub row: u8,
    /// 数えた HSYNC（`Video::lines`）。
    lines: u64,
}

impl Default for MainRamRefresh {
    fn default() -> Self {
        Self::new()
    }
}

impl MainRamRefresh {
    pub fn new() -> Self {
        Self {
            dram: Dram::new(GROUPS),
            row: 0,
            lines: 0,
        }
    }

    /// 追跡を始める（Bend モード）。`lines` は今の HSYNC の数。
    pub fn start(&mut self, now: Time, lines: u64) {
        if !self.dram.tracking() {
            self.lines = lines;
        }
        self.dram.start(now);
    }

    /// 行のまとまり `g` を活性化し、崩れるセルがあれば崩す。
    fn activate(&mut self, ram: &mut [u8], g: usize, t: Time) {
        let Some(p) = self.dram.activate(g, t) else {
            return;
        };
        for row in [g, g | 0x80] {
            let dis = discharged(row);
            for word in row << 8..(row + 1) << 8 {
                let i = 2 * word;
                let v = u16::from_be_bytes([ram[i], ram[i + 1]]);
                let n = decay_word(&self.dram, MAIN_RAM_CHIP0, word, v, p, dis);
                ram[i..i + 2].copy_from_slice(&n.to_be_bytes());
            }
        }
    }

    /// 68000 の Main RAM へのアクセス（語の番地 `word`、時刻 `t`）。読み書きの前に呼ぶ。
    pub fn access(&mut self, ram: &mut [u8], word: usize, t: Time) {
        self.activate(ram, main_row(word) & (GROUPS - 1), t);
    }

    /// HSYNC を `lines` まで数えた：増えた数×5 行をリフレッシュする（時刻は最後の HSYNC の `t`）。
    pub fn hsync(&mut self, ram: &mut [u8], lines: u64, t: Time) {
        let n = lines.wrapping_sub(self.lines);
        if n == 0 {
            return;
        }
        self.lines = lines;
        let rows = n.saturating_mul(MAIN_ROWS_PER_HSYNC);
        for k in 0..rows.min(GROUPS as u64) {
            let g = (u64::from(self.row) + k) as usize & (GROUPS - 1);
            self.activate(ram, g, t);
        }
        self.row = ((u64::from(self.row) + rows % GROUPS as u64) % GROUPS as u64) as u8;
    }
}

/// VRAM の行のまとまり `g` を活性化し、崩れるセルがあれば崩す。
fn vram_activate(dram: &mut Dram, mem: &mut [u16], g: usize, t: Time) {
    let Some(p) = dram.activate(g, t) else {
        return;
    };
    for row in [g, g | 0x80] {
        let dis = discharged(row);
        for col in 0..256 {
            let word = row | (col << 8);
            mem[word] = decay_word(dram, VRAM_CHIP0, word, mem[word], p, dis);
        }
    }
}

/// 追跡しているときの VRAM：ACRTC の読み書きが行を活性化する。
pub struct TrackedVram<'a> {
    pub mem: &'a mut [u16],
    pub dram: &'a mut Dram,
    /// この 2CLK の立上りの時刻。
    pub at: Time,
}

impl TrackedVram<'_> {
    /// 語番地 `addr`（下位 16 bit を使う）の行を活性化する。
    pub fn activate_word(&mut self, addr: u32) {
        let g = vram_row((addr & 0xFFFF) as usize) & (GROUPS - 1);
        vram_activate(self.dram, self.mem, g, self.at);
    }

    /// リフレッシュの番地（MAD7–0）の行を活性化する。
    pub fn refresh(&mut self, rfa: u8) {
        vram_activate(
            self.dram,
            self.mem,
            usize::from(rfa) & (GROUPS - 1),
            self.at,
        );
    }
}

impl Vram for TrackedVram<'_> {
    fn read(&mut self, addr: u32) -> u16 {
        self.activate_word(addr);
        self.mem[(addr & 0xFFFF) as usize]
    }
    fn write(&mut self, addr: u32, data: u16) {
        self.activate_word(addr);
        self.mem[(addr & 0xFFFF) as usize] = data;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fc85_core::time::UNITS_PER_SECOND;

    #[test]
    fn rows_and_discharged_values_follow_the_mapping() {
        // Main RAM：語 0x1234 は A9〜A16＝0x12 の行。VRAM：語 0x1234 は 0x34 の行
        assert_eq!(main_row(0x1234), 0x12);
        assert_eq!(vram_row(0x1234), 0x34);
        assert_eq!(discharged(0x12), 0);
        assert_eq!(discharged(0x13), 0xFFFF);
    }

    #[test]
    fn hsync_refreshes_five_rows_per_line_and_wraps_at_128() {
        let mut m = MainRamRefresh::new();
        let mut ram = vec![0u8; 0x2_0000];
        m.start(0, 10);
        m.hsync(&mut ram, 13, 1);
        assert_eq!(m.row, 15);
        // 25.6 ラインで一巡：26 ライン後は 15＋130＝145 → 17
        m.hsync(&mut ram, 39, 2);
        assert_eq!(m.row, 17);
        // 10 秒止めた後の 1 ラインは 5 行（17〜21）だけを活性化し、その行は崩れる（10 秒の割合は 0.999998）
        let s = UNITS_PER_SECOND;
        ram[2 * (18 << 8)..2 * (19 << 8)].fill(0xFF);
        ram[2 * (30 << 8)..2 * (31 << 8)].fill(0xFF);
        m.hsync(&mut ram, 40, 10 * s);
        assert_eq!(m.row, 22);
        let ones = |ram: &[u8], row: usize| -> u32 {
            ram[2 * (row << 8)..2 * ((row + 1) << 8)]
                .iter()
                .map(|b| b.count_ones())
                .sum()
        };
        // 行 17・145（RA0＝1、補のセル）は 1 に、行 18（RA0＝0、真のセル）は 0 になる
        assert!(ones(&ram, 17) >= 4090 && ones(&ram, 145) >= 4090);
        assert!(ones(&ram, 18) <= 6);
        // 活性化していない行 30 は、崩れていても読み書きされるまで値が変わらない（模型の約束）
        assert_eq!(ones(&ram, 30), 4096);
    }
}
