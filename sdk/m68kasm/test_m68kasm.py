"""m68kasm の試験。`python -m unittest test_m68kasm`（sdk/m68kasm で実行）。

SingleStepTests との比較は環境変数で有効にする（データはリポジトリに入れない）:
    M68KASM_SST_DIR=<scratchpad>/m68k-tests  [M68KASM_SST_LIMIT=1000]  python -m unittest test_m68kasm
"""

import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from m68kasm import AsmError, Assembler, assemble_line, assemble_text  # noqa: E402


def words(line, pc=0):
    return " ".join(f"{w:04X}" for w in assemble_line(line, pc))


def asm(src, **kw):
    return assemble_text(src, **kw)


class Encoding(unittest.TestCase):
    """命令の符号化。値は M68000 Programmer's Reference Manual の表から手で組み立てた。"""

    CASES = [
        ("NOP", "4E71"),
        ("RTS", "4E75"),
        ("RTE", "4E73"),
        ("RTR", "4E77"),
        ("RESET", "4E70"),
        ("TRAPV", "4E76"),
        ("ILLEGAL", "4AFC"),
        ("STOP #$2000", "4E72 2000"),
        ("TRAP #15", "4E4F"),
        ("MOVEQ #1,D0", "7001"),
        ("MOVEQ #-1,D7", "7EFF"),
        ("MOVE.L D0,D1", "2200"),
        ("MOVE.W D0,(A1)+", "32C0"),
        ("MOVE.B (A0)+,D1", "1218"),
        ("MOVE.W -(A7),D0", "3027"),
        ("MOVE.L #$12345678,D0", "203C 1234 5678"),
        ("MOVE.W #$1234,$100000", "33FC 1234 0010 0000"),
        ("MOVE.B 4(A0,D1.L),-(A7)", "1F30 1804"),
        ("MOVE.W (8,A6),(-2,A5)", "3B6E 0008 FFFE"),
        ("MOVE.L A0,A1", "2248"),
        ("MOVEA.W D0,A2", "3440"),
        ("MOVE SR,D0", "40C0"),
        ("MOVE.W #$2700,SR", "46FC 2700"),
        ("MOVE #0,CCR", "44FC 0000"),
        ("MOVE USP,A0", "4E68"),
        ("MOVE A1,USP", "4E61"),
        ("LEA (A0),A1", "43D0"),
        ("LEA 16(A5),A6", "4DED 0010"),
        ("LEA ($10,PC),A0", "41FA 000E"),
        ("PEA ($1234).W", "4878 1234"),
        ("JMP ($1234).W", "4EF8 1234"),
        ("JSR $400000", "4EB9 0040 0000"),
        ("JMP (A0)", "4ED0"),
        ("MOVEM.L D0-D7/A0-A6,-(SP)", "48E7 FFFE"),
        ("MOVEM.L (SP)+,D0-D7/A0-A6", "4CDF 7FFF"),
        ("MOVEM.W D0/A0,(A1)", "4891 0101"),
        ("MOVEM.L D0,-(A7)", "48E7 8000"),
        ("MOVEP.W D0,(0,A0)", "0188 0000"),
        ("MOVEP.L (4,A1),D2", "0549 0004"),
        ("ADD.B #1,D0", "D03C 0001"),
        ("ADDI.B #1,D0", "0600 0001"),
        ("ADD.W D1,(A0)", "D350"),
        ("ADD.L (A0),D1", "D290"),
        ("ADD.L D0,A0", "D1C0"),
        ("ADDA.W #4,A7", "DEFC 0004"),
        ("ADD.W #1,(A0)", "0650 0001"),
        ("ADDQ.L #8,A0", "5088"),
        ("ADDQ.W #1,D0", "5240"),
        ("SUBQ.B #1,(A0)", "5310"),
        ("SUB.L D1,D2", "9481"),
        ("SUBX.L -(A0),-(A1)", "9388"),
        ("ADDX.W D0,D1", "D340"),
        ("ABCD D0,D1", "C300"),
        ("SBCD -(A0),-(A1)", "8308"),
        ("AND.W D0,D1", "C240"),
        ("ANDI.W #$00FF,D0", "0240 00FF"),
        ("AND.W #$FF,D0", "C07C 00FF"),
        ("ANDI #$FE,CCR", "023C 00FE"),
        ("ORI #$0700,SR", "007C 0700"),
        ("OR.L D0,(A1)", "8191"),
        ("EOR.B D0,D1", "B101"),
        ("EORI.L #-1,D2", "0A82 FFFF FFFF"),
        ("NOT.W D3", "4643"),
        ("NEG.L D0", "4480"),
        ("NEGX.B (A0)", "4010"),
        ("CLR.L D0", "4280"),
        ("CLR.W $100000", "4279 0010 0000"),
        ("TST.B (A0)", "4A10"),
        ("TAS D0", "4AC0"),
        ("NBCD D1", "4801"),
        ("CMP.W D0,D1", "B240"),
        ("CMP.L #10,D0", "B0BC 0000 000A"),
        ("CMPI.L #10,D0", "0C80 0000 000A"),
        ("CMPI.B #1,(A0)", "0C10 0001"),
        ("CMPA.L A0,A1", "B3C8"),
        ("CMP.B (A0)+,(A1)+", "B308"),
        ("CMPM.W (A0)+,(A1)+", "B348"),
        ("MULU D0,D1", "C2C0"),
        ("MULS #3,D2", "C5FC 0003"),
        ("DIVU (A0),D3", "86D0"),
        ("DIVS D1,D0", "81C1"),
        ("CHK D1,D0", "4181"),
        ("EXT.W D0", "4880"),
        ("EXT.L D1", "48C1"),
        ("SWAP D2", "4842"),
        ("EXG D0,D1", "C141"),
        ("EXG A0,A1", "C149"),
        ("EXG D4,A6", "C98E"),
        ("EXG A6,D4", "C98E"),
        ("LINK A6,#-8", "4E56 FFF8"),
        ("UNLK A6", "4E5E"),
        ("LSL.L #2,D1", "E589"),
        ("LSR.W #8,D0", "E048"),
        ("ASL.B D1,D2", "E322"),
        ("ASR.W (A0)", "E0D0"),
        ("ROL.W #1,D0", "E358"),
        ("ROR.L D3,D4", "E6BC"),
        ("ROXL.B #4,D5", "E915"),
        ("ROXR.W (A1)+", "E4D9"),
        ("LSL.W D0", "E348"),
        ("BTST #3,D0", "0800 0003"),
        ("BTST D1,(A0)", "0310"),
        ("BSET #7,(A1)", "08D1 0007"),
        ("BCLR D2,D3", "0583"),
        ("BCHG #0,$100000", "0879 0000 0010 0000"),
        ("BTST D0,#5", "013C 0005"),
        ("SEQ D0", "57C0"),
        ("ST (A0)", "50D0"),
        ("SF D1", "51C1"),
        ("SNE $100000", "56F9 0010 0000"),
        ("DBRA D0,$0", "51C8 FFFE"),
        ("DBF D1,$10", "51C9 000E"),
        ("DBEQ D2,$4", "57CA 0002"),
        ("BRA.S $10", "600E"),
        ("BRA $100", "6000 00FE"),
        ("BSR.W $40", "6100 003E"),
        ("BEQ.S $0", "67FE"),
        ("BNE $8", "6606"),
        ("BHS.S $20", "641E"),
        ("BLO.S $20", "651E"),
        ("MOVE.B ($FFFF8000).W,D0", "1038 8000"),
        ("MOVE.W (PC),D0", "303A 0000"),
        ("MOVE.W (6,PC,D1.W),D0", "303B 1004"),
        ("TST.W $7FFF", "4A78 7FFF"),
        ("TST.W $8000", "4A79 0000 8000"),
    ]

    def test_cases(self):
        for line, want in self.CASES:
            with self.subTest(line=line):
                self.assertEqual(words(line), want)

    def test_pc_relative_uses_extension_address(self):
        # MOVEM のマスクの後の拡張語が基準
        self.assertEqual(words("MOVEM.L ($20,PC),D0", 0x10), "4CFA 0001 000C")
        # BTST #n の後の拡張語が基準
        self.assertEqual(words("BTST #1,($20,PC)", 0x10), "083A 0001 000C")

    def test_modes_rejected(self):
        bad = [
            "MOVE.W D0,(4,PC)",      # 書込み先に PC 相対
            "MOVE.B A0,D0",          # バイトでアドレスレジスタ
            "ADDQ.B #1,A0",
            "ADDQ.W #9,D0",
            "LEA D0,A0",
            "JMP (A0)+",
            "CLR.W A0",
            "MOVEQ #1,(A0)",
            "MOVE.W D0,#1",
            "EXT.B D0",
            "LSL.W #1,(A0)",
            "BRA.L $100",
            "MOVEM.L D0,(4,PC)",
            "FOO D0",
            "DIVU.L D0,D1",
        ]
        for line in bad:
            with self.subTest(line=line):
                with self.assertRaises(AsmError):
                    assemble_line(line)

    def test_ranges(self):
        for line in ["MOVE.B #256,D0", "MOVE.W #65536,D0", "BRA.S $100",
                     "MOVE.W (40000,A0),D0", "MOVE.W (200,A0,D0),D0", "TRAP #16",
                     "BEQ.S $2"]:
            with self.subTest(line=line):
                with self.assertRaises(AsmError):
                    assemble_line(line)


