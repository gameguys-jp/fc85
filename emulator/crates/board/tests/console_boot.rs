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
    // バスエラーはない。カートリッジの有無は INPUT_IRQ_STATUS の bit 4 で見て、差していなければ窓を
    // 読まない（DR-095。それまでは $400000 を読んでバスエラーが 1 回出ていた）
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
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
            c.sys.log.bus_errors, 0,
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
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
    let changed = vram
        .iter()
        .zip(&c.sys.ports.acrtc.vram.0)
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 100, "VRAM が崩れていない（{changed} 語）");
}

#[test]
fn the_patch_cartridge_comes_back_after_a_slow_doc_clock() {
    // DR-079・DR-080・DR-082：SAMPLE 3 で DOC の CLK を 10 kHz に 1 秒下げてから戻す。下げている間に
    // Fabric の port 2 が時間切れになり、ACRTC のコマンドの列がずれて CER で止まる。BIOS の FIFO の
    // 待ちが CER を見てすぐ打ち切る（CCR の DDM は保つ）ので、戻して 2 秒のうちにアプリが動き出す
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
    for _ in 0..120 {
        c.run_frame();
    }
    c.sys.bend("clock doc 10e3").unwrap();
    c.sys.bend("mode bend").unwrap();
    for _ in 0..60 {
        c.run_frame();
    }
    c.sys.bend("mode normal").unwrap();
    let f = bios_frame(&c);
    let mut in_cart = 0;
    for _ in 0..120 {
        c.run_frame();
        if (0x40_0000..0x50_0000).contains(&c.cpu.pc) {
            in_cart += 1;
        }
    }
    assert!(bios_frame(&c) >= f + 119, "VBlank が止まった");
    assert!(
        in_cart > 10,
        "アプリが動き出さない（PC {:06X}、{in_cart}）",
        c.cpu.pc
    );
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
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

/// 0.5 秒ほどの音（48 kHz）の、30〜300 Hz で一番強い周波数。12 標本の平均で 4 kHz に間引き、
/// 0.25 Hz ごとに Goertzel で強さを比べる。
fn strongest_hz(x: &[f64]) -> f64 {
    let y: Vec<f64> = x
        .chunks_exact(12)
        .map(|c| c.iter().sum::<f64>() / 12.0)
        .collect();
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    let rate = 48_000.0 / 12.0;
    let mut best = (0.0, 0.0);
    for k in 0..=1080 {
        let f = 30.0 + f64::from(k) * 0.25;
        let coeff = 2.0 * (2.0 * std::f64::consts::PI * f / rate).cos();
        let (mut s1, mut s2) = (0.0, 0.0);
        for &v in &y {
            let s = v - mean + coeff * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        let p = s1 * s1 + s2 * s2 - coeff * s1 * s2;
        if p > best.1 {
            best = (f, p);
        }
    }
    best.0
}

#[test]
fn the_bend_cartridge_plays_and_draws() {
    // sdk/samples/bend の SAMPLE 5：仕様書 §11 のベンド点を試す試験信号（sdk/samples/bend/README.md）。
    // DOC の持続音・低音・分散和音と ADPCM の太鼓が 5 秒途切れず、画面は毎フレーム変わる。B で持続音
    // だけにし、DOC の CLK を 8 MHz から 4 MHz に下げると、持続音（A2、110 Hz）の高さが半分になる
    let Some(cart) = rom("sdk/samples/bend/build/bend.bin") else {
        eprintln!("sdk/samples/bend/build/bend.bin がないので飛ばす");
        return;
    };
    let Some(mut c) = boot(true) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    c.sys.cart = Some(cart);
    c.power_on();
    let word = |c: &Console, a: usize| {
        let i = a - 0x10_0000;
        u16::from_be_bytes([c.sys.main_ram[i], c.sys.main_ram[i + 1]])
    };
    let v_vbl_miss = 0x10_0806; // main.s の RSSET APP_RAM の 4 番目
    let v_errors = 0x10_0808; // 5 番目
    let mut audio = Vec::new();
    let mut frames = Vec::new();
    for i in 0..300 {
        c.sys.set_ports(0, 0);
        c.run_frame();
        audio.append(&mut c.sys.audio_out);
        if i == 149 || i == 150 {
            frames.push(c.sys.ports.acrtc.video.frame.clone());
        }
    }
    assert_ne!(c.cpu.state, RunState::Halted, "PC {:06X}", c.cpu.pc);
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
    assert_eq!(
        c.sys.log.doc_rule_warnings, 0,
        "{:?}",
        c.sys.log.last_doc_rule
    );
    assert_eq!(word(&c, v_errors), 0, "Fabric の誤り");
    assert_eq!(word(&c, v_vbl_miss), 0, "VBlank を待って時間切れ");
    // 1 秒ごとに、L と R のどちらも鳴っている
    assert!(audio.len() >= 4 * 48_000, "{}", audio.len());
    for (s, w) in audio.chunks_exact(48_000).enumerate() {
        let rms = |f: fn(&(i16, i16)) -> i16| {
            (w.iter().map(|p| f64::from(f(p)).powi(2)).sum::<f64>() / w.len() as f64).sqrt()
        };
        let (l, r) = (rms(|p| p.0), rms(|p| p.1));
        assert!(
            l > 300.0 && r > 300.0,
            "{s} 秒目が静か（RMS L {l:.0}、R {r:.0}）"
        );
    }
    // 箱と状態の行が動く
    let first = frames[0][0];
    assert!(frames[0].iter().any(|&p| p != first), "画面が一色");
    assert_ne!(frames[0], frames[1], "画面が止まっている");
    // B で持続音だけにして、L（偶数のオシレータ）の一番強い周波数を測る
    let measure = |c: &mut Console, press_b: bool| {
        let mut x = Vec::new();
        for i in 0..60 {
            let pad = if press_b && i == 0 { 0x0020 } else { 0 };
            c.sys.set_ports(pad, 0);
            c.run_frame();
            if i >= 30 {
                x.extend(c.sys.audio_out.iter().map(|p| f64::from(p.0)));
            }
            c.sys.audio_out.clear();
        }
        strongest_hz(&x)
    };
    let before = measure(&mut c, true);
    c.sys.bend("mode bend").unwrap();
    c.sys.bend("clock doc 4e6").unwrap();
    let after = measure(&mut c, false);
    eprintln!("持続音 {before:.2} Hz → DOC 4 MHz で {after:.2} Hz");
    assert!((before - 110.2).abs() < 2.0, "持続音が {before:.2} Hz");
    assert!(
        (after / before - 0.5).abs() < 0.03,
        "{before:.2} Hz → {after:.2} Hz"
    );
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
}

#[test]
fn the_showcase_cartridge_draws_beyond_sixteen_colours() {
    // sdk/samples/showcase の SAMPLE 6：通常モードの映像と描画の予算（sdk/samples/showcase/README.md）。
    // DOC の子基板なしで、毎ライン HBlank の割込みでパレット 1・2 を書き換え、1 フレームに 16 色を大きく
    // 超える色を出す。DSP（Data Fabric の port 3・4）で回した立方体と、玉のスプライトが毎フレーム動く。
    // Fabric・DSP・ACRTC の誤りと VBlank の待ちの時間切れは、カートリッジが数える（README.md §05 の表）
    let Some(cart) = rom("sdk/samples/showcase/build/showcase.bin") else {
        eprintln!("sdk/samples/showcase/build/showcase.bin がないので飛ばす");
        return;
    };
    let Some(mut c) = boot(false) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    c.sys.cart = Some(cart);
    c.power_on();
    let app = 0x10_0800;
    let word = |c: &Console, a: usize| {
        let i = a - 0x10_0000;
        u16::from_be_bytes([c.sys.main_ram[i], c.sys.main_ram[i + 1]])
    };
    let mut frames = Vec::new();
    let mut counted = Vec::new();
    for i in 0..150 {
        c.sys.set_ports(0, 0);
        c.run_frame();
        if i == 120 || i == 121 {
            frames.push(c.sys.ports.acrtc.video.frame.clone());
        }
        if i == 100 || i == 149 {
            counted.push(bios_frame(&c));
        }
    }
    assert_ne!(c.cpu.state, RunState::Halted, "PC {:06X}", c.cpu.pc);
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
    assert_eq!(c.sys.log.doc_rule_warnings, 0);
    let names = [
        "Fabric の時間切れか ERROR",
        "DSP の誤り",
        "尾の語の食い違い",
        "DSP と 68000 の突合せ",
        "ACRTC の打切り",
        "VBlank の待ちの時間切れ",
    ];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(word(&c, app + 2 + 2 * k), 0, "{name}");
    }
    assert_eq!(word(&c, app + 14), 1, "DSP を読み込めていない");
    assert_eq!(word(&c, app + 38), 1, "DSP で計算していない");
    assert!(word(&c, app) >= 50, "主ループ {}", word(&c, app));
    // VBlank の割込み（BIOS の処理につなぐ）は数え続ける
    assert!(counted[1] >= counted[0] + 45, "bios_frame {counted:?}");
    // 1 フレームの色の数
    let colours: std::collections::HashSet<u16> = frames[0].iter().copied().collect();
    eprintln!("1 フレームの色 {}", colours.len());
    assert!(colours.len() > 48, "色が {} しかない", colours.len());
    assert_ne!(frames[0], frames[1], "画面が止まっている");
}

#[test]
fn the_showcase_cartridge_reloads_the_dsp_after_an_overclock() {
    // DR-083：DSP の CLKIN を 25.3 MHz に上げると、プログラム RAM の読出しが間に合わず、崩れた命令の
    // TBLW がプログラム RAM を書き換える。誤りにならない所（透視の表を引く命令）が書き換わると、
    // 尾の語は合ったまま頂点が 1 点に集まる。SAMPLE 6 は誤りのあとに初めて通ったときにもプログラムを
    // 読み込み直すので（sdk/samples/showcase/README.md §04）、クロックを戻すと元の立方体に戻る
    let (Some(cart), Some(code)) = (
        rom("sdk/samples/showcase/build/showcase.bin"),
        rom("sdk/samples/showcase/build/xform.bin"),
    ) else {
        eprintln!("sdk/samples/showcase/build/ がないので飛ばす");
        return;
    };
    let Some(mut c) = boot(false) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    c.sys.cart = Some(cart);
    c.power_on();
    let code: Vec<u16> = code
        .chunks(2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect();
    let app = 0x800;
    let errors = |c: &Console| -> u32 {
        (0..3)
            .map(|k| {
                u32::from(u16::from_be_bytes([
                    c.sys.main_ram[app + 2 + 2 * k],
                    c.sys.main_ram[app + 3 + 2 * k],
                ]))
            })
            .sum()
    };
    // 画面の座標（Staging RAM の $0100 から X・Y×8）
    let vertices = |c: &Console| -> Vec<(i16, i16)> {
        let s = &c.sys.ports.staging;
        let w = |k: usize| i16::from_be_bytes([s[0x100 + 2 * k], s[0x101 + 2 * k]]);
        (0..8).map(|v| (w(2 * v), w(2 * v + 1))).collect()
    };
    let mut after = 0;
    for i in 0..260 {
        match i {
            120 => {
                c.sys.bend("mode bend").unwrap();
                c.sys.bend("clock dsp 25.3e6").unwrap();
            }
            200 => {
                c.sys.bend("clock dsp off").unwrap();
                assert!(errors(&c) > 0, "上げても誤りがない");
            }
            230 => after = errors(&c),
            _ => {}
        }
        c.sys.set_ports(0, 0);
        c.run_frame();
    }
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
    assert_eq!(errors(&c), after, "戻してからも誤りが続く");
    let prog = &c.sys.ports.dsp.prog[..code.len()];
    let changed = prog.iter().zip(&code).filter(|(a, b)| a != b).count();
    assert_eq!(changed, 0, "プログラム RAM が {changed} 語違う");
    let v = vertices(&c);
    assert!(
        v.iter().any(|p| *p != v[0]),
        "頂点が 1 点に集まっている {v:?}"
    );
}

/// 字形の 1 行（MSB が左）を、BIOS の描き方（bios-abi.md §07）で 4 bpp の 2 語にする。語の中は左の画素が
/// 下位ニブル（台帳 HD63484.md §4.5）。
fn glyph_words(row: u8, fg: u16, bg: u16) -> [u16; 2] {
    let mut w = [0u16; 2];
    for (k, word) in w.iter_mut().enumerate() {
        for i in 0..4 {
            let on = row & (0x80 >> (4 * k + i)) != 0;
            *word |= (if on { fg } else { bg }) << (4 * i);
        }
    }
    w
}

/// BIOS_PUTS_KUTEN の符号（$00xx：JIS X 0201、$KKTT：区点）→ 文字 ROM の offset（仕様書 §08）。ROM にない
/// 区点は〓（2 区 14 点。bios-abi.md §05）。
fn kuten_offset(code: u16) -> usize {
    let (ku, ten) = (usize::from(code >> 8), usize::from(code & 0xFF));
    let frame = |ku: usize, ten: usize| 0x800 + ((ku - 1) * 94 + ten - 1) * 8;
    match (ku, ten) {
        (0, _) => ten * 8,
        (_, 0) | (_, 95..) => frame(2, 14),
        (1..=8, _) => frame(ku, ten),
        (16..=47, _) => 0x1F80 + ((ku - 16) * 94 + ten - 1) * 8,
        _ => frame(2, 14),
    }
}

/// VRAM の (x, y) から、文字 ROM の offset の字が前景 `fg`・背景 `bg` で並んでいるか。
fn assert_text(c: &Console, x: usize, y: usize, offsets: &[usize], fg: u16, bg: u16) {
    let vram = &c.sys.ports.acrtc.vram.0;
    for r in 0..8 {
        for (k, &off) in offsets.iter().enumerate() {
            let want = glyph_words(c.sys.ports.char_rom[off + r], fg, bg);
            let a = (y + r) * 64 + x / 4 + 2 * k;
            assert_eq!(
                [vram[a], vram[a + 1]],
                want,
                "({x},{y}) の {k} 字目の {r} 行目"
            );
        }
    }
}

#[test]
fn the_bios_keeps_its_promises() {
    // DR-094：試験のカートリッジ firmware/boot/abitest.s が、BIOS の呼出しと割込みを試して、結果を
    // Main RAM の $100800 からに書く（番地は abitest.s の表）。DR-095：差してあるのでバスエラーはない
    let Some(cart) = rom("firmware/boot/build/abitest.bin") else {
        eprintln!("firmware/boot/build/abitest.bin がないので飛ばす");
        return;
    };
    let Some(mut c) = boot(false) else {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
        return;
    };
    c.sys.cart = Some(cart);
    c.power_on();
    for _ in 0..40 {
        c.sys.set_ports(0, 0);
        c.run_frame();
    }
    let w = |a: usize| {
        let i = a - 0x10_0000;
        u16::from_be_bytes([c.sys.main_ram[i], c.sys.main_ram[i + 1]])
    };
    let l = |a: usize| (u32::from(w(a)) << 16) | u32::from(w(a + 2));
    let app = 0x10_0800;
    let z = 0x0004; // SR の Z
    assert_eq!(w(app), 0x600D, "終わっていない（PC {:06X}）", c.cpu.pc);
    assert_ne!(c.cpu.state, RunState::Halted, "PC {:06X}", c.cpu.pc);
    // 時間（VBlank の割込みの直後から戻るまでのライン数）。版 1 の BIOS でも測る（abitest.s）ので、先に出す
    let lines: Vec<u16> = (0..6).map(|k| w(app + 0x20 + 2 * k)).collect();
    eprintln!(
        "ライン数（版 {}）：PUTS 1 字 {}、10 字 {}、PUT_HEX 6 桁 {}（描き終わるまで {}）、PUTS_KUTEN 6 字 {}、ACRTC_SEND 64 語 {}",
        w(app + 0x02),
        lines[0],
        lines[1],
        lines[2],
        lines[3],
        lines[4],
        lines[5]
    );
    // 版（BIOS_INFO と入口の D0）、カートリッジのビット
    assert_eq!(w(app + 0x02), 2, "BIOS_INFO_VERSION");
    assert_eq!(l(app + 0x04), 2, "入口の D0");
    assert_eq!(w(app + 0x08) & 0x10, 0x10, "INPUT_IRQ_STATUS の bit 4");
    // 出力のほかのレジスタ、SP、USP を保つ。割込み（アプリの処理 → BIOS の処理 → フック）の間も保つ
    // 最初に違った呼出しの、戻った値と約束の値（D0–D7/A0–A6。abitest.s の bad_out）
    let regs = |base: usize| -> Vec<String> {
        (0..15)
            .map(|k| format!("{:08X}", l(base + 4 * k)))
            .collect()
    };
    assert_eq!(
        w(app + 0x0A),
        0,
        "レジスタを保たない呼出し（bit が表の番号）：戻った値 {:?}、約束 {:?}",
        regs(app + 0x1B4),
        regs(app + 0x1F0)
    );
    assert_eq!(w(app + 0x0C), 0, "SP が戻らない呼出し");
    assert_eq!(w(app + 0x0E), 0, "割込みの間に変わったレジスタの数");
    assert_eq!(l(app + 0x10), 0x5A5A_A5A5, "USP");
    // ACRTC を使う呼出し（ACRTC_SEND、CLS、PUTS、PUT_HEX、PUTS_KUTEN）は打ち切らなければ Z＝1。
    // GLYPH は ROM にある区点で Z＝1
    for i in [1, 2, 3, 4, 7, 8] {
        assert_eq!(w(app + 0x50 + 2 * i) & z, z, "表の {i} 番目の呼出しの Z");
    }
    // 打切りを返す（DR-080、DR-082、DR-094）：誤りの列は Z＝0 で 1 回、SR の CER。次の正しい列は Z＝1
    assert_eq!(w(app + 0x16) & z, 0, "誤りの列の後の Z");
    assert_eq!(w(app + 0x18), 1, "打ち切った数");
    assert_eq!(w(app + 0x1A) & 0x80, 0x80, "打ち切る前の SR に CER");
    assert_eq!(w(app + 0x14) & z, z, "正しい列の後の Z");
    // レベル 6：アプリの処理から BIOS の処理へつなぐと、VBlank の数え、パッド、フックが動き続ける
    let (irq6, hook) = (w(app + 0x1C), w(app + 0x1E));
    assert!(hook >= 30 && irq6 >= hook, "レベル 6 {irq6}、フック {hook}");
    assert!(bios_frame(&c) >= 35, "bios_frame {}", bios_frame(&c));
    // BIOS_GLYPH：$0041、16 区 1 点、47 区 94 点、ROM にない 3 つ（〓、Z＝0）
    let geta = 0x01_0000 + 2 * kuten_offset(0x020E) as u32;
    let want = [
        (0x01_0000 + 0x41 * 16, z),
        (0x01_0000 + 0x1F80 * 2, z),
        (0x01_0000 + 0x7D78 * 2, z),
        (geta, 0),
        (geta, 0),
        (geta, 0),
    ];
    for (k, (a0, zf)) in want.into_iter().enumerate() {
        assert_eq!(l(app + 0x2C + 4 * k), a0, "BIOS_GLYPH {k} の A0");
        assert_eq!(w(app + 0x44 + 2 * k) & z, zf, "BIOS_GLYPH {k} の Z");
    }
    // OI-065 の目安：6 桁の描き直しが VBlank（70 ライン）に入る
    assert!(lines[3] < 70, "PUT_HEX 6 桁が VBlank に入らない：{lines:?}");
    assert!(lines[1] < 10 * 4, "PUTS が速くなっていない：{lines:?}");
    // 照合用の字：PUTS（A、1、ｱ、Z、空白）と PUTS_KUTEN（A、16 区 1 点、〓、4 区 2 点、〓）
    assert_text(
        &c,
        16,
        8,
        &[0x41 * 8, 0x31 * 8, 0xB1 * 8, 0x5A * 8, 0x20 * 8],
        15,
        1,
    );
    let codes = [0x0041, 0x1001, 0x0914, 0x2422, 0x0100];
    let offsets: Vec<usize> = codes.into_iter().map(kuten_offset).collect();
    assert_text(&c, 16, 24, &offsets, 3, 0);
    // バスエラーなし。WFE を見て 8 語を続けて書いても、満杯の FIFO に書かない
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
    assert_eq!(
        c.sys.ports.acrtc.acrtc.error_counts().0,
        0,
        "満杯の FIFO に書いた"
    );
}
