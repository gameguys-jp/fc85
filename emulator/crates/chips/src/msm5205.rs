//! OKI MSM5205 ADPCM 音声合成 IC。
//!
//! 事実と出典は `docs/parts/MSM5205.md`（以下「台帳」）。本機での使い方は仕様書 §05・§07。
//! 復号の式は台帳 §4.2（MAME の実装を読んで書き起こしたもの）に従い、コードは自前で書いた。
//!
//! モデルの範囲：
//! - 発振 384 kHz の 1 クロックを `tick` の 1 回とする。
//! - V̄C̄K̄ はデューティ 50%。立下りから 6 クロック後にデータを取り込む（台帳 §3.2）。
//! - 予測値は 12 bit。あふれたときは回り込みを標準にし、クリップは比較用に選べる（台帳 §4.4、§6 の注意 1）。
//! - RESET は取込みの時点で見る。V̄C̄K̄ 2 周期以上という最小幅は模擬しない（台帳 §3.3、[V]）。
//! - DA OUT の遅れ（台帳 §3.2）と、アナログの電圧・LPF は別の層で扱う。ここでは取込み列と DAC のコードを出す。

use serde::{Deserialize, Serialize};

/// ステップ値 floor(16 × 1.1^i)、i = 0〜48（台帳 §4.2）。
pub const STEP_TABLE: [i16; 49] = [
    16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130,
    143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796,
    876, 963, 1060, 1166, 1282, 1411, 1552,
];

/// 大きさ（B2 B1 B0）ごとのステップ番号の増減（台帳 §4.2）。
const INDEX_SHIFT: [i8; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// 取込みから V̄C̄K̄ の立下りまでのクロック数（台帳 §3.2）。
pub const CAPTURE_DELAY: u8 = 6;

/// 予測値があふれたときの扱い。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Overflow {
    /// 12 bit の 2 の補数で回り込む。データシートの記述に従う。本機の標準 [P]。
    Wrap,
    /// −2048〜+2047 で止める。MAME と同じ。比較用。
    Clip,
}

/// S1・S2 のピンで決まるサンプリングの分周（台帳 §3.1）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleRate {
    /// S1 = L、S2 = L：1/96（384 kHz で 4 kHz）。
    Div96,
    /// S1 = L、S2 = H：1/64（6 kHz）。
    Div64,
    /// S1 = H、S2 = L：1/48（8 kHz）。
    Div48,
    /// S1 = S2 = H：データシートでは禁止。内部の分周が止まり、V̄C̄K̄ が入力になる（MAME の解釈、台帳 §4.5）。
    /// 本機では Bend モードでだけ使う [P]。
    Slave,
}

impl SampleRate {
    pub fn from_pins(s1: bool, s2: bool) -> Self {
        match (s1, s2) {
            (false, false) => Self::Div96,
            (false, true) => Self::Div64,
            (true, false) => Self::Div48,
            (true, true) => Self::Slave,
        }
    }

    /// 1 サンプルあたりの発振クロック数。`Slave` では内部の分周がないので `None`。
    pub fn divider(self) -> Option<u32> {
        match self {
            Self::Div96 => Some(96),
            Self::Div64 => Some(64),
            Self::Div48 => Some(48),
            Self::Slave => None,
        }
    }
}

/// 1 回の取込みの記録。V2 のビット列比較はこの列で行う（台帳 §6 の注意 5）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capture {
    /// 取り込んだ D3〜D0。3-bit モードでは D0 を 0 にした値。
    pub nibble: u8,
    /// 取込みの時点で RESET が H だったか。
    pub reset: bool,
    /// 取込み後の予測値（−2048〜+2047）。
    pub signal: i16,
    /// 取込み後のステップ番号（0〜48）。
    pub step: u8,
}

/// V̄C̄K̄ の縁。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edge {
    /// H になった。本機のブリッジはここで次のデータをラッチする（台帳 §3.2、§4.6）。
    Rise,
    /// L になった。`CAPTURE_DELAY` クロック後に取り込む。
    Fall,
}

/// 1 クロックの間に起きたこと。取込みと縁が同じクロックに重なることもある
/// （`Slave` から内部の分周に戻した直後など）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    pub capture: Option<Capture>,
    pub edge: Option<Edge>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Msm5205 {
    // 入力ピン
    data: u8,
    reset: bool,
    rate: SampleRate,
    four_bit: bool,
    // 内部状態
    signal: i16,
    step: u8,
    /// V̄C̄K̄ のピンのレベル。`Slave` では入力。
    vck: bool,
    /// サンプル周期の中の位置（発振クロック数）。
    phase: u32,
    /// 取込みまでの残りクロック数。
    capture_in: Option<u8>,
    overflow: Overflow,
}

impl Default for Msm5205 {
    fn default() -> Self {
        Self::new()
    }
}

impl Msm5205 {
    /// 電源投入直後の状態。予測値とステップ番号は 0、4-bit、8 kHz とする。
    /// 実 IC の電源投入時の状態は未確認 [V]。使う前に RESET をかける。
    pub fn new() -> Self {
        Self {
            data: 0,
            reset: false,
            rate: SampleRate::Div48,
            four_bit: true,
            signal: 0,
            step: 0,
            vck: true,
            phase: 0,
            capture_in: None,
            overflow: Overflow::Wrap,
        }
    }