class Expressions(unittest.TestCase):
    def val(self, expr):
        return int.from_bytes(asm(f"\tDC.L {expr}\n"), "big")

    def test_numbers(self):
        self.assertEqual(self.val("$1F"), 0x1F)
        self.assertEqual(self.val("%1010"), 10)
        self.assertEqual(self.val("0x10"), 16)
        self.assertEqual(self.val("'A'"), 0x41)
        self.assertEqual(self.val("'AB'"), 0x4142)

    def test_precedence(self):
        self.assertEqual(self.val("1+2*3"), 7)
        self.assertEqual(self.val("(1+2)*3"), 9)
        self.assertEqual(self.val("1<<4|1"), 17)
        self.assertEqual(self.val("$FF&$0F<<4"), 0xF0)
        self.assertEqual(self.val("~0&$FF"), 0xFF)
        self.assertEqual(self.val("-7/2"), (-3) & 0xFFFFFFFF)
        self.assertEqual(self.val("7%3"), 1)
        self.assertEqual(self.val("10-2-3"), 5)
        self.assertEqual(self.val("2*%11"), 6)
        self.assertEqual(self.val("3=3"), 1)
        self.assertEqual(self.val("3<>3"), 0)

    def test_pc_symbol(self):
        self.assertEqual(asm("\tORG $100\n\tDC.L *\n"), bytes.fromhex("00000100"))

    def test_even_and_align_pad_after_emitted_bytes(self):
        # 像の終わりの EVEN・ALIGN も埋め草を出す。DS は出さない
        self.assertEqual(asm("\tDC.B 1\n\tEVEN\n"), bytes.fromhex("01FF"))
        self.assertEqual(asm("\tDC.B 1\n\tALIGN 4\n", fill=0), bytes.fromhex("01000000"))
        self.assertEqual(asm("\tDC.B 1\n\tDS.B 3\n"), bytes.fromhex("01"))
        # 何も出していない番地（RAM の変数）では出さない
        self.assertEqual(asm("\tDC.W 1\n\tORG $1001\n\tEVEN\n"), bytes.fromhex("0001"))
        self.assertEqual(asm("\tORG $100\n\tDC.W 0\n\tDC.L *-2\n"), bytes.fromhex("000000000100"))

    def test_local_labels_named_like_sizes_are_rejected(self):
        # f.w は abs.W と読むので、局所ラベル .w・.l は定義の時点で拒む（.b はそのまま使える）
        for name in (".w", ".L"):
            with self.assertRaises(AsmError):
                asm(f"f:\n\tBRA.S\tf{name}\n\tNOP\n{name}\tRTS\n")
        for ref in ("f.b", ".b"):
            self.assertEqual(asm(f"f:\n\tBRA.S\t{ref}\n\tNOP\n.b\tRTS\n"), bytes.fromhex("60024E714E75"))
        # 参照だけでも、理由の分かる誤りにする（「式がない」にしない）
        with self.assertRaisesRegex(AsmError, "大きさの指定と同じ名前"):
            asm("f:\n\tBEQ.S\t.w\n\tRTS\n")


