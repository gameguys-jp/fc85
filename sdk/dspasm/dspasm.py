#!/usr/bin/env python3
"""dspasm: TMS32010 のアセンブラ（TI の書式）。Python 3 の標準ライブラリだけを使う。

使い方:
    python dspasm.py src.dsp -o out.bin [-l out.lst] [-I dir] [-D NAME=VALUE]
                     [--base N] [--size N] [--fill N]

要点:
- TMS32010 の全 60 命令（台帳 docs/parts/TMS32010.md §3.7・§4.3）。符号はエミュレータの解読
  （emulator/crates/chips/src/tms32010.rs）と同じで、試験（test_dspasm.py）が台帳と解読の両方と照らす。
- 番地は語（16 bit）の単位。プログラムの番地は 0〜4095（12 bit）。データの番地は 0〜143 で、直接アドレスの
  符号には下位 7 bit を入れる（ページは実行時の DP が選ぶ。アセンブラは DP を追わない）。
- オペランドの書式（TI の書式。[] は省略できる）:
    ADD・SUB・LAC          dma[,shift]   または  ind[,shift[,narp]]      shift は 0〜15
    SACH                   dma[,shift]   または  ind[,shift[,narp]]      shift は 0・1・4
    SACL                   dma[,0]       または  ind[,0[,narp]]          TMS32010 のシフトは 0 だけ
    ほかのデータの命令       dma           または  ind[,narp]
                           （ADDH ADDS AND DMOV LDP LST LT LTA LTD MAR MPY OR SST SUBC SUBH SUBS TBLR
                             TBLW XOR ZALH ZALS）
    SAR・LAR               ar,dma        または  ar,ind[,narp]
    IN・OUT                dma,pa        または  ind,pa[,narp]           pa は 0〜7
    LACK k（0〜255）、LARK ar,k（0〜255）、LARP k（0・1）、LDPK k（0・1）、MPYK k（−4096〜4095）
    分岐（B BANZ BGEZ BGZ BIOZ BLEZ BLZ BNZ BV BZ CALL）  pma（0〜4095。2 語目に入る）
    オペランドのない命令      ABS APAC CALA DINT EINT NOP PAC POP PUSH RET ROVM SOVM SPAC ZAC
  ind は `*`（そのまま）、`*+`（使った後に 1 足す）、`*-`（1 引く）。narp（0・1）を書くと、その命令の後で
  ARP をその値にする。書かなければ ARP は変わらない。データの命令のオペランドでは、`*`・`*+`・`*-` だけの
  字句は間接アドレス。ほかの式の中の `*` は文の番地（例：`B *`、`DATA *+2`）。
- 記号 AR0・AR1（0・1）と PA0〜PA7（0〜7）は前もって定義してある（TI のアセンブラと同じ）。
- 数: 10 進、$ 16 進、0x 16 進、> 16 進（値を待つ位置だけ。TI の書き方）、% 2 進、'A' 文字定数（2 文字まで）。
- 演算子（優先順位の低い順）: 比較（= == != <> < > <= >=、真は 1）、|、^、&、<< >>、+ -、* / %、
  単項 - + ~ !。割り算は 0 の向きへ切り捨てる。
- ラベルは 1 桁目から（: は任意）。インデントしたラベルには : を付ける。. で始まるラベルは、直前の大域
  ラベルの中だけで見える。注釈は ; から行末まで。1 桁目の * は行全体が注釈。
- 指令: ORG（AORG） EQU = SET DATA（WORD、DC、DC.W） DS（BSS、DS.W） ALIGN INCLUDE END
  IF IFDEF IFNDEF ELSE ENDIF REPT ENDR RSRESET RSSET RS ASSERT。DS は番地を進めるだけで語を出さない。
  RSSET・RS はデータ RAM の番地を語の単位で割り当てる（`X RS 1`）。LIST・NOLIST・PAGE・TITL・IDT は読み飛ばす。
- 出力は、語を big-endian（上位バイトが先）で並べた平らなバイナリ。既定では 0 番地から、出した語の最大の
  番地まで。空きは --fill（既定 0x0000）。リストには番地、語、サイクル数（台帳 §3.7）、行、ソースを出す。
- 警告（Assembler.warnings）: 直接アドレスが 144〜255（データ RAM の外）、直接アドレスの SST が 0〜127
  （SST の直接アドレスは DP によらずページ 1 に書く。台帳 §4.1）。
- 実行時にしか分からない規約は調べない。TBLW で 0〜7 番地に書くと OUT と同じになる（DR-029 (2)）ので、
  SDK の規約で禁じる。
"""

