//! Data Fabric の門の模型（DR-085）。fabric.md §06 の順序と状態の PAL の式から生成した関数
//! （`fabric_pal`）を、PAL のピンの値のレジスタでつなぎ、1 スロットずつ進める。
//!
//! 外側は動作の模型 `fabric::Fabric` と同じ（`Ports`、CPU の窓、`SlotRecord`）。2 つを並べて動かし、
//! スロットごとに突き合わせる（試験 `fabric_lockstep`）。製品の経路は動作の模型のまま（DR-085）。
//! この模型は試験の側にだけ置き、`fc85-board` の lib には入れない。lib に入れるだけで、コードの
//! 配置が変わって本番が遅くなったため（DR-085）。
//!
//! 門の模型にした部分：CST、ELG、SEQ、ELS、EVT、ADC、CHS×2、STS×2、AFC0–1、RDY、変換部（DR-084）、
//! A ファイル（74S189）と加算器（74LS283）、PS・PD、IL。
//! 動作のまま書いた部分（PAL の式はまだない。OI-050 (1) の残り）：アクセスの制御（NX・OP の
//! レジスタ、2 スロットと延びたアクセス、時間切れのカウンタ、port 1 の DONE の比較）、CPU の窓の
//! 番地の解読と要求の FF、トリガの発生源の選択（74LS151）、MODULES。
//!
//! クロック（fabric.md §03）：SCLK（スロットの頭）で SEQ と NX・OP、MCLK（中ほど）で RDY、LCLK
//! （4 分の 3）で CHS・STS・ELS・U36 と A ファイルの書込み。CPU の書込みのうち START・STOP と
//! STATUS の W1C は要求の FF（74LS74）に入り、次の LCLK で効いて消える。CPU の 2 回のアクセスの間には
//! 1 スロット以上ある（68000 のバスサイクルは 4 クロック＝2 スロット）とする。同じスロットの間の
//! 書込みと読出しでは、動作の模型（書込みがすぐ効く）と STATUS の値が違う。
//!
//! PAL のレジスタは、電源投入時に出力のピンが 1 になる（負の論理の信号はすべて 0）とする [V]。

// 試験ごとに使う入口が違うので、使わない入口を咎めない。
#![allow(dead_code)]

use fc85_board::fabric::{
    Access, Aop, CHANNELS, ChannelRegs, Decision, Dir, HostReply, Phase, PortReply, Ports,
    SlotRecord, TIMEOUT_SLOTS, atr, fd_value, input_is_word, is_long, output_is_word,
};
use fc85_board::fabric_pal::{
    ATR_ADDR, ATR_FLAG, ATR_OKD, ATR_OKN, ATR_SONLY, ATR_TWO, ChsOut, CstOut, StsOut, adc, afc0,
    afc1, chs, cst, elg, els, evt, rdy, seq, sts,
};

/// CHS（PAL20R6A）のレジスタ。ピンの値（負の論理）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChsRegs {
    p2n: bool,
    p1n: bool,
    p0n: bool,
    okn: bool,
    donen: bool,
    errn: bool,
}

/// STS（PAL20R6A）のレジスタ。ピンの値（負の論理）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StsRegs {
    syn2n: bool,
    risen: bool,
    armn: bool,
    pendn: bool,
    missn: bool,
    staln: bool,
}

/// NX のレジスタ：次のスロットのアクセス（チャネル、ポート、向き）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Next {
    ch: u8,
    port: u8,
    dir: Dir,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Single,
    Two,
    Long,
}

/// OP のレジスタ：このスロットのアクセス。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Op {
    acc: Access,
    kind: Kind,
    elapsed: u32,
    flag: bool,
    result: Option<u16>,
    end_mark: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HostReq {
    ch: u8,
    reg: u8,
    write: Option<u16>,
    result: Option<HostReply>,
}

