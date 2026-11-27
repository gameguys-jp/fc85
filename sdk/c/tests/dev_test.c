/* 周辺の関数（acrtc.h・fabric.h・doc.h・adpcm.h。DR-106。video.h・dsp.h。DR-109）の試験の ROM。sdk/c/test_devices.py
 * が組み、エミュレータで動かして、結果（r_*）、VRAM、Staging RAM、DOC の様子、音、画面を確かめる。
 *
 * 順：ACRTC の図形と打切り → Fabric の写し → ADPCM（v_phase 1→2）→ DOC（v_phase 3）→ 40 フレーム待つ →
 * HBlank の割込みで色 0 を替える（v_phase 4。その後 10 フレームはそのまま）→ 同じことを DIRECT で（v_phase 5）→
 * DSP（v_phase 6）→ ACRTC の DMA（v_phase 7）。試験は v_phase を見張って区切り、ADPCM の間と DOC の間の音を
 * 分けて測り、v_phase 4・5 で画面を撮る。
 *
 * echo_dsp.h は試験が tests/echo.dsp から dspasm --c-array で作る。
 */
#include <stdint.h>

#include "acrtc.h"
#include "adpcm.h"
#include "bios.h"
#include "doc.h"
#include "dsp.h"
#include "echo_dsp.h"
#include "fabric.h"
#include "fc85.h"
#include "video.h"

FC85_CART("C DEVICE TEST   ");

volatile uint16_t v_phase;
volatile uint16_t done;
uint16_t r_acrtc[16];
uint16_t r_fab[4];
uint16_t r_adpcm[6];
uint16_t r_doc[8];
uint16_t r_video[6];
uint16_t r_dsp[8];
uint16_t r_dma[8];

/* ラスタの区切り（画面の y＝60 と 120 の辺りのラスタで比べる） */
#define SPLIT1 (VIDEO_TOP_RASTER + 60U)
#define SPLIT2 (VIDEO_TOP_RASTER + 120U)
static volatile uint16_t s_hblanks;

static const char s_text[16] = "FABRIC COPY 0123";
static const uint16_t s_words[8] = {0x1234, 0xABCD, 0x0F0F, 0xF0F0, 0x0000, 0xFFFF, 0x8001, 0x7FFE};
static uint8_t s_wave[256];
static uint8_t s_adpcm[1024];

enum {
    STAGE_TEXT = 0x2000, /* Staging RAM の byte 番地 */
    STAGE_COPY = 0x2100,
    STAGE_WORDS = 0x2200,
    STAGE_XOR = 0x2300,
    STAGE_DMA = 0x2400,
    STAGE_WAVE = 0x3000,
    STAGE_ADPCM = 0x4000,
};

static void acrtc_tests(void)
{
    bios_cls(0);
    r_acrtc[0] = acrtc_color(5);
    r_acrtc[1] = acrtc_fill(8, 8, 23, 15, ACRTC_OPM_REP);
    r_acrtc[2] = acrtc_color(9);
    r_acrtc[3] = acrtc_line(0, 40, 16, 40, 0);
    r_acrtc[4] = acrtc_dot(101, 50, 0);
    r_acrtc[5] = acrtc_box(32, 64, 47, 71, 0);
    r_acrtc[6] = acrtc_circle(128, 128, 10, 0);
    r_acrtc[7] = acrtc_wait();
    /* パラメタを読み返す：RPR で CL0 を読出し FIFO に */
    r_acrtc[8] = acrtc_put(ACRTC_RPR | ACRTC_P_CL0);
    r_acrtc[9] = acrtc_get(&r_acrtc[10]);
    r_acrtc[11] = acrtc_read_reg(ACRTC_CCR);
    /* 未定義のコマンド：CER で打ち切って 0。その後も描ける */
    acrtc_put(0x0000);
    r_acrtc[12] = acrtc_wait();
    r_acrtc[13] = acrtc_color(3);
    r_acrtc[14] = acrtc_fill(200, 100, 203, 100, 0) && acrtc_wait();
    r_acrtc[15] = acrtc_read_reg(ACRTC_CCR);
}

