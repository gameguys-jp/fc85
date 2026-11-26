#!/usr/bin/env python3
"""ACRTC の表示のサンプルのカートリッジ SAMPLE 4 を組む。

    py sdk/samples/video/build.py     → sdk/samples/video/build/video.bin（$400000 から）と video.lst

video.s を sdk/m68kasm/m68kasm.py で組む。INCLUDE は、このフォルダと sdk/include から探す。
"""

import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "m68kasm"))

from m68kasm import AsmError, Assembler  # noqa: E402

CART_BASE = 0x400000
CART_MAX = 0x100000


def shown(path):
    try:
        return os.path.relpath(path, ROOT)
    except ValueError:          # Windows で別のドライブ
        return os.path.basename(path)


def main(argv=None):
    ap = argparse.ArgumentParser(description="ACRTC の表示のサンプルのカートリッジを組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "video.bin"))
    args = ap.parse_args(argv)
    out = os.path.abspath(args.output)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    asm = Assembler(include_dirs=[HERE, os.path.join(ROOT, "sdk", "include")],
                    base=CART_BASE, fill=0xFF)
    try:
        image = asm.assemble_file(os.path.join(HERE, "video.s"))
    except AsmError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return 1
    assert len(image) <= CART_MAX
    with open(out, "wb") as f:
        f.write(image)
    with open(os.path.splitext(out)[0] + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    for w in asm.warnings:
        print(f"注意: {w}", file=sys.stderr)
    print(f"{shown(out)}: {len(image)} バイト")
    return 0


if __name__ == "__main__":
    sys.exit(main())
