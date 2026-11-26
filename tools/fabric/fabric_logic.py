#!/usr/bin/env python3
"""Data Fabric（最終試作機）の PAL の式と PROM の中身を確かめ、エミュレータの Rust を作る。

PAL の式の元は `tools/fabric/pal/*.pal`（PALASM。書き方は palasm.py）。PAL に焼く式そのものを読み、
評価して、仕様の定義（docs/spec/hardware.md §06、DR-046）と突き合わせる。docs/spec/fabric.md の
式のコードブロックは、このファイルの中身と同じにする（--check で確かめる）。PROM（MSK0・MSK1、ATR）の
中身は、このファイルの関数で作る。

    py tools/fabric/fabric_logic.py            # 確かめて、ピンと項の使い方を標準出力へ
    py tools/fabric/fabric_logic.py --rust     # 確かめて、emulator/crates/board/src/fabric_pal.rs を書く
    py tools/fabric/fabric_logic.py --check    # 生成した Rust と fabric.md の式が今の式と同じか
    py tools/fabric/fabric_logic.py --prom DIR # PROM の中身を DIR に書く（1 行 1 番地の 16 進）

外部のライブラリは使わない。
"""
import random
import re
import sys
from pathlib import Path

import palasm

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
PAL_DIR = HERE / 'pal'
# fabric.md の式のコードブロックの順。
PAL_FILES = ['transform.pal', 'afc.pal', 'rdy.pal', 'seq.pal']
RUST_OUT = ROOT / 'emulator' / 'crates' / 'board' / 'src' / 'fabric_pal.rs'
FABRIC_MD = ROOT / 'docs' / 'spec' / 'fabric.md'
REGEN = 'py tools/fabric/fabric_logic.py --rust'

M16 = 0xFFFF


def load_pals():
    """PAL の名前 → Pal（ファイルの順）。"""
    pals = {}
    for f in PAL_FILES:
        for p in palasm.load(PAL_DIR / f):
            if p.name in pals:
                raise palasm.PalError(f'{p.name} が 2 回ある')
            pals[p.name] = p
    return pals


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


def msk_rom(k):
    """MSK0（k=0、出力の bit 0–7）・MSK1（k=1、bit 8–15）の 256 番地。番地は SHIFT(bit 4–0) と FORMAT(bit 7–5)。"""
    out = []
    for a in range(256):
        s5, f = a & 31, a >> 5
        s = s5 - 32 if s5 >= 16 else s5
        out.append((mask_prom(f, s) >> (8 * k)) & 0xFF)
    return out


def comb_bus(c, buses, out):
    """組合せの PAL を評価して、出力の bus の値と、出している bit を返す。"""
    comb, oe, _ = c.run(c.pack(buses))
    return c.unpack(comb, out), c.unpack(oe, out)


def pal_transform(cp, r, f, s5, mask, xor, wen=True):
    """式そのものを評価して、W のスロットで FD に出る 16 bit を返す。cp は PAL の名前 → Compiled。
    T3 = S3 xor F4（74LS86）。返す値は（FD の値、出している bit）。"""
    t = (s5 & 15) ^ (8 if f == 4 else 0)
    fv = comb_bus(cp['ROT0'], {'d': r, 't': t}, 'f')[0] | comb_bus(cp['ROT1'], {'d': r, 't': t}, 'f')[0]
    rv = comb_bus(cp['ROT2'], {'f': fv, 't': t}, 'r')[0] | comb_bus(cp['ROT3'], {'f': fv, 't': t}, 'r')[0]
    a = s5 | f << 5
    m = MSK[0][a] | MSK[1][a] << 8
    q = oe = 0
    for k in range(4):
        v, e = comb_bus(cp[f'ANX{k}'], {'r': rv, 'm': m, 'k': mask, 'x': xor, 'wen': wen}, 'q')
        q |= v
        oe |= e
    return q, oe


MSK = [msk_rom(0), msk_rom(1)]


def compiled(pals):
    return {n: palasm.Compiled(p) for n, p in pals.items()}


