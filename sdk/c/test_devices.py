#!/usr/bin/env python3
"""周辺の関数（acrtc.h・fabric.h・doc.h・adpcm.h。DR-106。video.h・dsp.h。DR-109）の試験。
`py -m unittest discover -s sdk/c` で走る。

1. GCC があれば、試験の ROM（tests/dev_test.c）を組み、逆アセンブルを見る：周辺の関数は I/O の窓に読み書き
   一体の命令（CLR、BSET、ADDQ など）を使わない（guarantees.md §03 の 3・6）。奇数番地への語・長語の
   アクセスがない（68000 ではアドレスエラー。m68kgcc.py の -fno-store-merging）。
2. GCC とエミュレータの窓口（fc85-api）と Boot ROM があれば、ROM を動かし、VRAM・Staging RAM・DOC の様子・
   音を確かめる。期待値は台帳と仕様書から：ACRTC の画素の並び（1 語に 4 画素、左が下位）と、ALINE は終わりの
   点を描かない・AFRCT は両端を含む（HD63484.md）、Fabric の変換（AND MASK → XOR。仕様書 §06）、
   ADPCM の長さ（1 byte に 2 ニブル、8 kHz・4 kHz）、DOC の 0xE1（(N−1)×2）。HBlank の割込みで替えた色 0 の
   区切り（画面の PNG。DIRECT ではラスタの途中で変わる）、DSP のメールボックスの往復（tests/echo.dsp を
   dspasm --c-array で C の見出しにする）。
"""

import os
import re
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "tools"))
sys.path.insert(0, os.path.join(ROOT, "sdk", "dspasm"))

import dspasm  # noqa: E402
import m68kgcc  # noqa: E402

DEVICE_SOURCES = ["acrtc.c", "fabric.c", "doc.c", "adpcm.c", "video.c", "dsp.c"]
IO = (0x200000, 0x300000)            # I/O の窓（registers.md）
STAGING = 0x120000
FRAME_HZ = 59.92

# 書く前に読む命令（読み書き一体）。68000 の CLR も読む
RMW = re.compile(r"^(clr|tas|bset|bclr|bchg|not|negx?|nbcd|ori|andi|eori|addi|subi|addq|subq|addx|subx|or|and|eor|"
                 r"add|sub|as[lr]|ls[lr]|rox?[lr]|s(?:t|f|hi|ls|cc|cs|ne|eq|vc|vs|pl|mi|ge|lt|gt|le))[bwl]?$")
INSN = re.compile(r"^\s+[0-9a-f]+:\t(\S+)\s*(.*)$")
FUNC = re.compile(r"^[0-9a-f]{8} <([A-Za-z_]\w*)>:$")
DEF = re.compile(r"^(?:static\s+)?[A-Za-z_][\w \t*]*?\b([A-Za-z_]\w*)\s*\([^;{]*\)\s*\n\{", re.M)

TC = m68kgcc.find_toolchain()


def png_rows(data):
    """fc85-api の画面の PNG（RGB 8 bit、フィルタなし）→ 行ごとの [(r, g, b)]。"""
    import struct
    import zlib
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, w, h = 8, b"", 0, 0
    while pos < len(data):
        n, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h = struct.unpack(">II", body[:8])
        elif kind == b"IDAT":
            idat += body
        pos += 12 + n
    raw = zlib.decompress(idat)
    stride = 1 + 3 * w
    rows = []
    for y in range(h):
        row = raw[y * stride:(y + 1) * stride]
        assert row[0] == 0, "フィルタなしのはず"
        rows.append([tuple(row[1 + 3 * x:4 + 3 * x]) for x in range(w)])
    return rows


def functions(dis):
    """逆アセンブル → {関数の名前: [(命令, オペランド)]}。"""
    out, name = {}, None
    for line in dis.splitlines():
        m = FUNC.match(line)
        if m:
            name = m.group(1)
            out[name] = []
            continue
        m = INSN.match(line)
        if m and name:
            out[name].append((m.group(1), m.group(2)))
    return out


def last_operand(operands):
    """MIT の書き方で、括弧の外の最後のコンマの後（行き先）。"""
    depth, cut = 0, -1
    for i, ch in enumerate(operands):
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        elif ch == "," and depth == 0:
            cut = i
    return operands[cut + 1:].strip()


def io_destination(operand):
    """行き先が I/O の窓か、どこを指すか分からないメモリ（%sp 以外のアドレスレジスタ経由）なら True。"""
    operand = operand.split(" <")[0].strip()
    if re.fullmatch(r"[0-9a-f]+", operand):
        return IO[0] <= int(operand, 16) < IO[1]
    return "@" in operand and not operand.startswith("%sp@")


