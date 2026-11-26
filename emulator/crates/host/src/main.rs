//! fc85 のホスト（仕様書 §12 の host 層。DR-055）。
//!
//! 仮想機（board 層の `Console`）を 1 フレームずつ進め、画面を窓に、音声をオーディオ装置に出し、
//! キーボードを PORT_A に渡す。F5 で状態を保存し、F9 で戻す。F2 で通常モードと Bend モードを
//! 切り替える（DR-065）。ホストの安全層（音量の制限、表示の保持）は仮想機の挙動を変えない。
//!
//! 使い方：`fc85 [--boot boot.bin] [--char char.bin] [--cart cart.bin] [--no-doc] [--scale N]
//! [--frames N] [--headless] [--dump out.ppm] [--dump-at F,F..] [--wav out.wav] [--trace N]
//! [--pad-script FILE] [--doc-clock HZ] [--dsp-clock HZ] [--adpcm-clock HZ]
//! [--audio-clock HZ] [--midi-list] [--midi-in N] [--midi-out N] [--char-bend SPEC] [--bend CMD]
//! [--bend-script FILE]`
//!
//! ベンド点（仕様書 §11、DR-065）は `--bend CMD` で設定する。CMD は `Bends::command` の形：
//! `mode normal|bend|toggle`、`char A1=A2,A2=A1,D6=0,D2=Z`、`clock doc|dsp|adpcm|audio HZ|off`、
//! `fifo dac|dsp_in|dsp_out|adpcm empty=0|1|open full=0|1|open`、`factory`。ベンド点を 1 つでも
//! 設定すると Bend モードで始まる（`--bend "mode normal"` で通常モードから始め、F2 で入れる）。
//! `--bend-script` は「フレーム 命令」の行の台本で、そのフレームの頭で実行する。
//! `--*-clock HZ` は `--bend "clock … HZ"`、`--char-bend SPEC` は `--bend "char SPEC"` の短い形。

mod midi;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use fc85_board::console::{AUDIO_RATE, BOOT_ROM_BYTES, Console};
use fc85_board::video::{RASTER_H, RASTER_W, rgb888};
use fc85_chips::hd68000::RunState;

/// 窓に出す範囲（ドット、ライン）。案B の表示（64〜319 ドット、43〜234 ライン）を真ん中に置く。
const VIEW_X: usize = 22;
const VIEW_Y: usize = 19;
const VIEW_W: usize = 320;
const VIEW_H: usize = 240;

/// PORT_A のビット（DR-055）。
const PAD_UP: u16 = 0x01;
const PAD_DOWN: u16 = 0x02;
const PAD_LEFT: u16 = 0x04;
const PAD_RIGHT: u16 = 0x08;
const PAD_A: u16 = 0x10;
const PAD_B: u16 = 0x20;
const PAD_C: u16 = 0x40;
const PAD_START: u16 = 0x80;

struct Args {
    boot: Option<String>,
    chars: Option<String>,
    cart: Option<String>,
    doc: bool,
    scale: usize,
    frames: Option<u64>,
    /// 最初の N 命令の PC と SR を標準エラーに出す。
    trace: u64,
    /// headless の入力の台本：(フレーム, PORT_A)。その値をそのフレームから次の行まで保つ。
    pad_script: Vec<(u64, u16)>,
    /// headless で画面を書き出すフレーム（`--dump` の名前に `-<フレーム>` を足す）。
    dump_at: Vec<u64>,
    headless: bool,
    dump: Option<String>,
    wav: Option<String>,
    /// ベンド点の命令（`Bends::command`）。起動のときに順に実行する。
    bends: Vec<String>,
    /// ベンド点の台本：(フレーム, 命令)。
    bend_script: Vec<(u64, String)>,
    /// MIDI の装置の番号（`--midi-list`）。
    midi_in: Option<usize>,
    midi_out: Option<usize>,
}

