#!/usr/bin/env python3
"""この ROM を組む。

    py build.py            → build/rom.bin（$400000 から）と build/rom.lst
    py build.py -o X.bin   → X.bin と X.lst

main.s を sdk/m68kasm/m68kasm.py で組む。INCLUDE と INCBIN は、このフォルダ、build/、sdk/include から探す。
リポジトリの根（sdk/ があるところ）は、--root、環境変数 FC85_ROOT、上のフォルダ、`fc85-root` の順に探す
（sdk/tools/fc85emu.py の find_root と同じ）。
"""

import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CART_BASE = 0x400000
CART_MAX = 0x100000
SOURCE = "main.s"
OUTPUT = os.path.join(HERE, "build", "rom.bin")


def find_root(start=HERE):
    """リポジトリの根。fc85emu.find_root を読む前に要るので、同じ探し方をここにも書く。"""
    env = os.environ.get("FC85_ROOT")
    if env and os.path.isfile(os.path.join(env, "sdk", "m68kasm", "m68kasm.py")):
        return os.path.abspath(env)
    d = os.path.abspath(start)
    while True:
        if os.path.isfile(os.path.join(d, "sdk", "m68kasm", "m68kasm.py")):
            return d
        marker = os.path.join(d, "fc85-root")
        if os.path.isfile(marker):
            with open(marker, encoding="utf-8") as f:
                p = os.path.normpath(os.path.join(d, f.read().strip()))
            if os.path.isfile(os.path.join(p, "sdk", "m68kasm", "m68kasm.py")):
                return p
        up = os.path.dirname(d)
        if up == d:
            return None
        d = up


def main(argv=None):
    ap = argparse.ArgumentParser(description="この ROM を組む")
    ap.add_argument("-o", "--output", default=OUTPUT)
    ap.add_argument("--root", help="リポジトリの根（sdk/ があるところ）")
    args = ap.parse_args(argv)
    root = os.path.abspath(args.root) if args.root else find_root()
    if root is None:
        print("エラー: リポジトリの根が見つからない（--root、FC85_ROOT、fc85-root のどれかで示す）",
              file=sys.stderr)
        return 1
    sys.path.insert(0, os.path.join(root, "sdk", "m68kasm"))
    from m68kasm import AsmError, Assembler

    out = os.path.abspath(args.output)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    asm = Assembler(include_dirs=[HERE, os.path.join(HERE, "build"), os.path.join(root, "sdk", "include")],
                    base=CART_BASE, fill=0xFF)
    try:
        image = asm.assemble_file(os.path.join(HERE, SOURCE))
    except AsmError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return 1
    if len(image) > CART_MAX:
        print(f"エラー: {len(image)} バイトはカートリッジの窓（1 MiB）に入らない", file=sys.stderr)
        return 1
    with open(out, "wb") as f:
        f.write(image)
    with open(os.path.splitext(out)[0] + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    for w in asm.warnings:
        print(f"注意: {w}", file=sys.stderr)
    print(f"{os.path.basename(out)}: {len(image)} バイト")
    return 0


if __name__ == "__main__":
    sys.exit(main())
