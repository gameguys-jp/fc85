#!/usr/bin/env python3
"""C の SDK の試験（DR-103）。`py -m unittest discover -s sdk/c` で走る。

1. bios.h の番地と値が bios.inc と同じ（GCC は要らない）。
2. GCC があれば、試験の ROM（tests/rt_test.c）を組み、逆アセンブルを見る。リンクで止まるべきもの（見出しが
   ない、浮動小数点）が止まる。
3. GCC とエミュレータの窓口（fc85-api）と Boot ROM があれば、試験の ROM を動かし、掛け算・割り算・64 bit・
   メモリの関数・BIOS の関数・割込みの結果を Python の計算と比べる。
"""

import os
import random
import re
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "tools"))

import m68kgcc  # noqa: E402

BIOS_INC = os.path.join(ROOT, "sdk", "include", "bios.inc")
BIOS_H = os.path.join(HERE, "include", "bios.h")
M32 = (1 << 32) - 1
M64 = (1 << 64) - 1


def read_lines(path):
    with open(path, encoding="utf-8") as f:
        return f.read().splitlines()


def parse_bios_inc():
    """bios.inc → ({名前: 値}, {変数の名前: (番地, byte)})"""
    equ, var = {}, {}
    base = None
    for line in read_lines(BIOS_INC):
        line = line.split(";")[0].rstrip()
        m = re.match(r"^(\w+)\s+EQU\s+(\S+)$", line)
        if m:
            name, v = m.groups()
            m2 = re.match(r"^(\w+)\+(\d+)$", v)
            equ[name] = equ[m2.group(1)] + int(m2.group(2)) if m2 else int(v.lstrip("$"), 16)
            continue
        m = re.match(r"^\s+RSSET\s+(\w+)$", line)
        if m:
            base = equ[m.group(1)] if m.group(1) in equ else int(m.group(1))
            continue
        m = re.match(r"^(\w+)\s+RS\.([BWL])\s+(\d+)$", line)
        if m and base is not None:
            size = {"B": 1, "W": 2, "L": 4}[m.group(2)] * int(m.group(3))
            if base >= 0x100000:
                var[m.group(1)] = (base, size)
            base += size
    return equ, var


def parse_bios_h():
    defs, var = {}, {}
    sizes = {"uint8_t": 1, "uint16_t": 2, "uint32_t": 4, "bios_hook_t": 4}
    for line in read_lines(BIOS_H):
        m = re.match(r"^#define\s+(\w+)\s+0x([0-9A-Fa-f]+)U?L?\b", line)
        if m:
            defs[m.group(1)] = int(m.group(2), 16)
            continue
        m = re.match(r"^#define\s+(\w+)\s+\((\w+)\s*\+\s*(\d+)\)", line)
        if m:
            defs[m.group(1)] = defs[m.group(2)] + int(m.group(3))
            continue
        m = re.match(r"^#define\s+(bios_\w+)\s+BIOS_(VAR|ARRAY)\((\w+),\s*0x([0-9A-Fa-f]+)UL\)", line)
        if m:
            var[m.group(1)] = (int(m.group(4), 16), sizes[m.group(3)] if m.group(2) == "VAR" else None)
    return defs, var


class TestBiosHeader(unittest.TestCase):
    def test_values_match_bios_inc(self):
        equ, _ = parse_bios_inc()
        defs, _ = parse_bios_h()
        self.assertIn("BIOS_WAIT_VBL", equ)
        for name, value in equ.items():
            self.assertIn(name, defs, f"bios.h に {name} がない")
            self.assertEqual(defs[name], value, name)

    def test_variables_match_bios_inc(self):
        _, var_inc = parse_bios_inc()
        defs, var_h = parse_bios_h()
        for name, (addr, size) in var_inc.items():
            if size == 0:        # bios_vars_used：ここまで
                self.assertEqual(defs["BIOS_VARS_USED"], addr)
                continue
            self.assertIn(name, var_h, f"bios.h に {name} がない")
            haddr, hsize = var_h[name]
            self.assertEqual(haddr, addr, name)
            if hsize is not None:
                self.assertEqual(hsize, size, name)
        self.assertEqual(set(var_h), {n for n, (_, s) in var_inc.items() if s})


def cdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q


def s32(x):
    return x - (1 << 32) if x & 0x80000000 else x


def s64(x):
    return x - (1 << 64) if x >> 63 else x


def vectors():
    rnd = random.Random(1985)
    p32 = [(0, 1), (1, 1), (M32, 1), (M32, M32), (0x80000000, 1), (0x7FFFFFFF, 2), (12345678, 0xFFFF),
           (M32, 0x10000), (M32, 0x10001), (0xDEADBEEF, 0x80000000), (0x80000000, 0x80000001), (100, 7),
           (-100 & M32, 7), (100, -7 & M32), (-100 & M32, -7 & M32), (0x10000, 0x10000), (0xFFFF, 0xFFFF),
           (0x12345678, 0x9ABC), (0x7FFFFFFF, 0x7FFFFFFF), (0x80000000, 0xFFFF)]
    while len(p32) < 48:
        a = rnd.getrandbits(32)
        b = rnd.getrandbits(rnd.choice((8, 16, 17, 24, 32))) or 3
        if not (s32(a) == -(1 << 31) and s32(b) == -1):
            p32.append((a, b))
    p64 = [(0, 1, 0), (1, 1, 1), (M64, 1, 63), (M64, M64, 32), (1 << 63, 3, 31), (M64, 0xFFFFFFFF, 33),
           (0x123456789ABCDEF0, 0x1000, 4), (0x123456789ABCDEF0, 0x100000000, 12), (-1000 & M64, 7, 62),
           (1000, -7 & M64, 5), (-(1 << 40) & M64, -(1 << 20) & M64, 20), (0xFEDCBA9876543210, 0x0123456789ABCDEF, 0)]
    while len(p64) < 28:
        a = rnd.getrandbits(64)
        b = rnd.getrandbits(rnd.choice((16, 32, 33, 48, 64))) or 5
        p64.append((a, b, rnd.randrange(64)))
    return p32, p64


def vectors_h(p32, p64):
    lines = ["/* 試験が作る（sdk/c/test_c.py）。手で直さない */",
             f"#define N32 {len(p32)}", f"#define N64 {len(p64)}",
             "static const uint32_t a32[N32] = {" + ", ".join(f"0x{a:08X}UL" for a, _ in p32) + "};",
             "static const uint32_t b32[N32] = {" + ", ".join(f"0x{b:08X}UL" for _, b in p32) + "};",
             "static const uint64_t a64[N64] = {" + ", ".join(f"0x{a:016X}ULL" for a, _, _ in p64) + "};",
             "static const uint64_t b64[N64] = {" + ", ".join(f"0x{b:016X}ULL" for _, b, _ in p64) + "};",
             "static const uint8_t s64[N64] = {" + ", ".join(str(s) for _, _, s in p64) + "};"]
    return "\n".join(lines) + "\n"


TC = m68kgcc.find_toolchain()