fn parse_args() -> Args {
    let mut a = Args {
        boot: None,
        chars: None,
        cart: None,
        doc: true,
        scale: 3,
        frames: None,
        trace: 0,
        pad_script: Vec::new(),
        dump_at: Vec::new(),
        headless: false,
        dump: None,
        wav: None,
        bends: Vec::new(),
        bend_script: Vec::new(),
        midi_in: None,
        midi_out: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "--boot" => a.boot = it.next(),
            "--char" => a.chars = it.next(),
            "--char-bend" => a.bends.extend(it.next().map(|v| format!("char {v}"))),
            "--bend" => a.bends.extend(it.next()),
            "--bend-script" => {
                a.bend_script = it.next().map(|p| read_bend_script(&p)).unwrap_or_default()
            }
            "--cart" => a.cart = it.next(),
            "--no-doc" => a.doc = false,
            "--scale" => a.scale = it.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            "--frames" => a.frames = it.next().and_then(|v| v.parse().ok()),
            "--headless" => a.headless = true,
            "--dump" => a.dump = it.next(),
            "--wav" => a.wav = it.next(),
            "--midi-list" => {
                midi::list();
                std::process::exit(0);
            }
            "--midi-in" => a.midi_in = it.next().and_then(|v| v.parse().ok()),
            "--midi-out" => a.midi_out = it.next().and_then(|v| v.parse().ok()),
            "--trace" => a.trace = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--pad-script" => {
                a.pad_script = it.next().map(|p| read_pad_script(&p)).unwrap_or_default()
            }
            "--dump-at" => {
                a.dump_at = it
                    .next()
                    .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
                    .unwrap_or_default()
            }
            "--doc-clock" | "--dsp-clock" | "--adpcm-clock" | "--audio-clock" => {
                let which = &s[2..s.len() - 6];
                a.bends
                    .extend(it.next().map(|v| format!("clock {which} {v}")));
            }
            "-h" | "--help" => {
                println!(
                    "fc85 [--boot boot.bin] [--char char.bin] [--cart cart.bin] [--no-doc] \
                     [--scale N] [--frames N] [--headless] [--dump out.ppm] [--dump-at F,F..] [--wav out.wav] [--trace N] \
                     [--pad-script FILE] [--doc-clock HZ] \
                     [--dsp-clock HZ] [--adpcm-clock HZ] [--audio-clock HZ] [--midi-list] [--midi-in N] [--midi-out N] [--char-bend SPEC] [--bend CMD] [--bend-script FILE]"
                );
                std::process::exit(0);
            }
            _ => {
                eprintln!("知らない引数: {s}");
                std::process::exit(2);
            }
        }
    }
    a
}

/// ベンド点の台本を読む。1 行に「フレーム 命令」。`#` から後は注釈。
fn read_bend_script(path: &str) -> Vec<(u64, String)> {
    let text = String::from_utf8_lossy(&load(path)).into_owned();
    let mut v = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (frame, cmd) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match frame.parse::<u64>() {
            Ok(f) if !cmd.trim().is_empty() => v.push((f, cmd.trim().to_string())),
            _ => eprintln!("{path}:{}: 「フレーム 命令」ではない", i + 1),
        }
    }
    v.sort_by_key(|e| e.0);
    v
}

/// ベンド点の命令を実行して、回路に入れる。
fn bend(c: &mut Console, cmd: &str) {
    match c.sys.bend(cmd) {
        Ok(()) => eprintln!("ベンド点：{cmd}（{}）", c.sys.bends.summary()),
        Err(e) => eprintln!("ベンド点：{e}"),
    }
}

/// 台本のうち、フレーム `frame` の頭で実行する命令。
fn run_bend_script(c: &mut Console, a: &Args, frame: u64) {
    for (_, cmd) in a.bend_script.iter().filter(|e| e.0 == frame) {
        bend(c, cmd);
    }
}

