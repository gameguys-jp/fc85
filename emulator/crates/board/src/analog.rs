//! 音声のアナログの段（段階 4）：DOC の I/V 変換・サンプルホールド・加算・LPF、本体 DAC の I/V 変換と
//! LPF、MSM5205 の DA OUT と LPF、ミキサ、DC 遮断。
//!
//! 仕様書 §10「音響回路・信号ルーティング」、§11 のベンド点（DOC 出力の片側）、DR-009（オペアンプの級）、
//! DR-011（SIG± の両側と、片側の配線のジャンパ）、DR-019（本体 DAC の道）、DR-034、DR-054（左右の割り当てと
//! 音量の比）、台帳 ES5503.md §2・§3.6・§4.5、MSM5205.md §2・§3.2・§3.5、review-signal.md §6。
//!
//! 回路（定数は `parts`。値はすべて [P]。式は教科書の回路の式で、ほかのエミュレータのコードは使っていない）：
//!
//! ```text
//! DOC 子基板
//!   SIG+ ─ I/V（LM318、2.0 kΩ）─┐
//!   SIG− ─ I/V（LM318、2.0 kΩ）─┴─ 差（LM318）─ 4051×2（CA3–CA0、CSTRB）─ S/H 16 個（470 pF、TL082）
//!     ジャンパ ONE：SIG− の側を切る（DR-011 の初期の配線）
//!   S/H 偶数 8 個 ─ 39 kΩ×8 ─ 反転加算（4558、10 kΩ∥1000 pF：15.9 kHz）─ SK（4558：15.8 kHz、Q 0.99）─ L
//!   S/H 奇数 8 個 ─ 同じ ───────────────────────────────────────────────────────── R
//! 本体（L・R ごと）
//!   HA17408 ─ I/V（4558、2.0 kΩ、両極のオフセット）─ MFB（4558：20.4 kHz、Q 0.72）─ 4066（子基板があれば切る）
//!   MSM5205 DA OUT ─ フォロワ（4558）─ SK（3.38 kHz、Q 0.55）─ MFB（3.39 kHz、Q 1.28）─ 2.2 µF ─ L・R へ
//!   ミキサ（4558、反転加算、10 kΩ）：DOC 10 kΩ、本体 DAC 20 kΩ、ADPCM 100 kΩ
//!   ─ 100 Ω ─ 10 µF ─ LINE OUT（10 kΩ の負荷で測る）
//! ```
//!
//! 模型：
//! - 刻みは 192 kHz（`TICK`。48 kHz の 4 倍）[P]。刻みの入力は、区分的に一定の源（S/H の和、本体 DAC の
//!   ラッチ、DA OUT）の、刻みの間の時間の平均（`Hold`）。フィルタの段は双一次変換で、各段の f0 で
//!   前置きゆがみの補正をする。
//! - 48 kHz の 1 標本は、4 つの刻みの出力の平均（ホストの録音の積分）。デジタルの出力（`audio_out`）と同じ
//!   時刻の条件で出すので、数がそろう。
//! - S/H（DOC 子基板）は `doc.rs` の `DocBoard` が CSTRB ごとに更新する。取り込みは不完全で、1 回の CSTRB
//!   （Low 500 ns）で差の `sh_alpha`（300 Ω×470 pF＝141 ns の RC で 0.971）だけ近づく。保持中の垂下は
//!   無視する（TL082 の入力電流 30 pA で、18 µs に約 1 µV。計算）。4051 の電荷の注入（台座）も作らない
//!   （直流の分は出力の結合容量で切れる）[V]。
//! - SIG± は乗算型の DAC の相補の電流とみる：I± ＝ I_FS×(音量/255)×(1/2 ± s/256)、s は波形の符号付きの値
//!   [P][V]。両側の差は 2×sig、片側（SIG+ だけ）は sig＋128×音量（sig は ES5503 のコアの SIG±＝s×音量）。
//!   片側では振幅が半分になり、音量に比例した直流が乗る（DR-011 の「音色と直流分が変わる」）。
//! - オペアンプの飽和は、加算とミキサの出力で ±`parts::V_SAT` に切る（`clips` に数える）。
//! - 電源投入時のアナログの状態は、入力が中央で落ち着いた状態（コンデンサの直流は中央の値）[P]。電源の
//!   立上りのポップは作らない。
//! - 状態の f64 はビット列で保存するので、保存して戻すと同じ値になる。

