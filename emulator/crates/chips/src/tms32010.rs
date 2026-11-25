//! TI TMS32010 DSP。
//!
//! 事実と出典は `docs/parts/TMS32010.md`（以下「台帳」）。命令の意味は TI の資料（台帳の T1・T3）に従い、
//! MAME（台帳の M1）は照合にだけ使った。コードは自前で書いた。
//!
//! モデルの範囲：
//! - `tick` の 1 回を 1 マシンサイクル（CLKOUT の 1 周期 ＝ CLKIN の 4 周期）とする。CLKIN の分周は board 層の仕事。
//! - サイクル数は TI の表（台帳 §3.7）どおり。条件分岐は成否によらず 2（台帳 追補 A）。
//! - IN・OUT は 1 サイクル目にポートを読み書きする。TBLR・TBLW は 2 サイクル目にプログラムメモリを読み書きする（台帳 §3.3、§3.4）。
//! - BIO は BIOZ の 1 サイクル目に読む [P]（台帳 追補 A、DR-032）。
//! - 割込みの受付は 1 ダミーサイクル（台帳 追補 A）。MPY・MPYK・EINT の次の命令が終わるまで受け付けない。
//! - プログラムメモリと I/O は `Bus` の向こう側にある。OUT と TBLW が 0〜7 番地で重なる現象は board 層で再現する（台帳 §2.2）。
//! - 資料で決まらない動作は、`docs/impl/emulator.md` §03 にどう扱ったかを書く。

use serde::{Deserialize, Serialize};

/// 内部のデータ RAM の語数（0〜143 番地）。
pub const DATA_RAM_WORDS: usize = 144;

/// プログラムメモリ、I/O ポート、BIO ピン。board 層（またはテスト）が実装する。
pub trait Bus {
    /// 命令のフェッチと TBLR（MEN）。番地は 12 bit。
    fn read_program(&mut self, addr: u16) -> u16;
    /// TBLW（WE）。番地は 12 bit。
    fn write_program(&mut self, addr: u16, data: u16);
    /// IN（DEN）。ポートは 0〜7。
    fn port_in(&mut self, port: u8) -> u16;
    /// OUT（WE）。ポートは 0〜7。
    fn port_out(&mut self, port: u8, data: u16);
    /// BIO ピンが Low か。BIOZ の 1 サイクル目に読む。
    fn bio_low(&mut self) -> bool;
}

