"""C で書いた Temporal RAM の残像のサンプル SAMPLE 8 の試験：py -m unittest discover -s sdk/samples/echo

68000 の GCC があれば組み、見出し（bios-abi.md §04）を確かめる。エミュレータの窓口（fc85-api）と Boot ROM も
あれば動かし、主ループの 1 回ごと（v_busy が 0 に戻ったところ）に VRAM の 4 つの矩形を読む。右の k 番目の矩形は、
k×d 回前の左の矩形と語まで同じ（Temporal RAM に上位・下位の byte で記録し、DWT と DMOD の OR で戻す）。
A で HOLD にすると記録は止まり、ポインタは進むので、右の矩形には 64 回（64 KiB）前の周の記録が出る。
主ループは 1 フレームに 1 回回り、Fabric と ACRTC の誤りは 0。
"""

import os
import struct
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(ROOT, "sdk", "tools"))

import build  # noqa: E402
import m68kgcc  # noqa: E402

TC = m68kgcc.find_toolchain()
T_W, T_H, T_Y = 64, 32, 56
WORDS = T_W // 4
LAP = 0x10000 // (2 * WORDS * T_H)      # 64 回で Temporal RAM を 1 周
DELAY = 8


@unittest.skipIf(TC is None, "68000 の GCC がない（sdk/docs/c.md §02）")
class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.bin = os.path.join(cls.tmp.name, "echo.bin")
        devnull = open(os.devnull, "w", encoding="utf-8")
        old = sys.stdout
        sys.stdout = devnull
        try:
            rc = build.main(["-o", cls.bin])
        finally:
            sys.stdout = old
            devnull.close()
        assert rc == 0
        with open(cls.bin, "rb") as f:
            cls.image = f.read()

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_header(self):
        magic, version, _flags, entry, size = struct.unpack(">IHHII", self.image[:16])
        self.assertEqual(magic, 0x43415254)
        self.assertEqual(version, 1)
        self.assertTrue(0x400000 <= entry < 0x500000 and entry % 2 == 0)
        self.assertEqual(size, len(self.image))
        self.assertEqual(self.image[16:32], b"SAMPLE 8        ")

    def test_runs_on_the_machine(self):
        from fc85emu import Emulator, find_api
        if not find_api(ROOT):
            self.skipTest("fc85-api がない")
        if not os.path.isfile(os.path.join(ROOT, "firmware", "boot", "build", "boot.bin")):
            self.skipTest("Boot ROM がない")
        with Emulator(ROOT) as emu:
            emu.call("load", cart=self.bin)

            def peek(name, size=2):
                return int(emu.call("peek", space="cpu", addr=name, len=size)["hex"], 16)

            def rects():
                """4 つの矩形の語（矩形ごとに、行ごとに左から）。"""
                h = emu.call("peek", space="vram", addr=T_Y * 64 * 2, len=T_H * 64 * 2)["hex"]
                vram = [int(h[i:i + 4], 16) for i in range(0, len(h), 4)]
                return [tuple(vram[y * 64 + k * WORDS + i] for y in range(T_H) for i in range(WORDS))
                        for k in range(4)]

            def loop_end():
                """主ループの 1 回が終わるところまで進め、(回の数, HOLD, 4 つの矩形) を返す。"""
                for _ in range(4):
                    r = emu.call("run", frames=2, watch=["v_busy"], stop_on_bus_error=True)
                    self.assertEqual(r["bus_errors"], 0)
                    self.assertEqual(r["stop"], "watch", "主ループが回らない")
                    if peek("v_busy") == 0:
                        return peek("v_frame", 4), peek("v_hold"), rects()
                self.fail("v_busy が 0 に戻らない")

            emu.call("run", frames=5, stop_on_bus_error=True)
            live, hold, seen = {}, {}, []

            def sample(n):
                for _ in range(n):
                    i, h, r = loop_end()
                    live[i], hold[i] = r[0], h
                    seen.append((i, r))

            with self.subTest("1 フレームに 1 回回る"):
                f0 = peek("v_frame", 4)
                emu.call("run", frames=60, stop_on_bus_error=True)
                self.assertEqual(peek("v_frame", 4) - f0, 60)
                self.assertEqual(peek("v_errors"), 0)
            sample(LAP + 4)

            with self.subTest("k×d 回前の記録"):
                n, r = seen[-1]
                self.assertNotEqual(live[n], live[n - 1], "玉が動いている")
                for k in (1, 2, 3):
                    self.assertEqual(r[k], live[n - k * DELAY], f"k={k}")
                self.assertEqual(peek("v_errors"), 0)

            with self.subTest("HOLD"):
                emu.call("run", frames=3, input=[[0, "A"], [1, 0]], stop_on_bus_error=True)
                self.assertEqual(peek("v_hold"), 1)
                start = len(seen)
                sample(3 * DELAY + 4)
                checked = wrapped = 0
                for n, r in seen[start:]:
                    for k in (1, 2, 3):
                        # 記録の番地は回ごとに進み、LAP 回で 1 周する。HOLD の回は書かないので、前の周の値が残る
                        m = n - k * DELAY
                        while m in hold and hold[m]:
                            m -= LAP
                        if m in live:
                            self.assertEqual(r[k], live[m], f"n={n} k={k} m={m}")
                            checked += 1
                            wrapped += m != n - k * DELAY
                self.assertGreater(checked, 3 * DELAY, "HOLD の後の記録を比べた")
                self.assertGreater(wrapped, DELAY, "HOLD の回の記録は前の周の値")
