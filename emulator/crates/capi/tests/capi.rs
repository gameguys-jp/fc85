//! C の ABI（DR-104）の試験。C から呼ぶのと同じ関数を、同じ形（生のポインタ）で呼ぶ。
//! Boot ROM・文字 ROM（`py firmware/boot/build.py`・`py firmware/charrom/build.py`）がなければ飛ばす。

use std::ffi::{CStr, CString};
use std::path::PathBuf;

use fc85_api::{Area, Config, Machine};
use fc85emu::*;

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(rel)
}

fn roms() -> Option<(Vec<u8>, Vec<u8>)> {
    let boot = std::fs::read(repo("firmware/boot/build/boot.bin")).ok();
    let chars = std::fs::read(repo("firmware/charrom/build/char.bin")).ok();
    match (boot, chars) {
        (Some(b), Some(c)) => Some((b, c)),
        _ => {
            eprintln!("Boot ROM か文字 ROM がない（firmware を組む）。飛ばす");
            None
        }
    }
}

fn last_error() -> String {
    // SAFETY: 0 で終わる文字列を返す
    unsafe { CStr::from_ptr(fc85_last_error()) }
        .to_string_lossy()
        .into_owned()
}

/// 仮想機を作る（ROM を渡す）。
fn new(boot: &[u8], chars: &[u8], flags: u32) -> *mut Fc85 {
    // SAFETY: 長さどおりのバイト列
    let m = unsafe {
        fc85_new(
            boot.as_ptr(),
            boot.len(),
            chars.as_ptr(),
            chars.len(),
            flags,
        )
    };
    assert!(!m.is_null(), "fc85_new: {}", last_error());
    m
}

fn screen(m: *mut Fc85, area: i32) -> Vec<u32> {
    let n = (fc85_screen_width(area) * fc85_screen_height(area)) as usize;
    let mut out = vec![0u32; n];
    // SAFETY: out は n 個
    let r = unsafe { fc85_screen(m, area, out.as_mut_ptr(), out.len()) };
    assert_eq!(r, n as i32, "fc85_screen: {}", last_error());
    out
}

fn drain_audio(m: *mut Fc85) -> Vec<i16> {
    let mut all = Vec::new();
    let mut buf = vec![0i16; 2 * 1000];
    loop {
        // SAFETY: buf は 2×1000 個
        let n = unsafe { fc85_audio(m, buf.as_mut_ptr(), 1000) };
        assert!(n >= 0, "fc85_audio: {}", last_error());
        all.extend_from_slice(&buf[..2 * n as usize]);
        if n < 1000 {
            return all;
        }
    }
}

fn call(m: *mut Fc85, req: &str) -> String {
    let c = CString::new(req).unwrap();
    // SAFETY: 0 で終わる文字列
    let p = unsafe { fc85_call(m, c.as_ptr()) };
    assert!(!p.is_null(), "fc85_call: {}", last_error());
    // SAFETY: fc85_call が返した
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    // SAFETY: fc85_call が返した
    unsafe { fc85_free_string(p) };
    s
}

#[test]
fn constants_and_sizes() {
    assert_eq!(fc85_abi_version(), ABI_VERSION);
    assert_eq!(fc85_audio_rate(), 48_000);
    // 2CLK（21.47727 MHz の 1/4）のドット 342 × ライン 262（DR-002）
    let fps = 21_477_272.727 / 4.0 / (342.0 * 262.0);
    assert!(
        (fc85_frame_rate() - fps).abs() < 1e-3,
        "{}",
        fc85_frame_rate()
    );
    assert_eq!((fc85_screen_width(0), fc85_screen_height(0)), (256, 192));
    assert_eq!((fc85_screen_width(1), fc85_screen_height(1)), (320, 240));
    assert_eq!((fc85_screen_width(2), fc85_screen_height(2)), (342, 262));
    assert_eq!(fc85_screen_width(3), 0);
    // SAFETY: 静的な文字列
    let v = unsafe { CStr::from_ptr(fc85_version()) }.to_str().unwrap();
    assert_eq!(v, env!("CARGO_PKG_VERSION"));
}

