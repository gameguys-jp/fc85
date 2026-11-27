//! Data Fabric の門の模型（DR-085、DR-089）。fabric.md §06・§07 の PAL の式から生成した関数
//! （`fabric_pal`）を、PAL のピンの値のレジスタでつなぎ、1 スロットずつ進める。
//!
//! 外側は動作の模型 `fabric::Fabric` と同じ（`Ports`、CPU の窓、`SlotRecord`）。2 つを並べて動かし、
//! スロットごとに突き合わせる（試験 `fabric_lockstep`）。製品の経路は動作の模型のまま（DR-085）。
//! この模型は試験の側にだけ置き、`fc85-board` の lib には入れない。lib に入れるだけで、コードの
//! 配置が変わって本番が遅くなったため（DR-085）。
//!
//! 門の模型にした部分：TIM、CST、ELG、SEQ、ELS、EVT、ADC、CHS×2、STS×2、AFC0–1、RDY、DPA、
//! アクセスの制御（OPA、OPB、NX・OP のレジスタ、U38 の 74LS163）、CPU の窓（HDEC0–1、HACK、U5–U6 の
//! 74LS138、PRST）、変換部（DR-084）、A ファイル（74S189）と語の bit 0 の選択（U64）、加算器（74LS283）、
//! 0 の検出（DR-091）、PS・PD、IL、トリガの発生源の選択（74LS151）と同期化の 1 段目（U36）。
//! ポートの向こう（P1A、P2A、STB、FIFO、ACRTC と DOC）は `Ports` の模型が受け持つ。P1A・P2A・STB の式は
//! fabric_logic.py が、ポートの模型（acrtc.rs、doc.rs）の 16 MHz の縁と突き合わせる。
//! 時間切れの打切りは、OPB の TOR が次の頭で P1A・P2A に届く。動作の模型と同じく、時間切れのスロットで
//! `abort_access` を呼ぶ（ポートの模型は、次のスロットの頭で効かせる）。延びたアクセスの答えは、q0 の縁なら
//! そのスロットに、q1〜q3 の縁なら預かって（P1A・P2A の PND）次のスロットに数える（`Ports::reply_edge`、
//! DR-091）。頭の打切り（ABQ）は預かった答えを捨てる。
//!
//! クロック（fabric.md §03）：TIM が 16 MHz の縁を数え、SCLK（スロットの頭）で SEQ・AFC0–1・OPA と NX・OP、
//! q2 の縁で MCLK（RDY）と LCLK（OPB・CHS・STS・ELS・U38・U36、A ファイルの読みの取込み：PS・PD・U7–U8・
//! SL）、q3（WPH）で A ファイルの書込みと HACK の AK。CPU の書込みの
//! うち START・STOP と STATUS の W1C は要求の FF（74LS74）に入り、次の LCLK で効いて消える。CPU の 2 回の
//! アクセスの間には 1 スロット以上ある（68000 のバスサイクルは 4 クロック＝2 スロット）とする。同じ
//! スロットの間の書込みと読出しでは、動作の模型（書込みがすぐ効く）と STATUS の値が違う。CPU のアクセスは
//! スロットの境目に来る（エミュレータの約束）。実機で、スロットの途中に来た要求がいつ効くかは OI-088。
//!
//! PAL のレジスタは、電源投入時に出力のピンが 1 になる（負の論理の信号はすべて 0）とする [V]。TIM は
//! 電源投入の RESN で q3・X に戻ったところから始める。

// 試験ごとに使う入口が違うので、使わない入口を咎めない。
#![allow(dead_code)]

use fc85_board::fabric::{
    Access, Aop, CHANNELS, ChannelRegs, Decision, Dir, HostReply, Phase, PortReply, Ports,
    SlotRecord, atr, fd_value,
};
use fc85_board::fabric_pal::{
    ATR_FLAG, ATR_OKD, ATR_OKN, ATR_TWO, ATR_W16, Afc0Out, Afc1Out, ChsOut, CstOut, HackOut,
    Hdec0Out, Hdec1Out, StsOut, adc, afc0, afc1, chs, cst, dpa, elg, els, evt, hack, hdec0, hdec1,
    opa, opb, prst, rdy, seq, sts, tim,
};

/// AFC0（PAL20R6A、SCLK）のレジスタ。ピンの値（負の論理）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Afc0Regs {
    aa3n: bool,
    aa2n: bool,
    aa1n: bool,
    ar0n: bool,
    aw0n: bool,
    awn: bool,
}

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

