/*
 * fc85emu.h : fc85 のエミュレータを C の ABI でほかのソフトに組み込む窓口（DR-104）。
 *
 * 実体は emulator/crates/capi（Rust。cargo build --release -p fc85-capi で fc85emu.dll・libfc85emu.so・
 * libfc85emu.a など）。説明は sdk/docs/emulator-api.md §08。ROM の中で使う fc85.h（レジスタの表）とは別物。
 *
 * 約束
 *   - 誤りは戻り値（-1 か NULL）で知らせ、文は fc85_last_error()（UTF-8）。
 *   - 1 つの仮想機を同時に 2 つのスレッドから使わない（別々の仮想機なら別のスレッドでよい）。
 *   - 渡したバイト列（ROM、状態）は関数の中で写す。呼んだ後に手放してよい。
 *   - 返された領域は、対の関数（fc85_free_bytes、fc85_free_string）で返す。free() では返さない。
 *   - 仮想機の 68000 が止まっても（halt）、フレームも映像も音も進む。
 *
 * ライセンスは MIT（リポジトリの LICENSE）。
 */
#ifndef FC85EMU_H
#define FC85EMU_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* C の ABI の版（fc85_abi_version）。関数の形を変えたら上げる。 */
#define FC85_ABI_VERSION 1

/* fc85_new の flags */
#define FC85_NO_DOC        0x01u /* DOC の子基板を挿さない */
#define FC85_DIGITAL_AUDIO 0x02u /* 音をチップの出力のデジタルの和にする（既定は LINE OUT） */

/* 画面の範囲（area） */
#define FC85_AREA_ACTIVE 0 /* 表示の領域 256×192 */
#define FC85_AREA_VIEW   1 /* テレビに映る範囲 320×240（縁を含む） */
#define FC85_AREA_RASTER 2 /* ラスタの全部 342×262（帰線を含む） */

/* メモリの空間（space） */
#define FC85_SPACE_CPU      0 /* 68000 から見たメモリの窓（Boot ROM、Main RAM、Staging、カートリッジ） */
#define FC85_SPACE_VRAM     1 /* ACRTC の VRAM 128 KiB */
#define FC85_SPACE_WAVE     2 /* DOC の波形 RAM 64 KiB */
#define FC85_SPACE_DSP_PROG 3 /* TMS32010 のプログラム RAM */
#define FC85_SPACE_TEMPORAL 4 /* Temporal RAM */
#define FC85_SPACE_CHAR_ROM 5 /* 文字 ROM */
#define FC85_SPACE_PALETTE  6 /* 表示中のパレット（読むだけ） */

/* PORT_A・PORT_B のビット（PAD） */
#define FC85_PAD_UP    0x01u
#define FC85_PAD_DOWN  0x02u
#define FC85_PAD_LEFT  0x04u
#define FC85_PAD_RIGHT 0x08u
#define FC85_PAD_A     0x10u
#define FC85_PAD_B     0x20u
#define FC85_PAD_C     0x40u
#define FC85_PAD_START 0x80u

typedef struct Fc85 fc85_t;

/* 版と定数 */
uint32_t fc85_abi_version(void);
const char *fc85_version(void);
uint32_t fc85_audio_rate(void);          /* 48000 */
double fc85_frame_rate(void);            /* 約 59.92 */
uint32_t fc85_screen_width(int32_t area);
uint32_t fc85_screen_height(int32_t area);
const char *fc85_last_error(void);

/* 作る・壊す。boot・chars は NULL でもよい（Boot ROM がなければ 68000 はすぐ止まる） */
fc85_t *fc85_new(const uint8_t *boot, size_t boot_len, const uint8_t *chars, size_t chars_len,
                 uint32_t flags);
void fc85_free(fc85_t *m);
int32_t fc85_load_cart(fc85_t *m, const uint8_t *cart, size_t len); /* len 0 で抜く。電源を入れ直す */
int32_t fc85_reset(fc85_t *m);

/* 進める */
int32_t fc85_set_input(fc85_t *m, uint16_t port_a, uint16_t port_b);
int32_t fc85_run_frames(fc85_t *m, uint32_t n);
uint64_t fc85_frame(fc85_t *m);

/* 画面（0x00RRGGBB、左上から行ごと。戻り値は画素の数）と音（L・R の s16。戻り値は組の数） */
int32_t fc85_screen(fc85_t *m, int32_t area, uint32_t *out, size_t out_len);
/* 同じ画面を R・G・B・A（A は 255）の byte の順で（ブラウザの ImageData の形）。out_len は byte の数 */
int32_t fc85_screen_rgba(fc85_t *m, int32_t area, uint8_t *out, size_t out_len);
int32_t fc85_audio(fc85_t *m, int16_t *out, size_t max_frames);

/* 状態。fc85_save_state の領域は fc85_free_bytes で返す */
uint8_t *fc85_save_state(fc85_t *m, size_t *len);
int32_t fc85_load_state(fc85_t *m, const uint8_t *data, size_t len);
void fc85_free_bytes(uint8_t *p, size_t len);

/* メモリ（バスを通さない） */
int32_t fc85_peek(fc85_t *m, int32_t space, uint32_t addr, uint8_t *out, size_t len);
int32_t fc85_poke(fc85_t *m, int32_t space, uint32_t addr, const uint8_t *data, size_t len);

/* JSON の手順（sdk/docs/emulator-api.md）。答えの行は fc85_free_string で返す */
char *fc85_call(fc85_t *m, const char *request);
void fc85_free_string(char *s);

/* 領域（WebAssembly の呼ぶ側のため。C からは使わなくてよい） */
uint8_t *fc85_alloc(size_t n);
void fc85_dealloc(uint8_t *p, size_t n);

#ifdef __cplusplus
}
#endif

#endif /* FC85EMU_H */
