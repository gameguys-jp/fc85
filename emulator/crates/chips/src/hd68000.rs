//! HD68000-8（日立。MC68000L8 と同等品）のコア。台帳 `docs/parts/HD68000.md`。
//!
//! 外から見えるのはバスサイクルの列だけにする。CPU は命令を 1 つずつ実行し、バスサイクルごとに
//! `Bus` を呼ぶ。`Bus` は S0 の時刻（電源投入からの CPU クロック数）を受け取り、基板の側をその時刻
//! まで進めてから答える。待ち（DTACK の遅れ、VPA の同期サイクル）は答えの `waits` で返す。
//!
//! モデルの範囲：
//! - 命令の時間は台帳 §2.8・§2.9 の表（DB85 表27〜39、M68 表7-3〜7-16）による。合計は 4×（バスサイクルの数）＋待ち＋
//!   内部の空きクロック。空きクロックの位置とバスサイクルの順は、プリフェッチ（IR と IRC の 2 語）
//!   の動きから組み立て、SingleStepTests（台帳の SST）と突き合わせた。
//! - `pc` は命令の境目で IR にある命令の番地。命令の途中では、使った拡張語の分だけ進む。
//!   バスエラー・アドレスエラーで積む PC はこの途中の値（分岐先の読出しなら分岐先 − 4）。
//! - 資料で決まらない動作と SST との差は `docs/impl/emulator.md` §03 に書く。
//! - コードは M68・DB85 の表と説明から自前で書いた。ほかのエミュレータのコードは写していない。

use serde::{Deserialize, Serialize};

/// 機能コード（FC2–FC0）。
pub mod fc {
    pub const USER_DATA: u8 = 1;
    pub const USER_PROGRAM: u8 = 2;
    pub const SUPERVISOR_DATA: u8 = 5;
    pub const SUPERVISOR_PROGRAM: u8 = 6;
    /// 割込みの応答（IACK）。
    pub const CPU_SPACE: u8 = 7;
}

/// バスサイクルの幅。バイトは番地の bit 0 で UDS（偶数）か LDS（奇数）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Size {
    Byte,
    Word,
}

/// 読出し・書込みのバスサイクルの答え。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusReply {
    /// DTACK（または VPA の同期サイクルの終わり）。`waits` は待ちのクロック数（サイクルは 4＋waits
    /// クロック）。読出しの値は `data`（バイトは下位 8 bit）。
    Ok { data: u16, waits: u32 },
    /// BERR。`clocks` は S0 からサイクルを打ち切るまでのクロック数。CPU はバスエラーの例外処理に入る。
    BusError { clocks: u32 },
}

/// 割込みの応答（IACK）サイクルの答え。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IackReply {
    /// 装置がベクタ番号を返した。
    Vector { vector: u8, waits: u32 },
    /// VPA：オートベクタ（24＋レベル）。
    Autovector { waits: u32 },
    /// BERR：スプリアス割込み（ベクタ 24）。
    Spurious { clocks: u32 },
}

/// CPU から見たバス。`at` はバスサイクルの S0 の立上りの時刻（電源投入からの CPU クロック数）。
pub trait Bus {
    /// 読出しのバスサイクル。
    fn read(&mut self, at: u64, addr: u32, fc: u8, size: Size) -> BusReply;
    /// 書込みのバスサイクル。バイトは `value` の下位 8 bit（68000 は両方のレーンに同じ値を出す）。
    fn write(&mut self, at: u64, addr: u32, fc: u8, size: Size, value: u16) -> BusReply;
    /// 割込みの応答サイクル（`level` は 1〜7）。
    fn iack(&mut self, at: u64, level: u8) -> IackReply;
    /// 命令の境目で IPL2–IPL0 を見る（0 は要求なし、7 はノンマスカブル）。
    fn ipl(&mut self, at: u64) -> u8;
    /// RESET 命令が RESET 端子を 124 クロック出す。周辺をリセットする。
    fn reset_out(&mut self, _at: u64) {}
}

/// 実行の状態。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    #[default]
    Running,
    /// STOP 命令で割込みを待つ。
    Stopped,
    /// 二重バスフォルトで止まった（HALT を出す）。リセットでだけ戻る。
    Halted,
}

/// レジスタと内部の状態。`pc` は IR にある命令の番地（命令の境目で）。`prefetch` は IR と IRC。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hd68000 {
    pub d: [u32; 8],
    /// A0〜A6。A7 は `usp`・`ssp` のうち SR の S で選ぶ方。
    pub a: [u32; 7],
    pub usp: u32,
    pub ssp: u32,
    pub sr: u16,
    pub pc: u32,
    pub prefetch: [u16; 2],
    pub state: RunState,
    /// 電源投入からの CPU クロック数。
    pub clock: u64,
    /// 実行中の命令（IRD）。グループ 0 の例外のフレームに積む。
    #[serde(default)]
    ird: u16,
    /// 前の命令の境目で見た IPL。レベル 7 は、7 に変わったときに受け付ける。
    #[serde(default)]
    ipl_seen: u8,
    /// 命令が不正命令・特権違反になった（トレースを起こさない）。命令の中だけで使う。
    #[serde(skip)]
    no_trace: bool,
}

const FLAG_C: u16 = 0x0001;
const FLAG_V: u16 = 0x0002;
const FLAG_Z: u16 = 0x0004;
const FLAG_N: u16 = 0x0008;
const FLAG_X: u16 = 0x0010;
const SR_S: u16 = 0x2000;
const SR_T: u16 = 0x8000;
/// SR の実装されたビット（T、S、I2–I0、X、N、Z、V、C）。
const SR_MASK: u16 = 0xa71f;
/// 外に出る番地は A1–A23 と UDS・LDS の 24 bit。
const ADDR_MASK: u32 = 0x00ff_ffff;

/// 例外のベクタ番号。
mod vector {
    pub const BUS_ERROR: u8 = 2;
    pub const ADDRESS_ERROR: u8 = 3;
    pub const ILLEGAL: u8 = 4;
    pub const ZERO_DIVIDE: u8 = 5;
    pub const CHK: u8 = 6;
    pub const TRAPV: u8 = 7;
    pub const PRIVILEGE: u8 = 8;
    pub const TRACE: u8 = 9;
    pub const LINE_A: u8 = 10;
    pub const LINE_F: u8 = 11;
    pub const SPURIOUS: u8 = 24;
    pub const AUTOVECTOR: u8 = 24;
    pub const TRAP: u8 = 32;
}

/// フォルトを起こしたアクセス（グループ 0 の例外のフレームに積む）。
#[derive(Clone, Copy, Debug)]
struct Access {
    addr: u32,
    fc: u8,
    read: bool,
    /// 命令（プログラム空間）の読出し。
    program: bool,
}

/// 命令を打ち切るフォルト。
#[derive(Clone, Copy, Debug)]
enum Fault {
    Bus(Access),
    Address(Access),
}

type R<T> = Result<T, Fault>;

/// 演算の幅。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sz {
    B,
    W,
    L,
}

impl Sz {
    /// 命令の size フィールド（00、01、10）。
    fn from_bits(bits: u16) -> Sz {
        match bits & 3 {
            0 => Sz::B,
            1 => Sz::W,
            _ => Sz::L,
        }
    }
    fn mask(self) -> u32 {
        match self {
            Sz::B => 0xff,
            Sz::W => 0xffff,
            Sz::L => 0xffff_ffff,
        }
    }
    fn msb(self) -> u32 {
        match self {
            Sz::B => 0x80,
            Sz::W => 0x8000,
            Sz::L => 0x8000_0000,
        }
    }
    fn bytes(self) -> u32 {
        match self {
            Sz::B => 1,
            Sz::W => 2,
            Sz::L => 4,
        }
    }
}

/// 幅に合わせて符号を拡張する。
fn sext(v: u32, sz: Sz) -> u32 {
    match sz {
        Sz::B => v as u8 as i8 as i32 as u32,
        Sz::W => v as u16 as i16 as i32 as u32,
        Sz::L => v,
    }
}

/// 実効番地。
#[derive(Clone, Copy, Debug)]
enum Ea {
    D(usize),
    A(usize),
    M(u32),
    I(u32),
}

/// 実効番地のモードの番号（0〜11。12 は使えない組み合わせ）。
fn ea_kind(mode: u16, reg: u16) -> u16 {
    if mode < 7 {
        mode
    } else if reg < 5 {
        7 + reg
    } else {
        12
    }
}

// 使える実効番地の組（bit n がモード番号 n）。M68 表7-1 の分類。
const EA_ANY: u16 = 0x0fff;
const EA_DATA: u16 = 0x0ffd;
const EA_CTRL: u16 = 0x07e4;
const EA_DATA_ALT: u16 = 0x01fd;
const EA_MEM_ALT: u16 = 0x01fc;
const EA_CTRL_ALT: u16 = 0x01e4;

/// 命令の下位 6 bit の実効番地が `set` に入るか。
fn ea_ok(op: u16, set: u16) -> bool {
    let k = ea_kind((op >> 3) & 7, op & 7);
    k < 12 && set & (1 << k) != 0
}

/// 2 項演算の種類。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Alu {
    Or,
    And,
    Sub,
    Add,
    Eor,
    Cmp,
}

/// DIVU の時間（クロック数。源がデータレジスタのときの命令全体）。
///
/// 76 を基本に、商を 1 bit ずつ求める 15 回の繰返しで 0・2・4 クロックを足す。桁あふれは 10。
/// 手順は台帳 §2.9（解析 CWIK を書き起こしたもの）による。
fn divu_clocks(dividend: u32, divisor: u16) -> u32 {
    if dividend >> 16 >= u32::from(divisor) {
        return 10;
    }
    let mut cycles = 38;
    let hdivisor = u32::from(divisor) << 16;
    let mut rem = dividend;
    for _ in 0..15 {
        let carry = rem & 0x8000_0000 != 0;
        rem <<= 1;
        if carry {
            rem = rem.wrapping_sub(hdivisor);
        } else {
            cycles += 2;
            if rem >= hdivisor {
                rem -= hdivisor;
                cycles -= 1;
            }
        }
    }
    cycles * 2
}

