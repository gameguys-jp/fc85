"""PALASM の式（PAL に焼く中身）を読み、ピンと項に収まるかを確かめ、評価し、Rust に直す。

書き方は docs/spec/fabric.md §01 のとおり（/ は否定、* は AND、+ は OR、:= は登録出力）。

- 1 つの PAL は、見出しのコメント `; 名前 (型番)  説明` で始める。
- 次に `PIN` の行に、ピン 1 から順に信号の名前を並べる。電源は GND・VCC、使わないピンは NC。
  登録出力の PAL のピン 1 はクロック、/OE のピンは、いつも出すなら GND。
- 式は `/X = 積和`（組合せの出力）、`/X := 積和`（登録出力。クロックの立上りで取り込む）、
  `IF (積) /X = 積和`（3 ステートの出力。積が 1 のときだけ出す）。`+` で始まる行は前の式の続き。
- 出力は反転する（L 型と R 型）。左辺はいつも `/X` と書き、ピン X の値は積和の否定になる。
  式の中の名前は、どれもピンの値（正の論理）を指す。登録出力を式に使うと、レジスタの今の値（帰還）。
- `;` から行の終わりはコメント。

外部のライブラリは使わない。
"""
import re
from pathlib import Path

# ピンの働き：I 入力、O 出力（帰還なし）、B 出力か入力（帰還あり）、R 登録出力（帰還あり）、
# C クロック、E 出力の許可（/OE。登録出力に効く）、G GND、V VCC。
# MMI の PAL の標準の構成 [D MMIPAL83]、[D MMI85 p.5-31]（review-digital.md）。
DEVICES = {
    'PAL16L8': 'IIIIIIIIIGIOBBBBBBOV',
    'PAL16R8': 'CIIIIIIIIGERRRRRRRRV',
    'PAL16R6': 'CIIIIIIIIGEBRRRRRRBV',
    'PAL16R4': 'CIIIIIIIIGEBBRRRRBBV',
    'PAL20L8': 'IIIIIIIIIIIGIIOBBBBBBOIV',
    'PAL20R8': 'CIIIIIIIIIIGEIRRRRRRRRIV',
    'PAL20R6': 'CIIIIIIIIIIGEIBRRRRRRBIV',
    'PAL20R4': 'CIIIIIIIIIIGEIBBRRRRBBIV',
}
# 1 出力の積の数：組合せの出力は 7（ほかに出力の許可の積が 1 つ）、登録出力は 8。
TERMS = {'O': 7, 'B': 7, 'R': 8}
# Rust で真理値表にする出力の入力の数の上限（それより多い式は積和のまま）。
TABLE_MAX = 10

NAME = r'[A-Za-z][A-Za-z0-9_]*'
HEADER = re.compile(r'^;\s*(' + NAME + r')\s*\((PAL(?:16|20)[LR][468])(A-2|A)?\)')
EQ = re.compile(r'^(?:IF\s*\(([^)]*)\)\s*)?/(' + NAME + r')\s*(:=|=)\s*(.*)$')


class PalError(Exception):
    pass


def parse_term(text, where):
    lits = []
    for lt in text.split('*'):
        lt = lt.strip()
        m = re.fullmatch(r'(/?)(' + NAME + ')', lt)
        if not m:
            raise PalError(f'{where}: 項が読めない：{text!r}')
        lits.append((m.group(2), m.group(1) != '/'))
    names = [n for n, _ in lits]
    if len(set(names)) != len(names):
        raise PalError(f'{where}: 同じ入力が 1 つの項に 2 回ある：{text!r}')
    return tuple(lits)


def parse_sop(text, where):
    return [parse_term(t, where) for t in text.split('+')]


class Equation:
    def __init__(self, out, registered, terms, enable, text):
        self.out = out
        self.registered = registered
        self.terms = terms          # [((名前, 正か), ...), ...]
        self.enable = enable        # 出力の許可の積（なければ None）
        self.text = text            # 書いたとおりの式（行の続きを含む）


