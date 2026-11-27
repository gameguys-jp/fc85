/* adpcm.h：ADPCM（MSM5205）の音を Data Fabric で鳴らす関数（DR-106。説明は sdk/docs/c.md §09）。
 *
 * 道：Staging RAM ─（Fabric のチャネル、port 0 → port 5、FORMAT 0、即時、ブロック）→ ADPCM の FIFO（64 byte）
 *     ─（bridge が V̄C̄K̄ ごとにニブルを渡す）→ MSM5205。
 * 手順は guarantees.md §03 の 19（DR-060）で、サンプルの効果音（sdk/samples/game/sfx.s）と同じ：RESET をかけて
 * FIFO を空にし、1 フレーム置いてから START、FIFO に 16 byte たまったら RESET を外す。DONE が立って FIFO が空に
 * なったら止めて RESET に戻す。鳴っている音を替えるときは、先にチャネルを止めて FIFO を空にし、次のフレームで
 * RESET をかける（RESET の前に残るニブルを 0 か 8 にする）。
 *
 * 使い方：起動時に adpcm_init、音のデータ（4 bit の ADPCM。1 byte に 2 ニブル、既定は上位から）を
 * fab_to_staging で Staging RAM に写し、adpcm_play で求め、毎フレーム 1 回 adpcm_update を呼ぶ。求めから音が
 * 出るまで 1〜3 フレーム。adpcm_init で渡したチャネルは、ほかに使わない。adpcm_play と adpcm_update は同じ流れ
 * （どちらも割込みの外か、どちらも同じ割込みの処理）から呼ぶ。
 */
#ifndef FC85_ADPCM_H
#define FC85_ADPCM_H

#include <stdint.h>

/* Fabric のチャネル ch を ADPCM に使う。ADPCM を RESET にし、FIFO を空にする */
void adpcm_init(unsigned ch);
/* Staging RAM の byte 番地 staging から bytes 個を鳴らす。ctrl は ADPCM の CTRL の標本化と並び
 * （ADPCM_CTL_8K・6K・4K と ADPCM_CTL_LOW_FIRST）。鳴っている音は替わる。bytes が 0 なら止める */
void adpcm_play(uint16_t staging, uint16_t bytes, uint16_t ctrl);
/* 止める（adpcm_play(0, 0, 0) と同じ） */
void adpcm_stop(void);
/* 1 フレームに 1 回 */
void adpcm_update(void);
/* 鳴っているか、鳴らす途中か、求めがあるか */
int adpcm_busy(void);

#endif