/// 入力の台本を読む。1 行に「フレーム 値」（値は 16 進。`$`・`0x` を付けてもよい）。`#` から後は注釈。
fn read_pad_script(path: &str) -> Vec<(u64, u16)> {
    // 注釈が UTF-8 でなくても（cp932 など）数の行は読めるようにする
    let text = String::from_utf8_lossy(&load(path)).into_owned();
    let mut v = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut f = line.split_whitespace();
        let frame = f.next().and_then(|x| x.parse::<u64>().ok());
        let value = f.next().and_then(|x| {
            let x = x.trim_start_matches('$').trim_start_matches("0x");
            u16::from_str_radix(x, 16).ok()
        });
        match (frame, value) {
            (Some(fr), Some(val)) => v.push((fr, val)),
            _ => eprintln!("{path}:{}: 「フレーム 値」ではない", i + 1),
        }
    }
    v.sort_by_key(|e| e.0);
    v
}

/// `out.ppm` → `out-<n>.ppm`。
fn numbered(path: &str, n: u64) -> String {
    match path.rfind('.') {
        Some(i) => format!("{}-{n}{}", &path[..i], &path[i..]),
        None => format!("{path}-{n}"),
    }
}

fn load(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("{path} を読めない: {e}");
        std::process::exit(1);
    })
}

