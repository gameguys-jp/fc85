#!/usr/bin/env python3
"""MSM5205 の V3（独立した実装との照合）の入力の列を作る。

場合ごとに、S1・S2・4B のピンと、取込みごとのデータと RESET を決める。S1＝S2＝H（V̄C̄K̄ が入力）の
場合は、V̄C̄K̄ を外から動かす半周期（クロック数）も決める。照合の相手（MAME の msm5205、
docs/impl/v3-mame.md）とエミュレータの試験（emulator/crates/chips/tests/msm5205_v3.rs）が、同じ入力を読む。

    python tools/v3/msm5205_vectors.py OUT [件数] [取込みの数] [種]

形（1 行に 1 つ）：
    C 番号 S1 S2 4B 取込みの数 半周期     半周期は S1＝S2＝1 のときだけ使う（ほかは 0）
    D 語 …                               取込みの数＋1 個。上の桁が RESET、下の桁が D3〜D0（16 進 2 桁）

どちらの側も、0 番目の語を初めに入れ、最初の V̄C̄K̄ の立下りの後の立上りごとに次の語を入れる。
3-bit モード（4B＝0）では D3〜D1 だけが意味を持つ。

外部のライブラリは使わない。
"""
import random
import sys


def words(rng, n):
    """取込みの数＋1 個の語。乱数のほか、予測値を端まで振る列（あふれの扱いを比べる）を混ぜる。"""
    kind = rng.random()
    out = []
    for i in range(n + 1):
        if kind < 0.5:
            d = rng.getrandbits(4)
        elif kind < 0.8:
            # 同じ向きの大きい差分を続けて、予測値を端まで振る
            up = (i // rng.choice([20, 40, 80])) % 2 == 0
            d = (0x0 if up else 0x8) | rng.choice([4, 5, 6, 7, 7, 7])
        else:
            # ステップ番号を下げる小さい差分と、大きい差分の混ぜ合わせ
            d = rng.choice([0, 1, 2, 3, 8, 9, 0xA, 0xB, 7, 0xF])
        r = 1 if rng.random() < 0.02 else 0
        out.append(r << 4 | d)
    return out


def main(argv):
    out = argv[0]
    cases = int(argv[1]) if len(argv) > 1 else 2000
    n = int(argv[2]) if len(argv) > 2 else 300
    seed = int(argv[3]) if len(argv) > 3 else 1985
    rng = random.Random(seed)
    with open(out, 'w', encoding='ascii', newline='\n') as f:
        for c in range(cases):
            s1, s2 = rng.getrandbits(1), rng.getrandbits(1)
            four = 1 if rng.random() < 0.8 else 0
            half = rng.randrange(3, 200) if s1 and s2 else 0
            f.write(f'C {c} {s1} {s2} {four} {n} {half}\n')
            f.write('D ' + ' '.join(f'{w:02X}' for w in words(rng, n)) + '\n')
    print(f'{out}: {cases} 件、取込み {n} 回ずつ')


if __name__ == '__main__':
    main(sys.argv[1:])