def check_transform(pals):
    cp = compiled(pals)
    rnd = random.Random(1985)
    n = 0
    for f in range(6):
        for s in range(-16, 16):
            vals = [rnd.randrange(65536) for _ in range(300)] + [1 << b for b in range(16)] + [0, M16]
            for r in vals:
                mask, xor = rnd.randrange(65536), rnd.randrange(65536)
                a = spec_transform(r, f, s, mask, xor)
                b, oe = pal_transform(cp, r, f, s & 31, mask, xor)
                assert oe == M16, (f, s, hex(oe))
                if f not in OUT16:
                    b &= 0xFF      # 8 bit の出力では、上位バイト（XOR の上位）をポートが使わない
                assert a == b, (f, s, hex(r), hex(a), hex(b))
                n += 1
    return n


def check_rot_equations(pals):
    """ROT の式そのものを評価して、回す量 0〜15 で正しく回るかを確かめる。"""
    cp = compiled(pals)
    rnd = random.Random(85)
    for _ in range(2000):
        d = rnd.randrange(65536)
        t = rnd.randrange(16)
        fv = comb_bus(cp['ROT0'], {'d': d, 't': t}, 'f')[0] | comb_bus(cp['ROT1'], {'d': d, 't': t}, 'f')[0]
        rv = comb_bus(cp['ROT2'], {'f': fv, 't': t}, 'r')[0] | comb_bus(cp['ROT3'], {'f': fv, 't': t}, 'r')[0]
        assert rv == rotl(d, t), (hex(d), t, hex(rv))


def check_anx_equations(pals):
    """ANX0〜3 の 4 bit ずつを、1 bit の 32 通り（R、M、K、X、WEN）で確かめる。"""
    cp = compiled(pals)
    for k in range(4):
        for v in range(32):
            r, m, kk, x, wen = (v & 1, v >> 1 & 1, v >> 2 & 1, v >> 3 & 1, v >> 4 & 1)
            for b in range(4 * k, 4 * k + 4):
                q, oe = comb_bus(cp[f'ANX{k}'], {'r': r << b, 'm': m << b, 'k': kk << b, 'x': x << b,
                                                 'wen': wen}, 'q')
                assert bool(q >> b & 1) == bool((r & m & kk) ^ x), (k, b, v)
                assert bool(oe >> b & 1) == bool(wen), (k, b, v)


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

def ready_ref(p, w, lines):
    """そのポートに今アクセスしてよいか（仕様）。準備の信号を持たないポートは常に可。"""
    if 11 <= p <= 15:
        return lines['XRDY']
    table = {1: 'RDY1', 2: 'RDY2'}
    table.update({3: 'IR3', 5: 'IR5'} if w else {4: 'OR4', 10: 'OR10'})
    return lines[table[p]] if p in table else True


def check_rdy(pals):
    p = pals['RDY']
    names = ['RDY1', 'RDY2', 'OR4', 'OR10', 'IR3', 'IR5', 'XRDY']
    for v in range(1 << len(names)):
        lines = {n: bool(v >> i & 1) for i, n in enumerate(names)}
        for port in range(16):
            env = dict(lines)
            env.update({f'S{b}': bool(port >> b & 1) for b in range(4)})
            env.update({f'D{b}': bool(port >> b & 1) for b in range(4)})
            # READY は取り込んだ 2 つの準備の AND（レジスタの今の値）。
            env.update(SRDY=bool(v & 1), DRDY=bool(port & 1))
            comb, _, nxt = p.eval(env)
            assert comb['READY'] == (env['SRDY'] and env['DRDY'])
            if PORTS.get(port, PORT2_DOC)[0] or port == 2:
                assert nxt['SRDY'] == ready_ref(port, False, lines), ('S', port, lines)
            if PORTS.get(port, PORT2_DOC)[1] or port == 2:
                assert nxt['DRDY'] == ready_ref(port, True, lines), ('D', port, lines)


# ---------------------------------------------------------------- A ファイルの操作（AFC0・AFC1）

# AOP の番号（SEQ の AOP2–AOP0）。
AOPS = {'NONE': 0, 'HOST': 1, 'SST': 2, 'DST': 3, 'CNT': 4, 'R0': 5, 'R1': 6, 'R2': 7}
# CPU の窓の語（HA3、HA2）→ A ファイルの語の番号（設定値。作業値は +1）。fabric.md §05。
HOST_WORDS = {(0, 1): 0, (1, 0): 2, (1, 1): 4}


