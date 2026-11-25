//! 日立 HD63484 ACRTC（Advanced CRT Controller）。
//!
//! 事実と出典は `docs/parts/HD63484.md`（以下「台帳」）。動作は日立のユーザーズマニュアル（1985年5月、台帳の U85）に
//! 従った。コードは自前で書き、MAME（台帳の M1）は解釈を比べるためだけに読んだ。
//!
//! モデルの範囲（段階 1）：
//! - `tick` の 1 回を 2CLK の 1 周期とする（本機では 5.369318 MHz、台帳 §3.3）。コマンドの所要時間は
//!   U85 p.173 の式で数える。表示とのメモリの取り合いは数えない（台帳 §7 の 4）。
//! - ホストとのやり取りは 16 bit バスだけ（本機の接続。台帳 §2）。`read`・`write` の 1 回が CS の 1 サイクル。
//! - 描画先のフレームバッファ（VRAM）は `Vram` トレイトの向こう側に置く。番地は 20 bit の語番地。
//! - 表示（同期信号、表示アドレス、ラスタカウンタ）はまだ作っていない。タイミング・表示のレジスタは値を保つだけ。
//! - DMA のハンドシェイク（DREQ、DACK、DONE）は段階 2 で足す。データ転送はプログラム転送（DDM＝0）で動く。
//! - 資料で決まらない動作は、`docs/impl/emulator.md` §03 にどう扱ったかを書く。

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// 読出し・書込みの FIFO の深さ（語）。U85 p.62。
pub const FIFO_WORDS: usize = 8;

// SR のビット（U85 p.61–62）。
pub const SR_CER: u8 = 0x80;
pub const SR_ARD: u8 = 0x40;
pub const SR_CED: u8 = 0x20;
pub const SR_LPD: u8 = 0x10;
pub const SR_RFF: u8 = 0x08;
pub const SR_RFR: u8 = 0x04;
pub const SR_WFR: u8 = 0x02;
pub const SR_WFE: u8 = 0x01;

const CCR_ABT: u16 = 0x8000;
const CCR_PSE: u16 = 0x4000;
const CCR_DDM: u16 = 0x2000;
const OMR_STR: u16 = 0x4000;

/// 20 bit の語番地。
const ADDR_MASK: i64 = 0xF_FFFF;

/// フレームバッファ。board 層（またはテスト）が実装する。
pub trait Vram {
    fn read(&mut self, addr: u32) -> u16;
    fn write(&mut self, addr: u32, data: u16);
}

/// コマンド（U85 p.173 の表）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Cmd {
    Org,
    Wpr(u8),
    Rpr(u8),
    Wptn(u8),
    Rptn(u8),
    Drd,
    Dwt,
    Dmod(u8),
    Rd,
    Wt,
    Mod(u8),
    Clr,
    Sclr(u8),
    Cpy {
        s: bool,
        dsd: u8,
        mm: Option<u8>,
    },
    Amove,
    Rmove,
    Line {
        rel: bool,
    },
    Rect {
        rel: bool,
    },
    Poly {
        rel: bool,
        closed: bool,
    },
    /// 段階 1 の後半で作る。パラメータの数だけ読み捨てる。
    Unimplemented(u8),
    Frct {
        rel: bool,
    },
    Dot,
    Gcpy {
        rel: bool,
        s: bool,
        dsd: u8,
    },
}

impl Cmd {
    fn decode(op: u16) -> Option<Cmd> {
        let mm = (op & 3) as u8;
        let s = op & 0x0800 != 0;
        let dsd = ((op >> 8) & 7) as u8;
        let cmd = match op >> 12 {
            0x6 => Cmd::Cpy { s, dsd, mm: None },
            0x7 => Cmd::Cpy {
                s,
                dsd,
                mm: Some(mm),
            },
            0xD => Cmd::Unimplemented(1), // PTN
            0xE => Cmd::Gcpy { rel: false, s, dsd },
            0xF => Cmd::Gcpy { rel: true, s, dsd },
            _ => match op >> 9 {
                0x54 => Cmd::Unimplemented(1),        // CRCL
                0x56 => Cmd::Unimplemented(3),        // ELPS
                0x58 | 0x5A => Cmd::Unimplemented(4), // AARC、RARC
                0x5C | 0x5E => Cmd::Unimplemented(6), // AEARC、REARC
                0x64 => Cmd::Unimplemented(0),        // PAINT
                _ => match op >> 10 {
                    0x01 => Cmd::Org,
                    0x02 => Cmd::Wpr((op & 0x1F) as u8),
                    0x03 => Cmd::Rpr((op & 0x1F) as u8),
                    0x06 => Cmd::Wptn((op & 0xF) as u8),
                    0x07 => Cmd::Rptn((op & 0xF) as u8),
                    0x09 => Cmd::Drd,
                    0x0A => Cmd::Dwt,
                    0x0B => Cmd::Dmod(mm),
                    0x11 => Cmd::Rd,
                    0x12 => Cmd::Wt,
                    0x13 => Cmd::Mod(mm),
                    0x16 => Cmd::Clr,
                    0x17 => Cmd::Sclr(mm),
                    0x20 => Cmd::Amove,
                    0x21 => Cmd::Rmove,
                    0x22 => Cmd::Line { rel: false },
                    0x23 => Cmd::Line { rel: true },
                    0x24 => Cmd::Rect { rel: false },
                    0x25 => Cmd::Rect { rel: true },
                    0x26 => Cmd::Poly {
                        rel: false,
                        closed: false,
                    },
                    0x27 => Cmd::Poly {
                        rel: true,
                        closed: false,
                    },
                    0x28 => Cmd::Poly {
                        rel: false,
                        closed: true,
                    },
                    0x29 => Cmd::Poly {
                        rel: true,
                        closed: true,
                    },
                    0x30 => Cmd::Frct { rel: false },
                    0x31 => Cmd::Frct { rel: true },
                    0x33 => Cmd::Dot,
                    _ => return None,
                },
            },
        };
        Some(cmd)
    }

