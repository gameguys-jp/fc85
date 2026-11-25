#!/usr/bin/env python3
"""m68kasm: 68000 のアセンブラ（Motorola の書式）。Python 3 の標準ライブラリだけを使う。

使い方:
    python m68kasm.py src.s -o out.bin [-l out.lst] [-I dir] [-D NAME=VALUE]
                      [--base N] [--size N] [--fill N]

要点:
- 68000 の全命令と全アドレッシングモード。サイズ .B/.W/.L（省略時は .W）、分岐の .S（.B）/.W。
  MOVE→MOVEA、ADD・SUB→ADDA・SUBA・ADDI・SUBI、CMP→CMPA・CMPI・CMPM、AND・OR・EOR→ANDI・ORI・
  EORI（CCR・SR を含む）は、オペランドから選ぶ。ADD #n,Dn は ADDI ではなく ADD の形になる。ADDQ・
  SUBQ・MOVEQ への置き換えはしない（明示して書く）。
- オペランド: Dn An (An) (An)+ -(An) (d,An) d(An) (d,An,Xn[.W|.L]) d(An,Xn) abs abs.W abs.L
  (abs).W (abs).L (expr,PC) expr(PC) (expr,PC,Xn) #imm SR CCR USP、レジスタの並び D0-D3/A0/A5。
  PC 相対の expr は飛び先の番地で、変位はアセンブラが計算する（(PC) と (PC,Xn) は変位 0）。
  MOVEM は #mask（D0 が bit 0 の並び）も書ける。
- 数: 10 進、$ 16 進、% 2 進、0x 16 進、'A' 文字定数（4 文字まで）、* は文の先頭の番地。
- 演算子（優先順位の低い順）: 比較（= == != <> < > <= >=、真は 1）、|、^、&、<< >>、+ -、* / %、
  単項 - + ~ !。割り算は 0 の向きへ切り捨てる。
- ラベルは 1 桁目から（: は任意）。1 桁目に命令を書かない。インデントしたラベルには : を付ける。
  . で始まるラベルは、直前の大域ラベルの中だけで見える（外からは global.local で参照できる）。
- 注釈は ; から行末まで。1 桁目の * は行全体が注釈。
- 指令: ORG EQU = SET DC.B/W/L DCB.B/W/L DS.B/W/L EVEN ALIGN INCLUDE INCBIN END
  IF IFDEF IFNDEF ELSE ENDIF REPT ENDR RSRESET RSSET RS.B/W/L ASSERT。DS・EVEN・ALIGN は番地を
  進めるだけで、バイトを出さない（空きは --fill の値になる）。DC.B の文字列は ASCII と半角カナ
  （JIS X 0201 の 0xA1〜0xDF）。
- 分岐（Bcc、BRA、BSR）と、大きさを書かない絶対番地は、多パスの緩和で短い形と長い形を選ぶ
  （最初は短い形で、届かなければ長い形にする）。大きさは増える向きにだけ変わるので、必ず収束する。
  絶対番地の短い形（.W）は 0〜$7FFF と $FFFF8000〜$FFFFFFFF。
- 番地の計算は 32 bit で回り込む（分岐と PC 相対の変位）。
- 出力は平らなバイナリと、リスト（番地、バイト列、行、ソース、記号の表）。出力の範囲は、出した
  バイトの最小の番地（または --base）から最大の番地（または --size）まで。
- 検証: SingleStepTests 680x0 の 68000/v1 の 1,000,060 件で、命令の語と拡張語が全件一致した
  （sst.py、test_m68kasm.py の SingleStepTests）。
"""

import argparse
import os
import re
import sys

__all__ = ["AsmError", "Assembler", "assemble_text", "assemble_line", "main"]

MAX_PASSES = 32


class AsmError(Exception):
    """アセンブルの誤り。where は (ファイル, 行番号)。"""

    def __init__(self, msg, where=None):
        self.msg = msg
        self.where = where
        if where:
            super().__init__(f"{where[0]}:{where[1]}: {msg}")
        else:
            super().__init__(msg)


# ---------------------------------------------------------------------------
# 文字の符号（JIS X 0201）

def char_code(c):
    """1 文字を 8 bit の符号にする。ASCII と、半角カナ（U+FF61〜U+FF9F → 0xA1〜0xDF）、¥、‾。"""
    o = ord(c)
    if o < 0x80:
        return o
    if 0xFF61 <= o <= 0xFF9F:
        return o - 0xFF61 + 0xA1
    if c == "¥":
        return 0x5C
    if c == "‾":
        return 0x7E
    if o <= 0xFF:
        return o
    raise AsmError(f"文字 {c!r} は 8 bit の符号にできない")


def wrap32(v):
    """32 bit で回り込む差を、符号付きの値にする（68000 の番地の計算は 32 bit）。"""
    return ((v + 0x80000000) & 0xFFFFFFFF) - 0x80000000


def unquote(tok):
    q = tok[0]
    body = tok[1:-1].replace(q + q, q)
    return body


# ---------------------------------------------------------------------------
# 式

_BINOPS = [
    {"=", "==", "!=", "<>", "<", ">", "<=", ">="},
    {"|"},
    {"^"},
    {"&"},
    {"<<", ">>"},
    {"+", "-"},
    {"*", "/", "%"},
]


def tokenize_expr(s):
    """式を字句に分ける。* と % は、値を待つ位置ではそれぞれ現在番地と 2 進数の接頭辞。"""
    toks = []
    i = 0
    n = len(s)
    want_value = True
    while i < n:
        c = s[i]
        if c in " \t":
            i += 1
            continue
        if want_value:
            if c == "$":
                m = re.match(r"\$[0-9A-Fa-f]+", s[i:])
                if not m:
                    raise AsmError(f"16 進数が正しくない: {s[i:]!r}")
                toks.append(("num", int(m.group()[1:], 16)))
                i += m.end()
                want_value = False
                continue
            if c == "%":
                m = re.match(r"%[01]+", s[i:])
                if not m:
                    raise AsmError(f"2 進数が正しくない: {s[i:]!r}")
                toks.append(("num", int(m.group()[1:], 2)))
                i += m.end()
                want_value = False
                continue
            if c.isdigit():
                m = re.match(r"0[xX][0-9A-Fa-f]+|[0-9]+", s[i:])
                t = m.group()
                v = int(t, 16) if t[:2].lower() == "0x" else int(t, 10)
                toks.append(("num", v))
                i += m.end()
                want_value = False
                continue
            if c in "'\"":
                j = i + 1
                while True:
                    if j >= n:
                        raise AsmError(f"引用符が閉じていない: {s[i:]!r}")
                    if s[j] == c:
                        if j + 1 < n and s[j + 1] == c:
                            j += 2
                            continue
                        break
                    j += 1
                body = unquote(s[i:j + 1])
                if not 1 <= len(body) <= 4:
                    raise AsmError(f"文字定数は 1〜4 文字: {s[i:j + 1]}")
                v = 0
                for ch in body:
                    v = (v << 8) | char_code(ch)
                toks.append(("num", v))
                i = j + 1
                want_value = False
                continue
            if c == "*":
                toks.append(("pc", None))
                i += 1
                want_value = False
                continue
            m = re.match(r"\.?[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*"
                         r"|\.[0-9][A-Za-z0-9_]*", s[i:])
            if m:
                toks.append(("id", m.group()))
                i += m.end()
                want_value = False
                continue
            if c in "-+~!(":
                toks.append(("op", c))
                i += 1
                continue
            raise AsmError(f"式に値がない: {s[i:]!r}")
        # 演算子を待つ位置
        two = s[i:i + 2]
        if two in ("<<", ">>", "<=", ">=", "<>", "!=", "=="):
            toks.append(("op", two))
            i += 2
            want_value = True
            continue
        if c in "+-*/%&|^<>=":
            toks.append(("op", c))
            i += 1
            want_value = True
            continue
        if c == ")":
            toks.append(("op", c))
            i += 1
            continue
        raise AsmError(f"式の演算子が正しくない: {s[i:]!r}")
    return toks


