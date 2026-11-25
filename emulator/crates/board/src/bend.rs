//! ROM のソケットの線のベンディング（仕様書 §11 のベンド点「文字ROMの線」、DR-063）。
//!
//! 文字 ROM（32 KiB）のソケットとテストピンで、番地のピン A0〜A14 とデータのピン D0〜D7 を、
//! GND・+5 V への固定、ほかの線との入替え、浮かせにできる。68000 から読んでも Data Fabric の
//! port 9 から読んでも、同じソケットの線を通る。番号は ROM のピンの番号で、68000 の番地では
//! ROM の A0 が 68000 の A1 に当たる（仕様書 §08）。

use serde::{Deserialize, Serialize};

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
    fn bad_specs_are_rejected() {
        let mut l = RomLines::default();
        for s in ["A15=0", "D8=1", "A1=D1", "X1=0", "A1", "A1=Q"] {
            assert!(l.parse(s).is_err(), "{s}");
        }
    }
}
