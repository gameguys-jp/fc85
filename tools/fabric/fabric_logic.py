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
PAL_FILES = ['transform.pal', 'afc.pal', 'rdy.pal', 'seq.pal', 'access.pal', 'host.pal']
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
    """AFC0・AFC1 は、SCLK で SEQ と同じ入力（ELG の AC、CST の CK、次の ACH ＝ DBCH）から登録する。届く AC
    （000〜101。再設定 101 の候補の段階は 00・01・10）とチャネル、CPU の窓の語と向きのすべてで、SEQ が登録する
    AOP・ACH の §05 の表の語を読み（q0〜q1）書き（q2〜q3）、B と Cin と取込みを選ぶか。A ファイルの番地は負の
    論理のまま 74S189 へ行くので、語の番号は番地のピンの反転。"""
    a0, a1, seq = pals['AFC0'], pals['AFC1'], pals['SEQ']
    seq_env = {n: True for n in seq.inputs() + seq.feedback()}
    for ac in range(6):
        for k1, k0 in ((0, 0), (0, 1), (1, 0)) if ac == 5 else ((0, 0), (0, 1), (1, 0), (1, 1)):
            for ch in (0, 1):
                for hch in (0, 1):
                    for (ha3, ha2), word in HOST_WORDS.items():
                        for hrw in (0, 1):
                            env = {'AC2N': not ac >> 2 & 1, 'AC1N': not ac >> 1 & 1, 'AC0N': not ac & 1,
                                   'CK1N': not k1, 'CK0N': not k0, 'DBCHN': not ch, 'HCH': bool(hch),
                                   'HA3': bool(ha3), 'HA2': bool(ha2), 'HRW': bool(hrw)}
                            _, _, s = seq.eval(dict(seq_env, **{k: v for k, v in env.items() if k in seq_env}))
                            aop = (not s['AOP2N']) << 2 | (not s['AOP1N']) << 1 | (not s['AOP0N'])
                            assert (not s['ACHN']) == bool(ch), ('ACH', ac, ch)
                            _, _, n0 = a0.eval(dict(env, **{n: True for n in a0.feedback()}))
                            _, _, n1 = a1.eval(env)
                            # SCLK の後の組合せの出力（CIN）。語の bit 0 は U64（74S157）が、q0〜q1 は AR0N（読み）、
                            # q2〜q3 は AW0N（書き）を選ぶ
                            c0 = a0.eval(dict(env, **n0))[0]
                            word_of = (lambda b0: (~(n0['AA3N'] << 3 | n0['AA2N'] << 2 | n0['AA1N'] << 1
                                                     | b0)) & 15)
                            r0, w0 = n0['AR0N'], n0['AW0N']
                            rd, wr, b, cin, cap = afc_expect(aop, ch, hch, word, hrw)
                            what = (ac, k1, k0, ch, hch, word, hrw)
                            if rd is not None:
                                assert word_of(r0) == rd, ('読む語', what)
                            assert (not n0['AWN']) == (wr is not None), ('書込み', what)
                            if wr is not None:
                                assert word_of(w0) == wr, ('書く語', what)
                            # A ファイルのデータの入力を出すのは、多くて 1 つ（和のラッチか CPU の書込み）
                            assert (not n1['SLOEN']) == (aop >= AOPS['SST']), ('和のラッチ', what)
                            assert (not n1['HWOEN']) == (aop == AOPS['HOST'] and not hrw), ('CPU の書込み', what)
                            assert (not n1['BENN']) == (b is not None), ('B', what)
                            if b is not None:
                                assert (n1['BA1'] << 1 | n1['BA0']) == b, ('B の番地', what)
                            if cin is not None:
                                assert c0['CIN'] == bool(cin), ('Cin', what)
                            for k in ('PSC', 'PDC', 'HRC'):
                                assert (not n1[k + 'N']) == (cap == k), (k, what)


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
            # 次が動作 ＝ NRUN0 ∧ ¬BLK、次が止まっていない ＝ NBSYX ∨ 次が動作（STS で合わせる）
            nrun = not comb['NRUN0N'] and not x['blk']
            assert ((not comb['NBSYXN']) or nrun) == (want != 'I'), ('NBSY', what)
            assert nrun == (want in ('RUN', 'NM')), ('NRUN', what)
            n += 1
    return n


