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

/// AFC0（PAL20R6A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Afc0Out {
    /// 登録出力：AA3N
    pub aa3n: bool,
    /// 登録出力：AA2N
    pub aa2n: bool,
    /// 登録出力：AA1N
    pub aa1n: bool,
    /// 登録出力：AR0N
    pub ar0n: bool,
    /// 登録出力：AW0N
    pub aw0n: bool,
    /// 登録出力：AWN
    pub awn: bool,
    /// 組合せの出力：CIN
    pub cin: bool,
}

/// AFC0（PAL20R6A）。SCLK で A ファイルの番地を登録する（SEQ と同じ入力：ELG の AC2–AC0、CST の CK1・CK0、次の ACH ＝ DBCH）。語の bit 3–1（AA3N–AA1N、負の論理のまま 74S189 へ）、語の bit 0 の読みの側（AR0N）と書きの側（AW0N。74S157 の U64 が PH1 で、読み（q0〜q1）と書き（q2〜q3）を選んで AA0N にする）、書込みのある操作（AWN。TIM の AWQN へ）。CIN は加算器の桁上げの入力
/// クロックは SCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn afc0(ac2n: bool, ac1n: bool, ac0n: bool, ck1n: bool, ck0n: bool, dbchn: bool, hch: bool, ha: u32, hrw: bool, aa2n: bool, ar0n: bool) -> Afc0Out {
    let cin = !((!ar0n) | (!aa2n));
    let aa3n_d = !((!ac2n & !dbchn) | (!ac1n & !dbchn) | (ac2n & ac1n & hch));
    let aa2n_d = !((!ac2n & !ck1n) | (!ac2n & ac0n) | (ac2n & ac1n & ((ha & 0xc) == 0xc)));
    let aa1n_d = !((!ac1n & !ac0n) | (!ac2n & !ac0n & !ck0n) | (ac2n & ac1n & ((ha & 0x4) == 0x0)));
    let ar0n_d = !((ac0n) | (ac2n));
    let aw0n_d = !((!ac2n) | (!ac1n));
    let awn_d = !((!ac2n) | (!ac1n) | (!ac0n & !hrw));
    Afc0Out {
        aa3n: aa3n_d,
        aa2n: aa2n_d,
        aa1n: aa1n_d,
        ar0n: ar0n_d,
        aw0n: aw0n_d,
        awn: awn_d,
        cin,
    }
}

/// AFC1（PAL16R8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Afc1Out {
    /// 登録出力：BENN
    pub benn: bool,
    /// 登録出力：BA1、BA0
    pub ba: u32,
    /// 登録出力：SLOEN
    pub sloen: bool,
    /// 登録出力：HWOEN
    pub hwoen: bool,
    /// 登録出力：PSCN
    pub pscn: bool,
    /// 登録出力：PDCN
    pub pdcn: bool,
    /// 登録出力：HRCN
    pub hrcn: bool,
}

