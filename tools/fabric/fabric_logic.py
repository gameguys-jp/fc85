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
PAL_FILES = ['transform.pal', 'rdy.pal']
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
            _, _, nxt = p.eval(env)
            if PORTS.get(port, PORT2_DOC)[0] or port == 2:
                assert nxt['SRDY'] == ready_ref(port, False, lines), ('S', port, lines)
            if PORTS.get(port, PORT2_DOC)[1] or port == 2:
                assert nxt['DRDY'] == ready_ref(port, True, lines), ('D', port, lines)


def check_all(pals=None):
    pals = pals or load_pals()
    n = check_transform(pals)
    check_rot_equations(pals)
    check_anx_equations(pals)
    check_rdy(pals)
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
    print(f'ok: 変換 {n} 通り、ROT・ANX・RDY の式', file=sys.stderr)
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