use std::sync::OnceLock;

use fc85_core::time::{Time, UNITS_PER_SECOND};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 刻み（192 kHz）の時刻の単位の数。
pub const TICK: Time = 78_750;
/// 刻みの周波数。
pub const TICK_RATE: f64 = (UNITS_PER_SECOND / TICK) as f64;
/// 48 kHz の 1 標本の刻みの数。
pub const TICKS_PER_SAMPLE: u8 = 4;
/// LINE OUT の電圧と 16 bit の標本の対応：±`LINE_FULL_SCALE` V が ±32768 [P]。DR-054 のデジタルの
/// 全振幅と同じ大きさになるように、回路の利得を選んだ。
pub const LINE_FULL_SCALE: f64 = 2.0;

/// 回路の定数 [P]。抵抗は Ω、容量は F、電流は A、電圧は V。
pub mod parts {
    /// ES5503 の SIG+ と SIG− の電流の和の最大（音量 255）[P][V]（台帳 ES5503.md §7 の 8）。
    pub const DOC_I_FS: f64 = 1.0e-3;
    /// DOC の I/V 変換（LM318）の帰還抵抗。
    pub const DOC_R_IV: f64 = 2.0e3;
    /// S/H の取り込みの抵抗：4051 のオン抵抗（約 200 Ω、±5 V で [V]）と、LM318 の出力に入れる 100 Ω。
    pub const SH_R: f64 = 300.0;
    /// S/H の保持容量（ポリスチレン）。
    pub const SH_C: f64 = 470.0e-12;
    /// DOC の加算（反転、偶数か奇数の 8 チャネル）：入力抵抗、帰還抵抗、帰還容量。
    pub const DOC_SUM_R_IN: f64 = 39.0e3;
    pub const DOC_SUM_R_FB: f64 = 10.0e3;
    pub const DOC_SUM_C_FB: f64 = 1.0e-9;
    /// DOC の LPF（Sallen-Key、利得 1）：R1＝R2、C1 は帰還、C2 は接地。
    pub const DOC_SK_R: f64 = 9.1e3;
    pub const DOC_SK_C1: f64 = 2.2e-9;
    pub const DOC_SK_C2: f64 = 560.0e-12;

    /// 本体 DAC（HA17408）の基準電流と、I/V 変換（4558）の帰還抵抗。両極のオフセットで中央（128）が 0 V。
    pub const DAC_I_REF: f64 = 2.0e-3;
    pub const DAC_R_IV: f64 = 2.0e3;
    /// 本体 DAC の LPF（多重帰還、R1＝R2＝R3、C1 は接地、C2 は帰還）。約 20 kHz（仕様書 §10 [V]）。
    pub const DAC_MFB_R: f64 = 3.6e3;
    pub const DAC_MFB_C1: f64 = 4.7e-9;
    pub const DAC_MFB_C2: f64 = 1.0e-9;

