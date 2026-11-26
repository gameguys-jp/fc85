//! Data Fabric の PAL の式と PROM の中身（生成したファイル。手で直さない）。
//!
//! 元は `tools/fabric/pal/*.pal`（PALASM の式。`docs/spec/fabric.md` の式と同じ）と、
//! `tools/fabric/fabric_logic.py` の PROM の中身。作り直すには、リポジトリの根で
//! `py tools/fabric/fabric_logic.py --rust` を実行する。式と同じかは `py -m unittest discover -s tools/fabric` で確かめる。
//!
//! PAL は、入力の bus（名前の末尾の数字が bit）と 1 本の信号（bool）を受けて、出力のピンの値を返す。
//! 出力は反転する（`/X = 積和` のピン X は積和の否定）。登録出力はクロックの後の値（D の入力）を返し、
//! 帰還はレジスタの今の値を引数で受ける。3 ステートの出力は、出しているかを `*_oe` で返す。

/// ROT0（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rot0Out {
    /// 組合せの出力：F0、F1、F2、F3、F4、F5、F6、F7
    pub f: u32,
}

/// ROT0（PAL20L8A）。入力 D0–D7、D13–D15、T0、T1。細かい段：0〜3 だけ左へ回す
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn rot0(d: u32, t: u32) -> Rot0Out {
    let f_v = !(((t >> 1 & 1).wrapping_sub(1) & (t >> 0 & 1).wrapping_sub(1) & !(d)) | ((t >> 1 & 1).wrapping_sub(1) & 0u32.wrapping_sub(t >> 0 & 1) & !((d as u16).rotate_right(15) as u32)) | (0u32.wrapping_sub(t >> 1 & 1) & (t >> 0 & 1).wrapping_sub(1) & !((d as u16).rotate_right(14) as u32)) | (0u32.wrapping_sub(t >> 1 & 1) & 0u32.wrapping_sub(t >> 0 & 1) & !((d as u16).rotate_right(13) as u32))) & 0xff;
    Rot0Out {
        f: f_v,
    }
}

/// ROT1（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rot1Out {
    /// 組合せの出力：F8、F9、F10、F11、F12、F13、F14、F15
    pub f: u32,
}

/// ROT1（PAL20L8A）。入力 D5–D15、T0、T1
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn rot1(d: u32, t: u32) -> Rot1Out {
    let f_v = !(((t >> 1 & 1).wrapping_sub(1) & (t >> 0 & 1).wrapping_sub(1) & !(d)) | ((t >> 1 & 1).wrapping_sub(1) & 0u32.wrapping_sub(t >> 0 & 1) & !((d as u16).rotate_right(15) as u32)) | (0u32.wrapping_sub(t >> 1 & 1) & (t >> 0 & 1).wrapping_sub(1) & !((d as u16).rotate_right(14) as u32)) | (0u32.wrapping_sub(t >> 1 & 1) & 0u32.wrapping_sub(t >> 0 & 1) & !((d as u16).rotate_right(13) as u32))) & 0xff00;
    Rot1Out {
        f: f_v,
    }
}

/// ROT2（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rot2Out {
    /// 組合せの出力：R0、R1、R4、R5、R8、R9、R12、R13
    pub r: u32,
}

/// ROT2（PAL20L8A）。入力 F0、F1、F4、F5、F8、F9、F12、F13、T2、T3。粗い段：4 の倍数だけ回す
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn rot2(f: u32, t: u32) -> Rot2Out {
    let r_v = !(((t >> 3 & 1).wrapping_sub(1) & (t >> 2 & 1).wrapping_sub(1) & !(f)) | ((t >> 3 & 1).wrapping_sub(1) & 0u32.wrapping_sub(t >> 2 & 1) & !((f as u16).rotate_right(12) as u32)) | (0u32.wrapping_sub(t >> 3 & 1) & (t >> 2 & 1).wrapping_sub(1) & !((f as u16).rotate_right(8) as u32)) | (0u32.wrapping_sub(t >> 3 & 1) & 0u32.wrapping_sub(t >> 2 & 1) & !((f as u16).rotate_right(4) as u32))) & 0x3333;
    Rot2Out {
        r: r_v,
    }
}

/// ROT3（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rot3Out {
    /// 組合せの出力：R2、R3、R6、R7、R10、R11、R14、R15
    pub r: u32,
}

/// ROT3（PAL20L8A）。入力 F2、F3、F6、F7、F10、F11、F14、F15、T2、T3
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn rot3(f: u32, t: u32) -> Rot3Out {
    let r_v = !(((t >> 3 & 1).wrapping_sub(1) & (t >> 2 & 1).wrapping_sub(1) & !(f)) | ((t >> 3 & 1).wrapping_sub(1) & 0u32.wrapping_sub(t >> 2 & 1) & !((f as u16).rotate_right(12) as u32)) | (0u32.wrapping_sub(t >> 3 & 1) & (t >> 2 & 1).wrapping_sub(1) & !((f as u16).rotate_right(8) as u32)) | (0u32.wrapping_sub(t >> 3 & 1) & 0u32.wrapping_sub(t >> 2 & 1) & !((f as u16).rotate_right(4) as u32))) & 0xcccc;
    Rot3Out {
        r: r_v,
    }
}

/// ANX0（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anx0Out {
    /// 組合せの出力：Q0、Q1、Q2、Q3
    pub q: u32,
    /// 出力の許可：Q0、Q1、Q2、Q3
    pub q_oe: u32,
}

