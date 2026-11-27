//! fc85 の基板の回路（仕様書 §12 の board 層）。
//!
//! chips 層の IC のコアをつなぐ回路を置く。段階 2 の最初は Data Fabric（仕様書 §06、DR-046、
//! `docs/spec/fabric.md`）と、そのポートの側の部品（Staging RAM、文字 ROM、Temporal RAM、67401 の FIFO）。
//! 次に、ACRTC の DMA（port 1）のつなぎ（`acrtc`）、音声クロックと本体 DAC（`audio`）、ADPCM の
//! アダプタ（`adpcm`）、DSP のアダプタ（`dsp`）、DOC 子基板と port 2 のアダプタ（`doc`）、音声のアナログの段
//! （`analog`。段階 4）、DRAM の保持の模型（`dram`。ベンド点）、映像のクロックのベンド点（`vclock`）と
//! 機械の外のテレビの模型（`tv`）、クロックを上げすぎたときのメモリのアクセス時間の模型（`access`。
//! ベンド点、DR-083）、同期化の FF とメタステーブルの模型（`sync`。DR-088）、本体の番地とレジスタの表
//! （`regmap`。生成物、DR-100）。時刻は core 層の単位で数える。
//! 状態はすべて serde で保存・復元できる（DR-040）。

pub mod access;
pub mod acrtc;
pub mod adpcm;
pub mod analog;
pub mod audio;
pub mod bend;
pub mod console;
pub mod doc;
pub mod dram;
pub mod dsp;
pub mod fabric;
/// PAL の式と PROM の中身から生成した関数（DR-084）。生成したまま置くので、整形しない。
#[rustfmt::skip]
pub mod fabric_pal;
pub mod fifo;
pub mod ports;
/// 本体の番地とレジスタの表（`docs/spec/registers.toml` から生成、DR-100）。生成したまま置くので、整形しない。
#[rustfmt::skip]
pub mod regmap;
pub mod sync;
pub mod tv;
pub mod vclock;
pub mod video;
pub mod wav;
