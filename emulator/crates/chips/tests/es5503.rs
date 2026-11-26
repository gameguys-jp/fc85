//! ES5503 DOC の単体試験（仕様書 §12 段階 1：発音と停止）。
//! 期待値は台帳 `docs/parts/ES5503.md` の記述と計算（§3.2、§4.1〜§4.8、§6）から取る。

use fc85_chips::es5503::{ADC_CYCLES, Es5503, Slot, Tick, WaveBus};

// ---- 制御レジスタの値（台帳 §4.2：CA3–CA0 IE M1 M0 H） ----

const HALT: u8 = 0x01;
const ONESHOT: u8 = 0x02;
const SYNC_AM: u8 = 0x04;
const SWAP: u8 = 0x06;
const IE: u8 = 0x08;

/// 波形 RAM（BS を含めて 128 KiB）。既定は 0x80（中心）。
struct Ram(Vec<u8>);

impl Ram {
    fn new() -> Self {
        Ram(vec![0x80; 0x2_0000])
    }
    fn fill(&mut self, at: u32, data: &[u8]) {
        let at = at as usize;
        self.0[at..at + data.len()].copy_from_slice(data);
    }
}

impl WaveBus for Ram {
    fn read_wave(&mut self, addr: u32) -> u8 {
        assert!(addr < 0x2_0000, "番地は 17 bit");
        self.0[addr as usize]
    }
}

fn set_osc(doc: &mut Es5503, n: u8, freq: u16, vol: u8, ptr: u8, size: u8) {
    doc.write(n, freq as u8);
    doc.write(0x20 + n, (freq >> 8) as u8);
    doc.write(0x40 + n, vol);
    doc.write(0x80 + n, ptr);
    doc.write(0xC0 + n, size);
}

/// 有効数を N にし、全オシレータをワンショットで止めて 1 周させる（M0＝1、H＝1 でアキュムレータが 0 になる）。
fn boot(n: u8) -> (Es5503, Ram) {
    let mut doc = Es5503::new();
    let mut ram = Ram::new();
    for i in 0..32 {
        set_osc(&mut doc, i, 0, 0, 0, 0);
        doc.write(0xA0 + i, ONESHOT | HALT);
    }
    doc.write(0xE1, (n - 1) * 2);
    for _ in 0..(u32::from(n) + 2) {
        doc.tick(&mut ram);
    }
    (doc, ram)
}

/// オシレータ `n` を訪れるまで進める。
fn visit(doc: &mut Es5503, ram: &mut Ram, n: u8) -> Tick {
    for _ in 0..100 {
        let t = doc.tick(ram);
        if let Slot::Osc { n: m, .. } = t.slot
            && m == n
        {
            return t;
        }
    }
    panic!("オシレータ {n} を訪れない");
}

fn visits(doc: &mut Es5503, ram: &mut Ram, n: u8, count: usize) -> Vec<Tick> {
    (0..count).map(|_| visit(doc, ram, n)).collect()
}

fn addr_of(t: &Tick) -> u32 {
    match t.slot {
        Slot::Osc { addr, .. } => addr,
        _ => panic!("オシレータのサイクルではない"),
    }
}

// ---- 電源投入と有効数（台帳 §4.7、§4.8） ----

#[test]
fn power_on_waits_for_the_first_e1_write() {
    let mut doc = Es5503::new();
    let mut ram = Ram::new();
    assert_eq!(doc.read(0xE1), 0xC1, "有効数 1。未使用の bit は 1");
    assert_eq!(doc.read(0xA0), 0xFF, "レジスタは全ビット 1");
    for _ in 0..10 {
        assert_eq!(doc.tick(&mut ram).slot, Slot::Stopped);
    }
    doc.write(0xE1, 0x1E);
    assert_eq!(doc.enabled(), 16);
    assert!(matches!(doc.tick(&mut ram).slot, Slot::Osc { n: 0, .. }));
}

#[test]
fn enable_register_holds_n_minus_1_times_2() {
    let mut doc = Es5503::new();
    for (value, n) in [(0x1E, 16), (62, 32), (64, 1), (0, 1)] {
        doc.write(0xE1, value);
        assert_eq!(doc.enabled(), n, "0xE1 に {value}");
    }
    doc.write(0xE1, 62);
    assert_eq!(doc.read(0xE1), 0xFF);
}

