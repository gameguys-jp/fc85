//! 音声のアナログの段（analog.rs）、DOC の S/H と SIG± のジャンパ（doc.rs、DR-011）、MSM5205 の DA OUT の
//! 遅れ（adpcm.rs）の試験。
//! 期待値は部品の定数（analog.rs の `parts`、[P]）から、教科書の回路の式で手計算した値（emulator.md §03 の
//! 「音声のアナログの段」の表）と、台帳 ES5503.md §3.6（CSTRB の Low 500 ns）、MSM5205.md §3.2
//! （DA OUT は V̄C̄K̄ の立下りの 2 クロック後）から取る。

use std::f64::consts::PI;

use fc85_board::adpcm::AdpcmPort;
use fc85_board::analog::{
    Analog, Coefs, Inputs, SigWiring, TICK_RATE, mfb, parts, rc_corner, sallen_key,
};
use fc85_board::console::System;
use fc85_board::fifo::Fifo;
use fc85_core::time::{PERIOD_384K, Time};

fn close(got: f64, want: f64, rel: f64) {
    assert!(
        (got - want).abs() <= rel * want.abs(),
        "{got} と {want}（相対 {rel}）"
    );
}

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

/// アナログの 2 次の LPF の利得。
fn lp2(f: f64, f0: f64, q: f64) -> f64 {
    let r = f / f0;
    1.0 / ((1.0 - r * r).powi(2) + (r / q).powi(2)).sqrt()
}

fn lp1(f: f64, f0: f64) -> f64 {
    1.0 / (1.0 + (f / f0).powi(2)).sqrt()
}

fn hp1(f: f64, f0: f64) -> f64 {
    let r = f / f0;
    r / (1.0 + r * r).sqrt()
}

// ---------------------------------------------------------------- 部品の定数と各段

#[test]
fn component_values_give_the_documented_corners() {
    // 手計算（emulator.md §03）：DOC の加算の極 1/(2π·10 kΩ·1 nF)
    close(
        rc_corner(parts::DOC_SUM_R_FB, parts::DOC_SUM_C_FB),
        15_915.5,
        1e-4,
    );
    // DOC の Sallen-Key：R＝9.1 kΩ、C1＝2.2 nF、C2＝560 pF → 15.757 kHz、Q 0.9910
    let (f, q) = sallen_key(9.1e3, 9.1e3, 2.2e-9, 560e-12);
    close(f, 15_757.0, 1e-4);
    close(q, 0.991_03, 1e-4);
    // 本体 DAC の多重帰還：R＝3.6 kΩ×3、C1＝4.7 nF、C2＝1.0 nF → 20.392 kHz、Q 0.7226、利得 −1
    let (f, q, g) = mfb(3.6e3, 3.6e3, 3.6e3, 4.7e-9, 1.0e-9);
    close(f, 20_392.4, 1e-4);
    close(q, 0.722_65, 1e-4);
    assert_eq!(g, -1.0);
    // ADPCM：SK（11 kΩ、4.7 nF、3.9 nF）→ 3.379 kHz、Q 0.5489。MFB（18 kΩ×3、10 nF、680 pF）→ 3.391 kHz、Q 1.2783
    let (f, q) = sallen_key(11e3, 11e3, 4.7e-9, 3.9e-9);
    close(f, 3_379.45, 1e-4);
    close(q, 0.548_89, 1e-4);
    let (f, q, _) = mfb(18e3, 18e3, 18e3, 10e-9, 680e-12);
    close(f, 3_390.73, 1e-4);
    close(q, 1.278_27, 1e-4);
    // 結合容量：ADPCM 2.2 µF と 50 kΩ → 1.447 Hz、出力 10 µF と 10.1 kΩ → 1.576 Hz
    close(rc_corner(50e3, 2.2e-6), 1.4469, 1e-4);
    close(rc_corner(10.1e3, 10e-6), 1.5758, 1e-4);
    // 定数がこの手計算の値を使っている
    let (f, q) = sallen_key(
        parts::DOC_SK_R,
        parts::DOC_SK_R,
        parts::DOC_SK_C1,
        parts::DOC_SK_C2,
    );
    close(f, 15_757.0, 1e-4);
    close(q, 0.991_03, 1e-4);
}

