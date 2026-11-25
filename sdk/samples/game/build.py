#!/usr/bin/env python3
"""サンプルのカートリッジ SAMPLE 1 を組む。

    python sdk/samples/game/build.py     → sdk/samples/game/build/game.bin（$400000 から）と game.lst

1. 効果音を合成し（正弦波・三角波・雑音。外の音源は使わない）、sdk/tools/adpcm.py で MSM5205 の 4 bit ADPCM に
   する。build/sfx.bin（ADPCM の byte の列）と build/sfx.inc（番地・長さ・S1/S2 の EQU）を書く。
   確かめ用に、復号した音を build/sfx-<名前>.wav にも書く。
2. game.s を sdk/m68kasm/m68kasm.py で組む。INCLUDE と INCBIN は、このフォルダ、build/、sdk/include から探す。

効果音の byte の列（README.md §04）：
    頭 5 byte の 0x08（ニブル 0000・1000 の交互）→ 音の本体 → 終わりに無音へ戻す分 → 4 byte の 0x08
符号は RESET の直後の状態（予測値 0、ステップ 0）から頭の 0x08 を復号した状態から作る。RESET を外した直後に
古いニブル（sfx.s では 0 か 8）が 1 つ復号されると、予測値が ±2 ずれたまま鳴る。頭の 0x08 は、古いニブルが
大きかったときにステップ番号を 0 にそろえる保険 [P]。
"""

import argparse
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "sdk", "m68kasm"))
sys.path.insert(0, os.path.join(ROOT, "sdk", "tools"))

import adpcm  # noqa: E402
from m68kasm import AsmError, Assembler  # noqa: E402

CART_BASE = 0x400000
CART_MAX = 0x100000

# ADPCM の CTRL（DR-053）：bit 1 S1、bit 2 S2。MSM5205 の 384 kHz を 1/48・1/64・1/96 に分ける（台帳 §3.1）
CTRL_S1 = 0x0002
CTRL_S2 = 0x0004
RATES = {8000: CTRL_S1, 6000: CTRL_S2, 4000: 0}

LEAD_IN = bytes([adpcm.UNDERRUN_BYTE]) * 5
TAIL = bytes([adpcm.UNDERRUN_BYTE]) * 4
PEAK = 1500           # 予測値（12 bit）の山。OKI は最大範囲の 80% 以下を勧める（台帳 §4.4）
LIMIT = 1800          # 符号器に許す予測値の範囲


# ---------------------------------------------------------------------------
# 合成（値は −1〜1。後で PEAK を掛ける）


def tone(rate, freq, dur, shape="sine", decay=0.0, attack=0.004):
    """1 つの音。shape は sine・tri・square（倍音を 3 つまで足した柔らかい矩形）。"""
    n = int(rate * dur)
    out = []
    ph = 0.0
    f0 = freq if callable(freq) else (lambda t, f=freq: f)
    for k in range(n):
        t = k / rate
        f = f0(t)
        ph += f / rate
        x = ph % 1.0
        if shape == "sine":
            v = math.sin(2 * math.pi * x)
        elif shape == "tri":
            v = 4 * x - 1 if x < 0.5 else 3 - 4 * x
        else:
            v = 0.0
            for h in (1, 3, 5):
                if f * h < rate * 0.45:
                    v += math.sin(2 * math.pi * x * h) / h
            v *= 0.9
        env = min(1.0, t / attack) if attack > 0 else 1.0
        if decay > 0:
            env *= math.exp(-t / decay)
        out.append(v * env)
    return out


def noise(rate, dur, decay, cutoff):
    """16 bit の LFSR の雑音を 1 次の低域通過に通す。"""
    n = int(rate * dur)
    lfsr = 0xACE1
    a = 1 - math.exp(-2 * math.pi * cutoff / rate)
    y = 0.0
    out = []
    for k in range(n):
        bit = ((lfsr >> 0) ^ (lfsr >> 2) ^ (lfsr >> 3) ^ (lfsr >> 5)) & 1
        lfsr = (lfsr >> 1) | (bit << 15)
        x = 1.0 if lfsr & 1 else -1.0
        y += a * (x - y)
        out.append(y * math.exp(-k / rate / decay))
    return out


def mix(*parts):
    n = max(len(p) for p in parts)
    return [sum(p[k] for p in parts if k < len(p)) for k in range(n)]


def concat(*parts):
    out = []
    for p in parts:
        out.extend(p)
    return out


def fade(samples, rate, dur):
    n = min(len(samples), int(rate * dur))
    out = list(samples)
    for k in range(n):
        out[len(out) - n + k] *= 1 - (k + 1) / n
    return out


def normalize(samples, peak=1.0):
    m = max(abs(v) for v in samples) or 1.0
    return [v * peak / m for v in samples]


def sfx_start(rate):
    """始まりの音：C5・E5・G5・C6 の分散和音。"""
    notes = [(523.25, 0.07), (659.26, 0.07), (783.99, 0.07), (1046.5, 0.25)]
    parts = [tone(rate, f, d, "square", decay=0.12 if d > 0.1 else 0.0) for f, d in notes]
    return fade(concat(*parts), rate, 0.06)


def sfx_collect(rate):
    """拾う音：E6 から B6 へ跳ぶ短い鈴。"""
    a = tone(rate, 1318.5, 0.05, "sine")
    b = tone(rate, 1975.5, 0.17, "sine", decay=0.06)
    return fade(concat(a, b), rate, 0.03)


