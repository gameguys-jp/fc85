//! fc85 の基板の回路（仕様書 §12 の board 層）。
//!
//! chips 層の IC のコアをつなぐ回路を置く。段階 2 の最初は Data Fabric（仕様書 §06、DR-046、
//! `docs/spec/fabric.md`）と、そのポートの側の部品（Staging RAM、文字 ROM、Temporal RAM、67401 の FIFO）。
//! ACRTC の DMA（port 1）、DOC 子基板（port 2）、DSP・MSM5205・本体 DAC のポートアダプタは後で足す。
//! 状態はすべて serde で保存・復元できる（DR-040）。

pub mod fabric;
pub mod fifo;
pub mod ports;
