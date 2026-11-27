/* temporal.c：Temporal RAM のポインタと制御（DR-113。約束は include/temporal.h）。 */
#include "fc85.h"
#include "temporal.h"

void tmp_reset(uint16_t mode)
{
    REG_TEMPORAL_CTRL = (uint16_t)(TMP_CLEAR | (mode & TMP_MODES));
}

void tmp_mode(uint16_t mode)
{
    /* OVERRUN・UNDERRUN に 0 を書くので、印は残る */
    REG_TEMPORAL_CTRL = (uint16_t)(mode & TMP_MODES);
}

uint16_t tmp_get_mode(void)
{
    uint16_t v = REG_TEMPORAL_CTRL;
    return (uint16_t)(v & TMP_MODES);
}

void tmp_seek(uint16_t wptr, uint16_t rptr)
{
    REG_TEMPORAL_WPTR = wptr;
    REG_TEMPORAL_RPTR = rptr;
}

uint16_t tmp_flags(void)
{
    uint16_t v = REG_TEMPORAL_CTRL;
    return (uint16_t)(v & (TMP_EQUAL | TMP_OVERRUN | TMP_UNDERRUN));
}

void tmp_clear_flags(void)
{
    uint16_t v = REG_TEMPORAL_CTRL;
    REG_TEMPORAL_CTRL = (uint16_t)((v & TMP_MODES) | TMP_OVERRUN | TMP_UNDERRUN);
}
