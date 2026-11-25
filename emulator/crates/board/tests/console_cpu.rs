//! 本体（段階 3）：68000 のコアを載せて、手で組んだ小さな ROM を動かす。
//! 期待値は仕様書 §05、DR-025〜DR-028、DR-053 と、台帳 HD68000.md の例外処理の手順から取る。

use fc85_board::console::Console;
use fc85_chips::hd68000::RunState;

/// ROM の語の列を番地 `at` から置く。
fn put(rom: &mut [u8], at: usize, words: &[u16]) {
    for (i, w) in words.iter().enumerate() {
        rom[at + 2 * i..at + 2 * i + 2].copy_from_slice(&w.to_be_bytes());
    }
}

fn long(v: u32) -> [u16; 2] {
    [(v >> 16) as u16, v as u16]
}

/// SSP＝0x120000（Main RAM の末尾の次）、PC＝0x400。レベル 6 のオートベクタ（ベクタ 30）は 0x500。
fn rom_with_vblank_counter() -> Vec<u8> {
    let mut rom = vec![0xFF; 0x1_0000];
    put(&mut rom, 0, &long(0x0012_0000));
    put(&mut rom, 4, &long(0x0000_0400));
    put(&mut rom, 30 * 4, &long(0x0000_0500));
    let mut prog = vec![
        0x41F9, 0x0027, 0x0000, // LEA $270000,A0
        0x317C, 0x0F00, 0x0002, // MOVE.W #$0F00,2(A0)：パレット 1
        0x317C, 0x0001, 0x0022, // MOVE.W #1,$22(A0)：VBL_EN
        0x43F9, 0x0020, 0x0000, // LEA $200000,A1
    ];
    // ACRTC の案B（台帳 HD63484.md §3.4）
    for (r, v) in [
        (0x82, 0xAA0D),
        (0x84, 0x127F),
        (0x86, 0x0106),
        (0x88, 0x2703),
        (0x8A, 0x00C0),
        (0xCA, 0x0040),
        (0x04, 0xC020),
    ] {
        prog.extend_from_slice(&[0x32BC, r, 0x337C, v, 0x0002]); // MOVE.W #r,(A1)；MOVE.W #v,2(A1)
    }
    prog.extend_from_slice(&[
        0x46FC, 0x2000, // MOVE.W #$2000,SR：割込みのマスクを 0 に
        0x5280, // loop: ADDQ.L #1,D0
        0x60FC, // BRA.S loop
    ]);
    put(&mut rom, 0x400, &prog);
    put(
        &mut rom,
        0x500,
        &[
            0x5281, // ADDQ.L #1,D1
            0x33FC, 0x0101, 0x0027, 0x0022, // MOVE.W #$0101,$270022：VBL を消す
            0x4E73, // RTE
        ],
    );
    rom
}

#[test]
fn the_cpu_boots_from_rom_and_counts_vblanks() {
    let mut c = Console::new();
    c.sys.boot_rom = rom_with_vblank_counter();
    c.power_on();
    assert_eq!(c.cpu.ssp, 0x0012_0000);
    for _ in 0..4 {
        c.run_frame();
    }
    assert_eq!(c.cpu.state, RunState::Running);
    assert_eq!(c.sys.ports.acrtc.video.back[1], 0x0F00);
    let frames = c.sys.ports.acrtc.video.frames;
    assert!(frames >= 3, "{frames}");
    // VBlank ごとに 1 回、レベル 6 のオートベクタで数える
    let d1 = c.cpu.d[1];
    assert!(
        (frames - 1..=frames + 1).contains(&u64::from(d1)),
        "D1 {d1}、フレーム {frames}"
    );
    assert!(c.cpu.d[0] > 10_000);
    assert_eq!(c.sys.log.bus_errors, 0);
    assert_eq!(c.sys.log.iacks, u64::from(d1));
}

#[test]
fn an_unassigned_address_takes_the_bus_error_vector() {
    // MOVE.W $290000,D0 はバスエラー（DR-052）。ベクタ 2 は 0x600 で、そこで止まる
    let mut rom = vec![0xFF; 0x1_0000];
    put(&mut rom, 0, &long(0x0012_0000));
    put(&mut rom, 4, &long(0x0000_0400));
    put(&mut rom, 2 * 4, &long(0x0000_0600));
    put(&mut rom, 0x400, &[0x3039, 0x0029, 0x0000, 0x60FE]);
    put(&mut rom, 0x600, &[0x7E2A, 0x60FE]); // MOVEQ #42,D7；BRA.S *
    let mut c = Console::new();
    c.sys.boot_rom = rom;
    c.power_on();
    c.run_until(2000);
    assert_eq!(c.cpu.d[7], 42);
    assert_eq!(c.sys.log.bus_errors, 1);
    assert_eq!(c.sys.log.last_bus_error, Some((0x29_0000, false)));
    // 例外の枠：7 語（状態語、番地、IR、SR、PC）
    assert_eq!(c.cpu.ssp, 0x0012_0000 - 14);
}

#[test]
fn doc_read_modify_write_is_warned() {
    // CLR.B $210000 は読んで書く（DR-030 の違反）。MOVE.B D0,$210002 は 1 回の書込みで規約どおり
    let mut rom = vec![0xFF; 0x1_0000];
    put(&mut rom, 0, &long(0x0012_0000));
    put(&mut rom, 4, &long(0x0000_0400));
    put(
        &mut rom,
        0x400,
        &[0x4239, 0x0021, 0x0000, 0x13C0, 0x0021, 0x0002, 0x60FE],
    );
    let mut c = Console::new();
    c.sys.boot_rom = rom;
    c.sys.ports.doc = true;
    c.power_on();
    c.run_until(400);
    assert_eq!(c.sys.log.bus_errors, 0, "{:?}", c.sys.log.last_bus_error);
    assert_eq!(c.sys.log.doc_rule_warnings, 1);
    assert_eq!(c.sys.log.last_doc_rule, Some(0x21_0000));
}