/// AFC1（PAL16R8A）。SCLK で B ファイルの読出し（BEN、番地 BA1・BA0）、A ファイルへのデータ（SLOE：和のラッチ、HWOE：CPU の書込み）、PS・PD・U7–U8 の取込み（PSC、PDC、HRC）を登録する。入力は AFC0 と同じ
/// クロックは SCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn afc1(ac2n: bool, ac1n: bool, ac0n: bool, dbchn: bool, hrw: bool) -> Afc1Out {
    let benn_d = !(ac2n & !ac1n);
    let ba_1_d = !(dbchn);
    let ba_0_d = !(ac0n);
    let sloen_d = !((!ac2n) | (!ac1n));
    let hwoen_d = !(ac2n & ac1n & !ac0n & !hrw);
    let pscn_d = !(ac2n & !ac1n & ac0n);
    let pdcn_d = !(ac2n & !ac1n & !ac0n);
    let hrcn_d = !(ac2n & ac1n & !ac0n & hrw);
    Afc1Out {
        benn: benn_d,
        ba: (ba_1_d as u32) << 1 | (ba_0_d as u32) << 0,
        sloen: sloen_d,
        hwoen: hwoen_d,
        pscn: pscn_d,
        pdcn: pdcn_d,
        hrcn: hrcn_d,
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
pub fn elg(infn: bool, q1n: bool, epcn: bool, crunn: bool, cran: bool, crldn: bool, carmn: bool, ready: bool, goks: bool, addrs: bool, sokd: bool, cd: bool, hreq: bool, addrd: bool) -> ElgOut {
    let grn = !(infn & cd & !cran & !carmn & ready & goks & !hreq);
    let ac2n = !((infn & cd & !cran & !carmn & ready & goks & !hreq & !addrs) | (!infn & !crunn & q1n & cd & sokd & !addrd & !epcn & !hreq) | (!infn & !crunn & !q1n & !hreq) | (!crldn & cd & !hreq));
    let ac1n = !((infn & cd & !cran & !carmn & ready & goks & !hreq & addrs) | (!infn & !crunn & q1n & cd & sokd & addrd & !hreq));
    let ac0n = !((hreq) | (!infn & !crunn & q1n & cd & sokd & addrd) | (cd & !crldn));
    let gwn = !((!infn & !crunn & q1n & cd & sokd & !addrd & epcn) | (!infn & !crunn & q1n & cd & sokd & !hreq));
    let keepn = !((infn & cd & !cran & !carmn & ready & goks & !hreq) | (!infn & !crunn & !q1n & hreq) | (!infn & !crunn & q1n & !cd) | (!infn & !crunn & q1n & !sokd) | (!infn & !crunn & q1n & addrd & hreq) | (!infn & !crunn & q1n & !epcn & hreq) | (!infn & !crunn & q1n & !epcn & addrd & !hreq));
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
pub fn evt(stpq: u32, errkn: bool, opch: bool, tln: bool, tchn: bool, tlastn: bool, dnw: bool, infn: bool, q1n: bool, q0n: bool, aop2n: bool, cy: bool, achn: bool) -> EvtOut {
    let ern_0 = !(!errkn & !opch);
    let kiln_0 = !(((stpq & 0x1) == 0x1) | (!errkn & !opch));
    let kiln_1 = !(((stpq & 0x2) == 0x2) | (!errkn & opch));
    let ern_1 = !(!errkn & opch);
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

/// CHS の NBSYXN の真理値表（番地の bit は RLDN、KILN、STRQ、LOOP、P2N、P1N、P0N、OKN の順）。
const CHS_NBSYXN_T: [u64; 4] = [0x3f3f3333373f373f, 0x3f3f33333f3f3337, 0x3f3f3333373f373f, 0x3f3f33333f3f3737];

/// CHS の P1N の真理値表（番地の bit は RLDN、CY、BLKN、KILN、P2N、P1N、P0N、OKN の順）。
const CHS_P1N_T: [u64; 4] = [0xffffaaff5fff5fff, 0xffffffff5fff1bff, 0xffffaaff5fff5fff, 0xffffffff5fff5fff];

/// CHS の P0N の真理値表（番地の bit は RLDN、CY、BLKN、KILN、STRQ、P2N、P1N、P0N、OKN の順）。
const CHS_P0N_T: [u64; 8] = [0x0aff5fff0aff5fff, 0x0aff5fff0aff5fff, 0xaaffffffaaffbbff, 0xaaffffffaaffaaff, 0x0aff5fff0aff5fff, 0x0aff5fff0aff5fff, 0xaaffffffaaffffff, 0xaaffffffaaffaaff];

/// CHS の OKN の真理値表（番地の bit は RLDN、OKS、STRQ、P2N、P1N、P0N、OKN の順）。
const CHS_OKN_T: [u64; 2] = [0x1511000015110000, 0xbfbbbfbfbfbfbfbf];

/// CHS（PAL20R6A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChsOut {
    /// 組合せの出力：NBSYXN
    pub nbsyxn: bool,
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
    /// 組合せの出力：NRUN0N
    pub nrun0n: bool,
}

/// CHS（PAL20R6A）。LCLK でチャネルの段階（P2–P0：止まっている 000、再設定 100・101・110、動作 111、最後と分かった動作 011）、START の検査（OK）、DONE、ERROR を登録する。CHS0・CHS1 は同じ式。NBSYX・NRUN0 は次の段階の、ブロックの終わり（BLK）を除いた部分：次が止まっていない ＝ NBSYX ∨ NRUN0∧¬BLK、次が動作 ＝ NRUN0∧¬BLK（STS で合わせる）
/// クロックは LCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn chs(rldn: bool, cntn: bool, cy: bool, oks: bool, blkn: bool, kiln: bool, ern: bool, strq: bool, loop_: bool, wdq: bool, weq: bool, p2n: bool, p1n: bool, p0n: bool, okn: bool, donen: bool, errn: bool) -> ChsOut {
    let nbsyxn = { let i = (rldn as u32) << 0 | (kiln as u32) << 1 | (strq as u32) << 2 | (loop_ as u32) << 3 | (p2n as u32) << 4 | (p1n as u32) << 5 | (p0n as u32) << 6 | (okn as u32) << 7; CHS_NBSYXN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let nrun0n = !((kiln & !rldn & !p2n & !p1n & p0n & !okn & cy) | (kiln & rldn & !p1n & !p0n));
    let p2n_d = !((kiln & !p2n & p1n) | (kiln & strq) | (kiln & !rldn & !p2n & !p1n & p0n & !okn & cy) | (kiln & !rldn & !p2n & !p1n & p0n & !okn & loop_) | (kiln & !blkn & loop_) | (kiln & rldn & !p2n & p0n) | (kiln & rldn & blkn & !p2n & cntn) | (kiln & rldn & blkn & !p2n & cy));
    let p1n_d = { let i = (rldn as u32) << 0 | (cy as u32) << 1 | (blkn as u32) << 2 | (kiln as u32) << 3 | (p2n as u32) << 4 | (p1n as u32) << 5 | (p0n as u32) << 6 | (okn as u32) << 7; CHS_P1N_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let p0n_d = { let i = (rldn as u32) << 0 | (cy as u32) << 1 | (blkn as u32) << 2 | (kiln as u32) << 3 | (strq as u32) << 4 | (p2n as u32) << 5 | (p1n as u32) << 6 | (p0n as u32) << 7 | (okn as u32) << 8; CHS_P0N_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let okn_d = { let i = (rldn as u32) << 0 | (oks as u32) << 1 | (strq as u32) << 2 | (p2n as u32) << 3 | (p1n as u32) << 4 | (p0n as u32) << 5 | (okn as u32) << 6; CHS_OKN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let donen_d = !((!donen & !wdq) | (!rldn & !p2n & !p1n & p0n & !okn & !cy) | (!blkn));
    let errn_d = !((!errn & !weq) | (!rldn & !p2n & !p1n & p0n & okn) | (!ern));
    ChsOut {
        nbsyxn,
        p2n: p2n_d,
        p1n: p1n_d,
        p0n: p0n_d,
        okn: okn_d,
        donen: donen_d,
        errn: errn_d,
        nrun0n,
    }
}

/// STS の PENDN の真理値表（番地の bit は NBSYXN、NRUN0N、STRQ、STEN、RISEN、ARMN、PENDN、BLKN の順）。
const STS_PENDN_T: [u64; 4] = [0xf0f0a0a0f0f0a0a0, 0xffffaaaaffffaaaa, 0xf3f3b3b3c0f380b3, 0xffffbbbbffff88bb];

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

/// STS（PAL20R6A）。LCLK でトリガの同期化の 2 段目（SYN2）と立上り（RISE）、許可（ARM）、保留（PEND）、MISS、STALL を登録する。STS0・STS1 は同じ式。次が動作（NRUN0∧¬BLK）と次が止まっていない（NBSYX ∨ NRUN0∧¬BLK）は CHS の NBSYX・NRUN0 と EVT の BLK から（BLK から CHS を通さない）。ARMX・WAIT は今の値
/// クロックは LCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn sts(s1q: bool, nbsyxn: bool, nrun0n: bool, strq: bool, sten: bool, wmq: bool, imm: bool, svn: bool, stln: bool, p1n: bool, p0n: bool, blkn: bool, syn2n: bool, risen: bool, armn: bool, pendn: bool, missn: bool, staln: bool) -> StsOut {
    let armxn = !((imm) | (!armn));
    let waitn = !(!p1n & !p0n & !imm & armn);
    let syn2n_d = !(s1q);
    let risen_d = !(s1q & syn2n);
    let armn_d = !((!nrun0n & blkn & !armn & sten) | (!nrun0n & blkn & !pendn) | (!nrun0n & blkn & !risen));
    let pendn_d = { let i = (nbsyxn as u32) << 0 | (nrun0n as u32) << 1 | (strq as u32) << 2 | (sten as u32) << 3 | (risen as u32) << 4 | (armn as u32) << 5 | (pendn as u32) << 6 | (blkn as u32) << 7; STS_PENDN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let missn_d = !((!missn & !wmq) | (!risen & !nbsyxn & !pendn & !strq) | (!risen & !nrun0n & blkn & !pendn & !strq));
    let staln_d = !((!nrun0n & blkn & !svn & !stln) | (!nrun0n & blkn & svn & !staln));
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

/// TIM の SX の真理値表（番地の bit は RESN、PH1、PH0、SX の順）。
const TIM_SX_T: u64 = 0x0000000000002a80;

/// TIM（PAL16R8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimOut {
    /// 登録出力：PH1、PH0
    pub ph: u32,
    /// 登録出力：SX
    pub sx: bool,
    /// 登録出力：SCLK
    pub sclk: bool,
    /// 登録出力：MCLK
    pub mclk: bool,
    /// 登録出力：LCLK
    pub lclk: bool,
    /// 登録出力：WPHN
    pub wphn: bool,
    /// 登録出力：AWQN
    pub awqn: bool,
}

/// TIM（PAL16R8A）。16 MHz の位相（PH1・PH0：q0〜q3 が 0〜3）、S・X（SX＝1 で今のスロットが S）、SCLK（q0 の縁で立つ）、MCLK・LCLK（q2 の縁で立つ。同じ信号を負荷で分ける）、WPHN（q3 の間 Low：次の 16 MHz の縁が q0）、A ファイルの書込みの相（AWQN：AW のとき q3 の間 Low。74S32 で C16 と OR して /AWE）。RESN で q3・X に戻す
/// クロックは C16。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn tim(resn: bool, awn: bool, ph: u32, sx: bool) -> TimOut {
    let ph_1_d = !((resn & ((ph & 0x3) == 0x0)) | (resn & ((ph & 0x3) == 0x3)));
    let ph_0_d = !(resn & ((ph & 0x1) == 0x1));
    let sx_d = { let i = (resn as u32) << 0 | (ph >> 1 & 0x1) << 1 | (ph >> 0 & 0x1) << 2 | (sx as u32) << 3; TIM_SX_T >> i & 1 != 0 };
    let sclk_d = !((!resn) | ((ph & 0x2) == 0x0) | ((ph & 0x1) == 0x0));
    let mclk_d = !((!resn) | ((ph & 0x2) == 0x2) | ((ph & 0x1) == 0x0));
    let lclk_d = !((!resn) | ((ph & 0x2) == 0x2) | ((ph & 0x1) == 0x0));
    let wphn_d = !((!resn) | ((ph & 0x3) == 0x2));
    let awqn_d = !(resn & !awn & ((ph & 0x3) == 0x2));
    TimOut {
        ph: (ph_1_d as u32) << 1 | (ph_0_d as u32) << 0,
        sx: sx_d,
        sclk: sclk_d,
        mclk: mclk_d,
        lclk: lclk_d,
        wphn: wphn_d,
        awqn: awqn_d,
    }
}

/// DPA（PAL16L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DpaOut {
    /// 組合せの出力：GOKS
    pub goks: bool,
    /// 組合せの出力：ADDRS
    pub addrs: bool,
    /// 組合せの出力：SOKD
    pub sokd: bool,
    /// 組合せの出力：ADDRD
    pub addrd: bool,
}

/// DPA（PAL16L8A）。判断バスの読出し元（S3–S0）と書込み先（D3–D0）の属性：新しい要素を始めてよい（GOKS：S のスロットの規則と FLAG の待ち）、番地を持つ（ADDRS、ADDRD）、書込み先の S の規則（SOKD）。X は今が X のスロット
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn dpa(s: u32, d: u32, x: bool, opv: bool, opfl: bool, opvd: bool, nxfl: bool) -> DpaOut {
    let goks = !((!x & ((s & 0xf) == 0x0)) | (opv & opfl) | (opvd & nxfl));
    let addrs = !(((s & 0x4) == 0x4) | ((s & 0x9) == 0x1) | ((s & 0xa) == 0xa));
    let sokd = !(!x & ((d & 0xf) == 0x0));
    let addrd = !(((d & 0x4) == 0x4) | ((d & 0x9) == 0x1) | ((d & 0xa) == 0xa));
    DpaOut {
        goks,
        addrs,
        sokd,
        addrd,
    }
}

/// OPA の CD の真理値表（番地の bit は NXV、NXCH、NXTW、NXLG、OPV、OPCH、OPTW、ODN、STPQ0、STPQ1 の順）。
const OPA_CD_T: [u64; 16] = [0x0000555f0000555f, 0x555f555f555f555f, 0x555f555f555f555f, 0x555f555f555f555f, 0x0000777f777f777f, 0x777f777f777f777f, 0x777f777f777f777f, 0x777f777f777f777f, 0xdddfdddf0000dddf, 0xdddfdddfdddfdddf, 0xdddfdddfdddfdddf, 0xdddfdddfdddfdddf, 0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff];

/// OPA の SDN の真理値表（番地の bit は NXV、NXCH、NXTW、NXLG、OPV、OPCH、OPTW、ODN、STPQ0、STPQ1 の順）。
const OPA_SDN_T: [u64; 16] = [0xfff5fff5fff5fff5, 0x0000fff50000fff5, 0xfff5fff5fff5fff5, 0xfff5fff5fff5fff5, 0xfff7fff7fff7fff7, 0x0000fff7fff7fff7, 0xfff7fff7fff7fff7, 0xfff7fff7fff7fff7, 0xfffdfffdfffdfffd, 0xfffdfffd0000fffd, 0xfffdfffdfffdfffd, 0xfffdfffdfffdfffd, 0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff, 0xffffffffffffffff];

/// OPA の LAN の真理値表（番地の bit は NXV、NXCH、NXLG、OPV、OPCH、OPLG、ODN、STPQ0、STPQ1 の順）。
const OPA_LAN_T: [u64; 8] = [0x005f005f5f5f5f5f, 0x5f5f5f5f5f5f5f5f, 0x007f7f7f7f7f7f7f, 0x7f7f7f7f7f7f7f7f, 0xdfdf00dfdfdfdfdf, 0xdfdfdfdfdfdfdfdf, 0xffffffffffffffff, 0xffffffffffffffff];

/// OPA（PAL20R4A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpaOut {
    /// 組合せの出力：OPLDN
    pub opldn: bool,
    /// 組合せの出力：OPVD
    pub opvd: bool,
    /// 登録出力：SDN
    pub sdn: bool,
    /// 登録出力：LAN
    pub lan: bool,
    /// 組合せの出力：CD
    pub cd: bool,
    /// 組合せの出力：ABTN
    pub abtn: bool,
}

/// OPA（PAL20R4A）。SCLK の前に OP が空くか（OPLD：OP の 74LS377 の許可）、NX のアクセスが STOP で消えないか（OPVD：OP の有効の入力）、判断してよいか（CD）、延びたアクセスの打切り（ABT）。SCLK で、このスロットで終わる 1 スロットのアクセス（SD）と延びたアクセス（LA）を登録する
/// クロックは SCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn opa(nxv: bool, nxch: bool, nxtw: bool, nxlg: bool, opv: bool, opch: bool, optw: bool, oplg: bool, odn: bool, stpq: u32, tor: bool) -> OpaOut {
    let opldn = !((odn) | (!opv) | (!opch & ((stpq & 0x1) == 0x1)) | (opch & ((stpq & 0x2) == 0x2)));
    let opvd = !((!nxv) | (nxch & ((stpq & 0x2) == 0x2)) | (!nxch & ((stpq & 0x1) == 0x1)));
    let cd = { let i = (nxv as u32) << 0 | (nxch as u32) << 1 | (nxtw as u32) << 2 | (nxlg as u32) << 3 | (opv as u32) << 4 | (opch as u32) << 5 | (optw as u32) << 6 | (odn as u32) << 7 | (stpq >> 0 & 0x3) << 8; OPA_CD_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let abtn = !((tor) | (opv & opch & oplg & !odn & ((stpq & 0x2) == 0x2)) | (opv & !opch & oplg & !odn & ((stpq & 0x1) == 0x1)));
    let sdn_d = { let i = (nxv as u32) << 0 | (nxch as u32) << 1 | (nxtw as u32) << 2 | (nxlg as u32) << 3 | (opv as u32) << 4 | (opch as u32) << 5 | (optw as u32) << 6 | (odn as u32) << 7 | (stpq >> 0 & 0x3) << 8; OPA_SDN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let lan_d = { let i = (nxv as u32) << 0 | (nxch as u32) << 1 | (nxlg as u32) << 2 | (opv as u32) << 3 | (opch as u32) << 4 | (oplg as u32) << 5 | (odn as u32) << 6 | (stpq >> 0 & 0x3) << 7; OPA_LAN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    OpaOut {
        opldn,
        opvd,
        sdn: sdn_d,
        lan: lan_d,
        cd,
        abtn,
    }
}

/// OPB の DNW の真理値表（番地の bit は SDN、OPW、P1RN、P2RN、P1MN、INFN、Q1N、TLN、TLASTN の順）。
const OPB_DNW_T: [u64; 8] = [0x40c04ccc4ccc4ccc, 0x40c04ccc40c04ccc, 0x4ccc4ccc4ccc4ccc, 0x4ccc4ccc4ccc4ccc, 0x4ccc00004ccc4ccc, 0x4ccc00004ccc0000, 0x4ccc4ccc4ccc4ccc, 0x4ccc4ccc4ccc4ccc];

/// OPB（PAL20R4A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpbOut {
    /// 組合せの出力：DNR
    pub dnr: bool,
    /// 登録出力：ODN
    pub odn: bool,
    /// 登録出力：TOR
    pub tor: bool,
    /// 登録出力：DNRQN
    pub dnrqn: bool,
    /// 組合せの出力：DNW
    pub dnw: bool,
    /// 組合せの出力：ERRKN
    pub errkn: bool,
}

/// OPB（PAL20R4A）。LCLK の前の、このスロットのアクセスの結果：読出し・書込みの完了（DNR、DNW）、ERROR（ERRK：port 1 の DONE の食い違いか時間切れ）。LCLK で、OP のアクセスが終わった（ODN）、時間切れ（TOR）、読出しの完了（DNRQ：IL が次の SCLK で取り込む許可）を登録する。P1R・P2R・P1M は P1A・P2A の答え（q0 の縁で登録）と DONE の印、TOUT は U38
/// クロックは LCLK。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn opb(sdn: bool, lan: bool, opw: bool, p1rn: bool, p2rn: bool, p1mn: bool, tout: bool, infn: bool, q1n: bool, epcn: bool, elastn: bool, tln: bool, tlastn: bool) -> OpbOut {
    let dnr = !((opw) | (sdn & p1rn & p2rn) | (!p1mn & !infn & q1n & epcn & elastn) | (!p1rn & p1mn & !infn & q1n & epcn & !elastn));
    let dnw = { let i = (sdn as u32) << 0 | (opw as u32) << 1 | (p1rn as u32) << 2 | (p2rn as u32) << 3 | (p1mn as u32) << 4 | (infn as u32) << 5 | (q1n as u32) << 6 | (tln as u32) << 7 | (tlastn as u32) << 8; OPB_DNW_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let errkn = !((!lan & p1rn & p2rn & tout) | (opw & !p1mn & infn & !tln & tlastn) | (opw & !p1rn & p1mn & infn & !tln & !tlastn) | (opw & !p1mn & q1n & !tln & tlastn) | (opw & !p1rn & p1mn & q1n & !tln & !tlastn) | (!opw & !p1mn & !infn & q1n & epcn & elastn) | (!opw & !p1rn & p1mn & !infn & q1n & epcn & !elastn));
    let odn_d = !((sdn & p1rn & p2rn & lan) | (sdn & p1rn & p2rn & !tout));
    let tor_d = !((lan) | (!p1rn) | (!p2rn) | (!tout));
    let dnrqn_d = !(dnr);
    OpbOut {
        dnr,
        odn: odn_d,
        tor: tor_d,
        dnrqn: dnrqn_d,
        dnw,
        errkn,
    }
}

/// STB0 の XSN の真理値表（番地の bit は OPV、OPP0、OPP1、OPP2、OPP3 の順）。
const STB0_XSN_T: u64 = 0x00000000555dffff;

/// STB0（PAL16L8A-2）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stb0Out {
    /// 組合せの出力：RQ0N
    pub rq0n: bool,
    /// 組合せの出力：RQ9N
    pub rq9n: bool,
    /// 組合せの出力：ABHN
    pub abhn: bool,
    /// 組合せの出力：ABLN
    pub abln: bool,
    /// 組合せの出力：ABSN
    pub absn: bool,
    /// 組合せの出力：T67N
    pub t67n: bool,
    /// 組合せの出力：XSN
    pub xsn: bool,
    /// 組合せの出力：WEN
    pub wen: bool,
}

/// STB0（PAL16L8A-2）。A 基板のポート（0、1、9）の要求と緩衝器（U45 上位、U46 下位、U47 下位と上位の入替え）、Temporal RAM の緩衝器（U56）、拡張（U57、port 8・10〜15）、ANX の出力の許可（WEN）。OP から（OPWORD は OP の W16）
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn stb0(opv: bool, opw: bool, opp: u32, opword: bool, pa: u32) -> Stb0Out {
    let rq0n = !(opv & ((opp & 0xf) == 0x0));
    let rq9n = !(opv & ((opp & 0xf) == 0x9));
    let abhn = !(opv & opword & ((opp & 0xe) == 0x0));
    let abln = !((opv & opword & ((opp & 0xe) == 0x0)) | (opv & !opword & ((opp & 0xf) == 0x0) & ((pa & 0x1) == 0x1)));
    let absn = !((opv & !opword & ((opp & 0xf) == 0x0) & ((pa & 0x1) == 0x0)) | (opv & !opword & ((opp & 0xf) == 0x9)));
    let t67n = !(opv & ((opp & 0xe) == 0x6));
    let xsn = { let i = (opv as u32) << 0 | (opp >> 0 & 0xf) << 1; STB0_XSN_T >> i & 1 != 0 };
    let wen = !((!opv) | (!opw));
    Stb0Out {
        rq0n,
        rq9n,
        abhn,
        abln,
        absn,
        t67n,
        xsn,
        wen,
    }
}

/// STB1（PAL16L8A-2）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stb1Out {
    /// 組合せの出力：SIDN
    pub sidn: bool,
    /// 組合せの出力：SI3N
    pub si3n: bool,
    /// 組合せの出力：SI5N
    pub si5n: bool,
    /// 組合せの出力：SO4N
    pub so4n: bool,
    /// 組合せの出力：OE4N
    pub oe4n: bool,
    /// 組合せの出力：T6WN
    pub t6wn: bool,
    /// 組合せの出力：T7RN
    pub t7rn: bool,
}

/// STB1（PAL16L8A-2）。B 基板の FIFO と Temporal RAM のアクセスの信号：本体 DAC の FIFO（port 2、DOC 子基板がないとき）・port 3・5 の SI（q2〜q3）、port 4 の SO（q3）と出力の緩衝器（U51–U52）、Temporal RAM の書込み（port 6、q2〜q3）と読出し（port 7）
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn stb1(opv: bool, opp: u32, doc: bool, ph: u32, wphn: bool) -> Stb1Out {
    let sidn = !(opv & !doc & ((opp & 0xf) == 0x2) & ((ph & 0x2) == 0x2));
    let si3n = !(opv & ((opp & 0xf) == 0x3) & ((ph & 0x2) == 0x2));
    let si5n = !(opv & ((opp & 0xf) == 0x5) & ((ph & 0x2) == 0x2));
    let so4n = !(opv & !wphn & ((opp & 0xf) == 0x4));
    let oe4n = !(opv & ((opp & 0xf) == 0x4));
    let t6wn = !(opv & ((opp & 0xf) == 0x6) & ((ph & 0x2) == 0x2));
    let t7rn = !(opv & ((opp & 0xf) == 0x7));
    Stb1Out {
        sidn,
        si3n,
        si5n,
        so4n,
        oe4n,
        t6wn,
        t7rn,
    }
}

/// P1A の DNMN の真理値表（番地の bit は DTS、DNF、ABQN、DACKN、DNMN の順）。
const P1A_DNMN_T: u64 = 0x00000000ff7f0020;

/// P1A（PAL16R6A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct P1aOut {
    /// 登録出力：DACKN
    pub dackn: bool,
    /// 登録出力：PNDN
    pub pndn: bool,
    /// 登録出力：DNMN
    pub dnmn: bool,
    /// 登録出力：P1RN
    pub p1rn: bool,
    /// 登録出力：P1MN
    pub p1mn: bool,
    /// 登録出力：L2N
    pub l2n: bool,
}

