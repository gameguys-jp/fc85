/* fabric.c：Data Fabric を C から使う関数（DR-106。約束は include/fabric.h）。手順はサンプル（sdk/samples/game の
 * music.s・sfx.s）と同じ。 */
#include <string.h>

#include "fabric.h"
#include "fc85.h"

void fab_stop(unsigned ch)
{
    REG_FAB_CTRL(ch) = FAB_STOP;
    while (REG_FAB_STATUS(ch) & FAB_ST_BUSY) {
    }
    REG_FAB_STATUS(ch) = FAB_ST_W1C;
}

void fab_set(unsigned ch, const struct fab_xfer *x)
{
    fab_stop(ch);
    /* TRIGGER の発生源を替えてから START まで 4 スロット以上（guarantees.md §03 の 24）。最初に書く */
    REG_FAB_TRIGGER(ch) = x->trigger;
    REG_FAB_SRC(ch) = x->src;
    REG_FAB_DST(ch) = x->dst;
    REG_FAB_SRC_ADDR_L(ch) = x->src_addr;
    REG_FAB_DST_ADDR_L(ch) = x->dst_addr;
    REG_FAB_COUNT(ch) = x->count;
    REG_FAB_SRC_STEP(ch) = (uint16_t)x->src_step;
    REG_FAB_DST_STEP(ch) = (uint16_t)x->dst_step;
    REG_FAB_FORMAT(ch) = x->format;
    REG_FAB_MASK(ch) = (uint16_t)~x->clear;
    REG_FAB_XOR(ch) = x->flip;
    REG_FAB_SHIFT(ch) = x->shift;
}

void fab_start(unsigned ch)
{
    REG_FAB_STATUS(ch) = FAB_ST_W1C;
    REG_FAB_CTRL(ch) = FAB_START;
}

unsigned fab_status(unsigned ch)
{
    return REG_FAB_STATUS(ch);
}

unsigned fab_wait(unsigned ch)
{
    for (;;) {
        unsigned s = REG_FAB_STATUS(ch);
        if (s & (FAB_ST_DONE | FAB_ST_ERROR))
            return s;
    }
}

unsigned fab_run(unsigned ch, const struct fab_xfer *x)
{
    fab_set(ch, x);
    fab_start(ch);
    unsigned s = fab_wait(ch);
    fab_stop(ch);
    return s;
}

void fab_to_staging(uint16_t addr, const void *src, unsigned n)
{
    memcpy((void *)(STAGING + addr), src, n);
}
