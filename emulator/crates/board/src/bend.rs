//! 通常モードと Bend モード、ベンド点の設定（仕様書 §11、DR-034、DR-035、DR-063、DR-065）。
//!
//! ベンド点の設定は `Bends` にまとめて持ち、Bend モードのときだけ回路に入れる
//! （`System::apply_bends`）。通常モードは工場出荷の機械で、設定は残したまま回路から外す。
//!
//! 文字 ROM（32 KiB）のソケットとテストピンで、番地のピン A0〜A14 とデータのピン D0〜D7 を、
//! GND・+5 V への固定、ほかの線との入替え、浮かせにできる。68000 から読んでも Data Fabric の
//! port 9 から読んでも、同じソケットの線を通る。番号は ROM のピンの番号で、68000 の番地では
//! ROM の A0 が 68000 の A1 に当たる（仕様書 §08）。

use crate::analog::SigWiring;
use crate::doc::DocLines;
use crate::dram::DramEnv;
use fc85_core::time::{PERIOD_8M, PERIOD_20M, PERIOD_384K, Time, UNITS_PER_SECOND};
use serde::{Deserialize, Serialize};

/// 通常モードと Bend モード（仕様書 §11）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// 工場出荷の機械。ベンド点は何もつながず、ソフトの規約違反は警告する。
    #[default]
    Normal,
    /// ベンド点の設定を回路に入れる。規約違反は警告しない（実行は両方のモードで同じ）。
    Bend,
}

/// テストピンにつなぐもの。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pin {
    /// 何もつながない。
    #[default]
    Open,
    /// GND に固定（1 kΩ 程度を挟む）。
    Low,
    /// +5 V に固定（1 kΩ 程度を挟む）。
    High,
}

/// FIFO（67401）の EMPTY・FULL のテストピン（DR-034）。EMPTY は OR の反転、FULL は IR の反転で、
/// FIFO の外の回路（Data Fabric、アダプタ、DSP の BIO、状態のレジスタ）はみなこの線を見る。
/// FIFO の中は線によらず、満杯なら Shift In を捨て、空なら前の値を出し続ける。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FifoPins {
    pub empty: Pin,
    pub full: Pin,
}

/// テストピンのある FIFO の名前（`FabricPorts` の dac・dsp_in・dsp_out・adpcm の順）。
pub const FIFO_NAMES: [&str; 4] = ["dac", "dsp_in", "dsp_out", "adpcm"];

/// クロックのジャンパ（DR-035）。`None` は基板の発振器。値は周期。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clocks {
    /// DOC の CLK（基板では 8 MHz）。
    pub doc: Option<Time>,
    /// TMS32010 の CLKIN（基板では 20 MHz）。
    pub dsp: Option<Time>,
    /// MSM5205 の XT（基板では 384 kHz）。
    pub adpcm: Option<Time>,
    /// 音声クロックの分周器の入力（基板では 8 MHz）。
    pub audio: Option<Time>,
}

impl Clocks {
    /// ジャンパの先の周期（ジャンパがなければ基板の発振器）。順は DOC、DSP の CLKIN、XT、音声。
    pub fn periods(&self) -> [Time; 4] {
        [
            self.doc.unwrap_or(PERIOD_8M),
            self.dsp.unwrap_or(PERIOD_20M),
            self.adpcm.unwrap_or(PERIOD_384K),
            self.audio.unwrap_or(PERIOD_8M),
        ]
    }
}

/// ベンド点の設定とモード（DR-065）。保存と復元に入る。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bends {
    pub mode: Mode,
    /// 文字 ROM のソケットの線（DR-063）。
    pub char_lines: RomLines,
    pub clocks: Clocks,
    /// FIFO のテストピン（`FIFO_NAMES` の順）。
    pub fifos: [FifoPins; 4],
    /// DOC の SIG± の配線のジャンパ（DR-011。仕様書 §11 のベンド点「DOC 出力の片側」）。
    #[serde(default)]
    pub sig: SigWiring,
    /// DRAM の個体と温度（リフレッシュが遅れたときの崩れ方。`dram`、DR-072）。
    #[serde(default)]
    pub dram: DramEnv,
    /// DOC 子基板のバスのテストピン（仕様書 §11 のベンド点「DOC波形RAMの線」。`docbus`）。
    #[serde(default)]
    pub doc_lines: DocLines,
}

