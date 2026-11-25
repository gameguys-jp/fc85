//! HD68000 のコアを SingleStepTests（680x0 の `68000/v1`、台帳 `docs/parts/HD68000.md` の SST）と
//! 突き合わせる。
//!
//! 試験のデータはリポジトリに入れない（ライセンスの記載がない）。環境変数 `FC85_M68K_TESTS` に
//! `*.json.gz` のあるフォルダを指定したときだけ走り、指定がなければ何もせずに合格する。
//!
//! ```text
//! FC85_M68K_TESTS=<dir> cargo test --release -p fc85-chips --test hd68000_sst -- --nocapture
//! ```
//!
//! - 照合するのは、命令の後のレジスタ（D0〜D7、A0〜A6、USP、SSP、SR、PC）、プリフェッチ（IR と
//!   IRC）、RAM、クロック数（`length`）。全体の 99% 以上で合格とする。
//! - バスサイクルの列（`transactions`）は参考に数える（合否に入れない）。TAS の読み書き一体の
//!   サイクルは表し方が違うので数えない。
//! - `FC85_M68K_FILTER` でファイル名の頭を絞る。`FC85_M68K_VERBOSE` に数を入れると、ファイルごとに
//!   その数まで不一致の中身を出す。

use fc85_chips::hd68000::{Bus, BusReply, Hd68000, IackReply, RunState, Size};
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Case {
    name: String,
    initial: State,
    #[serde(rename = "final")]
    fin: State,
    length: u64,
    transactions: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct State {
    d0: u32,
    d1: u32,
    d2: u32,
    d3: u32,
    d4: u32,
    d5: u32,
    d6: u32,
    d7: u32,
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    usp: u32,
    ssp: u32,
    sr: u16,
    pc: u32,
    prefetch: [u16; 2],
    ram: Vec<[u32; 2]>,
}

impl State {
    fn d(&self) -> [u32; 8] {
        [
            self.d0, self.d1, self.d2, self.d3, self.d4, self.d5, self.d6, self.d7,
        ]
    }
    fn a(&self) -> [u32; 7] {
        [
            self.a0, self.a1, self.a2, self.a3, self.a4, self.a5, self.a6,
        ]
    }
}

/// バスサイクルの記録（S0 の時刻、種類、FC、番地、ワードか、値）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tok {
    Idle(u64),
    Cycle(char, u8, u32, bool, u32),
}

/// 平らなメモリ（24 bit の番地）。待ちはない（SST の `length` は DTACK がすぐ返る前提）。
struct TestBus {
    mem: HashMap<u32, u8>,
    log: Vec<(u64, Tok)>,
}

impl TestBus {
    fn byte(&self, addr: u32) -> u8 {
        *self.mem.get(&(addr & 0xff_ffff)).unwrap_or(&0)
    }
}

impl Bus for TestBus {
    fn read(&mut self, at: u64, addr: u32, fc: u8, size: Size) -> BusReply {
        let (word, v) = match size {
            Size::Word => (
                true,
                u32::from(self.byte(addr)) << 8 | u32::from(self.byte(addr + 1)),
            ),
            Size::Byte => (false, u32::from(self.byte(addr))),
        };
        self.log.push((at, Tok::Cycle('r', fc, addr, word, v)));
        BusReply::Ok {
            data: v as u16,
            waits: 0,
        }
    }

    fn write(&mut self, at: u64, addr: u32, fc: u8, size: Size, value: u16) -> BusReply {
        let word = size == Size::Word;
        if word {
            self.mem.insert(addr, (value >> 8) as u8);
            self.mem.insert((addr + 1) & 0xff_ffff, value as u8);
        } else {
            self.mem.insert(addr, value as u8);
        }
        let v = if word {
            u32::from(value)
        } else {
            u32::from(value & 0xff)
        };
        self.log.push((at, Tok::Cycle('w', fc, addr, word, v)));
        BusReply::Ok { data: 0, waits: 0 }
    }

    fn iack(&mut self, _at: u64, _level: u8) -> IackReply {
        IackReply::Spurious { clocks: 4 }
    }

    fn ipl(&mut self, _at: u64) -> u8 {
        0
    }
}

/// SST の `transactions` を記録の列にする（続く空きはまとめる）。TAS の `t` があれば `None`。
fn sst_tokens(tx: &[serde_json::Value]) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    for t in tx {
        let a = t.as_array()?;
        let kind = a[0].as_str()?;
        match kind {
            "n" => push_idle(&mut out, a[1].as_u64()?),
            "r" | "w" => out.push(Tok::Cycle(
                kind.chars().next()?,
                a[2].as_u64()? as u8,
                a[3].as_u64()? as u32,
                a[4].as_str()? == ".w",
                a[5].as_u64()? as u32,
            )),
            _ => return None,
        }
    }
    Some(out)
}

fn push_idle(out: &mut Vec<Tok>, n: u64) {
    if n == 0 {
        return;
    }
    if let Some(Tok::Idle(m)) = out.last_mut() {
        *m += n;
    } else {
        out.push(Tok::Idle(n));
    }
}

/// コアのバスサイクルの記録を SST と同じ形にする。
fn my_tokens(log: &[(u64, Tok)], length: u64) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut end = 0;
    for &(at, tok) in log {
        push_idle(&mut out, at.saturating_sub(end));
        out.push(tok);
        end = at + 4;
    }
    push_idle(&mut out, length.saturating_sub(end));
    out
}

struct FileResult {
    name: String,
    total: usize,
    pass: usize,
    tx_total: usize,
    tx_pass: usize,
    report: Vec<String>,
}

