"""MIDI で弾く DOC のシンセのサンプル SAMPLE 9 の試験：py -m unittest discover -s sdk/samples/midi

FC の表（main.c の s_fc_top）が、DOC の式（台帳 ES5503.md §4.3：f＝SR×FC÷2^(17＋R)、8 MHz・走査 16 で
SR＝1 MHz÷18）と平均律（ノート 69＝440 Hz）から計算した値と同じかを確かめる。68000 の GCC があれば組み、
見出しを確かめる。エミュレータの窓口（fc85-api）と Boot ROM もあれば、窓口の midi の手順（DR-126）で MIDI IN に
byte を置いて動かし、DOC のオシレータ（周波数、音量、制御のモードと CA）、音の左右、MIDI OUT、画面の鍵盤を
確かめる。
"""

import math
import os
import re
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
SR = 1e6 / 18                  # DOC 8 MHz、走査 16（bios-abi.md §06）
R = 2                          # 1024 byte のテーブル（T＝R＝2）
CA_IDLE = 15
MODE_SYNC = 0x04               # 制御の M1 M0＝10（偶数番は同期、奇数番は AM）
RED = (255, 68, 68)            # 既定のパレットの色 3（$F44）
KEY_X, KEY_W = 2, 12


def fc_top():
    with open(os.path.join(HERE, "main.c"), encoding="utf-8") as f:
        src = f.read()
    body = re.search(r"s_fc_top\[12\] = \{([^}]*)\}", src).group(1)
    return [int(x) for x in re.findall(r"\d+", body)]


def fc_of_note(n):
    """main.c の fc_of_note と同じ計算。"""
    n = max(0, min(131, n))
    o, s = divmod(n, 12)
    sh = 10 - o
    b = fc_top()[s]
    return (b + (1 << (sh - 1))) >> sh if sh else b


def key_x(k):
    o, s = divmod(k, 12)
    x = KEY_X + KEY_W * (7 * o + [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6][s])
    return x + KEY_W - 4 if s in (1, 3, 6, 8, 10) else x


class TableTest(unittest.TestCase):
    def test_fc_table_follows_the_doc_formula(self):
        want = [round(440 * 2 ** ((n - 69) / 12) * 2 ** (17 + R) / SR) for n in range(120, 132)]
        self.assertEqual(fc_top(), want)

    def test_notes_are_in_tune(self):
        for n in range(21, 109):                # ピアノの音域
            f = SR * fc_of_note(n) / 2 ** (17 + R)
            cents = 1200 * math.log2(f / (440 * 2 ** ((n - 69) / 12)))
            self.assertLess(abs(cents), 4, f"ノート {n}")
        self.assertEqual(fc_of_note(69), 4152)