/// ANX0（PAL20L8A）。入力 R・M（MSK）・K（MASK）・X（XOR）の 4 bit 分と WEN。Q = (R*M*K) ⊕ X
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn anx0(r: u32, m: u32, k: u32, x: u32, wen: bool) -> Anx0Out {
    let q_v = !(((r) & (m) & (k) & (x)) | (!(r) & !(x)) | (!(m) & !(x)) | (!(k) & !(x))) & 0xf;
    let q_v_oe = 0u32.wrapping_sub(wen as u32) & 0xf;
    Anx0Out {
        q: q_v,
        q_oe: q_v_oe,
    }
}

/// ANX1（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anx1Out {
    /// 組合せの出力：Q4、Q5、Q6、Q7
    pub q: u32,
    /// 出力の許可：Q4、Q5、Q6、Q7
    pub q_oe: u32,
}

/// ANX1（PAL20L8A）。bit 4–7
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn anx1(r: u32, m: u32, k: u32, x: u32, wen: bool) -> Anx1Out {
    let q_v = !(((r) & (m) & (k) & (x)) | (!(r) & !(x)) | (!(m) & !(x)) | (!(k) & !(x))) & 0xf0;
    let q_v_oe = 0u32.wrapping_sub(wen as u32) & 0xf0;
    Anx1Out {
        q: q_v,
        q_oe: q_v_oe,
    }
}

/// ANX2（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anx2Out {
    /// 組合せの出力：Q8、Q9、Q10、Q11
    pub q: u32,
    /// 出力の許可：Q8、Q9、Q10、Q11
    pub q_oe: u32,
}

/// ANX2（PAL20L8A）。bit 8–11
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn anx2(r: u32, m: u32, k: u32, x: u32, wen: bool) -> Anx2Out {
    let q_v = !(((r) & (m) & (k) & (x)) | (!(r) & !(x)) | (!(m) & !(x)) | (!(k) & !(x))) & 0xf00;
    let q_v_oe = 0u32.wrapping_sub(wen as u32) & 0xf00;
    Anx2Out {
        q: q_v,
        q_oe: q_v_oe,
    }
}

/// ANX3（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anx3Out {
    /// 組合せの出力：Q12、Q13、Q14、Q15
    pub q: u32,
    /// 出力の許可：Q12、Q13、Q14、Q15
    pub q_oe: u32,
}

/// ANX3（PAL20L8A）。bit 12–15
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn anx3(r: u32, m: u32, k: u32, x: u32, wen: bool) -> Anx3Out {
    let q_v = !(((r) & (m) & (k) & (x)) | (!(r) & !(x)) | (!(m) & !(x)) | (!(k) & !(x))) & 0xf000;
    let q_v_oe = 0u32.wrapping_sub(wen as u32) & 0xf000;
    Anx3Out {
        q: q_v,
        q_oe: q_v_oe,
    }
}

/// AFC0 の AA3N の真理値表（番地の bit は AOP2N、AOP1N、AOP0N、ACHN、HCH の順）。
const AFC0_AA3N_T: u64 = 0x00000000f700ff08;

/// AFC0 の AA2N の真理値表（番地の bit は AOP2N、AOP1N、AOP0N、HA3、HA2 の順）。
const AFC0_AA2N_T: u64 = 0x00000000b6bebebe;

/// AFC0 の AA1N の真理値表（番地の bit は AOP2N、AOP1N、AOP0N、HA3、HA2 の順）。
const AFC0_AA1N_T: u64 = 0x00000000edede5ed;

/// AFC0 の AA0N の真理値表（番地の bit は AOP2N、AOP1N、AOP0N、HRW、WPH の順）。
const AFC0_AA0N_T: u64 = 0x000000008088959d;

/// AFC0（PAL16L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Afc0Out {
    /// 組合せの出力：AA3N
    pub aa3n: bool,
    /// 組合せの出力：AA2N
    pub aa2n: bool,
    /// 組合せの出力：AA1N
    pub aa1n: bool,
    /// 組合せの出力：AA0N
    pub aa0n: bool,
    /// 組合せの出力：AWEN
    pub awen: bool,
    /// 組合せの出力：CIN
    pub cin: bool,
}

/// AFC0（PAL16L8A）。A ファイルの番地（AA3–AA0、負の論理のまま 74S189 へ）、書込み（AWE）、加算器の桁上げの入力（CIN）。WPH は q3（書込みの相）
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn afc0(aop2n: bool, aop1n: bool, aop0n: bool, achn: bool, hch: bool, ha: u32, hrw: bool, wph: bool) -> Afc0Out {
    let aa3n = { let i = (aop2n as u32) << 0 | (aop1n as u32) << 1 | (aop0n as u32) << 2 | (achn as u32) << 3 | (hch as u32) << 4; AFC0_AA3N_T >> i & 1 != 0 };
    let aa2n = { let i = (aop2n as u32) << 0 | (aop1n as u32) << 1 | (aop0n as u32) << 2 | (ha >> 3 & 0x1) << 3 | (ha >> 2 & 0x1) << 4; AFC0_AA2N_T >> i & 1 != 0 };
    let aa1n = { let i = (aop2n as u32) << 0 | (aop1n as u32) << 1 | (aop0n as u32) << 2 | (ha >> 3 & 0x1) << 3 | (ha >> 2 & 0x1) << 4; AFC0_AA1N_T >> i & 1 != 0 };
    let aa0n = { let i = (aop2n as u32) << 0 | (aop1n as u32) << 1 | (aop0n as u32) << 2 | (hrw as u32) << 3 | (wph as u32) << 4; AFC0_AA0N_T >> i & 1 != 0 };
    let awen = !((wph & !aop2n) | (wph & !aop1n) | (wph & aop2n & aop1n & !aop0n & !hrw));
    let cin = !((aop2n) | (aop1n & aop0n) | (!aop1n & !aop0n));
    Afc0Out {
        aa3n,
        aa2n,
        aa1n,
        aa0n,
        awen,
        cin,
    }
}