class Pal:
    """1 つの PAL。"""

    def __init__(self, name, part, grade, comment):
        self.name = name
        self.part = part            # 'PAL20L8' など
        self.grade = grade or ''    # 'A'、'A-2'
        self.comment = comment
        self.pins = []
        self.eqs = []

    @property
    def roles(self):
        return DEVICES[self.part]

    def pin_of(self, name):
        return self.pins.index(name) + 1

    def check(self):
        """ピンの割当てと項の数が型番に収まるかを確かめる。"""
        roles = self.roles
        w = self.name
        if len(self.pins) != len(roles):
            raise PalError(f'{w}: {self.part} のピンは {len(roles)} 本（PIN の行は {len(self.pins)} 本）')
        seen = set()
        for p, (n, r) in enumerate(zip(self.pins, roles), 1):
            if r == 'G' and n != 'GND' or r == 'V' and n != 'VCC':
                raise PalError(f'{w}: ピン {p} は {"GND" if r == "G" else "VCC"}')
            if n in ('GND', 'VCC', 'NC'):
                # クロックはつなぐ。/OE は GND か信号（開けると出力が止まる）。
                if r in 'CE' and n != 'GND' or (n != 'NC' and r not in 'GVE'):
                    raise PalError(f'{w}: ピン {p} に {n} は置けない')
                continue
            if n in seen:
                raise PalError(f'{w}: ピン名 {n} が 2 回ある')
            seen.add(n)
        outs = {}
        for e in self.eqs:
            if e.out not in self.pins:
                raise PalError(f'{w}: 出力 {e.out} のピンがない')
            r = roles[self.pin_of(e.out) - 1]
            if e.registered and r != 'R' or not e.registered and r not in 'OB':
                raise PalError(f'{w}: {e.out}（ピン {self.pin_of(e.out)}）は'
                               f'{"登録出力" if e.registered else "組合せの出力"}のピンではない')
            if e.out in outs:
                raise PalError(f'{w}: {e.out} の式が 2 つある')
            outs[e.out] = e
            if e.enable is not None and e.registered:
                raise PalError(f'{w}: 登録出力 {e.out} に IF は書けない（/OE のピンで決まる）')
            if len(e.terms) > TERMS[r]:
                raise PalError(f'{w}: {e.out} の積は {len(e.terms)} 個（{TERMS[r]} 個まで）')
        for p, (n, r) in enumerate(zip(self.pins, roles), 1):
            if r == 'R' and n not in ('NC',) and n not in outs:
                raise PalError(f'{w}: 登録出力のピン {p}（{n}）に式がない')
            if r == 'O' and n != 'NC' and n not in outs:
                raise PalError(f'{w}: 出力だけのピン {p}（{n}）に式がない')
        for e in self.eqs:
            for t in e.terms + ([e.enable] if e.enable else []):
                for n, _ in t:
                    if n not in self.pins:
                        raise PalError(f'{w}: {e.out} の式の {n} はピンにない')
                    r = roles[self.pin_of(n) - 1]
                    if r in 'CEGV':
                        raise PalError(f'{w}: {n}（ピン {self.pin_of(n)}）は積の入力にならない')
                    if r == 'O':
                        raise PalError(f'{w}: {n}（ピン {self.pin_of(n)}）は帰還のない出力')
        self.comb_order()

    def outputs(self):
        return {e.out: e for e in self.eqs}

    def inputs(self):
        """積に入る外からの信号（ピンの順）。登録出力の帰還は含めない。"""
        outs = self.outputs()
        used = {n for e in self.eqs for t in e.terms + ([e.enable] if e.enable else []) for n, _ in t}
        return [n for n in self.pins if n in used and n not in outs]

    def feedback(self):
        """積に入る登録出力（レジスタの今の値）。"""
        outs = self.outputs()
        used = {n for e in self.eqs for t in e.terms + ([e.enable] if e.enable else []) for n, _ in t}
        return [n for n in self.pins if n in used and n in outs and outs[n].registered]

    def comb_order(self):
        """組合せの出力を、帰還に使う順に並べる（輪になっていれば誤り）。"""
        outs = self.outputs()
        comb = [e for e in self.eqs if not e.registered]
        deps = {e.out: {n for t in e.terms + ([e.enable] if e.enable else []) for n, _ in t
                        if n in outs and not outs[n].registered} for e in comb}
        order, done = [], set()
        while len(order) < len(comb):
            ready = [e for e in comb if e.out not in done and deps[e.out] <= done]
            if not ready:
                raise PalError(f'{self.name}: 組合せの出力の帰還が輪になっている')
            for e in ready:
                order.append(e)
                done.add(e.out)
        return order

    def usage(self):
        """使ったピンと項の数。"""
        roles = self.roles
        outs = self.outputs()
        n_in = len(self.inputs())
        n_out = len(outs)
        max_t = max((len(e.terms) for e in self.eqs), default=0)
        n_io_in = sum(1 for n, r in zip(self.pins, roles) if r == 'B' and n not in outs and n != 'NC')
        n_dedicated = roles.count('I')
        free = sum(1 for n, r in zip(self.pins, roles) if n == 'NC' and r in 'IBOR')
        return {'inputs': n_in, 'outputs': n_out, 'max_terms': max_t,
                'io_as_input': n_io_in, 'dedicated_inputs': n_dedicated, 'free_pins': free,
                'clock': self.pins[0] if roles[0] == 'C' else None}

    # ------------------------------------------------------------ 評価

    def eval(self, env):
        """env：入力と登録出力の今の値（名前 → bool）。返す値：組合せの出力の値と許可、
        登録出力の次の値（クロックの後のピンの値）。"""
        env = dict(env)
        comb, oe, nxt = {}, {}, {}
        for e in self.comb_order():
            v = not any(all(env[n] == pos for n, pos in t) for t in e.terms)
            comb[e.out] = v
            oe[e.out] = e.enable is None or all(env[n] == pos for n, pos in e.enable)
            env[e.out] = v
        for e in self.eqs:
            if e.registered:
                nxt[e.out] = not any(all(env[n] == pos for n, pos in t) for t in e.terms)
        return comb, oe, nxt


