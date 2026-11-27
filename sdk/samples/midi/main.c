/* SAMPLE 9：MIDI で弾く DOC のシンセ（DR-128。README.md）。build.py が GCC で組む（sdk/docs/c.md）。
 *
 * MIDI IN のノートを DOC（子基板）の 8 声で鳴らす。声ごとに DOC の出力のチャンネル（CA）を 1 つずつ使い
 * （偶数 L、奇数 R）、鳴らさないオシレータは CA 15 に置く。音色は 5 つ：三角・のこぎり・矩形（声に 1 つの
 * オシレータ）、SYNC（偶数番を同期のモードにして、上の奇数番を頭に戻す。DOC のハードの同期）、AM（奇数番を
 * AM のモードにして、上の偶数番の音量を波形で変える。7 声）。音量の包絡とビブラートは 68000 が 1 フレームに
 * 1 回書く。
 * MIDI：ノートオン・オフ、ピッチベンド（±2 半音）、CC 1（モジュレーション：ビブラート、SYNC と AM では上の
 * オシレータの音程）、CC 120・123（全部止める）、プログラムチェンジ（音色）。チャンネルは問わない。
 * THRU を入れると、受けたメッセージを MIDI OUT に送り直す。パッドで弾いた音も MIDI OUT にチャンネル 1 で送る。
 * 左・右：鍵盤の位置。A：弾く。上・下：オクターブ。B：THRU。C：次の音色。START：全部止める。
 *
 * 字は英字・数字・空白だけ（文字 ROM の記号の字形はまだそろっていない）。シャープは S（CS4 は C♯4）。
 */
#include <stdint.h>

#include "acrtc.h"
#include "bios.h"
#include "doc.h"
#include "fabric.h"
#include "fc85.h"
#include "midi.h"

FC85_CART("SAMPLE 9        ");

enum { COL_BG = 0, COL_RED = 3, COL_CYAN = 7, COL_DIM = 11, COL_TEXT = 15 };
enum { PROG_TRI, PROG_SAW, PROG_SQUARE, PROG_SYNC, PROG_AM, PROGS };
enum { STAGE_OFF, STAGE_ATTACK, STAGE_DECAY, STAGE_RELEASE };
enum {
    VOICES = 8,
    AM_VOICES = 7,              /* AM は (2v+1, 2v+2) の組なので 7 声 */
    PAD_CHAN = 16,              /* パッドで弾いた音（MIDI のチャンネル 0〜15 と分ける） */
    KEYS = 36,                  /* 画面の鍵盤（3 オクターブ） */
    WAVE_BYTES = 1024,          /* 波形のテーブル（T＝R＝2） */
    STAGE_WAVE = 0x4000,        /* 波形を置く Staging RAM の byte 番地 */
    CA_IDLE = 15,               /* 鳴らさないオシレータのチャンネル（bios-abi.md §06） */
    BEND_RANGE = 2 * 64,        /* ピッチベンドの幅（1/64 半音） */
    SWEEP0 = 18 * 64,           /* SYNC の掃引の始まり（1/64 半音） */
};
#define SIZE_1K 0x12U           /* T＝2（1024 byte）、R＝2 */

/* 波形 RAM の頁（1024 byte のテーブルは 4 頁ごと） */
enum { PAGE_TRI = 0x00, PAGE_SAW = 0x04, PAGE_SQUARE = 0x08 };

struct prog {
    uint8_t page;       /* 鳴らすオシレータの波形 */
    uint8_t aux_page;   /* SYNC の親・AM の変調の波形 */
    uint8_t attack;     /* 1 フレームに上げる量（包絡の上位 byte） */
    uint8_t decay;
    uint8_t sustain;
    uint8_t release;
};

