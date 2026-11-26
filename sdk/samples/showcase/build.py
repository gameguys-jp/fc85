#!/usr/bin/env python3
"""通常モードの映像と描画の予算を見せるカートリッジ SAMPLE 6 を組む。

    py sdk/samples/showcase/build.py     → sdk/samples/showcase/build/showcase.bin（$400000 から）と showcase.lst

1. 表を作る（値はすべて [P]、README.md §06）。
   - ラスタのパレットの表：基本画面の 160 行に、パレット 1（空と海）と 2（立体と波頭）の色を 1 行ずつ。
   - コッパーバー 2 本の色（15 行ずつ）と、縦の位置の表（256 項目）。
   - 正弦の表（Q15、256 項目）と透視の表（Q15、256 項目）。DSP と 68000 の両方に同じ値を置く。
   - 立方体の頂点（8 個）と、辺を折れ線（APLL）にまとめた道。
   - 海の模様（64 行×64 語、4 bpp）→ build/sea.bin。玉のスプライト（16×16、4 通りにずらした型と絵）→
     build/bank.bin。
   - 玉の初めの位置と速さ（MAX_OBJ 個）。漢字の行の文字 ROM の offset（仕様書 §08 の式）。
   build/consts.inc（EQU。main.s の頭で INCLUDE）と build/tables.inc（表。main.s の後ろで INCLUDE）。
2. xform.dsp を sdk/dspasm/dspasm.py で組む → build/xform.bin と xform.lst。表は build/dsptab.inc。
3. main.s を sdk/m68kasm/m68kasm.py で組む。INCLUDE と INCBIN は、このフォルダ、build/、sdk/include から探す。
"""

import argparse
import colorsys
import math
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "m68kasm"))
sys.path.insert(0, os.path.join(ROOT, "sdk", "dspasm"))

import dspasm  # noqa: E402
import m68kasm  # noqa: E402

CART_BASE = 0x400000
CART_MAX = 0x100000

# ---------------------------------------------------------------------------
# 画面の配置（基本画面の行。0 が基本画面の頭＝画面の y 16）

SKY_ROWS = 96           # 空（立方体とコッパーバー）
SEA_ROWS = 64           # 海（玉）。最初の行は水平線
BASE_ROWS = SKY_ROWS + SEA_ROWS
BORDER_WORDS = 1        # 左右の 4 画素（色 0。処理の時間の帯が見える縁）

# 68000 と DSP の約束（protocol.inc と同じ値。test_build.py が照らす）
HEAD_MAGIC = 0x53
TAIL_MAGIC = 0xAC
CX = 128
CY = 48

# ---------------------------------------------------------------------------
# ラスタのパレット（12 bit、$0RGB）


def rgb(r, g, b):
    """0〜15 の 3 つの値（範囲の外は切る）を $0RGB にする。"""
    c = [max(0, min(15, round(v))) for v in (r, g, b)]
    return (c[0] << 8) | (c[1] << 4) | c[2]


def ramp(stops, n):
    """(行, (r, g, b)) の折れ線を n 行に広げる。色の成分ごとに丸めるので、段の位置は成分ごとにずれる。"""
    out = []
    for i in range(n):
        for (y0, c0), (y1, c1) in zip(stops, stops[1:]):
            if y0 <= i <= y1:
                t = (i - y0) / (y1 - y0) if y1 > y0 else 0.0
                out.append(rgb(*(a + (b - a) * t for a, b in zip(c0, c1))))
                break
    assert len(out) == n
    return out


SKY_STOPS = [(0, (0, 1, 4)), (56, (3, 7, 12)), (SKY_ROWS - 1, (11, 14, 15))]
SEA_STOPS = [(0, (4, 10, 12)), (SEA_ROWS - 2, (0, 1, 5))]
HORIZON = (rgb(14, 15, 15), rgb(15, 15, 15))


def cube_color(row):
    """立方体（パレット 2）の色：上から下へ色相を回す。"""
    h = (0.08 + 0.85 * row / (SKY_ROWS - 1)) % 1.0
    r, g, b = colorsys.hsv_to_rgb(h, 0.55, 1.0)
    return rgb(r * 15, g * 15, b * 15)


def crest_color(c1):
    """波頭（海のパレット 2）：海の色を明るくする。"""
    r, g, b = (c1 >> 8) & 15, (c1 >> 4) & 15, c1 & 15
    return rgb(r + 5, g + 4, b + 3)


