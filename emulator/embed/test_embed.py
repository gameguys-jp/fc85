"""組込みの窓口（DR-104）の試験。

- C の ABI の共有ライブラリを ctypes で呼び（C から呼ぶのと同じ）、WebAssembly を Node で動かした結果
  （check.mjs）と、画面・音・状態がビットまで同じかを突き合わせる。WebAssembly は build.py と同じく、
  wasm-opt があれば最適化したもの（DR-108）。
- fc85emu.h が C99 の見出しとして通るか（GCC があれば。sdk/tools/m68kgcc.py で探す）。
- libretro のコアの共有ライブラリが libretro の関数をすべて出しているか。

cargo、Boot ROM・文字 ROM（firmware の組立て）、Node、wasm32 の target がなければ、その試験を飛ばす。
"""

import ctypes
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import build  # noqa: E402

sys.path.insert(0, os.path.join(build.REPO, "sdk", "tools"))
import m68kgcc  # noqa: E402

BOOT = build.FIRMWARE["boot.bin"]
CHARS = build.FIRMWARE["char.bin"]
CART = os.path.join(build.REPO, "sdk", "samples", "game", "build", "game.bin")
HEADER_DIR = os.path.join(build.EMULATOR, "crates", "capi", "include")
RIGHT = 0x08
VIEW, RASTER = 1, 2

LIBRETRO_API = """retro_set_environment retro_set_video_refresh retro_set_audio_sample
retro_set_audio_sample_batch retro_set_input_poll retro_set_input_state retro_init retro_deinit
retro_api_version retro_get_system_info retro_get_system_av_info retro_set_controller_port_device
retro_reset retro_run retro_serialize_size retro_serialize retro_unserialize retro_cheat_reset
retro_cheat_set retro_load_game retro_load_game_special retro_unload_game retro_get_region
retro_get_memory_data retro_get_memory_size""".split()


