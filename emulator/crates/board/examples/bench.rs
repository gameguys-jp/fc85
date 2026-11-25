//! 基板の模型の速さ（CPU なし）：仮想機の 1 秒を進める時間を測る。`cargo run --release -p fc85-board --example bench`
use fc85_board::console::Console;
use fc85_board::console::System;
use fc85_chips::hd68000::{Bus, Size, fc};
use std::time::Instant;

/// CPU も載せる：ACRTC の案B を設定し、Main RAM の語を数え続けるループで、仮想機の 1 秒を進める。
fn with_cpu(doc: bool) {
    let mut rom = vec![0xFFu8; 0x1_0000];
    let mut w = vec![0x0012u16, 0x0000, 0x0000, 0x0400];
    w.resize(0x200, 0xFFFF);
    let mut prog = vec![0x43F9u16, 0x0020, 0x0000]; // LEA $200000,A1
    for (r, v) in [
        (0x82u16, 0xAA0Du16),
        (0x84, 0x127F),
        (0x86, 0x0106),
        (0x88, 0x2703),
        (0x8A, 0x00C0),
        (0xCA, 0x0040),
        (0x04, 0xC008),
    ] {
        prog.extend_from_slice(&[0x32BC, r, 0x337C, v, 0x0002]);
    }
    // loop: ADDQ.W #1,$100000 ; BRA.S loop
    prog.extend_from_slice(&[0x5279, 0x0010, 0x0000, 0x60F8]);
    w.extend_from_slice(&prog);
    for (i, x) in w.iter().enumerate() {
        rom[2 * i..2 * i + 2].copy_from_slice(&x.to_be_bytes());
    }
    let mut c = Console::new();
    c.sys.ports.doc = doc;
    c.sys.boot_rom = rom;
    c.power_on();
    let start = Instant::now();
    c.run_until(8_000_000);
    println!(
        "CPU あり DOC {doc}: 仮想機の 1 秒に {:.3} 秒（{} フレーム）",
        start.elapsed().as_secs_f64(),
        c.sys.ports.acrtc.video.frames
    );
}

fn main() {
    for doc in [false, true] {
        let mut s = System::new();
        s.ports.doc = doc;
        let mut t = 0u64;
        for (r, v) in [
            (0x82u16, 0xAA0Du16),
            (0x84, 0x127F),
            (0x86, 0x0106),
            (0x88, 0x2703),
            (0x8A, 0x00C0),
            (0xCA, 0x0040),
            (0x04, 0xC008),
        ] {
            s.write(t, 0x20_0000, fc::SUPERVISOR_DATA, Size::Word, r);
            t += 10;
            s.write(t, 0x20_0002, fc::SUPERVISOR_DATA, Size::Word, v);
            t += 10;
        }
        let start = Instant::now();
        s.sync(t + 8_000_000);
        let el = start.elapsed();
        println!(
            "DOC {doc}: 仮想機の 1 秒に {:.3} 秒（{} フレーム）",
            el.as_secs_f64(),
            s.ports.acrtc.video.frames
        );
    }
    with_cpu(false);
    with_cpu(true);
}