// ---- 走査（台帳 §3.2、§6） ----

#[test]
fn scan_visits_n_oscillators_then_two_refresh_cycles() {
    for n in [1u8, 8, 15, 16, 32] {
        let (mut doc, mut ram) = boot(n);
        let round: Vec<Slot> = (0..(u32::from(n) + 2))
            .map(|_| doc.tick(&mut ram).slot)
            .collect();
        for (i, s) in round.iter().enumerate().take(usize::from(n)) {
            assert!(matches!(s, Slot::Osc { n: m, .. } if usize::from(*m) == i));
        }
        assert_eq!(round[usize::from(n)..], [Slot::Refresh, Slot::Refresh]);
        // サンプルレート＝CLK÷((N＋2)×8)。8 MHz、N＝16 で 55,555.6 Hz
        let rate = 8_000_000.0 / ((f64::from(n) + 2.0) * 8.0);
        if n == 16 {
            assert!((rate - 55_555.6).abs() < 0.1);
        }
    }
}

#[test]
fn a_scan_past_the_new_end_runs_on_to_31() {
    let (mut doc, mut ram) = boot(32);
    visit(&mut doc, &mut ram, 20);
    doc.write(0xE1, 0x1E); // N＝16
    let next: Vec<Slot> = (0..13).map(|_| doc.tick(&mut ram).slot).collect();
    let numbers: Vec<u8> = next
        .iter()
        .map(|s| match s {
            Slot::Osc { n, .. } => *n,
            _ => 99,
        })
        .collect();
    assert_eq!(numbers, [21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 0, 1]);
    visit(&mut doc, &mut ram, 15);
    assert_eq!(doc.tick(&mut ram).slot, Slot::Refresh);
}

#[test]
fn refresh_cycles_hold_the_last_sig_without_strobe() {
    // 最上位のオシレータが 3 サイクル分聞こえる現象（台帳 §6 の注意 2）の、ピンの上での形 [V]
    let (mut doc, mut ram) = boot(2);
    set_osc(&mut doc, 1, 0x200, 2, 0x10, 0);
    ram.fill(0x1000, &[0xC0; 256]);
    doc.write(0xA1, 0x30); // CA＝3、フリーラン、動作
    let t: Vec<Tick> = (0..5).map(|_| doc.tick(&mut ram)).collect();
    assert!(matches!(t[0].slot, Slot::Osc { n: 0, .. }));
    assert_eq!(
        (t[0].sig, t[0].strobe),
        (0, true),
        "止まっているオシレータは 0"
    );
    assert!(matches!(t[1].slot, Slot::Osc { n: 1, .. }));
    assert_eq!((t[1].sig, t[1].channel, t[1].strobe), (128, 3, true));
    for r in &t[2..4] {
        assert_eq!(r.slot, Slot::Refresh);
        assert_eq!((r.sig, r.channel, r.strobe), (128, 0x0F, false));
    }
    assert_eq!(t[4].sig, 0);
}

// ---- 番地の計算（台帳 §4.3） ----

