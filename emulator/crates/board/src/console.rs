//! 本体（段階 3）：68000 のバス、アドレスの解読、待ち、バスの時間切れ、割込み、本体の I/O レジスタ。
//!
//! 仕様書 §05（アドレスマップ）・§09（入力）、DR-016（ACRTC の CS）、DR-019（子基板がないとき）、
//! DR-024（Staging RAM の時分割）、DR-025（ベクタ表）、DR-026（ROM の待ち、Main RAM のリフレッシュ）、
//! DR-027（パレット）、DR-028（ACIA と割込み）、DR-029（DSP のプログラム窓）、DR-031（文字 ROM）、
//! DR-051（DOC のブリッジ）、DR-052（バスの時間切れと解読）、DR-053（I/O レジスタの番地とビット）、
//! DR-054（音声の出力）。
//!
//! 模型：
//! - 時刻は CPU クロック（8 MHz）の数で受け取り、core 層の単位（`PERIOD_8M` 倍）にする。CPU の立上りは
//!   スロットの頭と同じ格子にある [P]（DOC のブリッジと同じ）。
//! - 1 回のバスサイクルは、S0 の時刻を含むスロットまで基板を進めてから答える（`sync`）。読み書きの効果は
//!   その時刻に起きたとする。ずれは 1 スロット（250 ns）以内 [P]。ACRTC は T1 の 2CLK を含むスロット、
//!   DOC と Fabric の A ファイルは DTACK まで、スロットの列で進める。
//! - 待ちは仕様と DR の規則から計算する。バスの時間切れ（DR-052）：AS↓ の後の CPU クロックの立上りを
//!   64 数えて BERR を出す。BERR を S0 から 65.5 クロックの立下りで認識し、サイクルは 67 クロックで終わる。
//! - 割込み（DR-028）：要因をレベルにまとめ、74LS148 で IPL にする。応答はすべてオートベクタ（VPA）。

use fc85_chips::hd6850::Hd6850;
use fc85_chips::hd68000::{Bus, BusReply, Hd68000, IackReply, Size};
use fc85_core::time::{PERIOD_2CLK, PERIOD_8M, SLOT, Time};
use serde::{Deserialize, Serialize};

use crate::adpcm::AdpcmCtrl;
use crate::analog::{self, Analog, SigWiring, TICK};
use crate::bend::{Bends, FifoPins, Mode, RomLines};
use crate::doc;
use crate::dsp::{BioSel, DspCtrl};
use crate::fabric::{Fabric, HostReply};
use crate::ports::FabricPorts;

/// Boot ROM（64 KiB）と Main RAM（128 KiB）の大きさ。
pub const BOOT_ROM_BYTES: usize = 0x1_0000;
pub const MAIN_RAM_BYTES: usize = 0x2_0000;

/// BERR を認識する立下り（S0 からの半クロックの数。65.5 クロック）。DR-052。
pub const BERR_HALF_CLOCKS: u64 = 131;
/// BERR で終わるサイクルの長さ（S0 からのクロック数）。DR-052。
pub const BERR_CYCLE_CLOCKS: u32 = 67;

/// Main RAM のリフレッシュ（HSYNC ごとに 5 行、12.5 クロック）。DR-026。
const REFRESH: Time = 25 * PERIOD_8M / 2;
/// パレットの裏から表への写し（16 語×186 ns＝3.0 µs、24 クロック）。DR-027。
const PALETTE_COPY: Time = 24 * PERIOD_8M;
/// ACRTC の CS：S4 の立上りで登録する FF の遅れ（20 ns）、ACRTC の CS のセットアップ（40 ns）、
/// DTACK の遅れ（tDTKZL の中央 35 ns）、68000 の DTACK のセットアップ（20 ns）。host-acrtc-interface.md §5。
const ACRTC_CS: Time = 302;
const ACRTC_CS_SETUP: Time = 605;
const ACRTC_DTACK: Time = 529;
const CPU_DTACK_SETUP: Time = 302;

/// 音声の出力の既定の標本化周波数（ホストへ渡す）。
pub const AUDIO_RATE: u32 = 48_000;

/// INPUT_IRQ_STATUS のビット（DR-053）。
pub const INPUT_PORT_A: u16 = 0x0001;
pub const INPUT_PORT_B: u16 = 0x0002;
pub const INPUT_MIDI: u16 = 0x0004;
pub const INPUT_EXTRIG: u16 = 0x0008;

/// 入力のレジスタ（仕様書 §09、DR-053）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub port_a: u16,
    pub port_b: u16,
    pub axis: [u16; 8],
    /// INPUT_IRQ_STATUS のうち W1C のビット（bit 0、1、3）。
    pub status: u16,
    pub enable: u16,
}