/// DIVS の時間（クロック数。源がデータレジスタのときの命令全体）。手順は台帳 §2.9。商が 16 bit の
/// 符号付きに収まらないだけの桁あふれ（相対）は、早く終わらない（CWIK。SST とは違う。§03）。
fn divs_clocks(dividend: i32, divisor: i16) -> u32 {
    let mut cycles = 6;
    if dividend < 0 {
        cycles += 1;
    }
    let adividend = dividend.unsigned_abs();
    let adivisor = u32::from(divisor.unsigned_abs());
    if adividend >> 16 >= adivisor {
        return (cycles + 2) * 2;
    }
    let mut aquot = adividend / adivisor;
    cycles += 55;
    if divisor >= 0 {
        if dividend >= 0 {
            cycles -= 1;
        } else {
            cycles += 1;
        }
    }
    for _ in 0..15 {
        if aquot & 0x8000 == 0 {
            cycles += 1;
        }
        aquot <<= 1;
    }
    cycles * 2
}

impl Hd68000 {
    pub fn new() -> Self {
        Self {
            sr: 0x2700,
            ..Self::default()
        }
    }

    // ------------------------------------------------------------------ 外から呼ぶ

    /// リセットの例外処理（SSP と PC を 0 番地と 4 番地から読み、プリフェッチを満たす）。
    ///
    /// 40 クロック（6 回の読出し）。M68 表7-16 の値。DB85 は 34 で、食い違う（台帳 §2.8）。
    pub fn reset(&mut self, bus: &mut impl Bus) {
        self.state = RunState::Running;
        self.sr = 0x2700;
        self.ipl_seen = 0;
        self.idle(14);
        if self.reset_sequence(bus).is_err() {
            self.state = RunState::Halted;
        }
    }

    /// 命令を 1 つ（または例外処理を 1 つ）実行し、使ったクロック数を返す。止まっている間は、割込みを
    /// 見て 4 クロック進める。
    pub fn step(&mut self, bus: &mut impl Bus) -> u32 {
        let start = self.clock;
        match self.state {
            RunState::Halted => self.idle(4),
            RunState::Stopped => {
                if let Some(level) = self.poll_interrupt(bus) {
                    self.state = RunState::Running;
                    self.take_interrupt(bus, level);
                } else {
                    self.idle(4);
                }
            }
            RunState::Running => {
                if let Some(level) = self.poll_interrupt(bus) {
                    self.take_interrupt(bus, level);
                } else {
                    let trace = self.sr & SR_T != 0;
                    self.no_trace = false;
                    match self.execute(bus) {
                        Ok(()) => {
                            if trace && !self.no_trace && self.state == RunState::Running {
                                self.idle(4);
                                let pc = self.pc;
                                if let Err(f) = self.exception(bus, vector::TRACE, pc) {
                                    self.group0(bus, f);
                                }
                            }
                        }
                        Err(f) => self.group0(bus, f),
                    }
                }
            }
        }
        (self.clock - start) as u32
    }

    // ------------------------------------------------------------------ レジスタ

    fn supervisor(&self) -> bool {
        self.sr & SR_S != 0
    }

    fn data_fc(&self) -> u8 {
        if self.supervisor() {
            fc::SUPERVISOR_DATA
        } else {
            fc::USER_DATA
        }
    }

    fn program_fc(&self) -> u8 {
        if self.supervisor() {
            fc::SUPERVISOR_PROGRAM
        } else {
            fc::USER_PROGRAM
        }
    }

    /// An（A7 は S で選ぶ）。
    fn ar(&self, n: usize) -> u32 {
        if n < 7 {
            self.a[n]
        } else if self.supervisor() {
            self.ssp
        } else {
            self.usp
        }
    }

    fn set_ar(&mut self, n: usize, v: u32) {
        if n < 7 {
            self.a[n] = v;
        } else if self.supervisor() {
            self.ssp = v;
        } else {
            self.usp = v;
        }
    }

    /// Dn の下位 `sz` を書く（上位は残す）。
    fn set_d(&mut self, n: usize, v: u32, sz: Sz) {
        let m = sz.mask();
        self.d[n] = (self.d[n] & !m) | (v & m);
    }

    /// D0〜D7、A0〜A7 を 0〜15 の番号で読む。
    fn reg(&self, n: usize) -> u32 {
        if n < 8 { self.d[n] } else { self.ar(n - 8) }
    }

    fn set_reg(&mut self, n: usize, v: u32) {
        if n < 8 {
            self.d[n] = v;
        } else {
            self.set_ar(n - 8, v);
        }
    }

    /// SR を書く（実装されたビットだけ）。
    fn set_sr(&mut self, v: u16) {
        self.sr = v & SR_MASK;
    }

    fn idle(&mut self, clocks: u32) {
        self.clock += u64::from(clocks);
    }

    /// 条件（cc）。
    fn cond(&self, cc: u16) -> bool {
        let c = self.sr & FLAG_C != 0;
        let v = self.sr & FLAG_V != 0;
        let z = self.sr & FLAG_Z != 0;
        let n = self.sr & FLAG_N != 0;
        match cc & 15 {
            0 => true,
            1 => false,
            2 => !c && !z,
            3 => c || z,
            4 => !c,
            5 => c,
            6 => !z,
            7 => z,
            8 => !v,
            9 => v,
            10 => !n,
            11 => n,
            12 => n == v,
            13 => n != v,
            14 => !z && n == v,
            _ => z || n != v,
        }
    }

    // ------------------------------------------------------------------ バスサイクル

    fn cycle_read<B: Bus>(
        &mut self,
        bus: &mut B,
        addr: u32,
        fc: u8,
        size: Size,
        program: bool,
    ) -> R<u16> {
        match bus.read(self.clock, addr & ADDR_MASK, fc, size) {
            BusReply::Ok { data, waits } => {
                self.clock += 4 + u64::from(waits);
                Ok(data)
            }
            BusReply::BusError { clocks } => {
                self.clock += u64::from(clocks);
                Err(Fault::Bus(Access {
                    addr,
                    fc,
                    read: true,
                    program,
                }))
            }
        }
    }

    fn cycle_write<B: Bus>(
        &mut self,
        bus: &mut B,
        addr: u32,
        fc: u8,
        size: Size,
        value: u16,
    ) -> R<()> {
        match bus.write(self.clock, addr & ADDR_MASK, fc, size, value) {
            BusReply::Ok { waits, .. } => {
                self.clock += 4 + u64::from(waits);
                Ok(())
            }
            BusReply::BusError { clocks } => {
                self.clock += u64::from(clocks);
                Err(Fault::Bus(Access {
                    addr,
                    fc,
                    read: false,
                    program: false,
                }))
            }
        }
    }

    /// 奇数番地へのワードのアクセス。バスサイクルを出さずにアドレスエラーにする（台帳 §2.2）。
    fn odd(addr: u32, fc: u8, read: bool, program: bool) -> Fault {
        Fault::Address(Access {
            addr,
            fc,
            read,
            program,
        })
    }

    fn rd_b<B: Bus>(&mut self, bus: &mut B, addr: u32) -> R<u32> {
        let fc = self.data_fc();
        Ok(u32::from(
            self.cycle_read(bus, addr, fc, Size::Byte, false)? & 0xff,
        ))
    }

    fn rd_w<B: Bus>(&mut self, bus: &mut B, addr: u32) -> R<u32> {
        let fc = self.data_fc();
        if addr & 1 != 0 {
            return Err(Self::odd(addr, fc, true, false));
        }
        Ok(u32::from(self.cycle_read(
            bus,
            addr,
            fc,
            Size::Word,
            false,
        )?))
    }

    fn rd_l<B: Bus>(&mut self, bus: &mut B, addr: u32) -> R<u32> {
        let hi = self.rd_w(bus, addr)?;
        let lo = self.rd_w(bus, addr.wrapping_add(2))?;
        Ok(hi << 16 | lo)
    }

    fn rd<B: Bus>(&mut self, bus: &mut B, addr: u32, sz: Sz) -> R<u32> {
        match sz {
            Sz::B => self.rd_b(bus, addr),
            Sz::W => self.rd_w(bus, addr),
            Sz::L => self.rd_l(bus, addr),
        }
    }

    fn wr_b<B: Bus>(&mut self, bus: &mut B, addr: u32, v: u32) -> R<()> {
        let fc = self.data_fc();
        self.cycle_write(bus, addr, fc, Size::Byte, (v & 0xff) as u16)
    }

    fn wr_w<B: Bus>(&mut self, bus: &mut B, addr: u32, v: u32) -> R<()> {
        let fc = self.data_fc();
        if addr & 1 != 0 {
            return Err(Self::odd(addr, fc, false, false));
        }
        self.cycle_write(bus, addr, fc, Size::Word, v as u16)
    }

    /// 書込み。ロングは `lo_first` なら下位の語（番地＋2）を先に書く（読み書きする命令と -(An)）。
    fn wr<B: Bus>(&mut self, bus: &mut B, addr: u32, sz: Sz, v: u32, lo_first: bool) -> R<()> {
        match sz {
            Sz::B => self.wr_b(bus, addr, v),
            Sz::W => self.wr_w(bus, addr, v),
            Sz::L => {
                let a2 = addr.wrapping_add(2);
                if lo_first {
                    if addr & 1 != 0 {
                        return Err(Self::odd(a2, self.data_fc(), false, false));
                    }
                    self.wr_w(bus, a2, v)?;
                    self.wr_w(bus, addr, v >> 16)
                } else {
                    self.wr_w(bus, addr, v >> 16)?;
                    self.wr_w(bus, a2, v)
                }
            }
        }
    }

    /// スタックに積む（上位の語を先に、SP−4 に書く）。
    fn push_l<B: Bus>(&mut self, bus: &mut B, v: u32) -> R<()> {
        let sp = self.ar(7).wrapping_sub(4);
        self.set_ar(7, sp);
        self.wr_w(bus, sp, v >> 16)?;
        self.wr_w(bus, sp.wrapping_add(2), v)
    }

    fn rd_prog<B: Bus>(&mut self, bus: &mut B, addr: u32) -> R<u16> {
        let fc = self.program_fc();
        if addr & 1 != 0 {
            return Err(Self::odd(addr, fc, true, true));
        }
        self.cycle_read(bus, addr, fc, Size::Word, true)
    }

    // ------------------------------------------------------------------ プリフェッチ