/// Data Fabric の門の模型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateFabric {
    slot: u64,
    regs: [ChannelRegs; CHANNELS],
    /// A ファイル（74S189×4、16 語）。語は ch×8＋（0 SRC の設定値、1 作業値、2 DST の設定値、3 作業値、
    /// 4 COUNT の設定値、5 作業値）。
    afile: [u16; 16],
    ps: u16,
    pd: u16,
    il: u16,
    // RDY（MCLK）
    srdy: bool,
    drdy: bool,
    // SEQ（SCLK）
    dbchn: bool,
    achn: bool,
    aop2n: bool,
    aop1n: bool,
    aop0n: bool,
    accrn: bool,
    accwn: bool,
    // NX の STV・STL（SCLK）
    stvn: bool,
    stln: bool,
    // ELS（LCLK）
    infn: bool,
    q1n: bool,
    q0n: bool,
    epcn: bool,
    elastn: bool,
    tln: bool,
    tchn: bool,
    tlastn: bool,
    chs: [ChsRegs; CHANNELS],
    sts: [StsRegs; CHANNELS],
    /// U36：トリガのレベルの同期化の 1 段目（74LS74、LCLK）。
    s1q: [bool; CHANNELS],
    // CPU の要求の FF（74LS74）。次の LCLK で効いて消える。
    strq: [bool; CHANNELS],
    stpq: [bool; CHANNELS],
    wdq: [bool; CHANNELS],
    weq: [bool; CHANNELS],
    wmq: [bool; CHANNELS],
    hreq: Option<HostReq>,
    nx: Option<Next>,
    op: Option<Op>,
    /// 1 つ前のスロットのアクセスが、満杯・空の信号を持つポートだった（OP の FLAG）。
    prev_flag: bool,
    trig_in: u8,
}

impl Default for GateFabric {
    fn default() -> Self {
        Self::new()
    }
}

fn bus(v: [bool; CHANNELS]) -> u32 {
    v[0] as u32 | (v[1] as u32) << 1
}

fn bit(v: u32, c: usize) -> bool {
    v >> c & 1 != 0
}

/// AFC0 の番地のピン（負の論理）→ A ファイルの語。
fn word(aa3n: bool, aa2n: bool, aa1n: bool, aa0n: bool) -> usize {
    (!aa3n as usize) << 3 | (!aa2n as usize) << 2 | (!aa1n as usize) << 1 | !aa0n as usize
}

impl GateFabric {
    /// 電源投入時。A ファイルの中身は 0 にする（動作の模型と同じ）[V]。
    pub fn new() -> Self {
        let k = ChsRegs {
            p2n: true,
            p1n: true,
            p0n: true,
            okn: true,
            donen: true,
            errn: true,
        };
        let t = StsRegs {
            syn2n: true,
            risen: true,
            armn: true,
            pendn: true,
            missn: true,
            staln: true,
        };
        Self {
            slot: 0,
            regs: Default::default(),
            afile: [0; 16],
            ps: 0,
            pd: 0,
            il: 0,
            srdy: true,
            drdy: true,
            dbchn: true,
            achn: true,
            aop2n: true,
            aop1n: true,
            aop0n: true,
            accrn: true,
            accwn: true,
            stvn: true,
            stln: true,
            infn: true,
            q1n: true,
            q0n: true,
            epcn: true,
            elastn: true,
            tln: true,
            tchn: true,
            tlastn: true,
            chs: [k; CHANNELS],
            sts: [t; CHANNELS],
            s1q: [false; CHANNELS],
            strq: [false; CHANNELS],
            stpq: [false; CHANNELS],
            wdq: [false; CHANNELS],
            weq: [false; CHANNELS],
            wmq: [false; CHANNELS],
            hreq: None,
            nx: None,
            op: None,
            prev_flag: false,
            trig_in: 0,
        }
    }

    pub fn slot(&self) -> u64 {
        self.slot
    }

    fn imm(&self, c: usize) -> bool {
        self.regs[c].trigger & 7 == 0
    }

    /// STS の組合せの出力（ARMX、WAIT）。IMM と、STS・CHS のレジスタの今の値だけで決まるので、
    /// ほかの入力は何でもよい。
    fn sts_now(&self, c: usize) -> StsOut {
        let (k, t) = (&self.chs[c], &self.sts[c]);
        sts(
            false,
            true,
            true,
            false,
            true,
            false,
            self.imm(c),
            true,
            true,
            k.p1n,
            k.p0n,
            t.syn2n,
            t.risen,
            t.armn,
            t.pendn,
            t.missn,
            t.staln,
        )
    }