#[test]
fn each_section_matches_the_analog_response() {
    // 双一次変換は f0 で補正するので、f0 では離散の利得がアナログの式と一致する。f0/4 でも 0.01 dB 以内
    let c = Coefs::get();
    let fs = TICK_RATE;
    assert_eq!(fs, 192_000.0);
    let (sk_f, sk_q) = sallen_key(
        parts::DOC_SK_R,
        parts::DOC_SK_R,
        parts::DOC_SK_C1,
        parts::DOC_SK_C2,
    );
    let (dm_f, dm_q, _) = mfb(
        parts::DAC_MFB_R,
        parts::DAC_MFB_R,
        parts::DAC_MFB_R,
        parts::DAC_MFB_C1,
        parts::DAC_MFB_C2,
    );
    let (as_f, as_q) = sallen_key(
        parts::AD_SK_R,
        parts::AD_SK_R,
        parts::AD_SK_C1,
        parts::AD_SK_C2,
    );
    let (am_f, am_q, _) = mfb(
        parts::AD_MFB_R,
        parts::AD_MFB_R,
        parts::AD_MFB_R,
        parts::AD_MFB_C1,
        parts::AD_MFB_C2,
    );
    let sum_f = rc_corner(parts::DOC_SUM_R_FB, parts::DOC_SUM_C_FB);
    let g_sum = parts::DOC_SUM_R_FB / parts::DOC_SUM_R_IN;
    let out_f = rc_corner(parts::OUT_R_S + parts::OUT_R_LOAD, parts::OUT_C);
    let g_out = parts::OUT_R_LOAD / (parts::OUT_R_S + parts::OUT_R_LOAD);
    let ad_hp = rc_corner(parts::MIX_R_AD / 2.0, parts::AD_C_COUPLE);
    type Case<'a> = (
        &'a str,
        f64,
        Box<dyn Fn(f64) -> f64>,
        &'a fc85_board::analog::Section,
    );
    let cases: Vec<Case> = vec![
        (
            "DOC 加算",
            sum_f,
            Box::new(move |f| g_sum * lp1(f, sum_f)),
            &c.doc_sum,
        ),
        (
            "DOC SK",
            sk_f,
            Box::new(move |f| lp2(f, sk_f, sk_q)),
            &c.doc_sk,
        ),
        (
            "DAC MFB",
            dm_f,
            Box::new(move |f| lp2(f, dm_f, dm_q)),
            &c.dac_mfb,
        ),
        (
            "AD SK",
            as_f,
            Box::new(move |f| lp2(f, as_f, as_q)),
            &c.ad_sk,
        ),
        (
            "AD MFB",
            am_f,
            Box::new(move |f| lp2(f, am_f, am_q)),
            &c.ad_mfb,
        ),
        (
            "AD 結合",
            ad_hp,
            Box::new(move |f| hp1(f, ad_hp)),
            &c.ad_hpf,
        ),
        (
            "出力の結合",
            out_f,
            Box::new(move |f| g_out * hp1(f, out_f)),
            &c.out_hpf,
        ),
    ];
    for (name, f0, analog, sec) in &cases {
        let at = |f: f64| (db(sec.response(f, fs)), db(analog(f)));
        let (d, a) = at(*f0);
        assert!((d - a).abs() < 1e-6, "{name} の f0：{d} dB と {a} dB");
        let (d, a) = at(f0 / 4.0);
        assert!((d - a).abs() < 0.02, "{name} の f0/4：{d} dB と {a} dB");
    }
    // 2 次の段の f0 の利得は Q（−0.08 dB、−2.82 dB）
    close(c.doc_sk.response(sk_f, fs), sk_q, 1e-9);
    close(c.dac_mfb.response(dm_f, fs), dm_q, 1e-9);
}

#[test]
fn the_output_coupling_step_decays_with_rc() {
    // 出力の結合（10 µF、100 Ω＋10 kΩ）：τ＝101 ms。1 V の階段の後、τ で 0.990×e⁻¹
    let c = Coefs::get();
    let tau = (parts::OUT_R_S + parts::OUT_R_LOAD) * parts::OUT_C;
    close(tau, 0.101, 1e-9);
    let mut s = fc85_board::analog::State::default();
    let n = (tau * TICK_RATE).round() as usize;
    let mut y = 0.0;
    for _ in 0..=n {
        y = s.run(&c.out_hpf, 1.0);
    }
    close(y, 0.990_099 * (-1.0f64).exp(), 2e-3);
}

