#!/usr/bin/env python3
"""WebAssembly の試しの頁（emulator/embed/index.html）を手元で配る。

  py emulator/embed/build.py     # 先に build/fc85emu.wasm と Boot ROM・文字 ROM を置く
  py emulator/embed/serve.py     # http://127.0.0.1:8000/ を開く（--port で替える）

`py -m http.server` でもよいが、Windows では .js を text/plain で返すことがあり（登録簿の MIME の設定）、
ブラウザがモジュールを読まない。ここでは .js・.mjs・.wasm の MIME を決めて返す。自分の機械（127.0.0.1）
だけに開く。Python 3 の標準ライブラリだけで書く。
"""

import argparse
import functools
import http.server
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))

TYPES = {
    ".js": "text/javascript",
    ".mjs": "text/javascript",
    ".wasm": "application/wasm",
    ".html": "text/html; charset=utf-8",
    ".bin": "application/octet-stream",
}


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, **TYPES}

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--port", type=int, default=8000)
    a = ap.parse_args(argv)
    if not os.path.exists(os.path.join(HERE, "build", "fc85emu.wasm")):
        print("build/fc85emu.wasm がない（py emulator/embed/build.py で組む）", file=sys.stderr)
    handler = functools.partial(Handler, directory=HERE)
    with http.server.ThreadingHTTPServer(("127.0.0.1", a.port), handler) as s:
        print(f"http://127.0.0.1:{a.port}/ （Ctrl+C で止める）", flush=True)
        s.serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main())
