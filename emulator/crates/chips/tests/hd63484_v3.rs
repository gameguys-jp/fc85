//! HD63484 のコアを、独立した実装（MAME の hd63484）と照らし合わせる（V3、docs/impl/v3-mame.md §06）。
//!
//! 入力の列は `tools/v3/hd63484_vectors.py` で作り、照合の相手の結果と一緒に、リポジトリの外に置く。
//! 環境変数 `FC85_V3_DIR` に、`hd63484/in.txt` と `hd63484/out.txt` のあるフォルダを渡す。なければ飛ばす。
//!
//! FC85_V3_DIR=<v3> cargo test --release -p fc85-chips --test hd63484_v3 -- --nocapture
//!
//! MAME は時間を持たないので、コマンドを 1 つ書くごとに、コアをそのコマンドが終わるまで進めて、SR、CP、RWP、
//! 読出し FIFO から読んだ語、値が変わった VRAM の語を比べる。既知の差（v3-mame.md §06）は分類を付けて数え、
//! VRAM と CP を相手にそろえて続ける。既知の差でない食い違いは、その場合をそこで打ち切って数える。

use fc85_chips::hd63484::{Hd63484, SR_CER, SR_RFR, SR_WFE, SR_WFR, Vram};
use std::collections::BTreeMap;

const WORDS: usize = 1 << 20;
/// 1 つのコマンドを進める 2CLK の上限。
const LIMIT: u32 = 20_000_000;

/// 照合の両側に同じ式を持つ VRAM の初めの中身（`<v3>/hd63484/harness.cpp` の `vram_init`）。
fn vram_init(seed: u32, a: u32) -> u16 {
    let mut x = a.wrapping_mul(0x9E37_79B1).wrapping_add(seed);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    if x >> 29 == 0 { 0 } else { x as u16 }
}

/// 1 M 語の VRAM。コマンドの間に書いた番地と、その前の値を覚える。
struct Mem {
    m: Vec<u16>,
    mark: Vec<bool>,
    touched: Vec<(u32, u16)>,
}

impl Vram for Mem {
    fn read(&mut self, addr: u32) -> u16 {
        self.m[addr as usize & (WORDS - 1)]
    }
    fn write(&mut self, addr: u32, data: u16) {
        let a = addr as usize & (WORDS - 1);
        if !self.mark[a] {
            self.mark[a] = true;
            self.touched.push((a as u32, self.m[a]));
        }
        self.m[a] = data;
    }
}

impl Mem {
    /// 値が変わった語（番地の順）。
    fn changed(&mut self) -> Vec<(u32, u16)> {
        self.touched.sort_unstable();
        self.touched
            .iter()
            .filter(|&&(a, b)| self.m[a as usize] != b)
            .map(|&(a, _)| (a, self.m[a as usize]))
            .collect()
    }
    /// 覚えた番地を忘れる。
    fn forget(&mut self) {
        for &(a, _) in &self.touched {
            self.mark[a as usize] = false;
        }
        self.touched.clear();
    }
    /// このコマンドの書込みを取り消し、相手の書込みにそろえる。
    fn adopt(&mut self, theirs: &[(u32, u16)]) {
        for &(a, b) in &self.touched {
            self.m[a as usize] = b;
        }
        for &(a, v) in theirs {
            self.m[a as usize] = v;
        }
        self.forget();
    }
}

/// コマンドの後に比べるもの（相手の出力の 1 行と同じ形）。
#[derive(Clone, Debug, Default, PartialEq)]
struct Res {
    sr: u8,
    cp: (u16, u16),
    rwp: (u8, u32),
    fifo: Vec<u16>,
    vram: Vec<(u32, u16)>,
}

impl Res {
    /// `K i sr cpx cpy dn rwp F 語 … V 番地=値 …`
    fn parse(line: &str) -> Res {
        let w: Vec<&str> = line.split(' ').collect();
        let hx = |s: &str| u32::from_str_radix(s, 16).unwrap();
        assert_eq!(w[7], "F", "{line}");
        let v = 8 + w[8..].iter().position(|&s| s == "V").unwrap();
        Res {
            sr: hx(w[2]) as u8,
            cp: (hx(w[3]) as u16, hx(w[4]) as u16),
            rwp: (hx(w[5]) as u8, hx(w[6])),
            fifo: w[8..v].iter().map(|&s| hx(s) as u16).collect(),
            vram: w[v + 1..]
                .iter()
                .map(|s| {
                    let (a, d) = s.split_once('=').unwrap();
                    (hx(a), hx(d) as u16)
                })
                .collect(),
        }
    }

