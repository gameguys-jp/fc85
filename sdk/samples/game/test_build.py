"""サンプルのカートリッジの試験：py -m unittest discover -s sdk/samples/game

組んだ像の見出し（bios-abi.md §04）と、効果音の byte の列（README.md §04）を確かめる。
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


class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        out = os.path.join(cls.tmp.name, "game.bin")
        devnull = open(os.devnull, "w", encoding="utf-8")
        old = sys.stdout
        sys.stdout = devnull
        try:
            rc = build.main(["-o", out])
        finally:
            sys.stdout = old
            devnull.close()
        assert rc == 0
        with open(out, "rb") as f:
            cls.image = f.read()
        with open(os.path.join(cls.tmp.name, "sfx.inc"), encoding="utf-8") as f:
            cls.inc = f.read()
        with open(os.path.join(cls.tmp.name, "sfx.bin"), "rb") as f:
            cls.sfx = f.read()

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_header(self):
        magic, version, _reserved, entry, size = struct.unpack(">4sHHLL", self.image[:16])
        self.assertEqual(magic, b"CART")
        self.assertEqual(version, 1)
        self.assertEqual(size, len(self.image))
        self.assertEqual(entry % 2, 0)
        self.assertTrue(0x400000 + 0x20 <= entry < 0x400000 + size)
        self.assertEqual(self.image[16:32], b"SAMPLE 1".ljust(16))

    def test_fits_window(self):
        self.assertLessEqual(len(self.image), build.CART_MAX)

    def sounds(self):
        count = int(re.search(r"SFX_COUNT\s+EQU\s+(\d+)", self.inc).group(1))
        for i in range(count):
            off = int(re.search(rf"SFX{i}_OFF\s+EQU\s+\$([0-9A-F]+)", self.inc).group(1), 16)
            ln = int(re.search(rf"SFX{i}_LEN\s+EQU\s+(\d+)", self.inc).group(1))
            ctrl = int(re.search(rf"SFX{i}_CTRL\s+EQU\s+\$([0-9A-F]+)", self.inc).group(1), 16)
            yield i, self.sfx[off:off + ln], ctrl, off

    def test_sounds(self):
        self.assertEqual(len(list(self.sounds())), len(build.SOUNDS))
        for i, data, ctrl, off in self.sounds():
            with self.subTest(sound=build.SOUNDS[i][0]):
                self.assertEqual(off % 2, 0)
                self.assertEqual(ctrl, build.RATES[build.SOUNDS[i][2]])
                self.assertTrue(data.startswith(build.LEAD_IN))
                self.assertTrue(data.endswith(build.TAIL))
                nib = adpcm.unpack(data)
                wrap = adpcm.decode(nib)
                self.assertEqual(wrap, adpcm.decode(nib, overflow="clip"))
                self.assertLessEqual(max(abs(v) for v in wrap), build.LIMIT + 1)
                # 古いニブル（0 か 8）が 1 つ先に復号されても、ずれは ±2 のまま回り込まない
                for stale in (0x0, 0x8):
                    shifted = adpcm.decode([stale] + nib)[1:]
                    d = {a - b for a, b in zip(shifted, wrap)}
                    self.assertEqual(len(d), 1)
                    self.assertEqual(abs(d.pop()), 2)

    def test_sfx_data_in_image(self):
        self.assertIn(self.sfx, self.image)


if __name__ == "__main__":
    unittest.main()
