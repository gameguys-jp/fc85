/* SAMPLE 8：C で書いた Temporal RAM の残像のサンプル（DR-114。README.md）。build.py が GCC で組む（sdk/docs/c.md）。
 *
 * 左の矩形（64×32 画素）で玉が跳ねる。毎フレーム、その矩形を ACRTC の DRD と Data Fabric で Temporal RAM に記録し
 * （1 語を上位・下位の 2 byte に分けて 2 回。1 フレーム 1024 byte で、64 KiB に 64 フレーム）、右の 3 つの矩形に
 * d・2d・3d フレーム前の記録を DWT と DMOD の OR で描き戻す（acrtc.h の DMA、temporal.h、fabric.h）。
 * 上・下：d（1〜21）。A：HOLD（記録を止める。ポインタは進むので、残像は止める前の 64 フレームを繰り返す）。
 * START：d を戻し、HOLD を解く。起動の直後、まだ記録していない番地からは、Temporal RAM の電源投入時の中身が出る。
 *
 * DDM（DMA）の間は BIOS の字の呼出しを使わない（acrtc.h）。字は DDM を下ろしてから書く。
 */
#include <stdint.h>

#include "acrtc.h"
#include "bios.h"
#include "fabric.h"
#include "fc85.h"
#include "temporal.h"

FC85_CART("SAMPLE 8        ");

enum { COL_BG = 0, COL_DIM = 11, COL_TEXT = 15 };
enum {
    T_W = 64,                   /* 矩形の幅（画素） */
    T_H = 32,                   /* 高さ（8 回の転送が 1 フレームに収まる） */
    T_Y = 56,                   /* 画面の y */
    T_WORDS = T_W / 4 * T_H,    /* 1 回の転送の語数（512） */
    T_BYTES = 2 * T_WORDS,      /* 1 フレームの記録（byte） */
    COPIES = 3,
    DELAY0 = 8,
    DELAY_MAX = 21,             /* 3×21×1024 は 64 KiB に入る */
    BALL = 8,
};

/* Main RAM（試験がラベルで読む） */
uint16_t v_delay = DELAY0;
uint16_t v_hold;
uint16_t v_tbase;               /* 今のフレームを記録する Temporal RAM の番地 */
uint16_t v_busy;                /* 1 フレームの仕事の途中 */
uint16_t v_errors;              /* Fabric の ERROR と ACRTC の打切り */
uint32_t v_frame;               /* 仕事を終えたフレームの数 */
int16_t v_bx = 10, v_by = 6, v_vx = 2, v_vy = 1;

static void check(int ok)
{
    if (!ok)
        v_errors++;
}

static void transfer(uint8_t src, uint8_t dst, uint16_t format, uint16_t shift)
{
    struct fab_xfer x = {.src = src, .dst = dst, .count = T_WORDS, .format = format, .shift = shift,
                         .trigger = TRG_NOW};
    check(fab_run(0, &x) == FAB_ST_DONE);
}

/* 左の矩形を Temporal RAM の v_tbase から記録する：上位 byte の 768 個、続けて下位 byte の 768 個 */
static void capture(void)
{
    tmp_seek(v_tbase, tmp_rptr());
    check(acrtc_read_rect(ACRTC_WORD_ADDR(0, T_Y), T_W / 4, T_H));
    transfer(PORT_ACRTC, PORT_TEMPORAL_W, FMT_16_8_HI, 0);
    check(acrtc_read_rect(ACRTC_WORD_ADDR(0, T_Y), T_W / 4, T_H));
    transfer(PORT_ACRTC, PORT_TEMPORAL_W, FMT_16_8_LO, 0);
}

/* k×d フレーム前の記録を、右の k 番目の矩形に：上位 byte を DWT で（下位は 0）、下位 byte を DMOD の OR で */
static void play(unsigned k)
{
    tmp_seek(tmp_wptr(), (uint16_t)(v_tbase - k * v_delay * T_BYTES));
    uint32_t at = ACRTC_WORD_ADDR(T_W * k, T_Y);
    check(acrtc_write_rect(at, T_W / 4, T_H, ACRTC_DWT));
    transfer(PORT_TEMPORAL_R, PORT_ACRTC, FMT_8_16, 8);
    check(acrtc_write_rect(at, T_W / 4, T_H, ACRTC_DMOD | ACRTC_OPM_OR));
    transfer(PORT_TEMPORAL_R, PORT_ACRTC, FMT_8_16, 0);
}