    /// MSM5205 の DA OUT：10 bit の符号 ±512 で ±`AD_V_FS`、中央は VDD/2（2.5 V）[P][V]（台帳 MSM5205.md §2）。
    pub const AD_V_FS: f64 = 1.25;
    /// MSM5205 の LPF（台帳 MSM5205.md §3.5：fs/2×0.85＝3.4 kHz、4 次）。1 段目は Sallen-Key、2 段目は多重帰還。
    pub const AD_SK_R: f64 = 11.0e3;
    pub const AD_SK_C1: f64 = 4.7e-9;
    pub const AD_SK_C2: f64 = 3.9e-9;
    pub const AD_MFB_R: f64 = 18.0e3;
    pub const AD_MFB_C1: f64 = 10.0e-9;
    pub const AD_MFB_C2: f64 = 680.0e-12;
    /// DA OUT の直流を切る結合容量（L・R のミキサの 100 kΩ が並列で 50 kΩ）。
    pub const AD_C_COUPLE: f64 = 2.2e-6;

    /// ミキサ（4558、反転加算）の帰還抵抗と入力抵抗。DR-054 の音量の比（DOC：和÷4、本体 DAC：×128、
    /// ADPCM：×8 を半分ずつ）になるように選んだ。
    pub const MIX_R_FB: f64 = 10.0e3;
    pub const MIX_R_DOC: f64 = 10.0e3;
    pub const MIX_R_DAC: f64 = 20.0e3;
    pub const MIX_R_AD: f64 = 100.0e3;
    /// 出力：直列抵抗、結合容量、負荷（LINE の入力）。
    pub const OUT_R_S: f64 = 100.0;
    pub const OUT_C: f64 = 10.0e-6;
    pub const OUT_R_LOAD: f64 = 10.0e3;
    /// 4558 の出力の飽和（±12 V の電源）[P]。
    pub const V_SAT: f64 = 10.5;

    /// Audio IN（DR-129）：RCA の後の結合容量と入力抵抗（フォロワ 4558 の入力）、ミキサの入力抵抗。
    pub const LIN_C: f64 = 10.0e-6;
    pub const LIN_R: f64 = 47.0e3;
    pub const MIX_R_LIN: f64 = 10.0e3;
    /// ホストの 16 bit の入力の ±32768 を RCA の ±`LIN_V_FS` V とみる [P]。
    pub const LIN_V_FS: f64 = 1.0;
    /// DOC 子基板の A/D の段（DR-129）：L・R を 20 kΩ ずつ反転加算（帰還 10 kΩ）して (L＋R)/2 にし、
    /// もう 1 段の反転（10 kΩ、10 kΩ。非反転入力に 0.625 V）で中央を `ADC_BIAS` にする。続けて Sallen-Key の
    /// LPF（利得 1）、1 kΩ とダイオードの挟み。
    pub const ADC_BIAS: f64 = 1.25;
    pub const ADC_SK_R: f64 = 10.0e3;
    pub const ADC_SK_C1: f64 = 2.7e-9;
    pub const ADC_SK_C2: f64 = 1.5e-9;
    /// DOC の A/D の入力の範囲（0〜2.5 V。台帳 ES5503.md §2）。
    pub const ADC_V_FS: f64 = 2.5;
}

// ---------------------------------------------------------------------- 回路の式

/// RC の 1 次の極（Hz）。
pub fn rc_corner(r: f64, c: f64) -> f64 {
    1.0 / (2.0 * std::f64::consts::PI * r * c)
}

/// 利得 1 の Sallen-Key の LPF：H＝1/(s²R1R2C1C2＋sC2(R1＋R2)＋1)。C1 は帰還、C2 は接地。(f0, Q)。
pub fn sallen_key(r1: f64, r2: f64, c1: f64, c2: f64) -> (f64, f64) {
    let m = (r1 * r2 * c1 * c2).sqrt();
    (1.0 / (2.0 * std::f64::consts::PI * m), m / (c2 * (r1 + r2)))
}

/// 多重帰還の LPF：H＝−(R2/R1)/(s²C1C2R2R3＋sC2(R2＋R3＋R2R3/R1)＋1)。C1 は接地、C2 は帰還。
/// (f0, Q, 通過域の利得)。
pub fn mfb(r1: f64, r2: f64, r3: f64, c1: f64, c2: f64) -> (f64, f64, f64) {
    let m = (c1 * c2 * r2 * r3).sqrt();
    let b = c2 * (r2 + r3 + r2 * r3 / r1);
    (1.0 / (2.0 * std::f64::consts::PI * m), m / b, -r2 / r1)
}

