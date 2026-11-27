#!/usr/bin/env python3
"""本体の番地とレジスタの表（docs/spec/registers.toml）から、アセンブラ・C・Rust の定義と文書を作る（DR-100）。

    py tools/regmap/regmap.py            # 表を確かめて、生成物を書く
    py tools/regmap/regmap.py --check    # 作り直して、置いてあるファイルとの差を出す（差があれば 1 で終わる）

生成物（手で直さない）：

    sdk/include/fc85.inc                   m68kasm の EQU
    sdk/include/fc85.h                     C のヘッダ（m68k-elf-gcc）
    emulator/crates/board/src/regmap.rs    Rust の定数と、窓・メモリ・レジスタの表
    docs/spec/registers.md                 人が読む表

表の形は registers.toml の冒頭の注釈に書いた。確かめること（problems）：名前が 1 回だけ、値が幅に収まる、
窓が重ならない、レジスタが自分のブロックの窓の中で偶数番地、レジスタどうしが重ならない（同じ番地の読出しと
書込みの組は許す）、ビット番号（_B）とマスクがそろう、W1C のビットと of の名前がある。

Python 3.11 以上の標準ライブラリだけを使う（tomllib）。
"""
import ast
import difflib
import operator
import re
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
SRC = ROOT / 'docs' / 'spec' / 'registers.toml'
OUT = {
    'inc': ROOT / 'sdk' / 'include' / 'fc85.inc',
    'h': ROOT / 'sdk' / 'include' / 'fc85.h',
    'rs': ROOT / 'emulator' / 'crates' / 'board' / 'src' / 'regmap.rs',
    'md': ROOT / 'docs' / 'spec' / 'registers.md',
}
SRC_REL = 'docs/spec/registers.toml'
REGEN = 'py tools/regmap/regmap.py'
DR = 'DR-100'

ACCESS = ('r', 'w', 'rw')
KINDS = ('mask', 'bit', 'value', 'number')
STATUS = ('D', 'P', 'O', 'V')
WIDTHS = ('any', 'word', 'upper')
UNLISTED = ('berr', 'chip')
BITS = (8, 16, 32)
NAME = re.compile(r'[A-Z][A-Z0-9_]*$')
# 注釈の行をつなぐとき、間に空白を入れない文字（日本語の句読点と括弧）
TIGHT = set('、。，．（）「」『』：；・／〜')


class MapError(Exception):
    """表の誤り。"""


# ---------------------------------------------------------------- 表の形

@dataclass
class Const:
    """名前の付いた値（fc85.inc の EQU の 1 行）。kind は mask・bit・value・number と、addr（絶対番地）・
    offset（Data Fabric のチャネルの中）。width は C と Rust の型の幅、digits は fc85.inc の 16 進の桁（0 は 10 進）。"""
    name: str
    value: int
    kind: str
    width: int
    digits: int
    doc: list = field(default_factory=list)
    expr: str | None = None


@dataclass
class Group:
    title: str
    note: list
    of: str | None
    status: str | None
    refs: list
    items: list


@dataclass
class Reg:
    name: str
    addr: int
    bits: int
    access: str
    count: int
    stride: int
    w1c: int
    read_effect: str | None
    when: str
    doc: list
    status: str | None
    refs: list
    index: int | None = None
    offset: int | None = None
    low: str | None = None

    def span(self):
        """1 要素がバスの上で占める byte 数（8 bit の装置も語の枠を占める）。"""
        return 4 if self.bits == 32 else 2

    def elements(self):
        return [self.addr + i * self.stride for i in range(self.count)]


@dataclass
class Region:
    name: str
    addr: int
    bytes: int
    end: str | None
    access: str
    width: str
    within: str | None
    when: str
    doc: list
    status: str | None
    refs: list

    @property
    def last(self):
        return self.addr + self.bytes - 1


@dataclass
class Window:
    name: str
    first: int
    last: int
    unlisted: str
    when: str
    doc: list
    block: str


@dataclass
class Bank:
    title: str
    names: list
    addr: int
    stride: int
    stride_name: str
    reserved: int
    doc: list
    regs: list


@dataclass
class Block:
    id: str
    title: str
    refs: list
    status: str | None
    note: list
    base: Const | None
    index_stride: int | None
    windows: list = field(default_factory=list)
    regions: list = field(default_factory=list)
    bank: Bank | None = None
    regs: list = field(default_factory=list)
    groups: list = field(default_factory=list)

    def all_regs(self):
        return (self.bank.regs if self.bank else []) + self.regs

    def has_regs(self):
        """定数のまとまりのほかに、番地の名前があるか。"""
        return bool(self.base or self.regions or self.bank or self.regs)


@dataclass
class Map:
    meta: dict
    access: dict
    when: dict
    unlisted: dict
    blocks: list = field(default_factory=list)
    symbols: dict = field(default_factory=dict)

    def windows(self):
        """アドレスマップの窓（ブロックの窓と、ほかに含まれないメモリ）を番地の順に。"""
        out = []
        for b in self.blocks:
            out += b.windows
            out += [Window(r.name, r.addr, r.last, 'berr', r.when, r.doc, b.id) for r in b.regions if not r.within]
        return sorted(out, key=lambda w: w.first)

    def regs(self):
        return [r for b in self.blocks for r in b.all_regs()]

    def regions(self):
        return [r for b in self.blocks for r in b.regions]


# ---------------------------------------------------------------- 読込み

_OPS = {ast.BitOr: operator.or_, ast.BitAnd: operator.and_, ast.Add: operator.add, ast.Sub: operator.sub,
        ast.Mult: operator.mul, ast.LShift: operator.lshift}


