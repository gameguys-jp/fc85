//! MSM5205 のコアを、独立した実装（MAME の msm5205）と照らし合わせる（V3、docs/impl/v3-mame.md）。
//!
//! 入力の列は `tools/v3/msm5205_vectors.py` で作り、照合の相手の結果と一緒に、リポジトリの外に置く。
//! 環境変数 `FC85_V3_DIR` に、`msm5205/in.txt` と `msm5205/out.txt` のあるフォルダを渡す。なければ飛ばす。
//!
//! FC85_V3_DIR=<v3> cargo test --release -p fc85-chips --test msm5205_v3 -- --nocapture
//!
//! V̄C̄K̄ の縁と取込みを、最初の立下りからのクロック数で比べ、取込みでは予測値とステップ番号も比べる。
//! あふれはクリップ（MAME と同じ）で比べ、本機の標準の回り込みで食い違う場合は既知の差として数える。

use fc85_chips::msm5205::{Edge, Msm5205, Overflow};
use std::fmt::Write as _;

/// 1 件の進み具合。相手の道具と同じ決まりで入力を入れ、行を書く。
struct Run<'a> {
    ins: &'a [u8],
    next: usize,
    seen_fall: bool,
    t0: i64,
    ev: String,
}

impl Run<'_> {
    /// 次の語の RESET と D3〜D0 を入れる。
    fn apply(&mut self, m: &mut Msm5205) {
        let v = self.ins[self.next.min(self.ins.len() - 1)];
        self.next += 1;
        m.set_reset(v & 0x10 != 0);
        m.set_data(v & 0x0F);
    }

    /// V̄C̄K̄ の縁。最初の立下りの後の立上りごとに、次の語を入れる。
    fn edge(&mut self, level: bool, t: u64, m: &mut Msm5205) {
        if level {
            if self.seen_fall {
                writeln!(self.ev, "R {}", t as i64 - self.t0).unwrap();
                self.apply(m);
            }
        } else {
            if !self.seen_fall {
                self.seen_fall = true;
                self.t0 = t as i64;
            }
            writeln!(self.ev, "F {}", t as i64 - self.t0).unwrap();
        }
    }
}

/// 1 件を動かし、相手と同じ形の行を返す。
fn run_case(
    s1: bool,
    s2: bool,
    four: bool,
    n: usize,
    half: u64,
    ins: &[u8],
    overflow: Overflow,
) -> String {
    let mut m = Msm5205::new();
    m.set_rate_pins(s1, s2);
    m.set_four_bit(four);
    m.set_overflow(overflow);
    let mut r = Run {
        ins,
        next: 0,
        seen_fall: false,
        t0: 0,
        ev: String::new(),
    };
    r.apply(&mut m);
    let slave = s1 && s2;
    let mut caps = 0;
    let mut t = 0u64;
    while caps < n && t < 100_000_000 {
        t += 1;
        let tk = m.tick();
        if let Some(c) = tk.capture {
            // 取込みは最初の立下りの後にしか起きない
            writeln!(r.ev, "K {} {} {}", t as i64 - r.t0, c.signal, c.step).unwrap();
            caps += 1;
        }
        match tk.edge {
            Some(Edge::Rise) => r.edge(true, t, &mut m),
            Some(Edge::Fall) => r.edge(false, t, &mut m),
            None => {}
        }
        if slave && t.is_multiple_of(half) {
            // 立上りでは、V̄C̄K̄ を変える前にデータと RESET を入れる（相手と同じ順）
            let level = (t / half) % 2 == 1;
            r.edge(level, t, &mut m);
            m.set_vck_input(level);
        }
    }
    r.ev
}

#[test]
fn msm5205_matches_mame() {
    let Ok(dir) = std::env::var("FC85_V3_DIR") else {
        eprintln!("FC85_V3_DIR がないので飛ばす");
        return;
    };
    let dir = std::path::Path::new(&dir).join("msm5205");
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

    let mut expected: Vec<String> = Vec::new();
    for line in output.lines() {
        if line.starts_with("C ") {
            expected.push(String::new());
        } else if let Some(v) = expected.last_mut() {
            v.push_str(line);
            v.push('\n');
        }
    }

    let (mut cases, mut good, mut wrap_same, mut wrap_clip, mut caps) = (0, 0, 0, 0, 0usize);
    let mut bad = 0;
    let mut lines = input.lines();
    while let Some(head) = lines.next() {
        let Some(head) = head.strip_prefix("C ") else {
            continue;
        };
        let h: Vec<u64> = head
            .split_whitespace()
            .map(|w| w.parse().unwrap())
            .collect();
        let (s1, s2, four, n, half) = (h[1] == 1, h[2] == 1, h[3] == 1, h[4] as usize, h[5]);
        let ins: Vec<u8> = lines.next().unwrap()[2..]
            .split_whitespace()
            .map(|w| u8::from_str_radix(w, 16).unwrap())
            .collect();
        let theirs = &expected[cases];
        cases += 1;
        caps += n;

        let ours = run_case(s1, s2, four, n, half, &ins, Overflow::Clip);
        if ours == *theirs {
            good += 1;
        } else {
            bad += 1;
            if bad <= verbose {
                let at = ours.lines().zip(theirs.lines()).position(|(a, b)| a != b);
                let show = |s: &str| -> Vec<String> {
                    s.lines()
                        .skip(at.unwrap_or(0).saturating_sub(3))
                        .take(6)
                        .map(str::to_string)
                        .collect()
                };
                eprintln!(
                    "場合 {}（S1 {s1} S2 {s2} 4B {four} 半周期 {half}）：行 {at:?}\n  コア {:?}\n  相手 {:?}",
                    cases - 1,
                    show(&ours),
                    show(theirs)
                );
            }
        }

        // 本機の標準（回り込み）。食い違いが、相手がクリップした取込みから始まるなら既知の差
        let wrap = run_case(s1, s2, four, n, half, &ins, Overflow::Wrap);
        if wrap == *theirs {
            wrap_same += 1;
        } else if let Some((_, b)) = wrap.lines().zip(theirs.lines()).find(|(a, b)| a != b)
            && matches!(b.split_whitespace().nth(2), Some("2047" | "-2048"))
        {
            wrap_clip += 1;
        }
    }
    eprintln!(
        "MSM5205 の V3：{cases} 件（取込み {caps} 回）のうち、クリップで {good} 件が最後まで一致。\
         回り込み（本機の標準）では {wrap_same} 件が一致し、{wrap_clip} 件はあふれの扱いの既知の差"
    );
    assert_eq!(bad, 0, "クリップで比べて食い違う場合がある");
    assert_eq!(
        wrap_same + wrap_clip,
        cases,
        "回り込みで、あふれ以外の食い違いがある"
    );
}