fn run_case(c: &Case, verbose: bool) -> (bool, Option<bool>, Option<String>) {
    let mut cpu = Hd68000::new();
    cpu.d = c.initial.d();
    cpu.a = c.initial.a();
    cpu.usp = c.initial.usp;
    cpu.ssp = c.initial.ssp;
    cpu.sr = c.initial.sr;
    cpu.pc = c.initial.pc;
    cpu.prefetch = c.initial.prefetch;
    cpu.state = RunState::Running;
    cpu.clock = 0;
    let mut bus = TestBus {
        mem: c.initial.ram.iter().map(|&[a, v]| (a, v as u8)).collect(),
        log: Vec::new(),
    };
    let clocks = u64::from(cpu.step(&mut bus));

    let f = &c.fin;
    let mut diffs = Vec::new();
    for i in 0..8 {
        if cpu.d[i] != f.d()[i] {
            diffs.push(format!("d{i} {:08x} (期待 {:08x})", cpu.d[i], f.d()[i]));
        }
    }
    for i in 0..7 {
        if cpu.a[i] != f.a()[i] {
            diffs.push(format!("a{i} {:08x} (期待 {:08x})", cpu.a[i], f.a()[i]));
        }
    }
    let regs = [
        ("usp", cpu.usp, f.usp),
        ("ssp", cpu.ssp, f.ssp),
        ("sr", u32::from(cpu.sr), u32::from(f.sr)),
        ("pc", cpu.pc, f.pc),
        ("ir", u32::from(cpu.prefetch[0]), u32::from(f.prefetch[0])),
        ("irc", u32::from(cpu.prefetch[1]), u32::from(f.prefetch[1])),
    ];
    for (n, got, exp) in regs {
        if got != exp {
            diffs.push(format!("{n} {got:08x} (期待 {exp:08x})"));
        }
    }
    for &[a, v] in &f.ram {
        let got = bus.byte(a);
        if u32::from(got) != v {
            diffs.push(format!("ram[{a:06x}] {got:02x} (期待 {v:02x})"));
        }
    }
    if clocks != c.length {
        diffs.push(format!("length {clocks} (期待 {})", c.length));
    }
    let ok = diffs.is_empty();
    let tx_ok = sst_tokens(&c.transactions).map(|exp| exp == my_tokens(&bus.log, clocks));
    let report = (verbose && (!ok || tx_ok == Some(false))).then(|| {
        let mine = my_tokens(&bus.log, clocks);
        let exp = sst_tokens(&c.transactions).unwrap_or_default();
        format!(
            "  {}: {}\n    コア {}\n    SST  {}",
            c.name,
            if ok {
                "状態は一致、バスサイクルの列が違う".to_string()
            } else {
                diffs.join(", ")
            },
            fmt_tokens(&mine),
            fmt_tokens(&exp)
        )
    });
    (ok, tx_ok, report)
}

fn fmt_tokens(t: &[Tok]) -> String {
    t.iter()
        .map(|t| match *t {
            Tok::Idle(n) => format!("n{n}"),
            Tok::Cycle(k, fc, a, w, v) => {
                format!("{k}{fc}:{a:06x}{}={v:x}", if w { "" } else { "b" })
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_file(path: &Path, verbose: usize) -> FileResult {
    let name = path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .trim_end_matches(".json.gz")
        .to_string();
    let file = std::fs::File::open(path).expect("試験のファイルを開けない");
    let cases: Vec<Case> = serde_json::from_reader(std::io::BufReader::new(GzDecoder::new(
        std::io::BufReader::new(file),
    )))
    .expect("試験のファイルを読めない");
    let mut r = FileResult {
        name,
        total: cases.len(),
        pass: 0,
        tx_total: 0,
        tx_pass: 0,
        report: Vec::new(),
    };
    for c in &cases {
        let (ok, tx, rep) = run_case(c, r.report.len() < verbose);
        if ok {
            r.pass += 1;
        }
        if let Some(t) = tx {
            r.tx_total += 1;
            if t {
                r.tx_pass += 1;
            }
        }
        if let Some(s) = rep {
            r.report.push(s);
        }
    }
    r
}

#[test]
fn single_step_tests() {
    let Ok(dir) = std::env::var("FC85_M68K_TESTS") else {
        return;
    };
    let filter = std::env::var("FC85_M68K_FILTER").unwrap_or_default();
    let verbose: usize = std::env::var("FC85_M68K_VERBOSE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("FC85_M68K_TESTS のフォルダを読めない")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().to_string();
            n.ends_with(".json.gz") && n.starts_with(filter.as_str())
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "試験のファイルがない");

    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(p) = files.get(i) else { break };
                    let r = run_file(p, verbose);
                    results.lock().unwrap().push((i, r));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(i, _)| *i);

    let (mut total, mut pass, mut tx_total, mut tx_pass) = (0, 0, 0, 0);
    for (_, r) in &results {
        println!(
            "{:<12} {:>5}/{:<5} {:>7.3}%   バスサイクルの列 {:>5}/{:<5}",
            r.name,
            r.pass,
            r.total,
            100.0 * r.pass as f64 / r.total as f64,
            r.tx_pass,
            r.tx_total
        );
        for s in &r.report {
            println!("{s}");
        }
        total += r.total;
        pass += r.pass;
        tx_total += r.tx_total;
        tx_pass += r.tx_pass;
    }
    let rate = 100.0 * pass as f64 / total as f64;
    println!(
        "全体 {pass}/{total} {rate:.3}%   バスサイクルの列 {tx_pass}/{tx_total} {:.3}%",
        100.0 * tx_pass as f64 / tx_total.max(1) as f64
    );
    assert!(rate >= 99.0, "一致が 99% に届かない（{rate:.3}%）");
}