class _ExprParser:
    def __init__(self, toks, env):
        self.toks = toks
        self.i = 0
        self.env = env

    def peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else (None, None)

    def take(self):
        t = self.peek()
        self.i += 1
        return t

    def parse(self, level=0):
        if level == len(_BINOPS):
            return self.unary()
        v = self.parse(level + 1)
        while True:
            k, o = self.peek()
            if k != "op" or o not in _BINOPS[level]:
                return v
            self.take()
            w = self.parse(level + 1)
            v = _apply(o, v, w)

    def unary(self):
        k, o = self.peek()
        if k == "op" and o in "-+~!":
            self.take()
            v = self.unary()
            if o == "-":
                return -v
            if o == "+":
                return v
            if o == "~":
                return ~v
            return 0 if v else 1
        if k == "op" and o == "(":
            self.take()
            v = self.parse(0)
            k2, o2 = self.take()
            if (k2, o2) != ("op", ")"):
                raise AsmError("括弧が閉じていない")
            return v
        if k == "num":
            self.take()
            return o
        if k == "pc":
            self.take()
            return self.env.pc_value()
        if k == "id":
            self.take()
            return self.env.symbol_value(o)
        raise AsmError("式が途中で終わっている")


def _apply(o, a, b):
    if o == "+":
        return a + b
    if o == "-":
        return a - b
    if o == "*":
        return a * b
    if o in ("/", "%"):
        if b == 0:
            raise AsmError("0 で割った")
        q = abs(a) // abs(b)
        if (a < 0) != (b < 0):
            q = -q
        return q if o == "/" else a - q * b
    if o == "&":
        return a & b
    if o == "|":
        return a | b
    if o == "^":
        return a ^ b
    if o == "<<":
        return a << b
    if o == ">>":
        return a >> b
    if o in ("=", "=="):
        return int(a == b)
    if o in ("!=", "<>"):
        return int(a != b)
    if o == "<":
        return int(a < b)
    if o == ">":
        return int(a > b)
    if o == "<=":
        return int(a <= b)
    if o == ">=":
        return int(a >= b)
    raise AsmError(f"演算子 {o}")


# ---------------------------------------------------------------------------
# オペランド

_DREG = {f"D{i}": i for i in range(8)}
_AREG = {f"A{i}": i for i in range(8)}
_AREG["SP"] = 7


def split_args(s):
    """カンマで分ける。括弧と引用符の中のカンマは分けない。"""
    out = []
    depth = 0
    cur = []
    q = None
    for c in s:
        if q:
            cur.append(c)
            if c == q:
                q = None
            continue
        if c in "'\"":
            q = c
            cur.append(c)
            continue
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
        if c == "," and depth == 0:
            out.append("".join(cur).strip())
            cur = []
            continue
        cur.append(c)
    last = "".join(cur).strip()
    if last or out:
        out.append(last)
    return out


class Operand:
    """解析したオペランド。kind で種類を表す。"""

    __slots__ = ("kind", "reg", "expr", "xaddr", "xreg", "xlong", "size", "text", "mask")

    def __init__(self, kind, reg=0, expr=None, text=""):
        self.kind = kind
        self.reg = reg
        self.expr = expr
        self.xaddr = False
        self.xreg = 0
        self.xlong = False
        self.size = None
        self.text = text
        self.mask = 0

    def __repr__(self):
        return f"Operand({self.kind}, {self.reg}, {self.expr!r})"


_REGLIST_RE = re.compile(
    r"^\s*(?:[DA][0-7]|SP)(?:\s*-\s*(?:[DA][0-7]|SP))?"
    r"(?:\s*/\s*(?:[DA][0-7]|SP)(?:\s*-\s*(?:[DA][0-7]|SP))?)*\s*$",
    re.I,
)


def _reg_index(name):
    u = name.strip().upper()
    if u in _DREG:
        return _DREG[u]
    if u in _AREG:
        return 8 + _AREG[u]
    raise AsmError(f"レジスタではない: {name}")


def parse_reglist(s):
    mask = 0
    for part in s.split("/"):
        if "-" in part:
            a, b = part.split("-")
            ia, ib = _reg_index(a), _reg_index(b)
            if ia > ib:
                raise AsmError(f"レジスタの範囲が逆: {part}")
            for k in range(ia, ib + 1):
                mask |= 1 << k
        else:
            mask |= 1 << _reg_index(part)
    return mask


def _is_areg(s):
    return s.strip().upper() in _AREG


def _is_pc(s):
    return s.strip().upper() == "PC"


def _parse_index(s):
    """インデックスレジスタ: Dn、An、Dn.W、Dn.L（*1 の倍率は許す）。(アドレスか, 番号, ロングか)。"""
    u = s.strip().upper().replace(" ", "")
    if u.endswith("*1"):
        u = u[:-2]
    long_ = False
    if u.endswith(".W"):
        u = u[:-2]
    elif u.endswith(".L"):
        u = u[:-2]
        long_ = True
    if u in _DREG:
        return False, _DREG[u], long_
    if u in _AREG:
        return True, _AREG[u], long_
    raise AsmError(f"インデックスレジスタが正しくない: {s}")


def _matching_open(s):
    """末尾の ) に対応する ( の位置。"""
    depth = 0
    q = None
    for i in range(len(s) - 1, -1, -1):
        c = s[i]
        if q:
            if c == q:
                q = None
            continue
        if c in "'\"":
            q = c
            continue
        if c == ")":
            depth += 1
        elif c == "(":
            depth -= 1
            if depth == 0:
                return i
    raise AsmError(f"括弧が合わない: {s}")


def parse_operand(text):
    s = text.strip()
    if not s:
        raise AsmError("オペランドが空")
    u = s.upper()
    if u in _DREG:
        return Operand("dn", _DREG[u], text=s)
    if u in _AREG:
        return Operand("an", _AREG[u], text=s)
    if u in ("SR", "CCR", "USP"):
        return Operand(u.lower(), text=s)
    if s.startswith("#"):
        return Operand("imm", expr=s[1:].strip(), text=s)
    if _REGLIST_RE.match(s) and ("/" in s or "-" in s):
        op = Operand("reglist", text=s)
        op.mask = parse_reglist(s)
        return op
    m = re.match(r"^-\s*\(\s*([A-Za-z0-9]+)\s*\)$", s)
    if m and _is_areg(m.group(1)):
        return Operand("predec", _AREG[m.group(1).upper()], text=s)
    m = re.match(r"^\(\s*([A-Za-z0-9]+)\s*\)\s*\+$", s)
    if m and _is_areg(m.group(1)):
        return Operand("postinc", _AREG[m.group(1).upper()], text=s)
    size = None
    m = re.match(r"^(.*)\.([WLwl])$", s)
    if m and not _is_areg(m.group(1)):
        size = m.group(2).upper()
        s = m.group(1).strip()
    if s.endswith(")"):
        i = _matching_open(s)
        prefix = s[:i].strip()
        parts = split_args(s[i + 1:-1])
        base = parts[0] if parts else ""
        if prefix == "" and len(parts) == 1 and not _is_areg(base) and not _is_pc(base):
            op = Operand("abs", expr=s, text=text)
            op.size = size
            return op
        if size:
            raise AsmError(f"大きさの指定ができないオペランド: {text}")
        implicit = False
        if prefix:
            disp = prefix
            regs = parts
        elif _is_areg(base) or _is_pc(base):
            disp = "0"
            implicit = True
            regs = parts
        else:
            disp = base
            regs = parts[1:]
        if not regs or len(regs) > 2:
            # 括弧を含む式（例: (1+2)*3 はここに来ない。(X)+(Y) など）
            op = Operand("abs", expr=s, text=text)
            op.size = size
            return op
        r0 = regs[0]
        if _is_pc(r0):
            # (PC) と (PC,Xn) は変位 0。(expr,PC) の expr は飛び先の番地
            op = Operand("pcdisp" if len(regs) == 1 else "pcindex",
                         expr=None if implicit else disp, text=text)
        elif _is_areg(r0):
            reg = _AREG[r0.strip().upper()]
            if len(regs) == 1:
                op = Operand("ind" if implicit else "disp", reg, expr=disp, text=text)
            else:
                op = Operand("index", reg, expr=disp, text=text)
        else:
            if prefix == "" or not regs:
                raise AsmError(f"オペランドが正しくない: {text}")
            # 例: LABEL(X) は 68000 にない。式として扱う
            op = Operand("abs", expr=s, text=text)
            op.size = size
            return op
        if len(regs) == 2:
            op.xaddr, op.xreg, op.xlong = _parse_index(regs[1])
        return op
    op = Operand("abs", expr=s, text=text)
    op.size = size
    return op


