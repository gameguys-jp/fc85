//! fc85 のホスト（仕様書 §12 の host 層。DR-055）。
//!
//! 仮想機（board 層の `Console`）を 1 フレームずつ進め、画面を窓に、音声をオーディオ装置に出し、
//! キーボードを PORT_A に渡す。F5 で状態を保存し、F9 で戻す。F2 で通常モードと Bend モードを
//! 切り替える（DR-065）。ホストの安全層（音量の制限、表示の保持）は仮想機の挙動を変えない。
//!
//! クロックのつまみ（外部の発振器・VCO を回す操作）：F3 で回すクロック（DOC、映像、DSP、ADPCM、音声）を
//! 選び、PageUp・PageDown（PageUp・PageDown のないキーボードでは `]`・`[`）で半音ずつ、Shift を押しながら
//! なら 1/8 半音ずつ上げ下げする。押し続けると回り続ける。Home（か `\`）で基板の発振器に戻す。回すと
//! Bend モードに入る。窓を開けたときと F3 を押したときに、キーの案内を標準エラーに出す。
//!
//! 使い方：`fc85 [--boot boot.bin] [--char char.bin] [--cart cart.bin] [--no-doc] [--scale N]
//! [--frames N] [--headless] [--dump out.ppm] [--dump-at F,F..] [--wav out.wav] [--trace N]
//! [--pad-script FILE] [--doc-clock HZ] [--dsp-clock HZ] [--adpcm-clock HZ]
//! [--audio-clock HZ] [--video-clock HZ] [--midi-list] [--midi-in N] [--midi-out N] [--char-bend SPEC]
//! [--bend CMD] [--bend-script FILE] [--audio-tap digital|line] [--doc-sig both|one] [--audio-in FILE.wav]
//! [--audio-in-device N|default] [--audio-list] [--no-gamepad]`
//!
//! ベンド点（仕様書 §11、DR-065）は `--bend CMD` で設定する。CMD は `Bends::command` の形：
//! `mode normal|bend|toggle`、`char A1=A2,A2=A1,D6=0,D2=Z`、`clock doc|dsp|adpcm|audio HZ|off`、
//! `clock video HZ|0|off`（映像系の 21.47727 MHz。0 で止める）、`fifo dac|dsp_in|dsp_out|adpcm
//! empty=0|1|open full=0|1|open`、`sig both|one`、`dram unit N|temp C|refresh hsync|auto`、
//! `docbus A3=0,A4=A5,A5=A4,D7=1,A9=Z`（DOC 子基板のバスのテストピン。A0〜A7 と A8〜A15＝D0〜D7。
//! `normal` で戻す）、`factory`。ベンド点を
//! 1 つでも設定すると Bend モードで始まる（`--bend "mode normal"` で通常モードから始め、F2 で入れる）。
//! `--bend-script` は「フレーム 命令」の行の台本で、そのフレームの頭で実行する。
//! `--*-clock HZ` は `--bend "clock … HZ"`、`--char-bend SPEC` は `--bend "char SPEC"`、`--doc-sig one`
//! は `--bend "sig one"`（DOC の SIG± の配線のジャンパ。DR-011）の短い形。
//! 画面（窓と `--dump`）は機械の外のテレビの模型（board 層の `tv`）を通す。映像のクロックを替えると、同期と
//! 色が引込みの範囲を外れて崩れる。工場出荷のクロックでは画面の配列そのまま。テレビの状態が変わると
//! 標準エラーに出す。仮想機は 1 フレーム（ACRTC のフレームの終わり、上限は CPU の 2 フレーム分。映像の
//! クロックが基板の半分より遅いか止まっていれば 1/60 秒分）ずつ進めるので、映像のクロックを止めても固まらない。
//! `--audio-tap` は鳴らす音と WAV に書く音：`line`（既定）はアナログの段の LINE OUT（analog.rs）、`digital` は
//! チップの出力のデジタルの和（DR-054。`sdk/samples/dsp/verify_wav.py` が標本ごとに照合するのはこちら）。
//! `--audio-in` は背面の Audio IN に WAV を、`--audio-in-device` は録音の装置の音を入れる（DR-129。audio_in.rs）。
//! ゲームパッドは gilrs で読み、1 台目を PORT_A（キーボードと OR）、2 台目を PORT_B にする。`--no-gamepad` で
//! 読まない（DR-132。gamepad.rs）。

