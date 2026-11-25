"""dspasm の試験：py -m unittest discover -s sdk/dspasm

符号は 3 つの出どころと照らす。
1. 台帳 docs/parts/TMS32010.md の §4.3（上位バイトの表と 7F の細分）、§4.2（間接アドレスのビット）、§3.7（サイクル数）。
   期待する語は、この試験の中で台帳の規則から組み立てる（アセンブラの表を使わない）。
2. エミュレータの解読 emulator/crates/chips/src/tms32010.rs の `execute`・`execute_7f` の match の腕。
   組んだ語の上位バイト（7F なら下位バイト）が、どの腕に入るかを調べ、その腕の範囲が台帳の範囲と同じことを確かめる。
3. エミュレータの試験 emulator/crates/chips/tests/tms32010.rs と board/tests/dsp.rs の `const 名前: u16 = 値;`
   （命令の符号の定数）。
リポジトリの外で SDK だけを使うときは、2 と 3 のファイルがないので飛ばす。
"""

import os
import re
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)

import dspasm  # noqa: E402
from dspasm import AsmError, Assembler, assemble_line, assemble_text  # noqa: E402

LEDGER = os.path.join(ROOT, "docs", "parts", "TMS32010.md")
DECODER = os.path.join(ROOT, "emulator", "crates", "chips", "src", "tms32010.rs")
CHIP_TESTS = os.path.join(ROOT, "emulator", "crates", "chips", "tests", "tms32010.rs")
BOARD_TESTS = os.path.join(ROOT, "emulator", "crates", "board", "tests", "dsp.rs")


def need(*paths):
    """リポジトリの外（SDK だけ）では、台帳とエミュレータのファイルがないので飛ばす。"""
    for p in paths:
        if not os.path.isfile(p):
            raise unittest.SkipTest(f"{os.path.relpath(p, ROOT)} がない")


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


# ---------------------------------------------------------------------------
# 台帳の表を読む

def table_rows(text, header):
    """`header` で始まる Markdown の表の行（区切りの行の後）を、セルの並びで返す。"""
    lines = text.splitlines()
    start = next(i for i, ln in enumerate(lines) if ln.startswith(header))
    rows = []
    for ln in lines[start + 2:]:
        if not ln.startswith("|"):
            break
        rows.append([c.strip() for c in ln.strip().strip("|").split("|")])
    return rows


def parse_hex_range(cell):
    """「00–0F」「30/31」「68」→ 上位バイトの集合。"""
    cell = cell.replace("—", "–").replace("-", "–")
    out = set()
    for part in cell.split("/"):
        if "–" in part:
            a, b = part.split("–")
            out.update(range(int(a, 16), int(b, 16) + 1))
        else:
            out.add(int(part, 16))
    return out


def ledger_opcodes():
    """§4.3 → ({命令: 上位バイトの集合}, {命令: 7F の下位バイト})。"""
    hi, lo7f = {}, {}
    for rng, desc in table_rows(read(LEDGER), "| 上位バイト | 命令 |"):
        if rng == "7F":
            for b, name in re.findall(r"([0-9A-F]{2}) ([A-Z]+)", desc):
                lo7f[name] = int(b, 16)
            continue
        for name in re.findall(r"[A-Z]+", desc):
            hi[name] = parse_hex_range(rng)
    return hi, lo7f


def ledger_cycles():
    """§3.7 → {命令: サイクル数}。"""
    out = {}
    for cyc, names in table_rows(read(LEDGER), "| サイクル | 命令 |"):
        n = int(cyc[0])
        for item in re.split(r",\s*", names):
            out[re.match(r"[A-Z]+", item.strip()).group()] = n
    return out


# ---------------------------------------------------------------------------
# エミュレータの解読を読む

_ARM = re.compile(r"^\s*(0x[0-9A-Fa-f]{2}(?:\.\.=0x[0-9A-Fa-f]{2})?(?:\s*\|\s*0x[0-9A-Fa-f]{2})*)\s*=>",
                  re.M)


