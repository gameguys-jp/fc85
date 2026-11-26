"""ACRTC の表示のサンプルの試験：py -m unittest discover -s sdk/samples/video

組んだ像の見出し（bios-abi.md §04）と、レジスタの値（台帳 HD63484.md §3.4 の案B、U85 p.89–94）を確かめる。
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


class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        out = os.path.join(cls.tmp.name, "video.bin")
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
        with open(os.path.join(cls.tmp.name, "video.lst"), encoding="utf-8") as f:
            cls.lst = f.read()

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def symbol(self, name):
        m = re.search(rf"^\s+{name}\s+\$([0-9A-Fa-f]+)\s*$", self.lst, re.M)
        self.assertIsNotNone(m, name)
        return int(m.group(1), 16)

    def test_header(self):
        magic, version, _flags, entry, size = struct.unpack(">IHHII", self.image[:16])
        self.assertEqual(magic, 0x43415254)
        self.assertEqual(version, 1)
        self.assertTrue(0x400000 <= entry < 0x500000 and entry % 2 == 0)
        self.assertEqual(size, len(self.image))
        self.assertEqual(self.image[16:32], b"SAMPLE 4        ")

    def test_window_and_cursor_registers(self):
        # ウィンドウ：x 160 は表示の頭（メモリサイクル 32）から 80 メモリサイクル。HWS＝112−13＝99（書く値は 98）、
        # HWW＝32（31）。VWS は VSYNC の終わりから 51−3＝48（47）
        self.assertEqual(self.symbol("WIN_HWR"), 0x621F)
        self.assertEqual(self.symbol("WIN_VWS"), 47)
        # カーソル：x 112〜127 はメモリサイクル 88〜95。CYS＝140−3
        self.assertEqual(self.symbol("CUR_GCRX"), 0x5F58)
        self.assertEqual(self.symbol("CUR_CYS"), 137)
        self.assertEqual(self.symbol("DCR_ALL"), 0x7F00)


if __name__ == "__main__":
    unittest.main()