import argparse
import os
import re
import sys

__all__ = [
    "AsmError", "Assembler", "assemble_text", "assemble_line", "INSTRUCTIONS", "CYCLES", "main",
]

MAX_PASSES = 16
PROGRAM_WORDS = 4096
DATA_WORDS = 144


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
# 命令の表
#
# (種類, 符号の基本値)。種類ごとのオペランドは _encode で読む。符号は台帳 §4.3 の上位バイトの表と、
# エミュレータの解読（tms32010.rs の execute・execute_7f）に合わせた。

INSTRUCTIONS = {
    # データ（シフト 0〜15 を bit 11–8 に）
    "ADD": ("shift", 0x0000),
    "SUB": ("shift", 0x1000),
    "LAC": ("shift", 0x2000),
    # データ（シフトは決まった値だけ）
    "SACL": ("sacl", 0x5000),
    "SACH": ("sach", 0x5800),
    # データ（シフトなし）
    "ADDH": ("mem", 0x6000),
    "ADDS": ("mem", 0x6100),
    "SUBH": ("mem", 0x6200),
    "SUBS": ("mem", 0x6300),
    "SUBC": ("mem", 0x6400),
    "ZALH": ("mem", 0x6500),
    "ZALS": ("mem", 0x6600),
    "TBLR": ("mem", 0x6700),
    "MAR": ("mem", 0x6800),
    "DMOV": ("mem", 0x6900),
    "LT": ("mem", 0x6A00),
    "LTD": ("mem", 0x6B00),
    "LTA": ("mem", 0x6C00),
    "MPY": ("mem", 0x6D00),
    "LDP": ("mem", 0x6F00),
    "XOR": ("mem", 0x7800),
    "AND": ("mem", 0x7900),
    "OR": ("mem", 0x7A00),
    "LST": ("mem", 0x7B00),
    "SST": ("mem", 0x7C00),
    "TBLW": ("mem", 0x7D00),
    # 補助レジスタとデータ（AR を bit 8 に）
    "SAR": ("ar_mem", 0x3000),
    "LAR": ("ar_mem", 0x3800),
    # I/O（ポートを bit 10–8 に）
    "IN": ("port", 0x4000),
    "OUT": ("port", 0x4800),
    # 即値
    "LACK": ("lack", 0x7E00),
    "LARK": ("lark", 0x7000),
    "LARP": ("larp", 0x6880),
    "LDPK": ("ldpk", 0x6E00),
    "MPYK": ("mpyk", 0x8000),
    # オペランドなし（上位バイト 7F、下位バイトで細分）
    "NOP": ("none", 0x7F80),
    "DINT": ("none", 0x7F81),
    "EINT": ("none", 0x7F82),
    "ABS": ("none", 0x7F88),
    "ZAC": ("none", 0x7F89),
    "ROVM": ("none", 0x7F8A),
    "SOVM": ("none", 0x7F8B),
    "CALA": ("none", 0x7F8C),
    "RET": ("none", 0x7F8D),
    "PAC": ("none", 0x7F8E),
    "APAC": ("none", 0x7F8F),
    "SPAC": ("none", 0x7F90),
    "PUSH": ("none", 0x7F9C),
    "POP": ("none", 0x7F9D),
    # 分岐（2 語。2 語目がプログラムの番地）
    "BANZ": ("branch", 0xF400),
    "BV": ("branch", 0xF500),
    "BIOZ": ("branch", 0xF600),
    "CALL": ("branch", 0xF800),
    "B": ("branch", 0xF900),
    "BLZ": ("branch", 0xFA00),
    "BLEZ": ("branch", 0xFB00),
    "BGZ": ("branch", 0xFC00),
    "BGEZ": ("branch", 0xFD00),
    "BNZ": ("branch", 0xFE00),
    "BZ": ("branch", 0xFF00),
}

