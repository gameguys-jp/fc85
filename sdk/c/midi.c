/* midi.c：MIDI の受信のリングと ACIA の送信（DR-127。約束は include/midi.h）。メッセージの組立ては
 * midiparse.c（I/O に触れない）。 */
#include "fc85.h"
#include "midi.h"

int midi_getc(void)
{
    uint16_t t = bios_midi_tail;
    if (bios_midi_head == t) {
        return -1;
    }
    uint8_t b = bios_midi_buf[t];
    /* tail を書くのはアプリだけ（割込みの処理は head だけを書く） */
    bios_midi_tail = (uint16_t)((t + 1U) & 63U);
    return b;
}

int midi_tx_ready(void)
{
    uint8_t s = REG_MIDI_STATUS;
    return (s & ACIA_ST_TDRE) != 0;
}

void midi_putc(uint8_t b)
{
    while (!midi_tx_ready()) {
    }
    REG_MIDI_DATA = b;
}

void midi_send(const uint8_t *p, unsigned n)
{
    while (n-- > 0) {
        midi_putc(*p++);
    }
}

void midi_send_msg(const midi_msg_t *m)
{
    midi_putc(m->status);
    if (m->len > 0) {
        midi_putc(m->d1);
    }
    if (m->len > 1) {
        midi_putc(m->d2);
    }
}
