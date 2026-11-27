/* acrtc.h：ACRTC（HD63484）を C から使う関数（DR-106。説明は sdk/docs/c.md §09）。
 *
 * 守ること（sdk/docs/guarantees.md §03 の 6〜8）は関数の中で守る：AR/SR とデータポートは = の 1 回の読み書き
 * だけ、FIFO は SR の WFE・WFR・RFR を見てから読み書きする。待ちの数や命令の速さには頼らない。
 *
 * - AR：関数は AR が r00（FIFO）であるとして始め、r00 に戻して終わる。BIOS もカートリッジの入口と呼出しの後で
 *   r00 にする（bios-abi.md §04・§05）。AR を自分で替えたら、ACRTC_FIFO に戻してから呼ぶ。
 * - 誤り：待つ間に SR の CER（コマンドの誤り）が立てば、CCR の ABT で打ち切り（FIFO も空になる。CCR のほかの
 *   ビットは保つ）、0 を返す。残りの語は送らない。打ち切らなければ 1。
 * - 待つ上限はない（BIOS の 0.4 秒の見張りがない）。長くかかる正しいコマンドもそのまま送れる。終わらない形
 *   （パラメタが足りないコマンド）を送ると、acrtc_wait は戻らない。
 * - 再入できない（BIOS の ACRTC の呼出しと同じ）。割込みの処理から使うのは、割り込まれた側が ACRTC を使って
 *   いないときだけ。Data Fabric の port 1 で DMA している間は FIFO に触らない（guarantees.md §03 の 8）。
 *
 * 図形の関数は BIOS が入口で残す描画の状態（bios-abi.md §06：ORG を画面の左上、パターン RAM は 0、MASK は
 * $FFFF）を前提にする。座標は画面座標（x は右、y は下。関数が論理座標の y の符号を変える）。op はコマンドの
 * 下位 8 bit で、演算（ACRTC_OPM_*）・色（ACRTC_COL_*）・領域の指定の OR。0 は「置き換え、CL0 の色」。
 */
#ifndef FC85_ACRTC_H
#define FC85_ACRTC_H

#include <stdint.h>

/* ---- レジスタと FIFO */

/* 制御のレジスタ reg（ACRTC_CCR、ACRTC_OMR、ACRTC_DCR、r80 以降）に v を書く・読む。AR は r00 に戻す */
void acrtc_write_reg(unsigned reg, uint16_t v);
uint16_t acrtc_read_reg(unsigned reg);
/* 書込み FIFO に 1 語（WFR を待つ） */
int acrtc_put(uint16_t w);
/* words から count 語を書込み FIFO に送る。WFE（空）なら 8 語を続けて、WFR（空きあり）なら 1 語を書く */
int acrtc_send(const uint16_t *words, unsigned count);
/* 読出し FIFO から 1 語を *out に（RFR を待つ）。打ち切ったときは *out を書かない */
int acrtc_get(uint16_t *out);
/* 送ったコマンドが全部終わるまで待つ（WFE と CED） */
int acrtc_wait(void);
/* CCR の ABT でコマンドを打ち切る（FIFO も空になる）。CCR のほかのビットは保つ */
void acrtc_abort(void);

/* ---- 図形（画面座標。終わりを待たない。続けて描くなら、そのまま次を送ってよい） */

/* 描く色（CL0 と CL1）をパレットの番号 c（0〜15）にする（c×$1111） */
int acrtc_color(unsigned c);
/* (x, y) の 1 画素 */
int acrtc_dot(int x, int y, unsigned op);
/* (x0, y0) から (x1, y1) への線。終わりの点は描かない */
int acrtc_line(int x0, int y0, int x1, int y1, unsigned op);
/* (x0, y0) と (x1, y1) を角とする長方形の枠 */
int acrtc_box(int x0, int y0, int x1, int y1, unsigned op);
/* (x0, y0) と (x1, y1) を角とする長方形を塗る（両端を含む） */
int acrtc_fill(int x0, int y0, int x1, int y1, unsigned op);
/* 中心 (x, y)、半径 r の円 */
int acrtc_circle(int x, int y, unsigned r, unsigned op);

/* ---- 矩形の読み書きと DMA（Data Fabric の port 1。DR-112）
 *
 * CCR の DDM を立てると、DRD・DWT・DMOD のデータは FIFO を通らず、Fabric の port 1 で運ぶ（DMA）。手順は
 * SAMPLE 3（sdk/samples/patches、TEMPORAL FEEDBACK）と同じ：acrtc_dma(1) → acrtc_read_rect か acrtc_write_rect
 * → Fabric のチャネルで port 1 から（へ）words×rows 語（fabric.h の fab_run）→ acrtc_dma(0)。
 * DDM の間は、BIOS の字を描く呼出し（bios_puts・bios_put_hex など）を使わない（BIOS は DWT のデータを FIFO で
 * 書く。bios-abi.md §06）。DDM＝0 なら、データは FIFO で読み書きする（acrtc_get・acrtc_send）。 */

/* 基本画面（DN＝1）の画素 (x, y) を含む語の番地（1 行 64 語、1 語に 4 画素。BIOS の画面。bios-abi.md §06） */
#define ACRTC_WORD_ADDR(x, y) ((uint32_t)(y) * 64U + (uint32_t)(x) / 4U)

/* CCR の DDM を立てる（on）か下ろす。ACRTC の仕事が終わるのを待ってから書く（DDM は実行を始めたときの値で
 * 決まる）。CCR のほかのビットは保つ */
int acrtc_dma(int on);
/* RWP（読み書きの番地）を基本画面の語番地 addr にする */
int acrtc_rwp(uint32_t addr);
/* 語番地 addr から、横 words 語・縦 rows 行（下へ）を読む DRD を送る。データは words×rows 語 */
int acrtc_read_rect(uint32_t addr, unsigned words, unsigned rows);
/* 同じ形の矩形に書く。cmd は ACRTC_DWT か、ACRTC_DMOD | 演算（ACRTC_OPM_*） */
int acrtc_write_rect(uint32_t addr, unsigned words, unsigned rows, uint16_t cmd);

#endif