def make_gradient():
    """基本画面の 160 行の (パレット 1, パレット 2)。"""
    sky = ramp(SKY_STOPS, SKY_ROWS)
    sea = ramp(SEA_STOPS, SEA_ROWS - 1)
    rows = [(sky[i], cube_color(i)) for i in range(SKY_ROWS)]
    rows.append(HORIZON)
    rows += [(c, crest_color(c)) for c in sea]
    assert len(rows) == BASE_ROWS
    return rows


# コッパーバー：2 本、15 行。縦の位置は正弦で、位相を 128 ずらす（反対の側を回る）
BAR_H = 15
BAR_SPEED = 2           # 1 フレームに進める位相（256 で 1 回り）
BAR_CENTER = 40
BAR_AMP = 39


def bar_colors(kind):
    out = []
    for k in range(BAR_H):
        i = 1.0 - abs(k - (BAR_H - 1) / 2) / ((BAR_H + 1) / 2)
        if kind == 0:       # 赤から黄
            c = (15 * i, 15 * i ** 2, 3 * i ** 3)
        else:               # 紫から桃
            c = (13 * i, 3 * i ** 2, 13 * i ** 1.5)
        if k == BAR_H // 2:
            c = tuple(v + 3 for v in c)
        out.append(rgb(*c))
    return out


def bar_positions():
    out = [round(BAR_CENTER + BAR_AMP * math.sin(2 * math.pi * k / 256)) for k in range(256)]
    assert min(out) >= 0 and max(out) + BAR_H <= SKY_ROWS
    return out


def count_colors():
    """1 フレームに出うる色の数の目安（ラスタの表、バーの色、固定の色）。"""
    s = set()
    for c1, c2 in make_gradient():
        s.add(c1)
        s.add(c2)
    s.update(bar_colors(0))
    s.update(bar_colors(1))
    s.update(PALETTE)
    return len(s)


# ---------------------------------------------------------------------------
# 回転と透視（protocol.inc の式。DSP と 68000 の両方がこの通りに計算する）

CUBE_HALF = 64
PERSP_D = 330           # 視点までの距離（立方体の中心から）
PERSP_F = 128           # 焦点の長さ。k＝F÷(D＋z)


def sin_table():
    return [round(32767 * math.sin(2 * math.pi * k / 256)) for k in range(256)]


def persp_table():
    out = []
    for i in range(256):
        v = round(32768 * PERSP_F / (PERSP_D + i - 128))
        out.append(max(0, min(32767, v)))
    return out


SIN = sin_table()
PERSP = persp_table()


def shr15(p):
    """積の和 p を 15 bit 右へ（負の向きに切り捨て）。DSP の SACH ,1 と 68000 の ASL.L #1・SWAP と同じ。"""
    return (p * 2) >> 16


def transform(verts, a, b, c):
    """頂点の列を回して透視し、ACRTC の論理座標 (X, Y) の列を返す。"""
    sa, ca = SIN[a & 255], SIN[(a + 64) & 255]
    sb, cb = SIN[b & 255], SIN[(b + 64) & 255]
    sc, cc = SIN[c & 255], SIN[(c + 64) & 255]
    out = []
    for x, y, z in verts:
        y1 = shr15(y * ca - z * sa)
        z1 = shr15(y * sa + z * ca)
        x2 = shr15(x * cb + z1 * sb)
        z2 = shr15(z1 * cb - x * sb)
        x3 = shr15(x2 * cc - y1 * sc)
        y3 = shr15(x2 * sc + y1 * cc)
        k = PERSP[(z2 + 128) & 255]
        out.append((shr15(x3 * k) + CX, shr15(y3 * k) - CY))
    return out


CUBE_VERTS = [
    (-CUBE_HALF, -CUBE_HALF, -CUBE_HALF), (CUBE_HALF, -CUBE_HALF, -CUBE_HALF),
    (CUBE_HALF, CUBE_HALF, -CUBE_HALF), (-CUBE_HALF, CUBE_HALF, -CUBE_HALF),
    (-CUBE_HALF, -CUBE_HALF, CUBE_HALF), (CUBE_HALF, -CUBE_HALF, CUBE_HALF),
    (CUBE_HALF, CUBE_HALF, CUBE_HALF), (-CUBE_HALF, CUBE_HALF, CUBE_HALF),
]
CUBE_EDGES = [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4),
              (0, 4), (1, 5), (2, 6), (3, 7)]


