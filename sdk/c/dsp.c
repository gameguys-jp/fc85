/* dsp.c：DSP（TMS32010）を C から使う関数（DR-109。約束は include/dsp.h）。手順はサンプル
 * （sdk/samples/dsp/main.s）と同じ（DR-029）。 */
#include "dsp.h"
#include "fc85.h"

/* PRGBUS を待つ回数（RESET から 8 CLKOUT＝1.6 µs。サンプルと同じく十分に長く） */
#define PRGBUS_TRIES 1000U

int dsp_load(const uint16_t *code, unsigned words, int zero_rest)
{
    if (words > DSP_PROGRAM_WORDS)
        words = DSP_PROGRAM_WORDS;
    REG_DSP_CTRL = DSP_CTRL_RESET;
    unsigned n = PRGBUS_TRIES;
    while (!(REG_DSP_STATUS & (1U << DSP_ST_PRGBUS_B))) {
        if (--n == 0)
            return -1;
    }
    unsigned end = zero_rest ? DSP_PROGRAM_WORDS : words;
    for (unsigned i = 0; i < end; i++)
        REG_DSP_PROGRAM(i) = i < words ? code[i] : 0;
    int bad = 0;
    for (unsigned i = 0; i < end; i++) {
        uint16_t want = i < words ? code[i] : 0;
        if (REG_DSP_PROGRAM(i) != want)
            bad++;
    }
    return bad;
}

void dsp_start(uint16_t ctrl)
{
    REG_DSP_CTRL = (uint16_t)(ctrl & ~DSP_CTRL_RESET);
}

void dsp_reset(void)
{
    REG_DSP_CTRL = DSP_CTRL_RESET;
}

int dsp_command(uint16_t cmd, uint16_t *reply, unsigned tries)
{
    REG_DSP_MAILBOX = cmd;
    while (!(REG_DSP_STATUS & (1U << DSP_ST_RESPONSE_B))) {
        if (tries-- == 0)
            return 0;
    }
    /* 読むと RESPONSE が下がる */
    uint16_t v = REG_DSP_RESPONSE;
    if (reply)
        *reply = v;
    return 1;
}

uint16_t dsp_status(void)
{
    return REG_DSP_STATUS;
}

void dsp_clear_flags(void)
{
    REG_DSP_STATUS = DSP_ST_UNDERRUN | DSP_ST_OVERRUN;
}
