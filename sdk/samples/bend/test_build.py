"""SAMPLE 5 の試験：py -m unittest discover -s sdk/samples/bend

組んだ像の見出し（bios-abi.md §04）と大きさ、DOC の波形（0x00 を含まない）、FC の表（台帳 ES5503.md §4.3 の
式を、build.py とは別に計算して比べる）、太鼓の ADPCM の byte の列、格子の DWT のデータを確かめる。
"""

import os
import re
import struct
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import build  # noqa: E402
import adpcm  # noqa: E402  (build.py が sdk/tools を path に足す)

CART = 0x400000


class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        out = os.path.join(cls.tmp.name, "bend.bin")
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
        cls.image = read("bend.bin")
        cls.lst = read("bend.lst", "r")
        cls.inc = read("tables.inc", "r")
        cls.waves = read("waves.bin")
        cls.drums = read("drums.bin")
        cls.pattern = read("pattern.bin")

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def symbol(self, name):
        m = re.search(r"^\s*" + re.escape(name) + r"\s+(?:=\s*)?\$?([0-9A-Fa-f]+)", self.lst, re.M)
        self.assertIsNotNone(m, name)
        return int(m.group(1), 16)

    def at(self, name, n):
        """像の中の、ラベル name から n byte。"""
        off = self.symbol(name) - CART
        self.assertTrue(0 <= off and off + n <= len(self.image), name)
        return self.image[off:off + n]

    # ---- 見出しと大きさ

    def test_header(self):
        magic, version, _reserved, entry, size = struct.unpack(">4sHHLL", self.image[:16])
        self.assertEqual(magic, b"CART")
        self.assertEqual(version, 1)
        self.assertEqual(size, len(self.image))
        self.assertEqual(entry % 2, 0)
        self.assertTrue(CART + 0x20 <= entry < CART + size)
        self.assertEqual(entry, self.symbol("start"))
        self.assertEqual(self.image[16:32], b"SAMPLE 5".ljust(16))

    def test_size(self):
        self.assertEqual(len(self.image) % 2, 0)
        self.assertLessEqual(len(self.image), build.CART_MAX)
        self.assertLess(len(self.image), 64 * 1024)      # 小さいカートリッジ（実際は 12 KiB ほど）

    # ---- DOC の波形

    def test_waves_have_no_zero(self):
        # 0x00 はオシレータを止める（台帳 ES5503.md）。波形 RAM の全 256 頁にこの 4 頁を写す
        self.assertEqual(len(self.waves), 4 * 256)
        self.assertNotIn(0, self.waves)
        self.assertEqual(self.at("wave_data", 1024), self.waves)
        for p, name in enumerate(build.WAVE_NAMES):
            page = self.waves[256 * p:256 * (p + 1)]
            with self.subTest(wave=name):
                self.assertGreaterEqual(min(page), 0x20)
                self.assertLessEqual(max(page), 0xE0)
                self.assertLess(abs(sum(page) / 256 - 0x80), 2)       # 中央はほぼ 0x80

    def test_wave_shapes(self):
        sine, tri, saw, square = (self.waves[256 * p:256 * (p + 1)] for p in range(4))
        self.assertEqual(set(square), {0x20, 0xE0})
        self.assertEqual(square.count(0xE0), 128)
        self.assertEqual(saw[0], min(saw))
        self.assertEqual(saw[255], max(saw))
        self.assertTrue(all(b >= a for a, b in zip(saw, saw[1:])))
        self.assertEqual(sine[64], max(sine))                  # 山は 1/4 周期
        self.assertEqual(tri[64], max(tri))                    # 三角も 0 から上がる
        self.assertEqual(tri[192], min(tri))
        self.assertEqual(sine[192], min(sine))

    # ---- FC の表

    @staticmethod
    def expected_fc(note):
        # 台帳 ES5503.md §4.3：標本化 SR＝CLK÷8÷(N＋2)、256 byte の表で f＝SR×FC÷2^17
        sr = 8_000_000 / 8 / (16 + 2)
        hz = 440.0 * 2.0 ** ((note - 69) / 12)
        return max(1, min(0xFFFF, int(hz * 131072 / sr + 0.5)))

    def test_pitch_table(self):
        table = struct.unpack(">128H", self.at("pitch_fc", 256))
        for n in range(128):
            self.assertEqual(table[n], self.expected_fc(n), n)
        self.assertEqual(table[45], 260)                      # A2：持続音、260×55555.6÷2^17＝110.2 Hz
        for n in build.song_notes() | {build.DRONE_NOTE}:
            with self.subTest(note=n):
                self.assertLess(abs(build.cents(n)), 10)       # 曲で鳴らす音は 10 セント以内

    def test_song_tables(self):
        self.assertEqual(list(self.at("bar_root", len(build.BARS))), [r for _, r, _ in build.BARS])
        self.assertEqual(list(self.at("lead_pat", 16)), list(build.LEAD))
        self.assertEqual(list(self.at("bass_pat", 16)), list(build.BASS))
        self.assertEqual(list(self.at("drum_pat", 8)), list(build.DRUM_PATTERN))
        tones = self.at("bar_tones", 8 * len(build.BARS))
        for i, (_, _, t) in enumerate(build.BARS):
            self.assertEqual(list(tones[8 * i:8 * i + len(t)]), list(t))
        self.assertTrue(all(i < 5 for i in build.LEAD))

    # ---- 太鼓（ADPCM）

    def sounds(self):
        count = int(re.search(r"DRUM_COUNT\s+EQU\s+(\d+)", self.inc).group(1))
        for i in range(count):
            off = int(re.search(rf"DRUM{i}_OFF\s+EQU\s+\$([0-9A-F]+)", self.inc).group(1), 16)
            ln = int(re.search(rf"DRUM{i}_LEN\s+EQU\s+(\d+)", self.inc).group(1))
            yield i, self.drums[off:off + ln], off, ln

    def test_drums(self):
        self.assertEqual(len(list(self.sounds())), len(build.DRUMS))
        for i, data, off, ln in self.sounds():
            with self.subTest(drum=build.DRUMS[i][0]):
                self.assertEqual(off % 2, 0)
                self.assertEqual(len(data), ln)
                self.assertTrue(data.startswith(build.LEAD_IN))
                self.assertTrue(data.endswith(build.TAIL))
                self.assertLess(ln, 0x10000)                   # FAB_COUNT は 16 bit
                self.assertLessEqual(ln * 2 / build.ADPCM_RATE, build.DRUM_MAX_SECONDS)
                nib = adpcm.unpack(data)
                wrap = adpcm.decode(nib)
                self.assertEqual(wrap, adpcm.decode(nib, overflow="clip"))
                self.assertLessEqual(max(abs(v) for v in wrap), build.LIMIT + 1)
                self.assertGreater(max(abs(v) for v in wrap), build.PEAK // 4)   # 鳴る
                # 古いニブル（0 か 8）が 1 つ先に復号されても、ずれは ±2 のまま回り込まない
                for stale in (0x0, 0x8):
                    shifted = adpcm.decode([stale] + nib)[1:]
                    d = {a - b for a, b in zip(shifted, wrap)}
                    self.assertEqual(len(d), 1)
                    self.assertEqual(abs(d.pop()), 2)

    def test_drums_in_image(self):
        self.assertEqual(self.at("drum_data", len(self.drums)), self.drums)
        # Staging RAM の $1000 から写す（main.s の DRUM_STAGE）。64 KiB に収まる
        self.assertLessEqual(0x1000 + len(self.drums) + 4, 0x10000)

    # ---- 格子

    def test_pattern(self):
        self.assertEqual(len(self.pattern), build.GRID_ROWS * 64 * 2)
        self.assertEqual(self.at("pattern_data", len(self.pattern)), self.pattern)
        words = struct.unpack(f">{len(self.pattern) // 2}H", self.pattern)
        self.assertEqual(words[0], 0x0F0F)          # 市松：左の画素（下位ニブル）が 15
        self.assertEqual(words[64], 0xF0F0)         # 次の行は逆
        self.assertEqual(words[32], 0xEEEE)         # 方眼の最初の行は線（色 14）
        self.assertEqual(words[64 + 32], 0x000E)    # 次の行は x＝128 の縦の線だけ
        self.assertEqual(words[63] >> 12, 0xE)      # 右端（x＝255）の線


if __name__ == "__main__":
    unittest.main()