def check_sts(pals):
    """トリガの同期化と立上り、保留、許可、MISS、STALL（fabric.rs の sync_trigger と decide の STALL）。次が動作か
    （NRUN0 ∧ ¬BLK）と次が止まっていないか（NBSYX ∨ 次が動作）は、CHS の NBSYX・NRUN0 と EVT の BLK から。"""
    p = pals['STS']
    names = ['s1', 'syn2', 'rise', 'arm', 'pend', 'miss', 'stall', 'nbsyx', 'nrun0', 'blk', 'st', 'ste', 'wm', 'imm',
             'sv', 'stl', 'p1', 'p0']
    for v in range(1 << len(names)):
        x = {k: bool(v >> i & 1) for i, k in enumerate(names)}
        # ブロックの終わりは動作中（再設定でなく、止める出来事もない）だけなので NRUN0 も 1。START の要求は、止まって
        # いるときだけで、次は再設定。
        if x['blk'] and not x['nrun0']:
            continue
        x['nrun'] = x['nrun0'] and not x['blk']
        x['nbusy'] = x['nbsyx'] or x['nrun']
        if x['st'] and (x['nrun'] or not x['nbusy'] or x['p1'] or x['p0']):
            continue
        env = {'S1Q': x['s1'], 'SYN2N': not x['syn2'], 'RISEN': not x['rise'], 'ARMN': not x['arm'],
               'PENDN': not x['pend'], 'MISSN': not x['miss'], 'STALN': not x['stall'], 'NBSYXN': not x['nbsyx'],
               'NRUN0N': not x['nrun0'], 'BLKN': not x['blk'], 'STRQ': x['st'], 'STEN': not x['ste'],
               'WMQ': x['wm'], 'IMM': x['imm'], 'SVN': not x['sv'], 'STLN': not x['stl'], 'P1N': not x['p1'],
               'P0N': not x['p0']}
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


# ---------------------------------------------------------------- アクセスの制御とポート（access.pal）
#
# OPA・OPB・DPA の働きは、エミュレータの動作の模型（fabric.rs の step_slot、begin、done_mismatch、時間切れ）
# と同じ。全体は門の模型と並べる試験（fabric_lockstep.rs）で突き合わせる。ここでは式ごとに、届きうる入力の
# 全部で仕様の定義と比べる。P1A・P2A は、ポートの向こうの模型（acrtc.rs、doc.rs の ff_edge と abort）の
# 1 回の 16 MHz の縁と比べる。

def bits_env(names, v):
    return {k: bool(v >> i & 1) for i, k in enumerate(names)}


def check_tim(pals):
    """電源投入（RESN）の後、q3・X から始まり、q0 の縁で S と X が入れ替わる。SCLK は q0、MCLK・LCLK は q2 の縁で
    立つ。WPH（WPHN の Low）は q3 の間（次の縁が q0）。AWQN は、書込みのある操作（AWN が Low）の q3 の間だけ Low。"""
    p = pals['TIM']
    st = {n: True for n in p.feedback()}
    _, _, st = p.eval(dict(st, RESN=False, AWN=False))
    assert (st['PH1'], st['PH0'], st['SX'], not st['WPHN'], st['AWQN']) == (True, True, False, True, True), st
    sx = st['SX']
    for k in range(24):
        aw = k % 3 != 1
        _, _, st = p.eval(dict(st, RESN=True, AWN=not aw))
        ph = st['PH1'] << 1 | st['PH0']
        assert ph == k % 4, (k, st)
        if ph == 0:
            assert st['SX'] != sx, k
            sx = st['SX']
        assert st['SX'] == sx, k
        assert (st['SCLK'], st['MCLK'], st['LCLK'], not st['WPHN']) == (ph == 0, ph == 2, ph == 2, ph == 3), (k, st)
        assert (not st['AWQN']) == (ph == 3 and aw), (k, st)


def port_attr(p, name):
    return bool(atr_prom(p) >> ATR_BITS.index(name) & 1)


def check_dpa(pals):
    """GOKS ＝（S のスロットだけのポートなら今が X）∧ ¬FB、FB ＝ OP の FLAG ∨ 入る NX の FLAG。ADDR と S の
    規則は ATR と同じ（向きと FORMAT によらない）。"""
    p = pals['DPA']
    for v in range(1 << 13):
        s, d = v & 15, v >> 4 & 15
        x, opv, opfl, opvd, nxfl = (bool(v >> k & 1) for k in range(8, 13))
        env = {f'S{b}': bool(s >> b & 1) for b in range(4)}
        env.update({f'D{b}': bool(d >> b & 1) for b in range(4)})
        env.update(X=x, OPV=opv, OPFL=opfl, OPVD=opvd, NXFL=nxfl)
        c, _, _ = p.eval(env)
        fb = opv and opfl or opvd and nxfl
        assert c['GOKS'] == ((not port_attr(s, 'SONLY') or x) and not fb), (s, x, fb)
        assert c['ADDRS'] == port_attr(s, 'ADDR'), s
        assert c['ADDRD'] == port_attr(d, 'ADDR'), d
        assert c['SOKD'] == (not port_attr(d, 'SONLY') or x), (d, x)


