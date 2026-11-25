"""SingleStepTests 680x0（68000/v1）の試験名から命令の文字列を作り、アセンブラの出力と比べる。

試験のデータは `<dir>/<名前>.json.gz`（125 個、約 193 MB）。ライセンスが付いていないので、リポジトリに
入れず、比較にだけ使う。取得: https://github.com/SingleStepTests/680x0 の 68000/v1。

試験名は "e3ae [LSL.l D1, D6] 5" の形で、先頭の 16 進が命令の語。括弧の中は命令の形だけを表し、
即値（#）、クイックの値（Q）、条件（cc）、変位の値は書かれていない。そこで、命令の語と、その後の拡張語
（initial の prefetch と ram）から値を取り出して、具体的な命令の文字列にする（正規化）。

- 条件（Bcc、DBcc、Scc）とクイックの値（Q）は命令の語から取る。ここは命令の語を写すだけなので、
  アセンブラの検証にはならない。モード・レジスタ・大きさ・opmode のビットは名前から独立に決まる。
- 拡張語は、名前に出てくるオペランドの順（MOVEM はマスクが先）に読む。
- 比べるのは、命令の語（そのまま）と、命令全体の語の並び（68000 が見ないビットを除く：簡略形の拡張語の
  bit 10–8、バイトの即値の上位バイト）。
"""

import gzip
import json
import os
import re

from m68kasm import AsmError, assemble_line, split_args

CC_NAMES = ["T", "F", "HI", "LS", "CC", "CS", "NE", "EQ", "VC", "VS", "PL", "MI",
            "GE", "LT", "GT", "LE"]
IMM_FORMS = {"ADD": "ADDI", "SUB": "SUBI", "AND": "ANDI", "OR": "ORI", "CMP": "CMPI",
             "EOR": "EORI"}
CCR_SR = {"ANDItoCCR": ("ANDI", "CCR"), "ANDItoSR": ("ANDI", "SR"),
          "EORItoCCR": ("EORI", "CCR"), "EORItoSR": ("EORI", "SR"),
          "ORItoCCR": ("ORI", "CCR"), "ORItoSR": ("ORI", "SR")}
BITOPS = ("BTST", "BCHG", "BCLR", "BSET")
SHIFTS = ("ASL", "ASR", "LSL", "LSR", "ROL", "ROR", "ROXL", "ROXR")


def sext(v, bits):
    m = 1 << (bits - 1)
    return (v & (m * 2 - 1) ^ m) - m


class Stream:
    """命令の語の並び。pc の語は prefetch[0]、pc+2 は prefetch[1]、その先は ram。"""

    def __init__(self, test):
        ini = test["initial"]
        self.pc = ini["pc"]
        self.pre = ini["prefetch"]
        self.ram = {a: v for a, v in ini["ram"]}
        self.pos = self.pc + 2  # 次に読む拡張語

    def word(self, addr):
        if addr == self.pc:
            return self.pre[0]
        if addr == self.pc + 2:
            return self.pre[1]
        return (self.ram.get(addr, 0) << 8) | self.ram.get(addr + 1, 0)

    def take(self):
        a = self.pos
        self.pos += 2
        return a, self.word(a)


def _reglist(mask, predec):
    if predec:
        mask = int(f"{mask:016b}"[::-1], 2)
    names = [f"D{i}" for i in range(8)] + [f"A{i}" for i in range(8)]
    regs = [names[i] for i in range(16) if mask >> i & 1]
    return "/".join(regs) if regs else "#0"