/// NX（74LS374×2）と OP（74LS377×2）の中身：有効、チャネル、ポート、向き（1 で書込み）、FORMAT、ATR の
/// W16・FLAG・TWO、延びたアクセス（LG）。NX は STALL の評価（STV、STL）も持つ（別に置く）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AccReg {
    v: bool,
    ch: bool,
    port: u8,
    w: bool,
    format: u8,
    w16: bool,
    lg: bool,
    flag: bool,
    two: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HostReq {
    ch: u8,
    reg: u8,
    write: Option<u16>,
    result: Option<HostReply>,
}

/// TIM（PAL16R8A）のレジスタ。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TimRegs {
    ph: u32,
    sx: bool,
    sclk: bool,
    mclk: bool,
    lclk: bool,
    /// q3 の間 Low（次の縁が q0）。
    wphn: bool,
    /// A ファイルの書込みの相（AWN のとき q3 の間 Low。/AWE ＝ AWQN ∨ C16）。
    awqn: bool,
}

impl TimRegs {
    fn of(t: fc85_board::fabric_pal::TimOut) -> Self {
        Self {
            ph: t.ph,
            sx: t.sx,
            sclk: t.sclk,
            mclk: t.mclk,
            lclk: t.lclk,
            wphn: t.wphn,
            awqn: t.awqn,
        }
    }
}

/// Data Fabric の門の模型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateFabric {
    slot: u64,
    tim: TimRegs,
    regs: [ChannelRegs; CHANNELS],
    /// A ファイル（74S189×4、16 語）。語は ch×8＋（0 SRC の設定値、1 作業値、2 DST の設定値、3 作業値、
    /// 4 COUNT の設定値、5 作業値）。
    afile: [u16; 16],
    // AFC0・AFC1（SCLK）
    afc0: Afc0Regs,
    afc1: Afc1Out,
    /// SL（74ALS534×2）：LCLK で取り込んだ和。
    sl: u16,
    /// U7–U8（74LS534×2）：CPU の読出しで LCLK に取り込んだ A バス。
    u78: u16,
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
    nx: AccReg,
    op: AccReg,
    // OPA（SCLK）
    sdn: bool,
    lan: bool,
    // OPB（LCLK）
    odn: bool,
    tor: bool,
    /// U38（74LS163）：延びたアクセス（LA）の間、LCLK を数える。15 で TOUT（RCO）。
    tcnt: u8,
    /// OP のアクセスの番地とデータ（PA、FD）。OP に入れたスロットの頭の PS・PD と IL の変換。
    acc: Access,
    /// port 9 の 1 スロット目に A 基板が出した値（2 スロット目の終わりに IL が取り込む）。
    two_data: u16,
    /// P1A・P2A が預かった答え（q1〜q3 の縁で来た値と DONE の印）。次の q0 の縁で P1R・P2R になる。
    pnd: Option<(u16, bool)>,
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
    /// CPU の A ファイルのアクセス（HDEC の AREQ の間）。
    hreq: Option<HostReq>,
    /// HACK の AK（負の論理）：許可のスロットの q3 の縁で立ち、CPU のサイクルが終わると消える。
    akn: bool,
    trig_in: u8,
    /// このスロットで起きたこと（試験の網羅の目安）。
    ev: SlotEvents,
}

/// 1 スロットの間に起きたこと。突き合わせには使わない。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotEvents {
    /// STOP で延びたアクセスを打ち切った（OPA の ABT の STOP の項）。
    pub stop_abort: bool,
    /// 時間切れ（OPB の TOR）のポート。
    pub timeout: Option<u8>,
    /// port 1 の DONE の印が、要素が最後かと食い違った（OPB の ERRK の印の項）。
    pub mark_mismatch: bool,
    /// 延びたアクセスの答えがポートから来た 16 MHz の縁（0〜3 が q0〜q3）。
    pub answer_edge: Option<u8>,
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

/// LG のつなぎ（74LS 系）：/P3∧/P2∧(/P1∧P0 ∨ P1∧/P0∧DOC)。延びたアクセスのポート（port 1、DOC 子基板の
/// port 2）。NX が判断バスのポートと一緒に取り込む。
fn lg_glue(p: u8, doc: bool) -> bool {
    let b = |k: u8| p >> k & 1 != 0;
    !b(3) && !b(2) && (!b(1) && b(0) || b(1) && !b(0) && doc)
}

