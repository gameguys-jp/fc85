//! fc85 の窓口（DR-092 (4)・(5)、DR-097）。core・chips・board の上で、外から仮想機を動かす。
//!
//! - `Machine`：仮想機（board 層の `Console`）と、テレビの模型、ラベル、音の溜め。ROM を読み、フレームを
//!   進め、画面と音を取り出し、メモリとレジスタを読み書きし、状態を保存・復元する。
//! - `protocol`：JSON の手順（1 行に 1 つの要求と答え。JSON-RPC 2.0 の形）。ほかのソフトと Web の GUI が使う。
//! - `mcp`：同じ手順を MCP のツールとして出す（エージェントが使う）。
//!
//! 決定的：同じ ROM、同じ設定、同じ要求の列なら、同じ結果になる。`run` はホストの `Console::run_frame` と
//! 同じ区切り（64 クロックごとに基板を追いつかせる）で進めるので、止める条件に当たらなければ、
//! ホストで同じフレーム数を進めたときと同じ状態になる。仮想機の挙動は変えない（読むだけの数え方と、
//! 止める条件だけを足す）。

pub mod encode;
pub mod http;
pub mod mcp;
pub mod protocol;
pub mod symbols;

use std::collections::VecDeque;

use fc85_board::bend::Mode;
/// 音の標本化の頻度（Hz）。
pub use fc85_board::console::AUDIO_RATE;
use fc85_board::console::{BOOT_ROM_BYTES, Console, MAIN_RAM_BYTES};
use fc85_board::fabric::{CHANNELS, Phase};
use fc85_board::tv::Tv;
use fc85_board::video::{RASTER_H, RASTER_W};
use fc85_chips::es5503::OSCILLATORS;
use fc85_chips::hd68000::{Bus, BusReply, RunState, Size};
use fc85_core::time::PERIOD_21M;
use serde::Serialize;

pub use symbols::Symbols;

/// 窓口の手順の版。要求・答えの形を変えたら上げる（足すだけなら小さい方の数）。
pub const API_VERSION: &str = "0.1";

/// 音を溜める上限（48 kHz で 60 秒）。超えたら古い方から捨てる。
pub const AUDIO_KEEP: usize = AUDIO_RATE as usize * 60;

/// 1 回の `run` のフレーム数の上限（約 10 分）。
pub const MAX_FRAMES: u64 = 36_000;

/// 仮想機のフレームの頻度（Hz）。1 フレームはラスタ 342 ドット × 262 ライン、ドットは 2CLK（DR-002）。
pub const FRAME_RATE: f64 = fc85_core::time::UNITS_PER_SECOND as f64
    / (RASTER_W as u64 * RASTER_H as u64 * fc85_core::time::PERIOD_2CLK) as f64;

/// `pack_state` の頭の印と版。
pub const STATE_MAGIC: &[u8; 8] = b"FC85ST01";

/// 68000 の 1 µs のクロック数（8 MHz）。
pub const CPU_CLOCKS_PER_US: u64 = 8;

/// MIDI OUT の 1 byte と、ACIA が送り終えた時刻（電源を入れてからの µs。DR-126）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct MidiByte {
    pub us: u64,
    pub byte: u8,
}

/// 番地の範囲（CPU から見た番地。仕様書 §05）。
pub const BOOT_ROM: u32 = 0x00_0000;
pub const MAIN_RAM: u32 = 0x10_0000;
pub const STAGING: u32 = 0x12_0000;
pub const STAGING_BYTES: u32 = 0x1_0000;
pub const CART: u32 = 0x40_0000;
pub const CART_BYTES: u32 = 0x10_0000;
/// BIOS の版の語（bios-abi.md。DR-094）。
pub const BIOS_INFO_VERSION: usize = 0x480;

/// PORT_A のビット（DR-055）。
pub const PAD_BITS: [(&str, u16); 8] = [
    ("UP", 0x01),
    ("DOWN", 0x02),
    ("LEFT", 0x04),
    ("RIGHT", 0x08),
    ("A", 0x10),
    ("B", 0x20),
    ("C", 0x40),
    ("START", 0x80),
];

/// 仮想機の組み立て。
#[derive(Clone, Debug)]
pub struct Config {
    pub boot: Option<Vec<u8>>,
    pub chars: Option<Vec<u8>>,
    pub cart: Option<Vec<u8>>,
    /// DOC の子基板を挿す。
    pub doc: bool,
    /// ベンド点の命令（`Bends::command`）。1 つでも設定すると Bend モードで始まる（`mode …` があればそれに従う）。
    pub bends: Vec<String>,
    /// 取り出す音：true は LINE OUT（アナログの段）、false はチップの出力のデジタルの和（DR-054）。
    pub line: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            boot: None,
            chars: None,
            cart: None,
            doc: true,
            bends: Vec::new(),
            line: true,
        }
    }
}

/// 画面の切り出し方。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    /// 表示の範囲 256×192（64〜319 ドット、43〜234 ライン）。
    Active,
    /// ホストの窓と同じ 320×240（22 ドット、19 ラインから）。
    View,
    /// ラスタ全体 342×262（帰線を含む）。
    Raster,
}

impl Area {
    /// 幅と高さ。
    pub fn size(self) -> (usize, usize) {
        let (_, _, w, h) = self.rect();
        (w, h)
    }

    fn rect(self) -> (usize, usize, usize, usize) {
        match self {
            Area::Active => (64, 43, 256, 192),
            Area::View => (22, 19, 320, 240),
            Area::Raster => (0, 0, RASTER_W, RASTER_H),
        }
    }
}

/// メモリの空間（`peek`・`poke`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    /// CPU から見た番地のうち、メモリの窓（Boot ROM、Main RAM、Staging RAM、カートリッジ）。I/O は `bus_read`。
    Cpu,
    /// ACRTC の VRAM（128 KiB、語は big-endian）。
    Vram,
    /// DOC の波形 RAM（64 KiB）。
    Wave,
    /// DSP のプログラム RAM（4K 語、big-endian）。
    DspProg,
    /// Temporal RAM。
    Temporal,
    /// 文字 ROM。
    CharRom,
    /// パレットの表（いま表示に使っている 16 語、big-endian）。読むだけ。
    Palette,
}

