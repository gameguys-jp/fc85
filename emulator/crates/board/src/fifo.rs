//! MMI 67401（64×4、非同期の FIFO）を幅の方向に並べた FIFO。
//!
//! 事実と出典は `docs/parts/review-digital.md` 付録 A.1。本機では 4 個で 64×16（port 2・3・4）、
//! 2 個で 64×8（port 5）にする（仕様書 §06、DR-017）。
//!
//! モデルの範囲：
//! - 深さは 64 語。IR（入力の準備）は空きがあるとき 1、OR（出力の準備）はデータがあるとき 1。
//!   4 個を並べたときの IR・OR の AND（fabric.md §08 の U53–U55）は、ここでは 1 つの FIFO として扱う。
//! - 満杯のときの Shift In は無視される。空のときの Shift Out も無視され、出力は前の値のまま
//!   （付録 A.1）。どちらも回数を数える。
//! - 通り抜けの時間（書いた語が出力に出るまで）は扱わない。書いた次のスロットから読める [V]。
//! - MR（マスタリセット）の後の出力の値は資料にない。0 にする [V]。

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// 67401 の深さ（語）。
pub const DEPTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fifo {
    mask: u16,
    data: VecDeque<u16>,
    out: u16,
    /// 満杯のときに捨てた Shift In の回数。
    pub rejected: u32,
    /// 空のときに読んだ（前の値を返した）回数。
    pub empty_reads: u32,
}

impl Fifo {
    /// `bits` は語の幅（8 か 16）。
    pub fn new(bits: u32) -> Self {
        Self {
            mask: if bits >= 16 {
                0xFFFF
            } else {
                (1u16 << bits) - 1
            },
            data: VecDeque::with_capacity(DEPTH),
            out: 0,
            rejected: 0,
            empty_reads: 0,
        }
    }

    /// IR：書き込める。
    pub fn ir(&self) -> bool {
        self.data.len() < DEPTH
    }

    /// OR：読み出せる。
    pub fn or(&self) -> bool {
        !self.data.is_empty()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Shift In。満杯なら捨てて `false`。
    pub fn shift_in(&mut self, v: u16) -> bool {
        if self.ir() {
            self.data.push_back(v & self.mask);
            true
        } else {
            self.rejected += 1;
            false
        }
    }

    /// 出力の値を読んで Shift Out する。空なら前の値を返す。
    pub fn shift_out(&mut self) -> u16 {
        match self.data.pop_front() {
            Some(v) => {
                self.out = v;
                v
            }
            None => {
                self.empty_reads += 1;
                self.out
            }
        }
    }

    /// MR：中身を捨てる。
    pub fn reset(&mut self) {
        self.data.clear();
        self.out = 0;
    }
}