static const struct prog s_progs[PROGS] = {
    [PROG_TRI] = {PAGE_TRI, 0, 255, 2, 200, 6},
    [PROG_SAW] = {PAGE_SAW, 0, 255, 3, 150, 8},
    [PROG_SQUARE] = {PAGE_SQUARE, 0, 64, 4, 220, 24},
    [PROG_SYNC] = {PAGE_SAW, PAGE_TRI, 255, 2, 180, 8},
    [PROG_AM] = {PAGE_SAW, PAGE_TRI, 255, 0, 255, 255},
};
static const char *const s_prog_names[PROGS] = {"TRI   ", "SAW   ", "SQUARE", "SYNC  ", "AM    "};
static const char *const s_names[12] = {"C ", "CS", "D ", "DS", "E ", "F ", "FS", "G ", "GS", "A ", "AS", "B "};

/* 音程 → FC。ノート 120〜131 の FC（8 MHz、走査 16、1 MHz÷18＝55.6 kHz、T＝R＝2 で f＝SR×FC÷2^19）。
 * 下のオクターブは右へずらす。ノート 69（440 Hz）は 132876÷32＝4152 */
static const uint32_t s_fc_top[12] = {79008, 83706, 88684, 93957, 99544, 105463,
                                      111735, 118379, 125418, 132876, 140777, 149148};

struct voice {
    uint8_t note;
    uint8_t chan;
    uint8_t vel;
    uint8_t stage;
    uint16_t level;     /* 包絡（8.8） */
    uint16_t sweep;     /* SYNC の掃引（1/64 半音） */
    uint16_t age;
    uint16_t fc;        /* 書いた FC（鳴らすオシレータ） */
    uint16_t fc_aux;    /* 書いた FC（親・変調） */
    uint8_t vol;        /* 書いた音量 */
    uint8_t shown;      /* 画面に出した印（ノート＋1、0 は空き） */
};

/* Main RAM（試験がラベルで読む） */
struct voice v_voices[VOICES];
uint32_t v_frame;
uint16_t v_prog, v_thru, v_mod, v_oct = 3, v_cursor = 12, v_doc;
int16_t v_bend;
uint16_t v_in, v_out, v_last[3];
uint16_t v_age;

static uint8_t s_wave[WAVE_BYTES];
static uint8_t s_lit[KEYS];
static uint8_t s_lfo;
static uint16_t s_pad;
static uint8_t s_pad_note = 0xFF;
static uint8_t s_dirty = 1;

/* ------------------------------------------------------------------ 音程 */

static uint32_t fc_of_note(int n)
{
    if (n < 0)
        n = 0;
    if (n > 131)
        n = 131;
    unsigned o = 0;
    while (n >= 12) {
        n -= 12;
        o++;
    }
    unsigned sh = 10 - o;
    uint32_t b = s_fc_top[n];
    return sh ? (b + (1UL << (sh - 1))) >> sh : b;
}

/* 1/64 半音の音程 → FC（隣の半音の間を直線で埋める） */
static uint16_t fc_of_pitch(int32_t p)
{
    if (p < 0)
        p = 0;
    int n = (int)(p >> 6);
    uint16_t frac = (uint16_t)(p & 63);
    uint32_t a = fc_of_note(n);
    uint32_t b = fc_of_note(n + 1);
    uint32_t fc = a + (((uint32_t)(uint16_t)(b - a) * frac) >> 6);
    if (fc < 1)
        fc = 1;
    if (fc > 0xFFFF)
        fc = 0xFFFF;
    return (uint16_t)fc;
}

/* ------------------------------------------------------------------ DOC */

/* 声 v が使うオシレータ（鳴らすもの・親か変調）。AM は (2v+2, 2v+1)、ほかは (2v, 2v+1)。
 * SYNC では偶数番（2v）が親で、鳴るのは奇数番（2v+1） */
static unsigned osc_main(unsigned v)
{
    if (v_prog == PROG_AM)
        return 2 * v + 2;
    if (v_prog == PROG_SYNC)
        return 2 * v + 1;
    return 2 * v;
}

static unsigned osc_aux(unsigned v)
{
    if (v_prog == PROG_SYNC)
        return 2 * v;
    return 2 * v + 1;
}

