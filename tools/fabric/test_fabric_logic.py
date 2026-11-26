"""Data Fabric の PAL の式の試験。`py -m unittest discover -s tools/fabric`。

- 式（pal/*.pal）がピンと項に収まり、仕様の定義と同じ値を出す。順序と状態の式（seq.pal）は、
  チャネルの段階とトリガの状態と AOP をここで確かめ、全体はエミュレータの試験 fabric_lockstep で確かめる。
- 生成した Rust（emulator/crates/board/src/fabric_pal.rs）が、今の式から作り直したものと同じ。
- docs/spec/fabric.md の式のコードブロックが pal/*.pal と同じ。
"""
import unittest

import fabric_logic as fl
import palasm


class FabricLogicTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.pals = fl.load_pals()

    def test_equations_follow_the_spec(self):
        self.assertEqual(fl.check_all(self.pals), 61056)

    def test_generated_rust_is_current(self):
        want = fl.rust_source(self.pals)
        have = fl.RUST_OUT.read_bytes().decode('utf-8')
        self.assertEqual(have, want, f'作り直す：{fl.REGEN}')

    def test_fabric_md_shows_the_burned_equations(self):
        self.assertEqual(fl.stale(), [])

    def test_fit_checks_reject_what_does_not_fit(self):
        base = '; X (PAL16L8A)\nPIN A B C D E F G H I GND J O1 NC NC NC NC NC NC O2 VCC\n'
        # 積が 8 個
        eq = '/O1 = ' + ' + '.join(['A*B'] * 7 + ['C']) + '\n/O2 = A\n'
        with self.assertRaises(palasm.PalError):
            palasm.parse(base + eq)
        # 帰還のない出力（ピン 12）を入力に使う
        with self.assertRaises(palasm.PalError):
            palasm.parse(base + '/O1 = A\n/O2 = O1\n')
        # 登録出力の式を組合せの PAL に書く
        with self.assertRaises(palasm.PalError):
            palasm.parse(base + '/O1 := A\n/O2 = A\n')
        # 収まるもの
        palasm.parse(base + '/O1 = A*/B + C\n/O2 = D\n')


if __name__ == '__main__':
    unittest.main()
