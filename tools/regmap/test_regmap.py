"""番地とレジスタの表（docs/spec/registers.toml）と生成器の試験。`py -m unittest discover -s tools/regmap`。

- 生成物（fc85.inc、fc85.h、regmap.rs、registers.md）が、今の表から作り直したものと同じ（--check と同じ）。
- 表に誤りがない：名前が 1 回だけ、窓とレジスタが重ならない、ビット番号とマスクがそろう、など。
  誤りのある表は problems() か MapError で止まる。
- fc85.inc を m68kasm で組むと、記号の値が表と同じ。
- fc85.h が C の構文として通り（gcc -fsyntax-only）、マクロの値が表と同じ。gcc がなければ飛ばす。
"""
import contextlib
import io
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import regmap as rm

sys.path.insert(0, str(rm.ROOT / 'sdk' / 'm68kasm'))
import m68kasm  # noqa: E402

# 誤りを入れて確かめる、最小の表
HEAD = '''
[meta]
title = "t"
rules_title = "r"
rules = ["a"]
[access]
r = "r"
w = "w"
rw = "rw"
[when]
always = "a"
[unlisted]
berr = "b"
chip = "c"
'''


def mini(body):
    return rm.parse(HEAD + body)


BLOCK = '''
[[block]]
id = "x"
title = "X"
[[block.window]]
name = "X"
first = 0x200000
last = 0x20000F
'''


class RegmapTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.m = rm.load()

    def test_generated_files_are_current(self):
        self.assertEqual([rel for rel, _ in rm.stale(rm.outputs(self.m))], [], f'作り直す：{rm.REGEN}')

    def test_check_option_passes(self):
        with contextlib.redirect_stderr(io.StringIO()) as err:
            self.assertEqual(rm.main(['--check']), 0, err.getvalue())

    def test_table_has_no_problems(self):
        self.assertEqual(rm.problems(self.m), [])

    def test_windows_and_registers_do_not_overlap(self):
        wins = self.m.windows()
        for a, b in zip(wins, wins[1:]):
            self.assertLess(a.last, b.first, f'{a.name} と {b.name}')
        seen = {}
        for r in self.m.regs():
            for a in r.elements():
                for w in range(a, a + r.span(), 2):
                    other = seen.get(w)
                    if other is not None:
                        # 同じ番地の読出しと書込みの組（ACRTC_AR・ACRTC_SR、MIDI_CTRL・MIDI_STATUS）
                        self.assertEqual({other.access, r.access}, {'r', 'w'}, f'{other.name} と {r.name}')
                    seen[w] = r
        self.assertTrue(seen)

    def test_fc85_inc_assembles_to_the_table(self):
        a = m68kasm.Assembler(include_dirs=[str(rm.OUT['inc'].parent)])
        a.assemble_source('    INCLUDE "fc85.inc"\n', name='t.s', base_dir=str(rm.OUT['inc'].parent))
        want = {n: c.value for n, c in self.m.symbols.items()}
        have = {n: v for n, v in a.sym.items()}
        self.assertEqual(have, want)

    def test_fc85_inc_says_it_is_generated(self):
        head = rm.OUT['inc'].read_text(encoding='utf-8').splitlines()[0]
        self.assertIn('生成物', head)
        self.assertIn(rm.SRC_REL, head)

    @unittest.skipUnless(shutil.which('gcc'), 'gcc がない')
    def test_c_header_compiles_and_matches_the_table(self):
        lines = ['#include "fc85.h"', '']
        for n, c in self.m.symbols.items():
            lines.append(f'_Static_assert({n} == {c.value}UL, "{n}");')
        lines += ['', 'unsigned long touch(void);', 'unsigned long touch(void)', '{', '    unsigned long s = 0;']
        for blk in self.m.blocks:
            for r in blk.all_regs():
                if blk.bank:
                    arg = '(1)' if r in blk.bank.regs else ''
                else:
                    arg = '(1)' if r.count > 1 else ''
                ref = f'REG_{r.name}{arg}'
                if r.access in ('r', 'rw'):
                    lines.append(f'    s += {ref};')
                if r.access in ('w', 'rw'):
                    lines.append(f'    {ref} = 0;')
        lines += ['    return s;', '}', '']
        with tempfile.TemporaryDirectory() as d:
            src = Path(d) / 't.c'
            src.write_text('\n'.join(lines), encoding='utf-8')
            r = subprocess.run(['gcc', '-fsyntax-only', '-std=c11', '-Wall', '-Wextra', '-pedantic', '-Werror',
                                '-I', str(rm.OUT['h'].parent), str(src)], capture_output=True, text=True,
                               encoding='utf-8', errors='replace')
        self.assertEqual(r.returncode, 0, r.stderr)

    # ------------------------------------------------------------ 誤りのある表

    def assertProblem(self, body, text):
        m = mini(body)
        probs = rm.problems(m)
        self.assertTrue(any(text in p for p in probs), probs)

    def test_overlapping_registers_are_rejected(self):
        self.assertProblem(BLOCK + '''
[[block.reg]]
name = "A"
addr = 0x200000
bits = 32
access = "rw"
[[block.reg]]
name = "B"
addr = 0x200002
bits = 16
access = "rw"
''', '重なる')

    def test_read_write_pair_at_one_address_is_allowed(self):
        m = mini(BLOCK + '''
[[block.reg]]
name = "A"
addr = 0x200000
bits = 16
access = "w"
[[block.reg]]
name = "B"
addr = 0x200000
bits = 16
access = "r"
''')
        self.assertEqual(rm.problems(m), [])

    def test_register_outside_its_window_is_rejected(self):
        self.assertProblem(BLOCK + '''
[[block.reg]]
name = "A"
addr = 0x200010
bits = 16
access = "rw"
''', '窓の外')

    def test_odd_register_address_is_rejected(self):
        self.assertProblem(BLOCK + '''
[[block.reg]]
name = "A"
addr = 0x200001
bits = 8
access = "rw"
''', '奇数')

    def test_overlapping_windows_are_rejected(self):
        self.assertProblem(BLOCK + '''
[[block]]
id = "y"
title = "Y"
[[block.window]]
name = "Y"
first = 0x200008
last = 0x20001F
''', '重なる')

    def test_bit_number_must_match_its_mask(self):
        self.assertProblem(BLOCK + '''
[[block.group]]
title = "g"
kind = "mask"
width = 16
items = [
  { name = "F", value = 0x0004 },
  { name = "F_B", value = 3, kind = "bit" },
]
''', 'そろわない')

    def test_value_must_fit_its_width(self):
        self.assertProblem(BLOCK + '''
[[block.group]]
title = "g"
kind = "mask"
width = 8
items = [{ name = "F", value = 0x100 }]
''', '収まらない')

    def test_duplicate_names_and_unknown_keys_stop_the_parse(self):
        with self.assertRaises(rm.MapError):
            mini(BLOCK + '''
[[block.group]]
title = "g"
kind = "number"
items = [{ name = "F", value = 1 }, { name = "F", value = 2 }]
''')
        with self.assertRaises(rm.MapError):
            mini(BLOCK + '''
[[block.reg]]
name = "A"
addr = 0x200000
bits = 16
acess = "rw"
''')
        with self.assertRaises(rm.MapError):
            mini(BLOCK + '''
[[block.group]]
title = "g"
kind = "number"
items = [{ name = "F", expr = "G|1" }]
''')

    def test_expressions_use_earlier_names(self):
        m = mini(BLOCK + '''
[[block.group]]
title = "g"
kind = "mask"
width = 8
items = [
  { name = "A", value = 0x80 },
  { name = "B", value = 0x14 },
  { name = "C", expr = "A|B|0x01" },
]
''')
        self.assertEqual(m.symbols['C'].value, 0x95)
        self.assertEqual(rm.asm_value(m.symbols['C']), 'A|B|$01')


if __name__ == '__main__':
    unittest.main()