impl Space {
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(match s {
            "cpu" => Space::Cpu,
            "vram" => Space::Vram,
            "wave" => Space::Wave,
            "dsp_prog" => Space::DspProg,
            "temporal" => Space::Temporal,
            "char_rom" => Space::CharRom,
            "palette" => Space::Palette,
            _ => {
                return Err(format!(
                    "空間 {s:?} はない（cpu、vram、wave、dsp_prog、temporal、char_rom、palette）"
                ));
            }
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Space::Cpu => "cpu",
            Space::Vram => "vram",
            Space::Wave => "wave",
            Space::DspProg => "dsp_prog",
            Space::Temporal => "temporal",
            Space::CharRom => "char_rom",
            Space::Palette => "palette",
        }
    }
}

/// `run` の止め方と数え方。
#[derive(Clone, Debug, Default)]
pub struct RunOptions {
    pub frames: u64,
    /// 入力の台本：(この実行の何フレーム目から, PORT_A)。次の行まで保つ。最初の行の前は今の値のまま。
    pub input: Vec<(u64, u16)>,
    pub port_b: Option<u16>,
    /// この番地の命令を実行する前に止める。
    pub break_pc: Vec<u32>,
    /// バスエラーが起きたら、その命令の後で止める。
    pub stop_on_bus_error: bool,
    /// 68000 が止まっても（二重バスフォルト）フレームを進め続ける。既定は止める。
    pub run_when_halted: bool,
    /// 待ちのループの番地の範囲 [from, to)。ここの命令の時間を CPU の空きとして数える（STOP の間も空き）。
    pub idle_pc: Vec<(u32, u32)>,
    /// DOC のオシレータが鳴り始め・止まったときを記録する（`RunReport::voices`）。
    pub voices: bool,
    /// メモリの見張り。中身が変わったら、その命令の後で止める（`RunReport::watch_hit`）。
    pub watch: Vec<Watch>,
    /// 最後に実行した命令をこの数だけ覚えて返す（`RunReport::trace`。最大 `MAX_TRACE`）。
    pub trace: usize,
}

/// メモリの見張りの 1 つ：`space` の `addr` から `len` byte。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watch {
    pub space: Space,
    pub addr: u32,
    pub len: u32,
}

/// 見張る byte の数の合計の上限と、命令の記録の上限。
pub const MAX_WATCH_BYTES: u32 = 4096;
pub const MAX_TRACE: usize = 4096;

/// 見張りに当たったとき。
#[derive(Clone, Debug, Serialize)]
pub struct WatchHit {
    pub space: &'static str,
    /// 見張りの頭の番地と、変わった最初の byte の番地。
    pub watch_addr: u32,
    pub addr: u32,
    pub label: Option<String>,
    /// 見張りの範囲の前と後の中身（16 進）。
    pub old: String,
    pub new: String,
    /// 変わったのを見つけた命令（この命令の間か、その前の基板の処理（DMA など）で変わった）。
    pub pc: u32,
    pub pc_label: Option<String>,
}

/// 命令の記録の 1 行。
#[derive(Clone, Debug, Serialize)]
pub struct TraceLine {
    pub pc: u32,
    pub label: Option<String>,
    pub cpu_clock: u64,
}

/// DOC の今の様子（`Machine::doc`）。
#[derive(Clone, Debug, Serialize)]
pub struct DocState {
    /// 有効なオシレータの数 N（0xE1 のレジスタから）。
    pub enabled: u8,
    pub oscillators: Vec<Oscillator>,
}

/// 1 つのオシレータのレジスタ（読んでも DOC は変わらない）。
#[derive(Clone, Debug, Serialize)]
pub struct Oscillator {
    /// 制御のレジスタの停止ビットが 0。
    pub on: bool,
    pub freq: u16,
    pub vol: u8,
    pub wave_ptr: u8,
    pub wave_size: u8,
    pub control: u8,
}

/// DOC のオシレータの発音の記録の上限（1 回の `run`）。
pub const MAX_VOICE_EVENTS: usize = 2000;

/// DOC のオシレータが鳴り始めたか止まった出来事。基板を追いつかせる区切り（64 クロック＝8 µs）ごとに、
/// 有効なオシレータ（0〜N−1）の制御のレジスタの停止ビットを見る [P]。
#[derive(Clone, Debug, Serialize)]
pub struct VoiceEvent {
    pub frame: u64,
    pub cpu_clock: u64,
    pub osc: u8,
    /// true は鳴り始め（停止ビットが 0 になった）、false は止まった。
    pub on: bool,
    /// 周波数（16 bit）、音量、波形の表の頭、表の大きさ（BS・T・R）、制御（モード、IE、チャネル）のレジスタ。
    pub freq: u16,
    pub vol: u8,
    pub wave_ptr: u8,
    pub wave_size: u8,
    pub control: u8,
}

/// 1 フレームの予算（割合 0〜1）。CPU は STOP と `idle_pc` の間を空きとして数える。
#[derive(Clone, Debug, Default, Serialize)]
pub struct Budget {
    /// CPU が働いていた割合（全体）と、いちばん重いフレームの値。
    pub cpu_busy: f64,
    pub cpu_busy_max: f64,
    /// ACRTC がコマンドを実行していた割合（`Hd63484::idle` でない間）。
    pub acrtc_busy: f64,
    pub acrtc_busy_max: f64,
    /// Data Fabric のどれかのチャネルが動いていた割合。
    pub fabric_busy: f64,
    pub fabric_busy_max: f64,
}

/// 音の要約（L、R）。
#[derive(Clone, Debug, Default, Serialize)]
pub struct AudioStats {
    pub samples: usize,
    pub seconds: f64,
    pub peak: [u16; 2],
    pub rms: [f64; 2],
}

