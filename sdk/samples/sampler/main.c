/* SAMPLE 10：Audio IN を DOC の A/D で録って弾くサンプラー（DR-131。README.md）。build.py が GCC で組む
 * （sdk/docs/c.md）。
 *
 * 背面の Audio IN（DR-129）の音を、DOC（子基板）の A/D で録り（doc_adc_record。ラスタ 1 本ごとに 1 標本で
 * 15.7 kHz）、波形 RAM の 16 KiB のテーブル（頁 0）に置いて、MIDI IN のノートとパッドで 8 声で弾く。
 * 録る周波数は 15.7・7.85・3.93 kHz（1.04・2.09・4.17 秒）。A/D の前の LPF は 7.9 kHz で固定なので、低い周波数で
 * 録ると折返しが残る（本機の録った音の色。DR-129）。DOC は 32 オシレータを走査し（1 MHz÷34＝29.4 kHz）、補間
 * せずに鳴らす。ノート 60 が録った高さ。声ごとに DOC の出力のチャンネル（CA）を 1 つずつ使う（偶数 L、奇数 R）。
 * 鳴らし方はワンショット（テーブルの終わりで止まる）か LOOP（テーブル全体をくり返し、離すと音量を下げて止める）。
 * 録る前のテーブルは、68000 が作った減衰するのこぎり波（C4）。
 * 録る間は割込みを止めるので、画面・パッド・MIDI の受信は止まる（そのあいだの MIDI の byte は失う）。
 * MIDI：ノートオン・オフ、ピッチベンド（±2 半音）、CC 120・123（全部止める）。チャンネルは問わない。
 * A：弾く（左・右で半音ずつ）。B：録る。C：録る周波数。上：LOOP。下：TRIG（しきい値を待つか、すぐ録るか。
 * 待つのは 5 秒まで）。START：全部止める。
 *
 * 字は英字・数字・空白だけ（文字 ROM の記号の字形はまだそろっていない）。シャープは S。
 */
#include <stdint.h>

#include "acrtc.h"
#include "bios.h"
#include "doc.h"
#include "fabric.h"
#include "fc85.h"
#include "midi.h"
#include "video.h"

FC85_CART("SAMPLE 10       ");

enum { COL_BG = 0, COL_RED = 3, COL_CYAN = 7, COL_DIM = 11, COL_TEXT = 15 };
enum { ST_READY, ST_NO_INPUT, ST_NO_DOC };
enum { OFF, HELD, RELEASED };
enum {
    VOICES = 8,
    OSCS = 32,                  /* 走査するオシレータ（29.4 kHz） */
    CA_IDLE = 15,               /* 鳴らさないオシレータのチャンネル（bios-abi.md §06） */
    REC_BYTES = 16384,          /* テーブル（T＝6）と録る標本の数 */
    STAGE_REC = 0x8000,         /* 録った音を置く Staging RAM の byte 番地 */
    TRIG_LEVEL = 12,            /* しきい値（|値−128|） */
    ROOT_NOTE = 60,             /* 録った高さで鳴るノート */
    BEND_RANGE = 2 * 64,        /* ピッチベンドの幅（1/64 半音） */
    RELEASE = 24,               /* LOOP で離した後、1 フレームに下げる音量 */
    PAD_CHAN = 16,              /* パッドで弾いた音（MIDI のチャンネル 0〜15 と分ける） */
    KEY_RANGE = 24,             /* パッドの音程の幅（半音） */
};
#define WAIT_LINES (5UL * DOC_ADC_LINE_HZ)    /* しきい値を待つ長さ（5 秒） */
#define SIZE_16K 0x37U                       /* T＝6（16 KiB）、R＝7 */
/* 録った高さの FC×256。1 標本で FC÷2^(9＋R−T)＝FC÷2^10 byte 進む（台帳 ES5503.md §4.3）。ラスタの周波数
 * 5.3693175 MHz÷342＝15,699.76 Hz の標本を、29,411.76 Hz の走査で 1 つずつ進めるには FC＝546.60 */
#define FC_ROOT_Q8 139930UL

