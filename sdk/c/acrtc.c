/* acrtc.c：ACRTC（HD63484）を C から使う関数（DR-106。約束は include/acrtc.h、説明は sdk/docs/c.md §09）。
 * 書き方は Boot ROM の FIFO の書き方（firmware/boot/video.s、bios-abi.md §05）にそろえ、見張りの回数だけを
 * 除いた。 */
#include "acrtc.h"
#include "fc85.h"

void acrtc_abort(void)
{
    /* Boot ROM と同じく、CCR に触るたびに AR を書き直す */
    REG_ACRTC_AR = ACRTC_CCR;
    uint16_t ccr = REG_ACRTC_DATA;
    REG_ACRTC_AR = ACRTC_CCR;
    REG_ACRTC_DATA = ccr | ACRTC_CCR_ABT;
    REG_ACRTC_AR = ACRTC_CCR;
    REG_ACRTC_DATA = ccr & (uint16_t)~ACRTC_CCR_ABT;
    REG_ACRTC_AR = ACRTC_FIFO;
}

void acrtc_write_reg(unsigned reg, uint16_t v)
{
    REG_ACRTC_AR = reg;
    REG_ACRTC_DATA = v;
    REG_ACRTC_AR = ACRTC_FIFO;
}

uint16_t acrtc_read_reg(unsigned reg)
{
    REG_ACRTC_AR = reg;
    uint16_t v = REG_ACRTC_DATA;
    REG_ACRTC_AR = ACRTC_FIFO;
    return v;
}

/* 書込み FIFO に続けて書いてよい語の数：空（WFE）なら 8、空でなく空きがある（WFR）なら 1（FIFO は 8 語。
 * 台帳 HD63484.md）。CER なら打ち切って 0 */
static unsigned room(void)
{
    for (;;) {
        uint16_t sr = REG_ACRTC_SR;
        if (sr & ACRTC_SR_CER) {
            acrtc_abort();
            return 0;
        }
        if (sr & ACRTC_SR_WFE)
            return 8;
        if (sr & ACRTC_SR_WFR)
            return 1;
    }
}

int acrtc_put(uint16_t w)
{
    if (room() == 0)
        return 0;
    REG_ACRTC_DATA = w;
    return 1;
}

int acrtc_send(const uint16_t *words, unsigned count)
{
    unsigned n = 0;
    REG_ACRTC_AR = ACRTC_FIFO;
    while (count--) {
        if (n == 0 && (n = room()) == 0)
            return 0;
        REG_ACRTC_DATA = *words++;
        n--;
    }
    return 1;
}

int acrtc_get(uint16_t *out)
{
    for (;;) {
        uint16_t sr = REG_ACRTC_SR;
        if (sr & ACRTC_SR_CER) {
            acrtc_abort();
            return 0;
        }
        if (sr & ACRTC_SR_RFR)
            break;
    }
    *out = REG_ACRTC_DATA;
    return 1;
}

int acrtc_wait(void)
{
    /* FIFO にコマンドを書くと CED は 0 になる（U85 p.62）。FIFO が空で CED が立てば、送った分は終わった */
    for (;;) {
        uint16_t sr = REG_ACRTC_SR;
        if (sr & ACRTC_SR_CER) {
            acrtc_abort();
            return 0;
        }
        if ((sr & (ACRTC_SR_WFE | ACRTC_SR_CED)) == (ACRTC_SR_WFE | ACRTC_SR_CED))
            return 1;
    }
}

int acrtc_color(unsigned c)
{
    uint16_t v = (uint16_t)((c & 15) * 0x1111U);
    const uint16_t w[4] = {ACRTC_WPR | ACRTC_P_CL0, v, ACRTC_WPR | ACRTC_P_CL1, v};
    return acrtc_send(w, 4);
}

/* AMOVE で (x0, y0) に置き、cmd を 2 つのパラメタ（論理座標）で送る。論理座標の y は画面の y の符号を変えたもの */
static int two_points(uint16_t cmd, int x0, int y0, int x1, int y1)
{
    const uint16_t w[6] = {ACRTC_AMOVE, (uint16_t)x0, (uint16_t)-y0, cmd, (uint16_t)x1, (uint16_t)-y1};
    return acrtc_send(w, 6);
}

int acrtc_dot(int x, int y, unsigned op)
{
    const uint16_t w[4] = {ACRTC_AMOVE, (uint16_t)x, (uint16_t)-y, (uint16_t)(ACRTC_DOT | (op & 0xFF))};
    return acrtc_send(w, 4);
}

int acrtc_line(int x0, int y0, int x1, int y1, unsigned op)
{
    return two_points((uint16_t)(ACRTC_ALINE | (op & 0xFF)), x0, y0, x1, y1);
}

int acrtc_box(int x0, int y0, int x1, int y1, unsigned op)
{
    return two_points((uint16_t)(ACRTC_ARCT | (op & 0xFF)), x0, y0, x1, y1);
}

int acrtc_fill(int x0, int y0, int x1, int y1, unsigned op)
{
    return two_points((uint16_t)(ACRTC_AFRCT | (op & 0xFF)), x0, y0, x1, y1);
}

int acrtc_circle(int x, int y, unsigned r, unsigned op)
{
    const uint16_t w[5] = {ACRTC_AMOVE, (uint16_t)x, (uint16_t)-y, (uint16_t)(ACRTC_CRCL | (op & 0xFF)),
                           (uint16_t)r};
    return acrtc_send(w, 5);
}

int acrtc_dma(int on)
{
    if (!acrtc_wait())
        return 0;
    uint16_t ccr = acrtc_read_reg(ACRTC_CCR);
    ccr = on ? (uint16_t)(ccr | ACRTC_CCR_DDM) : (uint16_t)(ccr & ~ACRTC_CCR_DDM);
    acrtc_write_reg(ACRTC_CCR, ccr);
    return 1;
}

int acrtc_rwp(uint32_t addr)
{
    /* RWPH：DN（基本画面＝1）<< 14 と語番地の bit 19–12。RWPL：bit 11–0 << 4（Boot ROM の字の描画と同じ） */
    const uint16_t w[4] = {ACRTC_WPR | ACRTC_P_RWPH, (uint16_t)(0x4000U | ((addr >> 12) & 0xFFU)),
                           ACRTC_WPR | ACRTC_P_RWPL, (uint16_t)((addr & 0x0FFFU) << 4)};
    return acrtc_send(w, 4);
}

/* RWP を addr にして、cmd を AX＝words−1（右へ）、AY＝−(rows−1)（下へ）で送る */
static int rect(uint32_t addr, unsigned words, unsigned rows, uint16_t cmd)
{
    if (words == 0 || rows == 0 || !acrtc_rwp(addr))
        return 0;
    const uint16_t w[3] = {cmd, (uint16_t)(words - 1), (uint16_t)-(int)(rows - 1)};
    return acrtc_send(w, 3);
}

int acrtc_read_rect(uint32_t addr, unsigned words, unsigned rows)
{
    return rect(addr, words, rows, ACRTC_DRD);
}

int acrtc_write_rect(uint32_t addr, unsigned words, unsigned rows, uint16_t cmd)
{
    return rect(addr, words, rows, cmd);
}