# サイクル数（台帳 §3.7、TI の表 3-2。条件分岐は成否によらず 2。DR-032）
CYCLES = {mn: 1 for mn in INSTRUCTIONS}
for _mn in ("CALA", "RET", "PUSH", "POP", "IN", "OUT"):
    CYCLES[_mn] = 2
for _mn, (_kind, _) in INSTRUCTIONS.items():
    if _kind == "branch":
        CYCLES[_mn] = 2
CYCLES["TBLR"] = 3
CYCLES["TBLW"] = 3

# 前もって定義する記号（TI のアセンブラと同じ名前）
PREDEFINED = {"AR0": 0, "AR1": 1}
PREDEFINED.update({f"PA{i}": i for i in range(8)})

# 間接アドレスの下位 8 bit（bit 7＝1、bit 5 増、bit 4 減、bit 3＝1 で ARP を変えない。台帳 §4.2）
INDIRECT = {"*": 0x88, "*+": 0xA8, "*-": 0x98}


# ---------------------------------------------------------------------------
# 文字と式

def char_code(c):
    """1 文字を 8 bit の符号にする。ASCII と、半角カナ（U+FF61〜U+FF9F → 0xA1〜0xDF）。"""
    o = ord(c)
    if o < 0x80:
        return o
    if 0xFF61 <= o <= 0xFF9F:
        return o - 0xFF61 + 0xA1
    raise AsmError(f"文字 {c!r} は 8 bit の符号にできない")


def unquote(tok):
    q = tok[0]
    return tok[1:-1].replace(q + q, q)


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
    """式を字句に分ける。値を待つ位置では、* は文の番地、% は 2 進数、> は 16 進数の接頭辞。"""
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
            m = None
            if c == "$" or c == ">":
                m = re.match(r"[$>][0-9A-Fa-f]+", s[i:])
                if not m:
                    raise AsmError(f"16 進数が正しくない: {s[i:]!r}")
                toks.append(("num", int(m.group()[1:], 16)))
            elif c == "%":
                m = re.match(r"%[01]+", s[i:])
                if not m:
                    raise AsmError(f"2 進数が正しくない: {s[i:]!r}")
                toks.append(("num", int(m.group()[1:], 2)))
            elif c.isdigit():
                m = re.match(r"0[xX][0-9A-Fa-f]+|[0-9]+", s[i:])
                t = m.group()
                toks.append(("num", int(t, 16) if t[:2].lower() == "0x" else int(t, 10)))
            if m:
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
                if not 1 <= len(body) <= 2:
                    raise AsmError(f"文字定数は 1〜2 文字: {s[i:j + 1]}")
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
            v = _apply(o, v, self.parse(level + 1))

    def unary(self):
        k, o = self.peek()
        if k == "op" and o in "-+~!":
            self.take()
            v = self.unary()
            return {"-": -v, "+": v, "~": ~v}.get(o, 0 if v else 1)
        if k == "op" and o == "(":
            self.take()
            v = self.parse(0)
            if self.take() != ("op", ")"):
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
    return int({
        "=": a == b, "==": a == b, "!=": a != b, "<>": a != b,
        "<": a < b, ">": a > b, "<=": a <= b, ">=": a >= b,
    }[o])


# ---------------------------------------------------------------------------
# 行の分解

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
    if label is None:
        m2 = re.match(r"(EQU|SET)\b\s*(.*)$", args, re.I)
        if m2:
            return op, m2.group(1).upper(), m2.group(2).strip()
    return label, op, args


DIRECTIVES = {
    "ORG", "AORG", "DATA", "WORD", "DC", "DS", "BSS", "ALIGN", "END", "RSRESET", "RSSET",
    "ASSERT", "LIST", "NOLIST", "PAGE", "TITL", "IDT", "OPT",
}


# ---------------------------------------------------------------------------
# アセンブラ本体