/* 2^(k/12) の Q12（k＝0〜12） */
static const uint16_t s_ratio[13] = {4096, 4340, 4598, 4871, 5161, 5468, 5793, 6137, 6502, 6889, 7298, 7732, 8192};
static const char *const s_names[12] = {"C ", "CS", "D ", "DS", "E ", "F ", "FS", "G ", "GS", "A ", "AS", "B "};
static const char *const s_rates[3] = {"15700", "7850 ", "3925 "};
static const char *const s_status[3] = {"READY   ", "NO INPUT", "NO DOC  "};

struct voice {
    uint8_t note;
    uint8_t chan;
    uint8_t vel;
    uint8_t state;      /* OFF、HELD、RELEASED */
    uint8_t vol;        /* 書いた音量 */
    uint8_t shown;      /* 画面の印（1 は点いている） */
    uint16_t fc;        /* 書いた FC */
    uint16_t age;
};

/* Main RAM（試験がラベルで読む） */
struct voice v_voices[VOICES];
uint32_t v_frame;
uint16_t v_doc, v_rate, v_rec_rate, v_loop, v_trig = 1, v_len, v_recs, v_status, v_meter, v_in, v_age;
int16_t v_key, v_bend;
uint8_t v_rec[REC_BYTES];       /* 録った音（波形 RAM の頁 0 と同じ） */

static midi_parser_t s_parser;
static uint16_t s_pad;
static uint8_t s_pad_note = 0xFF;
static uint8_t s_dirty = 1, s_wave_dirty = 1, s_rec_req;

/* ------------------------------------------------------------------ 音程 */

/* 1/64 半音の音程（0 が録った高さ）→ FC。隣の半音の間は直線で埋める */
static uint16_t fc_of_pitch(int32_t p)
{
    int oct = 0;
    while (p < 0) {
        p += 12 * 64;
        oct--;
    }
    while (p >= 12 * 64) {
        p -= 12 * 64;
        oct++;
    }
    unsigned k = (unsigned)p >> 6, frac = (unsigned)p & 63;
    uint32_t r = s_ratio[k] + (((uint32_t)(s_ratio[k + 1] - s_ratio[k]) * frac) >> 6);
    int sh = 20 + (int)v_rec_rate - oct;            /* Q8 と Q12、録った周波数の 1/2^rate */
    if (sh > 31)
        return 1;
    uint32_t fc = (FC_ROOT_Q8 * r + (1UL << (sh - 1))) >> sh;
    if (fc < 1)
        fc = 1;
    if (fc > 0xFFFF)
        fc = 0xFFFF;
    return (uint16_t)fc;
}

static int32_t pitch_of(const struct voice *s)
{
    return ((int32_t)s->note - ROOT_NOTE) * 64 + v_bend;
}

/* ------------------------------------------------------------------ DOC */

/* ラスタの変わり目を n 回待つ（ラスタ 1 本は約 63.7 µs） */
static void wait_lines(unsigned n)
{
    unsigned line = video_line();
    while (n--) {
        unsigned now;
        while ((now = video_line()) == line) {
        }
        line = now;
    }
}

/* 止めて、鳴らさないチャンネルへ（制御の 1 回の書込み） */
static void osc_idle(unsigned osc)
{
    REG_DOC_CONTROL(osc) = (uint8_t)(DOC_CTL_HALT | (CA_IDLE << DOC_CTL_CA_SH));
}

/* 声 v はオシレータ v、CA v。テーブルの頭から鳴らす：H＝1・M0＝1 を書くと、止まった後の 2 回目の訪問で
 * アキュムレータが 0 に戻る（台帳 ES5503.md §4.2・§5）。訪問は 34 µs ごとなので、ラスタ 3 本（127 µs 以上）
 * 待ってから H を下ろす */
static void voice_start(unsigned v)
{
    struct voice *s = &v_voices[v];
    s->fc = fc_of_pitch(pitch_of(s));
    s->vol = (uint8_t)(s->vel * 2U);
    if (!v_doc)
        return;
    struct doc_osc o = {.freq = s->fc, .volume = s->vol, .pointer = 0x00, .size = SIZE_16K,
                        .control = (uint8_t)(DOC_CTL_HALT | DOC_CTL_ONESHOT | (v << DOC_CTL_CA_SH))};
    doc_set(v, &o);
    wait_lines(3);
    REG_DOC_CONTROL(v) = (uint8_t)((v_loop ? DOC_CTL_FREE : DOC_CTL_ONESHOT) | (v << DOC_CTL_CA_SH));
}

static void voice_stop(unsigned v)
{
    v_voices[v].state = OFF;
    if (v_doc)
        osc_idle(v);
}