impl Bends {
    /// 設定がどれも工場出荷のままか（モードは見ない）。
    pub fn is_factory(&self) -> bool {
        self.char_lines.is_normal()
            && self.clocks == Clocks::default()
            && self.fifos == [FifoPins::default(); 4]
            && self.sig == SigWiring::Both
            && self.dram == DramEnv::default()
            && self.doc_lines.is_normal()
    }

    /// 1 行の命令で設定を変える。形は次のとおり（大文字・小文字は区別しない）。
    ///
    /// - `mode normal`・`mode bend`・`mode toggle`
    /// - `char A1=A2,A2=A1,D6=0,D2=Z`（`char normal` で戻す。`RomLines::parse`）
    /// - `clock doc|dsp|adpcm|audio <Hz>`（`off` で基板の発振器に戻す。`6e6` の形も読む）
    /// - `fifo dac|dsp_in|dsp_out|adpcm empty=0|1|open full=0|1|open`（`open` だけで両方を外す）
    /// - `sig both|one`：DOC の SIG± の配線（`one` は SIG+ だけの初期の片側の配線。DR-011）
    /// - `dram unit <N>`・`dram temp <°C>`：DRAM の個体と温度（リフレッシュが遅れたときの崩れ方。DR-072）
    /// - `docbus A3=0,A4=A5,A5=A4,D7=1,A9=Z`：DOC 子基板のバスのテストピン（`normal` で戻す。`DocLines::parse`）
    /// - `factory`：設定を全部工場出荷に戻す（モードは変えない）
    pub fn command(&mut self, line: &str) -> Result<(), String> {
        let line = line.trim();
        let (head, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        match head.to_ascii_lowercase().as_str() {
            "mode" => {
                self.mode = match rest.to_ascii_lowercase().as_str() {
                    "normal" => Mode::Normal,
                    "bend" => Mode::Bend,
                    "toggle" => match self.mode {
                        Mode::Normal => Mode::Bend,
                        Mode::Bend => Mode::Normal,
                    },
                    _ => return Err(format!("mode は normal・bend・toggle: {line}")),
                }
            }
            "char" => self.char_lines.parse(rest)?,
            "docbus" => self.doc_lines.parse(rest)?,
            "clock" => {
                let mut f = rest.split_whitespace();
                let (Some(which), Some(hz)) = (f.next(), f.next()) else {
                    return Err(format!("clock <doc|dsp|adpcm|audio> <Hz|off>: {line}"));
                };
                let period = if hz.eq_ignore_ascii_case("off") {
                    None
                } else {
                    let hz: f64 = hz
                        .parse()
                        .ok()
                        .filter(|h: &f64| *h > 0.0)
                        .ok_or_else(|| format!("周波数でない: {hz}"))?;
                    Some((UNITS_PER_SECOND as f64 / hz).round().max(1.0) as Time)
                };
                let slot = match which.to_ascii_lowercase().as_str() {
                    "doc" => &mut self.clocks.doc,
                    "dsp" => &mut self.clocks.dsp,
                    "adpcm" => &mut self.clocks.adpcm,
                    "audio" => &mut self.clocks.audio,
                    _ => return Err(format!("クロックの名前でない: {which}")),
                };
                *slot = period;
            }
            "fifo" => {
                let mut f = rest.split_whitespace();
                let name = f.next().unwrap_or("").to_ascii_lowercase();
                let k = FIFO_NAMES.iter().position(|n| *n == name).ok_or_else(|| {
                    format!("FIFO の名前でない（{}）: {name}", FIFO_NAMES.join("・"))
                })?;
                for item in f {
                    let item = item.to_ascii_lowercase();
                    if item == "open" {
                        self.fifos[k] = FifoPins::default();
                        continue;
                    }
                    let (pin, v) = item
                        .split_once('=')
                        .ok_or_else(|| format!("empty=… か full=…: {item}"))?;
                    let v = match v {
                        "0" => Pin::Low,
                        "1" => Pin::High,
                        "open" => Pin::Open,
                        _ => return Err(format!("0・1・open: {item}")),
                    };
                    match pin {
                        "empty" => self.fifos[k].empty = v,
                        "full" => self.fifos[k].full = v,
                        _ => return Err(format!("empty か full: {item}")),
                    }
                }
            }
            "sig" => {
                self.sig = match rest.trim().to_ascii_lowercase().as_str() {
                    "both" => SigWiring::Both,
                    "one" => SigWiring::One,
                    _ => return Err(format!("sig は both・one: {line}")),
                }
            }
            "dram" => {
                let mut f = rest.split_whitespace();
                let what = f.next().unwrap_or("").to_ascii_lowercase();
                let v = f.next().unwrap_or("");
                match what.as_str() {
                    "unit" => {
                        self.dram.unit =
                            v.parse().map_err(|_| format!("個体の番号でない: {line}"))?;
                    }
                    "temp" => {
                        let t: i32 = v.parse().map_err(|_| format!("温度でない: {line}"))?;
                        if !(-20..=100).contains(&t) {
                            return Err(format!("温度は -20〜100 °C: {line}"));
                        }
                        self.dram.temp_c = t;
                    }
                    _ => return Err(format!("dram unit <N> か dram temp <°C>: {line}")),
                }
            }
            "factory" => {
                *self = Self {
                    mode: self.mode,
                    ..Self::default()
                }
            }
            _ => {
                return Err(format!(
                    "知らない命令（mode・char・clock・fifo・sig・dram・docbus・factory）: {line}"
                ));
            }
        }
        Ok(())
    }

    /// 今の設定を 1 行で表す（ホストの表示用）。
    pub fn summary(&self) -> String {
        let mut v = vec![match self.mode {
            Mode::Normal => "通常モード".to_string(),
            Mode::Bend => "Bend モード".to_string(),
        }];
        if !self.char_lines.is_normal() {
            v.push("文字 ROM の線".into());
        }
        if !self.doc_lines.is_normal() {
            v.push("DOC のバスの線".into());
        }
        let names = ["DOC", "DSP", "ADPCM", "音声"];
        let c = self.clocks;
        for (n, p) in names.iter().zip([c.doc, c.dsp, c.adpcm, c.audio]) {
            if let Some(p) = p {
                v.push(format!("{n} {:.0} Hz", UNITS_PER_SECOND as f64 / p as f64));
            }
        }
        for (n, p) in FIFO_NAMES.iter().zip(self.fifos) {
            if p != FifoPins::default() {
                v.push(format!("FIFO {n}"));
            }
        }
        if self.sig == SigWiring::One {
            v.push("DOC の SIG＋だけ".into());
        }
        if self.dram != DramEnv::default() {
            v.push(format!(
                "DRAM 個体 {} {} °C",
                self.dram.unit, self.dram.temp_c
            ));
        }
        v.join("、")
    }
}

/// 1 本のピンに何をつなぐか。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wire {
    /// 線 n。番地のピンなら基板の側の番地線 n、データの線なら ROM の出力 D n。
    /// 自分の番号ならそのまま、別の番号なら入替え。
    Line(u8),
    /// GND に固定。
    Low,
    /// +5 V に固定（1 kΩ 程度を挟む）。
    High,
    /// 浮かせる。値は不定 [V]。エミュレータは読むたびに擬似乱数で決める [P]。
    Float,
}