OPA_IN = ['NXV', 'NXCH', 'NXTW', 'NXLG', 'OPV', 'OPCH', 'OPTW', 'OPLG', 'ODN', 'STPQ0', 'STPQ1', 'TOR']


def spec_opa(x):
    """SCLK の前の値から：OP が空くか（終わった、空、STOP）、NX が STOP で消えないか、判断してよいか（fabric.rs
    の can_decide：次の OP が 1 スロットのアクセスか空、2 スロットの 2 スロット目）、打切り、登録する SD・LA。"""
    stop = lambda ch: x['STPQ1'] if ch else x['STPQ0']
    free = not x['OPV'] or x['ODN'] or stop(x['OPCH'])
    nxv = x['NXV'] and not stop(x['NXCH'])
    if free:
        cd = not nxv or (not x['NXTW'] and not x['NXLG'])
        sd, la = nxv and not x['NXTW'] and not x['NXLG'], nxv and x['NXLG']
    else:
        cd, sd, la = x['OPTW'], x['OPTW'], x['OPLG']
    abt = x['TOR'] or x['OPV'] and x['OPLG'] and not x['ODN'] and stop(x['OPCH'])
    return dict(free=free, nxv=nxv, cd=cd, sd=sd, la=la, abt=abt)


def opa_reachable(x):
    """届く組：TW と LG は同時にない。OP が空なら ODN は 0。NX が有効なら、OP はこの頭で空く（前の頭の CD）。
    TOR は延びたアクセスの時間切れの次のスロットだけ（そのとき ODN も 1）。"""
    if x['OPTW'] and x['OPLG'] or x['NXTW'] and x['NXLG']:
        return False
    if not x['OPV'] and x['ODN']:
        return False
    stop = lambda ch: x['STPQ1'] if ch else x['STPQ0']
    if x['NXV'] and not (not x['OPV'] or x['ODN'] or stop(x['OPCH'])):
        return False
    return not x['TOR'] or x['ODN'] and x['OPV'] and x['OPLG']


def check_opa(pals):
    p = pals['OPA']
    for v in range(1 << len(OPA_IN)):
        x = bits_env(OPA_IN, v)
        if not opa_reachable(x):
            continue
        c, _, nxt = p.eval(dict(x, SDN=True, LAN=True))
        w = spec_opa(x)
        assert (not c['OPLDN'], c['OPVD'], c['CD'], not c['ABTN']) == (w['free'], w['nxv'], w['cd'], w['abt']), x
        assert (not nxt['SDN'], not nxt['LAN']) == (w['sd'], w['la']), x


OPB_IN = ['SD', 'LA', 'OPW', 'P1R', 'P2R', 'P1M', 'TOUT', 'INF', 'Q1', 'EPC', 'ELAST', 'TL', 'TLAST']


def check_opb(pals):
    """終わり（1 スロットか延びたアクセスの答え）、port 1 の DONE の印と要素が最後かの食い違い（fabric.rs の
    done_mismatch）、時間切れ（延びたアクセスに答えがなく、U38 が 15 スロットを数えた）。"""
    p = pals['OPB']
    for v in range(1 << len(OPB_IN)):
        x = bits_env(OPB_IN, v)
        # 届く組：SD と LA は同時にない。答えは延びたアクセスの間だけで、P1A・P2A の両方からは来ない。印は
        # P1A の答えと一緒
        if x['SD'] and x['LA'] or (x['P1R'] or x['P2R']) and not x['LA'] or x['P1R'] and x['P2R']:
            continue
        if x['P1M'] and not x['P1R']:
            continue
        env = {('SDN' if k == 'SD' else 'LAN' if k == 'LA' else k + 'N' if k not in ('OPW', 'TOUT') else k):
               (not b if k not in ('OPW', 'TOUT') else b) for k, b in x.items()}
        c, _, nxt = p.eval(env)
        done = x['SD'] or x['LA'] and (x['P1R'] or x['P2R'])
        if x['OPW']:
            mm = not (x['INF'] and x['Q1']) and x['TL'] and x['TLAST'] != x['P1M']
        else:
            mm = x['INF'] and not x['Q1'] and not x['EPC'] and x['ELAST'] != x['P1M']
        mm = mm and x['P1R']
        to = x['LA'] and not x['P1R'] and not x['P2R'] and x['TOUT']
        assert c['DNR'] == (done and not x['OPW'] and not mm), x
        assert c['DNW'] == (done and x['OPW'] and not mm), x
        assert (not c['ERRKN']) == (mm or to), x
        assert (nxt['ODN'], nxt['TOR']) == (done or to, to), x
        assert (not nxt['DNRQN']) == c['DNR'], x


