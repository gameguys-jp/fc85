"""SAMPLE 2 の DSP のプログラム（effects.dsp）の、ビットまで同じ Python のモデル。

TMS32010 の命令の動き（台帳 docs/parts/TMS32010.md §3・§4）を、effects.dsp の並びのとおりに追う。
- ACC は 32 bit（ROVM なので回り込む）。P＝T×係数（32 bit）。LAC x,s は符号拡張して左へ s。
- SACH a,s は (ACC << s) の上位 16 bit。SACL は下位 16 bit。
- データ RAM の語は 16 bit の符号付きとして読む（LAC・LT は符号拡張）。

    from model import Model
    m = Model(fir_taps)
    words = [m.step(b, mode) for b in source_bytes]      # 本体 DAC の語（u·257）
"""

import os

HERE = os.path.dirname(os.path.abspath(__file__))

MODE_THRU, MODE_LPF, MODE_ECHO = 0, 1, 2
MODES = (MODE_THRU, MODE_LPF, MODE_ECHO)
MODE_NAMES = {MODE_THRU: "THRU", MODE_LPF: "LPF", MODE_ECHO: "ECHO"}
RING_START, RING_END = 0x0400, 0x1000     # protocol.inc


def s16(v):
    v &= 0xFFFF
    return v - 0x10000 if v & 0x8000 else v


def s32(v):
    v &= 0xFFFFFFFF
    return v - 0x100000000 if v & 0x80000000 else v


def sach(acc, shift):
    """SACH：(ACC << shift) の上位 16 bit（符号付きで返す）。"""
    return s16(((acc << shift) & 0xFFFFFFFF) >> 16)


def fabric_input(b):
    """Data Fabric のチャネル 0：8 bit → 16 bit、SHIFT 8、MASK $FFFF、XOR $8000。"""
    return s16(((b << 8) & 0xFFFF) ^ 0x8000)


def dac_byte(y):
    """y（16 bit）→ 本体 DAC の 8 bit の符号なし u。0 は 1 にする（effects.dsp）。"""
    u = (sach(y, 8) & 0xFF) ^ 0x80
    return u or 1


def dac_word(y):
    u = dac_byte(y)
    return u << 8 | u


class Model:
    """1 標本ずつ進める。3 つのモードの出力を毎回すべて計算する（DSP と同じ）。"""

    def __init__(self, taps):
        assert len(taps) == 31
        self.h = list(taps)
        self.xn = [0] * len(taps)                       # XN＋k＝x[n−k]
        self.ring = [0] * (RING_END - RING_START)       # 68000 が 0 で埋める
        self.rptr = 0
        self.count = 0
        self.sum = 0
        self.peak = 0

    def outputs(self, b):
        """入力の byte を 1 つ取り、(y_thru, y_lpf, y_echo) を返して状態を進める。"""
        x = fabric_input(b)
        self.xn[0] = x
        # LPF：Σ h[k]·x[n−k]（32 bit で回り込む）。LTD が遅延線を 1 つ後ろへずらす
        acc = 0
        for k in range(len(self.h) - 1, -1, -1):
            acc = s32(acc + self.xn[k] * self.h[k])
        y_lpf = sach(acc, 4)
        self.xn[1:] = self.xn[:-1]
        # ECHO：w＝(x＋w[n−D]) >> 1
        d = self.ring[self.rptr]
        w = sach(s32((x << 15) + (d << 15)), 0)
        self.ring[self.rptr] = w
        self.rptr = (self.rptr + 1) % len(self.ring)
        return x, y_lpf, w

    def step(self, b, mode):
        """入力の byte を 1 つ取り、mode の出力の DAC の語を返す。数・和・山も進める。"""
        ys = self.outputs(b)
        y = ys[mode]
        word = dac_word(y)
        self.count += 1
        self.sum = (self.sum + word) & 0xFFFF
        self.peak = max(self.peak, abs(y))
        return word


def all_outputs(taps, source, n):
    """n 標本の (THRU, LPF, ECHO) の DAC の byte の並び。入力は source を繰り返す（Fabric の LOOP）。"""
    m = Model(taps)
    out = ([], [], [])
    for i in range(n):
        ys = m.outputs(source[i % len(source)])
        for k in MODES:
            out[k].append(dac_byte(ys[k]))
    return out


def load_build(builddir=os.path.join(HERE, "build")):
    """build.py が作った試験の音と FIR の係数を読む。"""
    with open(os.path.join(builddir, "source.bin"), "rb") as f:
        source = f.read()
    taps = {}
    with open(os.path.join(builddir, "fir.inc"), encoding="utf-8") as f:
        for ln in f:
            p = ln.split()
            if len(p) >= 3 and p[1] == "EQU" and p[0].startswith("H") and p[0][1:].isdigit():
                taps[int(p[0][1:])] = int(p[2])
    return source, [taps[k] for k in range(len(taps))]