def afc_expect(aop, ch, hch, word, hrw):
    """fabric.md §05 の表：（読む語、書く語か None、B の番地か None、Cin、取込み）。語は ch×8 + 番号。"""
    if aop == AOPS['HOST']:
        if hrw:
            return hch * 8 + word + 1, None, None, None, 'HRC'
        return None, hch * 8 + word, None, None, None
    table = {'SST': (1, 1, 0, 0, 'PSC'), 'DST': (3, 3, 1, 0, 'PDC'), 'CNT': (5, 5, None, 0, None),
             'R0': (0, 1, None, 1, None), 'R1': (2, 3, None, 1, None), 'R2': (4, 5, None, 0, None)}
    name = [k for k, v in AOPS.items() if v == aop][0]
    if name == 'NONE':
        return None, None, None, None, None
    r, w, b, cin, cap = table[name]
    return ch * 8 + r, ch * 8 + w, (None if b is None else ch * 2 + b), cin, cap


def check_afc(pals):
    """AFC0・AFC1 の式が、AOP ごとに §05 の表の語を読み書きし、B と Cin と取込みを選ぶか。
    A ファイルの番地は負の論理のまま 74S189 へ行くので、語の番号は番地のピンの反転。"""
    a0, a1 = pals['AFC0'], pals['AFC1']
    for aop in range(8):
        for ch in (0, 1):
            for hch in (0, 1):
                for (ha3, ha2), word in HOST_WORDS.items():
                    for hrw in (0, 1):
                        env = {'AOP2N': not aop >> 2 & 1, 'AOP1N': not aop >> 1 & 1, 'AOP0N': not aop & 1,
                               'ACHN': not ch, 'HCH': bool(hch), 'HA3': bool(ha3), 'HA2': bool(ha2),
                               'HRW': bool(hrw)}
                        pins = []
                        for wph in (False, True):
                            c0, _, _ = a0.eval(dict(env, WPH=wph))
                            c1, _, _ = a1.eval(dict(env, WPH=wph))
                            pins.append((c0, c1))
                        (r0, r1), (w0, w1) = pins
                        word_of = (lambda c: (~(c['AA3N'] << 3 | c['AA2N'] << 2 | c['AA1N'] << 1 | c['AA0N'])) & 15)
                        rd, wr, b, cin, cap = afc_expect(aop, ch, hch, word, hrw)
                        what = (aop, ch, hch, word, hrw)
                        if rd is not None:
                            assert word_of(r0) == rd, ('読む語', what)
                        assert r0['AWEN'], ('読みの相で書いた', what)
                        assert (not w0['AWEN']) == (wr is not None), ('書込み', what)
                        if wr is not None:
                            assert word_of(w0) == wr, ('書く語', what)
                            assert (not w1['SLOEN']) == (aop != AOPS['HOST']), ('和のラッチ', what)
                            assert (not w1['HWOEN']) == (aop == AOPS['HOST']), ('CPU の書込み', what)
                        assert (not r1['BENN']) == (b is not None), ('B', what)
                        if b is not None:
                            assert (r1['BA1'] << 1 | r1['BA0']) == b, ('B の番地', what)
                        if cin is not None:
                            assert r0['CIN'] == bool(cin), ('Cin', what)
                        for k in ('PSC', 'PDC', 'HRC'):
                            assert (not r1[k + 'N']) == (cap == k), (k, what)


# ---------------------------------------------------------------- 順序と状態（seq.pal）
#
# 式の全体は、エミュレータの動作の模型（fabric.rs）と門の模型（board の試験の fabric_gate）を並べて動かす試験
# （emulator/crates/board/tests/fabric_lockstep.rs）で突き合わせる。ここでは、チャネルの段階とトリガの
# 状態の機械と、AOP の番号を、届きうる入力の全部で確かめる。

# 段階（P2、P1、P0）。
PHASES = {'I': (0, 0, 0), 'R0': (1, 0, 0), 'R1': (1, 0, 1), 'R2': (1, 1, 0), 'RUN': (1, 1, 1), 'NM': (0, 1, 1)}