/// 1 サイクルで何をしたか（trace 用）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cycle {
    /// RS が Low で止まっている。
    Reset,
    /// RS を解除した直後の 1 サイクル（台帳 §3.5）。
    Restart,
    /// 命令の 1 サイクル目。`pc` はその命令の番地。
    Exec { pc: u16, opcode: u16 },
    /// 複数サイクルの命令の 2 サイクル目以降。
    Busy,
    /// 割込みの受付のダミーサイクル。
    Interrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Pending {
    Tblr { addr: u16, dma: u8 },
    Tblw { addr: u16, dma: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tms32010 {
    acc: u32,
    p: u32,
    t: u16,
    ar: [u16; 2],
    arp: u8,
    dp: u8,
    pc: u16,
    /// stack[0] が最上段。
    stack: [u16; 4],
    ov: bool,
    ovm: bool,
    intm: bool,
    intf: bool,
    ram: Vec<u16>,
    // ピン
    /// RS が Low（リセット中）か。
    rs_low: bool,
    /// INT ピンのレベル（true で High）。
    int_high: bool,
    // 実行の状態
    restart: bool,
    busy: u8,
    pending: Option<Pending>,
    /// 次の命令の前で割込みを受け付けない（MPY・MPYK・EINT の直後）。
    int_block: bool,
    /// 未定義の命令を実行した回数。
    illegal: u64,
}

impl Default for Tms32010 {
    fn default() -> Self {
        Self::new()
    }
}

impl Tms32010 {
    /// 電源投入直後。RS は Low（台帳 §2 の本機での接続）。レジスタとデータ RAM は 0 とする。
    /// 実チップの電源投入時の値は未確認 [V]。ソフトで初期化する前提（台帳 §4.4）。
    pub fn new() -> Self {
        Self {
            acc: 0,
            p: 0,
            t: 0,
            ar: [0; 2],
            arp: 0,
            dp: 0,
            pc: 0,
            stack: [0; 4],
            ov: false,
            ovm: false,
            intm: true,
            intf: false,
            ram: vec![0; DATA_RAM_WORDS],
            rs_low: true,
            int_high: true,
            restart: false,
            busy: 0,
            pending: None,
            int_block: false,
            illegal: 0,
        }
    }

    // ---- ピン ----

    /// RS ピン。`low` が true でリセット。
    /// リセットで決まるのは PC＝0、INTM＝1、INTF＝0 だけで、OVM は変わらない（台帳 §3.5）。
    /// ほかのレジスタとデータ RAM は変えない [V]。5 サイクル以上という最小幅は模擬しない [V]。
    pub fn set_rs(&mut self, low: bool) {
        if low {
            self.pc = 0;
            self.intm = true;
            self.intf = false;
            self.busy = 0;
            self.pending = None;
            self.int_block = false;
            self.restart = false;
        } else if self.rs_low {
            self.restart = true;
        }
        self.rs_low = low;
    }

    /// INT ピン。立下りで INTF をラッチする。Low の間も要求になる（台帳 §3.6）。
    pub fn set_int(&mut self, high: bool) {
        if self.int_high && !high {
            self.intf = true;
        }
        self.int_high = high;
    }

    // ---- 状態の読み出し ----

    pub fn acc(&self) -> u32 {
        self.acc
    }
    pub fn p(&self) -> u32 {
        self.p
    }
    pub fn t(&self) -> u16 {
        self.t
    }
    pub fn ar(&self, i: usize) -> u16 {
        self.ar[i]
    }
    pub fn arp(&self) -> u8 {
        self.arp
    }
    pub fn dp(&self) -> u8 {
        self.dp
    }
    pub fn pc(&self) -> u16 {
        self.pc
    }
    pub fn stack(&self) -> [u16; 4] {
        self.stack
    }
    pub fn ov(&self) -> bool {
        self.ov
    }
    pub fn ovm(&self) -> bool {
        self.ovm
    }
    pub fn intm(&self) -> bool {
        self.intm
    }
    pub fn illegal_count(&self) -> u64 {
        self.illegal
    }
    /// 複数サイクルの命令の途中か（次の `tick` が `Cycle::Busy` になるか）。
    pub fn mid_instruction(&self) -> bool {
        self.busy > 0
    }

    /// SST が書く形の状態語。bit15＝OV、14＝OVM、13＝INTM、8＝ARP、0＝DP、ほかは 1（台帳 §4.1）。
    pub fn status(&self) -> u16 {
        0x1EFE
            | (u16::from(self.ov) << 15)
            | (u16::from(self.ovm) << 14)
            | (u16::from(self.intm) << 13)
            | (u16::from(self.arp) << 8)
            | u16::from(self.dp)
    }

    /// データ RAM の読み出し。144 番地以上は 0 [V]。
    pub fn data(&self, addr: u8) -> u16 {
        self.ram.get(usize::from(addr)).copied().unwrap_or(0)
    }

    /// データ RAM への直接の書込み。実機にはない、エミュレータだけの内部介入（試験と Bend の道具）。
    pub fn set_data(&mut self, addr: u8, value: u16) {
        self.write(addr, value);
    }

    // ---- 時間 ----

    /// 1 マシンサイクル進める。
    pub fn tick(&mut self, bus: &mut impl Bus) -> Cycle {
        if self.rs_low {
            return Cycle::Reset;
        }
        if self.restart {
            self.restart = false;
            return Cycle::Restart;
        }
        if self.busy > 0 {
            if self.busy == 2
                && let Some(p) = self.pending.take()
            {
                self.finish_table(p, bus);
            }
            self.busy -= 1;
            return Cycle::Busy;
        }
        if !self.int_block && !self.intm && (self.intf || !self.int_high) {
            self.push(self.pc);
            self.pc = 2;
            self.intm = true;
            self.intf = false;
            return Cycle::Interrupt;
        }
        self.int_block = false;
        let pc = self.pc;
        let opcode = bus.read_program(pc);
        self.pc = (pc + 1) & 0x0FFF;
        let cycles = self.execute(opcode, bus);
        self.busy = cycles - 1;
        Cycle::Exec { pc, opcode }
    }

    // ---- 内部 ----

    fn write(&mut self, addr: u8, value: u16) {
        if let Some(w) = self.ram.get_mut(usize::from(addr)) {
            *w = value;
        }
    }

    /// DMOV・LTD の写し先。255 番地から 0 番地へは回り込まない（144 番地以上への書込みは捨てる）。
    fn write_next(&mut self, addr: u8, value: u16) {
        if let Some(next) = addr.checked_add(1) {
            self.write(next, value);
        }
    }

    fn push(&mut self, v: u16) {
        self.stack = [v & 0x0FFF, self.stack[0], self.stack[1], self.stack[2]];
    }

    /// 最上段を取り出す。最下段はそのまま残る（MAME と同じ。TI の資料に記述なし [V]）。
    fn pop(&mut self) -> u16 {
        let v = self.stack[0];
        self.stack = [self.stack[1], self.stack[2], self.stack[3], self.stack[3]];
        v
    }

    /// 間接アドレスの後処理：AR の下位 9 bit の増減と、次の ARP（台帳 §4.2）。
    fn update_ar(&mut self, op: u16) {
        let i = usize::from(self.arp);
        if op & 0x0030 != 0 {
            let mut v = self.ar[i];
            if op & 0x0020 != 0 {
                v = v.wrapping_add(1);
            }
            if op & 0x0010 != 0 {
                v = v.wrapping_sub(1);
            }
            self.ar[i] = (self.ar[i] & 0xFE00) | (v & 0x01FF);
        }
        if op & 0x0008 == 0 {
            self.arp = (op & 1) as u8;
        }
    }

    /// オペランドの番地を求め、間接なら AR と ARP を更新する。
    fn operand(&mut self, op: u16) -> u8 {
        if op & 0x0080 != 0 {
            let a = (self.ar[usize::from(self.arp)] & 0x00FF) as u8;
            self.update_ar(op);
            a
        } else {
            (self.dp << 7) | (op & 0x007F) as u8
        }
    }

    fn load(&mut self, op: u16) -> (u8, u16) {
        let a = self.operand(op);
        (a, self.data(a))
    }

    fn set_acc(&mut self, old: u32, result: u32, overflow: bool) {
        self.acc = if overflow {
            self.ov = true;
            if self.ovm {
                if (old as i32) < 0 {
                    0x8000_0000
                } else {
                    0x7FFF_FFFF
                }
            } else {
                result
            }
        } else {
            result
        };
    }

    fn add_acc(&mut self, v: u32) {
        let old = self.acc;
        let r = old.wrapping_add(v);
        self.set_acc(old, r, (!(old ^ v) & (old ^ r)) & 0x8000_0000 != 0);
    }

    fn sub_acc(&mut self, v: u32) {
        let old = self.acc;
        let r = old.wrapping_sub(v);
        self.set_acc(old, r, ((old ^ v) & (old ^ r)) & 0x8000_0000 != 0);
    }

    fn multiply(&mut self, operand: u16) {
        // 0x8000 × 0x8000 は 0xC0000000 になる（台帳 §4.3）。
        self.p = if self.t == 0x8000 && operand == 0x8000 {
            0xC000_0000
        } else {
            (i32::from(self.t as i16) * i32::from(operand as i16)) as u32
        };
    }

    fn finish_table(&mut self, p: Pending, bus: &mut impl Bus) {
        match p {
            Pending::Tblr { addr, dma } => {
                let v = bus.read_program(addr);
                self.write(dma, v);
            }
            Pending::Tblw { addr, dma } => {
                let v = self.data(dma);
                bus.write_program(addr, v);
            }
        }
        self.pc = self.pop();
    }

    fn branch(&mut self, cond: bool, bus: &mut impl Bus) -> u8 {
        let target = bus.read_program(self.pc) & 0x0FFF;
        self.pc = (self.pc + 1) & 0x0FFF;
        if cond {
            self.pc = target;
        }
        2
    }

    /// 命令を実行し、サイクル数を返す。`self.pc` は命令の次の語を指している。
    fn execute(&mut self, op: u16, bus: &mut impl Bus) -> u8 {
        let hi = (op >> 8) as u8;
        match hi {
            // ADD・SUB・LAC：0〜15 bit の左シフト、符号拡張
            0x00..=0x0F => {
                let (_, d) = self.load(op);
                self.add_acc(((d as i16 as i32) << (hi & 0x0F)) as u32);
                1
            }
            0x10..=0x1F => {
                let (_, d) = self.load(op);
                self.sub_acc(((d as i16 as i32) << (hi & 0x0F)) as u32);
                1
            }
            0x20..=0x2F => {
                let (_, d) = self.load(op);
                self.acc = ((d as i16 as i32) << (hi & 0x0F)) as u32;
                1
            }
            // SAR：増減する前の AR の値を書く [V]（MAME は増減の後の値）
            0x30 | 0x31 => {
                let v = self.ar[usize::from(hi & 1)];
                let a = self.operand(op);
                self.write(a, v);
                1
            }
            // LAR：間接で同じ AR を増減したときは、読んだ値が勝つ
            0x38 | 0x39 => {
                let (_, d) = self.load(op);
                self.ar[usize::from(hi & 1)] = d;
                1
            }
            0x40..=0x47 => {
                let a = self.operand(op);
                let v = bus.port_in(hi & 7);
                self.write(a, v);
                2
            }
            0x48..=0x4F => {
                let (_, d) = self.load(op);
                bus.port_out(hi & 7, d);
                2
            }
            0x50 => {
                let a = self.operand(op);
                self.write(a, self.acc as u16);
                1
            }
            // SACH：シフトは 0・1・4 が規定。ほかの値も同じ式で扱う [V]
            0x58..=0x5F => {
                let v = ((self.acc << (hi & 7)) >> 16) as u16;
                let a = self.operand(op);
                self.write(a, v);
                1
            }
            0x60 => {
                let (_, d) = self.load(op);
                self.add_acc(u32::from(d) << 16);
                1
            }
            0x61 => {
                let (_, d) = self.load(op);
                self.add_acc(u32::from(d));
                1
            }
            0x62 => {
                let (_, d) = self.load(op);
                self.sub_acc(u32::from(d) << 16);
                1
            }
            0x63 => {
                let (_, d) = self.load(op);
                self.sub_acc(u32::from(d));
                1
            }
            // SUBC：OV は変えない [V]
            0x64 => {
                let (_, d) = self.load(op);
                let r = self.acc.wrapping_sub(u32::from(d) << 15);
                self.acc = if (r as i32) >= 0 {
                    (r << 1).wrapping_add(1)
                } else {
                    self.acc << 1
                };
                1
            }
            0x65 => {
                let (_, d) = self.load(op);
                self.acc = u32::from(d) << 16;
                1
            }
            0x66 => {
                let (_, d) = self.load(op);
                self.acc = u32::from(d);
                1
            }
            0x67 => {
                let dma = self.operand(op);
                self.push(self.pc);
                self.pending = Some(Pending::Tblr {
                    addr: (self.acc & 0x0FFF) as u16,
                    dma,
                });
                3
            }
            // MAR・LARP：直接アドレスでは何もしない
            0x68 => {
                if op & 0x0080 != 0 {
                    self.update_ar(op);
                }
                1
            }
            0x69 => {
                let (a, d) = self.load(op);
                self.write_next(a, d);
                1
            }
            0x6A => {
                let (_, d) = self.load(op);
                self.t = d;
                1
            }
            0x6B => {
                let (a, d) = self.load(op);
                self.t = d;
                self.add_acc(self.p);
                self.write_next(a, d);
                1
            }
            0x6C => {
                let (_, d) = self.load(op);
                self.t = d;
                self.add_acc(self.p);
                1
            }
            0x6D => {
                let (_, d) = self.load(op);
                self.multiply(d);
                self.int_block = true;
                1
            }
            0x6E => {
                self.dp = (op & 1) as u8;
                1
            }
            0x6F => {
                let (_, d) = self.load(op);
                self.dp = (d & 1) as u8;
                1
            }
            0x70 | 0x71 => {
                self.ar[usize::from(hi & 1)] = op & 0x00FF;
                1
            }
            0x78 => {
                let (_, d) = self.load(op);
                self.acc ^= u32::from(d);
                1
            }
            0x79 => {
                let (_, d) = self.load(op);
                self.acc &= u32::from(d);
                1
            }
            0x7A => {
                let (_, d) = self.load(op);
                self.acc |= u32::from(d);
                1
            }
            // LST：INTM は変えない。間接で指定した次の ARP は、読んだ ARP に上書きされる
            0x7B => {
                let (_, d) = self.load(op);
                self.ov = d & 0x8000 != 0;
                self.ovm = d & 0x4000 != 0;
                self.arp = ((d >> 8) & 1) as u8;
                self.dp = (d & 1) as u8;
                1
            }
            // SST：直接アドレスでは DP によらずページ 1。増減する前の状態を書く [V]
            0x7C => {
                let v = self.status();
                let a = if op & 0x0080 != 0 {
                    self.operand(op)
                } else {
                    0x80 | (op & 0x007F) as u8
                };
                self.write(a, v);
                1
            }
            0x7D => {
                let dma = self.operand(op);
                self.push(self.pc);
                self.pending = Some(Pending::Tblw {
                    addr: (self.acc & 0x0FFF) as u16,
                    dma,
                });
                3
            }
            0x7E => {
                self.acc = u32::from(op & 0x00FF);
                1
            }
            0x7F => self.execute_7f(op),
            // MPYK：13 bit の符号付き定数
            0x80..=0x9F => {
                let k = (((op & 0x1FFF) << 3) as i16) >> 3;
                self.multiply(k as u16);
                self.int_block = true;
                1
            }
            0xF4 => {
                let i = usize::from(self.arp);
                let cond = self.ar[i] & 0x01FF != 0;
                self.ar[i] = (self.ar[i] & 0xFE00) | (self.ar[i].wrapping_sub(1) & 0x01FF);
                self.branch(cond, bus)
            }
            0xF5 => {
                let cond = self.ov;
                if cond {
                    self.ov = false;
                }
                self.branch(cond, bus)
            }
            0xF6 => {
                let cond = bus.bio_low();
                self.branch(cond, bus)
            }
            0xF8 => {
                let target = bus.read_program(self.pc) & 0x0FFF;
                self.push((self.pc + 1) & 0x0FFF);
                self.pc = target;
                2
            }
            0xF9 => self.branch(true, bus),
            0xFA => self.branch((self.acc as i32) < 0, bus),
            0xFB => self.branch((self.acc as i32) <= 0, bus),
            0xFC => self.branch((self.acc as i32) > 0, bus),
            0xFD => self.branch((self.acc as i32) >= 0, bus),
            0xFE => self.branch(self.acc != 0, bus),
            0xFF => self.branch(self.acc == 0, bus),
            _ => self.illegal(),
        }
    }

    fn execute_7f(&mut self, op: u16) -> u8 {
        match op & 0x00FF {
            0x80 => 1,
            0x81 => {
                self.intm = true;
                1
            }
            0x82 => {
                self.intm = false;
                self.int_block = true;
                1
            }
            // ABS：0x80000000 は OVM＝1 なら 0x7FFFFFFF。OV は変えない [V]
            0x88 => {
                if (self.acc as i32) < 0 {
                    self.acc = self.acc.wrapping_neg();
                    if self.ovm && self.acc == 0x8000_0000 {
                        self.acc = 0x7FFF_FFFF;
                    }
                }
                1
            }
            0x89 => {
                self.acc = 0;
                1
            }
            0x8A => {
                self.ovm = false;
                1
            }
            0x8B => {
                self.ovm = true;
                1
            }
            0x8C => {
                self.push(self.pc);
                self.pc = (self.acc & 0x0FFF) as u16;
                2
            }
            0x8D => {
                self.pc = self.pop();
                2
            }
            0x8E => {
                self.acc = self.p;
                1
            }
            0x8F => {
                self.add_acc(self.p);
                1
            }
            0x90 => {
                self.sub_acc(self.p);
                1
            }
            0x9C => {
                self.push(self.acc as u16);
                2
            }
            0x9D => {
                self.acc = u32::from(self.pop());
                2
            }
            _ => self.illegal(),
        }
    }

    /// 未定義の命令。NOP（1 サイクル）として扱い、回数を数える [V]。
    fn illegal(&mut self) -> u8 {
        self.illegal += 1;
        1
    }
}