/// バスの出来事の記録（エミュレータの警告と統計）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusLog {
    pub bus_errors: u64,
    /// 最後のバスエラーの番地と、書込みだったか。
    pub last_bus_error: Option<(u32, bool)>,
    pub iacks: u64,
    /// DOC の窓の規約（DR-030）に反した命令の数と、最後の番地。1 命令の中で同じ番地を読んで
    /// 書いた（読み書き一体）か、窓に 2 回以上触った（MOVEM、MOVEP など）ときに数える。
    /// 通常モードの警告で、動作は変えない。
    #[serde(default)]
    pub doc_rule_warnings: u64,
    #[serde(default)]
    pub last_doc_rule: Option<u32>,
}

/// 68000 の外側：基板とメモリと I/O。`Bus` を実装する。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct System {
    pub fabric: Fabric,
    pub ports: FabricPorts,
    pub boot_rom: Vec<u8>,
    pub main_ram: Vec<u8>,
    /// カートリッジの ROM（なければ窓はバスエラー）。
    pub cart: Option<Vec<u8>>,
    pub acia: Hd6850,
    /// ACIA に渡した CPU クロック（16 の倍数。ACIA は 500 kHz＝CPU の 16 クロック）。
    acia_clock: u64,
    pub input: Input,
    /// DSP bridge の CTRL の IRQ_EN（DR-053）。
    pub dsp_irq_en: bool,
    /// Temporal の CTRL の bit 1：VBlank の始まりで両方のポインタを 0 にする（DR-053）。
    pub temporal_sync: bool,
    /// Temporal の CTRL の bit 2：VBlank の始まりで読出しのポインタだけを 0 にする（繰返し。DR-067）。
    #[serde(default)]
    pub temporal_rsync: bool,
    vblank_prev: bool,
    pub log: BusLog,
    /// ベンド点の設定とモード（DR-065）。変えたら `apply_bends` で回路に入れる。
    #[serde(default)]
    pub bends: Bends,
    /// 今の命令の中の DOC の窓へのアクセス（番地、書込みか）。命令ごとに `Console` が見て消す。
    #[serde(skip)]
    doc_trace: Vec<(u32, bool)>,
    /// 音声の出力（L、R）。ホストが取り出す。保存には入れない。
    #[serde(skip)]
    pub audio_out: Vec<(i16, i16)>,
    /// 次の音声の標本の時刻と、その間隔。
    next_sample: Time,
    pub sample_period: Time,
    /// 音声のアナログの段（analog.rs）。DOC の S/H は `ports.doc_board` が持つ。
    #[serde(default)]
    pub analog: Analog,
    /// LINE OUT（アナログの段の出力、48 kHz、L・R）。`audio_out` と同じ時刻に 1 標本ずつ出す。
    /// ホストが取り出す。保存には入れない。
    #[serde(skip)]
    pub line_out: Vec<(i16, i16)>,
}

impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}

/// 番地とアクセスの幅から UDS・LDS。
fn lanes(addr: u32, size: Size) -> (bool, bool) {
    match size {
        Size::Word => (true, true),
        Size::Byte => (addr & 1 == 0, addr & 1 != 0),
    }
}

/// メモリ（byte の配列、big-endian）の読出し。
fn mem_read(m: &[u8], off: usize, size: Size) -> u16 {
    match size {
        Size::Byte => u16::from(m[off]),
        Size::Word => u16::from_be_bytes([m[off & !1], m[(off & !1) + 1]]),
    }
}

fn mem_write(m: &mut [u8], off: usize, size: Size, v: u16) {
    match size {
        Size::Byte => m[off] = v as u8,
        Size::Word => {
            let [hi, lo] = v.to_be_bytes();
            m[off & !1] = hi;
            m[(off & !1) + 1] = lo;
        }
    }
}

/// 上位バイトレーンの 8 bit の装置の値を、アクセスの幅に合わせる。下位レーンはプルアップで 0xFF [P]。
fn upper_lane(v: u8, size: Size) -> u16 {
    match size {
        Size::Byte => u16::from(v),
        Size::Word => (u16::from(v) << 8) | 0xFF,
    }
}

/// ACRTC のアクセスの T1（ACRTC が CS を認識する 2CLK の立上り）と、68000 の待ち。`edge` は 2CLK の
/// 立上りの 1 つ（位相の基準）。CS は S4 の立上りで登録し（DR-016）、T1 の次の立上りの後に DTACK が出る
/// （host-acrtc-interface.md §5.1。遅れは中央の値）。
pub fn acrtc_timing(s0: Time, edge: Time) -> (Time, u32) {
    let p = PERIOD_2CLK;
    let at_or_after = |t: Time| {
        if t <= edge {
            edge - (edge - t) / p * p
        } else {
            edge + (t - edge).div_ceil(p) * p
        }
    };
    let t1 = at_or_after(s0 + 2 * PERIOD_8M + ACRTC_CS + ACRTC_CS_SETUP);
    let ack = t1 + p + ACRTC_DTACK;
    let (_, waits) = doc::host_recognize(s0, ack + CPU_DTACK_SETUP - 1);
    (t1, waits)
}

/// 1 回のアクセスの結果（値と待ち）。`None` は DTACK を返さない（バスの時間切れでバスエラー）。
type Access = Option<(u16, u32)>;

const fn ok(v: u16, waits: u32) -> Access {
    Some((v, waits))
}