def fnv(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{h:016x}"


def read(p):
    with open(p, "rb") as f:
        return f.read()


def load_copy(path):
    """共有ライブラリを一時のフォルダに写して読む（Windows で元のファイルを掴んだままにしない）。"""
    d = tempfile.mkdtemp(prefix="fc85-embed-")
    p = os.path.join(d, os.path.basename(path))
    shutil.copyfile(path, p)
    return ctypes.CDLL(p)


class Native:
    """C の ABI（fc85emu.h）を ctypes で。"""

    def __init__(self, lib):
        c = ctypes
        self.lib = lib
        sig = {
            "fc85_new": ([c.c_char_p, c.c_size_t, c.c_char_p, c.c_size_t, c.c_uint32], c.c_void_p),
            "fc85_free": ([c.c_void_p], None),
            "fc85_load_cart": ([c.c_void_p, c.c_char_p, c.c_size_t], c.c_int32),
            "fc85_set_input": ([c.c_void_p, c.c_uint16, c.c_uint16], c.c_int32),
            "fc85_run_frames": ([c.c_void_p, c.c_uint32], c.c_int32),
            "fc85_frame": ([c.c_void_p], c.c_uint64),
            "fc85_screen": ([c.c_void_p, c.c_int32, c.POINTER(c.c_uint32), c.c_size_t], c.c_int32),
            "fc85_screen_rgba": ([c.c_void_p, c.c_int32, c.c_char_p, c.c_size_t], c.c_int32),
            "fc85_screen_width": ([c.c_int32], c.c_uint32),
            "fc85_screen_height": ([c.c_int32], c.c_uint32),
            "fc85_audio": ([c.c_void_p, c.POINTER(c.c_int16), c.c_size_t], c.c_int32),
            "fc85_save_state": ([c.c_void_p, c.POINTER(c.c_size_t)], c.c_void_p),
            "fc85_load_state": ([c.c_void_p, c.c_char_p, c.c_size_t], c.c_int32),
            "fc85_free_bytes": ([c.c_void_p, c.c_size_t], None),
            "fc85_peek": ([c.c_void_p, c.c_int32, c.c_uint32, c.c_char_p, c.c_size_t], c.c_int32),
            "fc85_poke": ([c.c_void_p, c.c_int32, c.c_uint32, c.c_char_p, c.c_size_t], c.c_int32),
            "fc85_call": ([c.c_void_p, c.c_char_p], c.c_void_p),
            "fc85_free_string": ([c.c_void_p], None),
            "fc85_last_error": ([], c.c_char_p),
            "fc85_frame_rate": ([], c.c_double),
            "fc85_abi_version": ([], c.c_uint32),
        }
        for name, (args, res) in sig.items():
            f = getattr(lib, name)
            f.argtypes = args
            f.restype = res
        self.m = None

    def check(self, r, what):
        if r is None or (isinstance(r, int) and r < 0):
            raise RuntimeError(f"{what}: {self.lib.fc85_last_error().decode('utf-8', 'replace')}")
        return r

    def create(self, boot, chars):
        self.m = self.check(self.lib.fc85_new(boot, len(boot), chars, len(chars), 0), "fc85_new")

    def screen(self, area):
        n = self.lib.fc85_screen_width(area) * self.lib.fc85_screen_height(area)
        buf = (ctypes.c_uint32 * n)()
        self.check(self.lib.fc85_screen(self.m, area, buf, n), "fc85_screen")
        return bytes(buf)

    def screen_rgba(self, area, size=None):
        """fc85_screen_rgba の戻り値と、書かれた byte 列（size は渡す領域の大きさ。既定は画面の分）。"""
        n = self.lib.fc85_screen_width(area) * self.lib.fc85_screen_height(area)
        size = 4 * n if size is None else size
        buf = ctypes.create_string_buffer(size)
        return self.lib.fc85_screen_rgba(self.m, area, buf, size), buf.raw

    def audio(self):
        out = b""
        chunk = 4096
        buf = (ctypes.c_int16 * (2 * chunk))()
        while True:
            n = self.check(self.lib.fc85_audio(self.m, buf, chunk), "fc85_audio")
            out += bytes(buf)[: 4 * n]
            if n < chunk:
                return out

    def save_state(self):
        n = ctypes.c_size_t()
        p = self.check(self.lib.fc85_save_state(self.m, ctypes.byref(n)), "fc85_save_state")
        data = ctypes.string_at(p, n.value)
        self.lib.fc85_free_bytes(p, n.value)
        return data

    def call(self, method):
        req = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method}).encode()
        p = self.check(self.lib.fc85_call(self.m, req), "fc85_call")
        s = ctypes.string_at(p).decode("utf-8")
        self.lib.fc85_free_string(p)
        return json.loads(s)


def have_roms():
    return os.path.exists(BOOT) and os.path.exists(CHARS)