    /// STATUS（bit 0 BUSY、1 DONE、2 STALL、3 ERROR、4 MISS、5 WAIT）。BUSY は段階が 000 でない。
    pub fn status(&self, c: usize) -> u16 {
        let (k, t) = (&self.chs[c], &self.sts[c]);
        let busy = !k.p2n || !k.p1n;
        busy as u16
            | (!k.donen as u16) << 1
            | (!t.staln as u16) << 2
            | (!k.errn as u16) << 3
            | (!t.missn as u16) << 4
            | (!self.sts_now(c).waitn as u16) << 5
    }

    /// 段階（CHS の P2–P0）。
    pub fn phase(&self, c: usize) -> Phase {
        let k = &self.chs[c];
        match (!k.p2n, !k.p1n, !k.p0n) {
            (_, true, true) => Phase::Run,
            (true, false, false) => Phase::Reload(0),
            (true, false, true) => Phase::Reload(1),
            (true, true, false) => Phase::Reload(2),
            _ => Phase::Idle,
        }
    }

    pub fn regs(&self, c: usize) -> &ChannelRegs {
        &self.regs[c]
    }

    /// 割込みの要求（IRQ_EN∧(DONE∨ERROR) の 2 チャネルの OR）。
    pub fn irq(&self) -> bool {
        (0..CHANNELS)
            .any(|c| self.regs[c].trigger & 0x20 != 0 && (!self.chs[c].donen || !self.chs[c].errn))
    }

    pub fn set_trigger_inputs(&mut self, levels: u8) {
        self.trig_in = levels;
    }

    // ------------------------------------------------------------------ CPU の側

    pub fn host_read<P: Ports + ?Sized>(&mut self, offset: u16, ports: &P) -> HostReply {
        if offset & 1 != 0 {
            return HostReply::BusError;
        }
        match offset {
            0x000..=0x07F => {
                let c = (offset >> 6) as usize;
                let r = (offset & 0x3F) as u8;
                match r {
                    0x06 | 0x0A | 0x0C => self.request(c, r, None),
                    0x1E => HostReply::Data(self.status(c)),
                    0x00..=0x1C => HostReply::Data(0),
                    _ => HostReply::BusError,
                }
            }
            0x100 | 0x102 => HostReply::Data(0),
            0x104 => {
                let mut m = (ports.doc_present() as u16) << 2;
                for p in [8u8, 10, 11, 12, 13, 14, 15] {
                    m |= (ports.module_present(p) as u16) << p;
                }
                HostReply::Data(m)
            }
            _ => HostReply::BusError,
        }
    }

    pub fn host_write<P: Ports + ?Sized>(
        &mut self,
        offset: u16,
        value: u16,
        ports: &mut P,
    ) -> HostReply {
        if offset & 1 != 0 {
            return HostReply::BusError;
        }
        match offset {
            0x000..=0x07F => {
                let c = (offset >> 6) as usize;
                let r = (offset & 0x3F) as u8;
                let regs = &mut self.regs[c];
                match r {
                    0x00 => regs.src = value as u8 & 15,
                    0x02 => regs.dst = value as u8 & 15,
                    0x04 | 0x08 => {}
                    0x06 | 0x0A | 0x0C => return self.request(c, r, Some(value)),
                    0x0E => regs.src_step = value,
                    0x10 => regs.dst_step = value,
                    0x12 => regs.format = value as u8 & 7,
                    0x14 => regs.mask = value,
                    0x16 => regs.xor = value,
                    0x18 => regs.shift = value as u8 & 31,
                    0x1A => regs.trigger = value as u8 & 0x3F,
                    // CTRL：STOP が先。START は段階が 000 のときだけ要求の FF に入る。
                    0x1C => {
                        if value & 2 != 0 {
                            self.stpq[c] = true;
                        } else if value & 1 != 0 && self.chs[c].p2n && self.chs[c].p1n {
                            self.strq[c] = true;
                        }
                    }
                    0x1E => {
                        self.wdq[c] |= value & 0x02 != 0;
                        self.weq[c] |= value & 0x08 != 0;
                        self.wmq[c] |= value & 0x10 != 0;
                    }
                    _ => return HostReply::BusError,
                }
                HostReply::Ack
            }
            0x100 => {
                ports.port_reset(value);
                HostReply::Ack
            }
            0x102 => {
                ports.write_audio_n(value & 0x0FFF);
                HostReply::Ack
            }
            0x104 => HostReply::Ack,
            _ => HostReply::BusError,
        }
    }

