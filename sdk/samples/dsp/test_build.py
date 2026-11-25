"""DSP のサンプルの試験：py -m unittest discover -s sdk/samples/dsp

組んだ像の見出し（bios-abi.md §04）、DSP のプログラムの置き場所、FIR の設計、モデル（model.py）の値の範囲、
1 標本の処理のサイクル数を確かめる。エミュレータとの照合は verify_wav.py（README.md §07）。
"""

import os
import re
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import build  # noqa: E402
import model  # noqa: E402

CYCLES_PER_SAMPLE = 5_000_000 // build.RATE        # CLKOUT 5 MHz ÷ 16 kHz ＝ 312（台帳 §1）


class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        out = os.path.join(cls.tmp.name, "dsp.bin")
        devnull = open(os.devnull, "w", encoding="utf-8")
        old = sys.stdout
        sys.stdout = devnull
        try:
            rc = build.main(["-o", out])
        finally:
            sys.stdout = old
            devnull.close()
        assert rc == 0

        def read(name, mode="rb"):
            enc = None if "b" in mode else "utf-8"
            with open(os.path.join(cls.tmp.name, name), mode, encoding=enc) as f:
                return f.read()
        cls.image = read("dsp.bin")
        cls.code = read("effects.bin")
        cls.source = read("source.bin")
        cls.lst = read("effects.lst", "r")
        cls.src, cls.taps = model.load_build(cls.tmp.name)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_header(self):
        self.assertEqual(self.image[0:4], b"CART")
        self.assertEqual(self.image[16:32], b"SAMPLE 2        ")
        entry = int.from_bytes(self.image[8:12], "big")
        self.assertTrue(0x400020 <= entry < 0x400000 + len(self.image))
        self.assertEqual(int.from_bytes(self.image[12:16], "big"), len(self.image))

    def test_dsp_program_is_in_the_image(self):
        self.assertEqual(len(self.code) % 2, 0)
        self.assertLessEqual(len(self.code) // 2, model.RING_START)
        at = self.image.find(self.code)
        self.assertGreater(at, 0)
        self.assertEqual(at % 2, 0)
        # 0 番地は B init、2 番地は RET（台帳 §4.4 のベクタ）
        self.assertEqual(self.code[0], 0xF9)
        self.assertEqual(self.code[4:6], bytes([0x7F, 0x8D]))

    def test_source(self):
        self.assertEqual(len(self.source), build.RATE * build.SECONDS)
        self.assertEqual(self.image.find(self.source) % 2, 0)
        self.assertGreater(min(self.source), 0)         # 0 は THRU で 1 になる（effects.dsp）
        self.assertLessEqual(max(abs(b - 128) for b in self.source), round(build.SRC_PEAK * 127))

    def test_fir(self):
        h = self.taps
        self.assertEqual(len(h), 31)
        self.assertEqual(sum(h), 4096)
        self.assertEqual(h, h[::-1])
        self.assertTrue(all(-4096 <= v <= 4095 for v in h))
        self.assertGreater(build.fir_response(h, 300), -1.0)
        self.assertLess(build.fir_response(h, 3000), -40.0)
        # 最悪の入力でも ACC＞＞12 が 16 bit に収まる
        xmax = max(abs(model.fabric_input(b)) for b in self.source)
        self.assertLess(sum(abs(v) for v in h) * xmax, 32768 * 4096)

    def test_model_conversions(self):
        self.assertEqual(model.fabric_input(0x80), 0)
        self.assertEqual(model.fabric_input(0xFF), 0x7F00)
        self.assertEqual(model.fabric_input(0x00), -32768)
        for b in range(1, 256):
            self.assertEqual(model.dac_byte(model.fabric_input(b)), b)      # THRU は入力の byte のまま
        self.assertEqual(model.dac_byte(model.fabric_input(0)), 1)
        self.assertEqual(model.dac_word(0), 0x8080)

    def test_model_ranges(self):
        """2 周（4 秒）回して、LPF の ACC とエコーが 16 bit の外へ出ないこと。"""
        m = model.Model(self.taps)
        hi = 0
        for i in range(2 * len(self.src)):
            x = model.fabric_input(self.src[i % len(self.src)])
            m.xn[0] = x
            acc = sum(m.xn[k] * self.taps[k] for k in range(31))
            self.assertLess(abs(acc >> 12), 32768)
            _, y_lpf, w = m.outputs(self.src[i % len(self.src)])
            hi = max(hi, abs(w))
        self.assertLessEqual(hi, max(abs(model.fabric_input(b)) for b in self.src))

    def test_echo_delay(self):
        """インパルスを入れると、D、2D、3D… の後に半分ずつで返る。"""
        src = bytes([128 + 64]) + bytes([128]) * 9999
        out = model.all_outputs(self.taps, src, 3 * 3072 + 1)[model.MODE_ECHO]
        self.assertEqual(out[0], 128 + 32)
        self.assertEqual(out[3072], 128 + 16)
        self.assertEqual(out[2 * 3072], 128 + 8)
        self.assertEqual(out[3 * 3072], 128 + 4)
        self.assertEqual(sum(1 for v in out if v != 128), 4)

    def test_cycles_per_sample(self):
        """idle から B idle まで（どの分岐も通ると数えた上限）が 1 標本の時間に収まる。"""
        total = 0
        sym = dict(re.findall(r"^  (\S+)\s+\$([0-9A-F]{4})", self.lst, re.M))
        start, end = int(sym["idle"], 16), int(sym["command"], 16)
        for m in re.finditer(r"^([0-9A-F]{3})  [0-9A-F]{4}(?: [0-9A-F]{4})?\s+(\d)\s", self.lst, re.M):
            a = int(m.group(1), 16)
            if start <= a < end:
                total += int(m.group(2))
        self.assertLess(total, CYCLES_PER_SAMPLE // 2)


if __name__ == "__main__":
    unittest.main()