static int has_aux(void)
{
    return v_prog == PROG_SYNC || v_prog == PROG_AM;
}

static unsigned voices(void)
{
    return v_prog == PROG_AM ? AM_VOICES : VOICES;
}

/* 止めて、鳴らさないチャンネルへ（制御の 1 回の書込み） */
static void osc_idle(unsigned osc)
{
    REG_DOC_CONTROL(osc) = (uint8_t)(DOC_CTL_HALT | (CA_IDLE << DOC_CTL_CA_SH));
}

static uint8_t volume_of(const struct voice *s)
{
    if (v_prog == PROG_AM)
        return 0;                                   /* 音量は変調が上書きする */
    unsigned vel = 64U + s->vel;                    /* 65〜191 */
    return (uint8_t)(((s->level >> 8) * vel) >> 8);
}

static int32_t pitch_main(const struct voice *s)
{
    int32_t p = (int32_t)s->note * 64 + v_bend;
    if (v_prog == PROG_SYNC) {
        p += (int32_t)((uint32_t)(uint16_t)v_mod * 1536U >> 7) + s->sweep;
    } else if (v_prog != PROG_AM) {
        int x = s_lfo < 128 ? s_lfo : 255 - s_lfo;  /* 0〜127 */
        int depth = (int)(v_mod >> 2);              /* 0〜31（1/64 半音） */
        p += ((x - 64) * depth) >> 6;
    }
    return p;
}

static int32_t pitch_aux(const struct voice *s)
{
    int32_t p = (int32_t)s->note * 64 + v_bend;
    if (v_prog == PROG_AM)
        p += (int32_t)((uint32_t)(uint16_t)v_mod * 1536U >> 7);
    return p;
}

static void voice_start(unsigned v)
{
    struct voice *s = &v_voices[v];
    const struct prog *pr = &s_progs[v_prog];
    s->fc = fc_of_pitch(pitch_main(s));
    s->vol = volume_of(s);
    if (!v_doc)
        return;
    unsigned m = osc_main(v);
    struct doc_osc o = {.freq = s->fc, .volume = s->vol, .pointer = pr->page, .size = SIZE_1K,
                        .control = (uint8_t)(DOC_CTL_FREE | (v << DOC_CTL_CA_SH))};
    if (!has_aux()) {
        doc_set(m, &o);
        return;
    }
    s->fc_aux = fc_of_pitch(pitch_aux(s));
    /* 親（SYNC）は音量 0 で鳴らさず、変調（AM）は自分では鳴らない。どちらも CA 15 に置く */
    struct doc_osc a = {.freq = s->fc_aux, .volume = 0, .pointer = pr->aux_page, .size = SIZE_1K,
                        .control = (uint8_t)(DOC_CTL_SYNC | (CA_IDLE << DOC_CTL_CA_SH))};
    doc_set(m, &o);
    doc_set(osc_aux(v), &a);
}

static void voice_stop(unsigned v)
{
    v_voices[v].stage = STAGE_OFF;
    if (!v_doc)
        return;
    osc_idle(osc_main(v));
    if (has_aux())
        osc_idle(osc_aux(v));
}