/// 既定の置き場所（リポジトリの firmware のビルド結果）から探す。
fn default_path(rel: &str) -> Option<String> {
    for base in ["", "../", "../../"] {
        let p = format!("{base}{rel}");
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    None
}

fn build_console(a: &Args) -> Console {
    let mut c = Console::new();
    c.sys.ports.doc = a.doc;
    let boot = a
        .boot
        .clone()
        .or_else(|| default_path("firmware/boot/build/boot.bin"));
    match boot {
        Some(p) => {
            let rom = load(&p);
            let n = rom.len().min(BOOT_ROM_BYTES);
            c.sys.boot_rom[..n].copy_from_slice(&rom[..n]);
        }
        None => eprintln!("Boot ROM がない（--boot）。0xFF のまま始める"),
    }
    if let Some(p) = a
        .chars
        .clone()
        .or_else(|| default_path("firmware/charrom/build/char.bin"))
    {
        let rom = load(&p);
        let n = rom.len().min(c.sys.ports.char_rom.len());
        c.sys.ports.char_rom[..n].copy_from_slice(&rom[..n]);
    }
    for cmd in &a.bends {
        if let Err(e) = c.sys.bends.command(cmd) {
            eprintln!("--bend: {e}");
            std::process::exit(2);
        }
    }
    let explicit_mode = a.bends.iter().any(|b| b.trim_start().starts_with("mode"));
    if !c.sys.bends.is_factory() && !explicit_mode {
        c.sys.bends.mode = fc85_board::bend::Mode::Bend;
    }
    c.sys.apply_bends();
    if !a.bends.is_empty() {
        eprintln!("ベンド点：{}", c.sys.bends.summary());
    }
    if let Some(p) = &a.cart {
        c.sys.cart = Some(load(p));
    }
    c.power_on();
    c
}

/// 画面の配列から、窓に出す範囲を 0RGB の u32 にする。
fn view(c: &Console, out: &mut [u32]) {
    let f = &c.sys.ports.acrtc.video.frame;
    for y in 0..VIEW_H {
        for x in 0..VIEW_W {
            let (sx, sy) = (VIEW_X + x, VIEW_Y + y);
            let v = if sx < RASTER_W && sy < RASTER_H {
                f[sy * RASTER_W + sx]
            } else {
                0
            };
            let [r, g, b] = rgb888(v);
            out[y * VIEW_W + x] = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
        }
    }
}

fn dump_ppm(path: &str, buf: &[u32]) {
    let mut data = format!("P6\n{VIEW_W} {VIEW_H}\n255\n").into_bytes();
    for p in buf {
        data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    if let Err(e) = std::fs::write(path, data) {
        eprintln!("{path} に書けない: {e}");
    }
}

// ---------------------------------------------------------------------- 音声

/// 48 kHz・16 bit・ステレオの WAV に書く（headless で音を確かめる）。
fn write_wav(path: &str, pcm: &[(i16, i16)]) {
    let bytes = (pcm.len() * 4) as u32;
    let mut d = Vec::with_capacity(44 + bytes as usize);
    d.extend_from_slice(b"RIFF");
    d.extend_from_slice(&(36 + bytes).to_le_bytes());
    d.extend_from_slice(b"WAVEfmt ");
    d.extend_from_slice(&16u32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&2u16.to_le_bytes());
    d.extend_from_slice(&AUDIO_RATE.to_le_bytes());
    d.extend_from_slice(&(AUDIO_RATE * 4).to_le_bytes());
    d.extend_from_slice(&4u16.to_le_bytes());
    d.extend_from_slice(&16u16.to_le_bytes());
    d.extend_from_slice(b"data");
    d.extend_from_slice(&bytes.to_le_bytes());
    for (l, r) in pcm {
        d.extend_from_slice(&l.to_le_bytes());
        d.extend_from_slice(&r.to_le_bytes());
    }
    if let Err(e) = std::fs::write(path, d) {
        eprintln!("{path} に書けない: {e}");
    }
}

type Ring = Arc<Mutex<VecDeque<(i16, i16)>>>;

/// オーディオ装置を開く。装置がなければ `None`（音なしで動く）。
fn open_audio(ring: Ring) -> Option<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let dev = host.default_output_device()?;
    let cfg = dev.default_output_config().ok()?;
    let rate = cfg.sample_rate().0;
    let channels = usize::from(cfg.channels());
    let step = f64::from(AUDIO_RATE) / f64::from(rate);
    let mut pos = 0.0f64;
    let mut cur = (0i16, 0i16);
    let config: cpal::StreamConfig = cfg.clone().into();
    let err = |e| eprintln!("音声: {e}");
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => dev.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                let mut q = ring.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    pos += step;
                    while pos >= 1.0 {
                        pos -= 1.0;
                        if let Some(s) = q.pop_front() {
                            cur = s;
                        }
                    }
                    for (i, o) in frame.iter_mut().enumerate() {
                        let v = if i % 2 == 0 { cur.0 } else { cur.1 };
                        *o = f32::from(v) / 32768.0;
                    }
                }
            },
            err,
            None,
        ),
        cpal::SampleFormat::I16 => dev.build_output_stream(
            &config,
            move |data: &mut [i16], _| {
                let mut q = ring.lock().unwrap();
                for frame in data.chunks_mut(channels) {
                    pos += step;
                    while pos >= 1.0 {
                        pos -= 1.0;
                        if let Some(s) = q.pop_front() {
                            cur = s;
                        }
                    }
                    for (i, o) in frame.iter_mut().enumerate() {
                        *o = if i % 2 == 0 { cur.0 } else { cur.1 };
                    }
                }
            },
            err,
            None,
        ),
        _ => return None,
    }
    .ok()?;
    stream.play().ok()?;
    Some(stream)
}

/// 実時間に合わせて待つ。音声の装置があれば、リングに残る標本が 3 フレーム分を切るまで待つ
/// （仮想機の音声の速さに合わせるので、リングがあふれも尽きもしない）。なければ 1/59.92 秒ごと。
fn pace(audio: bool, ring: &Ring, next: &mut std::time::Instant) {
    use std::time::{Duration, Instant};
    if audio {
        let target = 3 * AUDIO_RATE as usize / 60;
        let start = Instant::now();
        while ring.lock().unwrap().len() > target && start.elapsed() < Duration::from_millis(100) {
            std::thread::sleep(Duration::from_millis(1));
        }
    } else {
        *next += Duration::from_secs_f64(1.0 / 59.92);
        let now = Instant::now();
        if *next > now {
            std::thread::sleep(*next - now);
        } else {
            *next = now;
        }
    }
}