#[test]
fn address_follows_table_size_and_resolution() {
    let (mut doc, mut ram) = boot(1);
    // T＝R＝0、FC＝0x200：1 サンプルで 1 byte 進む
    set_osc(&mut doc, 0, 0x200, 0, 0x12, 0x00);
    doc.write(0xA0, 0);
    let a: Vec<u32> = visits(&mut doc, &mut ram, 0, 3)
        .iter()
        .map(addr_of)
        .collect();
    assert_eq!(a, [0x1200, 0x1201, 0x1202]);

    // T＝R＝1：512 byte のテーブル。ポインタは 512 byte 単位に切り捨てる
    let (mut doc, mut ram) = boot(1);
    set_osc(&mut doc, 0, 0x200, 0, 0x13, 0x09);
    doc.write(0xA0, 0);
    let a: Vec<u32> = visits(&mut doc, &mut ram, 0, 2)
        .iter()
        .map(addr_of)
        .collect();
    assert_eq!(a, [0x1200, 0x1201]);

    // R＝1、T＝0：同じ FC で半分の速さ
    let (mut doc, mut ram) = boot(1);
    set_osc(&mut doc, 0, 0x200, 0, 0x12, 0x01);
    doc.write(0xA0, 0);
    let a: Vec<u32> = visits(&mut doc, &mut ram, 0, 4)
        .iter()
        .map(addr_of)
        .collect();
    assert_eq!(a, [0x1200, 0x1200, 0x1201, 0x1201]);

    // T＝7：32 KiB のテーブルはポインタの P7 だけを使う。BS は bit 16
    let (mut doc, mut ram) = boot(1);
    set_osc(&mut doc, 0, 0x200, 0, 0xFF, 0x40 | 0x38 | 0x07);
    doc.write(0xA0, 0);
    assert_eq!(addr_of(&visit(&mut doc, &mut ram, 0)), 0x1_8000);
}

#[test]
fn free_run_wraps_every_256_samples_217_hz() {
    // 8 MHz、N＝16、256 byte のテーブル、FC＝0x200 で 217.0 Hz（台帳 §4.3 の例）
    let (mut doc, mut ram) = boot(16);
    set_osc(&mut doc, 0, 0x200, 0x80, 0x10, 0x00);
    doc.write(0xA0, IE);
    let mut at = Vec::new();
    let mut cycle = 0u64;
    while at.len() < 3 {
        cycle += 1;
        if doc.tick(&mut ram).irq {
            at.push(cycle);
            assert_eq!(doc.read(0xE0), 0x41, "OIR：bit7＝0、オシレータ 0");
        }
    }
    let period = at[2] - at[1];
    assert_eq!(period, 256 * 18);
    let hz = 1_000_000.0 / period as f64;
    assert!((hz - 217.0).abs() < 0.05, "{hz}");
    assert_eq!(doc.control(0) & HALT, 0, "フリーランは止まらない");
}

// ---- 停止（台帳 §4.2、§4.4） ----

#[test]
fn zero_halts_and_data_register_keeps_the_last_nonzero() {
    let (mut doc, mut ram) = boot(1);
    set_osc(&mut doc, 0, 0x200, 1, 0x40, 0);
    ram.fill(0x4000, &[0x90, 0xA0, 0x00, 0x00, 0x00]);
    doc.write(0xA0, IE);
    let t = visits(&mut doc, &mut ram, 0, 3);
    assert_eq!([t[0].sig, t[1].sig, t[2].sig], [16, 32, 0]);
    assert!(t[2].irq);
    assert_eq!(doc.control(0) & HALT, HALT);
    assert_eq!(doc.read(0x60), 0xA0, "0 は波形データのレジスタに入らない");
    assert_eq!(doc.read(0xE0), 0x41);
    // フリーランで 0 に止まったオシレータは、訪れるたびに 0 を見つける
    assert!(visit(&mut doc, &mut ram, 0).irq);
}

#[test]
fn one_shot_halts_at_the_end_and_rewinds() {
    let (mut doc, mut ram) = boot(1);
    set_osc(&mut doc, 0, 0x300, 1, 0x10, 0);
    doc.write(0xA0, ONESHOT);
    // 0x300×171 で初めて 2^17 を超える
    visits(&mut doc, &mut ram, 0, 170);
    assert_eq!(doc.control(0) & HALT, 0);
    visit(&mut doc, &mut ram, 0);
    assert_eq!(doc.control(0) & HALT, HALT);
    assert_eq!(doc.accumulator(0), 0x100, "端数が残る");
    visit(&mut doc, &mut ram, 0);
    assert_eq!(doc.accumulator(0), 0, "M0＝1、H＝1 で 0 に戻る");
}