/// S/H が 1 回の CSTRB（Low の幅 `strobe` 秒）で差を縮める割合：1−exp(−幅/(R×C))。
pub fn sh_alpha(strobe: f64) -> f64 {
    1.0 - (-strobe / (parts::SH_R * parts::SH_C)).exp()
}

/// DOC の CLK の周期（時刻の単位）から、S/H の割合（Q16）。CSTRB の Low は E High の間（CLK の 4 周期）
/// （台帳 ES5503.md §3.6）。
pub fn sh_alpha_q16(clk_period: Time) -> u32 {
    let strobe = 4.0 * clk_period as f64 / UNITS_PER_SECOND as f64;
    (sh_alpha(strobe) * 65536.0).round() as u32
}

/// DOC の S/H の値（`DocBoard` の単位：SIG の和の半分の単位×65536）を電圧にする係数。
/// 両側の差は 2×sig、全振幅で I/V の出力が ±R×I_FS。
pub const fn doc_volts_per_unit() -> f64 {
    parts::DOC_R_IV * parts::DOC_I_FS / 65280.0 / 65536.0
}

// ---------------------------------------------------------------------- 状態と係数

/// f64 の状態。保存はビット列（u64）で、戻すと同じ値になる。比較もビット列。
#[derive(Clone, Copy, Debug, Default)]
pub struct F(pub f64);

impl PartialEq for F {
    fn eq(&self, o: &Self) -> bool {
        self.0.to_bits() == o.0.to_bits()
    }
}

impl Eq for F {}

impl Serialize for F {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(self.0.to_bits())
    }
}

impl<'de> Deserialize<'de> for F {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        u64::deserialize(d).map(|b| F(f64::from_bits(b)))
    }
}

/// 2 次（または 1 次）の段の係数。H(z)＝(b0＋b1 z⁻¹＋b2 z⁻²)/(1＋a1 z⁻¹＋a2 z⁻²)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Section {
    /// 前置きゆがみの係数 tan(π f0/fs)。
    fn warp(f0: f64, fs: f64) -> f64 {
        (std::f64::consts::PI * f0 / fs).tan()
    }

    /// 2 次の LPF ω0²/(s²＋(ω0/Q)s＋ω0²) の双一次変換（f0 で補正）。
    pub fn lowpass2(f0: f64, q: f64, fs: f64) -> Self {
        let k = Self::warp(f0, fs);
        let n = 1.0 / (1.0 + k / q + k * k);
        let b0 = k * k * n;
        Self {
            b0,
            b1: 2.0 * b0,
            b2: b0,
            a1: 2.0 * (k * k - 1.0) * n,
            a2: (1.0 - k / q + k * k) * n,
        }
    }

    /// 1 次の LPF ω0/(s＋ω0)。
    pub fn lowpass1(f0: f64, fs: f64) -> Self {
        let k = Self::warp(f0, fs);
        Self {
            b0: k / (1.0 + k),
            b1: k / (1.0 + k),
            b2: 0.0,
            a1: (k - 1.0) / (k + 1.0),
            a2: 0.0,
        }
    }

    /// 1 次の HPF s/(s＋ω0)。
    pub fn highpass1(f0: f64, fs: f64) -> Self {
        let k = Self::warp(f0, fs);
        Self {
            b0: 1.0 / (1.0 + k),
            b1: -1.0 / (1.0 + k),
            b2: 0.0,
            a1: (k - 1.0) / (k + 1.0),
            a2: 0.0,
        }
    }

    /// 利得を掛ける。
    pub fn gain(self, g: f64) -> Self {
        Self {
            b0: self.b0 * g,
            b1: self.b1 * g,
            b2: self.b2 * g,
            ..self
        }
    }

    /// 周波数 f の利得（|H(e^jω)|）。試験と解析に使う。
    pub fn response(&self, f: f64, fs: f64) -> f64 {
        let w = 2.0 * std::f64::consts::PI * f / fs;
        let (c1, s1, c2, s2) = (w.cos(), -w.sin(), (2.0 * w).cos(), -(2.0 * w).sin());
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = self.b1 * s1 + self.b2 * s2;
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = self.a1 * s1 + self.a2 * s2;
        ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt()
    }
}

