//! Data Fabric（1985年の最終試作機、2 チャネル）のスロット単位の模型。
//!
//! 動作の定義は仕様書 §06 と DR-046、回路は `docs/spec/fabric.md`（以下 fabric.md）。
//! fabric.md の順序と状態の PAL の式は、この模型のスロットの列に合わせて書く（OI-050）。
//!
//! `step_slot` の 1 回が 1 スロット（8 MHz の 2 クロック、250 ns）。スロット 0 は S（Staging RAM の
//! 時分割の前半で、port 0 が使える）で、S と X が交互に来る。1 スロットの中の順は fabric.md §03：
//!
//! 1. 頭：1 つ前の判断で決めたアクセスを、このスロットのアクセスにする。判断（このスロットの
//!    A ファイルの操作と、次のスロットのアクセス）を、1 つ前のスロットの中ほどで取り込んだ準備の
//!    信号で行う。判断バスのチャネル（候補）を選び直す。
//! 2. 中ほど：候補のチャネルの読出し元と書込み先の準備を取り込む。このスロットのアクセスの結果は、
//!    まだ見えない（FIFO の満杯・空の信号は、アクセスの信号の終わりから変わる）。
//! 3. アクセス。読んだ値はスロットの終わりに IL に入る。
//! 4. 4 分の 3：A ファイルの操作。PS・PD は歩進の前の番地を取り込み、次のスロットのアクセスに使う。
//! 5. 終わり：要素の完了と DONE、トリガの同期化、延びたアクセスの時間切れ。
//!
//! CPU の側は `host_read`・`host_write`（窓 0x250000〜0x250FFF の中のオフセット、16 bit だけ）。
//! A ファイルの語（SRC_ADDR・DST_ADDR・COUNT）へのアクセスは、次のスロットの頭で許可され、
//! それまで `HostReply::Wait` を返す。結果は、許可のスロットの後に `host_poll` で受け取る。
//!
//! 変換部（ROT0–3、MSK0–1、ANX0–3）、準備の信号（RDY）、ポートの属性（ATR）は、PAL に焼く式と
//! PROM の中身から生成した `fabric_pal` を通す（DR-084）。

use crate::fabric_pal::{
    ATR, ATR_ADDR, ATR_F4, ATR_FLAG, ATR_OKD, ATR_OKN, ATR_SONLY, ATR_TWO, ATR_W16, MSK0, MSK1,
    anx0, anx1, anx2, anx3, rdy, rot0, rot1, rot2, rot3,
};
use serde::{Deserialize, Serialize};

/// 実装したチャネルの数（窓は 4 チャネル分を予約。仕様書 §06）。
pub const CHANNELS: usize = 2;

/// 延びたアクセスの時間切れ：32 クロック（4 µs）＝16 スロット（DR-046 (9)）。
pub const TIMEOUT_SLOTS: u32 = 16;

/// アクセスの向き。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    Read,
    Write,
}

/// ポートの幅。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortWidth {
    Byte,
    Word,
    /// 8 bit と 16 bit のどちらでも使える（port 0、拡張）。
    Either,
}

/// ポートの属性（fabric.md §06 の表。`tools/fabric/fabric_logic.py` の `PORTS` と同じ）。
/// 人が読む表で、試験で ATR の中身と突き合わせる。Fabric の判断は ATR（`atr`）を使う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortAttr {
    pub read: bool,
    pub write: bool,
    pub width: PortWidth,
    /// 番地を持ち、歩進する（ATR の ADDR）。
    pub addr: bool,
    /// 満杯・空の信号を持つ（ATR の FLAG）。
    pub flag: bool,
}

/// ポートの属性。`doc` は DOC 子基板があるか（port 2 の中身が変わる）。
pub fn port_attr(port: u8, doc: bool) -> PortAttr {
    use PortWidth::*;
    let a = |read, write, width, addr, flag| PortAttr {
        read,
        write,
        width,
        addr,
        flag,
    };
    match port & 15 {
        0 => a(true, true, Either, true, false),
        1 => a(true, true, Word, false, true),
        2 if doc => a(true, true, Byte, true, true),
        // DOC 子基板がないときは本体 DAC の FIFO。番地は使わないが、歩進はする（fabric.md §05）。
        2 => a(false, true, Word, true, true),
        3 => a(false, true, Word, false, true),
        4 => a(true, false, Word, false, true),
        5 => a(false, true, Byte, false, true),
        6 => a(false, true, Byte, false, false),
        7 => a(true, false, Byte, false, false),
        8 => a(true, false, Byte, true, false),
        9 => a(true, false, Byte, true, false),
        10 => a(true, false, Byte, false, true),
        _ => a(true, true, Either, false, true),
    }
}

