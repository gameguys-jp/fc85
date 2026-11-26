"""SAMPLE 6 の試験：py -m unittest discover -s sdk/samples/showcase

組んだ像の見出し（bios-abi.md §04）と大きさ、ラスタの表（滑らかさと色の数）、正弦と透視の表（build.py とは
別に計算して比べる）、立方体の頂点と辺の道、DSP のプログラムの像、海と玉の絵、漢字の行の offset（仕様書 §08 の
式と字形の表）を確かめる。
"""

import math
import os
import re
import struct
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import build  # noqa: E402

CART = 0x400000


def words(data):
    return list(struct.unpack(f">{len(data) // 2}h", data))


def uwords(data):
    return list(struct.unpack(f">{len(data) // 2}H", data))


def unpack_row(data):
    """ACRTC の語（左の画素が下位ニブル）の列を画素の列にする。"""
    out = []
    for w in uwords(data):
        out += [(w >> (4 * i)) & 15 for i in range(4)]
    return out


class CartridgeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        out = os.path.join(cls.tmp.name, "showcase.bin")
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
        cls.image = read("showcase.bin")
        cls.lst = read("showcase.lst", "r")
        cls.dsp = read("xform.bin")
        cls.sea = read("sea.bin")
        cls.bank = read("bank.bin")

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
        self.assertEqual(self.image[16:32], b"SAMPLE 6".ljust(16))

    def test_size(self):
        self.assertEqual(len(self.image) % 2, 0)
        self.assertLessEqual(len(self.image), build.CART_MAX)
        self.assertLess(len(self.image), 64 * 1024)

    # ---- ラスタの表

    def test_gradient_in_image(self):
        grad = build.make_gradient()
        got = uwords(self.at("grad_table", 4 * build.BASE_ROWS))
        self.assertEqual(got, [c for row in grad for c in row])

    def test_gradient_is_smooth(self):
        """空と海の色（パレット 1）は、隣の行と成分ごとに 1 段まで。水平線の 1 行だけが飛ぶ。"""
        grad = [c1 for c1, _ in build.make_gradient()]
        horizon = build.SKY_ROWS
        for i in range(1, len(grad)):
            if i in (horizon, horizon + 1):
                continue
            a, b = grad[i - 1], grad[i]
            for s in (8, 4, 0):
                self.assertLessEqual(abs(((a >> s) & 15) - ((b >> s) & 15)), 1, f"行 {i}")

    def test_more_than_sixteen_colours(self):
        """基本画面の行ごとのパレット 1 だけで 16 色を超える。"""
        grad = build.make_gradient()
        sky_sea = {c1 for c1, _ in grad}
        self.assertGreater(len(sky_sea), 16)
        cube = {c2 for _, c2 in grad[:build.SKY_ROWS]}
        self.assertGreater(len(cube), 16)
        self.assertGreater(build.count_colors(), 64)

    def test_bars(self):
        pos = words(self.at("bar_pos", 512))
        self.assertEqual(pos, build.bar_positions())
        self.assertTrue(all(0 <= p and p + build.BAR_H <= build.SKY_ROWS for p in pos))
        for name, kind in (("bar_colors_a", 0), ("bar_colors_b", 1)):
            got = uwords(self.at(name, 2 * build.BAR_H))
            self.assertEqual(got, build.bar_colors(kind))
            # 真ん中が一番明るい
            lum = [((c >> 8) & 15) + ((c >> 4) & 15) + (c & 15) for c in got]
            self.assertEqual(max(lum), lum[build.BAR_H // 2])

    # ---- 正弦と透視

    def test_sine_table(self):
        got = words(self.at("sin_table", 512))
        for k, v in enumerate(got):
            self.assertAlmostEqual(v / 32767, math.sin(2 * math.pi * k / 256), delta=1 / 32767)
            self.assertNotEqual(v, -32768)          # 乗数に −32768 を使わない
        self.assertEqual(got[64], 32767)
        self.assertEqual(got[192], -32767)
        for k in range(128):
            self.assertEqual(got[k + 128], -got[k])

    def test_perspective_table(self):
        got = words(self.at("persp_table", 512))
        for i, v in enumerate(got):
            self.assertEqual(v, round(32768 * build.PERSP_F / (build.PERSP_D + i - 128)))
            self.assertTrue(0 < v <= 32767)
        self.assertTrue(all(a > b for a, b in zip(got, got[1:])))

    def test_transform_matches_floating_point(self):
        """protocol.inc の固定小数点の式は、浮動小数点の回転と透視から 2 画素以内。"""
        for a, b, c in ((0, 0, 0), (17, 85, 200), (100, 33, 250), (255, 128, 64), (37, 211, 9)):
            got = build.transform(build.CUBE_VERTS, a, b, c)
            ra, rb, rc = (2 * math.pi * t / 256 for t in (a, b, c))
            for (x, y, z), (gx, gy) in zip(build.CUBE_VERTS, got):
                y1 = y * math.cos(ra) - z * math.sin(ra)
                z1 = y * math.sin(ra) + z * math.cos(ra)
                x2 = x * math.cos(rb) + z1 * math.sin(rb)
                z2 = z1 * math.cos(rb) - x * math.sin(rb)
                x3 = x2 * math.cos(rc) - y1 * math.sin(rc)
                y3 = x2 * math.sin(rc) + y1 * math.cos(rc)
                k = build.PERSP_F / (build.PERSP_D + z2)
                self.assertAlmostEqual(gx, x3 * k + build.CX, delta=2)
                self.assertAlmostEqual(gy, y3 * k - build.CY, delta=2)

    def test_cube_stays_in_the_sky(self):
        """どの角でも、立方体は空（論理座標 X 4〜251、Y −95〜0）の中。"""
        for a in range(0, 256, 9):
            for b in range(0, 256, 11):
                for c in range(0, 256, 13):
                    for x, y in build.transform(build.CUBE_VERTS, a, b, c):
                        self.assertTrue(4 <= x <= 251 and -95 <= y <= 0, (a, b, c, x, y))

    # ---- 立方体

    def test_vertices_and_paths(self):
        verts = words(self.at("cube_verts", 6 * len(build.CUBE_VERTS)))
        self.assertEqual(verts, [v for p in build.CUBE_VERTS for v in p])
        data = words(self.at("cube_paths", 64))
        edges = []
        i = 0
        while data[i]:
            k = data[i]
            path = data[i + 1:i + 2 + k]
            edges += [tuple(sorted(e)) for e in zip(path, path[1:])]
            i += k + 2
        self.assertEqual(sorted(edges), sorted(tuple(sorted(e)) for e in build.CUBE_EDGES))
        self.assertEqual(len(set(edges)), 12)

    # ---- DSP

    def test_dsp_program_is_in_the_image(self):
        self.assertEqual(len(self.dsp) % 2, 0)
        self.assertLessEqual(len(self.dsp) // 2, 4096)
        at = self.image.find(self.dsp)
        self.assertGreater(at, 0)
        self.assertEqual(at % 2, 0)
        self.assertEqual(at + CART, self.symbol("dsp_code"))
        # 0 番地は B init、2 番地は RET（台帳 TMS32010.md §4.4 のベクタ）
        self.assertEqual(self.dsp[0], 0xF9)
        self.assertEqual(self.dsp[4:6], bytes([0x7F, 0x8D]))

    def test_dsp_tables(self):
        """DSP のプログラム RAM の正弦と透視の表は、68000 の表と同じ値（同じ結果を出すため）。"""
        prog = words(self.dsp)
        sin = words(self.at("sin_table", 512))
        persp = words(self.at("persp_table", 512))
        # KTAB（3 番地から）の 9 番目と 10 番目が表の番地
        ktab = prog[3:3 + 13]
        self.assertEqual(ktab[0], 1)
        self.assertEqual(ktab[5] & 0xFFFF, build.HEAD_MAGIC << 8)
        self.assertEqual(ktab[6] & 0xFFFF, build.TAIL_MAGIC << 8)
        self.assertEqual(prog[ktab[8]:ktab[8] + 256], sin)
        self.assertEqual(prog[ktab[9]:ktab[9] + 256], persp)
        self.assertEqual(ktab[10], build.CX)
        self.assertEqual(ktab[11], build.CY)

    def test_protocol_constants(self):
        with open(os.path.join(HERE, "protocol.inc"), encoding="utf-8") as f:
            text = f.read()

        def equ(name):
            m = re.search(r"^" + name + r"\s+EQU\s+(\$?)([0-9A-Fa-f]+)", text, re.M)
            self.assertIsNotNone(m, name)
            return int(m.group(2), 16 if m.group(1) else 10)
        self.assertEqual(equ("HEAD_MAGIC"), build.HEAD_MAGIC)
        self.assertEqual(equ("TAIL_MAGIC"), build.TAIL_MAGIC)
        self.assertEqual(equ("CX"), build.CX)
        self.assertEqual(equ("CY"), build.CY)
        self.assertLessEqual(len(build.CUBE_VERTS), equ("MAX_VERTS"))

    # ---- 海と玉

    def test_sea(self):
        self.assertEqual(len(self.sea), 2 * 64 * build.SEA_ROWS)
        self.assertEqual(self.sea, self.at("sea_data", len(self.sea)))
        rows = [unpack_row(self.sea[128 * y:128 * (y + 1)]) for y in range(build.SEA_ROWS)]
        crest = 0
        for row in rows:
            self.assertEqual(row[:4], [0] * 4)
            self.assertEqual(row[-4:], [0] * 4)
            self.assertTrue(set(row[4:-4]) <= {1, 2})
            crest += row.count(2)
        self.assertGreater(crest, 300)
        self.assertNotIn(2, rows[0])            # 水平線の行は地の色だけ

    def test_sprite_bank(self):
        self.assertEqual(len(self.bank), 2 * 64 * 16)
        self.assertEqual(self.bank, self.at("bank_data", len(self.bank)))
        rows = [unpack_row(self.bank[128 * y:128 * (y + 1)]) for y in range(16)]
        w = 4 * build.SPR_WORDS

        def block(word, y):
            return rows[y][4 * word:4 * word + w]
        for s in range(4):
            for y in range(16):
                mask = block(build.BANK_MASK + build.SPR_WORDS * s, y)
                self.assertTrue(set(mask) <= {0, 15})
                # 横のずれ：s 画素だけ右へ
                base = block(build.BANK_MASK, y)
                self.assertEqual(mask[s:s + 16], base[:16])
                for v in range(len(build.BALL_SHADES)):
                    img = block(build.BANK_IMAGE + 4 * build.SPR_WORDS * v + build.SPR_WORDS * s, y)
                    # 型が透明（F）の画素は絵が 0、不透明（0）の画素は絵が 0 でない
                    for m, p in zip(mask, img):
                        self.assertEqual(m == 15, p == 0)
        # 玉は丸い：真ん中の行は幅 16、端の行は狭い
        mid = block(build.BANK_MASK, 8)
        self.assertEqual(mid[:16].count(0), 16)
        self.assertLess(block(build.BANK_MASK, 0).count(0), 10)

    def test_objects(self):
        objs = words(self.at("obj_init", 10 * build.MAX_OBJ))
        for i in range(build.MAX_OBJ):
            x, y, dx, dy, kind = objs[5 * i:5 * i + 5]
            self.assertTrue(build.SPR_XMIN <= x <= build.SPR_XMAX)
            self.assertTrue(build.SPR_YMIN <= y <= build.SPR_YMAX)
            self.assertIn(abs(dx), (1, 2))
            self.assertEqual(abs(dy), 1)
            self.assertIn(kind, (0, 1))
        # 玉は海の中だけ（水平線より下、基本画面の下の端まで）
        self.assertGreater(build.SPR_YMIN, build.SKY_ROWS)
        self.assertLessEqual(build.SPR_YMAX + 16, build.BASE_ROWS)

    # ---- 文字

    def test_kanji_line(self):
        """漢字の行の offset は、仕様書 §08 の式（区 16〜47 は 0x1F80 から、1〜8 区は 0x0800 から、1 字 8 byte）。"""
        got = uwords(self.at("kanji_line", 2 * len(build.KANJI_TEXT)))
        table = build.read_kuten()
        expect = []
        for ch in build.KANJI_TEXT:
            ku, ten = table[ch]
            base, first = (0x1F80, 16) if ku >= 16 else (0x0800, 1)
            expect.append(base + ((ku - first) * 94 + (ten - 1)) * 8)
        self.assertEqual(got, expect)
        self.assertEqual(table["日"], (38, 92))
        self.assertEqual(table["の"], (4, 46))

    def test_phase_colours(self):
        got = uwords(self.at("phase_colors", 2 * len(build.PHASE_COLORS)))
        self.assertEqual(got, build.PHASE_COLORS)
        self.assertEqual(len(set(got)), len(got))
        self.assertEqual(got[-1], 0)            # 待ちは黒


if __name__ == "__main__":
    unittest.main()
