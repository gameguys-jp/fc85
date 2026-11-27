"""Audio IN を録って弾くサンプラーのサンプル SAMPLE 10 の試験：py -m unittest discover -s sdk/samples/sampler

音程の表（main.c の s_ratio と FC_ROOT_Q8）が、DOC の式（台帳 ES5503.md §4.3：1 標本で FC÷2^(9＋R−T) byte 進む。
8 MHz・走査 32 で 1 MHz÷34）とラスタの周波数（21.47727 MHz÷4÷342）と平均律から計算した値と同じかを確かめる。
68000 の GCC があれば組み、見出しを確かめる。エミュレータの窓口（fc85-api）と Boot ROM もあれば、窓口の
audio_in（DR-129）で Audio IN に正弦波を入れて録らせ、録った標本の周期と大きさ、波形 RAM、1 オクターブ上で
弾いた音の周波数、ワンショットと LOOP、録る周波数、しきい値の時間切れ、画面の波形を確かめる。
"""

import math
import os
import re
import struct
import sys
import tempfile
import unittest
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(ROOT, "sdk", "tools"))
sys.path.insert(0, os.path.join(ROOT, "sdk", "c"))

import build  # noqa: E402
import m68kgcc  # noqa: E402
from test_devices import png_rows  # noqa: E402

TC = m68kgcc.find_toolchain()
SR = 1e6 / 34                       # DOC 8 MHz、走査 32
LINE_HZ = 21.47727e6 / 4 / 342      # ラスタ 1 本ごとに 1 標本（2CLK÷342 ドット）
SHIFT = 9 + 7 - 6                   # R＝7、T＝6（16 KiB）
CA_IDLE = 15
ONESHOT, FREE = 0x02, 0x00
PAD_UP, PAD_DOWN, PAD_A, PAD_B, PAD_C = 0x01, 0x02, 0x10, 0x20, 0x40
WAVE_TOP, WAVE_H = 72, 64


def source():
    with open(os.path.join(HERE, "main.c"), encoding="utf-8") as f:
        return f.read()


def ratio():
    body = re.search(r"s_ratio\[13\] = \{([^}]*)\}", source()).group(1)
    return [int(x) for x in re.findall(r"\d+", body)]


def root_q8():
    return int(re.search(r"#define FC_ROOT_Q8 (\d+)UL", source()).group(1))


def fc_of_pitch(p, rate):
    """main.c の fc_of_pitch と同じ計算（p は 1/64 半音、0 が録った高さ）。"""
    r, octave = ratio(), 0
    while p < 0:
        p += 768
        octave -= 1
    while p >= 768:
        p -= 768
        octave += 1
    k, frac = p >> 6, p & 63
    q = r[k] + (((r[k + 1] - r[k]) * frac) >> 6)
    sh = 20 + rate - octave
    return max(1, min(0xFFFF, (root_q8() * q + (1 << (sh - 1))) >> sh))


def ups(xs, mid=128):
    return sum(1 for p, q in zip(xs, xs[1:]) if p < mid <= q)


class TableTest(unittest.TestCase):
    def test_tables_follow_the_doc_formula(self):
        self.assertEqual(ratio(), [round(4096 * 2 ** (k / 12)) for k in range(13)])
        self.assertEqual(root_q8(), round(256 * 2 ** SHIFT * LINE_HZ / SR))

    def test_notes_are_in_tune(self):
        # FC の分解能で、録る周波数が低いほど低い音が外れる
        for rate, bound in ((0, 6), (1, 10), (2, 16)):
            for n in range(36, 97):
                got = SR * fc_of_pitch((n - 60) * 64, rate) / 2 ** SHIFT
                want = LINE_HZ / 2 ** rate * 2 ** ((n - 60) / 12)
                self.assertLess(abs(1200 * math.log2(got / want)), bound, f"{rate} のノート {n}")
        self.assertEqual([fc_of_pitch(0, r) for r in range(3)], [547, 273, 137])


