"""実験パッチのサンプルの試験：py -m unittest discover -s sdk/samples/patches

組んだ像の見出し（bios-abi.md §04）と、main.s の変数の置き場所（エミュレータの試験 console_boot.rs が
v_page と v_errors の番地を使う）を確かめる。動きの照合はエミュレータの試験
`the_patch_cartridge_runs_every_page`（README.md §04）。
"""

import os
import re
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
        out = os.path.join(cls.tmp.name, "patches.bin")
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
        with open(os.path.join(cls.tmp.name, "patches.lst"), encoding="utf-8") as f:
            cls.lst = f.read()

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def symbol(self, name):
        m = re.search(r"^\s*" + re.escape(name) + r"\s+(?:=\s*)?\$?([0-9A-Fa-f]+)", self.lst, re.M)
        self.assertIsNotNone(m, name)
        return int(m.group(1), 16)

    def test_header(self):
        self.assertEqual(self.image[0:4], b"CART")
        self.assertEqual(self.image[16:32], b"SAMPLE 3        ")
        entry = int.from_bytes(self.image[8:12], "big")
        self.assertTrue(0x400020 <= entry < 0x400000 + len(self.image))
        self.assertEqual(int.from_bytes(self.image[12:16], "big"), len(self.image))

    def test_variables_used_by_the_emulator_test(self):
        self.assertEqual(self.symbol("v_page"), 0x100800)
        self.assertEqual(self.symbol("v_errors"), 0x100800 + 28)


if __name__ == "__main__":
    unittest.main()
