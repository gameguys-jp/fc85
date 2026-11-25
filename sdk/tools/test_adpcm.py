"""adpcm.py の試験。`python -m unittest discover -s sdk/tools`。

期待値は台帳 docs/parts/MSM5205.md（§4.2 のステップ表、§4.3 の OKI の例、§4.6、追補 A.3）と、
エミュレータの試験 emulator/crates/chips/tests/msm5205.rs の値から取る。
"""

import math
import os
import re
import unittest

import adpcm

HERE = os.path.dirname(os.path.abspath(__file__))
LEDGER = os.path.normpath(os.path.join(HERE, "..", "..", "docs", "parts", "MSM5205.md"))


def ledger_step_values():
    """台帳 §4.2 の「値は 16, 17, …, 1552」を読む。"""
    with open(LEDGER, encoding="utf-8") as f:
        text = f.read()
    sec = text.split("### 4.2", 1)[1].split("### 4.3", 1)[0]
    m = re.search(r"値は\s*([0-9][0-9,\s]*[0-9])", sec)
    return [int(v) for v in m.group(1).replace(" ", "").split(",")]


class StepTable(unittest.TestCase):
    def test_matches_the_ledger(self):
        self.assertEqual(list(adpcm.STEP_TABLE), ledger_step_values())

    def test_matches_the_formula(self):
        # floor(16 × 1.1^i) を整数で計算する
        self.assertEqual(list(adpcm.STEP_TABLE), [16 * 11 ** i // 10 ** i for i in range(49)])
        self.assertEqual(len(adpcm.STEP_TABLE), 49)

    def test_index_shift(self):
        self.assertEqual(adpcm.INDEX_SHIFT, (-1, -1, -1, -1, 2, 4, 6, 8))


class Decoder(unittest.TestCase):
    def test_a3_wrap_and_clip(self):
        # 追補 A.3（DR-021）：リセット直後に 0111 を 6 個
        self.assertEqual(adpcm.decode([7] * 6), [30, 93, 229, 522, 1153, -1586])
        self.assertEqual(adpcm.decode([7] * 6, overflow="clip"), [30, 93, 229, 522, 1153, 2047])
        d = adpcm.Decoder()
        steps = []
        for _ in range(6):
            steps.append(d.step)
            d.feed(7)
        self.assertEqual(steps, [0, 8, 16, 24, 32, 40])

    def test_a3_differences(self):
        d = adpcm.Decoder()
        got = []
        for _ in range(6):
            got.append(adpcm.diff(d.step, 7))
            d.feed(7)
        self.assertEqual(got, [30, 63, 136, 293, 631, 1357])

    def test_oki_example_1(self):
        # §4.3 の例 (1)：0,0,0,3,3,3,7,F,7,F,4,C,4,C の後に 4,0,9,C,8,1 を繰り返す → 1221 LSB（1.49 Vp-p）
        head = [0, 0, 0, 3, 3, 3, 7, 15, 7, 15, 4, 12, 4, 12]
        s = adpcm.decode(head + [4, 0, 9, 12, 8, 1] * 40)
        tail = s[-60:]
        self.assertEqual(max(tail) - min(tail), 1221)

    def test_holding_zero_wraps_after_1024(self):
        # §4.6 (a)、msm5205.rs の holding_zero_wraps_after_1024_samples
        s = adpcm.decode([0] * 1024)
        self.assertEqual(s[1022], 2046)
        self.assertEqual(s[1023], -2048)
        self.assertEqual(adpcm.decode([0] * 1024, overflow="clip")[1023], 2047)

    def test_underrun_pattern_stays_near_center(self):
        # §4.6 (b)、DR-017：0x08 の byte（0000 と 1000）
        s = adpcm.decode(adpcm.unpack(bytes([adpcm.UNDERRUN_BYTE]) * 500))
        self.assertTrue(all(0 <= v <= 2 for v in s))

    def test_dac_code(self):
        # msm5205.rs の dac_code_drops_two_low_bits
        d = adpcm.Decoder()
        d.feed(0x7)
        self.assertEqual(d.dac_code(), 7)
        d.feed(0xF)
        self.assertEqual(d.signal, -33)
        self.assertEqual(d.dac_code(), -9)

    def test_reset(self):
        d = adpcm.Decoder()
        d.feed(7)
        d.reset()
        self.assertEqual((d.signal, d.step), (0, 0))


class Packing(unittest.TestCase):
    def test_high_nibble_first(self):
        self.assertEqual(adpcm.pack([1, 2, 3, 4]), bytes([0x12, 0x34]))
        self.assertEqual(adpcm.unpack(bytes([0x12, 0x34])), [1, 2, 3, 4])

    def test_low_first(self):
        self.assertEqual(adpcm.pack([1, 2, 3, 4], low_first=True), bytes([0x21, 0x43]))
        self.assertEqual(adpcm.unpack(bytes([0x21, 0x43]), low_first=True), [1, 2, 3, 4])

    def test_odd_count_is_padded(self):
        self.assertEqual(adpcm.pack([1, 2, 3], pad=8), bytes([0x12, 0x38]))


class RoundTrip(unittest.TestCase):
    def sine(self, freq, rate, amp, n):
        return [amp * math.sin(2 * math.pi * freq * k / rate) for k in range(n)]

    def test_sine_8k(self):
        ref = self.sine(500, 8000, 1200, 4000)
        out = adpcm.decode(adpcm.encode(ref))
        # 立上り（ステップが育つまで）を除いて比べる
        self.assertGreater(adpcm.snr_db(ref[200:], out[200:]), 25)

    def test_sine_low_rate(self):
        ref = self.sine(300, 4000, 900, 2000)
        out = adpcm.decode(adpcm.encode(ref))
        self.assertGreater(adpcm.snr_db(ref[100:], out[100:]), 20)

    def test_depth_2(self):
        ref = self.sine(700, 8000, 1000, 1000)
        out = adpcm.decode(adpcm.encode(ref, depth=2))
        self.assertGreater(adpcm.snr_db(ref[100:], out[100:]), 20)

    def test_never_wraps_on_full_scale(self):
        # 範囲いっぱいの矩形波：符号器は回り込むニブルを選ばない
        ref = ([2047] * 20 + [-2048] * 20) * 20
        nib = adpcm.encode(ref)
        wrap = adpcm.decode(nib)
        clip = adpcm.decode(nib, overflow="clip")
        self.assertEqual(wrap, clip)

    def test_limit(self):
        ref = ([2047] * 20 + [-2048] * 20) * 5
        s = adpcm.decode(adpcm.encode(ref, limit=1500))
        self.assertTrue(all(-1501 <= v <= 1500 for v in s))

    def test_start_state(self):
        # 途中の状態から符号にできる
        ref = self.sine(400, 8000, 800, 800)
        nib = adpcm.encode(ref, signal=100, step=10)
        out = adpcm.decode(nib, signal=100, step=10)
        self.assertGreater(adpcm.snr_db(ref[100:], out[100:]), 20)


if __name__ == "__main__":
    unittest.main()
