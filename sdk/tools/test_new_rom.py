#!/usr/bin/env python3
"""new_rom.py（ひな形から ROM のフォルダを作る）と fc85emu.py（窓口の道具）の試験。"""

import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import fc85emu  # noqa: E402
import new_rom  # noqa: E402


def quiet(f, *args):
    """知らせ（標準出力と標準エラー）を捨てて呼ぶ。"""
    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
        return f(*args)


class TestNewRom(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dest = os.path.join(self.tmp.name, "my-first rom")

    def tearDown(self):
        self.tmp.cleanup()

    def test_creates_a_project_that_builds_and_finds_the_sdk(self):
        self.assertEqual(quiet(new_rom.main, [self.dest, "--title", "HELLO 85", "--lang", "asm"]), 0)
        for name in new_rom.FILES + ["main.s", "fc85-root", ".mcp.json"]:
            self.assertTrue(os.path.isfile(os.path.join(self.dest, name)), name)
        self.assertFalse(os.path.exists(os.path.join(self.dest, "main.c")))
        with open(os.path.join(self.dest, "main.s"), encoding="utf-8") as f:
            text = f.read()
        self.assertIn('"HELLO 85        "', text)
        self.assertIn('"HELLO 85",0', text)
        self.assertNotIn("TEMPLATE", text)
        # フォルダの外からでも根を見つける
        self.assertEqual(os.path.normcase(fc85emu.find_root(self.dest)), os.path.normcase(new_rom.ROOT))
        with open(os.path.join(self.dest, ".mcp.json"), encoding="utf-8") as f:
            server = json.load(f)["mcpServers"]["fc85"]
        self.assertEqual(server["args"][:2], ["--mcp", "--root"])
        self.assertTrue(server["command"].endswith(new_rom.EXE))
        r = subprocess.run([sys.executable, "build.py"], cwd=self.dest, capture_output=True,
                           text=True, encoding="utf-8")
        self.assertEqual(r.returncode, 0, r.stderr)
        with open(os.path.join(self.dest, "build", "rom.bin"), "rb") as f:
            image = f.read()
        self.assertEqual(image[:4], b"CART")
        self.assertEqual(image[16:32], b"HELLO 85".ljust(16))
        # 作ったフォルダの試験（エミュレータの窓口があれば ROM を動かす）
        r = subprocess.run([sys.executable, "-m", "unittest"], cwd=self.dest, capture_output=True,
                           text=True, encoding="utf-8")
        self.assertEqual(r.returncode, 0, r.stderr)
        # .mcp.json の設定で MCP サーバーが立ち、initialize に答える
        exe = os.path.normpath(os.path.join(self.dest, server["command"]))
        if os.path.isfile(exe):
            req = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                              "params": {"protocolVersion": "2025-06-18"}})
            r = subprocess.run([exe, *server["args"]], cwd=self.dest, input=req + "\n",
                               capture_output=True, text=True, encoding="utf-8", timeout=60)
            self.assertEqual(json.loads(r.stdout)["result"]["serverInfo"]["name"], "fc85")

    def test_the_c_project_is_the_default(self):
        notes = quiet(new_rom.create, self.dest, "HELLO C")
        self.assertTrue(os.path.isfile(os.path.join(self.dest, "main.c")))
        self.assertFalse(os.path.exists(os.path.join(self.dest, "main.s")))
        with open(os.path.join(self.dest, "main.c"), encoding="utf-8") as f:
            text = f.read()
        self.assertIn('FC85_CART("HELLO C         ");', text)
        self.assertIn('s_title[] = "HELLO C";', text)
        self.assertNotIn("TEMPLATE", text)
        sys.path.insert(0, HERE)
        import m68kgcc
        if m68kgcc.find_toolchain() is None:
            self.assertTrue(any("GCC" in n for n in notes))
            self.skipTest("68000 の GCC がない")
        r = subprocess.run([sys.executable, "build.py"], cwd=self.dest, capture_output=True,
                           text=True, encoding="utf-8")
        self.assertEqual(r.returncode, 0, r.stderr)
        for name in ("rom.bin", "rom.lst", "rom.elf", "rom.map", "rom.dis"):
            self.assertTrue(os.path.isfile(os.path.join(self.dest, "build", name)), name)
        with open(os.path.join(self.dest, "build", "rom.bin"), "rb") as f:
            self.assertEqual(f.read()[16:32], b"HELLO C".ljust(16))
        # 同じ試験（test_rom.py）が C の ROM でも通る（ラベルは rom.lst の記号の表から）
        r = subprocess.run([sys.executable, "-m", "unittest"], cwd=self.dest, capture_output=True,
                           text=True, encoding="utf-8")
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_the_default_title_comes_from_the_folder(self):
        self.assertEqual(new_rom.default_title(self.dest), "MY FIRST ROM")
        self.assertEqual(new_rom.default_title(os.path.join(self.tmp.name, "---")), "ROM")

    def test_refuses_bad_titles_and_full_folders(self):
        with self.assertRaises(ValueError):
            new_rom.create(self.dest, "TOO LONG FOR THE HEADER")
        with self.assertRaises(ValueError):
            new_rom.create(self.dest, 'QUOTE"')
        with self.assertRaises(ValueError):
            new_rom.create(self.dest, "OK", "basic")
        os.makedirs(self.dest)
        with open(os.path.join(self.dest, "x"), "w") as f:
            f.write("x")
        self.assertEqual(quiet(new_rom.main, [self.dest]), 1)


class TestEmulatorClient(unittest.TestCase):
    def test_find_root_and_the_api(self):
        self.assertEqual(os.path.normcase(fc85emu.find_root(HERE)), os.path.normcase(new_rom.ROOT))
        exe = fc85emu.find_api(new_rom.ROOT)
        if exe is None:
            self.skipTest("fc85-api がない")
        with fc85emu.Emulator(new_rom.ROOT) as emu:
            self.assertIn("api_version", emu.call("info"))
            with self.assertRaises(fc85emu.EmuError):
                emu.call("no_such_method")


if __name__ == "__main__":
    unittest.main()
