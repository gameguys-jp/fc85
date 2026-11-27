//! 本体の番地とレジスタ（68000 から見た MMIO）。生成したファイルで、手で直さない。
//!
//! 元は `docs/spec/registers.toml`（DR-100）。作り直すには、リポジトリの根で `py tools/regmap/regmap.py` を実行する。古くないかは
//! `py tools/regmap/regmap.py --check` で確かめる。名前と値は `sdk/include/fc85.inc`・`fc85.h` と同じ。
//! 表（`WINDOWS`、`REGIONS`、`REGISTERS`）は、試験 `tests/regmap.rs` が本体のバス（`console::System`）の
//! 解読と突き合わせる。

/// アクセスの向き。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// r：読出し。書込みはバスエラーにならない（ROM と I/O のレジスタは捨てる。DR-052、DR-053）。
    R,
    /// w：書込み。読出しはバスエラーにならない（I/O のレジスタは 0 が読める。DR-046、DR-053）。
    W,
    /// rw：読み書き。
    Rw,
}

impl Access {
    /// 読出しの向きがあるか。
    pub const fn reads(self) -> bool {
        matches!(self, Self::R | Self::Rw)
    }

    /// 書込みの向きがあるか。
    pub const fn writes(self) -> bool {
        matches!(self, Self::W | Self::Rw)
    }
}

/// 使える条件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// always：いつでも。
    Always,
    /// doc：DOC の子基板がある（FABRIC_MODULES の bit 2）。ないと窓はバスエラー（DR-019）。
    Doc,
    /// no_doc：DOC の子基板がない。あると窓はバスエラー（DR-019）。
    NoDoc,
    /// dsp_prgbus：DSP をリセットし PRGBUS＝1 の間。ほかはバスエラー（DR-029）。
    DspPrgbus,
    /// cart：カートリッジが差してある（INPUT_IRQ_STATUS の bit 4）。ないと窓はバスエラー（DR-095）。
    Cart,
}

/// 窓の中で表にない番地。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unlisted {
    /// 表にない番地はバスエラー（DR-052、DR-053）。
    BusError,
    /// 窓の中の番地はすべて IC に届く（DOC のネイティブのレジスタ 256 個）。
    Chip,
}

/// メモリの領域のアクセスの幅。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    /// byte と語。
    Any,
    /// 語だけ（byte はバスエラー）。
    Word,
    /// 上位バイトレーン（偶数番地の byte）。
    Upper,
}

/// アドレスマップの窓。窓の外の番地はバスエラー。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    /// 名前（メモリは領域の名前）。
    pub name: &'static str,
    /// 最初の番地。
    pub first: u32,
    /// 最後の番地（含む）。
    pub last: u32,
    /// 窓の中で表にない番地。
    pub unlisted: Unlisted,
    /// 使える条件。満たさなければ窓はバスエラー。
    pub when: When,
}

/// メモリの領域。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    /// 名前（fc85.inc と同じ）。
    pub name: &'static str,
    /// 最初の番地。
    pub first: u32,
    /// 最後の番地（含む）。
    pub last: u32,
    /// アクセスの向き。
    pub access: Access,
    /// アクセスの幅。
    pub width: Width,
    /// 使える条件。
    pub when: When,
}

/// レジスタ。並び（`count` 個）は `stride` byte ごと。Data Fabric のチャネルのレジスタは、チャネル 0 の番地と、
/// チャネルの数と間隔で表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Register {
    /// 名前（fc85.inc と同じ。Data Fabric のチャネルのレジスタは offset の名前）。
    pub name: &'static str,
    /// 番地（並びは最初の要素）。
    pub addr: u32,
    /// 幅：8 は上位バイトレーンの 1 byte（偶数番地）、16 は語、32 は 2 語。
    pub bits: u8,
    /// アクセスの向き。
    pub access: Access,
    /// 並びの要素の数。
    pub count: u32,
    /// 並びの要素の間隔（byte）。
    pub stride: u32,
    /// W1C のビット（1 を書くと消える）。
    pub w1c: u16,
    /// 読むと変わること。
    pub read_effect: Option<&'static str>,
    /// 使える条件。満たさなければバスエラー。
    pub when: When,
}

impl Register {
    /// 並びの要素の番地。
    pub fn addrs(&self) -> impl Iterator<Item = u32> {
        let (addr, stride) = (self.addr, self.stride);
        (0..self.count).map(move |i| addr + i * stride)
    }

    /// 1 要素がバスの上で占める byte 数（8 bit の装置も語の枠を占める）。
    pub const fn span(&self) -> u32 {
        if self.bits == 32 { 4 } else { 2 }
    }
}

/// アドレスマップの窓（番地の順）。
pub const WINDOWS: &[Window] = &[
    Window { name: "BOOT_ROM", first: 0x000000, last: 0x00FFFF, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "CHAR_ROM", first: 0x010000, last: 0x01FFFF, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "MAIN_RAM", first: 0x100000, last: 0x11FFFF, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "STAGING", first: 0x120000, last: 0x12FFFF, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "ACRTC", first: 0x200000, last: 0x200003, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "DOC", first: 0x210000, last: 0x2101FF, unlisted: Unlisted::Chip, when: When::Doc },
    Window { name: "DSP bridge", first: 0x220000, last: 0x22001F, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "ADPCM bridge", first: 0x230000, last: 0x23001F, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "Temporal", first: 0x240000, last: 0x24003F, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "Data Fabric", first: 0x250000, last: 0x250FFF, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "Controller / MIDI", first: 0x260000, last: 0x2600FF, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "Palette", first: 0x270000, last: 0x27003F, unlisted: Unlisted::BusError, when: When::Always },
    Window { name: "DSP program", first: 0x280000, last: 0x281FFF, unlisted: Unlisted::BusError, when: When::DspPrgbus },
    Window { name: "CART", first: 0x400000, last: 0x4FFFFF, unlisted: Unlisted::BusError, when: When::Cart },
];

