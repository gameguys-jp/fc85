#!/usr/bin/env python3
"""エミュレータの窓口（fc85-api。sdk/docs/emulator-api.md）を Python から使う小さな道具。

ROM の試験とエージェントの台本から使う。窓口のプログラムを子のプロセスとして動かし、JSON の手順（1 行に
1 つの要求）でやり取りする。

    from fc85emu import Emulator
    with Emulator() as emu:                       # リポジトリの根を自動で探す
        emu.call("load", cart="build/rom.bin")
        r = emu.call("run", frames=60, input=[[0, 0], [30, "START"], [31, 0]])
        assert r["bus_errors"] == 0
        png = emu.screenshot()                    # PNG の byte

窓口のプログラムは `emulator/` で `cargo build --release -p fc85-api` で作る（なければ debug も探す）。
"""

import base64
import json
import os
import subprocess

EXE = "fc85-api.exe" if os.name == "nt" else "fc85-api"


class EmuError(Exception):
    """窓口が誤りを返した。"""


def find_root(start=None):
    """リポジトリの根（sdk/m68kasm/m68kasm.py があるところ）を探す。

    順に：環境変数 FC85_ROOT、start から上へたどったフォルダ、start から上へたどって見つけた `fc85-root`
    （中に根への道を 1 行。相対ならそのファイルのあるフォルダから）。見つからなければ None。
    """
    env = os.environ.get("FC85_ROOT")
    if env and _is_root(env):
        return os.path.abspath(env)
    d = os.path.abspath(start or os.getcwd())
    while True:
        if _is_root(d):
            return d
        marker = os.path.join(d, "fc85-root")
        if os.path.isfile(marker):
            with open(marker, encoding="utf-8") as f:
                p = f.read().strip()
            p = os.path.normpath(os.path.join(d, p))
            if _is_root(p):
                return p
        up = os.path.dirname(d)
        if up == d:
            return None
        d = up


def _is_root(d):
    return os.path.isfile(os.path.join(d, "sdk", "m68kasm", "m68kasm.py"))


def find_api(root):
    """窓口のプログラムの道（release を先に）。なければ None。"""
    for kind in ("release", "debug"):
        p = os.path.join(root, "emulator", "target", kind, EXE)
        if os.path.isfile(p):
            return p
    return None


class Emulator:
    """窓口のプログラムを 1 つ動かす。`call` で手順を呼ぶ。`with` で使うと終わりに閉じる。"""

    def __init__(self, root=None, exe=None, args=()):
        self.root = root or find_root()
        if self.root is None:
            raise EmuError("リポジトリの根が見つからない（FC85_ROOT か fc85-root を置く）")
        self.exe = exe or find_api(self.root)
        if self.exe is None:
            raise EmuError("fc85-api がない（emulator/ で cargo build --release -p fc85-api）")
        self.proc = subprocess.Popen(
            [self.exe, "--root", self.root, *args],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, encoding="utf-8")
        self._id = 0

    def call(self, method, **params):
        """手順を 1 つ呼び、`result` を返す。誤りなら EmuError。PNG は `result` の `png_base64` に入る。"""
        self._id += 1
        line = json.dumps({"id": self._id, "method": method, "params": params}, ensure_ascii=False)
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()
        reply = self.proc.stdout.readline()
        if not reply:
            err = self.proc.stderr.read()
            raise EmuError(f"窓口が終わった：{err.strip()}")
        v = json.loads(reply)
        if "error" in v:
            raise EmuError(f"{method}: {v['error'].get('message')}")
        return v["result"]

    def screenshot(self, area="active", scale=1):
        """画面の PNG の byte。"""
        r = self.call("screenshot", area=area, scale=scale)
        return base64.b64decode(r["png_base64"])

    def close(self):
        if self.proc.poll() is None:
            self.proc.stdin.close()
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        for f in (self.proc.stdout, self.proc.stderr):
            f.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