/// AFC1（PAL16L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Afc1Out {
    /// 組合せの出力：BENN
    pub benn: bool,
    /// 組合せの出力：BA1、BA0
    pub ba: u32,
    /// 組合せの出力：SLOEN
    pub sloen: bool,
    /// 組合せの出力：HWOEN
    pub hwoen: bool,
    /// 組合せの出力：PSCN
    pub pscn: bool,
    /// 組合せの出力：PDCN
    pub pdcn: bool,
    /// 組合せの出力：HRCN
    pub hrcn: bool,
}

/// AFC1（PAL16L8A）。B ファイルの読出し（BEN、番地 BA1・BA0）、A ファイルへのデータ（SLOE：和のラッチ、HWOE：CPU の書込み）、PS・PD・U7–U8 の取込み（PSC、PDC、HRC）
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn afc1(aop2n: bool, aop1n: bool, aop0n: bool, achn: bool, hrw: bool, wph: bool) -> Afc1Out {
    let benn = !(aop2n & !aop1n);
    let ba_1 = !(achn);
    let ba_0 = !((!aop2n) | (aop1n) | (aop0n));
    let sloen = !((wph & !aop2n) | (wph & !aop1n));
    let hwoen = !(wph & aop2n & aop1n & !aop0n & !hrw);
    let pscn = !(aop2n & !aop1n & aop0n);
    let pdcn = !(aop2n & !aop1n & !aop0n);
    let hrcn = !(aop2n & aop1n & !aop0n & hrw);
    Afc1Out {
        benn,
        ba: (ba_1 as u32) << 1 | (ba_0 as u32) << 0,
        sloen,
        hwoen,
        pscn,
        pdcn,
        hrcn,
    }
}

/// RDY の SRDY の真理値表（番地の bit は S0、S1、S2、S3、RDY1、RDY2、OR4、OR10、XRDY の順）。
const RDY_SRDY_T: [u64; 8] = [0x03ef03ed03eb03e9, 0x03ff03fd03fb03f9, 0x07ef07ed07eb07e9, 0x07ff07fd07fb07f9, 0xfbeffbedfbebfbe9, 0xfbfffbfdfbfbfbf9, 0xffefffedffebffe9, 0xfffffffdfffbfff9];

/// RDY の DRDY の真理値表（番地の bit は D0、D1、D2、D3、RDY1、RDY2、IR3、IR5、XRDY の順）。
const RDY_DRDY_T: [u64; 8] = [0x07d707d507d307d1, 0x07df07dd07db07d9, 0x07f707f507f307f1, 0x07ff07fd07fb07f9, 0xffd7ffd5ffd3ffd1, 0xffdfffddffdbffd9, 0xfff7fff5fff3fff1, 0xfffffffdfffbfff9];

/// RDY（PAL20R4A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RdyOut {
    /// 登録出力：SRDY
    pub srdy: bool,
    /// 登録出力：DRDY
    pub drdy: bool,
    /// 組合せの出力：READY
    pub ready: bool,
}

/// RDY（PAL20R4A）。MCLK で取り込む。入力 S3–S0、D3–D0（判断バス）、RDY1、RDY2、OR4、OR10、IR3、IR5、XRDY。READY は両方の準備（今の値）
/// クロックは MCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn rdy(s: u32, d: u32, rdy: u32, or: u32, ir: u32, xrdy: bool, srdy: bool, drdy: bool) -> RdyOut {
    let ready = !((!srdy) | (!drdy));
    let srdy_d = { let i = (s >> 0 & 0xf) << 0 | (rdy >> 1 & 0x3) << 4 | (or >> 4 & 0x1) << 6 | (or >> 10 & 0x1) << 7 | (xrdy as u32) << 8; RDY_SRDY_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let drdy_d = { let i = (d >> 0 & 0xf) << 0 | (rdy >> 1 & 0x3) << 4 | (ir >> 3 & 0x1) << 6 | (ir >> 5 & 0x1) << 7 | (xrdy as u32) << 8; RDY_DRDY_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    RdyOut {
        srdy: srdy_d,
        drdy: drdy_d,
        ready,
    }
}

/// CST（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CstOut {
    /// 組合せの出力：CRUNN
    pub crunn: bool,
    /// 組合せの出力：CRAN
    pub cran: bool,
    /// 組合せの出力：CRLDN
    pub crldn: bool,
    /// 組合せの出力：CARMN
    pub carmn: bool,
    /// 組合せの出力：CK1N
    pub ck1n: bool,
    /// 組合せの出力：CK0N
    pub ck0n: bool,
    /// 組合せの出力：ORLDN
    pub orldn: bool,
    /// 組合せの出力：ORUNN
    pub orunn: bool,
}

