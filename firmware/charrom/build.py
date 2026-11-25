#!/usr/bin/env python3
"""文字 ROM（32 KiB、27256×1）のイメージを、字形のデータ docs/design/glyphs-v0.1.tsv から作る。

出力:
    firmware/charrom/build/char.bin         32,768 バイト。ROM の中の offset の順（CPU からは 0x010000＋offset×2）
    firmware/charrom/build/char-status.tsv  枠ごとの状態（offset、種類、符号、字）

配置（仕様書 §08 の表と式）:
    0000–07FF  英数かな 256 字枠      offset ＝ JIS X 0201 の符号 × 8
    0800–1F7F  非漢字 1〜8 区（752 枠） offset ＝ 0x0800 ＋ ((区−1)×94 ＋ (点−1))×8
    1F80–7D7F  第一水準 16〜47 区     offset ＝ 0x1F80 ＋ ((区−16)×94 ＋ (点−1))×8
    7D80–7FFF  予約
字形は 1 行 1 バイト、MSB が左、8 行で 8 バイト。

仕様書が決めていない点は、次のように決めた [P]（sdk/docs/bios-abi.md §05 にも書く）:
- 枠の種類は 5 つ。glyph（字形がある）、blank（空白の字。0x20 と 1 区 1 点。字形は 0x00×8）、
  missing（規格に字があるが、まだ描いていない。診断用の市松模様 AA 54 AA 54 AA 54 AA 00）、
  empty（規格の空き枠。0x00×8）、reserved（予約の 640 バイト。消去した EPROM と同じ 0xFF）。
- 英数字・カナは、JIS X 0201 の符号の枠と、JIS X 0208 の区点の枠の両方に同じ字形を置く。
- 規格の割当ては JIS C 6226-1978 の書き起こし。JIS X 0201：0x20–0x7E、0xA1–0xDF。1 区は全部、
  2 区は 1〜14 点、3 区は 16〜25・33〜58・65〜90 点、4 区は 1〜83 点、5 区は 1〜86 点、6 区は 1〜24・
  33〜56 点、7 区は 1〜33・49〜81 点、8 区は空き、16〜46 区は全部、47 区は 1〜51 点（合わせて第一水準
  2,965 字）。1983 年の改正で足された 2 区の 39 字と 8 区の 32 字は、JIS83 の差分 ROM の側（空き枠）[V]。
- 市松模様は、右の列（bit 0）と下の行を空ける字の規則（design/main.md §02）を守り、字と見分けやすい。
"""

import argparse
import os
import sys

ROM_SIZE = 0x8000
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
DEFAULT_TSV = os.path.join(ROOT, "docs", "design", "glyphs-v0.1.tsv")

PLACEHOLDER = bytes([0xAA, 0x54, 0xAA, 0x54, 0xAA, 0x54, 0xAA, 0x00])
BLANK = bytes(8)

ANK_BASE = 0x0000
NONKANJI_BASE = 0x0800
KANJI_BASE = 0x1F80
RESERVED_BASE = 0x7D80


def ank_offset(code):
    return ANK_BASE + code * 8


def kuten_offset(ku, ten):
    """区点から ROM の offset。範囲外は None。"""
    if not 1 <= ten <= 94:
        return None
    if 1 <= ku <= 8:
        return NONKANJI_BASE + ((ku - 1) * 94 + (ten - 1)) * 8
    if 16 <= ku <= 47:
        return KANJI_BASE + ((ku - 16) * 94 + (ten - 1)) * 8
    return None


def jis78_assigned(ku, ten):
    """JIS C 6226-1978 で字が割り当てられている区点か（1〜8 区と 16〜47 区だけを見る）。"""
    ranges = {
        1: [(1, 94)],
        2: [(1, 14)],
        3: [(16, 25), (33, 58), (65, 90)],
        4: [(1, 83)],
        5: [(1, 86)],
        6: [(1, 24), (33, 56)],
        7: [(1, 33), (49, 81)],
        8: [],
    }
    if ku in ranges:
        return any(a <= ten <= b for a, b in ranges[ku])
    if 16 <= ku <= 46:
        return True
    if ku == 47:
        return ten <= 51
    return False


