#!/usr/bin/env python3
"""HD63484 の V3（独立した実装との照合）の入力の列を作る。

場合ごとに、VRAM の初めの中身の種と、レジスタの書込み、コマンドの列を決める。照合の相手（MAME の hd63484、
docs/impl/v3-mame.md §06）とエミュレータの試験（emulator/crates/chips/tests/hd63484_v3.rs）が、同じ入力を読む。

    python tools/v3/hd63484_vectors.py OUT [件数] [コマンドの数] [種]

形（1 行に 1 つ、数は 16 進）：
    C 番号 VRAM の種           場合の始まり。VRAM（1 M 語）の中身は種から作る（照合の両側に同じ式を持つ）
    R 番地 値                  AR に番地を書き、そのレジスタに値を書く（CCR、OMR、MWR）
    K 語 …                     書込み FIFO に書く 1 つのコマンド（コマンド語、パラメータ、DWT・DMOD のデータ）
    H 語 …                     途中までのコマンド（パラメータが足りない）。この後は ABT で打ち切る

MAME と比べられる範囲だけを使う（v3-mame.md §06）：領域モードは 000・100、色モードは 00〜10、パターン RAM は
場合ごとに全部 0 か全部 1。曲線、PAINT、RPTN、DRD は使わない。

外部のライブラリは使わない。
"""
import random
import sys

ORG, WPR, RPR, WPTN = 0x0400, 0x0800, 0x0C00, 0x1800
DWT, DMOD, RD, WT, MOD = 0x2800, 0x2C00, 0x4400, 0x4800, 0x4C00
CLR, SCLR, CPY, SCPY = 0x5800, 0x5C00, 0x6000, 0x7000
AMOVE, RMOVE, ALINE, RLINE = 0x8000, 0x8400, 0x8800, 0x8C00
ARCT, RRCT, APLL, RPLL, APLG, RPLG = 0x9000, 0x9400, 0x9800, 0x9C00, 0xA000, 0xA400
AFRCT, RFRCT, DOT, PTN, AGCPY, RGCPY = 0xC000, 0xC400, 0xCC00, 0xD000, 0xE000, 0xF000

# 上位 6 bit がどちらの解読でも未定義になるコマンド語（U85 p.173 の表にない）
UNDEFINED = [0x00, 0x04, 0x05, 0x08, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x14, 0x15]

# PTN の 45° ごとの向き（表 C36-1）。+Y は画面の上
DIRS = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]


def s16(v):
    v &= 0xFFFF
    return v - 0x10000 if v & 0x8000 else v


def tie(dx, dy):
    """直線の画素の選び方で、誤差がちょうど半分になる点があるか（照合の試験と同じ式）。"""
    a, b = abs(dx), abs(dy)
    if a == b or a == 0 or b == 0:
        return False
    major, minor = max(a, b), min(a, b)
    return any((2 * minor * j - major) % (2 * major) == 0 for j in range(1, major))