#[test]
fn errors_are_returned_not_raised() {
    let mut px = [0u32; 4];
    // SAFETY: NULL の仮想機は誤りとして返す
    assert_eq!(
        unsafe { fc85_screen(std::ptr::null_mut(), 1, px.as_mut_ptr(), 4) },
        -1
    );
    assert!(last_error().contains("NULL"), "{}", last_error());
    // SAFETY: NULL は何もしない
    unsafe { fc85_free(std::ptr::null_mut()) };
    // ROM なしで作る（68000 はすぐ止まる）。PC の側は止まらず、フレームも画面も進む
    // SAFETY: NULL の ROM は「なし」
    let m = unsafe { fc85_new(std::ptr::null(), 0, std::ptr::null(), 0, 0) };
    assert!(!m.is_null(), "{}", last_error());
    // SAFETY: 生きている仮想機
    unsafe {
        assert_eq!(fc85_run_frames(m, 10), 0);
        assert_eq!(
            fc85_screen(m, 1, px.as_mut_ptr(), 4),
            -1,
            "小さすぎる out は誤り"
        );
        assert!(last_error().contains("76800"), "{}", last_error());
        assert_eq!(fc85_screen(m, 9, px.as_mut_ptr(), 4), -1);
        assert_eq!(fc85_peek(m, 99, 0, px.as_mut_ptr().cast(), 1), -1);
        assert_eq!(fc85_load_state(m, b"xx".as_ptr(), 2), -1);
        assert!(last_error().contains("印"), "{}", last_error());
        fc85_free(m);
    }
}

#[test]
fn same_picture_and_sound_as_the_api() {
    let Some((boot, chars)) = roms() else { return };
    let cart = std::fs::read(repo("sdk/samples/game/build/game.bin")).ok();
    let m = new(&boot, &chars, 0);
    if let Some(c) = &cart {
        // SAFETY: 長さどおり
        assert_eq!(
            unsafe { fc85_load_cart(m, c.as_ptr(), c.len()) },
            0,
            "{}",
            last_error()
        );
    }
    // SAFETY: 生きている仮想機
    unsafe {
        assert_eq!(fc85_run_frames(m, 120), 0);
        assert_eq!(fc85_frame(m), 120);
    }
    let pic = screen(m, 1);
    let pcm = drain_audio(m);
    // 約 801 組／フレーム
    let per = pcm.len() as f64 / 2.0 / 120.0;
    assert!((per - 48_000.0 / fc85_frame_rate()).abs() < 2.0, "{per}");

    let mut api = Machine::new(Config {
        boot: Some(boot),
        chars: Some(chars),
        cart,
        doc: true,
        bends: Vec::new(),
        line: true,
    })
    .unwrap();
    api.run_frames(120);
    let mut want = Vec::new();
    api.screen_xrgb(Area::View, &mut want);
    assert_eq!(pic, want, "画面が窓口と違う");
    let want: Vec<i16> = api
        .take_audio(false)
        .into_iter()
        .flat_map(|(l, r)| [l, r])
        .collect();
    assert_eq!(pcm, want, "音が窓口と違う");
    assert!(pic.iter().any(|&p| p != pic[0]), "画面が一色");
    // SAFETY: 生きている仮想機
    unsafe { fc85_free(m) };
}