/// P1A（PAL16R6A）。port 1（ACRTC）。16 MHz の縁ごと。NX の port 1 のアクセスが OP に入る q0 の縁（WPHN が Low）で DACK を出し（DACKN）、同期化した DTACK（DTS）で答え、そのときの DONE（DNF）を印にする（DNM）。答えは、q0 の縁ならそのスロットの答え（P1R、印 P1M）、q1〜q3 の縁なら預かり（PND）にして次の q0 の縁で P1R にする。打切り（ABQ ＝ ABT∧WPH。74S32）は q0 の縁だけで効き、DACK を戻し、預かった答えを捨てる。L2（port 2 の延びたアクセスが OP にある：DOC 子基板のとき。P2A へ）は、OP の写しとして、OP に入る q0 の縁で NX から取り込む（DR-091：OP の 74LS377 は SCLK の 27 ns 後で、P2A の q1 の縁に間に合わない）
/// クロックは C16。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn p1a(wphn: bool, opldn: bool, opvd: bool, nxlg: bool, nxp: u32, dts: bool, dnf: bool, abqn: bool, dackn: bool, pndn: bool, dnmn: bool, p1rn: bool, p1mn: bool, l2n: bool) -> P1aOut {
    let dackn_d = !((!wphn & !opldn & opvd & nxlg & ((nxp & 0x1) == 0x1)) | (!dackn & !dts & abqn));
    let pndn_d = !((wphn & !dackn & dts) | (wphn & !pndn));
    let dnmn_d = { let i = (dts as u32) << 0 | (dnf as u32) << 1 | (abqn as u32) << 2 | (dackn as u32) << 3 | (dnmn as u32) << 4; P1A_DNMN_T >> i & 1 != 0 };
    let p1rn_d = !((!wphn & abqn & !dackn & dts) | (!wphn & abqn & !pndn) | (wphn & !p1rn));
    let p1mn_d = !((!wphn & abqn & !dackn & dts & dnf) | (!wphn & abqn & !pndn & !dnmn) | (wphn & !p1mn));
    let l2n_d = !((!wphn & !opldn & opvd & nxlg & ((nxp & 0x1) == 0x0)) | (wphn & !l2n) | (opldn & !l2n));
    P1aOut {
        dackn: dackn_d,
        pndn: pndn_d,
        dnmn: dnmn_d,
        p1rn: p1rn_d,
        p1mn: p1mn_d,
        l2n: l2n_d,
    }
}

