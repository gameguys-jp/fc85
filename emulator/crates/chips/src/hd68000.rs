//! HD68000-8（日立。MC68000L8 と同等品）のコア。台帳 `docs/parts/HD68000.md`。
//!
//! 外から見えるのはバスサイクルの列だけにする。CPU は命令を 1 つずつ実行し、バスサイクルごとに
//! `Bus` を呼ぶ。`Bus` は S0 の時刻（電源投入からの CPU クロック数）を受け取り、基板の側をその時刻
//! まで進めてから答える。待ち（DTACK の遅れ、VPA の同期サイクル）は答えの `waits` で返す。
//!
//! 段階 3 の骨組み（この節の型とトレイトは board 層の Console が使う約束）。命令の実行は別に書く。

use serde::{Deserialize, Serialize};

/// 機能コード（FC2–FC0）。
pub mod fc {
    pub const USER_DATA: u8 = 1;
    pub const USER_PROGRAM: u8 = 2;
    pub const SUPERVISOR_DATA: u8 = 5;
    pub const SUPERVISOR_PROGRAM: u8 = 6;
    /// 割込みの応答（IACK）。
    pub const CPU_SPACE: u8 = 7;
}

/// バスサイクルの幅。バイトは番地の bit 0 で UDS（偶数）か LDS（奇数）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Size {
    Byte,
    Word,
}

/// 読出し・書込みのバスサイクルの答え。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusReply {
    /// DTACK（または VPA の同期サイクルの終わり）。`waits` は待ちのクロック数（サイクルは 4＋waits
    /// クロック）。読出しの値は `data`（バイトは下位 8 bit）。
    Ok { data: u16, waits: u32 },
    /// BERR。`clocks` は S0 からサイクルを打ち切るまでのクロック数。CPU はバスエラーの例外処理に入る。
    BusError { clocks: u32 },
}

/// 割込みの応答（IACK）サイクルの答え。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IackReply {
    /// 装置がベクタ番号を返した。
    Vector { vector: u8, waits: u32 },
    /// VPA：オートベクタ（24＋レベル）。
    Autovector { waits: u32 },
    /// BERR：スプリアス割込み（ベクタ 24）。
    Spurious { clocks: u32 },
}

/// CPU から見たバス。`at` はバスサイクルの S0 の立上りの時刻（電源投入からの CPU クロック数）。
pub trait Bus {
    /// 読出しのバスサイクル。
    fn read(&mut self, at: u64, addr: u32, fc: u8, size: Size) -> BusReply;
    /// 書込みのバスサイクル。バイトは `value` の下位 8 bit（68000 は両方のレーンに同じ値を出す）。
    fn write(&mut self, at: u64, addr: u32, fc: u8, size: Size, value: u16) -> BusReply;
    /// 割込みの応答サイクル（`level` は 1〜7）。
    fn iack(&mut self, at: u64, level: u8) -> IackReply;
    /// 命令の境目で IPL2–IPL0 を見る（0 は要求なし、7 はノンマスカブル）。
    fn ipl(&mut self, at: u64) -> u8;
    /// RESET 命令が RESET 端子を 124 クロック出す。周辺をリセットする。
    fn reset_out(&mut self, _at: u64) {}
}

/// 実行の状態。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    #[default]
    Running,
    /// STOP 命令で割込みを待つ。
    Stopped,
    /// 二重バスフォルトで止まった（HALT を出す）。リセットでだけ戻る。
    Halted,
}

/// レジスタと内部の状態。`pc` は IR にある命令の番地（命令の境目で）。`prefetch` は IR と IRC。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hd68000 {
    pub d: [u32; 8],
    /// A0〜A6。A7 は `usp`・`ssp` のうち SR の S で選ぶ方。
    pub a: [u32; 7],
    pub usp: u32,
    pub ssp: u32,
    pub sr: u16,
    pub pc: u32,
    pub prefetch: [u16; 2],
    pub state: RunState,
    /// 電源投入からの CPU クロック数。
    pub clock: u64,
}

impl Hd68000 {
    pub fn new() -> Self {
        Self {
            sr: 0x2700,
            ..Self::default()
        }
    }

    /// リセットの例外処理（SSP と PC を 0 番地と 4 番地から読み、プリフェッチを満たす）。
    pub fn reset(&mut self, bus: &mut impl Bus) {
        let _ = bus;
        unimplemented!("段階 3：68000 のコア")
    }

    /// 命令を 1 つ（または例外処理を 1 つ）実行し、使ったクロック数を返す。止まっている間は、割込みを
    /// 見て 4 クロック進める。
    pub fn step(&mut self, bus: &mut impl Bus) -> u32 {
        let _ = bus;
        unimplemented!("段階 3：68000 のコア")
    }
}