def _arm_set(pattern):
    out = set()
    for part in pattern.split("|"):
        part = part.strip()
        if "..=" in part:
            a, b = part.split("..=")
            out.update(range(int(a, 16), int(b, 16) + 1))
        else:
            out.add(int(part, 16))
    return frozenset(out)


def decoder_arms():
    """tms32010.rs → (execute の腕の集合の並び, execute_7f の腕の集合の並び, 既定の腕が illegal か)。"""
    src = read(DECODER)
    main = src[src.index("fn execute(&mut self"):src.index("fn execute_7f(")]
    sub = src[src.index("fn execute_7f("):src.index("fn illegal(")]
    arms = [_arm_set(m) for m in _ARM.findall(main)]
    arms7f = [_arm_set(m) for m in _ARM.findall(sub)]
    default = "_ => self.illegal()" in main and "_ => self.illegal()" in sub
    return arms, arms7f, default


def rust_consts(path):
    """`const 名前: u16 = 0x....;` を読む。"""
    return {n: int(v, 16) for n, v in re.findall(r"const ([A-Z_0-9]+): u16 = 0x([0-9A-Fa-f]+);", read(path))}


# ---------------------------------------------------------------------------
# 台帳の規則から期待する語を組み立てる（§4.2：bit 7＝間接、bit 5 増、bit 4 減、bit 3＝0 で bit 0 を次の ARP に）

def ind(mode, narp=None):
    m = {"*": 0x80, "*+": 0xA0, "*-": 0x90}[mode]
    return m | (0x08 if narp is None else narp)


# 命令ごとの組み方：(ソース, 期待する語の並び)。上位バイトは台帳 §4.3 の範囲の先頭から計算する。
def expected_cases(hi):
    def h(name):
        return min(hi[name]) << 8

    cases = []
    for name in ("ADD", "SUB", "LAC"):
        b = h(name)
        cases += [
            (f"{name} 5", [b | 5]),
            (f"{name} 127,15", [b | 0x0F00 | 0x7F]),
            (f"{name} 130,4", [b | 0x0400 | 2]),       # ページの bit は DP が選ぶ
            (f"{name} *", [b | ind("*")]),
            (f"{name} *+,3", [b | 0x0300 | ind("*+")]),
            (f"{name} *-,0,1", [b | ind("*-", 1)]),
            (f"{name} *,15,0", [b | 0x0F00 | ind("*", 0)]),
        ]
    cases += [
        ("SACL 9", [h("SACL") | 9]),
        ("SACL 9,0", [h("SACL") | 9]),
        ("SACL *+,0,1", [h("SACL") | ind("*+", 1)]),
        ("SACH 9", [h("SACH") | 9]),
        ("SACH 9,1", [h("SACH") | 0x0100 | 9]),
        ("SACH 9,4", [h("SACH") | 0x0400 | 9]),
        ("SACH *-,4,0", [h("SACH") | 0x0400 | ind("*-", 0)]),
    ]
    for name in ("ADDH", "ADDS", "SUBH", "SUBS", "SUBC", "ZALH", "ZALS", "TBLR", "MAR", "DMOV",
                 "LT", "LTD", "LTA", "MPY", "LDP", "XOR", "AND", "OR", "LST", "TBLW"):
        b = h(name)
        cases += [
            (f"{name} 0", [b]),
            (f"{name} 127", [b | 0x7F]),
            (f"{name} *", [b | ind("*")]),
            (f"{name} *+", [b | ind("*+")]),
            (f"{name} *-,1", [b | ind("*-", 1)]),
        ]
    cases += [
        ("SST 130", [h("SST") | 2]),
        ("SST *+,0", [h("SST") | ind("*+", 0)]),
    ]
    for name in ("SAR", "LAR"):
        lo, hi1 = min(hi[name]), max(hi[name])
        cases += [
            (f"{name} AR0,7", [lo << 8 | 7]),
            (f"{name} AR1,7", [hi1 << 8 | 7]),
            (f"{name} 1,*-,0", [hi1 << 8 | ind("*-", 0)]),
        ]
    for name in ("IN", "OUT"):
        lo, top = min(hi[name]), max(hi[name])
        cases += [
            (f"{name} 3,PA0", [lo << 8 | 3]),
            (f"{name} 3,PA7", [top << 8 | 3]),
            (f"{name} *+,2,1", [(lo + 2) << 8 | ind("*+", 1)]),
        ]
    cases += [
        ("LACK 0", [h("LACK")]),
        ("LACK 255", [h("LACK") | 0xFF]),
        ("LARK AR0,200", [min(hi["LARK"]) << 8 | 200]),
        ("LARK AR1,1", [max(hi["LARK"]) << 8 | 1]),
        ("LARP 0", [h("LARP") | 0x80]),             # LARP は MAR の「* と次の ARP」と同じ符号（§4.3 の 68）
        ("LARP 1", [h("LARP") | 0x81]),
        ("LDPK 0", [h("LDPK")]),
        ("LDPK 1", [h("LDPK") | 1]),
        ("MPYK 0", [h("MPYK")]),
        ("MPYK 4095", [h("MPYK") | 0x0FFF]),
        ("MPYK -1", [h("MPYK") | 0x1FFF]),
        ("MPYK -4096", [h("MPYK") | 0x1000]),
    ]
    for name in ("B", "BANZ", "BGEZ", "BGZ", "BIOZ", "BLEZ", "BLZ", "BNZ", "BV", "BZ", "CALL"):
        cases += [
            (f"{name} 0", [h(name), 0]),
            (f"{name} 4095", [h(name), 0x0FFF]),
        ]
    return cases


