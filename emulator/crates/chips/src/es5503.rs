//! Ensoniq ES5503 DOC（Digital Oscillator Chip）。
//!
//! 事実と出典は `docs/parts/ES5503.md`（以下「台帳」）。動作は Ensoniq のデータシート（台帳の E2）と
//! Apple の ERS（E1）に従い、資料にない細部は、台帳 §4・§6 に書き起こした MAME（E10。IIGS の実機の測定に基づく）の
//! 動作を使った。コードは自前で書いた。
//!
//! モデルの範囲：
//! - `tick` の 1 回を 1 DOC サイクル（CLK の 8 周期＝E の 1 周期。8 MHz 入力で 1 µs）とする（台帳 §3.2）。
//! - 1 サイクルに有効なオシレータを 1 つ訪れる。最後のオシレータの後にリフレッシュの 2 サイクルが続く。
//! - ホストのアクセス（`read`、`write`）は E High の窓で行う。1 サイクル 1 回の制限と、波形 RAM との調停は
//!   board 層の仕事（台帳 §3.3）。
//! - 出力は、サイクルごとの SIG±（符号付きの波形値×音量）、CA3–CA0、CSTRB。チャンネルごとのサンプルホールドと
//!   加算は board 層の仕事（仕様書 §10）。リフレッシュの 2 サイクルは、最後のオシレータの SIG± を保つ [V]。
//! - 音量は単純な乗算（E1 p.6）。IIGS で測った非線形の特性（台帳 §4.5）はアナログの層で足す。
//! - 資料で決まらない動作は、`docs/impl/emulator.md` §03 にどう扱ったかを書く。

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// オシレータの数。
pub const OSCILLATORS: usize = 32;
/// A/D の変換にかかるサイクル数。E2 の 30 µs を 8 MHz 入力のサイクルで数えた値（E1 は 26 サイクル）[V]。
pub const ADC_CYCLES: u32 = 30;
/// 走査で訪れないオシレータのレジスタが消えるまでのサイクル数。約 1.5 秒（MAME の解釈）[V]。
pub const DECAY_CYCLES: u64 = 1_500_000;

const MODE_ONESHOT: u8 = 1;
const MODE_SYNC_AM: u8 = 2;
const MODE_SWAP: u8 = 3;

/// 波形 RAM。board 層（またはテスト）が実装する。
pub trait WaveBus {
    /// 1 byte を読む。番地は BS を bit 16 に含めた 17 bit。
    fn read_wave(&mut self, addr: u32) -> u8;
}

/// サイクルの種類。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// リセットの後、0xE1 に書くまで走査しない（MAME の解釈）。
    Stopped,
    /// オシレータ `n` を訪れ、番地 `addr` から `wave` を読んだ。
    Osc { n: u8, addr: u32, wave: u8 },
    /// DRAM のリフレッシュ。
    Refresh,
}

/// 1 サイクルの外部ピン。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    pub slot: Slot,
    /// CA3–CA0。リフレッシュと停止の間は 0xF（E2 p.22）。
    pub channel: u8,
    /// CSTRB が有効（Low）か。オシレータのサイクルだけ有効。
    pub strobe: bool,
    /// SIG± が表す値：符号付きの波形値（−127〜127）×音量（0〜255）。
    pub sig: i32,
    /// IRQ ピンが Low か。
    pub irq: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Osc {
    freq: u16,
    vol: u8,
    data: u8,
    ptr: u8,
    control: u8,
    /// 0xC0 のレジスタ：bit6＝BS、bit5–3＝T（テーブルの大きさ）、bit2–0＝R（分解能）。
    size: u8,
    /// 24 bit のアキュムレータ。
    acc: u32,
    /// 最後にリフレッシュしたサイクル。
    refreshed: u64,
}