/// 仮想機の音声をリングに入れる。遅れすぎたら古いものを捨てる（ホストの安全層。仮想機は変えない）。
fn push_audio(c: &mut Console, ring: &Ring) {
    let mut q = ring.lock().unwrap();
    q.extend(c.sys.audio_out.drain(..));
    let max = AUDIO_RATE as usize / 5;
    while q.len() > max {
        q.pop_front();
    }
}

// ---------------------------------------------------------------------- 保存

/// 保存の置き場所（今のフォルダ）。
const STATE_FILE: &str = "fc85.state.json";

/// F5：仮想機の状態をまるごと保存する（DR-040 の snapshot）。画面の配列と音声の待ちは入れない。
fn save_state(c: &Console) {
    match serde_json::to_vec(c) {
        Ok(v) => match std::fs::write(STATE_FILE, v) {
            Ok(()) => eprintln!("{STATE_FILE} に保存した"),
            Err(e) => eprintln!("{STATE_FILE} に書けない: {e}"),
        },
        Err(e) => eprintln!("保存できない: {e}"),
    }
}

/// F9：保存した状態に戻す。
fn load_state(c: &mut Console) {
    let r = std::fs::read(STATE_FILE)
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_slice::<Console>(&v).map_err(|e| e.to_string()));
    match r {
        Ok(s) => {
            *c = s;
            eprintln!("{STATE_FILE} から戻した");
        }
        Err(e) => eprintln!("{STATE_FILE} から戻せない: {e}"),
    }
}

// ---------------------------------------------------------------------- 入力

fn pad(w: &minifb::Window) -> u16 {
    use minifb::Key;
    let mut v = 0;
    for (k, b) in [
        (Key::Up, PAD_UP),
        (Key::Down, PAD_DOWN),
        (Key::Left, PAD_LEFT),
        (Key::Right, PAD_RIGHT),
        (Key::Z, PAD_A),
        (Key::X, PAD_B),
        (Key::C, PAD_C),
        (Key::Enter, PAD_START),
    ] {
        if w.is_key_down(k) {
            v |= b;
        }
    }
    v
}

/// 最初の N 命令を 1 つずつ実行し、実行前の PC と SR、D0〜D3、A7 を出す。
fn trace(c: &mut Console, n: u64) {
    for _ in 0..n {
        let cpu = &c.cpu;
        eprintln!(
            "{:>10} PC {:06X} SR {:04X} D0 {:08X} D1 {:08X} D2 {:08X} D3 {:08X} A7 {:08X}",
            cpu.clock,
            cpu.pc,
            cpu.sr,
            cpu.d[0],
            cpu.d[1],
            cpu.d[2],
            cpu.d[3],
            if cpu.sr & 0x2000 != 0 {
                cpu.ssp
            } else {
                cpu.usp
            }
        );
        c.step();
    }
    let t = c.cpu.clock;
    c.sys.sync(t);
}

/// バスの警告が増えたら出す（ホストの表示。仮想機は変えない）。
fn report(c: &Console, seen: &mut (u64, u64, bool)) {
    let l = &c.sys.log;
    let halted = c.cpu.state == RunState::Halted;
    if halted && !seen.2 {
        eprintln!(
            "68000 が止まった（二重バスフォルト。PC {:06X}）。リセットまで動かない",
            c.cpu.pc
        );
    }
    seen.2 = halted;
    if l.bus_errors != seen.0 {
        if let Some((addr, w)) = l.last_bus_error {
            eprintln!(
                "バスエラー {} 回（最後は {addr:06X} の{}）",
                l.bus_errors,
                if w { "書込み" } else { "読出し" }
            );
        }
        seen.0 = l.bus_errors;
    }
    // Bend モードでは規約違反を警告しない（実行はどちらのモードでも同じ。DR-065）
    if l.doc_rule_warnings != seen.1 && c.sys.bends.mode == fc85_board::bend::Mode::Normal {
        if let Some(addr) = l.last_doc_rule {
            eprintln!(
                "DOC の窓の規約（DR-030）に反した命令 {} 回（最後は {addr:06X}）",
                l.doc_rule_warnings
            );
        }
        seen.1 = l.doc_rule_warnings;
    }
}

