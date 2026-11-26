#!/usr/bin/env python3
"""ES5503 の V3（独立した実装との照合）の入力の列を作る。

場合ごとに、波形 RAM の中身（乱数の種と 0 の割合）、初めに書くレジスタ、走るスロットの数と、途中の
ホストのアクセス（レジスタの書込みと 0xE0 の読出し）を決める。照合の相手（MAME の es5503、
docs/impl/v3-mame.md）とエミュレータの試験（emulator/crates/chips/tests/es5503_v3.rs）が、同じ入力を読む。

    python tools/v3/es5503_vectors.py OUT [件数] [スロットの数] [種]

形（1 行に 1 つ、数は 16 進）：
    C 番号 スロットの数 波形の種 0の割合     0 の割合は 256 分の いくつ
    A スロット 番地 値 …                   スロットの後に書く（0 は最初のスロットの前）。複数の組を並べる
    Q スロット …                           スロットの後に 0xE0 を読む

波形 RAM（128 KiB）は、種から xorshift32 で作る（`wave_ram`。照合の両側に同じ式を持つ）。

外部のライブラリは使わない。
"""
import math
import random
import sys


def wave_ram(seed, zero):
    """xorshift32 で 128 KiB を作る。下位 8 bit が 0 の割合より小さければ 0、ほかは 8〜15 bit。"""
    x = seed or 1
    out = bytearray(0x20000)
    for i in range(0x20000):
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        b = (x >> 8) & 0xFF
        out[i] = 0 if (x & 0xFF) < zero or b == 0 else b
    return out


def osc_regs(rng, n):
    """オシレータ n の 6 つのレジスタ（番地、値）。"""
    t = rng.choice([0, 0, 1, 1, 2, 3, 4, 7])
    r = t if rng.random() < 0.7 else rng.randrange(8)
    top = 1 << (17 + r)
    # 1 回の通過が 2〜150 回の訪問になる周波数（対数で一様）
    fc = int(math.exp(rng.uniform(math.log(top / 150), math.log(top / 2))))
    fc = max(1, min(0xFFFF, fc))
    mode = rng.choice([0, 0, 1, 1, 2, 3, 3])
    ie = rng.random() < 0.6
    halt = rng.random() < 0.15
    ch = rng.randrange(16)
    ctrl = ch << 4 | (8 if ie else 0) | mode << 1 | (1 if halt else 0)
    size = (0x40 if rng.random() < 0.3 else 0) | t << 3 | r
    return [
        (0x00 | n, fc & 0xFF),
        (0x20 | n, fc >> 8),
        (0x40 | n, rng.randrange(256)),
        (0x80 | n, rng.randrange(256)),
        (0xC0 | n, size),
        (0xA0 | n, ctrl),
    ]


def main(argv):
    out = argv[0]
    cases = int(argv[1]) if len(argv) > 1 else 1000
    slots = int(argv[2]) if len(argv) > 2 else 4000
    seed = int(argv[3]) if len(argv) > 3 else 1985
    rng = random.Random(seed)
    with open(out, 'w', encoding='ascii', newline='\n') as f:
        for c in range(cases):
            n = rng.choice([1, 2, 3, 4, 4, 6, 8, 8, 16, 32])
            zero = rng.choice([0, 1, 3, 10])
            f.write(f'C {c} {slots:X} {rng.getrandbits(32):X} {zero:X}\n')
            # 初めに全部のオシレータを書き、有効数を書く
            first = []
            for i in range(32):
                first += osc_regs(rng, i)
            first.append((0xE1, (n - 1) << 1))
            f.write('A 0 ' + ' '.join(f'{a:X} {v:X}' for a, v in first) + '\n')
            # 途中のアクセス
            t = 0
            while True:
                t += rng.randrange(1, 200)
                if t >= slots:
                    break
                k = rng.random()
                if k < 0.35:
                    f.write(f'Q {t:X}\n')
                    continue
                i = rng.randrange(n + 1) & 31
                if k < 0.7:
                    reg = osc_regs(rng, i)[5]          # 制御（起動・停止・モード）
                elif k < 0.85:
                    reg = osc_regs(rng, i)[rng.randrange(5)]
                elif k < 0.9:
                    n2 = rng.choice([1, 2, 4, 8, 16, 32])
                    reg = (0xE1, (n2 - 1) << 1)
                else:
                    reg = (0xA0 | i, rng.randrange(256))
                f.write(f'A {t:X} {reg[0]:X} {reg[1]:X}\n')
    print(f'{out}: {cases} 件、{slots} スロットずつ')


if __name__ == '__main__':
    main(sys.argv[1:])
