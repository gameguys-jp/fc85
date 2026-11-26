//! 本体（段階 3）：firmware/ でビルドした Boot ROM と文字 ROM で起動する。
//! ROM はリポジトリに入れない（firmware/*/build/）。ないときは試験を飛ばす。
//! 作るには `py firmware/charrom/build.py` と `py firmware/boot/build.py`。

use fc85_board::console::{BOOT_ROM_BYTES, Console};
use fc85_chips::hd68000::RunState;

fn rom(rel: &str) -> Option<Vec<u8>> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(rel);
    std::fs::read(p).ok()
}

fn boot(doc: bool) -> Option<Console> {
    let boot = rom("firmware/boot/build/boot.bin")?;
    let mut c = Console::new();
    c.sys.ports.doc = doc;
    let n = boot.len().min(BOOT_ROM_BYTES);
    c.sys.boot_rom[..n].copy_from_slice(&boot[..n]);
    if let Some(ch) = rom("firmware/charrom/build/char.bin") {
        let n = ch.len().min(c.sys.ports.char_rom.len());
        c.sys.ports.char_rom[..n].copy_from_slice(&ch[..n]);
    }
    c.power_on();
    Some(c)
}

fn check(doc: bool) {
    let Some(mut c) = boot(doc) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    let mut audio = Vec::new();
    for _ in 0..60 {
        c.run_frame();
        audio.append(&mut c.sys.audio_out);
    }
    assert_eq!(c.cpu.state, RunState::Running, "PC {:06X}", c.cpu.pc);
    // バスエラーは、カートリッジの有無を調べる 0x400000 の読出しの 1 回だけ（bios-abi.md）
    assert_eq!(
        (c.sys.log.bus_errors, c.sys.log.last_bus_error),
        (1, Some((0x40_0000, false)))
    );
    assert_eq!(c.sys.log.doc_rule_warnings, 0);
    let v = &c.sys.ports.acrtc.video;
    assert!(c.sys.ports.acrtc.acrtc.display_running());
    assert!(v.frames >= 55, "フレーム {}", v.frames);
    // 画面に 2 色以上ある
    let first = v.frame[0];
    assert!(v.frame.iter().any(|&p| p != first), "画面が一色");
    // VBlank の割込みを受けている
    assert!(c.sys.log.iacks >= 50, "IACK {}", c.sys.log.iacks);
    // 音が出ている（無音ではない）
    assert!(audio.len() > 40_000);
    assert!(
        audio
            .iter()
            .any(|&(l, r)| l != audio[0].0 || r != audio[0].1),
        "無音"
    );
}

#[test]
fn the_boot_rom_runs_with_the_doc_board() {
    check(true);
}

#[test]
fn the_boot_rom_runs_without_the_doc_board() {
    check(false);
}

#[test]
fn a_snapshot_of_the_running_bios_resumes_identically() {
    // 動いている BIOS（DOC あり）を保存して戻し、同じ入力で 10 フレーム進めると、元と同じ状態になる
    let Some(mut a) = boot(true) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    for _ in 0..10 {
        a.run_frame();
    }
    a.sys.audio_out.clear();
    a.sys.line_out.clear();
    let json = serde_json::to_string(&a).unwrap();
    let mut b: Console = serde_json::from_str(&json).unwrap();
    for i in 0..10 {
        let pad = if i == 3 { 0x0010 } else { 0 };
        a.sys.set_ports(pad, 0);
        b.sys.set_ports(pad, 0);
        a.run_frame();
        b.run_frame();
    }
    assert_eq!(a.cpu, b.cpu);
    assert_eq!(a.sys.audio_out, b.sys.audio_out);
    // LINE OUT（アナログの段の f64 の状態もビット列で保存する）も同じ標本の列になる
    assert_eq!(a.sys.line_out, b.sys.line_out);
    assert!(!a.sys.line_out.is_empty());
    assert!(a == b, "保存して戻した後の状態が違う");
}

