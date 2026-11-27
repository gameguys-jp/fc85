/* adpcm.c：ADPCM（MSM5205）の音を Data Fabric で鳴らす関数（DR-106。約束は include/adpcm.h）。状態はサンプルの
 * 効果音（sdk/samples/game/sfx.s）と同じ。
 *   IDLE    RESET。チャネルは止まっている。
 *   FLUSH   鳴っている音を止めた：チャネルを止め、FIFO を空にした。RESET は次のフレームでかける。
 *   ARMED   RESET をかけた（S1・S2 は次の音のもの）。次の adpcm_update で始める。
 *   PLAYING チャネルを始め、FIFO に PRIME byte たまったら RESET を外した。DONE で FIFO が空になったら IDLE。
 */
#include "adpcm.h"
#include "fabric.h"
#include "fc85.h"

enum { IDLE, FLUSH, ARMED, PLAYING };
enum {
    PRIME = 16,       /* RESET を外す前に FIFO にためる byte 数 [P] */
    PRIME_POLL = 400, /* その待ちで STATUS を読む回数の上限（1 byte は約 1 µs。届かなくても外す） */
};

struct sound {
    uint16_t staging, bytes, ctrl;
};

static uint8_t s_ch;
static uint8_t s_state;
static uint8_t s_req;
static struct sound s_cur, s_next;

static uint16_t rate(uint16_t ctrl)
{
    /* S1・S2 を両方 1 にしない（Bend の操作。guarantees.md §03 の 20） */
    ctrl &= ADPCM_CTL_S1 | ADPCM_CTL_S2 | ADPCM_CTL_LOW_FIRST;
    if ((ctrl & (ADPCM_CTL_S1 | ADPCM_CTL_S2)) == (ADPCM_CTL_S1 | ADPCM_CTL_S2))
        ctrl &= (uint16_t)~ADPCM_CTL_S2;
    return ctrl;
}

static void flush_fifo(void)
{
    REG_FABRIC_PORT_RESET = 1U << PORT_ADPCM;
}

void adpcm_init(unsigned ch)
{
    s_ch = (uint8_t)ch;
    REG_ADPCM_CTRL = ADPCM_CTL_RESET | ADPCM_CTL_8K;
    fab_stop(ch);
    flush_fifo();
    REG_ADPCM_STATUS = ADPCM_ST_UNDERRUN;
    s_state = IDLE;
    s_req = 0;
}

void adpcm_play(uint16_t staging, uint16_t bytes, uint16_t ctrl)
{
    s_next.staging = staging;
    s_next.bytes = bytes;
    s_next.ctrl = rate(ctrl);
    s_req = 1;
}

void adpcm_stop(void)
{
    adpcm_play(0, 0, 0);
}

int adpcm_busy(void)
{
    return s_state != IDLE || s_req;
}

/* RESET をかけ（S1・S2 は次の音のもの）、チャネルを止めて FIFO を空にする */
static void arm(void)
{
    REG_ADPCM_CTRL = s_cur.ctrl | ADPCM_CTL_RESET;
    fab_stop(s_ch);
    flush_fifo();
    s_state = s_cur.bytes ? ARMED : IDLE;
}

static void start(void)
{
    const struct fab_xfer x = {
        .src = PORT_STAGING,
        .dst = PORT_ADPCM,
        .src_addr = s_cur.staging,
        .count = s_cur.bytes,
        .src_step = 1,
        .format = FMT_8_8,
        .trigger = TRG_NOW,
    };
    fab_set(s_ch, &x);
    fab_start(s_ch);
    for (unsigned i = 0; i < PRIME_POLL && (REG_ADPCM_STATUS & ADPCM_ST_COUNT) < PRIME; i++) {
    }
    REG_ADPCM_STATUS = ADPCM_ST_UNDERRUN;
    REG_ADPCM_CTRL = s_cur.ctrl;
    s_state = PLAYING;
}

static void finish(void)
{
    fab_stop(s_ch);
    REG_ADPCM_CTRL = s_cur.ctrl | ADPCM_CTL_RESET;
    REG_ADPCM_STATUS = ADPCM_ST_UNDERRUN;
    s_state = IDLE;
}

void adpcm_update(void)
{
    if (s_req) {
        s_req = 0;
        s_cur = s_next;
        if (s_state == PLAYING) {
            /* 鳴っている音を止める。bridge は 0x08 を置きはじめる。RESET は次のフレームで */
            fab_stop(s_ch);
            flush_fifo();
            s_state = FLUSH;
        } else if (s_state != FLUSH) {
            arm();
        }
        return;
    }
    switch (s_state) {
    case FLUSH:
        arm();
        break;
    case ARMED:
        start();
        break;
    case PLAYING: {
        unsigned s = fab_status(s_ch);
        if ((s & FAB_ST_ERROR) || ((s & FAB_ST_DONE) && (REG_ADPCM_STATUS & ADPCM_ST_COUNT) == 0))
            finish();
        break;
    }
    default:
        break;
    }
}