#[test]
fn swap_pairs_take_turns() {
    let (mut doc, mut ram) = boot(2);
    set_osc(&mut doc, 0, 0x200, 1, 0x10, 0);
    set_osc(&mut doc, 1, 0x200, 1, 0x20, 0);
    doc.write(0xA0, SWAP);
    doc.write(0xA1, SWAP | HALT);
    visits(&mut doc, &mut ram, 0, 255);
    assert_eq!(doc.control(1) & HALT, HALT);
    visit(&mut doc, &mut ram, 0);
    assert_eq!(doc.control(0) & HALT, HALT, "偶数番は 256 サンプルで止まる");
    assert_eq!(doc.control(1) & HALT, 0, "奇数番が動き出す");
    let t = visit(&mut doc, &mut ram, 1);
    assert_eq!(addr_of(&t), 0x2000, "奇数番はテーブルの頭から");
    visits(&mut doc, &mut ram, 1, 255);
    assert_eq!(doc.control(1) & HALT, HALT);
    assert_eq!(doc.control(0) & HALT, 0, "奇数番が止まると偶数番が動き出す");
}

#[test]
fn sync_resets_the_odd_partner() {
    let (mut doc, mut ram) = boot(2);
    set_osc(&mut doc, 0, 0x200, 1, 0x10, 0);
    set_osc(&mut doc, 1, 0x100, 1, 0x20, 0);
    doc.write(0xA0, SYNC_AM);
    doc.write(0xA1, 0);
    visits(&mut doc, &mut ram, 0, 255);
    assert_eq!(
        doc.accumulator(1),
        254 * 0x100,
        "奇数番は同じ周でまだ訪れていない"
    );
    visit(&mut doc, &mut ram, 0);
    assert_eq!(doc.accumulator(1), 0, "偶数番が頭に戻ると奇数番も戻る");
    assert_eq!(addr_of(&visit(&mut doc, &mut ram, 1)), 0x2000);
}

#[test]
fn am_odd_oscillator_sets_the_volume_of_the_next() {
    let (mut doc, mut ram) = boot(3);
    set_osc(&mut doc, 1, 0x200, 0x7F, 0x30, 0);
    set_osc(&mut doc, 2, 0x200, 0x00, 0x40, 0);
    ram.fill(0x3000, &[0x40; 256]);
    ram.fill(0x4000, &[0xC0; 256]);
    doc.write(0xA1, SYNC_AM);
    doc.write(0xA2, 0);
    let t1 = visit(&mut doc, &mut ram, 1);
    assert_eq!(t1.sig, 0, "奇数番は鳴らさない");
    assert_eq!(doc.volume(2), 0x40);
    let t2 = visit(&mut doc, &mut ram, 2);
    assert_eq!(t2.sig, 64 * 0x40);

    // 上の偶数番が止まっていれば書かない
    doc.write(0xA2, HALT);
    doc.write(0x42, 0x11);
    visit(&mut doc, &mut ram, 1);
    assert_eq!(doc.volume(2), 0x11);
}

#[test]
fn halted_oscillator_still_fetches_and_outputs_center() {
    let (mut doc, mut ram) = boot(1);
    set_osc(&mut doc, 0, 0x200, 0xFF, 0x55, 0);
    ram.fill(0x5500, &[0xFF]);
    doc.write(0xA0, HALT);
    let t = visit(&mut doc, &mut ram, 0);
    assert_eq!(
        t.slot,
        Slot::Osc {
            n: 0,
            addr: 0x5500,
            wave: 0xFF
        }
    );
    assert_eq!(t.sig, 0);
    assert_eq!(doc.read(0x60), 0xFF);
}

// ---- 割込み（台帳 §4.6） ----

#[test]
fn irq_requests_come_out_first_in_first_out() {
    let (mut doc, mut ram) = boot(2);
    // オシレータ 1 は 1 周目、オシレータ 0 は 2 周目に 0 を読む
    set_osc(&mut doc, 0, 0x200, 1, 0x10, 0);
    set_osc(&mut doc, 1, 0x200, 1, 0x20, 0);
    ram.fill(0x1000, &[0x80, 0x00]);
    ram.fill(0x2000, &[0x00]);
    doc.write(0xA0, IE);
    doc.write(0xA1, IE);
    visit(&mut doc, &mut ram, 1);
    let t = visit(&mut doc, &mut ram, 0);
    assert!(t.irq);
    assert_eq!(doc.read(0xE0), 0x41 | (1 << 1), "先に完了したオシレータ 1");
    assert!(doc.irq());
    assert_eq!(doc.read(0xE0), 0x41);
    assert!(!doc.irq());
    assert_eq!(doc.read(0xE0), 0xC1, "要求がなければ bit7＝1");
}