#[test]
fn the_sample_cartridge_starts_and_plays_adpcm() {
    // sdk/samples/game の SAMPLE 1：BIOS がカートリッジの見出しを見て入り、START で始まりの音を
    // Data Fabric から ADPCM へ送る（sdk/samples/game/README.md）
    let Some(cart) = rom("sdk/samples/game/build/game.bin") else {
        eprintln!("sdk/samples/game/build/game.bin がないので飛ばす");
        return;
    };
    let Some(mut c) = boot(false) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    c.sys.cart = Some(cart);
    c.power_on();
    for i in 0..100 {
        let pad = match i {
            40..44 => 0x0080, // START
            50..90 => 0x0004, // 左
            _ => 0,
        };
        c.sys.set_ports(pad, 0);
        c.run_frame();
    }
    // BIOS_WAIT_VBL の STOP で待っていることがある
    assert_ne!(c.cpu.state, RunState::Halted, "PC {:06X}", c.cpu.pc);
    // カートリッジがあるので、見出しを読んでもバスエラーにならない
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
    assert_eq!(c.sys.log.doc_rule_warnings, 0);
    let a = &c.sys.ports.adpcm_port;
    eprintln!("ADPCM の取込み {}、UNDERRUN {}", a.captures, a.underruns);
    assert!(a.captures > 2000, "ADPCM の取込み {}", a.captures);
}

/// `bios_frame`（$100600）。
fn bios_frame(c: &Console) -> u32 {
    let i = 0x600;
    u32::from_be_bytes(c.sys.main_ram[i..i + 4].try_into().unwrap())
}

#[test]
fn an_unclaimed_doc_interrupt_does_not_halt_the_bios() {
    // DR-076：DOC の割込みには基板の許可がない。ベンドで崩れたレジスタ（DR-074）や線（DR-073）が IE を
    // 立てても、BIOS の既定の処理が要求を取り（読めなければマスクを上げ）、VBlank を数え続ける。
    // 起動から入れる。DOC の CLK が約 1.1 MHz より遅いと 68000 の DOC へのアクセスは時間切れになるが、
    // DTACK で終わる（DR-079）ので、BIOS は起動の音を鳴らしながら進む
    let spurious = |c: &Console| u16::from_be_bytes([c.sys.main_ram[0x658], c.sys.main_ram[0x659]]);
    // docbus A0=1：BIOS のレジスタの書込みが奇数のオシレータに入り、IE が立つ
    for bend in ["clock doc 10e3", "docbus A0=1"] {
        let Some(mut c) = boot(true) else {
            eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
            return;
        };
        c.sys.bend(bend).unwrap();
        c.sys.bend("mode bend").unwrap();
        for _ in 0..60 {
            c.run_frame();
        }
        let f = bios_frame(&c);
        for _ in 0..20 {
            c.run_frame();
        }
        assert!(
            bios_frame(&c) >= f + 19,
            "{bend}：BIOS が止まった（PC {:06X}）",
            c.cpu.pc
        );
        if bend.starts_with("docbus") {
            assert!(spurious(&c) > 0, "{bend}：DOC の割込みが来ていない");
        }
        assert_eq!(
            c.sys.log.bus_errors, 1,
            "{bend}：{:?}",
            c.sys.log.last_bus_error
        );
    }
}

#[test]
fn stopping_the_video_clock_does_not_halt_the_bios() {
    // DR-079：映像のクロックを止めると、描画中の 68000 の ACRTC へのアクセスは時間切れになり、DTACK で
    // 終わる（書込みは届かない）。VBlank が来ないので BIOS は待ち、戻すと数え続ける。止めた 0.25 秒の
    // 間に VRAM は崩れる（DR-077）
    let Some(mut c) = boot(true) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    for _ in 0..120 {
        c.run_frame();
    }
    let vram = c.sys.ports.acrtc.vram.0.clone();
    c.sys.bend("mode bend").unwrap();
    c.sys.bend("clock video 0").unwrap();
    let f = bios_frame(&c);
    for _ in 0..15 {
        c.run_frame();
    }
    assert!(bios_frame(&c) <= f + 1, "止めている間は VBlank が来ない");
    c.sys.bend("clock video off").unwrap();
    for _ in 0..30 {
        c.run_frame();
    }
    assert!(
        bios_frame(&c) >= f + 28,
        "BIOS が止まった（PC {:06X}）",
        c.cpu.pc
    );
    assert_eq!(c.sys.log.bus_errors, 1, "{:?}", c.sys.log.last_bus_error);
    let changed = vram
        .iter()
        .zip(&c.sys.ports.acrtc.vram.0)
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 100, "VRAM が崩れていない（{changed} 語）");
}

