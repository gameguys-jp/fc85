//! DOC 子基板（ES5503 と波形 RAM 64 KiB）と、port 2 のアダプタ P2A、68000 から DOC のレジスタへの
//! ブリッジ。
//!
//! DR-011、DR-016、DR-019、DR-030、DR-031 (4)、DR-035、DR-050、仕様書 §05・§06（port 2）、
//! fabric.md §06・§08、台帳 ES5503.md §2・§3、HD68000.md §2.2、host-acrtc-interface.md §3〜§6。
//!
//! 模型：
//! - DOC のサイクルは E の 1 周期（CLK 8 クロック。8 MHz で 1 µs＝15120 単位）。E の立下りでサイクルが
//!   始まり、E Low の間に DOC がオシレータを 1 つ訪れて波形 RAM を読む（`tick`）。E High はホストの窓
//!   （台帳 §3.3）。E の立下りは `set_phase` で決める（既定は 0）。CLK を外部のクロックに替えるジャンパは
//!   `set_clock_period`（DR-035）。
//! - P2A（fabric.md §08、DR-050）：P2A はアクセスを 1 つ預かる。Fabric が port 2 のアクセスを始め、
//!   預かっているものがなければ、番地・データ・向きをラッチする（スロットの頭の 20 ns 後の、16 MHz の
//!   次の立上り。番地は U48–U49、書込みのデータは U50 の 74LS374）。書込みはラッチしたらすぐ Fabric に
//!   終わり（OPDN）を返す。預かっているものがあれば、その窓が終わるまで Fabric を待たせる。
//! - E を 16 MHz の 2 段の FF で同期化し、預かったアクセスは、同期化した E の立下り（ラッチと同じ縁を
//!   含む）で次の E High の窓を取る。窓では CS̄ を出し、書込みはその窓で波形 RAM に書き、読出しは窓の
//!   終わり（E の立下り）の値を読出しのラッチ（U58 の 74LS374）に取る。同期化した E の次の立下りで
//!   預かりを空け、読出しならそこで Fabric に終わりを返す [P]。続けて書くと、窓の終わりの縁で次の
//!   書込みをラッチし、すぐ後の窓を取る。窓の前に番地を出しておくのは、台帳 §2 の注意 2（アドレスは
//!   直前の E Low の間に出す）による。
//! - 打ち切り（時間切れ、STOP）：待たせている間なら取り消す。預かった読出しは、窓を取る前なら取り消し、
//!   E が上がった後なら窓のアクセスは行い、答えだけを捨てる [V]。預かった書込みは答えた後なので、
//!   打ち切りとは関係なく窓で書く。
//! - 波形 RAM は 64 KiB。DOC の BS（番地の bit 16）はつながず、上下のバンクは同じ RAM（台帳 §2）。
//! - 68000 のブリッジ（段階 3。OI-055）：DOC の窓（0x210000〜0x2101FF）の reg n は 0x210000＋2n で、
//!   上位バイトレーン（UDS だけ）。ほかの幅と、子基板がないときは DTACK を返さず、バスの時間切れで
//!   バスエラー（仕様書 §05、DR-019）。ブリッジは CS を S4 の立上りで登録し（DR-016 の ACRTC と同じ形）、
//!   レジスタの番号・データ・向きを取り込む。P2A と同じ同期化した E を見て、同期化した E の立下り（取り
//!   込みと同じ縁を含む）で次の窓を取る。P2A と同じ縁で取り合うときは 68000 が先 [P]。窓の終わり
//!   （E の立下り）で DOC のレジスタに書くか、読んだ値をラッチし、同期化した E の次の立下りで DTACK を
//!   出す。68000 はその後の最初のクロックの立下りで DTACK を認識する（S4 の終わりより前にはならない）。
//!   68000 の書込みは預からない（68000 は DTACK まで番地とデータを保つ）[P]。
//! - 68000 のクロック（8 MHz）は 16 MHz の 2 分周で、立上りは `PERIOD_8M` の倍数の時刻 [P]。
//! - DOC の出力段（段階 4）：CSTRB ごとに、CA3–CA0 のチャネルの S/H を 2 通りに更新する。デジタルの出力
//!   （`chan`、DR-054）は SIG の値をそのまま取る。アナログの S/H（`sh`、analog.rs）は SIG± の配線
//!   （`sig_wiring`、DR-011 のジャンパ）で電流を決め、RC の充電で `sh_alpha` だけ近づく。偶数・奇数の和の
//!   時間の積分を LINE OUT（`console` の `line`）が刻みごとに取り出す（`take_line`）。片側の配線の電流は、
//!   訪れたオシレータの音量のレジスタを使う [V]。