/// CST（PAL20L8A）。候補の状態。CHS の段階と、START・STOP の要求の FF から、判断バスのチャネル（DBCH）ともう一方の状態を出す。出力は負の論理
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn cst(p2n: u32, p1n: u32, p0n: u32, strq: u32, stpq: u32, dbchn: bool, armxn: u32) -> CstOut {
    let crunn = !((dbchn & ((p0n & 0x1) == 0x0) & ((p1n & 0x1) == 0x0) & ((stpq & 0x1) == 0x0)) | (!dbchn & ((p0n & 0x2) == 0x0) & ((p1n & 0x2) == 0x0) & ((stpq & 0x2) == 0x0)));
    let cran = !((dbchn & ((p0n & 0x1) == 0x0) & ((p1n & 0x1) == 0x0) & ((p2n & 0x1) == 0x0) & ((stpq & 0x1) == 0x0)) | (!dbchn & ((p0n & 0x2) == 0x0) & ((p1n & 0x2) == 0x0) & ((p2n & 0x2) == 0x0) & ((stpq & 0x2) == 0x0)));
    let crldn = !((dbchn & ((p1n & 0x1) == 0x1) & ((p2n & 0x1) == 0x0) & ((stpq & 0x1) == 0x0)) | (dbchn & ((p0n & 0x1) == 0x1) & ((p2n & 0x1) == 0x0) & ((stpq & 0x1) == 0x0)) | (dbchn & ((stpq & 0x1) == 0x0) & ((strq & 0x1) == 0x1)) | (!dbchn & ((p1n & 0x2) == 0x2) & ((p2n & 0x2) == 0x0) & ((stpq & 0x2) == 0x0)) | (!dbchn & ((p0n & 0x2) == 0x2) & ((p2n & 0x2) == 0x0) & ((stpq & 0x2) == 0x0)) | (!dbchn & ((stpq & 0x2) == 0x0) & ((strq & 0x2) == 0x2)));
    let carmn = !((dbchn & ((armxn & 0x1) == 0x0)) | (!dbchn & ((armxn & 0x2) == 0x0)));
    let ck1n = !((dbchn & ((p1n & 0x1) == 0x0)) | (!dbchn & ((p1n & 0x2) == 0x0)));
    let ck0n = !((dbchn & ((p0n & 0x1) == 0x0)) | (!dbchn & ((p0n & 0x2) == 0x0)));
    let orldn = !((!dbchn & ((p1n & 0x1) == 0x1) & ((p2n & 0x1) == 0x0) & ((stpq & 0x1) == 0x0)) | (!dbchn & ((p0n & 0x1) == 0x1) & ((p2n & 0x1) == 0x0) & ((stpq & 0x1) == 0x0)) | (!dbchn & ((stpq & 0x1) == 0x0) & ((strq & 0x1) == 0x1)) | (dbchn & ((p1n & 0x2) == 0x2) & ((p2n & 0x2) == 0x0) & ((stpq & 0x2) == 0x0)) | (dbchn & ((p0n & 0x2) == 0x2) & ((p2n & 0x2) == 0x0) & ((stpq & 0x2) == 0x0)) | (dbchn & ((stpq & 0x2) == 0x0) & ((strq & 0x2) == 0x2)));
    let orunn = !((!dbchn & ((p0n & 0x1) == 0x0) & ((p1n & 0x1) == 0x0) & ((stpq & 0x1) == 0x0)) | (dbchn & ((p0n & 0x2) == 0x0) & ((p1n & 0x2) == 0x0) & ((stpq & 0x2) == 0x0)));
    CstOut {
        crunn,
        cran,
        crldn,
        carmn,
        ck1n,
        ck0n,
        orldn,
        orunn,
    }
}

/// ELG（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElgOut {
    /// 組合せの出力：GRN
    pub grn: bool,
    /// 組合せの出力：AC2N
    pub ac2n: bool,
    /// 組合せの出力：AC1N
    pub ac1n: bool,
    /// 組合せの出力：AC0N
    pub ac0n: bool,
    /// 組合せの出力：GWN
    pub gwn: bool,
    /// 組合せの出力：KEEPN
    pub keepn: bool,
    /// 組合せの出力：STVN
    pub stvn: bool,
    /// 組合せの出力：STLN
    pub stln: bool,
}

/// ELG（PAL20L8A）。このスロットの頭の判断：A ファイルの操作の種類（AC2–AC0）、新しい要素（GR）、書込み（GW）、要素が残るか（KEEP）、STALL の評価（STV、STL）。出力は負の論理
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn elg(infn: bool, q1n: bool, epcn: bool, crunn: bool, cran: bool, crldn: bool, carmn: bool, ready: bool, sok: bool, addr: bool, fb: bool, cd: bool, hreq: bool) -> ElgOut {
    let grn = !(infn & cd & !cran & !carmn & ready & sok & !fb & !hreq);
    let ac2n = !((infn & cd & !cran & !carmn & ready & sok & !fb & !hreq & !addr) | (!infn & !crunn & q1n & cd & sok & !addr & !epcn & !hreq) | (!infn & !crunn & !q1n & !hreq) | (!crldn & cd & !hreq));
    let ac1n = !((infn & cd & !cran & !carmn & ready & sok & !fb & !hreq & addr) | (!infn & !crunn & q1n & cd & sok & addr & !hreq));
    let ac0n = !((hreq) | (!infn & !crunn & q1n & cd & sok & addr) | (cd & !crldn));
    let gwn = !((!infn & !crunn & q1n & cd & sok & !addr & epcn) | (!infn & !crunn & q1n & cd & sok & !hreq));
    let keepn = !((infn & cd & !cran & !carmn & ready & sok & !fb & !hreq) | (!infn & !crunn & !q1n & hreq) | (!infn & !crunn & q1n & !cd) | (!infn & !crunn & q1n & !sok) | (!infn & !crunn & q1n & addr & hreq) | (!infn & !crunn & q1n & !epcn & hreq) | (!infn & !crunn & q1n & !epcn & addr & !hreq));
    let stvn = !(infn & cd & !cran);
    let stln = !(!carmn & !ready);
    ElgOut {
        grn,
        ac2n,
        ac1n,
        ac0n,
        gwn,
        keepn,
        stvn,
        stln,
    }
}

/// SEQ の DBCHN の真理値表（番地の bit は KEEPN、CRLDN、ORLDN、ORUNN、DBCHN、HOLD の順）。
const SEQ_DBCHN_T: u64 = 0xf7f70808f7770888;