/// P2A の S1N の真理値表（番地の bit は SYN1、L2N、ABQN、BWN、SYN2N、S1N、S0N、RDHN、PNDN、P2RN の順）。
const P2A_S1N_T: [u64; 16] = [0xffffafff00005555, 0xffffffff00005555, 0xffffaaff00004555, 0xffffffff00004555, 0xffffafff00005555, 0xffffffff00005555, 0xffffaaff00004555, 0xffffffff00004555, 0xffffafff00005555, 0xffffffff00005555, 0xffffaaff00004555, 0xffffffff00004555, 0xffffafff00005555, 0xffffefff00004555, 0xffffaaff00004555, 0xffffefff00004555];

/// P2A の S0N の真理値表（番地の bit は SYN1、L2N、ABQN、SYN2N、S1N、S0N、RDHN、PNDN、P2RN の順）。
const P2A_S0N_T: [u64; 8] = [0xffffffef0f5f0f5f, 0xffff00ef00000045, 0xffffffef0f5f0f5f, 0xffff00ef00000045, 0xffffffef0f5f0f5f, 0xffff00ef00000045, 0xcfcfffef0f4f0f5f, 0xcfcf00ef00000045];

/// P2A の RDHN の真理値表（番地の bit は SYN1、SYN2N、S1N、S0N、RDHN、WQN の順）。
const P2A_RDHN_T: u64 = 0x00fe0000f1fff1ff;

