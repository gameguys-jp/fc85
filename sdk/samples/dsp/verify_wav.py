#!/usr/bin/env python3
"""エミュレータの WAV（--wav）と画面の数を、モデル model.py と標本ごとに比べる。

    py sdk/samples/dsp/verify_wav.py <scratchpad>/d.wav [--sw S]... [--count C --sum S]...
    py sdk/samples/dsp/verify_wav.py <scratchpad>/e.wav --doc [--sw S]... [--count C --sum S]...

本体 DAC の道（既定。DOC の子基板がない）
1. WAV（48 kHz・16 bit・ステレオ）の L・R は、本体 DAC の (u−128)×128 に ADPCM の分（止まっていれば一定）
   を足したもの（console.rs の mix）。u に戻す。
2. 本体 DAC は 16 kHz（8 MHz÷500）なので、WAV の 3 標本に 1 つ。同じ値が 3 つ続く位相を選んで間引く。
3. THRU の出力は試験の音の byte そのもの。最初の THRU の区間で、DSP の 0 番目の標本の位置を探す。
4. そこから先を 3 つのモードのモデルの出力と比べ、モードを追う。

DOC の道（--doc）
1. DOC（E＝1 MHz）は 18 サイクルごとにオシレータ 0 を訪れ、輪の byte を読んで次まで (u−128)×255 を保つ。
   アキュムレータは 1 回に FC＝144、512 で 1 byte なので、j 回目に読む byte は floor((9j＋b)/32)。
   console.rs の mix は、前に取ったときからの E の各サイクルの値の和を数で割り（切捨て）、4 で割る（切捨て）。
   取るのは 48 kHz の時刻の後の最初のスロット（250 ns）の境。
2. 音の立ち上がりと 15.625 kHz の階段で b を見積もり、E の位相と b を、WAV と整数で一致するように探す。
3. WAV の各標本を、3 つのモードのモデルから計算した値と比べ、モードを追う。モードが替わる WAV の標本は、
   2 つのモードの出力を平均するので、どちらとも合わない（切り替えの印になる）。

両方の道の最後
- --sw に画面の SW（モードを替えた標本の番号の下位 16 bit）を与えると、WAV で見えた切り替えの近くの、下位
  16 bit が SW と同じ番号を本当の切り替えにする（その間は、前と新しいモードの出力が WAV の上で同じ）。
- 切り替えの番号どおりにモードを替えたモデルの出力と、WAV の全部の標本を比べる（合わない数が 0 なら合格）。
- --count と --sum に画面の COUNT と SUM を与えると、その数までの出力の語の和をモデルで計算して比べる。
"""

import argparse
import math
import os
import struct
import sys
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import model  # noqa: E402

WAV_RATE = 48000
DAC_RATE = 16000        # 8 MHz÷500
DOC_RATE = 15625        # 8 MHz÷512
DECIM = WAV_RATE // DAC_RATE
LOOK = 64               # モードを選ぶときに先を見る標本の数


def read_wav(path):
    with wave.open(path, "rb") as w:
        assert w.getnchannels() == 2 and w.getsampwidth() == 2, "48 kHz・16 bit・ステレオのはず"
        rate = w.getframerate()
        raw = w.readframes(w.getnframes())
    v = struct.unpack(f"<{len(raw) // 2}h", raw)
    return rate, list(v[0::2]), list(v[1::2])


def mixed(outs, switches, n):
    """切り替えの並び [(番号, モード)] どおりにモードを替えた出力（DAC の byte の並び）。"""
    out = []
    k = 0
    for i in range(n):
        while k + 1 < len(switches) and switches[k + 1][0] <= i:
            k += 1
        out.append(outs[switches[k][1]][i])
    return out


# ---------------------------------------------------------------------------
# 本体 DAC の道

