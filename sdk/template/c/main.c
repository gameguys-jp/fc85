/* ひな形のカートリッジ（C）。build.py が GCC で組む（sdk/docs/c.md）。作り方と試し方は AGENTS.md。
 *
 * 画面に題、フレームの数、PORT_A の値を出し、方向キーで四角（「*」と呼ぶ）を 8 画素ずつ動かす。START で
 * 「*」を真ん中に戻す。文字 ROM の記号の字形はまだそろっていない（描いていない字は市松模様になる）ので、
 * 文字列には英字・数字・空白を使う。ここから書き足して自分の ROM にする。ACRTC の図形・Data Fabric・DOC・ADPCM・
 * パレットと HBlank の割込み・DSP・Temporal RAM を C から使う関数は acrtc.h・fabric.h・doc.h・adpcm.h・video.h・
 * dsp.h・temporal.h（sdk/docs/c.md §09）。音・映像・DSP の使い方の手本は sdk/samples/ の各サンプル（README.md）。C で書いた
 * ものは sdk/samples/raster（HBlank のラスタのパレット）、ほかはアセンブラ。
 *
 * 番地とビットの名前は fc85.h、BIOS は bios.h（sdk/docs/bios-abi.md）、ROM が守ることは sdk/docs/guarantees.md。
 */
#include <stdint.h>

#include "bios.h"
#include "fc85.h"

FC85_CART("TEMPLATE        "); /* 題（16 字。new_rom.py が書き換える） */

/* 色はパレットの番号（BIOS の既定のパレット。bios-abi.md §06） */
enum { COL_BG = 0, COL_DIM = 11, COL_TEXT = 15 };

enum {
    CELL = 8, /* 文字は 8×8 画素。bios_puts の x は 4 の倍数 */
    FIELD_X0 = 0, /* 「*」が動く範囲（両端を含む） */
    FIELD_Y0 = 40,
    FIELD_X1 = 248,
    FIELD_Y1 = 184,
    HOME_X = 128,
    HOME_Y = 112,
};

static const char s_title[] = "TEMPLATE"; /* 画面の題（new_rom.py が書き換える） */

/* Main RAM（試験がラベルで読む） */
uint32_t v_frame; /* 入口からのフレームの数 */
int16_t v_x, v_y; /* 「*」の位置 */

/* 「*」は空白の字を color の背景で描いた 8×8 の四角（文字 ROM にまだない記号を使わない） */
static void put_at_cursor(unsigned color)
{
    bios_puts(v_x, v_y, " ", COL_BG, color);
}

static void show_numbers(uint16_t pad)
{
    bios_put_hex(56, 24, v_frame, 8, COL_TEXT, COL_BG);
    bios_put_hex(168, 24, pad, 4, COL_TEXT, COL_BG);
}

static void move_cursor(uint16_t pad, uint16_t pressed)
{
    int x = v_x, y = v_y;
    if (pressed & PAD_START) {
        x = HOME_X;
        y = HOME_Y;
    } else {
        if ((pad & PAD_LEFT) && x > FIELD_X0)
            x -= CELL;
        if ((pad & PAD_RIGHT) && x < FIELD_X1)
            x += CELL;
        if ((pad & PAD_UP) && y > FIELD_Y0)
            y -= CELL;
        if ((pad & PAD_DOWN) && y < FIELD_Y1)
            y += CELL;
    }
    if (x == v_x && y == v_y)
        return;
    put_at_cursor(COL_BG); /* 前の位置を背景の色で消す */
    v_x = x;
    v_y = y;
    put_at_cursor(COL_TEXT);
}

int main(void)
{
    v_x = HOME_X;
    v_y = HOME_Y;
    bios_cls(COL_BG);
    bios_puts(8, 8, s_title, COL_TEXT, COL_BG);
    bios_puts(8, 24, "FRAME", COL_DIM, COL_BG);
    bios_puts(136, 24, "PAD", COL_DIM, COL_BG);
    put_at_cursor(COL_TEXT);
    for (;;) {
        bios_wait_vbl(); /* 描画は VBlank の直後にまとめる */
        v_frame++;
        uint16_t pressed;
        uint16_t pad = bios_read_pad(&pressed);
        show_numbers(pad);
        move_cursor(pad, pressed);
    }
}
