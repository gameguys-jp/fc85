/* bios.h：Boot ROM（BIOS）とカートリッジの見出しを C から使う（DR-103）。bios.inc の C 版。
 *
 * 番地と値は sdk/include/bios.inc と同じで、手で書いた。試験（sdk/c/test_c.py）が bios.inc とそろっているかを
 * 確かめる。約束の説明は sdk/docs/bios-abi.md（BIOS の版 2 の暫定の約束 [P]。DR-057、DR-094、DR-095）、
 * C での使い方は sdk/docs/c.md。
 *
 * BIOS の呼出しは、レジスタで渡す BIOS の約束を C の呼び方に直す小さな関数（sdk/c/bios.S）を通す。
 * 「Z の出力」は版 2 の約束で、関数は Z＝1 のとき 1 を返す。版 1 の BIOS では返す値に頼らない。
 */
#ifndef FC85_BIOS_H
#define FC85_BIOS_H

#include <stdint.h>

#define BIOS_VERSION           0x0002

/* ------------------------------------------------------------ 呼出しの入口（ROM の固定番地。JMP abs.L） */

#define BIOS_WAIT_VBL          0x000400UL
#define BIOS_ACRTC_SEND        0x000406UL
#define BIOS_CLS               0x00040CUL
#define BIOS_PUTS              0x000412UL
#define BIOS_PUT_HEX           0x000418UL
#define BIOS_SET_PAL           0x00041EUL
#define BIOS_READ_PAD          0x000424UL
#define BIOS_PUTS_KUTEN        0x00042AUL     /* 版 2 */
#define BIOS_GLYPH             0x000430UL     /* 版 2 */
#define BIOS_CALLS_END         0x000436UL
#define BIOS_INFO              0x000480UL
#define BIOS_INFO_VERSION      (BIOS_INFO + 0)

/* ------------------------------------------------------------ Main RAM の使い方 */

#define BIOS_VARS              0x100600UL
#define BIOS_VARS_END          0x100800UL
#define APP_RAM                0x100800UL     /* .data と .bss はここから（sdk/c/fc85.ld） */
#define INITIAL_SSP            0x120000UL     /* スタックはここから下へ */

/* BIOS の変数。カートリッジは読んでよい。書くのは「書いてよい」のものだけ（bios-abi.md §03） */
#define BIOS_VAR(type, addr)   (*(volatile type *)(addr))
#define BIOS_ARRAY(type, addr) ((volatile type *)(addr))

typedef void (*bios_hook_t)(void);

#define bios_frame             BIOS_VAR(uint32_t, 0x100600UL)     /* VBlank の数 */
#define bios_pad_a             BIOS_VAR(uint16_t, 0x100604UL)     /* VBlank ごとに読んだ PORT_A */
#define bios_pad_b             BIOS_VAR(uint16_t, 0x100606UL)     /* VBlank ごとに読んだ PORT_B */
#define bios_modules           BIOS_VAR(uint16_t, 0x100608UL)     /* 起動時の FABRIC_MODULES */
#define bios_vbl_hook          BIOS_VAR(bios_hook_t, 0x10060AUL)  /* 書いてよい。0 でなければ VBlank の割込みが呼ぶ */
#define bios_input_events      BIOS_VAR(uint16_t, 0x10060EUL)     /* 入力の割込みの数 */
#define bios_input_status      BIOS_VAR(uint16_t, 0x100610UL)     /* 書いてよい。消した INPUT_IRQ_STATUS の OR */
#define bios_midi_head         BIOS_VAR(uint16_t, 0x100612UL)     /* MIDI の受信のリングの書く位置 */
#define bios_midi_tail         BIOS_VAR(uint16_t, 0x100614UL)     /* 書いてよい。読む位置 */
#define bios_midi_buf          BIOS_ARRAY(uint8_t, 0x100616UL)    /* 64 byte */
#define bios_midi_lost         BIOS_VAR(uint16_t, 0x100656UL)     /* リングが満杯で捨てた数 */
#define bios_spurious          BIOS_VAR(uint16_t, 0x100658UL)     /* スプリアス・レベル 7・DOC（レベル 4）の割込みの数 */
#define bios_pad_prev          BIOS_VAR(uint16_t, 0x10065AUL)     /* 内部（版で変わる。頼らない） */
#define bios_exc_vector        BIOS_VAR(uint16_t, 0x10065CUL)     /* 致命的な例外のベクタ番号 */
#define bios_exc_sr            BIOS_VAR(uint16_t, 0x10065EUL)
#define bios_exc_pc            BIOS_VAR(uint32_t, 0x100660UL)
#define bios_exc_addr          BIOS_VAR(uint32_t, 0x100664UL)
#define bios_exc_busy          BIOS_VAR(uint16_t, 0x100668UL)     /* 内部 */
#define bios_probe_sp          BIOS_VAR(uint32_t, 0x10066AUL)     /* 内部 */
#define bios_hexbuf            BIOS_ARRAY(uint8_t, 0x10066EUL)    /* 内部。10 byte */
#define bios_acrtc_aborts      BIOS_VAR(uint16_t, 0x100678UL)     /* 版 2 */
#define bios_acrtc_abort_sr    BIOS_VAR(uint16_t, 0x10067AUL)     /* 版 2 */
#define BIOS_VARS_USED         0x10067CUL