use crate::acrtc::{T_OUT, T_SETUP};
use crate::analog::{Hold, SigWiring, sh_alpha_q16};
use crate::fabric::{Access, Dir, PortReply};
use fc85_chips::es5503::{Es5503, Slot, WaveBus};
use fc85_core::time::{Clock, PERIOD_8M, PERIOD_16M, PERIOD_DOC_E, SLOT, Time};
use serde::{Deserialize, Serialize};

/// 波形 RAM の大きさ（HM4864A×8。DR-031 (4)）。
pub const WAVE_BYTES: usize = 0x10000;

/// 時刻つきの線のレベル。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Level {
    level: bool,
    prev: bool,
    at: Time,
}

impl Level {
    fn value(&self, t: Time) -> bool {
        if t >= self.at { self.level } else { self.prev }
    }

    fn set(&mut self, level: bool, t: Time) {
        self.prev = self.value(t);
        self.level = level;
        self.at = t;
    }
}

/// 68000 の DOC の窓。
pub const HOST_BASE: u32 = 0x21_0000;
pub const HOST_LAST: u32 = 0x21_01FF;

/// 68000 の番地とデータストローブから、DOC のレジスタの番号を出す。上位バイトレーン（UDS だけ）の
/// 偶数番地でなければ `None`（DTACK を返さない。仕様書 §05、台帳 ES5503.md §2 の注意 4）。
pub fn host_register(addr: u32, uds: bool, lds: bool) -> Option<u8> {
    if !(HOST_BASE..=HOST_LAST).contains(&addr) || addr & 1 != 0 || !uds || lds {
        return None;
    }
    Some(((addr - HOST_BASE) >> 1) as u8)
}

/// P2A とブリッジの段。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum Stage {
    /// 預かっているアクセスがない。
    #[default]
    Idle,
    /// 預かった。同期化した E の立下りを待つ。
    Wait,
    /// 次の E High の窓を取った。
    Claimed,
}

/// P2A がラッチしたアクセス。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Request {
    write: bool,
    addr: u16,
    data: u8,
    /// ラッチの時刻。
    at: Time,
}

/// ブリッジが取り込んだ 68000 のバスサイクル。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct HostReq {
    reg: u8,
    write: Option<u8>,
    /// S0 の立上り。
    s0: Time,
    /// S4 の立上り（CS の登録）。
    at: Time,
}

/// DTACK を返した 68000 の DOC のアクセス。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostDone {
    pub reg: u8,
    pub write: Option<u8>,
    /// 読んだ値（D15–D8）。
    pub read: Option<u8>,
    pub s0: Time,
    /// 使った窓の E の立上りと立下り。
    pub rise: Time,
    pub fall: Time,
    /// ブリッジが DTACK を出した時刻（16 MHz の立上り）。
    pub dtack: Time,
    /// 68000 が DTACK を認識したクロックの立下り。
    pub recognized: Time,
    /// 待ちの数（68000 のクロック）。バスサイクルは 4＋waits クロック。
    pub waits: u32,
}

/// 68000 が DTACK を認識する立下りと、待ちの数。DTACK はブリッジの FF から出るので、出た時刻と同じ
/// 立下りには間に合わない（FF の遅れと tASI 20 ns。HD68000.md §2.1 の #47）。
pub fn host_recognize(s0: Time, dtack: Time) -> (Time, u32) {
    let half = PERIOD_8M / 2;
    let s4_end = s0 + 2 * PERIOD_8M + half;
    let mut f = dtack / PERIOD_8M * PERIOD_8M + half;
    if f <= dtack {
        f += PERIOD_8M;
    }
    let f = f.max(s4_end);
    (f, ((f - s4_end) / PERIOD_8M) as u32)
}

