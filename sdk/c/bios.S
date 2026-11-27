/* bios.S：BIOS の呼出しを C から呼ぶ小さな関数（DR-103。宣言は include/bios.h、BIOS の約束は sdk/docs/bios-abi.md）。
 *
 * C（GCC の m68k の呼び方）：引数はスタックに 4 byte ずつ（int の値は下位の語が +2）、結果は D0（ポインタは
 * D0 と A0）、D0・D1・A0・A1 は壊してよく、ほかは保つ。BIOS は出力のほかのレジスタを保つが、ここで D2〜D5 に
 * 入力を入れるので、呼んだ側の D2〜D5 は自分で積んで戻す。Z の出力は 1（Z＝1）か 0 にして返す。
 */

#define FUNC(name)                         \
        .section .text.name,"ax",@progbits; \
        .globl  name;                       \
        .type   name,@function;             \
name:

/* D0 ← Z（1 か 0）。D1 を壊す */
#define Z_TO_D0                             \
        seq     %d1;                        \
        moveq   #1,%d0;                     \
        and.l   %d1,%d0

/* void bios_wait_vbl(void) */
FUNC(bios_wait_vbl)
        jmp     0x400

/* int bios_acrtc_send(const uint16_t *words, unsigned count) */
FUNC(bios_acrtc_send)
        movea.l 4(%sp),%a0
        move.w  10(%sp),%d1
        jsr     0x406
        Z_TO_D0
        rts

/* int bios_cls(unsigned color) */
FUNC(bios_cls)
        move.w  6(%sp),%d0
        jsr     0x40C
        Z_TO_D0
        rts

/* int bios_puts(int x, int y, const char *s, unsigned fg, unsigned bg) */
FUNC(bios_puts)
        movem.l %d2-%d3,-(%sp)          /* 引数は +8 */
        move.w  14(%sp),%d0
        move.w  18(%sp),%d1
        movea.l 20(%sp),%a0
        move.w  26(%sp),%d2
        move.w  30(%sp),%d3
        jsr     0x412
        Z_TO_D0
        movem.l (%sp)+,%d2-%d3
        rts

/* int bios_put_hex(int x, int y, uint32_t value, unsigned digits, unsigned fg, unsigned bg) */
FUNC(bios_put_hex)
        movem.l %d2-%d5,-(%sp)          /* 引数は +16 */
        move.w  22(%sp),%d0
        move.w  26(%sp),%d1
        move.l  28(%sp),%d4
        move.w  34(%sp),%d5
        move.w  38(%sp),%d2
        move.w  42(%sp),%d3
        jsr     0x418
        Z_TO_D0
        movem.l (%sp)+,%d2-%d5
        rts

/* void bios_set_pal(const uint16_t pal[16]) */
FUNC(bios_set_pal)
        movea.l 4(%sp),%a0
        jmp     0x41E

/* uint16_t bios_read_pad(uint16_t *pressed) */
FUNC(bios_read_pad)
        jsr     0x424
        movea.l 4(%sp),%a0
        cmpa.w  #0,%a0
        beq.s   1f
        move.w  %d1,(%a0)
1:      andi.l  #0xFFFF,%d0
        rts

/* int bios_puts_kuten(int x, int y, const uint16_t *s, unsigned fg, unsigned bg) */
FUNC(bios_puts_kuten)
        movem.l %d2-%d3,-(%sp)
        move.w  14(%sp),%d0
        move.w  18(%sp),%d1
        movea.l 20(%sp),%a0
        move.w  26(%sp),%d2
        move.w  30(%sp),%d3
        jsr     0x42A
        Z_TO_D0
        movem.l (%sp)+,%d2-%d3
        rts

/* const volatile uint16_t *bios_glyph(unsigned code, int *found) */
FUNC(bios_glyph)
        move.w  6(%sp),%d0
        jsr     0x430
        seq     %d1
        movea.l 8(%sp),%a1
        cmpa.w  #0,%a1
        beq.s   1f
        moveq   #1,%d0
        and.l   %d1,%d0
        move.l  %d0,(%a1)
1:      move.l  %a0,%d0
        rts