    pub fn host_poll(&mut self) -> Option<HostReply> {
        let r = self.hreq?.result?;
        self.hreq = None;
        Some(r)
    }

    fn request(&mut self, ch: usize, reg: u8, write: Option<u16>) -> HostReply {
        assert!(
            self.hreq.is_none_or(|r| r.result.is_some()),
            "CPU は前の A ファイルのアクセスの DTACK を待っている"
        );
        self.hreq = Some(HostReq {
            ch: ch as u8,
            reg,
            write,
            result: None,
        });
        HostReply::Wait
    }

    // ------------------------------------------------------------------ スロット

    /// CST の今の出力。
    fn cst(&self) -> CstOut {
        let k = &self.chs;
        let armxn = [self.sts_now(0).armxn, self.sts_now(1).armxn];
        cst(
            bus([k[0].p2n, k[1].p2n]),
            bus([k[0].p1n, k[1].p1n]),
            bus([k[0].p0n, k[1].p0n]),
            bus(self.strq),
            bus(self.stpq),
            self.dbchn,
            bus(armxn),
        )
    }

    /// 判断バスのポート（U32 の選択）：要素があるか、再設定の 2 段目なら書込み先、ほかは読出し元。
    fn bus_port(&self, cst: &CstOut) -> (usize, u8, Dir) {
        let c = !self.dbchn as usize;
        let r = &self.regs[c];
        let usel = !self.infn || (!cst.crldn && !cst.ck0n);
        if usel {
            (c, r.dst, Dir::Write)
        } else {
            (c, r.src, Dir::Read)
        }
    }

