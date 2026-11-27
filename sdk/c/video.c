/* video.c：パレットと HBlank の割込みを C から使う関数（DR-109。約束は include/video.h）。割込みの入口は
 * video_irq.S。 */
#include "bios.h"
#include "fc85.h"
#include "video.h"

/* video_irq.S が使う：hook と、つなぐ先（BIOS のレベル 6 の処理） */
video_hook_t fc85_video_hook;
bios_hook_t fc85_video_prev;
void fc85_video_irq(void);

#define VIDEO_VECTOR (VEC_AUTO1 + IRQ_VIDEO - 1U)

void video_set_palette(const uint16_t *rgb, unsigned first, unsigned count)
{
    for (unsigned i = 0; i < count && first + i < 16; i++)
        REG_PALETTE(first + i) = rgb[i];
}

void video_hblank(video_hook_t hook, int line)
{
    uint16_t sr = fc85_irq_disable();
    fc85_video_hook = hook;
    if (fc85_video_prev == 0)
        fc85_video_prev = fc85_set_vector(VIDEO_VECTOR, fc85_video_irq);
    uint16_t en = (uint16_t)((REG_VIDEO_IRQ & VIRQ_VBL_EN) | VIRQ_HBL_EN);
    if (line >= 0) {
        REG_VIDEO_LINE_CMP = (uint16_t)(line & 0x0FFF);
        en |= VIRQ_LINE_CMP;
    }
    /* 許可を書き、前からある HBL の保留を消す（VBL の保留は BIOS の処理に残す） */
    REG_VIDEO_IRQ = (uint16_t)(en | VIRQ_HBL);
    fc85_irq_restore(sr);
}

void video_hblank_off(void)
{
    uint16_t sr = fc85_irq_disable();
    REG_VIDEO_IRQ = (uint16_t)((REG_VIDEO_IRQ & VIRQ_VBL_EN) | VIRQ_HBL);
    if (fc85_video_prev != 0) {
        fc85_set_vector(VIDEO_VECTOR, fc85_video_prev);
        fc85_video_prev = 0;
    }
    fc85_video_hook = 0;
    fc85_irq_restore(sr);
}