def spec_chs(ph, ok, done, err, rl, cnt, cy, oks, blk, kil, er, st, lp, wd, we):
    """チャネルの段階の次（fabric.rs の ctrl・exec_aop の再設定と COUNT・block_end・kill）。
    返す値は（段階、OK（再設定の途中だけ意味がある）、DONE、ERROR）。"""
    set_done = set_err = False
    nok = None
    if kil:
        nxt = 'I'
    elif rl and (ph == 'R0' or st):
        nxt, nok = 'R1', oks
    elif rl and ph == 'R1':
        nxt, nok = 'R2', ok and oks
    elif rl and ph == 'R2':
        if not ok:
            nxt, set_err = 'I', True
        elif not cy:
            nxt, set_done = ('R0' if lp else 'I'), True
        else:
            nxt = 'RUN'
    elif st:
        nxt = 'R0'
    elif blk:
        nxt, set_done = ('R0' if lp else 'I'), True
    elif cnt and not cy and ph == 'RUN':
        nxt = 'NM'
    else:
        nxt = ph
    return nxt, nok, (done and not wd) or set_done, (err and not we) or set_err or er


def check_chs(pals):
    p = pals['CHS']
    names = ['ok', 'done', 'err', 'rl', 'cnt', 'cy', 'oks', 'blk', 'kil', 'er', 'st', 'lp', 'wd', 'we']
    n = 0
    for ph, (p2, p1, p0) in PHASES.items():
        for v in range(1 << len(names)):
            x = {k: bool(v >> i & 1) for i, k in enumerate(names)}
            # 届かない組：START の要求は止まっているときだけ。再設定は再設定の段か START のとき、COUNT と
            # ブロックの終わりは動作中だけ（再設定とは同じスロットにない）。ERROR の出来事は止める出来事。
            # 止める出来事は、再設定のスロットとブロックの終わりには重ならない。
            if (x['st'] and ph != 'I') or (x['rl'] and ph not in ('R0', 'R1', 'R2') and not x['st']):
                continue
            if (x['cnt'] or x['blk']) and (ph not in ('RUN', 'NM') or x['rl']):
                continue
            if (x['er'] and not x['kil']) or (x['kil'] and (x['rl'] or x['blk'])):
                continue
            env = {'P2N': not p2, 'P1N': not p1, 'P0N': not p0, 'OKN': not x['ok'], 'DONEN': not x['done'],
                   'ERRN': not x['err'], 'RLDN': not x['rl'], 'CNTN': not x['cnt'], 'CY': x['cy'], 'OKS': x['oks'],
                   'BLKN': not x['blk'], 'KILN': not x['kil'], 'ERN': not x['er'], 'STRQ': x['st'],
                   'LOOP': x['lp'], 'WDQ': x['wd'], 'WEQ': x['we']}
            comb, _, nxt = p.eval(env)
            want, nok, done, err = spec_chs(ph, **x)
            got = (not nxt['P2N'], not nxt['P1N'], not nxt['P0N'])
            what = (ph, x)
            assert got == PHASES[want], ('段階', what, got, want)
            if nok is not None:
                assert (not nxt['OKN']) == nok, ('OK', what)
            assert (not nxt['DONEN'], not nxt['ERRN']) == (done, err), ('DONE・ERROR', what)
            assert (not comb['NBSYN']) == (want != 'I'), ('NBSY', what)
            assert (not comb['NRUNN']) == (want in ('RUN', 'NM')), ('NRUN', what)
            n += 1
    return n