# アドレッシングモードの分類
DATA = {"dn", "ind", "postinc", "predec", "disp", "index", "abs", "pcdisp", "pcindex", "imm"}
MEMORY = DATA - {"dn"}
CONTROL = {"ind", "disp", "index", "abs", "pcdisp", "pcindex"}
ALTERABLE = {"dn", "an", "ind", "postinc", "predec", "disp", "index", "abs"}
DALT = ALTERABLE - {"an"}
MALT = ALTERABLE - {"dn", "an"}
ALL = DATA | {"an"}

SIZE_BITS = {"B": 0, "W": 1, "L": 2}
MOVE_SIZE = {"B": 1, "W": 3, "L": 2}
CC = {
    "T": 0, "F": 1, "HI": 2, "LS": 3, "CC": 4, "HS": 4, "CS": 5, "LO": 5,
    "NE": 6, "EQ": 7, "VC": 8, "VS": 9, "PL": 10, "MI": 11, "GE": 12, "LT": 13,
    "GT": 14, "LE": 15,
}
SHIFTS = {"ASR": (0, 0), "ASL": (0, 1), "LSR": (1, 0), "LSL": (1, 1),
          "ROXR": (2, 0), "ROXL": (2, 1), "ROR": (3, 0), "ROL": (3, 1)}
BITOPS = {"BTST": 0, "BCHG": 1, "BCLR": 2, "BSET": 3}
IMMOPS = {"ORI": 0x0000, "ANDI": 0x0200, "SUBI": 0x0400, "ADDI": 0x0600,
          "EORI": 0x0A00, "CMPI": 0x0C00}
SIMPLE = {"NOP": 0x4E71, "RESET": 0x4E70, "RTE": 0x4E73, "RTS": 0x4E75,
          "TRAPV": 0x4E76, "RTR": 0x4E77, "ILLEGAL": 0x4AFC}
SINGLE = {"NEGX": 0x4000, "CLR": 0x4200, "NEG": 0x4400, "NOT": 0x4600, "TST": 0x4A00}

DIRECTIVES = {
    "ORG", "EQU", "SET", "=", "DC", "DCB", "DS", "EVEN", "ALIGN", "INCLUDE", "INCBIN",
    "END", "IF", "IFDEF", "IFNDEF", "ELSE", "ENDIF", "ENDC", "REPT", "ENDR", "RSRESET",
    "RSSET", "RS", "ASSERT", "LIST", "NOLIST", "PAGE", "TTL", "OPT",
}


# ---------------------------------------------------------------------------
# 行の分解

def strip_comment(line):
    q = None
    for i, c in enumerate(line):
        if q:
            if c == q:
                q = None
            continue
        if c in "'\"":
            q = c
            continue
        if c == ";":
            return line[:i]
    return line


_IDENT = r"\.?[A-Za-z_][A-Za-z0-9_]*|\.[0-9][A-Za-z0-9_]*"


def split_line(line):
    """1 行を (ラベル, 命令, オペランド) に分ける。"""
    if line[:1] == "*":
        return None, None, ""
    s = strip_comment(line).rstrip()
    if not s.strip():
        return None, None, ""
    label = None
    rest = s
    if s[0] not in " \t":
        m = re.match(r"(" + _IDENT + r")\s*:?", s)
        if not m:
            raise AsmError(f"ラベルが正しくない: {s}")
        label = m.group(1)
        rest = s[m.end():]
    else:
        m = re.match(r"\s*(" + _IDENT + r")\s*:", s)
        if m:
            label = m.group(1)
            rest = s[m.end():]
    rest = rest.strip()
    if not rest:
        return label, None, ""
    if rest[0] == "=":
        return label, "=", rest[1:].strip()
    m = re.match(r"([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z]+)?)\s*(.*)$", rest)
    if not m:
        raise AsmError(f"命令が読めない: {rest}")
    op = m.group(1)
    args = m.group(2).strip()
    # インデントしたラベルの EQU（例: "  NAME EQU 5"）
    if label is None:
        m2 = re.match(r"(EQU|SET)\b\s*(.*)$", args, re.I)
        if m2:
            return op, m2.group(1).upper(), m2.group(2).strip()
        if args.startswith("=") and not args.startswith("=="):
            return op, "=", args[1:].strip()
    return label, op, args


# ---------------------------------------------------------------------------
# アセンブラ本体

