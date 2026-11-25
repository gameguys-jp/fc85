//! 基板の模型の速さ（CPU なし）：仮想機の 1 秒を進める時間を測る。`cargo run --release -p fc85-board --example bench`
use fc85_board::console::System;
use fc85_chips::hd68000::{Bus, Size, fc};
use std::time::Instant;

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
            (0x04, 0xC020),
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
}
