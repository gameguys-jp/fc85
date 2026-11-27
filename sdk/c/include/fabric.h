/* fabric.h：Data Fabric を C から使う関数（DR-106。説明は sdk/docs/c.md §09）。
 *
 * 守ること（sdk/docs/guarantees.md §03 の 21〜27）のうち、手順で守れるものは関数の中で守る：レジスタは語の
 * アクセスだけ、設定のレジスタはチャネルを止めてから書く、START の前に DONE・ERROR・MISS を消す、TRIGGER を
 * 書いてから START まで 4 スロット以上あける（fab_set は TRIGGER を最初に書き、その後に設定のレジスタを 10 個、
 * fab_start が STATUS を書いてから CTRL を書く。語の書込みは続けても 1 回に 1 スロットより長くかかる（MOVE.L の
 * 2 回で 3 スロット。仕様書 §06）。実装されていないポート（FABRIC_MODULES）や FORMAT と合わない向き・
 * 幅を使わないのは呼ぶ側が守る（守らなければ START で ERROR になる）。
 *
 * チャネルは 0 と 1。Fabric は Staging RAM（port 0）を読めるが、カートリッジは読めない。カートリッジの
 * データは、先に CPU が Staging RAM に写す（fab_to_staging）。
 */
#ifndef FC85_FABRIC_H
#define FC85_FABRIC_H

#include <stdint.h>

/* 転送の設定。0 で埋めた形は「変換しない、即時、ブロック」 */
struct fab_xfer {
    uint8_t src, dst;            /* ポート（PORT_*） */
    uint16_t src_addr, dst_addr; /* 番地を持つポートの番地（port 0 は Staging RAM の byte 番地、port 2 は DOC の
                                    波形 RAM の byte 番地）。番地を持たないポートは 0 */
    uint16_t count;              /* 要素の数 */
    int16_t src_step, dst_step;  /* 1 要素ごとに番地に足す数（番地を持たないポートは 0） */
    uint16_t format;             /* FMT_* */
    uint16_t trigger;            /* TRG_* の発生源と TRG_EACH・TRG_LOOP・TRG_IRQ_EN */
    uint16_t clear;              /* 消すビット（MASK に ~clear を書く） */
    uint16_t flip;               /* 反転するビット（XOR） */
    uint16_t shift;              /* FMT_16_ROT の回転の数 */
};

/* STOP を書き、BUSY が下りるのを待ち、DONE・ERROR・MISS を消す */
void fab_stop(unsigned ch);
/* チャネルを止めてから設定のレジスタを書く（TRIGGER を最初に） */
void fab_set(unsigned ch, const struct fab_xfer *x);
/* DONE・ERROR・MISS を消して START */
void fab_start(unsigned ch);
/* STATUS（FAB_ST_*） */
unsigned fab_status(unsigned ch);
/* DONE か ERROR が立つまで待ち、STATUS を返す。トリガが来なければ戻らない */
unsigned fab_wait(unsigned ch);
/* fab_set・fab_start・fab_wait をして、DONE・ERROR・MISS を消し、待った STATUS を返す（TRG_LOOP には使わない） */
unsigned fab_run(unsigned ch, const struct fab_xfer *x);
/* CPU が src から n byte を Staging RAM の byte 番地 addr に写す（memcpy） */
void fab_to_staging(uint16_t addr, const void *src, unsigned n);

#endif
