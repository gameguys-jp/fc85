#!/usr/bin/env python3
"""この ROM を組む。

    py build.py            → build/rom.bin（$400000 から）と build/rom.lst
    py build.py -o X.bin   → X.bin と X.lst

このフォルダに .c があれば C の ROM：このフォルダの .c と .S（GNU as。大文字の S）を、SDK の実行時ライブラリと
一緒に GCC で組む（sdk/tools/m68kgcc.py、sdk/docs/c.md）。build/rom.elf・rom.map・rom.dis（逆アセンブル）も書く。
.c がなければアセンブラの ROM：main.s を sdk/m68kasm/m68kasm.py で組む。INCLUDE と INCBIN は、このフォルダ、
build/、sdk/include から探す。

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
OPT = "-Os"         # C の最適化（-O2 なども使える）
CFLAGS = []         # C に足す引数（例：["-DDEBUG"]）


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


def c_sources():
    """このフォルダの .c と .S（名前の順）。大文字と小文字を区別する（.s は m68kasm の書き方）。"""
    return sorted(os.path.join(HERE, n) for n in os.listdir(HERE) if n.endswith((".c", ".S")))


def is_c():
    return any(n.endswith(".c") for n in os.listdir(HERE))


def build_c(root, out):
    sys.path.insert(0, os.path.join(root, "sdk", "tools"))
    from m68kgcc import ToolchainError, build
    try:
        return build(c_sources(), out, root, include_dirs=[HERE], cflags=CFLAGS, opt=OPT)
    except ToolchainError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return None


def build_asm(root, out):
    sys.path.insert(0, os.path.join(root, "sdk", "m68kasm"))
    from m68kasm import AsmError, Assembler
    asm = Assembler(include_dirs=[HERE, os.path.join(HERE, "build"), os.path.join(root, "sdk", "include")],
                    base=CART_BASE, fill=0xFF)
    try:
        image = asm.assemble_file(os.path.join(HERE, SOURCE))
    except AsmError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return None
    if len(image) > CART_MAX:
        print(f"エラー: {len(image)} バイトはカートリッジの窓（1 MiB）に入らない", file=sys.stderr)
        return None
    with open(out, "wb") as f:
        f.write(image)
    with open(os.path.splitext(out)[0] + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    for w in asm.warnings:
        print(f"注意: {w}", file=sys.stderr)
    return len(image)


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
    out = os.path.abspath(args.output)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    size = build_c(root, out) if is_c() else build_asm(root, out)
    if size is None:
        return 1
    print(f"{os.path.basename(out)}: {size} バイト")
    return 0


if __name__ == "__main__":
    sys.exit(main())