#[test]
fn ie_zero_keeps_the_request_until_ie_is_set() {
    let (mut doc, mut ram) = boot(1);
    set_osc(&mut doc, 0, 0x200, 1, 0x10, 0);
    ram.fill(0x1000, &[0x00]);
    doc.write(0xA0, ONESHOT);
    let t = visit(&mut doc, &mut ram, 0);
    assert!(!t.irq, "IE＝0 では OIR に渡らない");
    doc.write(0xA0, ONESHOT | HALT | IE);
    assert!(doc.irq(), "IE を 1 にすると渡る（E1 p.7）");
    assert_eq!(doc.read(0xE0), 0x41);
}

// ---- レジスタ（台帳 §4.1、§3.7、§4.7、§4.8） ----

#[test]
fn unused_bits_read_as_one() {
    let (mut doc, _) = boot(1);
    doc.write(0xC5, 0x00);
    assert_eq!(doc.read(0xC5), 0x80);
    doc.write(0xC5, 0xFF);
    assert_eq!(doc.read(0xC5), 0xFF);
    doc.write(0x65, 0x12);
    assert_ne!(doc.read(0x65), 0x12, "波形データは読出し専用");
    for reg in [0xE3, 0xF0, 0xFF] {
        assert_eq!(doc.read(reg), 0xFF);
    }
}

#[test]
fn adc_converts_after_the_conversion_time() {
    let (mut doc, mut ram) = boot(1);
    doc.set_ad_input(0x5A);
    assert_eq!(doc.read(0xE2), 0, "読むと変換が始まる");
    for _ in 0..ADC_CYCLES {
        doc.tick(&mut ram);
    }
    assert_eq!(doc.read(0xE2), 0x5A);
    // 変換中に読むと、値は失われて変換をやり直す
    doc.set_ad_input(0x22);
    for _ in 0..10 {
        doc.tick(&mut ram);
    }
    assert_eq!(doc.read(0xE2), 0x5A);
    for _ in 0..(ADC_CYCLES - 1) {
        doc.tick(&mut ram);
    }
    assert_eq!(doc.read(0xE2), 0x5A, "やり直したので、まだ前の値");
    for _ in 0..ADC_CYCLES {
        doc.tick(&mut ram);
    }
    assert_eq!(doc.read(0xE2), 0x22);
}

/// オシレータ `from..32` の FL・FH・VOL・PTR・SIZE を 0 にし（制御は HALT）、`ticks` サイクル進めてから
/// 読んで、1 になったビットの割合を返す（SIZE は 7 bit。台帳 §4.1）。
fn decayed_share(doc: &mut Es5503, ram: &mut Ram, from: u8, ticks: u32) -> f64 {
    for n in from..32 {
        set_osc(doc, n, 0, 0, 0, 0);
    }
    for _ in 0..ticks {
        doc.tick(ram);
    }
    let mut ones = 0;
    let mut bits = 0;
    for n in from..32 {
        for base in [0x00u8, 0x20, 0x40, 0x80] {
            ones += doc.read(base + n).count_ones();
            bits += 8;
        }
        ones += (doc.read(0xC0 + n) & 0x7F).count_ones();
        bits += 7;
    }
    f64::from(ones) / f64::from(bits)
}

/// 対数ロジスティックの分布の割合（手計算の式。es5503.rs の REG_* の値）。
fn share(t_s: f64) -> f64 {
    if t_s < 0.002 {
        0.0
    } else {
        1.0 / (1.0 + (t_s / 0.02).powf(-3.6))
    }
}