mod audio_in;
mod gamepad;
mod midi;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use fc85_board::bend::Mode;
use fc85_board::console::{AUDIO_RATE, BOOT_ROM_BYTES, Console};
use fc85_board::tv::Tv;
use fc85_board::video::{RASTER_H, RASTER_W};
use fc85_chips::hd68000::RunState;
use fc85_core::time::UNITS_PER_SECOND;

/// 窓に出す範囲（ドット、ライン）。案B の表示（64〜319 ドット、43〜234 ライン）を真ん中に置く。
const VIEW_X: usize = 22;
const VIEW_Y: usize = 19;
const VIEW_W: usize = 320;
const VIEW_H: usize = 240;

/// PORT_A・PORT_B のビット（DR-055）。
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
    /// 鳴らして WAV に書く音：true は LINE OUT、false はデジタルの和。
    line: bool,
    /// Audio IN の源（DR-129）：WAV のパスか、録音の装置（`default` か番号）。
    audio_in: Option<String>,
    audio_in_device: Option<String>,
    /// ゲームパッドを読む（DR-132）。
    gamepad: bool,
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
        line: true,
        audio_in: None,
        audio_in_device: None,
        gamepad: true,
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
            "--no-gamepad" => a.gamepad = false,
            "--scale" => a.scale = it.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            "--frames" => a.frames = it.next().and_then(|v| v.parse().ok()),
            "--headless" => a.headless = true,
            "--dump" => a.dump = it.next(),
            "--wav" => a.wav = it.next(),
            "--midi-list" => {
                midi::list();
                std::process::exit(0);
            }
            "--audio-in" => a.audio_in = it.next(),
            "--audio-in-device" => a.audio_in_device = it.next(),
            "--audio-list" => {
                audio_in::list();
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
            "--doc-clock" | "--dsp-clock" | "--adpcm-clock" | "--audio-clock" | "--video-clock" => {
                let which = &s[2..s.len() - 6];
                a.bends
                    .extend(it.next().map(|v| format!("clock {which} {v}")));
            }
            "--doc-sig" => a.bends.extend(it.next().map(|v| format!("sig {v}"))),
            "--audio-tap" => {
                a.line = match it.next().as_deref() {
                    Some("line") => true,
                    Some("digital") => false,
                    v => {
                        eprintln!("--audio-tap は digital か line（{v:?}）");
                        std::process::exit(2);
                    }
                }
            }
            "-h" | "--help" => {
                println!(
                    "fc85 [--boot boot.bin] [--char char.bin] [--cart cart.bin] [--no-doc] \
                     [--scale N] [--frames N] [--headless] [--dump out.ppm] [--dump-at F,F..] [--wav out.wav] [--trace N] \
                     [--pad-script FILE] [--doc-clock HZ] \
                     [--dsp-clock HZ] [--adpcm-clock HZ] [--audio-clock HZ] [--video-clock HZ] [--midi-list] [--midi-in N] [--midi-out N] [--char-bend SPEC] [--bend CMD] [--bend-script FILE] \
                     [--audio-tap digital|line] [--doc-sig both|one] [--audio-in FILE.wav] [--audio-in-device N|default] [--audio-list] [--no-gamepad]\n\
                     --bend CMD：mode normal|bend|toggle、char A1=A2,D6=0,D2=Z、clock doc|dsp|adpcm|audio HZ|off、\
                     clock video HZ|0|off（映像系の 21.47727 MHz、0 で止める）、\
                     fifo dac|dsp_in|dsp_out|adpcm empty=0|1|open full=0|1|open、sig both|one、dram unit N|temp C|refresh hsync|auto、\
                     docbus A3=0,A4=A5,D7=1,A9=Z（DOC のバスのテストピン。normal で戻す）、factory\n\
                     画面は機械の外のテレビの模型を通す（同期と色の引込み。工場出荷のクロックでは画面の配列そのまま）"
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

/// クロックのつまみ：`clock` の名前、表示の名前、基板の発振器の周波数。
const KNOBS: [(&str, &str, f64); 5] = [
    ("doc", "DOC の CLK", 8e6),
    ("video", "映像のクロック", 21_477_272.727),
    ("dsp", "DSP の CLKIN", 20e6),
    ("adpcm", "ADPCM の XT", 384e3),
    ("audio", "音声クロック", 8e6),
];

/// つまみ `i` の今の周波数（Hz。0 は止まっている）。
fn knob_hz(c: &Console, i: usize) -> f64 {
    let k = &c.sys.bends.clocks;
    let per = |p: Option<u64>| p.map(|p| UNITS_PER_SECOND as f64 / p as f64);
    match i {
        0 => per(k.doc),
        1 => k.video.map(|m| m as f64 / 1000.0),
        2 => per(k.dsp),
        3 => per(k.adpcm),
        _ => per(k.audio),
    }
    .unwrap_or(KNOBS[i].2)
}

fn fmt_hz(f: f64) -> String {
    if f >= 1e6 {
        format!("{:.4} MHz", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.3} kHz", f / 1e3)
    } else {
        format!("{f:.2} Hz")
    }
}

/// 窓を開けたときに出すキーの案内。
const KEY_HELP: &str = "キー：矢印、Z（A）、X（B）、C（C）、Enter（START）が釦（ゲームパッドも読む）、F2 で Bend モード、F3 でつまみ（クロック）を選ぶ、\
PageUp・PageDown（か ]・[）で回す（Shift で細かく）、Home（か \\）で戻す、F5 で保存、F9 で戻す、Esc で終わる";
/// F3 で出す回し方の案内。
const KNOB_HELP: &str =
    "PageUp・PageDown（か ]・[）で半音ずつ、Shift で 1/8 半音、Home（か \\）で戻す";

/// つまみ `i` を `semitones` 回す（`None` は基板の発振器に戻す）。止まっていれば基板の周波数から回す。
/// `pos` はつまみの位置（基板から何半音か）。基板の周期は整数の時刻単位に丸まるので、読み直した
/// 周波数から回すと誤差がたまる。位置から周波数を決め、台本などで半音の半分より大きく替わって
/// いたときだけ、今の周波数から位置を読み直す。
fn turn_knob(c: &mut Console, i: usize, pos: &mut f64, semitones: Option<f64>) {
    let (name, label, f0) = KNOBS[i];
    let lo = 12.0 * (1.0 / f0).log2();
    let cmd = match semitones {
        Some(s) => {
            let now = knob_hz(c, i);
            if now <= 0.0 {
                *pos = 0.0;
            } else {
                let p = 12.0 * (now / f0).log2();
                if (p - *pos).abs() > 0.5 {
                    *pos = p;
                }
                *pos = (*pos + s).clamp(lo, 24.0);
            }
            format!("clock {name} {}", f0 * 2f64.powf(*pos / 12.0))
        }
        None => {
            *pos = 0.0;
            format!("clock {name} off")
        }
    };
    if c.sys.bends.mode != Mode::Bend {
        let _ = c.sys.bend("mode bend");
    }
    match c.sys.bend(&cmd) {
        Ok(()) => {
            let f = knob_hz(c, i);
            if f > 0.0 {
                // -0.00 と出さない
                let p = if pos.abs() < 0.005 { 0.0 } else { *pos };
                eprintln!("つまみ：{label} {}（基板から {p:+.2} 半音）", fmt_hz(f));
            } else {
                eprintln!("つまみ：{label} 止まっている");
            }
        }
        Err(e) => eprintln!("つまみ：{e}"),
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

/// 既定の置き場所（リポジトリの firmware のビルド結果）から探す。今のフォルダとその上 2 段、
/// 実行ファイルのフォルダとその上 4 段（`emulator/target/release/` から根まで）の順。
fn default_path(rel: &str) -> Option<String> {
    let mut bases: Vec<std::path::PathBuf> = ["", "..", "../.."]
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
    if let Ok(exe) = std::env::current_exe() {
        bases.extend(exe.ancestors().skip(1).take(5).map(|p| p.to_path_buf()));
    }
    bases
        .iter()
        .map(|b| b.join(rel))
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
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
        None => eprintln!(
            "Boot ROM が見つからない。リポジトリの根で `py firmware/boot/build.py` を実行するか、\
             --boot で指定する。0xFF のまま始めるので、68000 はすぐ二重バスフォルトで止まる"
        ),
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

/// テレビの絵（`Tv::picture`）から、窓に出す範囲を 0RGB の u32 にする。
fn view(c: &Console, tv: &Tv, pic: &mut Vec<u32>, out: &mut [u32]) {
    tv.picture(&c.sys, pic);
    for y in 0..VIEW_H {
        for x in 0..VIEW_W {
            let (sx, sy) = (VIEW_X + x, VIEW_Y + y);
            out[y * VIEW_W + x] = if sx < RASTER_W && sy < RASTER_H {
                pic[sy * RASTER_W + sx]
            } else {
                0
            };
        }
    }
}

/// テレビの状態が変わったら出す（ホストの表示）。工場出荷と同じに映る状態は、崩れた状態を出した後だけ出す
/// （起動の「同期なし」から同期したときは黙る）。
fn report_tv(c: &Console, tv: &Tv, seen: &mut (String, bool)) {
    let st = tv.status(&c.sys);
    let s = st.summary();
    if seen.0 != s {
        if !st.normal() || seen.1 {
            eprintln!("{s}");
            seen.1 = true;
        }
        seen.0 = s;
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

/// 鳴らす音（`--audio-tap`）を取り出し、もう一方は捨てる。
fn take_audio(c: &mut Console, line: bool) -> Vec<(i16, i16)> {
    let (keep, drop) = if line {
        (&mut c.sys.line_out, &mut c.sys.audio_out)
    } else {
        (&mut c.sys.audio_out, &mut c.sys.line_out)
    };
    drop.clear();
    std::mem::take(keep)
}

/// 仮想機の音声をリングに入れる。遅れすぎたら古いものを捨てる（ホストの安全層。仮想機は変えない）。
fn push_audio(c: &mut Console, ring: &Ring, line: bool) {
    let mut q = ring.lock().unwrap();
    q.extend(take_audio(c, line));
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

/// 出した警告の数（`report`）。
#[derive(Default)]
struct Seen {
    bus_errors: u64,
    doc_rule: u64,
    acrtc_rule: u64,
    halted: bool,
    bridge: u64,
    /// ブリッジの時間切れを最後に出したフレーム。
    bridge_frame: Option<u64>,
}

/// ブリッジの時間切れ（DR-079）を出す間（フレーム）。クロックを下げている間は毎フレーム増えるので、まとめる。
const BRIDGE_REPORT_FRAMES: u64 = 120;

/// バスの警告が増えたら出す（ホストの表示。仮想機は変えない）。`frame` はホストのフレームの数。
fn report(c: &Console, seen: &mut Seen, frame: u64) {
    let l = &c.sys.log;
    let halted = c.cpu.state == RunState::Halted;
    if halted && !seen.halted {
        eprintln!(
            "68000 が止まった（二重バスフォルト。PC {:06X}）。リセットまで動かない",
            c.cpu.pc
        );
    }
    seen.halted = halted;
    if l.bus_errors != seen.bus_errors {
        if let Some((addr, w)) = l.last_bus_error {
            eprintln!(
                "バスエラー {} 回（最後は {addr:06X} の{}）",
                l.bus_errors,
                if w { "書込み" } else { "読出し" }
            );
        }
        seen.bus_errors = l.bus_errors;
    }
    if l.bridge_timeouts != seen.bridge
        && seen
            .bridge_frame
            .is_none_or(|f| frame >= f + BRIDGE_REPORT_FRAMES)
    {
        if let Some((addr, w)) = l.last_bridge_timeout {
            eprintln!(
                "ブリッジの時間切れ {} 回（前の表示から +{}。最後は {addr:06X} の{}。DR-079）",
                l.bridge_timeouts,
                l.bridge_timeouts - seen.bridge,
                if w { "書込み" } else { "読出し" }
            );
        }
        seen.bridge = l.bridge_timeouts;
        seen.bridge_frame = Some(frame);
    }
    // Bend モードでは規約違反を警告しない（実行はどちらのモードでも同じ。DR-065）
    if l.doc_rule_warnings != seen.doc_rule && c.sys.bends.mode == Mode::Normal {
        if let Some(addr) = l.last_doc_rule {
            eprintln!(
                "DOC の窓の規約（DR-030）に反した命令 {} 回（最後は {addr:06X}）",
                l.doc_rule_warnings
            );
        }
        seen.doc_rule = l.doc_rule_warnings;
    }
    // ACRTC の FIFO の規約（DR-096）：満杯の書込み FIFO への書込みと、空の読出し FIFO の読出し（DR-121）
    let (over, under) = c.sys.ports.acrtc.acrtc.error_counts();
    if over + under != seen.acrtc_rule && c.sys.bends.mode == Mode::Normal {
        eprintln!(
            "ACRTC の FIFO の規約（DR-096）に反したアクセス {} 回（満杯への書込み {over}、空からの読出し {under}）",
            over + under
        );
        seen.acrtc_rule = over + under;
    }
}

fn main() {
    let a = parse_args();
    let mut c = build_console(&a);
    let mut seen = Seen::default();
    if a.trace > 0 {
        trace(&mut c, a.trace);
    }
    let mut buf = vec![0u32; VIEW_W * VIEW_H];
    let tv = Tv::default();
    let mut pic = Vec::new();
    // 最初の状態（表示が動き出す前の「同期なし」）は出さない
    let mut tv_seen = (tv.status(&c.sys).summary(), false);
    let mut line_in = match (&a.audio_in, &a.audio_in_device) {
        (Some(p), _) => audio_in::Source::file(p),
        (None, Some(d)) => audio_in::Source::device(d),
        (None, None) => None,
    };

    if a.headless {
        let n = a.frames.unwrap_or(60);
        let mut pcm = Vec::new();
        for i in 0..n {
            run_bend_script(&mut c, &a, i);
            if let Some(&(_, v)) = a.pad_script.iter().rev().find(|e| e.0 <= i) {
                c.sys.set_ports(v, 0);
            }
            if let Some(s) = &mut line_in {
                s.feed(&mut c);
            }
            c.run_frame();
            report(&c, &mut seen, i);
            report_tv(&c, &tv, &mut tv_seen);
            if a.dump_at.contains(&(i + 1))
                && let Some(p) = &a.dump
            {
                view(&c, &tv, &mut pic, &mut buf);
                dump_ppm(&numbered(p, i + 1), &buf);
            }
            let mut s = take_audio(&mut c, a.line);
            if a.wav.is_some() {
                pcm.append(&mut s);
            }
        }
        view(&c, &tv, &mut pic, &mut buf);
        if let Some(p) = &a.dump {
            dump_ppm(p, &buf);
        }
        if let Some(p) = &a.wav {
            write_wav(p, &pcm);
        }
        println!(
            "{} フレーム、CPU {} クロック、バスエラー {}、ブリッジの時間切れ {}",
            c.sys.ports.acrtc.video.frames,
            c.cpu.clock,
            c.sys.log.bus_errors,
            c.sys.log.bridge_timeouts
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
    let mut pads = gamepad::Pads::open(a.gamepad);
    let mut frames = 0u64;
    let mut knob = 0usize;
    let mut knob_pos = [0.0f64; KNOBS.len()];
    eprintln!("{KEY_HELP}");
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
        if w.is_key_pressed(minifb::Key::F3, minifb::KeyRepeat::No) {
            knob = (knob + 1) % KNOBS.len();
            eprintln!(
                "つまみ：{}（今 {}）。{KNOB_HELP}",
                KNOBS[knob].1,
                fmt_hz(knob_hz(&c, knob))
            );
        }
        let fine = w.is_key_down(minifb::Key::LeftShift) || w.is_key_down(minifb::Key::RightShift);
        let step = if fine { 0.125 } else { 1.0 };
        let pressed = |k: &[minifb::Key], r| k.iter().any(|&k| w.is_key_pressed(k, r));
        if pressed(
            &[minifb::Key::PageUp, minifb::Key::RightBracket],
            minifb::KeyRepeat::Yes,
        ) {
            turn_knob(&mut c, knob, &mut knob_pos[knob], Some(step));
        }
        if pressed(
            &[minifb::Key::PageDown, minifb::Key::LeftBracket],
            minifb::KeyRepeat::Yes,
        ) {
            turn_knob(&mut c, knob, &mut knob_pos[knob], Some(-step));
        }
        if pressed(
            &[minifb::Key::Home, minifb::Key::Backslash],
            minifb::KeyRepeat::No,
        ) {
            turn_knob(&mut c, knob, &mut knob_pos[knob], None);
        }
        run_bend_script(&mut c, &a, frames);
        let gp = pads.read();
        c.sys.set_ports(pad(&w) | gp[0], gp[1]);
        // MIDI IN はフレームの頭に線に来たとする。MIDI OUT は仮想機の時刻の間隔のまま送る（DR-126）
        let start = c.cpu.clock;
        c.sys.midi_receive(start, &midi.take_received());
        if let Some(s) = &mut line_in {
            s.feed(&mut c);
        }
        c.run_frame();
        let sent: Vec<(u64, u8)> = c.sys.midi_out.drain(..).collect();
        midi.send(&sent, start, std::time::Instant::now());
        push_audio(&mut c, &ring, a.line);
        pace(audio.is_some(), &ring, &mut next);
        view(&c, &tv, &mut pic, &mut buf);
        if w.update_with_buffer(&buf, VIEW_W, VIEW_H).is_err() {
            break;
        }
        report(&c, &mut seen, frames);
        report_tv(&c, &tv, &mut tv_seen);
        frames += 1;
        if a.frames.is_some_and(|n| frames >= n) {
            break;
        }
    }
    if let Some(p) = &a.dump {
        dump_ppm(p, &buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_knob_returns_to_the_board_clock_without_drift() {
        // 基板の周期は整数の時刻単位に丸まる。半音ずつ上げ下げしても、位置から決めるので誤差がたまらない
        for (i, &(_, label, f0)) in KNOBS.iter().enumerate() {
            let mut c = Console::new();
            let mut pos = 0.0;
            for _ in 0..7 {
                turn_knob(&mut c, i, &mut pos, Some(1.0));
            }
            for _ in 0..7 {
                turn_knob(&mut c, i, &mut pos, Some(-1.0));
            }
            assert_eq!(pos, 0.0, "{label}");
            let cents = 1200.0 * (knob_hz(&c, i) / f0).log2();
            assert!(cents.abs() < 0.5, "{label}：{cents:.3} セント");
            // 上限（基板の 4 倍）で止まり、Home で基板の発振器に戻る
            for _ in 0..30 {
                turn_knob(&mut c, i, &mut pos, Some(1.0));
            }
            assert_eq!(pos, 24.0, "{label}");
            turn_knob(&mut c, i, &mut pos, None);
            assert_eq!(pos, 0.0, "{label}");
            assert_eq!(knob_hz(&c, i), f0, "{label}");
        }
    }
}