impl System {
    pub fn new() -> Self {
        Self {
            fabric: Fabric::new(),
            ports: FabricPorts::new(),
            boot_rom: vec![0xFF; BOOT_ROM_BYTES],
            main_ram: vec![0; MAIN_RAM_BYTES],
            cart: None,
            acia: Hd6850::new(),
            acia_clock: 0,
            input: Input::default(),
            dsp_irq_en: false,
            temporal_sync: false,
            temporal_rsync: false,
            vblank_prev: false,
            log: BusLog::default(),
            bends: Bends::default(),
            doc_trace: Vec::new(),
            audio_out: Vec::new(),
            next_sample: 0,
            sample_period: fc85_core::time::UNITS_PER_SECOND / Time::from(AUDIO_RATE),
            analog: Analog::new(),
            line_out: Vec::new(),
        }
    }

    /// 次に進めるスロットの頭の時刻。
    pub fn now(&self) -> Time {
        self.ports.now()
    }

    /// 1 スロット進める。
    pub fn step_slot(&mut self) {
        self.fabric.step_slot(&mut self.ports);
        let (vblank, _) = self.ports.acrtc.video.blank_levels();
        if vblank && !self.vblank_prev {
            if self.temporal_sync {
                self.ports.temporal.wptr = 0;
            }
            if self.temporal_sync || self.temporal_rsync {
                self.ports.temporal.rptr = 0;
            }
        }
        self.vblank_prev = vblank;
        if self.analog.due(self.ports.now()) {
            self.line();
        }
        while self.next_sample < self.ports.now() {
            let s = self.mix();
            self.audio_out.push(s);
            self.next_sample += self.sample_period;
        }
    }

    /// 時刻 `t` を含むスロットまで進める（`now ≤ t < now＋SLOT`）。
    pub fn sync_time(&mut self, t: Time) {
        while self.ports.now() + SLOT <= t {
            self.step_slot();
        }
    }

    /// CPU クロック `clock` まで進める。ACIA もここで進める。
    pub fn sync(&mut self, clock: u64) {
        self.sync_time(clock * PERIOD_8M);
        let n = clock.saturating_sub(self.acia_clock) / 16;
        if n > 0 {
            self.acia.clock(n as u32);
            self.acia_clock += n * 16;
        }
    }

    // ------------------------------------------------------------------ 割込み

    /// 割込みのレベルごとの要求（bit n がレベル n）。DR-028 (4)。
    pub fn irq_levels(&self) -> u8 {
        let p = &self.ports;
        let mut l = 0u8;
        if p.acrtc.video.irq() || p.acrtc.acrtc.irq() {
            l |= 1 << 6;
        }
        if self.acia.irq() {
            l |= 1 << 5;
        }
        if p.doc && p.doc_board.doc.irq() {
            l |= 1 << 4;
        }
        if self.fabric.irq() || p.adpcm_port.irq(&p.adpcm) {
            l |= 1 << 3;
        }
        if self.dsp_irq_en && p.dsp.irq() {
            l |= 1 << 2;
        }
        if self.input.status & self.input.enable & (INPUT_PORT_A | INPUT_PORT_B | INPUT_EXTRIG) != 0
        {
            l |= 1 << 1;
        }
        l
    }

    /// 74LS148 の出力（いちばん高いレベル）。
    pub fn ipl_level(&self) -> u8 {
        let l = self.irq_levels();
        if l == 0 {
            0
        } else {
            7 - l.leading_zeros() as u8
        }
    }

    // ------------------------------------------------------------------ 待ち

    /// VPA のサイクル（6800 系の周辺とオートベクタ）の待ち。E は 10 クロックの周期で回り続け、E↑ は
    /// 10k＋5.5 クロックの立下り [P]。S0 から認識（2.5 クロック）まで、認識から E↑ まで d（3〜12）、
    /// E の High 4、S7 の半分を足して 7＋d クロック（台帳 HD68000.md §2.5）。
    pub fn vpa_waits(at: u64) -> u32 {
        let rec = 2 * at + 5;
        let k = (rec + 6).saturating_sub(11).div_ceil(20);
        let e = 20 * k + 11;
        let d = ((e - rec) / 2) as u32;
        3 + d
    }

    /// Staging RAM の待ち（DR-024）：500 ns の枠の後半が CPU。S0 の枠の中の位置で 0、0、2、1。
    pub fn staging_waits(at: u64) -> u32 {
        [0, 0, 2, 1][(at % 4) as usize]
    }

    /// Main RAM のリフレッシュとぶつかったときの待ち（DR-026）。
    fn main_ram_waits(&self, at: u64) -> u32 {
        let hs = self.ports.acrtc.video.hsync_at;
        if !self.ports.acrtc.acrtc.display_running() {
            return 0;
        }
        let t = at * PERIOD_8M;
        let end = hs + REFRESH;
        if t >= hs && t + PERIOD_8M < end {
            (end - (t + PERIOD_8M)).div_ceil(PERIOD_8M) as u32
        } else {
            0
        }
    }

    // ------------------------------------------------------------------ 解読