class Assembler:
    def __init__(self, include_dirs=(), defines=None, base=0, size=None, fill=0x0000):
        self.include_dirs = list(include_dirs)
        self.defines = dict(defines or {})
        self.base = base
        self.out_size = size
        self.fill = fill & 0xFFFF
        self.file_cache = {}
        self.warnings = []
        self.prev = {}
        self.main_lines = None
        self.main_dir = "."

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
        stable = False
        for n in range(MAX_PASSES):
            self._pass(final=False)
            same = self.sym == self.prev
            self.prev = self.sym
            if n > 0 and same:
                stable = True
                break
        if not stable:
            raise AsmError("ラベルの値が収束しない")
        self._pass(final=True)
        return self.image()

    def words(self):
        """出力の語の並び（base から）。"""
        if not self.emitted:
            return []
        lo = min(self.emitted)
        hi = max(self.emitted) + 1
        base = lo if self.base is None else self.base
        if lo < base:
            raise AsmError(f"番地 ${lo:03X} が出力の先頭 ${base:03X} より前にある")
        end = hi if self.out_size is None else base + self.out_size
        if hi > end:
            raise AsmError(f"出力が大きすぎる: {hi - base} 語（上限 {end - base} 語）")
        buf = [self.fill] * (end - base)
        for a, w in self.emitted.items():
            buf[a - base] = w
        return buf

    def image(self):
        """語を big-endian で並べたバイト列。"""
        return b"".join(w.to_bytes(2, "big") for w in self.words())

    # ---- パス ----
    def _pass(self, final):
        self.final = final
        self.sym = dict(PREDEFINED)
        self.kinds = {k: "equ" for k in PREDEFINED}
        for k, v in self.defines.items():
            self.sym[k] = v
            self.kinds[k] = "equ"
        if final:
            self.warnings = []
        self.pc = 0
        self.stmt_pc = 0
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
        self._lines(self._read(path), path, os.path.dirname(path))

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
            self._list(self.pc, [], 0, text)
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
            if label:
                self._define_label(label)
            self._file(self._find(self._string_arg(args), cur_dir))
            return i
        self._statement(label, op, args, text)
        return i

    def _list(self, addr, words, cycles, text):
        if self.final:
            self.listing.append((self.where, addr, list(words), cycles, text))

    def warn(self, msg):
        if self.final:
            w = f"{self.where[0]}:{self.where[1]}: {msg}" if self.where else msg
            self.warnings.append(w)

    # ---- 記号 ----
    def _full(self, name):
        return self.scope + name if name.startswith(".") else name

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

    def _value(self, expr):
        return self.eval(expr)[0]

    def _eval_now(self, expr):
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

    def _statement(self, label, op, args, text):
        self.stmt_pc = self.pc
        if op is None:
            if label:
                self._define_label(label)
            self._list(self.pc, [], 0, text)
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
                return              # 前方参照の EQU は次のパスで決まる
            self._define(label, v, "set" if mnu == "SET" else "equ")
            self._list(v, [], 0, text)
            return
        if mnu == "RS":
            if not label:
                raise AsmError("RS にラベルがない")
            if size not in (None, "W"):
                raise AsmError(f"RS の大きさは語だけ: {op}")
            n = self._eval_now(args) if args.strip() else 1
            self._define(label, self.rs, "equ")
            self._list(self.rs, [], 0, text)
            self.rs += n
            return
        if mnu in ("ORG", "AORG"):
            if label:
                raise AsmError("ORG にラベルは付けられない")
            v = self._eval_now(args)
            if not 0 <= v <= PROGRAM_WORDS:
                raise AsmError(f"ORG の番地が 0〜{PROGRAM_WORDS} の外: {v}")
            self.pc = v
            self._list(self.pc, [], 0, text)
            return
        if label:
            self._define_label(label)
        if mnu in DIRECTIVES:
            self._directive(mnu, size, args, text)
            return
        if size is not None:
            raise AsmError(f"命令に大きさは付けられない: {op}")
        if mnu not in INSTRUCTIONS:
            raise AsmError(f"知らない命令: {mn}")
        words = self.encode(mnu, args)
        self._emit(words, CYCLES[mnu], text)

    def _emit(self, words, cycles, text):
        if self.pc + len(words) > PROGRAM_WORDS:
            raise AsmError(f"プログラムの番地が {PROGRAM_WORDS} 語を超えた")
        if self.final:
            for k, w in enumerate(words):
                a = self.pc + k
                if a in self.emitted:
                    raise AsmError(f"番地 ${a:03X} に二度書いた")
                self.emitted[a] = w & 0xFFFF
        self._list(self.pc, words, cycles, text)
        self.pc += len(words)

    def _directive(self, mnu, size, args, text):
        if mnu in ("DATA", "WORD", "DC"):
            if size not in (None, "W"):
                raise AsmError(f"{mnu} の大きさは語だけ: {size}")
            words = []
            for a in split_args(args):
                if not a:
                    raise AsmError(f"{mnu} の値が空")
                v = self._value(a)
                self.check(-0x8000 <= v <= 0xFFFF, f"値 {v} が 16 bit に入らない")
                words.append(v & 0xFFFF)
            self._emit(words, 0, text)
            return
        if mnu in ("DS", "BSS"):
            if size not in (None, "W"):
                raise AsmError(f"{mnu} の大きさは語だけ: {size}")
            n = self._eval_now(args)
            if n < 0:
                raise AsmError(f"{mnu} の語数が負")
            self._list(self.pc, [], 0, text)
            self.pc += n
            if self.pc > PROGRAM_WORDS:
                raise AsmError(f"プログラムの番地が {PROGRAM_WORDS} 語を超えた")
            return
        if mnu == "ALIGN":
            n = self._eval_now(args) if args.strip() else 2
            if n <= 0:
                raise AsmError("ALIGN の値が正しくない")
            end = (self.pc + n - 1) // n * n
            if self.pc < end and (self.pc - 1) in self.emitted:
                for a in range(self.pc, end):
                    self.emitted[a] = self.fill
            self.pc = end
            self._list(self.pc, [], 0, text)
            return
        if mnu == "END":
            self._list(self.pc, [], 0, text)
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
            v, _ = self.eval(parts[0])
            if self.final and not v:
                msg = self._string_arg(parts[1]) if len(parts) > 1 else parts[0]
                raise AsmError(f"ASSERT が成り立たない: {msg}")
            return
        # LIST・NOLIST・PAGE・TITL・IDT・OPT は読み飛ばす

    def check(self, cond, msg):
        if not cond and self.final:
            raise AsmError(msg)

    # ---------------------------------------------------------------------
    # 命令の符号化

    def encode(self, mn, args):
        """命令 1 つを語の並びにする。"""
        ops = split_args(args) if args.strip() else []
        kind, base = INSTRUCTIONS[mn]
        return getattr(self, "_enc_" + kind)(mn, base, ops)

    def _need(self, mn, ops, lo, hi=None):
        hi = lo if hi is None else hi
        if not lo <= len(ops) <= hi:
            want = f"{lo}" if lo == hi else f"{lo}〜{hi}"
            raise AsmError(f"{mn} のオペランドは {want} 個（{len(ops)} 個ある）")

    def _range(self, what, v, lo, hi):
        self.check(lo <= v <= hi, f"{what} が {lo}〜{hi} の外: {v}")
        return v

    def _addr(self, mn, ops, narp_at):
        """データのオペランド（ops[0]）を下位 8 bit にする。間接なら ops[narp_at] を次の ARP とする。"""
        t = ops[0].strip()
        if t in INDIRECT:
            m = INDIRECT[t]
            if len(ops) > narp_at:
                narp = self._range("次の ARP", self._value(ops[narp_at]), 0, 1)
                m = (m & ~0x08) | narp
            return m
        if len(ops) > narp_at:
            raise AsmError(f"{mn}：次の ARP は間接アドレスのときだけ書ける")
        v = self._value(t)
        self._range("データの番地", v, 0, 255)
        if self.final and v >= DATA_WORDS:
            self.warn(f"{mn}：データの番地 {v} はデータ RAM（0〜{DATA_WORDS - 1}）の外")
        if self.final and mn == "SST" and v < 0x80:
            self.warn(f"SST：直接アドレスは DP によらずページ 1（{0x80 | (v & 0x7F)} 番地）に書く")
        return v & 0x7F

    def _shift(self, mn, ops, allowed):
        if len(ops) < 2:
            return 0
        v = self._value(ops[1])
        if allowed is None:
            return self._range(f"{mn} のシフト", v, 0, 15)
        self.check(v in allowed, f"{mn} のシフトは {'・'.join(map(str, allowed))} だけ: {v}")
        return v

    def _enc_shift(self, mn, base, ops):
        self._need(mn, ops, 1, 3)
        return [base | (self._shift(mn, ops, None) & 15) << 8 | self._addr(mn, ops, 2)]

    def _enc_sacl(self, mn, base, ops):
        self._need(mn, ops, 1, 3)
        self._shift(mn, ops, (0,))
        return [base | self._addr(mn, ops, 2)]

    def _enc_sach(self, mn, base, ops):
        self._need(mn, ops, 1, 3)
        return [base | (self._shift(mn, ops, (0, 1, 4)) & 7) << 8 | self._addr(mn, ops, 2)]

    def _enc_mem(self, mn, base, ops):
        self._need(mn, ops, 1, 2)
        return [base | self._addr(mn, ops, 1)]

    def _enc_ar_mem(self, mn, base, ops):
        self._need(mn, ops, 2, 3)
        ar = self._range("補助レジスタ", self._value(ops[0]), 0, 1)
        return [base | (ar & 1) << 8 | self._addr(mn, ops[1:], 1)]

    def _enc_port(self, mn, base, ops):
        self._need(mn, ops, 2, 3)
        pa = self._range("ポート", self._value(ops[1]), 0, 7)
        rest = [ops[0]] + ops[2:]
        return [base | (pa & 7) << 8 | self._addr(mn, rest, 1)]

    def _enc_lack(self, mn, base, ops):
        self._need(mn, ops, 1)
        return [base | self._range("LACK の定数", self._value(ops[0]), 0, 255) & 0xFF]

    def _enc_lark(self, mn, base, ops):
        self._need(mn, ops, 2)
        ar = self._range("補助レジスタ", self._value(ops[0]), 0, 1)
        k = self._range("LARK の定数", self._value(ops[1]), 0, 255)
        return [base | (ar & 1) << 8 | (k & 0xFF)]

    def _enc_larp(self, mn, base, ops):
        self._need(mn, ops, 1)
        return [base | self._range("ARP", self._value(ops[0]), 0, 1) & 1]

    def _enc_ldpk(self, mn, base, ops):
        self._need(mn, ops, 1)
        return [base | self._range("DP", self._value(ops[0]), 0, 1) & 1]

    def _enc_mpyk(self, mn, base, ops):
        self._need(mn, ops, 1)
        return [base | self._range("MPYK の定数", self._value(ops[0]), -4096, 4095) & 0x1FFF]

    def _enc_none(self, mn, base, ops):
        self._need(mn, ops, 0)
        return [base]

    def _enc_branch(self, mn, base, ops):
        self._need(mn, ops, 1)
        target = self._range("分岐先", self._value(ops[0]), 0, PROGRAM_WORDS - 1)
        return [base, target & 0x0FFF]

    # ---- リスト ----
    def listing_text(self):
        out = ["番地 語         サ  行     ソース"]
        total = 0
        for where, addr, words, cycles, text in self.listing:
            ln = where[1] if where else 0
            hexs = " ".join(f"{w:04X}" for w in words[:2])
            cyc = f"{cycles}" if cycles else " "
            total += cycles
            out.append(f"{addr & 0xFFF:03X}  {hexs:<9}  {cyc:>2}  {ln:5d}  {text}")
            for k in range(2, len(words), 2):
                more = " ".join(f"{w:04X}" for w in words[k:k + 2])
                out.append(f"{(addr + k) & 0xFFF:03X}  {more:<9}")
        out.append("")
        out.append("サ：命令のサイクル数（台帳 §3.7。条件分岐は成否によらず 2）。1 サイクルは 200 ns（20 MHz）")
        out.append("")
        out.append("記号の表:")
        for name in sorted(self.sym):
            if name in PREDEFINED and name not in self.defines:
                continue
            out.append(f"  {name:<32} ${self.sym[name] & 0xFFFF:04X}  {self.sym[name]}")
        return "\n".join(out) + "\n"