/// SEQ（PAL20R8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeqOut {
    /// 登録出力：DBCHN
    pub dbchn: bool,
    /// 登録出力：ACHN
    pub achn: bool,
    /// 登録出力：AOP2N
    pub aop2n: bool,
    /// 登録出力：AOP1N
    pub aop1n: bool,
    /// 登録出力：AOP0N
    pub aop0n: bool,
    /// 登録出力：ACCRN
    pub accrn: bool,
    /// 登録出力：ACCWN
    pub accwn: bool,
}

/// SEQ（PAL20R8A）。SCLK で判断を登録する：次の候補（DBCH）、判断のチャネル（ACH）、A ファイルの操作（AOP2–AOP0）、決めたアクセス（ACCR、ACCW）。出力は負の論理
/// クロックは SCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn seq(ac2n: bool, ac1n: bool, ac0n: bool, grn: bool, gwn: bool, keepn: bool, ck1n: bool, ck0n: bool, crldn: bool, orldn: bool, orunn: bool, hold: bool, dbchn: bool) -> SeqOut {
    let dbchn_d = { let i = (keepn as u32) << 0 | (crldn as u32) << 1 | (orldn as u32) << 2 | (orunn as u32) << 3 | (dbchn as u32) << 4 | (hold as u32) << 5; SEQ_DBCHN_T >> i & 1 != 0 };
    let achn_d = !(!dbchn);
    let aop2n_d = !(!ac2n);
    let aop1n_d = !((!ac1n) | (!ac2n & ac1n & !ac0n & !ck0n) | (!ac2n & ac1n & !ac0n & !ck1n));
    let aop0n_d = !((!ac0n & ac2n) | (!ac0n & !ac1n) | (!ac0n & ck0n));
    let accrn_d = !(!grn);
    let accwn_d = !(!gwn);
    SeqOut {
        dbchn: dbchn_d,
        achn: achn_d,
        aop2n: aop2n_d,
        aop1n: aop1n_d,
        aop0n: aop0n_d,
        accrn: accrn_d,
        accwn: accwn_d,
    }
}

/// ELS の Q0N の真理値表（番地の bit は ACCWN、AOP2N、DNR、DNW、INFN、Q1N、Q0N の順）。
const ELS_Q0N_T: [u64; 2] = [0xffff5555ffff3333, 0xffff5f5fffff33ff];

/// ELS の TCHN の真理値表（番地の bit は ACCWN、AOP2N、ACHN、INFN、Q1N、TCHN の順）。
const ELS_TCHN_T: u64 = 0xfafafaf850505070;

/// ELS の TLASTN の真理値表（番地の bit は ACCWN、AOP2N、AOP1N、CY、INFN、Q1N、EPCN、ELASTN、TLASTN の順）。
const ELS_TLASTN_T: [u64; 8] = [0x5505550555057704, 0x0000000000002200, 0x5505550555057704, 0x5555555555557744, 0xffafffafffafff8c, 0xaaaaaaaaaaaaaa88, 0xffafffafffafff8c, 0xffffffffffffffcc];

/// ELS（PAL20R8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElsOut {
    /// 登録出力：INFN
    pub infn: bool,
    /// 登録出力：Q1N
    pub q1n: bool,
    /// 登録出力：Q0N
    pub q0n: bool,
    /// 登録出力：EPCN
    pub epcn: bool,
    /// 登録出力：ELASTN
    pub elastn: bool,
    /// 登録出力：TLN
    pub tln: bool,
    /// 登録出力：TCHN
    pub tchn: bool,
    /// 登録出力：TLASTN
    pub tlastn: bool,
}

/// ELS（PAL20R8A）。LCLK で要素の状態を登録する：動いている要素（INF、段階 Q1・Q0、COUNT の更新の待ち EPC、最後 ELAST）と、書込みの残り（TL、TCH、TLAST）。出力は負の論理
/// クロックは LCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn els(accrn: bool, accwn: bool, aop2n: bool, aop1n: bool, cy: bool, dnr: bool, dnw: bool, achn: bool, ken: bool, ktn: bool, infn: bool, q1n: bool, q0n: bool, epcn: bool, elastn: bool, tln: bool, tchn: bool, tlastn: bool) -> ElsOut {
    let infn_d = !((ken & !accrn) | (ken & !infn & accwn & q1n) | (ken & !infn & accwn & aop2n) | (ken & !accwn & !epcn & !aop1n));
    let q1n_d = !((!accwn & !epcn & !aop1n) | (!infn & !q1n & aop2n));
    let q0n_d = { let i = (accwn as u32) << 0 | (aop2n as u32) << 1 | (dnr as u32) << 2 | (dnw as u32) << 3 | (infn as u32) << 4 | (q1n as u32) << 5 | (q0n as u32) << 6; ELS_Q0N_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let epcn_d = !((!accrn & !aop1n) | (accrn & !epcn));
    let elastn_d = !((!accrn & aop1n & !cy) | (accrn & !elastn));
    let tln_d = !((!tln & !dnw & ktn) | (ken & !accwn & epcn) | (ken & !accwn & aop1n) | (ken & !infn & !q1n & !aop2n & q0n & !dnw));
    let tchn_d = { let i = (accwn as u32) << 0 | (aop2n as u32) << 1 | (achn as u32) << 2 | (infn as u32) << 3 | (q1n as u32) << 4 | (tchn as u32) << 5; ELS_TCHN_T >> i & 1 != 0 };
    let tlastn_d = { let i = (accwn as u32) << 0 | (aop2n as u32) << 1 | (aop1n as u32) << 2 | (cy as u32) << 3 | (infn as u32) << 4 | (q1n as u32) << 5 | (epcn as u32) << 6 | (elastn as u32) << 7 | (tlastn as u32) << 8; ELS_TLASTN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    ElsOut {
        infn: infn_d,
        q1n: q1n_d,
        q0n: q0n_d,
        epcn: epcn_d,
        elastn: elastn_d,
        tln: tln_d,
        tchn: tchn_d,
        tlastn: tlastn_d,
    }
}

/// EVT（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvtOut {
    /// 組合せの出力：ERN0、ERN1
    pub ern: u32,
    /// 組合せの出力：KILN0、KILN1
    pub kiln: u32,
    /// 組合せの出力：KEN
    pub ken: bool,
    /// 組合せの出力：KTN
    pub ktn: bool,
    /// 組合せの出力：BLKN0、BLKN1
    pub blkn: u32,
}