    fn access(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        let addr = addr & 0xFF_FFFF;
        match addr {
            0x00_0000..=0x00_FFFF => {
                self.sync(at);
                // ROM への書込みは捨てる（DTACK は返す）[P]（DR-052）
                ok(mem_read(&self.boot_rom, addr as usize, size), 1)
            }
            0x01_0000..=0x01_FFFF => {
                self.sync(at);
                let i = ((addr - 0x1_0000) >> 1) as usize & 0x7FFF;
                let v = match lanes(addr, size) {
                    (true, _) => {
                        let p = &mut self.ports;
                        upper_lane(p.char_lines.read(&p.char_rom, i), size)
                    }
                    (false, _) => 0xFF,
                };
                ok(v, 1)
            }
            0x10_0000..=0x11_FFFF => {
                self.sync(at);
                let w = self.main_ram_waits(at);
                let off = (addr - 0x10_0000) as usize;
                match write {
                    Some(v) => {
                        mem_write(&mut self.main_ram, off, size, v);
                        ok(0, w)
                    }
                    None => ok(mem_read(&self.main_ram, off, size), w),
                }
            }
            0x12_0000..=0x12_FFFF => {
                self.sync(at);
                let w = Self::staging_waits(at);
                let off = (addr - 0x12_0000) as usize;
                match write {
                    Some(v) => {
                        mem_write(&mut self.ports.staging, off, size, v);
                        ok(0, w)
                    }
                    None => ok(mem_read(&self.ports.staging, off, size), w),
                }
            }
            0x20_0000..=0x20_0003 => self.acrtc(at, addr, size, write),
            0x21_0000..=0x21_01FF => self.doc(at, addr, size, write),
            0x22_0000..=0x22_001F => self.dsp_bridge(at, addr, size, write),
            0x23_0000..=0x23_001F => self.adpcm_bridge(at, addr, size, write),
            0x24_0000..=0x24_003F => self.temporal(at, addr, size, write),
            0x25_0000..=0x25_0FFF => self.fabric_regs(at, addr, size, write),
            0x26_0000..=0x26_00FF => self.input_regs(at, addr, size, write),
            0x27_0000..=0x27_003F => self.video_regs(at, addr, size, write),
            0x28_0000..=0x28_1FFF => {
                self.sync(at);
                if size == Size::Byte {
                    return None;
                }
                let word = ((addr - 0x28_0000) >> 1) as u16;
                match write {
                    Some(v) => self.ports.dsp.prog_write(word, v).then_some((0, 0)),
                    None => self.ports.dsp.prog_read(word).map(|v| (v, 0)),
                }
            }
            0x40_0000..=0x4F_FFFF => {
                self.sync(at);
                let cart = self.cart.as_ref()?;
                let off = (addr - 0x40_0000) as usize;
                let v = if off | 1 < cart.len() {
                    mem_read(cart, off, size)
                } else {
                    match size {
                        Size::Byte => 0xFF,
                        Size::Word => 0xFFFF,
                    }
                };
                // カートリッジへの書込みは捨てる [P]
                ok(v, 1)
            }
            _ => None,
        }
    }

    /// ACRTC（DR-016）：CS を S4 の立上りで登録し、ACRTC が 2CLK の立上り（T1）で認識し、次の立上りの
    /// 後に DTACK を出す（host-acrtc-interface.md §5.1、遅れは中央の値）。
    fn acrtc(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        self.sync(at);
        if size == Size::Byte {
            return None;
        }
        let rs = addr & 2 != 0;
        let s0 = at * PERIOD_8M;
        let (t1, waits) = acrtc_timing(s0, self.ports.acrtc.edge_at_or_after(s0));
        self.sync_time(t1);
        let a = &mut self.ports.acrtc.acrtc;
        let v = match write {
            Some(v) => {
                a.write(rs, v);
                0
            }
            None => a.read(rs),
        };
        ok(v, waits)
    }

    /// DOC のブリッジ（DR-051）。DTACK まで進め、時間切れならバスエラー。
    fn doc(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        self.sync(at);
        if self.doc_trace.len() < 8 {
            self.doc_trace.push((addr, write.is_some()));
        }
        let s0 = at * PERIOD_8M;
        let (uds, lds) = lanes(addr, size);
        let w = write.map(|v| match size {
            Size::Byte => v << 8,
            Size::Word => v,
        });
        if self.ports.doc_host_begin(s0, addr, uds, lds, w) == HostReply::BusError {
            return None;
        }
        let berr = s0 + BERR_HALF_CLOCKS * PERIOD_8M / 2;
        loop {
            self.step_slot();
            if let Some(d) = self.ports.doc_host_poll() {
                if d.recognized >= berr {
                    return None;
                }
                return ok(u16::from(d.read.unwrap_or(0)), d.waits);
            }
            if self.ports.now() > berr {
                self.ports.doc_board.host_abort();
                return None;
            }
        }
    }