#[test]
fn state_round_trip_and_memory() {
    let Some((boot, chars)) = roms() else { return };
    let m = new(&boot, &chars, 0);
    // SAFETY: 生きている仮想機。返された領域は対の関数で返す
    unsafe {
        assert_eq!(fc85_run_frames(m, 60), 0);
        drain_audio(m);
        let mut len = 0usize;
        let st = fc85_save_state(m, &mut len);
        assert!(!st.is_null(), "{}", last_error());
        assert!(len > 8 && len < 4 << 20, "{len}");
        let saved = std::slice::from_raw_parts(st, len).to_vec();
        fc85_free_bytes(st, len);

        let run = |m| {
            assert_eq!(fc85_set_input(m, 0x08, 0), 0);
            assert_eq!(fc85_run_frames(m, 30), 0);
            (screen(m, 2), drain_audio(m), fc85_frame(m))
        };
        let a = run(m);
        assert_eq!(
            fc85_load_state(m, saved.as_ptr(), saved.len()),
            0,
            "{}",
            last_error()
        );
        assert_eq!(fc85_frame(m), 60);
        let b = run(m);
        assert_eq!(a.2, b.2, "フレーム");
        assert!(a.0 == b.0, "状態を戻して同じ入力で進めた画面が違う");
        assert!(a.1 == b.1, "状態を戻して同じ入力で進めた音が違う");

        // メモリ（Main RAM の APP_RAM）
        let data = [0x12u8, 0x34, 0x56];
        assert_eq!(
            fc85_poke(m, 0, 0x10_0800, data.as_ptr(), 3),
            0,
            "{}",
            last_error()
        );
        let mut back = [0u8; 3];
        assert_eq!(fc85_peek(m, 0, 0x10_0800, back.as_mut_ptr(), 3), 0);
        assert_eq!(back, data);
        // Boot ROM の版の語（BIOS_INFO）
        let mut v = [0u8; 2];
        assert_eq!(fc85_peek(m, 0, 0x480, v.as_mut_ptr(), 2), 0);
        assert_eq!(u16::from_be_bytes(v), 2, "Boot ROM の版");
        fc85_free(m);
    }
}

