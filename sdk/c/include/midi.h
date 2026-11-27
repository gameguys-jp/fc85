/* midi.h：MIDI（HD6850 ACIA、31.25 kbit/s、8N1）を C から使う関数（DR-127。説明は sdk/docs/c.md §09）。
 *
 * 受信：BIOS が ACIA を受信の割込みつきで設定し（$95）、割込みの処理（ジャンプ表のベクタ 29）が受けた byte を
 * 受信のリング（bios_midi_buf、63 byte まで。bios-abi.md §03）に入れる。関数はリングの tail から読む。
 * リングが満杯の間に来た byte は捨てられ、bios_midi_lost に数える（midi_lost）。
 * 送信：ACIA の TDRE（送信データのレジスタが空き）を待って 1 byte ずつ書く。ACIA は送信データのレジスタと
 * シフトレジスタの 2 段なので、続けて 2 byte までは待たずに書け、その後は 1 byte に 320 µs かかる。
 * 割込みは使わないので、送っている間は関数から戻らない（3 byte のメッセージで最長約 0.64 ms）。
 *
 * メッセージの組立て（midi_parse）：ランニングステータスを開き、リアルタイム（$F8〜$FF）はほかのメッセージの
 * 途中にも入る。SysEx（$F0〜$F7）の中身は読み飛ばす。データだけでステータスがない byte は捨てる。
 */
#ifndef FC85_MIDI_H
#define FC85_MIDI_H

#include <stdint.h>

#include "bios.h"
#include "fc85.h"

/* メッセージ（ステータス、データ 0〜2 byte）。チャネルは status の下位 4 bit */
typedef struct {
    uint8_t status;
    uint8_t d1;
    uint8_t d2;
    uint8_t len; /* データの byte 数 */
} midi_msg_t;

/* 組立ての途中の様子。midi_parser_init で初めにする */
typedef struct {
    uint8_t status; /* ランニングステータス（0 はなし） */
    uint8_t got;    /* 今のメッセージで受けたデータの数 */
    uint8_t d1;
    uint8_t sysex;  /* SysEx の中 */
} midi_parser_t;

#define MIDI_NOTE_OFF   0x80U
#define MIDI_NOTE_ON    0x90U
#define MIDI_POLY_AT    0xA0U
#define MIDI_CC         0xB0U
#define MIDI_PROGRAM    0xC0U
#define MIDI_CHAN_AT    0xD0U
#define MIDI_PITCH_BEND 0xE0U
#define MIDI_CLOCK      0xF8U
#define MIDI_START      0xFAU
#define MIDI_CONTINUE   0xFBU
#define MIDI_STOP       0xFCU

/* 受信のリングにある byte の数 */
static inline unsigned midi_avail(void)
{
    return (unsigned)(bios_midi_head - bios_midi_tail) & 63U;
}

/* リングが満杯で捨てた byte の数（BIOS が数える） */
static inline uint16_t midi_lost(void)
{
    return bios_midi_lost;
}

/* 受信のリングから 1 byte。なければ -1 */
int midi_getc(void);
/* ACIA の送信データのレジスタが空いているか */
int midi_tx_ready(void);
/* 1 byte 送る（TDRE を待つ） */
void midi_putc(uint8_t b);
/* n byte 送る */
void midi_send(const uint8_t *p, unsigned n);
/* メッセージを送る（ステータスとデータ len byte。ランニングステータスは使わない） */
void midi_send_msg(const midi_msg_t *m);

void midi_parser_init(midi_parser_t *p);
/* 1 byte を足し、メッセージがそろえば *out に入れて 1。まだなら 0 */
int midi_parse(midi_parser_t *p, uint8_t b, midi_msg_t *out);
/* 受信のリングから読んで組み立てる。メッセージがそろえば 1、リングが空になれば 0 */
int midi_read(midi_parser_t *p, midi_msg_t *out);

/* ノートオンか（ベロシティ 0 のノートオンはノートオフとして扱う） */
static inline int midi_is_note_on(const midi_msg_t *m)
{
    return (m->status & 0xF0U) == MIDI_NOTE_ON && m->d2 != 0;
}

static inline int midi_is_note_off(const midi_msg_t *m)
{
    return (m->status & 0xF0U) == MIDI_NOTE_OFF || ((m->status & 0xF0U) == MIDI_NOTE_ON && m->d2 == 0);
}

#endif