/// 68000 のブリッジ。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Bridge {
    /// 始まった、まだ S4 の来ていないサイクル。
    queued: Option<HostReq>,
    stage: Stage,
    req: Option<HostReq>,
    /// 取った窓に E が上がった。
    owned: bool,
    /// 窓が済んだ：読んだ値と E の立下りの時刻。
    op: Option<(Option<u8>, Time)>,
    /// バスエラーで終わったサイクルの窓。DTACK を出さない。
    discard: bool,
    done: Option<HostDone>,
}

/// DOC が訪れたオシレータの読出し（記録）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OscRead {
    pub n: u8,
    pub addr: u32,
    pub wave: u8,
}

/// P2A が使った窓（記録）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    /// P2A がアクセスをラッチした時刻。
    pub latched: Time,
    /// E の立上りと立下りの時刻。
    pub rise: Time,
    pub fall: Time,
    pub write: bool,
    pub addr: u16,
    pub data: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocBoard {
    pub doc: Es5503,
    pub wave: Vec<u8>,
    /// E の立下り（DOC サイクルの頭）。
    fall: Clock,
    /// 今のサイクルの E の立上りがまだか。
    rise_pending: bool,
    e: Level,
    /// E の同期化の FF（16 MHz）。
    sync: [bool; 2],
    stage: Stage,
    /// Fabric が始めた、まだラッチしていないアクセス（Fabric は待っている）。
    queued: Option<Request>,
    /// 預かっているアクセス。
    req: Option<Request>,
    /// 取った窓に E が上がった。
    owned: bool,
    /// 窓のアクセスが済んだ（読んだ値）。
    op_done: Option<u8>,
    /// 打ち切った後で、答えを捨てる。
    discard: bool,
    reply: Option<u16>,
    /// DOC のサイクルの数。
    pub cycles: u64,
    /// 最後に訪れたオシレータの読出しと、最後の SIG±。
    pub last_osc: Option<OscRead>,
    pub last_sig: i32,
    /// 出力段のサンプルホールド（CA3–CA0 のチャネルごと。CSTRB で SIG を取る）と、DOC のサイクルごとの和
    /// （音声の出力が平均をとる）。DR-054。
    pub chan: [i32; 16],
    pub chan_sum: [i64; 16],
    pub chan_cycles: u32,
    /// P2A が最後に使った窓。
    pub last_window: Option<Window>,
    /// P2A が使った窓の数。
    pub windows: u64,
    bridge: Bridge,
    /// 68000 が使った窓の数。
    pub host_windows: u64,
    /// SIG± の配線（DR-011 のジャンパ。仕様書 §11 のベンド点「DOC 出力の片側」）。Bend の切り替えはここ。
    #[serde(default)]
    pub sig_wiring: SigWiring,
    /// アナログの S/H が 1 回の CSTRB で差を縮める割合（Q16）。CLK の周期で決まる（analog.rs）。
    #[serde(default = "default_sh_alpha")]
    sh_alpha: u32,
    /// アナログの S/H 16 個の値（SIG± の差の半分の単位×65536。analog.rs の `doc_volts_per_unit`）。
    #[serde(default)]
    sh: [i64; 16],
    /// S/H の偶数（L）・奇数（R）の和の、時間の積分（LINE OUT の入力）。
    #[serde(default)]
    sh_sum: [Hold; 2],
}

fn default_sh_alpha() -> u32 {
    sh_alpha_q16(PERIOD_8M)
}

impl Default for DocBoard {
    fn default() -> Self {
        Self::new()
    }
}

struct WaveRam<'a>(&'a [u8]);

impl WaveBus for WaveRam<'_> {
    fn read_wave(&mut self, addr: u32) -> u8 {
        self.0[(addr as usize) % WAVE_BYTES]
    }
}

