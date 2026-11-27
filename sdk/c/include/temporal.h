/* temporal.h：Temporal RAM（64 KiB の SRAM。Data Fabric の port 6 で書き、port 7 で読む）のポインタと制御を C から
 * 使う関数（DR-113。説明は sdk/docs/c.md §09）。
 *
 * 書込みのポインタ WPTR と読出しのポインタ RPTR は byte 番地で、Fabric が 1 要素（8 bit）を運ぶたびに 1 進み、
 * 64 KiB で回る。遅延は WPTR−RPTR（DIST）で、読み書きの速さの差は Fabric のチャネルの速さで作る（DR-067 (5)）。
 * 追越しは止めず、印だけを立てる：書込みで WPTR＝RPTR になったら OVERRUN（読んでいない値を上書きする）、
 * 読出しで一致したら UNDERRUN（前の周の値を読む）。書いていない番地は、電源投入時の中身（決まらない）を読む。
 *
 * CTRL（TEMPORAL_CTRL）の書込みは、モード（SYNC・RSYNC・HOLD）を毎回決め直し、CLEAR は 1 回の動作、OVERRUN・
 * UNDERRUN は 1 を書いて消す（W1C）。関数は値を作ってから 1 回で書き、読み書き一体の命令を使わない。
 */
#ifndef FC85_TEMPORAL_H
#define FC85_TEMPORAL_H

#include <stdint.h>

#include "fc85.h"

/* モードのビット（tmp_reset・tmp_mode に渡す） */
#define TMP_MODES (TMP_SYNC | TMP_RSYNC | TMP_HOLD)

/* 両方のポインタと印を 0 にし、モードを mode（TMP_SYNC・TMP_RSYNC・TMP_HOLD の OR）にする */
void tmp_reset(uint16_t mode);
/* モードだけを mode にする（ポインタと印はそのまま） */
void tmp_mode(uint16_t mode);
/* 今のモード */
uint16_t tmp_get_mode(void);
/* ポインタを置く */
void tmp_seek(uint16_t wptr, uint16_t rptr);
/* 印（TMP_EQUAL・TMP_OVERRUN・TMP_UNDERRUN） */
uint16_t tmp_flags(void);
/* OVERRUN・UNDERRUN を消す（モードはそのまま） */
void tmp_clear_flags(void);

static inline uint16_t tmp_wptr(void)
{
    return REG_TEMPORAL_WPTR;
}

static inline uint16_t tmp_rptr(void)
{
    return REG_TEMPORAL_RPTR;
}

/* WPTR−RPTR（遅延の byte 数） */
static inline uint16_t tmp_dist(void)
{
    return REG_TEMPORAL_DIST;
}

#endif