def mnemonic_of(src):
    return src.split()[0]


class LedgerTest(unittest.TestCase):
    """台帳 §4.3・§4.2・§3.7 と照らす。"""

    @classmethod
    def setUpClass(cls):
        need(LEDGER)
        cls.hi, cls.lo7f = ledger_opcodes()

    def test_every_instruction_is_in_the_ledger(self):
        ledger = set(self.hi) | set(self.lo7f)
        self.assertEqual(len(ledger), 60)
        self.assertEqual(ledger, set(dspasm.INSTRUCTIONS))

    def test_operand_instructions(self):
        cases = expected_cases(self.hi)
        covered = {mnemonic_of(s) for s, _ in cases}
        self.assertEqual(covered | set(self.lo7f), set(dspasm.INSTRUCTIONS))
        for src, want in cases:
            with self.subTest(src=src):
                self.assertEqual(assemble_line(src), want)
                self.assertIn(want[0] >> 8, self.hi[mnemonic_of(src)])

    def test_no_operand_instructions(self):
        for name, lo in self.lo7f.items():
            with self.subTest(name=name):
                self.assertEqual(assemble_line(name), [0x7F00 | lo])

    def test_cycles(self):
        self.assertEqual(ledger_cycles(), dspasm.CYCLES)


class DecoderTest(unittest.TestCase):
    """エミュレータの解読（tms32010.rs）と照らす。"""

    @classmethod
    def setUpClass(cls):
        need(LEDGER, DECODER)
        cls.hi, cls.lo7f = ledger_opcodes()
        cls.arms, cls.arms7f, cls.default_illegal = decoder_arms()

    def arm_of(self, arms, b):
        found = [a for a in arms if b in a]
        self.assertEqual(len(found), 1, f"{b:02X} の腕が {len(found)} 個")
        return found[0]

    def test_decoder_structure_matches_ledger(self):
        self.assertTrue(self.default_illegal)
        ledger_sets = {frozenset(s) for s in self.hi.values()}
        self.assertEqual(set(self.arms) - {frozenset({0x7F})}, ledger_sets)
        self.assertIn(frozenset({0x7F}), self.arms)
        self.assertEqual(set(self.arms7f), {frozenset({b}) for b in self.lo7f.values()})

    def test_every_encoding_lands_in_its_arm(self):
        cases = expected_cases(self.hi) + [(n, [0x7F00 | b]) for n, b in self.lo7f.items()]
        names = set()
        for src, _ in cases:
            name = mnemonic_of(src)
            names.add(name)
            w = assemble_line(src)[0]
            with self.subTest(src=src):
                arm = self.arm_of(self.arms, w >> 8)
                if name in self.lo7f:
                    self.assertEqual(arm, {0x7F})
                    self.assertEqual(self.arm_of(self.arms7f, w & 0xFF), {self.lo7f[name]})
                else:
                    self.assertEqual(arm, self.hi[name])
        self.assertEqual(names, set(dspasm.INSTRUCTIONS))

    def test_unused_opcodes_are_illegal(self):
        used = set().union(*self.arms)
        for name, (_, base) in dspasm.INSTRUCTIONS.items():
            self.assertIn(base >> 8, used, name)
        # 51〜57（SACL のシフトの欄）は TMS32010 では解読しない
        for b in range(0x51, 0x58):
            self.assertNotIn(b, used)