    /// 食い違う項目。
    fn diff(&self, o: &Res) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.sr != o.sr {
            v.push("SR");
        }
        if self.cp != o.cp {
            v.push("CP");
        }
        if self.rwp != o.rwp {
            v.push("RWP");
        }
        if self.fifo != o.fifo {
            v.push("FIFO");
        }
        if self.vram != o.vram {
            v.push("VRAM");
        }
        v
    }

    fn show(&self) -> String {
        let mut s = format!(
            "SR {:02X} CP ({}, {}) RWP {}:{:05X} FIFO {:04X?} VRAM {} 語",
            self.sr,
            self.cp.0 as i16,
            self.cp.1 as i16,
            self.rwp.0,
            self.rwp.1,
            self.fifo,
            self.vram.len()
        );
        for (a, d) in self.vram.iter().take(6) {
            s += &format!(" {a:05X}={d:04X}");
        }
        s
    }
}

struct Rig {
    a: Hd63484,
    mem: Mem,
}

impl Rig {
    fn reg(&mut self, ar: u16, v: u16) {
        self.a.write(false, ar);
        self.a.write(true, v);
    }

    /// 書込み FIFO に書く。満杯なら空くまで進める。
    fn put(&mut self, words: &[u16]) {
        self.a.write(false, 0);
        for &w in words {
            let mut n = 0;
            while self.a.status() & SR_WFR == 0 && n < LIMIT {
                self.a.tick(&mut self.mem);
                n += 1;
            }
            self.a.write(true, w);
        }
    }

    /// コマンドを書き、終わるまで（未定義のコマンドは CER が立つまで）進める。終わらなければ false。
    fn run(&mut self, words: &[u16]) -> bool {
        self.put(words);
        for _ in 0..LIMIT {
            if self.a.idle() || self.a.status() & SR_CER != 0 {
                return true;
            }
            self.a.tick(&mut self.mem);
        }
        false
    }

    /// 途中までのコマンド：書込み FIFO が空になり、書込みが VRAM に入るまで進め、少し待つ。
    fn run_partial(&mut self, words: &[u16]) {
        self.put(words);
        let mut n = 0;
        while (self.a.status() & SR_WFE == 0 || self.a.pending_writes() > 0) && n < LIMIT {
            self.a.tick(&mut self.mem);
            n += 1;
        }
        for _ in 0..200 {
            self.a.tick(&mut self.mem);
        }
    }

    /// SR を読み、読出し FIFO を空にして、相手と同じ形の結果を作る。
    fn result(&mut self) -> Res {
        let sr = self.a.read(false) as u8;
        self.a.write(false, 0);
        let mut fifo = Vec::new();
        while self.a.status() & SR_RFR != 0 && fifo.len() < 16 {
            fifo.push(self.a.read(true));
        }
        let (x, y) = self.a.cp();
        Res {
            sr,
            cp: (x as u16, y as u16),
            rwp: self.a.rwp(),
            fifo,
            vram: self.mem.changed(),
        }
    }
}

/// コマンドの名前（報告用）。
fn name(op: u16) -> &'static str {
    match op >> 12 {
        0x6 => "CPY",
        0x7 => "SCPY",
        0xD => "PTN",
        0xE => "AGCPY",
        0xF => "RGCPY",
        _ => match op >> 10 {
            0x01 => "ORG",
            0x02 => "WPR",
            0x03 => "RPR",
            0x06 => "WPTN",
            0x0A => "DWT",
            0x0B => "DMOD",
            0x11 => "RD",
            0x12 => "WT",
            0x13 => "MOD",
            0x16 => "CLR",
            0x17 => "SCLR",
            0x20 => "AMOVE",
            0x21 => "RMOVE",
            0x22 => "ALINE",
            0x23 => "RLINE",
            0x24 => "ARCT",
            0x25 => "RRCT",
            0x26 => "APLL",
            0x27 => "RPLL",
            0x28 => "APLG",
            0x29 => "RPLG",
            0x30 => "AFRCT",
            0x31 => "RFRCT",
            0x33 => "DOT",
            _ => "未定義",
        },
    }
}

