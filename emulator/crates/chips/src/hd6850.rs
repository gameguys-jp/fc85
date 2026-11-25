//! HD6850 ACIA（日立。MC6850 と同等品）のコア。台帳 `docs/parts/MC6850.md`。
//!
//! 模型：
//! - `clock` の 1 回を送受信のクロック（本機では 16 MHz÷32＝500 kHz。台帳 §3.5）の 1 周期とする。
//!   ÷16 と ÷64 は、1 ビットを 16 または 64 周期として数える。÷1（外部でのビット同期）は ÷16 と同じに
//!   扱う [V]。
//! - 受信は byte の単位で扱う。線に来た byte（`receive`）は、スタートからストップまでのビット数の時間で
//!   RDR に入る。受信のビットの標本化、偽スタートの除去、FE・PE の判定は作っていない（線の誤りを作らない
//!   ので、FE と PE は 0）。
//! - 送信は、シフトレジスタが空いていれば TDR の byte を移し、ビット数の時間で送り終える。送った byte は
//!   `take_sent` で受け取る。CTS̅ と DCD̅ は Low につないだとする（状態の bit 2・3 は 0）[P]。
//! - オーバーラン：RDRF のまま次の byte を受けると、その byte を捨てて OVRN を立てる。RDR を読むと
//!   RDRF と OVRN が消える。台帳 §4.3 の「状態→データの順」の手順は、RDR の読出しで消すことで代える [V]。
//! - 電源投入の後はマスタリセット（CR1:0＝11）を書くまで動かない（台帳 §4.2 の注意）。その間 IRQ̅ は出さない。

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

pub const SR_RDRF: u8 = 0x01;
pub const SR_TDRE: u8 = 0x02;
pub const SR_OVRN: u8 = 0x20;
pub const SR_IRQ: u8 = 0x80;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hd6850 {
    cr: u8,
    /// マスタリセットの中（電源投入の直後を含む）。
    in_reset: bool,
    rdr: u8,
    rdrf: bool,
    ovrn: bool,
    tdr: u8,
    tdr_full: bool,
    /// 送信中の byte と、送り終えるまでのクロック数。
    tsr: u8,
    tx_left: u32,
    /// 受信中の byte と、受け終えるまでのクロック数。
    rx: Option<u8>,
    rx_left: u32,
    /// 線に来て、まだ受信を始めていない byte。
    line_in: VecDeque<u8>,
    /// 送り終えた byte。
    sent: VecDeque<u8>,
}

impl Default for Hd6850 {
    fn default() -> Self {
        Self::new()
    }
}

impl Hd6850 {
    pub fn new() -> Self {
        Self {
            cr: 0,
            in_reset: true,
            rdr: 0,
            rdrf: false,
            ovrn: false,
            tdr: 0,
            tdr_full: false,
            tsr: 0,
            tx_left: 0,
            rx: None,
            rx_left: 0,
            line_in: VecDeque::new(),
            sent: VecDeque::new(),
        }
    }

    /// 1 文字のビット数（スタート、データ、パリティ、ストップ。CR4:2）。
    fn frame_bits(&self) -> u32 {
        match (self.cr >> 2) & 7 {
            0 | 1 => 1 + 7 + 1 + 2,
            2 | 3 => 1 + 7 + 1 + 1,
            4 => 1 + 8 + 2,
            5 => 1 + 8 + 1,
            _ => 1 + 8 + 1 + 1,
        }
    }

    /// 1 文字のクロック数。
    fn frame_clocks(&self) -> u32 {
        let div = if self.cr & 3 == 2 { 64 } else { 16 };
        self.frame_bits() * div
    }

    /// データのビット数のマスク（7 ビットの語では bit 7 を 0 にする）。
    fn data_mask(&self) -> u8 {
        if (self.cr >> 2) & 7 < 4 { 0x7F } else { 0xFF }
    }

    /// 状態レジスタ。
    pub fn status(&self) -> u8 {
        if self.in_reset {
            return 0;
        }
        let mut s = 0;
        if self.rdrf {
            s |= SR_RDRF;
        }
        if !self.tdr_full {
            s |= SR_TDRE;
        }
        if self.ovrn {
            s |= SR_OVRN;
        }
        if self.irq() {
            s |= SR_IRQ;
        }
        s
    }

    /// IRQ̅ を出しているか。
    pub fn irq(&self) -> bool {
        if self.in_reset {
            return false;
        }
        let rx = self.cr & 0x80 != 0 && (self.rdrf || self.ovrn);
        let tx = (self.cr >> 5) & 3 == 1 && !self.tdr_full;
        rx || tx
    }

    /// 読出し。`rs` が false なら状態、true なら RDR。
    pub fn read(&mut self, rs: bool) -> u8 {
        if !rs {
            return self.status();
        }
        self.rdrf = false;
        self.ovrn = false;
        self.rdr
    }

    /// 書込み。`rs` が false なら制御、true なら TDR。
    pub fn write(&mut self, rs: bool, v: u8) {
        if !rs {
            self.cr = v;
            if v & 3 == 3 {
                self.in_reset = true;
                self.rdrf = false;
                self.ovrn = false;
                self.tdr_full = false;
                self.tx_left = 0;
                self.rx = None;
                self.rx_left = 0;
            } else {
                self.in_reset = false;
            }
            return;
        }
        if self.in_reset {
            return;
        }
        self.tdr = v;
        self.tdr_full = true;
        self.start_tx();
    }

    fn start_tx(&mut self) {
        if self.tx_left == 0 && self.tdr_full {
            self.tsr = self.tdr & self.data_mask();
            self.tdr_full = false;
            self.tx_left = self.frame_clocks();
        }
    }

    /// 線に byte が来た（MIDI IN）。前の byte を受け終えてから受け始める。
    pub fn receive(&mut self, byte: u8) {
        self.line_in.push_back(byte);
    }

    /// 送り終えた byte（MIDI OUT）。
    pub fn take_sent(&mut self) -> Option<u8> {
        self.sent.pop_front()
    }

    /// 送受信のクロックを `n` 周期進める。
    pub fn clock(&mut self, mut n: u32) {
        while n > 0 {
            if self.in_reset {
                // マスタリセットの間も線の byte は流れてくるが、受け取らない
                self.line_in.clear();
                return;
            }
            if self.rx.is_none()
                && let Some(b) = self.line_in.pop_front()
            {
                self.rx = Some(b);
                self.rx_left = self.frame_clocks();
            }
            let step = match (self.rx.is_some(), self.tx_left > 0) {
                (false, false) => return,
                (true, false) => self.rx_left.min(n),
                (false, true) => self.tx_left.min(n),
                (true, true) => self.rx_left.min(self.tx_left).min(n),
            };
            n -= step;
            if let Some(b) = self.rx {
                self.rx_left -= step;
                if self.rx_left == 0 {
                    self.rx = None;
                    if self.rdrf {
                        self.ovrn = true;
                    } else {
                        self.rdr = b & self.data_mask();
                        self.rdrf = true;
                    }
                }
            }
            if self.tx_left > 0 {
                self.tx_left -= step;
                if self.tx_left == 0 {
                    self.sent.push_back(self.tsr);
                    self.start_tx();
                }
            }
        }
    }
}