    /// DSP bridge（DR-029、DR-049、DR-053）。
    fn dsp_bridge(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        self.sync(at);
        if size == Size::Byte {
            return None;
        }
        let p = &mut self.ports;
        match (addr & 0x1F, write) {
            (0x00, Some(v)) => {
                let bio = match (v >> 1) & 3 {
                    0 => BioSel::InputReady,
                    1 => BioSel::OutputSpace,
                    2 => BioSel::Mailbox,
                    _ => BioSel::AudioTick,
                };
                p.dsp.set_ctrl(DspCtrl {
                    reset: v & 1 != 0,
                    bio,
                });
                self.dsp_irq_en = v & 8 != 0;
                ok(0, 0)
            }
            (0x00, None) => {
                let c = p.dsp.ctrl();
                let bio = match c.bio {
                    BioSel::InputReady => 0,
                    BioSel::OutputSpace => 1,
                    BioSel::Mailbox => 2,
                    BioSel::AudioTick => 3,
                };
                ok(
                    u16::from(c.reset) | (bio << 1) | (u16::from(self.dsp_irq_en) << 3),
                    0,
                )
            }
            (0x02, None) => {
                let s = p.dsp.status(&p.dsp_in, &p.dsp_out);
                let v = u16::from(s.prgbus)
                    | (u16::from(s.input_ready) << 1)
                    | (u16::from(s.output_space) << 2)
                    | (u16::from(s.command) << 3)
                    | (u16::from(s.response) << 4)
                    | (u16::from(s.underrun) << 5)
                    | (u16::from(s.overrun) << 6);
                ok(v, 0)
            }
            (0x02, Some(v)) => {
                p.dsp.clear_flags(v & 0x20 != 0, v & 0x40 != 0);
                ok(0, 0)
            }
            (0x04, Some(v)) => {
                p.dsp.write_mailbox(v);
                ok(0, 0)
            }
            (0x04, None) => ok(0, 0),
            (0x06, None) => ok(p.dsp.read_response(), 0),
            (0x06, Some(_)) => ok(0, 0),
            _ => None,
        }
    }

    /// ADPCM bridge と本体 DAC（DR-017、DR-019、DR-053）。
    fn adpcm_bridge(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        self.sync(at);
        if size == Size::Byte {
            return None;
        }
        let p = &mut self.ports;
        let off = addr & 0x1F;
        if off >= 0x10 && p.doc {
            return None;
        }
        match (off, write) {
            (0x00, Some(v)) => {
                p.adpcm_port.set_ctrl(AdpcmCtrl {
                    reset: v & 1 != 0,
                    s1: v & 2 != 0,
                    s2: v & 4 != 0,
                    low_first: v & 8 != 0,
                    no_replace: v & 0x10 != 0,
                    half_irq: v & 0x20 != 0,
                });
                ok(0, 0)
            }
            (0x00, None) => {
                let c = p.adpcm_port.ctrl();
                let v = u16::from(c.reset)
                    | (u16::from(c.s1) << 1)
                    | (u16::from(c.s2) << 2)
                    | (u16::from(c.low_first) << 3)
                    | (u16::from(c.no_replace) << 4)
                    | (u16::from(c.half_irq) << 5);
                ok(v, 0)
            }
            (0x02, None) => {
                let n = p.adpcm.len() as u16;
                let v = n | (u16::from(p.adpcm_port.underrun) << 8) | (u16::from(n <= 32) << 9);
                ok(v, 0)
            }
            (0x02, Some(v)) => {
                if v & 0x100 != 0 {
                    p.adpcm_port.underrun = false;
                }
                ok(0, 0)
            }
            (0x10, Some(v)) => {
                p.main_dac.direct = v & 1 != 0;
                ok(0, 0)
            }
            (0x10, None) => ok(u16::from(p.main_dac.direct), 0),
            (0x12, None) => {
                let v = p.dac.len() as u16 | (u16::from(p.main_dac.underrun) << 8);
                ok(v, 0)
            }
            (0x12, Some(v)) => {
                if v & 0x100 != 0 {
                    p.main_dac.underrun = false;
                }
                ok(0, 0)
            }
            (0x14, Some(v)) => {
                if p.main_dac.direct {
                    p.main_dac.latch = v;
                }
                ok(0, 0)
            }
            (0x14, None) => ok(p.main_dac.latch, 0),
            _ => None,
        }
    }

    /// Temporal RAM のポインタ（DR-053）。Capture RAM（0x240020〜）は [O] でバスエラー。
    fn temporal(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        self.sync(at);
        if size == Size::Byte {
            return None;
        }
        let t = &mut self.ports.temporal;
        match (addr & 0x3F, write) {
            (0x00, Some(v)) => t.wptr = v,
            (0x00, None) => return ok(t.wptr, 0),
            (0x02, Some(v)) => t.rptr = v,
            (0x02, None) => return ok(t.rptr, 0),
            (0x04, Some(v)) => {
                if v & 1 != 0 {
                    t.wptr = 0;
                    t.rptr = 0;
                    t.overrun = false;
                    t.underrun = false;
                }
                self.temporal_sync = v & 2 != 0;
                self.temporal_rsync = v & 4 != 0;
                t.hold = v & 8 != 0;
                // W1C（DR-067）
                if v & 0x100 != 0 {
                    t.overrun = false;
                }
                if v & 0x200 != 0 {
                    t.underrun = false;
                }
            }
            (0x04, None) => {
                let v = u16::from(self.temporal_sync) << 1
                    | u16::from(self.temporal_rsync) << 2
                    | u16::from(t.hold) << 3
                    | u16::from(t.wptr == t.rptr) << 4
                    | u16::from(t.overrun) << 8
                    | u16::from(t.underrun) << 9;
                return ok(v, 0);
            }
            (0x06, None) => return ok(t.wptr.wrapping_sub(t.rptr), 0),
            (0x06, Some(_)) => {}
            _ => return None,
        }
        ok(0, 0)
    }

