/* crt0.S：C の ROM の入口（DR-103。説明は sdk/docs/c.md §03）。
 *
 * BIOS は見出し（FC85_CART）の入口 _start へ、スーパバイザ、SR＝$2000、SSP＝$120000 で飛ぶ
 * （sdk/docs/bios-abi.md §04）。ここで .data の初期値を ROM から Main RAM へ写し、.bss を 0 にして main を呼ぶ。
 * main から戻ったら VBlank を待ち続ける（割込みは受け続ける）。
 */

        .section .text.start,"ax",@progbits
        .globl  _start
        .type   _start,@function
_start:
        lea     __stack_top,%sp
        lea     __data_load,%a0         /* .data：ROM の初期値を RAM へ（語ずつ。両端は偶数） */
        lea     __data_start,%a1
        lea     __data_end,%a2
1:      cmpa.l  %a2,%a1
        bcc.s   2f
        move.w  (%a0)+,(%a1)+
        bra.s   1b
2:      lea     __bss_start,%a1         /* .bss：0 にする */
        lea     __bss_end,%a2
        moveq   #0,%d0
3:      cmpa.l  %a2,%a1
        bcc.s   4f
        move.w  %d0,(%a1)+
        bra.s   3b
4:      jsr     main
5:      jsr     0x400                   /* BIOS_WAIT_VBL */
        bra.s   5b
        .size   _start,.-_start