class Compiled:
    """速く評価するための形。ピンの値を int の bit（ピン 1 が bit 0）で持ち、積を (mask, 値) にする。
    bus の値からピンを作るのは `pack`、出力のピンから bus の値を読むのは `unpack`。"""

    def __init__(self, pal):
        self.pal = pal
        idx = {n: i for i, n in enumerate(pal.pins)}

        def tv(t):
            m = v = 0
            for n, pos in t:
                m |= 1 << idx[n]
                v |= pos << idx[n]
            return m, v

        self.comb = [(1 << idx[e.out], [tv(t) for t in e.terms], tv(e.enable) if e.enable else None)
                     for e in pal.comb_order()]
        self.reg = [(1 << idx[e.out], [tv(t) for t in e.terms]) for e in pal.eqs if e.registered]
        self.buses = {}
        for i, n in enumerate(pal.pins):
            b, bit = split_bus(n)
            self.buses.setdefault(b, []).append((i, bit))

    def pack(self, buses):
        vec = 0
        for b, v in buses.items():
            for i, bit in self.buses.get(b, ()):
                if (v >> bit & 1) if bit is not None else v:
                    vec |= 1 << i
        return vec

    def unpack(self, vec, bus):
        v = 0
        for i, bit in self.buses.get(bus, ()):
            if vec >> i & 1:
                v |= 1 << (bit or 0)
        return v

    def run(self, vec):
        """返す値：（組合せの出力のピン、出しているピン、登録出力の次の値のピン）。"""
        comb = oe = 0
        for bit, terms, en in self.comb:
            if any(vec & m == v for m, v in terms):
                vec &= ~bit
            else:
                comb |= bit
                vec |= bit
            if en is None or vec & en[0] == en[1]:
                oe |= bit
        nxt = 0
        for bit, terms in self.reg:
            if not any(vec & m == v for m, v in terms):
                nxt |= bit
        return comb, oe, nxt


def parse(text, source='<text>'):
    """PALASM の文字列を読み、PAL の並びを返す。"""
    pals = []
    cur = None
    last = None
    for i, raw in enumerate(text.splitlines(), 1):
        where = f'{source}:{i}'
        m = HEADER.match(raw)
        if m:
            cur = Pal(m.group(1), m.group(2), m.group(3), raw[m.end():].strip())
            pals.append(cur)
            last = None
            continue
        line = raw.split(';', 1)[0].strip()
        if not line:
            continue
        if cur is None:
            raise PalError(f'{where}: PAL の見出しの前に式がある')
        if line.startswith('PIN'):
            if cur.pins:
                raise PalError(f'{where}: PIN の行が 2 つある')
            cur.pins = line.split()[1:]
            continue
        if line.startswith('+'):
            if last is None:
                raise PalError(f'{where}: 続ける式がない')
            last.terms += parse_sop(line[1:], where)
            last.text += '\n' + raw.rstrip()
            continue
        m = EQ.match(line)
        if not m:
            raise PalError(f'{where}: 式が読めない：{line!r}')
        enable = parse_term(m.group(1), where) if m.group(1) is not None else None
        last = Equation(m.group(2), m.group(3) == ':=', parse_sop(m.group(4), where), enable, raw.rstrip())
        cur.eqs.append(last)
    for p in pals:
        p.check()
    return pals


