/* dsp.h：DSP（TMS32010。子基板）を C から使う関数（DR-109。説明は sdk/docs/c.md §09）。
 *
 * 手順はサンプル（sdk/samples/dsp/main.s）と同じ（DR-029）：RESET を書き、PRGBUS を待ってからプログラム窓
 * （$280000〜、4K 語）に語で書く。PRGBUS＝0 の間に窓に触るとバスエラーなので、dsp_load() が待つ。RESET を
 * 解くと DSP は 0 番地から動く。68000 との受け渡しは、メールボックス（68000 → DSP、DSP は IN 3 で読む）と
 * 応答（DSP の OUT 2 → 68000）、音や値の流れは Data Fabric の port 3（入力 FIFO）・port 4（出力 FIFO）。
 *
 * DSP のプログラムは dspasm で組む。`py sdk/dspasm/dspasm.py prog.dsp -o prog.h --c-array prog` で、C の配列
 * （static const uint16_t prog[]、語数 prog_WORDS）の見出しになる。
 *
 * DSP_CTRL・DSP_STATUS は読み書き一体の命令で触らない（関数は値を作ってから 1 回で書く）。
 */
#ifndef FC85_DSP_H
#define FC85_DSP_H

#include <stdint.h>

/* DSP を RESET にし、PRGBUS を待ち、プログラム窓の 0 番地から code の words 語を書き、zero_rest なら残りを 0 に
 * して、書いた範囲を読み返す。違った語の数を返す（0 で正しい）。PRGBUS が来なければ（子基板がない）−1。DSP は
 * RESET のまま */
int dsp_load(const uint16_t *code, unsigned words, int zero_rest);
/* RESET を解いて動かす。ctrl は DSP_CTRL に書く値（RESET 以外：BIO の選び方 DSP_CTRL_BIO_*、DSP_CTRL_IRQ_EN） */
void dsp_start(uint16_t ctrl);
/* RESET にする（プログラム窓が開く） */
void dsp_reset(void);
/* メールボックスに cmd を書き、応答を待って *reply に入れる。tries 回（1 回 約 3 µs）見て来なければ 0 を返す */
int dsp_command(uint16_t cmd, uint16_t *reply, unsigned tries);
/* DSP_STATUS（PRGBUS、入力の語あり、出力に空き、COMMAND、RESPONSE、UNDERRUN、OVERRUN） */
uint16_t dsp_status(void);
/* UNDERRUN・OVERRUN の印を消す（W1C） */
void dsp_clear_flags(void);

#endif