fn main() {
    let a = parse_args();
    let mut c = build_console(&a);
    let mut seen = (0, 0, false);
    if a.trace > 0 {
        trace(&mut c, a.trace);
    }
    let mut buf = vec![0u32; VIEW_W * VIEW_H];

    if a.headless {
        let n = a.frames.unwrap_or(60);
        let mut pcm = Vec::new();
        for i in 0..n {
            run_bend_script(&mut c, &a, i);
            if let Some(&(_, v)) = a.pad_script.iter().rev().find(|e| e.0 <= i) {
                c.sys.set_ports(v, 0);
            }
            c.run_frame();
            report(&c, &mut seen);
            if a.dump_at.contains(&(i + 1))
                && let Some(p) = &a.dump
            {
                view(&c, &mut buf);
                dump_ppm(&numbered(p, i + 1), &buf);
            }
            if a.wav.is_some() {
                pcm.append(&mut c.sys.audio_out);
            } else {
                c.sys.audio_out.clear();
            }
        }
        view(&c, &mut buf);
        if let Some(p) = &a.dump {
            dump_ppm(p, &buf);
        }
        if let Some(p) = &a.wav {
            write_wav(p, &pcm);
        }
        println!(
            "{} フレーム、CPU {} クロック、バスエラー {}",
            c.sys.ports.acrtc.video.frames, c.cpu.clock, c.sys.log.bus_errors
        );
        return;
    }

    let ring: Ring = Arc::new(Mutex::new(VecDeque::new()));
    let audio = open_audio(ring.clone());
    let scale = match a.scale {
        1 => minifb::Scale::X1,
        2 => minifb::Scale::X2,
        4 => minifb::Scale::X4,
        _ => minifb::Scale::FitScreen,
    };
    let mut w = minifb::Window::new(
        "fc85",
        VIEW_W,
        VIEW_H,
        minifb::WindowOptions {
            scale,
            resize: true,
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("窓を開けない: {e}");
        std::process::exit(1);
    });
    // 速さはホストが合わせる（pace）。minifb の 60 fps の制限は仮想機の約 59.92 Hz とずれる。
    w.set_target_fps(0);
    let mut next = std::time::Instant::now();
    let mut midi = midi::Midi::open(a.midi_in, a.midi_out);
    let mut frames = 0u64;
    while w.is_open() && !w.is_key_down(minifb::Key::Escape) {
        if w.is_key_pressed(minifb::Key::F5, minifb::KeyRepeat::No) {
            save_state(&c);
        }
        if w.is_key_pressed(minifb::Key::F9, minifb::KeyRepeat::No) {
            load_state(&mut c);
        }
        if w.is_key_pressed(minifb::Key::F2, minifb::KeyRepeat::No) {
            bend(&mut c, "mode toggle");
        }
        run_bend_script(&mut c, &a, frames);
        c.sys.set_ports(pad(&w), 0);
        for b in midi.take_received() {
            c.sys.acia.receive(b);
        }
        c.run_frame();
        while let Some(b) = c.sys.acia.take_sent() {
            midi.send(b);
        }
        push_audio(&mut c, &ring);
        pace(audio.is_some(), &ring, &mut next);
        view(&c, &mut buf);
        if w.update_with_buffer(&buf, VIEW_W, VIEW_H).is_err() {
            break;
        }
        report(&c, &mut seen);
        frames += 1;
        if a.frames.is_some_and(|n| frames >= n) {
            break;
        }
    }
    if let Some(p) = &a.dump {
        dump_ppm(p, &buf);
    }
}
