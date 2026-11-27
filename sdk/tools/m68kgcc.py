#!/usr/bin/env python3
"""C の ROM を GCC で組む（DR-103。説明は sdk/docs/c.md）。

    from m68kgcc import build, find_toolchain
    build(["main.c"], "build/rom.bin", root)     # rom.bin・rom.lst・rom.elf・rom.map・rom.dis

GCC は外の道具で、SDK に入れない（DR-092 (3)）。探す順：
1. 環境変数 FC85_GCC：道具の名前の前置き（`m68k-elf-`、`/opt/m68k/bin/m68k-elf-` など）。
   `wsl:m68k-linux-gnu-` のように `wsl:` を付けると WSL の中の道具を使う。
2. PATH の `m68k-elf-gcc`、`m68k-linux-gnu-gcc`。
3. Windows では WSL の中の `m68k-linux-gnu-gcc`、`m68k-elf-gcc`（環境変数 FC85_WSL_DISTRO で distro を選ぶ）。

どの GCC でも、68000 の命令だけを使い、GCC の実行時ライブラリ（libgcc）と C の標準ライブラリはつながない。
要る関数（掛け算・割り算、64 bit のシフト、memcpy など）は sdk/c/ の小さな実行時ライブラリが持つ。
"""

import os
import re
import shlex
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SDK = os.path.normpath(os.path.join(HERE, ".."))
C_DIR = os.path.join(SDK, "c")
RUNTIME = ["crt0.S", "bios.S", "runtime.S", "runtime64.c", "string.c"]
LINKER_SCRIPT = "fc85.ld"
CART_MAX = 0x100000

CFLAGS = [
    "-m68000", "-std=gnu11", "-g",
    "-ffreestanding", "-nostdinc",
    "-fno-pic", "-fno-pie", "-fno-stack-protector", "-fno-delete-null-pointer-checks",
    "-fno-asynchronous-unwind-tables", "-fno-unwind-tables", "-fno-common",
    "-ffunction-sections", "-fdata-sections",
    "-Wall", "-Wextra", "-Werror=implicit-function-declaration",
]
LDFLAGS = [
    "-nostdlib", "-nostartfiles", "-no-pie", "-static",
    "-Wl,--gc-sections", "-Wl,--build-id=none", "-Wl,--orphan-handling=warn",
]
DEFAULT_OPT = "-Os"
GCC_INCLUDE = object()        # 引数の列の中で、GCC 自身の include のフォルダ（stdint.h など）に置き換える印


class ToolchainError(Exception):
    pass


def _wsl_base():
    distro = os.environ.get("FC85_WSL_DISTRO")
    return ["wsl"] + (["-d", distro] if distro else []) + ["-e"]


def _clean_stderr(data):
    """WSL が出す UTF-16 の注意（NUL が混じる行）を落とす。"""
    text = data.decode("utf-8", "replace")
    return "\n".join(line for line in text.splitlines() if "\x00" not in line).strip()