/// メモリの領域。
pub const REGIONS: &[Region] = &[
    Region { name: "BOOT_ROM", first: 0x000000, last: 0x00FFFF, access: Access::R, width: Width::Any, when: When::Always },
    Region { name: "CHAR_ROM", first: 0x010000, last: 0x01FFFF, access: Access::R, width: Width::Upper, when: When::Always },
    Region { name: "MAIN_RAM", first: 0x100000, last: 0x11FFFF, access: Access::Rw, width: Width::Any, when: When::Always },
    Region { name: "JUMP_TABLE", first: 0x100000, last: 0x1005FF, access: Access::Rw, width: Width::Any, when: When::Always },
    Region { name: "STAGING", first: 0x120000, last: 0x12FFFF, access: Access::Rw, width: Width::Any, when: When::Always },
    Region { name: "CART", first: 0x400000, last: 0x4FFFFF, access: Access::R, width: Width::Any, when: When::Cart },
];

/// レジスタ。
pub const REGISTERS: &[Register] = &[
    Register { name: "ACRTC_AR", addr: 0x200000, bits: 16, access: Access::W, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "ACRTC_SR", addr: 0x200000, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "ACRTC_DATA", addr: 0x200002, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: Some("r00 では読出し FIFO から 1 語を取る"), when: When::Always },
    Register { name: "DOC_FREQ_LO", addr: 0x210000, bits: 8, access: Access::Rw, count: 32, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Doc },
    Register { name: "DOC_FREQ_HI", addr: 0x210040, bits: 8, access: Access::Rw, count: 32, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Doc },
    Register { name: "DOC_VOLUME", addr: 0x210080, bits: 8, access: Access::Rw, count: 32, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Doc },
    Register { name: "DOC_DATA", addr: 0x2100C0, bits: 8, access: Access::R, count: 32, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Doc },
    Register { name: "DOC_POINTER", addr: 0x210100, bits: 8, access: Access::Rw, count: 32, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Doc },
    Register { name: "DOC_CONTROL", addr: 0x210140, bits: 8, access: Access::Rw, count: 32, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Doc },
    Register { name: "DOC_SIZE", addr: 0x210180, bits: 8, access: Access::Rw, count: 32, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Doc },
    Register { name: "DOC_IRQ", addr: 0x2101C0, bits: 8, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: Some("割込みの要求を 1 つ取る"), when: When::Doc },
    Register { name: "DOC_OSC_ENABLE", addr: 0x2101C2, bits: 8, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Doc },
    Register { name: "DOC_ADC", addr: 0x2101C4, bits: 8, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: Some("A/D の変換を始める"), when: When::Doc },
    Register { name: "DSP_CTRL", addr: 0x220000, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "DSP_STATUS", addr: 0x220002, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0060, read_effect: None, when: When::Always },
    Register { name: "DSP_MAILBOX", addr: 0x220004, bits: 16, access: Access::W, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "DSP_RESPONSE", addr: 0x220006, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: Some("RESPONSE と割込みの要求を下げる（DR-053）"), when: When::Always },
    Register { name: "DSP_PROGRAM", addr: 0x280000, bits: 16, access: Access::Rw, count: 4096, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::DspPrgbus },
    Register { name: "ADPCM_CTRL", addr: 0x230000, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "ADPCM_STATUS", addr: 0x230002, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0100, read_effect: None, when: When::Always },
    Register { name: "DAC_CTRL", addr: 0x230010, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::NoDoc },
    Register { name: "DAC_STATUS", addr: 0x230012, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0100, read_effect: None, when: When::NoDoc },
    Register { name: "DAC_DATA", addr: 0x230014, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::NoDoc },
    Register { name: "TEMPORAL_WPTR", addr: 0x240000, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "TEMPORAL_RPTR", addr: 0x240002, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "TEMPORAL_CTRL", addr: 0x240004, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0300, read_effect: None, when: When::Always },
    Register { name: "TEMPORAL_DIST", addr: 0x240006, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_SRC", addr: 0x250000, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_DST", addr: 0x250002, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_SRC_ADDR", addr: 0x250004, bits: 32, access: Access::Rw, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_DST_ADDR", addr: 0x250008, bits: 32, access: Access::Rw, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_COUNT", addr: 0x25000C, bits: 16, access: Access::Rw, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_SRC_STEP", addr: 0x25000E, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_DST_STEP", addr: 0x250010, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_FORMAT", addr: 0x250012, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_MASK", addr: 0x250014, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_XOR", addr: 0x250016, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_SHIFT", addr: 0x250018, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_TRIGGER", addr: 0x25001A, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_CTRL", addr: 0x25001C, bits: 16, access: Access::W, count: 2, stride: 0x40, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FAB_STATUS", addr: 0x25001E, bits: 16, access: Access::R, count: 2, stride: 0x40, w1c: 0x001A, read_effect: None, when: When::Always },
    Register { name: "FABRIC_PORT_RESET", addr: 0x250100, bits: 16, access: Access::W, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FABRIC_AUDIO_N", addr: 0x250102, bits: 16, access: Access::W, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "FABRIC_MODULES", addr: 0x250104, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "PORT_A", addr: 0x260000, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "PORT_B", addr: 0x260002, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "AXIS0", addr: 0x260004, bits: 16, access: Access::R, count: 8, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "MIDI_CTRL", addr: 0x260014, bits: 8, access: Access::W, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "MIDI_STATUS", addr: 0x260014, bits: 8, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "MIDI_DATA", addr: 0x260016, bits: 8, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: Some("受けた byte を取り、RDRF を下げる"), when: When::Always },
    Register { name: "INPUT_IRQ_STATUS", addr: 0x260018, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x000B, read_effect: None, when: When::Always },
    Register { name: "INPUT_IRQ_ENABLE", addr: 0x26001A, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "PALETTE", addr: 0x270000, bits: 16, access: Access::W, count: 16, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "PAL_CTRL", addr: 0x270020, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "VIDEO_IRQ", addr: 0x270022, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0300, read_effect: None, when: When::Always },
    Register { name: "VIDEO_LINE", addr: 0x270024, bits: 16, access: Access::R, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
    Register { name: "VIDEO_LINE_CMP", addr: 0x270026, bits: 16, access: Access::Rw, count: 1, stride: 0x2, w1c: 0x0000, read_effect: None, when: When::Always },
];

// ------------------------------------------------------------ メモリ（§05）[P]

/// 64 KiB
pub const BOOT_ROM: u32 = 0x000000;
/// 文字 ROM。ROM の offset n は CHAR_ROM+2n（上位レーン）
pub const CHAR_ROM: u32 = 0x010000;
/// 128 KiB
pub const MAIN_RAM: u32 = 0x100000;
pub const MAIN_RAM_END: u32 = 0x120000;
/// 例外のジャンプ表。ベクタ n は JUMP_TABLE+6n（JMP abs.L）（DR-025）
pub const JUMP_TABLE: u32 = 0x100000;
pub const JUMP_TABLE_END: u32 = 0x100600;
/// 64 KiB の AV staging RAM（Fabric の port 0）
pub const STAGING: u32 = 0x120000;
pub const STAGING_END: u32 = 0x130000;
/// カートリッジの窓（最大 1 MiB）。なければバスエラー
pub const CART: u32 = 0x400000;
pub const CART_END: u32 = 0x500000;

// ------------------------------------------------------------ ACRTC HD63484（§05、DR-016、DR-033）[P]

/// 書込み：AR（レジスタの番号）
pub const ACRTC_AR: u32 = 0x200000;
/// 読出し：SR（上位バイトは 1）
pub const ACRTC_SR: u32 = 0x200000;
/// AR が指すレジスタ。r00 は FIFO
pub const ACRTC_DATA: u32 = 0x200002;

// SR のビット（[15]）[D]
/// コマンドの誤り
pub const ACRTC_SR_CER: u16 = 0x80;
/// 領域の検出
pub const ACRTC_SR_ARD: u16 = 0x40;
/// コマンドの終わり
pub const ACRTC_SR_CED: u16 = 0x20;
/// ライトペン
pub const ACRTC_SR_LPD: u16 = 0x10;
/// 読出し FIFO が満杯
pub const ACRTC_SR_RFF: u16 = 0x08;
/// 読出し FIFO に語がある
pub const ACRTC_SR_RFR: u16 = 0x04;
/// 書込み FIFO に空きがある
pub const ACRTC_SR_WFR: u16 = 0x02;
/// 書込み FIFO が空
pub const ACRTC_SR_WFE: u16 = 0x01;

// SR のビット番号（BTST 用）（[15]）[D]
pub const ACRTC_SR_CER_B: u32 = 7;
pub const ACRTC_SR_CED_B: u32 = 5;
pub const ACRTC_SR_RFR_B: u32 = 2;
pub const ACRTC_SR_WFR_B: u32 = 1;
pub const ACRTC_SR_WFE_B: u32 = 0;

// レジスタの番号（AR に書く値）。r80 以降は語ごとに AR が 2 進む（[15]）[D]
pub const ACRTC_FIFO: u16 = 0x00;
/// コマンドの制御
pub const ACRTC_CCR: u16 = 0x02;
/// 動作モード
pub const ACRTC_OMR: u16 = 0x04;
/// 表示の制御
pub const ACRTC_DCR: u16 = 0x06;
/// ラスタの数（読出し）
pub const ACRTC_RCR: u16 = 0x80;
/// 水平同期
pub const ACRTC_HSR: u16 = 0x82;
/// 水平表示
pub const ACRTC_HDR: u16 = 0x84;
/// 垂直同期
pub const ACRTC_VSR: u16 = 0x86;
/// 垂直表示
pub const ACRTC_VDR: u16 = 0x88;
/// 分割画面の幅 1
pub const ACRTC_SSW: u16 = 0x8A;
/// 上画面のラスタの数（U85 p.91）
pub const ACRTC_SSW0: u16 = 0x8C;
/// 下画面のラスタの数
pub const ACRTC_SSW2: u16 = 0x8E;
/// 属性の点滅：BON1・BOFF1・BON2・BOFF2（4 bit ずつ、U85 p.92–93）
pub const ACRTC_BCR: u16 = 0x90;
/// ウィンドウの横：上位 HWS−1（HSYNC の終わりから）、下位 HWW−1
pub const ACRTC_HWR: u16 = 0x92;
/// ウィンドウの縦の始まり−1（VSYNC の終わりから、U85 p.89–90）
pub const ACRTC_VWS: u16 = 0x94;
/// ウィンドウのラスタの数
pub const ACRTC_VWW: u16 = 0x96;
/// グラフィックカーソル：上位 CXE、下位 CXS（HSYNC の始まりからのメモリサイクル）
pub const ACRTC_GCRX: u16 = 0x98;
/// CYS（VSYNC の終わりからのラスタ、U85 p.94）
pub const ACRTC_GCRYS: u16 = 0x9A;
/// CYE
pub const ACRTC_GCRYE: u16 = 0x9C;
/// 上画面の記憶の幅
pub const ACRTC_MWR0: u16 = 0xC2;
pub const ACRTC_SAR0H: u16 = 0xC4;
pub const ACRTC_SAR0L: u16 = 0xC6;
/// 基本画面の記憶の幅
pub const ACRTC_MWR1: u16 = 0xCA;
/// 基本画面の開始番地（bit 11–8 は SDA：横のスムーズスクロール）
pub const ACRTC_SAR1H: u16 = 0xCC;
pub const ACRTC_SAR1L: u16 = 0xCE;
/// 下画面
pub const ACRTC_MWR2: u16 = 0xD2;
pub const ACRTC_SAR2H: u16 = 0xD4;
pub const ACRTC_SAR2L: u16 = 0xD6;
/// ウィンドウ
pub const ACRTC_MWR3: u16 = 0xDA;
pub const ACRTC_SAR3H: u16 = 0xDC;
pub const ACRTC_SAR3L: u16 = 0xDE;
/// カーソルの定義（U85 p.107–109）
pub const ACRTC_CDR: u16 = 0xE8;
/// ズーム：bit 15–12 HZF、bit 11–8 VZF（倍率−1、基本画面だけ、U85 p.110）
pub const ACRTC_ZFR: u16 = 0xEA;

// CCR のビット（[15]）[D]
/// 打切り（リセットの後は 1）
pub const ACRTC_CCR_ABT: u16 = 0x8000;
pub const ACRTC_CCR_PSE: u16 = 0x4000;
/// データの DMA（DWT・DRD・DMOD を Fabric の port 1 で運ぶ）
pub const ACRTC_CCR_DDM: u16 = 0x2000;
pub const ACRTC_CCR_CDM: u16 = 0x1000;
/// DMA の要求：1 はサイクルスチール
pub const ACRTC_CCR_DRC: u16 = 0x0800;
/// 4 bit/画素
pub const ACRTC_CCR_GBM4: u16 = 0x0200;

// OMR のビット（[15]）[D]
/// マスタ
pub const ACRTC_OMR_MS: u16 = 0x8000;
/// 表示と描画を始める
pub const ACRTC_OMR_STR: u16 = 0x4000;
/// インタリーブ
pub const ACRTC_OMR_ACM_I: u16 = 0x0008;
/// 単一アクセスで描画を優先
pub const ACRTC_OMR_ACP: u16 = 0x2000;
/// カーソルのスキュー（×1〜3 メモリサイクル。十字では 0 にしない）
pub const ACRTC_OMR_CSK1: u16 = 0x0400;
/// DISP のスキュー（×1〜3 メモリサイクル）
pub const ACRTC_OMR_DSK1: u16 = 0x0100;
/// 1：HSYNC の間に DRAM をリフレッシュしない
pub const ACRTC_OMR_RAM: u16 = 0x0080;

// DCR のビット（U85 p.75–78）。SE は 0x 使わない、10 ブランク、11 表示（基本画面の SE1 は 1 bit）（[15]）[D]
pub const ACRTC_DCR_DSP: u16 = 0x8000;
/// 基本画面を表示（BIOS が書く値。DR-071）
pub const ACRTC_DCR_BASE: u16 = 0x4000;
/// 上画面を表示（ブランクは $2000）
pub const ACRTC_DCR_UPPER: u16 = 0x3000;
/// 下画面を表示（ブランクは $0800）
pub const ACRTC_DCR_LOWER: u16 = 0x0C00;
/// ウィンドウを表示（ブランクは $0200）
pub const ACRTC_DCR_WIN: u16 = 0x0300;

// CDR のビット（U85 p.107–109）。CON・COFF は 4 フィールド単位（[15]）[D]
/// GCR のカーソルを CUD1 に
pub const ACRTC_CDR_GRAPHIC: u16 = 0x8000;
/// 十字（横が CUD1、縦が CUD2）
pub const ACRTC_CDR_CROSS: u16 = 0xC000;
/// ×1〜7：ON は 4×(n＋1) フィールド。0 は出さない
pub const ACRTC_CDR_CON1: u16 = 0x0800;
/// ×1〜7：OFF は 4×(n＋1) フィールド。0 は常に ON
pub const ACRTC_CDR_COFF1: u16 = 0x0100;

// 案B の表示（256×192、4 bpp、262 ライン、台帳 HD63484.md §3.4）[P]
pub const ACRTC_B_HSR: u16 = 0xAA0D;
pub const ACRTC_B_HDR: u16 = 0x127F;
pub const ACRTC_B_VSR: u16 = 0x0106;
pub const ACRTC_B_VDR: u16 = 0x2703;
pub const ACRTC_B_SSW: u16 = 0x00C0;
/// 1 行 64 語（256 画素）
pub const ACRTC_B_MW: u16 = 0x0040;
/// M/S＝1、STR＝1、ACM＝10（DSK・CSK は 0）
pub const ACRTC_B_OMR: u16 = 0xC008;

// コマンド（U85 p.173）。下位ビット：OPM（2–0）、COL（4–3）、AREA（7–5）（[15]）[D]
/// DPH, DPL
pub const ACRTC_ORG: u16 = 0x0400;
/// | rn、data
pub const ACRTC_WPR: u16 = 0x0800;
/// | rn
pub const ACRTC_RPR: u16 = 0x0C00;
/// | pra、n、data×n
pub const ACRTC_WPTN: u16 = 0x1800;
/// | pra、n
pub const ACRTC_RPTN: u16 = 0x1C00;
/// AX, AY
pub const ACRTC_DRD: u16 = 0x2400;
/// AX, AY、data×(|AX|+1)(|AY|+1)
pub const ACRTC_DWT: u16 = 0x2800;
/// | mm、AX, AY
pub const ACRTC_DMOD: u16 = 0x2C00;
pub const ACRTC_RD: u16 = 0x4400;
/// data
pub const ACRTC_WT: u16 = 0x4800;
/// | mm、data
pub const ACRTC_MOD: u16 = 0x4C00;
/// data, AX, AY（RWP から）
pub const ACRTC_CLR: u16 = 0x5800;
/// | mm、data, AX, AY
pub const ACRTC_SCLR: u16 = 0x5C00;
/// | S | DSD、Sah, Sal, AX, AY
pub const ACRTC_CPY: u16 = 0x6000;
pub const ACRTC_SCPY: u16 = 0x7000;
/// X, Y
pub const ACRTC_AMOVE: u16 = 0x8000;
/// dX, dY
pub const ACRTC_RMOVE: u16 = 0x8400;
/// X, Y
pub const ACRTC_ALINE: u16 = 0x8800;
/// dX, dY
pub const ACRTC_RLINE: u16 = 0x8C00;
/// X, Y
pub const ACRTC_ARCT: u16 = 0x9000;
/// dX, dY
pub const ACRTC_RRCT: u16 = 0x9400;
/// n, X1, Y1, ...
pub const ACRTC_APLL: u16 = 0x9800;
pub const ACRTC_RPLL: u16 = 0x9C00;
pub const ACRTC_APLG: u16 = 0xA000;
pub const ACRTC_RPLG: u16 = 0xA400;
/// r（| ACRTC_CW で時計回り）
pub const ACRTC_CRCL: u16 = 0xA800;
/// a, b, dX
pub const ACRTC_ELPS: u16 = 0xAC00;
pub const ACRTC_AARC: u16 = 0xB000;
pub const ACRTC_RARC: u16 = 0xB400;
pub const ACRTC_AEARC: u16 = 0xB800;
pub const ACRTC_REARC: u16 = 0xBC00;
/// X, Y（CP から X・Y までを塗る）
pub const ACRTC_AFRCT: u16 = 0xC000;
/// dX, dY
pub const ACRTC_RFRCT: u16 = 0xC400;
pub const ACRTC_PAINT: u16 = 0xC800;
pub const ACRTC_DOT: u16 = 0xCC00;
/// | S | SD、SZ
pub const ACRTC_PTN: u16 = 0xD000;
pub const ACRTC_AGCPY: u16 = 0xE000;
pub const ACRTC_RGCPY: u16 = 0xF000;
/// 円・弧：時計回り
pub const ACRTC_CW: u16 = 0x0100;

// 演算（OPM）（[15]）[D]
/// 置き換え
pub const ACRTC_OPM_REP: u16 = 0;
pub const ACRTC_OPM_OR: u16 = 1;
pub const ACRTC_OPM_AND: u16 = 2;
pub const ACRTC_OPM_EOR: u16 = 3;

// 色（COL）（[15]）[D]
/// パターンのビットで CL0・CL1
pub const ACRTC_COL_PAT: u16 = 0x00;
/// 1 は CL1、0 は描かない
pub const ACRTC_COL_CL1: u16 = 0x08;
/// 0 は CL0、1 は描かない
pub const ACRTC_COL_CL0: u16 = 0x10;
/// パターン RAM の語をそのまま
pub const ACRTC_COL_DIR: u16 = 0x18;

// パラメータレジスタ（WPR の rn）（[15]）[D]
pub const ACRTC_P_CL0: u16 = 0x00;
pub const ACRTC_P_CL1: u16 = 0x01;
pub const ACRTC_P_CCMP: u16 = 0x02;
pub const ACRTC_P_EDG: u16 = 0x03;
pub const ACRTC_P_MASK: u16 = 0x04;
pub const ACRTC_P_PRC0: u16 = 0x05;
pub const ACRTC_P_PRC1: u16 = 0x06;
pub const ACRTC_P_PRC2: u16 = 0x07;
pub const ACRTC_P_ADR_XMIN: u16 = 0x08;
pub const ACRTC_P_ADR_YMIN: u16 = 0x09;
pub const ACRTC_P_ADR_XMAX: u16 = 0x0A;
pub const ACRTC_P_ADR_YMAX: u16 = 0x0B;
/// DN<<14 | 語番地の bit 19–12
pub const ACRTC_P_RWPH: u16 = 0x0C;
/// 語番地の bit 11–0 << 4
pub const ACRTC_P_RWPL: u16 = 0x0D;

// ------------------------------------------------------------ DOC ES5503（§05、DR-019、DR-030）[P]
// 子基板。ネイティブのレジスタ n は DOC+2n の上位バイト。MOVE.B の 1 回の読みか書きだけ。子基板がなければバスエラー（FABRIC_MODULES の bit 2 で確かめる）。波形 RAM は Fabric の port 2 からだけ読み書きできる。

pub const DOC: u32 = 0x210000;
/// +2×osc（DOC+2*0x00）
pub const DOC_FREQ_LO: u32 = 0x210000;
/// DOC+2*0x20
pub const DOC_FREQ_HI: u32 = 0x210040;
/// DOC+2*0x40
pub const DOC_VOLUME: u32 = 0x210080;
/// 最後に読んだ波形の値（読出し）（DOC+2*0x60）
pub const DOC_DATA: u32 = 0x2100C0;
/// 波形テーブルの頁（番地の bit 15–8）（DOC+2*0x80）
pub const DOC_POINTER: u32 = 0x210100;
/// DOC+2*0xA0
pub const DOC_CONTROL: u32 = 0x210140;
/// bit 6 BS、bit 5–3 テーブルの大きさ T、bit 2–0 分解能 R（DOC+2*0xC0）
pub const DOC_SIZE: u32 = 0x210180;
/// 読むと割込みの要求を 1 つ取る（DOC の割込みを受ける処理だけが、1 回につき 1 回読む。DR-123）（DOC+2*0xE0）
pub const DOC_IRQ: u32 = 0x2101C0;
/// (N−1)×2。書くと走査を始める（DOC+2*0xE1）
pub const DOC_OSC_ENABLE: u32 = 0x2101C2;
/// 読むと A/D の変換を始め、前の変換の結果を返す。読んで 30 µs 以上待ってから読み直す（DR-123）（DOC+2*0xE2）
pub const DOC_ADC: u32 = 0x2101C4;

// オシレータの並び
/// オシレータ 1 つぶんの番地の差
pub const DOC_OSC_STRIDE: u32 = 2;

// CONTROL のビット（[18]）[D]
pub const DOC_CTL_HALT: u8 = 0x01;
/// モード（bit 2–1）
pub const DOC_CTL_FREE: u8 = 0x00;
pub const DOC_CTL_ONESHOT: u8 = 0x02;
pub const DOC_CTL_SYNC: u8 = 0x04;
pub const DOC_CTL_SWAP: u8 = 0x06;
pub const DOC_CTL_IE: u8 = 0x08;
/// CA3–0（bit 7–4）。偶数は L、奇数は R（DR-054）
pub const DOC_CTL_CA_SH: u32 = 4;

// ------------------------------------------------------------ DSP bridge（§05、DR-029、DR-049、DR-053）[P]

/// bit 0 RESET、bit 2–1 BIOSEL、bit 3 IRQ_EN
pub const DSP_CTRL: u32 = 0x220000;
/// bit 0 PRGBUS、1 入力の語あり、2 出力に空き、3 COMMAND、4 RESPONSE、5 UNDERRUN（W1C）、6 OVERRUN（W1C）
pub const DSP_STATUS: u32 = 0x220002;
pub const DSP_MAILBOX: u32 = 0x220004;
pub const DSP_RESPONSE: u32 = 0x220006;
/// 4K 語。DSP をリセットし PRGBUS＝1 の間だけ（語アクセス）
pub const DSP_PROGRAM: u32 = 0x280000;

// CTRL のビット
pub const DSP_CTRL_RESET: u16 = 0x0001;
pub const DSP_CTRL_IRQ_EN: u16 = 0x0008;

// CTRL の BIOSEL（bit 2–1）：DSP の BIO に出す条件（DR-029 (3)、DR-049）
/// 入力 FIFO が空でない（既定）
pub const DSP_CTRL_BIO_INPUT: u16 = 0x0000;
/// 出力 FIFO に空き
pub const DSP_CTRL_BIO_SPACE: u16 = 0x0002;
/// メールボックスに着信
pub const DSP_CTRL_BIO_MAIL: u16 = 0x0004;
/// 音声クロックの拍のラッチ
pub const DSP_CTRL_BIO_TICK: u16 = 0x0006;

// STATUS のビット番号（BTST 用。MOVE.W で読んでから調べる。バイトのアクセスはバスエラー）
pub const DSP_ST_PRGBUS_B: u32 = 0;
pub const DSP_ST_INPUT_B: u32 = 1;
pub const DSP_ST_SPACE_B: u32 = 2;
pub const DSP_ST_COMMAND_B: u32 = 3;
pub const DSP_ST_RESPONSE_B: u32 = 4;

// STATUS の W1C のビット
/// W1C
pub const DSP_ST_UNDERRUN: u16 = 0x0020;
/// W1C
pub const DSP_ST_OVERRUN: u16 = 0x0040;

// プログラム RAM の大きさ
pub const DSP_PROGRAM_WORDS: u32 = 4096;

// ------------------------------------------------------------ ADPCM bridge と本体 DAC（§05、DR-017、DR-019、DR-053）[P]

/// bit 0 RESET、1 S1、2 S2、3 下位ニブルから、4 置換しない、5 半分空きの割込み
pub const ADPCM_CTRL: u32 = 0x230000;
/// bit 6–0 FIFO の byte 数（0〜64）、bit 7 は 0、bit 8 UNDERRUN（W1C）、bit 9 半分空き、bit 10 V̄C̄K̄
pub const ADPCM_STATUS: u32 = 0x230002;
/// bit 0 DIRECT。DOC の子基板があるときはバスエラー
pub const DAC_CTRL: u32 = 0x230010;
/// bit 6–0 FIFO の語数（0〜64）、bit 7 は 0、bit 8 UNDERRUN（W1C）
pub const DAC_STATUS: u32 = 0x230012;
/// DIRECT のとき：上位バイト L、下位バイト R（符号なし、$80 が中央）
pub const DAC_DATA: u32 = 0x230014;

// 本体 DAC の CTRL と DATA
pub const DAC_CTRL_DIRECT: u16 = 0x0001;
/// DATA の中央（L・R とも $80）
pub const DAC_CENTER: u16 = 0x8080;

// ADPCM の CTRL のビット（DR-053）。S1・S2 は MSM5205 の標本化：S1＝1・S2＝0 で 8 kHz、S1＝0・S2＝1 で 6 kHz、両方 0 で 4 kHz（384 kHz の 1/48・1/64・1/96、台帳 MSM5205.md §3.1）。両方 1 は Bend（V̄C̄K̄ が入力になる。エミュレータでは取込みが起きない [V]）。RESET は電源投入時 1
pub const ADPCM_CTL_RESET: u16 = 0x0001;
pub const ADPCM_CTL_S1: u16 = 0x0002;
pub const ADPCM_CTL_S2: u16 = 0x0004;
/// ニブルを下位から
pub const ADPCM_CTL_LOW_FIRST: u16 = 0x0008;
/// FIFO が空でも 0x08 に置き換えない（Bend）
pub const ADPCM_CTL_NO_REPLACE: u16 = 0x0010;
/// 半分空きの割込み（レベル 3）
pub const ADPCM_CTL_HALF_IRQ: u16 = 0x0020;
/// ADPCM_CTL_S1
pub const ADPCM_CTL_8K: u16 = 0x0002;
/// ADPCM_CTL_S2
pub const ADPCM_CTL_6K: u16 = 0x0004;
pub const ADPCM_CTL_4K: u16 = 0x0000;

// ADPCM の STATUS のビット（DR-053、DR-096）
/// FIFO の byte 数
pub const ADPCM_ST_COUNT: u16 = 0x007F;
/// W1C
pub const ADPCM_ST_UNDERRUN: u16 = 0x0100;
/// 残りが 32 byte 以下
pub const ADPCM_ST_HALF: u16 = 0x0200;
/// V̄C̄K̄ のピンのレベル（DR-096）。RESET を外すのは、1→0 を見てから 16 µs より後、次の 0→1 の前（取込みと立上りの間。OI-064）
pub const ADPCM_ST_VCK: u16 = 0x0400;

// ------------------------------------------------------------ Temporal（§05、DR-053、DR-067）[P]
// 0x240020〜0x24003F は Capture RAM [O]。実装しないので、今はバスエラー。

pub const TEMPORAL_WPTR: u32 = 0x240000;
pub const TEMPORAL_RPTR: u32 = 0x240002;
/// 下の TMP_*（DR-067）
pub const TEMPORAL_CTRL: u32 = 0x240004;
/// WPTR−RPTR
pub const TEMPORAL_DIST: u32 = 0x240006;

// CTRL のビット
/// 両方のポインタと印を 0 に
pub const TMP_CLEAR: u16 = 0x0001;
/// VBlank の始まりで両方のポインタを 0 に
pub const TMP_SYNC: u16 = 0x0002;
/// VBlank の始まりで読出しのポインタだけを 0 に（繰返し）
pub const TMP_RSYNC: u16 = 0x0004;
/// 書込みの /WE を止める。ポインタは進む（保持）
pub const TMP_HOLD: u16 = 0x0008;
/// 読出し専用：WPTR＝RPTR
pub const TMP_EQUAL: u16 = 0x0010;
/// 書く側が追いついた。W1C
pub const TMP_OVERRUN: u16 = 0x0100;
/// 読む側が追いついた。W1C
pub const TMP_UNDERRUN: u16 = 0x0200;

// ------------------------------------------------------------ Data Fabric（§06、DR-046）[P]

pub const FABRIC: u32 = 0x250000;
pub const FABRIC_CH0: u32 = 0x250000;
pub const FABRIC_CH1: u32 = 0x250040;
pub const FABRIC_CH_SIZE: u32 = 0x40;
// チャネルの中の offset。チャネル ch のレジスタは FABRIC_CH0＋FABRIC_CH_SIZE×ch＋offset。チャネル 2・3 の窓（予約）と、各チャネルの 0x20〜0x3E はバスエラー。
/// ポートの ID
pub const FAB_SRC: u32 = 0x00;
pub const FAB_DST: u32 = 0x02;
/// 32 bit（下位語 $06 だけを使う）
pub const FAB_SRC_ADDR: u32 = 0x04;
pub const FAB_SRC_ADDR_L: u32 = 0x06;
pub const FAB_DST_ADDR: u32 = 0x08;
pub const FAB_DST_ADDR_L: u32 = 0x0A;
pub const FAB_COUNT: u32 = 0x0C;
pub const FAB_SRC_STEP: u32 = 0x0E;
pub const FAB_DST_STEP: u32 = 0x10;
pub const FAB_FORMAT: u32 = 0x12;
pub const FAB_MASK: u32 = 0x14;
pub const FAB_XOR: u32 = 0x16;
pub const FAB_SHIFT: u32 = 0x18;
pub const FAB_TRIGGER: u32 = 0x1A;
pub const FAB_CTRL: u32 = 0x1C;
pub const FAB_STATUS: u32 = 0x1E;
/// bit n：port n の FIFO・ポインタを初期化（書込み専用）
pub const FABRIC_PORT_RESET: u32 = 0x250100;
/// 音声クロック＝8 MHz÷N（12 bit）
pub const FABRIC_AUDIO_N: u32 = 0x250102;
/// 読出し専用
pub const FABRIC_MODULES: u32 = 0x250104;

// MODULES のビット
/// bit 2：DOC 子基板
pub const FABRIC_MOD_DOC: u16 = 0x0004;
pub const FABRIC_MOD_DOC_B: u32 = 2;

// ポートの ID
pub const PORT_STAGING: u16 = 0;
pub const PORT_ACRTC: u16 = 1;
/// 子基板がないときは本体 DAC の FIFO（語、上位 L・下位 R）
pub const PORT_DOC: u16 = 2;
pub const PORT_DSP_IN: u16 = 3;
pub const PORT_DSP_OUT: u16 = 4;
pub const PORT_ADPCM: u16 = 5;
pub const PORT_TEMPORAL_W: u16 = 6;
pub const PORT_TEMPORAL_R: u16 = 7;
pub const PORT_CAPTURE: u16 = 8;
pub const PORT_CHAR_ROM: u16 = 9;
pub const PORT_AUDIO_IN: u16 = 10;

// FORMAT
pub const FMT_8_8: u16 = 0;
pub const FMT_16_16: u16 = 1;
/// ゼロ拡張
pub const FMT_8_16: u16 = 2;
pub const FMT_16_8_LO: u16 = 3;
pub const FMT_16_8_HI: u16 = 4;
/// SHIFT で回転
pub const FMT_16_ROT: u16 = 5;

// TRIGGER の発生源（bit 2–0）（DR-018）
pub const TRG_NOW: u16 = 0;
pub const TRG_VBLANK: u16 = 1;
pub const TRG_HBLANK: u16 = 2;
pub const TRG_AUDIO: u16 = 3;
pub const TRG_EXTRIG: u16 = 4;
pub const TRG_ADPCM_VCK: u16 = 5;

// TRIGGER のビット
/// 1 回のトリガで 1 要素
pub const TRG_EACH: u16 = 0x0008;
pub const TRG_LOOP: u16 = 0x0010;
/// DONE か ERROR で割込み（レベル 3）
pub const TRG_IRQ_EN: u16 = 0x0020;

// CTRL
pub const FAB_START: u16 = 0x0001;
pub const FAB_STOP: u16 = 0x0002;

// STATUS
pub const FAB_ST_BUSY: u16 = 0x0001;
/// W1C
pub const FAB_ST_DONE: u16 = 0x0002;
pub const FAB_ST_STALL: u16 = 0x0004;
/// W1C
pub const FAB_ST_ERROR: u16 = 0x0008;
/// W1C
pub const FAB_ST_MISS: u16 = 0x0010;
pub const FAB_ST_WAIT: u16 = 0x0020;
pub const FAB_ST_W1C: u16 = 0x001A;

// ------------------------------------------------------------ 入力と ACIA（§09、DR-028、DR-053、DR-055）[P]

pub const INPUT: u32 = 0x260000;
/// コントローラ A（active-high）
pub const PORT_A: u32 = 0x260000;
/// コントローラ B（同じ割当て）
pub const PORT_B: u32 = 0x260002;
/// AXIS0〜7：$260004〜$260012（下位 8 bit にアナログ値）
pub const AXIS0: u32 = 0x260004;
/// ACIA の制御（書込み）／状態（読出し）。上位レーン、MOVE.B
pub const MIDI_CTRL: u32 = 0x260014;
/// ACIA の状態
pub const MIDI_STATUS: u32 = 0x260014;
/// ACIA の送受信データ。上位レーン、MOVE.B
pub const MIDI_DATA: u32 = 0x260016;
/// bit 0、1、3 は W1C。bit 2 は ACIA の IRQ の写し、bit 4 はカートリッジ（読出し専用）
pub const INPUT_IRQ_STATUS: u32 = 0x260018;
pub const INPUT_IRQ_ENABLE: u32 = 0x26001A;

// INPUT_IRQ_STATUS・INPUT_IRQ_ENABLE のビット
pub const INPUT_IRQ_PORT_A: u16 = 0x0001;
pub const INPUT_IRQ_PORT_B: u16 = 0x0002;
pub const INPUT_IRQ_MIDI: u16 = 0x0004;
pub const INPUT_IRQ_EXTRIG: u16 = 0x0008;
/// INPUT_IRQ_STATUS だけ：カートリッジがある（端子 /CART。読出し専用、割込みにはならない）[P]（DR-095）
pub const INPUT_IRQ_CART: u16 = 0x0010;
pub const INPUT_IRQ_CART_B: u32 = 4;

// PORT_A・PORT_B のビット（DR-055）。bit 15–8 は予約（0）[P]
pub const PAD_UP: u16 = 0x0001;
pub const PAD_DOWN: u16 = 0x0002;
pub const PAD_LEFT: u16 = 0x0004;
pub const PAD_RIGHT: u16 = 0x0008;
pub const PAD_A: u16 = 0x0010;
pub const PAD_B: u16 = 0x0020;
pub const PAD_C: u16 = 0x0040;
pub const PAD_START: u16 = 0x0080;

// PORT_A・PORT_B のビット番号（BTST 用）[P]
pub const PAD_UP_B: u32 = 0;
pub const PAD_DOWN_B: u32 = 1;
pub const PAD_LEFT_B: u32 = 2;
pub const PAD_RIGHT_B: u32 = 3;
pub const PAD_A_B: u32 = 4;
pub const PAD_B_B: u32 = 5;
pub const PAD_C_B: u32 = 6;
pub const PAD_START_B: u32 = 7;

// HD6850 ACIA の制御（[23]）[D]
/// マスタリセット
pub const ACIA_RESET: u8 = 0x03;
/// 500 kHz÷16＝31.25 kbit/s
pub const ACIA_DIV16: u8 = 0x01;
pub const ACIA_8N1: u8 = 0x14;
/// 受信の割込み
pub const ACIA_RIE: u8 = 0x80;
/// $95（ACIA_RIE|ACIA_8N1|ACIA_DIV16）
pub const ACIA_MIDI: u8 = 0x95;

// HD6850 ACIA の状態（[23]）[D]
pub const ACIA_ST_RDRF: u8 = 0x01;
pub const ACIA_ST_TDRE: u8 = 0x02;
pub const ACIA_ST_OVRN: u8 = 0x20;
pub const ACIA_ST_IRQ: u8 = 0x80;

// ------------------------------------------------------------ パレットと映像（§05、DR-027、DR-053）[P]

/// $0RGB×16。書込み専用、語アクセス。次のラインから効く
pub const PALETTE: u32 = 0x270000;
/// bit 0 DIRECT（Bend）
pub const PAL_CTRL: u32 = 0x270020;
/// 割込みの許可と保留（W1C）、今のレベル。下の VIRQ_*
pub const VIDEO_IRQ: u32 = 0x270022;
/// 今のラスタ
pub const VIDEO_LINE: u32 = 0x270024;
/// 12 bit
pub const VIDEO_LINE_CMP: u32 = 0x270026;

// VIDEO_IRQ のビット
pub const VIRQ_VBL_EN: u16 = 0x0001;
pub const VIRQ_HBL_EN: u16 = 0x0002;
/// HBlank の割込みを LINE_CMP の 1 ラインだけにする
pub const VIRQ_LINE_CMP: u16 = 0x0004;
/// 保留（W1C）
pub const VIRQ_VBL: u16 = 0x0100;
/// 保留（W1C）
pub const VIRQ_HBL: u16 = 0x0200;
/// 今のレベル
pub const VIRQ_HBLANK: u16 = 0x4000;
pub const VIRQ_VBLANK: u16 = 0x8000;

// 画面の大きさ
pub const SCREEN_W: u32 = 256;
pub const SCREEN_H: u32 = 192;

// ------------------------------------------------------------ 割込み（§05、DR-028）[P]
// すべてオートベクタ。レベル L のベクタは 24＋L。

// レベル
/// VBlank・HBlank、ACRTC
pub const IRQ_VIDEO: u32 = 6;
pub const IRQ_ACIA: u32 = 5;
pub const IRQ_DOC: u32 = 4;
/// Data Fabric、ADPCM bridge
pub const IRQ_FABRIC: u32 = 3;
pub const IRQ_DSP: u32 = 2;
pub const IRQ_INPUT: u32 = 1;

// 68000 の例外のベクタの番号（[31]）[D]
pub const VEC_BUS_ERROR: u32 = 2;
pub const VEC_ADDR_ERROR: u32 = 3;
pub const VEC_ILLEGAL: u32 = 4;
pub const VEC_ZERO_DIV: u32 = 5;
pub const VEC_CHK: u32 = 6;
pub const VEC_TRAPV: u32 = 7;
pub const VEC_PRIVILEGE: u32 = 8;
pub const VEC_TRACE: u32 = 9;
pub const VEC_LINE_A: u32 = 10;
pub const VEC_LINE_F: u32 = 11;
pub const VEC_SPURIOUS: u32 = 24;
/// レベル 1〜7：25〜31
pub const VEC_AUTO1: u32 = 25;
/// TRAP #0〜15：32〜47
pub const VEC_TRAP0: u32 = 32;
