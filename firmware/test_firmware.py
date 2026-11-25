"""ファームウェアのイメージの試験。`python -m unittest firmware/test_firmware.py`（リポジトリの根で）。

Boot ROM は組んだイメージの形（ベクタ、呼出しの入口、版）を、sdk/include/bios.inc と仕様書 §05（DR-025）の
約束に照らす。文字 ROM は仕様書 §08 の配置と字形の表に照らす。動作はエミュレータで確かめる（bios-abi.md §08）。
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, "sdk", "m68kasm"))
sys.path.insert(0, os.path.join(HERE, "charrom"))

import build as charrom  # noqa: E402
from m68kasm import Assembler  # noqa: E402


def be32(b, off):
    return int.from_bytes(b[off:off + 4], "big")


def be16(b, off):
    return int.from_bytes(b[off:off + 2], "big")


class BootRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.asm = Assembler(include_dirs=[os.path.join(HERE, "boot"), os.path.join(ROOT, "sdk", "include")],
                            base=0, size=0x10000, fill=0xFF)
        cls.rom = cls.asm.assemble_file(os.path.join(HERE, "boot", "bios.s"))
        cls.sym = cls.asm.symbol_value

    def test_size_and_reset_vectors(self):
        self.assertEqual(len(self.rom), 0x10000)
        self.assertEqual(be32(self.rom, 0), 0x120000)           # SSP：Main RAM の終わり
        self.assertEqual(be32(self.rom, 4), self.sym("reset"))

    def test_vectors_point_to_jump_table(self):
        # DR-025：ベクタ 2〜255 は $100000＋6n
        for n in range(2, 256):
            self.assertEqual(be32(self.rom, 4 * n), 0x100000 + 6 * n, n)

    def test_entry_table(self):
        names = ["bios_wait_vbl", "bios_acrtc_send", "bios_cls", "bios_puts", "bios_put_hex",
                 "bios_set_pal", "bios_read_pad"]
        for i, name in enumerate(names):
            off = 0x400 + 6 * i
            self.assertEqual(be16(self.rom, off), 0x4EF9, name)    # JMP abs.L
            self.assertEqual(be32(self.rom, off + 2), self.sym(name), name)
        self.assertEqual(be16(self.rom, 0x480), 0x0001)          # BIOS_INFO：版

    def test_exception_stubs(self):
        # 入口ごとに MOVE.W #n,-(SP)（3F3C nnnn）と BRA.W exc_fatal
        base = self.sym("exc_stubs")
        fatal = self.sym("exc_fatal")
        for n in range(2, 256):
            off = base + 8 * (n - 2)
            self.assertEqual(be16(self.rom, off), 0x3F3C)
            self.assertEqual(be16(self.rom, off + 2), n)
            self.assertEqual(be16(self.rom, off + 4), 0x6000)
            disp = be16(self.rom, off + 6)
            disp -= 0x10000 if disp & 0x8000 else 0
            self.assertEqual(off + 6 + disp, fatal)

    def test_bios_variables_fit(self):
        self.assertLessEqual(self.sym("d_vars_end"), 0x100800)
        self.assertEqual(self.sym("bios_frame"), 0x100600)

    def test_code_in_rom(self):
        self.assertLess(self.sym("exc_stubs_end"), 0x10000)
        # 使っていないところは 0xFF（消去した EPROM）
        self.assertEqual(self.rom[-1], 0xFF)


class CharRom(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows = charrom.read_tsv(charrom.DEFAULT_TSV)
        cls.rom, cls.status = charrom.build(cls.rows)

    def test_size_and_reserved(self):
        self.assertEqual(len(self.rom), 0x8000)
        self.assertEqual(self.rom[0x7D80:], b"\xFF" * 640)

    def test_offsets(self):
        # 仕様書 §08 の式
        self.assertEqual(charrom.kuten_offset(1, 1), 0x0800)
        self.assertEqual(charrom.kuten_offset(8, 94), 0x1F78)
        self.assertEqual(charrom.kuten_offset(16, 1), 0x1F80)
        self.assertEqual(charrom.kuten_offset(47, 94), 0x7D78)
        self.assertIsNone(charrom.kuten_offset(9, 1))
        self.assertIsNone(charrom.kuten_offset(48, 1))

    def test_glyphs_in_both_slots(self):
        for ku, ten, code, ch, glyph in self.rows:
            off = charrom.kuten_offset(ku, ten)
            self.assertEqual(self.rom[off:off + 8], glyph, ch)
            if code is not None:
                self.assertEqual(self.rom[code * 8:code * 8 + 8], glyph, ch)

    def test_slot_kinds(self):
        self.assertEqual(self.rom[0x20 * 8:0x21 * 8], bytes(8))                  # 空白
        self.assertEqual(self.rom[0x61 * 8:0x62 * 8], charrom.PLACEHOLDER)       # 'a' は未作成
        self.assertEqual(self.rom[0x80 * 8:0x81 * 8], bytes(8))                  # 0x80 は空き枠
        off = charrom.kuten_offset(8, 1)                                        # JIS78 では空き
        self.assertEqual(self.rom[off:off + 8], bytes(8))
        self.assertEqual(self.status[off][0], "empty")

    def test_placeholder_keeps_margins(self):
        p = charrom.PLACEHOLDER
        self.assertEqual(p[7], 0)
        self.assertFalse(any(b & 1 for b in p))


if __name__ == "__main__":
    unittest.main()
