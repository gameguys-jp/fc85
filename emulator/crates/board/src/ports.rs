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
//! まだないもの（`ready` は 0、アクセスは何もしない）：port 1（ACRTC の DMA）、DOC 子基板の
//! port 2、port 8（Capture RAM、[O]）、port 11〜15（拡張）。FIFO の向こう側（DSP、MSM5205、
//! 本体 DAC）のアダプタもまだなく、試験や上の層が FIFO を直接読み書きする。

use crate::fabric::{Access, Dir, PortReply, Ports};
use crate::fifo::Fifo;
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
    fn ready(&self, port: u8, dir: Dir) -> bool {
        match (port, dir) {
            (0 | 6 | 7 | 9, _) => true,
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
            (2, _) if self.doc => return PortReply::Wait,
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
            // port 1 は ACRTC の応答まで延びる。まだ ACRTC をつないでいない。
            (1, _) => return PortReply::Wait,
            _ => 0,
        };
        PortReply::Done(v)
    }

    fn continue_access(&mut self, _port: u8) -> PortReply {
        PortReply::Wait
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
    }
}
