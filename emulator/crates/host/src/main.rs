//! fc85 のホスト（仕様書 §12 の host 層。DR-055）。
//!
//! 仮想機（board 層の `Console`）を 1 フレームずつ進め、画面を窓に、音声をオーディオ装置に出し、
//! キーボードを PORT_A に渡す。ホストの安全層（音量の制限、表示の保持）は仮想機の挙動を変えない。
//!
//! 使い方：`fc85 [--boot boot.bin] [--char char.bin] [--cart cart.bin] [--no-doc] [--scale N]
//! [--frames N] [--headless] [--dump out.ppm]`

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use fc85_board::console::{AUDIO_RATE, BOOT_ROM_BYTES, Console};
use fc85_board::video::{RASTER_H, RASTER_W, rgb888};

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
    headless: bool,
    dump: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args {
        boot: None,
        chars: None,
        cart: None,
        doc: true,
        scale: 3,
        frames: None,
        headless: false,
        dump: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "--boot" => a.boot = it.next(),
            "--char" => a.chars = it.next(),
            "--cart" => a.cart = it.next(),
            "--no-doc" => a.doc = false,
            "--scale" => a.scale = it.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            "--frames" => a.frames = it.next().and_then(|v| v.parse().ok()),
            "--headless" => a.headless = true,
            "--dump" => a.dump = it.next(),
            "-h" | "--help" => {
                println!(
                    "fc85 [--boot boot.bin] [--char char.bin] [--cart cart.bin] [--no-doc] \
                     [--scale N] [--frames N] [--headless] [--dump out.ppm]"
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

/// 仮想機の音声をリングに入れる。遅れすぎたら古いものを捨てる（ホストの安全層。仮想機は変えない）。
fn push_audio(c: &mut Console, ring: &Ring) {
    let mut q = ring.lock().unwrap();
    q.extend(c.sys.audio_out.drain(..));
    let max = AUDIO_RATE as usize / 5;
    while q.len() > max {
        q.pop_front();
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

fn main() {
    let a = parse_args();
    let mut c = build_console(&a);
    let mut buf = vec![0u32; VIEW_W * VIEW_H];

    if a.headless {
        let n = a.frames.unwrap_or(60);
        for _ in 0..n {
            c.run_frame();
            c.sys.audio_out.clear();
        }
        view(&c, &mut buf);
        if let Some(p) = &a.dump {
            dump_ppm(p, &buf);
        }
        println!(
            "{} フレーム、CPU {} クロック、バスエラー {}",
            c.sys.ports.acrtc.video.frames, c.cpu.clock, c.sys.log.bus_errors
        );
        return;
    }

    let ring: Ring = Arc::new(Mutex::new(VecDeque::new()));
    let _audio = open_audio(ring.clone());
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
    w.set_target_fps(60);
    let mut frames = 0u64;
    while w.is_open() && !w.is_key_down(minifb::Key::Escape) {
        c.sys.set_ports(pad(&w), 0);
        c.run_frame();
        push_audio(&mut c, &ring);
        view(&c, &mut buf);
        if w.update_with_buffer(&buf, VIEW_W, VIEW_H).is_err() {
            break;
        }
        frames += 1;
        if a.frames.is_some_and(|n| frames >= n) {
            break;
        }
    }
    if let Some(p) = &a.dump {
        dump_ppm(p, &buf);
    }
}