/// ATR（63S281A）の出力。番地はポート、向き、FORMAT（fabric.md §06）。
#[inline]
pub fn atr(port: u8, dir: Dir, format: u8) -> u8 {
    ATR[(port & 15) as usize | ((dir == Dir::Write) as usize) << 4 | ((format & 7) as usize) << 5]
}

/// FORMAT の入力が 16 bit か（読出し元の側の W16）。
pub fn input_is_word(format: u8) -> bool {
    atr(0, Dir::Read, format) & ATR_W16 != 0
}

/// FORMAT の出力が 16 bit か（書込み先の側の W16）。
pub fn output_is_word(format: u8) -> bool {
    atr(0, Dir::Write, format) & ATR_W16 != 0
}

/// このポートを、この向きと FORMAT で使えるか（ATR の OKD・OKN）。モジュールの有無は見ない。
pub fn port_ok(port: u8, dir: Dir, format: u8, doc: bool) -> bool {
    atr(port, dir, format) & if doc { ATR_OKD } else { ATR_OKN } != 0
}

/// 番地を持ち、歩進するポートか（ATR の ADDR。向きと FORMAT によらない）。
#[inline]
fn has_addr(port: u8) -> bool {
    atr(port, Dir::Read, 0) & ATR_ADDR != 0
}

/// S のスロットでだけアクセスするポートか（ATR の SONLY。port 0）。
#[inline]
fn s_only(port: u8) -> bool {
    atr(port, Dir::Read, 0) & ATR_SONLY != 0
}

/// IC の応答まで延びるアクセス（port 1 と、DOC 子基板があるときの port 2。DR-046 (1)）。
pub fn is_long(port: u8, doc: bool) -> bool {
    port == 1 || (port == 2 && doc)
}

/// SHIFT の 5 bit を −16〜15 にする。
pub fn shift_amount(shift: u8) -> i32 {
    let s = (shift & 31) as i32;
    if s >= 16 { s - 32 } else { s }
}

/// 書込みのスロットで FD バスに出る 16 bit（fabric.md §04 の ANX の出力）。
///
/// 回路のとおり、IL → ROT0–1 → ROT2–3 → ANX0–3（MSK0–1、MASK、XOR）を、PAL の式と PROM の中身から
/// 生成した関数で計算する（DR-084）。回す量 T は SHIFT の下位 4 bit で、T3 ＝ S3 ⊕ F4（74LS86）。
///
/// 8 bit の出力の FORMAT では、MSK が上位 8 bit を 0 にするので、上位には XOR の上位 8 bit が出る。
/// 8 bit のポートは下位バイトだけを使う。FORMAT 6・7 は MSK が 0 なので XOR がそのまま出る
/// （START の検査で ERROR になるので、動作中の書換えのときだけ）。
#[inline]
pub fn fd_value(input: u16, format: u8, shift: u8, mask: u16, xor: u16) -> u16 {
    let f = (format & 7) as usize;
    let s5 = (shift & 31) as usize;
    let f4 = atr(0, Dir::Write, format) & ATR_F4 != 0;
    let t = (s5 as u32 & 15) ^ if f4 { 8 } else { 0 };
    let d = input as u32;
    let fine = rot0(d, t).f | rot1(d, t).f;
    let r = rot2(fine, t).r | rot3(fine, t).r;
    let a = s5 | f << 5;
    let m = MSK0[a] as u32 | (MSK1[a] as u32) << 8;
    let (k, x) = (mask as u32, xor as u32);
    let q = anx0(r, m, k, x, true).q
        | anx1(r, m, k, x, true).q
        | anx2(r, m, k, x, true).q
        | anx3(r, m, k, x, true).q;
    q as u16
}

/// 仕様書 §06・DR-046 (5) の変換：入力（8 bit のポートは下位バイト）→ FORMAT と SHIFT →
/// AND MASK → XOR → 出力の幅で切捨て。
pub fn transform(input: u16, format: u8, shift: u8, mask: u16, xor: u16) -> u16 {
    let v = fd_value(input, format, shift, mask, xor);
    if output_is_word(format) { v } else { v & 0xFF }
}

/// ポートへの 1 回のアクセス。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Access {
    pub ch: u8,
    pub port: u8,
    pub dir: Dir,
    /// ポートの番地（読出しは PS、書込みは PD）。逐次のポートでは意味がない。
    pub addr: u16,
    /// 16 bit のアクセスか（ATR の W16。読出しは FORMAT の入力の幅、書込みは出力の幅）。
    pub word: bool,
    /// 書込みは FD に出した値。読出しは、終わったスロットの記録で読んだ値（それまでは 0）。
    pub data: u16,
}

/// ポートの応答。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortReply {
    /// 終わった。読出しでは FD の値（8 bit のポートは下位バイト）。
    Done(u16),
    /// 終わった。ポートが転送の最後の語の印を出した（port 1 の ACRTC の DONE。DR-022）。
    Last(u16),
    /// まだ終わらない（port 1、DOC の port 2 だけ）。
    Wait,
}