# ---------------------------------------------------------------------------
# 便利な入口

def assemble_text(text, base=0, size=None, fill=0x0000, include_dirs=(), defines=None):
    """文字列のソースをアセンブルしてバイト列（big-endian の語）を返す。"""
    a = Assembler(include_dirs=include_dirs, defines=defines, base=base, size=size, fill=fill)
    return a.assemble_source(text)


def assemble_line(line, pc=0, symbols=None):
    """1 命令を番地 pc でアセンブルし、語の並びを返す。試験用。"""
    a = Assembler()
    a.final = True
    a.sym = dict(PREDEFINED)
    a.sym.update(symbols or {})
    a.kinds = {k: "equ" for k in a.sym}
    a.prev = {}
    a.pc = a.stmt_pc = pc
    a.scope = ""
    a.where = None
    a.warnings = []
    _, op, args = split_line("\t" + line)
    mnu = op.upper()
    if mnu not in INSTRUCTIONS:
        raise AsmError(f"知らない命令: {op}")
    return a.encode(mnu, args)


def c_array_text(img, name, source):
    """組んだ語（big-endian のバイト列）を、C の見出し（static const uint16_t name[]）にする。DR-109。"""
    words = [int.from_bytes(img[i:i + 2], "big") for i in range(0, len(img), 2)]
    lines = [f"/* dspasm が {source} から作った TMS32010 のプログラム（{len(words)} 語）。dsp_load() で書く（dsp.h） */",
             "#include <stdint.h>",
             "",
             f"#define {name}_WORDS {len(words)}U",
             f"static const uint16_t {name}[{max(len(words), 1)}] = {{"]
    for i in range(0, len(words), 8):
        lines.append("    " + ", ".join(f"0x{w:04X}" for w in words[i:i + 8]) + ",")
    lines.append("};")
    return "\n".join(lines) + "\n"


