/* rt_test.c：C の実行時ライブラリと BIOS の関数の試験の ROM（sdk/c/test_c.py が組んで動かす）。
 *
 * vectors.h（試験が作る）の入力で計算し、結果を Main RAM の r_* に置く。試験は Python で同じ計算をして
 * ラベルで読み比べる。終わったら done に $C0DE を書き、VBlank を待ち続けてパッドを r_pad に写す。
 */
#include <stdint.h>
#include <string.h>

#include "bios.h"
#include "fc85.h"
#include "vectors.h"

FC85_CART("C RUNTIME TEST  ");

uint32_t r_mul[N32], r_udiv[N32], r_umod[N32];
int32_t r_sdiv[N32], r_smod[N32];
uint64_t r_mul64[N64], r_udiv64[N64], r_umod64[N64], r_shl64[N64], r_shr64[N64];
int64_t r_sdiv64[N64], r_smod64[N64], r_sar64[N64];

uint32_t data_word = 0x12345678UL; /* .data の初期値 */
uint16_t data_half = 0xBEEF;
uint32_t bss_words[4];             /* .bss は 0 */
uint32_t r_init[4];

uint8_t r_mem[64];
int16_t r_cmp[6];
uint16_t r_bios[10];
uint32_t r_glyph;
volatile uint16_t hook_count;
volatile uint16_t trap_count;
uint16_t r_hook, r_trap;
uint16_t r_pad[2];
uint16_t done;

static void hook(void)
{
    hook_count++;
}

__attribute__((interrupt_handler)) static void on_trap0(void)
{
    trap_count++;
}

/* volatile のレジスタへの 0 の書込み（逆アセンブルに CLR がないことを試験が見る）。AR＝r00 は BIOS の決まりの状態 */
__attribute__((noinline)) void probe_zero_store(void)
{
    REG_ACRTC_AR = 0;
}

static void arith(void)
{
    const volatile uint32_t *pa = a32, *pb = b32;
    for (int i = 0; i < N32; i++) {
        uint32_t a = pa[i], b = pb[i];
        r_mul[i] = a * b;
        r_udiv[i] = a / b;
        r_umod[i] = a % b;
        r_sdiv[i] = (int32_t)a / (int32_t)b;
        r_smod[i] = (int32_t)a % (int32_t)b;
    }
    const volatile uint64_t *qa = a64, *qb = b64;
    const volatile uint8_t *qs = s64;
    for (int i = 0; i < N64; i++) {
        uint64_t a = qa[i], b = qb[i];
        int s = qs[i];
        r_mul64[i] = a * b;
        r_udiv64[i] = a / b;
        r_umod64[i] = a % b;
        r_sdiv64[i] = (int64_t)a / (int64_t)b;
        r_smod64[i] = (int64_t)a % (int64_t)b;
        r_shl64[i] = a << s;
        r_shr64[i] = a >> s;
        r_sar64[i] = (int64_t)a >> s;
    }
}

static void mem(void)
{
    static const char text[] = "0123456789ABCDEFGHIJKLMNOPQRSTUV";
    memset(r_mem, 0xA5, sizeof r_mem);        /* 全部 $A5 */
    memcpy(r_mem + 1, text, 10);              /* 奇数の番地へ */
    memcpy(r_mem + 16, text, 16);             /* 偶数どうし（長語） */
    memmove(r_mem + 34, r_mem + 32, 0);       /* 0 byte */
    memcpy(r_mem + 40, text, 12);
    memmove(r_mem + 42, r_mem + 40, 12);      /* 重なり（後ろへ） */
    memset(r_mem + 57, 0, 3);
    r_cmp[0] = memcmp("abc", "abd", 3);
    r_cmp[1] = memcmp("abc", "abc", 3);
    r_cmp[2] = (int16_t)strlen(text);
    r_cmp[3] = strcmp("abc", "abd");
    r_cmp[4] = strcmp("b", "abc");
    char buf[8];
    strcpy(buf, "xyz");
    r_cmp[5] = strcmp(buf, "xyz");
}

static void bios(void)
{
    static const uint16_t kuten[] = {0x0041, 0x0042, 0};
    int found = -1;
    r_bios[0] = bios_version();
    r_bios[1] = bios_cls(0);
    r_bios[2] = bios_puts(8, 8, "C RUNTIME", 15, 0);
    r_bios[3] = bios_put_hex(8, 24, 0x1234ABCDUL, 8, 15, 0);
    r_bios[4] = bios_puts_kuten(8, 40, kuten, 15, 0);
    r_glyph = (uint32_t)bios_glyph('A', &found);
    r_bios[5] = (uint16_t)found;
    r_bios[6] = bios_acrtc_send(kuten, 0);
    r_bios[7] = bios_modules;
    r_bios[8] = REG_FABRIC_MODULES;
    r_bios[9] = (REG_ACRTC_SR & ACRTC_SR_CER) ? 1 : 0;
}

int main(void)
{
    r_init[0] = data_word;
    r_init[1] = data_half;
    r_init[2] = bss_words[0] | bss_words[1] | bss_words[2] | bss_words[3];
    r_init[3] = (uint32_t)&data_word;
    arith();
    mem();
    bios();
    probe_zero_store();

    bios_vbl_hook = hook;
    for (int i = 0; i < 3; i++)
        bios_wait_vbl();
    bios_vbl_hook = 0;
    r_hook = hook_count;

    bios_hook_t old = fc85_set_vector(32, on_trap0);
    __asm__ volatile("trap #0\n\ttrap #0" ::: "memory");
    fc85_set_vector(32, old);
    r_trap = trap_count;

    done = 0xC0DE;
    for (;;) {
        bios_wait_vbl();
        uint16_t pressed = 0;
        r_pad[0] = bios_read_pad(&pressed);
        r_pad[1] |= pressed;
    }
}