static void fabric_tests(void)
{
    fab_to_staging(STAGE_TEXT, s_text, sizeof s_text);
    struct fab_xfer x = {
        .src = PORT_STAGING,
        .dst = PORT_STAGING,
        .src_addr = STAGE_TEXT,
        .dst_addr = STAGE_COPY,
        .count = sizeof s_text,
        .src_step = 1,
        .dst_step = 1,
        .format = FMT_8_8,
        .trigger = TRG_NOW,
    };
    r_fab[0] = fab_run(0, &x);
    /* 16 bit の語で、上位 4 bit を消し、下位 8 bit を反転する */
    fab_to_staging(STAGE_WORDS, s_words, sizeof s_words);
    x.src_addr = STAGE_WORDS;
    x.dst_addr = STAGE_XOR;
    x.count = 8;
    x.src_step = 2;
    x.dst_step = 2;
    x.format = FMT_16_16;
    x.clear = 0xF000;
    x.flip = 0x00FF;
    r_fab[1] = fab_run(1, &x);
    r_fab[2] = fab_status(0);
    r_fab[3] = fab_status(1);
}

/* 鳴っている間のフレームを数える。止まったら戻る（上限 limit） */
static uint16_t play_frames(unsigned limit)
{
    uint16_t busy = 0;
    for (unsigned f = 0; f < limit; f++) {
        bios_wait_vbl();
        adpcm_update();
        if (!adpcm_busy())
            break;
        busy++;
    }
    return busy;
}

static void adpcm_tests(void)
{
    /* 0x77（大きく上へ）と 0xFF（大きく下へ）の 64 byte ずつ：約 62 Hz の矩形に近い波 */
    for (unsigned i = 0; i < sizeof s_adpcm; i++)
        s_adpcm[i] = (i & 64) ? 0xFF : 0x77;
    fab_to_staging(STAGE_ADPCM, s_adpcm, sizeof s_adpcm);
    adpcm_init(0);
    v_phase = 1;
    /* 1024 byte＝2048 ニブル、8 kHz で 0.256 秒（約 15 フレーム） */
    adpcm_play(STAGE_ADPCM, sizeof s_adpcm, ADPCM_CTL_8K);
    r_adpcm[0] = play_frames(120);
    /* 鳴っている途中で替える（FLUSH を通る）*/
    adpcm_play(STAGE_ADPCM, sizeof s_adpcm, ADPCM_CTL_8K);
    for (unsigned f = 0; f < 6; f++) {
        bios_wait_vbl();
        adpcm_update();
    }
    adpcm_play(STAGE_ADPCM, 256, ADPCM_CTL_4K);
    r_adpcm[1] = play_frames(120);
    /* 止める */
    adpcm_play(STAGE_ADPCM, sizeof s_adpcm, ADPCM_CTL_6K);
    for (unsigned f = 0; f < 6; f++) {
        bios_wait_vbl();
        adpcm_update();
    }
    adpcm_stop();
    r_adpcm[2] = play_frames(120);
    r_adpcm[3] = fab_status(0);
    r_adpcm[4] = REG_ADPCM_STATUS & ADPCM_ST_COUNT;
    r_adpcm[5] = adpcm_busy();
    v_phase = 2;
}

static void doc_tests(void)
{
    r_doc[0] = doc_present();
    if (!r_doc[0])
        return;
    /* 三角波 $01〜$FF（0 はオシレータを止める印なので使わない） */
    for (unsigned i = 0; i < 256; i++)
        s_wave[i] = (uint8_t)(i < 128 ? 2 * i + 1 : 255 - 2 * (i - 128));
    fab_to_staging(STAGE_WAVE, s_wave, sizeof s_wave);
    r_doc[1] = doc_load_wave(1, 0x0000, STAGE_WAVE, sizeof s_wave);
    r_doc[2] = doc_oscillators();
    doc_set_oscillators(20);
    r_doc[3] = doc_oscillators();
    struct doc_osc o = {.freq = 0x0400, .volume = 200, .pointer = 0, .size = 0, .control = 0};
    doc_set(0, &o);
    o.control = 1U << DOC_CTL_CA_SH;
    doc_set(1, &o);
    doc_halt(1);
    r_doc[4] = REG_DOC_CONTROL(1);
    doc_go(1);
    r_doc[7] = REG_DOC_CONTROL(1);
    doc_freq(0, 0x0500);
    doc_volume(0, 180);
    r_doc[5] = REG_DOC_FREQ_HI(0);
    r_doc[6] = REG_DOC_CONTROL(16);
    v_phase = 3;
}

/* VBlank：色 0 を青に。HBlank（SPLIT1）：赤にして次は SPLIT2、（SPLIT2）：緑にして次は SPLIT1 */
static void vbl_hook(void)
{
    video_set_color(0, 0x000F);
}

static void hblank_hook(void)
{
    s_hblanks++;
    if (video_line() < SPLIT2) {
        video_set_color(0, 0x0F00);
        video_hblank_at(SPLIT2);
    } else {
        video_set_color(0, 0x00F0);
        video_hblank_at(SPLIT1);
    }
}