/// 74LS151：選んだ入力。トリガの発生源の 0（即時）、6、7 の入力は Low に固定する。
fn ls151(sel: u8, d: u8) -> bool {
    d >> (sel & 7) & 1 != 0
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
        // TIM：RESN の縁の後（q3、X）。
        let r = tim(false, true, 0, false);
        Self {
            slot: 0,
            tim: TimRegs::of(r),
            regs: Default::default(),
            afile: [0; 16],
            afc0: Afc0Regs {
                aa3n: true,
                aa2n: true,
                aa1n: true,
                ar0n: true,
                aw0n: true,
                awn: true,
            },
            afc1: Afc1Out {
                benn: true,
                ba: 3,
                sloen: true,
                hwoen: true,
                pscn: true,
                pdcn: true,
                hrcn: true,
            },
            sl: 0,
            u78: 0,
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
            nx: AccReg::default(),
            op: AccReg::default(),
            sdn: true,
            lan: true,
            odn: false,
            tor: false,
            tcnt: 0,
            acc: Access {
                ch: 0,
                port: 0,
                dir: Dir::Read,
                addr: 0,
                word: false,
                data: 0,
            },
            two_data: 0,
            pnd: None,
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
            akn: true,
            trig_in: 0,
            ev: SlotEvents::default(),
        }
    }

    pub fn slot(&self) -> u64 {
        self.slot
    }

    pub fn events(&self) -> SlotEvents {
        self.ev
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
            true,
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

    /// HDEC0–1：窓の番地の解読（16-bit のアクセス。窓の外は /FCS が来ない）。
    fn decode(offset: u16, rw: bool) -> (Hdec0Out, Hdec1Out) {
        let a = offset as u32 & 0x0FFE;
        let h0 = hdec0(offset > 0x0FFF, false, false, rw, a);
        let h1 = hdec1(h0.chn, h0.gln, rw, a);
        (h0, h1)
    }

    /// HACK の今の値（Fabric の窓の側。DOC のブリッジの側は使わない）。`mclk` は次の縁が q3 か。
    fn hack_now(&self, mclk: bool) -> HackOut {
        hack(
            self.hreq.is_none(),
            true,
            self.aop2n,
            self.aop1n,
            self.aop0n,
            mclk,
            0,
            true,
            true,
            self.akn,
            true,
            true,
            true,
        )
    }

    pub fn host_read<P: Ports + ?Sized>(&mut self, offset: u16, ports: &P) -> HostReply {
        if offset & 1 != 0 {
            return HostReply::BusError;
        }
        let (h0, h1) = Self::decode(offset, true);
        if !h0.areqn {
            return self.request((offset >> 6 & 1) as usize, (offset & 0x3F) as u8, None);
        }
        if h0.vldn {
            return HostReply::BusError;
        }
        // RD は引上げ（何も出さなければ 0）。STATUS は U9–U10、MODULES は U11。
        HostReply::Data(if !h1.rst0n {
            self.status(0)
        } else if !h1.rst1n {
            self.status(1)
        } else if !h0.rmodn {
            let mut m = (ports.doc_present() as u16) << 2;
            for p in [8u8, 10, 11, 12, 13, 14, 15] {
                m |= (ports.module_present(p) as u16) << p;
            }
            m
        } else {
            0
        })
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
        let (h0, h1) = Self::decode(offset, false);
        let c = (offset >> 6 & 1) as usize;
        if !h0.areqn {
            return self.request(c, (offset & 0x3F) as u8, Some(value));
        }
        if h0.vldn {
            return HostReply::BusError;
        }
        let regs = &mut self.regs[c];
        if !h1.wsrcn {
            regs.src = value as u8 & 15;
        }
        if !h1.wdstn {
            regs.dst = value as u8 & 15;
        }
        // B ファイルの書込みのポート：語は A6（チャネル）と A4（0 で SRC、1 で DST の STEP）。
        if !h1.wbfn {
            if offset & 0x10 != 0 {
                regs.dst_step = value;
            } else {
                regs.src_step = value;
            }
        }
        // U5–U6（74LS138。A6 でチャネル）：オフセット 0x10〜0x1E を A3–A1 で選ぶ。
        if !h1.w1xn {
            match offset >> 1 & 7 {
                1 => regs.format = value as u8 & 7,
                2 => regs.mask = value,
                3 => regs.xor = value,
                4 => regs.shift = value as u8 & 31,
                5 => regs.trigger = value as u8 & 0x3F,
                // CTRL：STOP が先。START は段階が 000 のときだけ要求の FF に入る。
                6 => {
                    if value & 2 != 0 {
                        self.stpq[c] = true;
                    } else if value & 1 != 0 && self.chs[c].p2n && self.chs[c].p1n {
                        self.strq[c] = true;
                    }
                }
                7 => {
                    self.wdq[c] |= value & 0x02 != 0;
                    self.weq[c] |= value & 0x08 != 0;
                    self.wmq[c] |= value & 0x10 != 0;
                }
                _ => {}
            }
        }
        if !h1.wprstn {
            let p = prst(false, value as u32, true);
            let mut mask = 0;
            for (q, n) in [
                (2, p.rst2n),
                (3, p.rst3n),
                (4, p.rst4n),
                (5, p.rst5n),
                (6, p.rst6n),
                (7, p.rst7n),
                (10, p.rst10n),
            ] {
                mask |= (!n as u16) << q;
            }
            ports.port_reset(mask);
        }
        if !h1.waudn {
            ports.write_audio_n(value & 0x0FFF);
        }
        HostReply::Ack
    }

    pub fn host_poll(&mut self) -> Option<HostReply> {
        let r = self.hreq?.result?;
        // CPU のサイクルが終わる：AREQ が落ち、次の 16 MHz の縁で AK も消える。
        self.hreq = None;
        self.akn = true;
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
        self.akn = true;
        HostReply::Wait
    }

    // ------------------------------------------------------------------ スロット

    /// TIM の 1 回の 16 MHz の縁。AWN は AFC0 のレジスタ。
    fn tim_edge(&mut self) {
        self.tim = TimRegs::of(tim(true, self.afc0.awn, self.tim.ph, self.tim.sx));
    }

    /// AFC0 の組合せの出力（CIN）。AFC0 のレジスタだけで決まるので、ほかの入力は何でもよい。
    fn afc0_now(&self) -> Afc0Out {
        let f = &self.afc0;
        afc0(
            true, true, true, true, true, true, false, 0, true, f.aa2n, f.ar0n,
        )
    }

    /// A ファイルの語の bit 0（U64、74S157）：PH1 で、q0〜q1 は AFC0 の AR0N（読み）、q2〜q3（`write`）は
    /// AW0N（書き）を選ぶ。
    fn aa0n(&self, write: bool) -> bool {
        if write {
            self.afc0.aw0n
        } else {
            self.afc0.ar0n
        }
    }

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

    /// OPA の今の出力（SCLK の前の NX・OP・ODN・STOP の要求から）。
    fn opa_now(&self, tor: bool) -> fc85_board::fabric_pal::OpaOut {
        let (n, o) = (&self.nx, &self.op);
        opa(
            n.v,
            n.ch,
            n.two,
            n.lg,
            o.v,
            o.ch,
            o.two,
            o.lg,
            self.odn,
            bus(self.stpq),
            tor,
        )
    }

    pub fn step_slot<P: Ports + ?Sized>(&mut self, ports: &mut P) -> SlotRecord {
        let doc = ports.doc_present();
        // TIM：q0 の縁の前は WPH（q3。次の縁がスロットの頭）。
        debug_assert!(!self.tim.wphn);

        // STOP の要求のあるチャネルの延びたアクセスの打切り（OPA の ABT の STOP の項）。P1A・P2A は
        // この頭の縁で効かせる（ABQ ＝ ABT∧WPH）。時間切れの打切り（TOR）は、前のスロットで済んでいる。
        self.ev = SlotEvents::default();
        if !self.opa_now(false).abtn {
            ports.abort_access(self.op.port);
            self.ev.stop_abort = true;
        }

        // 1. SCLK の前の値で判断する。OPA：OP が空くか（FREE）、NX が STOP で消えないか（OPVD）、判断して
        //    よいか（CD）。DPA：判断バスのポートの属性と FB（OP と、入る NX の FLAG）。
        let o = self.opa_now(self.tor);
        let free = !o.opldn;
        // 打切り（ABQ）は、P1A・P2A が預かった答えを捨てる
        if !o.abtn {
            self.pnd = None;
        }
        debug_assert!(
            self.pnd.is_none() || !free,
            "預かった答えのアクセスは、数えるまで OP にある"
        );
        debug_assert!(!self.nx.v || free, "NX が有効なら、OP はこの頭で空く");
        // DPA の X は TIM の SX のピン：縁の前は前のスロットが S、つまり縁の後のスロットが X。
        let x = self.tim.sx;
        let k = self.cst();
        let (c, port, dir) = self.bus_port(&k);
        let r = self.regs[c];
        let dp = dpa(
            r.src as u32,
            r.dst as u32,
            x,
            self.op.v,
            self.op.flag,
            o.opvd,
            self.nx.flag,
        );
        let ready = rdy(0, 0, 0, 0, 0, false, self.srdy, self.drdy).ready;
        let hreq = self.hack_now(false).hreq;
        let g = elg(
            self.infn, self.q1n, self.epcn, k.crunn, k.cran, k.crldn, k.carmn, ready, dp.goks,
            dp.addrs, dp.sokd, o.cd, hreq, dp.addrd,
        );

        // SCLK の縁：TIM、SEQ、AFC0–1、OPA の登録、OP ← NX（空いていれば。有効は OPVD）、NX ← 判断バス。
        self.tim_edge();
        debug_assert!(self.tim.sclk && self.tim.ph == 0);
        let s = self.tim.sx;
        debug_assert_eq!(s, !x);
        let hold = s && !k.crunn && r.src == 0;
        let q = seq(
            g.ac2n, g.ac1n, g.ac0n, g.grn, g.gwn, g.keepn, k.ck1n, k.ck0n, k.crldn, k.orldn,
            k.orunn, hold, self.dbchn,
        );
        // AFC0–1：SEQ と同じ入力（AC、CK、次の ACH ＝ 縁の前の DBCH）と、CPU の窓の番地（bit 6 がチャネル、
        // bit 3・2 が語）と向き
        let (hch, ha, hrw) = match self.hreq {
            Some(h) => (h.ch == 1, h.reg as u32 & 0x0C, h.write.is_none()),
            None => (false, 0, true),
        };
        let f = self.afc0;
        let a0 = afc0(
            g.ac2n, g.ac1n, g.ac0n, k.ck1n, k.ck0n, self.dbchn, hch, ha, hrw, f.aa2n, f.ar0n,
        );
        self.afc0 = Afc0Regs {
            aa3n: a0.aa3n,
            aa2n: a0.aa2n,
            aa1n: a0.aa1n,
            ar0n: a0.ar0n,
            aw0n: a0.aw0n,
            awn: a0.awn,
        };
        self.afc1 = afc1(g.ac2n, g.ac1n, g.ac0n, self.dbchn, hrw);
        let new_op = free && o.opvd;
        if free {
            self.op = AccReg {
                v: o.opvd,
                ..self.nx
            };
        }
        let decided = !g.grn || !g.gwn;
        debug_assert!(!decided || dir == if !g.grn { Dir::Read } else { Dir::Write });
        let attr = atr(port, dir, r.format);
        self.nx = AccReg {
            v: decided,
            ch: c == 1,
            port,
            w: dir == Dir::Write,
            format: r.format,
            w16: attr & ATR_W16 != 0,
            lg: lg_glue(port, doc),
            flag: attr & ATR_FLAG != 0,
            two: attr & ATR_TWO != 0,
        };
        self.sdn = o.sdn;
        self.lan = o.lan;
        // P1A の L2 は、OP に入る縁で取る OP の写し（OPVD ∧ NXLG ∧ ¬NXP0）。OPA の LA ∧ ¬OPP0 と同じであるには、
        // LA が OP の有効 ∧ LG と同じであればよい（DR-091）
        debug_assert_eq!(!self.lan, self.op.v && self.op.lg, "LA は OP の有効 ∧ LG");
        self.stvn = g.stvn;
        self.stln = g.stln;
        self.dbchn = q.dbchn;
        self.achn = q.achn;
        self.aop2n = q.aop2n;
        self.aop1n = q.aop1n;
        self.aop0n = q.aop0n;
        self.accrn = q.accrn;
        self.accwn = q.accwn;
        let starting = if new_op {
            self.acc = self.begin();
            Some(self.acc)
        } else {
            None
        };
        ports.run_slot(starting.as_ref());

        // 2. q2 の縁（MCLK・LCLK）：判断バスの新しいチャネルの準備を RDY が取り込む。
        self.tim_edge();
        self.tim_edge();
        debug_assert!(self.tim.mclk && self.tim.lclk);
        let nr = &self.regs[!self.dbchn as usize];
        let lines = ports.ready_lines();
        let o2 = rdy(
            nr.src as u32,
            nr.dst as u32,
            lines,
            lines,
            lines,
            lines & 1 << 11 != 0,
            self.srdy,
            self.drdy,
        );
        self.srdy = o2.srdy;
        self.drdy = o2.drdy;

        // 3. アクセス。ポートの答え（P1A・P2A が q0 の縁で登録する P1R・P2R、port 1 の DONE の印 P1M）と U38 の
        //    TOUT から、OPB が完了・ERROR・時間切れを出す。終わった読出しは、次の SCLK で IL へ（/EN ＝ DNRQ）。
        //    q1〜q3 の縁で来た答えは預かり（PND）、次のスロットの頭で P1R・P2R にする。
        let la = !self.lan;
        let mut record_access = None;
        let mut access_done = false;
        let (mut p1r, mut p2r, mut p1m) = (false, false, false);
        let mut v = 0;
        if self.op.v {
            let held = self.pnd.take();
            let reply = if let Some((x, last)) = held {
                Some(if last {
                    PortReply::Last(x)
                } else {
                    PortReply::Done(x)
                })
            } else if new_op {
                Some(ports.access(&self.acc))
            } else if la {
                Some(ports.continue_access(self.op.port))
            } else {
                None
            };
            let (mut got, val, last) = match reply {
                Some(PortReply::Done(x)) => (true, x, false),
                Some(PortReply::Last(x)) => (true, x, true),
                _ => (false, 0, false),
            };
            if got && la && held.is_none() {
                let q = ports.reply_edge(self.op.port);
                self.ev.answer_edge = Some(q);
                if q > 0 {
                    self.pnd = Some((val, last));
                    got = false;
                }
            }
            if self.op.two {
                if new_op {
                    self.two_data = if got { val } else { 0 };
                }
                v = self.two_data;
            } else if got {
                v = val;
            }
            p1r = la && got && self.op.port == 1;
            p2r = la && got && self.op.port != 1;
            p1m = p1r && last;
            access_done = !self.sdn || p1r || p2r;
            if access_done && !self.op.w {
                self.acc.data = v;
            }
            record_access = Some(self.acc);
        }
        let b = opb(
            self.sdn,
            self.lan,
            self.op.w,
            !p1r,
            !p2r,
            !p1m,
            self.tcnt == 15,
            self.infn,
            self.q1n,
            self.epcn,
            self.elastn,
            self.tln,
            self.tlastn,
        );
        if b.dnr {
            self.il = v;
        }
        if b.tor {
            ports.abort_access(self.op.port);
            self.ev.timeout = Some(self.op.port);
        }
        let errk = !b.errkn;
        self.ev.mark_mismatch = errk && !b.tor;
        let opch = self.op.ch;
        debug_assert!(!errk || !self.nx.v, "延びたアクセスの間は、NX に判断がない");

        // 4. LCLK（q2 の縁の続き）
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
        self.lclk(ports, doc, b.dnr, b.dnw, errk, opch);
        // U38（74LS163、/CLR ＝ LA）
        self.tcnt = if la { (self.tcnt + 1) & 15 } else { 0 };
        self.odn = b.odn;
        self.tor = b.tor;

        // 5. q3 の縁（WPH）
        self.tim_edge();
        debug_assert!(!self.tim.wphn && self.tim.ph == 3);
        self.wph();

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

    /// OP に入れたアクセスの番地（PS・PD）とデータ（IL の変換。FORMAT は OP、SHIFT・MASK・XOR は
    /// チャネルのレジスタ）。
    fn begin(&self) -> Access {
        let n = self.op;
        let regs = &self.regs[n.ch as usize];
        let (dir, addr, data) = if n.w {
            (
                Dir::Write,
                self.pd,
                fd_value(self.il, n.format, regs.shift, regs.mask, regs.xor),
            )
        } else {
            (Dir::Read, self.ps, 0)
        };
        Access {
            ch: n.ch as u8,
            port: n.port,
            dir,
            addr,
            word: n.w16,
            data,
        }
    }

    /// LCLK（q2）：A ファイルの読み（SCLK に AFC0 が登録した読みの語）と、CHS・STS・ELS の登録。要求の FF を
    /// 消す。
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
        // A ファイル：q0〜q1（PH1＝0）の読みの語を読み、B（74LS670 の B ファイルか、全部 1）と Cin を足す。
        // SL が和を、PS・PD・U7–U8 が A バスを取り込む。桁上げ（CY）は加算器ではなく A バスの 0 の検出
        // （74S02×2・74S30）：A ≠ 0。使うのは COUNT と再設定の 3 段目（B ＝ FFFF、Cin ＝ 0）だけで、そこでは
        // 加算器の桁上げと同じ。
        let f = self.afc0;
        let rd = self.afc0_now();
        let av = self.afile[word(f.aa3n, f.aa2n, f.aa1n, self.aa0n(false))];
        let f1 = self.afc1;
        let bv = if f1.benn {
            0xFFFF
        } else {
            let r = &self.regs[(f1.ba >> 1) as usize];
            if f1.ba & 1 != 0 {
                r.dst_step
            } else {
                r.src_step
            }
        };
        let sum = av as u32 + bv as u32 + rd.cin as u32;
        let cy = av != 0;
        debug_assert!(
            !matches!(self.aop(), Aop::Count(_) | Aop::Reload(_, 2)) || cy == (sum > 0xFFFF)
        );
        self.sl = sum as u16;
        if !f1.pscn {
            self.ps = av;
        }
        if !f1.pdcn {
            self.pd = av;
        }
        if !f1.hrcn {
            self.u78 = av;
        }

        // 組合せ：EVT、ADC、START の検査（判断バスのポートの ATR と /PRES）。
        let k = self.cst();
        let (c, port, dir) = self.bus_port(&k);
        let fmt = self.regs[c].format;
        let pres = !matches!(port, 8 | 10..=15) || ports.module_present(port);
        let oks = atr(port, dir, fmt) & if doc { ATR_OKD } else { ATR_OKN } != 0 && pres;
        let e = evt(
            bus(self.stpq),
            !errk,
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
        let mut nk = [ChsOut {
            nbsyxn: true,
            p2n: true,
            p1n: true,
            p0n: true,
            okn: true,
            donen: true,
            errn: true,
            nrun0n: true,
        }; CHANNELS];
        let mut nt = [self.sts_now(0); CHANNELS];
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
                nk[c].nbsyxn,
                nk[c].nrun0n,
                self.strq[c],
                bit(d.sten, c),
                self.wmq[c],
                self.imm(c),
                bit(d.svn, c),
                self.stln,
                kr.p1n,
                kr.p0n,
                bit(e.blkn, c),
                t.syn2n,
                t.risen,
                t.armn,
                t.pendn,
                t.missn,
                t.staln,
            );
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
        // U36：同じ LCLK の縁で、選んだトリガのレベルを取り込む（STS は縁の前の値を使った）。発生源の選択
        // （U34–U35、74LS151）の入力 0（即時）、6、7 は Low に固定。
        let trig = (self.trig_in | ports.trigger_levels()) & 0x3E;
        for c in 0..CHANNELS {
            self.s1q[c] = ls151(self.regs[c].trigger & 7, trig);
        }
    }

    /// q3 の縁（WPH）：A ファイルの書込み（/AWE ＝ AWQN ∨ C16 で q3 の後半。PH1＝1 の書きの語に、SL の和か
    /// CPU のデータ）、HACK の AK（許可のスロットの q3 の縁で立ち、DTACK を返す）。
    fn wph(&mut self) {
        if !self.tim.awqn {
            let f = self.afc0;
            let w = word(f.aa3n, f.aa2n, f.aa1n, self.aa0n(true));
            let v = if !self.afc1.sloen {
                self.sl
            } else {
                debug_assert!(!self.afc1.hwoen);
                self.hreq.and_then(|h| h.write).unwrap_or(0xFFFF)
            };
            self.afile[w] = v;
        }
        let hk = self.hack_now(true);
        if self.akn
            && !hk.akn
            && let Some(h) = &mut self.hreq
        {
            h.result = Some(match h.write {
                Some(_) => HostReply::Ack,
                None => {
                    debug_assert!(!self.afc1.hrcn);
                    HostReply::Data(self.u78)
                }
            });
        }
        self.akn = hk.akn;
    }
}