/* 1 フレーム：包絡、音程、音量 */
static void voice_frame(unsigned v)
{
    struct voice *s = &v_voices[v];
    const struct prog *pr = &s_progs[v_prog];
    uint8_t hi = (uint8_t)(s->level >> 8);
    switch (s->stage) {
    case STAGE_OFF:
        return;
    case STAGE_ATTACK:
        if (255 - hi <= pr->attack) {
            s->level = 0xFF00;
            s->stage = STAGE_DECAY;
        } else {
            s->level = (uint16_t)(s->level + ((uint16_t)pr->attack << 8));
        }
        break;
    case STAGE_DECAY:
        if (hi > pr->sustain)
            s->level = (uint16_t)(hi - pr->decay > pr->sustain ? s->level - ((uint16_t)pr->decay << 8)
                                                               : (uint16_t)pr->sustain << 8);
        break;
    case STAGE_RELEASE:
        if (v_prog == PROG_AM || hi <= pr->release) {
            voice_stop(v);
            return;
        }
        s->level = (uint16_t)(s->level - ((uint16_t)pr->release << 8));
        break;
    }
    if (s->sweep)
        s->sweep = (uint16_t)(s->sweep - (s->sweep >> 3) - 1);
    if (!v_doc)
        return;
    uint16_t fc = fc_of_pitch(pitch_main(s));
    if (fc != s->fc) {
        s->fc = fc;
        doc_freq(osc_main(v), fc);
    }
    if (has_aux()) {
        fc = fc_of_pitch(pitch_aux(s));
        if (fc != s->fc_aux) {
            s->fc_aux = fc;
            doc_freq(osc_aux(v), fc);
        }
    }
    uint8_t vol = volume_of(s);
    if (vol != s->vol && v_prog != PROG_AM) {
        s->vol = vol;
        doc_volume(osc_main(v), vol);
    }
}

/* ------------------------------------------------------------------ 声の割当て */

static void note_on(uint8_t chan, uint8_t note, uint8_t vel)
{
    unsigned n = voices(), pick = 0;
    /* 空き → 離した音のうち古いもの → いちばん古いもの */
    int best = -1;
    for (unsigned v = 0; v < n; v++) {
        const struct voice *s = &v_voices[v];
        int score = s->stage == STAGE_OFF ? 0x30000 : s->stage == STAGE_RELEASE ? 0x20000 : 0x10000;
        score += (uint16_t)(v_age - s->age);
        if (score > best) {
            best = score;
            pick = v;
        }
    }
    struct voice *s = &v_voices[pick];
    if (s->stage != STAGE_OFF)
        voice_stop(pick);
    s->note = note;
    s->chan = chan;
    s->vel = vel;
    s->stage = STAGE_ATTACK;
    s->level = 0;
    s->sweep = v_prog == PROG_SYNC ? SWEEP0 : 0;
    s->age = ++v_age;
    voice_start(pick);
}

static void note_off(uint8_t chan, uint8_t note)
{
    for (unsigned v = 0; v < VOICES; v++) {
        struct voice *s = &v_voices[v];
        if (s->note == note && s->chan == chan && (s->stage == STAGE_ATTACK || s->stage == STAGE_DECAY))
            s->stage = STAGE_RELEASE;
    }
}

static void all_off(void)
{
    for (unsigned v = 0; v < VOICES; v++)
        if (v_voices[v].stage != STAGE_OFF)
            voice_stop(v);
    s_pad_note = 0xFF;
}

static void set_prog(unsigned p)
{
    all_off();
    v_prog = (uint16_t)(p % PROGS);
    s_dirty = 1;
}

/* ------------------------------------------------------------------ MIDI */

static void send(uint8_t status, uint8_t d1, uint8_t d2, uint8_t len)
{
    midi_msg_t m = {.status = status, .d1 = d1, .d2 = d2, .len = len};
    midi_send_msg(&m);
    v_out = (uint16_t)(v_out + 1 + len);
}

static void midi_message(const midi_msg_t *m)
{
    v_in++;
    if (v_thru)
        send(m->status, m->d1, m->d2, m->len);
    if (m->status >= MIDI_CLOCK)
        return;                                     /* リアルタイム */
    v_last[0] = m->status;
    v_last[1] = m->len > 0 ? m->d1 : 0x100;
    v_last[2] = m->len > 1 ? m->d2 : 0x100;
    s_dirty = 1;
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
        if (m->d1 == 1)
            v_mod = m->d2;
        else if (m->d1 == 120 || m->d1 == 123)
            all_off();
        break;
    case MIDI_PITCH_BEND: {
        int16_t b = (int16_t)(((uint16_t)m->d2 << 7 | m->d1) - 8192);
        v_bend = (int16_t)(b / (8192 / BEND_RANGE));
        break;
    }
    case MIDI_PROGRAM:
        set_prog(m->d1);
        break;
    default:
        break;
    }
}