class DacRoute:
    name = "本体 DAC"
    slack = 0

    def __init__(self, left, source, taps):
        a = left[0] % 128
        self.notmul = sum(1 for s in left if (s - a) % 128)
        u = [(s - a) // 128 + 128 for s in left]
        print(f"DAC の外の一定の分: {a}、128 の倍数にならない標本: {self.notmul}")
        best = None
        for p in range(DECIM):
            groups = [u[i:i + DECIM] for i in range(p, len(u) - DECIM + 1, DECIM)]
            bad = sum(1 for g in groups if g.count(g[0]) != DECIM)
            if best is None or bad < best[2]:
                best = ([g[0] for g in groups], p, bad)
        dac, phase, self.uneven = best
        print(f"間引き: 位相 {phase}、{len(dac)} 標本（16 kHz）、3 つがそろわない組: {self.uneven}")
        thru = [source[i % len(source)] for i in range(4000)]
        j0 = next((j for j in range(len(dac) - 4000) if dac[j:j + 4000] == thru), None)
        if j0 is None:
            raise SystemExit("THRU の区間が見つからない")
        self.dsp = dac[j0:]
        self.n = len(self.dsp)
        print(f"DSP の 0 番目の標本は、DAC の {j0} 番目（{j0 / DAC_RATE * 1000:.1f} ms）。比べる標本: {self.n}")
        self.outs = model.all_outputs(taps, source, self.n)

    def track(self):
        """戻り値：切り替えの並び [(標本の番号, モード)]。"""
        mode = model.MODE_THRU
        switches = [(0, mode)]
        for i in range(self.n):
            if self.outs[mode][i] == self.dsp[i]:
                continue
            best, run = None, 0
            for m in model.MODES:
                k = 0
                while k < LOOK and i + k < self.n and self.outs[m][i + k] == self.dsp[i + k]:
                    k += 1
                if k > run:
                    best, run = m, k
            if best is not None:
                mode = best
                switches.append((i, mode))
        return switches

    def holds(self, m, t, i):
        """標本 t〜i で、モード m の出力が WAV と合う。"""
        return all(self.outs[m][k] == self.dsp[k] for k in range(t, i + 1))

    def final(self, switches):
        y = mixed(self.outs, switches, self.n)
        bad = sum(1 for a, b in zip(y, self.dsp) if a != b)
        print(f"全部の標本（{self.n}）: 合わない {bad}")
        return bad == 0 and self.notmul == 0 and self.uneven == 0


# ---------------------------------------------------------------------------
# DOC の道

TICK_NS = 125                   # 8 MHz の 1 周期
E_TICKS = 8                     # E＝8 MHz÷8
VISIT_E = 18                    # オシレータ 0 を訪れる間隔（E のサイクル。16 個＋リフレッシュ 2）
VOL = 255


def take_tick(k):
    """WAV の k 番目の標本を取る時刻（125 ns の単位）：k/48000 の後の最初の 250 ns の境。"""
    return (k * 1_000_000_000 // WAV_RATE // 250 + 1) * 2


def trunc_div(a, b):
    q = abs(a) // b
    return q if a >= 0 else -q


class DocModel:
    """DOC のチャネル 0 の値の、E のサイクルについての累積和（訪れるごとに持つ）。"""

    def __init__(self, u, eps, phase, bshift):
        self.u, self.eps, self.phase, self.bshift = u, eps, phase, bshift
        self.prefix = [0]
        self.vals = []

    def _fill(self, j):
        while len(self.vals) <= j:
            idx = (9 * len(self.vals) + self.bshift) // 32
            if idx >= len(self.u):
                raise IndexError
            v = 0 if idx < 0 else (self.u[idx] - 128) * VOL        # t0 より前は輪を埋めた $80
            self.vals.append(v)
            self.prefix.append(self.prefix[-1] + v * VISIT_E)

    def cum(self, m):
        """E のサイクル 0〜m−1 の値の和（サイクル c の値は、c 以前に最後に訪れたときの byte）。"""
        j = (m - self.phase) // VISIT_E
        if j < 0:
            return 0
        self._fill(j)
        return self.prefix[j] + (m - (self.phase + VISIT_E * j)) * self.vals[j]

    def wav(self, k):
        """WAV の k 番目の標本（整数）。範囲の外は None。"""
        m0 = (take_tick(k - 1) - self.eps) // E_TICKS + 1
        m1 = (take_tick(k) - self.eps) // E_TICKS + 1
        try:
            s = self.cum(m1) - self.cum(m0)
        except IndexError:
            return None
        return trunc_div(trunc_div(s, m1 - m0), 4)


class DocRoute:
    name = "DOC"
    slack = 2

    def __init__(self, left, source, taps):
        self.left = left
        self.n = int(len(left) / WAV_RATE * DOC_RATE) + 2
        self.outs = model.all_outputs(taps, source, self.n)
        thru = self.outs[model.MODE_THRU]
        # 1. 音の立ち上がりで大まかに、次に 15.625 kHz の階段で、DSP の 0 番目の標本を読み始める時刻 t0
        peak_w = max(abs(v) for v in left[:WAV_RATE * 2]) or 1
        k_on = next(k for k, v in enumerate(left) if abs(v) > peak_w * 0.1)
        peak_s = max(abs(v - 128) for v in thru[:DOC_RATE])
        n_on = next(i for i, v in enumerate(thru) if abs(v - 128) > peak_s * 0.1)
        t0 = k_on / WAV_RATE - n_on / DOC_RATE
        ks = range(k_on, k_on + WAV_RATE // 10)
        ys = [left[k] for k in ks]
        for step, span in ((1 / DOC_RATE / 4, 12), (1 / DOC_RATE / 32, 4)):
            cands = [t0 + j * step for j in range(-span, span + 1)]
            t0 = min(cands, key=lambda t: self._fit_error([self._box(thru, t, k) for k in ks], ys))
        # 2. E の位相（eps、phase）と b を、立ち上がりの後 300 標本で探す（WAV と整数で一致する組）
        #    j 回目の訪問は時刻 ≈ j·18 µs、読む byte は (j·18 µs − t0)·15625 ＝ (9j − 32·15625·t0)/32
        b_guess = round(-32 * DOC_RATE * t0)
        ks = range(k_on, k_on + 300)
        best = None
        for eps in range(E_TICKS):
            for phase in range(VISIT_E):
                for d in range(-48, 49):
                    dm = DocModel(thru, eps, phase, b_guess + d)
                    err = 0
                    for k in ks:
                        e = dm.wav(k)
                        err += abs(left[k] - e) if e is not None else 10**9
                        if best is not None and err >= best[0]:
                            break
                    if best is None or err < best[0]:
                        best = (err, eps, phase, b_guess + d)
        err, self.eps, self.phase, self.bshift = best
        self.t0 = -self.bshift / 32 / DOC_RATE
        print(f"DOC の格子：E の位相 {self.eps}×125 ns、オシレータ 0 を訪れるのは E の {self.phase} 番目。"
              f"DSP の 0 番目の標本を読み始めるのは {self.t0 * 1000:.3f} ms（探した 300 標本の差の和 {err}）")
        self.models = [DocModel(self.outs[m], self.eps, self.phase, self.bshift) for m in model.MODES]
        self.first = int(self.t0 * WAV_RATE) + 2

    @staticmethod
    def _box(u, t0, k):
        """u の 15.625 kHz の階段を、WAV の k 番目の標本の幅で平均する（大まかな合わせ用）。"""
        a = ((k - 1) / WAV_RATE - t0) * DOC_RATE
        b = (k / WAV_RATE - t0) * DOC_RATE
        n0 = math.floor(a)
        v0 = u[n0] - 128 if n0 >= 0 else 0
        v1 = u[n0 + 1] - 128 if n0 + 1 >= 0 else 0
        if math.floor(b) == n0:
            return v0
        return (v0 * (n0 + 1 - a) + v1 * (b - n0 - 1)) / (b - a)

    @staticmethod
    def _fit_error(xs, ys):
        sxx = sum(x * x for x in xs)
        g = sum(x * y for x, y in zip(xs, ys)) / sxx if sxx else 0.0
        return sum((y - g * x) ** 2 for x, y in zip(xs, ys))

    def ok(self, m, k):
        if k >= len(self.left):
            return None
        e = self.models[m].wav(k)
        return None if e is None else e == self.left[k]

    def index(self, k):
        """WAV の k 番目の標本の終わりに DOC が読んでいる DSP の標本の番号（おおよそ）。"""
        return math.floor((k / WAV_RATE - self.t0) * DOC_RATE)

    def wav_index(self, i):
        return math.floor((self.t0 + i / DOC_RATE) * WAV_RATE)

    def track(self):
        mode = model.MODE_THRU
        switches = [(0, mode)]
        k = self.first
        while True:
            r = self.ok(mode, k)
            if r is None:
                break
            if not r:
                # 合わない：次の標本から LOOK 標本、一番長く合うモード（この標本は 2 つのモードの間）
                best, run = None, 0
                for m in model.MODES:
                    j = 0
                    while j < LOOK and self.ok(m, k + 1 + j):
                        j += 1
                    if j > run:
                        best, run = m, j
                if best is not None and best != mode:
                    mode = best
                    switches.append((self.index(k), mode))
            k += 1
        self.end = k
        return switches

    def holds(self, m, t, i):
        """標本 t〜i（WAV で見えた切り替え）の間、モード m の出力でも WAV が合う。"""
        lo = self.wav_index(min(t, i)) + 2
        hi = self.wav_index(max(t, i)) - 1
        return all(self.ok(m, k) for k in range(lo, hi))

    def final(self, switches):
        dm = DocModel(mixed(self.outs, switches, self.n), self.eps, self.phase, self.bshift)
        bad = []
        total = 0
        for k in range(self.first, min(self.end, len(self.left))):
            e = dm.wav(k)
            if e is None:
                break
            total += 1
            if e != self.left[k]:
                bad.append(k)
        print(f"全部の標本（WAV の {total}、{total / WAV_RATE:.2f} 秒）: 合わない {len(bad)}"
              + (f"（最初は WAV の {bad[0]}、DSP の標本 {self.index(bad[0])} のあたり）" if bad else ""))
        return not bad


# ---------------------------------------------------------------------------

def exact_switches(route, switches, sws):
    """画面の SW で、WAV で見えた切り替えを本当の番号にする。戻り値：(切り替えの並び, 合わなかった数)。"""
    exact = list(switches)
    bad = 0
    for s in sws:
        hit = None
        for idx, (i, m) in enumerate(switches[1:], 1):
            # i 以前（DOC は i＋slack まで）で、下位 16 bit が s の番号
            for t in (i - ((i - s) & 0xFFFF), i + ((s - i) & 0xFFFF)):
                if i - 0x10000 < t <= i + route.slack and t >= 0 and route.holds(m, t, i):
                    hit = (idx, t, i, m)
                    break
            if hit:
                break
        if hit:
            idx, t, i, m = hit
            exact[idx] = (t, m)
            print(f"SW ${s:04X}: 標本 {t} から {model.MODE_NAMES[m]}（WAV で見えた切り替えは {i}）。合う")
        else:
            bad += 1
            print(f"SW ${s:04X}: 合う切り替えがない")
    return exact, bad


def check_sums(outs_taps_source, switches, counts, sums):
    taps, source = outs_taps_source
    m = model.Model(taps)
    k = 0
    got = {}
    bad = 0
    for i in range(max(counts)):
        while k + 1 < len(switches) and switches[k + 1][0] <= i:
            k += 1
        m.step(source[i % len(source)], switches[k][1])
        if i + 1 in counts:
            got[i + 1] = m.sum
    for c, s in zip(counts, sums + [None] * len(counts)):
        mark = "" if s is None else ("  一致" if s == got[c] else f"  画面は ${s:04X}：違う")
        if s is not None and s != got[c]:
            bad += 1
        print(f"COUNT ${c:08X} までの SUM（モデル）: ${got[c]:04X}{mark}")
    return bad


def main(argv=None):
    ap = argparse.ArgumentParser(description="WAV と画面の数をモデルと比べる")
    ap.add_argument("wav")
    ap.add_argument("--doc", action="store_true", help="DOC の子基板の道（15.625 kHz）")
    ap.add_argument("--build", default=os.path.join(HERE, "build"))
    num = lambda s: int(s, 0)  # noqa: E731
    ap.add_argument("--sw", type=num, action="append", default=[], help="画面の SW")
    ap.add_argument("--count", type=num, action="append", default=[], help="画面の COUNT（16 進は 0x を付ける）")
    ap.add_argument("--sum", type=num, action="append", default=[], help="画面の SUM（--count と同じ順）")
    args = ap.parse_args(argv)

    rate, left, right = read_wav(args.wav)
    print(f"WAV: {rate} Hz、{len(left)} 標本（{len(left) / rate:.3f} 秒）、L と R は"
          + ("同じ" if left == right else "違う"))
    source, taps = model.load_build(args.build)
    route = (DocRoute if args.doc else DacRoute)(left, source, taps)
    switches = route.track()
    print("モードの切り替え（WAV で見えたところ。DSP の標本の番号）:")
    for i, m in switches:
        print(f"  {i:7d}  ${i & 0xFFFF:04X}  {model.MODE_NAMES[m]}")
    switches, bad = exact_switches(route, switches, args.sw)
    ok = route.final(switches)
    if args.count:
        bad += check_sums((taps, source), switches, args.count, args.sum)
    # DOC では L（オシレータ 0）と R（オシレータ 1）を走らせた時刻が少し違う
    same = left == right or args.doc
    return 0 if ok and bad == 0 and same else 2


if __name__ == "__main__":
    sys.exit(main())