/// 段の状態（転置直接形 II）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    s1: F,
    s2: F,
}

impl State {
    #[inline]
    pub fn run(&mut self, c: &Section, x: f64) -> f64 {
        let y = c.b0 * x + self.s1.0;
        self.s1.0 = c.b1 * x - c.a1 * y + self.s2.0;
        self.s2.0 = c.b2 * x - c.a2 * y;
        y
    }
}

/// 回路の係数（`parts` と刻みの周波数から一度だけ計算する）。
#[derive(Clone, Debug)]
pub struct Coefs {
    /// DOC：反転加算（利得 −R_FB/R_IN と帰還の極）、Sallen-Key。
    pub doc_sum: Section,
    pub doc_sk: Section,
    /// 本体 DAC：多重帰還（利得 −1）。
    pub dac_mfb: Section,
    /// ADPCM：Sallen-Key、多重帰還（利得 −1）、結合容量の HPF。
    pub ad_sk: Section,
    pub ad_mfb: Section,
    pub ad_hpf: Section,
    /// 出力：結合容量の HPF と、直列抵抗と負荷の分圧。
    pub out_hpf: Section,
    /// 入力の換算：DOC の S/H の和の単位、本体 DAC の 1 段、DA OUT の 1 段の電圧。
    pub doc_volts: f64,
    pub dac_volts: f64,
    pub ad_volts: f64,
    /// ミキサの利得（反転加算なので負）。
    pub mix_doc: f64,
    pub mix_dac: f64,
    pub mix_ad: f64,
    /// Audio IN：結合容量の HPF、ミキサの利得、A/D の段の LPF（DR-129）。
    pub lin_hpf: Section,
    pub mix_lin: f64,
    pub adc_sk: Section,
}

impl Coefs {
    pub fn new(fs: f64) -> Self {
        use parts::*;
        let (sk_f, sk_q) = sallen_key(DOC_SK_R, DOC_SK_R, DOC_SK_C1, DOC_SK_C2);
        let (dm_f, dm_q, dm_g) = mfb(DAC_MFB_R, DAC_MFB_R, DAC_MFB_R, DAC_MFB_C1, DAC_MFB_C2);
        let (as_f, as_q) = sallen_key(AD_SK_R, AD_SK_R, AD_SK_C1, AD_SK_C2);
        let (am_f, am_q, am_g) = mfb(AD_MFB_R, AD_MFB_R, AD_MFB_R, AD_MFB_C1, AD_MFB_C2);
        Self {
            doc_sum: Section::lowpass1(rc_corner(DOC_SUM_R_FB, DOC_SUM_C_FB), fs)
                .gain(-DOC_SUM_R_FB / DOC_SUM_R_IN),
            doc_sk: Section::lowpass2(sk_f, sk_q, fs),
            dac_mfb: Section::lowpass2(dm_f, dm_q, fs).gain(dm_g),
            ad_sk: Section::lowpass2(as_f, as_q, fs),
            ad_mfb: Section::lowpass2(am_f, am_q, fs).gain(am_g),
            ad_hpf: Section::highpass1(rc_corner(MIX_R_AD / 2.0, AD_C_COUPLE), fs),
            out_hpf: Section::highpass1(rc_corner(OUT_R_S + OUT_R_LOAD, OUT_C), fs)
                .gain(OUT_R_LOAD / (OUT_R_S + OUT_R_LOAD)),
            doc_volts: doc_volts_per_unit(),
            dac_volts: DAC_R_IV * DAC_I_REF / 256.0,
            ad_volts: AD_V_FS / 512.0,
            mix_doc: -MIX_R_FB / MIX_R_DOC,
            mix_dac: -MIX_R_FB / MIX_R_DAC,
            mix_ad: -MIX_R_FB / MIX_R_AD,
            lin_hpf: Section::highpass1(rc_corner(LIN_R, LIN_C), fs),
            mix_lin: -MIX_R_FB / MIX_R_LIN,
            adc_sk: {
                let (f, q) = sallen_key(ADC_SK_R, ADC_SK_R, ADC_SK_C1, ADC_SK_C2);
                Section::lowpass2(f, q, fs)
            },
        }
    }