/// `Analog` の刻みに正弦を入れ、LINE OUT の電圧の振幅を測る（0.5 秒の後の整数周期で相関をとる）。
fn measure(f: f64, inp: impl Fn(f64) -> Inputs, out: impl Fn(&Analog) -> f64) -> f64 {
    let mut a = Analog::new();
    let skip = (0.5 * TICK_RATE) as usize;
    let n = ((0.2 * f).round() / f * TICK_RATE).round() as usize;
    let (mut si, mut co) = (0.0, 0.0);
    for k in 0..skip + n {
        let t = k as f64 / TICK_RATE;
        a.tick(&inp((2.0 * PI * f * t).sin()));
        if k >= skip {
            let y = out(&a);
            si += y * (2.0 * PI * f * t).sin();
            co += y * (2.0 * PI * f * t).cos();
        }
    }
    2.0 * (si * si + co * co).sqrt() / n as f64
}

#[test]
fn doc_line_follows_the_component_values() {
    // DOC の S/H の和に振幅 A（S/H の単位）の正弦。LINE OUT の振幅は A×電圧の係数×10/39×|LPF1|×|SK|
    // ×1（ミキサ）×|出力の結合|。−3 dB は手計算で 15.71 kHz
    let c = Coefs::get();
    let amp = 32640.0 * 65536.0;
    let (sk_f, sk_q) = sallen_key(
        parts::DOC_SK_R,
        parts::DOC_SK_R,
        parts::DOC_SK_C1,
        parts::DOC_SK_C2,
    );
    let sum_f = rc_corner(parts::DOC_SUM_R_FB, parts::DOC_SUM_C_FB);
    let out_f = rc_corner(parts::OUT_R_S + parts::OUT_R_LOAD, parts::OUT_C);
    for f in [200.0, 1000.0, 10_000.0, 15_714.0, 20_000.0] {
        let got = measure(
            f,
            |s| Inputs {
                doc: [amp * s, 0.0],
                ..Inputs::default()
            },
            |a| a.output().0,
        );
        let want = amp
            * c.doc_volts
            * (10.0 / 39.0)
            * lp1(f, sum_f)
            * lp2(f, sk_f, sk_q)
            * 0.990_099
            * hp1(f, out_f);
        let err = db(got) - db(want);
        assert!(
            err.abs() < if f > 16_000.0 { 0.5 } else { 0.05 },
            "{f} Hz：{} dB（{err:+.3} dB）",
            db(got)
        );
    }
    // 1 V の S/H（全振幅の 1 チャネル）は、1 kHz で 0.2539 V
    close(amp * c.doc_volts, 1.0, 1e-9);
}