    pub fn step_slot<P: Ports + ?Sized>(&mut self, ports: &mut P) -> SlotRecord {
        let doc = ports.doc_present();
        let s = self.slot.is_multiple_of(2);

        // STOP の要求のあるチャネルのアクセスを、OP・NX から捨てる。延びたアクセスは打ち切る。
        if let Some(a) = self.op
            && self.stpq[a.acc.ch as usize]
        {
            if a.kind == Kind::Long && a.result.is_none() {
                ports.abort_access(a.acc.port);
            }
            self.op = None;
        }
        if self.nx.is_some_and(|n| self.stpq[n.ch as usize]) {
            self.nx = None;
        }

        // 1. SCLK。NX は空いた OP へ移る。判断の入力の CD・FB は、SCLK の前の OP・NX から作る
        //    （ここでは移した後の OP から求める。同じ値になる）。
        if self.op.is_none()
            && let Some(n) = self.nx.take()
        {
            self.op = Some(self.begin(n, doc));
        }
        let cd = match &self.op {
            None => true,
            Some(a) => match a.kind {
                Kind::Single => true,
                Kind::Two => a.elapsed == 1,
                Kind::Long => false,
            },
        };
        let fb = self.op.is_some_and(|a| a.flag) || self.prev_flag;
        let k = self.cst();
        let (c, port, dir) = self.bus_port(&k);
        let r = self.regs[c];
        let attr = atr(port, dir, r.format);
        let sok = attr & ATR_SONLY == 0 || !s;
        let ready = rdy(0, 0, 0, 0, 0, false, self.srdy, self.drdy).ready;
        let hreq = self.hreq.is_some_and(|h| h.result.is_none());
        let g = elg(
            self.infn,
            self.q1n,
            self.epcn,
            k.crunn,
            k.cran,
            k.crldn,
            k.carmn,
            ready,
            sok,
            attr & ATR_ADDR != 0,
            fb,
            cd,
            hreq,
        );
        let hold = s && !k.crunn && r.src == 0;
        let q = seq(
            g.ac2n, g.ac1n, g.ac0n, g.grn, g.gwn, g.keepn, k.ck1n, k.ck0n, k.crldn, k.orldn,
            k.orunn, hold, self.dbchn,
        );
        if !g.grn || !g.gwn {
            debug_assert!(self.nx.is_none());
            self.nx = Some(if !g.grn {
                Next {
                    ch: c as u8,
                    port: r.src,
                    dir: Dir::Read,
                }
            } else {
                Next {
                    ch: c as u8,
                    port: r.dst,
                    dir: Dir::Write,
                }
            });
        }
        self.stvn = g.stvn;
        self.stln = g.stln;
        self.dbchn = q.dbchn;
        self.achn = q.achn;
        self.aop2n = q.aop2n;
        self.aop1n = q.aop1n;
        self.aop0n = q.aop0n;
        self.accrn = q.accrn;
        self.accwn = q.accwn;
        let starting = self.op.filter(|a| a.elapsed == 0).map(|a| a.acc);
        ports.run_slot(starting.as_ref());

        // 2. MCLK：判断バスの新しいチャネルの準備を RDY が取り込む。
        let nr = &self.regs[!self.dbchn as usize];
        let lines = ports.ready_lines();
        let o = rdy(
            nr.src as u32,
            nr.dst as u32,
            lines,
            lines,
            lines,
            lines & 1 << 11 != 0,
            self.srdy,
            self.drdy,
        );
        self.srdy = o.srdy;
        self.drdy = o.drdy;

        // 3. アクセス（動作のまま）。終わった読出しは IL へ。ERROR で止める出来事は ERRK・OPCH。
        let mut record_access = None;
        let mut access_done = false;
        let mut slot_flag = false;
        let (mut dnr, mut dnw, mut errk, mut opch) = (false, false, false, false);
        if let Some(mut a) = self.op.take() {
            slot_flag = a.flag;
            let reply = if a.elapsed == 0 {
                Some(ports.access(&a.acc))
            } else if a.kind == Kind::Long && a.result.is_none() {
                Some(ports.continue_access(a.acc.port))
            } else {
                None
            };
            match reply {
                Some(PortReply::Done(v)) => a.result = Some(v),
                Some(PortReply::Last(v)) => {
                    a.result = Some(v);
                    a.end_mark = true;
                }
                _ => {}
            }
            a.elapsed += 1;
            let finished = match a.kind {
                Kind::Single => true,
                Kind::Two => a.elapsed >= 2,
                Kind::Long => a.result.is_some(),
            };
            if finished {
                let v = a.result.unwrap_or(0);
                if a.acc.dir == Dir::Read {
                    a.acc.data = v;
                }
                if a.acc.port == 1 && self.done_mismatch(&a) {
                    errk = true;
                } else if a.acc.dir == Dir::Read {
                    self.il = v;
                    dnr = true;
                } else {
                    dnw = true;
                }
                access_done = true;
            } else if a.kind == Kind::Long && a.elapsed >= TIMEOUT_SLOTS {
                ports.abort_access(a.acc.port);
                errk = true;
            } else {
                self.op = Some(a);
            }
            opch = a.acc.ch == 1;
            record_access = Some(a.acc);
        }
        if errk {
            self.nx = self.nx.filter(|n| (n.ch == 1) != opch);
        }

        // 4. LCLK
        let aop = self.aop();
        let decision = if !self.accrn {
            Decision::Start(!self.achn as u8)
        } else if !self.accwn {
            Decision::Write(!self.achn as u8)
        } else if matches!(aop, Aop::Count(_)) && !self.infn && !self.q1n {
            Decision::CountAfterWrite(!self.achn as u8)
        } else {
            Decision::None
        };
        self.lclk(ports, doc, dnr, dnw, errk, opch);

        self.prev_flag = slot_flag;
        let rec = SlotRecord {
            slot: self.slot,
            s,
            candidate: c as u8,
            aop,
            decision,
            access: record_access,
            access_done,
        };
        self.slot += 1;
        rec
    }

    /// AOP と ACH の今の値（SlotRecord の形）。
    fn aop(&self) -> Aop {
        let n = (!self.aop2n as u8) << 2 | (!self.aop1n as u8) << 1 | !self.aop0n as u8;
        let c = !self.achn as u8;
        match n {
            0 => Aop::None,
            1 => Aop::Host,
            2 => Aop::SrcStep(c),
            3 => Aop::DstStep(c),
            4 => Aop::Count(c),
            k => Aop::Reload(c, k - 5),
        }
    }