def load(path):
    path = Path(path)
    return parse(path.read_text(encoding='utf-8'), path.name)


# ---------------------------------------------------------------- Rust

RUST_KEYWORDS = set('''as break const continue crate else enum extern false fn for if impl in let loop
match mod move mut pub ref return self static struct super trait true type unsafe use where while async
await dyn abstract become box do final macro override priv typeof unsized virtual yield try gen'''.split())


def split_bus(name):
    """`D15` → ('d', 15)、`WEN` → ('wen', None)。末尾の数字が bus の bit。"""
    m = re.fullmatch(r'(.*?[A-Za-z_])(\d+)', name)
    if m:
        return m.group(1).lower(), int(m.group(2))
    return name.lower(), None


def ident(s):
    return s + '_' if s in RUST_KEYWORDS else s


def camel(s):
    return ''.join(w[:1].upper() + w[1:].lower() for w in s.split('_'))


def groups(names):
    """名前を bus（u32）と 1 本の信号（bool）に分ける。返す値は [(識別子, bit か None), ...] と、
    識別子ごとの名前の並び。"""
    order = []
    members = {}
    for n in names:
        b, bit = split_bus(n)
        key = (b, bit is not None)
        if key not in members:
            members[key] = []
            order.append(key)
        members[key].append((n, bit))
    ids = [k[0] for k in order]
    if len(set(ids)) != len(ids):
        raise PalError(f'bus と 1 本の信号の名前がぶつかる：{names}')
    return [(ident(b), is_bus, members[(b, is_bus)]) for b, is_bus in order]


def rust_term(term, where):
    """1 つの積を、bus ごとの mask の比較と bool の AND にする。"""
    per_bus = {}
    parts = []
    for n, pos in term:
        b, bit = split_bus(n)
        if bit is None:
            parts.append(ident(b) if pos else '!' + ident(b))
        else:
            m, v = per_bus.get(b, (0, 0))
            per_bus[b] = (m | 1 << bit, v | (pos << bit))
    for b in sorted(per_bus):
        m, v = per_bus[b]
        parts.append(f'(({where(b)} & {m:#x}) == {v:#x})')
    return ' & '.join(parts) if parts else 'true'


def vector_shape(pal, mem):
    """bus の出力の式が、bit の位置をずらしただけの同じ形なら、その形を返す（16 bit をまとめて計算する）。
    積の中の各リテラルは、どの出力でも同じ bit なら ('fix', 名前, 正か)、出力の bit と同じだけずれるなら
    ('sh', bus, ずれ, 正か)。返す値は（積の並び、出力の許可の積か None、出力の bit の mask）。"""
    outs = pal.outputs()
    eqs = [outs[n] for n, _ in mem]
    bits = [bit for _, bit in mem]
    if len(eqs) < 2 or any(b >= 16 for b in bits):
        return None
    e0 = eqs[0]
    if any(e.registered != e0.registered or len(e.terms) != len(e0.terms)
           or (e.enable is None) != (e0.enable is None) for e in eqs):
        return None
    comb_fb = {n for n, e in outs.items() if not e.registered}

    def shape(col_terms):
        n = len(col_terms[0])
        if any(len(t) != n for t in col_terms):
            return None
        lits = []
        for j in range(n):
            col = [t[j] for t in col_terms]
            if any(c[0] in comb_fb for c in col) or len({c[1] for c in col}) != 1:
                return None
            names = [split_bus(c[0]) for c in col]
            if len({b for b, _ in names}) != 1:
                return None
            ks = [k for _, k in names]
            pos = col[0][1]
            if all(c[0] == col[0][0] for c in col):
                lits.append(('fix', col[0][0], pos))
                continue
            offs = {(k - i) % 16 for k, i in zip(ks, bits)}
            if len(offs) != 1 or any(k >= 16 for k in ks):
                return None
            lits.append(('sh', names[0][0], offs.pop(), pos))
        return lits

    terms = []
    for j in range(len(e0.terms)):
        s = shape([e.terms[j] for e in eqs])
        if s is None:
            return None
        terms.append(s)
    en = None
    if e0.enable is not None:
        en = shape([e.enable for e in eqs])
        if en is None:
            return None
    return terms, en, sum(1 << b for b in bits)


