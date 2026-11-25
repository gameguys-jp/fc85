#!/usr/bin/env python3
"""エミュレータの WAV（--wav）を、モデル model.py と標本ごとに比べる（DOC の子基板がない道）。

    py sdk/samples/dsp/verify_wav.py <scratchpad>/d.wav [--count N --sum S ...]

1. WAV（48 kHz・16 bit・ステレオ）の L・R は、本体 DAC の (u−128)×128 に ADPCM の分（止まっていれば一定）
   を足したもの（console.rs の mix）。u に戻す。
2. 本体 DAC は 16 kHz（8 MHz÷500）なので、WAV の 3 標本に 1 つ。同じ値が 3 つ続く位相を選んで間引く。
3. THRU の出力は試験の音の byte そのもの。最初の THRU の区間で、DAC の並びの中の DSP の 0 番目の標本の
   位置を探す。
4. そこから先を、3 つのモードのモデルの出力と比べる。今のモードが合わなくなったら、そこから先に長く合うモード
   へ切り替える。切り替えた標本の番号（画面の SW と同じ）と、どのモードとも合わない標本の数を出す。
5. --count と --sum を与えると、その数までの出力の語の和（画面の COUNT と SUM）をモデルで計算して比べる。
"""

import argparse
import os
import struct
import sys
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import model  # noqa: E402

DECIM = 3       # 48 kHz ÷ 16 kHz


def read_wav(path):
    with wave.open(path, "rb") as w:
        assert w.getnchannels() == 2 and w.getsampwidth() == 2, "48 kHz・16 bit・ステレオのはず"
        rate = w.getframerate()
        raw = w.readframes(w.getnframes())
    v = struct.unpack(f"<{len(raw) // 2}h", raw)
    return rate, list(v[0::2]), list(v[1::2])


def to_u(samples):
    """(u−128)×128＋a → u。a（ADPCM の分）は最初の標本の 128 の余りから取る。"""
    a = samples[0] % 128
    bad = sum(1 for s in samples if (s - a) % 128)
    return [(s - a) // 128 + 128 for s in samples], a, bad


def decimate(u):
    """3 つずつ同じ値が並ぶ位相を選ぶ。戻り値：(間引いた並び, 位相, 3 つがそろわなかった組の数)。"""
    best = None
    for p in range(DECIM):
        groups = [u[i:i + DECIM] for i in range(p, len(u) - DECIM + 1, DECIM)]
        bad = sum(1 for g in groups if g.count(g[0]) != DECIM)
        if best is None or bad < best[2]:
            best = ([g[0] for g in groups], p, bad)
    return best


def find_start(dac, thru, probe=4000):
    """dac[j0+n]＝thru[n]（n＜probe）となる j0。"""
    for j0 in range(0, len(dac) - probe):
        if dac[j0:j0 + probe] == thru[:probe]:
            return j0
    return None


def track(dac, outs, n, look=64):
    """モードを追う。戻り値：(切り替えの並び [(標本の番号, モード)], 合わない標本の数, 最初に合わなかった番号)。"""
    mode = model.MODE_THRU
    switches = [(0, mode)]
    bad = 0
    first_bad = None
    for i in range(n):
        if outs[mode][i] == dac[i]:
            continue
        # 今のモードが合わない：ここから look 標本で一番長く合うモード
        best, run = None, 0
        for m in model.MODES:
            k = 0
            while k < look and i + k < n and outs[m][i + k] == dac[i + k]:
                k += 1
            if k > run:
                best, run = m, k
        if best is None:
            bad += 1
            if first_bad is None:
                first_bad = i
            continue
        mode = best
        switches.append((i, mode))
    return switches, bad, first_bad


def main(argv=None):
    ap = argparse.ArgumentParser(description="WAV をモデルと比べる")
    ap.add_argument("wav")
    ap.add_argument("--build", default=os.path.join(HERE, "build"))
    ap.add_argument("--count", type=lambda s: int(s, 0), action="append", default=[],
                    help="画面の COUNT（16 進は 0x を付ける）")
    ap.add_argument("--sum", type=lambda s: int(s, 0), action="append", default=[],
                    help="画面の SUM（--count と同じ順）")
    ap.add_argument("--sw", type=lambda s: int(s, 0), action="append", default=[],
                    help="画面の SW（モードを替えた標本の番号の下位 16 bit）")
    args = ap.parse_args(argv)

    rate, left, right = read_wav(args.wav)
    print(f"WAV: {rate} Hz、{len(left)} 標本（{len(left) / rate:.3f} 秒）")
    if left != right:
        print("注意: L と R が違う")
    u, a, bad = to_u(left)
    print(f"DAC の外の一定の分: {a}、128 の倍数にならない標本: {bad}")
    dac, phase, uneven = decimate(u)
    print(f"間引き: 位相 {phase}、{len(dac)} 標本（16 kHz）、3 つがそろわない組: {uneven}")

    source, taps = model.load_build(args.build)
    thru = [source[i % len(source)] for i in range(8000)]
    j0 = find_start(dac, thru)
    if j0 is None:
        print("THRU の区間が見つからない")
        return 1
    n = len(dac) - j0
    print(f"DSP の 0 番目の標本は、DAC の {j0} 番目（{j0 / 16000 * 1000:.1f} ms）。比べる標本: {n}")
    outs = model.all_outputs(taps, source, n)
    switches, nbad, first_bad = track(dac[j0:], outs, n)
    print("モードの切り替え（標本の番号、画面の SW は下位 16 bit）:")
    for i, m in switches:
        print(f"  {i:7d}  ${i & 0xFFFF:04X}  {model.MODE_NAMES[m]}")
    print(f"どのモードとも合わない標本: {nbad}" + (f"（最初は {first_bad}）" if nbad else ""))

    # 画面の SW：WAV で見えた切り替え i より前の、下位 16 bit が SW の番号 t から、新しいモードの出力が
    # DAC と合っているか（t〜i の間は、前のモードと新しいモードの出力が同じだった）
    dsp = dac[j0:]
    sw_bad = 0
    for s in args.sw:
        hits = []
        for i, m in switches[1:]:
            t = i - ((i - s) & 0xFFFF)
            if t >= 0 and all(outs[m][k] == dsp[k] for k in range(t, i + 1)):
                hits.append((t, i, m))
        if hits:
            t, i, m = hits[0]
            print(f"SW ${s:04X}: 標本 {t} から {model.MODE_NAMES[m]}（WAV で違いが見えたのは {i}）。合う")
        else:
            sw_bad += 1
            print(f"SW ${s:04X}: 合う切り替えがない")

    # 画面の COUNT・SUM
    if args.count:
        modes = []
        k = 0
        for i in range(max(args.count)):
            while k + 1 < len(switches) and switches[k + 1][0] <= i:
                k += 1
            modes.append(switches[k][1])
        m = model.Model(taps)
        sums = {}
        for i in range(max(args.count)):
            m.step(source[i % len(source)], modes[i])
            if i + 1 in args.count:
                sums[i + 1] = m.sum
        for c, s in zip(args.count, args.sum + [None] * len(args.count)):
            got = sums[c]
            mark = "" if s is None else ("  一致" if s == got else f"  画面は ${s:04X}：違う")
            if s is not None and s != got:
                sw_bad += 1
            print(f"COUNT ${c:08X} までの SUM（モデル）: ${got:04X}{mark}")
    return 0 if nbad == 0 and sw_bad == 0 else 2


if __name__ == "__main__":
    sys.exit(main())
