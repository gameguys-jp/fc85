//! TMS32010 のコアを、独立した実装（MAME の TMS320C1x）と照らし合わせる（V3、docs/impl/v3-mame.md）。
//!
//! 入力の列は `tools/v3/tms32010_vectors.py` で作り、照合の相手の結果と一緒に、リポジトリの外に置く
//! （MAME から作ったものはリポジトリに入れない）。環境変数 `FC85_V3_DIR` に、`tms32010/in.txt` と
//! `tms32010/out.txt` のあるフォルダを渡す。なければ飛ばす。
//!
//! FC85_V3_DIR=<v3> cargo test --release -p fc85-chips --test tms32010_v3 -- --nocapture
//!
//! 1 命令ごとに、PC・ACC・P・T・AR0・AR1・状態語・スタック・サイクル数と、その命令の OUT・TBLW・IN を比べ、
//! 終わりにデータ RAM を比べる。場合ごとに最初の食い違いで止め（その先は状態がずれる）、命令と項目で数える。
//! `FC85_V3_VERBOSE` に数を入れると、食い違いの例をその数だけ出す。

use fc85_chips::tms32010::{Bus, Cycle, Tms32010};
use std::collections::BTreeMap;
use std::fmt::Write as _;

struct TestBus {
    prog: Vec<u16>,
    ports: [u16; 8],
    bio: bool,
    events: String,
}

impl Bus for TestBus {
    fn read_program(&mut self, addr: u16) -> u16 {
        self.prog[usize::from(addr & 0xFFF)]
    }
    fn write_program(&mut self, addr: u16, data: u16) {
        self.prog[usize::from(addr & 0xFFF)] = data;
        writeln!(self.events, "W {:03X} {data:04X}", addr & 0xFFF).unwrap();
    }
    fn port_in(&mut self, port: u8) -> u16 {
        writeln!(self.events, "N {port:X}").unwrap();
        self.ports[usize::from(port & 7)]
    }
    fn port_out(&mut self, port: u8, data: u16) {
        writeln!(self.events, "O {port:X} {data:04X}").unwrap();
    }
    fn bio_low(&mut self) -> bool {
        self.bio
    }
}

fn hex_words(s: &str) -> Vec<u16> {
    s.split_whitespace()
        .map(|w| u16::from_str_radix(w, 16).unwrap())
        .collect()
}

/// 命令の名前（数える単位）。上位 byte で分ける。
fn mnemonic(op: u16) -> String {
    let h = (op >> 8) as u8;
    let name = match h {
        0x00..=0x0F => "ADD",
        0x10..=0x1F => "SUB",
        0x20..=0x2F => "LAC",
        0x30 | 0x31 => "SAR",
        0x38 | 0x39 => "LAR",
        0x40..=0x47 => "IN",
        0x48..=0x4F => "OUT",
        0x50 => "SACL",
        0x58..=0x5F => "SACH",
        0x60 => "ADDH",
        0x61 => "ADDS",
        0x62 => "SUBH",
        0x63 => "SUBS",
        0x64 => "SUBC",
        0x65 => "ZALH",
        0x66 => "ZALS",
        0x67 => "TBLR",
        0x68 => "LARP/MAR",
        0x69 => "DMOV",
        0x6A => "LT",
        0x6B => "LTD",
        0x6C => "LTA",
        0x6D => "MPY",
        0x6E => "LDPK",
        0x6F => "LDP",
        0x70 | 0x71 => "LARK",
        0x78 => "XOR",
        0x79 => "AND",
        0x7A => "OR",
        0x7B => "LST",
        0x7C => "SST",
        0x7D => "TBLW",
        0x7E => "LACK",
        0x7F => {
            return match op & 0xFF {
                0x80 => "NOP",
                0x81 => "DINT",
                0x82 => "EINT",
                0x88 => "ABS",
                0x89 => "ZAC",
                0x8A => "ROVM",
                0x8B => "SOVM",
                0x8C => "CALA",
                0x8D => "RET",
                0x8E => "PAC",
                0x8F => "APAC",
                0x90 => "SPAC",
                0x9C => "PUSH",
                0x9D => "POP",
                _ => "7F(未定義)",
            }
            .to_string();
        }
        0x80..=0x9F => "MPYK",
        0xF4 => "BANZ",
        0xF5 => "BV",
        0xF6 => "BIOZ",
        0xF8 => "CALL",
        0xF9 => "B",
        0xFA => "BLZ",
        0xFB => "BLEZ",
        0xFC => "BGZ",
        0xFD => "BGEZ",
        0xFE => "BNZ",
        0xFF => "BZ",
        _ => "未定義",
    };
    name.to_string()
}

