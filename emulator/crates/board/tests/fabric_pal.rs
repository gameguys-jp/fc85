//! PAL の式と PROM の中身から生成した関数（`fabric_pal`、DR-084）の試験。
//! 生成した経路が、それまでの式（仕様書 §06・DR-046 の変換、fabric.md §06 のポートの表と準備の信号）と
//! 同じ値を出すかを確かめる。生成したファイルが式と同じかは `tools/fabric` の Python の試験で確かめる。

use fc85_board::fabric::{
    Dir, PortWidth, atr, fd_value, input_is_word, output_is_word, port_attr, port_ok, shift_amount,
    transform,
};
use fc85_board::fabric_pal::{
    ATR_ADDR, ATR_F4, ATR_FLAG, ATR_SONLY, ATR_TWO, ATR_W16, MSK0, MSK1, anx0, anx1, anx2, anx3,
    rdy, rot0, rot1, rot2, rot3,
};

/// DR-084 の前の `fd_value`（変換の定義を式で書いたもの）。
fn fd_value_formula(input: u16, format: u8, shift: u8, mask: u16, xor: u16) -> u16 {
    let f = format & 7;
    if f >= 6 {
        return xor;
    }
    let s = shift_amount(shift);
    let y = if f == 5 {
        input.rotate_left((s & 15) as u32)
    } else {
        let x = match f {
            0 | 2 | 3 => input & 0xFF,
            4 => input >> 8,
            _ => input,
        } as u32;
        (if s >= 0 { x << s } else { x >> -s }) as u16
    };
    let v = (y & mask) ^ xor;
    if matches!(f, 1 | 2 | 5) {
        v
    } else {
        (v & 0x00FF) | (xor & 0xFF00)
    }
}

/// 試験の擬似乱数（xorshift）。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn u16(&mut self) -> u16 {
        (self.next() >> 24) as u16
    }
}

#[test]
fn rot_pals_rotate_every_word_by_every_amount() {
    for t in 0..16u32 {
        for d in 0..=0xFFFFu32 {
            let fine = rot0(d, t).f | rot1(d, t).f;
            let r = rot2(fine, t).r | rot3(fine, t).r;
            assert_eq!(r, (d as u16).rotate_left(t) as u32, "d {d:#x} t {t}");
        }
    }
}

#[test]
fn anx_pals_are_and_then_xor_with_three_state() {
    // ビットごとに独立なので、R・M・K・X の 16 通りを全部の bit に並べて調べる。
    for v in 0..16u32 {
        let all = |b: u32| if v >> b & 1 != 0 { 0xFFFF } else { 0 };
        let (r, m, k, x) = (all(0), all(1), all(2), all(3));
        let want = (r & m & k) ^ x;
        for wen in [false, true] {
            let (a0, a1, a2, a3) = (
                anx0(r, m, k, x, wen),
                anx1(r, m, k, x, wen),
                anx2(r, m, k, x, wen),
                anx3(r, m, k, x, wen),
            );
            let q = a0.q | a1.q | a2.q | a3.q;
            let oe = a0.q_oe | a1.q_oe | a2.q_oe | a3.q_oe;
            assert_eq!(q, want, "R M K X = {v:04b}");
            assert_eq!(oe, if wen { 0xFFFF } else { 0 });
        }
    }
}

#[test]
fn msk_proms_match_the_valid_bits() {
    for a in 0..256usize {
        let (s, f) = (shift_amount(a as u8 & 31), (a >> 5) as u8);
        let m = MSK0[a] as u16 | (MSK1[a] as u16) << 8;
        // 入力の有効なビットを 1 にした値を、MASK＝0xFFFF・XOR＝0 で通したものが、有効な出力のビット。
        let ones = match f {
            0 | 2 | 3 => 0x00FF,
            4 => 0xFF00,
            _ => 0xFFFF,
        };
        let want = if f >= 6 {
            0
        } else {
            fd_value_formula(ones, f, a as u8 & 31, 0xFFFF, 0)
        };
        assert_eq!(m, want, "FORMAT {f} SHIFT {s}");
    }
}