def rmw_on_io(funcs, names):
    """names の関数の中で、I/O の窓（か、どこを指すか分からないメモリ）に読み書き一体の命令を使う所。"""
    return [f"{name}: {op} {args}" for name in sorted(names & set(funcs)) for op, args in funcs[name]
            if RMW.match(op) and io_destination(last_operand(args))]


def odd_accesses(funcs):
    """68000 は語・長語を奇数番地に読み書きできない（アドレスエラー）。%sp は偶数なので、%sp からの奇数の
    変位と、奇数の絶対番地を探す。"""
    bad = []
    for name, insns in funcs.items():
        for op, args in insns:
            if not re.fullmatch(r"[a-z]+[wl]", op) or op.startswith(("b", "db", "j")):
                continue
            odd = any(int(d) & 1 for d in re.findall(r"%sp@\((-?\d+)\)", args))
            odd |= any(int(a, 16) & 1 for a in re.findall(r"(?:^|[,#\s])([0-9a-f]{5,8}) <", args))
            if odd:
                bad.append(f"{name}: {op} {args}")
    return bad


class TestCheckers(unittest.TestCase):
    """逆アセンブルの見張りが、悪い形を見つけ、よい形を通す（GCC は要らない）。"""

    def test_rmw_on_io(self):
        funcs = {"f": [("orib", "#1,%a0@(12)"), ("clrw", "200000 <__stack_top+0xf0000>"),
                       ("bset", "#0,210041"), ("clrb", "100d55 <s_state>"), ("addql", "#4,%sp"),
                       ("subql", "#1,%sp@(8)"), ("movew", "#0,200002"), ("orw", "#1,%d0")]}
        self.assertEqual(rmw_on_io(funcs, {"f"}), ["f: orib #1,%a0@(12)", "f: clrw 200000 <__stack_top+0xf0000>",
                                                   "f: bset #0,210041"])
        self.assertEqual(rmw_on_io(funcs, {"g"}), [])

    def test_odd_accesses(self):
        funcs = {"f": [("movel", "#268435712,%sp@(77)"), ("movel", "#536879360,%sp@(72)"),
                       ("movew", "100d53 <x+0x1>,%d0"), ("movew", "100d52 <x>,230000 <y>"),
                       ("moveb", "#1,%sp@(81)"), ("bnew", "400ca6 <f+0x7e>")]}
        self.assertEqual(odd_accesses(funcs), ["f: movel #268435712,%sp@(77)", "f: movew 100d53 <x+0x1>,%d0"])