/// ソケットの線のつなぎ方。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomLines {
    /// ROM の番地のピン k に来るもの。
    pub addr: Vec<Wire>,
    /// 基板のデータ線 k に来るもの。
    pub data: Vec<Wire>,
    /// 浮かせた線の値（16 bit の LFSR）。保存して戻しても同じ列が続く。
    noise: u16,
}

impl Default for RomLines {
    fn default() -> Self {
        Self::new(15)
    }
}

impl RomLines {
    /// 番地のピンが `addr_bits` 本の ROM の、手を加えていないつなぎ方。
    pub fn new(addr_bits: u8) -> Self {
        Self {
            addr: (0..addr_bits).map(Wire::Line).collect(),
            data: (0..8).map(Wire::Line).collect(),
            noise: 0xACE1,
        }
    }

    /// 手を加えていないか。
    pub fn is_normal(&self) -> bool {
        let same = |v: &[Wire]| v.iter().enumerate().all(|(k, w)| *w == Wire::Line(k as u8));
        same(&self.addr) && same(&self.data)
    }

    fn noise_bit(&mut self) -> usize {
        let b = self.noise & 1;
        self.noise >>= 1;
        if b != 0 {
            self.noise ^= 0xB400;
        }
        b as usize
    }

    fn level(&mut self, w: Wire, bits: usize) -> usize {
        match w {
            Wire::Line(n) => (bits >> n) & 1,
            Wire::Low => 0,
            Wire::High => 1,
            Wire::Float => self.noise_bit(),
        }
    }