/// EVT（PAL20L8A）。このスロットのチャネルの出来事：止める（KIL）、ERROR で止める（ER）、ブロックの終わり（BLK）、要素と書込みの残りのチャネルが止まる（KE、KT）。出力は負の論理
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn evt(stpq: u32, errk: bool, opch: bool, tln: bool, tchn: bool, tlastn: bool, dnw: bool, infn: bool, q1n: bool, q0n: bool, aop2n: bool, cy: bool, achn: bool) -> EvtOut {
    let ern_0 = !(errk & !opch);
    let kiln_0 = !(((stpq & 0x1) == 0x1) | (errk & !opch));
    let kiln_1 = !(((stpq & 0x2) == 0x2) | (errk & opch));
    let ern_1 = !(errk & opch);
    let ken = !((achn & !kiln_0) | (!achn & !kiln_1));
    let ktn = !((tchn & !kiln_0) | (!tchn & !kiln_1));
    let blkn_0 = !((!tln & dnw & !tlastn & tchn & kiln_0) | (!infn & !q1n & !aop2n & !cy & !q0n & achn & kiln_0) | (!infn & !q1n & !aop2n & !cy & dnw & achn & kiln_0));
    let blkn_1 = !((!tln & dnw & !tlastn & !tchn & kiln_1) | (!infn & !q1n & !aop2n & !cy & !q0n & !achn & kiln_1) | (!infn & !q1n & !aop2n & !cy & dnw & !achn & kiln_1));
    EvtOut {
        ern: (ern_0 as u32) << 0 | (ern_1 as u32) << 1,
        kiln: (kiln_0 as u32) << 0 | (kiln_1 as u32) << 1,
        ken,
        ktn,
        blkn: (blkn_0 as u32) << 0 | (blkn_1 as u32) << 1,
    }
}

/// ADC（PAL16L8A-2）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdcOut {
    /// 組合せの出力：RLDN0、RLDN1
    pub rldn: u32,
    /// 組合せの出力：CNTN0、CNTN1
    pub cntn: u32,
    /// 組合せの出力：STEN0、STEN1
    pub sten: u32,
    /// 組合せの出力：SVN0、SVN1
    pub svn: u32,
}

/// ADC（PAL16L8A-2）。AOP と ACH をチャネルごとの信号に直す：再設定（RLD）、COUNT の更新（CNT）、EACH の要素の始め（STE）、STALL の評価（SV）。出力は負の論理
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn adc(aop2n: bool, aop1n: bool, aop0n: bool, achn: bool, accrn: bool, stvn: bool, each: u32, imm: u32) -> AdcOut {
    let rldn_0 = !((!aop2n & !aop1n & achn) | (!aop2n & !aop0n & achn));
    let rldn_1 = !((!aop2n & !aop1n & !achn) | (!aop2n & !aop0n & !achn));
    let cntn_0 = !(!aop2n & aop1n & aop0n & achn);
    let cntn_1 = !(!aop2n & aop1n & aop0n & !achn);
    let sten_0 = !(!accrn & achn & ((each & 0x1) == 0x1) & ((imm & 0x1) == 0x0));
    let sten_1 = !(!accrn & !achn & ((each & 0x2) == 0x2) & ((imm & 0x2) == 0x0));
    let svn_0 = !(!stvn & achn);
    let svn_1 = !(!stvn & !achn);
    AdcOut {
        rldn: (rldn_0 as u32) << 0 | (rldn_1 as u32) << 1,
        cntn: (cntn_0 as u32) << 0 | (cntn_1 as u32) << 1,
        sten: (sten_0 as u32) << 0 | (sten_1 as u32) << 1,
        svn: (svn_0 as u32) << 0 | (svn_1 as u32) << 1,
    }
}

/// CHS の NBSYN の真理値表（番地の bit は RLDN、CY、BLKN、KILN、STRQ、LOOP、P2N、P1N、P0N、OKN の順）。
const CHS_NBSYN_T: [u64; 16] = [0x00ff50ff00ff5fff, 0x00ff50ff00ff5fff, 0x00ff00ff00ff00ff, 0x00fff0ff00ffffff, 0x00ff00ff00ff11ff, 0x00fff0ff00ffffff, 0x00ff00ff00ff00ff, 0x00fff0ff00ffffff, 0x00ff50ff00ff5fff, 0x00ff50ff00ff5fff, 0x00ff00ff00ff00ff, 0x00fff0ff00ffffff, 0x00ff50ff00ff55ff, 0x00fff0ff00ffffff, 0x00ff00ff00ff00ff, 0x00fff0ff00ffffff];

/// CHS の P1N の真理値表（番地の bit は RLDN、CY、BLKN、KILN、P2N、P1N、P0N、OKN の順）。
const CHS_P1N_T: [u64; 4] = [0xffffaaff5fff5fff, 0xffffffff5fff1bff, 0xffffaaff5fff5fff, 0xffffffff5fff5fff];

