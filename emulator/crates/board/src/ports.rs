//! Data Fabric のポートの側の部品（仕様書 §05・§06、fabric.md §08）。
//!
//! 段階 2 の最初に作ったもの：
//! - port 0：Staging RAM 64 KiB。番地は byte。16 bit のアクセスは bit 0 を無視し、8 bit のアクセスは
//!   偶数番地が上位バイト（big-endian。DR-046 (3)）。CPU の側との時分割は数えない（Fabric は S の
//!   スロットだけで使う）。
//! - port 2：DOC 子基板がないときの本体 DAC の FIFO（64×16）。
//! - port 3・4：DSP の入力・出力の FIFO（64×16）。port 5：ADPCM の入力の FIFO（64×8）。
//! - port 6・7：Temporal RAM 64 KiB の書込み・読出し。ポインタは独立で、64 KiB で回る。
//! - port 9：文字 ROM 32 KiB。bit 15 を無視する。
//! - port 10：音声入力の FIFO（64×8。拡張基板があるとき）。
//!
//! - port 1：ACRTC の DMA。ACRTC と VRAM は `acrtc`（`AcrtcBridge`）が持ち、スロットごとに進める。
//! - 音声クロック（TRIGGER 3）と、DOC 子基板がないときの本体 DAC（port 2 の FIFO を読む）。`audio`。
//! - ADPCM のアダプタ（port 5 の FIFO を MSM5205 へ。TRIGGER 5 の V̄C̄K̄）。`adpcm`。
//! - DSP のアダプタ（port 3・4 の FIFO と TMS32010、プログラム RAM）。`dsp`。
//! - DOC 子基板（`doc` が true のとき）：ES5503 と波形 RAM、port 2 のアダプタ P2A。`doc`（`DocBoard`）。
//!   68000 から DOC の窓へのバスサイクルは `doc_host_begin`（段階 3 のブリッジ）。
//!
//! まだないもの（`ready` は 0、アクセスは何もしない）：port 8（Capture RAM、[O]）、port 11〜15（拡張）。

use crate::acrtc::AcrtcBridge;
use crate::adpcm::AdpcmPort;
use crate::audio::{AudioClock, MainDac};
use crate::doc::{self, DocBoard, HostDone};
use crate::dsp::DspPort;
use crate::fabric::{Access, Dir, HostReply, PortReply, Ports};
use crate::fifo::Fifo;
use fc85_core::time::{SLOT, Time};
use serde::{Deserialize, Serialize};

/// Temporal RAM（HM6264×8、8 bit。仕様書 §05、DR-031）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalRam {
    pub mem: Vec<u8>,
    /// 書込みのポインタ（port 6）。
    pub wptr: u16,
    /// 読出しのポインタ（port 7）。
    pub rptr: u16,
}

impl TemporalRam {
    pub fn new() -> Self {
        Self {
            mem: vec![0; 0x10000],
            wptr: 0,
            rptr: 0,
        }
    }

    pub fn write(&mut self, v: u8) {
        self.mem[self.wptr as usize] = v;
        self.wptr = self.wptr.wrapping_add(1);
    }

    pub fn read(&mut self) -> u8 {
        let v = self.mem[self.rptr as usize];
        self.rptr = self.rptr.wrapping_add(1);
        v
    }
}