@unittest.skipIf(TC is None, "68000 の GCC がない（sdk/docs/c.md §02）")
class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.bin = os.path.join(cls.tmp.name, "midi.bin")
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
        self.assertEqual(self.image[16:32], b"SAMPLE 9        ")

    def emulator(self):
        from fc85emu import Emulator, find_api
        if not find_api(ROOT):
            self.skipTest("fc85-api がない")
        if not os.path.isfile(os.path.join(ROOT, "firmware", "boot", "build", "boot.bin")):
            self.skipTest("Boot ROM がない")
        return Emulator(ROOT)

    def run_frames(self, emu, frames, **kw):
        r = emu.call("run", frames=frames, stop_on_bus_error=True, **kw)
        self.assertEqual(r["bus_errors"], 0)
        return r

    def test_plays_midi_on_the_doc(self):
        with self.emulator() as emu:
            emu.call("load", cart=self.bin)
            self.run_frames(emu, 30)

            def osc(i):
                return emu.call("doc")["oscillators"][i]

            def ca(o):
                return o["control"] >> 4

            def running():
                return [i for i, o in enumerate(emu.call("doc")["oscillators"][:16]) if o["on"]]

            with self.subTest("起動"):
                st = emu.call("doc")
                self.assertEqual(st["enabled"], 16)
                self.assertEqual(running(), [])
                self.assertEqual({ca(o) for o in st["oscillators"][:16]}, {CA_IDLE}, "鳴らさないオシレータは CA 15")

            with self.subTest("ノートオン：声ごとに CA を 1 つ"):
                emu.call("midi", **{"in": [0x90, 69, 100]})
                left = self.run_frames(emu, 5)["audio"]["peak"]
                o = osc(0)
                self.assertEqual((o["on"], o["freq"], ca(o), o["wave_size"]), (True, 4152, 0, 0x12))
                self.assertTrue(100 < o["vol"] <= 163, o["vol"])
                self.assertGreater(left[0], 1000, "CA 0 は L")
                self.assertLess(left[1], 100)
                emu.call("midi", **{"in": [64, 90]})    # ランニングステータス
                right = self.run_frames(emu, 5)["audio"]["peak"]
                o = osc(2)
                self.assertEqual((o["on"], o["freq"], ca(o)), (True, fc_of_note(64), 1))
                self.assertGreater(right[1], 1000, "CA 1 は R")

            with self.subTest("ピッチベンド"):
                emu.call("midi", **{"in": [0xE0, 0x00, 0x60]})     # ＋4096 → ＋1 半音
                self.run_frames(emu, 2)
                self.assertEqual(osc(0)["freq"], fc_of_note(70))
                emu.call("midi", **{"in": [0xE0, 0x00, 0x40]})
                self.run_frames(emu, 2)
                self.assertEqual(osc(0)["freq"], 4152)

            with self.subTest("ノートオフ：離して止め、CA 15 に戻す"):
                emu.call("midi", **{"in": [0x80, 69, 0, 64, 0]})
                self.run_frames(emu, 60)
                self.assertEqual(running(), [])
                self.assertEqual({ca(o) for o in emu.call("doc")["oscillators"][:16]}, {CA_IDLE})

            with self.subTest("SYNC：偶数番が同期のモードで、上の奇数番を頭に戻す"):
                emu.call("midi", **{"in": [0xC0, 3, 0x90, 57, 120]})
                self.run_frames(emu, 3)
                on = running()
                self.assertEqual(len(on), 2)
                m, s = on                                   # 声 v の (2v, 2v＋1)
                self.assertEqual((m % 2, s), (0, m + 1))
                master, slave = osc(m), osc(s)
                self.assertEqual(master["control"] & 0x07, MODE_SYNC)
                self.assertEqual((master["on"], master["vol"], ca(master)), (True, 0, CA_IDLE))
                self.assertEqual(master["freq"], fc_of_note(57))
                self.assertEqual((slave["on"], ca(slave), slave["control"] & 0x07), (True, m // 2, 0))
                self.assertGreater(slave["freq"], master["freq"] * 1.5, "掃引の始まり")
                self.run_frames(emu, 90)
                self.assertEqual(osc(s)["freq"], fc_of_note(57), "掃引が終わる")
                emu.call("midi", **{"in": [0xB0, 1, 127]})
                peak = self.run_frames(emu, 2)["audio"]["peak"]
                self.assertAlmostEqual(osc(s)["freq"] / osc(m)["freq"], 2 ** (1524 / 64 / 12), delta=0.02)
                self.assertGreater(max(peak), 1000)
                emu.call("midi", **{"in": [0xB0, 1, 0, 123, 0]})
                self.run_frames(emu, 2)
                self.assertEqual(running(), [])

            with self.subTest("AM：奇数番の波形が上の偶数番の音量になる"):
                emu.call("midi", **{"in": [0xC0, 4, 0x90, 57, 120]})
                self.run_frames(emu, 2)
                on = running()
                self.assertEqual(len(on), 2)
                m, c = on                                   # 声 v の (2v＋1, 2v＋2)
                self.assertEqual((m % 2, c), (1, m + 1))
                mod, car = osc(m), osc(c)
                self.assertEqual((mod["on"], mod["control"] & 0x07, ca(mod)), (True, MODE_SYNC, CA_IDLE))
                self.assertEqual((car["on"], ca(car), car["freq"]), (True, m // 2, fc_of_note(57)))
                vols = set()
                for _ in range(8):
                    emu.call("step", count=37)
                    vols.add(osc(c)["vol"])
                self.assertGreater(len(vols), 3, f"音量が動く：{vols}")
                emu.call("midi", **{"in": [0x80, 57, 0]})
                self.run_frames(emu, 2)
                self.assertEqual(running(), [], "AM は離すとすぐ止める（音量の包絡を持てない）")
                emu.call("midi", **{"in": [0xC0, 0]})
                self.run_frames(emu, 1)

            with self.subTest("THRU とパッド"):
                self.run_frames(emu, 2, input=[[0, 0x20], [1, 0]])                 # B：THRU
                self.assertEqual(emu.call("peek", addr="v_thru", len=2)["hex"], "0001")
                emu.call("midi", **{"in": [0x91, 72, 90, 0xF8]})
                self.run_frames(emu, 3, input=[[0, 0x10]])                          # A：C4 を弾く
                out = emu.call("midi")["out"]
                self.assertEqual([b["byte"] for b in out], [0x91, 72, 90, 0xF8, 0x90, 60, 100])
                gaps = [b["us"] - a["us"] for a, b in zip(out, out[1:])]
                self.assertTrue(all(g >= 319 for g in gaps), gaps)
                rows = png_rows(emu.screenshot())
                base = 48                                                            # オクターブ 3 の C
                for note in (60, 72):
                    k = note - base
                    self.assertEqual(rows[168][key_x(k) + 5], RED, f"ノート {note} の鍵")
                self.assertNotEqual(rows[168][key_x(62 - base) + 5], RED)
                self.run_frames(emu, 2, input=[[0, 0]])
                self.assertEqual([b["byte"] for b in emu.call("midi")["out"]], [0x80, 60, 64])

            self.assertEqual(emu.call("peek", addr="bios_exc_vector", len=2)["hex"], "0000", "例外で止まった")

    def test_runs_without_the_doc_board(self):
        with self.emulator() as emu:
            emu.call("load", cart=self.bin, doc=False)
            self.run_frames(emu, 30)
            self.assertEqual(emu.call("peek", addr="v_doc", len=2)["hex"], "0000")
            self.run_frames(emu, 2, input=[[0, 0x20], [1, 0]])
            emu.call("midi", **{"in": [0x90, 60, 100]})
            self.run_frames(emu, 3)
            self.assertEqual([b["byte"] for b in emu.call("midi")["out"]], [0x90, 60, 100], "THRU は子基板なしでも")
            self.assertEqual(emu.call("peek", addr="bios_exc_vector", len=2)["hex"], "0000")


if __name__ == "__main__":
    unittest.main()