@unittest.skipIf(shutil.which("cargo") is None, "cargo がない")
class EmbedTest(unittest.TestCase):
    def native_run(self):
        """check.mjs と同じ手順を C の ABI で。"""
        n = Native(load_copy(build.build_native()))
        self.assertEqual(n.lib.fc85_abi_version(), 1)
        n.create(read(BOOT), read(CHARS))
        if os.path.exists(CART):
            cart = read(CART)
            n.check(n.lib.fc85_load_cart(n.m, cart, len(cart)), "fc85_load_cart")
        n.check(n.lib.fc85_run_frames(n.m, 120), "fc85_run_frames")
        out = {
            "frame": n.lib.fc85_frame(n.m),
            "screen": fnv(n.screen(VIEW)),
            "audio": fnv(n.audio()),
        }
        # RGBA は 0x00RRGGBB（リトルエンディアンの u32 の byte は B・G・R・0）を並べ替えたもの
        xrgb = n.screen(VIEW)
        want = bytearray()
        for i in range(0, len(xrgb), 4):
            want += bytes([xrgb[i + 2], xrgb[i + 1], xrgb[i], 255])
        self.assertEqual(n.screen_rgba(VIEW), (320 * 240, bytes(want)))
        r, _ = n.screen_rgba(VIEW, len(want) - 1)
        self.assertEqual(r, -1, "fc85_screen_rgba が足りない領域を誤りにしない")
        st = n.save_state()
        out["state_len"] = len(st)

        def go():
            n.check(n.lib.fc85_set_input(n.m, RIGHT, 0), "fc85_set_input")
            n.check(n.lib.fc85_run_frames(n.m, 30), "fc85_run_frames")
            return f"{fnv(n.screen(RASTER))} {fnv(n.audio())} {n.lib.fc85_frame(n.m)}"

        a = go()
        n.check(n.lib.fc85_load_state(n.m, st, len(st)), "fc85_load_state")
        out["round_trip"] = a == go()
        out["after"] = a
        n.check(n.lib.fc85_poke(n.m, 0, 0x100800, b"\x12\x34", 2), "fc85_poke")
        buf = ctypes.create_string_buffer(2)
        n.check(n.lib.fc85_peek(n.m, 0, 0x100800, buf, 2), "fc85_peek")
        out["peek"] = list(buf.raw)
        out["pc"] = n.call("regs")["result"]["pc"]
        self.assertEqual(n.lib.fc85_load_state(n.m, b"\x01\x02\x03", 3), -1)
        self.assertIn("印", n.lib.fc85_last_error().decode("utf-8"))
        n.lib.fc85_free(n.m)
        return out

    @unittest.skipUnless(have_roms(), "Boot ROM か文字 ROM がない（firmware を組む）")
    def test_native_and_webassembly_agree(self):
        native = self.native_run()
        self.assertTrue(native["round_trip"], "状態を戻して同じ入力で進めた結果が違う（C の ABI）")
        self.assertEqual(native["frame"], 120)
        self.assertEqual(native["peek"], [0x12, 0x34])
        if shutil.which("node") is None:
            self.skipTest("Node がない（WebAssembly は確かめない）")
        if not build.has_wasm_target():
            self.skipTest("wasm32-unknown-unknown がない（WebAssembly は確かめない）")
        wasm = build.build_wasm(copy_firmware=False)
        args = ["node", os.path.join(HERE, "check.mjs"), wasm, BOOT, CHARS]
        if os.path.exists(CART):
            args.append(CART)
        r = subprocess.run(args, capture_output=True)
        self.assertEqual(r.returncode, 0, r.stderr.decode("utf-8", "replace"))
        web = json.loads(r.stdout.decode("utf-8"))
        self.assertTrue(web["round_trip"], "状態を戻して同じ入力で進めた結果が違う（WebAssembly）")
        self.assertEqual(web["bios_version"], [0, 2])
        self.assertIn("印", web["bad_state"])
        self.assertEqual(web["rgba_len"], 256 * 192 * 4)
        self.assertTrue(web["rgba_ok"], "fc85_screen_rgba が fc85_screen の並べ替えになっていない（WebAssembly）")
        for k in ("frame", "screen", "audio", "state_len", "after", "peek", "pc"):
            self.assertEqual(web[k], native[k], f"{k} が C の ABI と WebAssembly で違う")

    def test_a_missing_wasm_opt_is_reported(self):
        missing = os.path.join(tempfile.mkdtemp(prefix="fc85-wasm-opt-"), "wasm-opt.exe")
        with self.assertRaises(build.BuildError):
            build.find_wasm_opt(missing)

    def test_header_compiles_as_c99(self):
        tc = m68kgcc.find_toolchain()
        if tc is None:
            self.skipTest("GCC がない（sdk/docs/c.md §02）")
        d = tempfile.mkdtemp(prefix="fc85-header-")
        src = os.path.join(d, "use.c")
        with open(src, "w", encoding="utf-8", newline="\n") as f:
            f.write(USE_C)
        argv = [tc.tool("gcc"), "-std=c99", "-pedantic", "-Wall", "-Wextra", "-Werror", "-fsyntax-only",
                "-ffreestanding", "-nostdinc", "-isystem", m68kgcc.GCC_INCLUDE,
                "-I", tc.path(HEADER_DIR), tc.path(src)]
        try:
            tc.run([(argv, None)])
        except m68kgcc.ToolchainError as e:
            self.fail(f"fc85emu.h が C99 で通らない：{e}")
        finally:
            shutil.rmtree(d, ignore_errors=True)

    def test_libretro_core_exports_the_api(self):
        lib = load_copy(build.build_libretro())
        missing = [n for n in LIBRETRO_API if not hasattr(lib, n)]
        self.assertEqual(missing, [])
        lib.retro_api_version.restype = ctypes.c_uint
        self.assertEqual(lib.retro_api_version(), 1)

        class Info(ctypes.Structure):
            _fields_ = [("library_name", ctypes.c_char_p), ("library_version", ctypes.c_char_p),
                        ("valid_extensions", ctypes.c_char_p), ("need_fullpath", ctypes.c_bool),
                        ("block_extract", ctypes.c_bool)]

        info = Info()
        lib.retro_get_system_info(ctypes.byref(info))
        self.assertEqual(info.library_name, b"fc85")
        self.assertEqual(info.valid_extensions, b"bin")
        self.assertFalse(info.need_fullpath)
        lib.retro_get_region.restype = ctypes.c_uint
        self.assertEqual(lib.retro_get_region(), 0)