impl Default for TemporalRam {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FabricPorts {
    /// Staging RAM（port 0）。添字が byte 番地。
    pub staging: Vec<u8>,
    /// 文字 ROM（port 9）。
    pub char_rom: Vec<u8>,
    pub temporal: TemporalRam,
    /// port 2（DOC 子基板がないとき）：本体 DAC の FIFO。上位 byte が L、下位 byte が R。
    pub dac: Fifo,
    /// port 3：DSP の入力の FIFO。
    pub dsp_in: Fifo,
    /// port 4：DSP の出力の FIFO。
    pub dsp_out: Fifo,
    /// port 5：ADPCM の入力の FIFO（8 bit）。
    pub adpcm: Fifo,
    /// port 10：音声入力の FIFO（8 bit、拡張基板）。
    pub audio_in: Fifo,
    /// DOC 子基板があるか。
    pub doc: bool,
    /// モジュールの /PRES（bit n が port n。8、10〜15 を見る）。
    pub modules: u16,
    /// 最後に書かれた AUDIO_N。
    pub audio_n: u16,
    /// port 1：ACRTC と VRAM、ブリッジと P1A。
    pub acrtc: AcrtcBridge,
    /// 音声クロック（TRIGGER 3）。
    pub audio: AudioClock,
    /// 本体 DAC（DOC 子基板がないとき port 2 の FIFO を読む）。
    pub main_dac: MainDac,
    /// ADPCM のアダプタ（port 5 の FIFO を読む）。
    pub adpcm_port: AdpcmPort,
    /// DSP のアダプタ（port 3・4 の FIFO の向こう）。
    pub dsp: DspPort,
    /// DOC 子基板（`doc` が true のとき port 2）。
    pub doc_board: DocBoard,
    /// 次のスロットの頭の時刻。
    now: Time,
}

impl FabricPorts {
    pub fn new() -> Self {
        Self {
            staging: vec![0; 0x10000],
            char_rom: vec![0; 0x8000],
            temporal: TemporalRam::new(),
            dac: Fifo::new(16),
            dsp_in: Fifo::new(16),
            dsp_out: Fifo::new(16),
            adpcm: Fifo::new(8),
            audio_in: Fifo::new(8),
            doc: false,
            modules: 0,
            audio_n: 0,
            acrtc: AcrtcBridge::new(),
            audio: AudioClock::new(),
            main_dac: MainDac::new(),
            adpcm_port: AdpcmPort::new(),
            dsp: DspPort::new(),
            doc_board: DocBoard::new(),
            now: 0,
        }
    }

    fn staging_read(&self, addr: u16, word: bool) -> u16 {
        if word {
            let i = (addr & !1) as usize;
            u16::from_be_bytes([self.staging[i], self.staging[i + 1]])
        } else {
            self.staging[addr as usize] as u16
        }
    }

    fn staging_write(&mut self, addr: u16, word: bool, v: u16) {
        if word {
            let i = (addr & !1) as usize;
            let [hi, lo] = v.to_be_bytes();
            self.staging[i] = hi;
            self.staging[i + 1] = lo;
        } else {
            self.staging[addr as usize] = v as u8;
        }
    }

    /// 次に進めるスロットの頭の時刻。
    pub fn now(&self) -> Time {
        self.now
    }

    /// 68000 から DOC の窓（0x210000〜0x2101FF）へのバスサイクルを始める。`s0` は S0 の立上り。
    /// 子基板がないか、上位バイトレーンの 1 byte でなければ DTACK を返さない（バスの時間切れで
    /// バスエラー。DR-019、仕様書 §05）。受けたら `Wait` を返し、`doc_host_poll` で終わりを受け取る。
    pub fn doc_host_begin(
        &mut self,
        s0: Time,
        addr: u32,
        uds: bool,
        lds: bool,
        write: Option<u16>,
    ) -> HostReply {
        match doc::host_register(addr, uds, lds) {
            Some(reg) if self.doc => {
                self.doc_board
                    .host_begin(s0, reg, write.map(|w| (w >> 8) as u8));
                HostReply::Wait
            }
            _ => HostReply::BusError,
        }
    }

    /// DTACK を返した DOC のアクセス。
    pub fn doc_host_poll(&mut self) -> Option<HostDone> {
        self.doc_board.host_poll()
    }