class EmulatorTestConstantsTest(unittest.TestCase):
    """エミュレータの試験が使う命令の定数と同じ符号になるか。"""

    @classmethod
    def setUpClass(cls):
        need(CHIP_TESTS)

    def check_file(self, path):
        consts = rust_consts(path)
        checked = 0
        for name, value in consts.items():
            base = re.sub(r"\d+$", "", name)
            if base not in dspasm.INSTRUCTIONS:
                continue
            kind, enc = dspasm.INSTRUCTIONS[base]
            with self.subTest(file=os.path.basename(path), name=name):
                if value >= 0x100:
                    if kind == "none":
                        self.assertEqual(assemble_line(base), [value])
                    else:
                        self.assertEqual(kind, "branch")
                        self.assertEqual(assemble_line(f"{base} 0x123"), [value, 0x123])
                else:
                    # 上位バイトだけの定数（SAR0、LAR0 のように AR を数字で付けたものを含む）
                    ar = int(name[len(base):] or 0)
                    if kind == "ar_mem":
                        self.assertEqual(assemble_line(f"{base} AR{ar},5"), [value << 8 | 5])
                    elif kind == "port":
                        self.assertEqual(assemble_line(f"{base} 5,PA0"), [value << 8 | 5])
                    else:
                        self.assertEqual(assemble_line(f"{base} 5"), [value << 8 | 5])
            checked += 1
        return consts, checked

    def test_chip_tests(self):
        consts, checked = self.check_file(CHIP_TESTS)
        self.assertGreaterEqual(checked, 40)
        self.assertEqual(consts["IND"], assemble_line("LAC *")[0] & 0xFF)
        self.assertEqual(consts["IND_INC"], assemble_line("LAC *+")[0] & 0xFF)
        self.assertEqual(consts["IND_DEC"], assemble_line("LAC *-")[0] & 0xFF)

    def test_board_tests(self):
        need(BOARD_TESTS)
        _, checked = self.check_file(BOARD_TESTS)
        self.assertGreaterEqual(checked, 10)

    def test_const_fns(self):
        # 試験の const fn（lack、lark、ldpk、larp、mpyk）と同じ式
        self.assertEqual(assemble_line("LACK 0x5A"), [0x7E00 | 0x5A])
        self.assertEqual(assemble_line("LARK AR1,0x34"), [0x7000 | 1 << 8 | 0x34])
        self.assertEqual(assemble_line("LDPK 1"), [0x6E00 | 1])
        self.assertEqual(assemble_line("LARP 1"), [0x6880 | 1])
        self.assertEqual(assemble_line("MPYK -3"), [0x8000 | (-3 & 0x1FFF)])