@unittest.skipIf(TC is None, "68000 の GCC がない（sdk/docs/c.md §02）")
class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.bin = os.path.join(cls.tmp.name, "sampler.bin")
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
        self.assertEqual(self.image[16:32], b"SAMPLE 10       ")

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

    def press(self, emu, button, frames=3):
        return self.run_frames(emu, frames, input=[[0, button], [1, 0]])

    def word(self, emu, label):
        return int(emu.call("peek", addr=label, len=2)["hex"], 16)

    def bytes_at(self, emu, addr, n, space="cpu"):
        return list(bytes.fromhex(emu.call("peek", space=space, addr=addr, len=n)["hex"]))

    def right_hz(self, emu, frames):
        """frames の間の R の音の周波数（直流を除いて、上へ 0 を横切る数から）。"""
        emu.call("audio", keep=False)
        self.run_frames(emu, frames)
        path = os.path.join(self.tmp.name, "out.wav")
        emu.call("audio", path=path, keep=False)
        with wave.open(path, "rb") as w:
            rate, n = w.getframerate(), w.getnframes()
            pcm = struct.unpack(f"<{2 * n}h", w.readframes(n))
        r = pcm[1::2]
        mean = sum(r) / len(r)
        return ups([v - mean for v in r], 0) * rate / len(r)

    def test_records_and_plays(self):
        with self.emulator() as emu:
            emu.call("load", cart=self.bin)
            self.run_frames(emu, 60)
            self.assertGreater(self.word(emu, "v_frame") << 16 | self.word(emu, "v_frame+2"), 0, "主のループに来た")

            def osc(i):
                return emu.call("doc")["oscillators"][i]

            def ca(o):
                return o["control"] >> 4

            with self.subTest("起動：走査 32、録る前ののこぎり波"):
                st = emu.call("doc")
                self.assertEqual(st["enabled"], 32)
                self.assertEqual({(o["on"], ca(o)) for o in st["oscillators"]}, {(False, CA_IDLE)})
                saw = [max(1, 128 + ((((i % 60) * 4 - 118) * (256 - (i >> 6))) >> 8)) for i in range(16384)]
                self.assertEqual(self.bytes_at(emu, 0, 4096, "wave"), saw[:4096])
                self.assertEqual(self.bytes_at(emu, "v_rec", 4096), saw[:4096])
                self.assertEqual(self.bytes_at(emu, 0x3C00, 1024, "wave"), saw[0x3C00:], "テーブルの終わりまで")
                self.assertEqual(self.word(emu, "v_len"), 16384)

            with self.subTest("A：ワンショット、CA 0（L）、録った高さ"):
                peak = self.press(emu, PAD_A)["audio"]["peak"]
                o = osc(0)
                self.assertEqual((o["on"], o["freq"], o["wave_ptr"], o["wave_size"], ca(o), o["control"] & 6),
                                 (True, 547, 0, 0x37, 0, ONESHOT))
                self.assertGreater(peak[0], 1000)
                self.assertLess(peak[1], 100)
                # 16,384 標本を 1 標本 547÷1024 で進めると 1.04 秒（63 フレーム）で止まる
                self.run_frames(emu, 70)
                self.assertEqual((osc(0)["on"], ca(osc(0))), (False, CA_IDLE))

            with self.subTest("B：しきい値を待って録る"):
                for _ in range(2):
                    emu.call("audio_in", tone={"hz": 440, "amp": 0.5, "ms": 1000})
                self.press(emu, PAD_B)
                self.run_frames(emu, 90)                    # 1.04 秒と、運んで描く間
                self.assertEqual([self.word(emu, n) for n in ("v_recs", "v_len", "v_status", "v_rec_rate")],
                                 [1, 16384, 0, 0])
                rec = self.bytes_at(emu, "v_rec", 4096)
                self.assertGreaterEqual(abs(rec[0] - 128), 12, "しきい値を超えた標本から")
                self.assertTrue(75 <= min(rec) <= 80 and 176 <= max(rec) <= 181, f"{min(rec)}〜{max(rec)}")
                # 440 Hz を 15,699.76 Hz で録ると 1 周期 35.68 標本。4,096 標本で 114.8 周期
                self.assertIn(ups(rec), (114, 115))
                self.assertEqual(self.bytes_at(emu, 0, 4096, "wave"), rec, "波形 RAM の頁 0 に運んだ")
                rows = png_rows(emu.screenshot())
                bg = rows[WAVE_TOP][0]
                drawn = [y for y in range(WAVE_TOP, WAVE_TOP + WAVE_H) if rows[y][100] != bg]
                # 76〜179 は (255−値)÷4 で 19〜44（中央の線 31 を含む）
                self.assertTrue(drawn and drawn[0] - WAVE_TOP <= 20 and drawn[-1] - WAVE_TOP >= 43, drawn)

            with self.subTest("MIDI：1 オクターブ上は 880 Hz"):
                emu.call("midi", **{"in": [0x90, 72, 100]})
                self.run_frames(emu, 2)
                o = osc(1)                                  # 空きのうち、使っていない声
                self.assertEqual((o["on"], o["freq"], ca(o), o["vol"]), (True, fc_of_pitch(768, 0), 1, 200))
                hz = self.right_hz(emu, 20)                 # 2 倍の速さでは 0.52 秒で終わる
                self.assertAlmostEqual(hz, 880, delta=880 * 0.02)
                emu.call("midi", **{"in": [0xB0, 123, 0]})
                self.run_frames(emu, 2)
                self.assertFalse(osc(1)["on"])

            with self.subTest("LOOP：テーブルをくり返し、離すと音量を下げて止める"):
                self.press(emu, PAD_UP)
                emu.call("midi", **{"in": [0x90, 60, 127]})
                self.run_frames(emu, 90)                    # 1.04 秒を超えても鳴っている
                on = [i for i, o in enumerate(emu.call("doc")["oscillators"][:8]) if o["on"]]
                self.assertEqual(len(on), 1)
                self.assertEqual(osc(on[0])["control"] & 6, FREE)
                emu.call("midi", **{"in": [0x80, 60, 0]})
                self.run_frames(emu, 12)                    # 254 から 1 フレーム 24 ずつ
                self.assertFalse(osc(on[0])["on"])
                self.press(emu, PAD_UP)

            with self.subTest("C と下：7.85 kHz で、しきい値なしですぐ録る"):
                self.press(emu, PAD_C)
                self.press(emu, PAD_DOWN)
                self.assertEqual([self.word(emu, "v_rate"), self.word(emu, "v_trig")], [1, 0])
                self.press(emu, PAD_B)
                # 16,384×2 本＝2.09 秒（125 フレーム）。録った後、波形 RAM へ運んで波形を描く間（約 9 フレーム）は
                # 主のループが回らず、その間に押したボタンは BIOS が覚えない
                self.run_frames(emu, 150)
                self.assertEqual([self.word(emu, n) for n in ("v_recs", "v_rec_rate")], [2, 1])
                self.assertEqual(set(self.bytes_at(emu, "v_rec", 2048)), {128}, "何も来ない Audio IN は 128")
                self.press(emu, PAD_A)
                on = [o for o in emu.call("doc")["oscillators"][:8] if o["on"]]
                self.assertEqual([o["freq"] for o in on], [273], "録った周波数の半分")

            with self.subTest("しきい値を 5 秒待って来なければ取りやめる"):
                self.press(emu, PAD_DOWN)
                self.press(emu, PAD_B)
                self.run_frames(emu, 320)
                self.assertEqual([self.word(emu, n) for n in ("v_recs", "v_status")], [2, 1])

            self.assertEqual(self.word(emu, "bios_exc_vector"), 0, "例外で止まった")

    def test_runs_without_the_doc_board(self):
        with self.emulator() as emu:
            emu.call("load", cart=self.bin, doc=False)
            self.run_frames(emu, 60)
            self.assertEqual([self.word(emu, "v_doc"), self.word(emu, "v_status")], [0, 2])
            self.press(emu, PAD_B)
            self.press(emu, PAD_A)
            emu.call("midi", **{"in": [0x90, 60, 100]})
            self.run_frames(emu, 3)
            self.assertEqual(self.word(emu, "v_in"), 1)
            self.assertEqual(self.word(emu, "v_recs"), 0)
            self.assertEqual(self.word(emu, "bios_exc_vector"), 0)


if __name__ == "__main__":
    unittest.main()
