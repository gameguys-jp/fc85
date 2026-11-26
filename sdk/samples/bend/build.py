#!/usr/bin/env python3
"""ベンド点を試すための試験信号のカートリッジ SAMPLE 5 を組む。

    py sdk/samples/bend/build.py     → sdk/samples/bend/build/bend.bin（$400000 から）と bend.lst

1. DOC の波形（正弦・三角・のこぎり・矩形、256 byte ずつ）、MIDI の音ごとの FC の表、曲の表を作る。
   build/waves.bin、build/song.inc（表。main.s の後ろで INCLUDE）。
2. 太鼓（キック・スネア・ハイハット）を合成し（外の音源は使わない）、sdk/tools/adpcm.py で MSM5205 の 4 bit
   ADPCM にする。build/drums.bin（byte の列）、build/tables.inc（offset・長さの EQU。main.s の頭で INCLUDE）。
   確かめ用に、復号した音を build/drum-<名前>.wav にも書く。
3. 静止画の格子の部分（市松模様と方眼、256×48 画素）を ACRTC の DWT のデータ（4 bpp の語、左の画素が下位
   ニブル）にする。build/pattern.bin。
4. main.s を sdk/m68kasm/m68kasm.py で組む。INCLUDE と INCBIN は、このフォルダ、build/、sdk/include から探す。

値はすべて [P]（README.md §06）。
"""

import argparse
import math
import os
import struct
import sys
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "m68kasm"))
sys.path.insert(0, os.path.join(ROOT, "sdk", "tools"))

import adpcm  # noqa: E402
from m68kasm import AsmError, Assembler  # noqa: E402

CART_BASE = 0x400000
CART_MAX = 0x100000

# ---------------------------------------------------------------------------
# DOC（台帳 docs/parts/ES5503.md §3.2・§4.3）

DOC_CLK = 8_000_000     # 子基板の CLK（工場出荷）
DOC_OSCS = 16           # 走査するオシレータの数（BIOS の 0xE1＝$1E。main.s も同じ値を書く）
DOC_SR = DOC_CLK / 8 / (DOC_OSCS + 2)   # 標本化 55,555.6 Hz
TABLE_BITS = 17         # 256 byte の表（T＝0）、分解能 R＝0：f＝SR×FC÷2^17

WAVE_NAMES = ("sine", "triangle", "saw", "square")   # 頁 0・1・2・3（4 頁ごとに繰り返す）
WAVE_CENTER = 0x80
WAVE_AMP = 0x60         # 中央 ±0x60（0x20〜0xE0）。0x00 はオシレータを止めるので使わない


def midi_hz(note):
    """MIDI の音の番号の周波数（A4＝69＝440 Hz、平均律）。"""
    return 440.0 * 2.0 ** ((note - 69) / 12)


def fc_of(hz, clk=DOC_CLK, oscs=DOC_OSCS):
    """周波数 hz を 256 byte の表で鳴らす FC（16 bit）。"""
    sr = clk / 8 / (oscs + 2)
    return max(1, min(0xFFFF, round(hz * 2 ** TABLE_BITS / sr)))


def fc_hz(fc, clk=DOC_CLK, oscs=DOC_OSCS):
    """FC で鳴る周波数。"""
    return clk / 8 / (oscs + 2) * fc / 2 ** TABLE_BITS


PITCH_FC = [fc_of(midi_hz(n)) for n in range(128)]


def wave_value(name, k):
    """波形 name の k 番目（0〜255）の値（1〜255）。"""
    x = k / 256
    if name == "sine":
        v = math.sin(2 * math.pi * x)
    elif name == "triangle":
        v = 1 - 4 * abs(((x + 0.25) % 1.0) - 0.5)       # 0 から上がる三角
    elif name == "saw":
        v = 2 * x - 1
    else:
        v = 1.0 if k < 128 else -1.0
    return max(1, min(255, WAVE_CENTER + round(WAVE_AMP * v)))


def make_waves():
    return bytes(wave_value(name, k) for name in WAVE_NAMES for k in range(256))


# ---------------------------------------------------------------------------
# 曲（A の短調、4 小節で一巡り。1 小節は 16 歩、1 歩は 16 分音符）[P]

REST = 0xFF             # 低音：休み（音量 0 を書く）
HOLD = 0xFE             # 低音：何も書かない（前の音が続く）
NO_DRUM = 0xFF

DRONE_NOTE = 45         # A2（110 Hz）。のこぎり波、L と R