impl DocBoard {
    /// 電源投入時。波形 RAM は 0 [V]。
    pub fn new() -> Self {
        Self {
            doc: Es5503::new(),
            wave: vec![0; WAVE_BYTES],
            fall: Clock::new(PERIOD_DOC_E, 0),
            rise_pending: false,
            e: Level::default(),
            sync: [false; 2],
            stage: Stage::Idle,
            queued: None,
            req: None,
            owned: false,
            op_done: None,
            discard: false,
            reply: None,
            cycles: 0,
            last_osc: None,
            last_sig: 0,
            chan: [0; 16],
            chan_sum: [0; 16],
            chan_cycles: 0,
            last_window: None,
            windows: 0,
            bridge: Bridge::default(),
            host_windows: 0,
            sig_wiring: SigWiring::Both,
            sh_alpha: default_sh_alpha(),
            sh: [0; 16],
            sh_sum: [Hold::default(); 2],
        }
    }

    /// 68000 のバスサイクルを始める（`host_register` で番号を出した後）。`s0` は S0 の立上りで、次に
    /// 進めるスロットの頭より前にしない。書込みは `write` に D15–D8。
    pub fn host_begin(&mut self, s0: Time, reg: u8, write: Option<u8>) {
        debug_assert!(s0.is_multiple_of(PERIOD_8M));
        debug_assert!(self.bridge.queued.is_none() && self.bridge.stage == Stage::Idle);
        self.bridge.queued = Some(HostReq {
            reg,
            write,
            s0,
            at: s0 + 2 * PERIOD_8M,
        });
    }

    /// DTACK を返したアクセス。
    pub fn host_poll(&mut self) -> Option<HostDone> {
        self.bridge.done.take()
    }

    /// バスエラー（時間切れ）でサイクルが終わった。窓を取る前なら取り消す。E が上がった後なら、
    /// 書込みは DOC に届き、読んだ値は捨てる（0xE0・0xE2 の読出しの副作用は起きる）[V]。
    pub fn host_abort(&mut self) {
        let b = &mut self.bridge;
        b.queued = None;
        b.done = None;
        if b.owned || b.op.is_some() {
            b.discard = true;
        } else {
            b.stage = Stage::Idle;
            b.req = None;
        }
    }

    /// ブリッジがサイクルを持っているか。
    pub fn host_busy(&self) -> bool {
        self.bridge.queued.is_some() || self.bridge.stage != Stage::Idle
    }

    /// E の立下りの位相（既定は 0）。`now` は次に進めるスロットの頭。
    pub fn set_phase(&mut self, now: Time, units: Time) {
        let p = self.fall.period();
        self.fall.set_next(now + units % p);
        self.settle(now);
    }

    /// 次の E の立下りから、`now` の E のレベルと同期化の FF を決める（位相を決めた直後に、偽の縁を
    /// 見ず、本当の縁を落とさないため）。
    fn settle(&mut self, now: Time) {
        let high = self.fall.next() - now <= self.fall.period() / 2;
        self.e = Level {
            level: high,
            prev: high,
            at: now,
        };
        self.rise_pending = !high;
        self.sync = [high; 2];
    }

    /// DOC の CLK の周期（既定は 8 MHz。外部クロックのジャンパ。DR-035）。E の周期はその 8 倍。
    /// 次の E の立下りは変えない。
    pub fn set_clock_period(&mut self, clk: Time) {
        self.fall.set_period(8 * clk);
        self.sh_alpha = sh_alpha_q16(clk);
    }

    /// Fabric への答え。
    pub fn port_reply(&mut self) -> PortReply {
        match self.reply.take() {
            Some(v) => PortReply::Done(v),
            None => PortReply::Wait,
        }
    }

    /// 打ち切り（時間切れ、STOP）。
    pub fn abort(&mut self) {
        self.reply = None;
        // 待たせているアクセス：取り消す。預かっているもの（前の書込み）はそのまま
        if self.queued.take().is_some() {
            return;
        }
        // 預かった読出し
        if self.req.is_some_and(|r| !r.write) {
            if self.owned {
                self.discard = true;
            } else {
                self.stage = Stage::Idle;
                self.req = None;
            }
        }
    }

    /// 預かっているアクセスがあるか（書込みの窓がまだ）。
    pub fn busy(&self) -> bool {
        self.stage != Stage::Idle
    }

