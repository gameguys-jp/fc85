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