/// Fabric の外側の回路：ポート、モジュールの /PRES、PORT_RESET と AUDIO_N の書込みの信号。
pub trait Ports {
    /// このスロットの間、ポートの向こうの回路（ACRTC のつなぎなど）を進める。スロットの頭の判断の後に
    /// 毎スロット呼び、このスロットで始まるアクセスを渡す。準備の信号とアクセスの応答は、この後に問う。
    fn run_slot(&mut self, _starting: Option<&Access>) {}
    /// ポートの向こうの回路が出すトリガのレベル（bit n が発生源 n。DR-018）。スロットの終わりに取り込む。
    /// `Fabric::set_trigger_inputs` の値と OR をとる。
    fn trigger_levels(&self) -> u8 {
        0
    }
    /// そのポートの準備の信号。スロットの中ほどで、このスロットのアクセスより前の状態を返す。
    /// Fabric は `ready_lines` を通して見る。
    fn ready(&self, port: u8, dir: Dir) -> bool;
    /// RDY の PAL の入力の線（fabric.md §06）。bit n が port n の線（1：RDY1、2：RDY2、3：IR3、4：OR4、
    /// 5：IR5、10：OR10）、bit 11 が拡張の XRDY。既定は `ready` から作る。
    fn ready_lines(&self) -> u32 {
        let mut v = 0;
        for (bit, port, dir) in [
            (1, 1, Dir::Read),
            (2, 2, Dir::Write),
            (3, 3, Dir::Write),
            (4, 4, Dir::Read),
            (5, 5, Dir::Write),
            (10, 10, Dir::Read),
            (11, 11, Dir::Read),
        ] {
            v |= (self.ready(port, dir) as u32) << bit;
        }
        v
    }
    /// アクセスを始める。port 1 と DOC の port 2 のほかは、すぐ `Done` を返す。
    fn access(&mut self, acc: &Access) -> PortReply;
    /// 延びたアクセスの続き。始めた次のスロットから、終わるまで毎スロット呼ぶ。
    fn continue_access(&mut self, _port: u8) -> PortReply {
        PortReply::Done(0)
    }
    /// 延びたアクセスを打ち切る（時間切れ、STOP）。
    fn abort_access(&mut self, _port: u8) {}
    /// DOC 子基板があるか（MODULES の bit 2）。
    fn doc_present(&self) -> bool;
    /// port 8、10〜15 のモジュールがあるか（/PRES）。ほかのポートは問われない。
    fn module_present(&self, port: u8) -> bool;
    /// PORT_RESET：bit n で port n の FIFO・ポインタを初期化する。
    fn port_reset(&mut self, mask: u16);
    /// AUDIO_N の書込み（音声クロックの分周器が取り込む。DR-018）。
    fn write_audio_n(&mut self, n: u16);
}

/// A ファイル（と CPU）の 1 スロットの操作（fabric.md §05）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Aop {
    None,
    /// CPU の読み書き。
    Host,
    /// 読出し元の歩進。PS が歩進の前の番地を取り込む。
    SrcStep(u8),
    /// 書込み先の歩進。PD が歩進の前の番地を取り込む。
    DstStep(u8),
    /// COUNT の更新（作業値−1）。
    Count(u8),
    /// 再設定の段（0：SRC_ADDR、1：DST_ADDR、2：COUNT）。
    Reload(u8, u8),
}

/// スロットの頭の判断のうち、要素に関わるもの。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    None,
    /// 新しい要素を始めた。次のスロットで読む。
    Start(u8),
    /// 動いている要素の書込みを決めた。次のスロットで書く。
    Write(u8),
    /// 読出し元と書込み先の両方が番地を持つ要素の、W の後の COUNT の更新。
    CountAfterWrite(u8),
}

/// 1 スロットの記録（trace）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotRecord {
    pub slot: u64,
    /// S のスロット（port 0 が使える）か。
    pub s: bool,
    /// 判断に使った候補のチャネル。
    pub candidate: u8,
    pub aop: Aop,
    pub decision: Decision,
    /// このスロットで行っていたアクセス（2 スロット以上のアクセスは、各スロットに出る）。
    pub access: Option<Access>,
    /// このスロットでアクセスが終わったか。
    pub access_done: bool,
}

/// CPU の応答。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostReply {
    /// 書込みを受けた（DTACK）。
    Ack,
    /// 読んだ値（DTACK）。
    Data(u16),
    /// A ファイルの許可を待つ。`host_poll` で結果を受け取る。
    Wait,
    /// DTACK を返さない。A 基板のバスの時間切れでバスエラーになる（DR-046 (10)）。
    BusError,
}

/// チャネルの設定のレジスタ（書込み専用）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelRegs {
    pub src: u8,
    pub dst: u8,
    pub src_step: u16,
    pub dst_step: u16,
    pub format: u8,
    pub mask: u16,
    pub xor: u16,
    pub shift: u8,
    pub trigger: u8,
}