#[test]
fn registers_of_disabled_oscillators_decay() {
    // 有効でないオシレータはリフレッシュされず、値を保たない（E2 p.15。台帳 §4.7）。保持時間は
    // 25 °C で最短 2 ms、中央 20 ms [P]。8 MHz 入力で 1 サイクル 1 µs
    for (ticks, lo, hi) in [
        (1_900, 0.0, 0.0),
        // 20 ms（中央）で約半分。31 個×39 ビット＝1,209 ビットの標本で ±5%（3.5σ）
        (20_000, 0.45, 0.55),
        // 200 ms で 1−1/(1＋10^3.6)＝0.99975
        (200_000, 0.995, 1.0),
    ] {
        let (mut doc, mut ram) = boot(1);
        let s = decayed_share(&mut doc, &mut ram, 1, ticks);
        let want = share(f64::from(ticks) * 1e-6);
        assert!(s >= lo && s <= hi, "{ticks} サイクル：{s}（式は {want}）");
    }
    // 走査されるオシレータ 0 は保つ
    let (mut doc, mut ram) = boot(1);
    doc.write(0x40, 0x34);
    for _ in 0..1_000_000 {
        doc.tick(&mut ram);
    }
    assert_eq!(doc.read(0x40), 0x34);
    // 比較用に止められる
    let (mut doc, mut ram) = boot(1);
    doc.set_decay(false);
    assert_eq!(decayed_share(&mut doc, &mut ram, 1, 200_000), 0.0);
}

#[test]
fn only_the_written_register_is_renewed() {
    // ホストの書込みは、書いたレジスタだけを新しくする [P]。FL を 1.5 ms ごとに書き直しても、
    // 書かない FH は前に書いた時刻から数えて崩れる
    let (mut doc, mut ram) = boot(1);
    for n in 1..32 {
        set_osc(&mut doc, n, 0, 0, 0, 0);
    }
    for _ in 0..40 {
        for _ in 0..1_500 {
            doc.tick(&mut ram);
        }
        for n in 1..32 {
            doc.write(n, 0);
        }
    }
    // 60 ms：書き直した FL は 1.5 ms 以内なので 0 のまま。FH は 60 ms で 1−1/(1＋3^3.6)＝0.98
    let fl: u32 = (1..32).map(|n| doc.read(n).count_ones()).sum();
    let fh: u32 = (1..32).map(|n| doc.read(0x20 + n).count_ones()).sum();
    assert_eq!(fl, 0);
    assert!(f64::from(fh) / 248.0 > 0.95, "{fh}");
}

#[test]
fn heat_shortens_the_retention() {
    // 保持時間は 10 °C ごとに半分（DR-072 と同じ）[P]。70 °C では 2^4.5＝22.6 倍速く崩れ、
    // 最短は 88 µs。1 ms の経過は 25 °C の 22.6 ms に当たり、割合は 1/(1＋1.13^−3.6)＝0.61
    let (mut doc, mut ram) = boot(1);
    assert_eq!(decayed_share(&mut doc, &mut ram, 1, 1_000), 0.0);
    let (mut doc, mut ram) = boot(1);
    doc.set_retention(0, 70);
    let s = decayed_share(&mut doc, &mut ram, 1, 1_000);
    assert!((s - 0.61).abs() < 0.05, "{s}");
    // 個体が違えば、崩れるビットの並びが違う
    let (mut a, mut ram_a) = boot(1);
    let (mut b, mut ram_b) = boot(1);
    a.set_retention(0, 25);
    b.set_retention(7, 25);
    for d in [&mut a, &mut b] {
        set_osc(d, 9, 0, 0, 0, 0);
    }
    for _ in 0..20_000 {
        a.tick(&mut ram_a);
        b.tick(&mut ram_b);
    }
    let va: Vec<u8> = [0x09u8, 0x29, 0x49, 0x89]
        .iter()
        .map(|&r| a.read(r))
        .collect();
    let vb: Vec<u8> = [0x09u8, 0x29, 0x49, 0x89]
        .iter()
        .map(|&r| b.read(r))
        .collect();
    assert_ne!(va, vb);
}