class OperandErrorTest(unittest.TestCase):
    def bad(self, src, pattern):
        with self.assertRaises(AsmError) as cm:
            assemble_line(src)
        self.assertRegex(str(cm.exception), pattern)

    def test_errors(self):
        self.bad("FOO 1", "知らない命令")
        self.bad("ADD", "オペランド")
        self.bad("ADD 1,2,3", "間接アドレスのときだけ")
        self.bad("ADD 1,16", "シフト")
        self.bad("ADD *,-1", "シフト")
        self.bad("ADD *,0,2", "次の ARP")
        self.bad("ADD 256", "データの番地")
        self.bad("ADD -1", "データの番地")
        self.bad("SACL 1,1", "SACL のシフトは 0 だけ")
        self.bad("SACH 1,2", "SACH のシフトは 0・1・4 だけ")
        self.bad("LT 1,1", "間接アドレスのときだけ")
        self.bad("LT *,0,1", "オペランド")
        self.bad("SAR 2,1", "補助レジスタ")
        self.bad("SAR AR2,1", "定義されていない記号")
        self.bad("LAR AR0", "オペランド")
        self.bad("IN 1,8", "ポート")
        self.bad("OUT 1", "オペランド")
        self.bad("LACK 256", "LACK の定数")
        self.bad("LACK -1", "LACK の定数")
        self.bad("LARK AR0,256", "LARK の定数")
        self.bad("LARP 2", "ARP")
        self.bad("LDPK 2", "DP")
        self.bad("MPYK 4096", "MPYK の定数")
        self.bad("MPYK -4097", "MPYK の定数")
        self.bad("B 4096", "分岐先")
        self.bad("B -1", "分岐先")
        self.bad("NOP 1", "オペランド")
        self.bad("B", "オペランド")
        self.bad("ADD X", "定義されていない記号")


