/* video.h：パレットと映像の割込み（HBlank）を C から使う関数（DR-109。説明は sdk/docs/c.md §09）。
 *
 * パレット（PALETTE）は裏の 16 語で、表示は HSYNC の始まりに表へ写した値を使う（2CLK に 1 語ずつ。DR-027、
 * DR-069）。書いた色は、写しが通った次のラインから効く。ラインの途中で色を替える表現（ラスタごとの色）は、
 * HBlank の割込みで裏のパレットを書く。
 *
 * HBlank の割込みはレベル 6 で、VBlank と同じ線（BIOS の処理がある）。video_hblank() は、ジャンプ表の
 * ベクタ 30 に小さな処理（sdk/c/video_irq.S）を置き、HBL の保留を自分で消して hook を呼び、BIOS の処理へ
 * つなぐ（bios-abi.md の「処理を差し替える」）。hook は割込みの中（マスク 6）で、ふつうの C の関数として
 * 呼ばれる。短くする。
 *
 * ラスタ L の HBlank の割込みで書いた色は、ラスタ L＋2 から効く：HBlank の始まりから次のラインの写しまでは
 * 22 ドット（約 4 µs）で、68000 の割込みの応答が間に合わない（エミュレータで確かめた [V]。test_devices.py）。
 * 色を替えたいラスタの 2 つ前で比べる。
 *
 * ラスタの番号は VSYNC の始まりから（VIDEO_LINE・VIDEO_LINE_CMP と同じ）。BIOS の案B の画面では、画面の
 * y＝0 はラスタ VIDEO_TOP_RASTER（43）。
 */
#ifndef FC85_VIDEO_H
#define FC85_VIDEO_H

#include <stdint.h>

#include "fc85.h"

/* BIOS の案B の画面の最初の表示のラスタ（VDR の VSW＋VDS。ACRTC_B_VDR から） */
#define VIDEO_TOP_RASTER ((ACRTC_B_VDR & 0x1FU) + (ACRTC_B_VDR >> 8) + 1U)
/* 1 フレームのラスタの数（VSR の VC。ACRTC_B_VSR から） */
#define VIDEO_RASTERS    (ACRTC_B_VSR & 0x0FFFU)

typedef void (*video_hook_t)(void);

/* パレットの色 n（0〜15）を $0RGB にする。次の写しから効く */
static inline void video_set_color(unsigned n, uint16_t rgb)
{
    REG_PALETTE(n & 15U) = rgb;
}

/* パレットの first から count 語を書く */
void video_set_palette(const uint16_t *rgb, unsigned first, unsigned count);

/* PAL_CTRL の DIRECT（Bend。DR-027）。1 にすると、書いた色は裏を通らずに表へ直接入り、ラインの途中から効く。
 * 書込みのストローブの間は、書いた色が点状に出る（ノイズ）。DIRECT の間は写しが止まり、書いた色は裏に入らない
 * ので、0 に戻すと次のラインから裏の色に戻る */
static inline void video_direct(int on)
{
    REG_PAL_CTRL = on ? 1U : 0U;
}

/* 今のラスタ（VSYNC の始まりから） */
static inline unsigned video_line(void)
{
    return REG_VIDEO_LINE;
}

/* HBlank の割込みを始める。line が 0 以上ならそのラスタの HBlank だけ（LINE_CMP）、負なら毎ライン。hook は
 * 割込みのたびに呼ぶ（0 なら保留を消すだけ）。VBlank の割込み（BIOS）はそのまま */
void video_hblank(video_hook_t hook, int line);
/* 比べるラスタを替える（hook の中で次の区切りを決めるのに使う） */
static inline void video_hblank_at(unsigned line)
{
    REG_VIDEO_LINE_CMP = (uint16_t)(line & 0x0FFFU);
}
/* HBlank の割込みを止め、ベクタ 30 を前の処理に戻す */
void video_hblank_off(void);

#endif