# 小節ごとの和音：名前、低音の根音、分散和音の 5 つの音
BARS = [
    ("Am", 33, (57, 60, 64, 69, 72)),   # A1 ／ A3 C4 E4 A4 C5
    ("F", 29, (57, 60, 65, 69, 72)),    # F1 ／ A3 C4 F4 A4 C5
    ("C", 36, (55, 60, 64, 67, 72)),    # C2 ／ G3 C4 E4 G4 C5
    ("G", 31, (55, 59, 62, 67, 71)),    # G1 ／ G3 B3 D4 G4 B4
]
# 分散和音：歩ごとの和音の音の番号。偶数の歩はオシレータ 4（L、三角波）、奇数は 5（R、正弦波）
LEAD = (0, 1, 2, 3, 4, 3, 2, 1, 0, 1, 2, 3, 4, 2, 3, 1)
# 低音：根音からの半音の数（REST は休み）。オシレータ 2（L）と 3（R）、矩形波
BASS = (0, REST, 0, REST, 12, REST, 0, REST, 0, REST, 0, REST, 12, REST, 7, REST)

# 太鼓：8 分音符ごとの音の番号（DRUMS の順）
DRUMS = [
    ("kick-hat", "キックとハイハット"),
    ("snare-hat", "スネアとハイハット"),
    ("hat", "ハイハット"),
]
KH, SH, HH = 0, 1, 2
DRUM_PATTERN = (KH, HH, SH, HH, KH, KH, SH, HH)


def song_notes():
    """曲で鳴らす DOC の音（MIDI の番号）の集合。"""
    notes = {DRONE_NOTE}
    for _, root, tones in BARS:
        notes.update(tones[i] for i in LEAD)
        notes.update(root + b for b in BASS if b not in (REST, HOLD))
    return notes


# ---------------------------------------------------------------------------
# 太鼓（ADPCM。sdk/samples/game/build.py と同じ形の byte の列）

ADPCM_RATE = 8000       # S1（384 kHz÷48）
LEAD_IN = bytes([adpcm.UNDERRUN_BYTE]) * 5
TAIL = bytes([adpcm.UNDERRUN_BYTE]) * 4
PEAK = 1800             # 予測値（12 bit）の山（DOC の声と釣り合うように game より大きく）[P]
LIMIT = 1950            # 符号器に許す予測値の範囲
DRUM_MAX_SECONDS = 0.20  # 8 分音符（15 フレーム）のうち、次を準備するまでに鳴り終える長さ


def lfsr_noise(n, seed=0xACE1):
    out = []
    lfsr = seed
    for _ in range(n):
        bit = ((lfsr >> 0) ^ (lfsr >> 2) ^ (lfsr >> 3) ^ (lfsr >> 5)) & 1
        lfsr = (lfsr >> 1) | (bit << 15)
        out.append(1.0 if lfsr & 1 else -1.0)
    return out


def kick(rate):
    """150 Hz から 45 Hz へ下がる正弦波。"""
    out = []
    ph = 0.0
    for k in range(int(rate * 0.15)):
        t = k / rate
        ph += (45 + 105 * math.exp(-t / 0.025)) / rate
        out.append(math.sin(2 * math.pi * ph) * math.exp(-t / 0.055))
    return out


def snare(rate):
    """低域通過の雑音と 185 Hz の胴鳴り。"""
    n = int(rate * 0.14)
    nz = lfsr_noise(n, 0x1D2B)
    a = 1 - math.exp(-2 * math.pi * 3000 / rate)
    y = 0.0
    out = []
    for k in range(n):
        t = k / rate
        y += a * (nz[k] - y)
        body = math.sin(2 * math.pi * 185 * t) * math.exp(-t / 0.035)
        out.append(0.75 * y * math.exp(-t / 0.045) + 0.5 * body)
    return out


def hat(rate):
    """高域通過の雑音（差分）。"""
    n = int(rate * 0.05)
    nz = lfsr_noise(n + 1, 0x7A31)
    return [0.5 * (nz[k + 1] - nz[k]) * math.exp(-k / rate / 0.014) for k in range(n)]


def mix(*parts):
    """(倍率, 音) の和。"""
    n = max(len(p) for _, p in parts)
    return [sum(g * p[k] for g, p in parts if k < len(p)) for k in range(n)]


def fade(samples, rate, dur):
    n = min(len(samples), int(rate * dur))
    out = list(samples)
    for k in range(n):
        out[len(out) - n + k] *= 1 - (k + 1) / n
    return out


def drum_sources(rate=ADPCM_RATE):
    """DRUMS の順の音（値は −1〜1 の近く。山をそろえる前）。"""
    k, s, h = kick(rate), snare(rate), hat(rate)
    srcs = [
        mix((1.0, k), (0.5, h)),
        mix((0.9, s), (0.5, h)),
        mix((0.7, h)),
    ]
    return [fade(x, rate, 0.02) for x in srcs]