def check_stb(pals):
    """ポートごとのアクセスの信号は、OP の有効なアクセスのポートと位相だけで出る。"""
    s0, s1 = pals['STB0'], pals['STB1']
    for port in range(16):
        for v in range(1 << 6):
            opv, opw, w16, pa0, doc, ph1 = (bool(v >> k & 1) for k in range(6))
            for wph in ((False, True) if ph1 else (False,)):
                env = {f'OPP{b}': bool(port >> b & 1) for b in range(4)}
                env.update(OPV=opv, OPW=opw, OPWORD=w16, PA0=pa0, DOC=doc, PH1=ph1, WPHN=not wph)
                c0, _, _ = s0.eval(env)
                c1, _, _ = s1.eval(env)
                on = lambda *ps: opv and port in ps
                want0 = {'RQ0N': on(0), 'RQ9N': on(9), 'ABHN': on(0, 1) and w16,
                         'ABLN': on(0, 1) and w16 or on(0) and not w16 and pa0,
                         'ABSN': on(0) and not w16 and not pa0 or on(9) and not w16,
                         'T67N': on(6, 7), 'XSN': on(8, 10, 11, 12, 13, 14, 15)}
                for k, w in want0.items():
                    assert (not c0[k]) == w, (k, port, v, wph)
                assert c0['WEN'] == (opv and opw), (port, v)
                want1 = {'SIDN': on(2) and not doc and ph1, 'SI3N': on(3) and ph1, 'SI5N': on(5) and ph1,
                         'SO4N': on(4) and wph, 'OE4N': on(4), 'T6WN': on(6) and ph1, 'T7RN': on(7)}
                for k, w in want1.items():
                    assert (not c1[k]) == w, (k, port, v, wph)


def spec_answer(x, ans, abt):
    """P1A・P2A の答えの数え方（fabric.md §03）：q0 の縁（WPH：次の縁が q0）の答えはそのスロットの答え（P1R・P2R）。
    q1〜q3 の縁の答えは預かり（PND）にして、次の q0 の縁で P1R・P2R にする。打切り（q0 の縁だけ）は預かりを捨てる。
    返す値は（預かり、そのスロットの答え）の次の値。"""
    if x['WPH']:
        return False, not abt and (ans or x['PND'])
    return ans or x['PND'], x['PR']


P1A_IN = ['WPH', 'FREE', 'OPVD', 'NXLG', 'NXP0', 'DTS', 'DNF', 'ABT', 'ACT', 'PND', 'DNM', 'PR', 'PM', 'L2']


def spec_p1a(x):
    """acrtc.rs の 1 回の 16 MHz の縁：NX の port 1 のアクセスが OP に入る q0 の縁で DACK を出す。動いていて
    同期化した DTACK があれば答え、DONE を印にする。打切り（ABT ∧ WPH）は答えより先。"""
    abt = x['ABT'] and x['WPH']
    start = x['WPH'] and x['FREE'] and x['OPVD'] and x['NXLG'] and x['NXP0']
    ans = x['ACT'] and x['DTS'] and not abt
    pnd, p1r = spec_answer(x, ans, abt)
    if x['WPH']:
        p1m = not abt and (ans and x['DNF'] or x['PND'] and x['DNM'])
    else:
        p1m = x['PM']
    # L2：OP の写し（port 2 の延びたアクセス）。OP に入る q0 の縁（WPH ∧ FREE）で NX から取り、ほかは保つ（DR-091）
    load = x['WPH'] and x['FREE']
    l2 = x['OPVD'] and x['NXLG'] and not x['NXP0'] if load else x['L2']
    return dict(act=start or x['ACT'] and not x['DTS'] and not abt, pnd=pnd, dnm=x['DNF'] if ans else x['DNM'],
                p1r=p1r, p1m=p1m, l2=l2)


def check_p1a(pals):
    p = pals['P1A']
    for v in range(1 << len(P1A_IN)):
        x = bits_env(P1A_IN, v)
        start = x['WPH'] and x['FREE'] and x['OPVD'] and x['NXLG'] and x['NXP0']
        # 届く組：始めるのは、port 1 のアクセスがなく、打ち切るアクセスもない頭だけ。答えを預かっているときは、
        # DACK を戻している
        if start and (x['ACT'] or x['ABT']) or x['PND'] and x['ACT']:
            continue
        env = {k: x[k] for k in ('OPVD', 'NXLG', 'NXP0', 'DTS', 'DNF')}
        env.update(WPHN=not x['WPH'], OPLDN=not x['FREE'], ABQN=not (x['ABT'] and x['WPH']), DACKN=not x['ACT'],
                   PNDN=not x['PND'], DNMN=not x['DNM'], P1RN=not x['PR'], P1MN=not x['PM'], L2N=not x['L2'])
        _, _, nxt = p.eval(env)
        w = spec_p1a(x)
        got = dict(act=not nxt['DACKN'], pnd=not nxt['PNDN'], dnm=not nxt['DNMN'], p1r=not nxt['P1RN'],
                   p1m=not nxt['P1MN'], l2=not nxt['L2N'])
        assert got == w, (x, got, w)