def evaluate(expr, symbols):
    """式（前に出た名前、整数、| & + - * <<）の値。"""
    def ev(n):
        if isinstance(n, ast.Constant) and type(n.value) is int:
            return n.value
        if isinstance(n, ast.Name):
            if n.id not in symbols:
                raise MapError(f'式 {expr!r} の {n.id} が前に出ていない')
            return symbols[n.id].value
        if isinstance(n, ast.BinOp) and type(n.op) in _OPS:
            return _OPS[type(n.op)](ev(n.left), ev(n.right))
        raise MapError(f'式 {expr!r} に使えない形がある')
    try:
        tree = ast.parse(expr, mode='eval')
    except SyntaxError as e:
        raise MapError(f'式 {expr!r} が読めない') from e
    return ev(tree.body)


def _keys(t, allowed, where):
    if not isinstance(t, dict):
        raise MapError(f'{where}：表（table）でない')
    extra = sorted(set(t) - set(allowed))
    if extra:
        raise MapError(f'{where}：知らない項目 {"、".join(extra)}')


def _need(t, key, where):
    if key not in t:
        raise MapError(f'{where}：{key} がない')
    return t[key]


def _lines(v, where):
    if v is None:
        return []
    if isinstance(v, str):
        return [v]
    if isinstance(v, list) and all(isinstance(s, str) for s in v):
        return list(v)
    raise MapError(f'{where}：文字列か文字列の配列でない')


def _int(v, where):
    if type(v) is not int:
        raise MapError(f'{where}：整数でない')
    return v


def _choice(v, allowed, where):
    if v not in allowed:
        raise MapError(f'{where}：{v!r} は {"・".join(map(str, allowed))} のどれでもない')
    return v


def _status(v, where):
    return None if v is None else _choice(v, STATUS, where)


def _define(m, c, where):
    if not NAME.match(c.name):
        raise MapError(f'{where}：名前 {c.name!r} は英大文字・数字・_ で書く')
    if c.name in m.symbols:
        raise MapError(f'{where}：{c.name} が 2 回出た')
    m.symbols[c.name] = c
    return c


def parse(text):
    """registers.toml の中身を読む。形の誤りは MapError。中身の誤りは problems() で調べる。"""
    try:
        t = tomllib.loads(text)
    except tomllib.TOMLDecodeError as e:
        raise MapError(f'TOML として読めない：{e}') from e
    _keys(t, {'meta', 'access', 'when', 'unlisted', 'block'}, '表')
    meta = _need(t, 'meta', '表')
    _keys(meta, {'title', 'intro', 'rules_title', 'rules'}, '[meta]')
    rules = _need(meta, 'rules', '[meta]')
    m = Map(meta={'title': _need(meta, 'title', '[meta]'),
                  'intro': _lines(meta.get('intro'), '[meta] intro'),
                  'rules_title': _need(meta, 'rules_title', '[meta]'),
                  'rules': [_lines(r, '[meta] rules') for r in rules]},
            access=_need(t, 'access', '表'), when=_need(t, 'when', '表'), unlisted=_need(t, 'unlisted', '表'))
    if sorted(m.access) != sorted(ACCESS):
        raise MapError(f'[access] は {"・".join(ACCESS)} の意味を書く')
    if sorted(m.unlisted) != sorted(UNLISTED):
        raise MapError(f'[unlisted] は {"・".join(UNLISTED)} の意味を書く')
    if 'always' not in m.when:
        raise MapError('[when] に always がない')
    for k in m.when:
        if not re.fullmatch(r'[a-z][a-z0-9_]*', k):
            raise MapError(f'[when] の {k!r} は英小文字・数字・_ で書く')
    for b in _need(t, 'block', '表'):
        m.blocks.append(_block(m, b))
    return m


def _block(m, b):
    where = f'block {b.get("id", "?")}'
    _keys(b, {'id', 'title', 'refs', 'status', 'note', 'base', 'index_stride', 'window', 'region', 'bank', 'reg',
              'group'}, where)
    blk = Block(id=_need(b, 'id', where), title=_need(b, 'title', where), refs=_lines(b.get('refs'), where),
                status=_status(b.get('status'), where), note=_lines(b.get('note'), where), base=None,
                index_stride=b.get('index_stride'))
    if any(x.id == blk.id for x in m.blocks):
        raise MapError(f'{where}：id が 2 回出た')
    if 'base' in b:
        bb = b['base']
        _keys(bb, {'name', 'addr'}, f'{where} base')
        blk.base = _define(m, Const(_need(bb, 'name', where), _int(_need(bb, 'addr', where), where), 'addr', 32, 6),
                           where)
    for w in b.get('window', []):
        ww = f'{where} window {w.get("name", "?")}'
        _keys(w, {'name', 'first', 'last', 'doc', 'unlisted', 'when'}, ww)
        blk.windows.append(Window(_need(w, 'name', ww), _int(_need(w, 'first', ww), ww), _int(_need(w, 'last', ww), ww),
                                  _choice(w.get('unlisted', 'berr'), UNLISTED, ww),
                                  _choice(w.get('when', 'always'), list(m.when), ww), _lines(w.get('doc'), ww), blk.id))
    for r in b.get('region', []):
        blk.regions.append(_region(m, blk, r))
    if 'bank' in b:
        blk.bank = _bank(m, blk, b['bank'])
    for r in b.get('reg', []):
        blk.regs.append(_reg(m, blk, r, None))
    for g in b.get('group', []):
        blk.groups.append(_group(m, blk, g))
    # 条件付きの窓の中のレジスタは、窓の条件を引き継ぐ
    for r in blk.all_regs():
        for w in blk.windows:
            if w.first <= r.addr <= w.last and r.when == 'always':
                r.when = w.when
    return blk


