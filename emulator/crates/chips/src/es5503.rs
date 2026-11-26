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
//! - SIG± の音量は単純な乗算（E1 p.6）。IIGS で測った非線形の特性（台帳 §4.5）は `volume_level` に置き、
//!   board 層のアナログの S/H が使う。
//! - オシレータのレジスタはダイナミックで、走査で訪れたときに書き戻される（E2 p.15。台帳 §3.1・§4.7）。
//!   訪れない間が保持時間を超えたビットは 1 になる。保持時間はビットごとに分布から決める [P][V]
//!   （`REG_T_MIN_S` ほか）。時刻は core 層の単位で、`tick` ごとに E の周期（`set_cycle_time`）だけ進む。
//!   全体のレジスタ（0xE0〜0xE2）と割込みの提示は崩れない [P]。オシレータごとの見えないビット（前の訪問の
//!   停止と、割込みの要求）は、制御のレジスタと一緒に崩れて 1 になる（MAME v3）[V]。
//! - 停止・スワップ・割込みの細部は、MAME v3（IIGS の実機の測定に基づく）と V3 で突き合わせた
//!   （`docs/impl/v3-mame.md`）。データシートと違うところは台帳の追補（2026-11-26）に書いた。
//! - 走査の最後の 2 サイクルで、波形 RAM の行を 1 つずつリフレッシュする（`WaveBus::refresh`。E2 p.1）。
//!   行のカウンタは 8 bit で、リフレッシュのサイクルごとに 1 進む [P][V]。
//! - 資料で決まらない動作は、`docs/impl/emulator.md` §03 にどう扱ったかを書く。

use std::sync::LazyLock;

use fc85_core::time::{PERIOD_DOC_E, Time, UNITS_PER_SECOND};
use serde::{Deserialize, Serialize};

/// 音量の特性（IIGS の測定に基づく MAME v3.1 の式。台帳 §4.5・追補 2026-11-26）[V]：
/// 伝導 g(v)＝(W·ln(1＋e^((v−K)/W)))^P、大きさ L(v)＝255×(g(v)×(1＋LEAK)/g(255) − LEAK)、L(0)＝0。
/// ソフトニーの中央の音量 K。
pub const VOL_KNEE: f64 = 22.1;
/// ニーの柔らかさ W（音量の段）。
pub const VOL_WIDTH: f64 = 2.77;
/// ニーより上の立ち上がり P。
pub const VOL_POWER: f64 = 1.104;
/// 反転して漏れる量（音量 255 の大きさに対する割合）。
pub const VOL_LEAK: f64 = 0.0042;

/// 音量のレジスタの値 `vol` から、SIG± の電流の大きさ（音量 255 で 255×2^16、0 で 0）。
/// 音量の掛け算はアナログで、ソフトニー（音量 22 付近）より下はほとんど通らず、波形がわずかに反転して漏れる
/// （音量 1〜21 で負）。上ではニーからの距離の 1.1 乗ほどで増え、途中の音量は値の比より小さい（音量 128 で
/// 106.2、64 で 37.5）[V]。
/// コアの `Tick::sig` は単純な掛け算（E1 p.6）のままで、board 層のアナログの段がこれを使う。
pub fn volume_level(vol: u8) -> i64 {
    static TABLE: LazyLock<[i64; 256]> = LazyLock::new(|| {
        let g = |v: f64| (VOL_WIDTH * ((v - VOL_KNEE) / VOL_WIDTH).exp().ln_1p()).powf(VOL_POWER);
        let scale = (1.0 + VOL_LEAK) / g(255.0);
        let mut t = [0i64; 256];
        for (v, l) in t.iter_mut().enumerate().skip(1) {
            *l = ((g(v as f64) * scale - VOL_LEAK) * 255.0 * 65536.0).round() as i64;
        }
        t
    });
    TABLE[usize::from(vol)]
}

