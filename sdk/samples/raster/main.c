/* SAMPLE 7：C で書いたラスタのパレットのサンプル（DR-111。README.md）。build.py が GCC で組む（sdk/docs/c.md）。
 *
 * 背景の色 0 を HBlank の割込み（video.h）で 8 ラスタごとに替え、32 色の虹の階調を毎フレームずらす。
 * A：PAL_CTRL の DIRECT（Bend）を切り替える。色が表へ直接入るので、帯の境がラスタの途中に移る。
 * B：DOC の三角波（子基板があるとき）を鳴らす・止める。音の高さは帯の流れについて回る。
 * 上・下：流れる速さ（0〜15）。START：速さを戻し、DIRECT と音を切る。
 *
 * 色はパレットの番号（BIOS の既定のパレット。bios-abi.md §06）。字は英字・数字・空白だけ（文字 ROM の記号の
 * 字形はまだそろっていない）。
 */
#include <stdint.h>

#include "bios.h"
#include "doc.h"
#include "fabric.h"
#include "fc85.h"
#include "video.h"

FC85_CART("SAMPLE 7        ");

enum { COL_BG = 0, COL_DIM = 11, COL_TEXT = 15 };
enum {
    BAND = 8,               /* 帯の高さ（ラスタ） */
    BANDS = 192 / BAND,     /* 画面の帯の数 */
    STEPS = 32,             /* 階調の色の数 */
    SPEED0 = 4,             /* 1 フレームに進む 1/16 帯 */
    STAGE_WAVE = 0x4000,    /* 三角波を置く Staging RAM の byte 番地 */
    TONE_OSC = 0,           /* 鳴らすオシレータ（CA 0、L） */
};

/* 帯 k（1〜BANDS−1）の色は、画面の y＝8k から。HBlank で書いた色は、比べたラスタの 2 つ後から効く（video.h） */
#define BAND_RASTER(k) (VIDEO_TOP_RASTER + BAND * (unsigned)(k) - 2U)

/* Main RAM（試験がラベルで読む） */
uint16_t v_grad[STEPS];     /* 虹の階調（$0RGB） */
volatile uint16_t v_band;   /* 次に色を替える帯 */
volatile uint16_t v_phase;  /* 階調のずれ（1/16 帯） */
volatile uint16_t v_hblanks;
uint16_t v_speed = SPEED0;
uint16_t v_direct, v_tone, v_doc;
uint32_t v_frame;

static uint8_t s_wave[256];

/* 0〜15〜0 の三角（32 で 1 周） */
static unsigned tri(unsigned i)
{
    i &= STEPS - 1;
    return i < 16 ? i : 31 - i;
}

/* R・G・B を 1/3 周ずつずらした虹。隣どうしの色はいつも違う（3 つが同時に頂か底で止まらない） */
static void make_gradient(void)
{
    for (unsigned i = 0; i < STEPS; i++)
        v_grad[i] = (uint16_t)((tri(i) << 8) | (tri(i + 11) << 4) | tri(i + 21));
}

static uint16_t color_of(unsigned band)
{
    return v_grad[((v_phase >> 4) + band) & (STEPS - 1)];
}

/* VBlank（BIOS のフック）：最初の帯の色と、最初の区切り */
static void top(void)
{
    v_phase += v_speed;
    v_band = 1;
    video_set_color(0, color_of(0));
    video_hblank_at(BAND_RASTER(1));
}

/* HBlank（BAND_RASTER(k)）：帯 k の色にして、次の区切りへ。最後の帯の後は VBlank が戻す */
static void band(void)
{
    unsigned k = v_band;
    v_hblanks++;
    video_set_color(0, color_of(k));
    if (k + 1 < BANDS) {
        v_band = (uint16_t)(k + 1);
        video_hblank_at(BAND_RASTER(k + 1));
    }
}

static void sound_init(void)
{
    v_doc = (uint16_t)doc_present();
    if (!v_doc)
        return;
    for (unsigned i = 0; i < 256; i++) /* $01〜$FF（0 はオシレータを止める印） */
        s_wave[i] = (uint8_t)(i < 128 ? 2 * i + 1 : 255 - 2 * (i - 128));
    fab_to_staging(STAGE_WAVE, s_wave, sizeof s_wave);
    doc_load_wave(0, 0x0000, STAGE_WAVE, sizeof s_wave);
    struct doc_osc o = {.freq = 0x0400, .volume = 160, .pointer = 0, .size = 0,
                        .control = DOC_CTL_HALT | DOC_CTL_FREE | (TONE_OSC << DOC_CTL_CA_SH)};
    doc_set(TONE_OSC, &o);
}

/* 音の高さ：帯の流れの位置で 1 オクターブを上り下りする */
static void sound_frame(void)
{
    if (v_doc && v_tone)
        doc_freq(TONE_OSC, (uint16_t)(0x0300 + 0x0030 * tri(v_phase >> 4)));
}

static void show_state(void)
{
    bios_put_hex(88, 160, v_speed, 1, COL_TEXT, COL_BG);
    bios_puts(88, 168, v_direct ? "ON " : "OFF", COL_TEXT, COL_BG);
    bios_puts(88, 176, v_doc ? (v_tone ? "ON " : "OFF") : "NO DOC", COL_TEXT, COL_BG);
}

static void set_tone(unsigned on)
{
    v_tone = (uint16_t)on;
    if (!v_doc)
        return;
    if (on)
        doc_go(TONE_OSC);
    else
        doc_halt(TONE_OSC);
}

static void handle_pad(uint16_t pressed)
{
    if (pressed & PAD_START) {
        v_speed = SPEED0;
        v_direct = 0;
        video_direct(0);
        set_tone(0);
    }
    if ((pressed & PAD_UP) && v_speed < 15)
        v_speed++;
    if ((pressed & PAD_DOWN) && v_speed > 0)
        v_speed--;
    if (pressed & PAD_A) {
        v_direct ^= 1;
        video_direct(v_direct);
    }
    if (pressed & PAD_B)
        set_tone(!v_tone);
}

int main(void)
{
    make_gradient();
    bios_cls(COL_BG);
    bios_puts(8, 8, "SAMPLE 7  RASTER PALETTE IN C", COL_TEXT, COL_BG);
    bios_puts(8, 160, "SPEED", COL_DIM, COL_BG);
    bios_puts(8, 168, "A DIRECT", COL_DIM, COL_BG);
    bios_puts(8, 176, "B TONE", COL_DIM, COL_BG);
    sound_init();
    show_state();
    bios_vbl_hook = top;
    video_hblank(band, (int)BAND_RASTER(1));
    for (;;) {
        bios_wait_vbl();
        v_frame++;
        uint16_t pressed;
        bios_read_pad(&pressed);
        if (pressed) {
            handle_pad(pressed);
            show_state();
        }
        sound_frame();
    }
}