/// P2A の PNDN の真理値表（番地の bit は SYN1、L2N、WPHN、SYN2N、S1N、S0N、RDHN、PNDN、P2RN、WQN の順）。
const P2A_PNDN_T: [u64; 16] = [0x0f0f0f0f0f0f0f0f, 0x0f0f0f0f0f0f0f0f, 0xffffffefffffffaf, 0xffffffefffffffef, 0x0f0f0f0f0f0f0f0f, 0x0f0f0f0f0f0f0f0f, 0xcfcfffefffffffaf, 0xcfcfffefffffffef, 0x0f0f0f0f0f0f0f0f, 0x0f0f0f0f0f0f0f0f, 0xffffffffffffffaf, 0xffffffffffffffff, 0x0f0f0f0f0f0f0f0f, 0x0f0f0f0f0f0f0f0f, 0xffffffffffffffaf, 0xffffffffffffffff];

/// P2A の LDQN の真理値表（番地の bit は SYN1、L2N、ABQN、SYN2N、S1N、S0N、RDHN、PNDN、P2RN の順）。
const P2A_LDQN_T: [u64; 8] = [0xffffffefffffffff, 0xffffffefffffffef, 0xffffffefffffffff, 0xffffffefffffffef, 0xffffffefffffffff, 0xffffffefffffffef, 0xcfcfffefffffffff, 0xcfcfffefffffffef];

