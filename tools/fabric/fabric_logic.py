#!/usr/bin/env python3
"""Data Fabric（最終試作機）の PAL の式と PROM の中身を作り、仕様と同じ結果になるかを確かめる。

対象は docs/spec/fabric.md の §04（変換部）と §06（ポートの属性と準備の信号）。
仕様の定義は docs/spec/hardware.md §06 と DR-046。

    python tools/fabric/fabric_logic.py            # 確かめて、式を標準出力へ
    python tools/fabric/fabric_logic.py --prom DIR # PROM の中身を DIR に書く（1 行 1 番地の 16 進）

外部のライブラリは使わない。PAL の式は PALASM の書き方（/ は否定、* は AND、+ は OR）。
L 型の PAL は出力が反転するので、式の左辺を /X と書き、入力の否定を項に入れて、ピンの論理を正にする。
"""
import random
import sys
from pathlib import Path

M16 = 0xFFFF

# ---------------------------------------------------------------- 仕様の定義

IN16 = {1, 3, 4, 5}   # 入力が 16 bit の FORMAT
OUT16 = {1, 2, 5}     # 出力が 16 bit の FORMAT


def rotl(v, n):
    n &= 15
    return ((v << n) | (v >> (16 - n))) & M16 if n else v


def spec_transform(r, f, s, mask, xor):
    """仕様書 §06・DR-046 の変換。r はバスの 16 bit（8 bit のポートは下位バイトに値、上位はでたらめ）。
    s は -16〜15。返す値は出力の幅で切り捨てる（8 bit の出力は上位を 0 にする）。"""
    if f == 5:
        y = rotl(r, s)
    else:
        x = {0: r & 0xFF, 1: r, 2: r & 0xFF, 3: r & 0xFF, 4: r >> 8}[f]
        y = (x << s) & M16 if s >= 0 else x >> (-s)
    out = (y & mask) ^ xor
    return out if f in OUT16 else out & 0xFF


# ---------------------------------------------------------------- 変換部の回路

def mask_prom(f, s):
    """MSK0/1（63S281A×2）の中身。番地は SHIFT(5 bit、2 の補数) と FORMAT(3 bit)。
    値は出力の位置で有効なビット。8 bit の出力では上位 8 bit を 0 にする。"""
    m = 0
    for j in range(16):
        if f == 1:
            ok = j >= s if s >= 0 else j <= 15 + s
        elif f in (0, 2, 3, 4):
            ok = s <= j <= s + 7
        elif f == 5:
            ok = True
        else:
            ok = False
        if f not in OUT16 and j > 7:
            ok = False
        m |= ok << j
    return m


def hw_transform(r, f, s5, mask, xor):
    """回路どおりに計算する。s5 は SHIFT の 5 bit（0〜31）。"""
    s = s5 - 32 if s5 >= 16 else s5
    t = (s5 & 15) ^ (8 if f == 4 else 0)            # T3 = S3 xor F4（74LS86）
    t0, t1, t2, t3 = t & 1, t >> 1 & 1, t >> 2 & 1, t >> 3 & 1
    fine = sum(((r >> ((i - (t1 * 2 + t0)) % 16)) & 1) << i for i in range(16))      # ROT0/1
    rot = sum(((fine >> ((i - 4 * (t3 * 2 + t2)) % 16)) & 1) << i for i in range(16))  # ROT2/3
    pm = mask_prom(f, s)
    out = 0
    for i in range(16):                                                                # ANX0..3
        a = (rot >> i) & (pm >> i) & (mask >> i) & 1
        out |= (a ^ ((xor >> i) & 1)) << i
    return out


def lit(name, pos):
    return name if pos else '/' + name


def rot_pals():
    pals = []
    for p, bits in ((0, range(0, 8)), (1, range(8, 16))):       # 細かい段：0〜3 だけ左へ回す
        eqs = []
        for i in bits:
            terms = [f"{lit('T1', k >> 1)}*{lit('T0', k & 1)}*/D{(i - k) % 16}" for k in range(4)]
            eqs.append(f"/F{i} = " + " + ".join(terms))
        pals.append((f'ROT{p}', 'PAL20L8A', eqs))
    for p, cls in ((2, (0, 1)), (3, (2, 3))):                   # 粗い段：4 の倍数だけ回す
        eqs = []
        for i in [i for i in range(16) if i % 4 in cls]:
            terms = [f"{lit('T3', k >> 1)}*{lit('T2', k & 1)}*/F{(i - 4 * k) % 16}" for k in range(4)]
            eqs.append(f"/R{i} = " + " + ".join(terms))
        pals.append((f'ROT{p}', 'PAL20L8A', eqs))
    return pals