P2A_IN = ['SYN1', 'L2', 'FREE', 'NXW', 'ABT', 'WPH', 'BW', 'SYN2', 'S1', 'S0', 'RDH', 'PND', 'PR', 'WQ']


def spec_p2a(x):
    """doc.rs の ff_edge と abort の 1 回の 16 MHz の縁。段（S1・S0）は 空き 00、窓を待つ 01、窓を取った 11、
    取った読出しを捨てる 10。打切り（abort。ABT ∧ WPH）が先、次に窓の終わり（E の立下り）で預かりを空け、空いて
    いて、答えを預かっても数えてもいなければ Fabric のアクセスをラッチし（LDQ。書込みはすぐ答える）、E の立下りで
    窓を取る（ブリッジが先）。答えは spec_answer。
    port 2 のアクセスがあるか（L2）は P1A の、向き（WQ）は P2A の、OP の写し（DR-091）。
    取った後の読出しの打切りは、E が上がる前でも捨てる（P2A は E の立上りを知らない。doc.rs も DR-110 でこれに
    合わせた。OI-089）。"""
    fell = x['SYN2'] and not x['SYN1']
    abt = x['ABT'] and x['WPH']
    s1, s0, rdh = x['S1'], x['S0'], x['RDH']
    if abt and rdh and s0:
        s0 = False
    ans = live = False
    if fell and s1:
        ans = live = s0 and rdh
        s1 = s0 = False
    lat = (x['L2'] and not abt and not x['PND'] and not x['PR'] and not live
           and not s1 and not s0)
    if lat:
        s0, rdh = True, not x['WQ']
        ans = ans or x['WQ']
    if fell and not x['BW'] and (s1, s0) == (False, True):
        s1 = True
    pnd, p2r = spec_answer(x, ans, abt)
    wq = x['NXW'] if x['WPH'] and x['FREE'] else x['WQ']
    return dict(syn2=x['SYN1'], s1=s1, s0=s0, rdh=rdh, pnd=pnd, p2r=p2r, ldq=lat, wq=wq)


def p2a_reachable(x):
    """P2A の届く組。E の 1 周期は 16 MHz の 8 周期以上（DOC の CLK は 16 MHz 以下）とする。"""
    fell = x['SYN2'] and not x['SYN1']
    a = x['L2']
    held = x['S1'] or x['S0']
    pr = x['PND'] or x['PR']
    if x['S1'] and not x['S0'] and not x['RDH']:
        return False          # 捨てる段（10）は読出しだけ
    if fell and x['S1'] and pr:
        return False          # 窓の終わりの縁では、答えはもう数えた（ラッチから E の 1 周期以上）
    if x['PND'] and x['PR'] or pr and not a:
        return False          # 答えを預かるか数えている間は、OP はそのアクセスのまま
    if pr and held and x['RDH']:
        return False          # 読出しを預かっている間は、答えはない
    if x['PR'] and x['ABT'] and x['WPH']:
        return False          # 数えたスロットの次の頭は、ODN があるので打ち切らない
    if held and x['RDH'] and x['S0'] and not (a and not x['WQ']):
        return False          # 捨てていない、預かった読出しは OP のアクセス
    return True


def check_p2a(pals):
    p = pals['P2A']
    for v in range(1 << len(P2A_IN)):
        x = bits_env(P2A_IN, v)
        if not p2a_reachable(x):
            continue
        env = {k: x[k] for k in ('SYN1', 'NXW')}
        env.update(L2N=not x['L2'], OPLDN=not x['FREE'], ABQN=not (x['ABT'] and x['WPH']), WPHN=not x['WPH'],
                   BWN=not x['BW'], SYN2N=not x['SYN2'], S1N=not x['S1'], S0N=not x['S0'], RDHN=not x['RDH'],
                   PNDN=not x['PND'], P2RN=not x['PR'], WQN=not x['WQ'])
        _, _, nxt = p.eval(env)
        w = spec_p2a(x)
        got = dict(syn2=not nxt['SYN2N'], s1=not nxt['S1N'], s0=not nxt['S0N'], rdh=not nxt['RDHN'],
                   pnd=not nxt['PNDN'], p2r=not nxt['P2RN'], ldq=not nxt['LDQN'], wq=not nxt['WQN'])
        if not (w['s1'] or w['s0']):
            del got['rdh'], w['rdh']    # 空き（00）の RDH は使わない
        assert got == w, (x, got, w)