/// CHS の P0N の真理値表（番地の bit は RLDN、CY、BLKN、KILN、STRQ、P2N、P1N、P0N、OKN の順）。
const CHS_P0N_T: [u64; 8] = [0x0aff5fff0aff5fff, 0x0aff5fff0aff5fff, 0xaaffffffaaffbbff, 0xaaffffffaaffaaff, 0x0aff5fff0aff5fff, 0x0aff5fff0aff5fff, 0xaaffffffaaffffff, 0xaaffffffaaffaaff];

/// CHS の OKN の真理値表（番地の bit は RLDN、OKS、STRQ、P2N、P1N、P0N、OKN の順）。
const CHS_OKN_T: [u64; 2] = [0x1511000015110000, 0xbfbbbfbfbfbfbfbf];

/// CHS（PAL20R6A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChsOut {
    /// 組合せの出力：NBSYN
    pub nbsyn: bool,
    /// 登録出力：P2N
    pub p2n: bool,
    /// 登録出力：P1N
    pub p1n: bool,
    /// 登録出力：P0N
    pub p0n: bool,
    /// 登録出力：OKN
    pub okn: bool,
    /// 登録出力：DONEN
    pub donen: bool,
    /// 登録出力：ERRN
    pub errn: bool,
    /// 組合せの出力：NRUNN
    pub nrunn: bool,
}