def normalize(test):
    """試験から (命令の文字列, 比べる語の数のマスクの並び) を作る。"""
    name = test["name"]
    m = re.match(r"^([0-9a-f]{4}) \[(.*)\]", name)
    opw = int(m.group(1), 16)
    body = m.group(2)
    mn, _, rest = body.partition(" ")
    ops = split_args(rest) if rest.strip() else []
    base, _, size = mn.partition(".")
    size = size.upper()
    st = Stream(test)
    masks = [0xFFFF]

    # 名前の言い換え
    if base in CCR_SR:
        base, reg = CCR_SR[base]
        ops = ops + [reg]
        size = "B" if reg == "CCR" else "W"
    elif base == "MOVEtoSR":
        base, ops, size = "MOVE", ops + ["SR"], "W"
    elif base == "MOVEfromSR":
        base, ops, size = "MOVE", ["SR"] + ops, "W"
    elif base == "MOVEtoCCR":
        base, ops, size = "MOVE", ops + ["CCR"], "W"
    elif base == "MOVEtoUSP":
        base, ops, size = "MOVE", ops + ["USP"], "L"
    elif base == "MOVEfromUSP":
        base, ops, size = "MOVE", ["USP"] + ops, "L"
    elif base == "UNLINK":
        base = "UNLK"
    elif base == "MOVE" and size == "Q":
        base, size = "MOVEQ", "L"
    elif base in ("Bcc", "DBcc", "Scc"):
        cc = CC_NAMES[(opw >> 8) & 15]
        if base == "Bcc":
            base = {"T": "BRA", "F": "BSR"}.get(cc, "B" + cc)
        elif base == "DBcc":
            base = "DB" + cc
        else:
            base = "S" + cc
    if base in IMM_FORMS and ops and ops[0] == "#" and opw >> 12 == 0:
        base = IMM_FORMS[base]
    if base in ("ADD", "SUB") and ops and ops[0] == "Q":
        base += "Q"

    out = []
    movem = base == "MOVEM"
    if movem:
        _, mask = st.take()
        masks.append(0xFFFF)
        predec = any(o.startswith("-(") for o in ops)
    for i, o in enumerate(ops):
        if o == "#":
            if movem:
                out.append(_reglist(mask, predec))
                continue
            if base in ("Bcc",) or base.startswith("B") and base[1:] in CC_NAMES + ["RA", "SR"]:
                a, w = st.take()
                masks.append(0xFFFF)
                out.append(f"${(st.pc + 2 + sext(w, 16)) & 0xFFFFFFFF:X}")
                continue
            if base.startswith("DB") or base == "LINK":
                a, w = st.take()
                masks.append(0xFFFF)
                if base == "LINK":
                    out.append(f"#{sext(w, 16)}")
                else:
                    out.append(f"${(st.pc + 2 + sext(w, 16)) & 0xFFFFFFFF:X}")
                continue
            if base in BITOPS:
                a, w = st.take()
                masks.append(0x00FF)
                out.append(f"#${w & 0xFF:X}")
                continue
            sz = size or "W"
            if base in ("CHK", "DIVS", "DIVU", "MULS", "MULU"):
                sz = "W"
            if sz == "L":
                _, hi = st.take()
                _, lo = st.take()
                masks += [0xFFFF, 0xFFFF]
                out.append(f"#${(hi << 16) | lo:X}")
            else:
                _, w = st.take()
                masks.append(0x00FF if sz == "B" else 0xFFFF)
                out.append(f"#${(w & 0xFF) if sz == 'B' else w:X}")
            continue
        if o == "Q":
            if base in ("ADDQ", "SUBQ") or base in SHIFTS:
                q = (opw >> 9) & 7 or 8
                out.append(f"#{q}")
            elif base == "MOVEQ":
                out.append(f"#{sext(opw, 8)}")
            elif base == "TRAP":
                out.append(f"#{opw & 15}")
            else:  # Bcc、BSR の 8 bit の変位
                out.append(f"${(st.pc + 2 + sext(opw, 8)) & 0xFFFFFFFF:X}")
            continue
        if o == "(d16, PC)":
            a, w = st.take()
            masks.append(0xFFFF)
            out.append(f"({a + sext(w, 16)},PC)")
            continue
        if o == "(d8, PC, Xn)":
            a, w = st.take()
            masks.append(0xF8FF)
            out.append(f"({a + sext(w, 8)},PC,{_xn(w)})")
            continue
        m2 = re.match(r"^\(d16, (A[0-7])\)$", o)
        if m2:
            _, w = st.take()
            masks.append(0xFFFF)
            out.append(f"({sext(w, 16)},{m2.group(1)})")
            continue
        m2 = re.match(r"^\(d8, (A[0-7]), Xn\)$", o)
        if m2:
            _, w = st.take()
            masks.append(0xF8FF)
            out.append(f"({sext(w, 8)},{m2.group(1)},{_xn(w)})")
            continue
        if o == "(xxx).w":
            _, w = st.take()
            masks.append(0xFFFF)
            out.append(f"(${w:04X}).W")
            continue
        if o == "(xxx).l":
            _, hi = st.take()
            _, lo = st.take()
            masks += [0xFFFF, 0xFFFF]
            out.append(f"(${(hi << 16) | lo:08X}).L")
            continue
        out.append(o)
    text = base + (f".{size}" if size else "") + (" " + ",".join(out) if out else "")
    return text, masks, st


def _xn(w):
    r = ("A" if w & 0x8000 else "D") + str((w >> 12) & 7)
    return r + (".L" if w & 0x800 else ".W")


def compare(test):
    """(opcode が一致したか, 全体が一致したか, 文字列, 得た語, 期待の語)。"""
    text, masks, st = normalize(test)
    try:
        got = assemble_line(text, st.pc)
    except AsmError as e:
        return False, False, text, str(e), None
    want = [st.word(st.pc + 2 * k) for k in range(len(masks))]
    op_ok = bool(got) and got[0] == want[0]
    full_ok = len(got) == len(masks) and all(
        (g & mk) == (w & mk) for g, w, mk in zip(got, want, masks))
    return op_ok, full_ok, text, got, want


def run(directory, limit=None, names=None, verbose=0):
    """ディレクトリの全ファイルを比べ、集計を返す。"""
    files = sorted(f for f in os.listdir(directory) if f.endswith(".json.gz"))
    if names:
        files = [f for f in files if f[:-8] in names]
    total = op_ok = full_ok = 0
    per_file = {}
    failures = []
    for f in files:
        with gzip.open(os.path.join(directory, f)) as fh:
            tests = json.load(fh)
        if limit:
            tests = tests[:limit]
        n = a = b = 0
        for t in tests:
            o, fu, text, got, want = compare(t)
            n += 1
            a += o
            b += fu
            if not (o and fu) and len(failures) < 200:
                failures.append((t["name"], text, got, want))
        per_file[f[:-8]] = (n, a, b)
        total += n
        op_ok += a
        full_ok += b
        if verbose:
            print(f"{f[:-8]:14} {a}/{n} {b}/{n}")
    return {"total": total, "opcode": op_ok, "full": full_ok, "files": per_file,
            "failures": failures}


if __name__ == "__main__":
    import sys
    d = sys.argv[1]
    lim = int(sys.argv[2]) if len(sys.argv) > 2 else None
    r = run(d, lim, verbose=1)
    print(f"opcode {r['opcode']}/{r['total']}  full {r['full']}/{r['total']}")
    for x in r["failures"][:40]:
        print(x)