def spec_hdec(off, fcs, uds, lds, rw):
    """仕様書 §06・fabric.md §07 の窓：16-bit のアクセスだけ。チャネル 0・1 の 0x00〜0x1E と 0x100〜0x104。"""
    s = fcs and uds and lds
    chreg = s and off < 0x80 and off & 0x20 == 0
    r = off & 0x1F
    glob = s and off in (0x100, 0x102, 0x104)
    af = chreg and r in (0x06, 0x0A, 0x0C)
    w = not rw
    return dict(VLDN=chreg and not af or glob, AREQN=af, RAFN=af and rw, RMODN=glob and rw and off == 0x104,
                RDENN=fcs and rw, W1XN=chreg and w and r >= 0x10, WSRCN=chreg and w and r == 0,
                WDSTN=chreg and w and r == 2, WBFN=chreg and w and r in (0x0E, 0x10),
                WPRSTN=glob and w and off == 0x100, WAUDN=glob and w and off == 0x102,
                RST0N=chreg and rw and r == 0x1E and off < 0x40, RST1N=chreg and rw and r == 0x1E and off >= 0x40)


def check_hdec(pals):
    h0, h1 = pals['HDEC0'], pals['HDEC1']
    for off in range(0, 0x1000, 2):
        for v in range(16):
            fcs, uds, lds, rw = (bool(v >> k & 1) for k in range(4))
            env = {f'A{b}': bool(off >> b & 1) for b in range(1, 12)}
            env.update(FCSN=not fcs, UDSN=not uds, LDSN=not lds, RW=rw)
            c0, _, _ = h0.eval(env)
            c1, _, _ = h1.eval(dict(env, CHN=c0['CHN'], GLN=c0['GLN']))
            got = dict(c0, **c1)
            for k, want in spec_hdec(off, fcs, uds, lds, rw).items():
                assert (not got[k]) == want, (k, hex(off), v)


def check_hack(pals):
    """Fabric の窓の同期化と DTACK（DR-098）、DOC の窓のブリッジ（doc.rs の ff_edge の 68000 の側：S4 で /DCS を
    取り込み、E の立下りで窓を取り、次の立下りで DTACK。サイクルが途中で終われば取り消すか、取った窓の値を捨てる）。
    Y は毎縁 VLD ∨ AREQ。q1 の縁（SCLK）だけで、CAP（Y ∧ ¬DTK）、HREQ（Y ∧ AREQ ∧ ¬HREQ ∧ ¬DTK）、DTK の立上り
    （Y ∧ VLD、または許可のスロットの HREQ ∧ AREQ）を決める。DTK は、Fabric の窓ではストローブの間（VLD ∨ AREQ）
    保ち、DOC の窓では BDT と同じ。"""
    p = pals['HACK']
    names = ['AREQ', 'VLD', 'SCLK', 'SYN1', 'SYN2', 'DCS', 'Y', 'CAP', 'HREQ', 'DTK', 'BW', 'BC', 'BDT']
    for v in range(1 << len(names)):
        x = bits_env(names, v)
        # 窓は 1 つだけ（HDEC0 の VLD・AREQ と、A 基板の /DCS）。ブリッジの状態も 1 つだけ
        if x['AREQ'] + x['VLD'] + x['DCS'] > 1 or x['BW'] + x['BC'] + x['BDT'] > 1:
            continue
        env = dict(AREQN=not x['AREQ'], VLDN=not x['VLD'], SCLK=x['SCLK'], SYN1=x['SYN1'], SYN2N=not x['SYN2'],
                   DCSN=not x['DCS'], YN=not x['Y'], CAPN=not x['CAP'], HREQN=not x['HREQ'], DTKN=not x['DTK'],
                   BWN=not x['BW'], BCN=not x['BC'], BDTN=not x['BDT'])
        _, _, nxt = p.eval(env)
        fell = x['SYN2'] and not x['SYN1']
        st = 'C' if x['BC'] else 'W' if x['BW'] else 'D' if x['BDT'] else 'I'
        if st == 'C':
            nst = ('D' if x['DCS'] else 'I') if fell else 'C'
        elif st == 'W':
            nst = 'I' if not x['DCS'] else 'C' if fell else 'W'
        elif st == 'D':
            nst = 'D' if x['DCS'] else 'I'
        else:
            nst = ('C' if fell else 'W') if x['DCS'] else 'I'
        q1, y = x['SCLK'], x['Y']
        want = dict(
            Y=x['VLD'] or x['AREQ'],
            CAP=(y and not x['DTK']) if q1 else x['CAP'],
            HREQ=(y and x['AREQ'] and not x['HREQ'] and not x['DTK']) if q1 else x['HREQ'],
            DTK=(q1 and (y and x['VLD'] or x['HREQ'] and x['AREQ'])
                 or x['DTK'] and not x['BDT'] and (x['VLD'] or x['AREQ']) or nst == 'D'),
            BW=nst == 'W', BC=nst == 'C', BDT=nst == 'D')
        got = {k: not nxt[k + 'N'] for k in want}
        assert got == want, (x, got, want)