    /// Data Fabric のレジスタ（DR-046 (10)、fabric.md §09）。A ファイルは許可のスロットの 3/4 で DTACK。
    fn fabric_regs(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        self.sync(at);
        if size == Size::Byte {
            return None;
        }
        let off = (addr - 0x25_0000) as u16;
        let r = match write {
            Some(v) => self.fabric.host_write(off, v, &mut self.ports),
            None => self.fabric.host_read(off, &self.ports),
        };
        let r = match r {
            HostReply::Wait => loop {
                self.step_slot();
                if let Some(r) = self.fabric.host_poll() {
                    let dtack = self.ports.now() - SLOT / 4;
                    let (_, waits) = doc::host_recognize(at * PERIOD_8M, dtack);
                    break match r {
                        HostReply::Data(v) => return ok(v, waits),
                        HostReply::Ack => return ok(0, waits),
                        r => r,
                    };
                }
            },
            r => r,
        };
        match r {
            HostReply::Ack => ok(0, 0),
            HostReply::Data(v) => ok(v, 0),
            _ => None,
        }
    }

    /// 入力と ACIA（仕様書 §09、DR-028、DR-053）。
    fn input_regs(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        self.sync(at);
        let off = addr & 0xFF;
        if off == 0x14 || off == 0x16 {
            // ACIA は VPA のサイクル。上位バイトレーン（UDS）だけ
            if !lanes(addr, size).0 {
                return None;
            }
            let rs = off == 0x16;
            let waits = Self::vpa_waits(at);
            return match write {
                Some(v) => {
                    let b = match size {
                        Size::Byte => v as u8,
                        Size::Word => (v >> 8) as u8,
                    };
                    self.acia.write(rs, b);
                    ok(0, waits)
                }
                None => ok(upper_lane(self.acia.read(rs), size), waits),
            };
        }
        if size == Size::Byte {
            return None;
        }
        let i = &mut self.input;
        match (off, write) {
            (0x00, None) => ok(i.port_a, 0),
            (0x02, None) => ok(i.port_b, 0),
            (0x04..=0x12, None) => ok(i.axis[((off - 4) / 2) as usize], 0),
            (0x00..=0x12, Some(_)) => ok(0, 0),
            (0x18, None) => {
                let midi = if self.acia.irq() { INPUT_MIDI } else { 0 };
                ok(i.status | midi, 0)
            }
            (0x18, Some(v)) => {
                i.status &= !(v & (INPUT_PORT_A | INPUT_PORT_B | INPUT_EXTRIG));
                ok(0, 0)
            }
            (0x1A, None) => ok(i.enable, 0),
            (0x1A, Some(v)) => {
                i.enable = v & (INPUT_PORT_A | INPUT_PORT_B | INPUT_EXTRIG);
                ok(0, 0)
            }
            _ => None,
        }
    }

    /// パレットと映像の割込み（DR-027、DR-053）。語アクセスだけ。
    fn video_regs(&mut self, at: u64, addr: u32, size: Size, write: Option<u16>) -> Access {
        self.sync(at);
        if size == Size::Byte {
            return None;
        }
        let v = &mut self.ports.acrtc.video;
        let off = addr & 0x3F;
        match (off, write) {
            (0x00..=0x1F, Some(x)) => {
                // 写しの間の書込みは、写しが終わるまで待つ
                let mut waits = 0;
                if !v.direct && self.ports.acrtc.acrtc.display_running() {
                    let t = (at + 2) * PERIOD_8M;
                    let end = v.hsync_at + PALETTE_COPY;
                    if t >= v.hsync_at && t < end {
                        waits = (end - t).div_ceil(PERIOD_8M) as u32;
                    }
                }
                // 書込みのストローブは S4〜S7（DIRECT の点状のノイズ）[P]
                let from = (at + 2) * PERIOD_8M;
                let to = (at + 4 + u64::from(waits)) * PERIOD_8M;
                v.write_palette_at((off >> 1) as usize, x, from, to);
                ok(0, waits)
            }
            // パレットは書込み専用で、読むと 0 [P]
            (0x00..=0x1F, None) => ok(0, 0),
            (0x20, Some(x)) => {
                v.direct = x & 1 != 0;
                ok(0, 0)
            }
            (0x20, None) => ok(u16::from(v.direct), 0),
            (0x22, Some(x)) => {
                v.write_irq(x);
                ok(0, 0)
            }
            (0x22, None) => ok(v.read_irq(), 0),
            (0x24, None) => ok(v.raster, 0),
            (0x24, Some(_)) => ok(0, 0),
            (0x26, Some(x)) => {
                v.line_cmp = x & 0x0FFF;
                ok(0, 0)
            }
            (0x26, None) => ok(v.line_cmp, 0),
            _ => None,
        }
    }