    /// NX を OP に移す（番地は PS・PD、書込みのデータは IL の変換、種類と FLAG は ATR）。
    fn begin(&self, n: Next, doc: bool) -> Op {
        let regs = &self.regs[n.ch as usize];
        let (addr, word, data) = match n.dir {
            Dir::Read => (self.ps, input_is_word(regs.format), 0),
            Dir::Write => (
                self.pd,
                output_is_word(regs.format),
                fd_value(self.il, regs.format, regs.shift, regs.mask, regs.xor),
            ),
        };
        let attr = atr(n.port, n.dir, regs.format);
        let kind = if is_long(n.port, doc) {
            Kind::Long
        } else if attr & ATR_TWO != 0 {
            Kind::Two
        } else {
            Kind::Single
        };
        Op {
            acc: Access {
                ch: n.ch,
                port: n.port,
                dir: n.dir,
                addr,
                word,
                data,
            },
            kind,
            elapsed: 0,
            flag: attr & ATR_FLAG != 0,
            result: None,
            end_mark: false,
        }
    }

    /// port 1 の DONE の印が、要素が最後か（COUNT の桁上げ）と食い違う。読出しは要素（COUNT を
    /// 更新してあれば ELAST）、書込みは書込みの残り（TLAST）と比べる。W の後の COUNT の更新を
    /// 待つ要素は、まだ最後か分からないので比べない。
    fn done_mismatch(&self, a: &Op) -> bool {
        // ELAST・TLAST（ピンは負の論理）が印と違う ⇔ ピンの値が印と同じ。
        let inf = !self.infn;
        let count_waits = inf && !self.q1n;
        match a.acc.dir {
            Dir::Read => inf && self.q1n && self.epcn && self.elastn == a.end_mark,
            Dir::Write => !count_waits && !self.tln && self.tlastn == a.end_mark,
        }
    }

