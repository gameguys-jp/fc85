#!/usr/bin/env python3
"""adpcm: OKI MSM5205 の 4 bit ADPCM の符号器と復号器。Python 3 の標準ライブラリだけを使う。

復号は台帳 docs/parts/MSM5205.md §4.2 の式で、エミュレータの
emulator/crates/chips/src/msm5205.rs（`Msm5205::capture`、`diff`）と同じ結果を出す。

- 予測値 X は 12 bit（−2048〜+2047）。あふれたときは回り込む（DR-021。本機の標準）。比較用にクリップも選べる。
- ステップ値 s ＝ floor(16 × 1.1^i)、i ＝ 0〜48。差分 Δ ＝ ±(s×B2 ＋ ⌊s/2⌋×B1 ＋ ⌊s/4⌋×B0 ＋ ⌊s/8⌋)。
- 番号 i は大きさ 0〜3 で −1、4〜7 で ＋2・＋4・＋6・＋8。0〜48 に制限する。
- DAC のコードは X の下位 2 bit を捨てた 10 bit。
- RESET の後は X＝0、i＝0。

符号器は、復号器の状態を写しに持ち、次の 16 通りのニブルを全部試して目標との差がいちばん小さいものを選ぶ
（depth＝2 なら 2 個先まで見る）。回り込むニブルは選ばない（回り込むと大きな音のひずみになる。台帳 §4.4）。

本機の ADPCM bridge（DR-017、DR-053）は byte の上位のニブルから使う（CTRL の bit 3 で下位から）。`pack` の
既定はこれに合わせる。

使い方（コマンド）:
    python adpcm.py encode in.wav out.bin [--rate 8000]   16 bit・モノの WAV を符号にする（標本化の変換はしない）
    python adpcm.py decode in.bin out.wav [--rate 8000]   符号を 16 bit・モノの WAV にする（DAC のコード×64）
"""

import argparse
import math
import struct
import sys
import wave

__all__ = [
    "STEP_TABLE", "INDEX_SHIFT", "SIGNAL_MIN", "SIGNAL_MAX", "UNDERRUN_BYTE",
    "diff", "Decoder", "decode", "dac_codes", "encode", "pack", "unpack", "snr_db",
]

# 台帳 §4.2 のステップ値 floor(16 × 1.1^i)、i = 0〜48（msm5205.rs の STEP_TABLE と同じ）。
STEP_TABLE = (
    16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130,
    143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796,
    876, 963, 1060, 1166, 1282, 1411, 1552,
)

# 大きさ（B2 B1 B0）ごとのステップ番号の増減（台帳 §4.2）。
INDEX_SHIFT = (-1, -1, -1, -1, 2, 4, 6, 8)

SIGNAL_MIN = -2048
SIGNAL_MAX = 2047

# ADPCM の FIFO が空のときに bridge が使う byte（DR-017 (2)。ニブル 0000 と 1000 が交互）。
UNDERRUN_BYTE = 0x08


def diff(step, nibble):
    """差分 Δ（台帳 §4.2、msm5205.rs の `diff`）。"""
    s = STEP_TABLE[step]
    d = s // 8
    if nibble & 4:
        d += s
    if nibble & 2:
        d += s // 2
    if nibble & 1:
        d += s // 4
    return -d if nibble & 8 else d


# diff の表：_DIFF[step][nibble]
_DIFF = tuple(tuple(diff(i, n) for n in range(16)) for i in range(49))
# 次のステップ番号：_NEXT_STEP[step][nibble]
_NEXT_STEP = tuple(
    tuple(min(48, max(0, i + INDEX_SHIFT[n & 7])) for n in range(16)) for i in range(49)
)


def _wrap(x):
    return ((x + 2048) % 4096) - 2048


def _clip(x):
    return max(SIGNAL_MIN, min(SIGNAL_MAX, x))


class Decoder:
    """MSM5205 の復号器（4 bit モード）。`overflow` は "wrap"（標準、DR-021）か "clip"（MAME と同じ、比較用）。"""

    def __init__(self, overflow="wrap"):
        if overflow not in ("wrap", "clip"):
            raise ValueError("overflow は wrap か clip")
        self.overflow = overflow
        self.signal = 0
        self.step = 0

    def reset(self):
        """RESET の取込み：予測値とステップ番号を 0 にする。"""
        self.signal = 0
        self.step = 0

    def feed(self, nibble):
        """1 つのニブルを取り込み、取込み後の予測値を返す。"""
        nibble &= 0x0F
        x = self.signal + _DIFF[self.step][nibble]
        self.signal = _wrap(x) if self.overflow == "wrap" else _clip(x)
        self.step = _NEXT_STEP[self.step][nibble]
        return self.signal

    def dac_code(self):
        """DAC に渡る 10 bit のコード（−512〜+511）。"""
        return self.signal >> 2