def anx_pals():
    pals = []
    for p in range(4):
        eqs = ["IF (WEN) " + f"/Q{i} = R{i}*M{i}*K{i}*X{i} + /R{i}*/X{i} + /M{i}*/X{i} + /K{i}*/X{i}"
               for i in range(4 * p, 4 * p + 4)]
        pals.append((f'ANX{p}', 'PAL20L8A', eqs))
    return pals


def eval_sop(rhs, env):
    """PALASM の積和の右辺を評価する。"""
    for term in rhs.split('+'):
        ok = True
        for lt in term.strip().split('*'):
            neg = lt.startswith('/')
            v = env[lt.lstrip('/')]
            if v == neg:
                ok = False
                break
        if ok:
            return True
    return False


def check_rot_equations():
    """ROT の式そのものを評価して、回す量 0〜15 で正しく回るかを確かめる。"""
    pals = dict((n, e) for n, _, e in rot_pals())
    rnd = random.Random(85)
    for _ in range(2000):
        d = rnd.randrange(65536)
        t = rnd.randrange(16)
        env = {f'D{i}': bool(d >> i & 1) for i in range(16)}
        env.update({f'T{k}': bool(t >> k & 1) for k in range(4)})
        for n in ('ROT0', 'ROT1'):
            for eq in pals[n]:
                lhs, rhs = eq.split(' = ')
                env[lhs.lstrip('/')] = not eval_sop(rhs, env)
        for n in ('ROT2', 'ROT3'):
            for eq in pals[n]:
                lhs, rhs = eq.split(' = ')
                env[lhs.lstrip('/')] = not eval_sop(rhs, env)
        got = sum(env[f'R{i}'] << i for i in range(16))
        assert got == rotl(d, t), (hex(d), t, hex(got))


def check_anx_equations():
    for r in (0, 1):
        for m in (0, 1):
            for k in (0, 1):
                for x in (0, 1):
                    env = {'R0': bool(r), 'M0': bool(m), 'K0': bool(k), 'X0': bool(x)}
                    rhs = anx_pals()[0][2][0].split(' = ')[1]
                    q = not eval_sop(rhs, env)
                    assert q == bool((r & m & k) ^ x)


def check_transform():
    rnd = random.Random(1985)
    n = 0
    for f in range(6):
        for s in range(-16, 16):
            vals = [rnd.randrange(65536) for _ in range(300)] + [1 << b for b in range(16)] + [0, M16]
            for r in vals:
                mask, xor = rnd.randrange(65536), rnd.randrange(65536)
                a = spec_transform(r, f, s, mask, xor)
                b = hw_transform(r, f, s & 31, mask, xor)
                if f not in OUT16:
                    b &= 0xFF      # 8 bit の出力では、上位バイト（XOR の上位）をポートが使わない
                assert a == b, (f, s, hex(r), hex(a), hex(b))
                n += 1
    return n


# ---------------------------------------------------------------- ポートの属性（ATR、63S281A×1）

# ポート：(読める, 書ける, 幅, 番地を持つ, 満杯・空の信号を持つ)。幅は 8、16、'any'。
PORTS = {
    0: (True, True, 'any', True, False),
    1: (True, True, 16, False, True),
    3: (False, True, 16, False, True),
    4: (True, False, 16, False, True),
    5: (False, True, 8, False, True),
    6: (False, True, 8, False, False),
    7: (True, False, 8, False, False),
    8: (True, False, 8, True, False),
    9: (True, False, 8, True, False),
    10: (True, False, 8, False, True),
}
for _p in range(11, 16):
    PORTS[_p] = (True, True, 'any', False, True)
PORT2_DOC = (True, True, 8, True, True)       # DOC 子基板があるとき：DOC Wave RAM
PORT2_NODOC = (False, True, 16, True, True)   # ないとき：本体 DAC の FIFO（番地は使わないが、歩進はする）

ATR_BITS = ['SONLY', 'ADDR', 'FLAG', 'OKD', 'OKN', 'W16', 'F4', 'TWO']


def port_ok(desc, w, f):
    rd, wr, width, _, _ = desc
    if f >= 6:
        return False
    if not (wr if w else rd):
        return False
    need16 = (f in OUT16) if w else (f in IN16)
    return width == 'any' or width == (16 if need16 else 8)