    /// コマンド語の後に、最初にまとめて読むパラメータの数。
    fn params(self) -> usize {
        match self {
            Cmd::Rpr(_) | Cmd::Rd | Cmd::Dot => 0,
            Cmd::Wpr(_) | Cmd::Wptn(_) | Cmd::Rptn(_) | Cmd::Wt | Cmd::Mod(_) => 1,
            Cmd::Poly { .. } => 1,
            Cmd::Org | Cmd::Drd | Cmd::Dwt | Cmd::Dmod(_) => 2,
            Cmd::Amove | Cmd::Rmove | Cmd::Line { .. } | Cmd::Rect { .. } | Cmd::Frct { .. } => 2,
            Cmd::Clr | Cmd::Sclr(_) => 3,
            Cmd::Cpy { .. } | Cmd::Gcpy { .. } => 4,
            Cmd::Unimplemented(n) => usize::from(n),
        }
    }
}

/// 語単位の矩形（CLR、SCLR、DWT、DMOD、DRD）。U85 p.187、p.202。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct WordRect {
    origin: u32,
    width: u32,
    rows: u32,
    dx: i64,
    /// 次の行への番地の差。+Y（画面の上）は −MW。
    row_step: i64,
}

impl WordRect {
    fn new(origin: u32, ax: u16, ay: u16, mw: u32) -> Self {
        let (ax, ay) = (i64::from(ax as i16), i64::from(ay as i16));
        WordRect {
            origin,
            width: (ax.abs() + 1) as u32,
            rows: (ay.abs() + 1) as u32,
            dx: if ax < 0 { -1 } else { 1 },
            row_step: if ay < 0 {
                i64::from(mw)
            } else {
                -i64::from(mw)
            },
        }
    }
    fn words(&self) -> u32 {
        self.width * self.rows
    }
    fn addr(&self, k: u32) -> u32 {
        let (i, j) = (k % self.width, k / self.width);
        wrap(i64::from(self.origin) + self.dx * i64::from(i) + self.row_step * i64::from(j))
    }
    /// 終了後の RWP：開始の列で、最後の行の 1 つ先（U85 p.202 の図、p.213 の図 C14-2）。
    fn end(&self) -> u32 {
        wrap(i64::from(self.origin) + self.row_step * i64::from(self.rows))
    }
    /// k 語目の所要サイクル。行の終わりに 8、8 語ごとに `refill`（U85 p.173 の式を語に割り振った）。
    fn word_cycles(&self, k: u32, refill: u32) -> u32 {
        let mut c = 4;
        if k.is_multiple_of(8) {
            c += refill;
        }
        if (k + 1).is_multiple_of(self.width) {
            c += 8;
        }
        c
    }
}

