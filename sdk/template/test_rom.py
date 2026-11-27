#!/usr/bin/env python3
"""この ROM の試験。`py -m unittest` で走る。

1. 組めて、見出しが正しい。
2. エミュレータの窓口（fc85-api）で動かし、バスエラーと DOC の窓の規約の警告がなく、入力に応える。
   窓口のプログラムか Boot ROM がなければ飛ばす（emulator/ で `cargo build --release -p fc85-api`、
   `py firmware/boot/build.py`、`py firmware/charrom/build.py`）。

自分の ROM にしたら、2 の試験を自分の画面と音に合わせて書き足す（sdk/docs/emulator-api.md）。
"""

import os
import struct
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import build  # noqa: E402

ROOT = build.find_root()
if ROOT:
    sys.path.insert(0, os.path.join(ROOT, "sdk", "tools"))


class TestRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.bin = os.path.join(cls.tmp.name, "rom.bin")
        if build.main(["-o", cls.bin]) != 0:
            raise AssertionError("組めない")
        with open(cls.bin, "rb") as f:
            cls.image = f.read()

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
        self.assertTrue(all(0x20 <= b < 0x7F for b in self.image[16:32]), "題は 16 byte の ASCII")

    def test_runs_and_answers_the_pad(self):
        from fc85emu import Emulator, find_api
        if not find_api(ROOT):
            self.skipTest("fc85-api がない")
        if not os.path.isfile(os.path.join(ROOT, "firmware", "boot", "build", "boot.bin")):
            self.skipTest("Boot ROM がない")
        with Emulator(ROOT) as emu:
            emu.call("load", cart=self.bin)     # 横の rom.lst も読むので、ラベルで番地を書ける
            r = emu.call("run", frames=60, stop_on_bus_error=True)
            self.assertEqual(r["stop"], "frames")
            self.assertEqual(r["bus_errors"], 0)
            self.assertEqual(r["doc_rule_warnings"], 0)
            self.assertEqual(r["acrtc_rule_warnings"], 0)
            frames = int(emu.call("peek", space="cpu", addr="v_frame", len=4)["hex"], 16)
            self.assertGreater(frames, 0, "入口に来ていない（見出しか BIOS の版を見る）")

            def x():
                return int(emu.call("peek", space="cpu", addr="v_x", len=2)["hex"], 16)

            home = x()
            shot = r["screen_hash"]
            r = emu.call("run", frames=6, input=[[0, "RIGHT"], [4, 0]])
            self.assertGreater(x(), home, "右で「*」が動かない")
            self.assertNotEqual(r["screen_hash"], shot)
            emu.call("run", frames=4, input=[[0, "START"], [2, 0]])
            self.assertEqual(x(), home, "START で真ん中に戻らない")
            # 1 フレームの予算：BIOS_WAIT_VBL は STOP で待つので、空いた時間は CPU の空きに入る
            self.assertLess(emu.call("run", frames=30)["budget"]["cpu_busy_max"], 0.9)


if __name__ == "__main__":
    unittest.main()