/// オシレータの数。
pub const OSCILLATORS: usize = 32;
/// A/D の変換にかかるサイクル数。E2 の 30 µs を 8 MHz 入力のサイクルで数えた値（E1 は 26 サイクル）[V]。
pub const ADC_CYCLES: u32 = 30;

/// レジスタのビットの保持時間（25 °C）[P][V]。チップの中のダイナミックな節点は DRAM のセル（中央 1 s、
/// 最短 50 ms。DR-072）より容量が小さいので短くした。選別で保証する最短は 2 ms で、70 °C では 88 µs。
/// 資料の範囲（CLK 8 MHz 以上）では、32 個を走査しても 34 µs ごとに訪れるので崩れない。
pub const REG_T_MIN_S: f64 = 0.002;
/// 保持時間の中央（25 °C、秒）[P][V]。
pub const REG_MEDIAN_S: f64 = 0.02;
/// 分布の形：対数ロジスティック F(t)＝1/(1＋(t/中央)^−β) の β [P]。対数正規の σ 0.5 に当たる。
pub const REG_SHAPE: f64 = 3.6;

const MODE_ONESHOT: u8 = 1;
const MODE_SYNC_AM: u8 = 2;
const MODE_SWAP: u8 = 3;

/// オシレータごとのレジスタ（保持の時刻とビットの番号に使う）。
const R_FL: usize = 0;
const R_FH: usize = 1;
const R_VOL: usize = 2;
const R_DATA: usize = 3;
const R_PTR: usize = 4;
const R_CTRL: usize = 5;
const R_SIZE: usize = 6;
const R_ACC: usize = 7;
const REGS: usize = 8;
/// レジスタのビットの数（SIZE の bit 7 はない。アキュムレータは 24 bit）。制御は、外から見えない
/// 2 つのビット（前の訪問の停止ビットと、割込みの要求）を bit 8・9 として一緒に扱う。
const REG_BITS: [u32; REGS] = [8, 8, 8, 8, 8, 10, 7, 24];

/// 波形 RAM（64K×1 の DRAM）。board 層（またはテスト）が実装する。
pub trait WaveBus {
    /// 1 byte を読む。番地は BS を bit 16 に含めた 17 bit。
    fn read_wave(&mut self, addr: u32) -> u8;
    /// リフレッシュのサイクル：行の番地 `row`（A8–A15 に出す）で行を活性化する。既定は何もしない。
    fn refresh(&mut self, _row: u8) {}
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
    /// 前の訪問のときの停止ビット（MAME v3）。
    #[serde(default)]
    prevhalt: bool,
    /// 割込みの要求（IE＝1 で完了した。MAME v3）。
    #[serde(default)]
    irqpend: bool,
}

impl Osc {
    /// 消えたレジスタ。全ビット 1（MAME の解釈。見えないビットも 1）[V]。
    const DECAYED: Osc = Osc {
        freq: 0xFFFF,
        vol: 0xFF,
        data: 0xFF,
        ptr: 0xFF,
        control: 0xFF,
        size: 0x7F,
        acc: 0x00FF_FFFF,
        prevhalt: true,
        irqpend: true,
    };

