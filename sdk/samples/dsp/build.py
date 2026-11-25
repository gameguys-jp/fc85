#!/usr/bin/env python3
"""DSP のサンプルのカートリッジ SAMPLE 2 を組む。

    py sdk/samples/dsp/build.py     → sdk/samples/dsp/build/dsp.bin（$400000 から）と dsp.lst

1. 試験の音を合成する（ノコギリ波のはじく音、雑音、無音。外の音源は使わない）。16 kHz・8 bit の符号なし、2 秒。
   build/source.bin（32000 byte）。確かめ用に build/source.wav。
2. FIR の低域通過を設計する（31 タップ、Hamming 窓の sinc、係数は Q12 の整数で和が 4096）。build/fir.inc
   （NTAPS と H0〜H30 の EQU）。
3. effects.dsp を sdk/dspasm/dspasm.py で組む → build/effects.bin（語を big-endian）と effects.lst。
4. main.s を sdk/m68kasm/m68kasm.py で組む → build/dsp.bin と dsp.lst。

INCLUDE と INCBIN は、このフォルダ、build/、sdk/include から探す。
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
sys.path.insert(0, os.path.join(ROOT, "sdk", "dspasm"))

import dspasm  # noqa: E402
import m68kasm  # noqa: E402

CART_BASE = 0x400000
CART_MAX = 0x100000

RATE = 16000            # 本体 DAC の音声クロック（8 MHz÷500）[P]
SECONDS = 2
SRC_PEAK = 0.6          # 試験の音の山（フルスケールに対する比）。FIR の行き過ぎとエコーの和であふれない [P]
NTAPS = 31
CUTOFF = 1200.0         # 低域通過の遮断（Hz）[P]
COEF_ONE = 4096         # Q12。MPYK は 13 bit の符号付き（−4096〜4095）


# ---------------------------------------------------------------------------
# 試験の音（値は −1〜1）

def saw(rate, freq, dur, decay, attack=0.002):
    """帯域を制限したノコギリ波（倍音を足す）を、指数で減衰させる。"""
    n = int(rate * dur)
    harm = [h for h in range(1, 64) if freq * h < rate * 0.45]
    out = []
    for k in range(n):
        t = k / rate
        v = sum(math.sin(2 * math.pi * freq * h * t) / h for h in harm) * (2 / math.pi)
        env = min(1.0, t / attack) * math.exp(-t / decay)
        out.append(v * env)
    return out


def noise(rate, dur, decay):
    """16 bit の LFSR の白い雑音を、指数で減衰させる。"""
    n = int(rate * dur)
    lfsr = 0xACE1
    out = []
    for k in range(n):
        bit = ((lfsr >> 0) ^ (lfsr >> 2) ^ (lfsr >> 3) ^ (lfsr >> 5)) & 1
        lfsr = (lfsr >> 1) | (bit << 15)
        out.append((1.0 if lfsr & 1 else -1.0) * math.exp(-k / rate / decay))
    return out


def place(buf, at, part, gain=1.0):
    for k, v in enumerate(part):
        if at + k < len(buf):
            buf[at + k] += v * gain


def synth_source(rate=RATE, seconds=SECONDS):
    """2 秒の試験の音。はじく音 4 つと雑音 2 つ。間に無音を置き、エコー（192 ms）が聞こえるようにする。"""
    buf = [0.0] * (rate * seconds)
    place(buf, int(0.00 * rate), saw(rate, 220.0, 0.30, 0.10))      # A3
    place(buf, int(0.25 * rate), saw(rate, 330.0, 0.30, 0.10))      # E4
    place(buf, int(0.55 * rate), noise(rate, 0.12, 0.03), 0.8)      # 打つ音
    place(buf, int(0.85 * rate), saw(rate, 440.0, 0.45, 0.15))      # A4
    place(buf, int(1.30 * rate), noise(rate, 0.12, 0.03), 0.8)
    place(buf, int(1.45 * rate), saw(rate, 165.0, 0.40, 0.12))      # E3
    m = max(abs(v) for v in buf)
    return [v * SRC_PEAK / m for v in buf]


def to_u8(values):
    """−1〜1 → 8 bit の符号なし（128 が中央）。"""
    return bytes(max(1, min(255, 128 + round(v * 127))) for v in values)


# ---------------------------------------------------------------------------
# FIR の低域通過

def design_fir(ntaps=NTAPS, cutoff=CUTOFF, rate=RATE, one=COEF_ONE):
    """Hamming 窓の sinc。整数に丸め、和が one になるよう真ん中のタップで合わせる。左右対称。"""
    mid = (ntaps - 1) / 2
    fc = cutoff / rate
    h = []
    for k in range(ntaps):
        x = k - mid
        s = 2 * fc if x == 0 else math.sin(2 * math.pi * fc * x) / (math.pi * x)
        w = 0.54 - 0.46 * math.cos(2 * math.pi * k / (ntaps - 1))
        h.append(s * w)
    total = sum(h)
    q = [round(v / total * one) for v in h]
    q[int(mid)] += one - sum(q)
    assert all(-4096 <= v <= 4095 for v in q)
    return q


def fir_response(h, freq, rate=RATE, one=COEF_ONE):
    """周波数 freq の利得（dB）。"""
    re = sum(v * math.cos(2 * math.pi * freq / rate * k) for k, v in enumerate(h))
    im = sum(v * math.sin(2 * math.pi * freq / rate * k) for k, v in enumerate(h))
    g = math.hypot(re, im) / one
    return 20 * math.log10(g) if g > 0 else -999.0


def write_fir_inc(path, h):
    lines = [
        "; build.py が作った。手で直さない。",
        f"; FIR の低域通過：{len(h)} タップ、Hamming 窓の sinc、遮断 {CUTOFF:.0f} Hz（{RATE} Hz）、Q12（和 {sum(h)}）。",
        "; H<k> は x[n−k] に掛ける係数。",
        f"NTAPS   EQU     {len(h)}",
    ]
    lines += [f"H{k:<6d}EQU     {v}" for k, v in enumerate(h)]
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


def write_wav(path, rate, data_u8):
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(struct.pack(f"<{len(data_u8)}h", *[(b - 128) << 8 for b in data_u8]))


# ---------------------------------------------------------------------------

def shown(path):
    try:
        return os.path.relpath(path, ROOT)
    except ValueError:          # Windows で別のドライブ
        return os.path.basename(path)


def build_dsp(outdir):
    asm = dspasm.Assembler(include_dirs=[HERE, outdir, os.path.join(ROOT, "sdk", "include")])
    words = asm.assemble_file(os.path.join(HERE, "effects.dsp"))
    path = os.path.join(outdir, "effects.bin")
    with open(path, "wb") as f:
        f.write(words)
    with open(os.path.join(outdir, "effects.lst"), "w", encoding="utf-8", newline="\n") as f:
        f.write(asm.listing_text())
    return asm, words


def main(argv=None):
    ap = argparse.ArgumentParser(description="DSP のサンプルのカートリッジを組む")
    ap.add_argument("-o", "--output", default=os.path.join(HERE, "build", "dsp.bin"))
    args = ap.parse_args(argv)
    out = os.path.abspath(args.output)
    outdir = os.path.dirname(out)
    os.makedirs(outdir, exist_ok=True)

    src = to_u8(synth_source())
    with open(os.path.join(outdir, "source.bin"), "wb") as f:
        f.write(src)
    write_wav(os.path.join(outdir, "source.wav"), RATE, src)

    h = design_fir()
    write_fir_inc(os.path.join(outdir, "fir.inc"), h)

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
    print(f"DSP のプログラム: {len(dwords) // 2} 語（effects.bin）")
    print(f"試験の音: {len(src)} byte、{RATE} Hz、{min(src)}〜{max(src)}")
    print(f"FIR: {NTAPS} タップ、和 {sum(h)}、|h| の和 {sum(abs(v) for v in h)}、"
          f"500 Hz {fir_response(h, 500):.1f} dB、{CUTOFF:.0f} Hz {fir_response(h, CUTOFF):.1f} dB、"
          f"3000 Hz {fir_response(h, 3000):.1f} dB、5000 Hz {fir_response(h, 5000):.1f} dB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
