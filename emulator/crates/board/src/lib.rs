//! fc85 の基板の回路（仕様書 §12 の board 層）。
//!
//! chips 層の IC のコアをつなぐ回路を置く。段階 2 の最初は Data Fabric（仕様書 §06、DR-046、
//! `docs/spec/fabric.md`）と、そのポートの側の部品（Staging RAM、文字 ROM、Temporal RAM、67401 の FIFO）。
//! 次に、ACRTC の DMA（port 1）のつなぎ（`acrtc`）、音声クロックと本体 DAC（`audio`）、ADPCM の
//! アダプタ（`adpcm`）、DSP のアダプタ（`dsp`）。DOC 子基板（port 2）は後で足す。時刻は core 層の単位で
//! 数える。
//! 状態はすべて serde で保存・復元できる（DR-040）。

pub mod acrtc;
pub mod adpcm;
pub mod audio;
pub mod dsp;
pub mod fabric;
pub mod fifo;
pub mod ports;