static void wait_frames(unsigned n)
{
    while (n--)
        bios_wait_vbl();
}

static void video_tests(void)
{
    static const uint16_t pal[3] = {0x0123, 0x0456, 0x0789};
    video_set_palette(pal, 13, 3);
    const volatile bios_hook_t *slot = (const volatile bios_hook_t *)(0x100000UL + 6UL * 30U + 2UL);
    bios_hook_t bios_handler = *slot;
    bios_vbl_hook = vbl_hook;
    video_hblank(hblank_hook, (int)SPLIT1);
    r_video[0] = REG_VIDEO_IRQ & 7U;
    r_video[1] = *slot != bios_handler;
    uint32_t f0 = bios_frame;
    wait_frames(4);
    r_video[2] = s_hblanks;
    r_video[3] = (uint16_t)(bios_frame - f0);
    v_phase = 4;
    wait_frames(10);
    video_hblank_off();
    bios_vbl_hook = 0;
    r_video[4] = REG_VIDEO_IRQ & 7U;
    r_video[5] = *slot == bios_handler;

    /* 同じ区切りを DIRECT（Bend）で。色はラスタの途中から変わる */
    video_set_color(0, 0x0000);
    video_direct(1);
    bios_vbl_hook = vbl_hook;
    video_hblank(hblank_hook, (int)SPLIT1);
    wait_frames(4);
    v_phase = 5;
    wait_frames(10);
    video_hblank_off();
    bios_vbl_hook = 0;
    video_direct(0);
}

static void dsp_tests(void)
{
    r_dsp[0] = (uint16_t)dsp_load(echo_dsp, echo_dsp_WORDS, 1);
    dsp_start(DSP_CTRL_BIO_INPUT);
    r_dsp[1] = (uint16_t)dsp_command(0x1234, &r_dsp[2], 1000);
    r_dsp[3] = (uint16_t)dsp_command(0xFFFF, &r_dsp[4], 1000);
    r_dsp[5] = dsp_status() & (1U << DSP_ST_RESPONSE_B);
    dsp_reset();
    r_dsp[6] = (uint16_t)dsp_command(0x0001, &r_dsp[7], 200);
    v_phase = 6;
}

/* ACRTC の DMA（DR-112）：色 1〜4 の 4 画素ずつの帯（x＝32〜47、y＝120〜123）を DRD で Staging RAM に読み、
 * 下（x＝64〜79、y＝130〜133）に DMOD の EOR で書く。下地は 0 なので同じ絵になる */
static void dma_tests(void)
{
    for (unsigned i = 0; i < 4; i++) {
        acrtc_color(i + 1);
        acrtc_fill((int)(32 + 4 * i), 120, (int)(35 + 4 * i), 123, ACRTC_OPM_REP);
    }
    r_dma[0] = (uint16_t)acrtc_dma(1);
    r_dma[1] = (uint16_t)acrtc_read_rect(ACRTC_WORD_ADDR(32, 120), 4, 4);
    struct fab_xfer x = {
        .src = PORT_ACRTC,
        .dst = PORT_STAGING,
        .dst_addr = STAGE_DMA,
        .count = 16,
        .dst_step = 2,
        .format = FMT_16_16,
        .trigger = TRG_NOW,
    };
    r_dma[2] = (uint16_t)fab_run(0, &x);
    r_dma[3] = (uint16_t)acrtc_write_rect(ACRTC_WORD_ADDR(64, 130), 4, 4, ACRTC_DMOD | ACRTC_OPM_EOR);
    struct fab_xfer y = {
        .src = PORT_STAGING,
        .dst = PORT_ACRTC,
        .src_addr = STAGE_DMA,
        .count = 16,
        .src_step = 2,
        .format = FMT_16_16,
        .trigger = TRG_NOW,
    };
    r_dma[4] = (uint16_t)fab_run(0, &y);
    r_dma[5] = (uint16_t)(acrtc_read_reg(ACRTC_CCR) & ACRTC_CCR_DDM);
    r_dma[6] = (uint16_t)acrtc_dma(0);
    r_dma[7] = (uint16_t)(acrtc_read_reg(ACRTC_CCR) & ACRTC_CCR_DDM);
    v_phase = 7;
}

int main(void)
{
    acrtc_tests();
    fabric_tests();
    adpcm_tests();
    doc_tests();
    wait_frames(40);
    video_tests();
    dsp_tests();
    dma_tests();
    done = 0xC0DE;
    return 0;
}