/* ------------------------------------------------------------------ パッド */

static uint8_t cursor_note(void)
{
    return (uint8_t)(12U * (v_oct + 1U) + v_cursor);
}

static void pad_note_off(void)
{
    if (s_pad_note == 0xFF)
        return;
    note_off(PAD_CHAN, s_pad_note);
    send(MIDI_NOTE_OFF, s_pad_note, 64, 2);
    s_pad_note = 0xFF;
}

static void pad_note_on(void)
{
    s_pad_note = cursor_note();
    note_on(PAD_CHAN, s_pad_note, 100);
    send(MIDI_NOTE_ON, s_pad_note, 100, 2);
}

static void handle_pad(uint16_t now, uint16_t pressed)
{
    uint16_t released = (uint16_t)(s_pad & ~now);
    s_pad = now;
    if (pressed & PAD_START) {
        all_off();
        send(MIDI_CC, 123, 0, 2);
    }
    if ((pressed & PAD_LEFT) && v_cursor > 0)
        v_cursor--;
    if ((pressed & PAD_RIGHT) && v_cursor < KEYS - 1)
        v_cursor++;
    if ((pressed & (PAD_UP | PAD_DOWN)) != 0) {
        pad_note_off();
        if ((pressed & PAD_UP) && v_oct < 6)
            v_oct++;
        if ((pressed & PAD_DOWN) && v_oct > 0)
            v_oct--;
        for (unsigned k = 0; k < KEYS; k++)
            s_lit[k] = 2;                           /* 鍵盤を描き直す */
    }
    if (pressed & PAD_B)
        v_thru ^= 1;
    if (pressed & PAD_C) {
        set_prog(v_prog + 1U);
        send(MIDI_PROGRAM, (uint8_t)v_prog, 0, 1);
    }
    if (released & PAD_A)
        pad_note_off();
    /* 押したまま左右に動いたら弾き直す */
    if ((pressed & PAD_A) || ((now & PAD_A) && (pressed & (PAD_LEFT | PAD_RIGHT)) && s_pad_note != cursor_note())) {
        pad_note_off();
        pad_note_on();
    }
    if (pressed)
        s_dirty = 1;
}

/* ------------------------------------------------------------------ 画面 */

enum { KEY_TOP = 136, BLACK_BOTTOM = 159, WHITE_BOTTOM = 175, MARK_Y = 179, KEY_X = 2, KEY_W = 12 };
/* 半音 → 白鍵の番号（黒鍵は左の白鍵の番号）と黒鍵か */
static const uint8_t s_white_of[12] = {0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6};
static const uint8_t s_is_black[12] = {0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0};

static unsigned key_x(unsigned k)
{
    unsigned o = 0;
    while (k >= 12) {
        k -= 12;
        o++;
    }
    unsigned x = KEY_X + KEY_W * (7 * o + s_white_of[k]);
    return s_is_black[k] ? x + KEY_W - 4 : x;
}

static int key_black(unsigned k)
{
    while (k >= 12)
        k -= 12;
    return s_is_black[k];
}

/* 鍵の色を塗る：白鍵は下の部分、黒鍵は内側 */
static void draw_key(unsigned k, int on)
{
    unsigned x = key_x(k);
    if (key_black(k)) {
        acrtc_color(on ? COL_RED : COL_BG);
        acrtc_fill(x + 1, KEY_TOP + 2, x + 5, BLACK_BOTTOM - 2, 0);
    } else {
        acrtc_color(on ? COL_RED : COL_TEXT);
        acrtc_fill(x + 1, BLACK_BOTTOM + 3, x + KEY_W - 3, WHITE_BOTTOM - 2, 0);
    }
}

