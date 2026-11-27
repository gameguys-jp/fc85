/* video_irq.S：レベル 6（映像）の割込みの入口（DR-109。使い方は include/video.h）。
 *
 * VIDEO_IRQ を読み、HBL の保留があれば、許可のビットはそのままで HBL だけを消し（W1C）、fc85_video_hook を
 * 呼ぶ。終わったら、レジスタを戻して BIOS の処理（fc85_video_prev）へ飛ぶ。BIOS は残った保留（VBlank）を
 * 消し、bios_frame を進め、RTE で戻る（bios-abi.md の「処理を差し替える」）。hook は C の関数で、D0・D1・
 * A0・A1 を壊してよいので、ここで積む。
 */

#define VIDEO_IRQ 0x270022

        .section .text.fc85_video_irq,"ax",@progbits
        .globl  fc85_video_irq
        .type   fc85_video_irq,@function
fc85_video_irq:
        movem.l %d0-%d1/%a0-%a1,-(%sp)
        move.w  VIDEO_IRQ,%d0
        btst    #9,%d0                  /* HBL の保留 */
        beq.s   1f
        andi.w  #0x0007,%d0             /* 許可（bit 2–0）はそのまま */
        ori.w   #0x0200,%d0             /* HBL だけを消す。VBL（bit 8）は 0 を書くので残る */
        move.w  %d0,VIDEO_IRQ
        move.l  fc85_video_hook,%d0
        beq.s   1f
        movea.l %d0,%a0
        jsr     (%a0)
1:      movem.l (%sp)+,%d0-%d1/%a0-%a1
        move.l  fc85_video_prev,-(%sp)
        rts
