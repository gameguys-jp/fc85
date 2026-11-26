//! ES5503 のコアを、独立した実装（MAME の es5503 v3.1）と照らし合わせる（V3、docs/impl/v3-mame.md）。
//!
//! 入力の列は `tools/v3/es5503_vectors.py` で作り、照合の相手の結果と一緒に、リポジトリの外に置く。
//! 環境変数 `FC85_V3_DIR` に、`es5503/in.txt` と `es5503/out.txt` のあるフォルダを渡す。なければ飛ばす。
//!
//! FC85_V3_DIR=<v3> cargo test --release -p fc85-chips --test es5503_v3 -- --nocapture
//!
//! 1 スロット（DOC サイクル）ごとに、訪れたオシレータ、波形の番地と値、アキュムレータ、制御、波形データ、
//! 音量、SIG±（符号付きの波形値×音量）、IRQ を比べ、0xE0 を読んだ値を比べる。レジスタの崩れは止める。

use fc85_chips::es5503::{Es5503, Slot, WaveBus};
use std::collections::BTreeMap;
use std::fmt::Write as _;

struct Ram(Vec<u8>);

impl WaveBus for Ram {
    fn read_wave(&mut self, addr: u32) -> u8 {
        self.0[(addr & 0x1FFFF) as usize]
    }
}

/// スロットの後にするホストのアクセス：番地と、書く値（`None` は 0xE0 を読む）。
type Acts = BTreeMap<u32, Vec<(u8, Option<u8>)>>;

/// 照合の両側に同じ式を持つ波形 RAM（tools/v3/es5503_vectors.py の `wave_ram`）。
fn wave_ram(seed: u32, zero: u32) -> Vec<u8> {
    let mut x = if seed == 0 { 1 } else { seed };
    (0..0x20000)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let b = ((x >> 8) & 0xFF) as u8;
            if (x & 0xFF) < zero || b == 0 { 0 } else { b }
        })
        .collect()
}

/// 1 件を動かし、相手と同じ形の行を返す。
fn run_case(slots: u32, seed: u32, zero: u32, acts: &Acts) -> Vec<String> {
    let mut ram = Ram(wave_ram(seed, zero));
    let mut doc = Es5503::new();
    doc.set_decay(false);
    let mut out = Vec::new();
    let host = |doc: &mut Es5503, s: u32, out: &mut Vec<String>| {
        if let Some(list) = acts.get(&s) {
            for &(a, v) in list {
                match v {
                    None => out.push(format!("Q {s:X} {:02X}", doc.read(0xE0))),
                    Some(v) => doc.write(a, v),
                }
            }
        }
    };
    host(&mut doc, 0, &mut out);
    for s in 1..=slots {
        let tk = doc.tick(&mut ram);
        let mut line = String::new();
        match tk.slot {
            Slot::Osc { n, addr, wave } => {
                let i = usize::from(n);
                let data = doc.read(0x60 | n);
                write!(
                    line,
                    "V {s:X} {n:X} {addr:05X} {wave:02X} {:06X} {:02X} {data:02X} {:02X} {} {}",
                    doc.accumulator(i),
                    doc.control(i),
                    doc.volume(i),
                    tk.sig,
                    u8::from(doc.irq())
                )
                .unwrap();
            }
            Slot::Refresh => write!(line, "R {s:X}").unwrap(),
            Slot::Stopped => write!(line, "O {s:X}").unwrap(),
        }
        out.push(line);
        host(&mut doc, s, &mut out);
    }
    out
}

/// 食い違った行の、最初に違う項目の名前。
fn field(ours: &str, theirs: &str) -> String {
    const NAMES: [&str; 11] = [
        "種類",
        "スロット",
        "オシレータ",
        "番地",
        "波形",
        "ACC",
        "制御",
        "DATA",
        "音量",
        "SIG",
        "IRQ",
    ];
    let (a, b): (Vec<&str>, Vec<&str>) = (ours.split(' ').collect(), theirs.split(' ').collect());
    if a[0] != b[0] {
        return format!("種類 {}/{}", a[0], b[0]);
    }
    if a[0] == "Q" {
        return "0xE0 の値".to_string();
    }
    let bad: Vec<&str> = (0..a.len().min(b.len()))
        .filter(|&i| a[i] != b[i])
        .map(|i| NAMES.get(i).copied().unwrap_or("?"))
        .collect();
    bad.join("+")
}

#[test]
fn es5503_matches_mame() {
    let Ok(dir) = std::env::var("FC85_V3_DIR") else {
        eprintln!("FC85_V3_DIR がないので飛ばす");
        return;
    };
    let dir = std::path::Path::new(&dir).join("es5503");
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

    // 入力を 1 件ずつに分ける
    let mut cases: Vec<(u32, u32, u32, Acts)> = Vec::new();
    for line in input.lines() {
        let mut w = line.split(' ');
        let hx = |s: &str| u32::from_str_radix(s, 16).unwrap();
        match w.next() {
            Some("C") => {
                let _id = w.next();
                let (slots, seed, zero) = (
                    hx(w.next().unwrap()),
                    hx(w.next().unwrap()),
                    hx(w.next().unwrap()),
                );
                cases.push((slots, seed, zero, BTreeMap::new()));
            }
            Some("A") => {
                let s = hx(w.next().unwrap());
                let v: Vec<u32> = w.map(hx).collect();
                let acts = &mut cases.last_mut().unwrap().3;
                for p in v.chunks(2) {
                    acts.entry(s)
                        .or_default()
                        .push((p[0] as u8, Some(p[1] as u8)));
                }
            }
            Some("Q") => {
                let s = hx(w.next().unwrap());
                cases
                    .last_mut()
                    .unwrap()
                    .3
                    .entry(s)
                    .or_default()
                    .push((0xE0, None));
            }
            _ => {}
        }
    }
    assert_eq!(cases.len(), expected.len(), "入力と相手の件数");

    let (mut good, mut lines_total, mut lines_good) = (0, 0usize, 0usize);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for (k, (slots, seed, zero, acts)) in cases.iter().enumerate() {
        let ours = run_case(*slots, *seed, *zero, acts);
        let theirs = &expected[k];
        lines_total += theirs.len();
        match ours.iter().zip(theirs.iter()).position(|(a, b)| a != b) {
            None if ours.len() == theirs.len() => {
                good += 1;
                lines_good += theirs.len();
            }
            at => {
                let at = at.unwrap_or(ours.len().min(theirs.len()));
                lines_good += at;
                let kind = if at < ours.len() && at < theirs.len() {
                    field(&ours[at], theirs[at])
                } else {
                    "行の数".to_string()
                };
                let n = kinds.entry(kind.clone()).or_default();
                *n += 1;
                if *n <= verbose {
                    let from = at.saturating_sub(4);
                    eprintln!("場合 {k}：{kind}（行 {at}）");
                    for i in from..(at + 3).min(ours.len()).min(theirs.len()) {
                        let mark = if i == at { ">" } else { " " };
                        eprintln!("  {mark} コア {:<60} 相手 {}", ours[i], theirs[i]);
                    }
                }
            }
        }
    }
    eprintln!(
        "ES5503 の V3：{} 件のうち {good} 件が最後まで一致。行 {lines_total} のうち、最初の食い違いまで {lines_good}",
        cases.len()
    );
    for (k, n) in &kinds {
        eprintln!("  {k}: {n}");
    }
    assert!(kinds.is_empty(), "MAME と食い違った");
}