static void draw_keyboard(void)
{
    acrtc_color(COL_TEXT);
    for (unsigned w = 0; w < KEYS / 12 * 7; w++)
        acrtc_fill(KEY_X + KEY_W * w, KEY_TOP, KEY_X + KEY_W * w + KEY_W - 2, WHITE_BOTTOM, 0);
    acrtc_color(COL_BG);
    for (unsigned k = 0; k < KEYS; k++)
        if (key_black(k))
            acrtc_fill(key_x(k), KEY_TOP, key_x(k) + 6, BLACK_BOTTOM, 0);
}

static void draw_keys(void)
{
    static uint16_t mark = 0xFFFF;
    unsigned base = 12U * (v_oct + 1U);
    uint8_t now[KEYS] = {0};
    for (unsigned v = 0; v < VOICES; v++) {
        const struct voice *s = &v_voices[v];
        if ((s->stage == STAGE_ATTACK || s->stage == STAGE_DECAY) && s->note >= base && s->note < base + KEYS)
            now[s->note - base] = 1;
    }
    for (unsigned k = 0; k < KEYS; k++) {
        if (now[k] != s_lit[k]) {
            draw_key(k, now[k]);
            s_lit[k] = now[k];
        }
    }
    if (mark != v_cursor) {
        acrtc_color(COL_BG);
        acrtc_fill(0, MARK_Y, 255, MARK_Y + 2, 0);
        acrtc_color(COL_CYAN);
        acrtc_fill(key_x(v_cursor) + 1, MARK_Y, key_x(v_cursor) + (key_black(v_cursor) ? 5 : KEY_W - 3), MARK_Y + 2, 0);
        mark = v_cursor;
    }
}

static void put_byte(unsigned x, unsigned y, uint16_t b)
{
    if (b > 0xFF)
        bios_puts(x, y, "  ", COL_TEXT, COL_BG);
    else
        bios_put_hex(x, y, b, 2, COL_TEXT, COL_BG);
}

static void draw_voice(unsigned v)
{
    const struct voice *s = &v_voices[v];
    int held = s->stage == STAGE_ATTACK || s->stage == STAGE_DECAY;
    uint8_t mark = held ? (uint8_t)(s->note + 1) : 0;
    if (mark == s->shown)
        return;
    v_voices[v].shown = mark;
    unsigned x = v < 4 ? 8 : 128, y = 64 + 8 * (v & 3);
    if (!held) {
        bios_puts(x + 16, y, "       ", COL_TEXT, COL_BG);
        return;
    }
    unsigned n = s->note, o = 0;
    while (n >= 12) {
        n -= 12;
        o++;
    }
    bios_puts(x + 16, y, s_names[n], COL_TEXT, COL_BG);
    if (o > 0)
        bios_put_hex(x + 32, y, o - 1, 1, COL_TEXT, COL_BG);
    else
        bios_puts(x + 32, y, "L", COL_TEXT, COL_BG);
    bios_put_hex(x + 48, y, s->vel, 2, COL_TEXT, COL_BG);
}

static void draw_labels(void)
{
    bios_puts(8, 8, "SAMPLE 9  MIDI DOC SYNTH", COL_TEXT, COL_BG);
    bios_puts(8, 24, "PROG", COL_DIM, COL_BG);
    bios_puts(120, 24, "OCT", COL_DIM, COL_BG);
    bios_puts(176, 24, "THRU", COL_DIM, COL_BG);
    bios_puts(8, 40, "MOD", COL_DIM, COL_BG);
    bios_puts(72, 40, "BEND", COL_DIM, COL_BG);
    bios_puts(160, 40, "DOC", COL_DIM, COL_BG);
    for (unsigned v = 0; v < VOICES; v++)
        bios_put_hex(v < 4 ? 8 : 128, 64 + 8 * (v & 3), v, 1, COL_DIM, COL_BG);
    bios_puts(8, 104, "LAST", COL_DIM, COL_BG);
    bios_puts(8, 120, "IN", COL_DIM, COL_BG);
    bios_puts(80, 120, "OUT", COL_DIM, COL_BG);
    bios_puts(160, 120, "LOST", COL_DIM, COL_BG);
    bios_puts(8, 184, "A PLAY B THRU C PROG", COL_DIM, COL_BG);
}