/* 1 フレーム：ワンショットの終わり、LOOP の離した後の音量、ピッチベンド */
static void voice_frame(unsigned v)
{
    struct voice *s = &v_voices[v];
    if (s->state == OFF)
        return;
    if (!v_doc) {
        if (s->state == RELEASED)
            s->state = OFF;
        return;
    }
    if (REG_DOC_CONTROL(v) & DOC_CTL_HALT) {        /* ワンショットがテーブルの終わりで止まった */
        voice_stop(v);
        return;
    }
    if (s->state == RELEASED) {
        if (s->vol <= RELEASE) {
            voice_stop(v);
            return;
        }
        s->vol = (uint8_t)(s->vol - RELEASE);
        doc_volume(v, s->vol);
    }
    uint16_t fc = fc_of_pitch(pitch_of(s));
    if (fc != s->fc) {
        s->fc = fc;
        doc_freq(v, fc);
    }
}

/* ------------------------------------------------------------------ 声の割当て */

static void note_on(uint8_t chan, uint8_t note, uint8_t vel)
{
    unsigned pick = 0;
    int best = -1;
    /* 空き → 離した音のうち古いもの → いちばん古いもの */
    for (unsigned v = 0; v < VOICES; v++) {
        const struct voice *s = &v_voices[v];
        int score = s->state == OFF ? 0x30000 : s->state == RELEASED ? 0x20000 : 0x10000;
        score += (uint16_t)(v_age - s->age);
        if (score > best) {
            best = score;
            pick = v;
        }
    }
    struct voice *s = &v_voices[pick];
    s->note = note;
    s->chan = chan;
    s->vel = vel;
    s->state = HELD;
    s->age = ++v_age;
    voice_start(pick);
}

/* ワンショットは離しても終わりまで鳴らす。LOOP は音量を下げて止める */
static void note_off(uint8_t chan, uint8_t note)
{
    for (unsigned v = 0; v < VOICES; v++) {
        struct voice *s = &v_voices[v];
        if (s->state == HELD && s->note == note && s->chan == chan)
            s->state = RELEASED;
    }
}

static void all_off(void)
{
    for (unsigned v = 0; v < VOICES; v++)
        if (v_voices[v].state != OFF)
            voice_stop(v);
    s_pad_note = 0xFF;
}

/* ------------------------------------------------------------------ MIDI */

static void midi_message(const midi_msg_t *m)
{
    if (m->status >= MIDI_CLOCK)
        return;                                     /* リアルタイム */
    v_in++;
    uint8_t ch = m->status & 0x0F;
    if (midi_is_note_on(m)) {
        note_on(ch, m->d1, m->d2);
        return;
    }
    if (midi_is_note_off(m)) {
        note_off(ch, m->d1);
        return;
    }
    switch (m->status & 0xF0) {
    case MIDI_CC:
        if (m->d1 == 120 || m->d1 == 123)
            all_off();
        break;
    case MIDI_PITCH_BEND: {
        int16_t b = (int16_t)(((uint16_t)m->d2 << 7 | m->d1) - 8192);
        v_bend = (int16_t)(b / (8192 / BEND_RANGE));
        break;
    }
    default:
        break;
    }
}

/* ------------------------------------------------------------------ 録る */

static void load_table(void)
{
    fab_to_staging(STAGE_REC, v_rec, REC_BYTES);
    doc_load_wave(0, 0x0000, STAGE_REC, REC_BYTES);
}

/* 録る前のテーブル：減衰するのこぎり波（15.7 kHz で 60 標本の周期＝261.7 Hz、C4）。標本 i は
 * 128＋((ph×4−118)×env)÷256（ph は周期の中の位置、env は 256−i÷64）。68000 の掛け算は遅いので、64 標本の
 * 塊ごとに 1 回だけ掛け、塊の中は足し算で進める */
static void make_saw(void)
{
    unsigned ph = 0;
    for (unsigned b = 0; b < REC_BYTES / 64; b++) {
        int32_t env = 256 - (int32_t)b;
        int32_t step = 4 * env;
        int32_t acc = ((int32_t)ph * 4 - 118) * env;
        for (unsigned j = 0; j < 64; j++) {
            int32_t v = 128 + (acc >> 8);
            v_rec[b * 64 + j] = (uint8_t)(v < 1 ? 1 : v);
            acc += step;
            if (++ph == 60) {
                ph = 0;
                acc -= 60 * step;
            }
        }
    }
    v_len = REC_BYTES;
    v_rec_rate = 0;
}