/// P2A（PAL16R8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct P2aOut {
    /// 登録出力：SYN2N
    pub syn2n: bool,
    /// 登録出力：S1N
    pub s1n: bool,
    /// 登録出力：S0N
    pub s0n: bool,
    /// 登録出力：RDHN
    pub rdhn: bool,
    /// 登録出力：PNDN
    pub pndn: bool,
    /// 登録出力：P2RN
    pub p2rn: bool,
    /// 登録出力：LDQN
    pub ldqn: bool,
    /// 登録出力：WQN
    pub wqn: bool,
}

/// P2A（PAL16R8A）。port 2（DOC 子基板）。16 MHz の縁ごと。DOC の E の同期化の 2 段目（SYN2。1 段目 SYN1 は外の 74ALS74。DR-088）、預かりの段（S1・S0：空き 00、窓を待つ 01、窓を取った 11、取った読出しを捨てる 10）、預かったものが読出し（RDH）。空きなら Fabric のアクセスをラッチし（LDQ：次の縁で U48–U49 が番地を取り込む）、E の立下りで窓を取り、次の立下りで空ける。答え（書込みのラッチ、読出しの窓の終わり）は、q0 の縁ならそのスロットの答え（P2R）、q1〜q3 の縁なら預かり（PND）にして次の q0 の縁で P2R にする。打切り（ABQ ＝ ABT∧WPH。74S32）は q0 の縁だけで効く。BWN は 68000 のブリッジが窓を待っていない（HACK）。port 2 のアクセスがあるか（L2）は P1A が、向き（WQ）は P2A が、OP に入る q0 の縁で NX から写す（DR-091。OPLDN は OPA の OP の空き）。組合せの出力を帰還しない
/// クロックは C16。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn p2a(syn: u32, l2n: bool, opldn: bool, nxw: bool, abqn: bool, wphn: bool, bwn: bool, syn2n: bool, s1n: bool, s0n: bool, rdhn: bool, pndn: bool, p2rn: bool, wqn: bool) -> P2aOut {
    let syn2n_d = !((syn & 0x2) == 0x2);
    let s1n_d = { let i = (syn >> 1 & 0x1) << 0 | (l2n as u32) << 1 | (abqn as u32) << 2 | (bwn as u32) << 3 | (syn2n as u32) << 4 | (s1n as u32) << 5 | (s0n as u32) << 6 | (rdhn as u32) << 7 | (pndn as u32) << 8 | (p2rn as u32) << 9; P2A_S1N_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let s0n_d = { let i = (syn >> 1 & 0x1) << 0 | (l2n as u32) << 1 | (abqn as u32) << 2 | (syn2n as u32) << 3 | (s1n as u32) << 4 | (s0n as u32) << 5 | (rdhn as u32) << 6 | (pndn as u32) << 7 | (p2rn as u32) << 8; P2A_S0N_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let rdhn_d = { let i = (syn >> 1 & 0x1) << 0 | (syn2n as u32) << 1 | (s1n as u32) << 2 | (s0n as u32) << 3 | (rdhn as u32) << 4 | (wqn as u32) << 5; P2A_RDHN_T >> i & 1 != 0 };
    let pndn_d = { let i = (syn >> 1 & 0x1) << 0 | (l2n as u32) << 1 | (wphn as u32) << 2 | (syn2n as u32) << 3 | (s1n as u32) << 4 | (s0n as u32) << 5 | (rdhn as u32) << 6 | (pndn as u32) << 7 | (p2rn as u32) << 8 | (wqn as u32) << 9; P2A_PNDN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let p2rn_d = !((wphn & !p2rn) | (!wphn & abqn & !pndn) | (!wphn & abqn & !syn2n & !s1n & !s0n & !rdhn & ((syn & 0x2) == 0x0)) | (!wphn & abqn & !syn2n & !l2n & !wqn & !s1n & ((syn & 0x2) == 0x0)) | (!wphn & abqn & !l2n & !wqn & s1n & s0n & p2rn));
    let ldqn_d = { let i = (syn >> 1 & 0x1) << 0 | (l2n as u32) << 1 | (abqn as u32) << 2 | (syn2n as u32) << 3 | (s1n as u32) << 4 | (s0n as u32) << 5 | (rdhn as u32) << 6 | (pndn as u32) << 7 | (p2rn as u32) << 8; P2A_LDQN_T[(i >> 6) as usize] >> (i & 63) & 1 != 0 };
    let wqn_d = !((!wphn & !opldn & nxw) | (wphn & !wqn) | (opldn & !wqn));
    P2aOut {
        syn2n: syn2n_d,
        s1n: s1n_d,
        s0n: s0n_d,
        rdhn: rdhn_d,
        pndn: pndn_d,
        p2rn: p2rn_d,
        ldqn: ldqn_d,
        wqn: wqn_d,
    }
}

/// HDEC0（PAL20L8A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hdec0Out {
    /// 組合せの出力：VLDN
    pub vldn: bool,
    /// 組合せの出力：AREQN
    pub areqn: bool,
    /// 組合せの出力：CHN
    pub chn: bool,
    /// 組合せの出力：GLN
    pub gln: bool,
    /// 組合せの出力：RAFN
    pub rafn: bool,
    /// 組合せの出力：RMODN
    pub rmodn: bool,
    /// 組合せの出力：RDENN
    pub rdenn: bool,
}

/// HDEC0（PAL20L8A）。CPU の窓（/FCS、16-bit のアクセス、A11–A1）の解読：DTACK をすぐ返すアクセス（VLD）、A ファイルのアクセス（AREQ：0x06・0x0A・0x0C）、チャネルのレジスタの範囲（CH）、全体のレジスタの範囲（GL：0x100〜0x104）、A ファイルの読出しの許可（RAF：U7–U8）、MODULES（RMOD：U11）、読出しの緩衝器（RDEN：U3–U4）
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn hdec0(fcsn: bool, udsn: bool, ldsn: bool, rw: bool, a: u32) -> Hdec0Out {
    let vldn = !((!fcsn & !udsn & !ldsn & ((a & 0xfb0) == 0x10)) | (!fcsn & !udsn & !ldsn & ((a & 0xfbc) == 0x0)) | (!fcsn & !udsn & !ldsn & ((a & 0xfba) == 0x0)) | (!fcsn & !udsn & !ldsn & ((a & 0xfb6) == 0x0)) | (!fcsn & !udsn & !ldsn & ((a & 0xfbe) == 0xe)) | (!fcsn & !udsn & !ldsn & ((a & 0xffc) == 0x100)) | (!fcsn & !udsn & !ldsn & ((a & 0xffa) == 0x100)));
    let areqn = !((!fcsn & !udsn & !ldsn & ((a & 0xfbe) == 0x6)) | (!fcsn & !udsn & !ldsn & ((a & 0xfbe) == 0xa)) | (!fcsn & !udsn & !ldsn & ((a & 0xfbe) == 0xc)));
    let chn = !(!fcsn & !udsn & !ldsn & ((a & 0xfa0) == 0x0));
    let gln = !(!fcsn & !udsn & !ldsn & ((a & 0xff8) == 0x100));
    let rdenn = !(!fcsn & rw);
    let rafn = !(rw & !areqn);
    let rmodn = !(rw & ((a & 0x6) == 0x4) & !gln);
    Hdec0Out {
        vldn,
        areqn,
        chn,
        gln,
        rafn,
        rmodn,
        rdenn,
    }
}

/// HDEC1（PAL16L8A-2）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hdec1Out {
    /// 組合せの出力：W1XN
    pub w1xn: bool,
    /// 組合せの出力：WSRCN
    pub wsrcn: bool,
    /// 組合せの出力：WDSTN
    pub wdstn: bool,
    /// 組合せの出力：WBFN
    pub wbfn: bool,
    /// 組合せの出力：WPRSTN
    pub wprstn: bool,
    /// 組合せの出力：WAUDN
    pub waudn: bool,
    /// 組合せの出力：RST0N
    pub rst0n: bool,
    /// 組合せの出力：RST1N
    pub rst1n: bool,
}