    /// `t0` から 1 スロット進める。`starting` は、このスロットで始まる port 2 のアクセス。
    pub fn run_slot(&mut self, t0: Time, starting: Option<&Access>) {
        let end = t0 + SLOT;
        // 子基板を後から挿した（試験の設定）：位相を保って今のスロットまで飛ばす
        let p = self.fall.period();
        if self.fall.next() + p < t0 {
            let skip = (t0 - self.fall.next()) / p * p;
            self.fall.set_next(self.fall.next() + skip);
            self.settle(t0);
        }
        if let Some(a) = starting {
            self.queued = Some(Request {
                write: a.dir == Dir::Write,
                addr: a.addr,
                data: a.data as u8,
                at: t0 + T_OUT,
            });
        }
        let mut q = 0;
        loop {
            let ff = (q < 4).then(|| t0 + q * PERIOD_16M);
            let rise = self
                .rise_pending
                .then(|| self.fall.next().saturating_sub(self.fall.period() / 2))
                .filter(|&r| r < end);
            let fall = (self.fall.next() < end).then(|| self.fall.next());
            // 同じ時刻なら E の縁を先にする（FF はセットアップの分だけ前の値を取る）
            let e_edge = match (rise, fall) {
                (Some(r), Some(f)) => Some(r.min(f)),
                (r, f) => r.or(f),
            };
            match (ff, e_edge) {
                (None, None) => break,
                (Some(f), Some(e)) if e <= f => self.e_edge(e, rise == Some(e)),
                (Some(f), _) => {
                    self.ff_edge(f);
                    q += 1;
                }
                (None, Some(e)) => self.e_edge(e, rise == Some(e)),
            }
        }
    }

    /// E の縁。
    fn e_edge(&mut self, t: Time, rising: bool) {
        if rising {
            self.rise_pending = false;
            self.e.set(true, t);
            if self.bridge.stage == Stage::Claimed {
                self.bridge.owned = true;
            }
            if self.stage == Stage::Claimed {
                self.owned = true;
                if let Some(r) = self.req.as_mut().filter(|r| r.write) {
                    self.wave[usize::from(r.addr)] = r.data;
                }
            }
            return;
        }
        let t = self.fall.take_before(Time::MAX).unwrap_or(t);
        // 窓の終わり：読出しの値を取る
        if self.owned {
            self.owned = false;
            if let Some(r) = self.req {
                let v = self.wave[usize::from(r.addr)];
                self.op_done = Some(if r.write { 0 } else { v });
                self.last_window = Some(Window {
                    latched: r.at,
                    rise: t - self.fall.period() / 2,
                    fall: t,
                    write: r.write,
                    addr: r.addr,
                    data: if r.write { r.data } else { v },
                });
                self.windows += 1;
            }
        }
        // 68000 の窓の終わり：レジスタに書くか、読んだ値をラッチする
        if self.bridge.owned {
            self.bridge.owned = false;
            if let Some(r) = self.bridge.req {
                let read = match r.write {
                    Some(v) => {
                        self.doc.write(r.reg, v);
                        None
                    }
                    None => Some(self.doc.read(r.reg)),
                };
                self.bridge.op = Some((read, t));
                self.host_windows += 1;
            }
        }
        self.e.set(false, t);
        self.rise_pending = true;
        // DOC のサイクル：E Low で波形 RAM を読む
        self.cycles += 1;
        let tick = self.doc.tick(&mut WaveRam(&self.wave));
        if let Slot::Osc { n, addr, wave } = tick.slot {
            self.last_osc = Some(OscRead { n, addr, wave });
        }
        self.last_sig = tick.sig;
        if tick.strobe {
            let c = usize::from(tick.channel & 15);
            self.chan[c] = tick.sig;
            if let Slot::Osc { n, .. } = tick.slot {
                self.sample_hold(t, c, tick.sig, n);
            }
        }
        for (s, &c) in self.chan_sum.iter_mut().zip(&self.chan) {
            *s += i64::from(c);
        }
        self.chan_cycles += 1;
    }