def check_sts(pals):
    """トリガの同期化と立上り、保留、許可、MISS、STALL（fabric.rs の sync_trigger と decide の STALL）。"""
    p = pals['STS']
    names = ['s1', 'syn2', 'rise', 'arm', 'pend', 'miss', 'stall', 'nbusy', 'nrun', 'st', 'ste', 'wm', 'imm', 'sv',
             'stl', 'p1', 'p0']
    for v in range(1 << len(names)):
        x = {k: bool(v >> i & 1) for i, k in enumerate(names)}
        # 次が動作なら次は止まっていない。START の要求は、止まっているときだけで、次は再設定。
        if (x['nrun'] and not x['nbusy']) or (x['st'] and (x['nrun'] or not x['nbusy'] or x['p1'] or x['p0'])):
            continue
        env = {'S1Q': x['s1'], 'SYN2N': not x['syn2'], 'RISEN': not x['rise'], 'ARMN': not x['arm'],
               'PENDN': not x['pend'], 'MISSN': not x['miss'], 'STALN': not x['stall'], 'NBSYN': not x['nbusy'],
               'NRUNN': not x['nrun'], 'STRQ': x['st'], 'STEN': not x['ste'], 'WMQ': x['wm'], 'IMM': x['imm'],
               'SVN': not x['sv'], 'STLN': not x['stl'], 'P1N': not x['p1'], 'P0N': not x['p0']}
        comb, _, nxt = p.eval(env)
        pend = x['pend'] and not x['st']
        rising = x['rise'] and x['nbusy']
        miss = (x['miss'] and not x['wm']) or (rising and pend)
        pend = pend or rising
        armed = x['arm'] and not x['ste']
        arm = x['nrun'] and (armed or pend)
        if arm and not armed:
            pend = False
        stall = x['nrun'] and (x['stl'] if x['sv'] else x['stall'])
        want = {'SYN2N': not x['s1'], 'RISEN': not (x['s1'] and not x['syn2']), 'ARMN': not arm, 'PENDN': not pend,
                'MISSN': not miss, 'STALN': not stall}
        for k, w in want.items():
            assert nxt[k] == w, (k, x)
        assert (not comb['ARMXN']) == (x['imm'] or x['arm']), ('ARMX', x)
        assert (not comb['WAITN']) == (x['p1'] and x['p0'] and not x['imm'] and not x['arm']), ('WAIT', x)


def check_aop(pals):
    """SEQ の AOP：ELG の操作の種類（再設定は 101）と、候補の段階（CK1・CK0）から。"""
    p = pals['SEQ']
    stages = {(0, 0): AOPS['R0'], (0, 1): AOPS['R1'], (1, 0): AOPS['R2']}
    for cls in (0, 1, 2, 3, 4, 5):
        for (k1, k0), r in stages.items():
            env = {n: True for n in p.inputs() + p.feedback()}
            env.update(AC2N=not cls >> 2 & 1, AC1N=not cls >> 1 & 1, AC0N=not cls & 1, CK1N=not k1, CK0N=not k0)
            _, _, nxt = p.eval(env)
            aop = (not nxt['AOP2N']) << 2 | (not nxt['AOP1N']) << 1 | (not nxt['AOP0N'])
            assert aop == (r if cls == 5 else cls), (cls, k1, k0, aop)


def check_all(pals=None):
    pals = pals or load_pals()
    n = check_transform(pals)
    check_rot_equations(pals)
    check_anx_equations(pals)
    check_rdy(pals)
    check_afc(pals)
    check_chs(pals)
    check_sts(pals)
    check_aop(pals)
    return n


# ---------------------------------------------------------------- 出力

def rust_source(pals):
    out = [
        '//! Data Fabric の PAL の式と PROM の中身（生成したファイル。手で直さない）。',
        '//!',
        '//! 元は `tools/fabric/pal/*.pal`（PALASM の式。`docs/spec/fabric.md` の式と同じ）と、',
        '//! `tools/fabric/fabric_logic.py` の PROM の中身。作り直すには、リポジトリの根で',
        f'//! `{REGEN}` を実行する。式と同じかは `py -m unittest discover -s tools/fabric` で確かめる。',
        '//!',
        '//! PAL は、入力の bus（名前の末尾の数字が bit）と 1 本の信号（bool）を受けて、出力のピンの値を返す。',
        '//! 出力は反転する（`/X = 積和` のピン X は積和の否定）。登録出力はクロックの後の値（D の入力）を返し、',
        '//! 帰還はレジスタの今の値を引数で受ける。3 ステートの出力は、出しているかを `*_oe` で返す。',
        '',
    ]
    for p in pals.values():
        out.append(palasm.rust_fn(p))
        out.append('')
    out.append(palasm.rust_const_u8(
        'MSK0', 'MSK0（63S281A）：出力の bit 0–7 の有効なビット。番地は SHIFT（bit 4–0）と FORMAT（bit 7–5）。',
        MSK[0]))
    out.append('')
    out.append(palasm.rust_const_u8('MSK1', 'MSK1（63S281A）：出力の bit 8–15 の有効なビット。番地は MSK0 と同じ。',
                                    MSK[1]))
    out.append('')
    out.append(palasm.rust_const_u8(
        'ATR', 'ATR（63S281A）：ポートの属性。番地は bit 3–0 ポート、bit 4 向き（1 で書込み）、bit 7–5 FORMAT。',
        [atr_prom(a) for a in range(256)]))
    out.append('')
    for i, b in enumerate(ATR_BITS):
        out.append(f'/// ATR の出力 {b}（bit {i}）。')
        out.append(f'pub const ATR_{b}: u8 = 1 << {i};')
    return '\n'.join(out) + '\n'