def rust_vector_lit(lit):
    if lit[0] == 'fix':
        _, n, pos = lit
        b, bit = split_bus(n)
        v = f'({ident(b)} >> {bit} & 1)' if bit is not None else f'({ident(b)} as u32)'
        return f'0u32.wrapping_sub{v}' if pos else f'{v}.wrapping_sub(1)'
    _, b, c, pos = lit
    v = f'({ident(b)} as u16).rotate_right({c}) as u32' if c else f'{ident(b)}'
    return f'({v})' if pos else f'!({v})'


def rust_fn(pal):
    """1 つの PAL を、入力の bus と信号を受けて出力を返す関数にする。"""
    name = pal.name.lower()
    outs = pal.outputs()
    args = groups(pal.inputs() + pal.feedback())
    out_groups = groups([n for n in pal.pins if n in outs])
    used_fb = {n for e in pal.eqs for t in e.terms + ([e.enable] if e.enable else []) for n, _ in t
               if n in outs and not outs[n].registered}
    vec = {}
    for i, is_bus, mem in out_groups:
        if is_bus and not any(n in used_fb for n, _ in mem):
            s = vector_shape(pal, mem)
            if s is not None:
                vec[i] = s
    st = camel(name) + 'Out'
    lines = []
    fields = []
    for i, is_bus, mem in out_groups:
        fields.append((i, is_bus, mem, False))
        if any(outs[n].enable is not None for n, _ in mem):
            fields.append((i + '_oe', is_bus, mem, True))
    doc = f'{pal.name}（{pal.part}{pal.grade}）の出力。'
    lines.append(f'/// {doc}登録出力はクロックの後のピンの値、組合せの出力は今のピンの値。')
    lines.append('#[derive(Clone, Copy, Debug, PartialEq, Eq)]')
    lines.append(f'pub struct {st} {{')
    for i, is_bus, mem, is_oe in fields:
        what = '出力の許可' if is_oe else ('登録出力' if outs[mem[0][0]].registered else '組合せの出力')
        sig = '、'.join(n for n, _ in mem)
        lines.append(f'    /// {what}：{sig}')
        lines.append(f'    pub {i}: {"u32" if is_bus else "bool"},')
    lines.append('}')
    lines.append('')
    cmt = pal.comment
    lines.append(f'/// {pal.name}（{pal.part}{pal.grade}）。{cmt}' if cmt else f'/// {pal.name}（{pal.part}{pal.grade}）。')
    if pal.roles[0] == 'C':
        lines.append(f'/// クロックは {pal.pins[0]}。')
    lines.append('#[inline]')
    lines.append('#[allow(clippy::too_many_arguments, clippy::identity_op, clippy::nonminimal_bool)]')
    sig = ', '.join(f'{i}: {"u32" if is_bus else "bool"}' for i, is_bus, _ in args)
    lines.append(f'pub fn {ident(name)}({sig}) -> {st} {{')

    def where(b):
        return ident(b)

    def bool_of(n):
        b, bit = split_bus(n)
        return f'{ident(b)}_{bit}' if bit is not None else ident(b)

    # 組合せの出力の帰還は、bus の値を作り直さずに、1 本ずつの局所の bool として使う。
    def term_code(t):
        per = []
        rest = []
        for n, pos in t:
            if n in outs and not outs[n].registered:
                rest.append((bool_of(n), pos))
            else:
                per.append((n, pos))
        code = rust_term(per, where) if per else ''
        extra = ' & '.join(v if pos else '!' + v for v, pos in rest)
        return ' & '.join(x for x in (code, extra) if x) or 'true'

    in_vec = {n for i, _, mem in out_groups if i in vec for n, _ in mem}
    consts = []

    def value_code(e):
        """出力の値。入力が少なく積が多い式は、式から作った真理値表を引く（値は式と同じ）。"""
        support = [n for n in pal.pins if any(n == m for t in e.terms for m, _ in t)]
        n_lits = sum(len(t) for t in e.terms)
        if not (len(support) <= TABLE_MAX and len(e.terms) >= 3 and n_lits >= 2 * len(support)):
            sop = ' | '.join(f'({term_code(t)})' for t in e.terms) or 'false'
            return f'!({sop})'
        bits = 0
        for i in range(1 << len(support)):
            env = {n: bool(i >> k & 1) for k, n in enumerate(support)}
            if not any(all(env[n] == pos for n, pos in t) for t in e.terms):
                bits |= 1 << i
        tname = f'{pal.name}_{e.out}_T'.upper()
        words = [(bits >> (64 * w)) & ((1 << 64) - 1) for w in range(max(1, (1 << len(support)) // 64))]
        sig = '、'.join(support)
        if len(words) == 1:
            consts.append(f'/// {pal.name} の {e.out} の真理値表（番地の bit は {sig} の順）。\n'
                          f'const {tname}: u64 = {words[0]:#018x};')
        else:
            body = ', '.join(f'{w:#018x}' for w in words)
            consts.append(f'/// {pal.name} の {e.out} の真理値表（番地の bit は {sig} の順）。\n'
                          f'const {tname}: [u64; {len(words)}] = [{body}];')
        # 番地：同じ bus の続いた bit はまとめて取り出す。
        parts = []
        k = 0
        while k < len(support):
            n = support[k]
            if n in outs and not outs[n].registered:
                parts.append(f'({bool_of(n)} as u32) << {k}')
                k += 1
                continue
            b, bit = split_bus(n)
            if bit is None:
                parts.append(f'({ident(b)} as u32) << {k}')
                k += 1
                continue
            run = 1
            while k + run < len(support):
                b2, bit2 = split_bus(support[k + run])
                if b2 != b or bit2 != bit + run or support[k + run] in outs and not outs[support[k + run]].registered:
                    break
                run += 1
            parts.append(f'({ident(b)} >> {bit} & {(1 << run) - 1:#x}) << {k}')
            k += run
        idx = ' | '.join(parts)
        if len(words) == 1:
            return f'{{ let i = {idx}; {tname} >> i & 1 != 0 }}'
        return f'{{ let i = {idx}; {tname}[(i >> 6) as usize] >> (i & 63) & 1 != 0 }}'

    # 同じ形の式の bus は、16 bit をまとめて、ずらした bus の AND・OR で計算する（式の値は 1 bit ずつと同じ）。
    for i, (terms, en, mask) in vec.items():
        sop = ' | '.join('(' + ' & '.join(rust_vector_lit(x) for x in t) + ')' for t in terms) or '0'
        lines.append(f'    let {i}_v = !({sop}) & {mask:#x};')
        if en is not None:
            lines.append(f'    let {i}_v_oe = ' + ' & '.join(rust_vector_lit(x) for x in en) + f' & {mask:#x};')
    for e in pal.comb_order():
        if e.out in in_vec:
            continue
        lines.append(f'    let {bool_of(e.out)} = {value_code(e)};')
        if e.enable is not None:
            lines.append(f'    let {bool_of(e.out)}_oe = {term_code(e.enable)};')
    for e in pal.eqs:
        if e.registered and e.out not in in_vec:
            lines.append(f'    let {bool_of(e.out)}_d = {value_code(e)};')
    lines.append(f'    {st} {{')
    for i, is_bus, mem, is_oe in fields:
        def val(n):
            e = outs[n]
            if is_oe:
                return f'{bool_of(n)}_oe' if e.enable is not None else 'true'
            return f'{bool_of(n)}_d' if e.registered else bool_of(n)
        base = i[:-3] if is_oe else i
        if base in vec:
            expr = f'{base}_v_oe' if is_oe else f'{base}_v'
        elif is_bus:
            expr = ' | '.join(f'({val(n)} as u32) << {bit}' for n, bit in mem)
        else:
            expr = val(mem[0][0])
        lines.append(f'        {i}: {expr},')
    lines.append('    }')
    lines.append('}')
    return '\n\n'.join(consts + ['\n'.join(lines)])


def rust_const_u8(name, doc, values):
    rows = []
    for i in range(0, len(values), 16):
        rows.append('    ' + ', '.join(f'0x{v:02X}' for v in values[i:i + 16]) + ',')
    return f'/// {doc}\npub const {name}: [u8; {len(values)}] = [\n' + '\n'.join(rows) + '\n];'
