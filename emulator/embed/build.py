#!/usr/bin/env python3
"""組込みの窓口（DR-104）を組む。

  py emulator/embed/build.py            WebAssembly（emulator/embed/build/fc85emu.wasm）と、Web の試しの頁で
                                        読む Boot ROM・文字 ROM（firmware の組立ての結果があれば）
  py emulator/embed/build.py --native   C の ABI の共有ライブラリ（emulator/target/release/ の fc85emu.dll・
                                        libfc85emu.so・libfc85emu.dylib と静的なライブラリ）
  py emulator/embed/build.py --libretro libretro のコア（emulator/target/release/ の fc85_libretro.dll など）

WebAssembly は Rust の wasm32-unknown-unknown の target が要る（rustup target add wasm32-unknown-unknown）。
Python 3 の標準ライブラリだけで書く。
"""

import argparse
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
EMULATOR = os.path.normpath(os.path.join(HERE, ".."))
REPO = os.path.normpath(os.path.join(EMULATOR, ".."))
BUILD = os.path.join(HERE, "build")
WASM_TARGET = "wasm32-unknown-unknown"
FIRMWARE = {
    "boot.bin": os.path.join(REPO, "firmware", "boot", "build", "boot.bin"),
    "char.bin": os.path.join(REPO, "firmware", "charrom", "build", "char.bin"),
}


class BuildError(Exception):
    pass


def cargo(args):
    if shutil.which("cargo") is None:
        raise BuildError("cargo がない（Rust を入れる）")
    r = subprocess.run(["cargo"] + args, cwd=EMULATOR, capture_output=True)
    if r.returncode != 0:
        raise BuildError(r.stderr.decode("utf-8", "replace")[-4000:])


def has_wasm_target():
    """wasm32-unknown-unknown の標準ライブラリが入っているか（rustup があるときだけ分かる）。"""
    if shutil.which("rustup") is None:
        return shutil.which("cargo") is not None
    r = subprocess.run(["rustup", "target", "list", "--installed"], capture_output=True)
    return WASM_TARGET in r.stdout.decode("utf-8", "replace").split()


def lib_name(stem):
    if sys.platform == "win32":
        return stem + ".dll"
    if sys.platform == "darwin":
        return "lib" + stem + ".dylib"
    return "lib" + stem + ".so"


def build_native():
    """C の ABI の共有ライブラリを組み、その道を返す。"""
    cargo(["build", "--release", "-p", "fc85-capi"])
    return os.path.join(EMULATOR, "target", "release", lib_name("fc85emu"))


def build_libretro():
    cargo(["build", "--release", "-p", "fc85-libretro"])
    return os.path.join(EMULATOR, "target", "release", lib_name("fc85_libretro"))


def build_wasm(copy_firmware=True):
    """WebAssembly を組んで build/ に写し、その道を返す。"""
    if not has_wasm_target():
        raise BuildError(f"Rust の {WASM_TARGET} がない（rustup target add {WASM_TARGET}）")
    cargo(["build", "--release", "--target", WASM_TARGET, "-p", "fc85-capi"])
    os.makedirs(BUILD, exist_ok=True)
    src = os.path.join(EMULATOR, "target", WASM_TARGET, "release", "fc85emu.wasm")
    dst = os.path.join(BUILD, "fc85emu.wasm")
    shutil.copyfile(src, dst)
    if copy_firmware:
        for name, path in FIRMWARE.items():
            if os.path.exists(path):
                shutil.copyfile(path, os.path.join(BUILD, name))
    return dst


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--native", action="store_true", help="C の ABI の共有ライブラリ")
    ap.add_argument("--libretro", action="store_true", help="libretro のコア")
    a = ap.parse_args(argv)
    try:
        if a.native:
            print(build_native())
        if a.libretro:
            print(build_libretro())
        if not (a.native or a.libretro):
            print(build_wasm())
    except BuildError as e:
        print(e, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