def doc_blocks(text):
    """fabric.md の、PAL の見出しで始まるコードブロック。"""
    blocks = re.findall(r'^```\n(.*?)^```$', text, flags=re.S | re.M)
    return [b for b in blocks if palasm.HEADER.match(b)]


def stale():
    """生成した Rust と fabric.md の式が、今の式と違えばその説明を返す。"""
    pals = load_pals()
    msgs = []
    want = rust_source(pals)
    have = RUST_OUT.read_bytes().decode('utf-8') if RUST_OUT.exists() else ''
    if want != have:
        msgs.append(f'{RUST_OUT.relative_to(ROOT).as_posix()} が古い（{REGEN}）')
    blocks = doc_blocks(FABRIC_MD.read_text(encoding='utf-8'))
    files = [(PAL_DIR / f).read_text(encoding='utf-8') for f in PAL_FILES]
    if len(blocks) != len(files):
        msgs.append(f'fabric.md の式のブロックが {len(blocks)} 個（ファイルは {len(files)} 個）')
    for f, b, t in zip(PAL_FILES, blocks, files):
        if b != t:
            msgs.append(f'fabric.md の式が pal/{f} と違う')
    return msgs


def fit_table(pals):
    rows = ['| PAL | 型番 | クロック | 入力 | 出力 | 積（最大） | 空きピン |', '|---|---|---|---:|---:|---:|---:|']
    for p in pals.values():
        u = p.usage()
        limit = max(palasm.TERMS[p.roles[p.pin_of(e.out) - 1]] for e in p.eqs)
        rows.append(f'| {p.name} | {p.part}{p.grade} | {u["clock"] or "なし"} | {u["inputs"]} | {u["outputs"]} | '
                    f'{u["max_terms"]}／{limit} | {u["free_pins"]} |')
    return '\n'.join(rows)


def main(argv):
    pals = load_pals()
    n = check_all(pals)
    print(f'ok: 変換 {n} 通り、ROT・ANX・RDY・AFC の式、CHS・STS の状態、SEQ の AOP', file=sys.stderr)
    if '--check' in argv:
        msgs = stale()
        for m in msgs:
            print(m, file=sys.stderr)
        sys.exit(1 if msgs else 0)
    if '--rust' in argv:
        with open(RUST_OUT, 'w', encoding='utf-8', newline='\n') as f:
            f.write(rust_source(pals))
        print(f'{RUST_OUT.relative_to(ROOT).as_posix()} を書いた', file=sys.stderr)
        return
    if len(argv) >= 2 and argv[0] == '--prom':
        out = Path(argv[1])
        out.mkdir(parents=True, exist_ok=True)
        for k in (0, 1):
            (out / f'MSK{k}.hex').write_text('\n'.join(f'{v:02X}' for v in MSK[k]) + '\n', encoding='ascii')
        (out / 'ATR.hex').write_text('\n'.join(f'{atr_prom(a):02X}' for a in range(256)) + '\n',
                                     encoding='ascii')
        print(f'PROM を {out} に書いた', file=sys.stderr)
        return
    print(fit_table(pals))


if __name__ == '__main__':
    main(sys.argv[1:])
