//! MSM5205 の単体試験（仕様書 §12 段階 1）。
//! 期待値は台帳 `docs/parts/MSM5205.md` の計算（§4.2、§4.6、追補 A.3）から取る。

use fc85_chips::msm5205::{
    CAPTURE_DELAY, Capture, Edge, Msm5205, Overflow, STEP_TABLE, SampleRate, diff,
};

/// RESET を 2 周期かけてから外した状態を作る。
fn after_reset(overflow: Overflow) -> Msm5205 {
    let mut c = Msm5205::new();
    c.set_overflow(overflow);
    c.set_reset(true);
    let mut n = 0;
    while n < 2 {
        if c.tick().capture.is_some() {
            n += 1;
        }
    }
    c.set_reset(false);
    c
}

/// ニブルを 1 つずつ与え、それぞれの取込みの記録を返す。
fn feed(c: &mut Msm5205, nibbles: &[u8]) -> Vec<Capture> {
    nibbles
        .iter()
        .map(|&n| {
            c.set_data(n);
            loop {
                if let Some(cap) = c.tick().capture {
                    break cap;
                }
            }
        })
        .collect()
}

#[test]
fn step_table_matches_formula() {
    for (i, &s) in STEP_TABLE.iter().enumerate() {
        let f = (16.0 * 1.1f64.powi(i as i32)).floor() as i16;
        assert_eq!(s, f, "i = {i}");
    }
}

#[test]
fn default_overflow_is_wrap() {
    assert_eq!(Msm5205::new().overflow(), Overflow::Wrap);
}

#[test]
fn rate_pins_follow_datasheet() {
    // 台帳 §3.1：S1→1/48、S2→1/64、両方 L→1/96。
    assert_eq!(SampleRate::from_pins(false, false).divider(), Some(96));
    assert_eq!(SampleRate::from_pins(false, true).divider(), Some(64));
    assert_eq!(SampleRate::from_pins(true, false).divider(), Some(48));
    assert_eq!(SampleRate::from_pins(true, true), SampleRate::Slave);
}

#[test]
fn capture_is_six_clocks_after_vck_fall() {
    for (s1, s2, p) in [(true, false, 48u64), (false, true, 64), (false, false, 96)] {
        let mut c = Msm5205::new();
        c.set_rate_pins(s1, s2);
        let mut falls = Vec::new();
        let mut rises = Vec::new();
        let mut caps = Vec::new();
        for t in 1..=(p * 3) {
            let k = c.tick();
            match k.edge {
                Some(Edge::Fall) => falls.push(t),
                Some(Edge::Rise) => rises.push(t),
                None => {}
            }
            if k.capture.is_some() {
                caps.push(t);
            }
        }
        assert_eq!(falls, vec![p / 2, p + p / 2, 2 * p + p / 2], "divider {p}");
        assert_eq!(rises, vec![p, 2 * p, 3 * p], "divider {p}");
        let d = u64::from(CAPTURE_DELAY);
        assert_eq!(
            caps,
            falls.iter().map(|f| f + d).collect::<Vec<_>>(),
            "divider {p}"
        );
    }
}

#[test]
fn reset_clears_signal_and_step() {
    let mut c = after_reset(Overflow::Wrap);
    feed(&mut c, &[0x7, 0x7, 0x7]);
    assert_ne!(c.signal(), 0);
    c.set_reset(true);
    let cap = feed(&mut c, &[0x7]);
    assert_eq!(
        cap[0],
        Capture {
            nibble: 0x7,
            reset: true,
            signal: 0,
            step: 0
        }
    );
}