static void do_record(void)
{
    all_off();
    if (!v_doc)
        return;
    bios_puts(48, 56, v_trig ? "WAIT    " : "REC     ", COL_RED, COL_BG);
    unsigned got = doc_adc_record(v_rec, REC_BYTES, 1U << v_rate, v_trig ? TRIG_LEVEL : 0, WAIT_LINES);
    midi_parser_init(&s_parser);                   /* 止めていた間の byte は失っている */
    s_dirty = 1;
    if (got == 0) {
        v_status = ST_NO_INPUT;
        load_table();                               /* 待つ間に v_rec は変えていないが、念のため波形 RAM とそろえる */
        return;
    }
    for (unsigned i = got; i < REC_BYTES; i++)
        v_rec[i] = 128;
    load_table();
    v_len = (uint16_t)got;
    v_rec_rate = v_rate;
    v_recs++;
    v_status = ST_READY;
    s_wave_dirty = 1;
}

/* ------------------------------------------------------------------ パッド */

static uint8_t pad_note(void)
{
    return (uint8_t)(ROOT_NOTE + v_key);
}

static void pad_note_off(void)
{
    if (s_pad_note == 0xFF)
        return;
    note_off(PAD_CHAN, s_pad_note);
    s_pad_note = 0xFF;
}

static void handle_pad(uint16_t now, uint16_t pressed)
{
    uint16_t released = (uint16_t)(s_pad & ~now);
    s_pad = now;
    if (pressed & PAD_START)
        all_off();
    if ((pressed & PAD_LEFT) && v_key > -KEY_RANGE)
        v_key--;
    if ((pressed & PAD_RIGHT) && v_key < KEY_RANGE)
        v_key++;
    if (pressed & PAD_UP)
        v_loop ^= 1;
    if (pressed & PAD_DOWN)
        v_trig ^= 1;
    if (pressed & PAD_C)
        v_rate = (uint16_t)(v_rate == 2 ? 0 : v_rate + 1);
    if (pressed & PAD_B)
        s_rec_req = 1;
    if (released & PAD_A)
        pad_note_off();
    /* 押したまま左右に動いたら弾き直す */
    if ((pressed & PAD_A) || ((now & PAD_A) && (pressed & (PAD_LEFT | PAD_RIGHT)) && s_pad_note != pad_note())) {
        pad_note_off();
        s_pad_note = pad_note();
        note_on(PAD_CHAN, s_pad_note, 110);
    }
    if (pressed)
        s_dirty = 1;
}

/* ------------------------------------------------------------------ 画面 */

enum { WAVE_TOP = 72, WAVE_H = 64, METER_X = 32, METER_Y = 41, VOICE_Y = 144 };

/* 録った音：横 256 ドットに 64 標本ずつの最小と最大 */
static void draw_wave(void)
{
    acrtc_color(COL_BG);
    acrtc_fill(0, WAVE_TOP, 255, WAVE_TOP + WAVE_H - 1, 0);
    acrtc_color(COL_DIM);
    acrtc_fill(0, WAVE_TOP + 31, 255, WAVE_TOP + 31, 0);
    acrtc_color(COL_CYAN);
    for (unsigned x = 0; x < 256; x++) {
        const uint8_t *p = &v_rec[x * 64U];
        uint8_t lo = 255, hi = 0;
        for (unsigned j = 0; j < 64; j++) {
            lo = p[j] < lo ? p[j] : lo;
            hi = p[j] > hi ? p[j] : hi;
        }
        acrtc_fill(x, WAVE_TOP + ((255U - hi) >> 2), x, WAVE_TOP + ((255U - lo) >> 2), 0);
    }
}

static void draw_meter(void)
{
    static uint16_t shown = 0xFFFF;
    if (shown == v_meter)
        return;
    shown = v_meter;
    if (v_meter) {
        acrtc_color(COL_CYAN);
        acrtc_fill(METER_X, METER_Y, METER_X + v_meter - 1, METER_Y + 4, 0);
    }
    acrtc_color(COL_BG);
    acrtc_fill(METER_X + v_meter, METER_Y, METER_X + 128, METER_Y + 4, 0);
}