class Directives(unittest.TestCase):
    def test_labels_and_forward_refs(self):
        src = """
start:  BRA.S   next
        DC.W    end-start
next    NOP
end
"""
        self.assertEqual(asm(src), bytes.fromhex("6002 0006 4E71".replace(" ", "")))

    def test_local_labels(self):
        src = """
a:      MOVEQ   #3,D0
.loop   DBRA    D0,.loop
b:      MOVEQ   #3,D0
.loop   DBRA    D0,.loop
        BRA.S   a.loop
"""
        out = asm(src)
        self.assertEqual(out.hex().upper(), "700351C8FFFE700351C8FFFE60F4")

    def test_equ_set_rs(self):
        src = """
A       EQU     $100000
B       =       A+4
C       SET     1
C       SET     C+1
        RSSET   $200
X       RS.B    1
Y       RS.W    1
Z       RS.L    2
W       RS.B    0
        DC.L    A,B,C,X,Y,Z,W
"""
        out = asm(src)
        vals = [int.from_bytes(out[i:i + 4], "big") for i in range(0, len(out), 4)]
        self.assertEqual(vals, [0x100000, 0x100004, 2, 0x200, 0x202, 0x204, 0x20C])

    def test_dc_ds_even_align(self):
        src = """
        ORG     $10
        DC.B    1,"AB",'C',0
        EVEN
        DC.W    $1234
        DS.B    3
        ALIGN   4
        DC.L    -1
        DCB.W   2,$5555
"""
        out = asm(src, fill=0xEE)
        self.assertEqual(out.hex().upper(), "0141424300EE1234EEEEEEEEFFFFFFFF55555555")

    def test_jis_x0201_kana(self):
        self.assertEqual(asm('\tDC.B "ｱｲｳ¥"\n'), bytes([0xB1, 0xB2, 0xB3, 0x5C]))

    def test_base_size_fill(self):
        out = asm("\tORG 2\n\tDC.W $ABCD\n", base=0, size=8, fill=0xFF)
        self.assertEqual(out, bytes.fromhex("FFFFABCDFFFFFFFF"))
        with self.assertRaises(AsmError):
            asm("\tORG 0\n\tDS.B 4\n\tDC.L 0\n", base=0, size=6)

    def test_ds_in_ram_is_not_emitted(self):
        src = """
        ORG     $100000
var     DS.L    1
        ORG     0
        MOVE.L  D0,var
"""
        self.assertEqual(asm(src).hex().upper(), "23C000100000")

    def test_if_else(self):
        src = """
F       EQU     1
        IF F=1
        DC.B    1
        ELSE
        DC.B    2
        ENDIF
        IFDEF   G
        DC.B    3
        ELSE
        IFNDEF  H
        DC.B    4
        ENDIF
        ENDIF
"""
        self.assertEqual(asm(src), bytes([1, 4]))

    def test_rept(self):
        src = """
N       SET     0
        REPT    4
        DC.B    N
N       SET     N+1
        ENDR
"""
        self.assertEqual(asm(src), bytes([0, 1, 2, 3]))

    def test_include_incbin(self):
        with tempfile.TemporaryDirectory() as d:
            with open(os.path.join(d, "inc.s"), "w") as f:
                f.write("VAL EQU $42\n")
            with open(os.path.join(d, "blob.bin"), "wb") as f:
                f.write(bytes([9, 8, 7, 6]))
            with open(os.path.join(d, "main.s"), "w") as f:
                f.write('\tINCLUDE "inc.s"\n\tDC.B VAL\n\tINCBIN "blob.bin",1,2\n')
            out = Assembler().assemble_file(os.path.join(d, "main.s"))
        self.assertEqual(out, bytes([0x42, 8, 7]))

    def test_assert(self):
        asm("\tASSERT 1+1=2\n")
        with self.assertRaises(AsmError):
            asm("\tASSERT 1=2,\"bad\"\n")

    def test_errors(self):
        for src in ["\tDC.B 1\n\tNOP\n", "\tMOVE.L X,D0\n", "a NOP\na NOP\n", "\tIF 1\n",
                    "\tDC.W 1\n\tDC.B 1\n\tDC.W 2\n"]:
            with self.subTest(src=src):
                with self.assertRaises(AsmError):
                    asm(src)

    def test_error_location(self):
        with self.assertRaises(AsmError) as cm:
            asm("\tNOP\n\tFOO\n")
        self.assertEqual(cm.exception.where[1], 2)

    def test_listing(self):
        a = Assembler()
        a.assemble_source("\tORG $400\nhere:\tMOVEQ #5,D0\n")
        text = a.listing_text()
        self.assertIn("000400  7005", text)
        self.assertIn("here", text)