class Toolchain:
    """前置き（`m68k-elf-` など）と、WSL を通すか。"""

    def __init__(self, prefix, wsl=False):
        self.prefix = prefix
        self.wsl = wsl
        self._include = None

    def __repr__(self):
        return f"Toolchain({self.prefix!r}, wsl={self.wsl})"

    def describe(self):
        return ("WSL の " if self.wsl else "") + self.prefix + "gcc"

    def tool(self, name):
        return self.prefix + name

    def path(self, p):
        """この道具から見た道。WSL なら /mnt/<ドライブ>/…（環境変数 FC85_WSL_MOUNT で /mnt/ を替える）。"""
        p = os.path.abspath(p)
        if not self.wsl:
            return p
        if p.startswith("\\\\") or len(p) < 2 or p[1] != ":":
            raise ToolchainError(f"WSL から見えない道：{p}")
        mount = os.environ.get("FC85_WSL_MOUNT", "/mnt/")
        return mount.rstrip("/") + "/" + p[0].lower() + p[2:].replace("\\", "/")

    def include_dir(self):
        """GCC 自身の include のフォルダ（-nostdinc の後に足す。stdint.h・stddef.h など）。"""
        if self._include is None:
            r = self.run([([self.tool("gcc"), "-print-file-name=include"], None)], capture=True)
            self._include = r.strip()
        return self._include

    def run(self, steps, capture=False):
        """steps は (引数の列, 標準出力を書くファイルか None) の列。WSL なら 1 回の sh で続けて回す。
        失敗したら ToolchainError（GCC の誤りの文を持つ）。capture なら最後の標準出力を返す。"""
        if self.wsl:
            parts = []
            for argv, out in steps:
                words = []
                for a in argv:
                    if a is GCC_INCLUDE:
                        words.append(f'"$({shlex.quote(self.tool("gcc"))} -print-file-name=include)"')
                    else:
                        words.append(shlex.quote(a))
                cmd = " ".join(words)
                if out is not None:
                    cmd += " > " + shlex.quote(self.path(out))
                parts.append(cmd)
            r = subprocess.run(_wsl_base() + ["sh", "-c", " && ".join(parts)], capture_output=True)
            err = _clean_stderr(r.stderr)
            if r.returncode != 0:
                raise ToolchainError(err or f"終了コード {r.returncode}")
            if err:
                print(err, file=sys.stderr)
            return r.stdout.decode("utf-8", "replace") if capture else None
        last = None
        for argv, out in steps:
            argv = [self.include_dir() if a is GCC_INCLUDE else a for a in argv]
            r = subprocess.run(argv, capture_output=True)
            err = r.stderr.decode("utf-8", "replace").strip()
            if r.returncode != 0:
                raise ToolchainError(err or f"{argv[0]}：終了コード {r.returncode}")
            if err:
                print(err, file=sys.stderr)
            if out is not None:
                with open(out, "wb") as f:
                    f.write(r.stdout)
            last = r.stdout
        return last.decode("utf-8", "replace") if capture and last is not None else None


_found = {}


