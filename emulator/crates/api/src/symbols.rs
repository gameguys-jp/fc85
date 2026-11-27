//! ラベル（m68kasm のリストの末尾の表）。番地をラベルの名前で受け、記録や停止の番地にラベルを付ける。

use std::collections::BTreeMap;

/// ラベルを付ける番地の範囲（Boot ROM、Main RAM と Staging RAM、カートリッジ）。I/O の番地は EQU の
/// 定数と区別できないので、近いラベル＋差では表さない。
const CODE_RANGES: [(u32, u32); 3] = [
    (0x00_0000, 0x01_0000),
    (0x10_0000, 0x13_0000),
    (0x40_0000, 0x50_0000),
];

/// 近いラベル＋差で表す差の上限。
const NEAR: u32 = 0x1000;

#[derive(Clone, Debug, Default)]
pub struct Symbols {
    by_name: BTreeMap<String, u32>,
    /// 番地 → 名前（同じ番地の名前は最初のもの）。CODE_RANGES の中だけ。
    by_addr: BTreeMap<u32, String>,
}

fn region(addr: u32) -> Option<usize> {
    CODE_RANGES
        .iter()
        .position(|&(lo, hi)| (lo..hi).contains(&addr))
}

impl Symbols {
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// m68kasm のリストを読む。末尾の表の行（2 字下げ、名前、`$` と 8 桁の 16 進）だけを拾う。
    /// 先に読んだ名前を上書きする。読んだ数を返す。
    pub fn add_listing(&mut self, text: &str) -> usize {
        let mut n = 0;
        for line in text.lines() {
            if !line.starts_with("  ") {
                continue;
            }
            let mut it = line.split_whitespace();
            let (Some(name), Some(val), None) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            let Some(hex) = val.strip_prefix('$') else {
                continue;
            };
            if hex.len() != 8 {
                continue;
            }
            let Ok(v) = u32::from_str_radix(hex, 16) else {
                continue;
            };
            self.by_name.insert(name.to_string(), v);
            if region(v).is_some() {
                self.by_addr.entry(v).or_insert_with(|| name.to_string());
            }
            n += 1;
        }
        n
    }

    pub fn get(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    /// 番地の名前：同じ番地のラベル、なければ同じ範囲の手前のラベル＋差（`$1000` 未満）。
    pub fn label(&self, addr: u32) -> Option<String> {
        let r = region(addr)?;
        let (&at, name) = self.by_addr.range(..=addr).next_back()?;
        if region(at) != Some(r) || addr - at >= NEAR {
            return None;
        }
        Some(if at == addr {
            name.clone()
        } else {
            format!("{name}+${:X}", addr - at)
        })
    }

    /// 名前が `prefix` で始まるラベル（多ければ `limit` まで）。
    pub fn find(&self, prefix: &str, limit: usize) -> Vec<(String, u32)> {
        self.by_name
            .range(prefix.to_string()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .take(limit)
            .map(|(k, &v)| (k.clone(), v))
            .collect()
    }

    /// 番地の書き方：数、`$` か `0x` の 16 進、10 進、ラベル、ラベル±差（差は同じ書き方の数）。
    pub fn parse_addr(&self, s: &str) -> Result<u32, String> {
        let s = s.trim();
        if let Some(v) = parse_number(s) {
            return Ok(v);
        }
        let (name, off) = match s.find(['+', '-']) {
            Some(i) if i > 0 => {
                let d = parse_number(s[i + 1..].trim())
                    .ok_or_else(|| format!("番地の差を読めない：{s}"))?;
                let d = if s.as_bytes()[i] == b'-' {
                    d.wrapping_neg()
                } else {
                    d
                };
                (s[..i].trim(), d)
            }
            _ => (s, 0),
        };
        self.get(name)
            .map(|v| v.wrapping_add(off))
            .ok_or_else(|| format!("ラベル {name} がない（リストを読んだか確かめる）"))
    }
}

/// `$1F`、`0x1F`、`31` を読む。
pub fn parse_number(s: &str) -> Option<u32> {
    if let Some(h) = s.strip_prefix('$').or_else(|| s.strip_prefix("0x")) {
        u32::from_str_radix(h, 16).ok()
    } else if s.bytes().all(|b| b.is_ascii_digit()) && !s.is_empty() {
        s.parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "\
400000                      151  cart_header:
400020                      164  start:

  ADPCM_STATUS                     $00230002
  start                            $00400020
  start.clr                        $00400030
  update                           $004000E6
  vbl_count                        $00100010
";

    #[test]
    fn listing_labels_resolve_both_ways() {
        let mut s = Symbols::default();
        assert_eq!(s.add_listing(LISTING), 5);
        assert_eq!(s.get("start"), Some(0x40_0020));
        assert_eq!(s.label(0x40_0020).as_deref(), Some("start"));
        assert_eq!(s.label(0x40_0034).as_deref(), Some("start.clr+$4"));
        assert_eq!(s.label(0x10_0012).as_deref(), Some("vbl_count+$2"));
        // I/O の番地と、範囲の外・遠い番地には付けない
        assert_eq!(s.label(0x23_0002), None);
        assert_eq!(s.label(0x40_3000), None);
        assert_eq!(s.find("start", 10).len(), 2);
    }

    #[test]
    fn addresses_parse_from_numbers_and_labels() {
        let mut s = Symbols::default();
        s.add_listing(LISTING);
        assert_eq!(s.parse_addr("$400000"), Ok(0x40_0000));
        assert_eq!(s.parse_addr("0x10"), Ok(0x10));
        assert_eq!(s.parse_addr("42"), Ok(42));
        assert_eq!(s.parse_addr("start+4"), Ok(0x40_0024));
        assert_eq!(s.parse_addr("update - $6"), Ok(0x40_00E0));
        assert!(s.parse_addr("nowhere").is_err());
    }
}