/// HDEC1（PAL16L8A-2）。チャネル・全体のレジスタの範囲から、書込みの信号と STATUS の読出しの許可：オフセット 0x10〜0x1E の書込み（W1X：U5–U6 の 74LS138 の許可。チャネルは A6）、SRC・DST（WSRC、WDST）、B ファイル（WBF：0x0E・0x10）、PORT_RESET（WPRST）、AUDIO_N（WAUD）、STATUS（RST0・RST1：U9–U10）
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn hdec1(chn: bool, gln: bool, rw: bool, a: u32) -> Hdec1Out {
    let w1xn = !(!chn & !rw & ((a & 0x10) == 0x10));
    let wsrcn = !(!chn & !rw & ((a & 0x1e) == 0x0));
    let wdstn = !(!chn & !rw & ((a & 0x1e) == 0x2));
    let wbfn = !((!chn & !rw & ((a & 0x1e) == 0xe)) | (!chn & !rw & ((a & 0x1e) == 0x10)));
    let wprstn = !(!gln & !rw & ((a & 0x6) == 0x0));
    let waudn = !(!gln & !rw & ((a & 0x6) == 0x2));
    let rst0n = !(!chn & rw & ((a & 0x5e) == 0x1e));
    let rst1n = !(!chn & rw & ((a & 0x5e) == 0x5e));
    Hdec1Out {
        w1xn,
        wsrcn,
        wdstn,
        wbfn,
        wprstn,
        waudn,
        rst0n,
        rst1n,
    }
}