fn wrap(addr: i64) -> u32 {
    (addr & ADDR_MASK) as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Push {
    Rd,
    Rpr(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Exec {
    Idle,
    /// パラメータを集めている。`credit` はすでに数えたサイクル（コマンド語を取り出したサイクル）。
    Params {
        op: u16,
        cmd: Cmd,
        got: Vec<u16>,
        credit: u32,
    },
    /// 読出し FIFO の空きを待つ（RD、RPR）。
    Push {
        what: Push,
        credit: u32,
    },
    Wptn {
        addr: u8,
        left: u16,
    },
    Rptn {
        addr: u8,
        left: u16,
    },
    /// DWT・DMOD（書く）と DRD（読む）。
    Transfer {
        rect: WordRect,
        mm: Option<u8>,
        read: bool,
        next: u32,
    },
    Poly {
        op: u16,
        rel: bool,
        closed: bool,
        left: u16,
        start: (i16, i16),
        x: Option<u16>,
        stopped: bool,
    },
    /// DRD の最後の語の後。ABT まで止まる（U85 p.128）。
    Hang,
    /// 最後のサイクルを待っている。終わると CED。
    Done,
    /// 未定義のコマンド。ABT まで止まる [V]。
    Error,
}

/// 領域の判定の結果（U85 p.154）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Area {
    Draw,
    Skip,
    Stop,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hd63484 {
    ar: u8,
    /// SR のうち FIFO から決まらないビット（CER、ARD、CED、LPD）。
    flags: u8,
    ccr: u16,
    omr: u16,
    dcr: u16,
    /// r80〜rFE（64 語）。タイミング制御 RAM と表示制御 RAM。
    regs: Vec<u16>,
    wfifo: VecDeque<u16>,
    rfifo: VecDeque<u16>,

    // 描画パラメータレジスタ（U85 p.114）
    cl0: u16,
    cl1: u16,
    ccmp: u16,
    edg: u16,
    mask: u16,
    ppx: u8,
    ppy: u8,
    pzcx: u8,
    pzcy: u8,
    psx: u8,
    psy: u8,
    pex: u8,
    pey: u8,
    pzx: u8,
    pzy: u8,
    xmin: i16,
    ymin: i16,
    xmax: i16,
    ymax: i16,
    rwp_dn: u8,
    rwp: u32,
    org_dn: u8,
    org: u32,
    org_dpd: u8,
    dp: u32,
    dpd: u8,
    cpx: i16,
    cpy: i16,
    pattern: [u16; 16],

    exec: Exec,
    busy: u32,
    cycles: u64,
    overruns: u64,
    underruns: u64,
    unimplemented: u64,
}

impl Default for Hd63484 {
    fn default() -> Self {
        Self::new()
    }
}

impl Hd63484 {
    /// 電源投入直後（RES の後）。レジスタは 0 とする（実チップは不定）。
    pub fn new() -> Self {
        Self {
            ar: 0,
            flags: SR_CED,
            ccr: CCR_ABT,
            omr: 0,
            dcr: 0,
            regs: vec![0; 64],
            wfifo: VecDeque::new(),
            rfifo: VecDeque::new(),
            cl0: 0,
            cl1: 0,
            ccmp: 0,
            edg: 0,
            mask: 0,
            ppx: 0,
            ppy: 0,
            pzcx: 0,
            pzcy: 0,
            psx: 0,
            psy: 0,
            pex: 0,
            pey: 0,
            pzx: 0,
            pzy: 0,
            xmin: 0,
            ymin: 0,
            xmax: 0,
            ymax: 0,
            rwp_dn: 0,
            rwp: 0,
            org_dn: 0,
            org: 0,
            org_dpd: 0,
            dp: 0,
            dpd: 0,
            cpx: 0,
            cpy: 0,
            pattern: [0; 16],
            exec: Exec::Idle,
            busy: 0,
            cycles: 0,
            overruns: 0,
            underruns: 0,
            unimplemented: 0,
        }
    }

    /// RES。CCR は ABT＝1 でほかは 0、SR は CED・WFR・WFE だけ 1、OMR は M/S と STR だけ 0（U85 p.61、p.65、p.69）。
    /// ほかのレジスタは変えない [V]。
    pub fn reset(&mut self) {
        self.ccr = CCR_ABT;
        self.omr &= 0x3FFF;
        self.flags = SR_CED;
        self.wfifo.clear();
        self.rfifo.clear();
        self.exec = Exec::Idle;
        self.busy = 0;
    }

    // ---- ホストのアクセス（16 bit バス） ----

    /// CS＝L の読出し。`rs` が false なら SR、true なら AR が指すレジスタ（U85 p.59–60）。
    pub fn read(&mut self, rs: bool) -> u16 {
        if !rs {
            // 上位バイトは MAME と同じく 1 [V]
            return 0xFF00 | u16::from(self.status());
        }
        let ar = self.ar;
        let v = match ar {
            0x00 => match self.rfifo.pop_front() {
                Some(w) => w,
                None => {
                    self.underruns += 1;
                    0
                }
            },
            0x02 => self.ccr,
            0x04 => self.omr,
            0x06 => self.dcr,
            0x80 => 0, // RCR：表示はまだ作っていない
            0x81..=0xFF => {
                if (0xEC..=0xEF).contains(&ar) {
                    self.flags &= !SR_LPD;
                }
                self.regs[usize::from((ar - 0x80) / 2)]
            }
            _ => 0,
        };
        self.advance_ar();
        v
    }

    /// CS＝L の書込み。`rs` が false なら AR、true なら AR が指すレジスタ。
    pub fn write(&mut self, rs: bool, data: u16) {
        if !rs {
            // 16 bit モードでは AR の最下位ビットは 0 とみなす（U85 p.60）
            self.ar = (data & 0xFE) as u8;
            return;
        }
        let ar = self.ar;
        match ar {
            0x00 => {
                if self.wfifo.len() < FIFO_WORDS {
                    self.wfifo.push_back(data);
                } else {
                    self.overruns += 1;
                }
                // 書込み FIFO にコマンドを書くと CED は 0（U85 p.62）
                self.flags &= !SR_CED;
            }
            0x02 => {
                let rising = data & CCR_ABT != 0 && self.ccr & CCR_ABT == 0;
                self.ccr = data;
                if rising {
                    self.abort();
                }
            }
            0x04 => self.omr = data,
            0x06 => self.dcr = data,
            0x80 => {} // RCR は読出し専用
            0x81..=0xFF => self.regs[usize::from((ar - 0x80) / 2)] = data,
            _ => {}
        }
        self.advance_ar();
    }

    fn advance_ar(&mut self) {
        // r80〜rFF へのアクセスでは AR を 2 進める（U85 p.60）
        if self.ar >= 0x80 {
            self.ar = self.ar.wrapping_add(2);
        }
    }

    /// ABT が 0 から 1 になった：実行を打ち切り、FIFO を空にし、SR を $23 にする（U85 p.66）。
    fn abort(&mut self) {
        self.exec = Exec::Idle;
        self.busy = 0;
        self.wfifo.clear();
        self.rfifo.clear();
        self.flags = SR_CED;
    }

    // ---- 状態の読出し（副作用なし） ----

    /// SR の下位 8 bit。
    pub fn status(&self) -> u8 {
        let mut sr = self.flags;
        if self.rfifo.len() >= FIFO_WORDS {
            sr |= SR_RFF;
        }
        if !self.rfifo.is_empty() {
            sr |= SR_RFR;
        }
        if self.wfifo.len() < FIFO_WORDS {
            sr |= SR_WFR;
        }
        if self.wfifo.is_empty() {
            sr |= SR_WFE;
        }
        sr
    }

    /// IRQ ピンが Low か。SR と CCR の下位 8 bit の両方が 1 のビットがあれば出す（U85 p.68）。
    pub fn irq(&self) -> bool {
        self.status() & (self.ccr as u8) != 0
    }

    /// 現在点（CP）。
    pub fn cp(&self) -> (i16, i16) {
        (self.cpx, self.cpy)
    }

    /// RWP（画面番号、20 bit の語番地）。
    pub fn rwp(&self) -> (u8, u32) {
        (self.rwp_dn, self.rwp)
    }

    /// パターン RAM の 1 語。
    pub fn pattern(&self, addr: usize) -> u16 {
        self.pattern[addr & 15]
    }

    /// 描画パラメータレジスタを、RPR と同じ形で読む（ARD は変えない）。
    pub fn param(&self, rn: u8) -> u16 {
        match rn {
            0x00 => self.cl0,
            0x01 => self.cl1,
            0x02 => self.ccmp,
            0x03 => self.edg,
            0x04 => self.mask,
            0x05 => pack4(self.ppy, self.pzcy, self.ppx, self.pzcx),
            0x06 => pack4(self.psy, 0, self.psx, 0),
            0x07 => pack4(self.pey, self.pzy, self.pex, self.pzx),
            0x08 => self.xmin as u16,
            0x09 => self.ymin as u16,
            0x0A => self.xmax as u16,
            0x0B => self.ymax as u16,
            0x0C => (u16::from(self.rwp_dn) << 14) | (self.rwp >> 12) as u16,
            0x0D => ((self.rwp & 0xFFF) << 4) as u16,
            0x10 => (u16::from(self.org_dn) << 14) | (self.dp >> 12) as u16,
            0x11 => (((self.dp & 0xFFF) << 4) as u16) | u16::from(self.dpd),
            0x12 => self.cpx as u16,
            0x13 => self.cpy as u16,
            _ => 0,
        }
    }

    /// コマンドを実行していないか（CED が 1 で、書込み FIFO が空）。
    pub fn idle(&self) -> bool {
        self.exec == Exec::Idle && self.busy == 0 && self.wfifo.is_empty()
    }

    /// これまでの 2CLK の数。
    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    /// 満杯の書込み FIFO に書いた回数、空の読出し FIFO を読んだ回数、未実装のコマンドの数。
    pub fn error_counts(&self) -> (u64, u64, u64) {
        (self.overruns, self.underruns, self.unimplemented)
    }

    // ---- 時間 ----

    /// 2CLK を 1 周期進める。
    pub fn tick(&mut self, vram: &mut impl Vram) {
        self.cycles += 1;
        // STR＝0 なら描画も止まる。PSE＝1 は一時停止、ABT＝1 は FIFO に触れない（U85 p.66、p.70）
        if self.omr & OMR_STR == 0 || self.ccr & (CCR_ABT | CCR_PSE) != 0 {
            return;
        }
        if self.busy > 0 {
            self.busy -= 1;
            if self.busy == 0 && self.exec == Exec::Done {
                self.complete();
            }
            return;
        }
        let cost = self.step(vram).max(1);
        self.busy = cost - 1;
        if self.busy == 0 && self.exec == Exec::Done {
            self.complete();
        }
    }

    fn complete(&mut self) {
        self.exec = Exec::Idle;
        self.flags |= SR_CED;
    }

    /// 1 段階進め、そのサイクル数を返す。
    fn step(&mut self, vram: &mut impl Vram) -> u32 {
        match std::mem::replace(&mut self.exec, Exec::Idle) {
            Exec::Idle => {
                let Some(op) = self.wfifo.pop_front() else {
                    return 1;
                };
                self.flags &= !SR_CED;
                let Some(cmd) = Cmd::decode(op) else {
                    // 未定義のコマンド（U85 p.61）
                    self.flags |= SR_CER;
                    self.exec = Exec::Error;
                    return 1;
                };
                self.collect(op, cmd, Vec::new(), 0, vram)
            }
            Exec::Params {
                op,
                cmd,
                got,
                credit,
            } => self.collect(op, cmd, got, credit, vram),
            Exec::Push { what, credit } => self.push(what, credit, vram),
            Exec::Wptn { addr, left } => {
                let Some(w) = self.wfifo.pop_front() else {
                    self.exec = Exec::Wptn { addr, left };
                    return 1;
                };
                self.pattern[usize::from(addr)] = w;
                self.exec = if left > 1 {
                    Exec::Wptn {
                        addr: (addr + 1) & 15,
                        left: left - 1,
                    }
                } else {
                    Exec::Done
                };
                4
            }
            Exec::Rptn { addr, left } => {
                if self.rfifo.len() >= FIFO_WORDS {
                    self.exec = Exec::Rptn { addr, left };
                    return 1;
                }
                self.rfifo.push_back(self.pattern[usize::from(addr)]);
                self.exec = if left > 1 {
                    Exec::Rptn {
                        addr: (addr + 1) & 15,
                        left: left - 1,
                    }
                } else {
                    Exec::Done
                };
                4
            }
            Exec::Transfer {
                rect,
                mm,
                read,
                next,
            } => self.transfer(rect, mm, read, next, vram),
            Exec::Poly {
                op,
                rel,
                closed,
                left,
                start,
                x,
                stopped,
            } => self.poly(op, rel, closed, left, start, x, stopped, vram),
            Exec::Hang => {
                self.exec = Exec::Hang;
                1
            }
            Exec::Error => {
                self.exec = Exec::Error;
                1
            }
            Exec::Done => {
                self.complete();
                1
            }
        }
    }

    /// パラメータを書込み FIFO から集め、そろったら実行する。足りなければ待つ（待ちは U85 の式の外に足す）。
    fn collect(
        &mut self,
        op: u16,
        cmd: Cmd,
        mut got: Vec<u16>,
        credit: u32,
        vram: &mut impl Vram,
    ) -> u32 {
        while got.len() < cmd.params() {
            match self.wfifo.pop_front() {
                Some(w) => got.push(w),
                None => break,
            }
        }
        if got.len() < cmd.params() {
            self.exec = Exec::Params {
                op,
                cmd,
                got,
                credit: credit.max(1),
            };
            return 1;
        }
        let cost = self.execute(op, cmd, &got, credit, vram);
        cost.saturating_sub(credit).max(1)
    }

    /// パラメータのそろったコマンドを実行し、U85 p.173 の式のサイクル数を返す。
    fn execute(&mut self, op: u16, cmd: Cmd, p: &[u16], credit: u32, vram: &mut impl Vram) -> u32 {
        let pcost = if op & 7 < 4 { 4 } else { 6 };
        self.exec = Exec::Done;
        match cmd {
            Cmd::Org => {
                self.org_dn = (p[0] >> 14) as u8;
                self.org = (u32::from(p[0] & 0xFF) << 12) | u32::from(p[1] >> 4);
                self.org_dpd = (p[1] & 0xF) as u8;
                self.set_cp(0, 0);
                8
            }
            Cmd::Wpr(rn) => {
                self.write_param(rn, p[0]);
                6
            }
            Cmd::Rpr(rn) => self.push(Push::Rpr(rn), credit, vram) + credit,
            Cmd::Wptn(addr) => {
                if p[0] > 0 {
                    self.exec = Exec::Wptn { addr, left: p[0] };
                }
                8
            }
            Cmd::Rptn(addr) => {
                if p[0] > 0 {
                    self.exec = Exec::Rptn { addr, left: p[0] };
                }
                10
            }
            Cmd::Dwt | Cmd::Dmod(_) | Cmd::Drd => {
                let rect = WordRect::new(self.rwp, p[0], p[1], self.mw(self.rwp_dn));
                let mm = if let Cmd::Dmod(mm) = cmd {
                    Some(mm)
                } else {
                    None
                };
                let read = cmd == Cmd::Drd;
                self.exec = Exec::Transfer {
                    rect,
                    mm,
                    read,
                    next: 0,
                };
                if read { 62 } else { 34 }
            }
            Cmd::Rd => self.push(Push::Rd, credit, vram) + credit,
            Cmd::Wt => {
                vram.write(self.rwp, p[0]);
                self.rwp = wrap(i64::from(self.rwp) + 1);
                8
            }
            Cmd::Mod(mm) => {
                let d = vram.read(self.rwp);
                vram.write(self.rwp, self.modify(d, p[0], mm));
                self.rwp = wrap(i64::from(self.rwp) + 1);
                8
            }
            Cmd::Clr | Cmd::Sclr(_) => {
                let rect = WordRect::new(self.rwp, p[1], p[2], self.mw(self.rwp_dn));
                for k in 0..rect.words() {
                    let a = rect.addr(k);
                    let v = match cmd {
                        Cmd::Sclr(mm) => {
                            let d = vram.read(a);
                            self.modify(d, p[0], mm)
                        }
                        _ => p[0],
                    };
                    vram.write(a, v);
                }
                self.rwp = rect.end();
                let (x, y) = (rect.width, rect.rows);
                if cmd == Cmd::Clr {
                    (2 * x + 8) * y + 12
                } else {
                    (4 * x + 6) * y + 12
                }
            }
            Cmd::Cpy { s, dsd, mm } => self.copy_words(p, s, dsd, mm, vram),
            Cmd::Amove => {
                self.set_cp(p[0] as i16, p[1] as i16);
                56
            }
            Cmd::Rmove => {
                let (x, y) = self.rel(p[0], p[1]);
                self.set_cp(x, y);
                56
            }
            Cmd::Line { rel } => {
                let (x, y) = if rel {
                    self.rel(p[0], p[1])
                } else {
                    (p[0] as i16, p[1] as i16)
                };
                let (n, _) = self.line(op, x, y, vram);
                pcost * n + 18
            }
            Cmd::Rect { rel } => {
                let (a, b) = (self.cpx, self.cpy);
                let (x, y) = if rel {
                    self.rel(p[0], p[1])
                } else {
                    (p[0] as i16, p[1] as i16)
                };
                // X の方向から描き始め、CP に戻る（U85 p.232）
                let mut n = 0;
                for (tx, ty) in [(x, b), (x, y), (a, y), (a, b)] {
                    let (k, stopped) = self.line(op, tx, ty, vram);
                    n += k;
                    if stopped {
                        break;
                    }
                }
                pcost * n + 54
            }
            Cmd::Poly { rel, closed } => {
                // 点が 0 個なら、多角形の閉じる線も長さ 0
                if p[0] > 0 {
                    self.exec = Exec::Poly {
                        op,
                        rel,
                        closed,
                        left: p[0],
                        start: (self.cpx, self.cpy),
                        x: None,
                        stopped: false,
                    };
                }
                if closed { 20 } else { 8 }
            }
            Cmd::Frct { rel } => {
                let (x, y) = if rel {
                    self.rel(p[0], p[1])
                } else {
                    (p[0] as i16, p[1] as i16)
                };
                let (pixels, rows) = self.fill_rect(op, x, y, vram);
                (pcost * pixels + 8 * rows) + 18
            }
            Cmd::Dot => {
                let (x, y) = (self.cpx, self.cpy);
                if self.area(op, x, y) == Area::Draw {
                    let c = self.pattern_color(op);
                    self.plot(op, x, y, c, vram);
                }
                self.advance_x();
                8
            }
            Cmd::Gcpy { rel, s, dsd } => {
                let (xs, ys) = if rel {
                    self.rel(p[0], p[1])
                } else {
                    (p[0] as i16, p[1] as i16)
                };
                let (pixels, rows) = self.copy_pixels(op, xs, ys, p[2], p[3], s, dsd, vram);
                (pcost + 2) * pixels + 10 * rows + 70
            }
            Cmd::Unimplemented(_) => {
                self.unimplemented += 1;
                8
            }
        }
    }

    // ---- コマンドごとの処理 ----

    fn write_param(&mut self, rn: u8, v: u16) {
        let nib = |s: u32| ((v >> s) & 15) as u8;
        match rn {
            0x00 => self.cl0 = v,
            0x01 => self.cl1 = v,
            0x02 => self.ccmp = v,
            0x03 => self.edg = v,
            0x04 => self.mask = v,
            0x05 => {
                (self.ppy, self.pzcy, self.ppx, self.pzcx) = (nib(12), nib(8), nib(4), nib(0));
            }
            0x06 => (self.psy, self.psx) = (nib(12), nib(4)),
            0x07 => {
                (self.pey, self.pzy, self.pex, self.pzx) = (nib(12), nib(8), nib(4), nib(0));
            }
            0x08 => self.xmin = v as i16,
            0x09 => self.ymin = v as i16,
            0x0A => self.xmax = v as i16,
            0x0B => self.ymax = v as i16,
            0x0C => {
                self.rwp_dn = (v >> 14) as u8;
                self.rwp = (self.rwp & 0xFFF) | (u32::from(v & 0xFF) << 12);
            }
            0x0D => self.rwp = (self.rwp & 0xF_F000) | u32::from(v >> 4),
            // 読出し専用（DP、CP）と、アクセスできない番号への書込みは捨てる [V]
            _ => {}
        }
    }

    /// RD と RPR：読出し FIFO に 1 語入れる。満杯なら空くまで待つ（U85 p.191）。
    fn push(&mut self, what: Push, credit: u32, vram: &mut impl Vram) -> u32 {
        if self.rfifo.len() >= FIFO_WORDS {
            self.exec = Exec::Push {
                what,
                credit: credit.max(1),
            };
            return 1;
        }
        let (v, cost) = match what {
            Push::Rd => {
                let v = vram.read(self.rwp);
                self.rwp = wrap(i64::from(self.rwp) + 1);
                (v, 12)
            }
            Push::Rpr(rn) => {
                // RPR は ARD を 0 にする（U85 p.61）
                self.flags &= !SR_ARD;
                (self.param(rn), 6)
            }
        };
        self.rfifo.push_back(v);
        self.exec = Exec::Done;
        cost - credit.min(cost - 1)
    }

    fn transfer(
        &mut self,
        rect: WordRect,
        mm: Option<u8>,
        read: bool,
        k: u32,
        vram: &mut impl Vram,
    ) -> u32 {
        let a = rect.addr(k);
        if read {
            if self.rfifo.len() >= FIFO_WORDS {
                self.exec = Exec::Transfer {
                    rect,
                    mm,
                    read,
                    next: k,
                };
                return 1;
            }
            let v = vram.read(a);
            self.rfifo.push_back(v);
        } else {
            let Some(w) = self.wfifo.pop_front() else {
                self.exec = Exec::Transfer {
                    rect,
                    mm,
                    read,
                    next: k,
                };
                return 1;
            };
            let v = match mm {
                Some(mm) => {
                    let d = vram.read(a);
                    self.modify(d, w, mm)
                }
                None => w,
            };
            vram.write(a, v);
        }
        let cost = rect.word_cycles(k, if read { 12 } else { 16 });
        if k + 1 < rect.words() {
            self.exec = Exec::Transfer {
                rect,
                mm,
                read,
                next: k + 1,
            };
        } else {
            self.rwp = rect.end();
            // プログラム転送の DRD は、最後の語の後で止まったままになる（U85 p.128）
            self.exec = if read && self.ccr & CCR_DDM == 0 {
                Exec::Hang
            } else {
                Exec::Done
            };
        }
        cost
    }

    #[allow(clippy::too_many_arguments)]
    fn poly(
        &mut self,
        op: u16,
        rel: bool,
        closed: bool,
        left: u16,
        start: (i16, i16),
        x: Option<u16>,
        mut stopped: bool,
        vram: &mut impl Vram,
    ) -> u32 {
        let pcost = if op & 7 < 4 { 4 } else { 6 };
        let mut cost = 0;
        let mut left = left;
        if left > 0 {
            // X と Y がそろうまで待つ。そろっていれば同じサイクルで両方を読む
            let x = x.or_else(|| self.wfifo.pop_front());
            let (Some(xw), Some(w)) = (x, x.and_then(|_| self.wfifo.pop_front())) else {
                self.exec = Exec::Poly {
                    op,
                    rel,
                    closed,
                    left,
                    start,
                    x,
                    stopped,
                };
                return 1;
            };
            // 領域の判定で止まった後も、残りの点は読み捨てて、次のコマンドとの区切りを保つ [V]
            if !stopped {
                let (tx, ty) = if rel {
                    self.rel(xw, w)
                } else {
                    (xw as i16, w as i16)
                };
                let (n, s) = self.line(op, tx, ty, vram);
                stopped = s;
                cost += pcost * n + 16;
            }
            left -= 1;
        }
        if left > 0 {
            self.exec = Exec::Poly {
                op,
                rel,
                closed,
                left,
                start,
                x: None,
                stopped,
            };
        } else {
            if closed && !stopped {
                let (n, _) = self.line(op, start.0, start.1, vram);
                cost += pcost * n;
            }
            self.exec = Exec::Done;
        }
        cost.max(1)
    }

    /// CPY と SCPY（語単位）。走査の方向は U85 p.212–213 の表。
    fn copy_words(
        &mut self,
        p: &[u16],
        s: bool,
        dsd: u8,
        mm: Option<u8>,
        vram: &mut impl Vram,
    ) -> u32 {
        let mw = i64::from(self.mw(self.rwp_dn));
        let pss = i64::from((u32::from(p[0] & 0xFF) << 12) | u32::from(p[1] >> 4));
        let (ax, ay) = (i64::from(p[2] as i16), i64::from(p[3] as i16));
        let (x, y) = (ax.abs() + 1, ay.abs() + 1);
        let sxs = if ax < 0 { -1 } else { 1 };
        // +Y は画面の上で、番地は MW だけ減る
        let sys = if ay < 0 { mw } else { -mw };
        // S＝0 は X を先に、S＝1 は Y を先に走査する
        let (prim, count) = if s { (y, x) } else { (x, y) };
        let (dxs, dys) = (
            if dsd & 2 != 0 { -1 } else { 1 },
            if dsd & 1 != 0 { mw } else { -mw },
        );
        let rwp = i64::from(self.rwp);
        for j in 0..count {
            for i in 0..prim {
                let src = if s {
                    pss + sxs * j + sys * i
                } else {
                    pss + sxs * i + sys * j
                };
                let dst = if dsd & 4 != 0 {
                    rwp + dxs * j + dys * i
                } else {
                    rwp + dxs * i + dys * j
                };
                let v = vram.read(wrap(src));
                let v = match mm {
                    Some(mm) => {
                        let d = vram.read(wrap(dst));
                        self.modify(d, v, mm)
                    }
                    None => v,
                };
                vram.write(wrap(dst), v);
            }
        }
        // RWPe：開始の位置から、2 番目の走査の方向へ 1 つ先（U85 p.213 の表 C14-2）
        self.rwp = wrap(if dsd & 4 != 0 {
            rwp + dxs * count
        } else {
            rwp + dys * count
        });
        ((6 * x + 10) * y + 12) as u32
    }

    /// AFRCT と RFRCT。CP の行から (x, y) の行まで、各行を CP の列から x の列まで塗る（U85 p.266）。
    /// 塗った画素の数と、始めた行の数を返す。
    fn fill_rect(&mut self, op: u16, x: i16, y: i16, vram: &mut impl Vram) -> (u32, u32) {
        let (x0, y0) = (i32::from(self.cpx), i32::from(self.cpy));
        let (x1, y1) = (i32::from(x), i32::from(y));
        let (sx, sy) = (step_sign(x1 - x0), step_sign(y1 - y0));
        let (w, h) = ((x1 - x0).abs() + 1, (y1 - y0).abs() + 1);
        // 各行の頭で、パターンの X の位置をコマンド開始時の値に戻す（U85 p.170 の図 6.17(b)）
        let (ppx0, pzcx0) = (self.ppx, self.pzcx);
        let mut pixels = 0;
        let mut rows = 0;
        'rows: for j in 0..h {
            rows += 1;
            (self.ppx, self.pzcx) = (ppx0, pzcx0);
            for i in 0..w {
                let (px, py) = ((x0 + sx * i) as i16, (y0 + sy * j) as i16);
                pixels += 1;
                match self.area(op, px, py) {
                    Area::Stop => {
                        self.set_cp(px, py);
                        break 'rows;
                    }
                    Area::Skip => {}
                    Area::Draw => {
                        let c = self.pattern_color(op);
                        self.plot(op, px, py, c, vram);
                    }
                }
                self.advance_x();
            }
            self.advance_y();
            if j + 1 == h {
                // 終点 Pe は、CP の列で、最後の行の 1 つ先（U85 p.266、p.268）
                self.set_cp(x0 as i16, (y1 + sy) as i16);
            }
        }
        (self.ppx, self.pzcx) = (ppx0, pzcx0);
        (pixels, rows)
    }

    /// AGCPY と RGCPY（画素単位）。走査の方向は U85 p.295–296 の表で、CPY と同じ形。
    #[allow(clippy::too_many_arguments)]
    fn copy_pixels(
        &mut self,
        op: u16,
        xs: i16,
        ys: i16,
        dx: u16,
        dy: u16,
        s: bool,
        dsd: u8,
        vram: &mut impl Vram,
    ) -> (u32, u32) {
        let (dx, dy) = (i32::from(dx as i16), i32::from(dy as i16));
        let (a, b) = (dx.abs() + 1, dy.abs() + 1);
        let (sxs, sys) = (step_sign(dx), step_sign(dy));
        let (prim, count) = if s { (b, a) } else { (a, b) };
        let (dxs, dys) = (
            if dsd & 2 != 0 { -1 } else { 1 },
            if dsd & 1 != 0 { -1 } else { 1 },
        );
        let (cx, cy) = (i32::from(self.cpx), i32::from(self.cpy));
        let (xs, ys) = (i32::from(xs), i32::from(ys));
        let mut pixels = 0;
        let mut rows = 0;
        'rows: for j in 0..count {
            rows += 1;
            for i in 0..prim {
                let (sx, sy) = if s {
                    (xs + sxs * j, ys + sys * i)
                } else {
                    (xs + sxs * i, ys + sys * j)
                };
                let (tx, ty) = if dsd & 4 != 0 {
                    (cx + dxs * j, cy + dys * i)
                } else {
                    (cx + dxs * i, cy + dys * j)
                };
                let (tx, ty) = (tx as i16, ty as i16);
                pixels += 1;
                match self.area(op, tx, ty) {
                    Area::Stop => {
                        self.set_cp(tx, ty);
                        break 'rows;
                    }
                    Area::Skip => {}
                    Area::Draw => {
                        let v = self.read_pixel(sx as i16, sy as i16, vram);
                        let (_, bit) = self.pixel_addr(tx, ty);
                        self.plot(op, tx, ty, Some(v << bit), vram);
                    }
                }
            }
            if j + 1 == count {
                // 終点 Pe（U85 p.294、p.296）
                let pe = if dsd & 4 != 0 {
                    (cx + dxs * count, cy)
                } else {
                    (cx, cy + dys * count)
                };
                self.set_cp(pe.0 as i16, pe.1 as i16);
            }
        }
        (pixels, rows)
    }

    /// CP から (x, y) へ直線を引く。終点は描かない（U85 p.228）。
    /// 処理した画素の数と、領域の判定で止まったかを返す。画素の選び方は Bresenham [V]。
    fn line(&mut self, op: u16, x: i16, y: i16, vram: &mut impl Vram) -> (u32, bool) {
        let (x0, y0) = (i32::from(self.cpx), i32::from(self.cpy));
        let (x1, y1) = (i32::from(x), i32::from(y));
        let (dx, dy) = (x1 - x0, y1 - y0);
        let (adx, ady) = (dx.abs(), dy.abs());
        let (sx, sy) = (dx.signum(), dy.signum());
        let n = adx.max(ady);
        let (mut px, mut py) = (x0, y0);
        let mut err = if adx >= ady {
            2 * ady - adx
        } else {
            2 * adx - ady
        };
        for k in 0..n {
            let (hx, hy) = (px as i16, py as i16);
            match self.area(op, hx, hy) {
                Area::Stop => {
                    self.set_cp(hx, hy);
                    return (k as u32 + 1, true);
                }
                Area::Skip => {}
                Area::Draw => {
                    let c = self.pattern_color(op);
                    self.plot(op, hx, hy, c, vram);
                }
            }
            self.advance_x();
            if adx >= ady {
                if err > 0 {
                    py += sy;
                    err -= 2 * adx;
                }
                err += 2 * ady;
                px += sx;
            } else {
                if err > 0 {
                    px += sx;
                    err -= 2 * ady;
                }
                err += 2 * adx;
                py += sy;
            }
        }
        self.set_cp(x, y);
        (n as u32, false)
    }

    // ---- 画素 ----

    fn bpp(&self) -> u32 {
        match (self.ccr >> 8) & 7 {
            g @ 0..=4 => 1 << g,
            // GBM＝101〜111 は無効（U85 p.67）。1 bit/画素とする（MAME と同じ）[V]
            _ => 1,
        }
    }

    fn mw(&self, dn: u8) -> u32 {
        // MWR0〜3 は rC2、rCA、rD2、rDA（台帳 追補）
        u32::from(self.regs[0x21 + 4 * usize::from(dn & 3)] & 0xFFF)
    }

    /// 論理座標 → (語番地, 画素の最下位ビットの位置)（U85 p.123、p.164）。
    fn pixel_addr(&self, x: i16, y: i16) -> (u32, u32) {
        let bpp = i64::from(self.bpp());
        let dpd = i64::from(self.org_dpd) & !(bpp - 1) & 15;
        let bit = dpd + i64::from(x) * bpp;
        let word = i64::from(self.org) + bit.div_euclid(16)
            - i64::from(y) * i64::from(self.mw(self.org_dn));
        (wrap(word), bit.rem_euclid(16) as u32)
    }

    fn pixel_mask(&self, bit: u32) -> u16 {
        let bpp = self.bpp();
        let m = if bpp == 16 { 0xFFFF } else { (1u16 << bpp) - 1 };
        m << bit
    }

    fn read_pixel(&self, x: i16, y: i16, vram: &mut impl Vram) -> u16 {
        let (addr, bit) = self.pixel_addr(x, y);
        (vram.read(addr) & self.pixel_mask(bit)) >> bit
    }

    /// 1 画素を描く。`color` は 16 bit の色データで、画素の位置のビットだけを使う。None は描かない。
    /// 演算モード（OPM）は U85 p.138–146。
    fn plot(&mut self, op: u16, x: i16, y: i16, color: Option<u16>, vram: &mut impl Vram) {
        let Some(color) = color else {
            return;
        };
        let (addr, bit) = self.pixel_addr(x, y);
        let m = self.pixel_mask(bit);
        let d = vram.read(addr);
        let (p, c) = (d & m, color & m);
        let v = match op & 7 {
            0 => c,
            1 => p | c,
            2 => p & c,
            3 => p ^ c,
            4 if p == self.ccmp & m => c,
            5 if p != self.ccmp & m => c,
            // P<CL、P>CL の CL は、描こうとしている色データ（U85 p.145–146）[V]
            6 if p < c => c,
            7 if p > c => c,
            _ => p,
        };
        vram.write(addr, (d & !m) | v);
    }

    /// 色モード（COL）とパターン RAM から色データを決める（U85 p.148–153）。
    fn pattern_color(&self, op: u16) -> Option<u16> {
        let col = (op >> 3) & 3;
        if col == 3 {
            // パターン RAM の直接指定：4×4 画素、1 画素が 1 語
            let i = usize::from(self.ppy & 3) * 4 + usize::from(self.ppx & 3);
            return Some(self.pattern[i]);
        }
        let bit = (self.pattern[usize::from(self.ppy)] >> self.ppx) & 1;
        match (col, bit) {
            (0, 0) | (2, 0) => Some(self.cl0),
            (0, _) | (1, 1) => Some(self.cl1),
            _ => None,
        }
    }

    /// パターンの X の位置を 1 画素ぶん進める。PZX＋1 回ごとに PPX が進み、PEX の次は PSX に戻る（U85 p.119、p.168）。
    fn advance_x(&mut self) {
        if self.pzcx >= self.pzx {
            self.pzcx = 0;
            self.ppx = if self.ppx == self.pex {
                self.psx
            } else {
                (self.ppx + 1) & 15
            };
        } else {
            self.pzcx += 1;
        }
    }

    fn advance_y(&mut self) {
        if self.pzcy >= self.pzy {
            self.pzcy = 0;
            self.ppy = if self.ppy == self.pey {
                self.psy
            } else {
                (self.ppy + 1) & 15
            };
        } else {
            self.pzcy += 1;
        }
    }

    /// 領域モード（AREA）で、CP の位置を描くか決める（U85 p.154–161）。
    fn area(&mut self, op: u16, x: i16, y: i16) -> Area {
        let inside = x >= self.xmin && x <= self.xmax && y >= self.ymin && y <= self.ymax;
        let mode = (op >> 5) & 7;
        if mode & 3 == 0 {
            return Area::Draw;
        }
        // 001〜011 は外へ出るとき、101〜111 は中へ入るときに働く
        let hit = if mode & 4 == 0 { !inside } else { inside };
        if !hit {
            return Area::Draw;
        }
        match mode & 3 {
            1 => {
                self.flags |= SR_ARD;
                Area::Stop
            }
            2 => Area::Skip,
            _ => {
                self.flags |= SR_ARD;
                Area::Skip
            }
        }
    }

    fn set_cp(&mut self, x: i16, y: i16) {
        (self.cpx, self.cpy) = (x, y);
        let (dp, dpd) = self.pixel_addr(x, y);
        (self.dp, self.dpd) = (dp, dpd as u8);
    }

    fn rel(&self, dx: u16, dy: u16) -> (i16, i16) {
        (
            self.cpx.wrapping_add(dx as i16),
            self.cpy.wrapping_add(dy as i16),
        )
    }

    /// 修飾モード（MM）：MASK が 1 のビットだけ、置換・OR・AND・EOR の結果にする（U85 p.130、p.117）。
    fn modify(&self, d: u16, s: u16, mm: u8) -> u16 {
        let r = match mm & 3 {
            0 => s,
            1 => d | s,
            2 => d & s,
            _ => d ^ s,
        };
        (d & !self.mask) | (r & self.mask)
    }
}

fn pack4(a: u8, b: u8, c: u8, d: u8) -> u16 {
    (u16::from(a) << 12) | (u16::from(b) << 8) | (u16::from(c) << 4) | u16::from(d)
}

/// 0 は正の向きとみなす。
fn step_sign(v: i32) -> i32 {
    if v < 0 { -1 } else { 1 }
}
