/* doc.c：DOC（ES5503）を C から使う関数（DR-106。約束は include/doc.h）。手順は bios-abi.md §06 と、サンプル
 * （sdk/samples/game/music.s）と同じ。 */
#include "bios.h"
#include "doc.h"
#include "fabric.h"
#include "fc85.h"

int doc_present(void)
{
    return (bios_modules & FABRIC_MOD_DOC) != 0;
}

unsigned doc_oscillators(void)
{
    /* 0xE1 は (N−1)×2 を bit 5–1 に返す（台帳 ES5503.md） */
    uint8_t v = REG_DOC_OSC_ENABLE;
    return ((v >> 1) & 0x1F) + 1;
}

void doc_set_oscillators(unsigned n)
{
    if (n < 1)
        n = 1;
    if (n > 32)
        n = 32;
    /* 走査の外のオシレータのレジスタは崩れている（DR-074）。H＝1・IE＝0 にしてから走査に入れ、続けて 0xE1 を書く
     * （bios-abi.md §06、DR-094 (7)） */
    uint16_t sr = fc85_irq_disable();
    for (unsigned o = doc_oscillators(); o < n; o++)
        REG_DOC_CONTROL(o) = (uint8_t)(DOC_CTL_HALT | (15U << DOC_CTL_CA_SH));
    REG_DOC_OSC_ENABLE = (uint8_t)((n - 1) * 2);
    fc85_irq_restore(sr);
}

void doc_set(unsigned osc, const struct doc_osc *v)
{
    REG_DOC_FREQ_LO(osc) = (uint8_t)v->freq;
    REG_DOC_FREQ_HI(osc) = (uint8_t)(v->freq >> 8);
    REG_DOC_VOLUME(osc) = v->volume;
    REG_DOC_POINTER(osc) = v->pointer;
    REG_DOC_SIZE(osc) = v->size;
    REG_DOC_CONTROL(osc) = v->control;
}

void doc_freq(unsigned osc, uint16_t freq)
{
    REG_DOC_FREQ_LO(osc) = (uint8_t)freq;
    REG_DOC_FREQ_HI(osc) = (uint8_t)(freq >> 8);
}

void doc_volume(unsigned osc, unsigned volume)
{
    REG_DOC_VOLUME(osc) = (uint8_t)volume;
}

void doc_halt(unsigned osc)
{
    uint8_t c = REG_DOC_CONTROL(osc);
    REG_DOC_CONTROL(osc) = (uint8_t)(c | DOC_CTL_HALT);
}

void doc_go(unsigned osc)
{
    uint8_t c = REG_DOC_CONTROL(osc);
    REG_DOC_CONTROL(osc) = (uint8_t)(c & ~DOC_CTL_HALT);
}

unsigned doc_load_wave(unsigned ch, uint16_t wave, uint16_t staging, uint16_t bytes)
{
    if (!doc_present())
        return FAB_ST_ERROR;
    const struct fab_xfer x = {
        .src = PORT_STAGING,
        .dst = PORT_DOC,
        .src_addr = staging,
        .dst_addr = wave,
        .count = bytes,
        .src_step = 1,
        .dst_step = 1,
        .format = FMT_8_8,
        .trigger = TRG_NOW,
    };
    return fab_run(ch, &x);
}

/* 0xE2 を MOVE.B で 1 回読む。値を使わない読みを GCC が TST にしないよう、命令を決めて書く（DR-030、DR-123） */
static inline uint8_t adc_read(void)
{
    uint8_t v;
    __asm__ volatile("move.b %1,%0" : "=d"(v) : "m"(REG_DOC_ADC));
    return v;
}

uint8_t doc_adc(void)
{
    return adc_read();
}

/* ラスタが line から変わるまで待ち、新しいラスタを返す */
static unsigned next_line(unsigned line)
{
    unsigned now;
    while ((now = REG_VIDEO_LINE) == line) {
    }
    return now;
}

unsigned doc_adc_record(uint8_t *buf, unsigned n, unsigned lines, unsigned threshold, uint32_t wait)
{
    if (n == 0 || !doc_present())
        return 0;
    if (lines < 1)
        lines = 1;
    uint16_t sr = fc85_irq_disable();
    unsigned line = next_line(REG_VIDEO_LINE);      /* ラスタの変わり目から */
    (void)adc_read();                               /* 最初の変換を始める（値は捨てる） */
    unsigned got = 0;
    uint32_t waited = 0;
    while (got < n) {
        for (unsigned k = 0; k < lines; k++)
            line = next_line(line);
        uint8_t v = adc_read();                     /* ラスタ lines 本前に始めた変換の結果 */
        if (got == 0 && threshold) {
            unsigned d = v >= 128 ? v - 128U : 128U - v;
            if (d < threshold) {
                waited += lines;
                if (waited >= wait)
                    break;
                continue;
            }
        }
        buf[got++] = v ? v : 1;
    }
    fc85_irq_restore(sr);
    return got;
}