def edge_paths(edges):
    """辺をすべて 1 回ずつ通る道（折れ線）の列にまとめる。奇数の次数の頂点から歩き始める（貪欲）。"""
    left = list(edges)
    paths = []
    while left:
        deg = {}
        for a, b in left:
            deg[a] = deg.get(a, 0) + 1
            deg[b] = deg.get(b, 0) + 1
        odd = [v for v in sorted(deg) if deg[v] % 2]
        v = odd[0] if odd else left[0][0]
        path = [v]
        while True:
            nxt = None
            for i, (a, b) in enumerate(left):
                if a == v or b == v:
                    nxt = i
                    break
            if nxt is None:
                break
            a, b = left.pop(nxt)
            v = b if a == v else a
            path.append(v)
        paths.append(path)
    return paths


def max_extent():
    """回転と透視の結果が、中心からどこまで出るか（|X−CX|、|Y|。立方体の外接球で調べる）。"""
    r = CUBE_HALF * math.sqrt(3)
    worst = 0.0
    for i in range(-1000, 1001):
        z = r * i / 1000
        lat = math.sqrt(max(0.0, r * r - z * z))
        worst = max(worst, lat * PERSP_F / (PERSP_D + z))
    return worst


# ---------------------------------------------------------------------------
# 海の模様（64 行×256 画素、4 bpp）。色 1 は地、色 2 は波頭（どちらもラスタで色が変わる）。左右の 4 画素は色 0


def sea_rows():
    px = [[1] * 256 for _ in range(SEA_ROWS)]
    band = 0
    y = 3.0
    while y < SEA_ROWS - 1:
        t = y / (SEA_ROWS - 1)
        period = 14 + 34 * t
        length = 0.5 * period
        h = 1 + 2.2 * t
        off = (band * 0.47 * period + band * 7) % period
        yb = int(round(y))
        for x in range(256):
            ph = (x + off) % period
            if ph < length:
                u = ph / length
                yy = yb - int(round(h * math.sin(math.pi * u)))
                if 1 <= yy < SEA_ROWS:
                    px[yy][x] = 2
        band += 1
        y += 3.2 + 6.5 * t
    for row in px:
        for x in range(4 * BORDER_WORDS):
            row[x] = 0
            row[255 - x] = 0
    return px


