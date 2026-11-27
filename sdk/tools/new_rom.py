#!/usr/bin/env python3
"""ひな形（sdk/template/）から ROM のフォルダを作る。

    py sdk/tools/new_rom.py DEST [--title "MY ROM"] [--lang c|asm]

DEST は空か、まだないフォルダ。リポジトリの中でも外でもよい。作るもの：
- main.c（--lang c。既定）か main.s（--lang asm）、build.py、test_rom.py、AGENTS.md、CLAUDE.md、.gitignore
  （ひな形の写し。題を書き換える）。C は GCC が要る（sdk/docs/c.md）
- fc85-root：リポジトリの根への道（1 行。DEST からの相対。別のドライブなら絶対）。build.py と
  sdk/tools/fc85emu.py がこれで SDK とエミュレータを探す
- .mcp.json：エミュレータの窓口を MCP サーバーとして使う設定（sdk/docs/emulator-api.md §05）

題は見出しの 16 byte と画面の 1 行目に入る。英字・数字・空白で 16 字まで（文字 ROM の記号の字形は
まだそろっていない）。
"""

import argparse
import json
import os
import re
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
TEMPLATE = os.path.join(ROOT, "sdk", "template")
FILES = ["build.py", "test_rom.py", "AGENTS.md", "CLAUDE.md", ".gitignore"]
SOURCES = {"c": ("c/main.c", "main.c"), "asm": ("main.s", "main.s")}     # 言語 → (ひな形の中, 写す先)
TITLE_RE = re.compile(r"^[A-Za-z0-9 ]{1,16}$")
EXE = "fc85-api.exe" if os.name == "nt" else "fc85-api"


def rel_or_abs(target, start):
    """start から target への道（/ で区切る）。別のドライブで相対にできなければ絶対の道と False。"""
    try:
        return os.path.relpath(target, start).replace(os.sep, "/"), True
    except ValueError:
        return os.path.abspath(target).replace(os.sep, "/"), False


def default_title(dest):
    t = re.sub(r"[^A-Za-z0-9 ]", " ", os.path.basename(os.path.abspath(dest))).upper().strip()
    return re.sub(r" +", " ", t)[:16] or "ROM"


def retitle(text, title, lang="asm"):
    """main.s か main.c の題の 2 か所（見出しの 16 byte、画面の文字列）を書き換える。"""
    if lang == "c":
        head, line = 'FC85_CART("TEMPLATE        ")', 's_title[] = "TEMPLATE"'
        new_head, new_line = f'FC85_CART("{title.ljust(16)}")', f's_title[] = "{title}"'
    else:
        head, line = 'DC.B    "TEMPLATE        "', 'DC.B    "TEMPLATE",0'
        new_head, new_line = f'DC.B    "{title.ljust(16)}"', f'DC.B    "{title}",0'
    if text.count(head) != 1 or text.count(line) != 1:
        raise ValueError("ひな形の本体に題の行が見つからない")
    return text.replace(head, new_head).replace(line, new_line)


def create(dest, title, lang="c"):
    """DEST にひな形を写す。注意（相対にできなかった道、GCC がない）の列を返す。"""
    if not TITLE_RE.match(title):
        raise ValueError(f"題は英字・数字・空白で 1〜16 字（{title!r}）")
    if lang not in SOURCES:
        raise ValueError(f"言語は c か asm（{lang!r}）")
    dest = os.path.abspath(dest)
    if os.path.exists(dest) and os.listdir(dest):
        raise ValueError(f"{dest} は空でない")
    os.makedirs(dest, exist_ok=True)
    for name in FILES:
        shutil.copyfile(os.path.join(TEMPLATE, name), os.path.join(dest, name))
    src, name = SOURCES[lang]
    with open(os.path.join(TEMPLATE, src), encoding="utf-8") as f:
        text = retitle(f.read(), title, lang)
    with open(os.path.join(dest, name), "w", encoding="utf-8", newline="\n") as f:
        f.write(text)

    notes = []
    if lang == "c":
        sys.path.insert(0, HERE)
        from m68kgcc import find_toolchain
        if find_toolchain() is None:
            notes.append("68000 の GCC が見つからない。組む前に入れる（sdk/docs/c.md §02）か、--lang asm で作り直す")
    root, ok = rel_or_abs(ROOT, dest)
    if not ok:
        notes.append("fc85-root と .mcp.json に絶対の道を書いた（別のドライブ）。公開するときは消すか書き換える")
    with open(os.path.join(dest, "fc85-root"), "w", encoding="utf-8", newline="\n") as f:
        f.write(root + "\n")
    exe, _ = rel_or_abs(os.path.join(ROOT, "emulator", "target", "release", EXE), dest)
    if ok and not exe.startswith("."):
        exe = "./" + exe
    mcp = {"mcpServers": {"fc85": {"command": exe, "args": ["--mcp", "--root", root]}}}
    with open(os.path.join(dest, ".mcp.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(mcp, f, ensure_ascii=False, indent=2)
        f.write("\n")
    return notes


def main(argv=None):
    ap = argparse.ArgumentParser(description="ひな形から ROM のフォルダを作る")
    ap.add_argument("dest", help="作るフォルダ（空か、まだないもの）")
    ap.add_argument("--title", help="題（英字・数字・空白で 16 字まで。既定はフォルダの名前）")
    ap.add_argument("--lang", choices=sorted(SOURCES), default="c", help="C（既定。GCC が要る）かアセンブラ")
    args = ap.parse_args(argv)
    title = args.title if args.title is not None else default_title(args.dest)
    try:
        notes = create(args.dest, title, args.lang)
    except (ValueError, OSError) as e:
        print(f"エラー: {e}", file=sys.stderr)
        return 1
    for n in notes:
        print(f"注意: {n}", file=sys.stderr)
    print(f"{args.dest} を作った（題 {title!r}、{'C' if args.lang == 'c' else 'アセンブラ'}）。次に：")
    print(f"  cd {args.dest}")
    print("  py build.py          # build/rom.bin")
    print("  py -m unittest       # 試験（エミュレータの窓口と Boot ROM があれば動かす）")
    print("  手引きは AGENTS.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