def ank_assigned(code):
    return 0x20 <= code <= 0x7E or 0xA1 <= code <= 0xDF


def read_tsv(path):
    rows = []
    with open(path, encoding="utf-8") as f:
        header = None
        for line in f:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            cols = line.split("\t")
            if header is None:
                header = cols
                continue
            rec = dict(zip(header, cols))
            glyph = bytes(int(h, 16) for h in rec["glyph"].split())
            if len(glyph) != 8:
                raise ValueError(f"字形が 8 バイトでない: {line}")
            if glyph[7] != 0 or any(b & 1 for b in glyph):
                raise ValueError(f"右の列か下の行に点がある: {line}")
            ku, ten = (int(x) for x in rec["kuten"].split("-"))
            code = int(rec["jisx0201"], 16) if rec.get("jisx0201") else None
            rows.append((ku, ten, code, rec.get("char", ""), glyph))
    return rows


def build(rows):
    rom = bytearray(ROM_SIZE)
    status = {}

    def put(off, data, kind, label):
        rom[off:off + 8] = data
        status[off] = (kind, label)

    # まず全部の枠を、規格の割当てに従って missing / empty / blank にする
    for code in range(256):
        off = ank_offset(code)
        label = f"0201:{code:02X}"
        if code == 0x20:
            put(off, BLANK, "blank", label)
        elif ank_assigned(code):
            put(off, PLACEHOLDER, "missing", label)
        else:
            put(off, BLANK, "empty", label)
    for ku in list(range(1, 9)) + list(range(16, 48)):
        for ten in range(1, 95):
            off = kuten_offset(ku, ten)
            label = f"{ku:02d}-{ten:02d}"
            if (ku, ten) == (1, 1):
                put(off, BLANK, "blank", label)
            elif jis78_assigned(ku, ten):
                put(off, PLACEHOLDER, "missing", label)
            else:
                put(off, BLANK, "empty", label)
    for off in range(RESERVED_BASE, ROM_SIZE, 8):
        rom[off:off + 8] = b"\xFF" * 8
        status[off] = ("reserved", "")
    # 描いた字形を置く
    for ku, ten, code, ch, glyph in rows:
        off = kuten_offset(ku, ten)
        if off is None:
            raise ValueError(f"区点 {ku}-{ten} は ROM の範囲外")
        if not jis78_assigned(ku, ten):
            raise ValueError(f"区点 {ku}-{ten} は JIS78 の空き枠")
        put(off, glyph, "glyph", f"{ku:02d}-{ten:02d} {ch}")
        if code is not None:
            put(ank_offset(code), glyph, "glyph", f"0201:{code:02X} {ch}")
    return bytes(rom), status


def main(argv=None):
    ap = argparse.ArgumentParser(description="文字 ROM のイメージを作る")
    ap.add_argument("--tsv", default=DEFAULT_TSV)
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "char.bin"))
    args = ap.parse_args(argv)
    rows = read_tsv(args.tsv)
    rom, status = build(rows)
    os.makedirs(os.path.dirname(os.path.abspath(args.output)), exist_ok=True)
    with open(args.output, "wb") as f:
        f.write(rom)
    st_path = os.path.join(os.path.dirname(os.path.abspath(args.output)), "char-status.tsv")
    with open(st_path, "w", encoding="utf-8", newline="\n") as f:
        f.write("offset\tkind\tslot\n")
        for off in sorted(status):
            kind, label = status[off]
            f.write(f"{off:04X}\t{kind}\t{label}\n")
    counts = {}
    for kind, _ in status.values():
        counts[kind] = counts.get(kind, 0) + 1
    summary = ", ".join(f"{k} {counts[k]}" for k in ("glyph", "blank", "missing", "empty", "reserved"))
    print(f"{os.path.relpath(args.output, ROOT)}: {len(rom)} バイト（{summary}）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
