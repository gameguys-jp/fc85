#!/usr/bin/env python3
"""MIDI で弾く DOC のシンセのサンプル SAMPLE 9 を組む（DR-128）。

    py sdk/samples/midi/build.py     → sdk/samples/midi/build/midi.bin（$400000 から）と midi.lst

main.c を、SDK の実行時ライブラリ（midi.c・doc.c・acrtc.c ほか）と一緒に 68000 の GCC で組む
（sdk/tools/m68kgcc.py、sdk/docs/c.md）。midi.elf・midi.map・midi.dis（逆アセンブル）も書く。
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
    ap = argparse.ArgumentParser(description="MIDI で弾く DOC のシンセのサンプルのカートリッジを組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "midi.bin"))
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
