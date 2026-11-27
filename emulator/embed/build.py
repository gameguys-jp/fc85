#!/usr/bin/env python3
"""組込みの窓口（DR-104）を組む。

  py emulator/embed/build.py            WebAssembly（emulator/embed/build/fc85emu.wasm）と、Web の試しの頁で
                                        読む Boot ROM・文字 ROM（firmware の組立ての結果があれば）
  py emulator/embed/build.py --native   C の ABI の共有ライブラリ（emulator/target/release/ の fc85emu.dll・
                                        libfc85emu.so・libfc85emu.dylib と静的なライブラリ）
  py emulator/embed/build.py --libretro libretro のコア（emulator/target/release/ の fc85_libretro.dll など）

WebAssembly は Rust の wasm32-unknown-unknown の target が要る（rustup target add wasm32-unknown-unknown）。
binaryen の wasm-opt があれば、組んだ WebAssembly を -O3 で最適化する（DR-108。約 1 割速くなり、振る舞いは
同じ）。--wasm-opt で道を渡すか、環境変数 WASM_OPT か PATH から探す。なければ最適化を省いて知らせる。
--no-wasm-opt で省く。Python 3 の標準ライブラリだけで書く。
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


def find_wasm_opt(path=None):
    """binaryen の wasm-opt を探す（引数、環境変数 WASM_OPT、PATH の順）。なければ None。"""
    for p in (path, os.environ.get("WASM_OPT")):
        if p:
            if not os.path.isfile(p):
                raise BuildError(f"wasm-opt がない：{p}")
            return p
    return shutil.which("wasm-opt")


def build_wasm(copy_firmware=True, optimize=True, wasm_opt=None):
    """WebAssembly を組んで build/ に置き、その道を返す。optimize なら、wasm-opt があれば -O3 で最適化する
    （なければ最適化を省き、標準エラーに知らせる）。"""
    if not has_wasm_target():
        raise BuildError(f"Rust の {WASM_TARGET} がない（rustup target add {WASM_TARGET}）")
    cargo(["build", "--release", "--target", WASM_TARGET, "-p", "fc85-capi"])
    os.makedirs(BUILD, exist_ok=True)
    src = os.path.join(EMULATOR, "target", WASM_TARGET, "release", "fc85emu.wasm")
    dst = os.path.join(BUILD, "fc85emu.wasm")
    exe = find_wasm_opt(wasm_opt) if optimize else None
    if exe:
        r = subprocess.run([exe, "-O3", src, "-o", dst], capture_output=True)
        if r.returncode != 0:
            raise BuildError("wasm-opt が失敗した：" + r.stderr.decode("utf-8", "replace")[-4000:])
    else:
        if optimize:
            print("wasm-opt（binaryen）がないので最適化を省いた（入れると約 1 割速い）", file=sys.stderr)
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
    ap.add_argument("--wasm-opt", metavar="PATH", help="binaryen の wasm-opt の道（既定は WASM_OPT か PATH）")
    ap.add_argument("--no-wasm-opt", action="store_true", help="WebAssembly を最適化しない")
    a = ap.parse_args(argv)
    try:
        if a.native:
            print(build_native())
        if a.libretro:
            print(build_libretro())
        if not (a.native or a.libretro):
            print(build_wasm(optimize=not a.no_wasm_opt, wasm_opt=a.wasm_opt))
    except BuildError as e:
        print(e, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