@unittest.skipIf(TC is None, "68000 の GCC がない（sdk/docs/c.md §02）")
class TestCRuntime(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.p32, cls.p64 = vectors()
        with open(os.path.join(cls.tmp.name, "vectors.h"), "w", encoding="utf-8", newline="\n") as f:
            f.write(vectors_h(cls.p32, cls.p64))
        cls.bin = os.path.join(cls.tmp.name, "rt.bin")
        cls.size = m68kgcc.build([os.path.join(HERE, "tests", "rt_test.c")], cls.bin, ROOT,
                                 include_dirs=[cls.tmp.name])

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def small(self, name, text):
        src = os.path.join(self.tmp.name, name + ".c")
        with open(src, "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
        return m68kgcc.build([src], os.path.join(self.tmp.name, name + ".bin"), ROOT)

    def test_header_and_symbols(self):
        with open(self.bin, "rb") as f:
            image = f.read()
        self.assertEqual(image[:8], b"CART\x00\x01\x00\x00")
        self.assertEqual(int.from_bytes(image[8:12], "big"), 0x400020, "入口は見出しの直後の _start")
        self.assertEqual(int.from_bytes(image[12:16], "big"), len(image))
        self.assertEqual(image[16:32], b"C RUNTIME TEST  ")
        with open(os.path.splitext(self.bin)[0] + ".lst", encoding="utf-8") as f:
            lst = f.read()
        self.assertRegex(lst, r"\n  main \$004[0-9A-F]{5}\n")
        self.assertRegex(lst, r"\n  r_mul \$0010[0-9A-F]{4}\n")
        self.assertNotRegex(lst, r"__(add|mul|div)[sd]f3", "浮動小数点の関数が入った")

    def test_volatile_zero_store_is_not_clr(self):
        """68000 の CLR は書く前に読む。volatile のレジスタへの 0 は MOVE で書く（ACRTC・DOC の規約）。"""
        with open(os.path.splitext(self.bin)[0] + ".dis", encoding="utf-8") as f:
            dis = f.read()
        body = re.split(r"\n[0-9a-f]{8} <", dis.split("<probe_zero_store>:")[1])[0]
        code = [line for line in body.splitlines() if re.match(r"^\s+[0-9a-f]+:\t", line)]
        self.assertTrue(any("rts" in line for line in code), body)
        self.assertFalse(any("clr" in line for line in code), body)
        self.assertTrue(any("200000" in line for line in code), body)

    def test_the_listed_headers_compile(self):
        """sdk/docs/c.md §05 のヘッダが -nostdinc で読め、limits.h の値が 68000 の GCC の型と合う。"""
        heads = ["stdint.h", "stddef.h", "stdbool.h", "stdarg.h", "stdalign.h", "stdnoreturn.h", "iso646.h",
                 "limits.h", "string.h", "bios.h", "fc85.h", "acrtc.h", "fabric.h", "doc.h", "adpcm.h"]
        checks = ["CHAR_BIT == 8", "CHAR_MIN == -128", "UCHAR_MAX == 255", "SHRT_MIN == -32768",
                  "USHRT_MAX == 65535", "INT_MAX == 2147483647", "INT_MIN == -INT_MAX - 1",
                  "UINT_MAX == 4294967295U", "LONG_MAX == INT_MAX", "ULLONG_MAX == 18446744073709551615ULL",
                  "LLONG_MIN < 0", "sizeof(int) == 4", "sizeof(void *) == 4"]
        text = "".join(f"#include <{h}>\n" for h in heads) + 'FC85_CART("HEADERS         ");\n'
        text += "".join(f'_Static_assert({c}, "{c}");\n' for c in checks) + "int main(void) { return 0; }\n"
        self.assertGreater(self.small("headers", text), 32)

    def test_link_stops_without_a_header(self):
        with self.assertRaises(m68kgcc.ToolchainError) as cm:
            self.small("nohead", "int main(void) { return 0; }\n")
        self.assertIn("見出しがない", str(cm.exception))

    def test_float_is_not_linked(self):
        """浮動小数点の関数は SDK にない（GCC の実行時ライブラリをつながない）。リンクで止まる。"""
        with self.assertRaises(m68kgcc.ToolchainError) as cm:
            self.small("float", '#include "bios.h"\nFC85_CART("FLOAT           ");\n'
                                "volatile float f = 1.5f;\nint main(void) { f = f * 3.0f; return 0; }\n")
        self.assertIn("__mulsf3", str(cm.exception))

    def test_runs_and_matches_python(self):
        from fc85emu import Emulator, find_api
        if not find_api(ROOT):
            self.skipTest("fc85-api がない")
        if not os.path.isfile(os.path.join(ROOT, "firmware", "boot", "build", "boot.bin")):
            self.skipTest("Boot ROM がない")
        with Emulator(ROOT) as emu:
            emu.call("load", cart=self.bin)
            r = emu.call("run", frames=120, stop_on_bus_error=True)
            self.assertEqual(r["stop"], "frames")
            self.assertEqual(r["bus_errors"], 0)

            def peek(label, size, n=None):
                h = emu.call("peek", space="cpu", addr=label, len=size * (n or 1))["hex"]
                v = [int(h[i:i + size * 2], 16) for i in range(0, len(h), size * 2)]
                return v if n else v[0]

            self.assertEqual(peek("bios_exc_vector", 2), 0, "例外で止まった（68000 にない命令など）")
            self.assertEqual(peek("done", 2), 0xC0DE, "終わりまで来ていない")

            n32, n64 = len(self.p32), len(self.p64)
            mul, udiv, umod = peek("r_mul", 4, n32), peek("r_udiv", 4, n32), peek("r_umod", 4, n32)
            sdiv, smod = peek("r_sdiv", 4, n32), peek("r_smod", 4, n32)
            for i, (a, b) in enumerate(self.p32):
                with self.subTest(a=hex(a), b=hex(b)):
                    self.assertEqual(mul[i], (a * b) & M32)
                    self.assertEqual(udiv[i], a // b)
                    self.assertEqual(umod[i], a % b)
                    q = cdiv(s32(a), s32(b))
                    self.assertEqual(s32(sdiv[i]), q)
                    self.assertEqual(s32(smod[i]), s32(a) - q * s32(b))
            got = {k: peek("r_" + k, 8, n64) for k in ("mul64", "udiv64", "umod64", "sdiv64", "smod64",
                                                         "shl64", "shr64", "sar64")}
            for i, (a, b, s) in enumerate(self.p64):
                with self.subTest(a=hex(a), b=hex(b), s=s):
                    self.assertEqual(got["mul64"][i], (a * b) & M64)
                    self.assertEqual(got["udiv64"][i], a // b)
                    self.assertEqual(got["umod64"][i], a % b)
                    q = cdiv(s64(a), s64(b))
                    self.assertEqual(s64(got["sdiv64"][i]), q)
                    self.assertEqual(s64(got["smod64"][i]), s64(a) - q * s64(b))
                    self.assertEqual(got["shl64"][i], (a << s) & M64)
                    self.assertEqual(got["shr64"][i], a >> s)
                    self.assertEqual(s64(got["sar64"][i]), s64(a) >> s)

            init = peek("r_init", 4, 4)
            self.assertEqual(init[:3], [0x12345678, 0xBEEF, 0], ".data の初期値と .bss の 0")
            self.assertTrue(0x100800 <= init[3] < 0x100810, ".data は APP_RAM から")

            text = b"0123456789ABCDEFGHIJKLMNOPQRSTUV"
            want = bytearray(b"\xA5" * 64)
            want[1:11] = text[:10]
            want[16:32] = text[:16]
            want[40:52] = text[:12]
            want[42:54] = bytes(want[40:52])
            want[57:60] = b"\0\0\0"
            self.assertEqual(bytes(peek("r_mem", 1, 64)), bytes(want))
            self.assertEqual([v - 0x10000 if v & 0x8000 else v for v in peek("r_cmp", 2, 6)],
                             [-1, 0, 32, -1, 1, 0])

            b = peek("r_bios", 2, 10)
            self.assertEqual(b[0], 2, "BIOS の版")
            self.assertEqual(b[1:5], [1, 1, 1, 1], "cls・puts・put_hex・puts_kuten の Z")
            self.assertEqual(b[5], 1, "glyph：A は ROM にある")
            self.assertEqual(b[6], 1, "acrtc_send の 0 語は打ち切らない")
            self.assertEqual(b[7], b[8], "bios_modules と FABRIC_MODULES")
            self.assertEqual(b[9], 0, "ACRTC の CER")
            self.assertTrue(0x010000 <= peek("r_glyph", 4) < 0x020000, "字形は文字 ROM の窓")
            self.assertEqual(peek("r_hook", 2), 3, "VBlank のフックは C の関数で呼べる")
            self.assertEqual(peek("r_trap", 2), 2, "interrupt_handler で TRAP #0 を受けて戻る")

            emu.call("run", frames=4, input=[[0, "START"], [2, 0]])
            self.assertTrue(peek("r_pad", 2, 2)[1] & 0x0080, "bios_read_pad の新しく押したボタン")


if __name__ == "__main__":
    unittest.main()
