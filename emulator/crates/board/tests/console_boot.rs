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