impl AudioStats {
    pub fn of(pcm: &[(i16, i16)]) -> Self {
        let mut acc = AudioAcc::default();
        acc.add(pcm);
        acc.stats()
    }
}

/// 音の要約を少しずつ数える（長い `run` の音を全部は持たない）。
#[derive(Clone, Debug, Default)]
struct AudioAcc {
    n: usize,
    peak: [u16; 2],
    sq: [f64; 2],
}

impl AudioAcc {
    fn add(&mut self, pcm: &[(i16, i16)]) {
        for &(l, r) in pcm {
            for (i, v) in [l, r].into_iter().enumerate() {
                self.peak[i] = self.peak[i].max(v.unsigned_abs());
                self.sq[i] += f64::from(v) * f64::from(v);
            }
        }
        self.n += pcm.len();
    }

    fn stats(&self) -> AudioStats {
        let n = self.n.max(1) as f64;
        AudioStats {
            samples: self.n,
            seconds: self.n as f64 / f64::from(AUDIO_RATE),
            peak: self.peak,
            rms: [(self.sq[0] / n).sqrt(), (self.sq[1] / n).sqrt()],
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BusErrorInfo {
    pub addr: u32,
    pub label: Option<String>,
    pub write: bool,
}

/// `run` の結果。
#[derive(Clone, Debug, Serialize)]
pub struct RunReport {
    /// 止まった理由：frames（数え終えた）、breakpoint、halted（二重バスフォルト）、bus_error、watch。
    pub stop: &'static str,
    /// 最後まで進めたフレームの数。
    pub frames_run: u64,
    /// 電源投入からの ACRTC のフレームの数。
    pub frame: u64,
    pub cpu_clock: u64,
    pub pc: u32,
    pub pc_label: Option<String>,
    /// この実行の間の数。
    pub bus_errors: u64,
    pub last_bus_error: Option<BusErrorInfo>,
    pub bridge_timeouts: u64,
    pub doc_rule_warnings: u64,
    /// ACRTC の FIFO の規約（DR-096）に反したアクセスの数：満杯の書込み FIFO への書込みと、空の読出し
    /// FIFO の読出し（DR-121）。
    pub acrtc_rule_warnings: u64,
    pub budget: Budget,
    pub audio: AudioStats,
    /// 表示の範囲（256×192）の画素の FNV-1a。前と同じ画面かを比べる。
    pub screen_hash: String,
    /// テレビの模型の状態（同期と色が取れているか）。
    pub tv: String,
    /// DOC の発音の記録（`RunOptions::voices`。上限 `MAX_VOICE_EVENTS`）と、上限で捨てた数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voices: Option<Vec<VoiceEvent>>,
    #[serde(skip_serializing_if = "is_zero")]
    pub voices_dropped: u64,
    /// 見張りに当たったとき（`stop` は watch）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_hit: Option<WatchHit>,
    /// 最後に実行した命令（古い順。`RunOptions::trace`）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<TraceLine>>,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

#[derive(Clone, Debug, Serialize)]
pub struct Regs {
    pub d: [u32; 8],
    /// A0〜A7（A7 は SR の S で選んだ方の SP）。
    pub a: [u32; 8],
    pub usp: u32,
    pub ssp: u32,
    pub sr: u16,
    pub pc: u32,
    pub pc_label: Option<String>,
    /// running、stopped（STOP）、halted（二重バスフォルト）。
    pub state: &'static str,
    pub clock: u64,
    pub frame: u64,
}

/// 仮想機と、外から見るための道具。
pub struct Machine {
    pub console: Console,
    pub config: Config,
    pub symbols: Symbols,
    tv: Tv,
    pic: Vec<u32>,
    /// 取り出していない音（`config.line` の方）。
    audio: Vec<(i16, i16)>,
    /// ブレークポイントで止まった番地。次の `run` の最初の 1 命令は止めない。
    resume_pc: Option<u32>,
    /// 発音の記録：前に見たときに鳴っていたオシレータ（bit n）、記録、捨てた数。
    voice_on: u32,
    voice_log: Vec<VoiceEvent>,
    voice_dropped: u64,
    /// 見張りの前の中身（`RunOptions::watch` と同じ順）と、当たったとき。
    watch_old: Vec<Vec<u8>>,
    watch_hit: Option<WatchHit>,
    /// 命令の記録（PC、クロック）。
    trace_buf: VecDeque<(u32, u64)>,
}

fn state_name(s: RunState) -> &'static str {
    match s {
        RunState::Running => "running",
        RunState::Stopped => "stopped",
        RunState::Halted => "halted",
    }
}

/// 番地 `addr` を含む窓（CPU から見たメモリ）の配列と、その中の位置。
enum Window {
    Boot(usize),
    Ram(usize),
    Staging(usize),
    Cart(usize),
}

fn window(addr: u32) -> Option<Window> {
    let a = addr & 0xFF_FFFF;
    let off = |base: u32| (a - base) as usize;
    if a < BOOT_ROM + BOOT_ROM_BYTES as u32 {
        Some(Window::Boot(off(BOOT_ROM)))
    } else if (MAIN_RAM..MAIN_RAM + MAIN_RAM_BYTES as u32).contains(&a) {
        Some(Window::Ram(off(MAIN_RAM)))
    } else if (STAGING..STAGING + STAGING_BYTES).contains(&a) {
        Some(Window::Staging(off(STAGING)))
    } else if (CART..CART + CART_BYTES).contains(&a) {
        Some(Window::Cart(off(CART)))
    } else {
        None
    }
}

impl Machine {
    /// 組み立てて電源を入れる。ベンド点の命令が読めなければ誤り。
    pub fn new(config: Config) -> Result<Self, String> {
        let mut c = Console::new();
        c.sys.ports.doc = config.doc;
        if let Some(rom) = &config.boot {
            let n = rom.len().min(BOOT_ROM_BYTES);
            c.sys.boot_rom[..n].copy_from_slice(&rom[..n]);
        }
        if let Some(rom) = &config.chars {
            let n = rom.len().min(c.sys.ports.char_rom.len());
            c.sys.ports.char_rom[..n].copy_from_slice(&rom[..n]);
        }
        for cmd in &config.bends {
            c.sys
                .bends
                .command(cmd)
                .map_err(|e| format!("bend {cmd:?}: {e}"))?;
        }
        let explicit_mode = config
            .bends
            .iter()
            .any(|b| b.trim_start().starts_with("mode"));
        if !c.sys.bends.is_factory() && !explicit_mode {
            c.sys.bends.mode = Mode::Bend;
        }
        c.sys.apply_bends();
        c.sys.cart = config.cart.clone();
        c.power_on();
        Ok(Self {
            console: c,
            config,
            symbols: Symbols::default(),
            tv: Tv::default(),
            pic: Vec::new(),
            audio: Vec::new(),
            resume_pc: None,
            voice_on: 0,
            voice_log: Vec::new(),
            voice_dropped: 0,
            watch_old: Vec::new(),
            watch_hit: None,
            trace_buf: VecDeque::new(),
        })
    }

    /// 見張りの範囲を読む。
    fn read_watch(&self, w: &Watch) -> Vec<u8> {
        (0..w.len)
            .map(|i| {
                self.read_byte(w.space, w.addr.wrapping_add(i))
                    .unwrap_or(0xFF)
            })
            .collect()
    }

    /// 見張りの中身が変わったか。変わったら `watch_hit` に残し true。
    fn check_watch(&mut self, o: &RunOptions, pc: u32) -> bool {
        for (i, w) in o.watch.iter().enumerate() {
            let new = self.read_watch(w);
            if new == self.watch_old[i] {
                continue;
            }
            let k = new
                .iter()
                .zip(&self.watch_old[i])
                .position(|(a, b)| a != b)
                .unwrap_or(0) as u32;
            let addr = w.addr.wrapping_add(k);
            let old = std::mem::replace(&mut self.watch_old[i], new.clone());
            self.watch_hit = Some(WatchHit {
                space: w.space.name(),
                watch_addr: w.addr,
                addr,
                label: (w.space == Space::Cpu).then(|| self.label(addr)).flatten(),
                old: encode::hex(&old),
                new: encode::hex(&new),
                pc,
                pc_label: self.label(pc),
            });
            return true;
        }
        false
    }

    /// 鳴っている DOC のオシレータ（bit n）。子基板がなければ 0。
    fn voices_now(&self) -> u32 {
        let sys = &self.console.sys;
        if !sys.ports.doc {
            return 0;
        }
        let doc = &sys.ports.doc_board.doc;
        let n = usize::from(doc.enabled()).min(OSCILLATORS);
        (0..n)
            .filter(|&i| doc.control(i) & 1 == 0)
            .fold(0, |m, i| m | 1 << i)
    }

    /// DOC の今の様子：有効なオシレータの数と、各オシレータのレジスタ。子基板がなければ None。
    pub fn doc(&self) -> Option<DocState> {
        let sys = &self.console.sys;
        if !sys.ports.doc {
            return None;
        }
        let doc = &sys.ports.doc_board.doc;
        let n = usize::from(doc.enabled()).min(OSCILLATORS);
        Some(DocState {
            enabled: n as u8,
            oscillators: (0..n)
                .map(|i| Oscillator {
                    on: doc.control(i) & 1 == 0,
                    freq: doc.freq(i),
                    vol: doc.volume(i),
                    wave_ptr: doc.wave_ptr(i),
                    wave_size: doc.wave_size(i),
                    control: doc.control(i),
                })
                .collect(),
        })
    }

    /// 鳴り始め・止まったオシレータを記録する。
    fn watch_voices(&mut self) {
        let now = self.voices_now();
        let changed = now ^ self.voice_on;
        if changed == 0 {
            return;
        }
        let doc = &self.console.sys.ports.doc_board.doc;
        for i in 0..OSCILLATORS {
            if changed & (1 << i) == 0 {
                continue;
            }
            if self.voice_log.len() >= MAX_VOICE_EVENTS {
                self.voice_dropped += 1;
                continue;
            }
            self.voice_log.push(VoiceEvent {
                frame: self.console.sys.ports.acrtc.video.frames,
                cpu_clock: self.console.cpu.clock,
                osc: i as u8,
                on: now & (1 << i) != 0,
                freq: doc.freq(i),
                vol: doc.volume(i),
                wave_ptr: doc.wave_ptr(i),
                wave_size: doc.wave_size(i),
                control: doc.control(i),
            });
        }
        self.voice_on = now;
    }

    /// 同じ ROM と設定で電源を入れ直す（ラベルは残す）。
    pub fn reset(&mut self) -> Result<(), String> {
        let symbols = std::mem::take(&mut self.symbols);
        *self = Self::new(self.config.clone())?;
        self.symbols = symbols;
        Ok(())
    }

    pub fn frame(&self) -> u64 {
        self.console.sys.ports.acrtc.video.frames
    }

    pub fn label(&self, addr: u32) -> Option<String> {
        self.symbols.label(addr)
    }

    /// 仮想機の出した音を取り、溜める（もう一方の出力は捨てる）。`acc` があれば要約に足す。
    fn collect_audio(&mut self, acc: Option<&mut AudioAcc>) {
        let sys = &mut self.console.sys;
        let (keep, drop) = if self.config.line {
            (&mut sys.line_out, &mut sys.audio_out)
        } else {
            (&mut sys.audio_out, &mut sys.line_out)
        };
        drop.clear();
        if let Some(acc) = acc {
            acc.add(keep);
        }
        self.audio.append(keep);
        if self.audio.len() > AUDIO_KEEP {
            let cut = self.audio.len() - AUDIO_KEEP;
            self.audio.drain(..cut);
        }
    }

    /// 溜めた音を取り出す（`keep` なら残す）。
    pub fn take_audio(&mut self, keep: bool) -> Vec<(i16, i16)> {
        self.collect_audio(None);
        if keep {
            self.audio.clone()
        } else {
            std::mem::take(&mut self.audio)
        }
    }

    /// フレームを進める。止める条件に当たれば途中で返す。
    pub fn run(&mut self, o: &RunOptions) -> RunReport {
        let log0 = self.console.sys.log.clone();
        let acrtc0 = self.console.sys.ports.acrtc.acrtc.error_counts();
        if o.voices {
            self.voice_on = self.voices_now();
            self.voice_log.clear();
            self.voice_dropped = 0;
        }
        self.watch_old = o.watch.iter().map(|w| self.read_watch(w)).collect();
        self.watch_hit = None;
        self.trace_buf.clear();
        let mut acc = AudioAcc::default();
        let mut budget = Budget::default();
        let (mut total, mut idle, mut acrtc, mut fabric) = (0u64, 0u64, 0u64, 0u64);
        let mut stop = "frames";
        let mut frames_run = 0;
        for i in 0..o.frames.min(MAX_FRAMES) {
            if let Some(&(_, v)) = o.input.iter().rev().find(|e| e.0 <= i) {
                let b = o.port_b.unwrap_or(self.console.sys.input.port_b);
                self.console.sys.set_ports(v, b);
            } else if let Some(b) = o.port_b {
                let a = self.console.sys.input.port_a;
                self.console.sys.set_ports(a, b);
            }
            let (s, f) = self.run_one_frame(o);
            let n = f.total.max(1) as f64;
            budget.cpu_busy_max = budget.cpu_busy_max.max(1.0 - f.idle as f64 / n);
            budget.acrtc_busy_max = budget.acrtc_busy_max.max(f.acrtc as f64 / n);
            budget.fabric_busy_max = budget.fabric_busy_max.max(f.fabric as f64 / n);
            total += f.total;
            idle += f.idle;
            acrtc += f.acrtc;
            fabric += f.fabric;
            self.collect_audio(Some(&mut acc));
            if let Some(s) = s {
                stop = s;
                break;
            }
            frames_run += 1;
        }
        let n = total.max(1) as f64;
        budget.cpu_busy = 1.0 - idle as f64 / n;
        budget.acrtc_busy = acrtc as f64 / n;
        budget.fabric_busy = fabric as f64 / n;
        let audio = acc.stats();
        let log = &self.console.sys.log;
        let bus_errors = log.bus_errors - log0.bus_errors;
        let last_bus_error = if bus_errors > 0 {
            log.last_bus_error.map(|(addr, write)| BusErrorInfo {
                addr,
                label: None,
                write,
            })
        } else {
            None
        };
        let (bridge, doc_rule) = (
            log.bridge_timeouts - log0.bridge_timeouts,
            log.doc_rule_warnings - log0.doc_rule_warnings,
        );
        let acrtc1 = self.console.sys.ports.acrtc.acrtc.error_counts();
        let acrtc_rule = acrtc1.0.saturating_sub(acrtc0.0) + acrtc1.1.saturating_sub(acrtc0.1);
        let (_, _, rgb) = self.screen(Area::Active);
        let cpu = &self.console.cpu;
        RunReport {
            stop,
            frames_run,
            frame: self.frame(),
            cpu_clock: cpu.clock,
            pc: cpu.pc,
            pc_label: self.label(cpu.pc),
            bus_errors,
            last_bus_error: last_bus_error.map(|mut e| {
                e.label = self.label(e.addr);
                e
            }),
            bridge_timeouts: bridge,
            doc_rule_warnings: doc_rule,
            acrtc_rule_warnings: acrtc_rule,
            budget,
            audio,
            screen_hash: format!("{:016x}", encode::fnv1a(&rgb)),
            tv: self.tv.status(&self.console.sys).summary(),
            voices: o.voices.then(|| std::mem::take(&mut self.voice_log)),
            voices_dropped: self.voice_dropped,
            watch_hit: self.watch_hit.take(),
            trace: (o.trace > 0).then(|| {
                let buf = std::mem::take(&mut self.trace_buf);
                buf.into_iter()
                    .map(|(pc, cpu_clock)| TraceLine {
                        pc,
                        label: self.label(pc),
                        cpu_clock,
                    })
                    .collect()
            }),
        }
    }

    /// 1 フレーム（`Console::run_frame` と同じ区切り）。止まった理由と、数えたクロック。
    fn run_one_frame(&mut self, o: &RunOptions) -> (Option<&'static str>, FrameCount) {
        const FRAME: u64 = 8_000_000 / 60;
        let mut f = FrameCount::default();
        let c = &mut self.console;
        let frame = c.sys.ports.acrtc.video.frames;
        let start = c.cpu.clock;
        let p21 = c.sys.ports.acrtc.video_period();
        let slow = p21 == 0 || p21 > 2 * PERIOD_21M;
        let errors = c.sys.log.bus_errors;
        loop {
            let c = &mut self.console;
            if c.sys.ports.acrtc.video.frames != frame {
                break;
            }
            let limit = if c.sys.ports.acrtc.acrtc.display_running() && !slow {
                start + 2 * FRAME
            } else {
                start + FRAME
            };
            if c.cpu.clock >= limit {
                break;
            }
            let chunk = (c.cpu.clock + 64).min(limit);
            loop {
                let c = &mut self.console;
                if c.cpu.clock >= chunk {
                    break;
                }
                let (pc, st) = (c.cpu.pc, c.cpu.state);
                if st == RunState::Running && o.break_pc.contains(&pc) {
                    if self.resume_pc == Some(pc) {
                        self.resume_pc = None;
                    } else {
                        self.resume_pc = Some(pc);
                        f.total = c.cpu.clock - start;
                        return (Some("breakpoint"), f);
                    }
                }
                self.resume_pc = None;
                if o.trace > 0 && st == RunState::Running {
                    if self.trace_buf.len() >= o.trace.min(MAX_TRACE) {
                        self.trace_buf.pop_front();
                    }
                    self.trace_buf.push_back((pc, c.cpu.clock));
                }
                let n = u64::from(c.step());
                if st == RunState::Stopped || o.idle_pc.iter().any(|&(a, b)| (a..b).contains(&pc)) {
                    f.idle += n;
                }
                if !c.sys.ports.acrtc.acrtc.idle() {
                    f.acrtc += n;
                }
                if (0..CHANNELS).any(|ch| c.sys.fabric.phase(ch) != Phase::Idle) {
                    f.fabric += n;
                }
                if c.cpu.state == RunState::Halted && !o.run_when_halted {
                    c.sys.sync(c.cpu.clock);
                    f.total = c.cpu.clock - start;
                    return (Some("halted"), f);
                }
                if o.stop_on_bus_error && c.sys.log.bus_errors != errors {
                    f.total = c.cpu.clock - start;
                    return (Some("bus_error"), f);
                }
                if !o.watch.is_empty() && self.check_watch(o, pc) {
                    f.total = self.console.cpu.clock - start;
                    return (Some("watch"), f);
                }
            }
            self.console.sys.sync(chunk);
            if o.voices {
                self.watch_voices();
            }
        }
        f.total = self.console.cpu.clock - start;
        (None, f)
    }

    /// 命令を `count` 個実行する（フレームの区切りなし）。基板を CPU に追いつかせて返す。
    pub fn step(&mut self, count: u64) -> Regs {
        for _ in 0..count {
            self.console.step();
            if self.console.cpu.state == RunState::Halted {
                break;
            }
        }
        self.resume_pc = None;
        let t = self.console.cpu.clock;
        self.console.sys.sync(t);
        self.collect_audio(None);
        self.regs()
    }

    pub fn regs(&self) -> Regs {
        let cpu = &self.console.cpu;
        let mut a = [0u32; 8];
        a[..7].copy_from_slice(&cpu.a);
        a[7] = if cpu.sr & 0x2000 != 0 {
            cpu.ssp
        } else {
            cpu.usp
        };
        Regs {
            d: cpu.d,
            a,
            usp: cpu.usp,
            ssp: cpu.ssp,
            sr: cpu.sr,
            pc: cpu.pc,
            pc_label: self.label(cpu.pc),
            state: state_name(cpu.state),
            clock: cpu.clock,
            frame: self.frame(),
        }
    }

    /// 画面（テレビの模型の絵）を RGB で切り出す。
    pub fn screen(&mut self, area: Area) -> (usize, usize, Vec<u8>) {
        self.tv.picture(&self.console.sys, &mut self.pic);
        let (x0, y0, w, h) = area.rect();
        let mut rgb = Vec::with_capacity(w * h * 3);
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let p = if x < RASTER_W && y < RASTER_H {
                    self.pic[y * RASTER_W + x]
                } else {
                    0
                };
                rgb.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
            }
        }
        (w, h, rgb)
    }

    fn read_byte(&self, space: Space, addr: u32) -> Result<u8, String> {
        let s = &self.console.sys;
        let a = addr as usize;
        let word = |v: u16| if a & 1 == 0 { (v >> 8) as u8 } else { v as u8 };
        let out = |n: usize| format!("番地 ${addr:X} は空間の外（{n} byte）");
        Ok(match space {
            Space::Cpu => match window(addr) {
                Some(Window::Boot(o)) => s.boot_rom[o],
                Some(Window::Ram(o)) => s.main_ram[o],
                Some(Window::Staging(o)) => s.ports.staging[o],
                Some(Window::Cart(o)) => match &s.cart {
                    Some(c) if o < c.len() => c[o],
                    Some(c) => return Err(out(c.len())),
                    None => return Err("カートリッジがない".into()),
                },
                None => {
                    return Err(format!(
                        "番地 ${addr:06X} はメモリの窓ではない（I/O は bus_read で読む）"
                    ));
                }
            },
            Space::Vram => {
                let m = &s.ports.acrtc.vram.0;
                word(*m.get(a / 2).ok_or_else(|| out(m.len() * 2))?)
            }
            Space::Wave => {
                let m = &s.ports.doc_board.wave;
                *m.get(a).ok_or_else(|| out(m.len()))?
            }
            Space::DspProg => {
                let m = &s.ports.dsp.prog;
                word(*m.get(a / 2).ok_or_else(|| out(m.len() * 2))?)
            }
            Space::Temporal => {
                let m = &s.ports.temporal.mem;
                *m.get(a).ok_or_else(|| out(m.len()))?
            }
            Space::CharRom => {
                let m = &s.ports.char_rom;
                *m.get(a).ok_or_else(|| out(m.len()))?
            }
            Space::Palette => {
                let m = &s.ports.acrtc.video.front;
                word(*m.get(a / 2).ok_or_else(|| out(m.len() * 2))?)
            }
        })
    }

    fn write_byte(&mut self, space: Space, addr: u32, v: u8) -> Result<(), String> {
        let a = addr as usize;
        let out = |n: usize| format!("番地 ${addr:X} は空間の外（{n} byte）");
        fn set_word(m: &mut [u16], a: usize, v: u8) -> bool {
            match m.get_mut(a / 2) {
                Some(w) => {
                    *w = if a & 1 == 0 {
                        (*w & 0x00FF) | (u16::from(v) << 8)
                    } else {
                        (*w & 0xFF00) | u16::from(v)
                    };
                    true
                }
                None => false,
            }
        }
        let s = &mut self.console.sys;
        let slot = match space {
            Space::Cpu => match window(addr) {
                Some(Window::Boot(o)) => &mut s.boot_rom[o],
                Some(Window::Ram(o)) => &mut s.main_ram[o],
                Some(Window::Staging(o)) => &mut s.ports.staging[o],
                Some(Window::Cart(o)) => match &mut s.cart {
                    Some(c) if o < c.len() => &mut c[o],
                    Some(c) => return Err(out(c.len())),
                    None => return Err("カートリッジがない".into()),
                },
                None => {
                    return Err(format!(
                        "番地 ${addr:06X} はメモリの窓ではない（I/O は bus_write で書く）"
                    ));
                }
            },
            Space::Vram => {
                let m = &mut s.ports.acrtc.vram.0;
                let n = m.len() * 2;
                return if set_word(m, a, v) {
                    Ok(())
                } else {
                    Err(out(n))
                };
            }
            Space::DspProg => {
                let m = &mut s.ports.dsp.prog;
                let n = m.len() * 2;
                return if set_word(m, a, v) {
                    Ok(())
                } else {
                    Err(out(n))
                };
            }
            Space::Wave => {
                let m = &mut s.ports.doc_board.wave;
                let n = m.len();
                m.get_mut(a).ok_or_else(|| out(n))?
            }
            Space::Temporal => {
                let m = &mut s.ports.temporal.mem;
                let n = m.len();
                m.get_mut(a).ok_or_else(|| out(n))?
            }
            Space::CharRom => {
                let m = &mut s.ports.char_rom;
                let n = m.len();
                m.get_mut(a).ok_or_else(|| out(n))?
            }
            Space::Palette => {
                return Err("パレットは peek だけ（書くときは bus_write で $270000〜）".into());
            }
        };
        *slot = v;
        Ok(())
    }

    /// メモリを読む。バスを通さないので、時刻も状態も変えない。
    pub fn peek(&self, space: Space, addr: u32, len: usize) -> Result<Vec<u8>, String> {
        (0..len as u32)
            .map(|i| self.read_byte(space, addr.wrapping_add(i)))
            .collect()
    }

    /// メモリを書く。バスを通さない（ROM も書き換えられる。試験の道具）。
    pub fn poke(&mut self, space: Space, addr: u32, data: &[u8]) -> Result<(), String> {
        for (i, &b) in data.iter().enumerate() {
            self.write_byte(space, addr.wrapping_add(i as u32), b)?;
        }
        Ok(())
    }

    /// CPU と同じバスサイクルで読む（今の CPU の時刻、スーパーバイザのデータ空間）。I/O の読出しの
    /// 副作用（FIFO を進める、W1C など）も起き、バスエラーは記録に数える。
    pub fn bus_read(&mut self, addr: u32, size: Size) -> Result<u16, String> {
        let at = self.console.cpu.clock;
        match self.console.sys.read(at, addr, 5, size) {
            BusReply::Ok { data, .. } => Ok(data),
            BusReply::BusError { .. } => Err(format!("バスエラー（${addr:06X} の読出し）")),
        }
    }

    pub fn bus_write(&mut self, addr: u32, size: Size, value: u16) -> Result<(), String> {
        let at = self.console.cpu.clock;
        match self.console.sys.write(at, addr, 5, size, value) {
            BusReply::Ok { .. } => Ok(()),
            BusReply::BusError { .. } => Err(format!("バスエラー（${addr:06X} の書込み）")),
        }
    }

    /// ベンド点の命令（`Bends::command`）を実行して回路に入れる。
    pub fn bend(&mut self, cmd: &str) -> Result<String, String> {
        self.console.sys.bend(cmd)?;
        Ok(self.console.sys.bends.summary())
    }

    pub fn mode(&self) -> &'static str {
        match self.console.sys.bends.mode {
            Mode::Normal => "normal",
            Mode::Bend => "bend",
        }
    }