class Case:
    def __init__(self, rng, f):
        self.rng = rng
        self.f = f
        self.cp = (0, 0)
        self.psx = self.psy = self.pex = self.pey = 0
        self.gbm = rng.choice([0, 1, 2, 2, 2, 2, 3, 4])
        self.fill = rng.choice([0x0000, 0xFFFF])

    # ---- 出力 ----

    def reg(self, ar, v):
        self.f.write(f'R {ar:X} {v & 0xFFFF:X}\n')

    def cmd(self, words, tag='K'):
        self.f.write(tag + ' ' + ' '.join(f'{w & 0xFFFF:X}' for w in words) + '\n')

    def ccr(self, abort=False):
        self.reg(0x02, (0x8000 if abort else 0) | self.gbm << 8)

    def abort(self):
        self.ccr(True)
        self.ccr(False)

    # ---- 値の選び方 ----

    def color(self):
        """色レジスタ：乱数か、画素の値を並べた語。"""
        r = self.rng
        if r.random() < 0.5:
            return r.getrandbits(16)
        bpp = 1 << self.gbm
        p = r.getrandbits(bpp)
        v = 0
        for i in range(0, 16, bpp):
            v |= p << i
        return v

    def low(self, gcpy=False):
        """描画のコマンド語の下位 8 bit：AREA（000 か 100）、COL（00〜10）、OPM。

        OPM＝110・111 は、MAME が CL0・CL1 と比べ、コアは描こうとしている色と比べる（既知の差）。多くは、
        描く色が MAME の比べる色（110 は CL0、111 は CL1）になる組にして、差の出ない形で比べる。
        """
        r = self.rng
        area = 0x80 if r.random() < 0.2 else 0
        col = r.choice([0, 0, 1, 2])
        opm = r.randrange(8)
        if opm >= 6 and r.random() < 0.8:
            if gcpy:
                opm = r.randrange(6)
            elif self.fill:
                opm, col = 7, r.choice([0, 1])
            else:
                opm, col = 6, r.choice([0, 2])
        return area | col << 3 | opm

    def delta(self, m):
        """直線の差分。4 分の 3 は誤差がちょうど半分になる点のない線にする。"""
        r = self.rng
        for _ in range(100):
            k = r.random()
            if k < 0.1:
                d = (r.randint(-m, m), 0)
            elif k < 0.2:
                d = (0, r.randint(-m, m))
            elif k < 0.3:
                v = r.randint(-m, m)
                d = (v, v * r.choice([1, -1]))
            else:
                d = (r.randint(-m, m), r.randint(-m, m))
            if r.random() < 0.25 or not tie(*d):
                return d
        return d

    def addr(self):
        return self.rng.getrandbits(20)

    def point(self, m):
        x, y = self.cp
        return (x + self.rng.randint(-m, m), y + self.rng.randint(-m, m))

    # ---- 初めの設定 ----

    def setup(self):
        r = self.rng
        self.ccr()
        self.reg(0x04, 0xC080)   # OMR：M/S＝1、STR＝1、RAM＝1
        for ar in (0xC2, 0xCA, 0xD2, 0xDA):
            self.reg(ar, self.mw())
        self.cmd([WPTN, 16] + [self.fill] * 16)
        self.org()
        for rn in range(0x0E):
            self.wpr(rn)

    def mw(self):
        r = self.rng
        k = r.random()
        if k < 0.6:
            return r.randint(1, 64)
        if k < 0.9:
            return r.randint(65, 512)
        if k < 0.98:
            return r.randint(513, 4095)
        return 0

    def org(self):
        r = self.rng
        a = self.addr()
        self.cmd([ORG, r.randrange(4) << 14 | a >> 12, (a & 0xFFF) << 4 | r.randrange(16)])
        self.cp = (0, 0)

    def wpr(self, rn):
        r = self.rng
        if rn in (0, 1, 2, 3):
            v = self.color()
        elif rn == 4:
            v = r.choice([0xFFFF, 0, r.getrandbits(16), r.getrandbits(16)])
        elif rn == 5:
            v = r.getrandbits(16)
        elif rn == 6:
            # PS ≦ PE を保つ（MAME はパターンの幅で割る）
            self.psx, self.psy = r.randint(0, self.pex), r.randint(0, self.pey)
            v = self.psy << 12 | self.psx << 4
        elif rn == 7:
            self.pex, self.pey = r.randint(self.psx, 15), r.randint(self.psy, 15)
            v = self.pey << 12 | r.randrange(16) << 8 | self.pex << 4 | r.randrange(16)
        elif rn == 0x0C:
            v = r.randrange(4) << 14 | r.getrandbits(8)
        elif rn == 0x0D:
            v = r.getrandbits(12) << 4
        else:
            v = r.getrandbits(16)
        self.cmd([WPR | rn, v])

    # ---- コマンド ----

    def line_cmd(self):
        r = self.rng
        rel = r.random() < 0.5
        dx, dy = self.delta(r.choice([8, 40, 120]))
        x, y = self.cp[0] + dx, self.cp[1] + dy
        p = (dx, dy) if rel else (x, y)
        self.cmd([(RLINE if rel else ALINE) | self.low(), p[0], p[1]])
        self.cp = (s16(x), s16(y))

    def rect_cmd(self):
        r = self.rng
        rel = r.random() < 0.5
        dx, dy = r.randint(-60, 60), r.randint(-60, 60)
        p = (dx, dy) if rel else (self.cp[0] + dx, self.cp[1] + dy)
        self.cmd([(RRCT if rel else ARCT) | self.low(), p[0], p[1]])

    def poly_cmd(self):
        r = self.rng
        rel, closed = r.random() < 0.5, r.random() < 0.5
        op = (APLG if closed else APLL) | (0x0400 if rel else 0)
        n = 0 if r.random() < 0.03 else r.randint(1, 6)
        start = self.cp
        for _ in range(20):
            pts, cur, ok = [], start, True
            for _ in range(n):
                d = self.delta(60)
                cur = (cur[0] + d[0], cur[1] + d[1])
                pts.append(d if rel else cur)
            if closed and n and tie(start[0] - cur[0], start[1] - cur[1]) and r.random() < 0.75:
                ok = False
            if ok:
                break
        words = [op | self.low(), n]
        for p in pts:
            words += [p[0], p[1]]
        self.cmd(words)
        if n == 0:
            # MAME は APLL・RPLL の n＝0 で CP を (0, 0) にする（既知の差。照合の試験は CP を MAME にそろえる）
            if not closed:
                self.cp = (0, 0)
        elif not closed:
            self.cp = (s16(cur[0]), s16(cur[1]))

    def frct_cmd(self):
        r = self.rng
        rel = r.random() < 0.5
        dx, dy = r.randint(-30, 30), r.randint(-30, 30)
        x, y = self.cp[0] + dx, self.cp[1] + dy
        p = (dx, dy) if rel else (x, y)
        self.cmd([(RFRCT if rel else AFRCT) | self.low(), p[0], p[1]])
        self.cp = (self.cp[0], s16(y + (-1 if dy < 0 else 1)))

    def ptn_cmd(self):
        r = self.rng
        sl, sd = r.randrange(2), r.randrange(8)
        szx, szy = r.randint(0, 23), r.randint(0, 23)
        if r.random() < 0.05:
            szx = r.randint(0, 255)
        elif r.random() < 0.05:
            szy = r.randint(0, 255)
        self.cmd([PTN | sl << 11 | sd << 8 | self.low(), szy << 8 | szx])
        adv = DIRS[(sd + (1 if sl else 2)) & 7]
        n = szy + 1
        self.cp = (s16(self.cp[0] + n * adv[0]), s16(self.cp[1] + n * adv[1]))

    def gcpy_cmd(self):
        r = self.rng
        rel = r.random() < 0.5
        s, dsd = r.randrange(2), r.randrange(8)
        dx, dy = r.randint(-16, 16), r.randint(-16, 16)
        src = self.point(40)
        p = (src[0] - self.cp[0], src[1] - self.cp[1]) if rel else src
        self.cmd([(RGCPY if rel else AGCPY) | s << 11 | dsd << 8 | self.low(True), p[0], p[1], dx, dy])
        count = (abs(dx) if s else abs(dy)) + 1
        if dsd & 4:
            self.cp = (s16(self.cp[0] + (-1 if dsd & 2 else 1) * count), self.cp[1])
        else:
            self.cp = (self.cp[0], s16(self.cp[1] + (-1 if dsd & 1 else 1) * count))

    def cpy_cmd(self):
        r = self.rng
        mm = r.random() < 0.5
        s, dsd = r.randrange(2), r.randrange(8)
        sa = self.addr()
        ax, ay = r.randint(-12, 12), r.randint(-12, 12)
        if r.random() < 0.3:
            # 転送先を転送元の近くに置き、重ねて写す
            rwp = (sa + r.randint(-40, 40)) & 0xFFFFF
            self.cmd([WPR | 0x0C, r.randrange(4) << 14 | rwp >> 12])
            self.cmd([WPR | 0x0D, (rwp & 0xFFF) << 4])
        op = (SCPY | r.randrange(4)) if mm else CPY
        self.cmd([op | s << 11 | dsd << 8, sa >> 12, (sa & 0xFFF) << 4, ax, ay])

    def clr_cmd(self):
        r = self.rng
        mm = r.random() < 0.5
        op = (SCLR | r.randrange(4)) if mm else CLR
        self.cmd([op, r.getrandbits(16), r.randint(-24, 24), r.randint(-24, 24)])

    def dwt_cmd(self):
        r = self.rng
        ax, ay = r.randint(-5, 5), r.randint(-5, 5)
        op = (DMOD | r.randrange(4)) if r.random() < 0.5 else DWT
        data = [r.getrandbits(16) for _ in range((abs(ax) + 1) * (abs(ay) + 1))]
        self.cmd([op, ax, ay] + data)

    def move_cmd(self):
        r = self.rng
        if r.random() < 0.5 or max(abs(self.cp[0]), abs(self.cp[1])) > 3000:
            x, y = r.randint(-300, 300), r.randint(-300, 300)
            self.cmd([AMOVE, x, y])
            self.cp = (x, y)
        else:
            dx, dy = r.randint(-100, 100), r.randint(-100, 100)
            self.cmd([RMOVE, dx, dy])
            self.cp = (s16(self.cp[0] + dx), s16(self.cp[1] + dy))

    def undefined(self):
        r = self.rng
        self.cmd([r.choice(UNDEFINED) << 10 | r.getrandbits(10)])
        self.abort()

    def partial(self):
        """パラメータが足りないところで止め、ABT で打ち切る。"""
        r = self.rng
        k = r.randrange(6)
        if k == 0:
            words = [ALINE | self.low(), r.randint(-50, 50)]
        elif k == 1:
            words = [CLR, r.getrandbits(16), r.randint(-5, 5)][:r.randint(1, 3)]
        elif k == 2:
            words = [AGCPY | self.low(), 3, 4, 5][:r.randint(1, 4)]
        elif k == 3:
            # 多角形と WPTN は使わない。コアは点・語が届くごとに描くが、MAME はパラメータがそろうまで待つ
            words = [CPY | r.randrange(8) << 8, 1, 2, 3][:r.randint(1, 4)]
        elif k == 4:
            ax, ay = r.randint(0, 3), r.randint(0, 3)
            total = (ax + 1) * (ay + 1)
            words = [DWT, ax, ay] + [r.getrandbits(16) for _ in range(r.randint(0, total - 1))]
        else:
            words = [ORG, r.getrandbits(16)]
        self.cmd(words, 'H')
        self.abort()

    def step(self):
        r = self.rng
        k = r.random()
        table = [
            (0.10, lambda: self.wpr(r.choice(list(range(0x0E)) + [0x0C, 0x0D, 0x0E, 0x12, 0x15]))),
            (0.07, lambda: self.cmd([RPR | r.choice(list(range(0x0E)) + [0x10, 0x11, 0x12, 0x13] * 3 + [0x0E, 0x14, 0x1F])])),
            (0.01, lambda: self.cmd([WPTN | r.randrange(16), (n := r.randint(0, 20))] + [self.fill] * n)),
            (0.02, self.org),
            (0.03, lambda: self.cmd([RD])),
            (0.03, lambda: self.cmd([WT, r.getrandbits(16)])),
            (0.03, lambda: self.cmd([MOD | r.randrange(4), r.getrandbits(16)])),
            (0.06, self.clr_cmd),
            (0.06, self.cpy_cmd),
            (0.05, self.dwt_cmd),
            (0.07, self.move_cmd),
            (0.12, self.line_cmd),
            (0.05, self.rect_cmd),
            (0.06, self.poly_cmd),
            (0.06, self.frct_cmd),
            (0.03, lambda: self.cmd([DOT | self.low()])),
            (0.06, self.ptn_cmd),
            (0.06, self.gcpy_cmd),
            (0.01, self.undefined),
            (0.01, self.partial),
        ]
        acc = 0.0
        for w, fn in table:
            acc += w
            if k < acc:
                fn()
                return
        # 残り：GBM か MW を変え、ORG で DP と CP を求め直す
        if r.random() < 0.5:
            self.gbm = r.randrange(5)
            self.ccr()
        else:
            self.reg(r.choice((0xC2, 0xCA, 0xD2, 0xDA)), self.mw())
        self.org()


def main(argv):
    out = argv[0]
    cases = int(argv[1]) if len(argv) > 1 else 200
    cmds = int(argv[2]) if len(argv) > 2 else 200
    seed = int(argv[3]) if len(argv) > 3 else 1985
    rng = random.Random(seed)
    with open(out, 'w', encoding='ascii', newline='\n') as f:
        for c in range(cases):
            f.write(f'C {c} {rng.getrandbits(32):X}\n')
            case = Case(rng, f)
            case.setup()
            for _ in range(cmds):
                case.step()
    print(f'{out}: {cases} 件、{cmds} コマンドずつ')


if __name__ == '__main__':
    main(sys.argv[1:])