def sfx_hit(rate):
    """当たる音：雑音と、220 Hz から 55 Hz へ下がる低音。"""
    nz = [0.8 * v for v in noise(rate, 0.30, decay=0.07, cutoff=1500)]
    thump = tone(rate, lambda t: 220 * math.exp(-t / 0.08) + 55, 0.30, "sine", decay=0.10)
    return fade(mix(nz, thump), rate, 0.05)


def sfx_over(rate):
    """終わりの音：G4・E4・C4 と、C4 から C3 へ下がる三角波（ゆれを付ける）。"""
    seq = concat(
        tone(rate, 392.0, 0.18, "tri", decay=0.5),
        tone(rate, 329.63, 0.18, "tri", decay=0.5),
        tone(rate, 261.63, 0.18, "tri", decay=0.5),
    )
    glide = tone(rate, lambda t: 261.63 * 2 ** (-t / 0.7) * (1 + 0.02 * math.sin(2 * math.pi * 6 * t)),
                 0.70, "tri", decay=0.5)
    return fade(concat(seq, glide), rate, 0.15)


# 番号は game.s の SFX_* と同じ順。（名前、関数、標本化周波数）
SOUNDS = [
    ("start", sfx_start, 8000),
    ("collect", sfx_collect, 8000),
    ("hit", sfx_hit, 6000),
    ("over", sfx_over, 4000),
]


def make_sound(fn, rate):
    """合成して ADPCM の byte の列にする。戻り値：(byte の列, 元の値, 復号した予測値)"""
    body = [v * PEAK for v in normalize(fn(rate))]
    lead = adpcm.unpack(LEAD_IN)
    dec = adpcm.Decoder()
    for n in lead:
        dec.feed(n)
    # 本体と、無音へ戻す 20 ms（予測値を 0 の近くに戻し、ステップを小さくする）
    target = body + [0.0] * int(rate * 0.02)
    nib = adpcm.encode(target, signal=dec.signal, step=dec.step, limit=LIMIT)
    if len(nib) % 2:
        nib.append(0)
    data = LEAD_IN + adpcm.pack(nib) + TAIL
    decoded = adpcm.decode(adpcm.unpack(data))
    return data, body, decoded


def write_wav(path, rate, values):
    import struct
    import wave
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(struct.pack(f"<{len(values)}h", *values))


def build_sounds(outdir):
    blob = bytearray()
    lines = [
        "; build.py が作った。手で直さない。",
        "; 効果音（ADPCM）の、sfx.bin の中の offset、byte 数、ADPCM の CTRL の S1・S2（DR-053）。",
        f"SFX_COUNT       EQU     {len(SOUNDS)}",
    ]
    report = []
    for i, (name, fn, rate) in enumerate(SOUNDS):
        data, body, decoded = make_sound(fn, rate)
        if len(blob) % 2:
            blob.append(adpcm.UNDERRUN_BYTE)
        off = len(blob)
        blob += data
        lead = len(adpcm.unpack(LEAD_IN))
        snr = adpcm.snr_db(body, decoded[lead:lead + len(body)])
        peak = max(abs(v) for v in decoded)
        assert peak <= LIMIT + 1, f"{name}: 予測値が {peak}"
        assert decoded == adpcm.decode(adpcm.unpack(data), overflow="clip"), f"{name}: 回り込む"
        lines += [
            f"SFX{i}_OFF        EQU     ${off:04X}           ; {name}",
            f"SFX{i}_LEN        EQU     {len(data)}",
            f"SFX{i}_CTRL       EQU     ${RATES[rate]:04X}           ; {rate} Hz",
        ]
        write_wav(os.path.join(outdir, f"sfx-{name}.wav"), rate, [(v >> 2) * 64 for v in decoded])
        report.append(f"  {i} {name:8s} {rate} Hz  {len(data):5d} byte  {len(data) * 2 / rate:.2f} s  "
                      f"SNR {snr:.1f} dB  山 {peak}")
    lines.append(f"SFX_BYTES       EQU     {len(blob)}")
    with open(os.path.join(outdir, "sfx.bin"), "wb") as f:
        f.write(blob)
    with open(os.path.join(outdir, "sfx.inc"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    return report


def main(argv=None):
    ap = argparse.ArgumentParser(description="サンプルのカートリッジを組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "game.bin"))
    args = ap.parse_args(argv)
    out = os.path.abspath(args.output)
    outdir = os.path.dirname(out)
    os.makedirs(outdir, exist_ok=True)
    report = build_sounds(outdir)
    asm = Assembler(include_dirs=[HERE, outdir, os.path.join(ROOT, "sdk", "include")],
                    base=CART_BASE, fill=0xFF)
    try:
        image = asm.assemble_file(os.path.join(HERE, "game.s"))
    except AsmError as e:
        print(f"エラー: {e}", file=sys.stderr)
        return 1
    assert len(image) <= CART_MAX
    with open(out, "wb") as f:
        f.write(image)
    with open(os.path.splitext(out)[0] + ".lst", "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    for w in asm.warnings:
        print(f"注意: {w}", file=sys.stderr)
    try:
        shown = os.path.relpath(out, ROOT)
    except ValueError:          # Windows で別のドライブ
        shown = os.path.basename(out)
    print(f"{shown}: {len(image)} バイト")
    print("効果音:")
    print("\n".join(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