/// CHS（PAL20R6A）。LCLK でチャネルの段階（P2–P0：止まっている 000、再設定 100・101・110、動作 111、最後と分かった動作 011）、START の検査（OK）、DONE、ERROR を登録する。CHS0・CHS1 は同じ式。NBSY・NRUN は次の段階
/// クロックは LCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn chs(rldn: bool, cntn: bool, cy: bool, oks: bool, blkn: bool, kiln: bool, ern: bool, strq: bool, loop_: bool, wdq: bool, weq: bool, p2n: bool, p1n: bool, p0n: bool, okn: bool, donen: bool, errn: bool) -> ChsOut {
    let nbsyn = { let i = (rldn as u32) << 0 | (cy as u32) << 1 | (blkn as u32) << 2 | (kiln as u32) << 3 | (strq as u32) << 4 | (loop_ as u32) << 5 | (p2n as u32) << 6 | (p1n as u32) << 7 | (p0n as u32) << 8 | (okn as u32) << 9; CHS_NBSYN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let nrunn = !((kiln & !rldn & !p2n & !p1n & p0n & !okn & cy) | (kiln & rldn & blkn & !p1n & !p0n));
    let p2n_d = !((kiln & !p2n & p1n) | (kiln & strq) | (kiln & !rldn & !p2n & !p1n & p0n & !okn & cy) | (kiln & !rldn & !p2n & !p1n & p0n & !okn & loop_) | (kiln & !blkn & loop_) | (kiln & rldn & !p2n & p0n) | (kiln & rldn & blkn & !p2n & cntn) | (kiln & rldn & blkn & !p2n & cy));
    let p1n_d = { let i = (rldn as u32) << 0 | (cy as u32) << 1 | (blkn as u32) << 2 | (kiln as u32) << 3 | (p2n as u32) << 4 | (p1n as u32) << 5 | (p0n as u32) << 6 | (okn as u32) << 7; CHS_P1N_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let p0n_d = { let i = (rldn as u32) << 0 | (cy as u32) << 1 | (blkn as u32) << 2 | (kiln as u32) << 3 | (strq as u32) << 4 | (p2n as u32) << 5 | (p1n as u32) << 6 | (p0n as u32) << 7 | (okn as u32) << 8; CHS_P0N_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let okn_d = { let i = (rldn as u32) << 0 | (oks as u32) << 1 | (strq as u32) << 2 | (p2n as u32) << 3 | (p1n as u32) << 4 | (p0n as u32) << 5 | (okn as u32) << 6; CHS_OKN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let donen_d = !((!donen & !wdq) | (!rldn & !p2n & !p1n & p0n & !okn & !cy) | (!blkn));
    let errn_d = !((!errn & !weq) | (!rldn & !p2n & !p1n & p0n & okn) | (!ern));
    ChsOut {
        nbsyn,
        p2n: p2n_d,
        p1n: p1n_d,
        p0n: p0n_d,
        okn: okn_d,
        donen: donen_d,
        errn: errn_d,
        nrunn,
    }
}

/// STS の PENDN の真理値表（番地の bit は NBSYN、NRUNN、STRQ、STEN、RISEN、ARMN、PENDN の順）。
const STS_PENDN_T: [u64; 2] = [0xf3f3b3b3c0f380b3, 0xffffbbbbffff88bb];

/// STS（PAL20R6A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StsOut {
    /// 組合せの出力：ARMXN
    pub armxn: bool,
    /// 登録出力：SYN2N
    pub syn2n: bool,
    /// 登録出力：RISEN
    pub risen: bool,
    /// 登録出力：ARMN
    pub armn: bool,
    /// 登録出力：PENDN
    pub pendn: bool,
    /// 登録出力：MISSN
    pub missn: bool,
    /// 登録出力：STALN
    pub staln: bool,
    /// 組合せの出力：WAITN
    pub waitn: bool,
}

/// STS（PAL20R6A）。LCLK でトリガの同期化の 2 段目（SYN2）と立上り（RISE）、許可（ARM）、保留（PEND）、MISS、STALL を登録する。STS0・STS1 は同じ式。ARMX・WAIT は今の値
/// クロックは LCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn sts(s1q: bool, nbsyn: bool, nrunn: bool, strq: bool, sten: bool, wmq: bool, imm: bool, svn: bool, stln: bool, p1n: bool, p0n: bool, syn2n: bool, risen: bool, armn: bool, pendn: bool, missn: bool, staln: bool) -> StsOut {
    let armxn = !((imm) | (!armn));
    let waitn = !(!p1n & !p0n & !imm & armn);
    let syn2n_d = !(s1q);
    let risen_d = !(s1q & syn2n);
    let armn_d = !((!nrunn & !armn & sten) | (!nrunn & !pendn) | (!nrunn & !risen));
    let pendn_d = { let i = (nbsyn as u32) << 0 | (nrunn as u32) << 1 | (strq as u32) << 2 | (sten as u32) << 3 | (risen as u32) << 4 | (armn as u32) << 5 | (pendn as u32) << 6; STS_PENDN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let missn_d = !((!missn & !wmq) | (!risen & !nbsyn & !pendn & !strq));
    let staln_d = !((!nrunn & !svn & !stln) | (!nrunn & svn & !staln));
    StsOut {
        armxn,
        syn2n: syn2n_d,
        risen: risen_d,
        armn: armn_d,
        pendn: pendn_d,
        missn: missn_d,
        staln: staln_d,
        waitn,
    }
}

/// MSK0（63S281A）：出力の bit 0–7 の有効なビット。番地は SHIFT（bit 4–0）と FORMAT（bit 7–5）。
pub const MSK0: [u8; 256] = [
    0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE0, 0xC0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x03, 0x07, 0x0F, 0x1F, 0x3F, 0x7F,
    0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE0, 0xC0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x01, 0x03, 0x07, 0x0F, 0x1F, 0x3F, 0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE0, 0xC0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x03, 0x07, 0x0F, 0x1F, 0x3F, 0x7F,
    0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE0, 0xC0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x03, 0x07, 0x0F, 0x1F, 0x3F, 0x7F,
    0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE0, 0xC0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x03, 0x07, 0x0F, 0x1F, 0x3F, 0x7F,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// MSK1（63S281A）：出力の bit 8–15 の有効なビット。番地は MSK0 と同じ。
pub const MSK1: [u8; 256] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE0, 0xC0, 0x80,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x03, 0x07, 0x0F, 0x1F, 0x3F, 0x7F,
    0x00, 0x01, 0x03, 0x07, 0x0F, 0x1F, 0x3F, 0x7F, 0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE0, 0xC0, 0x80,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// ATR（63S281A）：ポートの属性。番地は bit 3–0 ポート、bit 4 向き（1 で書込み）、bit 7–5 FORMAT。
pub const ATR: [u8; 256] = [
    0x1B, 0x04, 0x0E, 0x04, 0x04, 0x04, 0x00, 0x18, 0x1A, 0x9A, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C,
    0x1B, 0x04, 0x0E, 0x04, 0x04, 0x1C, 0x18, 0x00, 0x02, 0x82, 0x04, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C,
    0x3B, 0x3C, 0x26, 0x24, 0x3C, 0x24, 0x20, 0x20, 0x22, 0xA2, 0x24, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0x3B, 0x3C, 0x36, 0x3C, 0x24, 0x24, 0x20, 0x20, 0x22, 0xA2, 0x24, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0x1B, 0x04, 0x0E, 0x04, 0x04, 0x04, 0x00, 0x18, 0x1A, 0x9A, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C,
    0x3B, 0x3C, 0x36, 0x3C, 0x24, 0x24, 0x20, 0x20, 0x22, 0xA2, 0x24, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0x3B, 0x3C, 0x26, 0x24, 0x3C, 0x24, 0x20, 0x20, 0x22, 0xA2, 0x24, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0x1B, 0x04, 0x0E, 0x04, 0x04, 0x1C, 0x18, 0x00, 0x02, 0x82, 0x04, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C,
    0x7B, 0x7C, 0x66, 0x64, 0x7C, 0x64, 0x60, 0x60, 0x62, 0xE2, 0x64, 0x7C, 0x7C, 0x7C, 0x7C, 0x7C,
    0x5B, 0x44, 0x4E, 0x44, 0x44, 0x5C, 0x58, 0x40, 0x42, 0xC2, 0x44, 0x5C, 0x5C, 0x5C, 0x5C, 0x5C,
    0x3B, 0x3C, 0x26, 0x24, 0x3C, 0x24, 0x20, 0x20, 0x22, 0xA2, 0x24, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0x3B, 0x3C, 0x36, 0x3C, 0x24, 0x24, 0x20, 0x20, 0x22, 0xA2, 0x24, 0x3C, 0x3C, 0x3C, 0x3C, 0x3C,
    0x03, 0x04, 0x06, 0x04, 0x04, 0x04, 0x00, 0x00, 0x02, 0x82, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x03, 0x04, 0x06, 0x04, 0x04, 0x04, 0x00, 0x00, 0x02, 0x82, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x03, 0x04, 0x06, 0x04, 0x04, 0x04, 0x00, 0x00, 0x02, 0x82, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x03, 0x04, 0x06, 0x04, 0x04, 0x04, 0x00, 0x00, 0x02, 0x82, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
];

/// ATR の出力 SONLY（bit 0）。
pub const ATR_SONLY: u8 = 1 << 0;
/// ATR の出力 ADDR（bit 1）。
pub const ATR_ADDR: u8 = 1 << 1;
/// ATR の出力 FLAG（bit 2）。
pub const ATR_FLAG: u8 = 1 << 2;
/// ATR の出力 OKD（bit 3）。
pub const ATR_OKD: u8 = 1 << 3;
/// ATR の出力 OKN（bit 4）。
pub const ATR_OKN: u8 = 1 << 4;
/// ATR の出力 W16（bit 5）。
pub const ATR_W16: u8 = 1 << 5;
/// ATR の出力 F4（bit 6）。
pub const ATR_F4: u8 = 1 << 6;
/// ATR の出力 TWO（bit 7）。
pub const ATR_TWO: u8 = 1 << 7;