#[test]
fn main_dac_and_adpcm_lines_follow_the_component_values() {
    let (dm_f, dm_q, _) = mfb(
        parts::DAC_MFB_R,
        parts::DAC_MFB_R,
        parts::DAC_MFB_R,
        parts::DAC_MFB_C1,
        parts::DAC_MFB_C2,
    );
    let (as_f, as_q) = sallen_key(
        parts::AD_SK_R,
        parts::AD_SK_R,
        parts::AD_SK_C1,
        parts::AD_SK_C2,
    );
    let (am_f, am_q, _) = mfb(
        parts::AD_MFB_R,
        parts::AD_MFB_R,
        parts::AD_MFB_R,
        parts::AD_MFB_C1,
        parts::AD_MFB_C2,
    );
    let out_f = rc_corner(parts::OUT_R_S + parts::OUT_R_LOAD, parts::OUT_C);
    let ad_hp = rc_corner(parts::MIX_R_AD / 2.0, parts::AD_C_COUPLE);
    // 本体 DAC：±100 の段（1 段 15.625 mV）→ MFB → ミキサ ×0.5
    for f in [1000.0, 10_000.0, 20_392.0] {
        let got = measure(
            f,
            |s| Inputs {
                dac: [100.0 * s, 0.0],
                dac_on: true,
                ..Inputs::default()
            },
            |a| a.output().0,
        );
        let want = 100.0 * 0.015_625 * lp2(f, dm_f, dm_q) * 0.5 * 0.990_099 * hp1(f, out_f);
        let err = db(got) - db(want);
        assert!(err.abs() < 0.05, "DAC {f} Hz：{err:+.3} dB");
    }
    // 子基板があるときは 4066 で切る
    let got = measure(
        1000.0,
        |s| Inputs {
            dac: [100.0 * s, 0.0],
            dac_on: false,
            ..Inputs::default()
        },
        |a| a.output().0,
    );
    assert!(got < 1e-12);
    // ADPCM：±400 の符号（1 段 2.441 mV）→ SK → MFB → 結合 → ミキサ ×0.1。−3 dB は手計算で 3.372 kHz
    for f in [300.0, 1000.0, 3371.8, 8000.0] {
        let got = measure(
            f,
            |s| Inputs {
                da: 400.0 * s,
                ..Inputs::default()
            },
            |a| a.output().1,
        );
        let want = 400.0 * 1.25 / 512.0
            * lp2(f, as_f, as_q)
            * lp2(f, am_f, am_q)
            * hp1(f, ad_hp)
            * 0.1
            * 0.990_099
            * hp1(f, out_f);
        let err = db(got) - db(want);
        assert!(
            err.abs() < if f > 4000.0 { 0.25 } else { 0.05 },
            "ADPCM {f} Hz：{err:+.3} dB"
        );
    }
}

// ---------------------------------------------------------------- DOC の S/H と SIG± のジャンパ

/// DOC 子基板のある本体。オシレータ 0（`control` の CA、止めるか）は頁 0x30 の 256 byte、FC 0（番地は
/// 動かない。電源投入時のアキュムレータは全ビット 1 なので頁のどこか）、有効なオシレータは 1 個（走査は
/// 3 サイクル：オシレータ 0 とリフレッシュ 2 回）。
fn doc_system(wave: u8, vol: u8, control: u8) -> System {
    let mut s = System::new();
    s.ports.doc = true;
    set_wave(&mut s, wave);
    let doc = &mut s.ports.doc_board.doc;
    // BIOS と同じく、ほかのオシレータは止めて音量 0、チャンネル 15 に置く（firmware/boot/sound.s）。
    // リセットの後の走査は 22 番から 31 番を回ってから 0 番に来る（台帳 ES5503 追補 2026-11-26）
    for n in 1..32 {
        doc.write(0xA0 + n, 0xF1);
        doc.write(0x40 + n, 0);
    }
    for (reg, v) in [
        (0x00, 0x00),
        (0x20, 0x00),
        (0x40, vol),
        (0x80, 0x30),
        (0xC0, 0x00),
        (0xA0, control),
        (0xE1, 0x00),
    ] {
        doc.write(reg, v);
    }
    s
}

/// 頁 0x30 を 1 つの値で埋める。
fn set_wave(s: &mut System, v: u8) {
    s.ports.doc_board.wave[0x3000..0x3100].fill(v);
}

fn run_slots(s: &mut System, n: usize) {
    for _ in 0..n {
        s.step_slot();
    }
}

#[test]
fn the_sample_hold_charges_through_rc_in_each_strobe() {
    // CSTRB の Low 500 ns（台帳 ES5503.md §3.6）、300 Ω×470 pF＝141 ns：1−e^(−3.546)＝0.97116（Q16 で 63646）
    let mut s = doc_system(0xC0, 255, 0x00);
    let alpha = s.ports.doc_board.sh_alpha();
    let want = ((1.0 - (-500e-9f64 / (300.0 * 470e-12)).exp()) * 65536.0).round() as u32;
    assert_eq!(alpha, want);
    assert_eq!(alpha, 63646);
    // 両側：波形 0xC0（+64）×音量 255 → 差は 2×16320＝32640（S/H は 1.0 V）
    let x = 32640i64 << 16;
    let mut seen = vec![0i64];
    for _ in 0..200 {
        s.step_slot();
        let v = s.ports.doc_board.sample_holds()[0];
        if v != *seen.last().unwrap() {
            seen.push(v);
        }
    }
    assert!(seen.len() > 5, "{seen:?}");
    let a = i64::from(alpha);
    let p = 1.0 - f64::from(alpha) / 65536.0;
    for (k, w) in seen.windows(2).enumerate() {
        // 1 回の CSTRB の更新（Q16 の整数）
        assert_eq!(w[1], w[0] + (((x - w[0]) * a) >> 16), "{k}");
        // 閉じた形 x×(1−(1−α)^k)
        let closed = x as f64 * (1.0 - p.powi(k as i32 + 1));
        assert!((w[1] as f64 - closed).abs() < 1e-4 * x as f64);
    }
    // ほかのチャネルの S/H は 0 のまま
    assert!(
        s.ports.doc_board.sample_holds()[1..]
            .iter()
            .all(|&v| v == 0)
    );
}

