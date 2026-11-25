//! fc85 の core 層（仕様書 §12）：時刻、イベント、信号線、trace、snapshot。
//!
//! 今は時刻（`time`）だけ。クロック領域ごとの時刻を、1 つの整数の単位で数える。

pub mod time;