    /// 192 kHz の係数。
    pub fn get() -> &'static Coefs {
        static C: OnceLock<Coefs> = OnceLock::new();
        C.get_or_init(|| Coefs::new(TICK_RATE))
    }
}

// ---------------------------------------------------------------------- 区分的に一定の値の積分

/// 区分的に一定の値の、時間の積分（刻みの間の平均を出す）。値は整数で、保存で正確に戻る。
/// 値を変えた時刻が、取り出す時刻より後のこともある（スロットを進めてから刻みを処理するため）。
/// 取り出す時刻の後の変化は 1 回まで正しく扱う（1 スロットに E の立下りは 1 回まで）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hold {
    v: i64,
    prev: i64,
    /// 最後に値を変えたか、積分した時刻。
    t: Time,
    /// `t` までの積分（値×時刻の単位）。刻みごとに取り出す前提で、あふれは回り込みにする。
    acc: i64,
}

impl Hold {
    pub fn value(&self) -> i64 {
        self.v
    }

    pub fn time(&self) -> Time {
        self.t
    }

    /// 時刻 `t` に値を `v` にする。前に変えた時刻より前なら、その時刻にする。
    pub fn set(&mut self, t: Time, v: i64) {
        let t = t.max(self.t);
        self.acc = self
            .acc
            .wrapping_add(self.v.wrapping_mul((t - self.t) as i64));
        self.t = t;
        self.prev = self.v;
        self.v = v;
    }

    /// 前に取り出してから `to` までの積分を返す。`to` の後の変化は次に回す。
    pub fn take(&mut self, to: Time) -> i64 {
        if self.t <= to {
            let a = self
                .acc
                .wrapping_add(self.v.wrapping_mul((to - self.t) as i64));
            self.t = to;
            self.acc = 0;
            a
        } else {
            let over = self.prev.wrapping_mul((self.t - to) as i64);
            let a = self.acc.wrapping_sub(over);
            self.acc = over;
            a
        }
    }
}

// ---------------------------------------------------------------------- DOC 出力の配線

/// DOC の SIG± の配線（DR-011 のジャンパ。仕様書 §11 のベンド点「DOC 出力の片側」）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SigWiring {
    /// 両側に I/V 変換を入れて差をとる（通常）。
    #[default]
    Both,
    /// 初期の片側の配線：SIG+ だけ。振幅が半分になり、音量に比例した直流が乗る。
    One,
}

// ---------------------------------------------------------------------- LINE OUT

/// 1 つの刻みの入力（刻みの間の平均）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    /// DOC の S/H の和（偶数＝L、奇数＝R）。`DocBoard` の単位。子基板がなければ 0。
    pub doc: [f64; 2],
    /// 本体 DAC の (値−128)（L、R）。
    pub dac: [f64; 2],
    /// 本体 DAC の道をミキサにつなぐか（子基板があれば 4066 で切る [P]）。
    pub dac_on: bool,
    /// DA OUT の 10 bit の符号（中央からの差）。
    pub da: f64,
    /// Audio IN の RCA の電圧（L、R）。ホストが入力を渡していなければ `None`（DR-129）。
    pub line: Option<[f64; 2]>,
}