/* ------------------------------------------------------------ BIOS の呼出し（sdk/c/bios.S） */

/* 次の VBlank まで待つ（STOP。割込みのマスクが 0 のときに呼ぶ） */
void bios_wait_vbl(void);
/* words から count 語を ACRTC の FIFO に送る。打ち切らなかったら 1（版 2） */
int bios_acrtc_send(const uint16_t *words, unsigned count);
/* 画面を色 color で消す */
int bios_cls(unsigned color);
/* 文字列（JIS X 0201、32 字まで）を x（4 の倍数）・y に、前景 fg・背景 bg で描く */
int bios_puts(int x, int y, const char *s, unsigned fg, unsigned bg);
/* value を digits 桁（1〜8）の 16 進で描く */
int bios_put_hex(int x, int y, uint32_t value, unsigned digits, unsigned fg, unsigned bg);
/* 16 語（$0RGB）をパレットに書く */
void bios_set_pal(const uint16_t pal[16]);
/* PORT_A を返す。pressed が 0 でなければ、前の呼出しから新しく押したボタンを書く */
uint16_t bios_read_pad(uint16_t *pressed);
/* 語の列（$00xx は JIS X 0201、$KKTT は区 KK・点 TT、0 で終わる）を描く（版 2） */
int bios_puts_kuten(int x, int y, const uint16_t *s, unsigned fg, unsigned bg);
/* 符号の字形の 1 行目の CPU 番地（行は +2 ずつ）。found が 0 でなければ、ROM にあれば 1 を書く（版 2） */
const volatile uint16_t *bios_glyph(unsigned code, int *found);

/* 約束の版（BIOS_INFO_VERSION の語）。版 2 の呼出しを使う前に 2 以上かを確かめる */
static inline unsigned bios_version(void)
{
    return *(const volatile uint16_t *)BIOS_INFO_VERSION;
}

/* ------------------------------------------------------------ 割込み */

/* 割込みを止めて前の SR を返す / SR を戻す */
static inline uint16_t fc85_irq_disable(void)
{
    uint16_t sr;
    __asm__ volatile("move.w %%sr,%0\n\tmove.w #0x2700,%%sr" : "=d"(sr) : : "memory");
    return sr;
}

static inline void fc85_irq_restore(uint16_t sr)
{
    __asm__ volatile("move.w %0,%%sr" : : "d"(sr) : "memory", "cc");
}

/* ジャンプ表（DR-025）のベクタ n の飛び先を handler にし、前の飛び先を返す。割込みを止めて書く。
 * handler は __attribute__((interrupt_handler)) を付けた関数（使うレジスタを保ち、RTE で戻る）。 */
static inline bios_hook_t fc85_set_vector(unsigned n, bios_hook_t handler)
{
    volatile bios_hook_t *slot = (volatile bios_hook_t *)(0x100000UL + 6UL * n + 2UL);
    uint16_t sr = fc85_irq_disable();
    bios_hook_t old = *slot;
    *slot = handler;
    fc85_irq_restore(sr);
    return old;
}

/* ------------------------------------------------------------ カートリッジの見出し [P]（bios-abi.md §04） */

#define CART_MAGIC             0x43415254UL   /* 'CART' */
#define CART_HDR_VERSION       0x0001

struct fc85_cart_header {
    uint32_t magic;
    uint16_t version;
    uint16_t flags;     /* 予約（0） */
    uint32_t entry;     /* 入口の絶対番地 */
    uint32_t size;      /* ROM の大きさ（byte、参考） */
    char title[16];     /* JIS X 0201、空白で埋める */
};
_Static_assert(sizeof(struct fc85_cart_header) == 32, "見出しは 32 byte");

/* 見出しを置く。1 つの .c に 1 回だけ書く。題はちょうど 16 字（空白で埋める）。
 *   FC85_CART("MY GAME         ");
 * 入口は crt0.S の _start、大きさはリンカが決める（sdk/c/fc85.ld）。 */
#define FC85_CART(title)                                                                    \
    _Static_assert(sizeof(title) == 17, "題はちょうど 16 字（空白で埋める）");             \
    extern char _start[], __rom_size[];                                                     \
    __attribute__((section(".cart_header"), used)) const struct fc85_cart_header            \
        fc85_cart_header = {CART_MAGIC, CART_HDR_VERSION, 0, (uint32_t)_start, (uint32_t)__rom_size, title}

#endif