impl Osc {
    /// 消えたレジスタ。全ビット 1（MAME の解釈）[V]。
    const DECAYED: Osc = Osc {
        freq: 0xFFFF,
        vol: 0xFF,
        data: 0xFF,
        ptr: 0xFF,
        control: 0xFF,
        size: 0x7F,
        acc: 0x00FF_FFFF,
        refreshed: 0,
    };

    fn halted(&self) -> bool {
        self.control & 1 != 0
    }
    fn mode(&self) -> u8 {
        (self.control >> 1) & 3
    }
    fn ie(&self) -> bool {
        self.control & 0x08 != 0
    }
    fn table_bits(&self) -> u32 {
        u32::from((self.size >> 3) & 7)
    }
    fn resolution(&self) -> u32 {
        u32::from(self.size & 7)
    }
    /// テーブルの終わり：2^(17＋R)（台帳 §4.3）。
    fn top(&self) -> u32 {
        1 << (17 + self.resolution())
    }
    /// 波形の番地（台帳 §4.3）。ポインタはテーブルの大きさの単位に切り捨て、オフセットは A の上位から取る。
    fn address(&self) -> u32 {
        let t = self.table_bits();
        let len = 256u32 << t;
        let base = (u32::from(self.ptr) << 8) & !(len - 1) & 0xFFFF;
        let offset = (self.acc >> (9 + self.resolution() - t)) & (len - 1);
        (u32::from(self.size & 0x40) << 10) | base | offset
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Next {
    Off,
    Osc(u8),
    Refresh(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Es5503 {
    osc: [Osc; OSCILLATORS],
    /// 有効なオシレータの数 N（1〜32）。
    enabled: u8,
    next: Next,
    cycle: u64,
    /// 割込みテーブル：IE＝0 で完了したオシレータ（E1 p.7）。
    table: u32,
    /// OIR に渡った要求。先頭が OIR に見えている（先入れ先出し、E1 p.7）。
    stack: VecDeque<u8>,
    last_irq: u8,
    sig: i32,
    reset_low: bool,
    adc: u8,
    adc_input: u8,
    adc_busy: u32,
    decay: Option<u64>,
}

impl Default for Es5503 {
    fn default() -> Self {
        Self::new()
    }
}

impl Es5503 {
    /// 電源投入直後。レジスタは全ビット 1、有効数は 1、0xE1 に書くまで走査しない（台帳 §4.8）。
    pub fn new() -> Self {
        Self {
            osc: [Osc::DECAYED; OSCILLATORS],
            enabled: 1,
            next: Next::Off,
            cycle: 0,
            table: 0,
            stack: VecDeque::new(),
            last_irq: 0,
            sig: 0,
            reset_low: false,
            adc: 0,
            adc_input: 0,
            adc_busy: 0,
            decay: Some(DECAY_CYCLES),
        }
    }

    // ---- ピン ----

    /// RES ピン。`low` が true でリセット。有効数を 1 にし、割込みの要求を消し、走査を止める。
    /// オシレータのレジスタは変えない（MAME の解釈。走査されないので、やがて消える）[V]。
    pub fn set_reset(&mut self, low: bool) {
        if low {
            self.enabled = 1;
            self.next = Next::Off;
            self.table = 0;
            self.stack.clear();
            self.sig = 0;
            self.adc_busy = 0;
        }
        self.reset_low = low;
    }

    /// A/D の入力（0〜2.5 V を 0〜255 に量子化した値。量子化は board 層の仕事）。
    pub fn set_ad_input(&mut self, value: u8) {
        self.adc_input = value;
    }

    /// レジスタが消えるまでのサイクル数。`None` で消えない（比較と Bend 用）。
    pub fn set_decay(&mut self, cycles: Option<u64>) {
        self.decay = cycles;
    }

    // ---- ホストのアクセス（E High の窓） ----

    /// レジスタを読む。0xE0（割込み）と 0xE2（A/D）は読むと状態が変わる（台帳 §2 の注意 5）。
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg {
            0x00..=0xDF => {
                let i = usize::from(reg & 0x1F);
                self.check_decay(i);
                let o = &self.osc[i];
                match reg & 0xE0 {
                    0x00 => o.freq as u8,
                    0x20 => (o.freq >> 8) as u8,
                    0x40 => o.vol,
                    0x60 => o.data,
                    0x80 => o.ptr,
                    0xA0 => o.control,
                    _ => o.size | 0x80,
                }
            }
            0xE0 => match self.stack.pop_front() {
                Some(n) => {
                    self.last_irq = n;
                    0x41 | (n << 1)
                }
                None => 0xC1 | (self.last_irq << 1),
            },
            0xE1 => 0xC1 | ((self.enabled - 1) << 1),
            0xE2 => {
                // 変換中に読むと値は失われ、変換をやり直す（E1 p.10）。前の値を返す [V]。
                let v = self.adc;
                self.adc_busy = ADC_CYCLES;
                v
            }
            _ => 0xFF,
        }
    }

    /// レジスタに書く。0x60〜0x7F（波形データ）、0xE0、0xE2 への書込みは捨てる。
    pub fn write(&mut self, reg: u8, value: u8) {
        match reg {
            0x00..=0xDF => {
                let i = usize::from(reg & 0x1F);
                self.check_decay(i);
                let o = &mut self.osc[i];
                match reg & 0xE0 {
                    0x00 => o.freq = (o.freq & 0xFF00) | u16::from(value),
                    0x20 => o.freq = (o.freq & 0x00FF) | (u16::from(value) << 8),
                    0x40 => o.vol = value,
                    0x60 => {}
                    0x80 => o.ptr = value,
                    0xA0 => {
                        o.control = value;
                        // IE を 1 にすると、テーブルに残っていた要求が OIR へ渡る（E1 p.7）
                        if o.ie() && self.table & (1 << i) != 0 {
                            self.table &= !(1 << i);
                            self.stack.push_back(i as u8);
                        }
                    }
                    _ => o.size = value & 0x7F,
                }
                // 書いたレジスタは新しい。同じオシレータのほかのレジスタもまとめて新しいとみなす（近似）[V]
                self.osc[i].refreshed = self.cycle;
            }
            0xE1 => {
                self.enabled = ((value >> 1) & 0x1F) + 1;
                if self.next == Next::Off && !self.reset_low {
                    self.next = Next::Osc(0);
                }
            }
            _ => {}
        }
    }

    // ---- 状態の読み出し（副作用なし） ----

    /// 有効なオシレータの数 N。
    pub fn enabled(&self) -> u8 {
        self.enabled
    }
    /// IRQ ピンが Low か。
    pub fn irq(&self) -> bool {
        !self.stack.is_empty()
    }
    pub fn accumulator(&self, n: usize) -> u32 {
        self.osc[n].acc
    }
    pub fn control(&self, n: usize) -> u8 {
        self.osc[n].control
    }
    pub fn volume(&self, n: usize) -> u8 {
        self.osc[n].vol
    }

    // ---- 時間 ----

    /// 1 DOC サイクル進める。
    pub fn tick(&mut self, bus: &mut impl WaveBus) -> Tick {
        self.cycle += 1;
        if self.adc_busy > 0 {
            self.adc_busy -= 1;
            if self.adc_busy == 0 {
                self.adc = self.adc_input;
            }
        }
        let (slot, channel, strobe) = match self.next {
            _ if self.reset_low => {
                self.sig = 0;
                (Slot::Stopped, 0x0F, false)
            }
            Next::Off => {
                self.sig = 0;
                (Slot::Stopped, 0x0F, false)
            }
            Next::Refresh(k) => {
                self.next = if k == 0 {
                    Next::Refresh(1)
                } else {
                    Next::Osc(0)
                };
                (Slot::Refresh, 0x0F, false)
            }
            Next::Osc(n) => {
                let (addr, wave) = self.visit(usize::from(n), bus);
                // 走査の終わりは等値比較。すでに終点を過ぎていれば 31 まで回る（台帳 §6）
                self.next = if n + 1 == self.enabled {
                    Next::Refresh(0)
                } else {
                    Next::Osc((n + 1) & 0x1F)
                };
                let ch = self.osc[usize::from(n)].control >> 4;
                (Slot::Osc { n, addr, wave }, ch, true)
            }
        };
        Tick {
            slot,
            channel,
            strobe,
            sig: self.sig,
            irq: self.irq(),
        }
    }

    // ---- 内部 ----

    fn check_decay(&mut self, i: usize) {
        if let Some(limit) = self.decay
            && self.cycle.saturating_sub(self.osc[i].refreshed) > limit
        {
            self.osc[i] = Osc {
                refreshed: self.cycle,
                ..Osc::DECAYED
            };
        }
    }

    /// オシレータを 1 つ訪れる（台帳 §4.2〜§4.4）。読んだ番地と値を返し、SIG± を決める。
    fn visit(&mut self, i: usize, bus: &mut impl WaveBus) -> (u32, u8) {
        self.check_decay(i);
        self.osc[i].refreshed = self.cycle;
        let before = self.osc[i];
        let even = i & 1 == 0;
        let partner = i ^ 1;

        // M0＝1 かつ H＝1 なら、アキュムレータを 0 にする（E1 p.8）
        if before.halted() && before.mode() & 1 == 1 {
            self.osc[i].acc = 0;
        }
        // 止まっているオシレータも読む（MAME）
        let addr = self.osc[i].address();
        let wave = bus.read_wave(addr);
        // 波形データのレジスタには 0 が入らない（MAME）
        if wave != 0 {
            self.osc[i].data = wave;
        }

        let mut sig = 0;
        let ended;
        if !before.halted() {
            let mode = before.mode();
            if wave != 0 {
                if mode == MODE_SYNC_AM && !even {
                    // AM：奇数番は鳴らさず、上の偶数番の音量を波形値で上書きする（E1 p.8、MAME）
                    if i + 1 < OSCILLATORS && !self.osc[i + 1].halted() {
                        self.osc[i + 1].vol = wave;
                    }
                } else {
                    sig = i32::from((wave ^ 0x80) as i8) * i32::from(self.osc[i].vol);
                }
            }
            let top = before.top();
            let acc = (self.osc[i].acc & (top - 1)) + u32::from(before.freq);
            self.osc[i].acc = acc;
            if wave == 0 {
                self.osc[i].control |= 1;
                ended = true;
            } else if acc >= top {
                self.osc[i].acc = acc & (top - 1);
                ended = true;
                match mode {
                    MODE_ONESHOT | MODE_SWAP => self.osc[i].control |= 1,
                    // 同期：偶数番がテーブルの頭に戻ると、上の奇数番も頭に戻す（E1 p.8）
                    MODE_SYNC_AM if even => self.osc[partner].acc = 0,
                    _ => {}
                }
            } else {
                ended = false;
            }
        } else {
            // 0 で止まったオシレータは、訪れるたびに 0 を見つける（MAME）
            ended = wave == 0;
        }

        if ended {
            self.request_irq(i as u8);
            // スワップ：自分が止まったら、組の相手の停止を解く（E1 p.8）。偶数番・奇数番とも [V]
            if before.mode() == MODE_SWAP && !before.halted() && self.osc[i].halted() {
                self.osc[partner].control &= !1;
            }
        }
        self.sig = sig;
        (addr, wave)
    }

    fn request_irq(&mut self, n: u8) {
        if self.stack.contains(&n) {
            return;
        }
        if self.osc[usize::from(n)].ie() {
            self.table &= !(1 << n);
            self.stack.push_back(n);
        } else {
            self.table |= 1 << n;
        }
    }
}