    fn reply(&mut self, addr: u32, write: bool, a: Access) -> BusReply {
        match a {
            Some((data, waits)) => BusReply::Ok { data, waits },
            None => {
                self.log.bus_errors += 1;
                self.log.last_bus_error = Some((addr & 0xFF_FFFF, write));
                BusReply::BusError {
                    clocks: BERR_CYCLE_CLOCKS,
                }
            }
        }
    }

    // ------------------------------------------------------------------ 音声

    /// 音声の 1 標本（DR-054）。DOC はチャネルの平均を偶数＝L、奇数＝R に足す。子基板がないときは本体 DAC。
    /// ADPCM は両方に半分ずつ。
    fn mix(&mut self) -> (i16, i16) {
        let p = &mut self.ports;
        let (mut l, mut r) = (0i32, 0i32);
        if p.doc {
            let ch = p.doc_board.take_channel_average();
            for (c, v) in ch.iter().enumerate() {
                if c % 2 == 0 {
                    l += v;
                } else {
                    r += v;
                }
            }
            l /= 4;
            r /= 4;
        } else {
            l += (i32::from(p.main_dac.left()) - 128) << 7;
            r += (i32::from(p.main_dac.right()) - 128) << 7;
        }
        let a = i32::from(p.adpcm_port.msm.dac_code()) << 3;
        l += a / 2;
        r += a / 2;
        let c = |x: i32| x.clamp(i16::MIN.into(), i16::MAX.into()) as i16;
        (c(l), c(r))
    }

    /// LINE OUT（analog.rs）：192 kHz の刻みを `now` の前まで進め、48 kHz の標本を `line_out` に出す。
    /// 標本の時刻の条件は `mix` と同じ（`next_sample` から始める）。入力は刻みの間の平均：DOC の S/H の和は
    /// 子基板が積分したもの、本体 DAC のラッチは最後の拍の時刻に変わったとし（拍がなければ刻みの時刻。
    /// DIRECT の書込み）、DA OUT は変わった時刻をアダプタが記録する。
    #[inline(never)]
    fn line(&mut self) {
        let now = self.ports.now();
        loop {
            let t = self.analog.next_tick(self.next_sample);
            if t >= now {
                break;
            }
            let p = &mut self.ports;
            let a = &mut self.analog;
            for (h, u) in a
                .dac
                .iter_mut()
                .zip([p.main_dac.left(), p.main_dac.right()])
            {
                let v = i64::from(u) - 128;
                if v != h.value() {
                    let at = p.audio.beat_at().filter(|&b| b + TICK > t).unwrap_or(t);
                    h.set(at, v);
                }
            }
            let da = i64::from(p.adpcm_port.da_out);
            if da != a.da.value() {
                a.da.set(p.adpcm_port.da_at, da);
            }
            let doc = p.doc_board.take_line(t);
            let dac = [a.dac[0].take(t), a.dac[1].take(t)];
            let da = a.da.take(t);
            let span = TICK as f64;
            let inp = analog::Inputs {
                doc: if p.doc {
                    [doc[0] as f64 / span, doc[1] as f64 / span]
                } else {
                    [0.0; 2]
                },
                dac: [dac[0] as f64 / span, dac[1] as f64 / span],
                dac_on: !p.doc,
                da: da as f64 / span,
            };
            if let Some(s) = a.tick(&inp) {
                self.line_out.push(s);
            }
        }
    }

    // ------------------------------------------------------------------ ホストから

    /// ベンド点の設定を回路に入れる（Bend モード）か、工場出荷に戻す（通常モード）。DR-065。
    /// 時刻と IC の状態はそのまま続く（クロックは次の周期から変わる）。
    pub fn apply_bends(&mut self) {
        let on = self.bends.mode == Mode::Bend;
        let p = &mut self.ports;
        p.char_lines = if on {
            self.bends.char_lines.clone()
        } else {
            RomLines::default()
        };
        let [doc, dsp, adpcm, audio] = if on {
            self.bends.clocks
        } else {
            Default::default()
        }
        .periods();
        p.doc_board.set_clock_period(doc);
        p.doc_board.sig_wiring = if on { self.bends.sig } else { SigWiring::Both };
        p.dsp.set_input_period(4 * dsp);
        p.adpcm_port.set_input_period(adpcm);
        p.audio.set_input_period(audio);
        let pins = if on {
            self.bends.fifos
        } else {
            [FifoPins::default(); 4]
        };
        for (f, pin) in [&mut p.dac, &mut p.dsp_in, &mut p.dsp_out, &mut p.adpcm]
            .into_iter()
            .zip(pins)
        {
            f.pins = pin;
        }
    }