/// 直線の画素の選び方で、誤差がちょうど半分になる点があるか（`tools/v3/hd63484_vectors.py` の `tie`）。
/// コアは主の向きにだけ進み、MAME は副の向きにも進むので、その点から先の画素が違う（既知の差）。
fn tie(dx: i32, dy: i32) -> bool {
    let (a, b) = (dx.abs(), dy.abs());
    if a == b || a == 0 || b == 0 {
        return false;
    }
    let (major, minor) = (a.max(b), a.min(b));
    (1..major).any(|j| (2 * minor * j - major).rem_euclid(2 * major) == 0)
}

/// 直線・多角形のコマンドの線分（差分）。
fn segments(op: u16, p: &[u16], cp: (i16, i16)) -> Vec<(i32, i32)> {
    let rel = op & 0x0400 != 0;
    let at = |cur: (i16, i16), x: u16, y: u16| {
        if rel {
            (cur.0.wrapping_add(x as i16), cur.1.wrapping_add(y as i16))
        } else {
            (x as i16, y as i16)
        }
    };
    let d = |a: (i16, i16), b: (i16, i16)| {
        (
            i32::from(b.0) - i32::from(a.0),
            i32::from(b.1) - i32::from(a.1),
        )
    };
    match op >> 10 {
        0x22 | 0x23 => vec![d(cp, at(cp, p[0], p[1]))],
        0x26..=0x29 => {
            let mut cur = cp;
            let mut v = Vec::new();
            for i in 0..usize::from(p[0]) {
                let next = at(cur, p[1 + 2 * i], p[2 + 2 * i]);
                v.push(d(cur, next));
                cur = next;
            }
            if op >> 10 >= 0x28 {
                v.push(d(cur, cp));
            }
            v
        }
        _ => Vec::new(),
    }
}

/// コマンドの前の状態のうち、既知の差を見分けるのに使うもの。
struct Before {
    cp: (i16, i16),
    /// パターン RAM（場合ごとに全部 0 か全部 1）
    pat: u16,
}

/// 既知の差（v3-mame.md §06）の分類。`bad` は食い違った項目。
fn known(words: &[u16], b: &Before, bad: &[&str]) -> Option<&'static str> {
    let op = words[0];
    let p = &words[1..];
    let top6 = op >> 10;
    let draw = matches!(op >> 12, 0xD..=0xF) || matches!(top6, 0x22..=0x29 | 0x30 | 0x31 | 0x33);
    let gcpy = matches!(op >> 12, 0xE | 0xF);
    match bad {
        ["SR"] if name(op) == "未定義" => Some("(c) 未定義のコマンドの後の CED"),
        ["FIFO"] if top6 == 0x03 && op & 0x1F == 0x05 => Some("(b) Pr05 のパターンの位置"),
        ["CP"] if matches!(top6, 0x26 | 0x27) && p[0] == 0 => Some("(c) APLL・RPLL の n＝0 の CP"),
        ["VRAM"] => {
            if segments(op, p, b.cp).iter().any(|&(dx, dy)| tie(dx, dy)) {
                return Some("(c) 直線の同点の画素");
            }
            let opm = op & 7;
            if draw && (opm == 6 || opm == 7) {
                // コアは描こうとしている色と比べ、MAME は 110 で CL0、111 で CL1 と比べる
                let drawn = match ((op >> 3) & 3, b.pat) {
                    (0 | 2, 0) => Some(0),
                    (0 | 1, 1) => Some(1),
                    _ => None,
                };
                if gcpy || drawn == Some(if opm == 6 { 1 } else { 0 }) {
                    return Some("(c) OPM＝110・111 の CL");
                }
            }
            None
        }
        _ => None,
    }
}

enum Item {
    Reg(u16, u16),
    Cmd(char, Vec<u16>),
}