@unittest.skipIf(TC is None, "68000 の GCC がない（sdk/docs/c.md §02）")
class TestDeviceHelpers(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.bin = os.path.join(cls.tmp.name, "dev.bin")
        # DSP のプログラムを C の見出しにする（dspasm --c-array）
        header = os.path.join(cls.tmp.name, "echo_dsp.h")
        rc = dspasm.main([os.path.join(HERE, "tests", "echo.dsp"), "-o", header, "--c-array", "echo_dsp",
                          "-I", os.path.join(ROOT, "sdk", "include")])
        assert rc == 0, "echo.dsp が組めない"
        m68kgcc.build([os.path.join(HERE, "tests", "dev_test.c")], cls.bin, ROOT, include_dirs=[cls.tmp.name])
        with open(os.path.splitext(cls.bin)[0] + ".dis", encoding="utf-8") as f:
            cls.dis = f.read()
        cls.funcs = functions(cls.dis)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def device_functions(self):
        names = set()
        for src in DEVICE_SOURCES:
            with open(os.path.join(HERE, src), encoding="utf-8") as f:
                names.update(DEF.findall(f.read()))
        return names

    def test_runtime_lists_the_device_sources(self):
        for src in DEVICE_SOURCES:
            self.assertIn(src, m68kgcc.RUNTIME)

    def test_no_read_modify_write_on_io(self):
        names = self.device_functions()
        self.assertIn("acrtc_send", names)
        self.assertIn("fab_set", names)
        self.assertIn("doc_set_oscillators", names)
        self.assertIn("adpcm_update", names)
        self.assertGreater(len(names & set(self.funcs)), 20, "逆アセンブルに周辺の関数が見つからない")
        self.assertEqual(rmw_on_io(self.funcs, names), [])

    def test_no_odd_word_access(self):
        self.assertEqual(odd_accesses(self.funcs), [])

    def test_runs_on_the_machine(self):
        from fc85emu import Emulator, find_api
        if not find_api(ROOT):
            self.skipTest("fc85-api がない")
        if not os.path.isfile(os.path.join(ROOT, "firmware", "boot", "build", "boot.bin")):
            self.skipTest("Boot ROM がない")
        with Emulator(ROOT) as emu:
            emu.call("load", cart=self.bin)

            def peek(addr, size, n=1, space="cpu"):
                h = emu.call("peek", space=space, addr=addr, len=size * n)["hex"]
                return [int(h[i:i + size * 2], 16) for i in range(0, len(h), size * 2)]

            def to_phase(n):
                r = emu.call("run", frames=600, watch=["v_phase"], stop_on_bus_error=True)
                self.assertEqual(r["stop"], "watch", f"v_phase が {n} にならない")
                self.assertEqual(r["bus_errors"], 0)
                self.assertEqual(peek("v_phase", 2)[0], n)
                return r["audio"]

            quiet = to_phase(1)
            adpcm = to_phase(2)
            to_phase(3)
            doc = emu.call("run", frames=30, stop_on_bus_error=True)
            self.assertEqual(doc["bus_errors"], 0)
            to_phase(4)
            emu.call("run", frames=2, stop_on_bus_error=True)    # 区切りのあるフレームを丸ごと描かせる
            screen = png_rows(emu.screenshot())
            to_phase(5)
            emu.call("run", frames=2, stop_on_bus_error=True)
            direct = png_rows(emu.screenshot())
            to_phase(6)
            emu.call("run", frames=2, stop_on_bus_error=True)
            self.assertEqual(peek("bios_exc_vector", 2)[0], 0, "例外で止まった")
            self.assertEqual(peek("done", 2)[0], 0xC0DE, "終わりまで来ていない")

            with self.subTest("ACRTC"):
                a = peek("r_acrtc", 2, 16)
                self.assertEqual(a[:10], [1] * 10, "送った・待った")
                self.assertEqual(a[10], 0x9999, "RPR で読み返した CL0（色 9）")
                self.assertEqual(a[11], 0x0200, "CCR は BIOS が残した値")
                self.assertEqual(a[12], 0, "未定義のコマンドは CER で打ち切って 0")
                self.assertEqual(a[13:15], [1, 1], "打ち切った後も送れる")
                self.assertEqual(a[15], 0x0200, "打切りの後、ABT は下りていて、ほかのビットは同じ")
                vram = peek(0, 2, 64 * 192, space="vram")

                def px(x, y):
                    return (vram[y * 64 + x // 4] >> (4 * (x % 4))) & 15

                for y in range(6, 18):
                    for x in range(6, 26):
                        inside = 8 <= x <= 23 and 8 <= y <= 15
                        self.assertEqual(px(x, y), 5 if inside else 0, f"塗り ({x}, {y})")
                self.assertEqual([px(x, 40) for x in range(18)], [9] * 16 + [0, 0], "線は終わりの点を描かない")
                self.assertEqual([px(x, 50) for x in range(99, 104)], [0, 0, 9, 0, 0], "点")
                for y in range(63, 73):
                    for x in range(31, 49):
                        edge = (x in (32, 47) and 64 <= y <= 71) or (y in (64, 71) and 32 <= x <= 47)
                        self.assertEqual(px(x, y), 9 if edge else 0, f"枠 ({x}, {y})")
                self.assertEqual([px(138, 128), px(118, 128), px(128, 118), px(128, 138), px(128, 128)],
                                 [9, 9, 9, 9, 0], "円")
                self.assertEqual([px(x, 100) for x in range(199, 205)], [0, 3, 3, 3, 3, 0], "打ち切りの後の塗り")

            with self.subTest("Fabric"):
                f = peek("r_fab", 2, 4)
                self.assertEqual(f[:2], [2, 2], "fab_run は DONE を返す")
                self.assertEqual(f[2:], [0, 0], "fab_run の後は DONE・ERROR・MISS が消えている")
                self.assertEqual(bytes(peek(STAGING + 0x2100, 1, 16)), b"FABRIC COPY 0123")
                words = [0x1234, 0xABCD, 0x0F0F, 0xF0F0, 0x0000, 0xFFFF, 0x8001, 0x7FFE]
                self.assertEqual(peek(STAGING + 0x2300, 2, 8), [(w & 0x0FFF) ^ 0x00FF for w in words])

            with self.subTest("ADPCM"):
                p = peek("r_adpcm", 2, 6)
                # 鳴る長さ＋始めるまでの 2 フレーム（求め → ARMED → PLAYING）
                n8 = 1024 * 2 / 8000 * FRAME_HZ
                n4 = 256 * 2 / 4000 * FRAME_HZ
                self.assertTrue(n8 + 1 <= p[0] <= n8 + 4, f"8 kHz の 1024 byte：{p[0]} フレーム")
                self.assertTrue(n4 + 1 <= p[1] <= n4 + 4, f"途中で替えた 4 kHz の 256 byte：{p[1]} フレーム")
                self.assertTrue(1 <= p[2] <= 2, f"鳴っている途中で止めた：{p[2]} フレーム")
                self.assertEqual(p[3:], [0, 0, 0], "チャネルは止まり、FIFO は空、求めはない")
                self.assertEqual(max(quiet["peak"]), 0, "ADPCM の前は無音")
                self.assertGreater(min(adpcm["peak"]), 1000, "ADPCM が鳴った")

            with self.subTest("DOC"):
                d = peek("r_doc", 2, 8)
                self.assertEqual(d[0], 1, "子基板あり（既定の構成）")
                self.assertEqual(d[1], 2, "doc_load_wave は DONE を返す")
                self.assertEqual(d[2:4], [16, 20], "BIOS の走査は 16、doc_set_oscillators で 20")
                self.assertEqual(d[4], 0x11, "doc_halt：CA 1、H")
                self.assertEqual(d[7], 0x10, "doc_go：CA 1、H なし")
                self.assertEqual(d[5], 0x05, "doc_freq の上位")
                self.assertEqual(d[6], 0xF1, "走査に入れたオシレータは CA 15、H")
                wave = [i * 2 + 1 if i < 128 else 255 - 2 * (i - 128) for i in range(256)]
                self.assertEqual(peek(0, 1, 256, space="wave"), wave, "波形 RAM")
                st = emu.call("doc")
                self.assertEqual(st["enabled"], 20)
                o0, o1 = st["oscillators"][0], st["oscillators"][1]
                self.assertEqual((o0["on"], o0["freq"], o0["vol"]), (True, 0x0500, 180))
                self.assertEqual((o1["on"], o1["freq"], o1["vol"]), (True, 0x0400, 200))
                for o in st["oscillators"][16:20]:
                    self.assertEqual((o["on"], o["control"]), (False, 0xF1))
                self.assertGreater(max(doc["audio"]["peak"]), 1000, "DOC が鳴った")

            with self.subTest("映像"):
                v = peek("r_video", 2, 6)
                self.assertEqual(v[0], 7, "VBL_EN・HBL_EN・LINE_CMP")
                self.assertEqual(v[1], 1, "ベクタ 30 を差し替えた")
                self.assertEqual(v[3], 4, "BIOS の VBlank（bios_frame）はつないだ先で進む")
                self.assertEqual(v[2], 2 * 4, "1 フレームに 2 回（SPLIT1 と SPLIT2）")
                self.assertEqual(v[4:], [1, 1], "止めると VBL_EN だけ、ベクタは BIOS の処理に戻る")
                # 何も描いていない列 x＝250 の色 0 の移り変わり
                col = [row[250] for row in screen]
                runs = []
                for y, c in enumerate(col):
                    if not runs or runs[-1][1] != c:
                        runs.append((y, c))
                blue, red, green = (0, 0, 255), (255, 0, 0), (0, 255, 0)
                # ラスタ L の HBlank で書いた色は L＋2 から：HBlank の始まりから次のラインの写しまで 22 ドット（約 4 µs）
                # で、68000 の割込みの応答が間に合わない。画面の y＝60・120 のラスタで比べ、62・122 から変わる
                self.assertEqual(runs, [(0, blue), (62, red), (122, green)], "区切り")
                # DIRECT（Bend）：表へ直接書くので、ラスタ L＋1（y＝61・121）の途中で変わる
                for y, before, after in ((61, blue, red), (121, red, green)):
                    row = direct[y]
                    x = row.index(after)
                    self.assertEqual(direct[y - 1][250], before)
                    self.assertTrue(0 < x < 250, f"y={y} の途中で変わる（x={x}）")
                    self.assertEqual(row[0], before)
                    self.assertEqual(row[255], after)
                    self.assertEqual(direct[y + 1][0], after)

            with self.subTest("DSP"):
                d = peek("r_dsp", 2, 8)
                self.assertEqual(d[0], 0, "dsp_load は読み返して違いなし")
                self.assertEqual(d[1:5], [1, 0x1235, 1, 0x0000], "メールボックスの語＋1 が返る")
                self.assertEqual(d[5], 0, "応答を読むと RESPONSE が下がる")
                self.assertEqual(d[6], 0, "RESET の間は答えない（時間切れ）")


if __name__ == "__main__":
    unittest.main()