    /// ベンド点の命令（`Bends::command`）を 1 行実行し、回路に入れる。
    pub fn bend(&mut self, line: &str) -> Result<(), String> {
        self.bends.command(line)?;
        self.apply_bends();
        Ok(())
    }

    /// ホストの入力：PORT_A・PORT_B が変わったら INPUT_IRQ_STATUS の bit を立てる。
    pub fn set_ports(&mut self, a: u16, b: u16) {
        if a != self.input.port_a {
            self.input.status |= INPUT_PORT_A;
        }
        if b != self.input.port_b {
            self.input.status |= INPUT_PORT_B;
        }
        self.input.port_a = a;
        self.input.port_b = b;
    }

    /// RESET 命令：周辺をリセットする [P]（DR-052）。ACRTC、DSP と ADPCM の CTRL、本体 DAC、映像と
    /// 入力の割込みの許可、DOC（子基板の RESET）。ACIA には RESET の端子がない。Fabric は扱わない [V]。
    pub fn peripheral_reset(&mut self) {
        let p = &mut self.ports;
        p.acrtc.acrtc.reset();
        p.dsp.set_ctrl(DspCtrl::default());
        p.adpcm_port.set_ctrl(AdpcmCtrl::default());
        p.main_dac.direct = false;
        p.acrtc.video.write_irq(0);
        p.acrtc.video.direct = false;
        if p.doc {
            p.doc_board.doc.set_reset(true);
            p.doc_board.doc.set_reset(false);
        }
        self.dsp_irq_en = false;
        self.input.enable = 0;
        self.temporal_sync = false;
        self.temporal_rsync = false;
        self.ports.temporal.hold = false;
    }
}

impl Bus for System {
    fn read(&mut self, at: u64, addr: u32, _fc: u8, size: Size) -> BusReply {
        let a = self.access(at, addr, size, None);
        self.reply(addr, false, a)
    }

    fn write(&mut self, at: u64, addr: u32, _fc: u8, size: Size, value: u16) -> BusReply {
        let a = self.access(at, addr, size, Some(value));
        self.reply(addr, true, a)
    }

    fn iack(&mut self, at: u64, _level: u8) -> IackReply {
        self.sync(at);
        self.log.iacks += 1;
        IackReply::Autovector {
            waits: Self::vpa_waits(at),
        }
    }

    fn ipl(&mut self, at: u64) -> u8 {
        self.sync(at);
        self.ipl_level()
    }

    fn reset_out(&mut self, at: u64) {
        self.sync(at);
        self.peripheral_reset();
    }
}

/// 本体：68000 と、その外側。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Console {
    pub cpu: Hd68000,
    pub sys: System,
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Console {
    pub fn new() -> Self {
        Self {
            cpu: Hd68000::new(),
            sys: System::new(),
        }
    }

    /// 電源投入：68000 のリセットの例外処理（SSP と PC を読む）。
    pub fn power_on(&mut self) {
        self.cpu.reset(&mut self.sys);
    }

    /// 1 命令（または例外処理）を実行する。かかった CPU クロックを返す。基板は最後のバスサイクル
    /// までしか進まないので、続けて `run_until` か `sys.sync` で追いつかせる。
    pub fn step(&mut self) -> u32 {
        let n = self.cpu.step(&mut self.sys);
        if !self.sys.doc_trace.is_empty() {
            self.check_doc_rule();
        }
        n
    }

    /// CPU クロック `clock` まで命令を実行し、基板もそこまで進める。
    pub fn run_until(&mut self, clock: u64) {
        while self.cpu.clock < clock {
            self.step();
        }
        self.sys.sync(clock);
    }

    /// DR-030 の規約を 1 命令ごとに確かめる。TST・BTST・CMP で 0xE0・0xE2 を読む違反は、
    /// 1 回の読みなので見分けない [V]。
    fn check_doc_rule(&mut self) {
        let t = std::mem::take(&mut self.sys.doc_trace);
        let rmw = t
            .iter()
            .any(|&(a, w)| !w && t.iter().any(|&(b, w2)| w2 && a == b));
        if rmw || t.len() > 1 {
            self.sys.log.doc_rule_warnings += 1;
            self.sys.log.last_doc_rule = Some(t[0].0);
        }
    }

    /// 次のフレーム（ACRTC のラスタが 0 に戻る）まで進める。上限は 2 フレーム分。
    /// 表示が止まっていれば約 1/60 秒。
    pub fn run_frame(&mut self) {
        const FRAME: u64 = 8_000_000 / 60;
        let f = self.sys.ports.acrtc.video.frames;
        let start = self.cpu.clock;
        while self.sys.ports.acrtc.video.frames == f {
            let limit = if self.sys.ports.acrtc.acrtc.display_running() {
                start + 2 * FRAME
            } else {
                start + FRAME
            };
            if self.cpu.clock >= limit {
                break;
            }
            self.run_until((self.cpu.clock + 64).min(limit));
        }
    }
}