def decode(nibbles, overflow="wrap", signal=0, step=0):
    """ニブルの列を復号し、予測値の列を返す。既定は RESET の直後（X＝0、i＝0）から。"""
    d = Decoder(overflow)
    d.signal, d.step = signal, step
    return [d.feed(n) for n in nibbles]


def dac_codes(nibbles, overflow="wrap"):
    """ニブルの列を復号し、DAC のコード（予測値 >> 2）の列を返す。"""
    return [s >> 2 for s in decode(nibbles, overflow)]


def encode(samples, depth=1, signal=0, step=0, limit=SIGNAL_MAX):
    """目標の予測値の列（12 bit の範囲の整数）をニブルの列にする。

    復号器の状態（signal、step。既定は RESET の直後）を写しに持ち、16 通りのニブルを試して二乗誤差の
    小さいものを選ぶ。depth＝2 なら次の標本も合わせて最小にする（遅いが少し良い）。予測値が ±limit を
    超えるニブルは選ばない（回り込みを起こさない）。
    """
    if depth not in (1, 2):
        raise ValueError("depth は 1 か 2")
    targets = [int(round(t)) for t in samples]
    lo, hi = -limit - 1, limit
    out = []
    sig, st = signal, step
    n_targets = len(targets)
    for k in range(n_targets):
        t0 = targets[k]
        t1 = targets[k + 1] if depth == 2 and k + 1 < n_targets else None
        best = None
        best_cost = None
        row = _DIFF[st]
        for n in range(16):
            s1 = sig + row[n]
            if s1 < lo or s1 > hi:
                continue
            cost = (t0 - s1) * (t0 - s1)
            if t1 is not None:
                st1 = _NEXT_STEP[st][n]
                row1 = _DIFF[st1]
                m = None
                for n2 in range(16):
                    s2 = s1 + row1[n2]
                    if s2 < lo or s2 > hi:
                        continue
                    e = (t1 - s2) * (t1 - s2)
                    if m is None or e < m:
                        m = e
                if m is None:
                    continue
                cost += m
            if best_cost is None or cost < best_cost:
                best, best_cost = n, cost
        if best is None:
            # どのニブルでも範囲を出る（ありえないが、念のため 0 に向かう最小の差分）
            best = 8 if sig > 0 else 0
        out.append(best)
        sig = sig + row[best]
        st = _NEXT_STEP[st][best]
    return out


def pack(nibbles, low_first=False, pad=0x0):
    """ニブルの列を byte にする。既定は上位のニブルが先（DR-017 (3)）。奇数個なら pad を足す。"""
    ns = list(nibbles)
    if len(ns) % 2:
        ns.append(pad)
    out = bytearray()
    for i in range(0, len(ns), 2):
        a, b = ns[i] & 15, ns[i + 1] & 15
        out.append((b << 4) | a if low_first else (a << 4) | b)
    return bytes(out)


def unpack(data, low_first=False):
    """byte の列をニブルの列にする（`pack` の逆）。"""
    out = []
    for v in data:
        hi, lo = v >> 4, v & 15
        out.extend((lo, hi) if low_first else (hi, lo))
    return out


def snr_db(reference, test):
    """信号対雑音比（dB）。"""
    ps = sum(r * r for r in reference)
    pn = sum((r - t) * (r - t) for r, t in zip(reference, test))
    if pn == 0:
        return math.inf
    return 10 * math.log10(ps / pn)


# ---------------------------------------------------------------------------
# コマンド


def _read_wav(path):
    with wave.open(path, "rb") as w:
        if w.getsampwidth() != 2 or w.getnchannels() != 1:
            raise SystemExit("16 bit・モノの WAV だけを読む")
        rate = w.getframerate()
        raw = w.readframes(w.getnframes())
    pcm = struct.unpack(f"<{len(raw) // 2}h", raw)
    return rate, [v / 16 for v in pcm]  # 16 bit → 12 bit


def _write_wav(path, rate, values):
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(struct.pack(f"<{len(values)}h", *values))


def main(argv=None):
    ap = argparse.ArgumentParser(description="MSM5205 の 4 bit ADPCM")
    sub = ap.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("encode", help="16 bit・モノの WAV → ADPCM（上位のニブルが先）")
    e.add_argument("input")
    e.add_argument("output")
    e.add_argument("--depth", type=int, default=1)
    d = sub.add_parser("decode", help="ADPCM → 16 bit・モノの WAV")
    d.add_argument("input")
    d.add_argument("output")
    d.add_argument("--rate", type=int, default=8000)
    args = ap.parse_args(argv)
    if args.cmd == "encode":
        rate, samples = _read_wav(args.input)
        data = pack(encode(samples, depth=args.depth))
        with open(args.output, "wb") as f:
            f.write(data)
        print(f"{len(samples)} 標本（{rate} Hz）→ {len(data)} byte")
    else:
        with open(args.input, "rb") as f:
            codes = dac_codes(unpack(f.read()))
        _write_wav(args.output, args.rate, [c * 64 for c in codes])
        print(f"{len(codes)} 標本")
    return 0


if __name__ == "__main__":
    sys.exit(main())