/// DR-021：リセット直後に 0111 を 6 個。6 個目が回り込みなら −1586、クリップなら +2047（台帳 追補 A.3）。
#[test]
fn dr021_wrap_vs_clip_sequence() {
    let steps_before = [0u8, 8, 16, 24, 32, 40];
    let diffs = [30, 63, 136, 293, 631, 1357];
    for (i, (&s, &d)) in steps_before.iter().zip(&diffs).enumerate() {
        assert_eq!(diff(s, 0x7), d, "input {}", i + 1);
    }

    let mut wrap = after_reset(Overflow::Wrap);
    let w: Vec<i16> = feed(&mut wrap, &[0x7; 6])
        .iter()
        .map(|c| c.signal)
        .collect();
    assert_eq!(w, [30, 93, 229, 522, 1153, -1586]);

    let mut clip = after_reset(Overflow::Clip);
    let k: Vec<i16> = feed(&mut clip, &[0x7; 6])
        .iter()
        .map(|c| c.signal)
        .collect();
    assert_eq!(k, [30, 93, 229, 522, 1153, 2047]);
}

/// 台帳 §4.6 (a)：0000 を保持すると 1 サンプル 2 LSB ずつ上がり、約 1024 サンプルで回り込む。
#[test]
fn holding_zero_wraps_after_1024_samples() {
    let mut wrap = after_reset(Overflow::Wrap);
    let w = feed(&mut wrap, &[0x0; 1024]);
    assert_eq!(w[1022].signal, 2046);
    assert_eq!(w[1023].signal, -2048);
    assert!(w.iter().all(|c| c.step == 0));

    let mut clip = after_reset(Overflow::Clip);
    let k = feed(&mut clip, &[0x0; 1024]);
    assert_eq!(k[1023].signal, 2047);
}

/// 台帳 §4.6 (b)：0000 と 1000 を交互に出すと ±2 LSB で中心付近に留まる。
#[test]
fn alternating_zero_and_minus_zero_stays_near_center() {
    let mut c = after_reset(Overflow::Wrap);
    let nibbles: Vec<u8> = (0..1000)
        .map(|i| if i % 2 == 0 { 0x0 } else { 0x8 })
        .collect();
    let caps = feed(&mut c, &nibbles);
    assert!(
        caps.iter()
            .all(|c| (0..=2).contains(&c.signal) && c.step == 0)
    );
}

#[test]
fn three_bit_mode_ignores_d0() {
    let mut a = after_reset(Overflow::Wrap);
    let mut b = after_reset(Overflow::Wrap);
    a.set_four_bit(false);
    b.set_four_bit(false);
    let x = feed(&mut a, &[0x7, 0x3, 0xD]);
    let y = feed(&mut b, &[0x6, 0x2, 0xC]);
    assert_eq!(x, y);
    assert!(x.iter().all(|c| c.nibble & 1 == 0));
}

#[test]
fn dac_code_drops_two_low_bits() {
    let mut c = after_reset(Overflow::Wrap);
    feed(&mut c, &[0x7]); // 30
    assert_eq!(c.dac_code(), 7);
    feed(&mut c, &[0xF]); // 30 − 63 = −33
    assert_eq!(c.signal(), -33);
    assert_eq!(c.dac_code(), -9);
}

#[test]
fn slave_mode_captures_after_external_vck_fall() {
    let mut c = Msm5205::new();
    c.set_rate_pins(true, true);
    for _ in 0..200 {
        assert_eq!(c.tick(), Default::default(), "内部の分周は止まる");
    }
    c.set_data(0x7);
    assert_eq!(c.set_vck_input(false), Some(Edge::Fall));
    for _ in 1..CAPTURE_DELAY {
        assert_eq!(c.tick(), Default::default());
    }
    assert!(matches!(c.tick().capture, Some(Capture { signal: 30, .. })));
    assert_eq!(c.set_vck_input(true), Some(Edge::Rise));
}

/// 途中の状態を保存して戻すと、同じ入力から同じ取込み列が出る（snapshot、DR-040）。
#[test]
fn snapshot_round_trip() {
    let mut c = after_reset(Overflow::Wrap);
    feed(&mut c, &[0x3, 0x5, 0xB, 0x7]);
    for _ in 0..10 {
        c.tick(); // 周期の途中で止める
    }
    let saved = serde_json::to_string(&c).unwrap();
    let mut restored: Msm5205 = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored, c);

    let rest = [0x1, 0x9, 0x6, 0xE, 0x0, 0x4];
    assert_eq!(feed(&mut c, &rest), feed(&mut restored, &rest));
}