impl Default for ChannelRegs {
    /// 電源投入時。MASK・XOR は BIOS が書く値にする（DR-046 (4)）。ほかの値は資料にない [V]。
    fn default() -> Self {
        Self {
            src: 0,
            dst: 0,
            src_step: 0,
            dst_step: 0,
            format: 0,
            mask: 0xFFFF,
            xor: 0,
            shift: 0,
            trigger: 0,
        }
    }
}

/// チャネルの段階。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    #[default]
    Idle,
    /// 再設定の段（0〜2）を待つ。
    Reload(u8),
    Run,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Channel {
    regs: ChannelRegs,
    src_set: u16,
    src_work: u16,
    dst_set: u16,
    dst_work: u16,
    count_set: u16,
    count_work: u16,
    phase: Phase,
    src_ok: bool,
    dst_ok: bool,
    /// COUNT の更新で最後の要素と分かった。新しい要素を始めない。
    no_more: bool,
    done: bool,
    error: bool,
    miss: bool,
    stall: bool,
    /// トリガで許された（EACH＝0 はブロックの間、EACH＝1 は 1 要素）。
    armed: bool,
    pending: bool,
    sync: [bool; 2],
    prev: bool,
}

impl Channel {
    fn source(&self) -> u8 {
        self.regs.trigger & 7
    }
    fn each(&self) -> bool {
        self.regs.trigger & 0x08 != 0
    }
    fn looping(&self) -> bool {
        self.regs.trigger & 0x10 != 0
    }
    fn irq_en(&self) -> bool {
        self.regs.trigger & 0x20 != 0
    }
    /// 発生源 0（即時）はいつも許されている。
    fn is_armed(&self) -> bool {
        self.source() == 0 || self.armed
    }
    fn status(&self) -> u16 {
        let p = self.phase;
        (p != Phase::Idle) as u16
            | (self.done as u16) << 1
            | (self.stall as u16) << 2
            | (self.error as u16) << 3
            | (self.miss as u16) << 4
            | ((p == Phase::Run && !self.is_armed()) as u16) << 5
    }
    /// トリガの 2 段の同期化と立上り、保留、許可（スロットの終わり。fabric.md §06）。
    fn sync_trigger(&mut self, inputs: u8) {
        let src = self.source();
        let level = (1..=5).contains(&src) && inputs & (1 << src) != 0;
        let rising = self.sync[1] && !self.prev;
        self.prev = self.sync[1];
        self.sync[1] = self.sync[0];
        self.sync[0] = level;
        if rising && self.phase != Phase::Idle {
            if self.pending {
                self.miss = true;
            } else {
                self.pending = true;
            }
        }
        if self.phase == Phase::Run && !self.armed && self.pending {
            self.armed = true;
            self.pending = false;
        }
    }
}

/// 要素（読んで書く 1 回）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Element {
    ch: u8,
    /// IL（読んだ値）。
    data: u16,
    read_done: bool,
    w_decided: bool,
    /// 両方が番地を持つとき、W の後に COUNT を更新する。
    count_after_w: bool,
    w_done: bool,
    count_done: bool,
    /// COUNT の更新で、最後の要素と分かった。
    last: bool,
}

