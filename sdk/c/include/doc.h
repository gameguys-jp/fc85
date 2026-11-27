/* doc.h：DOC（ES5503。子基板）を C から使う関数（DR-106。説明は sdk/docs/c.md §09）。
 *
 * 守ること（sdk/docs/guarantees.md §03 の 3・12〜17）は関数の中で守る：DOC の窓は MOVE.B の 1 回の読みか書き
 * だけで触り、読み書き一体の命令を使わない。0xE0・0xE2 は読まない。走査の数を増やすときは、増やすオシレータを
 * H＝1・IE＝0、CA 15 にしてから 0xE1 を書く（割込みを止めて続けて）。オシレータの制御は周波数・音量・波形の
 * 番地と大きさの後に書く。
 *
 * 子基板がないと DOC の窓はバスエラーになる。doc_present() が 1 のときだけ、ほかの関数を呼ぶ。
 * 鳴らさないオシレータは、鳴らすオシレータと別のチャンネル（CA）に置く（BIOS は CA 15 に置く。§03 の 16）。
 * 波形の値の 0 はオシレータを止める印なので、波形は $01〜$FF で書く（§03 の 17）。
 */
#ifndef FC85_DOC_H
#define FC85_DOC_H

#include <stdint.h>

/* オシレータの設定（制御は最後に書く） */
struct doc_osc {
    uint16_t freq;    /* FC（16 bit） */
    uint8_t volume;   /* 0〜255 */
    uint8_t pointer;  /* 波形の頁（番地の bit 15–8） */
    uint8_t size;     /* bit 6 BS、bit 5–3 テーブルの大きさ T、bit 2–0 分解能 R */
    uint8_t control;  /* DOC_CTL_*（H、モード、IE）と CA（<< DOC_CTL_CA_SH） */
};

/* 子基板があるか（起動時の FABRIC_MODULES の bit 2。bios_modules） */
int doc_present(void);
/* 走査しているオシレータの数（1〜32。0xE1 を読む） */
unsigned doc_oscillators(void);
/* 走査の数を n（1〜32）にする。増やすオシレータは H＝1・IE＝0・CA 15 にしてから走査に入れる */
void doc_set_oscillators(unsigned n);
/* オシレータ osc を設定する（周波数、音量、波形の番地、大きさ、最後に制御） */
void doc_set(unsigned osc, const struct doc_osc *v);
/* 周波数・音量だけを替える */
void doc_freq(unsigned osc, uint16_t freq);
void doc_volume(unsigned osc, unsigned volume);
/* 制御の H を立てる・下ろす（ほかのビットは今の値のまま。制御を読んでから書く） */
void doc_halt(unsigned osc);
void doc_go(unsigned osc);
/* Staging RAM の byte 番地 staging から bytes 個を、波形 RAM の byte 番地 wave に、Fabric のチャネル ch で運ぶ。
 * 終わるまで待ち、Fabric の STATUS（FAB_ST_DONE か FAB_ST_ERROR）を返す。子基板がなければ何もせず FAB_ST_ERROR */
unsigned doc_load_wave(unsigned ch, uint16_t wave, uint16_t staging, uint16_t bytes);

#endif