class Assembler:
    def __init__(self, include_dirs=(), defines=None, base=None, size=None, fill=0xFF):
        self.include_dirs = list(include_dirs)
        self.defines = dict(defines or {})
        self.base = base
        self.out_size = size
        self.fill = fill
        self.file_cache = {}
        self.warnings = []
        self.prev = {}
        self.relax = {}

    # ---- 入口 ----
    def assemble_file(self, path):
        self.main_path = os.path.abspath(path)
        self.main_lines = None
        return self._run()

    def assemble_source(self, text, name="<source>", base_dir="."):
        self.main_path = name
        self.main_lines = text.splitlines()
        self.main_dir = os.path.abspath(base_dir)
        return self._run()

    def _run(self):
        self.prev = {}
        self.relax = {}
        stable = False
        for n in range(MAX_PASSES):
            self._pass(final=False)
            same = self.sym == self.prev
            self.prev = self.sym
            if n > 0 and same and not self.changed:
                stable = True
                break
        if not stable:
            raise AsmError("ラベルの値が収束しない")
        self._pass(final=True)
        if self.changed:
            raise AsmError("最後のパスで大きさが変わった（内部の誤り）")
        return self.image()

    def image(self):
        if not self.emitted:
            return b""
        lo = min(self.emitted)
        hi = max(self.emitted) + 1
        base = lo if self.base is None else self.base
        if lo < base:
            raise AsmError(f"番地 ${lo:X} が出力の先頭 ${base:X} より前にある")
        end = hi if self.out_size is None else base + self.out_size
        if hi > end:
            raise AsmError(f"出力が大きすぎる: ${hi - base:X} バイト（上限 ${end - base:X}）")
        buf = bytearray([self.fill & 0xFF]) * (end - base)
        for a, b in self.emitted.items():
            buf[a - base] = b
        return bytes(buf)

    # ---- パス ----
    def _pass(self, final):
        self.final = final
        self.sym = {}
        self.kinds = {}
        for k, v in self.defines.items():
            self.sym[k] = v
            self.kinds[k] = "equ"
        self.changed = False
        self.pc = 0
        self.stmt_pc = 0
        self.seq = 0
        self.scope = ""
        self.rs = 0
        self.emitted = {}
        self.listing = []
        self.ended = False
        self.cond = []
        self.where = None
        if self.main_lines is not None:
            self._lines(self.main_lines, self.main_path, self.main_dir)
        else:
            self._file(self.main_path)
        if self.cond:
            raise AsmError("IF が ENDIF で閉じていない")

    def _read(self, path):
        if path not in self.file_cache:
            try:
                with open(path, encoding="utf-8") as f:
                    self.file_cache[path] = f.read().splitlines()
            except OSError as e:
                raise AsmError(f"ファイルを読めない: {path}: {e.strerror}", self.where)
        return self.file_cache[path]

    def _find(self, name, cur_dir):
        cands = [os.path.join(cur_dir, name)] + [os.path.join(d, name) for d in self.include_dirs]
        for c in cands:
            if os.path.isfile(c):
                return os.path.abspath(c)
        raise AsmError(f"ファイルが見つからない: {name}", self.where)

    def _file(self, path):
        lines = self._read(path)
        self._lines(lines, path, os.path.dirname(path))

    def _active(self):
        return all(c[0] for c in self.cond)

    def _lines(self, lines, path, cur_dir, first_lineno=1, numbered=None):
        i = 0
        while i < len(lines) and not self.ended:
            lineno = numbered[i] if numbered else first_lineno + i
            self.where = (os.path.basename(path), lineno)
            try:
                i = self._line(lines, i, path, cur_dir, first_lineno, numbered)
            except AsmError as e:
                if e.where:
                    raise
                raise AsmError(e.msg, (os.path.basename(path), lineno)) from None

    def _line(self, lines, i, path, cur_dir, first_lineno, numbered):
        """lines[i] を処理し、次に処理する行の位置を返す。"""
        text = lines[i]
        i += 1
        label, op, args = split_line(text)
        opu = op.upper() if op else None
        # 条件の入れ子は、止まっている間も数える
        if opu in ("IF", "IFDEF", "IFNDEF"):
            if self._active():
                if opu == "IF":
                    v = self._eval_now(args)
                else:
                    name = self._full(args.strip())
                    v = name in self.sym or name in self.prev
                    if opu == "IFNDEF":
                        v = not v
                self.cond.append([bool(v), bool(v)])
            else:
                self.cond.append([False, True])
            self._list(self.pc, b"", text)
            return i
        if opu == "ELSE":
            if not self.cond:
                raise AsmError("ELSE に対応する IF がない")
            c = self.cond[-1]
            c[0] = not c[1]
            c[1] = True
            return i
        if opu in ("ENDIF", "ENDC"):
            if not self.cond:
                raise AsmError("ENDIF に対応する IF がない")
            self.cond.pop()
            return i
        if not self._active():
            return i
        if opu == "REPT":
            count = self._eval_now(args)
            body, nums = [], []
            depth = 1
            while i < len(lines):
                t = lines[i]
                ln = numbered[i] if numbered else first_lineno + i
                i += 1
                _, o2, _ = split_line(t)
                o2u = o2.upper() if o2 else None
                if o2u == "REPT":
                    depth += 1
                elif o2u == "ENDR":
                    depth -= 1
                    if depth == 0:
                        break
                body.append(t)
                nums.append(ln)
            else:
                raise AsmError("REPT が ENDR で閉じていない")
            for _ in range(count):
                self._lines(body, path, cur_dir, numbered=nums)
            return i
        if opu == "ENDR":
            raise AsmError("ENDR に対応する REPT がない")
        if opu == "INCLUDE":
            name = self._string_arg(args)
            if label:
                self._define_label(label)
            full = self._find(name, cur_dir)
            self._file(full)
            return i
        self._statement(label, op, args, text, cur_dir)
        return i

    def _list(self, addr, data, text):
        if self.final:
            self.listing.append((self.where, addr, bytes(data), text))

    # ---- 記号 ----
    def _full(self, name):
        if name.startswith("."):
            return self.scope + name
        return name

    def symbol_value(self, name):
        full = self._full(name)
        if full in self.sym:
            return self.sym[full]
        if full in self.prev:
            return self.prev[full]
        if self.final:
            raise AsmError(f"定義されていない記号: {name}")
        self._unknown = True
        return 0

    def pc_value(self):
        return self.stmt_pc

    def eval(self, expr):
        """式を評価して (値, 分かっているか) を返す。"""
        self._unknown = False
        if expr is None or not str(expr).strip():
            raise AsmError("式がない")
        toks = tokenize_expr(expr)
        p = _ExprParser(toks, self)
        v = p.parse(0)
        if p.i != len(toks):
            raise AsmError(f"式の終わりに余分なものがある: {expr}")
        return v, not self._unknown

    def _eval_now(self, expr):
        """その場で決まる式（IF、REPT、ORG など）。"""
        v, known = self.eval(expr)
        if not known:
            raise AsmError(f"この式は前方の記号を使えない: {expr}")
        return v

    def _define(self, name, value, kind):
        full = self._full(name)
        old = self.kinds.get(full)
        if old is not None:
            if kind == "set" and old == "set":
                pass
            elif kind == "set" or old == "set":
                raise AsmError(f"SET と EQU・ラベルを混ぜた: {name}")
            elif kind == "equ" and old == "equ" and self.sym[full] == value:
                pass
            else:
                raise AsmError(f"記号の二重定義: {name}")
        self.sym[full] = value
        self.kinds[full] = kind

    def _define_label(self, label):
        if not label.startswith("."):
            self.scope = label
        self._define(label, self.pc, "label")

    # ---- 文 ----
    def _string_arg(self, args):
        a = args.strip()
        if len(a) >= 2 and a[0] in "'\"" and a[-1] == a[0]:
            return unquote(a)
        if not a:
            raise AsmError("ファイル名がない")
        return a

    def _statement(self, label, op, args, text, cur_dir):
        self.stmt_pc = self.pc
        if op is None:
            if label:
                self._define_label(label)
            self._list(self.pc, b"", text)
            return
        mn, size = op, None
        if "." in op:
            mn, size = op.split(".", 1)
            size = size.upper()
        mnu = mn.upper()
        if mnu in ("EQU", "=", "SET"):
            if not label:
                raise AsmError(f"{mnu} にラベルがない")
            v, known = self.eval(args)
            if not known and mnu == "SET":
                raise AsmError("SET の式は前方の記号を使えない")
            if not known:
                # 前方参照の EQU は次のパスで決まる
                return
            self._define(label, v, "set" if mnu == "SET" else "equ")
            self._list(v, b"", text)
            return
        if mnu == "RS":
            if not label:
                raise AsmError("RS にラベルがない")
            unit = {"B": 1, "W": 2, "L": 4, None: 2}.get(size)
            if unit is None:
                raise AsmError(f"大きさが正しくない: {op}")
            n = self._eval_now(args) if args.strip() else 1
            if unit > 1 and self.rs & 1:
                self.rs += 1
            self._define(label, self.rs, "equ")
            self._list(self.rs, b"", text)
            self.rs += unit * n
            return
        if mnu == "ORG":
            if label:
                raise AsmError("ORG にラベルは付けられない")
            self.pc = self._eval_now(args)
            self._list(self.pc, b"", text)
            return
        if label:
            self._define_label(label)
        self.seq += 1
        if mnu in DIRECTIVES:
            self._directive(mnu, size, args, text, cur_dir)
            return
        if self.pc & 1:
            raise AsmError("命令が奇数番地にある")
        words = self.encode(mnu, size, args)
        data = bytearray()
        for w in words:
            data += bytes(((w >> 8) & 0xFF, w & 0xFF))
        self._emit(data, text)

    def _emit(self, data, text):
        if self.final:
            for k, b in enumerate(data):
                a = self.pc + k
                if a in self.emitted:
                    raise AsmError(f"番地 ${a:X} に二度書いた")
                self.emitted[a] = b
        self._list(self.pc, data, text)
        self.pc += len(data)

    def _directive(self, mnu, size, args, text, cur_dir):
        if mnu == "DC":
            unit = {"B": 1, "W": 2, "L": 4, None: 2}.get(size)
            if unit is None:
                raise AsmError(f"DC の大きさが正しくない: {size}")
            if unit > 1 and self.pc & 1:
                raise AsmError(f"DC.{size or 'W'} が奇数番地にある")
            data = bytearray()
            for a in split_args(args):
                if not a:
                    raise AsmError("DC の値が空")
                if a[0] in "'\"" and a[-1] == a[0] and len(a) >= 2 and unit == 1:
                    data += bytes(char_code(c) for c in unquote(a))
                    continue
                v, _ = self.eval(a)
                data += self._pack(v, unit)
            self._emit(data, text)
            return
        if mnu == "DCB":
            unit = {"B": 1, "W": 2, "L": 4, None: 2}.get(size)
            parts = split_args(args)
            n = self._eval_now(parts[0])
            v = self.eval(parts[1])[0] if len(parts) > 1 else 0
            if unit > 1 and self.pc & 1:
                raise AsmError("DCB が奇数番地にある")
            self._emit(self._pack(v, unit) * n, text)
            return
        if mnu == "DS":
            unit = {"B": 1, "W": 2, "L": 4, None: 2}.get(size)
            if unit is None:
                raise AsmError(f"DS の大きさが正しくない: {size}")
            if unit > 1 and self.pc & 1:
                self.pc += 1
            n = self._eval_now(args)
            self._list(self.pc, b"", text)
            self.pc += unit * n
            return
        if mnu == "EVEN":
            self.pc = (self.pc + 1) & ~1
            self._list(self.pc, b"", text)
            return
        if mnu == "ALIGN":
            n = self._eval_now(args) if args.strip() else 2
            if n <= 0:
                raise AsmError("ALIGN の値が正しくない")
            self.pc = (self.pc + n - 1) // n * n
            self._list(self.pc, b"", text)
            return
        if mnu == "INCBIN":
            parts = split_args(args)
            name = self._string_arg(parts[0])
            full = self._find(name, cur_dir)
            with open(full, "rb") as f:
                blob = f.read()
            off = self._eval_now(parts[1]) if len(parts) > 1 else 0
            ln = self._eval_now(parts[2]) if len(parts) > 2 else len(blob) - off
            self._emit(blob[off:off + ln], text)
            return
        if mnu == "END":
            self._list(self.pc, b"", text)
            self.ended = True
            return
        if mnu == "RSRESET":
            self.rs = 0
            return
        if mnu == "RSSET":
            self.rs = self._eval_now(args)
            return
        if mnu == "ASSERT":
            parts = split_args(args)
            v, known = self.eval(parts[0])
            if self.final and not v:
                msg = self._string_arg(parts[1]) if len(parts) > 1 else parts[0]
                raise AsmError(f"ASSERT が成り立たない: {msg}")
            return
        if mnu in ("LIST", "NOLIST", "PAGE", "TTL", "OPT"):
            return
        raise AsmError(f"{mnu} はここでは使えない")

    def _pack(self, v, unit):
        lo = -(1 << (8 * unit - 1))
        hi = (1 << (8 * unit)) - 1
        self.check(lo <= v <= hi, f"値 {v} が {unit} バイトに入らない")
        return (v & hi).to_bytes(unit, "big")

    def check(self, cond, msg):
        if not cond and self.final:
            raise AsmError(msg)

    # ---------------------------------------------------------------------
    # 命令の符号化

    def encode(self, mn, size, args):
        ops_text = split_args(args) if args.strip() else []
        ops = [parse_operand(t) for t in ops_text]
        return self._encode(mn, size, ops)

    def _size(self, size, allowed, default="W"):
        s = size or default
        if s not in allowed:
            raise AsmError(f"この命令に .{s} は使えない")
        return s

    def _need(self, ops, n):
        if len(ops) != n:
            raise AsmError(f"オペランドの数が違う（{n} 個のはず）")

    def _mode(self, op, allowed, what="オペランド"):
        if op.kind not in allowed:
            raise AsmError(f"{what} {op.text} はこの命令で使えない")

    def _abs_size(self, op, slot):
        if op.size:
            return op.size
        key = (self.seq, slot)
        if self.relax.get(key):
            return "L"
        v, known = self.eval(op.expr)
        if not known:
            return "W"
        if -0x8000 <= v <= 0x7FFF or 0xFFFF8000 <= v <= 0xFFFFFFFF:
            return "W"
        self.relax[key] = True
        self.changed = True
        return "L"

    def _ea(self, op, size, slot, words):
        """実効アドレス: (mode<<3|reg) を返し、拡張語を words に足す。"""
        k = op.kind
        ext_addr = self.stmt_pc + 2 * len(words)
        if k == "dn":
            return op.reg
        if k == "an":
            return 0o10 | op.reg
        if k == "ind":
            return 0o20 | op.reg
        if k == "postinc":
            return 0o30 | op.reg
        if k == "predec":
            return 0o40 | op.reg
        if k == "disp":
            d, _ = self.eval(op.expr)
            self.check(-0x8000 <= d <= 0x7FFF, f"変位 {d} が 16 bit に入らない")
            words.append(d & 0xFFFF)
            return 0o50 | op.reg
        if k == "index":
            d, _ = self.eval(op.expr)
            self.check(-0x80 <= d <= 0x7F, f"変位 {d} が 8 bit に入らない")
            words.append(self._brief(op, d))
            return 0o60 | op.reg
        if k == "abs":
            s = self._abs_size(op, slot)
            v, _ = self.eval(op.expr)
            if s == "W":
                self.check(-0x8000 <= v <= 0xFFFF or 0xFFFF8000 <= v <= 0xFFFFFFFF,
                           f"絶対番地 ${v & 0xFFFFFFFF:X} が .W に入らない")
                words.append(v & 0xFFFF)
                return 0o70
            self.check(-0x80000000 <= v <= 0xFFFFFFFF, f"絶対番地 {v} が 32 bit に入らない")
            words.append((v >> 16) & 0xFFFF)
            words.append(v & 0xFFFF)
            return 0o71
        if k == "pcdisp":
            d = 0 if op.expr is None else wrap32(self.eval(op.expr)[0] - ext_addr)
            self.check(-0x8000 <= d <= 0x7FFF, f"PC 相対の変位 {d} が 16 bit に入らない")
            words.append(d & 0xFFFF)
            return 0o72
        if k == "pcindex":
            d = 0 if op.expr is None else wrap32(self.eval(op.expr)[0] - ext_addr)
            self.check(-0x80 <= d <= 0x7F, f"PC 相対の変位 {d} が 8 bit に入らない")
            words.append(self._brief(op, d))
            return 0o73
        if k == "imm":
            v, _ = self.eval(op.expr)
            words.extend(self._imm(v, size))
            return 0o74
        raise AsmError(f"オペランド {op.text} は実効アドレスにならない")

    def _brief(self, op, d):
        return ((0x8000 if op.xaddr else 0) | (op.xreg << 12) | (0x800 if op.xlong else 0)
                | (d & 0xFF))

    def _imm(self, v, size):
        if size == "B":
            self.check(-0x80 <= v <= 0xFF, f"即値 {v} が .B に入らない")
            return [v & 0xFF]
        if size == "W":
            self.check(-0x8000 <= v <= 0xFFFF, f"即値 {v} が .W に入らない")
            return [v & 0xFFFF]
        self.check(-0x80000000 <= v <= 0xFFFFFFFF, f"即値 {v} が .L に入らない")
        return [(v >> 16) & 0xFFFF, v & 0xFFFF]

    def _no_byte_an(self, size, *ops):
        if size == "B" and any(o.kind == "an" for o in ops):
            raise AsmError("アドレスレジスタはバイトで使えない")

    def _branch(self, base, size, target_expr):
        """Bcc・BRA・BSR。base はオペコード（変位なし）。"""
        t, known = self.eval(target_expr)
        d = wrap32(t - (self.stmt_pc + 2))
        if size in ("S", "B"):
            short = True
        elif size == "W":
            short = False
        elif size is None:
            key = (self.seq, "br")
            short = not self.relax.get(key)
            if short and known and (d == 0 or not -0x80 <= d <= 0x7F):
                self.relax[key] = True
                self.changed = True
                short = False
        else:
            raise AsmError(f"分岐に .{size} は使えない（68000 は .S と .W）")
        if short:
            self.check(-0x80 <= d <= 0x7F and d != 0,
                       f"短い分岐の変位 {d} が範囲外（-128〜127、0 以外）")
            if self.final and t & 1:
                self.warnings.append(f"{self.where}: 分岐先が奇数番地")
            return [base | (d & 0xFF)]
        self.check(-0x8000 <= d <= 0x7FFF, f"分岐の変位 {d} が 16 bit に入らない")
        return [base, d & 0xFFFF]

    def _encode(self, mn, size, ops):
        # ---- 引数なし ----
        if mn in SIMPLE:
            self._need(ops, 0)
            return [SIMPLE[mn]]
        # ---- 分岐 ----
        if mn in ("BRA", "BSR") or (mn[0] == "B" and mn[1:] in CC and mn[1:] not in ("T", "F")):
            self._need(ops, 1)
            if mn == "BRA":
                cc = 0
            elif mn == "BSR":
                cc = 1
            else:
                cc = CC[mn[1:]]
            return self._branch(0x6000 | (cc << 8), size, ops[0].expr if ops[0].kind == "abs"
                                else ops[0].text)
        if mn.startswith("DB") and (mn[2:] in CC or mn[2:] == "RA"):
            self._need(ops, 2)
            if size not in (None, "W"):
                raise AsmError("DBcc は .W だけ")
            cc = 1 if mn[2:] == "RA" else CC[mn[2:]]
            self._mode(ops[0], {"dn"})
            t, _ = self.eval(ops[1].expr if ops[1].kind == "abs" else ops[1].text)
            d = wrap32(t - (self.stmt_pc + 2))
            self.check(-0x8000 <= d <= 0x7FFF, f"DBcc の変位 {d} が 16 bit に入らない")
            return [0x50C8 | (cc << 8) | ops[0].reg, d & 0xFFFF]
        if mn[0] == "S" and mn[1:] in CC:
            self._need(ops, 1)
            self._size(size, ("B",), "B")
            self._mode(ops[0], DALT)
            w = [0]
            w[0] = 0x50C0 | (CC[mn[1:]] << 8) | self._ea(ops[0], "B", 0, w)
            return w
        # ---- MOVE 系 ----
        if mn == "MOVE":
            return self._move(size, ops)
        if mn == "MOVEA":
            self._need(ops, 2)
            return self._movea(size, ops)
        if mn == "MOVEQ":
            self._need(ops, 2)
            self._size(size, ("L",), "L")
            self._mode(ops[0], {"imm"})
            self._mode(ops[1], {"dn"})
            v, _ = self.eval(ops[0].expr)
            self.check(-0x80 <= v <= 0xFF, f"MOVEQ の即値 {v} が 8 bit に入らない")
            return [0x7000 | (ops[1].reg << 9) | (v & 0xFF)]
        if mn == "MOVEM":
            return self._movem(size, ops)
        if mn == "MOVEP":
            return self._movep(size, ops)
        if mn == "LEA":
            self._need(ops, 2)
            self._size(size, ("L",), "L")
            self._mode(ops[0], CONTROL)
            self._mode(ops[1], {"an"})
            w = [0]
            w[0] = 0x41C0 | (ops[1].reg << 9) | self._ea(ops[0], "L", 0, w)
            return w
        if mn in ("PEA", "JMP", "JSR"):
            self._need(ops, 1)
            self._size(size, ("L",), "L")
            self._mode(ops[0], CONTROL)
            base = {"PEA": 0x4840, "JMP": 0x4EC0, "JSR": 0x4E80}[mn]
            w = [0]
            w[0] = base | self._ea(ops[0], "L", 0, w)
            return w
        # ---- 算術・論理 ----
        if mn in ("ADD", "SUB"):
            return self._addsub(mn, size, ops)
        if mn in ("ADDA", "SUBA", "CMPA"):
            self._need(ops, 2)
            return self._adda(mn, size, ops)
        if mn in IMMOPS:
            return self._immop(mn, size, ops)
        if mn in ("ADDQ", "SUBQ"):
            self._need(ops, 2)
            s = self._size(size, ("B", "W", "L"))
            self._mode(ops[0], {"imm"})
            self._mode(ops[1], ALTERABLE)
            self._no_byte_an(s, ops[1])
            v, _ = self.eval(ops[0].expr)
            self.check(1 <= v <= 8, f"{mn} の即値 {v} は 1〜8")
            w = [0]
            ea = self._ea(ops[1], s, 1, w)
            w[0] = (0x5000 if mn == "ADDQ" else 0x5100) | ((v & 7) << 9) | (SIZE_BITS[s] << 6) | ea
            return w
        if mn in ("ADDX", "SUBX", "ABCD", "SBCD"):
            self._need(ops, 2)
            if mn in ("ABCD", "SBCD"):
                s = self._size(size, ("B",), "B")
                base = 0xC100 if mn == "ABCD" else 0x8100
            else:
                s = self._size(size, ("B", "W", "L"))
                base = (0xD100 if mn == "ADDX" else 0x9100) | (SIZE_BITS[s] << 6)
            a, b = ops
            if a.kind == "dn" and b.kind == "dn":
                return [base | (b.reg << 9) | a.reg]
            if a.kind == "predec" and b.kind == "predec":
                return [base | 0x0008 | (b.reg << 9) | a.reg]
            raise AsmError(f"{mn} は Dy,Dx か -(Ay),-(Ax)")
        if mn in ("AND", "OR"):
            return self._andor(mn, size, ops)
        if mn == "EOR":
            self._need(ops, 2)
            if ops[0].kind == "imm":
                return self._immop("EORI", size, ops)
            s = self._size(size, ("B", "W", "L"))
            self._mode(ops[0], {"dn"})
            self._mode(ops[1], DALT)
            w = [0]
            w[0] = 0xB100 | (ops[0].reg << 9) | (SIZE_BITS[s] << 6) | self._ea(ops[1], s, 1, w)
            return w
        if mn == "CMP":
            return self._cmp(size, ops)
        if mn == "CMPM":
            self._need(ops, 2)
            s = self._size(size, ("B", "W", "L"))
            self._mode(ops[0], {"postinc"})
            self._mode(ops[1], {"postinc"})
            return [0xB108 | (ops[1].reg << 9) | (SIZE_BITS[s] << 6) | ops[0].reg]
        if mn in ("MULU", "MULS", "DIVU", "DIVS", "CHK"):
            self._need(ops, 2)
            self._size(size, ("W",))
            self._mode(ops[0], DATA)
            self._mode(ops[1], {"dn"})
            base = {"MULU": 0xC0C0, "MULS": 0xC1C0, "DIVU": 0x80C0, "DIVS": 0x81C0,
                    "CHK": 0x4180}[mn]
            w = [0]
            w[0] = base | (ops[1].reg << 9) | self._ea(ops[0], "W", 0, w)
            return w
        if mn in SINGLE:
            self._need(ops, 1)
            s = self._size(size, ("B", "W", "L"))
            self._mode(ops[0], DALT)
            w = [0]
            w[0] = SINGLE[mn] | (SIZE_BITS[s] << 6) | self._ea(ops[0], s, 0, w)
            return w
        if mn in ("NBCD", "TAS"):
            self._need(ops, 1)
            self._size(size, ("B",), "B")
            self._mode(ops[0], DALT)
            w = [0]
            w[0] = (0x4800 if mn == "NBCD" else 0x4AC0) | self._ea(ops[0], "B", 0, w)
            return w
        if mn == "EXT":
            self._need(ops, 1)
            s = self._size(size, ("W", "L"))
            self._mode(ops[0], {"dn"})
            return [(0x4880 if s == "W" else 0x48C0) | ops[0].reg]
        if mn == "SWAP":
            self._need(ops, 1)
            self._size(size, ("W",))
            self._mode(ops[0], {"dn"})
            return [0x4840 | ops[0].reg]
        if mn == "EXG":
            self._need(ops, 2)
            self._size(size, ("L",), "L")
            a, b = ops
            if a.kind == "dn" and b.kind == "dn":
                return [0xC140 | (a.reg << 9) | b.reg]
            if a.kind == "an" and b.kind == "an":
                return [0xC148 | (a.reg << 9) | b.reg]
            if a.kind == "dn" and b.kind == "an":
                return [0xC188 | (a.reg << 9) | b.reg]
            if a.kind == "an" and b.kind == "dn":
                return [0xC188 | (b.reg << 9) | a.reg]
            raise AsmError("EXG はレジスタどうし")
        if mn in SHIFTS:
            return self._shift(mn, size, ops)
        if mn in BITOPS:
            return self._bitop(mn, size, ops)
        if mn == "LINK":
            self._need(ops, 2)
            self._size(size, ("W",))
            self._mode(ops[0], {"an"})
            self._mode(ops[1], {"imm"})
            v, _ = self.eval(ops[1].expr)
            self.check(-0x8000 <= v <= 0xFFFF, f"LINK の変位 {v} が 16 bit に入らない")
            return [0x4E50 | ops[0].reg, v & 0xFFFF]
        if mn == "UNLK":
            self._need(ops, 1)
            self._mode(ops[0], {"an"})
            return [0x4E58 | ops[0].reg]
        if mn == "TRAP":
            self._need(ops, 1)
            self._mode(ops[0], {"imm"})
            v, _ = self.eval(ops[0].expr)
            self.check(0 <= v <= 15, f"TRAP の番号 {v} は 0〜15")
            return [0x4E40 | (v & 15)]
        if mn == "STOP":
            self._need(ops, 1)
            self._mode(ops[0], {"imm"})
            v, _ = self.eval(ops[0].expr)
            self.check(-0x8000 <= v <= 0xFFFF, f"STOP の即値 {v} が 16 bit に入らない")
            return [0x4E72, v & 0xFFFF]
        raise AsmError(f"知らない命令: {mn}")

    # ---- 個々の命令 ----
    def _move(self, size, ops):
        self._need(ops, 2)
        a, b = ops
        if a.kind == "sr":
            self._size(size, ("W",))
            self._mode(b, DALT)
            w = [0]
            w[0] = 0x40C0 | self._ea(b, "W", 1, w)
            return w
        if b.kind == "sr":
            self._size(size, ("W",))
            self._mode(a, DATA)
            w = [0]
            w[0] = 0x46C0 | self._ea(a, "W", 0, w)
            return w
        if b.kind == "ccr":
            self._size(size, ("B", "W"))
            self._mode(a, DATA)
            w = [0]
            w[0] = 0x44C0 | self._ea(a, "W", 0, w)
            return w
        if a.kind == "usp":
            self._size(size, ("L",), "L")
            self._mode(b, {"an"})
            return [0x4E68 | b.reg]
        if b.kind == "usp":
            self._size(size, ("L",), "L")
            self._mode(a, {"an"})
            return [0x4E60 | a.reg]
        if b.kind == "an":
            return self._movea(size, ops)
        s = self._size(size, ("B", "W", "L"))
        self._mode(a, ALL)
        self._mode(b, DALT)
        self._no_byte_an(s, a)
        w = [0]
        src = self._ea(a, s, 0, w)
        dst = self._ea(b, s, 1, w)
        dmode, dreg = dst >> 3, dst & 7
        w[0] = (MOVE_SIZE[s] << 12) | (dreg << 9) | (dmode << 6) | src
        return w

    def _movea(self, size, ops):
        s = self._size(size, ("W", "L"))
        self._mode(ops[0], ALL)
        self._mode(ops[1], {"an"})
        w = [0]
        w[0] = ((3 if s == "W" else 2) << 12) | (ops[1].reg << 9) | 0x0040 | self._ea(ops[0], s, 0, w)
        return w

    def _reglist(self, op):
        if op.kind == "reglist":
            return op.mask
        if op.kind == "dn":
            return 1 << op.reg
        if op.kind == "an":
            return 1 << (8 + op.reg)
        if op.kind == "imm":
            v, _ = self.eval(op.expr)
            self.check(0 <= v <= 0xFFFF, f"レジスタのマスク {v} が 16 bit に入らない")
            return v & 0xFFFF
        raise AsmError(f"レジスタの並びではない: {op.text}")

    def _movem(self, size, ops):
        self._need(ops, 2)
        s = self._size(size, ("W", "L"))
        a, b = ops
        szbit = 0x40 if s == "L" else 0
        regs_first = a.kind in ("reglist", "dn", "an", "imm") and b.kind not in ("reglist",)
        if regs_first and b.kind in CONTROL | {"predec"} - {"pcdisp", "pcindex"}:
            if b.kind in ("pcdisp", "pcindex"):
                raise AsmError("MOVEM の書込み先に PC 相対は使えない")
            mask = self._reglist(a)
            if b.kind == "predec":
                mask = int(f"{mask:016b}"[::-1], 2)
            w = [0, mask]
            w[0] = 0x4880 | szbit | self._ea(b, s, 1, w)
            return w
        if a.kind in CONTROL | {"postinc"}:
            mask = self._reglist(b)
            w = [0, mask]
            w[0] = 0x4C80 | szbit | self._ea(a, s, 0, w)
            return w
        raise AsmError("MOVEM のオペランドが正しくない")

    def _movep(self, size, ops):
        self._need(ops, 2)
        s = self._size(size, ("W", "L"))
        a, b = ops
        szbit = 0x40 if s == "L" else 0
        if a.kind == "dn" and b.kind in ("disp", "ind"):
            w = [0x0188 | szbit | (a.reg << 9) | b.reg]
            d = self.eval(b.expr)[0] if b.kind == "disp" else 0
            self.check(-0x8000 <= d <= 0x7FFF, f"変位 {d} が 16 bit に入らない")
            return w + [d & 0xFFFF]
        if a.kind in ("disp", "ind") and b.kind == "dn":
            w = [0x0108 | szbit | (b.reg << 9) | a.reg]
            d = self.eval(a.expr)[0] if a.kind == "disp" else 0
            self.check(-0x8000 <= d <= 0x7FFF, f"変位 {d} が 16 bit に入らない")
            return w + [d & 0xFFFF]
        raise AsmError("MOVEP は Dx,(d16,Ay) か (d16,Ay),Dx")

    def _addsub(self, mn, size, ops):
        self._need(ops, 2)
        a, b = ops
        if b.kind == "an":
            return self._adda(mn + "A", size, ops)
        if a.kind == "imm" and b.kind != "dn":
            return self._immop(mn + "I", size, ops)
        s = self._size(size, ("B", "W", "L"))
        base = 0xD000 if mn == "ADD" else 0x9000
        if b.kind == "dn":
            self._mode(a, ALL)
            self._no_byte_an(s, a)
            w = [0]
            w[0] = base | (b.reg << 9) | (SIZE_BITS[s] << 6) | self._ea(a, s, 0, w)
            return w
        self._mode(a, {"dn"})
        self._mode(b, MALT)
        w = [0]
        w[0] = base | 0x0100 | (a.reg << 9) | (SIZE_BITS[s] << 6) | self._ea(b, s, 1, w)
        return w

    def _adda(self, mn, size, ops):
        s = self._size(size, ("W", "L"))
        self._mode(ops[0], ALL)
        self._mode(ops[1], {"an"})
        base = {"ADDA": 0xD0C0, "SUBA": 0x90C0, "CMPA": 0xB0C0}[mn]
        w = [0]
        w[0] = base | (0x100 if s == "L" else 0) | (ops[1].reg << 9) | self._ea(ops[0], s, 0, w)
        return w

    def _immop(self, mn, size, ops):
        self._need(ops, 2)
        a, b = ops
        self._mode(a, {"imm"})
        base = IMMOPS[mn]
        if b.kind in ("ccr", "sr"):
            if mn not in ("ORI", "ANDI", "EORI"):
                raise AsmError(f"{mn} は CCR・SR に使えない")
            v, _ = self.eval(a.expr)
            if b.kind == "ccr":
                self._size(size, ("B",), "B")
                self.check(-0x80 <= v <= 0xFF, f"CCR の即値 {v} が 8 bit に入らない")
                return [base | 0x3C, v & 0xFF]
            self._size(size, ("W",))
            self.check(-0x8000 <= v <= 0xFFFF, f"SR の即値 {v} が 16 bit に入らない")
            return [base | 0x7C, v & 0xFFFF]
        s = self._size(size, ("B", "W", "L"))
        self._mode(b, DALT)
        w = [0]
        v, _ = self.eval(a.expr)
        w.extend(self._imm(v, s))
        w[0] = base | (SIZE_BITS[s] << 6) | self._ea(b, s, 1, w)
        return w

    def _andor(self, mn, size, ops):
        self._need(ops, 2)
        a, b = ops
        if a.kind == "imm" and b.kind != "dn":
            return self._immop(mn + "I", size, ops)
        s = self._size(size, ("B", "W", "L"))
        base = 0xC000 if mn == "AND" else 0x8000
        if b.kind == "dn":
            self._mode(a, DATA)
            w = [0]
            w[0] = base | (b.reg << 9) | (SIZE_BITS[s] << 6) | self._ea(a, s, 0, w)
            return w
        self._mode(a, {"dn"})
        self._mode(b, MALT)
        w = [0]
        w[0] = base | 0x0100 | (a.reg << 9) | (SIZE_BITS[s] << 6) | self._ea(b, s, 1, w)
        return w

    def _cmp(self, size, ops):
        self._need(ops, 2)
        a, b = ops
        if b.kind == "an":
            return self._adda("CMPA", size, ops)
        if a.kind == "postinc" and b.kind == "postinc":
            return self._encode("CMPM", size, ops)
        if a.kind == "imm" and b.kind != "dn":
            return self._immop("CMPI", size, ops)
        s = self._size(size, ("B", "W", "L"))
        self._mode(a, ALL)
        self._mode(b, {"dn"})
        self._no_byte_an(s, a)
        w = [0]
        w[0] = 0xB000 | (b.reg << 9) | (SIZE_BITS[s] << 6) | self._ea(a, s, 0, w)
        return w

    def _shift(self, mn, size, ops):
        typ, left = SHIFTS[mn]
        if len(ops) == 1 and ops[0].kind != "dn":
            self._size(size, ("W",))
            self._mode(ops[0], MALT)
            w = [0]
            w[0] = 0xE0C0 | (typ << 9) | (left << 8) | self._ea(ops[0], "W", 0, w)
            return w
        s = self._size(size, ("B", "W", "L"))
        if len(ops) == 1:
            cnt, dst = None, ops[0]
        else:
            self._need(ops, 2)
            cnt, dst = ops
        self._mode(dst, {"dn"})
        if cnt is None:
            c, ir = 1, 0
        elif cnt.kind == "imm":
            c, _ = self.eval(cnt.expr)
            self.check(1 <= c <= 8, f"シフトの回数 {c} は 1〜8")
            ir = 0
        elif cnt.kind == "dn":
            c, ir = cnt.reg, 1
        else:
            raise AsmError("シフトの回数は #n か Dn")
        return [0xE000 | ((c & 7) << 9) | (left << 8) | (SIZE_BITS[s] << 6) | (ir << 5)
                | (typ << 3) | dst.reg]

    def _bitop(self, mn, size, ops):
        self._need(ops, 2)
        t = BITOPS[mn]
        a, b = ops
        if size not in (None, "B", "L"):
            raise AsmError(f"{mn} は .B（メモリ）か .L（Dn）")
        if b.kind == "dn" and size == "B":
            raise AsmError(f"{mn} の Dn は .L")
        if b.kind != "dn" and size == "L":
            raise AsmError(f"{mn} のメモリは .B")
        if a.kind == "dn":
            allowed = DATA if mn == "BTST" else DALT
            self._mode(b, allowed)
            w = [0]
            w[0] = 0x0100 | (a.reg << 9) | (t << 6) | self._ea(b, "B", 1, w)
            return w
        if a.kind == "imm":
            allowed = (DATA - {"imm"}) if mn == "BTST" else DALT
            self._mode(b, allowed)
            v, _ = self.eval(a.expr)
            self.check(0 <= v <= 0xFF, f"ビットの番号 {v} が 8 bit に入らない")
            w = [0, v & 0xFF]
            w[0] = 0x0800 | (t << 6) | self._ea(b, "B", 1, w)
            return w
        raise AsmError(f"{mn} のビットの番号は Dn か #n")

    # ---- リスト ----
    def listing_text(self):
        out = []
        for where, addr, data, text in self.listing:
            ln = where[1] if where else 0
            hexs = data.hex().upper()
            first = hexs[:16]
            out.append(f"{addr & 0xFFFFFFFF:06X}  {first:<16}  {ln:5d}  {text}")
            for k in range(16, len(hexs), 16):
                out.append(f"{(addr + k // 2) & 0xFFFFFFFF:06X}  {hexs[k:k + 16]:<16}")
        out.append("")
        out.append("記号の表:")
        for name in sorted(self.sym):
            out.append(f"  {name:<32} ${self.sym[name] & 0xFFFFFFFF:08X}")
        return "\n".join(out) + "\n"