    /// Boot ROM の版の語（DR-094）。$FFFF は版の語がない ROM。
    pub fn bios_version(&self) -> u16 {
        let r = &self.console.sys.boot_rom;
        u16::from_be_bytes([r[BIOS_INFO_VERSION], r[BIOS_INFO_VERSION + 1]])
    }

    pub fn tv_status(&self) -> String {
        self.tv.status(&self.console.sys).summary()
    }

    /// 状態を JSON にする（DR-040 の snapshot。音の溜めとラベルは入れない）。
    pub fn save_state(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(&self.console).map_err(|e| e.to_string())
    }

    pub fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        self.console = serde_json::from_slice(data).map_err(|e| e.to_string())?;
        self.audio.clear();
        self.resume_pc = None;
        Ok(())
    }

    /// 保存した `Console` に戻す（同じ過程の中の保存の枠）。
    pub fn restore(&mut self, c: Console) {
        self.console = c;
        self.audio.clear();
        self.resume_pc = None;
    }

    /// 組込み用（C の ABI、libretro、WebAssembly。DR-104）：`n` フレームを、ホストの `Console::run_frame` と
    /// 同じに進める。止める条件も数え方もなく、68000 が止まってもフレームは進む（PC のアプリは止めない）。
    pub fn run_frames(&mut self, n: u64) {
        for _ in 0..n {
            self.console.run_frame();
            self.collect_audio(None);
        }
        self.resume_pc = None;
    }

    /// PORT_A・PORT_B に今の入力を置く（入力の割込みは基板が変化を見て出す）。
    pub fn set_input(&mut self, port_a: u16, port_b: u16) {
        self.console.sys.set_ports(port_a, port_b);
    }

    /// 仮想機の今の時刻（電源を入れてからの µs。68000 のクロックから）。
    pub fn time_us(&self) -> u64 {
        self.console.cpu.clock / CPU_CLOCKS_PER_US
    }

    /// MIDI IN の線に `bytes` を置く（DR-126）。今の時刻から `delay_us` µs 後に線に来たとし、ACIA が
    /// 31.25 kbit/s で 1 byte ずつ受け取る。前に置いた byte より早くはならない（線の順は変えない）。
    pub fn midi_in(&mut self, delay_us: u32, bytes: &[u8]) {
        let at = self.console.cpu.clock + u64::from(delay_us) * CPU_CLOCKS_PER_US;
        self.console.sys.midi_receive(at, bytes);
    }

    /// MIDI OUT：ACIA が送り終えた byte を古い順に取り出す。時刻は送り終えた µs（`time_us` と同じ数え方）。
    pub fn take_midi_out(&mut self) -> Vec<MidiByte> {
        self.console
            .sys
            .midi_out
            .drain(..)
            .map(|(t, byte)| MidiByte {
                us: t / CPU_CLOCKS_PER_US,
                byte,
            })
            .collect()
    }

    /// 取り出されないまま溜まりの上限（`MIDI_OUT_KEEP`）を超えて捨てた MIDI OUT の byte の数。
    pub fn midi_out_dropped(&self) -> u64 {
        self.console.sys.midi_out_dropped
    }

    /// 画面（テレビの模型の絵）を `0x00RRGGBB` で `out` に書く。幅と高さを返す。
    pub fn screen_xrgb(&mut self, area: Area, out: &mut Vec<u32>) -> (usize, usize) {
        self.tv.picture(&self.console.sys, &mut self.pic);
        let (x0, y0, w, h) = area.rect();
        out.clear();
        out.reserve(w * h);
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                out.push(if x < RASTER_W && y < RASTER_H {
                    self.pic[y * RASTER_W + x] & 0x00FF_FFFF
                } else {
                    0
                });
            }
        }
        (w, h)
    }

    /// 状態を小さくまとめる：`STATE_MAGIC` ＋ `save_state` の JSON を deflate したもの。
    pub fn pack_state(&self) -> Result<Vec<u8>, String> {
        pack_console(&self.console)
    }

    /// `pack_state` の大きさの見積りの上限：今の状態の大きなメモリ（Main RAM、Staging、VRAM、波形 RAM、
    /// DSP のプログラム RAM、Temporal RAM）を擬似乱数で埋めて詰めた大きさに、1/4 と 64 KiB の余裕を足す。
    /// 大きさを先に決めて領域を取る呼ぶ側（libretro の `retro_serialize_size`）のため。
    pub fn pack_state_bound(&self) -> Result<usize, String> {
        let mut c = self.console.clone();
        let mut x: u32 = 0x1985_0001;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x
        };
        let s = &mut c.sys;
        for m in [
            &mut s.main_ram,
            &mut s.ports.staging,
            &mut s.ports.doc_board.wave,
            &mut s.ports.temporal.mem,
        ] {
            m.iter_mut().for_each(|b| *b = next() as u8);
        }
        for m in [&mut s.ports.acrtc.vram.0, &mut s.ports.dsp.prog] {
            m.iter_mut().for_each(|w| *w = next() as u16);
        }
        let n = pack_console(&c)?.len();
        Ok(n + n / 4 + 0x1_0000)
    }

    /// `pack_state` の形を読む。
    pub fn unpack_state(&mut self, data: &[u8]) -> Result<(), String> {
        use std::io::Read;
        let body = data
            .strip_prefix(STATE_MAGIC.as_slice())
            .ok_or("状態の印がない（fc85 の pack_state の形ではない）")?;
        let mut json = Vec::new();
        flate2::read::DeflateDecoder::new(body)
            .read_to_end(&mut json)
            .map_err(|e| format!("状態を展開できない: {e}"))?;
        self.load_state(&json)
    }
}