# 見出しのすべての関数を、型の合う引数で呼ぶ（呼ぶだけで、動かさない）
USE_C = r"""
#include "fc85emu.h"

int use(void);

int use(void)
{
    fc85_t *m = fc85_new((const uint8_t *)0, 0, (const uint8_t *)0, 0, FC85_NO_DOC | FC85_DIGITAL_AUDIO);
    uint32_t px[1];
    int16_t pcm[2];
    uint8_t b[2];
    size_t n = 0;
    uint8_t *st;
    char *s;
    int32_t r = 0;
    double fps = fc85_frame_rate();
    (void)fps;
    r += (int32_t)fc85_abi_version() + (int32_t)fc85_audio_rate();
    r += (int32_t)fc85_screen_width(FC85_AREA_VIEW) + (int32_t)fc85_screen_height(FC85_AREA_RASTER);
    (void)fc85_version();
    (void)fc85_last_error();
    r += fc85_load_cart(m, b, sizeof b);
    r += fc85_reset(m);
    r += fc85_set_input(m, FC85_PAD_UP | FC85_PAD_START, 0);
    r += fc85_run_frames(m, 1);
    r += (int32_t)fc85_frame(m);
    r += fc85_screen(m, FC85_AREA_ACTIVE, px, 1);
    r += fc85_screen_rgba(m, FC85_AREA_VIEW, b, sizeof b);
    r += fc85_audio(m, pcm, 1);
    st = fc85_save_state(m, &n);
    r += fc85_load_state(m, st, n);
    fc85_free_bytes(st, n);
    r += fc85_peek(m, FC85_SPACE_CPU, 0x480u, b, 2);
    r += fc85_poke(m, FC85_SPACE_VRAM, 0, b, 2);
    s = fc85_call(m, "{\"method\":\"info\"}");
    fc85_free_string(s);
    st = fc85_alloc(16);
    fc85_dealloc(st, 16);
    fc85_free(m);
    return r + FC85_SPACE_PALETTE + FC85_SPACE_CHAR_ROM + FC85_SPACE_TEMPORAL + FC85_SPACE_DSP_PROG
        + FC85_SPACE_WAVE + FC85_PAD_DOWN + FC85_PAD_LEFT + FC85_PAD_RIGHT + FC85_PAD_A + FC85_PAD_B
        + FC85_PAD_C + FC85_ABI_VERSION;
}
"""


if __name__ == "__main__":
    unittest.main()
