//! fc85 の IC ごとのコア（仕様書 §12 の chips 層）。
//!
//! 各コアは、IC の外部ピンで見える動作を、IC 自身のクロックで刻んで再現する。
//! 基板（アドレスマップ、Data Fabric、ポートアダプタ）は board 層の仕事で、ここには置かない。
//! 状態はすべて serde で保存・復元できる（snapshot、Bend のパッチの undo、DR-040）。

pub mod msm5205;
pub mod tms32010;