    /// 基板の側の番地 `addr` で `rom` を読む。
    pub fn read(&mut self, rom: &[u8], addr: usize) -> u8 {
        let mask = rom.len() - 1;
        if self.is_normal() {
            return rom[addr & mask];
        }
        let mut a = 0;
        for k in 0..self.addr.len() {
            a |= self.level(self.addr[k], addr) << k;
        }
        let d = rom[a & mask] as usize;
        let mut out = 0;
        for k in 0..self.data.len() {
            out |= self.level(self.data[k], d) << k;
        }
        out as u8
    }

    /// `A3=0,A5=1,A2=A4,A4=A2,D7=Z` の形を読む。左がピン、右が `0`（GND）・`1`（+5 V）・
    /// `Z`（浮かせ）か、同じ側の線の名前。`normal` は手を加えていない形に戻す。
    pub fn parse(&mut self, spec: &str) -> Result<(), String> {
        for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if item.eq_ignore_ascii_case("normal") {
                *self = Self {
                    noise: self.noise,
                    ..Self::new(self.addr.len() as u8)
                };
                continue;
            }
            let (pin, src) = item
                .split_once('=')
                .ok_or_else(|| format!("「ピン=つなぎ先」の形でない: {item}"))?;
            let (kind, k) = Self::pin(pin)?;
            let n = if kind == 'A' {
                self.addr.len()
            } else {
                self.data.len()
            };
            if k >= n {
                return Err(format!("ピンがない: {pin}"));
            }
            let w = match src.trim().to_ascii_uppercase().as_str() {
                "0" => Wire::Low,
                "1" => Wire::High,
                "Z" => Wire::Float,
                s => {
                    let (k2, m) = Self::pin(s)?;
                    if k2 != kind || m >= n {
                        return Err(format!("つなげない線: {item}"));
                    }
                    Wire::Line(m as u8)
                }
            };
            if kind == 'A' {
                self.addr[k] = w;
            } else {
                self.data[k] = w;
            }
        }
        Ok(())
    }

    fn pin(s: &str) -> Result<(char, usize), String> {
        let s = s.trim().to_ascii_uppercase();
        let mut c = s.chars();
        let kind = c.next().filter(|k| *k == 'A' || *k == 'D');
        let num = c.as_str().parse::<usize>().ok();
        match (kind, num) {
            (Some(k), Some(n)) => Ok((k, n)),
            _ => Err(format!("ピンの名前でない（A0〜A14・D0〜D7）: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom() -> Vec<u8> {
        (0..0x8000u32).map(|i| (i ^ (i >> 8)) as u8).collect()
    }

    #[test]
    fn untouched_lines_read_the_rom_as_is() {
        let r = rom();
        let mut l = RomLines::default();
        assert!(l.is_normal());
        for a in [0, 1, 0x1234, 0x7FFF] {
            assert_eq!(l.read(&r, a), r[a]);
        }
    }

    #[test]
    fn tied_and_swapped_lines_move_the_address_and_the_bits() {
        let r = rom();
        let mut l = RomLines::default();
        l.parse("A0=1, A3=0, A1=A2, A2=A1, D7=0, D0=D1, D1=D0")
            .unwrap();
        assert!(!l.is_normal());
        let a = 0b1010_1010_1010;
        // A0 を 1、A3 を 0 に、A1 と A2 を入れ替える
        let moved = (a | 1) & !0b1000;
        let moved = (moved & !0b110) | ((a >> 1) & 0b10) | ((a << 1) & 0b100);
        let d = r[moved];
        let want = (d & 0x7C) | ((d >> 1) & 1) | ((d << 1) & 2);
        assert_eq!(l.read(&r, a), want);
        l.parse("normal").unwrap();
        assert!(l.is_normal());
    }

    #[test]
    fn floating_lines_are_noise_that_resumes_after_a_snapshot() {
        let r = vec![0u8; 0x8000];
        let mut l = RomLines::default();
        l.parse("D0=Z").unwrap();
        let mut copy = l.clone();
        let a: Vec<u8> = (0..64).map(|_| l.read(&r, 0)).collect();
        let b: Vec<u8> = (0..64).map(|_| copy.read(&r, 0)).collect();
        assert_eq!(a, b);
        assert!(a.contains(&0) && a.contains(&1));
        assert!(a.iter().all(|v| *v <= 1));
    }

    #[test]
    fn commands_set_the_bends() {
        let mut b = Bends::default();
        assert_eq!(b.mode, Mode::Normal);
        assert!(b.is_factory());
        b.command("clock doc 4e6").unwrap();
        b.command("CLOCK dsp 25000000").unwrap();
        b.command("fifo adpcm empty=0 full=1").unwrap();
        b.command("mode toggle").unwrap();
        assert_eq!(b.mode, Mode::Bend);
        assert_eq!(b.clocks.doc, Some(2 * PERIOD_8M));
        // 25 MHz は時刻の単位で 604.8。四捨五入する
        assert_eq!(b.clocks.dsp, Some(605));
        assert_eq!(
            b.fifos[3],
            FifoPins {
                empty: Pin::Low,
                full: Pin::High
            }
        );
        assert!(!b.is_factory());
        b.command("clock doc off").unwrap();
        b.command("fifo adpcm open").unwrap();
        b.command("clock dsp off").unwrap();
        assert!(b.is_factory());
        b.command("SIG one").unwrap();
        assert_eq!(b.sig, SigWiring::One);
        assert!(!b.is_factory());
        assert!(b.summary().contains("SIG"));
        b.command("sig both").unwrap();
        assert!(b.is_factory());
        b.command("dram temp 70").unwrap();
        b.command("DRAM unit 3").unwrap();
        assert_eq!(
            b.dram,
            DramEnv {
                unit: 3,
                temp_c: 70
            }
        );
        assert!(!b.is_factory());
        assert!(b.summary().contains("70 °C"));
        b.command("factory").unwrap();
        assert!(b.is_factory());
        assert_eq!(b.mode, Mode::Bend);
        // 基板の発振器の周期（DR-003）
        assert_eq!(
            Clocks::default().periods(),
            [PERIOD_8M, PERIOD_20M, PERIOD_384K, PERIOD_8M]
        );
        for bad in [
            "mode on",
            "clock doc",
            "clock vdp 1e6",
            "clock doc -5",
            "fifo port9 open",
            "fifo dac empty=2",
            "sig half",
            "dram temp 200",
            "dram unit x",
            "dram hot 1",
            "wobble",
        ] {
            assert!(b.command(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn bad_specs_are_rejected() {
        let mut l = RomLines::default();
        for s in ["A15=0", "D8=1", "A1=D1", "X1=0", "A1", "A1=Q"] {
            assert!(l.parse(s).is_err(), "{s}");
        }
    }
}