impl Element {
    fn new(ch: u8) -> Self {
        Self {
            ch,
            data: 0,
            read_done: false,
            w_decided: false,
            count_after_w: false,
            w_done: false,
            count_done: false,
            last: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Scheduled {
    ch: u8,
    port: u8,
    dir: Dir,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Kind {
    Single,
    /// port 9 の読出し（2 スロット）。
    Two,
    /// IC の応答まで延びる。
    Long,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Active {
    acc: Access,
    kind: Kind,
    elapsed: u32,
    flag: bool,
    result: Option<u16>,
    /// ポートが最後の語の印（DONE）を出した。
    end_mark: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct HostReq {
    ch: u8,
    /// チャネルの中のオフセット（0x06、0x0A、0x0C）。
    reg: u8,
    write: Option<u16>,
    result: Option<HostReply>,
}

/// Data Fabric。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fabric {
    ch: [Channel; CHANNELS],
    slot: u64,
    /// 判断バスのチャネル（次のスロットの頭の候補）。
    dbch: u8,
    rdy_s: bool,
    rdy_d: bool,
    ps: u16,
    pd: u16,
    scheduled: Option<Scheduled>,
    active: Option<Active>,
    /// 1 つ前のスロットのアクセスが、満杯・空の信号を持つポートだった。
    prev_flag: bool,
    /// 動いている要素（始めてから、W と COUNT の更新を決めるまで）。
    elem: Option<Element>,
    /// W と COUNT の更新を決めた後、終わるまでの要素。
    tail: Option<Element>,
    hreq: Option<HostReq>,
    trig_in: u8,
    abort: Option<u8>,
}

impl Default for Fabric {
    fn default() -> Self {
        Self::new()
    }
}

impl Fabric {
    /// 電源投入時。A ファイルの中身は資料にない。0 にする [V]。
    pub fn new() -> Self {
        Self {
            ch: Default::default(),
            slot: 0,
            dbch: 0,
            rdy_s: false,
            rdy_d: false,
            ps: 0,
            pd: 0,
            scheduled: None,
            active: None,
            prev_flag: false,
            elem: None,
            tail: None,
            hreq: None,
            trig_in: 0,
            abort: None,
        }
    }

    /// 次に進めるスロットの番号。
    pub fn slot(&self) -> u64 {
        self.slot
    }

    /// STATUS の値（bit 0 BUSY、1 DONE、2 STALL、3 ERROR、4 MISS、5 WAIT）。
    pub fn status(&self, ch: usize) -> u16 {
        self.ch[ch].status()
    }

    pub fn phase(&self, ch: usize) -> Phase {
        self.ch[ch].phase
    }

    /// 設定のレジスタ（CPU からは読めない。試験と Bend の道具）。
    pub fn regs(&self, ch: usize) -> &ChannelRegs {
        &self.ch[ch].regs
    }

    /// 割込みの要求（レベル 3。IRQ_EN∧(DONE∨ERROR) の 2 チャネルの OR）。
    pub fn irq(&self) -> bool {
        self.ch.iter().any(|c| c.irq_en() && (c.done || c.error))
    }

    /// トリガの発生源の入力の値。bit n が発生源 n（1：VBlank、2：HBlank、3：音声クロック、
    /// 4：EXTRIG、5：ADPCM の V̄C̄K̄）。スロットの終わりに取り込む。
    pub fn set_trigger_inputs(&mut self, levels: u8) {
        self.trig_in = levels;
    }

    // ------------------------------------------------------------------ CPU の側

    /// CPU の 16 bit の読出し。`offset` は窓の中（0〜0xFFF）。
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
                    0x1E => HostReply::Data(self.ch[c].status()),
                    0x00..=0x1C => HostReply::Data(0),
                    _ => HostReply::BusError,
                }
            }
            0x100 | 0x102 => HostReply::Data(0),
            0x104 => HostReply::Data(Self::modules(ports)),
            _ => HostReply::BusError,
        }
    }

    /// CPU の 16 bit の書込み。
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
                let regs = &mut self.ch[c].regs;
                match r {
                    0x00 => regs.src = value as u8 & 15,
                    0x02 => regs.dst = value as u8 & 15,
                    // 32 bit の上位の語は使わない（DR-046 (3)）。
                    0x04 | 0x08 => {}
                    0x06 | 0x0A | 0x0C => return self.request(c, r, Some(value)),
                    0x0E => regs.src_step = value,
                    0x10 => regs.dst_step = value,
                    0x12 => regs.format = value as u8 & 7,
                    0x14 => regs.mask = value,
                    0x16 => regs.xor = value,
                    0x18 => regs.shift = value as u8 & 31,
                    0x1A => regs.trigger = value as u8 & 0x3F,
                    0x1C => self.ctrl(c, value),
                    0x1E => {
                        let ch = &mut self.ch[c];
                        ch.done &= value & 0x02 == 0;
                        ch.error &= value & 0x08 == 0;
                        ch.miss &= value & 0x10 == 0;
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
            // MODULES は読出し専用。書込みは捨てる [P]。
            0x104 => HostReply::Ack,
            _ => HostReply::BusError,
        }
    }

    /// A ファイルのアクセスの結果。許可のスロットが済むまでは `None`。
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

    fn modules<P: Ports + ?Sized>(ports: &P) -> u16 {
        let mut m = (ports.doc_present() as u16) << 2;
        for p in [8u8, 10, 11, 12, 13, 14, 15] {
            m |= (ports.module_present(p) as u16) << p;
        }
        m
    }

    /// CTRL：bit 0 START、bit 1 STOP。同時なら STOP。動作中の START は無視（DR-046 (6)）。
    fn ctrl(&mut self, c: usize, value: u16) {
        if value & 2 != 0 {
            self.kill(c);
        } else if value & 1 != 0 && self.ch[c].phase == Phase::Idle {
            let ch = &mut self.ch[c];
            ch.phase = Phase::Reload(0);
            ch.pending = false;
            ch.armed = false;
            ch.stall = false;
            ch.no_more = false;
        }
    }

    /// チャネルを止め、書込みの済んでいない要素を捨てる（STOP、ERROR）。
    fn kill(&mut self, c: usize) {
        let c8 = c as u8;
        if self.elem.is_some_and(|e| e.ch == c8) {
            self.elem = None;
        }
        if self.tail.is_some_and(|e| e.ch == c8) {
            self.tail = None;
        }
        if self.scheduled.is_some_and(|s| s.ch == c8) {
            self.scheduled = None;
        }
        if let Some(a) = self.active
            && a.acc.ch == c8
        {
            if a.kind == Kind::Long && a.result.is_none() {
                self.abort = Some(a.acc.port);
            }
            self.active = None;
        }
        let ch = &mut self.ch[c];
        ch.phase = Phase::Idle;
        ch.armed = false;
        ch.stall = false;
        ch.no_more = false;
    }

    // ------------------------------------------------------------------ スロット

    /// 1 スロット進める。
    pub fn step_slot<P: Ports + ?Sized>(&mut self, ports: &mut P) -> SlotRecord {
        if let Some(p) = self.abort.take() {
            ports.abort_access(p);
        }
        let doc = ports.doc_present();
        let s = self.slot.is_multiple_of(2);
        let next_s = !s;

        // 1. 頭：このスロットのアクセス
        if self.active.is_none()
            && let Some(sc) = self.scheduled.take()
        {
            self.active = Some(self.begin(sc, doc));
        }
        let candidate = self.dbch;
        let (aop, decision) = self.decide(next_s);
        self.dbch = self.next_candidate(s);
        let starting = self.active.filter(|a| a.elapsed == 0).map(|a| a.acc);
        ports.run_slot(starting.as_ref());

        // 2. 中ほど：候補の準備の信号（RDY の PAL が MCLK で取り込む）。取り込んだ値を見るのは、次の頭で
        //    要素がなく、候補が動いているときだけ（`decide`）。そうなりえないスロットでは計算を省き、前の値の
        //    ままにする（判断は変わらない。要素はこの後の頭まで新しくできず、候補が Run になるのは、この
        //    スロットの再設定の 3 段目だけ）。
        let c = self.dbch;
        if self.elem.is_none()
            && (self.ch[c as usize].phase == Phase::Run || aop == Aop::Reload(c, 2))
        {
            let regs = self.ch[c as usize].regs;
            let lines = ports.ready_lines();
            let r = rdy(
                regs.src as u32,
                regs.dst as u32,
                lines,
                lines,
                lines,
                lines & 1 << 11 != 0,
                self.rdy_s,
                self.rdy_d,
            );
            self.rdy_s = r.srdy;
            self.rdy_d = r.drdy;
        }

        // 3. アクセス
        let mut record_access = None;
        let mut access_done = false;
        let mut slot_flag = false;
        if let Some(mut a) = self.active.take() {
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
                    // ACRTC の DONE と COUNT が合わない：ERROR にし、時間切れと同じく止める（DR-022）
                    let c = a.acc.ch as usize;
                    self.kill(c);
                    self.ch[c].error = true;
                } else {
                    self.complete(&a.acc, v);
                }
                access_done = true;
            } else if a.kind == Kind::Long && a.elapsed >= TIMEOUT_SLOTS {
                ports.abort_access(a.acc.port);
                let c = a.acc.ch as usize;
                self.kill(c);
                self.abort = None;
                self.ch[c].error = true;
            } else {
                self.active = Some(a);
            }
            record_access = Some(a.acc);
        }