#[test]
fn hd63484_matches_mame() {
    let Ok(dir) = std::env::var("FC85_V3_DIR") else {
        eprintln!("FC85_V3_DIR がないので飛ばす");
        return;
    };
    let dir = std::path::Path::new(&dir).join("hd63484");
    let (Ok(input), Ok(output)) = (
        std::fs::read_to_string(dir.join("in.txt")),
        std::fs::read_to_string(dir.join("out.txt")),
    ) else {
        eprintln!("{} に in.txt・out.txt がないので飛ばす", dir.display());
        return;
    };
    let verbose: usize = std::env::var("FC85_V3_VERBOSE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let mut expected: Vec<Vec<&str>> = Vec::new();
    for line in output.lines() {
        if line.starts_with("C ") {
            expected.push(Vec::new());
        } else if let Some(v) = expected.last_mut() {
            v.push(line);
        }
    }

    let hx = |s: &str| u32::from_str_radix(s, 16).unwrap();
    let mut cases: Vec<(u32, Vec<Item>)> = Vec::new();
    for line in input.lines() {
        let mut w = line.split(' ');
        let tag = w.next().unwrap_or("");
        match tag {
            "C" => {
                let _id = w.next();
                cases.push((hx(w.next().unwrap()), Vec::new()));
            }
            "R" => {
                let (ar, v) = (hx(w.next().unwrap()), hx(w.next().unwrap()));
                cases
                    .last_mut()
                    .unwrap()
                    .1
                    .push(Item::Reg(ar as u16, v as u16));
            }
            "K" | "H" => {
                let words = w.map(|s| hx(s) as u16).collect();
                let t = tag.chars().next().unwrap();
                cases.last_mut().unwrap().1.push(Item::Cmd(t, words));
            }
            _ => {}
        }
    }
    assert_eq!(cases.len(), expected.len(), "入力と相手の件数");

    let mut mem = Mem {
        m: vec![0; WORDS],
        mark: vec![false; WORDS],
        touched: Vec::new(),
    };
    let (mut good, mut total, mut matched) = (0usize, 0usize, 0usize);
    let mut by_cmd: BTreeMap<&str, usize> = BTreeMap::new();
    let mut known_n: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for (k, (seed, items)) in cases.iter().enumerate() {
        for (a, v) in mem.m.iter_mut().enumerate() {
            *v = vram_init(*seed, a as u32);
        }
        let mut rig = Rig {
            a: Hd63484::new(),
            mem,
        };
        let theirs = &expected[k];
        let mut i = 0;
        let mut clean = true;
        for item in items {
            let (t, words) = match item {
                Item::Reg(ar, v) => {
                    rig.reg(*ar, *v);
                    continue;
                }
                Item::Cmd(t, words) => (*t, words),
            };
            let before = Before {
                cp: rig.a.cp(),
                pat: rig.a.pattern(0) & 1,
            };
            let done = if t == 'H' {
                rig.run_partial(words);
                true
            } else {
                rig.run(words)
            };
            let ours = rig.result();
            let them = Res::parse(theirs[i]);
            i += 1;
            total += 1;
            let label = if t == 'H' {
                "途中で止めた"
            } else {
                name(words[0])
            };
            *by_cmd.entry(label).or_default() += 1;
            let bad = if done {
                ours.diff(&them)
            } else {
                vec!["終わらない"]
            };
            if bad.is_empty() {
                matched += 1;
                rig.mem.forget();
                continue;
            }
            let kn = if t == 'K' {
                known(words, &before, &bad)
            } else {
                None
            };
            let shown = match kn {
                Some(kn) => {
                    let n = known_n.entry(kn).or_default();
                    *n += 1;
                    (*n <= verbose).then(|| format!("既知 {kn}"))
                }
                None => {
                    let kind = format!("{label} の {}", bad.join("+"));
                    let n = kinds.entry(kind.clone()).or_default();
                    *n += 1;
                    clean = false;
                    (*n <= verbose).then_some(kind)
                }
            };
            if let Some(kind) = shown {
                eprintln!("場合 {k} のコマンド {}：{kind}", i - 1);
                eprintln!("  語   {words:04X?}");
                eprintln!("  コア {}", ours.show());
                eprintln!("  相手 {}", them.show());
            }
            if !clean {
                break;
            }
            // 既知の差：VRAM と CP を相手にそろえて続ける
            rig.mem.adopt(&them.vram);
            if ours.cp != them.cp {
                rig.run(&[0x8000, them.cp.0, them.cp.1]);
                rig.mem.forget();
            }
        }
        rig.mem.forget();
        mem = rig.mem;
        if clean {
            good += 1;
        }
    }
    eprintln!(
        "HD63484 の V3：{} 件のうち {good} 件が最後まで比べられた。コマンド {total} のうち {matched} が一致",
        cases.len()
    );
    let list: Vec<String> = by_cmd.iter().map(|(c, n)| format!("{c} {n}")).collect();
    eprintln!("  コマンド：{}", list.join("、"));
    for (kn, n) in &known_n {
        eprintln!("  既知 {kn}: {n}");
    }
    for (kind, n) in &kinds {
        eprintln!("  {kind}: {n}");
    }
    assert!(kinds.is_empty(), "MAME と食い違った");
}
