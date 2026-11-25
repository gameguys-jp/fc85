#!/usr/bin/env python3
"""Boot ROM（BIOS）を組む。

    python firmware/boot/build.py            → firmware/boot/build/boot.bin（64 KiB、空きは 0xFF）と boot.lst

アセンブラは sdk/m68kasm/m68kasm.py。INCLUDE は firmware/boot と sdk/include から探す。
"""

import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "m68kasm"))

from m68kasm import AsmError, Assembler  # noqa: E402

ROM_SIZE = 0x10000


def main(argv=None):
    ap = argparse.ArgumentParser(description="Boot ROM を組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "boot.bin"))
    ap.add_argument("-l", "--listing", default=None, help="既定は出力と同じ場所の boot.lst")
    args = ap.parse_args(argv)
    asm = Assembler(include_dirs=[HERE, os.path.join(ROOT, "sdk", "include")],
                    base=0, size=ROM_SIZE, fill=0xFF)
    try:
        image = asm.assemble_file(os.path.join(HERE, "bios.s"))
    except AsmError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return 1
    assert len(image) == ROM_SIZE
    out = os.path.abspath(args.output)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "wb") as f:
        f.write(image)
    lst = args.listing or os.path.splitext(out)[0] + ".lst"
    with open(lst, "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    used = ROM_SIZE - len(image.rstrip(b"\xFF"))
    print(f"{os.path.relpath(out, ROOT)}: {len(image)} バイト（後ろの空き {used} バイト）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