#[test]
fn generated_transform_matches_the_formula() {
    // FORMAT 0〜7 × SHIFT 32 通りの全部で、端の値と乱数の入力・MASK・XOR を 4,096 通りずつ。
    let mut rng = Rng(0x1985_0084);
    let mut n = 0;
    for f in 0..8u8 {
        for s in 0..32u8 {
            for i in 0..4096 {
                let input = match i {
                    0 => 0,
                    1 => 0xFFFF,
                    2..=17 => 1 << (i - 2),
                    _ => rng.u16(),
                };
                let (mask, xor) = if i < 18 {
                    (0xFFFF, 0)
                } else {
                    (rng.u16(), rng.u16())
                };
                let want = fd_value_formula(input, f, s, mask, xor);
                assert_eq!(
                    fd_value(input, f, s, mask, xor),
                    want,
                    "FORMAT {f} SHIFT {s} 入力 {input:#06x} MASK {mask:#06x} XOR {xor:#06x}"
                );
                let narrow = if matches!(f, 1 | 2 | 5) {
                    want
                } else {
                    want & 0xFF
                };
                assert_eq!(transform(input, f, s, mask, xor), narrow);
                n += 1;
            }
        }
    }
    assert_eq!(n, 8 * 32 * 4096);
}

/// DR-084 の前の `port_ok`（ポートの表から）。
fn port_ok_table(port: u8, dir: Dir, format: u8, doc: bool) -> bool {
    let f = format & 7;
    if f >= 6 {
        return false;
    }
    let a = port_attr(port, doc);
    let (can, word) = match dir {
        Dir::Read => (a.read, matches!(f, 1 | 3 | 4 | 5)),
        Dir::Write => (a.write, matches!(f, 1 | 2 | 5)),
    };
    can && match a.width {
        PortWidth::Either => true,
        PortWidth::Word => word,
        PortWidth::Byte => !word,
    }
}

#[test]
fn atr_prom_matches_the_port_table() {
    for port in 0..16u8 {
        for dir in [Dir::Read, Dir::Write] {
            for f in 0..8u8 {
                let a = atr(port, dir, f);
                for doc in [false, true] {
                    assert_eq!(port_ok(port, dir, f, doc), port_ok_table(port, dir, f, doc));
                    // ADDR・FLAG は port 2 の 2 つの中身で同じ。
                    assert_eq!(a & ATR_ADDR != 0, port_attr(port, doc).addr);
                    assert_eq!(a & ATR_FLAG != 0, port_attr(port, doc).flag);
                }
                assert_eq!(a & ATR_SONLY != 0, port == 0);
                assert_eq!(a & ATR_TWO != 0, port == 9);
                assert_eq!(a & ATR_F4 != 0, f == 4);
                let w16 = match dir {
                    Dir::Read => matches!(f, 1 | 3 | 4 | 5),
                    Dir::Write => matches!(f, 1 | 2 | 5),
                };
                assert_eq!(a & ATR_W16 != 0, w16, "port {port} {dir:?} FORMAT {f}");
            }
        }
        let _ = (input_is_word(0), output_is_word(0));
    }
}

#[test]
fn rdy_pal_selects_the_ready_line_of_each_port() {
    // 線の 128 通り（bit 1 RDY1、2 RDY2、3 IR3、4 OR4、5 IR5、10 OR10、11 XRDY）とポートの全部。
    let bits = [1u32, 2, 3, 4, 5, 10, 11];
    for v in 0..128u32 {
        let lines = bits
            .iter()
            .enumerate()
            .fold(0, |a, (i, &b)| a | (v >> i & 1) << b);
        let line = |b: u32| lines & 1 << b != 0;
        for p in 0..16u32 {
            let o = rdy(p, p, lines, lines, lines, line(11));
            let src = match p {
                1 | 2 | 4 | 10 => line(p),
                11..=15 => line(11),
                _ => true,
            };
            let dst = match p {
                1 | 2 | 3 | 5 => line(p),
                11..=15 => line(11),
                _ => true,
            };
            assert_eq!((o.srdy, o.drdy), (src, dst), "port {p} 線 {lines:#x}");
        }
    }
}