/// HACK の BWN の真理値表（番地の bit は SYN1、SYN2N、DCSN、BWN、BCN、BDTN の順）。
const HACK_BWN_T: u64 = 0xf1f1fff1fff1fff1;

/// HACK の BCN の真理値表（番地の bit は SYN1、SYN2N、DCSN、BWN、BCN、BDTN の順）。
const HACK_BCN_T: u64 = 0xfefe1110fffe1110;

/// HACK（PAL16R4A）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HackOut {
    /// 登録出力：AKN
    pub akn: bool,
    /// 登録出力：BWN
    pub bwn: bool,
    /// 登録出力：BCN
    pub bcn: bool,
    /// 登録出力：BDTN
    pub bdtn: bool,
    /// 組合せの出力：HREQ
    pub hreq: bool,
    /// 組合せの出力：DTKN
    pub dtkn: bool,
}

/// HACK（PAL16R4A）。68000 の DTACK（16 MHz の縁ごと）。Fabric の窓：A ファイルを使わないアクセスはすぐ（VLD）、A ファイルのアクセスは SEQ の許可（AOP＝001）のスロットの q3 の縁で返す（AK）。HREQ は ELG への要求で、許可のスロットの頭から下げる（AOP＝001）。DOC の窓のブリッジ（DR-035、DR-050）：/DCS を取り込んで窓を待ち（BW）、同期化した E の立下りで次の窓を取り（BC。P2A より先）、その窓の終わりの立下りで DTACK を出す（BDT）
/// クロックは C16。
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn hack(areqn: bool, vldn: bool, aop2n: bool, aop1n: bool, aop0n: bool, mclk: bool, syn: u32, syn2n: bool, dcsn: bool, akn: bool, bwn: bool, bcn: bool, bdtn: bool) -> HackOut {
    let hreq = !((areqn) | (!akn) | (aop2n & aop1n & !aop0n));
    let dtkn = !((!vldn) | (!akn & !areqn) | (!bdtn));
    let akn_d = !((!areqn & aop2n & aop1n & !aop0n & mclk) | (!areqn & !akn));
    let bwn_d = { let i = (syn >> 1 & 0x1) << 0 | (syn2n as u32) << 1 | (dcsn as u32) << 2 | (bwn as u32) << 3 | (bcn as u32) << 4 | (bdtn as u32) << 5; HACK_BWN_T >> i & 1 != 0 };
    let bcn_d = { let i = (syn >> 1 & 0x1) << 0 | (syn2n as u32) << 1 | (dcsn as u32) << 2 | (bwn as u32) << 3 | (bcn as u32) << 4 | (bdtn as u32) << 5; HACK_BCN_T >> i & 1 != 0 };
    let bdtn_d = !((!bcn & !syn2n & !dcsn & ((syn & 0x2) == 0x0)) | (!bdtn & !dcsn));
    HackOut {
        akn: akn_d,
        bwn: bwn_d,
        bcn: bcn_d,
        bdtn: bdtn_d,
        hreq,
        dtkn,
    }
}

/// PRST（PAL16L8A-2）の出力。登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrstOut {
    /// 組合せの出力：RST2N
    pub rst2n: bool,
    /// 組合せの出力：RST3N
    pub rst3n: bool,
    /// 組合せの出力：RST4N
    pub rst4n: bool,
    /// 組合せの出力：RST5N
    pub rst5n: bool,
    /// 組合せの出力：RST6N
    pub rst6n: bool,
    /// 組合せの出力：RST7N
    pub rst7n: bool,
    /// 組合せの出力：RST10N
    pub rst10n: bool,
}

/// PRST（PAL16L8A-2）。PORT_RESET の書込み（WPRST）と ID のビットから、ポートごとの初期化の信号（2：本体 DAC の FIFO、3・4：DSP の FIFO、5：ADPCM の FIFO とアダプタ、6・7：Temporal RAM のポインタ、10：音声入力の FIFO）。電源投入の RESN でも出す
#[inline]
#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]
pub fn prst(wprstn: bool, d: u32, resn: bool) -> PrstOut {
    let rst2n = !((!wprstn & ((d & 0x4) == 0x4)) | (!resn));
    let rst3n = !((!wprstn & ((d & 0x8) == 0x8)) | (!resn));
    let rst4n = !((!wprstn & ((d & 0x10) == 0x10)) | (!resn));
    let rst5n = !((!wprstn & ((d & 0x20) == 0x20)) | (!resn));
    let rst6n = !((!wprstn & ((d & 0x40) == 0x40)) | (!resn));
    let rst7n = !((!wprstn & ((d & 0x80) == 0x80)) | (!resn));
    let rst10n = !((!wprstn & ((d & 0x400) == 0x400)) | (!resn));
    PrstOut {
        rst2n,
        rst3n,
        rst4n,
        rst5n,
        rst6n,
        rst7n,
        rst10n,
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