const FIELDS: [&str; 12] = [
    "PC", "ACC", "P", "T", "AR0", "AR1", "ST", "STK0", "STK1", "STK2", "STK3", "CYC",
];

#[test]
fn tms32010_matches_mame() {
    let Ok(dir) = std::env::var("FC85_V3_DIR") else {
        eprintln!("FC85_V3_DIR がないので飛ばす");
        return;
    };
    let dir = std::path::Path::new(&dir).join("tms32010");
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

    // 相手の結果を場合ごとに分ける
    let mut expected: Vec<Vec<&str>> = Vec::new();
    for line in output.lines() {
        if line.starts_with("C ") {
            expected.push(Vec::new());
        } else if let Some(v) = expected.last_mut() {
            v.push(line);
        }
    }

    let mut lines = input.lines();
    let (mut cases, mut good_cases, mut steps, mut good_steps) = (0usize, 0usize, 0usize, 0usize);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut known_diffs: BTreeMap<&str, usize> = BTreeMap::new();
    let mut known_cases = 0usize;
    while let Some(head) = lines.next() {
        let Some(head) = head.strip_prefix("C ") else {
            continue;
        };
        let h: Vec<usize> = head
            .split_whitespace()
            .map(|w| w.parse().unwrap())
            .collect();
        let (n_steps, bio) = (h[1], h[2] == 1);
        let ram = hex_words(&lines.next().unwrap()[2..]);
        let ports = hex_words(&lines.next().unwrap()[2..]);
        let prog_words = hex_words(&lines.next().unwrap()[2..]);
        let exp = &expected[cases];
        cases += 1;

        let mut prog = vec![0u16; 4096];
        prog[..prog_words.len()].copy_from_slice(&prog_words);
        let mut bus = TestBus {
            prog,
            ports: ports.try_into().unwrap(),
            bio,
            events: String::new(),
        };
        let mut dsp = Tms32010::new();
        dsp.set_rs(false);
        assert_eq!(dsp.tick(&mut bus), Cycle::Restart);
        for (a, &v) in ram.iter().enumerate() {
            dsp.set_data(a as u8, v);
        }

        let mut e = exp.iter().peekable();
        let mut failed = None;
        let mut stopped = false;
        for _ in 0..n_steps {
            bus.events.clear();
            let ram_before: Vec<u16> = (0..144).map(|a| dsp.data(a as u8)).collect();
            let (st_before, ar_before) = (dsp.status(), [dsp.ar(0), dsp.ar(1)]);
            let before = format!(
                "PC {:03X} ACC {:08X} P {:08X} T {:04X} AR {:04X} {:04X} ST {:04X} STK {:03X?}",
                dsp.pc(),
                dsp.acc(),
                dsp.p(),
                dsp.t(),
                dsp.ar(0),
                dsp.ar(1),
                dsp.status(),
                dsp.stack()
            );
            let op = match dsp.tick(&mut bus) {
                Cycle::Exec { opcode, .. } => opcode,
                other => panic!("命令の始まりでない {other:?}"),
            };
            let mut cyc = 1;
            while dsp.mid_instruction() {
                dsp.tick(&mut bus);
                cyc += 1;
            }
            // その命令で変わったデータ RAM（D 番地 値）
            for (a, &old) in ram_before.iter().enumerate() {
                let v = dsp.data(a as u8);
                if v != old {
                    writeln!(bus.events, "D {a:02X} {v:04X}").unwrap();
                }
            }
            let s = dsp.stack();
            let ours = [
                u32::from(dsp.pc()),
                dsp.acc(),
                dsp.p(),
                u32::from(dsp.t()),
                u32::from(dsp.ar(0)),
                u32::from(dsp.ar(1)),
                u32::from(dsp.status()),
                u32::from(s[0]),
                u32::from(s[1]),
                u32::from(s[2]),
                u32::from(s[3]),
                cyc,
            ];
            let line = e.next().expect("相手の行が足りない");
            let theirs: Vec<u32> = line[2..]
                .split_whitespace()
                .enumerate()
                .map(|(i, w)| {
                    if i == 11 {
                        w.parse().unwrap()
                    } else {
                        u32::from_str_radix(w, 16).unwrap()
                    }
                })
                .collect();
            let mut their_events = String::new();
            while let Some(l) = e.peek() {
                if l.starts_with('S') || l.starts_with('M') {
                    break;
                }
                their_events.push_str(l);
                their_events.push('\n');
                e.next();
            }
            steps += 1;
            let bad: Vec<&str> = (0..12)
                .filter(|&i| ours[i] != theirs[i])
                .map(|i| FIELDS[i])
                .collect();
            let bad_events = bus.events != their_events;
            if bad.is_empty() && !bad_events {
                good_steps += 1;
                continue;
            }
            // 既知の差（v3-mame.md）：サイクル数だけが違い、状態は同じなら数えて続ける
            if bad == ["CYC"] && !bad_events {
                let name = mnemonic(op);
                let known = if is_conditional_branch(op) && theirs[11] == 1 && ours[11] == 2 {
                    // (b) 条件分岐は成否によらず 2 サイクル（台帳 追補 A）。MAME は飛ばないとき 1
                    Some("条件分岐で飛ばないときのサイクル数")
                } else if name.contains("未定義") && theirs[11] == 0 {
                    // (c) 未定義の命令。MAME は 0 サイクル（エミュレータの都合）
                    Some("未定義の命令のサイクル数")
                } else {
                    None
                };
                if let Some(k) = known {
                    *known_diffs.entry(k).or_default() += 1;
                    good_steps += 1;
                    continue;
                }
            }
            // (c) DMOV・LTD で 255 番地を指すと、MAME は 0 番地へ回り込んで写す（台帳 §7 の 6）。
            // 相手の RAM にそろえて続ける
            if matches!(op >> 8, 0x69 | 0x6B)
                && data_addr(op, st_before, ar_before) == 0xFF
                && bad.is_empty()
                && their_events
                    .lines()
                    .filter(|l| !l.starts_with("D 00 "))
                    .eq(bus.events.lines())
            {
                for v in their_events.lines().filter_map(|l| l.strip_prefix("D 00 ")) {
                    dsp.set_data(0, u16::from_str_radix(v, 16).unwrap());
                }
                *known_diffs
                    .entry("DMOV・LTD の 255 番地から 0 番地への回り込み")
                    .or_default() += 1;
                good_steps += 1;
                continue;
            }
            // (c) 7F の群の未定義の語。MAME は下位 5 bit だけで解読する（7FA0〜7FFF を 7F80〜7F9F と
            // 同じ命令にする）。状態がずれるので、その場合はここで打ち切る
            if mnemonic(op) == "7F(未定義)" && !mnemonic(0x7F80 | (op & 0x1F)).contains("未定義")
            {
                *known_diffs
                    .entry("7F の群の未定義の語（打ち切り）")
                    .or_default() += 1;
                stopped = true;
                break;
            }
            let mut what = bad.join("+");
            if bad_events {
                what.push_str(if what.is_empty() { "IO" } else { "+IO" });
            }
            failed = Some((
                format!("{} {what}", mnemonic(op)),
                format!(
                    "命令 {op:04X}（次の語 {:04X}）、前 {before}\n  コア {ours:X?} {:?}\n  相手 {theirs:X?} {their_events:?}",
                    bus.prog[usize::from(ours[0] as u16 & 0xFFF)],
                    bus.events
                ),
            ));
            break;
        }
        if stopped {
            known_cases += 1;
            continue;
        }
        if failed.is_none() {
            let m = e.find(|l| l.starts_with('M')).expect("RAM の行がない");
            let their_ram = hex_words(&m[1..]);
            let diff: Vec<String> = (0..144)
                .filter(|&a| dsp.data(a as u8) != their_ram[a])
                .map(|a| format!("{a:02X}: {:04X}/{:04X}", dsp.data(a as u8), their_ram[a]))
                .collect();
            if !diff.is_empty() {
                failed = Some(("データ RAM".to_string(), diff.join(", ")));
            }
        }
        match failed {
            None => good_cases += 1,
            Some((kind, detail)) => {
                let n = kinds.entry(kind.clone()).or_default();
                *n += 1;
                if *n <= verbose {
                    eprintln!("場合 {}：{kind}：{detail}", cases - 1);
                }
            }
        }
    }
    eprintln!(
        "TMS32010 の V3：{cases} 件のうち {good_cases} 件が最後まで一致（既知の差を含む）、{known_cases} 件は既知の差で打ち切り。\
         命令 {steps} のうち {good_steps} が一致"
    );
    for (k, n) in &known_diffs {
        eprintln!("  既知の差 {k}: {n}");
    }
    for (k, n) in &kinds {
        eprintln!("  {k}: {n}");
    }
}

/// 命令の前の状態語と AR から、データ RAM の番地（台帳 §4.2）。
fn data_addr(op: u16, st: u16, ar: [u16; 2]) -> u8 {
    if op & 0x80 != 0 {
        ar[usize::from((st >> 8) & 1)] as u8
    } else {
        (((st & 1) << 7) | (op & 0x7F)) as u8
    }
}

/// 条件分岐（BANZ、BV、BIOZ、BLZ、BLEZ、BGZ、BGEZ、BNZ、BZ）。
fn is_conditional_branch(op: u16) -> bool {
    matches!(op >> 8, 0xF4..=0xF6 | 0xFA..=0xFF)
}
