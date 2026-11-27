#!/usr/bin/env python3
"""C で書いた Temporal RAM の残像のサンプル SAMPLE 8 を組む（DR-114）。

    py sdk/samples/echo/build.py     → sdk/samples/echo/build/echo.bin（$400000 から）と echo.lst

main.c を、SDK の実行時ライブラリ（acrtc.c・fabric.c・temporal.c ほか）と一緒に 68000 の GCC で組む
（sdk/tools/m68kgcc.py、sdk/docs/c.md）。echo.elf・echo.map・echo.dis（逆アセンブル）も書く。
GCC がなければ、そう言って 2 を返す。
"""

import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "tools"))

import m68kgcc  # noqa: E402

NO_TOOLCHAIN = 2


def shown(path):
    try:
        return os.path.relpath(path, ROOT)
    except ValueError:          # Windows で別のドライブ
        return os.path.basename(path)


def main(argv=None):
    ap = argparse.ArgumentParser(description="C で書いた Temporal RAM の残像のサンプルのカートリッジを組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "echo.bin"))
    args = ap.parse_args(argv)
    out = os.path.abspath(args.output)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    tc = m68kgcc.find_toolchain()
    if tc is None:
        print("68000 の GCC がない（sdk/docs/c.md §02）ので組まない", file=sys.stderr)
        return NO_TOOLCHAIN
    try:
        size = m68kgcc.build([os.path.join(HERE, "main.c")], out, ROOT, include_dirs=[HERE], toolchain=tc)
    except m68kgcc.ToolchainError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return 1
    print(f"{shown(out)}: {size} バイト")
    return 0


if __name__ == "__main__":
    sys.exit(main())