def _wsl_has(name):
    try:
        r = subprocess.run(_wsl_base() + ["sh", "-c", f"command -v {shlex.quote(name)}"],
                           capture_output=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return False
    return r.returncode == 0 and r.stdout.strip() != b""


def find_toolchain():
    """GCC を探す。なければ None。結果はこのプロセスの中で覚える。"""
    if "tc" in _found:
        return _found["tc"]
    tc = None
    env = os.environ.get("FC85_GCC")
    if env:
        tc = Toolchain(env[4:], wsl=True) if env.startswith("wsl:") else Toolchain(env)
    else:
        for prefix in ("m68k-elf-", "m68k-linux-gnu-"):
            if shutil.which(prefix + "gcc"):
                tc = Toolchain(prefix)
                break
        if tc is None and os.name == "nt" and shutil.which("wsl"):
            for prefix in ("m68k-linux-gnu-", "m68k-elf-"):
                if _wsl_has(prefix + "gcc"):
                    tc = Toolchain(prefix, wsl=True)
                    break
    _found["tc"] = tc
    return tc


NM_RE = re.compile(r"^([0-9a-fA-F]{8})\s+([A-Za-z])\s+(\S+)$")


def symbol_table(nm_text):
    """`nm -n` の出力 → [(番地, 名前)]。関数・変数・定数・リンカの記号だけ。"""
    out = []
    for line in nm_text.splitlines():
        m = NM_RE.match(line.strip())
        if not m:
            continue
        addr, kind, name = int(m.group(1), 16), m.group(2), m.group(3)
        if kind.upper() not in "TDBRA" or name.startswith((".", "$")):
            continue
        out.append((addr, name))
    return out


def listing_text(symbols, sources):
    """エミュレータの窓口が読む形（m68kasm のリストの末尾の表と同じ。2 字下げ、名前、$ と 8 桁）。"""
    lines = ["; C の ROM の記号の表（sdk/tools/m68kgcc.py が rom.elf から作る）。逆アセンブルは rom.dis。",
             "; 元：" + "、".join(os.path.basename(s) for s in sources), "", "SYMBOLS"]
    lines += [f"  {name} ${addr:08X}" for addr, name in symbols]
    return "\n".join(lines) + "\n"


def build(sources, output, root=None, include_dirs=(), cflags=(), opt=DEFAULT_OPT, toolchain=None):
    """sources（.c と .S）と SDK の実行時ライブラリを組み、output（.bin）と横の .lst・.elf・.map・.dis を書く。
    大きさ（byte）を返す。失敗は ToolchainError。"""
    tc = toolchain or find_toolchain()
    if tc is None:
        raise ToolchainError("68000 の GCC が見つからない（sdk/docs/c.md §02。Ubuntu・WSL なら "
                             "`sudo apt install gcc-m68k-linux-gnu`。環境変数 FC85_GCC で示してもよい）")
    sdk = os.path.join(root, "sdk") if root else SDK
    c_dir = os.path.join(sdk, "c")
    output = os.path.abspath(output)
    base = os.path.splitext(output)[0]
    os.makedirs(os.path.dirname(output), exist_ok=True)
    elf, mapf, nmf, dis = base + ".elf", base + ".map", base + ".nm", base + ".dis"
    for p in (output, elf):
        if os.path.exists(p):
            os.remove(p)
    srcs = [os.path.abspath(s) for s in sources] + [os.path.join(c_dir, s) for s in RUNTIME]
    incs = [os.path.abspath(d) for d in include_dirs] + [os.path.join(c_dir, "include"), os.path.join(sdk, "include")]
    argv = [tc.tool("gcc"), opt, *CFLAGS, "-isystem", GCC_INCLUDE]
    for d in incs:
        argv += ["-I", tc.path(d)]
    argv += list(cflags) + [tc.path(s) for s in srcs]
    argv += [*LDFLAGS, "-T", tc.path(os.path.join(c_dir, LINKER_SCRIPT)), f"-Wl,-Map={tc.path(mapf)}",
             "-o", tc.path(elf)]
    tc.run([
        (argv, None),
        ([tc.tool("objcopy"), "-O", "binary", "-j", ".text", "-j", ".data", tc.path(elf), tc.path(output)], None),
        ([tc.tool("nm"), "-n", tc.path(elf)], nmf),
        ([tc.tool("objdump"), "-d", "-S", "--no-show-raw-insn", tc.path(elf)], dis),
    ])
    with open(nmf, encoding="utf-8", errors="replace") as f:
        symbols = symbol_table(f.read())
    os.remove(nmf)
    with open(base + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(listing_text(symbols, sources))
    size = os.path.getsize(output)
    if size > CART_MAX:
        raise ToolchainError(f"{size} バイトはカートリッジの窓（1 MiB）に入らない")
    return size


def main(argv=None):
    import argparse
    ap = argparse.ArgumentParser(description="C の ROM を組む（sdk/docs/c.md）")
    ap.add_argument("sources", nargs="*", help=".c と .S")
    ap.add_argument("-o", "--output", default="build/rom.bin")
    ap.add_argument("-I", dest="include", action="append", default=[])
    ap.add_argument("-O", dest="opt", default=DEFAULT_OPT[2:], help="最適化（s、2 など。既定 s）")
    ap.add_argument("--which", action="store_true", help="使う GCC を出して終わる")
    args = ap.parse_args(argv)
    if args.which:
        tc = find_toolchain()
        print(tc.describe() if tc else "見つからない")
        return 0 if tc else 1
    try:
        size = build(args.sources, args.output, include_dirs=args.include, opt="-O" + args.opt)
    except ToolchainError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return 1
    print(f"{os.path.basename(args.output)}: {size} バイト")
    return 0


if __name__ == "__main__":
    sys.exit(main())
