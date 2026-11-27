"""C で書いたラスタのパレットのサンプル SAMPLE 7 の試験：py -m unittest discover -s sdk/samples/raster

68000 の GCC があれば組み、見出し（bios-abi.md §04）を確かめる。エミュレータの窓口（fc85-api）と Boot ROM も
あれば動かし、画面の PNG で帯を確かめる：HBlank の割込みで、色 0 が画面の y＝8k（k＝0〜23）から替わる
（ラスタ L で比べた色は L＋2 から。video.h）。帯の色は虹の階調（main.c の make_gradient）を続けて並べたもの。
A で DIRECT（Bend）にすると、境がラスタ 8k−1 の途中に移る。B で DOC の音が鳴る。
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
sys.path.insert(0, os.path.join(ROOT, "sdk", "c"))

import build  # noqa: E402
import m68kgcc  # noqa: E402
from test_devices import png_rows  # noqa: E402

TC = m68kgcc.find_toolchain()
BAND, BANDS, STEPS = 8, 24, 32


def tri(i):
    i &= STEPS - 1
    return i if i < 16 else 31 - i


def gradient():
    """main.c の make_gradient と同じ虹（$0RGB）。"""
    return [tri(i) << 8 | tri(i + 11) << 4 | tri(i + 21) for i in range(STEPS)]


def rgb(c):
    """$0RGB → 画面の (r, g, b)。4 bit を 17 倍する。"""
    return ((c >> 8 & 15) * 17, (c >> 4 & 15) * 17, (c & 15) * 17)


def runs(column):
    out = []
    for y, c in enumerate(column):
        if not out or out[-1][1] != c:
            out.append((y, c))
    return out


class GradientTest(unittest.TestCase):
    def test_neighbours_differ(self):
        g = gradient()
        self.assertEqual(len(set(g)), STEPS)
        for i in range(STEPS):
            self.assertNotEqual(g[i], g[(i + 1) % STEPS])


@unittest.skipIf(TC is None, "68000 の GCC がない（sdk/docs/c.md §02）")
class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.bin = os.path.join(cls.tmp.name, "raster.bin")
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
        self.assertEqual(self.image[16:32], b"SAMPLE 7        ")

    def check_bands(self, screen):
        """列 x＝250（字のない列）の色の移り変わり：y＝8k から帯 k、色は階調を続けて並べたもの。"""
        r = runs([row[250] for row in screen])
        self.assertEqual([y for y, _ in r], [BAND * k for k in range(BANDS)])
        colours = [rgb(c) for c in gradient()]
        first = colours.index(r[0][1])
        self.assertEqual([c for _, c in r], [colours[(first + k) % STEPS] for k in range(BANDS)])
        return first

    def test_runs_on_the_machine(self):
        from fc85emu import Emulator, find_api
        if not find_api(ROOT):
            self.skipTest("fc85-api がない")
        if not os.path.isfile(os.path.join(ROOT, "firmware", "boot", "build", "boot.bin")):
            self.skipTest("Boot ROM がない")
        with Emulator(ROOT) as emu:
            emu.call("load", cart=self.bin)

            def peek(name):
                return int(emu.call("peek", space="cpu", addr=name, len=2)["hex"], 16)

            r = emu.call("run", frames=60, stop_on_bus_error=True)
            self.assertEqual(r["bus_errors"], 0)
            self.assertEqual(peek("v_grad"), gradient()[0])
            first = self.check_bands(png_rows(emu.screenshot()))

            with self.subTest("帯は毎フレーム 23 回の HBlank。主ループも毎フレーム回る"):
                def frame():
                    return int(emu.call("peek", space="cpu", addr="v_frame", len=4)["hex"], 16)
                n0, f0 = peek("v_hblanks"), frame()
                emu.call("run", frames=10, stop_on_bus_error=True)
                self.assertEqual(peek("v_hblanks") - n0, (BANDS - 1) * 10)
                self.assertEqual(frame() - f0, 10)

            with self.subTest("流れる"):
                emu.call("run", frames=8, stop_on_bus_error=True)   # 速さ 4：8 フレームで 2 帯
                later = self.check_bands(png_rows(emu.screenshot()))
                self.assertNotEqual(later, first)

            with self.subTest("A で DIRECT"):
                emu.call("run", frames=4, input=[[0, "A"], [2, 0]], stop_on_bus_error=True)
                self.assertEqual(peek("v_direct"), 1)
                screen = png_rows(emu.screenshot())
                colours = [rgb(c) for c in gradient()]
                first = colours.index(screen[0][0])
                for k in range(1, BANDS):
                    row = screen[BAND * k - 1]
                    before, after = colours[(first + k - 1) % STEPS], colours[(first + k) % STEPS]
                    self.assertEqual(row[0], before, f"y={BAND * k - 1}")
                    # 試験では x＝199〜212 の辺り。割り込まれた命令で数ドット動く
                    x = row.index(after)
                    self.assertTrue(0 < x and row[-1] == after, f"y={BAND * k - 1} の途中で変わる（x={x}）")
                    self.assertEqual(screen[BAND * k][0], after)

            with self.subTest("B で音"):
                r = emu.call("run", frames=30, input=[[0, "B"], [2, 0]], stop_on_bus_error=True)
                if peek("v_doc"):
                    self.assertEqual(peek("v_tone"), 1)
                    self.assertGreater(max(r["audio"]["peak"]), 1000)

            with self.subTest("START で戻す"):
                emu.call("run", frames=4, input=[[0, "START"], [2, 0]], stop_on_bus_error=True)
                self.assertEqual([peek("v_direct"), peek("v_tone"), peek("v_speed")], [0, 0, 4])
                emu.call("run", frames=2, stop_on_bus_error=True)
                self.check_bands(png_rows(emu.screenshot()))


if __name__ == "__main__":
    unittest.main()