class SourceTest(unittest.TestCase):
    def words(self, text, **kw):
        a = Assembler(**kw)
        img = a.assemble_source(text)
        self.assertEqual(len(img) % 2, 0)
        return [int.from_bytes(img[i:i + 2], "big") for i in range(0, len(img), 2)], a

    def test_big_endian_image(self):
        img = assemble_text("\tLACK $12\n\tB 0\n")
        self.assertEqual(img, bytes([0x7E, 0x12, 0xF9, 0x00, 0x00, 0x00]))

    def test_labels_and_branches(self):
        w, a = self.words(
            "start:\tB\tinit\n"
            "\tORG\t2\n"
            "intr\tRET\n"
            "init\tLACK\t1\n"
            ".loop\tBZ\t.loop\n"
            "\tB\t*\n"
            "\tCALL\tinit.loop\n"
            "\tBANZ\tstart\n"
        )
        self.assertEqual(w, [0xF900, 3, 0x7F8D, 0x7E01, 0xFF00, 4, 0xF900, 6, 0xF800, 4, 0xF400, 0])
        self.assertEqual(a.sym["init.loop"], 4)

    def test_equ_set_forward_and_expressions(self):
        w, _ = self.words(
            "\tLACK\tK\n"
            "K\tEQU\tJ+1\n"
            "J\tEQU\t>1F\n"
            "V\tSET\t%101\n"
            "\tLACK\tV\n"
            "V\tSET\tV*2\n"
            "\tLACK\tV\n"
            "\tLACK\t'A'\n"
            "\tDATA\t(1<<15)|3,-1,$FFFF,'AB'\n"
            "\tMPYK\t-(K>>1)\n"
            "\tLACK\t(3>2)+(2=2)\n"
        )
        self.assertEqual(w, [0x7E20, 0x7E05, 0x7E0A, 0x7E41, 0x8003, 0xFFFF, 0xFFFF, 0x4142,
                             0x8000 | (-16 & 0x1FFF), 0x7E02])

    def test_data_ram_allocation(self):
        w, a = self.words(
            "\tRSSET\t0\n"
            "XN\tRS\t31\n"
            "Y\tRS\n"
            "Z\tRS.W\t2\n"
            "\tRSSET\t$80\n"
            "P1\tRS\t1\n"
            "\tASSERT\tZ+2<=144\n"
            "\tLT\tXN+30\n"
            "\tSACH\tY,4\n"
            "\tLDPK\t1\n"
            "\tSACL\tP1\n"
        )
        self.assertEqual((a.sym["XN"], a.sym["Y"], a.sym["Z"], a.sym["P1"]), (0, 31, 32, 128))
        self.assertEqual(w, [0x6A1E, 0x5C1F, 0x6E01, 0x5000])

    def test_ds_align_and_fill(self):
        w, _ = self.words("\tNOP\n\tDS\t2\n\tNOP\n\tALIGN\t8\n\tORG\t9\n\tNOP\n", fill=0x7F80)
        self.assertEqual(w, [0x7F80] * 10)
        w, _ = self.words("\tORG\t2\n\tNOP\n", base=0, size=4)
        self.assertEqual(w, [0, 0, 0x7F80, 0])
        w, _ = self.words("\tORG\t2\n\tNOP\n", base=None)
        self.assertEqual(w, [0x7F80])

    def test_conditionals_and_repeat(self):
        w, _ = self.words(
            "N\tEQU\t3\n"
            "\tIF\tN>2\n\tLACK\t1\n\tELSE\n\tLACK\t2\n\tENDIF\n"
            "\tIFDEF\tN\n\tLACK\t3\n\tENDIF\n"
            "\tIFNDEF\tM\n\tLACK\t4\n\tENDIF\n"
            "\tREPT\tN\n\tLTD\t*-\n\tMPYK\t1\n\tENDR\n"
            "\tEND\n"
            "\tLACK\t9\n"
        )
        self.assertEqual(w, [0x7E01, 0x7E03, 0x7E04] + [0x6B98, 0x8001] * 3)

    def test_include_and_comments(self):
        with tempfile.TemporaryDirectory() as d:
            with open(os.path.join(d, "k.inc"), "w", encoding="utf-8") as f:
                f.write("* TI の注釈の行\nKONST\tEQU\t$42\t; 注釈\n")
            w, _ = self.words('\tINCLUDE\t"k.inc"\n\tLACK\tKONST ; 注釈\n', include_dirs=[d])
        self.assertEqual(w, [0x7E42])

    def test_errors_in_source(self):
        cases = [
            ("X\tNOP\nX\tNOP\n", "二重定義"),
            ("\tORG\t4095\n\tB\t0\n", "4096 語を超えた"),
            ("\tASSERT\t1=2,\"だめ\"\n", "ASSERT が成り立たない: だめ"),
            ("\tIF\t1\n\tNOP\n", "ENDIF"),
            ("\tREPT\t2\n\tNOP\n", "ENDR"),
            ("\tLACK.B\t1\n", "大きさ"),
            ("\tNOP\n\tORG\t0\n\tNOP\n", "二度書いた"),
            ("\tDATA\t$10000\n", "16 bit"),
            ("\tINCLUDE\t\"nothing.inc\"\n", "見つからない"),
        ]
        for text, pattern in cases:
            with self.subTest(text=text):
                with self.assertRaises(AsmError) as cm:
                    assemble_text(text)
                self.assertRegex(str(cm.exception), pattern)

    def test_error_location(self):
        with self.assertRaises(AsmError) as cm:
            Assembler().assemble_source("\tNOP\n\tLACK\t300\n", name="t.dsp")
        self.assertEqual(cm.exception.where, ("t.dsp", 2))

    def test_warnings(self):
        _, a = self.words("\tLAC\t150\n\tSST\t5\n\tSST\t133\n\tLAC\t143\n")
        self.assertEqual(len(a.warnings), 2)
        self.assertIn("データ RAM", a.warnings[0])
        self.assertIn("ページ 1", a.warnings[1])

    def test_listing(self):
        _, a = self.words("init\tLACK\t1\n\tTBLR\t0\n\tB\tinit\n")
        text = a.listing_text()
        self.assertRegex(text, r"000  7E01\s+1\s+1  init")
        self.assertRegex(text, r"001  6700\s+3\s+2")
        self.assertRegex(text, r"002  F900 0000\s+2\s+3")
        self.assertIn("init", text.split("記号の表:")[1])

    def test_command_line(self):
        with tempfile.TemporaryDirectory() as d:
            src = os.path.join(d, "a.dsp")
            with open(src, "w", encoding="utf-8") as f:
                f.write("\tLACK\tK\n\tB\t0\n")
            out = os.path.join(d, "a.bin")
            lst = os.path.join(d, "a.lst")
            rc = dspasm.main([src, "-o", out, "-l", lst, "-D", "K=$33", "--size", "4", "--fill", "0x7F80"])
            self.assertEqual(rc, 0)
            with open(out, "rb") as f:
                self.assertEqual(f.read(), bytes.fromhex("7E33F90000007F80"))
            self.assertTrue(os.path.getsize(lst) > 0)


if __name__ == "__main__":
    unittest.main()