// ---------------------------------------------------------------------- Audio IN と DOC の A/D

/// A/D の入力が中央（`ADC_BIAS`）のときの値。
pub const ADC_IDLE_CODE: u8 = 128;

/// DOC の A/D の入力の電圧を 8 bit にする：0〜2.5 V を 256 に分けて切り捨て、範囲の外は端に寄せる
/// （ダイオードの挟みと、変換の飽和）[P][V]。
pub fn adc_code(v: f64) -> u8 {
    (v / parts::ADC_V_FS * 256.0).floor().clamp(0.0, 255.0) as u8
}

/// Audio IN の段（DR-129）：本体の結合容量とフォロワ（ミキサへ）、DOC 子基板の A/D の段。
/// ホストが一度も入力を渡していなければ空のままで、状態の保存に入らない（入力を使わない ROM の状態は変わらない）。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineIn {
    /// 入力を渡されたことがある。
    pub used: bool,
    hpf: [State; 2],
    sk: State,
    /// A/D の入力の電圧の 8 bit の値（DOC の A/D が変換の終わりに取る）。
    pub code: u8,
}

impl LineIn {
    pub fn is_idle(&self) -> bool {
        !self.used
    }

    /// 1 刻み：RCA の電圧から、ミキサに入れる電圧（フォロワの出力）を返し、A/D の値を更新する。
    fn tick(&mut self, c: &Coefs, v: [f64; 2]) -> [f64; 2] {
        self.used = true;
        let l = self.hpf[0].run(&c.lin_hpf, v[0]);
        let r = self.hpf[1].run(&c.lin_hpf, v[1]);
        // (L＋R)/2 を 2 段の反転で正に戻し、中央を ADC_BIAS に。LPF は利得 1 なので交流の分だけを通す
        let ac = self.sk.run(&c.adc_sk, (l + r) * 0.5);
        self.code = adc_code(parts::ADC_BIAS + ac.clamp(-parts::V_SAT, parts::V_SAT));
        [l, r]
    }
}

/// アナログの段の状態と、48 kHz の LINE OUT の標本を作るところ。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Analog {
    /// 次の刻みの番号（時刻は番号×`TICK`）。`None` はまだ始めていない。
    next: Option<u64>,
    /// 始めたばかり：次の刻みでは今の出力をそのまま出す。
    fresh: bool,
    /// 本体 DAC（L、R）と DA OUT の値の積分。
    pub dac: [Hold; 2],
    pub da: Hold,
    doc_sum: [State; 2],
    doc_sk: [State; 2],
    dac_mfb: [State; 2],
    ad_sk: State,
    ad_mfb: State,
    ad_hpf: State,
    out_hpf: [State; 2],
    /// 出力（LINE OUT の電圧）と、この 48 kHz の標本の分の和。
    out: [F; 2],
    acc: [F; 2],
    phase: u8,
    /// 最後の刻みの、ミキサの出力（結合容量の前）の電圧。
    pub mixer: [F; 2],
    /// オペアンプの出力が飽和した刻みの数。
    pub clips: u64,
    /// Audio IN と DOC の A/D の段（DR-129）。使っていなければ保存しない。
    #[serde(default, skip_serializing_if = "LineIn::is_idle")]
    pub line: LineIn,
}

/// 電圧を 16 bit の標本にする（±`LINE_FULL_SCALE` V で切る）。
pub fn to_sample(v: f64) -> i16 {
    (v * (32768.0 / LINE_FULL_SCALE))
        .round()
        .clamp(i16::MIN.into(), i16::MAX.into()) as i16
}

impl Analog {
    pub fn new() -> Self {
        Self::default()
    }