    /// 拡張語を 1 つ使う（IRC を返し、次の語を IRC に読む）。
    fn ext<B: Bus>(&mut self, bus: &mut B) -> R<u16> {
        let v = self.prefetch[1];
        self.pc = self.pc.wrapping_add(2);
        self.prefetch[1] = self.rd_prog(bus, self.pc.wrapping_add(2))?;
        Ok(v)
    }

    fn ext_l<B: Bus>(&mut self, bus: &mut B) -> R<u32> {
        let hi = u32::from(self.ext(bus)?);
        let lo = u32::from(self.ext(bus)?);
        Ok(hi << 16 | lo)
    }

    /// 命令の終わりのプリフェッチ（IR ← IRC、次の語を IRC に読む）。
    fn np<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        self.prefetch[0] = self.prefetch[1];
        self.pc = self.pc.wrapping_add(2);
        self.prefetch[1] = self.rd_prog(bus, self.pc.wrapping_add(2))?;
        Ok(())
    }

    /// 分岐先の最初の語を IR に読む。フォルトのときに積む PC は分岐先 − 4。
    fn fetch_ir<B: Bus>(&mut self, bus: &mut B, target: u32) -> R<()> {
        self.pc = target.wrapping_sub(4);
        self.prefetch[0] = self.rd_prog(bus, target)?;
        Ok(())
    }

    /// 分岐先の 2 語目を IRC に読み、PC を分岐先にする。
    fn fetch_irc<B: Bus>(&mut self, bus: &mut B, target: u32) -> R<()> {
        self.prefetch[1] = self.rd_prog(bus, target.wrapping_add(2))?;
        self.pc = target;
        Ok(())
    }

    /// 分岐（IR と IRC を続けて読む）。
    fn jump<B: Bus>(&mut self, bus: &mut B, target: u32) -> R<()> {
        self.fetch_ir(bus, target)?;
        self.fetch_irc(bus, target)
    }

    /// SR を書き換えた後のプリフェッチのやり直し（次の命令の 2 語を読み直す）。
    fn refill<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        let next = self.pc.wrapping_add(2);
        self.jump(bus, next)
    }

    // ------------------------------------------------------------------ 例外処理

    fn reset_sequence<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        let f = fc::SUPERVISOR_PROGRAM;
        let ssp_hi = u32::from(self.cycle_read(bus, 0, f, Size::Word, true)?);
        let ssp_lo = u32::from(self.cycle_read(bus, 2, f, Size::Word, true)?);
        self.ssp = ssp_hi << 16 | ssp_lo;
        let pc_hi = u32::from(self.cycle_read(bus, 4, f, Size::Word, true)?);
        let pc_lo = u32::from(self.cycle_read(bus, 6, f, Size::Word, true)?);
        let pc = pc_hi << 16 | pc_lo;
        self.fetch_ir(bus, pc)?;
        self.idle(2);
        self.fetch_irc(bus, pc)
    }

    /// ベクタを読み、例外処理ルーチンの最初の 2 語を読む（読出しの間に 2 クロック空く）。
    fn vector_jump<B: Bus>(&mut self, bus: &mut B, vector: u8) -> R<()> {
        let addr = u32::from(vector) * 4;
        let hi = self.rd_w(bus, addr)?;
        let lo = self.rd_w(bus, addr + 2)?;
        let target = hi << 16 | lo;
        self.fetch_ir(bus, target)?;
        self.idle(2);
        self.fetch_irc(bus, target)
    }

    /// グループ 1・2 の例外処理。PC の下位、SR、PC の上位の順に積み、ベクタから分岐する。
    /// 3 回の書込みと 4 回の読出しで 30 クロック（前の空きは呼ぶ側が足す）。
    fn exception<B: Bus>(&mut self, bus: &mut B, vec: u8, pc: u32) -> R<()> {
        let old = self.sr;
        self.sr = (self.sr | SR_S) & !SR_T;
        let sp = self.ssp.wrapping_sub(6);
        self.ssp = sp;
        self.wr_w(bus, sp.wrapping_add(4), pc)?;
        self.wr_w(bus, sp, u32::from(old))?;
        self.wr_w(bus, sp.wrapping_add(2), pc >> 16)?;
        self.vector_jump(bus, vec)
    }

    /// 不正命令・特権違反・line A・line F（34 クロック）。積む PC は命令の番地。
    fn illegal<B: Bus>(&mut self, bus: &mut B, vec: u8) -> R<()> {
        self.no_trace = true;
        self.idle(4);
        let pc = self.pc;
        self.exception(bus, vec, pc)
    }

    /// グループ 0（バスエラー、アドレスエラー）。7 語のフレーム、50 クロック（台帳 §2.3、§2.8）。
    /// この途中でまたフォルトになると二重バスフォルトで止まる。
    fn group0<B: Bus>(&mut self, bus: &mut B, fault: Fault) {
        let (vec, acc) = match fault {
            Fault::Bus(a) => (vector::BUS_ERROR, a),
            Fault::Address(a) => (vector::ADDRESS_ERROR, a),
        };
        let pc = self.pc;
        let old = self.sr;
        self.sr = (self.sr | SR_S) & !SR_T;
        self.idle(4);
        // 状態語：R/W（bit 4）、I/N（bit 3）、FC。上位は IRD の上位（SST と同じ。I/N の極性は §03）。
        let status = u32::from(self.ird & 0xffe0)
            | if acc.read { 0x10 } else { 0 }
            | if acc.program { 0x08 } else { 0 }
            | u32::from(acc.fc);
        let ird = u32::from(self.ird);
        let result = self.group0_frame(bus, pc, old, ird, acc.addr, status, vec);
        if result.is_err() {
            self.state = RunState::Halted;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn group0_frame<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u32,
        old_sr: u16,
        ird: u32,
        addr: u32,
        status: u32,
        vec: u8,
    ) -> R<()> {
        let sp = self.ssp.wrapping_sub(14);
        self.ssp = sp;
        self.wr_w(bus, sp.wrapping_add(12), pc)?;
        self.wr_w(bus, sp.wrapping_add(8), u32::from(old_sr))?;
        self.wr_w(bus, sp.wrapping_add(10), pc >> 16)?;
        self.wr_w(bus, sp.wrapping_add(6), ird)?;
        self.wr_w(bus, sp.wrapping_add(4), addr)?;
        self.wr_w(bus, sp, status)?;
        self.wr_w(bus, sp.wrapping_add(2), addr >> 16)?;
        self.vector_jump(bus, vec)
    }

    /// 命令の境目で割込みの要求を見る。受け付けるならレベルを返す。
    fn poll_interrupt<B: Bus>(&mut self, bus: &mut B) -> Option<u8> {
        let level = bus.ipl(self.clock) & 7;
        let mask = ((self.sr >> 8) & 7) as u8;
        let nmi = level == 7 && self.ipl_seen != 7;
        self.ipl_seen = level;
        if level > mask || nmi {
            Some(level)
        } else {
            None
        }
    }

    /// 割込みの例外処理（44 クロック。IACK を 4 クロックとして。台帳 §2.4、§2.8）。
    fn take_interrupt<B: Bus>(&mut self, bus: &mut B, level: u8) {
        if let Err(f) = self.interrupt(bus, level) {
            self.group0(bus, f);
        }
    }

    fn interrupt<B: Bus>(&mut self, bus: &mut B, level: u8) -> R<()> {
        let old = self.sr;
        self.sr = ((self.sr | SR_S) & !SR_T & !0x0700) | (u16::from(level) << 8);
        self.idle(6);
        let pc = self.pc;
        let sp = self.ssp.wrapping_sub(6);
        self.ssp = sp;
        self.wr_w(bus, sp.wrapping_add(4), pc)?;
        let vec = match bus.iack(self.clock, level) {
            IackReply::Vector { vector, waits } => {
                self.clock += 4 + u64::from(waits);
                vector
            }
            IackReply::Autovector { waits } => {
                self.clock += 4 + u64::from(waits);
                vector::AUTOVECTOR + level
            }
            IackReply::Spurious { clocks } => {
                self.clock += u64::from(clocks);
                vector::SPURIOUS
            }
        };
        self.idle(4);
        self.wr_w(bus, sp, u32::from(old))?;
        self.wr_w(bus, sp.wrapping_add(2), pc >> 16)?;
        self.vector_jump(bus, vec)
    }

    // ------------------------------------------------------------------ 実効番地

    /// d8(An,Xn)・d8(PC,Xn) の番地。
    fn index(&self, base: u32, ext: u16) -> u32 {
        let r = usize::from((ext >> 12) & 7);
        let x = if ext & 0x8000 != 0 {
            self.ar(r)
        } else {
            self.d[r]
        };
        let x = if ext & 0x0800 != 0 { x } else { sext(x, Sz::W) };
        base.wrapping_add(sext(u32::from(ext), Sz::B))
            .wrapping_add(x)
    }

    /// (An)+・-(An) の増減（A7 のバイトは 2）。
    fn step_size(r: usize, sz: Sz) -> u32 {
        if sz == Sz::B && r == 7 { 2 } else { sz.bytes() }
    }

    /// 実効番地を求める。拡張語を使い、-(An) と d8(An,Xn) は 2 クロック空け、(An)+・-(An) の
    /// レジスタを更新する（M68 表7-3）。
    fn ea<B: Bus>(&mut self, bus: &mut B, mode: u16, reg: u16, sz: Sz) -> R<Ea> {
        let r = usize::from(reg);
        Ok(match mode {
            0 => Ea::D(r),
            1 => Ea::A(r),
            2 => Ea::M(self.ar(r)),
            3 => {
                let a = self.ar(r);
                self.set_ar(r, a.wrapping_add(Self::step_size(r, sz)));
                Ea::M(a)
            }
            4 => {
                self.idle(2);
                let a = self.ar(r).wrapping_sub(Self::step_size(r, sz));
                self.set_ar(r, a);
                Ea::M(a)
            }
            5 => {
                let d = sext(u32::from(self.ext(bus)?), Sz::W);
                Ea::M(self.ar(r).wrapping_add(d))
            }
            6 => {
                self.idle(2);
                let e = self.ext(bus)?;
                Ea::M(self.index(self.ar(r), e))
            }
            _ => match reg {
                0 => Ea::M(sext(u32::from(self.ext(bus)?), Sz::W)),
                1 => Ea::M(self.ext_l(bus)?),
                2 => {
                    let base = self.pc.wrapping_add(2);
                    let d = sext(u32::from(self.ext(bus)?), Sz::W);
                    Ea::M(base.wrapping_add(d))
                }
                3 => {
                    self.idle(2);
                    let base = self.pc.wrapping_add(2);
                    let e = self.ext(bus)?;
                    Ea::M(self.index(base, e))
                }
                _ => Ea::I(match sz {
                    Sz::L => self.ext_l(bus)?,
                    _ => u32::from(self.ext(bus)?) & sz.mask(),
                }),
            },
        })
    }

    fn read_ea<B: Bus>(&mut self, bus: &mut B, ea: Ea, sz: Sz) -> R<u32> {
        Ok(match ea {
            Ea::D(r) => self.d[r] & sz.mask(),
            Ea::A(r) => self.ar(r) & sz.mask(),
            Ea::M(a) => self.rd(bus, a, sz)?,
            Ea::I(v) => v,
        })
    }

    /// JMP・JSR の分岐先。最後の拡張語は IRC のまま使う（分岐先から読み直すので）。
    fn jump_target<B: Bus>(&mut self, bus: &mut B, mode: u16, reg: u16) -> R<u32> {
        let r = usize::from(reg);
        let irc = u32::from(self.prefetch[1]);
        Ok(match (mode, reg) {
            (2, _) => self.ar(r),
            (5, _) => {
                self.idle(2);
                self.ar(r).wrapping_add(sext(irc, Sz::W))
            }
            (6, _) => {
                self.idle(6);
                self.index(self.ar(r), irc as u16)
            }
            (7, 0) => {
                self.idle(2);
                sext(irc, Sz::W)
            }
            (7, 1) => {
                let hi = u32::from(self.ext(bus)?);
                hi << 16 | u32::from(self.prefetch[1])
            }
            (7, 2) => {
                self.idle(2);
                self.pc.wrapping_add(2).wrapping_add(sext(irc, Sz::W))
            }
            _ => {
                self.idle(6);
                self.index(self.pc.wrapping_add(2), irc as u16)
            }
        })
    }

    /// LEA・PEA・MOVEM の番地（制御モード）。拡張語を使う。d8 は前に 2 クロック空く。
    fn control_addr<B: Bus>(&mut self, bus: &mut B, mode: u16, reg: u16) -> R<u32> {
        let r = usize::from(reg);
        Ok(match (mode, reg) {
            (2, _) => self.ar(r),
            (5, _) => {
                let d = sext(u32::from(self.ext(bus)?), Sz::W);
                self.ar(r).wrapping_add(d)
            }
            (6, _) => {
                self.idle(2);
                let e = self.ext(bus)?;
                self.index(self.ar(r), e)
            }
            (7, 0) => sext(u32::from(self.ext(bus)?), Sz::W),
            (7, 1) => self.ext_l(bus)?,
            (7, 2) => {
                let base = self.pc.wrapping_add(2);
                base.wrapping_add(sext(u32::from(self.ext(bus)?), Sz::W))
            }
            _ => {
                self.idle(2);
                let base = self.pc.wrapping_add(2);
                let e = self.ext(bus)?;
                self.index(base, e)
            }
        })
    }

    // ------------------------------------------------------------------ フラグ

    /// N と Z を結果から、V と C を 0 にする（X は残す）。
    fn set_nz(&mut self, r: u32, sz: Sz) {
        let mut f = self.sr & (0xff00 | FLAG_X);
        if r & sz.mask() == 0 {
            f |= FLAG_Z;
        }
        if r & sz.msb() != 0 {
            f |= FLAG_N;
        }
        self.sr = f;
    }

    fn x(&self) -> u32 {
        u32::from(self.sr & FLAG_X != 0)
    }

    /// d + s + x。`keep_z` は ADDX（結果が 0 なら Z を変えない）。
    fn add(&mut self, s: u32, d: u32, sz: Sz, x: u32, keep_z: bool) -> u32 {
        let m = sz.mask();
        let (s, d) = (s & m, d & m);
        let r = d.wrapping_add(s).wrapping_add(x) & m;
        let msb = sz.msb();
        let mut f = self.sr & 0xff00;
        if ((s & d) | ((s | d) & !r)) & msb != 0 {
            f |= FLAG_C | FLAG_X;
        }
        if (s ^ r) & (d ^ r) & msb != 0 {
            f |= FLAG_V;
        }
        if r & msb != 0 {
            f |= FLAG_N;
        }
        if r == 0 && (!keep_z || self.sr & FLAG_Z != 0) {
            f |= FLAG_Z;
        }
        self.sr = f;
        r
    }

    /// d − s − x。`keep_z` は SUBX・NEGX。
    fn sub(&mut self, s: u32, d: u32, sz: Sz, x: u32, keep_z: bool) -> u32 {
        let m = sz.mask();
        let (s, d) = (s & m, d & m);
        let r = d.wrapping_sub(s).wrapping_sub(x) & m;
        let msb = sz.msb();
        let mut f = self.sr & 0xff00;
        if ((s & !d) | (r & !d) | (s & r)) & msb != 0 {
            f |= FLAG_C | FLAG_X;
        }
        if (s ^ d) & (r ^ d) & msb != 0 {
            f |= FLAG_V;
        }
        if r & msb != 0 {
            f |= FLAG_N;
        }
        if r == 0 && (!keep_z || self.sr & FLAG_Z != 0) {
            f |= FLAG_Z;
        }
        self.sr = f;
        r
    }

    /// d − s のフラグだけ（X は変えない）。
    fn cmp(&mut self, s: u32, d: u32, sz: Sz) {
        let x = self.sr & FLAG_X;
        self.sub(s, d, sz, 0, false);
        self.sr = (self.sr & !FLAG_X) | x;
    }

    fn alu(&mut self, alu: Alu, s: u32, d: u32, sz: Sz) -> u32 {
        match alu {
            Alu::Add => self.add(s, d, sz, 0, false),
            Alu::Sub => self.sub(s, d, sz, 0, false),
            Alu::Cmp => {
                self.cmp(s, d, sz);
                d
            }
            Alu::And => {
                let r = s & d;
                self.set_nz(r, sz);
                r
            }
            Alu::Or => {
                let r = s | d;
                self.set_nz(r, sz);
                r
            }
            Alu::Eor => {
                let r = s ^ d;
                self.set_nz(r, sz);
                r
            }
        }
    }

    // ------------------------------------------------------------------ 実行

    fn execute<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        let op = self.prefetch[0];
        self.ird = op;
        match op >> 12 {
            0x0 => self.line0(bus, op),
            0x1..=0x3 => self.op_move(bus, op),
            0x4 => self.line4(bus, op),
            0x5 => self.line5(bus, op),
            0x6 => self.op_bcc(bus, op),
            0x7 => self.op_moveq(bus, op),
            0x8 => self.line8(bus, op),
            0x9 => self.line9d(bus, op, Alu::Sub),
            0xa => self.illegal(bus, vector::LINE_A),
            0xb => self.lineb(bus, op),
            0xc => self.linec(bus, op),
            0xd => self.line9d(bus, op, Alu::Add),
            0xe => self.linee(bus, op),
            _ => self.illegal(bus, vector::LINE_F),
        }
    }

    /// 実効番地を読み、命令の終わりのプリフェッチをし、書き戻す（読み書きする命令）。
    /// レジスタ（Dn）ならロングのときに `reg_idle_l` クロック空ける。
    fn modify<B: Bus>(
        &mut self,
        bus: &mut B,
        op: u16,
        sz: Sz,
        reg_idle_l: u32,
        f: impl FnOnce(&mut Self, u32) -> u32,
    ) -> R<()> {
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, sz)?;
        match ea {
            Ea::D(r) => {
                self.np(bus)?;
                if sz == Sz::L {
                    self.idle(reg_idle_l);
                }
                let v = f(self, self.d[r] & sz.mask());
                self.set_d(r, v, sz);
            }
            Ea::M(a) => {
                let d = self.rd(bus, a, sz)?;
                self.np(bus)?;
                let v = f(self, d);
                self.wr(bus, a, sz, v, true)?;
            }
            _ => unreachable!("modify の実効番地は Dn かメモリ"),
        }
        Ok(())
    }

    // ---- line 0：即値、ビット操作、MOVEP

    fn line0<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if op & 0x0100 != 0 {
            if (op >> 3) & 7 == 1 {
                return self.op_movep(bus, op);
            }
            return self.op_bit(bus, op, false);
        }
        let kind = (op >> 9) & 7;
        if kind == 4 {
            return self.op_bit(bus, op, true);
        }
        match op {
            0x003c => return self.op_imm_ccr(bus, Alu::Or),
            0x023c => return self.op_imm_ccr(bus, Alu::And),
            0x0a3c => return self.op_imm_ccr(bus, Alu::Eor),
            0x007c => return self.op_imm_sr(bus, Alu::Or),
            0x027c => return self.op_imm_sr(bus, Alu::And),
            0x0a7c => return self.op_imm_sr(bus, Alu::Eor),
            _ => {}
        }
        let alu = match kind {
            0 => Alu::Or,
            1 => Alu::And,
            2 => Alu::Sub,
            3 => Alu::Add,
            5 => Alu::Eor,
            6 => Alu::Cmp,
            _ => return self.illegal(bus, vector::ILLEGAL),
        };
        self.op_imm(bus, op, alu)
    }

    /// ORI・ANDI・SUBI・ADDI・EORI・CMPI（M68 表7-7）。
    fn op_imm<B: Bus>(&mut self, bus: &mut B, op: u16, alu: Alu) -> R<()> {
        if (op >> 6) & 3 == 3 || !ea_ok(op, EA_DATA_ALT) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let sz = Sz::from_bits(op >> 6);
        let imm = match sz {
            Sz::L => self.ext_l(bus)?,
            _ => u32::from(self.ext(bus)?) & sz.mask(),
        };
        if alu == Alu::Cmp {
            let ea = self.ea(bus, (op >> 3) & 7, op & 7, sz)?;
            let d = self.read_ea(bus, ea, sz)?;
            self.np(bus)?;
            if sz == Sz::L && matches!(ea, Ea::D(_)) {
                self.idle(2);
            }
            self.cmp(imm, d, sz);
            return Ok(());
        }
        self.modify(bus, op, sz, 4, |cpu, d| cpu.alu(alu, imm, d, sz))
    }

    /// ORI・ANDI・EORI to CCR（20 クロック）。
    fn op_imm_ccr<B: Bus>(&mut self, bus: &mut B, alu: Alu) -> R<()> {
        let imm = self.ext(bus)? & 0x1f;
        self.idle(8);
        let ccr = self.sr & 0x1f;
        let v = match alu {
            Alu::Or => ccr | imm,
            Alu::And => ccr & imm,
            _ => ccr ^ imm,
        };
        self.sr = (self.sr & 0xff00) | v;
        self.refill(bus)
    }

    /// ORI・ANDI・EORI to SR（特権命令、20 クロック）。
    fn op_imm_sr<B: Bus>(&mut self, bus: &mut B, alu: Alu) -> R<()> {
        if !self.supervisor() {
            return self.illegal(bus, vector::PRIVILEGE);
        }
        let imm = self.ext(bus)?;
        self.idle(8);
        let v = match alu {
            Alu::Or => self.sr | imm,
            Alu::And => self.sr & imm,
            _ => self.sr ^ imm,
        };
        self.set_sr(v);
        self.refill(bus)
    }

    /// BTST・BCHG・BCLR・BSET（M68 表7-10）。`imm` は静的（ビット番号が拡張語）。
    fn op_bit<B: Bus>(&mut self, bus: &mut B, op: u16, imm: bool) -> R<()> {
        let kind = (op >> 6) & 3;
        let mode = (op >> 3) & 7;
        let set = if kind != 0 {
            EA_DATA_ALT
        } else if imm {
            EA_DATA & !(1 << 11)
        } else {
            EA_DATA
        };
        if !ea_ok(op, set) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let bit = if imm {
            u32::from(self.ext(bus)?)
        } else {
            self.d[usize::from((op >> 9) & 7)]
        };
        if mode == 0 {
            let r = usize::from(op & 7);
            let b = bit & 31;
            let v = self.d[r];
            self.np(bus)?;
            self.idle(match kind {
                0 => 2,
                2 => {
                    if b < 16 {
                        4
                    } else {
                        6
                    }
                }
                _ => {
                    if b < 16 {
                        2
                    } else {
                        4
                    }
                }
            });
            self.d[r] = self.bit_op(kind, v, b);
            return Ok(());
        }
        let ea = self.ea(bus, mode, op & 7, Sz::B)?;
        let v = self.read_ea(bus, ea, Sz::B)?;
        self.np(bus)?;
        if matches!(ea, Ea::I(_)) {
            // BTST Dn,#：レジスタと同じく 2 クロック空く（SST。M68 表7-10 の 4＋実効番地より 2 長い）。
            self.idle(2);
        }
        let r = self.bit_op(kind, v, bit & 7);
        if kind != 0
            && let Ea::M(a) = ea
        {
            self.wr_b(bus, a, r)?;
        }
        Ok(())
    }

    fn bit_op(&mut self, kind: u16, v: u32, b: u32) -> u32 {
        let m = 1u32 << b;
        if v & m == 0 {
            self.sr |= FLAG_Z;
        } else {
            self.sr &= !FLAG_Z;
        }
        match kind {
            0 => v,
            1 => v ^ m,
            2 => v & !m,
            _ => v | m,
        }
    }

    /// MOVEP（M68 表7-15）。偶数・奇数の片方のレーンにバイトで、上位から順に転送する。
    fn op_movep<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let dn = usize::from((op >> 9) & 7);
        let an = usize::from(op & 7);
        let d = sext(u32::from(self.ext(bus)?), Sz::W);
        let mut a = self.ar(an).wrapping_add(d);
        let bytes = if op & 0x40 != 0 { 4 } else { 2 };
        if op & 0x80 != 0 {
            let v = self.d[dn];
            for i in (0..bytes).rev() {
                self.wr_b(bus, a, v >> (i * 8))?;
                a = a.wrapping_add(2);
            }
        } else {
            let mut v = 0u32;
            for _ in 0..bytes {
                v = v << 8 | self.rd_b(bus, a)?;
                a = a.wrapping_add(2);
            }
            if bytes == 4 {
                self.d[dn] = v;
            } else {
                self.set_d(dn, v, Sz::W);
            }
        }
        self.np(bus)
    }

    // ---- line 1〜3：MOVE、MOVEA

    /// MOVE・MOVEA（M68 表7-4、7-5）。
    fn op_move<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let sz = match op >> 12 {
            1 => Sz::B,
            3 => Sz::W,
            _ => Sz::L,
        };
        let dm = (op >> 6) & 7;
        let dr = usize::from((op >> 9) & 7);
        let src_set = if sz == Sz::B { EA_DATA } else { EA_ANY };
        let dk = ea_kind(dm, dr as u16);
        let dst_ok = if dm == 1 {
            sz != Sz::B
        } else {
            dk < 12 && EA_DATA_ALT & (1 << dk) != 0
        };
        if !ea_ok(op, src_set) || !dst_ok {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let sm = (op >> 3) & 7;
        let src = self.ea(bus, sm, op & 7, sz)?;
        let v = self.read_ea(bus, src, sz)?;
        if dm == 1 {
            self.np(bus)?;
            self.set_ar(dr, sext(v, sz));
            return Ok(());
        }
        self.set_nz(v, sz);
        match dm {
            0 => {
                self.np(bus)?;
                self.set_d(dr, v, sz);
            }
            2 => {
                let a = self.ar(dr);
                self.wr(bus, a, sz, v, false)?;
                self.np(bus)?;
            }
            3 => {
                let a = self.ar(dr);
                self.wr(bus, a, sz, v, false)?;
                self.set_ar(dr, a.wrapping_add(Self::step_size(dr, sz)));
                self.np(bus)?;
            }
            4 => {
                let a = self.ar(dr).wrapping_sub(Self::step_size(dr, sz));
                self.np(bus)?;
                if sz == Sz::L {
                    // 2 ずつ減らしながら、下位の語、上位の語の順に書く。
                    self.set_ar(dr, a.wrapping_add(2));
                    self.wr_w(bus, a.wrapping_add(2), v)?;
                    self.set_ar(dr, a);
                    self.wr_w(bus, a, v >> 16)?;
                } else {
                    self.set_ar(dr, a);
                    self.wr(bus, a, sz, v, false)?;
                }
            }
            5 => {
                let d = sext(u32::from(self.ext(bus)?), Sz::W);
                let a = self.ar(dr).wrapping_add(d);
                self.wr(bus, a, sz, v, false)?;
                self.np(bus)?;
            }
            6 => {
                self.idle(2);
                let e = self.ext(bus)?;
                let a = self.index(self.ar(dr), e);
                self.wr(bus, a, sz, v, false)?;
                self.np(bus)?;
            }
            _ => {
                if dr == 0 {
                    let a = sext(u32::from(self.ext(bus)?), Sz::W);
                    self.wr(bus, a, sz, v, false)?;
                } else if matches!(src, Ea::M(_)) {
                    // 源がメモリのときは、番地の 2 語目を書込みの後で読む。
                    let hi = u32::from(self.ext(bus)?);
                    let a = hi << 16 | u32::from(self.prefetch[1]);
                    self.wr(bus, a, sz, v, false)?;
                    self.ext(bus)?;
                } else {
                    let a = self.ext_l(bus)?;
                    self.wr(bus, a, sz, v, false)?;
                }
                self.np(bus)?;
            }
        }
        Ok(())
    }

    // ---- line 4：その他

    fn line4<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        match op {
            0x4afc => return self.illegal(bus, vector::ILLEGAL),
            0x4e70 => return self.op_reset(bus),
            0x4e71 => return self.np(bus),
            0x4e72 => return self.op_stop(bus),
            0x4e73 => return self.op_rte(bus),
            0x4e75 => return self.op_rts(bus),
            0x4e76 => return self.op_trapv(bus),
            0x4e77 => return self.op_rtr(bus),
            _ => {}
        }
        match op & 0xfff8 {
            0x4e40 | 0x4e48 => {
                self.idle(4);
                let pc = self.pc.wrapping_add(2);
                return self.exception(bus, vector::TRAP + (op & 15) as u8, pc);
            }
            0x4e50 => return self.op_link(bus, op),
            0x4e58 => return self.op_unlk(bus, op),
            0x4e60 | 0x4e68 => return self.op_move_usp(bus, op),
            _ => {}
        }
        match op & 0xffc0 {
            0x4e80 => return self.op_jsr(bus, op),
            0x4ec0 => return self.op_jmp(bus, op),
            _ => {}
        }
        match op & 0x01c0 {
            0x01c0 => return self.op_lea(bus, op),
            0x0180 => return self.op_chk(bus, op),
            _ => {}
        }
        let size = (op >> 6) & 3;
        let mode = (op >> 3) & 7;
        match (op >> 8) & 0xf {
            0x0 if size == 3 => self.op_move_from_sr(bus, op),
            0x0 => self.op_unary(bus, op, 0),
            0x2 if size == 3 => self.illegal(bus, vector::ILLEGAL),
            0x2 => self.op_unary(bus, op, 2),
            0x4 if size == 3 => self.op_move_to_sr(bus, op, false),
            0x4 => self.op_unary(bus, op, 1),
            0x6 if size == 3 => self.op_move_to_sr(bus, op, true),
            0x6 => self.op_unary(bus, op, 3),
            0x8 => match size {
                0 => self.op_nbcd(bus, op),
                1 if mode == 0 => self.op_swap(bus, op),
                1 => self.op_pea(bus, op),
                _ if mode == 0 => self.op_ext(bus, op),
                _ => self.op_movem(bus, op),
            },
            0xa if size == 3 => self.op_tas(bus, op),
            0xa => self.op_tst(bus, op),
            0xc if size >= 2 => self.op_movem(bus, op),
            _ => self.illegal(bus, vector::ILLEGAL),
        }
    }

    /// NEGX（0）、NEG（1）、CLR（2）、NOT（3）（M68 表7-8）。CLR も先に読む。
    fn op_unary<B: Bus>(&mut self, bus: &mut B, op: u16, kind: u8) -> R<()> {
        if !ea_ok(op, EA_DATA_ALT) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let sz = Sz::from_bits(op >> 6);
        self.modify(bus, op, sz, 2, |cpu, d| match kind {
            0 => {
                let x = cpu.x();
                cpu.sub(d, 0, sz, x, true)
            }
            1 => cpu.sub(d, 0, sz, 0, false),
            2 => {
                cpu.set_nz(0, sz);
                0
            }
            _ => {
                let r = !d & sz.mask();
                cpu.set_nz(r, sz);
                r
            }
        })
    }

    fn op_tst<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_DATA_ALT) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let sz = Sz::from_bits(op >> 6);
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, sz)?;
        let v = self.read_ea(bus, ea, sz)?;
        self.set_nz(v, sz);
        self.np(bus)
    }

    /// TAS。メモリは読み書き一体のサイクル（読出し、2 クロック、書込み）。
    fn op_tas<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_DATA_ALT) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, Sz::B)?;
        match ea {
            Ea::D(r) => {
                let v = self.d[r];
                self.set_nz(v, Sz::B);
                self.d[r] = v | 0x80;
                self.np(bus)
            }
            Ea::M(a) => {
                let v = self.rd_b(bus, a)?;
                self.set_nz(v, Sz::B);
                self.idle(2);
                self.wr_b(bus, a, v | 0x80)?;
                self.np(bus)
            }
            _ => unreachable!(),
        }
    }

    fn op_nbcd<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_DATA_ALT) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, Sz::B)?;
        match ea {
            Ea::D(r) => {
                self.np(bus)?;
                self.idle(2);
                let v = self.sbcd(self.d[r] & 0xff, 0);
                self.set_d(r, v, Sz::B);
            }
            Ea::M(a) => {
                let d = self.rd_b(bus, a)?;
                self.np(bus)?;
                let v = self.sbcd(d, 0);
                self.wr_b(bus, a, v)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn op_swap<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let r = usize::from(op & 7);
        let v = self.d[r].rotate_left(16);
        self.d[r] = v;
        self.set_nz(v, Sz::L);
        self.np(bus)
    }

    fn op_ext<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let r = usize::from(op & 7);
        if op & 0x40 != 0 {
            let v = sext(self.d[r], Sz::W);
            self.d[r] = v;
            self.set_nz(v, Sz::L);
        } else {
            let v = sext(self.d[r], Sz::B);
            self.set_d(r, v, Sz::W);
            self.set_nz(v, Sz::W);
        }
        self.np(bus)
    }

    /// MOVE from SR（68000 では特権でない）。メモリは先に読む。
    fn op_move_from_sr<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_DATA_ALT) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, Sz::W)?;
        let sr = u32::from(self.sr);
        match ea {
            Ea::D(r) => {
                self.np(bus)?;
                self.idle(2);
                self.set_d(r, sr, Sz::W);
            }
            Ea::M(a) => {
                self.rd_w(bus, a)?;
                self.np(bus)?;
                self.wr_w(bus, a, sr)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    /// MOVE to CCR・MOVE to SR（12 クロック＋実効番地）。書いた後にプリフェッチをやり直す。
    fn op_move_to_sr<B: Bus>(&mut self, bus: &mut B, op: u16, whole: bool) -> R<()> {
        if !ea_ok(op, EA_DATA) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        if whole && !self.supervisor() {
            return self.illegal(bus, vector::PRIVILEGE);
        }
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, Sz::W)?;
        let v = self.read_ea(bus, ea, Sz::W)? as u16;
        self.idle(4);
        if whole {
            self.set_sr(v);
        } else {
            self.sr = (self.sr & 0xff00) | (v & 0x1f);
        }
        self.refill(bus)
    }

    fn op_lea<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_CTRL) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let a = self.control_addr(bus, (op >> 3) & 7, op & 7)?;
        if (op >> 3) & 7 == 6 || op & 0x3f == 0x3b {
            self.idle(2);
        }
        self.set_ar(usize::from((op >> 9) & 7), a);
        self.np(bus)
    }

    fn op_pea<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_CTRL) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let a = self.control_addr(bus, (op >> 3) & 7, op & 7)?;
        if (op >> 3) & 7 == 6 || op & 0x3f == 0x3b {
            self.idle(2);
        }
        if matches!(op & 0x3f, 0x38 | 0x39) {
            // (xxx).W・(xxx).L は積んでからプリフェッチする。
            self.push_l(bus, a)?;
            return self.np(bus);
        }
        self.np(bus)?;
        self.push_l(bus, a)
    }

    fn op_jmp<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_CTRL) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let target = self.jump_target(bus, (op >> 3) & 7, op & 7)?;
        self.jump(bus, target)
    }

    fn op_jsr<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_CTRL) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let mode = (op >> 3) & 7;
        let target = self.jump_target(bus, mode, op & 7)?;
        let ret = self.pc.wrapping_add(if mode == 2 { 2 } else { 4 });
        self.fetch_ir(bus, target)?;
        self.push_l(bus, ret)?;
        self.fetch_irc(bus, target)
    }

    /// CHK（M68 表7-14、例外は表7-16）。上限を超えると N を 0、負なら N を 1 にして例外。
    fn op_chk<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_DATA) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, Sz::W)?;
        let bound = self.read_ea(bus, ea, Sz::W)? as u16 as i16;
        self.np(bus)?;
        let dn = self.d[usize::from((op >> 9) & 7)] as u16 as i16;
        // 上限との比較が先（4 クロック）、負の判定が後（6 クロック）。例外のときの N は Dn の符号、
        // 例外にならないときの N は変えない。Z・V・C は資料にない（§03）。
        let mut f = self.sr & (0xff00 | FLAG_X);
        if dn == 0 {
            f |= FLAG_Z;
        }
        let n = if dn < 0 { FLAG_N } else { 0 };
        if dn > bound {
            self.sr = f | n;
            self.idle(4);
        } else if dn < 0 {
            self.sr = f | n;
            self.idle(6);
        } else {
            self.sr = f | (self.sr & FLAG_N);
            self.idle(6);
            return Ok(());
        }
        let pc = self.pc;
        self.exception(bus, vector::CHK, pc)
    }

    /// MOVEM（M68 表7-12）。メモリからレジスタへは最後に 1 語余分に読む。
    fn op_movem<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let to_reg = op & 0x0400 != 0;
        let mode = (op >> 3) & 7;
        let set = if to_reg {
            EA_CTRL | (1 << 3)
        } else {
            EA_CTRL_ALT | (1 << 4)
        };
        if !ea_ok(op, set) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let sz = if op & 0x40 != 0 { Sz::L } else { Sz::W };
        let mask = self.ext(bus)?;
        let an = usize::from(op & 7);
        if mode == 4 {
            // -(An)：A7 から D0 へ、番地の高い方から書く。ロングは下位の語が先。
            let mut a = self.ar(an);
            for i in 0..16 {
                if mask & (1 << i) == 0 {
                    continue;
                }
                let v = self.reg(15 - i);
                a = a.wrapping_sub(2);
                self.wr_w(bus, a, v)?;
                if sz == Sz::L {
                    a = a.wrapping_sub(2);
                    self.wr_w(bus, a, v >> 16)?;
                }
            }
            self.set_ar(an, a);
            return self.np(bus);
        }
        let mut a = if mode == 3 {
            // (An)+ は最初のアクセスの前に An を 2 進める（フォルトのときに残る値。SST）。
            let a = self.ar(an);
            self.set_ar(an, a.wrapping_add(2));
            a
        } else {
            self.control_addr(bus, mode, op & 7)?
        };
        for i in 0..16 {
            if mask & (1 << i) == 0 {
                continue;
            }
            if to_reg {
                let v = if sz == Sz::L {
                    self.rd_l(bus, a)?
                } else {
                    sext(self.rd_w(bus, a)?, Sz::W)
                };
                self.set_reg(i, v);
            } else {
                let v = self.reg(i);
                self.wr(bus, a, sz, v, false)?;
            }
            a = a.wrapping_add(sz.bytes());
        }
        if to_reg {
            self.rd_w(bus, a)?;
            if mode == 3 {
                self.set_ar(an, a);
            }
        }
        self.np(bus)
    }

    fn op_link<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let r = usize::from(op & 7);
        let d = sext(u32::from(self.ext(bus)?), Sz::W);
        let sp = self.ar(7).wrapping_sub(4);
        let v = if r == 7 { sp } else { self.ar(r) };
        self.push_l(bus, v)?;
        self.set_ar(r, sp);
        self.set_ar(7, sp.wrapping_add(d));
        self.np(bus)
    }

    fn op_unlk<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let r = usize::from(op & 7);
        let a = self.ar(r);
        self.set_ar(7, a);
        let v = self.rd_l(bus, a)?;
        self.set_ar(7, a.wrapping_add(4));
        self.set_ar(r, v);
        self.np(bus)
    }

    fn op_move_usp<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !self.supervisor() {
            return self.illegal(bus, vector::PRIVILEGE);
        }
        let r = usize::from(op & 7);
        if op & 8 != 0 {
            self.set_ar(r, self.usp);
        } else {
            self.usp = self.ar(r);
        }
        self.np(bus)
    }

    /// RESET（132 クロック）。4 クロックの後、RESET 端子を 124 クロック出す（台帳 §2.7）。
    fn op_reset<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        if !self.supervisor() {
            return self.illegal(bus, vector::PRIVILEGE);
        }
        self.idle(4);
        bus.reset_out(self.clock);
        self.idle(124);
        self.np(bus)
    }

    /// STOP。即値は IRC にあるので読まない。割込みで起きると、次の命令の番地を積む。
    fn op_stop<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        if !self.supervisor() {
            return self.illegal(bus, vector::PRIVILEGE);
        }
        let v = self.prefetch[1];
        self.set_sr(v);
        self.pc = self.pc.wrapping_add(4);
        self.idle(4);
        self.state = RunState::Stopped;
        Ok(())
    }

    /// RTE。PC の上位、SR、PC の下位の順に読む（SST と同じ順）。
    fn op_rte<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        if !self.supervisor() {
            return self.illegal(bus, vector::PRIVILEGE);
        }
        let sp = self.ssp;
        let hi = self.rd_w(bus, sp.wrapping_add(2))?;
        let sr = self.rd_w(bus, sp)?;
        let lo = self.rd_w(bus, sp.wrapping_add(4))?;
        self.ssp = sp.wrapping_add(6);
        self.set_sr(sr as u16);
        self.jump(bus, hi << 16 | lo)
    }

    fn op_rtr<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        let sp = self.ar(7);
        let hi = self.rd_w(bus, sp.wrapping_add(2))?;
        let ccr = self.rd_w(bus, sp)?;
        let lo = self.rd_w(bus, sp.wrapping_add(4))?;
        self.set_ar(7, sp.wrapping_add(6));
        self.sr = (self.sr & 0xff00) | (ccr as u16 & 0x1f);
        self.jump(bus, hi << 16 | lo)
    }

    fn op_rts<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        let sp = self.ar(7);
        let target = self.rd_l(bus, sp)?;
        self.set_ar(7, sp.wrapping_add(4));
        self.jump(bus, target)
    }

    fn op_trapv<B: Bus>(&mut self, bus: &mut B) -> R<()> {
        self.np(bus)?;
        if self.sr & FLAG_V != 0 {
            let pc = self.pc;
            return self.exception(bus, vector::TRAPV, pc);
        }
        Ok(())
    }

    // ---- line 5：ADDQ、SUBQ、Scc、DBcc

    fn line5<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let mode = (op >> 3) & 7;
        if (op >> 6) & 3 == 3 {
            if mode == 1 {
                return self.op_dbcc(bus, op);
            }
            return self.op_scc(bus, op);
        }
        let sz = Sz::from_bits(op >> 6);
        let q = match (op >> 9) & 7 {
            0 => 8,
            n => u32::from(n),
        };
        let alu = if op & 0x100 != 0 { Alu::Sub } else { Alu::Add };
        if mode == 1 {
            if sz == Sz::B {
                return self.illegal(bus, vector::ILLEGAL);
            }
            // An は 32 bit で足し引きし、フラグは変えない（M68 表7-7 は 8 クロック）。
            let r = usize::from(op & 7);
            self.np(bus)?;
            self.idle(4);
            let a = self.ar(r);
            self.set_ar(
                r,
                if alu == Alu::Add {
                    a.wrapping_add(q)
                } else {
                    a.wrapping_sub(q)
                },
            );
            return Ok(());
        }
        if !ea_ok(op, EA_DATA_ALT) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        self.modify(bus, op, sz, 4, |cpu, d| cpu.alu(alu, q, d, sz))
    }

    fn op_scc<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if !ea_ok(op, EA_DATA_ALT) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let t = self.cond(op >> 8);
        let v = if t { 0xff } else { 0 };
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, Sz::B)?;
        match ea {
            Ea::D(r) => {
                self.np(bus)?;
                if t {
                    self.idle(2);
                }
                self.set_d(r, v, Sz::B);
            }
            Ea::M(a) => {
                self.rd_b(bus, a)?;
                self.np(bus)?;
                self.wr_b(bus, a, v)?;
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    /// DBcc（M68 表7-11）。回数が尽きたときは分岐先を 1 語読んで捨てる（3 回の読出し）。
    fn op_dbcc<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let target = self
            .pc
            .wrapping_add(2)
            .wrapping_add(sext(u32::from(self.prefetch[1]), Sz::W));
        if self.cond(op >> 8) {
            self.idle(4);
            self.ext(bus)?;
            return self.np(bus);
        }
        let r = usize::from(op & 7);
        let count = (self.d[r] as u16).wrapping_sub(1);
        self.set_d(r, u32::from(count), Sz::W);
        self.idle(2);
        if count != 0xffff {
            return self.jump(bus, target);
        }
        let pc = self.pc;
        self.rd_prog(bus, target)?;
        self.pc = pc;
        self.ext(bus)?;
        self.np(bus)
    }

    // ---- line 6：Bcc、BRA、BSR

    fn op_bcc<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let cc = (op >> 8) & 15;
        let base = self.pc.wrapping_add(2);
        let d8 = op & 0xff;
        let target = if d8 == 0 {
            base.wrapping_add(sext(u32::from(self.prefetch[1]), Sz::W))
        } else {
            base.wrapping_add(sext(u32::from(d8), Sz::B))
        };
        if cc == 1 {
            let ret = self.pc.wrapping_add(if d8 == 0 { 4 } else { 2 });
            self.idle(2);
            self.push_l(bus, ret)?;
            return self.jump(bus, target);
        }
        if self.cond(cc) {
            self.idle(2);
            return self.jump(bus, target);
        }
        self.idle(4);
        if d8 == 0 {
            self.ext(bus)?;
        }
        self.np(bus)
    }

    // ---- line 7：MOVEQ

    fn op_moveq<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        if op & 0x100 != 0 {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let v = sext(u32::from(op), Sz::B);
        self.d[usize::from((op >> 9) & 7)] = v;
        self.set_nz(v, Sz::L);
        self.np(bus)
    }

    // ---- line 8・9・B・C・D：2 項演算

    fn line8<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        match (op >> 6) & 7 {
            3 => self.op_div(bus, op, false),
            7 => self.op_div(bus, op, true),
            4 if (op >> 3) & 7 <= 1 => self.op_bcd(bus, op, false),
            0..=2 => self.op_ea_dn(bus, op, Alu::Or),
            _ => self.op_dn_ea(bus, op, Alu::Or),
        }
    }

    fn line9d<B: Bus>(&mut self, bus: &mut B, op: u16, alu: Alu) -> R<()> {
        match (op >> 6) & 7 {
            3 | 7 => self.op_adda(bus, op, alu),
            4..=6 if (op >> 3) & 7 <= 1 => self.op_addx(bus, op, alu),
            0..=2 => self.op_ea_dn(bus, op, alu),
            _ => self.op_dn_ea(bus, op, alu),
        }
    }

    fn lineb<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        match (op >> 6) & 7 {
            3 | 7 => self.op_adda(bus, op, Alu::Cmp),
            0..=2 => self.op_ea_dn(bus, op, Alu::Cmp),
            _ if (op >> 3) & 7 == 1 => self.op_cmpm(bus, op),
            _ => self.op_dn_ea(bus, op, Alu::Eor),
        }
    }

    fn linec<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let mode = (op >> 3) & 7;
        match (op >> 6) & 7 {
            3 => self.op_mul(bus, op, false),
            7 => self.op_mul(bus, op, true),
            4 if mode <= 1 => self.op_bcd(bus, op, true),
            5 if mode <= 1 => self.op_exg(bus, op),
            6 if mode == 1 => self.op_exg(bus, op),
            0..=2 => self.op_ea_dn(bus, op, Alu::And),
            _ => self.op_dn_ea(bus, op, Alu::And),
        }
    }

    /// op <ea>,Dn（M68 表7-6）。ロングは 2 クロック、源がレジスタか即値なら 4 クロック空く（CMP は 2）。
    fn op_ea_dn<B: Bus>(&mut self, bus: &mut B, op: u16, alu: Alu) -> R<()> {
        let sz = Sz::from_bits(op >> 6);
        let set = if matches!(alu, Alu::And | Alu::Or) || sz == Sz::B {
            EA_DATA
        } else {
            EA_ANY
        };
        if !ea_ok(op, set) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, sz)?;
        let s = self.read_ea(bus, ea, sz)?;
        self.np(bus)?;
        if sz == Sz::L {
            self.idle(if alu == Alu::Cmp || matches!(ea, Ea::M(_)) {
                2
            } else {
                4
            });
        }
        let dn = usize::from((op >> 9) & 7);
        let r = self.alu(alu, s, self.d[dn] & sz.mask(), sz);
        if alu != Alu::Cmp {
            self.set_d(dn, r, sz);
        }
        Ok(())
    }

    /// op Dn,<ea>（メモリへ。EOR は Dn も）。
    fn op_dn_ea<B: Bus>(&mut self, bus: &mut B, op: u16, alu: Alu) -> R<()> {
        let set = if alu == Alu::Eor {
            EA_DATA_ALT
        } else {
            EA_MEM_ALT
        };
        if !ea_ok(op, set) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let sz = Sz::from_bits(op >> 6);
        let s = self.d[usize::from((op >> 9) & 7)];
        self.modify(bus, op, sz, 4, |cpu, d| cpu.alu(alu, s, d, sz))
    }

    /// ADDA・SUBA・CMPA。
    fn op_adda<B: Bus>(&mut self, bus: &mut B, op: u16, alu: Alu) -> R<()> {
        if !ea_ok(op, EA_ANY) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let sz = if op & 0x100 != 0 { Sz::L } else { Sz::W };
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, sz)?;
        let s = sext(self.read_ea(bus, ea, sz)?, sz);
        self.np(bus)?;
        let r = usize::from((op >> 9) & 7);
        let d = self.ar(r);
        match alu {
            Alu::Cmp => {
                self.idle(2);
                self.cmp(s, d, Sz::L);
            }
            _ => {
                self.idle(if sz == Sz::L && matches!(ea, Ea::M(_)) {
                    2
                } else {
                    4
                });
                self.set_ar(
                    r,
                    if alu == Alu::Add {
                        d.wrapping_add(s)
                    } else {
                        d.wrapping_sub(s)
                    },
                );
            }
        }
        Ok(())
    }

    /// ADDX・SUBX（M68 表7-13）。-(An) のロングは下位の語から読み、下位の語を先に書く。
    fn op_addx<B: Bus>(&mut self, bus: &mut B, op: u16, alu: Alu) -> R<()> {
        let sz = Sz::from_bits(op >> 6);
        let rx = usize::from(op & 7);
        let ry = usize::from((op >> 9) & 7);
        let calc = |cpu: &mut Self, s: u32, d: u32| {
            let x = cpu.x();
            if alu == Alu::Add {
                cpu.add(s, d, sz, x, true)
            } else {
                cpu.sub(s, d, sz, x, true)
            }
        };
        if op & 8 == 0 {
            self.np(bus)?;
            if sz == Sz::L {
                self.idle(4);
            }
            let r = calc(self, self.d[rx], self.d[ry]);
            self.set_d(ry, r, sz);
            return Ok(());
        }
        self.idle(2);
        let s = self.read_predec(bus, rx, sz)?;
        let d = self.read_predec(bus, ry, sz)?;
        let a = self.ar(ry);
        let r = calc(self, s, d);
        if sz == Sz::L {
            self.wr_w(bus, a.wrapping_add(2), r)?;
            self.np(bus)?;
            self.wr_w(bus, a, r >> 16)
        } else {
            self.np(bus)?;
            self.wr(bus, a, sz, r, false)
        }
    }

    /// -(An) で読む（ロングは下位の語から）。
    fn read_predec<B: Bus>(&mut self, bus: &mut B, r: usize, sz: Sz) -> R<u32> {
        if sz == Sz::L {
            let a = self.ar(r).wrapping_sub(2);
            self.set_ar(r, a);
            let lo = self.rd_w(bus, a)?;
            let a = a.wrapping_sub(2);
            self.set_ar(r, a);
            let hi = self.rd_w(bus, a)?;
            Ok(hi << 16 | lo)
        } else {
            let a = self.ar(r).wrapping_sub(Self::step_size(r, sz));
            self.set_ar(r, a);
            self.rd(bus, a, sz)
        }
    }

    fn op_cmpm<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let sz = Sz::from_bits(op >> 6);
        let ry = usize::from(op & 7);
        let rx = usize::from((op >> 9) & 7);
        let s = match self.ea(bus, 3, ry as u16, sz)? {
            Ea::M(a) => self.rd(bus, a, sz)?,
            _ => unreachable!(),
        };
        let d = match self.ea(bus, 3, rx as u16, sz)? {
            Ea::M(a) => self.rd(bus, a, sz)?,
            _ => unreachable!(),
        };
        self.np(bus)?;
        self.cmp(s, d, sz);
        Ok(())
    }

    /// ABCD・SBCD（M68 表7-13）。
    fn op_bcd<B: Bus>(&mut self, bus: &mut B, op: u16, add: bool) -> R<()> {
        let rx = usize::from(op & 7);
        let ry = usize::from((op >> 9) & 7);
        if op & 8 == 0 {
            self.np(bus)?;
            self.idle(2);
            let (s, d) = (self.d[rx] & 0xff, self.d[ry] & 0xff);
            let r = if add {
                self.abcd(s, d)
            } else {
                self.sbcd(s, d)
            };
            self.set_d(ry, r, Sz::B);
            return Ok(());
        }
        self.idle(2);
        let s = self.read_predec(bus, rx, Sz::B)?;
        let d = self.read_predec(bus, ry, Sz::B)?;
        let a = self.ar(ry);
        let r = if add {
            self.abcd(s, d)
        } else {
            self.sbcd(s, d)
        };
        self.np(bus)?;
        self.wr_b(bus, a, r)
    }

    /// d + s + X の 10 進補正。下位の補正（＋6）は下位の桁の和が 9 を超えたとき、上位の補正（＋0x60）と
    /// C は 2 進の和が 0x99 を超えたとき。V は補正で bit 7 が 0 から 1 になったとき。Z は結果が 0 で
    /// なければ 0 にし、0 なら変えない。
    fn abcd(&mut self, s: u32, d: u32) -> u32 {
        let x = self.x();
        let bin = s + d + x;
        let low = if (s & 0x0f) + (d & 0x0f) + x > 9 {
            6
        } else {
            0
        };
        let carry = bin > 0x99;
        let res = (bin + low + if carry { 0x60 } else { 0 }) & 0xff;
        self.bcd_flags(res, carry, !bin & res & 0x80 != 0);
        res
    }

    /// d − s − X の 10 進補正（NBCD は d＝0）。上位の補正（−0x60）は 2 進の差が負のとき、C は補正の後
    /// が負のとき。V は補正で bit 7 が 1 から 0 になったとき。
    fn sbcd(&mut self, s: u32, d: u32) -> u32 {
        let x = self.x() as i32;
        let bin = d as i32 - s as i32 - x;
        let low = if (d & 0x0f) as i32 - (s & 0x0f) as i32 - x < 0 {
            6
        } else {
            0
        };
        let r = bin - low - if bin < 0 { 0x60 } else { 0 };
        let res = r as u32 & 0xff;
        self.bcd_flags(res, r < 0, bin as u32 & !res & 0x80 != 0);
        res
    }

    fn bcd_flags(&mut self, res: u32, carry: bool, v: bool) {
        let mut f = self.sr & (0xff00 | FLAG_Z);
        if res != 0 {
            f &= !FLAG_Z;
        }
        if carry {
            f |= FLAG_C | FLAG_X;
        }
        if v {
            f |= FLAG_V;
        }
        if res & 0x80 != 0 {
            f |= FLAG_N;
        }
        self.sr = f;
    }

    fn op_exg<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let rx = usize::from((op >> 9) & 7);
        let ry = usize::from(op & 7);
        let (x, y) = match (op >> 3) & 0x1f {
            0x08 => (rx, ry),
            0x09 => (rx + 8, ry + 8),
            0x11 => (rx, ry + 8),
            _ => return self.illegal(bus, vector::ILLEGAL),
        };
        self.np(bus)?;
        self.idle(2);
        let (vx, vy) = (self.reg(x), self.reg(y));
        self.set_reg(x, vy);
        self.set_reg(y, vx);
        Ok(())
    }

    /// MULU・MULS（38＋2n クロック。M68 表7-6 の注）。
    fn op_mul<B: Bus>(&mut self, bus: &mut B, op: u16, signed: bool) -> R<()> {
        if !ea_ok(op, EA_DATA) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, Sz::W)?;
        let s = self.read_ea(bus, ea, Sz::W)?;
        self.np(bus)?;
        let dn = usize::from((op >> 9) & 7);
        let (r, n) = if signed {
            let r = (s as u16 as i16 as i32).wrapping_mul(self.d[dn] as u16 as i16 as i32) as u32;
            let x = s << 1;
            (r, ((x ^ (x >> 1)) & 0xffff).count_ones())
        } else {
            (s * (self.d[dn] & 0xffff), s.count_ones())
        };
        self.idle(34 + 2 * n);
        self.d[dn] = r;
        self.set_nz(r, Sz::L);
        Ok(())
    }

    /// DIVU・DIVS。時間は商のビットで変わる（`divu_clocks`・`divs_clocks`）。
    fn op_div<B: Bus>(&mut self, bus: &mut B, op: u16, signed: bool) -> R<()> {
        if !ea_ok(op, EA_DATA) {
            return self.illegal(bus, vector::ILLEGAL);
        }
        let ea = self.ea(bus, (op >> 3) & 7, op & 7, Sz::W)?;
        let s = self.read_ea(bus, ea, Sz::W)? as u16;
        let dn = usize::from((op >> 9) & 7);
        let dividend = self.d[dn];
        if s == 0 {
            // 0 除算（38 クロック＋実効番地。M68 表7-16 は 42(5/4) と数える。§03）。
            self.sr &= !(FLAG_N | FLAG_Z | FLAG_V | FLAG_C);
            self.idle(8);
            let pc = self.pc.wrapping_add(2);
            return self.exception(bus, vector::ZERO_DIVIDE, pc);
        }
        let (clocks, result) = if signed {
            let q = i64::from(dividend as i32) / i64::from(s as i16);
            let r = i64::from(dividend as i32) % i64::from(s as i16);
            let ok = (-0x8000..=0x7fff).contains(&q);
            (
                divs_clocks(dividend as i32, s as i16),
                ok.then_some((r as u32) << 16 | (q as u32 & 0xffff)),
            )
        } else {
            let q = dividend / u32::from(s);
            let r = dividend % u32::from(s);
            (
                divu_clocks(dividend, s),
                (q <= 0xffff).then_some(r << 16 | q),
            )
        };
        self.idle(clocks - 4);
        match result {
            Some(v) => {
                self.d[dn] = v;
                self.set_nz(v, Sz::W);
            }
            None => {
                self.sr = (self.sr & !FLAG_C) | FLAG_V;
            }
        }
        self.np(bus)
    }

    // ---- line E：シフト・ローテート

    fn linee<B: Bus>(&mut self, bus: &mut B, op: u16) -> R<()> {
        let left = op & 0x100 != 0;
        if (op >> 6) & 3 == 3 {
            // メモリ：ワードを 1 bit。
            if op & 0x0800 != 0 || !ea_ok(op, EA_MEM_ALT) {
                return self.illegal(bus, vector::ILLEGAL);
            }
            let kind = (op >> 9) & 3;
            return self.modify(bus, op, Sz::W, 0, |cpu, d| {
                cpu.shift(kind, left, d, 1, Sz::W)
            });
        }
        let sz = Sz::from_bits(op >> 6);
        let kind = (op >> 3) & 3;
        let count = if op & 0x20 != 0 {
            self.d[usize::from((op >> 9) & 7)] & 63
        } else {
            match (op >> 9) & 7 {
                0 => 8,
                n => u32::from(n),
            }
        };
        let r = usize::from(op & 7);
        self.np(bus)?;
        self.idle(if sz == Sz::L { 4 } else { 2 } + 2 * count);
        let v = self.shift(kind, left, self.d[r], count, sz);
        self.set_d(r, v, sz);
        Ok(())
    }

    /// AS（0）、LS（1）、ROX（2）、RO（3）。
    fn shift(&mut self, kind: u16, left: bool, v: u32, count: u32, sz: Sz) -> u32 {
        let msb = sz.msb();
        let mask = sz.mask();
        let mut v = v & mask;
        let mut c = false;
        let mut overflow = false;
        let mut x = self.sr & FLAG_X != 0;
        for _ in 0..count {
            if left {
                c = v & msb != 0;
                let shifted = (v << 1) & mask;
                v = match kind {
                    0 => {
                        if (shifted ^ v) & msb != 0 {
                            overflow = true;
                        }
                        shifted
                    }
                    1 => shifted,
                    2 => shifted | u32::from(x),
                    _ => shifted | u32::from(c),
                };
            } else {
                c = v & 1 != 0;
                let shifted = v >> 1;
                v = match kind {
                    0 => shifted | (v & msb),
                    1 => shifted,
                    2 => shifted | if x { msb } else { 0 },
                    _ => shifted | if c { msb } else { 0 },
                };
            }
            if kind != 3 {
                x = c;
            }
        }
        let mut f = self.sr & 0xff00;
        if kind == 3 || count == 0 {
            f |= self.sr & FLAG_X;
        } else if x {
            f |= FLAG_X;
        }
        if kind == 2 && count == 0 {
            c = x;
        }
        if c {
            f |= FLAG_C;
        }
        if overflow {
            f |= FLAG_V;
        }
        if v == 0 {
            f |= FLAG_Z;
        }
        if v & msb != 0 {
            f |= FLAG_N;
        }
        self.sr = f;
        v
    }
}