def check_req(pals):
    """REQ0・REQ1：CAP の間の LCLK で、CTRL（bit 0 START、bit 1 STOP）と STATUS の W1C（bit 1 DONE、3 ERROR、
    4 MISS）の書込みを、1 スロットの要求にする。STOP が先、START は止まっているチャネルだけ（DR-046 (6)）。"""
    p = pals['REQ']
    names = ['CAP', 'CTRL', 'STAT', 'D0', 'D1', 'D3', 'D4', 'BUSY']
    for v in range(1 << len(names)):
        x = bits_env(names, v)
        if x['CTRL'] and x['STAT']:
            continue
        env = dict(CAPN=not x['CAP'], CTRLN=not x['CTRL'], STATN=not x['STAT'], D0=x['D0'], D1=x['D1'],
                   D3=x['D3'], D4=x['D4'], BUSY=x['BUSY'])
        _, _, nxt = p.eval(env)
        c, s = x['CAP'] and x['CTRL'], x['CAP'] and x['STAT']
        want = dict(STRQ=c and x['D0'] and not x['D1'] and not x['BUSY'], STPQ=c and x['D1'], WDQ=s and x['D1'],
                    WEQ=s and x['D3'], WMQ=s and x['D4'])
        assert {k: nxt[k] for k in want} == want, x


# 68000 のバスサイクルと HACK・REQ（DR-098）。時刻は 16 MHz の縁の番号 E（スロット k の qj の縁が 4k＋j）。68000 の
# クロックは 16 MHz の 2 分周で、立上りが q0・q2 の縁、立下りが q1・q3 の縁の t_div（15 ns 以下）後。
# - ストローブ（読出しは S2 の /UDS・/LDS、書込みは S4）は、立上りの縁 t_s の後 t_div＋tCHSL（60 以下）で出る。
#   HDEC0（25）を通って HACK の Y（tSU 25）に入るのは、t_s＋1 の縁（速ければ）か t_s＋2 の縁（必ず。15＋60＋25＋25
#   ＝125）。t_s の縁には入らない。
# - DTACK（HACK の DTK）を 68000 が認識するのは、DTK が立った縁の後の最初の立下り（S4 の終わり E0＋5 より前なら
#   S4 の終わり）。認識の立下り F の後、S7 は F＋2、サイクルの終わりは F＋3。ストローブは S7 の後 t_div＋tCLSH
#   （70 以下）で戻り、Y に戻りが見えるのは F＋3 か F＋4 の縁（速ければ）、F＋5 の縁（必ず。15＋70＋25＋25 ＜ 187.5）。
# - SEQ は SCLK（q0 の縁）で HREQ を見て、見ればそのスロットの AOP を 001（CPU の A ファイルの操作）にする（ELG）。
# - REQ は LCLK（q2 の縁の 15 ns 後。CAP は q1 の縁でしか変わらない）で、CAP と、ストローブの間の解読を取り込む。
CPU_KINDS = {'R': (False, False), 'C': (True, False), 'S': (True, False), 'AR': (False, True), 'AW': (True, True)}


