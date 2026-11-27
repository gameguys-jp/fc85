/* midiparse.c：MIDI のメッセージの組立て（DR-127。約束は include/midi.h）。I/O には触れない（受信は midi.c の
 * midi_getc）。 */
#include "midi.h"

void midi_parser_init(midi_parser_t *p)
{
    p->status = 0;
    p->got = 0;
    p->d1 = 0;
    p->sysex = 0;
}

/* ステータスのデータの byte 数 */
static uint8_t data_len(uint8_t s)
{
    switch (s & 0xF0U) {
    case MIDI_PROGRAM:
    case MIDI_CHAN_AT:
        return 1;
    case 0xF0U:
        return s == 0xF2U ? 2 : 1; /* $F1 と $F3 は 1 */
    default:
        return 2;
    }
}

/* データのないメッセージ（リアルタイム、$F6） */
static int single(uint8_t b, midi_msg_t *out)
{
    out->status = b;
    out->d1 = 0;
    out->d2 = 0;
    out->len = 0;
    return 1;
}

int midi_parse(midi_parser_t *p, uint8_t b, midi_msg_t *out)
{
    if (b >= 0xF8U) {
        /* リアルタイム：ほかのメッセージの途中にも入り、組立ては続ける */
        return single(b, out);
    }
    if (b & 0x80U) {
        p->got = 0;
        p->sysex = (b == 0xF0U);
        if (b >= 0xF0U) {
            /* システムコモンはランニングステータスを消す */
            p->status = (b >= 0xF1U && b <= 0xF3U) ? b : 0;
            return b == 0xF6U ? single(b, out) : 0;
        }
        p->status = b;
        return 0;
    }
    if (p->sysex || p->status == 0) {
        return 0;
    }
    uint8_t need = data_len(p->status);
    if (p->got == 0) {
        p->d1 = b;
        p->got = 1;
        if (need > 1) {
            return 0;
        }
        out->d2 = 0;
    } else {
        out->d2 = b;
    }
    out->status = p->status;
    out->d1 = p->d1;
    out->len = need;
    p->got = 0;
    if (p->status >= 0xF0U) {
        p->status = 0;
    }
    return 1;
}

int midi_read(midi_parser_t *p, midi_msg_t *out)
{
    for (;;) {
        int c = midi_getc();
        if (c < 0) {
            return 0;
        }
        if (midi_parse(p, (uint8_t)c, out)) {
            return 1;
        }
    }
}