    // ---- 入力ピン ----

    /// D3〜D0。上位 4 bit は無視する。
    pub fn set_data(&mut self, nibble: u8) {
        self.data = nibble & 0x0F;
    }

    pub fn set_reset(&mut self, level: bool) {
        self.reset = level;
    }

    /// S1・S2。周期の途中で変えたときの実 IC の動作は未確認 [V]。
    /// このモデルでは周期の中の位置を保ち、新しい周期を超えていれば次の周期の頭から数え直す。
    pub fn set_rate_pins(&mut self, s1: bool, s2: bool) {
        self.rate = SampleRate::from_pins(s1, s2);
        if let Some(p) = self.rate.divider()
            && self.phase >= p
        {
            self.phase = 0;
        }
    }

    /// 4B ピン。L で 3-bit モード（D0 を使わない、台帳 §4.5）。
    /// 3-bit の差分式は MAME と同じく 4-bit の表に通す。実 IC と合うかは未確認 [V]。
    pub fn set_four_bit(&mut self, four_bit: bool) {
        self.four_bit = four_bit;
    }

    /// `Slave` のときの V̄C̄K̄ の入力。立下りから `CAPTURE_DELAY` クロック後に取り込む。
    /// `Slave` 以外では V̄C̄K̄ は出力なので、何もしない。
    pub fn set_vck_input(&mut self, level: bool) -> Option<Edge> {
        if self.rate != SampleRate::Slave || level == self.vck {
            return None;
        }
        self.vck = level;
        if level {
            Some(Edge::Rise)
        } else {
            self.capture_in = Some(CAPTURE_DELAY);
            Some(Edge::Fall)
        }
    }

    // ---- 比較用の設定 ----

    pub fn set_overflow(&mut self, overflow: Overflow) {
        self.overflow = overflow;
    }

    pub fn overflow(&self) -> Overflow {
        self.overflow
    }

    // ---- 出力と内部状態 ----

    /// V̄C̄K̄ のピンのレベル。
    pub fn vck(&self) -> bool {
        self.vck
    }

    pub fn rate(&self) -> SampleRate {
        self.rate
    }

    /// 予測値（12 bit、−2048〜+2047）。
    pub fn signal(&self) -> i16 {
        self.signal
    }

    /// ステップ番号（0〜48）。
    pub fn step(&self) -> u8 {
        self.step
    }

    /// DAC に渡る 10 bit のコード（−512〜+511）。予測値の下位 2 bit を捨てる（台帳 §4.2）。
    pub fn dac_code(&self) -> i16 {
        self.signal >> 2
    }

    // ---- 時間 ----

    /// 発振クロックを 1 つ進める。
    pub fn tick(&mut self) -> Tick {
        let mut out = Tick::default();

        if let Some(n) = self.capture_in {
            if n <= 1 {
                self.capture_in = None;
                out.capture = Some(self.capture());
            } else {
                self.capture_in = Some(n - 1);
            }
        }

        if let Some(p) = self.rate.divider() {
            self.phase += 1;
            if self.phase >= p {
                self.phase = 0;
            }
            if self.phase == 0 && !self.vck {
                self.vck = true;
                out.edge = Some(Edge::Rise);
            } else if self.phase == p / 2 && self.vck {
                self.vck = false;
                self.capture_in = Some(CAPTURE_DELAY);
                out.edge = Some(Edge::Fall);
            }
        }
        out
    }

    fn capture(&mut self) -> Capture {
        let nibble = if self.four_bit {
            self.data
        } else {
            self.data & 0x0E
        };
        if self.reset {
            self.signal = 0;
            self.step = 0;
        } else {
            let mag = usize::from(nibble & 0x07);
            let x = i32::from(self.signal) + diff(self.step, nibble);
            self.signal = match self.overflow {
                Overflow::Wrap => ((x + 2048).rem_euclid(4096) - 2048) as i16,
                Overflow::Clip => x.clamp(-2048, 2047) as i16,
            };
            self.step = (i32::from(self.step) + i32::from(INDEX_SHIFT[mag])).clamp(0, 48) as u8;
        }
        Capture {
            nibble,
            reset: self.reset,
            signal: self.signal,
            step: self.step,
        }
    }
}

/// 差分 Δ = ±(s×B2 + ⌊s/2⌋×B1 + ⌊s/4⌋×B0 + ⌊s/8⌋)（台帳 §4.2）。
pub fn diff(step: u8, nibble: u8) -> i32 {
    let s = i32::from(STEP_TABLE[usize::from(step)]);
    let mut d = s / 8;
    if nibble & 0x04 != 0 {
        d += s;
    }
    if nibble & 0x02 != 0 {
        d += s / 2;
    }
    if nibble & 0x01 != 0 {
        d += s / 4;
    }
    if nibble & 0x08 != 0 { -d } else { d }
}