#[test]
fn the_one_sided_jumper_halves_the_signal_and_adds_dc_by_volume() {
    // ミキサの出力（結合容量の前）の電圧。S/H の電圧 V（両側 ±1.0 V、片側 1.5 V と 0.5 V）×10/39
    let mixer = mixer_l;
    let mut s = doc_system(0xC0, 255, 0x00);
    run_slots(&mut s, 4000);
    let both_hi = mixer(&s);
    set_wave(&mut s, 0x40);
    run_slots(&mut s, 4000);
    let both_lo = mixer(&s);
    close(both_hi, 10.0 / 39.0, 1e-4);
    close(both_lo, -10.0 / 39.0, 1e-4);
    // Bend の切り替え：動いている途中で公開の欄を変える
    s.ports.doc_board.sig_wiring = SigWiring::One;
    run_slots(&mut s, 4000);
    let one_lo = mixer(&s);
    set_wave(&mut s, 0xC0);
    run_slots(&mut s, 4000);
    let one_hi = mixer(&s);
    close(one_hi, 1.5 * 10.0 / 39.0, 1e-4);
    close(one_lo, 0.5 * 10.0 / 39.0, 1e-4);
    // 振幅は半分、中央は音量に比例した直流（音量 255 で S/H の 1.0 V）
    close(one_hi - one_lo, (both_hi - both_lo) / 2.0, 1e-4);
    close((one_hi + one_lo) / 2.0, 10.0 / 39.0, 1e-4);
    // 音量 96 では直流も 96/255
    let mut s = doc_system(0xC0, 96, 0x00);
    s.ports.doc_board.sig_wiring = SigWiring::One;
    run_slots(&mut s, 4000);
    let hi = mixer(&s);
    set_wave(&mut s, 0x40);
    run_slots(&mut s, 4000);
    let lo = mixer(&s);
    close((hi + lo) / 2.0, 96.0 / 255.0 * 10.0 / 39.0, 1e-4);
    close(hi - lo, 96.0 / 255.0 * 0.5 * 2.0 * 10.0 / 39.0, 1e-4);
}

/// ミキサの L の電圧（結合容量の前）。
fn mixer_l(s: &System) -> f64 {
    s.analog.mixer[0].0
}

#[test]
fn a_halted_oscillator_at_volume_zero_is_silent_in_both_wirings() {
    // OI-062 の案：止めたオシレータも CSTRB で自分のチャネルを取る（ES5503 のコアの模型）。音量 0 なら、
    // 乗算型の DAC の電流は波形の値によらず 0 なので、両側でも片側でも S/H は 0 V になる
    for wiring in [SigWiring::Both, SigWiring::One] {
        let mut s = doc_system(0xC0, 0, 0x01);
        s.ports.doc_board.sig_wiring = wiring;
        run_slots(&mut s, 2000);
        assert_eq!(s.ports.doc_board.sample_holds()[0], 0, "{wiring:?}");
        assert_eq!(mixer_l(&s), 0.0);
    }
    // 音量が 0 でないと、片側では止めたオシレータが 128×音量の直流を出す（S/H 0.784 V、音量 200）
    let mut s = doc_system(0xC0, 200, 0x01);
    s.ports.doc_board.sig_wiring = SigWiring::One;
    run_slots(&mut s, 4000);
    close(
        mixer_l(&s),
        128.0 * 200.0 * 2.0 / 65280.0 * 10.0 / 39.0,
        1e-4,
    );
    // 両側では中央（0 V）
    let mut s = doc_system(0xC0, 200, 0x01);
    run_slots(&mut s, 4000);
    assert_eq!(s.ports.doc_board.sample_holds()[0], 0);
}