def _region(m, blk, r):
    where = f'block {blk.id} region {r.get("name", "?")}'
    _keys(r, {'name', 'addr', 'bytes', 'end', 'access', 'width', 'within', 'when', 'doc', 'status', 'refs'}, where)
    rg = Region(name=_need(r, 'name', where), addr=_int(_need(r, 'addr', where), where),
                bytes=_int(_need(r, 'bytes', where), where), end=r.get('end'),
                access=_choice(_need(r, 'access', where), ACCESS, where),
                width=_choice(r.get('width', 'any'), WIDTHS, where), within=r.get('within'),
                when=_choice(r.get('when', 'always'), list(m.when), where), doc=_lines(r.get('doc'), where),
                status=_status(r.get('status'), where), refs=_lines(r.get('refs'), where))
    _define(m, Const(rg.name, rg.addr, 'addr', 32, 6, rg.doc), where)
    if rg.end:
        _define(m, Const(rg.end, rg.addr + rg.bytes, 'addr', 32, 6), where)
    return rg


def _bank(m, blk, k):
    where = f'block {blk.id} bank'
    _keys(k, {'title', 'names', 'addr', 'stride', 'stride_name', 'reserved', 'doc', 'reg'}, where)
    bank = Bank(title=_need(k, 'title', where), names=_lines(_need(k, 'names', where), where),
                addr=_int(_need(k, 'addr', where), where), stride=_int(_need(k, 'stride', where), where),
                stride_name=_need(k, 'stride_name', where), reserved=_int(k.get('reserved', 0), where),
                doc=_lines(k.get('doc'), where), regs=[])
    for i, n in enumerate(bank.names):
        _define(m, Const(n, bank.addr + i * bank.stride, 'addr', 32, 6), where)
    _define(m, Const(bank.stride_name, bank.stride, 'number', 32, 2), where)
    for r in k.get('reg', []):
        bank.regs.append(_reg(m, blk, r, bank))
    return bank


def _reg(m, blk, r, bank):
    where = f'block {blk.id} reg {r.get("name", "?")}'
    _keys(r, {'name', 'addr', 'index', 'offset', 'bits', 'access', 'count', 'stride', 'low', 'w1c', 'read_effect',
              'when', 'status', 'refs', 'doc'}, where)
    name = _need(r, 'name', where)
    places = [k for k in ('addr', 'index', 'offset') if k in r]
    if bank and places != ['offset']:
        raise MapError(f'{where}：bank のレジスタは offset で置く')
    if not bank and places not in (['addr'], ['index']):
        raise MapError(f'{where}：addr か index のどちらか 1 つで置く')
    reg = Reg(name=name, addr=0, bits=_choice(_need(r, 'bits', where), BITS, where),
              access=_choice(_need(r, 'access', where), ACCESS, where), count=_int(r.get('count', 1), where),
              stride=_int(r.get('stride', 2), where), w1c=_int(r.get('w1c', 0), where),
              read_effect=r.get('read_effect'), when=_choice(r.get('when', 'always'), list(m.when), where),
              doc=_lines(r.get('doc'), where), status=_status(r.get('status'), where),
              refs=_lines(r.get('refs'), where), low=r.get('low'))
    if bank:
        if 'count' in r or 'stride' in r:
            raise MapError(f'{where}：bank のレジスタの並びは bank の names と stride で決まる')
        reg.offset = _int(r['offset'], where)
        reg.addr = bank.addr + reg.offset
        reg.count = len(bank.names)
        reg.stride = bank.stride
        _define(m, Const(name, reg.offset, 'offset', 32, 2, reg.doc), where)
        if reg.low:
            _define(m, Const(reg.low, reg.offset + 2, 'offset', 32, 2), where)
    elif 'index' in r:
        if not blk.base or not blk.index_stride:
            raise MapError(f'{where}：index を使うブロックには base と index_stride が要る')
        reg.index = _int(r['index'], where)
        reg.addr = blk.base.value + blk.index_stride * reg.index
        if 'stride' not in r:
            reg.stride = blk.index_stride
        expr = f'{blk.base.name}+{blk.index_stride}*0x{reg.index:02X}'
        _define(m, Const(name, reg.addr, 'addr', 32, 6, reg.doc, expr), where)
    else:
        reg.addr = _int(r['addr'], where)
        _define(m, Const(name, reg.addr, 'addr', 32, 6, reg.doc), where)
        if reg.low:
            _define(m, Const(reg.low, reg.addr + 2, 'addr', 32, 6), where)
    if reg.low and reg.bits != 32:
        raise MapError(f'{where}：low は 32 bit のレジスタだけ')
    if reg.count < 1:
        raise MapError(f'{where}：count は 1 以上')
    return reg