/// `Machine::pack_state` の形にする。
fn pack_console(c: &Console) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let json = serde_json::to_vec(c).map_err(|e| e.to_string())?;
    let mut z =
        flate2::write::DeflateEncoder::new(STATE_MAGIC.to_vec(), flate2::Compression::fast());
    z.write_all(&json).map_err(|e| e.to_string())?;
    z.finish().map_err(|e| e.to_string())
}

#[derive(Clone, Copy, Debug, Default)]
struct FrameCount {
    total: u64,
    idle: u64,
    acrtc: u64,
    fabric: u64,
}

/// 釦の名前（`UP+A` など。`+`・`|`・空白で区切る）か数を PORT_A の値にする。
pub fn parse_pad(s: &str) -> Result<u16, String> {
    if let Some(v) = symbols::parse_number(s.trim()) {
        return u16::try_from(v).map_err(|_| format!("入力の値が大きすぎる：{s}"));
    }
    let mut v = 0;
    for name in s
        .split(['+', '|', ' ', ','])
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        let up = name.to_ascii_uppercase();
        let bit = PAD_BITS
            .iter()
            .find(|(n, _)| *n == up)
            .map(|e| e.1)
            .ok_or_else(|| {
                format!("釦 {name:?} はない（UP DOWN LEFT RIGHT A B C START、または数）")
            })?;
        v |= bit;
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_names_and_numbers() {
        assert_eq!(parse_pad("UP+A"), Ok(0x11));
        assert_eq!(parse_pad("start | left"), Ok(0x84));
        assert_eq!(parse_pad("$80"), Ok(0x80));
        assert_eq!(parse_pad(""), Ok(0));
        assert!(parse_pad("JUMP").is_err());
    }

    #[test]
    fn peek_and_poke_reach_the_memories_without_the_bus() {
        let mut m = Machine::new(Config::default()).unwrap();
        m.poke(Space::Cpu, MAIN_RAM + 0x10, &[0x12, 0x34]).unwrap();
        assert_eq!(m.console.sys.main_ram[0x10..0x12], [0x12, 0x34]);
        assert_eq!(
            m.peek(Space::Cpu, MAIN_RAM + 0x10, 2).unwrap(),
            [0x12, 0x34]
        );
        m.poke(Space::Vram, 3, &[0xAB]).unwrap();
        assert_eq!(m.console.sys.ports.acrtc.vram.0[1], 0x00AB);
        assert_eq!(m.peek(Space::Vram, 2, 2).unwrap(), [0x00, 0xAB]);
        assert!(m.peek(Space::Cpu, 0x23_0000, 2).is_err());
        assert!(m.peek(Space::Cpu, CART, 1).is_err());
        assert!(m.poke(Space::Palette, 0, &[0]).is_err());
        // バスを通さないので、時刻もバスエラーの数も変わらない
        assert_eq!(m.console.sys.log.bus_errors, 0);
    }
}