    fn present(&self, port: u8) -> bool {
        self.modules & (1 << port) != 0
    }
}

impl Default for FabricPorts {
    fn default() -> Self {
        Self::new()
    }
}

impl Ports for FabricPorts {
    fn run_slot(&mut self, starting: Option<&Access>) {
        let t0 = self.now;
        let end = t0 + SLOT;
        self.acrtc.run_slot(t0, starting.filter(|a| a.port == 1));
        let beats = self.audio.run(end);
        if !self.doc {
            for _ in 0..beats {
                self.main_dac.beat(&mut self.dac);
            }
        }
        self.adpcm_port.run(end, &mut self.adpcm);
        self.dsp
            .run(end, &mut self.dsp_in, &mut self.dsp_out, &self.audio);
        if self.doc {
            self.doc_board
                .run_slot(t0, starting.filter(|a| a.port == 2));
        }
        self.now = end;
    }

    fn trigger_levels(&self) -> u8 {
        (u8::from(self.audio.level()) << 3) | (u8::from(self.adpcm_port.vck()) << 5)
    }

    fn ready(&self, port: u8, dir: Dir) -> bool {
        match (port, dir) {
            (0 | 6 | 7 | 9, _) => true,
            (1, _) => self.acrtc.rdy1(),
            // DOC 子基板があれば RDY2 は常に 1 で、アクセスが DOC の窓まで延びる（fabric.md §06）。
            (2, _) if self.doc => true,
            (2, Dir::Write) => self.dac.ir(),
            (3, Dir::Write) => self.dsp_in.ir(),
            (4, Dir::Read) => self.dsp_out.or(),
            (5, Dir::Write) => self.adpcm.ir(),
            (8, _) => self.present(8),
            (10, Dir::Read) => self.present(10) && self.audio_in.or(),
            _ => false,
        }
    }

    fn access(&mut self, a: &Access) -> PortReply {
        let v = match (a.port, a.dir) {
            (0, Dir::Read) => self.staging_read(a.addr, a.word),
            (0, Dir::Write) => {
                self.staging_write(a.addr, a.word, a.data);
                0
            }
            // DOC の port 2 は窓まで延びる（ラッチは `run_slot` でしている）。
            (2, _) if self.doc => return self.doc_board.port_reply(),
            (2, Dir::Write) => {
                self.dac.shift_in(a.data);
                0
            }
            (3, Dir::Write) => {
                self.dsp_in.shift_in(a.data);
                0
            }
            (4, Dir::Read) => self.dsp_out.shift_out(),
            (5, Dir::Write) => {
                self.adpcm.shift_in(a.data & 0xFF);
                0
            }
            (6, Dir::Write) => {
                self.temporal.write(a.data as u8);
                0
            }
            (7, Dir::Read) => self.temporal.read() as u16,
            (9, Dir::Read) => self.char_rom[(a.addr & 0x7FFF) as usize] as u16,
            (10, Dir::Read) => self.audio_in.shift_out(),
            // port 1 は ACRTC の応答まで延びる（DACK は `run_slot` で出している）。
            (1, _) => return self.acrtc.port_reply(),
            _ => 0,
        };
        PortReply::Done(v)
    }

    fn continue_access(&mut self, port: u8) -> PortReply {
        match port {
            1 => self.acrtc.port_reply(),
            2 if self.doc => self.doc_board.port_reply(),
            _ => PortReply::Wait,
        }
    }

    fn abort_access(&mut self, port: u8) {
        match port {
            1 => self.acrtc.abort(),
            2 if self.doc => self.doc_board.abort(),
            _ => {}
        }
    }

    fn doc_present(&self) -> bool {
        self.doc
    }

    fn module_present(&self, port: u8) -> bool {
        self.present(port)
    }

    fn port_reset(&mut self, mask: u16) {
        let on = |p: u8| mask & (1 << p) != 0;
        if on(2) {
            self.dac.reset();
        }
        if on(3) {
            self.dsp_in.reset();
        }
        if on(4) {
            self.dsp_out.reset();
        }
        if on(5) {
            self.adpcm.reset();
            self.adpcm_port.restart_byte();
        }
        if on(6) {
            self.temporal.wptr = 0;
        }
        if on(7) {
            self.temporal.rptr = 0;
        }
        if on(10) {
            self.audio_in.reset();
        }
    }

    fn write_audio_n(&mut self, n: u16) {
        self.audio_n = n;
        self.audio.set_n(n);
    }
}