#[test]
fn a_slow_clock_lets_scanned_registers_decay() {
    // DR-035 のジャンパで CLK を下げると、訪れる間が延びる（台帳 §3.2）。32 個の走査は 34 サイクル。
    // CLK 8 MHz（1 µs）なら 34 µs で崩れない。CLK 50 kHz（E 160 µs）なら 5.44 ms で、割合は
    // 1/(1＋0.272^−3.6)＝0.0091。訪れるたびに同じビットが崩れる（保持時間の短い順）
    let (mut doc, mut ram) = boot(32);
    assert_eq!(decayed_share(&mut doc, &mut ram, 0, 34 * 200), 0.0);
    let (mut doc, mut ram) = boot(32);
    doc.set_cycle_time(160 * 15_120);
    let s = decayed_share(&mut doc, &mut ram, 0, 34 * 20);
    assert!(s > 0.002 && s < 0.02, "{s}");
    // CLK 1 Hz（E 8 秒）：訪れるたびに全部が 1 になる。制御は 0xFF で、CA は 15、M0＝1・H＝1 なので
    // アキュムレータは 0 に戻る（台帳 §4.2）。T＝7（32 KiB）、ポインタ 0xFF、BS＝1 なので番地は 0x18000（§4.3）
    let (mut doc, mut ram) = boot(32);
    doc.set_cycle_time(8 * 15_120_000_000);
    for _ in 0..34 {
        doc.tick(&mut ram);
    }
    assert_eq!(doc.read(0xA0), 0xFF);
    assert_eq!(doc.read(0x40), 0xFF);
    let t = visit(&mut doc, &mut ram, 3);
    assert_eq!(t.channel, 0x0F);
    assert_eq!(addr_of(&t), 0x1_8000);
}

/// リフレッシュの行を記録する波形 RAM。
struct Rows(Ram, Vec<u8>);

impl WaveBus for Rows {
    fn read_wave(&mut self, addr: u32) -> u8 {
        self.0.read_wave(addr)
    }
    fn refresh(&mut self, row: u8) {
        self.1.push(row);
    }
}

#[test]
fn refresh_cycles_count_the_rows() {
    // 走査の最後の 2 サイクルで、行の番地を 1 つずつ出す（E2 p.1、台帳 §3.2）。カウンタは 8 bit [P][V]
    let (mut doc, _) = boot(4);
    let start = doc.refresh_row();
    let mut bus = Rows(Ram::new(), Vec::new());
    for _ in 0..6 * 200 {
        doc.tick(&mut bus);
    }
    assert_eq!(bus.1.len(), 400);
    for (k, &r) in bus.1.iter().enumerate() {
        assert_eq!(r, start.wrapping_add(k as u8));
    }
    // 400 回で 8 bit のカウンタは 144 進む
    assert_eq!(doc.refresh_row(), start.wrapping_add(144));
}

#[test]
fn reset_stops_the_scan_and_clears_requests() {
    let (mut doc, mut ram) = boot(4);
    set_osc(&mut doc, 0, 0x200, 1, 0x10, 0);
    ram.fill(0x1000, &[0x00]);
    doc.write(0xA0, IE);
    assert!(visit(&mut doc, &mut ram, 0).irq);
    doc.set_reset(true);
    assert!(!doc.irq());
    assert_eq!(doc.tick(&mut ram).slot, Slot::Stopped);
    doc.set_reset(false);
    assert_eq!(
        doc.tick(&mut ram).slot,
        Slot::Stopped,
        "0xE1 に書くまで止まったまま"
    );
    assert_eq!(doc.enabled(), 1);
    doc.write(0xE1, 0x06);
    assert!(matches!(doc.tick(&mut ram).slot, Slot::Osc { n: 0, .. }));
}

#[test]
fn snapshot_round_trip() {
    let (mut doc, mut ram) = boot(4);
    set_osc(&mut doc, 2, 0x123, 0x55, 0x10, 0x00);
    doc.write(0xA2, IE);
    for _ in 0..1000 {
        doc.tick(&mut ram);
    }
    let json = serde_json::to_string(&doc).unwrap();
    let mut back: Es5503 = serde_json::from_str(&json).unwrap();
    assert_eq!(back, doc);
    let mut ram2 = Ram::new();
    for _ in 0..1000 {
        assert_eq!(doc.tick(&mut ram), back.tick(&mut ram2));
    }
}
