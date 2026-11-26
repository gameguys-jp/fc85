#!/usr/bin/env python3
"""実験パッチのサンプルのカートリッジ SAMPLE 3 を組む。

    py sdk/samples/patches/build.py     → sdk/samples/patches/build/patches.bin（$400000 から）と patches.lst

main.s を sdk/m68kasm/m68kasm.py で組む。INCLUDE は、このフォルダと sdk/include から探す。
"""

import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "m68kasm"))

import m68kasm  # noqa: E402

CART_BASE = 0x400000
CART_MAX = 0x100000


def shown(path):
    """リポジトリの根からの相対パス（ローカルの絶対パスを出さない）。"""
    try:
        return os.path.relpath(path, ROOT).replace(os.sep, "/")
    except ValueError:
        return os.path.basename(path)


def main(argv=None):
    ap = argparse.ArgumentParser(description="実験パッチのサンプルのカートリッジを組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "patches.bin"))
    args = ap.parse_args(argv)
    out = os.path.abspath(args.output)
    os.makedirs(os.path.dirname(out), exist_ok=True)

    asm = m68kasm.Assembler(include_dirs=[HERE, os.path.join(ROOT, "sdk", "include")],
                            base=CART_BASE, fill=0xFF)
    try:
        image = asm.assemble_file(os.path.join(HERE, "main.s"))
    except m68kasm.AsmError as e:
        print(f"エラー（68000）: {e}", file=sys.stderr)
        return 1
    assert len(image) <= CART_MAX
    with open(out, "wb") as f:
        f.write(image)
    with open(os.path.splitext(out)[0] + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    for w in asm.warnings:
        print(f"注意（68000）: {w}", file=sys.stderr)
    print(f"{shown(out)}: {len(image)} バイト")
    return 0


if __name__ == "__main__":
    sys.exit(main())