    /// 次の刻みの時刻。始めていなければ、`start`（48 kHz の標本の時刻）から始める。
    pub fn next_tick(&mut self, start: Time) -> Time {
        match self.next {
            Some(k) => k * TICK,
            None => {
                let k = start.div_ceil(TICK * Time::from(TICKS_PER_SAMPLE))
                    * Time::from(TICKS_PER_SAMPLE);
                self.next = Some(k);
                self.fresh = true;
                k * TICK
            }
        }
    }

    /// 時刻 `now` の前に、処理する刻みがあるか（まだ始めていなければ true）。スロットごとに呼ぶので軽くする。
    #[inline]
    pub fn due(&self, now: Time) -> bool {
        self.next.is_none_or(|k| k * TICK < now)
    }

    /// 始めたばかりの刻みか（入力の積分を捨てて、今の出力を出す）。
    pub fn is_fresh(&self) -> bool {
        self.fresh
    }

    /// 次の刻みが、48 kHz の標本の最初の刻みか（始めたばかりの刻みは標本を作らないので含めない）。
    pub fn starts_sample(&self) -> bool {
        !self.fresh && self.phase == 0
    }

    /// 刻みを 1 つ進める。48 kHz の標本ができたら返す。
    pub fn tick(&mut self, inp: &Inputs) -> Option<(i16, i16)> {
        let k = self.next.unwrap_or(0);
        self.next = Some(k + 1);
        if self.fresh {
            self.fresh = false;
            self.phase = 0;
            self.acc = [F(0.0); 2];
            return Some((to_sample(self.out[0].0), to_sample(self.out[1].0)));
        }
        let c = Coefs::get();
        let sat = parts::V_SAT;
        let mut clipped = false;
        let mut clip = |v: f64| {
            if v.abs() > sat {
                clipped = true;
                v.clamp(-sat, sat)
            } else {
                v
            }
        };
        // ADPCM（モノラル）：フォロワ → SK → MFB → 結合容量
        let a = self.ad_sk.run(&c.ad_sk, inp.da * c.ad_volts);
        let a = self.ad_mfb.run(&c.ad_mfb, a);
        let a = self.ad_hpf.run(&c.ad_hpf, a);
        // Audio IN：入力を渡されているか、前に渡されて段が動いているときだけ（DR-129）
        let line = match inp.line {
            Some(v) => Some(self.line.tick(c, v)),
            None if self.line.used => Some(self.line.tick(c, [0.0; 2])),
            None => None,
        };
        for i in 0..2 {
            // DOC：反転加算 → SK
            let d = clip(self.doc_sum[i].run(&c.doc_sum, inp.doc[i] * c.doc_volts));
            let d = self.doc_sk[i].run(&c.doc_sk, d);
            // 本体 DAC：I/V → MFB
            let m = self.dac_mfb[i].run(&c.dac_mfb, inp.dac[i] * c.dac_volts);
            let m = if inp.dac_on { m } else { 0.0 };
            // ミキサ → 結合容量
            let mut sum = c.mix_doc * d + c.mix_dac * m + c.mix_ad * a;
            if let Some(l) = line {
                sum += c.mix_lin * l[i];
            }
            let x = clip(sum);
            self.mixer[i] = F(x);
            let y = self.out_hpf[i].run(&c.out_hpf, x);
            self.out[i] = F(y);
            self.acc[i].0 += y;
        }
        if clipped {
            self.clips += 1;
        }
        self.phase += 1;
        if self.phase < TICKS_PER_SAMPLE {
            return None;
        }
        self.phase = 0;
        let n = f64::from(TICKS_PER_SAMPLE);
        let s = (to_sample(self.acc[0].0 / n), to_sample(self.acc[1].0 / n));
        self.acc = [F(0.0); 2];
        Some(s)
    }

    /// 最後の刻みの LINE OUT の電圧（L、R）。
    pub fn output(&self) -> (f64, f64) {
        (self.out[0].0, self.out[1].0)
    }
}