class Relaxation(unittest.TestCase):
    def test_forward_short_branch(self):
        out = asm("\tBRA fwd\n\tNOP\nfwd\tNOP\n")
        self.assertEqual(out.hex().upper(), "60024E714E71")

    def test_far_branch_becomes_word(self):
        out = asm("\tBRA fwd\n\tDS.B 200\nfwd\tNOP\n", fill=0)
        self.assertEqual(out[:4].hex().upper(), "600000CA")

    def test_branch_to_next_is_word(self):
        self.assertEqual(asm("\tBRA nx\nnx\tNOP\n").hex().upper(), "600000024E71")

    def test_backward_short(self):
        self.assertEqual(asm("top\tNOP\n\tBNE top\n").hex().upper(), "4E7166FC")

    def test_chain_growth(self):
        # 前の分岐が伸びると、後ろの分岐の距離が変わる
        src = "\tBRA a\n\tBRA b\n\tDS.B 126\na\tNOP\nb\tNOP\n"
        out = asm(src, fill=0)
        self.assertEqual(out[:8].hex().upper(), "6000008460000082")

    def test_absolute_auto_size(self):
        out = asm("\tMOVE.W lo,D0\n\tMOVE.W hi,D0\nlo\tEQU $1000\nhi\tEQU $100000\n")
        self.assertEqual(out.hex().upper(), "30381000303900100000")

    def test_explicit_short_out_of_range(self):
        with self.assertRaises(AsmError):
            asm("\tBRA.S far\n\tDS.B 300\nfar\tNOP\n")


class SingleStepTests(unittest.TestCase):
    def test_opcodes(self):
        import sst
        limit = int(os.environ.get("M68KASM_SST_LIMIT", "0")) or None
        r = sst.run(os.environ["M68KASM_SST_DIR"], limit)
        total = r["total"]
        print(f"\nSingleStepTests: opcode {r['opcode']}/{total} "
              f"({100.0 * r['opcode'] / total:.3f}%), full {r['full']}/{total} "
              f"({100.0 * r['full'] / total:.3f}%)")
        for f in r["failures"][:20]:
            print("  ", f)
        self.assertEqual(r["opcode"], total)
        self.assertEqual(r["full"], total)


# データがあるときだけ動かす（デコレータの代わりに後から包む）
SingleStepTests = unittest.skipUnless(
    os.environ.get("M68KASM_SST_DIR"), "M68KASM_SST_DIR が設定されていない")(SingleStepTests)


if __name__ == "__main__":
    unittest.main()