static int16_t bounce(int16_t p, int16_t *v, int16_t max)
{
    int16_t n = (int16_t)(p + *v);
    if (n < 0 || n > max) {
        *v = (int16_t)-*v;
        n = (int16_t)(p + *v);
    }
    return n;
}

/* 左の矩形：消して、色の変わる玉を描く */
static void draw_live(void)
{
    v_bx = bounce(v_bx, &v_vx, T_W - BALL);
    v_by = bounce(v_by, &v_vy, T_H - BALL);
    check(acrtc_color(COL_BG));
    check(acrtc_fill(0, T_Y, T_W - 1, T_Y + T_H - 1, ACRTC_OPM_REP));
    check(acrtc_color(1 + (unsigned)(v_frame >> 3) % 14));
    check(acrtc_fill(v_bx, T_Y + v_by, v_bx + BALL - 1, T_Y + v_by + BALL - 1, ACRTC_OPM_REP));
}

static void show_state(void)
{
    bios_put_hex(80, 128, v_delay, 2, COL_TEXT, COL_BG);
    bios_puts(80, 136, v_hold ? "ON " : "OFF", COL_TEXT, COL_BG);
}

/* DIST（WPTR−RPTR。描き戻しの後は 3d フレーム分）と誤りの数。変わったときだけ描く（BIOS の字は 1 フレームの
 * 仕事に収まらないことがある） */
static void show_numbers(void)
{
    static uint16_t shown_dist = 0xFFFF, shown_errors = 0xFFFF;
    uint16_t d = tmp_dist();
    if (d != shown_dist) {
        shown_dist = d;
        bios_put_hex(80, 144, d, 4, COL_TEXT, COL_BG);
    }
    if (v_errors != shown_errors) {
        shown_errors = v_errors;
        bios_put_hex(80, 152, v_errors, 4, COL_TEXT, COL_BG);
    }
}

static void handle_pad(uint16_t pressed)
{
    if (pressed & PAD_START) {
        v_delay = DELAY0;
        v_hold = 0;
    }
    if ((pressed & PAD_UP) && v_delay < DELAY_MAX)
        v_delay++;
    if ((pressed & PAD_DOWN) && v_delay > 1)
        v_delay--;
    if (pressed & PAD_A)
        v_hold ^= 1;
    tmp_mode(v_hold ? TMP_HOLD : 0);
}

int main(void)
{
    bios_cls(COL_BG);
    bios_puts(8, 8, "SAMPLE 8  TEMPORAL ECHO IN C", COL_TEXT, COL_BG);
    bios_puts(8, 96, "NOW", COL_DIM, COL_BG);
    bios_puts(72, 96, "D", COL_DIM, COL_BG);
    bios_puts(136, 96, "2D", COL_DIM, COL_BG);
    bios_puts(200, 96, "3D", COL_DIM, COL_BG);
    bios_puts(8, 128, "DELAY", COL_DIM, COL_BG);
    bios_puts(8, 136, "A HOLD", COL_DIM, COL_BG);
    bios_puts(8, 144, "DIST", COL_DIM, COL_BG);
    bios_puts(8, 152, "ERRORS", COL_DIM, COL_BG);
    tmp_reset(0);
    show_state();
    for (;;) {
        bios_wait_vbl();
        v_busy = 1;
        uint16_t pressed;
        bios_read_pad(&pressed);
        if (pressed) {
            handle_pad(pressed);
            show_state();
        }
        draw_live();
        check(acrtc_dma(1));
        capture();
        for (unsigned k = 1; k <= COPIES; k++)
            play(k);
        check(acrtc_dma(0));
        v_tbase = (uint16_t)(v_tbase + T_BYTES);
        v_frame++;
        show_numbers();
        v_busy = 0;
    }
}