def _group(m, blk, g):
    where = f'block {blk.id} group {g.get("title", "?")}'
    _keys(g, {'title', 'note', 'of', 'kind', 'width', 'digits', 'status', 'refs', 'items'}, where)
    kind = _choice(_need(g, 'kind', where), KINDS, where)
    if kind in ('mask', 'value'):
        width = _choice(_need(g, 'width', where), BITS, where)
    else:
        width = _choice(g.get('width', 32), BITS, where)
    digits = _int(g.get('digits', width // 4 if kind in ('mask', 'value') else 0), where)
    grp = Group(title=_need(g, 'title', where), note=_lines(g.get('note'), where), of=g.get('of'),
                status=_status(g.get('status'), where), refs=_lines(g.get('refs'), where), items=[])
    for it in _need(g, 'items', where):
        iw = f'{where} {it.get("name", "?") if isinstance(it, dict) else "?"}'
        _keys(it, {'name', 'value', 'expr', 'doc', 'kind', 'digits'}, iw)
        ikind = _choice(it.get('kind', kind), KINDS, iw)
        iwidth = width if ikind in ('mask', 'value') else 32
        idigits = _int(it.get('digits', digits if ikind in ('mask', 'value') else 0), iw)
        expr = it.get('expr')
        if expr is None:
            value = _int(_need(it, 'value', iw), iw)
        else:
            value = evaluate(expr, m.symbols)
            if 'value' in it and it['value'] != value:
                raise MapError(f'{iw}：value {it["value"]:#x} と expr の値 {value:#x} が違う')
        c = Const(_need(it, 'name', iw), value, ikind, iwidth, idigits, _lines(it.get('doc'), iw), expr)
        grp.items.append(_define(m, c, iw))
    return grp


def load(path=SRC):
    """表を読んで確かめる。誤りがあれば MapError。"""
    m = parse(Path(path).read_text(encoding='utf-8'))
    probs = problems(m)
    if probs:
        raise MapError('\n'.join(probs))
    return m


# ---------------------------------------------------------------- 確かめ

def problems(m):
    """表の中身の誤りの説明の並び（なければ空）。"""
    out = []
    for c in m.symbols.values():
        lim = {'addr': 1 << 24, 'offset': 1 << 12, 'bit': 32}.get(c.kind, 1 << c.width)
        if not 0 <= c.value < lim:
            out.append(f'{c.name} の値 {c.value:#x} が幅に収まらない')
        for d in c.doc:
            if '*/' in d or '\n' in d:
                out.append(f'{c.name} の doc に */ か改行がある')
    # 窓は重ならず、偶数番地から奇数番地まで
    wins = m.windows()
    for w in wins:
        if w.first % 2 or w.last % 2 == 0 or w.first > w.last:
            out.append(f'窓 {w.name} の範囲 {w.first:#08x}–{w.last:#08x} は偶数から奇数まで')
    for a, b in zip(wins, wins[1:]):
        if b.first <= a.last:
            out.append(f'窓 {a.name} と {b.name} が重なる')
    names = {r.name for r in m.regs()}
    # レジスタは自分のブロックの窓の中で、偶数番地
    spans = []
    for blk in m.blocks:
        bw = blk.windows + [Window(r.name, r.addr, r.last, 'berr', r.when, [], blk.id) for r in blk.regions]
        for r in blk.all_regs():
            for a in r.elements():
                last = a + r.span() - 1
                if a % 2:
                    out.append(f'{r.name} の番地 {a:#08x} が奇数')
                if not any(w.first <= a and last <= w.last for w in bw):
                    out.append(f'{r.name} の番地 {a:#08x} がブロック {blk.id} の窓の外')
                for w in bw:
                    if w.first <= a <= w.last and w.when not in ('always', r.when):
                        out.append(f'{r.name} の条件 {r.when} が窓 {w.name} の条件 {w.when} と違う')
                spans.append((a, last, r))
            if r.w1c and (r.bits != 16 or r.w1c >> 16):
                out.append(f'{r.name} の w1c は 16 bit のレジスタのビット')
        if blk.bank:
            k = blk.bank
            if len(k.names) > max(k.reserved, len(k.names)):
                out.append(f'bank {k.title} の組が予約より多い')
            end = k.addr + max(k.reserved, len(k.names)) * k.stride - 1
            if not any(w.first <= k.addr and end <= w.last for w in bw):
                out.append(f'bank {k.title} の予約が窓の外')
            for r in k.regs:
                if r.offset + r.span() > k.stride:
                    out.append(f'{r.name} の offset が組の外')
        for g in blk.groups:
            if g.of and g.of not in names:
                out.append(f'group {g.title} の of {g.of} というレジスタがない')
    # レジスタどうしは重ならない。同じ番地の読出しと書込みの組は許す
    spans.sort(key=lambda s: (s[0], s[1]))
    for i, (a, last, r) in enumerate(spans):
        for b, blast, q in spans[i + 1:]:
            if b > last:
                break
            pair = (a, last) == (b, blast) and {r.access, q.access} == {'r', 'w'}
            if not pair:
                out.append(f'{r.name}（{a:#08x}）と {q.name}（{b:#08x}）が重なる')
    # メモリの領域：含まれる領域は親の中
    regions = {r.name: r for r in m.regions()}
    for r in regions.values():
        if r.within:
            p = regions.get(r.within)
            if p is None:
                out.append(f'{r.name} の within {r.within} という領域がない')
            elif not (p.addr <= r.addr and r.last <= p.last):
                out.append(f'{r.name} が {p.name} の外にはみ出す')
    # ビット番号とマスク
    for c in m.symbols.values():
        if c.kind == 'bit' and c.name.endswith('_B'):
            mask = m.symbols.get(c.name[:-2])
            if mask is not None and mask.kind == 'mask' and mask.value != 1 << c.value:
                out.append(f'{c.name}（{c.value}）と {mask.name}（{mask.value:#x}）がそろわない')
    return out


# ---------------------------------------------------------------- 文の部品

def join_lines(lines):
    """注釈の行を 1 つの文にする。日本語と英数字の間には空白を入れる。"""
    s = ''
    for ln in lines:
        if s and ln:
            a, b = s[-1], ln[0]
            ascii_edge = (a.isascii() and a.isalnum()) or (b.isascii() and b.isalnum())
            if ascii_edge and a not in TIGHT and b not in TIGHT and a != ' ' and b != ' ':
                s += ' '
        s += ln
    return s


def titled(title, refs, status):
    """見出し：題（根拠）[印]。"""
    s = f'{title}（{"、".join(refs)}）' if refs else title
    if status:
        s += f'[{status}]' if s.endswith('）') else f' [{status}]'
    return s


def heading(blk):
    return titled(blk.title, blk.refs, blk.status)


def group_heading(g):
    return titled(g.title, g.refs, g.status)


def camel(key):
    return ''.join(p.capitalize() for p in key.split('_'))


UNLISTED_RS = {'berr': 'BusError', 'chip': 'Chip'}


# ---------------------------------------------------------------- fc85.inc

def asm_value(c):
    if c.expr:
        return re.sub(r'0x([0-9A-Fa-f]+)', lambda mm: '$' + mm.group(1).upper(), c.expr)
    if c.digits == 0:
        return str(c.value)
    return f'${c.value:0{c.digits}X}'


def equ(c):
    s = c.name.ljust(15) + ' EQU'
    s += ' ' * max(1, 24 - len(s)) + asm_value(c)
    if not c.doc:
        return [s]
    s += ' ' * max(1, 40 - len(s)) + '; ' + c.doc[0]
    return [s] + [' ' * 40 + '; ' + d for d in c.doc[1:]]


def gen_inc(m):
    sym = m.symbols
    out = [f'; 生成物。直すときは {SRC_REL}（{REGEN} で作り直す。{DR}）。手で直さない。',
           f'; fc85.inc：{m.meta["title"]}。m68kasm 用。',
           ';']
    out += ['; ' + s for s in m.meta['intro']]
    out += [';', '; ' + m.meta['rules_title']]
    for r in m.meta['rules']:
        out += ['; - ' + r[0]] + [';   ' + s for s in r[1:]]
    out += ['; 書式：番地は絶対番地（.L で参照する）。Data Fabric のチャネルのレジスタはチャネルの先頭からの offset。',
            '; 並び（PALETTE、AXIS0、DOC のオシレータ、DSP_PROGRAM）は先頭の番地。']
    for blk in m.blocks:
        out += ['', '; ' + '-' * 66 + ' ' + heading(blk)]
        out += ['; ' + s for s in blk.note]
        if blk.has_regs():
            out.append('')
        if blk.base:
            out += equ(blk.base)
        for rg in blk.regions:
            out += equ(sym[rg.name])
            if rg.end:
                out += equ(sym[rg.end])
        if blk.bank:
            k = blk.bank
            for n in k.names:
                out += equ(sym[n])
            out += equ(sym[k.stride_name])
            out += ['; ' + k.title] + ['; ' + s for s in k.doc]
            for r in k.regs:
                out += equ(sym[r.name])
                if r.low:
                    out += equ(sym[r.low])
        for r in blk.regs:
            out += equ(sym[r.name])
            if r.low:
                out += equ(sym[r.low])
        for g in blk.groups:
            out += ['', '; ' + group_heading(g)] + ['; ' + s for s in g.note]
            for c in g.items:
                out += equ(c)
    return '\n'.join(out) + '\n'


# ---------------------------------------------------------------- fc85.h

def c_value(c):
    if c.expr:
        return f'({c.expr})'
    if c.kind == 'addr':
        return f'0x{c.value:06X}UL'
    if c.digits == 0:
        return str(c.value)
    return f'0x{c.value:0{c.digits}X}U'


def c_define(name, value, doc=None):
    s = f'#define {name:<22} {value}'
    if doc:
        s = f'{s:<45} /* {doc} */'
    return s


def c_const(c):
    return c_define(c.name, c_value(c), join_lines(c.doc))


def c_reg_macros(r, bank):
    io = {8: 'FC85_IO8', 16: 'FC85_IO16', 32: 'FC85_IO32'}[r.bits]
    if bank:
        base = f'{bank.names[0]} + {bank.stride_name} * (ch)'
        out = [c_define(f'REG_{r.name}(ch)', f'{io}({base} + {r.name})')]
        if r.low:
            out.append(c_define(f'REG_{r.low}(ch)', f'FC85_IO16({base} + {r.low})'))
        return out
    if r.count > 1:
        return [c_define(f'REG_{r.name}(n)', f'{io}({r.name} + {r.stride}UL * (n))')]
    out = [c_define(f'REG_{r.name}', f'{io}({r.name})')]
    if r.low:
        out.append(c_define(f'REG_{r.low}', f'FC85_IO16({r.low})'))
    return out


def gen_h(m):
    sym = m.symbols
    out = [f'/* 生成物。直すときは {SRC_REL}（{REGEN} で作り直す。{DR}）。手で直さない。',
           f' * fc85.h：{m.meta["title"]}。C（m68k-elf-gcc）用。',
           ' *']
    out += [' * ' + s for s in m.meta['intro']]
    out += [' *', ' * ' + m.meta['rules_title']]
    for r in m.meta['rules']:
        out += [' * - ' + r[0]] + [' *   ' + s for s in r[1:]]
    out += [' * 書式：名前と値は fc85.inc と同じ。REG_名前 はそのレジスタ（volatile のポインタを通した左辺値）。',
            ' * 並び（PALETTE、AXIS0、DOC のオシレータ、DSP_PROGRAM）は REG_名前(n)、Data Fabric のチャネルの',
            ' * レジスタは REG_名前(ch)。8 bit の装置は上位バイトレーンの偶数番地を 1 byte で読み書きする（FC85_IO8）。',
            ' */',
            '#ifndef FC85_H',
            '#define FC85_H',
            '',
            '#include <stdint.h>',
            '',
            '/* 番地 a の 8・16・32 bit のレジスタ（volatile のポインタを通した左辺値） */',
            '#define FC85_IO8(a)  (*(volatile uint8_t *)(uintptr_t)(a))',
            '#define FC85_IO16(a) (*(volatile uint16_t *)(uintptr_t)(a))',
            '#define FC85_IO32(a) (*(volatile uint32_t *)(uintptr_t)(a))']
    for blk in m.blocks:
        out += ['', f'/* {"-" * 60} {heading(blk)} */']
        if blk.note:
            out.append(f'/* {join_lines(blk.note)} */')
        if blk.has_regs():
            out.append('')
        if blk.base:
            out.append(c_const(blk.base))
        for rg in blk.regions:
            out.append(c_const(sym[rg.name]))
            if rg.end:
                out.append(c_const(sym[rg.end]))
        if blk.bank:
            k = blk.bank
            for n in k.names:
                out.append(c_const(sym[n]))
            out.append(c_const(sym[k.stride_name]))
            out.append(f'/* {k.title}。{join_lines(k.doc)} */' if k.doc else f'/* {k.title} */')
            for r in k.regs:
                out.append(c_const(sym[r.name]))
                if r.low:
                    out.append(c_const(sym[r.low]))
                out += c_reg_macros(r, k)
        for r in blk.regs:
            out.append(c_const(sym[r.name]))
            if r.low:
                out.append(c_const(sym[r.low]))
            out += c_reg_macros(r, None)
        for g in blk.groups:
            note = f'。{join_lines(g.note)}' if g.note else ''
            out += ['', f'/* {group_heading(g)}{note} */']
            out += [c_const(c) for c in g.items]
    out += ['', '#endif /* FC85_H */']
    return '\n'.join(out) + '\n'


# ---------------------------------------------------------------- regmap.rs

def rs_type(c):
    if c.kind in ('addr', 'offset', 'bit', 'number'):
        return 'u32'
    return {8: 'u8', 16: 'u16', 32: 'u32'}[c.width]


def rs_value(c):
    if c.kind == 'addr':
        return f'0x{c.value:06X}'
    if c.digits == 0:
        return str(c.value)
    return f'0x{c.value:0{c.digits}X}'


def rs_str(s):
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"') + '"'


def rs_const(c):
    doc = join_lines(c.doc)
    if c.expr:
        doc = f'{doc}（{c.expr}）' if doc else c.expr
    out = [f'/// {doc}'] if doc else []
    return out + [f'pub const {c.name}: {rs_type(c)} = {rs_value(c)};']


RS_TYPES = '''\
/// アクセスの向き。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {{
    /// r：{r}。
    R,
    /// w：{w}。
    W,
    /// rw：{rw}。
    Rw,
}}

impl Access {{
    /// 読出しの向きがあるか。
    pub const fn reads(self) -> bool {{
        matches!(self, Self::R | Self::Rw)
    }}

    /// 書込みの向きがあるか。
    pub const fn writes(self) -> bool {{
        matches!(self, Self::W | Self::Rw)
    }}
}}

/// 使える条件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {{
{when}
}}

/// 窓の中で表にない番地。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unlisted {{
    /// {berr}。
    BusError,
    /// {chip}。
    Chip,
}}

/// メモリの領域のアクセスの幅。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {{
    /// byte と語。
    Any,
    /// 語だけ（byte はバスエラー）。
    Word,
    /// 上位バイトレーン（偶数番地の byte）。
    Upper,
}}

/// アドレスマップの窓。窓の外の番地はバスエラー。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {{
    /// 名前（メモリは領域の名前）。
    pub name: &'static str,
    /// 最初の番地。
    pub first: u32,
    /// 最後の番地（含む）。
    pub last: u32,
    /// 窓の中で表にない番地。
    pub unlisted: Unlisted,
    /// 使える条件。満たさなければ窓はバスエラー。
    pub when: When,
}}

/// メモリの領域。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {{
    /// 名前（fc85.inc と同じ）。
    pub name: &'static str,
    /// 最初の番地。
    pub first: u32,
    /// 最後の番地（含む）。
    pub last: u32,
    /// アクセスの向き。
    pub access: Access,
    /// アクセスの幅。
    pub width: Width,
    /// 使える条件。
    pub when: When,
}}

/// レジスタ。並び（`count` 個）は `stride` byte ごと。Data Fabric のチャネルのレジスタは、チャネル 0 の番地と、
/// チャネルの数と間隔で表す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Register {{
    /// 名前（fc85.inc と同じ。Data Fabric のチャネルのレジスタは offset の名前）。
    pub name: &'static str,
    /// 番地（並びは最初の要素）。
    pub addr: u32,
    /// 幅：8 は上位バイトレーンの 1 byte（偶数番地）、16 は語、32 は 2 語。
    pub bits: u8,
    /// アクセスの向き。
    pub access: Access,
    /// 並びの要素の数。
    pub count: u32,
    /// 並びの要素の間隔（byte）。
    pub stride: u32,
    /// W1C のビット（1 を書くと消える）。
    pub w1c: u16,
    /// 読むと変わること。
    pub read_effect: Option<&'static str>,
    /// 使える条件。満たさなければバスエラー。
    pub when: When,
}}

impl Register {{
    /// 並びの要素の番地。
    pub fn addrs(&self) -> impl Iterator<Item = u32> {{
        let (addr, stride) = (self.addr, self.stride);
        (0..self.count).map(move |i| addr + i * stride)
    }}

    /// 1 要素がバスの上で占める byte 数（8 bit の装置も語の枠を占める）。
    pub const fn span(&self) -> u32 {{
        if self.bits == 32 {{ 4 }} else {{ 2 }}
    }}
}}
'''


def gen_rs(m):
    when_lines = []
    for k, v in m.when.items():
        when_lines += [f'    /// {k}：{v}。', f'    {camel(k)},']
    out = ['//! 本体の番地とレジスタ（68000 から見た MMIO）。生成したファイルで、手で直さない。',
           '//!',
           f'//! 元は `{SRC_REL}`（{DR}）。作り直すには、リポジトリの根で `{REGEN}` を実行する。古くないかは',
           f'//! `{REGEN} --check` で確かめる。名前と値は `sdk/include/fc85.inc`・`fc85.h` と同じ。',
           '//! 表（`WINDOWS`、`REGIONS`、`REGISTERS`）は、試験 `tests/regmap.rs` が本体のバス（`console::System`）の',
           '//! 解読と突き合わせる。',
           '',
           RS_TYPES.format(r=m.access['r'], w=m.access['w'], rw=m.access['rw'], when='\n'.join(when_lines),
                           berr=m.unlisted['berr'], chip=m.unlisted['chip']).rstrip('\n'),
           '',
           '/// アドレスマップの窓（番地の順）。',
           'pub const WINDOWS: &[Window] = &[']
    for w in m.windows():
        out.append(f'    Window {{ name: {rs_str(w.name)}, first: 0x{w.first:06X}, last: 0x{w.last:06X}, '
                   f'unlisted: Unlisted::{UNLISTED_RS[w.unlisted]}, when: When::{camel(w.when)} }},')
    out += ['];', '', '/// メモリの領域。', 'pub const REGIONS: &[Region] = &[']
    for r in m.regions():
        out.append(f'    Region {{ name: {rs_str(r.name)}, first: 0x{r.addr:06X}, last: 0x{r.last:06X}, '
                   f'access: Access::{camel(r.access)}, width: Width::{camel(r.width)}, when: When::{camel(r.when)} }},')
    out += ['];', '', '/// レジスタ。', 'pub const REGISTERS: &[Register] = &[']
    for r in m.regs():
        eff = f'Some({rs_str(r.read_effect)})' if r.read_effect else 'None'
        out.append(f'    Register {{ name: {rs_str(r.name)}, addr: 0x{r.addr:06X}, bits: {r.bits}, '
                   f'access: Access::{camel(r.access)}, count: {r.count}, stride: 0x{r.stride:X}, '
                   f'w1c: 0x{r.w1c:04X}, read_effect: {eff}, when: When::{camel(r.when)} }},')
    out.append('];')
    sym = m.symbols
    for blk in m.blocks:
        out += ['', f'// {"-" * 60} {heading(blk)}']
        if blk.note:
            out.append(f'// {join_lines(blk.note)}')
        if blk.has_regs():
            out.append('')
        if blk.base:
            out += rs_const(blk.base)
        for rg in blk.regions:
            out += rs_const(sym[rg.name])
            if rg.end:
                out += rs_const(sym[rg.end])
        if blk.bank:
            k = blk.bank
            for n in k.names:
                out += rs_const(sym[n])
            out += rs_const(sym[k.stride_name])
            out.append(f'// {k.title}。{join_lines(k.doc)}' if k.doc else f'// {k.title}')
            for r in k.regs:
                out += rs_const(sym[r.name])
                if r.low:
                    out += rs_const(sym[r.low])
        for r in blk.regs:
            out += rs_const(sym[r.name])
            if r.low:
                out += rs_const(sym[r.low])
        for g in blk.groups:
            note = f'。{join_lines(g.note)}' if g.note else ''
            out += ['', f'// {group_heading(g)}{note}']
            for c in g.items:
                out += rs_const(c)
    return '\n'.join(out) + '\n'


# ---------------------------------------------------------------- registers.md

def md(s):
    return s.replace('|', '\\|')


def md_value(c):
    v = str(c.value) if c.digits == 0 and c.kind != 'addr' else f'0x{c.value:0{max(c.digits, 2)}X}'
    return f'{v}（{md(c.expr)}）' if c.expr and c.kind != 'addr' else v


def md_bits(bits):
    return {8: '8（上位レーン）', 16: '16', 32: '32（2 語）'}[bits]


def md_when(m, w):
    return '' if w == 'always' else w


def md_place(r, bank):
    if bank:
        return f'+0x{r.offset:02X}'
    a = f'0x{r.addr:06X}'
    if r.count > 1:
        last = r.addr + (r.count - 1) * r.stride
        return f'{a}〜0x{last:06X}（{r.count} 個、{r.stride} ごと）'
    if r.index is not None:
        return f'{a}（native 0x{r.index:02X}）'
    return a


def md_reg_rows(rs, bank):
    rows = ['| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |', '|---|---|---|---|---|---|---|---|']
    for r in rs:
        names = f'`{r.name}`' + (f'、`{r.low}`（下位語）' if r.low else '')
        status = f' [{r.status}]' if r.status else ''
        refs = f'（{"、".join(r.refs)}）' if r.refs else ''
        rows.append(f'| {names} | {md_place(r, bank)} | {md_bits(r.bits)} | {r.access} | '
                    f'{f"0x{r.w1c:04X}" if r.w1c else ""} | {md(r.read_effect or "")} | {r.when if r.when != "always" else ""} | '
                    f'{md(join_lines(r.doc))}{status}{refs} |')
    return rows


def gen_md(m):
    out = ['# 本体の番地とレジスタ',
           '',
           f'> 生成物。直すときは [registers.toml](registers.toml)（`{REGEN}`。{DR}）。番地・幅・アクセス・ビットの'
           '名前と値はこの表（registers.toml）が正で、動作の説明と根拠は[仕様書](hardware.md) §05・§06・§09 と DR。'
           '同じ名前と値が `sdk/include/fc85.inc`（m68kasm）、`sdk/include/fc85.h`（C）、'
           '`emulator/crates/board/src/regmap.rs`（Rust）に出る。',
           '',
           f'{m.meta["title"]}。記述ステータスは見出しの印に従う（多くは [P]）。',
           '',
           '## 01　アクセスの規約',
           '',
           m.meta['rules_title'].rstrip('：'),
           '']
    out += ['- ' + join_lines(r) for r in m.meta['rules']]
    out += ['', '| アクセス | 意味 |', '|---|---|']
    out += [f'| {k} | {md(v)} |' for k, v in m.access.items()]
    out += ['', '| 条件 | 意味 |', '|---|---|']
    out += [f'| {k} | {md(v)} |' for k, v in m.when.items() if k != 'always']
    out += ['', '| 窓の中で表にない番地 | 意味 |', '|---|---|']
    out += [f'| {k} | {md(v)} |' for k, v in m.unlisted.items()]
    out += ['', '## 02　アドレスマップ', '', '窓の外の番地はバスエラー。', '',
            '| 番地（hex） | 窓 | 内容 | 条件 | 表にない番地 |', '|---|---|---|---|---|']
    for w in m.windows():
        out.append(f'| {w.first:06X}–{w.last:06X} | {md(w.name)} | {md(join_lines(w.doc))} | '
                   f'{md_when(m, w.when)} | {w.unlisted} |')
    out.append('| その他 | 予約 | 将来の拡張 |  | berr |')
    sym = m.symbols
    for i, blk in enumerate(m.blocks):
        out += ['', f'## {i + 3:02d}　{heading(blk)}', '']
        if blk.note:
            out += [join_lines(blk.note), '']
        if blk.base:
            out += [f'先頭：`{blk.base.name}`＝0x{blk.base.value:06X}。', '']
        if blk.regions:
            out += ['| 名前 | 番地（hex） | 大きさ | アクセス | 幅 | 条件 | 内容 |', '|---|---|---|---|---|---|---|']
            for r in blk.regions:
                names = f'`{r.name}`' + (f'（終わりの次は `{r.end}`）' if r.end else '')
                size = (f'{r.bytes >> 20} MiB' if r.bytes % (1 << 20) == 0 else
                        f'{r.bytes >> 10} KiB' if r.bytes % 1024 == 0 else f'{r.bytes:,} byte')
                refs = f'（{"、".join(r.refs)}）' if r.refs else ''
                inside = f'。{r.within} の中' if r.within else ''
                out.append(f'| {names} | {r.addr:06X}–{r.last:06X} | {size} | {r.access} | {r.width} | '
                           f'{md_when(m, r.when)} | {md(join_lines(r.doc))}{inside}{refs} |')
            out.append('')
        if blk.bank:
            k = blk.bank
            chans = '、'.join(f'`{n}`＝0x{sym[n].value:06X}' for n in k.names)
            out += [f'{k.title}：{chans}（間隔 `{k.stride_name}`＝0x{k.stride:02X}）。{md(join_lines(k.doc))}', '']
            out += md_reg_rows(k.regs, k) + ['']
        if blk.regs:
            out += md_reg_rows(blk.regs, None) + ['']
        for g in blk.groups:
            out += [f'### {group_heading(g)}', '']
            lead = []
            if g.of:
                lead.append(f'対象：`{g.of}`。')
            if g.note:
                lead.append(join_lines(g.note))
            if lead:
                out += [''.join(lead), '']
            out += ['| 名前 | 値 | 内容 |', '|---|---|---|']
            out += [f'| `{c.name}` | {md_value(c)} | {md(join_lines(c.doc))} |' for c in g.items]
            out.append('')
    text = '\n'.join(out).strip('\n')
    while '\n\n\n' in text:
        text = text.replace('\n\n\n', '\n\n')
    return text + '\n'


# ---------------------------------------------------------------- 出力

def outputs(m):
    """生成物の道 → 中身。"""
    return {OUT['inc']: gen_inc(m), OUT['h']: gen_h(m), OUT['rs']: gen_rs(m), OUT['md']: gen_md(m)}


def stale(outs):
    """置いてあるファイルが作り直したものと違えば、(リポジトリからの道, 差) の並び。"""
    bad = []
    for path, want in outs.items():
        have = path.read_bytes().decode('utf-8') if path.exists() else ''
        if have != want:
            rel = path.relative_to(ROOT).as_posix()
            diff = list(difflib.unified_diff(have.splitlines(), want.splitlines(), f'{rel}（今）', f'{rel}（作り直し）',
                                             lineterm='', n=1))
            bad.append((rel, diff))
    return bad


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv not in ([], ['--check']):
        print(__doc__, file=sys.stderr)
        return 2
    try:
        m = load()
    except MapError as e:
        print(f'{SRC_REL}：{e}', file=sys.stderr)
        return 1
    outs = outputs(m)
    if argv == ['--check']:
        bad = stale(outs)
        for rel, diff in bad:
            print(f'{rel} が古い', file=sys.stderr)
            for line in diff[:60]:
                print(line, file=sys.stderr)
            if len(diff) > 60:
                print(f'…（ほか {len(diff) - 60} 行）', file=sys.stderr)
        if bad:
            print(f'作り直す：{REGEN}', file=sys.stderr)
            return 1
        print(f'ok：{len(m.symbols)} 個の名前、{len(m.regs())} 個のレジスタ、生成物は今の表と同じ', file=sys.stderr)
        return 0
    for path, text in outs.items():
        old = path.read_bytes().decode('utf-8') if path.exists() else None
        if old != text:
            with open(path, 'w', encoding='utf-8', newline='\n') as f:
                f.write(text)
            print(f'{path.relative_to(ROOT).as_posix()} を書いた', file=sys.stderr)
    return 0


if __name__ == '__main__':
    sys.exit(main())