#[test]
fn the_patch_cartridge_runs_every_page() {
    // sdk/samples/patches の SAMPLE 3：仕様書 §11 の実験パッチを、ACRTC の DMA（port 1）、Data Fabric、
    // DOC の波形 RAM、ADPCM、文字 ROM（port 9）、Temporal RAM でつなぐ（sdk/samples/patches/README.md）
    let Some(cart) = rom("sdk/samples/patches/build/patches.bin") else {
        eprintln!("sdk/samples/patches/build/patches.bin がないので飛ばす");
        return;
    };
    let Some(mut c) = boot(true) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    c.sys.cart = Some(cart);
    c.power_on();
    let page = 0x10_0800; // v_page（main.s の RSSET APP_RAM の最初）
    let errors = 0x10_0800 + 28; // v_errors
    let word = |c: &Console, a: usize| {
        let ram = &c.sys.main_ram;
        let i = a - 0x10_0000;
        u16::from_be_bytes([ram[i], ram[i + 1]])
    };
    let run = |c: &mut Console, frames: usize, right_at: Option<usize>| {
        let mut audio = Vec::new();
        for i in 0..frames {
            let pad = if Some(i) == right_at { 0x0008 } else { 0 };
            c.sys.set_ports(pad, 0);
            c.run_frame();
            audio.append(&mut c.sys.audio_out);
        }
        audio
    };
    // 0. GRAPHICS AS WAVE：走査の 2 行が波形 RAM の頁 $20 に。GUARD で 0 の byte はない
    let audio = run(&mut c, 30, None);
    assert_eq!(word(&c, page), 0);
    let wave = &c.sys.ports.doc_board.wave[0x2000..0x2100];
    assert!(
        wave.iter().all(|&b| b != 0 && b & 1 == 1),
        "GUARD：奇数だけ"
    );
    let l0 = audio.len() / 2;
    assert!(audio[l0..].iter().any(|&(l, _)| l != audio[l0].0), "無音");
    // 1. IMAGE AS ADPCM
    let before = c.sys.ports.adpcm_port.captures;
    run(&mut c, 30, Some(0));
    assert_eq!(word(&c, page), 1);
    let got = c.sys.ports.adpcm_port.captures - before;
    assert!(got > 300, "ADPCM の取込み {got}");
    // 2. GLYPH AS WAVE：文字 ROM の 256 byte に XOR $01
    run(&mut c, 30, Some(0));
    assert_eq!(word(&c, page), 2);
    let off = 0x100; // 起動したときの v_glyph＝1
    let rom = &c.sys.ports.char_rom[off..off + 256];
    let wave = &c.sys.ports.doc_board.wave[0x2000..0x2100];
    assert!(rom.iter().zip(wave).all(|(r, w)| r ^ 1 == *w));
    // 3. TEMPORAL FEEDBACK：1 フレームに 768 byte を記録する
    run(&mut c, 30, Some(0));
    assert_eq!(word(&c, page), 3);
    let w0 = c.sys.ports.temporal.wptr;
    run(&mut c, 10, None);
    let w1 = c.sys.ports.temporal.wptr;
    assert_eq!(w1.wrapping_sub(w0) % 768, 0);
    assert!(w1.wrapping_sub(w0) >= 768 * 5, "{w0:#x} → {w1:#x}");
    assert_ne!(c.cpu.state, RunState::Halted, "PC {:06X}", c.cpu.pc);
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
    assert_eq!(c.sys.log.doc_rule_warnings, 0);
    assert_eq!(word(&c, errors), 0, "Fabric の誤り");
}