# ---------------------------------------------------------------------------
# 便利な入口

def assemble_text(text, base=None, size=None, fill=0xFF, include_dirs=(), defines=None):
    """文字列のソースをアセンブルしてバイト列を返す。"""
    a = Assembler(include_dirs=include_dirs, defines=defines, base=base, size=size, fill=fill)
    return a.assemble_source(text)


def assemble_line(line, pc=0):
    """1 命令（ラベルなし、記号なし）を番地 pc でアセンブルし、語の並びを返す。試験用。"""
    a = Assembler()
    a.final = True
    a.sym = {}
    a.kinds = {}
    a.prev = {}
    a.changed = False
    a.pc = a.stmt_pc = pc
    a.seq = 0
    a.scope = ""
    a.where = None
    _, op, args = split_line("\t" + line)
    mn, size = op, None
    if "." in op:
        mn, size = op.split(".", 1)
        size = size.upper()
    return a.encode(mn.upper(), size, args)


def _parse_num(s):
    s = s.strip()
    if s.startswith("$"):
        return int(s[1:], 16)
    return int(s, 0)


def main(argv=None):
    ap = argparse.ArgumentParser(description="68000 のアセンブラ（Motorola の書式）")
    ap.add_argument("source")
    ap.add_argument("-o", "--output", required=True, help="出力のバイナリ")
    ap.add_argument("-l", "--listing", help="リストのファイル")
    ap.add_argument("-I", "--include", action="append", default=[], help="INCLUDE を探すフォルダ")
    ap.add_argument("-D", "--define", action="append", default=[], help="NAME=VALUE")
    ap.add_argument("--base", help="出力の先頭の番地（省略時は最小の番地）")
    ap.add_argument("--size", help="出力の大きさ（足りない分は --fill で埋める）")
    ap.add_argument("--fill", default="0xFF", help="空きを埋める値（既定 0xFF）")
    args = ap.parse_args(argv)
    defines = {}
    for d in args.define:
        k, _, v = d.partition("=")
        defines[k] = _parse_num(v) if v else 1
    asm = Assembler(
        include_dirs=args.include,
        defines=defines,
        base=_parse_num(args.base) if args.base else None,
        size=_parse_num(args.size) if args.size else None,
        fill=_parse_num(args.fill),
    )
    try:
        img = asm.assemble_file(args.source)
    except AsmError as e:
        print(f"誤り: {e}", file=sys.stderr)
        return 1
    with open(args.output, "wb") as f:
        f.write(img)
    if args.listing:
        with open(args.listing, "w", encoding="utf-8", newline="\n") as f:
            f.write(asm.listing_text())
    for w in asm.warnings:
        print(f"注意: {w}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