    /// アナログの S/H（CSTRB の Low の間に、チャネル `c` の保持容量を SIG の電圧へ充電する）。
    /// 電流は I± ＝ I_FS×(音量/255)×(1/2 ± s/256)。両側は差 2×sig、片側は SIG+ だけで sig＋128×音量
    /// （単位は差の半分）[P][V]。
    fn sample_hold(&mut self, t: Time, c: usize, sig: i32, n: u8) {
        let x = match self.sig_wiring {
            SigWiring::Both => 2 * i64::from(sig),
            SigWiring::One => i64::from(sig) + 128 * i64::from(self.doc.volume(usize::from(n))),
        } << 16;
        let d = ((x - self.sh[c]) * i64::from(self.sh_alpha)) >> 16;
        if d != 0 {
            self.sh[c] += d;
            let side = &mut self.sh_sum[c & 1];
            let v = side.value() + d;
            side.set(t, v);
        }
    }

    /// アナログの S/H 16 個の値（analog.rs の単位）。
    pub fn sample_holds(&self) -> &[i64; 16] {
        &self.sh
    }

    /// S/H の割合（Q16）。
    pub fn sh_alpha(&self) -> u32 {
        self.sh_alpha
    }

    /// LINE OUT：前に取り出してから `to` までの、S/H の偶数（L）・奇数（R）の和の積分（値×時刻の単位）。
    pub fn take_line(&mut self, to: Time) -> [i64; 2] {
        [self.sh_sum[0].take(to), self.sh_sum[1].take(to)]
    }

    /// 前に呼んでからのチャネルごとの平均を返し、和を 0 にする。DOC のサイクルがなければ今の値。
    pub fn take_channel_average(&mut self) -> [i32; 16] {
        let n = i64::from(self.chan_cycles);
        let mut out = self.chan;
        if n > 0 {
            for (o, s) in out.iter_mut().zip(&self.chan_sum) {
                *o = (*s / n) as i32;
            }
        }
        self.chan_sum = [0; 16];
        self.chan_cycles = 0;
        out
    }

    /// 16 MHz の立上り：E の同期化と、P2A・ブリッジの段。
    fn ff_edge(&mut self, t: Time) {
        let s2_old = self.sync[1];
        self.sync[1] = self.sync[0];
        self.sync[0] = self.e.value(t.saturating_sub(T_SETUP));
        let fell = s2_old && !self.sync[1];
        // 68000 の窓が終わった：DTACK を出す
        let b = &mut self.bridge;
        if fell
            && b.stage == Stage::Claimed
            && let Some((read, fall)) = b.op.take()
        {
            if let Some(r) = b.req.filter(|_| !b.discard) {
                let (recognized, waits) = host_recognize(r.s0, t);
                b.done = Some(HostDone {
                    reg: r.reg,
                    write: r.write,
                    read,
                    s0: r.s0,
                    rise: fall - self.fall.period() / 2,
                    fall,
                    dtack: t,
                    recognized,
                    waits,
                });
            }
            b.discard = false;
            b.stage = Stage::Idle;
            b.req = None;
        }
        // S4 の立上りで CS を登録する
        if b.stage == Stage::Idle
            && let Some(r) = b.queued.filter(|r| r.at <= t)
        {
            b.queued = None;
            b.req = Some(r);
            b.stage = Stage::Wait;
        }
        // 窓が終わった：預かりを空ける。読出しはここで答える
        if fell
            && self.stage == Stage::Claimed
            && let Some(v) = self.op_done.take()
        {
            if self.req.is_some_and(|r| !r.write) && !self.discard {
                self.reply = Some(u16::from(v));
            }
            self.discard = false;
            self.stage = Stage::Idle;
            self.req = None;
        }
        // ラッチ：Fabric の出力が来ていて、預かっているものがなければ。書込みはすぐ答える
        if self.stage == Stage::Idle
            && let Some(r) = self.queued.filter(|r| r.at <= t)
        {
            self.queued = None;
            self.req = Some(Request { at: t, ..r });
            self.stage = Stage::Wait;
            if r.write {
                self.reply = Some(0);
            }
        }
        // 同期化した E の立下りで次の窓を取る（同じ縁でラッチしたものも。番地から E の立上りまで
        // 365 ns 以上ある）。68000 と P2A が同じ縁で待っていれば 68000 が先 [P]（OI-055）
        if fell {
            if self.bridge.stage == Stage::Wait {
                self.bridge.stage = Stage::Claimed;
            } else if self.stage == Stage::Wait {
                self.stage = Stage::Claimed;
            }
        }
    }
}