static void draw_voices(void)
{
    for (unsigned v = 0; v < VOICES; v++) {
        uint8_t on = v_voices[v].state != OFF;
        if (on == v_voices[v].shown)
            continue;
        v_voices[v].shown = on;
        acrtc_color(on ? COL_RED : COL_DIM);
        acrtc_fill(8 + 16 * v, VOICE_Y, 17 + 16 * v, VOICE_Y + 5, 0);
    }
}

static void draw_labels(void)
{
    bios_puts(8, 8, "SAMPLE 10  AUDIO IN SAMPLER", COL_TEXT, COL_BG);
    bios_puts(8, 24, "RATE", COL_DIM, COL_BG);
    bios_puts(120, 24, "LOOP", COL_DIM, COL_BG);
    bios_puts(192, 24, "TRIG", COL_DIM, COL_BG);
    bios_puts(8, 40, "IN", COL_DIM, COL_BG);
    bios_puts(176, 40, "KEY", COL_DIM, COL_BG);
    bios_puts(8, 56, "STAT", COL_DIM, COL_BG);
    bios_puts(136, 56, "LEN", COL_DIM, COL_BG);
    bios_puts(160, 144, "MIDI", COL_DIM, COL_BG);
    for (unsigned v = 0; v < VOICES; v++)
        v_voices[v].shown = 1;                      /* 印を描かせる */
    bios_puts(8, 168, "A PLAY B REC C RATE", COL_DIM, COL_BG);
    bios_puts(8, 176, "UP LOOP DOWN TRIG", COL_DIM, COL_BG);
}

static void draw_state(void)
{
    static uint16_t in = 0xFFFF;
    draw_meter();
    draw_voices();
    if (in != v_in) {
        in = v_in;
        bios_put_hex(200, 144, in, 4, COL_TEXT, COL_BG);
    }
    if (s_wave_dirty) {
        s_wave_dirty = 0;
        draw_wave();
    }
    if (!s_dirty)
        return;
    s_dirty = 0;
    bios_puts(48, 24, s_rates[v_rate], COL_TEXT, COL_BG);
    bios_puts(160, 24, v_loop ? "ON " : "OFF", COL_TEXT, COL_BG);
    bios_puts(232, 24, v_trig ? "ON " : "OFF", COL_TEXT, COL_BG);
    unsigned n = pad_note(), o = 0;
    while (n >= 12) {
        n -= 12;
        o++;
    }
    bios_puts(208, 40, s_names[n], COL_TEXT, COL_BG);
    bios_put_hex(224, 40, o - 1, 1, COL_TEXT, COL_BG);
    bios_puts(48, 56, s_status[v_status], v_status == ST_READY ? COL_TEXT : COL_RED, COL_BG);
    bios_put_hex(168, 56, v_len, 4, COL_TEXT, COL_BG);
}

/* ------------------------------------------------------------------ 起動 */

int main(void)
{
    bios_cls(COL_BG);
    v_doc = (uint16_t)doc_present();
    make_saw();
    if (v_doc) {
        doc_set_oscillators(OSCS);
        for (unsigned o = 0; o < OSCS; o++)
            osc_idle(o);
        load_table();
    } else {
        v_status = ST_NO_DOC;
    }
    draw_labels();
    midi_parser_init(&s_parser);
    /* 起動の前に来た byte は捨てる */
    while (midi_getc() >= 0) {
    }
    for (;;) {
        bios_wait_vbl();
        v_frame++;
        midi_msg_t m;
        while (midi_read(&s_parser, &m))
            midi_message(&m);
        uint16_t pressed = 0;                       /* BIOS は新しく押したボタンがないと書かない */
        uint16_t now = bios_read_pad(&pressed);
        handle_pad(now, pressed);
        if (s_rec_req) {
            s_rec_req = 0;
            do_record();
        }
        for (unsigned v = 0; v < VOICES; v++)
            voice_frame(v);
        if (v_doc) {
            /* 入力の大きさ：1 フレームに 1 回 A/D を読み（前のフレームに始めた変換）、山を少しずつ下げる */
            uint8_t a = doc_adc();
            uint16_t d = a >= 128 ? a - 128U : 128U - a;
            v_meter = d > v_meter ? d : v_meter > 2 ? (uint16_t)(v_meter - 2) : 0;
        }
        draw_state();
    }
}