def cpu_cycles(pals, kinds, e0, gaps, choices):
    """バスサイクルの列を 16 MHz の縁ごとに進める。kinds：CPU_KINDS の名前（'R' は STATUS の読出し、'C' は CTRL の
    STOP、'S' は STATUS の DONE の W1C）。e0：最初の S0 の縁。gaps：サイクルの間の空きのクロック数。choices：
    サイクルごとの（ストローブが t_s＋1 の縁で見えるか、戻りが見える縁 F＋3〜F＋5）。返す値：サイクルごとの
    記録と、REQ の要求の出たスロットの列。"""
    hack, req = pals['HACK'], pals['REQ']
    st = {n: True for n in ('YN', 'CAPN', 'HREQN', 'DTKN', 'BWN', 'BCN', 'BDTN')}
    cyc, pulses = [], []
    next_e0, end = e0, None
    e = e0 - 1
    while end is None or e < end:
        e += 1
        assert e < e0 + 200, (kinds, 'DTACK が来ない')
        if e == next_e0:
            k = len(cyc)
            w, af = CPU_KINDS[kinds[k]]
            early, back = choices[k]
            cyc.append(dict(kind=kinds[k], e0=e, w=w, af=af, ts=e + (4 if w else 2), early=early, back=back,
                            f=None, caps=[], dtks=[], grants=[]))
        cur = cyc[-1]
        # 68000 の立下りで DTACK を見る（DTK はこの縁の前の値）。認識したら、次の S0 か終わりが決まる
        if cur['f'] is None and e % 2 == 1 and e >= cur['e0'] + 5 and cur['dtks'] and not st['DTKN']:
            cur['f'] = e
            if len(cyc) < len(kinds):
                next_e0 = e + 3 + 2 * gaps[len(cyc) - 1]
            else:
                end = e + 12

        def strobe(c):
            """この縁で HACK の Y に見えるストローブ。"""
            if e <= c['ts']:
                return False
            if e == c['ts'] + 1:
                return c['early']
            return c['f'] is None or e < c['f'] + c['back']

        act = [c for c in cyc if strobe(c)]
        assert len(act) <= 1, (kinds, e)
        a = act[0] if act else None
        if e % 4 == 0 and not st['HREQN']:
            # SEQ は SCLK で HREQ を見て、このスロットを許可する
            assert a is not None and a['af'], (kinds, e, 'HREQ')
            a['grants'].append(e // 4)
        if e % 4 == 2:
            ctrl, stat = a is not None and a['kind'] == 'C', a is not None and a['kind'] == 'S'
            _, _, rq = req.eval(dict(CAPN=st['CAPN'], CTRLN=not ctrl, STATN=not stat, D0=False, D1=True, D3=False,
                                     D4=False, BUSY=False))
            pulses += [(n, e // 4) for n, v in rq.items() if v]
        old = st
        _, _, st = hack.eval(dict(st, AREQN=not (a is not None and a['af']), VLDN=not (a is not None and not a['af']),
                                  SCLK=e % 4 == 1, SYN1=False, SYN2N=True, DCSN=True))
        if old['CAPN'] and not st['CAPN']:
            assert a is not None, (kinds, e, 'CAP')
            a['caps'].append(e)
        if old['DTKN'] and not st['DTKN']:
            assert a is not None, (kinds, e, 'DTK')
            a['dtks'].append(e)
    return cyc, pulses


def check_cpu_cycles(pals):
    """2 回続くバスサイクルのすべての種類、最初の S0 の位相（q0・q2）、間の空き（0・1 クロック）、ストローブの
    見え方の選び方のすべてで、取込みのスロット c、許可のスロット g、待ち、REQ の要求の出るスロットが、S0 の
    時刻だけで決まること（仕様書 §06 の規則）。S0 のクロックを at（偶数なら q0）、スロットを n ＝ at÷2 とすると：
    c ＝ (at＋3＋書込み)÷2、A ファイルでは g ＝ c＋1。DTACK は q3(c) か q3(g) で認識され、待ちは 2r−at−1。"""
    n = 0
    for k0 in CPU_KINDS:
        for k1 in CPU_KINDS:
            for e0 in (8, 10):
                for gap in (0, 1):
                    for ch in range(36):
                        choices = [((ch >> 0) & 1 == 1, 3 + (ch >> 1) % 3), ((ch // 6) & 1 == 1, 3 + (ch // 12) % 3)]
                        cyc, pulses = cpu_cycles(pals, [k0, k1], e0, [gap], choices)
                        want_pulses = []
                        for c in cyc:
                            at = c['e0'] // 2
                            cap = (at + 3 + c['w']) // 2
                            r = cap + 1 if c['af'] else cap
                            what = (k0, k1, e0, gap, choices, c)
                            assert c['caps'][:1] == [4 * cap + 1], what
                            assert c['dtks'] == [4 * r + 1], what
                            assert c['grants'] == ([r] if c['af'] else []), what
                            assert c['f'] == 4 * r + 3, what
                            assert (c['f'] - c['e0'] - 5) // 2 == 2 * r - at - 1, what
                            if c['kind'] == 'C':
                                want_pulses.append(('STPQ', cap))
                            elif c['kind'] == 'S':
                                want_pulses.append(('WDQ', cap))
                        assert pulses == want_pulses, (k0, k1, e0, gap, choices, pulses)
                        n += 1
    return n


PRST_PORTS = (2, 3, 4, 5, 6, 7, 10)


def check_prst(pals):
    p = pals['PRST']
    for v in range(1 << 9):
        d = {f'D{q}': bool(v >> i & 1) for i, q in enumerate(PRST_PORTS)}
        wr, res = bool(v >> 7 & 1), bool(v >> 8 & 1)
        c, _, _ = p.eval(dict(d, WPRSTN=not wr, RESN=not res))
        for q in PRST_PORTS:
            assert (not c[f'RST{q}N']) == (res or wr and d[f'D{q}']), (q, v)


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
    check_tim(pals)
    check_dpa(pals)
    check_opa(pals)
    check_opb(pals)
    check_stb(pals)
    check_p1a(pals)
    check_p2a(pals)
    check_hdec(pals)
    check_hack(pals)
    check_req(pals)
    check_cpu_cycles(pals)
    check_prst(pals)
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
        src = rust_source(pals)
        with open(RUST_OUT, 'w', encoding='utf-8', newline='\n') as f:
            f.write(src)
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
