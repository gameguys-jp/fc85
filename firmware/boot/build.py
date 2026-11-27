#!/usr/bin/env python3
"""Boot ROM（BIOS）を組む。

    python firmware/boot/build.py            → firmware/boot/build/boot.bin（64 KiB、空きは 0xFF）と boot.lst

あわせて、BIOS の約束を確かめる試験のカートリッジ abitest.s を build/abitest.bin（$400000 から）と
abitest.lst に組む（board の試験 console_boot.rs が使う。DR-094）。

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
CART_BASE = 0x400000
CART_MAX = 0x100000


def assemble(src, base, size=None):
    asm = Assembler(include_dirs=[HERE, os.path.join(ROOT, "sdk", "include")],
                    base=base, size=size, fill=0xFF)
    return asm, asm.assemble_file(os.path.join(HERE, src))


def write(out, image, asm):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "wb") as f:
        f.write(image)
    with open(os.path.splitext(out)[0] + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())


def main(argv=None):
    ap = argparse.ArgumentParser(description="Boot ROM を組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "boot.bin"))
    ap.add_argument("-l", "--listing", default=None, help="既定は出力と同じ場所の boot.lst")
    args = ap.parse_args(argv)
    try:
        asm, image = assemble("bios.s", 0, ROM_SIZE)
        test_asm, test_image = assemble("abitest.s", CART_BASE)
    except AsmError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return 1
    assert len(image) == ROM_SIZE
    assert len(test_image) <= CART_MAX
    out = os.path.abspath(args.output)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "wb") as f:
        f.write(image)
    lst = args.listing or os.path.splitext(out)[0] + ".lst"
    with open(lst, "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    used = ROM_SIZE - len(image.rstrip(b"\xFF"))
    print(f"{os.path.relpath(out, ROOT)}: {len(image)} バイト（後ろの空き {used} バイト）")
    test_out = os.path.join(os.path.dirname(out), "abitest.bin")
    write(test_out, test_image, test_asm)
    print(f"{os.path.relpath(test_out, ROOT)}: {len(test_image)} バイト（試験のカートリッジ）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
