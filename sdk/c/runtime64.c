/* runtime64.c：GCC が 68000 で呼ぶ 64 bit（long long）のシフト・掛け算・割り算（DR-103。sdk/docs/c.md §05）。
 *
 * 名前と引数は GCC の実行時ライブラリと同じ。中身はここで書いた。足し算・引き算・比べるのは GCC が命令で書くので
 * ここにはない。この中では 64 bit の演算を使わず（自分を呼ぶことになる）、32 bit の上位と下位で書く。
 * 64 bit の割り算は 1 bit ずつ引くので遅い（約 64 回の繰返し）。速さの要るところでは 32 bit で書く。
 */
#include <stdint.h>

typedef union {
    uint64_t u;
    int64_t s;
    struct {
        uint32_t hi, lo; /* 68000 は big-endian */
    } w;
} dw;

uint64_t __ashldi3(uint64_t a, int b);
uint64_t __lshrdi3(uint64_t a, int b);
int64_t __ashrdi3(int64_t a, int b);
uint64_t __muldi3(uint64_t a, uint64_t b);
uint64_t __udivdi3(uint64_t n, uint64_t d);
uint64_t __umoddi3(uint64_t n, uint64_t d);
int64_t __divdi3(int64_t n, int64_t d);
int64_t __moddi3(int64_t n, int64_t d);

uint64_t __ashldi3(uint64_t a, int b)
{
    dw x = {.u = a};
    b &= 63;
    if (b >= 32) {
        x.w.hi = x.w.lo << (b - 32);
        x.w.lo = 0;
    } else if (b > 0) {
        x.w.hi = (x.w.hi << b) | (x.w.lo >> (32 - b));
        x.w.lo <<= b;
    }
    return x.u;
}

uint64_t __lshrdi3(uint64_t a, int b)
{
    dw x = {.u = a};
    b &= 63;
    if (b >= 32) {
        x.w.lo = x.w.hi >> (b - 32);
        x.w.hi = 0;
    } else if (b > 0) {
        x.w.lo = (x.w.lo >> b) | (x.w.hi << (32 - b));
        x.w.hi >>= b;
    }
    return x.u;
}

int64_t __ashrdi3(int64_t a, int b)
{
    dw x = {.s = a};
    uint32_t sign = (x.w.hi & 0x80000000UL) ? 0xFFFFFFFFUL : 0;
    b &= 63;
    if (b >= 32) {
        x.w.lo = (uint32_t)((int32_t)x.w.hi >> (b - 32));
        x.w.hi = sign;
    } else if (b > 0) {
        x.w.lo = (x.w.lo >> b) | (x.w.hi << (32 - b));
        x.w.hi = (uint32_t)((int32_t)x.w.hi >> b);
    }
    return x.s;
}

/* 32×32→64（16×16 の MULU を 4 回） */
static dw mul32x32(uint32_t a, uint32_t b)
{
    uint32_t al = a & 0xFFFF, ah = a >> 16, bl = b & 0xFFFF, bh = b >> 16;
    uint32_t ll = (uint16_t)al * (uint32_t)(uint16_t)bl;
    uint32_t lh = (uint16_t)al * (uint32_t)(uint16_t)bh;
    uint32_t hl = (uint16_t)ah * (uint32_t)(uint16_t)bl;
    uint32_t hh = (uint16_t)ah * (uint32_t)(uint16_t)bh;
    uint32_t mid = lh + hl;
    uint32_t carry = (mid < lh) ? 0x10000UL : 0;
    dw r;
    r.w.lo = ll + (mid << 16);
    r.w.hi = hh + (mid >> 16) + carry + (r.w.lo < ll ? 1 : 0);
    return r;
}

uint64_t __muldi3(uint64_t a, uint64_t b)
{
    dw x = {.u = a}, y = {.u = b};
    dw r = mul32x32(x.w.lo, y.w.lo);
    r.w.hi += x.w.lo * y.w.hi + x.w.hi * y.w.lo;
    return r.u;
}

/* n÷d と余り。d＝0 なら 32 bit の 0 除算（例外）にする */
static void udivmod(dw n, dw d, dw *q, dw *r)
{
    if (n.w.hi == 0 && d.w.hi == 0) {
        q->w.hi = 0;
        q->w.lo = n.w.lo / d.w.lo;
        r->w.hi = 0;
        r->w.lo = n.w.lo % d.w.lo;
        return;
    }
    if (d.w.hi == 0 && d.w.lo == 0) {
        volatile uint32_t zero = 0;
        q->w.lo = n.w.lo / zero;
    }
    dw quo = {.u = 0}, rem = {.u = 0};
    for (int i = 0; i < 64; i++) {
        /* (rem:n) を 1 bit 左へ */
        rem.w.hi = (rem.w.hi << 1) | (rem.w.lo >> 31);
        rem.w.lo = (rem.w.lo << 1) | (n.w.hi >> 31);
        n.w.hi = (n.w.hi << 1) | (n.w.lo >> 31);
        n.w.lo <<= 1;
        quo.w.hi = (quo.w.hi << 1) | (quo.w.lo >> 31);
        quo.w.lo <<= 1;
        if (rem.w.hi > d.w.hi || (rem.w.hi == d.w.hi && rem.w.lo >= d.w.lo)) {
            uint32_t borrow = rem.w.lo < d.w.lo;
            rem.w.lo -= d.w.lo;
            rem.w.hi -= d.w.hi + borrow;
            quo.w.lo |= 1;
        }
    }
    *q = quo;
    *r = rem;
}

uint64_t __udivdi3(uint64_t n, uint64_t d)
{
    dw q, r;
    udivmod((dw){.u = n}, (dw){.u = d}, &q, &r);
    return q.u;
}

uint64_t __umoddi3(uint64_t n, uint64_t d)
{
    dw q, r;
    udivmod((dw){.u = n}, (dw){.u = d}, &q, &r);
    return r.u;
}

/* 2 の補数の符号を反転（64 bit の NEG を使わずに） */
static dw neg(dw x)
{
    x.w.lo = ~x.w.lo + 1;
    x.w.hi = ~x.w.hi + (x.w.lo == 0 ? 1 : 0);
    return x;
}

int64_t __divdi3(int64_t n, int64_t d)
{
    dw a = {.s = n}, b = {.s = d}, q, r;
    int negq = 0;
    if (a.w.hi & 0x80000000UL) {
        a = neg(a);
        negq = !negq;
    }
    if (b.w.hi & 0x80000000UL) {
        b = neg(b);
        negq = !negq;
    }
    udivmod(a, b, &q, &r);
    return negq ? neg(q).s : q.s;
}

int64_t __moddi3(int64_t n, int64_t d)
{
    dw a = {.s = n}, b = {.s = d}, q, r;
    int negr = 0;
    if (a.w.hi & 0x80000000UL) {
        a = neg(a);
        negr = 1;
    }
    if (b.w.hi & 0x80000000UL)
        b = neg(b);
    udivmod(a, b, &q, &r);
    return negr ? neg(r).s : r.s;
}
