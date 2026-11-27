/* 周辺の関数（acrtc.h・fabric.h・doc.h・adpcm.h。DR-106）の試験の ROM。sdk/c/test_devices.py が組み、エミュレータで
 * 動かして、結果（r_*）、VRAM、Staging RAM、DOC の様子、音を確かめる。
 *
 * 順：ACRTC の図形と打切り → Fabric の写し → ADPCM（v_phase 1→2）→ DOC（v_phase 3）。試験は v_phase を見張って
 * 区切り、ADPCM の間と DOC の間の音を分けて測る。
 */
#include <stdint.h>

#include "acrtc.h"
#include "adpcm.h"
#include "bios.h"
#include "doc.h"
#include "fabric.h"
#include "fc85.h"

FC85_CART("C DEVICE TEST   ");

volatile uint16_t v_phase;
volatile uint16_t done;
uint16_t r_acrtc[16];
uint16_t r_fab[4];
uint16_t r_adpcm[6];
uint16_t r_doc[8];

static const char s_text[16] = "FABRIC COPY 0123";
static const uint16_t s_words[8] = {0x1234, 0xABCD, 0x0F0F, 0xF0F0, 0x0000, 0xFFFF, 0x8001, 0x7FFE};
static uint8_t s_wave[256];
static uint8_t s_adpcm[1024];

enum {
    STAGE_TEXT = 0x2000, /* Staging RAM の byte 番地 */
    STAGE_COPY = 0x2100,
    STAGE_WORDS = 0x2200,
    STAGE_XOR = 0x2300,
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

int main(void)
{
    acrtc_tests();
    fabric_tests();
    adpcm_tests();
    doc_tests();
    done = 0xC0DE;
    return 0;
}