#[test]
fn json_requests_go_to_the_same_machine() {
    let Some((boot, chars)) = roms() else { return };
    let m = new(&boot, &chars, FLAG_NO_DOC);
    // SAFETY: 生きている仮想機
    unsafe { assert_eq!(fc85_run_frames(m, 5), 0) };
    let r = call(m, r#"{"jsonrpc":"2.0","id":7,"method":"regs"}"#);
    assert!(r.contains(r#""id":7"#) && r.contains(r#""frame":5"#), "{r}");
    let r = call(m, r#"{"jsonrpc":"2.0","id":8,"method":"doc"}"#);
    assert!(r.contains(r#""result":null"#), "子基板なし：{r}");
    let r = call(m, r#"{"jsonrpc":"2.0","id":9,"method":"nothing"}"#);
    assert!(r.contains(r#""error""#), "{r}");
    // SAFETY: NULL の要求は誤り
    assert!(unsafe { fc85_call(m, std::ptr::null()) }.is_null());
    // SAFETY: 生きている仮想機
    unsafe { fc85_free(m) };
}

#[test]
fn midi_goes_in_and_comes_out_with_times() {
    // DR-126：来た byte を送り返す小さな Boot ROM（RDRF と TDRE を見る）
    let mut boot = vec![0xFFu8; 0x10000];
    let code: [u16; 25] = [
        0x13FC, 0x0003, 0x0026, 0x0014, 0x13FC, 0x0015, 0x0026, 0x0014, // ACIA：$03、$15
        0x0839, 0x0000, 0x0026, 0x0014, 0x67F6, // RDRF を待つ
        0x1039, 0x0026, 0x0016, // MOVE.B MIDI_DATA,D0
        0x0839, 0x0001, 0x0026, 0x0014, 0x67F6, // TDRE を待つ
        0x13C0, 0x0026, 0x0016, // MOVE.B D0,MIDI_DATA
        0x60DE,
    ];
    boot[..8].copy_from_slice(&[0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00]);
    for (i, w) in code.iter().enumerate() {
        boot[0x100 + 2 * i..0x102 + 2 * i].copy_from_slice(&w.to_be_bytes());
    }
    let m = new(&boot, &[], FLAG_NO_DOC);
    let msg = [0x90u8, 64, 1, 0x80, 64, 0];
    let mut out = [0u8; 4];
    let mut times = [0u64; 4];
    // SAFETY: 生きている仮想機。長さどおりの配列
    unsafe {
        assert_eq!(fc85_run_frames(m, 1), 0);
        let t0 = fc85_time_us(m);
        assert!(t0 > 16_000);
        assert_eq!(fc85_midi_in(m, msg.as_ptr(), msg.len(), 0), 0);
        assert_eq!(fc85_run_frames(m, 1), 0);
        // 4 個ずつ取り出し、残りは次に
        assert_eq!(fc85_midi_out(m, out.as_mut_ptr(), times.as_mut_ptr(), 4), 4);
        assert_eq!(out, [0x90, 64, 1, 0x80]);
        assert!(times[0] >= t0 + 640 && times[0] < t0 + 660, "{times:?}");
        assert!(times.windows(2).all(|w| w[1] - w[0] >= 320));
        assert_eq!(
            fc85_midi_out(m, out.as_mut_ptr(), std::ptr::null_mut(), 4),
            2
        );
        assert_eq!(out[..2], [64, 0]);
        assert_eq!(
            fc85_midi_out(m, out.as_mut_ptr(), std::ptr::null_mut(), 4),
            0
        );
        // NULL の out で取り出すものがあれば誤り
        assert_eq!(fc85_midi_in(m, msg.as_ptr(), 1, 0), 0);
        assert_eq!(fc85_run_frames(m, 1), 0);
        assert_eq!(
            fc85_midi_out(m, std::ptr::null_mut(), std::ptr::null_mut(), 4),
            -1
        );
        assert!(last_error().contains("NULL"));
        fc85_free(m);
    }
}

/// fc85emu.h の関数の宣言と、Rust の側で出している関数が同じ名前の組か。
#[test]
fn header_declares_every_export() {
    let h =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/include/fc85emu.h")).unwrap();
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();
    let mut declared: Vec<String> = h
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim_start().starts_with('*'))
        .filter_map(|l| {
            let head = &l[..l.find('(')?];
            Some(head[head.rfind("fc85_")?..].to_string())
        })
        .collect();
    let mut exported: Vec<String> = src
        .split("#[unsafe(no_mangle)]")
        .skip(1)
        .filter_map(|s| {
            let i = s.find("fn ")?;
            let rest = &s[i + 3..];
            Some(rest[..rest.find('(')?].to_string())
        })
        .collect();
    declared.sort();
    exported.sort();
    assert_eq!(declared, exported);
    for (name, v) in [
        ("FC85_ABI_VERSION", ABI_VERSION),
        ("FC85_NO_DOC", FLAG_NO_DOC),
        ("FC85_DIGITAL_AUDIO", FLAG_DIGITAL_AUDIO),
    ] {
        let line = h
            .lines()
            .find(|l| l.starts_with(&format!("#define {name} ")))
            .unwrap();
        let num = line
            .split_whitespace()
            .nth(2)
            .unwrap()
            .trim_end_matches('u');
        let n = u32::from_str_radix(
            num.trim_start_matches("0x"),
            if num.starts_with("0x") { 16 } else { 10 },
        )
        .unwrap();
        assert_eq!(n, v, "{name}");
    }
    // PAD のビットは窓口の PAD_BITS と同じ
    for (name, bit) in fc85_api::PAD_BITS {
        let def = format!("#define FC85_PAD_{name} ");
        let line = h
            .lines()
            .find(|l| l.starts_with(&def))
            .unwrap_or_else(|| panic!("{def}"));
        let num = line
            .split_whitespace()
            .nth(2)
            .unwrap()
            .trim_end_matches('u');
        assert_eq!(
            u16::from_str_radix(num.trim_start_matches("0x"), 16).unwrap(),
            bit,
            "{name}"
        );
    }
}