        // 4. 4 分の 3：A ファイルの操作
        self.exec_aop(aop, doc, ports);

        // 5. 終わり
        self.prev_flag = slot_flag;
        if let Some(t) = self.tail
            && t.w_done
            && t.count_done
        {
            self.tail = None;
            if t.last {
                self.block_end(t.ch as usize);
            }
        }
        let trig = self.trig_in | ports.trigger_levels();
        for c in &mut self.ch {
            c.sync_trigger(trig);
        }
        let rec = SlotRecord {
            slot: self.slot,
            s,
            candidate,
            aop,
            decision,
            access: record_access,
            access_done,
        };
        self.slot += 1;
        rec
    }

    /// 決めたアクセスを、このスロットで始める形にする（番地は PS・PD、データは IL の変換）。
    fn begin(&self, sc: Scheduled, doc: bool) -> Active {
        let regs = &self.ch[sc.ch as usize].regs;
        let (addr, word, data) = match sc.dir {
            Dir::Read => (self.ps, input_is_word(regs.format), 0),
            Dir::Write => {
                let il = [self.tail, self.elem]
                    .into_iter()
                    .flatten()
                    .find(|e| e.ch == sc.ch && e.w_decided && !e.w_done)
                    .map_or(0, |e| e.data);
                let fd = fd_value(il, regs.format, regs.shift, regs.mask, regs.xor);
                (self.pd, output_is_word(regs.format), fd)
            }
        };
        let attr = atr(sc.port, sc.dir, regs.format);
        let kind = if is_long(sc.port, doc) {
            Kind::Long
        } else if attr & ATR_TWO != 0 {
            Kind::Two
        } else {
            Kind::Single
        };
        Active {
            acc: Access {
                ch: sc.ch,
                port: sc.port,
                dir: sc.dir,
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

    /// port 1 のアクセスで、DONE の有無が、要素が最後かどうか（COUNT の桁上げ）と食い違う。
    fn done_mismatch(&self, a: &Active) -> bool {
        [self.elem, self.tail]
            .into_iter()
            .flatten()
            .find(|e| {
                e.ch == a.acc.ch
                    && match a.acc.dir {
                        Dir::Read => !e.read_done,
                        Dir::Write => e.w_decided && !e.w_done,
                    }
            })
            .is_some_and(|e| e.count_done && e.last != a.end_mark)
    }

    /// スロットの頭の判断（fabric.md §06 の ELG・SEQ）。優先は、CPU、動いている要素の W、
    /// その COUNT の更新、再設定、新しい要素の順（DR-046 (2)）。
    fn decide(&mut self, next_s: bool) -> (Aop, Decision) {
        // 2 スロット以上のアクセスの間は、次のアクセスを決めない。port 9 は 2 スロット目の頭で
        // 終わりが分かるので決める。延びるアクセスは、終わった次のスロットの頭で決める。
        let can_decide = match &self.active {
            None => true,
            Some(a) => match a.kind {
                Kind::Single => true,
                Kind::Two => a.elapsed == 1,
                Kind::Long => false,
            },
        };
        // 満杯・空の信号を持つポートにアクセスするスロットと、その次のスロットの頭では、
        // 新しい要素を始めない（DR-046 (2)）。
        let flag_block = self.active.is_some_and(|a| a.flag) || self.prev_flag;
        let mut aop = Aop::None;
        let mut decision = Decision::None;

        if self.hreq.is_some_and(|r| r.result.is_none()) {
            aop = Aop::Host;
        }

        if let Some(mut e) = self.elem.take() {
            if !e.w_decided {
                let r_ends = e.read_done
                    || self
                        .active
                        .is_some_and(|a| a.acc.dir == Dir::Read && a.acc.ch == e.ch);
                if r_ends && can_decide {
                    let regs = &self.ch[e.ch as usize].regs;
                    // 読出し元が番地を持つ要素は、始めたときに COUNT を更新していない。始めたときの
                    // 読出し元で決める（ELS の EPC。動作中に SRC を書き換えても、1 要素に 1 回。DR-085）。
                    let (sa, da) = (!e.count_done, has_addr(regs.dst));
                    // 書込み先が番地を持てば歩進、逐次で読出し元が番地を持てば COUNT の更新（fabric.md §05）。
                    let need = if da {
                        Some(Aop::DstStep(e.ch))
                    } else if sa {
                        Some(Aop::Count(e.ch))
                    } else {
                        None
                    };
                    if (!s_only(regs.dst) || next_s) && (need.is_none() || aop == Aop::None) {
                        if let Some(op) = need {
                            aop = op;
                        }
                        self.scheduled = Some(Scheduled {
                            ch: e.ch,
                            port: regs.dst,
                            dir: Dir::Write,
                        });
                        e.w_decided = true;
                        e.count_after_w = sa && da;
                        decision = Decision::Write(e.ch);
                    }
                }
            } else if e.count_after_w && aop == Aop::None {
                aop = Aop::Count(e.ch);
                e.count_after_w = false;
                decision = Decision::CountAfterWrite(e.ch);
            }
            if e.w_decided && !e.count_after_w {
                debug_assert!(self.tail.is_none());
                self.tail = Some(e);
            } else {
                self.elem = Some(e);
            }
        } else if can_decide {
            let c = self.dbch;
            let ch = &mut self.ch[c as usize];
            match ch.phase {
                Phase::Reload(k) if aop == Aop::None => aop = Aop::Reload(c, k),
                Phase::Run if !ch.no_more => {
                    let armed = ch.is_armed();
                    let ready = self.rdy_s && self.rdy_d;
                    ch.stall = armed && !ready;
                    let src = ch.regs.src;
                    if armed && ready && (!s_only(src) || next_s) && !flag_block && aop == Aop::None
                    {
                        // 読出し元が番地を持てば歩進、逐次なら COUNT の更新（fabric.md §05）。
                        aop = if has_addr(src) {
                            Aop::SrcStep(c)
                        } else {
                            Aop::Count(c)
                        };
                        self.scheduled = Some(Scheduled {
                            ch: c,
                            port: src,
                            dir: Dir::Read,
                        });
                        self.elem = Some(Element::new(c));
                        if ch.each() && ch.source() != 0 {
                            ch.armed = false;
                        }
                        decision = Decision::Start(c);
                    }
                }
                _ => {}
            }
        }
        (aop, decision)
    }

    /// 判断バスのチャネル：要素が動いていればそのチャネル、再設定を待つチャネルがあればそれ、
    /// 2 チャネルとも動いていれば入れ替える（DR-046 (2)）。ただし、候補の読出し元が port 0 で、
    /// 今が S のスロット（`s`）なら入れ替えない。次の X の頭で要素を始められるようにする（DR-048）。
    fn next_candidate(&self, s: bool) -> u8 {
        if let Some(e) = &self.elem {
            return e.ch;
        }
        let c = self.dbch as usize;
        let o = 1 - c;
        let reload = |x: usize| matches!(self.ch[x].phase, Phase::Reload(_));
        let hold = s && self.ch[c].phase == Phase::Run && self.ch[c].regs.src == 0;
        let pick = if reload(c) {
            c
        } else if reload(o) {
            o
        } else if hold {
            c
        } else if self.ch[o].phase == Phase::Run {
            o
        } else {
            c
        };
        pick as u8
    }

    /// アクセスが終わった。読んだ値は IL へ、書込みは要素の完了へ。
    fn complete(&mut self, acc: &Access, v: u16) {
        let slots = [&mut self.elem, &mut self.tail];
        for e in slots.into_iter().flatten() {
            if e.ch != acc.ch {
                continue;
            }
            match acc.dir {
                Dir::Read if !e.read_done => {
                    e.data = v;
                    e.read_done = true;
                    return;
                }
                Dir::Write if e.w_decided && !e.w_done => {
                    e.w_done = true;
                    return;
                }
                _ => {}
            }
        }
    }

    fn exec_aop<P: Ports + ?Sized>(&mut self, aop: Aop, doc: bool, ports: &P) {
        match aop {
            Aop::None => {}
            Aop::Host => {
                if let Some(r) = &mut self.hreq {
                    let ch = &mut self.ch[r.ch as usize];
                    let (set, work) = match r.reg {
                        0x06 => (&mut ch.src_set, ch.src_work),
                        0x0A => (&mut ch.dst_set, ch.dst_work),
                        _ => (&mut ch.count_set, ch.count_work),
                    };
                    r.result = Some(match r.write {
                        Some(v) => {
                            *set = v;
                            HostReply::Ack
                        }
                        None => HostReply::Data(work),
                    });
                }
            }
            Aop::SrcStep(c) => {
                let ch = &mut self.ch[c as usize];
                self.ps = ch.src_work;
                ch.src_work = ch.src_work.wrapping_add(ch.regs.src_step);
            }
            Aop::DstStep(c) => {
                let ch = &mut self.ch[c as usize];
                self.pd = ch.dst_work;
                ch.dst_work = ch.dst_work.wrapping_add(ch.regs.dst_step);
            }
            Aop::Count(c) => {
                let ch = &mut self.ch[c as usize];
                // 作業値＋0xFFFF の桁上げ。0 なら作業値は 0、つまり最後の要素。
                let cy = ch.count_work != 0;
                ch.count_work = ch.count_work.wrapping_sub(1);
                if !cy {
                    ch.no_more = true;
                }
                let target = if self.elem.is_some_and(|e| e.ch == c && !e.count_done) {
                    &mut self.elem
                } else {
                    &mut self.tail
                };
                if let Some(e) = target {
                    e.count_done = true;
                    e.last = !cy;
                }
            }
            Aop::Reload(c, k) => {
                let present = |p: u8| !matches!(p, 8 | 10..=15) || ports.module_present(p);
                let ch = &mut self.ch[c as usize];
                let r = ch.regs;
                match k {
                    0 => {
                        ch.src_work = ch.src_set;
                        ch.src_ok = port_ok(r.src, Dir::Read, r.format, doc) && present(r.src);
                        ch.phase = Phase::Reload(1);
                    }
                    1 => {
                        ch.dst_work = ch.dst_set;
                        ch.dst_ok = port_ok(r.dst, Dir::Write, r.format, doc) && present(r.dst);
                        ch.phase = Phase::Reload(2);
                    }
                    _ => {
                        ch.count_work = ch.count_set.wrapping_sub(1);
                        if !(ch.src_ok && ch.dst_ok) {
                            ch.error = true;
                            ch.phase = Phase::Idle;
                        } else if ch.count_set == 0 {
                            // 桁上げが 0：COUNT の設定値が 0。転送せずに DONE（DR-046 (9)）。
                            ch.done = true;
                            ch.phase = if ch.looping() {
                                Phase::Reload(0)
                            } else {
                                Phase::Idle
                            };
                        } else {
                            ch.phase = Phase::Run;
                            ch.no_more = false;
                        }
                    }
                }
            }
        }
    }

    /// ブロックの最後の要素が終わった。DONE にし、LOOP なら設定値で再開する。
    fn block_end(&mut self, c: usize) {
        let ch = &mut self.ch[c];
        ch.done = true;
        ch.stall = false;
        ch.armed = false;
        ch.phase = if ch.looping() {
            Phase::Reload(0)
        } else {
            Phase::Idle
        };
    }
}