static void draw_state(void)
{
    static uint16_t in = 0xFFFF, out = 0xFFFF, lost = 0xFFFF;
    for (unsigned v = 0; v < VOICES; v++)
        draw_voice(v);
    if (in != v_in || out != v_out || lost != midi_lost()) {
        in = v_in;
        out = v_out;
        lost = midi_lost();
        bios_put_hex(32, 120, in, 4, COL_TEXT, COL_BG);
        bios_put_hex(112, 120, out, 4, COL_TEXT, COL_BG);
        bios_put_hex(200, 120, lost, 4, COL_TEXT, COL_BG);
    }
    if (!s_dirty)
        return;
    s_dirty = 0;
    bios_puts(48, 24, s_prog_names[v_prog], COL_TEXT, COL_BG);
    bios_put_hex(152, 24, v_oct, 1, COL_TEXT, COL_BG);
    bios_puts(216, 24, v_thru ? "ON " : "OFF", COL_TEXT, COL_BG);
    bios_put_hex(40, 40, v_mod, 2, COL_TEXT, COL_BG);
    bios_put_hex(112, 40, (uint16_t)v_bend, 4, COL_TEXT, COL_BG);
    bios_puts(192, 40, v_doc ? "OK  " : "NONE", COL_TEXT, COL_BG);
    put_byte(48, 104, v_last[0]);
    put_byte(72, 104, v_last[1]);
    put_byte(96, 104, v_last[2]);
}

/* ------------------------------------------------------------------ 起動 */

static void load_wave(uint8_t page, unsigned kind)
{
    for (unsigned i = 0; i < WAVE_BYTES; i++) {
        uint8_t w;                                  /* $01〜$FF（0 はオシレータを止める印） */
        if (kind == PAGE_TRI)
            w = (uint8_t)(i < 512 ? 1U + ((i * 254U) >> 9) : 255U - (((i - 512U) * 254U) >> 9));
        else if (kind == PAGE_SAW)
            w = (uint8_t)(1U + ((i * 254U) >> 10));
        else
            w = i < 512 ? 0x30 : 0xD0;
        s_wave[i] = w;
    }
    fab_to_staging(STAGE_WAVE, s_wave, WAVE_BYTES);
    doc_load_wave(0, (uint16_t)page << 8, STAGE_WAVE, WAVE_BYTES);
}

static void sound_init(void)
{
    v_doc = (uint16_t)doc_present();
    if (!v_doc)
        return;
    load_wave(PAGE_TRI, PAGE_TRI);
    load_wave(PAGE_SAW, PAGE_SAW);
    load_wave(PAGE_SQUARE, PAGE_SQUARE);
    for (unsigned o = 0; o < 2 * VOICES; o++)
        osc_idle(o);
}

int main(void)
{
    bios_cls(COL_BG);
    v_last[0] = v_last[1] = v_last[2] = 0x100;
    sound_init();
    draw_labels();
    draw_keyboard();
    for (unsigned k = 0; k < KEYS; k++)
        s_lit[k] = 0;
    midi_parser_t parser;
    midi_parser_init(&parser);
    /* 起動の前に来た byte は捨てる */
    while (midi_getc() >= 0) {
    }
    for (;;) {
        bios_wait_vbl();
        v_frame++;
        midi_msg_t m;
        while (midi_read(&parser, &m))
            midi_message(&m);
        uint16_t pressed = 0;                       /* BIOS は新しく押したボタンがないと書かない */
        uint16_t now = bios_read_pad(&pressed);
        handle_pad(now, pressed);
        s_lfo = (uint8_t)(s_lfo + 23);
        for (unsigned v = 0; v < VOICES; v++)
            voice_frame(v);
        draw_keys();
        draw_state();
    }
}