    /// レジスタ `r` のビット `mask` を 1 にする（崩れたビット）。
    fn set_bits(&mut self, r: usize, mask: u32) {
        match r {
            R_FL => self.freq |= mask as u16,
            R_FH => self.freq |= (mask as u16) << 8,
            R_VOL => self.vol |= mask as u8,
            R_DATA => self.data |= mask as u8,
            R_PTR => self.ptr |= mask as u8,
            R_CTRL => {
                self.control |= mask as u8;
                self.prevhalt |= mask & 0x100 != 0;
                self.irqpend |= mask & 0x200 != 0;
            }
            R_SIZE => self.size |= mask as u8 & 0x7F,
            R_ACC => self.acc |= mask & 0x00FF_FFFF,
            _ => {}
        }
    }

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

/// リフレッシュの 2 スロットの間、走査のカウンタがいる値（21 と 22）。リセットの後も 21 にいる。
/// MAME v3（IIGS の DOCGRADE の測定に基づく。台帳 追補 2026-11-26）[V]。
const IDLE_SCAN: u8 = 21;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Es5503 {
    osc: [Osc; OSCILLATORS],
    /// 有効なオシレータの数 N（1〜32）。
    enabled: u8,
    /// 走査のカウンタ（最後に訪れたオシレータ。リフレッシュの間は `IDLE_SCAN` と次の値）。
    #[serde(default = "default_idle_scan")]
    scan: u8,
    /// リフレッシュの何スロット目に入るか（0 は走査中、1・2 はリフレッシュ）。
    #[serde(default = "default_one")]
    idle: u8,
    /// リセットの後、0xE1 に書くまで走査しない（MAME の解釈）。
    #[serde(default = "default_true")]
    scan_off: bool,
    /// OIR が示すオシレータ（最後に提示した要求）。
    #[serde(default)]
    rege0: u8,
    /// 割込みを提示している（IRQ̄ が Low）。要求は 1 件ずつ提示する（MAME v3）。
    #[serde(default)]
    irq_active: bool,
    /// 提示した DOC サイクルの時刻。同じサイクルのうちに 0xE0 を読んでも、要求は消えない（MAME v3）。
    #[serde(default)]
    present_at: Time,
    sig: i32,
    reset_low: bool,
    adc: u8,
    adc_input: u8,
    adc_busy: u32,
    /// 今の時刻（core 層の単位）。最後の `tick` の時刻。
    #[serde(default)]
    now: Time,
    /// DOC サイクル（E の 1 周期）の長さ。
    #[serde(default = "default_cycle_time")]
    cycle_time: Time,
    /// オシレータを最後に訪れた（全部のレジスタを書き戻した）時刻。
    #[serde(default)]
    visited: [Time; OSCILLATORS],
    /// レジスタごとに、ホストが最後に書いた時刻。
    #[serde(default)]
    written: [[Time; REGS]; OSCILLATORS],
    /// レジスタが崩れるか（比較用に止められる）。
    #[serde(default = "default_true")]
    decays: bool,
    /// チップの個体（どのビットが先に崩れるか）と温度（°C）。
    #[serde(default)]
    unit: u32,
    #[serde(default = "default_temp")]
    temp_c: i32,
    /// これより短い経過では、どのビットも崩れない（`temp_c` での選別の最短）。
    #[serde(default = "default_safe")]
    safe: Time,
    /// リフレッシュの行のカウンタ。
    #[serde(default)]
    refresh_row: u8,
}

fn default_cycle_time() -> Time {
    PERIOD_DOC_E
}
fn default_idle_scan() -> u8 {
    IDLE_SCAN
}
fn default_one() -> u8 {
    1
}
fn default_true() -> bool {
    true
}
fn default_temp() -> i32 {
    25
}
fn default_safe() -> Time {
    safe_time(25)
}

/// 経過を 25 °C の経過に直す倍率（保持時間は 10 °C ごとに半分。DR-072 と同じ）[P]。
fn temp_scale(temp_c: i32) -> f64 {
    2f64.powf(f64::from(temp_c - 25) / 10.0)
}

/// 温度 `temp_c` での選別の最短の保持時間（core 層の単位）。
fn safe_time(temp_c: i32) -> Time {
    (REG_T_MIN_S / temp_scale(temp_c) * UNITS_PER_SECOND as f64) as Time
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
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
            scan: IDLE_SCAN,
            idle: 1,
            scan_off: true,
            rege0: 0,
            irq_active: false,
            present_at: 0,
            sig: 0,
            reset_low: false,
            adc: 0,
            adc_input: 0,
            adc_busy: 0,
            now: 0,
            cycle_time: PERIOD_DOC_E,
            visited: [0; OSCILLATORS],
            written: [[0; REGS]; OSCILLATORS],
            decays: true,
            unit: 0,
            temp_c: 25,
            safe: safe_time(25),
            refresh_row: 0,
        }
    }

    // ---- ピン ----

    /// RES ピン。`low` が true でリセット。有効数を 1 にし、割込みの要求を消し、走査を止める。
    /// 走査のカウンタは 21 で、リフレッシュの 1 スロット目と同じ状態になる（MAME v3）[V]。
    /// オシレータのレジスタは変えない（MAME の解釈。走査されないので、やがて消える）[V]。
    pub fn set_reset(&mut self, low: bool) {
        if low {
            self.enabled = 1;
            self.scan = IDLE_SCAN;
            self.idle = 1;
            self.scan_off = true;
            // 提示をやめる。オシレータごとの要求は残る（MAME v3）[V]
            self.irq_active = false;
            self.present_at = 0;
            self.sig = 0;
            self.adc_busy = 0;
        }
        self.reset_low = low;
    }

    /// A/D の入力（0〜2.5 V を 0〜255 に量子化した値。量子化は board 層の仕事）。
    pub fn set_ad_input(&mut self, value: u8) {
        self.adc_input = value;
    }

    /// レジスタが崩れるか（既定は true。false は比較用）。
    pub fn set_decay(&mut self, on: bool) {
        self.decays = on;
    }

    /// DOC サイクル（E の 1 周期＝CLK の 8 周期）の長さ（core 層の単位。既定は 8 MHz の 1 µs）。
    pub fn set_cycle_time(&mut self, units: Time) {
        self.cycle_time = units;
    }

    /// レジスタの保持の個体と温度（°C）。既定は個体 0、25 °C。
    pub fn set_retention(&mut self, unit: u32, temp_c: i32) {
        self.unit = unit;
        self.temp_c = temp_c;
        self.safe = safe_time(temp_c);
    }

    /// 経過 `dt` で崩れるビットの割合（0〜1）。
    pub fn decay_fraction(&self, dt: Time) -> f64 {
        let t = dt as f64 / UNITS_PER_SECOND as f64 * temp_scale(self.temp_c);
        if t < REG_T_MIN_S {
            return 0.0;
        }
        1.0 / (1.0 + (t / REG_MEDIAN_S).powf(-REG_SHAPE))
    }

    // ---- ホストのアクセス（E High の窓） ----

    /// レジスタを読む。0xE0（割込み）と 0xE2（A/D）は読むと状態が変わる（台帳 §2 の注意 5）。
    pub fn read(&mut self, reg: u8) -> u8 {
        match reg {
            0x00..=0xDF => {
                let i = usize::from(reg & 0x1F);
                self.decay_reg(i, Self::reg_index(reg));
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
            // OIR：bit7 は割込みの提示がないとき 1、bit5–1 は最後に提示したオシレータ（E1 p.7）。
            // 読むと提示した要求を消す。ただし提示した DOC サイクルのうちに読んだときは残る（MAME v3）[V]
            0xE0 => {
                let v = 0x41 | (self.rege0 << 1);
                if !self.irq_active {
                    return v | 0x80;
                }
                if self.present_at != self.now {
                    self.osc[usize::from(self.rege0)].irqpend = false;
                    self.irq_active = false;
                }
                v
            }
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
                let r = Self::reg_index(reg);
                let o = &mut self.osc[i];
                match reg & 0xE0 {
                    0x00 => o.freq = (o.freq & 0xFF00) | u16::from(value),
                    0x20 => o.freq = (o.freq & 0x00FF) | (u16::from(value) << 8),
                    0x40 => o.vol = value,
                    // 波形データは読出し専用。書いても新しくならない
                    0x60 => return,
                    0x80 => o.ptr = value,
                    0xA0 => {
                        // CPU が止めても割込みは立たず、スワップの相手も動かない。頭に戻すのは走査の仕事。
                        // 書くと、まだ提示していない要求を消す（MAME v3）[V]
                        o.control = value;
                        o.irqpend = false;
                    }
                    _ => o.size = value & 0x7F,
                }
                // 書いたレジスタだけが新しくなる。ほかのレジスタは前に書き戻した時刻のまま [P]
                self.written[i][r] = self.now;
            }
            0xE1 => {
                self.enabled = ((value >> 1) & 0x1F) + 1;
                // リフレッシュのスロットの間に書くと、残りのリフレッシュをやめ、カウンタのいる値から
                // 走査を続ける。リセットの後は 22 番から走査を始める（MAME v3）[V]
                self.idle = 0;
                if !self.reset_low {
                    self.scan_off = false;
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
        self.irq_active
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
    /// 次のリフレッシュの行。
    pub fn refresh_row(&self) -> u8 {
        self.refresh_row
    }
    /// 今の時刻（最後の `tick` の時刻。core 層の単位）。
    pub fn now(&self) -> Time {
        self.now
    }

    // ---- 時間 ----

    /// 1 DOC サイクル進める。
    pub fn tick(&mut self, bus: &mut impl WaveBus) -> Tick {
        self.now += self.cycle_time;
        if self.adc_busy > 0 {
            self.adc_busy -= 1;
            if self.adc_busy == 0 {
                self.adc = self.adc_input;
            }
        }
        // 走査：最後のオシレータの次の 2 スロットがリフレッシュで、その間カウンタは 21・22 にいる。
        // 終わりは、そのスロットの初めに、カウンタと N−1 の等値比較で決める（すでに過ぎていれば 31 まで
        // 回る。台帳 §6）
        let (slot, channel, strobe) = if self.reset_low || self.scan_off {
            self.sig = 0;
            (Slot::Stopped, 0x0F, false)
        } else if self.idle == 1 {
            self.idle = 2;
            self.scan = IDLE_SCAN + 1;
            self.refresh_slot(bus)
        } else if self.idle == 0 && self.scan == self.enabled - 1 {
            self.idle = 1;
            self.scan = IDLE_SCAN;
            self.refresh_slot(bus)
        } else {
            let n = if self.idle == 2 {
                0
            } else {
                (self.scan + 1) & 0x1F
            };
            self.idle = 0;
            self.scan = n;
            let (addr, wave) = self.visit(usize::from(n), bus);
            let ch = self.osc[usize::from(n)].control >> 4;
            (Slot::Osc { n, addr, wave }, ch, true)
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

    /// リフレッシュのスロット。波形 RAM の行を 1 つ活性化する。SIG± は前のスロットの値を保つ。
    fn refresh_slot(&mut self, bus: &mut impl WaveBus) -> (Slot, u8, bool) {
        bus.refresh(self.refresh_row);
        self.refresh_row = self.refresh_row.wrapping_add(1);
        (Slot::Refresh, 0x0F, false)
    }

    /// レジスタの番地（0x00〜0xDF）から、オシレータの中のレジスタの番号。
    fn reg_index(reg: u8) -> usize {
        match reg & 0xE0 {
            0x00 => R_FL,
            0x20 => R_FH,
            0x40 => R_VOL,
            0x60 => R_DATA,
            0x80 => R_PTR,
            0xA0 => R_CTRL,
            _ => R_SIZE,
        }
    }

    /// オシレータ `i` のレジスタ `r` を、前に書き戻してからの経過で崩す。崩れたビットは 1 [P][V]。
    /// どのビットが先に崩れるかは、個体・オシレータ・レジスタ・ビットの hash で決まり、経過が長いほど
    /// 前に崩れたビットを含んで増える（保持時間の順）。
    fn decay_reg(&mut self, i: usize, r: usize) {
        if !self.decays {
            return;
        }
        let last = self.visited[i].max(self.written[i][r]);
        let dt = self.now.saturating_sub(last);
        if dt < self.safe {
            return;
        }
        let p = self.decay_fraction(dt);
        let mut mask = 0;
        for b in 0..REG_BITS[r] {
            let key = (u64::from(self.unit) << 40)
                ^ ((i as u64) << 16)
                ^ ((r as u64) << 8)
                ^ u64::from(b);
            let u = (splitmix64(key) >> 11) as f64 / (1u64 << 53) as f64;
            if u < p {
                mask |= 1 << b;
            }
        }
        self.osc[i].set_bits(r, mask);
    }

    /// オシレータを 1 つ訪れる（台帳 §4.2〜§4.4、追補 2026-11-26）。読んだ番地と値を返し、SIG± を決める。
    /// 停止・スワップ・割込みの細部は、IIGS の実機の測定に基づく MAME v3 の動作に合わせた [V]。
    fn visit(&mut self, i: usize, bus: &mut impl WaveBus) -> (u32, u8) {
        // 前に訪れてから選別の最短を過ぎていれば、レジスタを崩してから使う。訪れると全部を書き戻す
        if self.now.saturating_sub(self.visited[i]) >= self.safe {
            for r in 0..REGS {
                self.decay_reg(i, r);
            }
        }
        self.visited[i] = self.now;
        let even = i & 1 == 0;
        let partner = i ^ 1;
        let was_halted = self.osc[i].halted();
        let mode = self.osc[i].mode();
        let p = &self.osc[partner];
        let partner_swapped = p.halted() && p.mode() == MODE_SWAP;

        // ワンショットとスワップ（M0＝1）は、止まっているのを見つけた次の訪問で頭に戻る（1 訪問遅れる）。
        // CPU がこのモードで動かすと頭から始まり、フリーランと同期は止まったところから続く（MAME v3）[V]
        if self.osc[i].prevhalt && mode & 1 == 1 {
            self.osc[i].acc = 0;
        }
        self.osc[i].prevhalt = was_halted;
        // 止まっているオシレータも読む（MAME）
        let addr = self.osc[i].address();
        let wave = bus.read_wave(addr);
        // 波形データのレジスタには 0 が入らない（MAME）
        if wave != 0 {
            self.osc[i].data = wave;
        }

        let mut sig = 0;
        let mut ended = false;
        if !was_halted {
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
            let top = self.osc[i].top();
            let acc = (self.osc[i].acc & (top - 1)) + u32::from(self.osc[i].freq);
            self.osc[i].acc = acc;
            if wave == 0 {
                self.osc[i].control |= 1;
                ended = true;
            } else if acc >= top {
                self.osc[i].acc = acc & (top - 1);
                ended = true;
                match mode {
                    // ワンショット：組の奇数番がスワップで止まっていれば、偶数番は止まらずに続ける（MAME v3）[V]
                    MODE_ONESHOT if !even || !partner_swapped => self.osc[i].control |= 1,
                    // 同期：偶数番がテーブルの頭に戻ると、自分と上の奇数番を頭から始める（E1 p.8、MAME v3）
                    MODE_SYNC_AM if even => {
                        self.osc[i].acc = 0;
                        self.osc[partner].acc = 0;
                    }
                    MODE_SWAP => self.osc[i].control |= 1,
                    _ => {}
                }
            }
        } else {
            // 0 で止まったオシレータは、訪れるたびに 0 を見つける（MAME）
            ended = wave == 0;
            // 組の奇数番がスワップで止まっていれば、止まっている偶数番は動き出す（MAME v3）[V]
            if even && partner_swapped && wave != 0 {
                self.osc[i].control &= !1;
            }
        }

        if ended {
            // 割込みの要求は IE＝1 のときだけ立つ（MAME v3。E1 p.7 のテーブルとは違う。追補 2026-11-26）[V]
            if self.osc[i].ie() {
                self.osc[i].irqpend = true;
            }
            // スワップ：偶数番が止まって終わると、組の奇数番を止まったところから動かす（E1 p.8、MAME v3）[V]
            if even && mode == MODE_SWAP && self.osc[i].halted() {
                self.osc[partner].control &= !1;
            }
        }
        // 要求は 1 件ずつ、走査の順に提示し、0xE0 を読むまで保つ（MAME v3）[V]
        if self.osc[i].irqpend && !self.irq_active {
            self.rege0 = i as u8;
            self.irq_active = true;
            self.present_at = self.now;
        }
        self.sig = sig;
        (addr, wave)
    }
}