def atr_prom(addr):
    """番地：bit 3–0 ポート番号、bit 4 向き（0 読出し・1 書込み）、bit 7–5 FORMAT。"""
    p, w, f = addr & 15, addr >> 4 & 1, addr >> 5
    if p == 2:
        d, n = PORT2_DOC, PORT2_NODOC
    else:
        d = n = PORTS[p]
    bits = {
        'SONLY': p == 0,
        'ADDR': d[3],
        'FLAG': d[4],
        'OKD': port_ok(d, w, f),
        'OKN': port_ok(n, w, f),
        'W16': (f in OUT16) if w else (f in IN16),
        'F4': f == 4,
        'TWO': p == 9,
    }
    return sum(bits[b] << i for i, b in enumerate(ATR_BITS))


# ---------------------------------------------------------------- 準備の信号（RDY、PAL20R4A×1）

def nib(name, v, mask=0xF):
    return '*'.join(lit(f'{name}{b}', v >> b & 1) for b in (3, 2, 1, 0) if mask >> b & 1)


RDY_EQS = [
    "/SRDY := " + " + ".join([
        nib('S', 1) + '*/RDY1', nib('S', 2) + '*/RDY2', nib('S', 4) + '*/OR4',
        nib('S', 10) + '*/OR10', nib('S', 11) + '*/XRDY', nib('S', 12, 0b1100) + '*/XRDY']),
    "/DRDY := " + " + ".join([
        nib('D', 1) + '*/RDY1', nib('D', 2) + '*/RDY2', nib('D', 3) + '*/IR3',
        nib('D', 5) + '*/IR5', nib('D', 11) + '*/XRDY', nib('D', 12, 0b1100) + '*/XRDY']),
]


def ready_ref(p, w, lines):
    """そのポートに今アクセスしてよいか（仕様）。準備の信号を持たないポートは常に可。"""
    if 11 <= p <= 15:
        return lines['XRDY']
    table = {1: 'RDY1', 2: 'RDY2'}
    table.update({3: 'IR3', 5: 'IR5'} if w else {4: 'OR4', 10: 'OR10'})
    return lines[table[p]] if p in table else True


def check_rdy():
    names = ['RDY1', 'RDY2', 'OR4', 'OR10', 'IR3', 'IR5', 'XRDY']
    for v in range(1 << len(names)):
        lines = {n: bool(v >> i & 1) for i, n in enumerate(names)}
        for p in range(16):
            env = dict(lines)
            env.update({f'S{b}': bool(p >> b & 1) for b in range(4)})
            env.update({f'D{b}': bool(p >> b & 1) for b in range(4)})
            srdy = not eval_sop(RDY_EQS[0].split(' := ')[1], env)
            drdy = not eval_sop(RDY_EQS[1].split(' := ')[1], env)
            if PORTS.get(p, PORT2_DOC)[0] or p == 2:
                assert srdy == ready_ref(p, False, lines), ('S', p, lines)
            if PORTS.get(p, PORT2_DOC)[1] or p == 2:
                assert drdy == ready_ref(p, True, lines), ('D', p, lines)


# ---------------------------------------------------------------- 出力

def main(argv):
    n = check_transform()
    check_rot_equations()
    check_anx_equations()
    check_rdy()
    print(f'ok: 変換 {n} 通り、ROT・ANX・RDY の式', file=sys.stderr)
    if len(argv) >= 2 and argv[0] == '--prom':
        out = Path(argv[1])
        out.mkdir(parents=True, exist_ok=True)
        for k in (0, 1):
            rows = []
            for a in range(256):
                s5, f = a & 31, a >> 5
                s = s5 - 32 if s5 >= 16 else s5
                rows.append(f'{(mask_prom(f, s) >> (8 * k)) & 0xFF:02X}')
            (out / f'MSK{k}.hex').write_text('\n'.join(rows) + '\n', encoding='ascii')
        (out / 'ATR.hex').write_text('\n'.join(f'{atr_prom(a):02X}' for a in range(256)) + '\n',
                                     encoding='ascii')
        print(f'PROM を {out} に書いた', file=sys.stderr)
        return
    for name, part, eqs in rot_pals() + anx_pals() + [('RDY', 'PAL20R4A', RDY_EQS)]:
        print(f'; {name} ({part})')
        for e in eqs:
            print(e)
        print()


if __name__ == '__main__':
    main(sys.argv[1:])