#[test]
fn line_out_and_audio_out_have_the_same_number_of_samples() {
    for doc in [false, true] {
        let mut s = if doc {
            doc_system(0xC0, 255, 0x00)
        } else {
            System::new()
        };
        for n in [1, 7, 83, 4000, 12_345] {
            run_slots(&mut s, n);
            assert_eq!(s.line_out.len(), s.audio_out.len(), "{doc} {n}");
        }
    }
}

#[test]
fn line_and_digital_agree_in_the_passband() {
    // 子基板なし、本体 DAC の DIRECT で 375 Hz の矩形（±64 の段）を 0.6 秒。結合容量が落ち着いた後の
    // 平らな所で、LINE とデジタルの比は 0.990（手計算：15.625 mV×0.5×0.990 と 128×2 V/32768）
    let mut s = System::new();
    s.ports.main_dac.direct = true;
    let half = 4_000_000 / 375 / 2; // 半周期のスロット数（250 ns）
    for k in 0..(2_400_000 / half) {
        let v = if k % 2 == 0 { 0xC0C0 } else { 0x4040 };
        s.ports.main_dac.latch = v;
        run_slots(&mut s, half);
    }
    let n = s.audio_out.len();
    let (d, l) = (&s.audio_out[n / 2..], &s.line_out[n / 2..]);
    // 各半周期の中ほどの標本を比べる（立上りの後の 200 µs と、結合容量の傾きを避ける）
    let mut ratios = Vec::new();
    for i in 20..d.len() - 20 {
        if d[i].0 != d[i - 20].0 || d[i].0 != d[i + 20].0 {
            continue;
        }
        ratios.push(f64::from(l[i].0) / f64::from(d[i].0));
    }
    assert!(ratios.len() > 1000);
    let mean = ratios.iter().sum::<f64>() / ratios.len() as f64;
    close(
        mean,
        0.990_099 * 15.625e-3 * 0.5 / (128.0 * 2.0 / 32768.0),
        0.01,
    );
}

// ---------------------------------------------------------------- MSM5205 の DA OUT

#[test]
fn da_out_shows_the_previous_capture_two_clocks_after_vck_falls() {
    // 台帳 MSM5205.md §3.2：取込みは V̄C̄K̄ の立下りの 6 クロック後、DA OUT は 2 クロック後。DA OUT に
    // 出る値は前の周期に取り込んだもの。8 kHz では取込みから 44 クロック（114.6 µs）後
    let mut a = AdpcmPort::new();
    let mut fifo = Fifo::new(8);
    for _ in 0..40 {
        fifo.shift_in(0x77);
    }
    let mut c = a.ctrl();
    c.reset = false;
    a.set_ctrl(c);
    let mut vck = a.vck();
    let mut captures = a.captures;
    let mut last_code = a.msm.dac_code();
    let mut capture_clock = None;
    let mut pending: Option<(u64, i16)> = None;
    let mut checked = 0;
    let mut delays = Vec::new();
    for k in 0..48 * 30u64 {
        let t: Time = k * PERIOD_384K;
        let before = a.da_out;
        a.run(t + 1, &mut fifo);
        if a.captures != captures {
            captures = a.captures;
            last_code = a.msm.dac_code();
            capture_clock = Some((k, last_code));
        }
        if vck && !a.vck() {
            // 立下り：2 クロック後に、今の取込みまでの値が出る
            pending = Some((k + 2, last_code));
        }
        vck = a.vck();
        if let Some((at, code)) = pending
            && at == k
        {
            assert_eq!(a.da_out, code, "{k}");
            assert_eq!(a.da_at, t);
            pending = None;
            checked += 1;
            if let Some((ck, cc)) = capture_clock
                && cc == code
            {
                delays.push(k - ck);
            }
        } else if a.da_out != before {
            panic!("立下りの 2 クロック後でないのに DA OUT が変わった（{k}）");
        }
    }
    assert!(checked > 25);
    assert!(
        delays.iter().filter(|&&d| d == 44).count() > 20,
        "{delays:?}"
    );
}
