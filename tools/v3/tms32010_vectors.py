#!/usr/bin/env python3
"""TMS32010 の V3（独立した実装との照合）の入力の列を作る。

乱数の命令の列（TMS32010 の命令の形を重みにして選ぶ。20 分の 1 は完全な乱数の語）と、データ RAM・
IN のポートの値・BIO を、場合ごとに決める。照合の相手（MAME の TMS320C1x、docs/impl/v3-mame.md）と
エミュレータの試験（emulator/crates/chips/tests/tms32010_v3.rs）が、同じ入力を読む。

    python tools/v3/tms32010_vectors.py OUT [件数] [歩数] [種]

形（1 行に 1 つ、値は 16 進）：
    C 番号 歩数 BIO       BIO は 1 で Low（BIOZ が飛ぶ）
    R 144 語              データ RAM
    I 8 語                IN のポート 0〜7 が返す値
    P 512 語              プログラムメモリの 0〜511 番地（512〜4095 は 0）

外部のライブラリは使わない。
"""
import random
import sys

# 命令の上位 byte（台帳 docs/parts/TMS32010.md §4 の命令の表）
MEMREF = (
    list(range(0x00, 0x30))        # ADD・SUB・LAC（ずらし 0〜15）
    + [0x30, 0x31, 0x38, 0x39]     # SAR・LAR
    + list(range(0x40, 0x51))      # IN・OUT・SACL
    + [0x58, 0x59, 0x5C]           # SACH（ずらし 0・1・4）
    + list(range(0x60, 0x70))      # ADDH〜LDP
    + [0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D]  # XOR・AND・OR・LST・SST・TBLW
)
IMMED = [0x70, 0x71, 0x7E] + list(range(0x80, 0xA0))  # LARK・LACK・MPYK
OP7F = [0x80, 0x81, 0x82, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x8F, 0x90, 0x9C, 0x9D]
BRANCH = [0xF4, 0xF5, 0xF6, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF]

PROG_WORDS = 512


def program(rng):
    words = []
    while len(words) < PROG_WORDS:
        r = rng.random()
        if r < 0.05:
            words.append(rng.getrandbits(16))
        elif r < 0.70:
            words.append(rng.choice(MEMREF) << 8 | rng.getrandbits(8))
        elif r < 0.80:
            words.append(rng.choice(IMMED) << 8 | rng.getrandbits(8))
        elif r < 0.90:
            words.append(0x7F00 | rng.choice(OP7F))
        else:
            words.append(rng.choice(BRANCH) << 8 | rng.getrandbits(8))
            words.append(rng.randrange(PROG_WORDS))
    return words[:PROG_WORDS]


def main(argv):
    out = argv[0]
    cases = int(argv[1]) if len(argv) > 1 else 5000
    steps = int(argv[2]) if len(argv) > 2 else 24
    seed = int(argv[3]) if len(argv) > 3 else 1985
    rng = random.Random(seed)
    hx = lambda ws: ' '.join(f'{w:04X}' for w in ws)
    with open(out, 'w', encoding='ascii', newline='\n') as f:
        for c in range(cases):
            f.write(f'C {c} {steps} {rng.getrandbits(1)}\n')
            f.write('R ' + hx(rng.getrandbits(16) for _ in range(144)) + '\n')
            f.write('I ' + hx(rng.getrandbits(16) for _ in range(8)) + '\n')
            f.write('P ' + hx(program(rng)) + '\n')
    print(f'{out}: {cases} 件、{steps} 命令ずつ')


if __name__ == '__main__':
    main(sys.argv[1:])