def pack_row(pixels):
    """画素の列（4 の倍数）を ACRTC の語（左の画素が下位ニブル）の big-endian の byte にする。"""
    out = bytearray()
    for w in range(len(pixels) // 4):
        v = 0
        for i in range(4):
            v |= (pixels[4 * w + i] & 15) << (4 * i)
        out += struct.pack(">H", v)
    return bytes(out)


def make_sea():
    return b"".join(pack_row(r) for r in sea_rows())


# ---------------------------------------------------------------------------
# 玉のスプライト（16×16）。型（マスク）は透明が F、不透明が 0。絵は不透明が色、透明が 0。
# 横の 4 通りのずれ（0〜3 画素）を前もって作り、5 語（20 画素）の幅に置く。

SPR = 16
SPR_WORDS = 5
OUTLINE = 10
HIGHLIGHT = 15
BALL_SHADES = [(3, 4, 5), (6, 7, 8)]    # 玉 A（赤）、玉 B（黄）の暗・中・明


def ball_level(x, y):
    """None は透明、'o' は縁、0〜3 は暗・中・明・光。"""
    dx, dy = x - 7.5, y - 7.5
    d = math.hypot(dx, dy)
    if d > 7.7:
        return None
    if d > 6.7:
        return "o"
    nx, ny = dx / 7.2, dy / 7.2
    nz = math.sqrt(max(0.0, 1 - nx * nx - ny * ny))
    lum = -0.45 * nx - 0.55 * ny + 0.70 * nz
    if lum > 0.93:
        return 3
    if lum > 0.62:
        return 2
    if lum > 0.22:
        return 1
    return 0


def ball_pixels(variant):
    shades = BALL_SHADES[variant]
    rows = []
    for y in range(SPR):
        row = []
        for x in range(SPR):
            lv = ball_level(x, y)
            if lv is None:
                row.append(None)
            elif lv == "o":
                row.append(OUTLINE)
            elif lv == 3:
                row.append(HIGHLIGHT)
            else:
                row.append(shades[lv])
        rows.append(row)
    return rows


def shifted(rows, s, mask):
    out = []
    for row in rows:
        line = [None] * (4 * SPR_WORDS)
        for x, p in enumerate(row):
            line[s + x] = p
        if mask:
            out.append([15 if p is None else 0 for p in line])
        else:
            out.append([0 if p is None else p for p in line])
    return out


BANK_MASK = 0           # 語の位置：型 s は 5s、玉 A の絵は 20＋5s、玉 B の絵は 40＋5s
BANK_IMAGE = 20


def make_bank():
    """16 行×64 語。"""
    shape = ball_pixels(0)
    blocks = [shifted(shape, s, True) for s in range(4)]
    for v in range(len(BALL_SHADES)):
        img = ball_pixels(v)
        blocks += [shifted(img, s, False) for s in range(4)]
    out = bytearray()
    for y in range(SPR):
        line = []
        for b in blocks:
            line += b[y]
        line += [0] * (256 - len(line))
        out += pack_row(line)
    return bytes(out)


# 玉の動ける範囲（基本画面の座標。左上の画素）
MAX_OBJ = 96
SPR_XMIN = 4 * BORDER_WORDS
SPR_XMAX = 256 - 4 * BORDER_WORDS - SPR
SPR_YMIN = SKY_ROWS + 1
SPR_YMAX = BASE_ROWS - SPR


def initial_objects():
    """(x, y, dx, dy, 種類) を MAX_OBJ 個。16 bit の LFSR で決める。"""
    lfsr = 0x1985
    out = []

    def rnd(n):
        nonlocal lfsr
        v = 0
        for _ in range(16):
            bit = (lfsr ^ (lfsr >> 2) ^ (lfsr >> 3) ^ (lfsr >> 5)) & 1
            lfsr = (lfsr >> 1) | (bit << 15)
            v = (v << 1) | (lfsr & 1)
        return v % n

    for i in range(MAX_OBJ):
        x = SPR_XMIN + rnd(SPR_XMAX - SPR_XMIN + 1)
        y = SPR_YMIN + rnd(SPR_YMAX - SPR_YMIN + 1)
        dx = (1 + rnd(2)) * (1 if rnd(2) else -1)
        dy = 1 if rnd(2) else -1
        out.append((x, y, dx, dy, i % len(BALL_SHADES)))
    return out


# ---------------------------------------------------------------------------
# パレットと、処理の時間の帯の色

# 0 黒（時間の帯）、1・2 ラスタ、3〜5 玉 A、6〜8 玉 B、9 予備、10 玉の縁、11 灰、12 黄、13 緑、14 赤、15 白
PALETTE = [0x000, 0x26B, 0xFFF, 0x711, 0xD22, 0xF86, 0x740, 0xDB0, 0xFE6,
           0x0C0, 0x102, 0x888, 0xFE4, 0x4D4, 0xF44, 0xFFF]
# 段（main.s の PH_*）の順：K、R、E、X、C、S、T、終わり（待ち）
PHASE_NAMES = ["KICK", "RESTORE", "ERASE", "XFORM", "CUBE", "SPRITE", "TEXT", "IDLE"]
PHASE_COLORS = [0x0CC, 0x00F, 0xA0A, 0xFF0, 0x0C0, 0xF00, 0xF80, 0x000]

# ---------------------------------------------------------------------------
# 文字 ROM の漢字の行（仕様書 §08 の配置、firmware/charrom/build.py と同じ式）

KANJI_TEXT = "日本の文字を出す"
GLYPH_TSV = os.path.join(ROOT, "docs", "design", "glyphs-v0.1.tsv")


def kuten_offset(ku, ten):
    if 1 <= ku <= 8:
        return 0x0800 + ((ku - 1) * 94 + (ten - 1)) * 8
    if 16 <= ku <= 47:
        return 0x1F80 + ((ku - 16) * 94 + (ten - 1)) * 8
    raise ValueError(f"区 {ku}")


def read_kuten():
    """字 → (区, 点)。字形の表（docs/design/glyphs-v0.1.tsv）にある字だけ。"""
    table = {}
    with open(GLYPH_TSV, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#") or line.startswith("kuten"):
                continue
            cols = line.rstrip("\n").split("\t")
            if len(cols) >= 4 and cols[0]:
                ku, ten = (int(v) for v in cols[0].split("-"))
                table[cols[3]] = (ku, ten)
    return table


def kanji_offsets():
    table = read_kuten()
    return [kuten_offset(*table[ch]) for ch in KANJI_TEXT]


def nibble_masks():
    """字形の 4 bit（bit 3 が左）→ 4 画素の語のマスク（左の画素が下位ニブル）。"""
    out = []
    for v in range(16):
        w = 0
        for i in range(4):
            if v & (8 >> i):
                w |= 0xF << (4 * i)
        out.append(w)
    return out


# ---------------------------------------------------------------------------
# 出力


def dc_lines(directive, values, per_line, fmt):
    return [f"        {directive}    " + ",".join(fmt(v) for v in values[i:i + per_line])
            for i in range(0, len(values), per_line)]


def hexw(v):
    return f"${v & 0xFFFF:04X}"


def write(path, lines):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


def build_consts(outdir):
    paths = edge_paths(CUBE_EDGES)
    lines = [
        "; build.py が作った。手で直さない。main.s の頭で INCLUDE する。",
        f"SKY_ROWS        EQU     {SKY_ROWS}",
        f"SEA_ROWS        EQU     {SEA_ROWS}",
        f"BASE_ROWS       EQU     {BASE_ROWS}",
        f"BAR_H           EQU     {BAR_H}",
        f"BAR_SPEED       EQU     {BAR_SPEED}",
        f"CUBE_VERTS      EQU     {len(CUBE_VERTS)}",
        f"CUBE_PATHS      EQU     {len(paths)}",
        f"CUBE_SEGMENTS   EQU     {len(CUBE_EDGES)}",
        f"MAX_OBJ         EQU     {MAX_OBJ}",
        f"SPR_XMIN        EQU     {SPR_XMIN}",
        f"SPR_XMAX        EQU     {SPR_XMAX}",
        f"SPR_YMIN        EQU     {SPR_YMIN}",
        f"SPR_YMAX        EQU     {SPR_YMAX}",
        f"SPR_WORDS       EQU     {SPR_WORDS}",
        f"BANK_MASK       EQU     {BANK_MASK}",
        f"BANK_IMAGE      EQU     {BANK_IMAGE}",
        f"KANJI_COUNT     EQU     {len(KANJI_TEXT)}",
        f"PHASE_COUNT     EQU     {len(PHASE_NAMES) - 1}",
        f"FIXED_C1        EQU     ${PALETTE[1]:03X}          ; ラスタを止めたときのパレット 1・2",
        f"FIXED_C2        EQU     ${PALETTE[2]:03X}",
    ]
    write(os.path.join(outdir, "consts.inc"), lines)


def build_tables(outdir):
    grad = make_gradient()
    lines = [
        "; build.py が作った。手で直さない。main.s の表の節で INCLUDE する。",
        "",
        "; 基本画面の 160 行の色：パレット 1（空・水平線・海）、パレット 2（立方体・波頭）",
        "grad_table:",
    ]
    lines += dc_lines("DC.W", [v for row in grad for v in row], 8, hexw)
    lines += ["", "; コッパーバーの色（15 行ずつ）：A 赤から黄、B 紫", "bar_colors_a:"]
    lines += dc_lines("DC.W", bar_colors(0), 15, hexw)
    lines += ["bar_colors_b:"]
    lines += dc_lines("DC.W", bar_colors(1), 15, hexw)
    lines += ["", "; バーの縦の位置（空の行の番号、上の端）。256 で 1 回り", "bar_pos:"]
    lines += dc_lines("DC.W", bar_positions(), 16, str)
    lines += ["", "; 正弦（Q15）。sin[k]＝round(32767×sin(2πk/256))", "sin_table:"]
    lines += dc_lines("DC.W", SIN, 16, str)
    lines += ["", f"; 透視（Q15）。persp[i]＝round(32768×{PERSP_F}÷({PERSP_D}＋i−128))", "persp_table:"]
    lines += dc_lines("DC.W", PERSP, 16, str)
    lines += ["", "; 立方体の頂点（x, y, z）", "cube_verts:"]
    lines += dc_lines("DC.W", [v for p in CUBE_VERTS for v in p], 6, str)
    lines += ["", "; 辺の道：線分の数 k、続けて頂点の番号を k＋1 個。0 で終わり", "cube_paths:"]
    for p in edge_paths(CUBE_EDGES):
        lines += dc_lines("DC.W", [len(p) - 1] + p, 16, str)
    lines += ["        DC.W    0"]
    lines += ["", "; 玉の初めの (x, y, dx, dy, 種類)", "obj_init:"]
    for o in initial_objects():
        lines += dc_lines("DC.W", list(o), 5, str)
    lines += ["", "; パレット", "palette:"]
    lines += dc_lines("DC.W", PALETTE, 8, hexw)
    lines += ["", "; 処理の時間の帯の色（" + "・".join(PHASE_NAMES) + "）", "phase_colors:"]
    lines += dc_lines("DC.W", PHASE_COLORS, 8, hexw)
    lines += ["", f"; 漢字の行「{KANJI_TEXT}」の文字 ROM の offset（仕様書 §08）", "kanji_line:"]
    lines += dc_lines("DC.W", kanji_offsets(), 8, hexw)
    lines += ["", "; 字形の 4 bit（bit 3 が左）→ 4 画素の語のマスク", "nibble_masks:"]
    lines += dc_lines("DC.W", nibble_masks(), 8, hexw)
    write(os.path.join(outdir, "tables.inc"), lines)


def build_dsptab(outdir):
    lines = [
        "; build.py が作った。手で直さない。xform.dsp の後ろで INCLUDE する（TBLR で読む）。",
        "SINTAB",
    ]
    lines += dc_lines("DATA", SIN, 16, str)
    lines += ["PERSP"]
    lines += dc_lines("DATA", PERSP, 16, str)
    write(os.path.join(outdir, "dsptab.inc"), lines)


def shown(path):
    try:
        return os.path.relpath(path, ROOT).replace(os.sep, "/")
    except ValueError:          # Windows で別のドライブ
        return os.path.basename(path)


def build_dsp(outdir):
    asm = dspasm.Assembler(include_dirs=[HERE, outdir, os.path.join(ROOT, "sdk", "include")])
    words = asm.assemble_file(os.path.join(HERE, "xform.dsp"))
    with open(os.path.join(outdir, "xform.bin"), "wb") as f:
        f.write(words)
    with open(os.path.join(outdir, "xform.lst"), "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    return asm, words


def main(argv=None):
    ap = argparse.ArgumentParser(description="SAMPLE 6（通常モードの映像と描画の予算）を組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "showcase.bin"))
    args = ap.parse_args(argv)
    out = os.path.abspath(args.output)
    outdir = os.path.dirname(out)
    os.makedirs(outdir, exist_ok=True)

    assert max_extent() <= 46, f"立方体が空からはみ出す（{max_extent():.1f}）"
    with open(os.path.join(outdir, "sea.bin"), "wb") as f:
        f.write(make_sea())
    with open(os.path.join(outdir, "bank.bin"), "wb") as f:
        f.write(make_bank())
    build_consts(outdir)
    build_tables(outdir)
    build_dsptab(outdir)

    try:
        dasm, dwords = build_dsp(outdir)
    except dspasm.AsmError as e:
        print(f"エラー（DSP）: {e}", file=sys.stderr)
        return 1
    asm = m68kasm.Assembler(include_dirs=[HERE, outdir, os.path.join(ROOT, "sdk", "include")],
                            base=CART_BASE, fill=0xFF)
    try:
        image = asm.assemble_file(os.path.join(HERE, "main.s"))
    except m68kasm.AsmError as e:
        print(f"エラー（68000）: {e}", file=sys.stderr)
        return 1
    assert len(image) <= CART_MAX
    with open(out, "wb") as f:
        f.write(image)
    with open(os.path.splitext(out)[0] + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    for w in dasm.warnings:
        print(f"注意（DSP）: {w}", file=sys.stderr)
    for w in asm.warnings:
        print(f"注意（68000）: {w}", file=sys.stderr)

    print(f"{shown(out)}: {len(image)} バイト")
    print(f"DSP のプログラム: {len(dwords) // 2} 語（xform.bin）")
    print(f"ラスタの表と固定の色で出うる色: {count_colors()} 色")
    print(f"立方体の中心からの広がり: 最大 {max_extent():.1f} 画素、道 {edge_paths(CUBE_EDGES)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