def encode_drum(body):
    """予測値の列（山を PEAK にした）を ADPCM の byte の列にする。戻り値：(byte の列, 復号した予測値)"""
    dec = adpcm.Decoder()
    for n in adpcm.unpack(LEAD_IN):
        dec.feed(n)
    target = body + [0.0] * int(ADPCM_RATE * 0.02)       # 無音へ戻す 20 ms
    nib = adpcm.encode(target, signal=dec.signal, step=dec.step, limit=LIMIT)
    if len(nib) % 2:
        nib.append(0)
    data = LEAD_IN + adpcm.pack(nib) + TAIL
    return data, adpcm.decode(adpcm.unpack(data))


def build_drums(outdir):
    srcs = drum_sources()
    scale = PEAK / max(abs(v) for x in srcs for v in x)   # 3 つの音の比を保つ
    blob = bytearray()
    lines = [
        "; build.py が作った。手で直さない。",
        "; 太鼓（ADPCM）の、drums.bin の中の offset と byte 数。標本化は 8 kHz（S1）。",
        f"DRUM_COUNT      EQU     {len(DRUMS)}",
    ]
    report = []
    for i, ((name, label), src) in enumerate(zip(DRUMS, srcs)):
        body = [v * scale for v in src]
        data, decoded = encode_drum(body)
        if len(blob) % 2:
            blob.append(adpcm.UNDERRUN_BYTE)
        off = len(blob)
        blob += data
        lead = len(adpcm.unpack(LEAD_IN))
        snr = adpcm.snr_db(body, decoded[lead:lead + len(body)])
        peak = max(abs(v) for v in decoded)
        assert peak <= LIMIT + 1, f"{name}: 予測値が {peak}"
        assert decoded == adpcm.decode(adpcm.unpack(data), overflow="clip"), f"{name}: 回り込む"
        assert len(data) * 2 / ADPCM_RATE <= DRUM_MAX_SECONDS, f"{name}: 長すぎる"
        lines += [
            f"DRUM{i}_OFF       EQU     ${off:04X}           ; {label}",
            f"DRUM{i}_LEN       EQU     {len(data)}",
        ]
        write_wav(os.path.join(outdir, f"drum-{name}.wav"), ADPCM_RATE, [(v >> 2) * 64 for v in decoded])
        report.append(f"  {i} {name:10s} {len(data):4d} byte  {len(data) * 2 / ADPCM_RATE:.3f} s  "
                      f"SNR {snr:.1f} dB  山 {peak}")
    lines.append(f"DRUM_BYTES      EQU     {len(blob)}")
    lines += [
        "",
        "; 曲（song.inc の表の形）",
        f"DRONE_NOTE      EQU     {DRONE_NOTE}              ; A2",
        f"SONG_BARS       EQU     {len(BARS)}",
        f"REST            EQU     ${REST:02X}",
        f"HOLD            EQU     ${HOLD:02X}",
        f"NO_DRUM         EQU     ${NO_DRUM:02X}",
        f"DOC_OSCS        EQU     {DOC_OSCS}",
    ]
    with open(os.path.join(outdir, "drums.bin"), "wb") as f:
        f.write(blob)
    with open(os.path.join(outdir, "tables.inc"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    return report


def write_wav(path, rate, values):
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(struct.pack(f"<{len(values)}h", *values))


# ---------------------------------------------------------------------------
# 表（song.inc）


def note_name(n):
    names = ("C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B")
    return f"{names[n % 12]}{n // 12 - 1}"


def dc_lines(directive, values, per_line, fmt):
    out = []
    for i in range(0, len(values), per_line):
        out.append(f"        {directive}    " + ",".join(fmt(v) for v in values[i:i + per_line]))
    return out


def build_song(outdir):
    lines = [
        "; build.py が作った。手で直さない。main.s の表の節で INCLUDE する。",
        "",
        f"; MIDI の音 0〜127 の FC。DOC の CLK {DOC_CLK / 1e6:g} MHz、走査 {DOC_OSCS} 個（標本化 "
        f"{DOC_SR:.1f} Hz）、",
        "; 256 byte の表（T＝0、R＝0）：FC＝round(f×2^17÷標本化)（台帳 ES5503.md §4.3）",
        "pitch_fc:",
    ]
    lines += dc_lines("DC.W", PITCH_FC, 8, lambda v: f"${v:04X}")
    lines += ["", "; 小節ごとの低音の根音"]
    lines.append("bar_root:")
    lines += dc_lines("DC.B", [r for _, r, _ in BARS], 4, str)
    lines += ["", "; 小節ごとの分散和音の音（5 つ。8 byte ずつ）"]
    lines.append("bar_tones:")
    for name, _, tones in BARS:
        row = list(tones) + [0] * (8 - len(tones))
        lines.append("        DC.B    " + ",".join(str(v) for v in row)
                     + f"     ; {name}：" + " ".join(note_name(v) for v in tones))
    lines += ["", "; 分散和音：歩ごとの和音の音の番号（偶数の歩は L、奇数は R）", "lead_pat:"]
    lines += dc_lines("DC.B", list(LEAD), 16, str)
    lines += ["", "; 低音：根音からの半音の数（REST は休み、HOLD は書かない）", "bass_pat:"]
    lines += dc_lines("DC.B", list(BASS), 16, lambda v: f"${v:02X}")
    lines += ["", "; 太鼓：8 分音符ごとの音の番号（" + "、".join(f"{i} {lb}" for i, (_, lb) in enumerate(DRUMS))
              + "）", "drum_pat:"]
    lines += dc_lines("DC.B", list(DRUM_PATTERN), 8, str)
    lines += ["        EVEN"]
    with open(os.path.join(outdir, "song.inc"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    report = []
    worst = max(song_notes(), key=lambda n: abs(cents(n)))
    report.append(f"  DOC の音 {len(song_notes())} 個（{note_name(min(song_notes()))}〜"
                  f"{note_name(max(song_notes()))}）、平均律からの最大のずれ {cents(worst):+.1f} セント"
                  f"（{note_name(worst)}、FC {PITCH_FC[worst]}）")
    return report


def cents(note):
    """FC の表で鳴る音の、平均律からのずれ（セント）。"""
    return 1200 * math.log2(fc_hz(PITCH_FC[note]) / midi_hz(note))


# ---------------------------------------------------------------------------
# 格子（pattern.bin）：x 0〜127 は 1 画素の市松（色 15 と 0）、x 128〜255 は 8 画素ごとの方眼（色 14、地は 0）

GRID_ROWS = 48
GRID_CHECK_A = 15
GRID_CHECK_B = 0
GRID_LINE = 14
GRID_BG = 0


def grid_pixel(x, y):
    if x < 128:
        return GRID_CHECK_A if (x + y) % 2 == 0 else GRID_CHECK_B
    return GRID_LINE if x % 8 == 0 or y % 8 == 0 or x == 255 or y == GRID_ROWS - 1 else GRID_BG


def make_pattern():
    """GRID_ROWS 行 × 64 語（big-endian）。語の左の画素が下位ニブル（bios-abi.md §06）。"""
    out = bytearray()
    for y in range(GRID_ROWS):
        for w in range(64):
            v = 0
            for i in range(4):
                v |= grid_pixel(4 * w + i, y) << (4 * i)
            out += struct.pack(">H", v)
    return bytes(out)


# ---------------------------------------------------------------------------


def shown(path):
    """リポジトリの根からの相対パス（ローカルの絶対パスを出さない）。"""
    try:
        return os.path.relpath(path, ROOT).replace(os.sep, "/")
    except ValueError:          # Windows で別のドライブ
        return os.path.basename(path)


def main(argv=None):
    ap = argparse.ArgumentParser(description="ベンド点を試す試験信号のカートリッジを組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "bend.bin"))
    args = ap.parse_args(argv)
    out = os.path.abspath(args.output)
    outdir = os.path.dirname(out)
    os.makedirs(outdir, exist_ok=True)

    waves = make_waves()
    assert 0 not in waves
    with open(os.path.join(outdir, "waves.bin"), "wb") as f:
        f.write(waves)
    with open(os.path.join(outdir, "pattern.bin"), "wb") as f:
        f.write(make_pattern())
    report = build_drums(outdir)
    report += build_song(outdir)

    asm = Assembler(include_dirs=[HERE, outdir, os.path.join(ROOT, "sdk", "include")],
                    base=CART_BASE, fill=0xFF)
    try:
        image = asm.assemble_file(os.path.join(HERE, "main.s"))
    except AsmError as e:
        print(f"エラー（68000）: {e}", file=sys.stderr)
        return 1
    assert len(image) <= CART_MAX
    with open(out, "wb") as f:
        f.write(image)
    with open(os.path.splitext(out)[0] + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    for w in asm.warnings:
        print(f"注意（68000）: {w}", file=sys.stderr)
    print(f"{shown(out)}: {len(image)} バイト")
    print("太鼓と曲:")
    print("\n".join(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