    /// LCLK：A ファイルの操作と、CHS・STS・ELS・U36 の登録。要求の FF を消す。
    fn lclk<P: Ports + ?Sized>(
        &mut self,
        ports: &P,
        doc: bool,
        dnr: bool,
        dnw: bool,
        errk: bool,
        opch: bool,
    ) {
        let (a2, a1, a0, ach) = (self.aop2n, self.aop1n, self.aop0n, self.achn);
        // A ファイル：読みの相で AFC0 の番地を読み、B（74S189 の B ファイルか、全部 1）と Cin を足す。
        // PS・PD・U7–U8 は読んだ値を取り込む。書きの相で、和か CPU のデータを書く。
        let host = self.hreq.filter(|h| h.result.is_none());
        let (hch, ha, hrw, hdata) = match host {
            // 窓の番地の bit 6 がチャネル、bit 3・2 が語（HA3・HA2）。
            Some(h) => (h.ch == 1, h.reg as u32 & 0x0C, h.write.is_none(), h.write),
            None => (false, 0, true, None),
        };
        let rd0 = afc0(a2, a1, a0, ach, hch, ha, hrw, false);
        let rd1 = afc1(a2, a1, a0, ach, hrw, false);
        let av = self.afile[word(rd0.aa3n, rd0.aa2n, rd0.aa1n, rd0.aa0n)];
        let bv = if rd1.benn {
            0xFFFF
        } else {
            let r = &self.regs[(rd1.ba >> 1) as usize];
            if rd1.ba & 1 != 0 {
                r.dst_step
            } else {
                r.src_step
            }
        };
        let sum = av as u32 + bv as u32 + rd0.cin as u32;
        let cy = sum > 0xFFFF;
        if !rd1.pscn {
            self.ps = av;
        }
        if !rd1.pdcn {
            self.pd = av;
        }
        let wr0 = afc0(a2, a1, a0, ach, hch, ha, hrw, true);
        let wr1 = afc1(a2, a1, a0, ach, hrw, true);
        if !wr0.awen {
            let v = if !wr1.sloen {
                sum as u16
            } else {
                debug_assert!(!wr1.hwoen);
                hdata.unwrap_or(0xFFFF)
            };
            self.afile[word(wr0.aa3n, wr0.aa2n, wr0.aa1n, wr0.aa0n)] = v;
        }
        if let Some(h) = &mut self.hreq
            && h.result.is_none()
            && a2
            && a1
            && !a0
        {
            h.result = Some(match h.write {
                Some(_) => HostReply::Ack,
                None => {
                    debug_assert!(!rd1.hrcn);
                    HostReply::Data(av)
                }
            });
        }

        // 組合せ：EVT、ADC、START の検査（判断バスのポートの ATR と /PRES）。
        let k = self.cst();
        let (c, port, dir) = self.bus_port(&k);
        let fmt = self.regs[c].format;
        let pres = !matches!(port, 8 | 10..=15) || ports.module_present(port);
        let oks = atr(port, dir, fmt) & if doc { ATR_OKD } else { ATR_OKN } != 0 && pres;
        let e = evt(
            bus(self.stpq),
            errk,
            opch,
            self.tln,
            self.tchn,
            self.tlastn,
            dnw,
            self.infn,
            self.q1n,
            self.q0n,
            a2,
            cy,
            ach,
        );
        let each = [self.regs[0].trigger & 8 != 0, self.regs[1].trigger & 8 != 0];
        let d = adc(
            a2,
            a1,
            a0,
            ach,
            self.accrn,
            self.stvn,
            bus(each),
            bus([self.imm(0), self.imm(1)]),
        );
        let trig = self.trig_in | ports.trigger_levels();
        let mut nk = [ChsOut {
            nbsyn: true,
            p2n: true,
            p1n: true,
            p0n: true,
            okn: true,
            donen: true,
            errn: true,
            nrunn: true,
        }; CHANNELS];
        let mut nt = [self.sts_now(0); CHANNELS];
        let mut level = [false; CHANNELS];
        for c in 0..CHANNELS {
            let (r, kr, t) = (&self.regs[c], &self.chs[c], &self.sts[c]);
            nk[c] = chs(
                bit(d.rldn, c),
                bit(d.cntn, c),
                cy,
                oks,
                bit(e.blkn, c),
                bit(e.kiln, c),
                bit(e.ern, c),
                self.strq[c],
                r.trigger & 0x10 != 0,
                self.wdq[c],
                self.weq[c],
                kr.p2n,
                kr.p1n,
                kr.p0n,
                kr.okn,
                kr.donen,
                kr.errn,
            );
            nt[c] = sts(
                self.s1q[c],
                nk[c].nbsyn,
                nk[c].nrunn,
                self.strq[c],
                bit(d.sten, c),
                self.wmq[c],
                self.imm(c),
                bit(d.svn, c),
                self.stln,
                kr.p1n,
                kr.p0n,
                t.syn2n,
                t.risen,
                t.armn,
                t.pendn,
                t.missn,
                t.staln,
            );
            // トリガの発生源の選択（74LS151）。0（即時）、6・7 はレベルを出さない。
            let src = r.trigger & 7;
            level[c] = (1..=5).contains(&src) && trig & (1 << src) != 0;
        }
        let l = els(
            self.accrn,
            self.accwn,
            a2,
            a1,
            cy,
            dnr,
            dnw,
            ach,
            e.ken,
            e.ktn,
            self.infn,
            self.q1n,
            self.q0n,
            self.epcn,
            self.elastn,
            self.tln,
            self.tchn,
            self.tlastn,
        );

        // 登録
        for c in 0..CHANNELS {
            let (o, t) = (&nk[c], &nt[c]);
            self.chs[c] = ChsRegs {
                p2n: o.p2n,
                p1n: o.p1n,
                p0n: o.p0n,
                okn: o.okn,
                donen: o.donen,
                errn: o.errn,
            };
            self.sts[c] = StsRegs {
                syn2n: t.syn2n,
                risen: t.risen,
                armn: t.armn,
                pendn: t.pendn,
                missn: t.missn,
                staln: t.staln,
            };
        }
        self.s1q = level;
        self.infn = l.infn;
        self.q1n = l.q1n;
        self.q0n = l.q0n;
        self.epcn = l.epcn;
        self.elastn = l.elastn;
        self.tln = l.tln;
        self.tchn = l.tchn;
        self.tlastn = l.tlastn;
        self.strq = [false; CHANNELS];
        self.stpq = [false; CHANNELS];
        self.wdq = [false; CHANNELS];
        self.weq = [false; CHANNELS];
        self.wmq = [false; CHANNELS];
    }
}
