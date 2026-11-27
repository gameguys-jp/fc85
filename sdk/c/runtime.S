/* runtime.S：GCC が 68000 で呼ぶ 32 bit の掛け算と割り算（DR-103。sdk/docs/c.md §05）。
 *
 * 68000 には 32×32 の掛け算と 32÷32 の割り算の命令がないので、GCC はこの関数を呼ぶ（名前と引数は
 * GCC の実行時ライブラリと同じ。中身はここで書いた）。引数はスタック（4(%sp)、8(%sp)）、結果は D0。
 * D0・D1・A0・A1 のほかは保つ。0 で割ると DIVU の 0 除算の例外（ベクタ 5）になり、BIOS が赤い画面で止まる。
 */

        .text

/* uint32_t __mulsi3(uint32_t a, uint32_t b)：下位 32 bit。符号の有無で同じ */
        .section .text.__mulsi3,"ax",@progbits
        .globl  __mulsi3
        .type   __mulsi3,@function
__mulsi3:
        move.w  4(%sp),%d0              /* a の上位 × b の下位 */
        mulu.w  10(%sp),%d0
        move.w  6(%sp),%d1              /* a の下位 × b の上位 */
        mulu.w  8(%sp),%d1
        add.w   %d1,%d0
        swap    %d0
        clr.w   %d0
        move.w  6(%sp),%d1              /* a の下位 × b の下位 */
        mulu.w  10(%sp),%d1
        add.l   %d1,%d0
        rts
        .size   __mulsi3,.-__mulsi3

/* 符号なしの割り算の本体。入力 D0＝n、D1＝d。出力 D0＝n÷d、D1＝n mod d。D2・D3・A1 を壊す */
/* 割り算の 4 つと本体は 1 つの節に置く（BSR の届く範囲。使わなければまとめて落ちる） */
        .section .text.__fc85_div,"ax",@progbits
        .type   __fc85_udiv,@function
__fc85_udiv:
        cmp.l   #0xFFFF,%d1
        bhi.s   .Lbig
        /* d が 16 bit：上位と下位を DIVU で 2 回 */
        move.l  %d0,%d2
        clr.w   %d2
        swap    %d2                     /* D2＝n の上位 */
        divu.w  %d1,%d2                 /* D2＝余り r1 : 商 q1（d＝0 ならここで例外） */
        move.w  %d2,%a1                 /* A1 の下位語＝q1 */
        move.w  %d0,%d2                 /* D2＝r1 : n の下位 */
        divu.w  %d1,%d2                 /* D2＝余り : 商 q2（r1＜d なので桁あふれしない） */
        move.l  %d2,%d1
        clr.w   %d1
        swap    %d1                     /* D1＝余り */
        move.w  %a1,%d0
        swap    %d0                     /* D0 の上位語＝q1 */
        move.w  %d2,%d0                 /* D0 の下位語＝q2 */
        rts
.Lbig:  /* d ≥ $10000：商は 16 bit。n の上位から 1 bit ずつ引く（16 回） */
        move.l  %d0,%d2
        clr.w   %d2
        swap    %d2                     /* D2＝余り（n の上位から始める。d より小さい） */
        swap    %d0
        clr.w   %d0                     /* D0 の上位語＝n の下位。下位語に商がたまる */
        moveq   #15,%d3
1:      add.l   %d0,%d0
        addx.l  %d2,%d2                 /* (余り:n) を 1 bit 左へ */
        bcs.s   2f                      /* 余りが 32 bit を超えた：d より大きい */
        cmp.l   %d1,%d2
        bcs.s   3f                      /* 余り＜d */
2:      sub.l   %d1,%d2
        addq.l  #1,%d0                  /* 商の bit を立てる */
3:      dbra    %d3,1b
        move.l  %d2,%d1
        rts
        .size   __fc85_udiv,.-__fc85_udiv

/* uint32_t __udivsi3(uint32_t n, uint32_t d) */
        .globl  __udivsi3
        .type   __udivsi3,@function
__udivsi3:
        movem.l %d2-%d3,-(%sp)
        move.l  12(%sp),%d0
        move.l  16(%sp),%d1
        bsr     __fc85_udiv
        movem.l (%sp)+,%d2-%d3
        rts
        .size   __udivsi3,.-__udivsi3

/* uint32_t __umodsi3(uint32_t n, uint32_t d) */
        .globl  __umodsi3
        .type   __umodsi3,@function
__umodsi3:
        movem.l %d2-%d3,-(%sp)
        move.l  12(%sp),%d0
        move.l  16(%sp),%d1
        bsr     __fc85_udiv
        move.l  %d1,%d0
        movem.l (%sp)+,%d2-%d3
        rts
        .size   __umodsi3,.-__umodsi3

/* int32_t __divsi3(int32_t n, int32_t d)：0 の方へ丸める（C と同じ） */
        .globl  __divsi3
        .type   __divsi3,@function
__divsi3:
        movem.l %d2-%d3,-(%sp)
        move.l  12(%sp),%d0
        move.l  16(%sp),%d1
        move.l  %d0,%d2
        eor.l   %d1,%d2
        move.l  %d2,%a0                 /* A0 の bit 31＝商の符号 */
        tst.l   %d0
        bpl.s   1f
        neg.l   %d0
1:      tst.l   %d1
        bpl.s   2f
        neg.l   %d1
2:      bsr     __fc85_udiv
        move.l  %a0,%d2
        bpl.s   3f
        neg.l   %d0
3:      movem.l (%sp)+,%d2-%d3
        rts
        .size   __divsi3,.-__divsi3

/* int32_t __modsi3(int32_t n, int32_t d)：余りの符号は n と同じ（C と同じ） */
        .globl  __modsi3
        .type   __modsi3,@function
__modsi3:
        movem.l %d2-%d3,-(%sp)
        move.l  12(%sp),%d0
        move.l  16(%sp),%d1
        move.l  %d0,%a0                 /* A0 の bit 31＝余りの符号 */
        tst.l   %d0
        bpl.s   1f
        neg.l   %d0
1:      tst.l   %d1
        bpl.s   2f
        neg.l   %d1
2:      bsr     __fc85_udiv
        move.l  %d1,%d0
        move.l  %a0,%d2
        bpl.s   3f
        neg.l   %d0
3:      movem.l (%sp)+,%d2-%d3
        rts
        .size   __modsi3,.-__modsi3