def _parse_num(s):
    s = s.strip()
    if s.startswith("$"):
        return int(s[1:], 16)
    return int(s, 0)


def main(argv=None):
    ap = argparse.ArgumentParser(description="TMS32010 のアセンブラ（TI の書式）")
    ap.add_argument("source")
    ap.add_argument("-o", "--output", required=True, help="出力のバイナリ（語を big-endian で並べる）")
    ap.add_argument("-l", "--listing", help="リストのファイル")
    ap.add_argument("-I", "--include", action="append", default=[], help="INCLUDE を探すフォルダ")
    ap.add_argument("-D", "--define", action="append", default=[], help="NAME=VALUE")
    ap.add_argument("--base", default="0", help="出力の先頭の語の番地（既定 0）")
    ap.add_argument("--size", help="出力の語数（足りない分は --fill で埋める）")
    ap.add_argument("--fill", default="0", help="空きを埋める語（既定 0x0000）")
    ap.add_argument("--c-array", metavar="NAME",
                    help="出力をバイナリでなく C の見出しにする（static const uint16_t NAME[]、語数 NAME_WORDS。DR-109）")
    args = ap.parse_args(argv)
    if args.c_array is not None and not re.fullmatch(r"[A-Za-z_]\w*", args.c_array):
        print(f"誤り: C の名前でない: {args.c_array}", file=sys.stderr)
        return 1
    defines = {}
    for d in args.define:
        k, _, v = d.partition("=")
        defines[k] = _parse_num(v) if v else 1
    asm = Assembler(
        include_dirs=args.include,
        defines=defines,
        base=_parse_num(args.base),
        size=_parse_num(args.size) if args.size else None,
        fill=_parse_num(args.fill),
    )
    try:
        img = asm.assemble_file(args.source)
    except AsmError as e:
        print(f"誤り: {e}", file=sys.stderr)
        return 1
    if args.c_array is not None:
        with open(args.output, "w", encoding="utf-8", newline="\n") as f:
            f.write(c_array_text(img, args.c_array, os.path.basename(args.source)))
    else:
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
