# C で ROM を書く

**v0.8**  
2026年11月27日

> fc85 のカートリッジを C で書くための手引き（DR-103）。GCC は外の道具として使い、SDK には入れない（DR-092 (3)）。SDK が持つのは、起動のコード、リンカの設定、小さな実行時ライブラリ、BIOS を呼ぶ関数、ACRTC・Data Fabric・DOC・ADPCM を使う関数（§09、DR-106）、パレットと HBlank の割込み・DSP を使う関数（§09、DR-109）、組立ての道具。番地とレジスタは `sdk/include/fc85.h`（生成物。DR-100）、BIOS は `sdk/c/include/bios.h`、BIOS の約束は `bios-abi.md`、ROM が守ることは `guarantees.md` が正。アセンブラ（m68kasm）で書く ROM もそのまま使える。

## 01　はじめ方

```
py sdk/tools/new_rom.py ../my-rom --title "MY ROM"     # 既定で C のひな形（main.c）
cd ../my-rom
py build.py                                            # build/rom.bin ほか
py -m unittest                                         # 試験（エミュレータで動かす）
```

ひな形の `main.c` は、題・フレームの数・PORT_A を出し、方向キーで四角を動かす。作り方・試し方・守ることの要点はひな形の `AGENTS.md`。

## 02　GCC を入れる

68000 を出せる GCC なら、どれでもよい [P]。確かめたのは Ubuntu 22.04（WSL）の `gcc-m68k-linux-gnu`（GCC 11.4）。

| 環境 | 入れ方 |
|---|---|
| Ubuntu・Debian・WSL | `sudo apt install gcc-m68k-linux-gnu` |
| Windows | WSL の Ubuntu に上と同じものを入れる。組立ての道具が WSL を通して呼ぶ |
| そのほか | `m68k-elf-gcc`（クロスの GCC と binutils）を PATH に置く |

組立ての道具（`sdk/tools/m68kgcc.py`）が探す順：

1. 環境変数 `FC85_GCC`：道具の名前の前置き（`m68k-elf-`、`/opt/m68k/bin/m68k-elf-` など）。`wsl:m68k-linux-gnu-` のように `wsl:` を付けると WSL の中の道具。
2. PATH の `m68k-elf-gcc`、`m68k-linux-gnu-gcc`。
3. Windows では WSL の中の `m68k-linux-gnu-gcc`、`m68k-elf-gcc`。distro は `FC85_WSL_DISTRO`、ドライブを見せる道は `FC85_WSL_MOUNT`（既定 `/mnt/`）で替える。

どれを使うかは `py sdk/tools/m68kgcc.py --which` で分かる。

`m68k-linux-gnu` は Linux 向けの GCC だが、組立ての道具が 68000 の命令だけ（`-m68000`）、OS なし（`-ffreestanding`）、位置固定（`-fno-pie`）、スタック保護なしで呼ぶ。隣の欄への書込みをまとめる最適化も止める（`-fno-store-merging`）。GCC 11.4 は構造体の初期化で、奇数番地への長語の書込みを出すことがあり、68000 ではアドレスエラー（ベクタ 3）になる（`-mstrict-align` では止まらない。DR-106）[V]。GCC の実行時ライブラリ（libgcc）と C の標準ライブラリはつながない。Linux 向けの libgcc は 68020 向けに組まれていて、68000 にない命令（MULU.L、DIVU.L など）を含むため（逆アセンブルで確かめた）。

## 03　メモリと起動

リンカの設定は `sdk/c/fc85.ld` [P]。

| 番地 | 中身 |
|---|---|
| $400000 | 見出し（32 byte。§04） |
| $400020 | 入口 `_start`（`sdk/c/crt0.S`） |
| 続き | コード（.text）、定数（.rodata）、.data の初期値。合わせて 1 MiB まで |
| $100800（`APP_RAM`） | .data、続けて .bss |
| 〜$120000（`INITIAL_SSP`） | スタック（下へ）。既定で 4 KiB を取っておく |

- `_start` は、.data の初期値を ROM から RAM へ写し、.bss を 0 にして `main` を呼ぶ。`main` から戻ると VBlank を待ち続ける（割込みは受け続ける）。BIOS が入口に来たときの状態（スーパバイザ、SR＝$2000、SSP＝$120000）はそのまま（`bios-abi.md` §04）。
- .data＋.bss＋スタックが Main RAM（$100800〜$11FFFF、126 KiB）に入らないと、リンクで止まる。スタックを替えるときは `build.py` の `CFLAGS` に `-Wl,--defsym=__stack_size=0x2000` を足す。
- `__heap_start`〜`__heap_end`（.bss の後ろからスタックの手前まで）は空いている。`malloc` はない（§05）ので、使うなら自分で配る。
- 静的な初期化子のほかに、起動時に走る処理（C++ の構築子、`constructor` 属性）はない。

## 04　見出し

1 つの .c に 1 回だけ書く。題はちょうど 16 字（空白で埋める。英字・数字・空白）。

```c
#include "bios.h"
FC85_CART("MY ROM          ");
```

印（`CART`）、見出しの版、入口（`_start`）、大きさはリンクで埋まる（`bios-abi.md` §04）。ないか 2 つあるとリンクで止まる。

## 05　C で使えるもの

- **ヘッダ**：GCC の OS なしのヘッダ（`stdint.h`、`stddef.h`、`stdbool.h`、`stdarg.h`、`stdalign.h`、`stdnoreturn.h`、`iso646.h`）と、SDK の `limits.h`、`string.h`、`bios.h`、`fc85.h`、`acrtc.h`、`fabric.h`、`doc.h`、`adpcm.h`、`video.h`、`dsp.h`、`temporal.h`（§09）。`stdio.h`・`stdlib.h` などはない（インクルードで止まる）。
- **string.h**：`memcpy`、`memmove`、`memset`、`memcmp`、`strlen`、`strcmp`、`strcpy`（`sdk/c/string.c`）。両方が偶数番地なら長語ずつ写す。Main RAM と Staging RAM だけに使う（MMIO の窓には幅と回数が決まらないので使わない）。
- **整数**：32 bit の掛け算・割り算（`sdk/c/runtime.S`）と 64 bit（`long long`）のシフト・掛け算・割り算（`sdk/c/runtime64.c`）は、GCC が呼ぶ関数を SDK が持つ。名前は GCC の実行時ライブラリと同じで、中身はここで書いた。16×16 の掛け算と 32÷16 の割り算は 68000 の命令（MULU・DIVU）になる。64 bit の割り算は 1 bit ずつ引くので遅い。
- **0 で割る**：DIVU の 0 除算の例外（ベクタ 5）になり、BIOS が赤い画面で止まる（`bios-abi.md` §03）。
- **ないもの**：浮動小数点（`float`・`double` の演算はリンクで止まる）、`malloc`、`printf`、C++、起動時の構築子。固定小数点の整数で書く [P]。

## 06　呼び方とアセンブラ

GCC の m68k の呼び方 [D]：

- 引数はスタックに 4 byte ずつ積む（`int` と `short` はどちらも 4 byte。値は下位の語が +2）。
- 結果は D0（64 bit は D0:D1。ポインタは D0 と A0）。ポインタを返す C の関数は同じ値を D0 と A0 の両方に入れ、C の呼ぶ側は A0 を読む。アセンブラでポインタを返す関数を書くときは、A0 に入れる（D0 にも入れておくと、どちらを読む側にも合う。SDK の関数はそうしている）[V]（GCC 11.4 の m68k-linux-gnu で、-O0・-Os・-O2 のコードを見て確かめた。ほかの版と `m68k-elf` は確かめていない。OI-102）。
- D0・D1・A0・A1 は呼ばれた側が壊してよい。D2〜D7・A2〜A6 は保つ。
- `int` は 32 bit。`-mshort` は使わない。

C の ROM にアセンブラを混ぜるときは、GNU as の書き方の `.S`（大文字。C の前処理を通る。レジスタは `%d0`）をこのフォルダに置く。m68kasm の `.s` は平らな ROM を作る道具で、C とはつなげない。速さや時間が要るところをアセンブラで書くときの見本は `sdk/c/bios.S` と `sdk/c/runtime.S`。

関数の中に少しだけ書くなら GCC の `__asm__`（例：`bios.h` の `fc85_irq_disable`）。

## 07　BIOS

`bios.h` は `bios.inc` の C 版。番地と値は試験（`sdk/c/test_c.py`）が `bios.inc` と突き合わせる。

| 関数 | BIOS の呼出し（`bios-abi.md` §05） | 返す値 |
|---|---|---|
| `bios_wait_vbl()` | BIOS_WAIT_VBL | なし |
| `bios_acrtc_send(words, count)` | BIOS_ACRTC_SEND | 打ち切らなかったら 1 |
| `bios_cls(color)` | BIOS_CLS | Z（1 か 0） |
| `bios_puts(x, y, s, fg, bg)` | BIOS_PUTS | Z |
| `bios_put_hex(x, y, value, digits, fg, bg)` | BIOS_PUT_HEX | Z |
| `bios_set_pal(pal)` | BIOS_SET_PAL | なし |
| `bios_read_pad(&pressed)` | BIOS_READ_PAD | PORT_A。`pressed` に新しく押したボタン（0 なら書かない） |
| `bios_puts_kuten(x, y, s, fg, bg)` | BIOS_PUTS_KUTEN（版 2） | Z |
| `bios_glyph(code, &found)` | BIOS_GLYPH（版 2） | 字形の 1 行目の番地。`found` に ROM にあれば 1 |
| `bios_version()` | BIOS_INFO_VERSION を読む | 約束の版 |

- Z の出力は版 2 の約束。版 1 の BIOS では返す値に頼らない。版 2 の呼出しを使う前に `bios_version() >= 2` を確かめる（`bios-abi.md` §11）。
- BIOS の変数は同じ名前の左辺値（`bios_frame`、`bios_pad_a`、`bios_vbl_hook` など。volatile）。書いてよいのは `bios-abi.md` §03 の表で「書いてよい」としたものだけ。
- **VBlank のフック**：`bios_vbl_hook = 関数;`。BIOS は JSR で呼び、D0・D1・A0・A1 を壊してよいので、ふつうの C の関数でよい。割込みの中なので短くする。
- **割込みの項目**：`fc85_set_vector(ベクタ, 関数)` がジャンプ表（DR-025）の飛び先を割込みを止めて書き換え、前の飛び先を返す。関数には `__attribute__((interrupt_handler))` を付ける（使うレジスタを保ち、RTE で戻る）。DOC の割込み（ベクタ 28）を使うときは IE を立てる前に替える（`bios-abi.md` §03）。

## 08　レジスタ

`fc85.h`（生成物。`docs/spec/registers.toml` が正）の `REG_名前` は volatile の左辺値。

- 16 bit のレジスタは語で読み書きする（バイトのアクセスはバスエラー）。`REG_名前` は語の型なので、ふつうに読み書きすれば語になる。試験の ROM でビットの検査と 0 の書込みを確かめた [V]（GCC 11.4。§11）。
- 0 を書く `REG_ACRTC_AR = 0;` は MOVE になり、CLR（書く前に読む）にならない（GCC は 68000 の volatile のメモリに CLR を使わない）。試験が逆アセンブルで確かめる。
- ACRTC の AR/SR とデータポート、DOC の窓には `|=`・`&=`・`^=` を使わない。C でも「読んでから書く」になり、読むことに意味がある窓では規約に反する（`guarantees.md` §03）。値を作ってから `=` で書く。
- 8 bit の装置（DOC、ACIA、文字 ROM）は `REG_DOC_…(n)` などの 1 byte の左辺値（上位バイトレーンの偶数番地）。DOC の窓は 1 回の読みか書きだけ（DR-030）。
- ACRTC の FIFO は SR の WFR・RFR を見てから読み書きする（DR-096）。待ちの数や命令の速さに頼らない。§09 の関数はこれを中で守る。

## 09　周辺の関数

`acrtc.h`・`fabric.h`・`doc.h`・`adpcm.h`（DR-106）と `video.h`・`dsp.h`（DR-109）、`temporal.h`（DR-113）（中身は `sdk/c/` の同じ名前の .c。HBlank の割込みの入口は `video_irq.S`）[P]。組立ての道具が実行時の関数と一緒に組み、使わない関数はリンクで落ちる。`guarantees.md` §03 の規約のうち、手順で守れるものは関数の中で守る。手順は Boot ROM（ACRTC。`bios-abi.md` §05）とサンプル（Fabric・DOC・ADPCM。`sdk/samples/game`）で確かめたものと同じ。どの関数も再入できない（割込みの処理と、割り込まれた側の両方から同じ装置を使わない）。

```c
#include "acrtc.h"
#include "adpcm.h"
#include "fabric.h"

acrtc_color(12);                                  /* 色 12 で */
acrtc_fill(0, 176, 255, 191, ACRTC_OPM_REP);      /* 下の 16 行を塗る */
fab_to_staging(0x4000, s_shot, sizeof s_shot);    /* ADPCM のデータを Staging RAM へ */
adpcm_init(0);                                    /* Fabric のチャネル 0 を ADPCM に */
adpcm_play(0x4000, sizeof s_shot, ADPCM_CTL_8K);  /* 鳴らす。毎フレーム adpcm_update() を呼ぶ */
```

### ACRTC（`acrtc.h`）

| 関数 | すること | 返す値 |
|---|---|---|
| `acrtc_write_reg(reg, v)`・`acrtc_read_reg(reg)` | 制御のレジスタ（`ACRTC_CCR`・`ACRTC_OMR`・`ACRTC_DCR`、r80 から先）を読み書きする | 読んだ値 |
| `acrtc_put(w)` | 書込み FIFO に 1 語（WFR を待つ） | 1。打ち切ったら 0（以下同じ） |
| `acrtc_send(words, count)` | 語の列を送る。WFE（空）なら 8 語を続けて、WFR（空きあり）なら 1 語を書く | 1 か 0 |
| `acrtc_get(&w)` | 読出し FIFO から 1 語（RFR を待つ） | 1 か 0 |
| `acrtc_wait()` | 送ったコマンドが全部終わるまで待つ（WFE と CED） | 1 か 0 |
| `acrtc_abort()` | CCR の ABT で打ち切る（FIFO も空になる。CCR のほかのビットは保つ） | なし |
| `acrtc_color(c)` | 描く色（CL0・CL1）をパレットの番号 c にする | 1 か 0 |
| `acrtc_dot(x, y, op)` | 点 | 1 か 0 |
| `acrtc_line(x0, y0, x1, y1, op)` | 線。終わりの点は描かない | 1 か 0 |
| `acrtc_box(x0, y0, x1, y1, op)` | 長方形の枠 | 1 か 0 |
| `acrtc_fill(x0, y0, x1, y1, op)` | 長方形を塗る。両端を含む | 1 か 0 |
| `acrtc_circle(x, y, r, op)` | 円 | 1 か 0 |
| `acrtc_dma(on)` | CCR の DDM を立てる・下ろす。ACRTC の仕事が終わるのを待ってから書き、CCR のほかのビットは保つ | 1 か 0 |
| `acrtc_rwp(addr)` | RWP を基本画面の語番地 addr にする | 1 か 0 |
| `acrtc_read_rect(addr, words, rows)` | 語番地 addr から横 words 語・縦 rows 行（下へ）を読む DRD を送る | 1 か 0（words・rows が 0 でも 0） |
| `acrtc_write_rect(addr, words, rows, cmd)` | 同じ形の矩形に書く。cmd は `ACRTC_DWT` か `ACRTC_DMOD \| ACRTC_OPM_*` | 1 か 0 |

- AR は r00（FIFO）で始め、r00 に戻す。BIOS もカートリッジの入口と ACRTC の呼出しの後で r00 にする（`bios-abi.md` §04・§05）ので、BIOS の呼出しと同じ流れで混ぜてよい。AR を自分で替えたら、`ACRTC_FIFO` に戻してから呼ぶ。
- 待つ間に SR の CER（コマンドの誤り）が立てば、CCR の ABT で打ち切り、残りの語を送らずに 0 を返す。BIOS の版 2 と違い、`bios_acrtc_aborts` は数えない。
- 待つ上限はない。BIOS の 0.4 秒の見張り（`guarantees.md` §03 の 35）がないので、長くかかる正しいコマンドも送れる。パラメタの足りない列を送ると `acrtc_wait` は戻らない。
- Data Fabric の port 1 で DMA している間は呼ばない（§03 の 8）。
- 図形の関数は、BIOS が入口で残す描画の状態（ORG が画面の左上、パターン RAM が 0、MASK が $FFFF。`bios-abi.md` §06）を前提にする。座標は画面座標（x は右、y は下）で、関数が AMOVE と論理座標の y の符号を扱う。`op` はコマンドの下位 8 bit（演算 `ACRTC_OPM_*`、色の選び `ACRTC_COL_*`、領域の指定）で、0 は「置き換え、パターンの色（パターン RAM が 0 なら CL0）」。終わりを待たないので、続けて描くならそのまま次を呼ぶ。
- 矩形の読み書きと DMA（DR-112）：CCR の DDM を立てると、DRD・DWT・DMOD のデータは FIFO を通らず、Data Fabric の port 1 で運ぶ。手順は SAMPLE 3（`sdk/samples/patches` の TEMPORAL FEEDBACK）と同じで、`acrtc_dma(1)` → `acrtc_read_rect` か `acrtc_write_rect` → Fabric のチャネルで port 1 から（へ）words×rows 語（`fab_run`。FORMAT は 16 bit の `FMT_16_16` など）→ `acrtc_dma(0)`。語番地は `ACRTC_WORD_ADDR(x, y)`（1 行 64 語、1 語に 4 画素で左の画素が下位）。語は行ごとに左から並ぶ。DDM の間は BIOS の字の呼出し（`bios_puts`・`bios_put_hex` など）を使わない（BIOS は DWT のデータを FIFO で書く。`bios-abi.md` §06）。DDM＝0 なら、データは `acrtc_get`・`acrtc_send` で読み書きする。
- Temporal RAM（port 6 に書き、port 7 から読む）に絵を残すのも同じ形で、Fabric の相手を `PORT_TEMPORAL_W`・`PORT_TEMPORAL_R` にし、ポインタと制御は `temporal.h`（下）で扱う（SAMPLE 3、DR-067）。Temporal RAM は 8 bit なので、ACRTC の語は `FMT_16_8_HI`・`FMT_16_8_LO` で byte にして書き、`FMT_8_16` と SHIFT で語に戻す。

```c
acrtc_dma(1);
acrtc_read_rect(ACRTC_WORD_ADDR(0, 64), 16, 32);  /* (0, 64) から 64×32 画素を読む */
struct fab_xfer x = {.src = PORT_ACRTC, .dst = PORT_STAGING, .dst_addr = 0x8000,
                     .count = 16 * 32, .dst_step = 2, .format = FMT_16_16, .trigger = TRG_NOW};
fab_run(0, &x);                                   /* Staging RAM の $8000〜へ */
acrtc_dma(0);
```

### Data Fabric（`fabric.h`）

転送は構造体 `struct fab_xfer` で書く：ポート（`src`・`dst`）、番地（`src_addr`・`dst_addr`。port 0 は Staging RAM の byte 番地、port 2 は波形 RAM の byte 番地）、要素の数 `count`、番地に足す数（`src_step`・`dst_step`）、`format`（`FMT_*`）、`trigger`（`TRG_*`）、消すビット `clear`（MASK に ~clear を書く）、反転するビット `flip`（XOR）、回転 `shift`。0 で埋めた形は「FORMAT 0、即時、ブロック、変換しない」。

| 関数 | すること | 返す値 |
|---|---|---|
| `fab_set(ch, &x)` | チャネルを止め、設定のレジスタを書く（TRIGGER を最初に） | なし |
| `fab_start(ch)` | DONE・ERROR・MISS を消して START | なし |
| `fab_wait(ch)` | DONE か ERROR が立つまで待つ | STATUS |
| `fab_run(ch, &x)` | 設定・START・待ち・止めをまとめて | 待った STATUS |
| `fab_stop(ch)` | STOP、BUSY が下りるまで待ち、DONE・ERROR・MISS を消す | なし |
| `fab_status(ch)` | STATUS を読む | STATUS（`FAB_ST_*`） |
| `fab_to_staging(addr, src, n)` | CPU が src から n byte を Staging RAM の byte 番地 addr に写す | なし |

- 関数が守る：設定のレジスタは止めてから書く、START の前に W1C、TRIGGER を書いてから START まで 4 スロット以上（`fab_set` は TRIGGER の後に設定のレジスタを 10 個書く）（`guarantees.md` §03 の 22〜24）。
- 呼ぶ側が守る：実装されていないポート（`bios_modules`・`FABRIC_MODULES`）と、FORMAT に合わない向き・幅を使わない（§03 の 25。START で ERROR になる）。`TRG_LOOP` のチャネルを `fab_wait`・`fab_run` で待たない（終わらない）。ADPCM に渡したチャネル（`adpcm_init`）は使わない。
- Fabric はカートリッジを読めない。カートリッジのデータは、先に `fab_to_staging` で Staging RAM に写す。

### DOC（`doc.h`）

| 関数 | すること | 返す値 |
|---|---|---|
| `doc_present()` | 子基板があるか（`bios_modules` の `FABRIC_MOD_DOC`） | 1 か 0 |
| `doc_oscillators()` | 走査しているオシレータの数（0xE1 を読む） | 1〜32 |
| `doc_set_oscillators(n)` | 走査の数を n にする。増やすオシレータを H＝1・IE＝0・CA 15 にしてから、割込みを止めて続けて 0xE1 を書く（§03 の 15） | なし |
| `doc_set(osc, &o)` | `struct doc_osc` の周波数、音量、波形の頁、大きさを書き、最後に制御を書く | なし |
| `doc_freq(osc, f)`・`doc_volume(osc, v)` | 周波数・音量だけを替える | なし |
| `doc_halt(osc)`・`doc_go(osc)` | 制御を読み、H だけを替えて書く | なし |
| `doc_load_wave(ch, wave, staging, bytes)` | Staging RAM から波形 RAM へ、Fabric のチャネル ch で運ぶ（port 0 → port 2、FORMAT 0）。終わるまで待つ | STATUS |

- 子基板がないと DOC の窓はバスエラー（§03 の 3）。`doc_present()` が 1 のときだけほかの関数を呼ぶ。`doc_load_wave` は自分で確かめ、なければ何もせず `FAB_ST_ERROR` を返す（port 2 は本体 DAC の FIFO になっているため）。
- 窓は MOVE.B の 1 回の読みか書きだけで触る。0xE0・0xE2 は読まない（§03 の 12・13）。
- 鳴らさないオシレータは、鳴らすものと別のチャンネル（CA）に置く。波形の値は $01〜$FF（0 はオシレータを止める印）（§03 の 16・17）。
- 制御に IE を立てるなら、先にジャンプ表のベクタ 28 を替える（`guarantees.md` §03 の 14。替え方は §07 の `fc85_set_vector`）。
- `doc_halt`・`doc_go` は読みと書きの 2 回のアクセスで、その間に DOC が制御を変えることがある（ワンショットの終わりで H が立つ、スワップ）。`doc_go` はそれでも動かす。

### ADPCM（`adpcm.h`）

| 関数 | すること | 返す値 |
|---|---|---|
| `adpcm_init(ch)` | Fabric のチャネル ch を ADPCM に使う。RESET にし、FIFO を空にする | なし |
| `adpcm_play(staging, bytes, ctrl)` | Staging RAM の byte 番地 staging から bytes 個を鳴らす求め。鳴っている音は替わる。bytes が 0 なら止める | なし |
| `adpcm_stop()` | 止める求め | なし |
| `adpcm_update()` | 1 フレームに 1 回呼ぶ | なし |
| `adpcm_busy()` | 鳴っているか、鳴らす途中か、求めがあるか | 1 か 0 |

- 手順は `guarantees.md` §03 の 19（DR-060）で、サンプルの効果音（`sdk/samples/game/sfx.s`）と同じ：求めの次の `adpcm_update` で RESET をかけ、その次で START し、FIFO に 16 byte たまったら RESET を外す。DONE の後に FIFO が空になったら止めて RESET に戻す。求めから音が出るまで 1〜2 フレーム。鳴っている音を替えるときは、先にチャネルを止めて FIFO を空にし、1 フレーム置いて RESET をかけるので、もう 1 フレームかかる。
- データは 4 bit の ADPCM で、1 byte に 2 ニブル（既定は上位から）。鳴る時間は bytes×2÷標本化の周波数。
- `ctrl` は標本化（`ADPCM_CTL_8K`・`6K`・`4K`）と並び（`ADPCM_CTL_LOW_FIRST`）。ほかのビットは落とす。S1・S2 を両方 1 にすると S2 を落とす（両方 1 は Bend の操作。§03 の 20）。
- `adpcm_play` と `adpcm_update` は同じ流れ（どちらも割込みの外か、どちらも同じ割込みの処理）から呼ぶ。

### 映像（`video.h`）

| 関数 | すること | 返す値 |
|---|---|---|
| `video_set_color(n, rgb)` | パレットの色 n（0〜15）を `$0RGB` にする（裏のパレット） | なし |
| `video_set_palette(rgb, first, count)` | パレットの first から count 語を書く | なし |
| `video_direct(on)` | `PAL_CTRL` の DIRECT（Bend）。1 なら書いた色が表へ直接入り、ラインの途中から効く | なし |
| `video_line()` | 今のラスタ（VSYNC の始まりから。`VIDEO_LINE`） | 番号 |
| `video_hblank(hook, line)` | HBlank の割込みを始める。line が 0 以上ならそのラスタだけ（`LINE_CMP`）、負なら毎ライン。割込みのたびに hook を呼ぶ | なし |
| `video_hblank_at(line)` | 比べるラスタを替える（hook の中で次の区切りを決める） | なし |
| `video_hblank_off()` | HBlank の割込みを止め、ベクタ 30 を前の処理に戻す | なし |

- パレットは裏の 16 語で、表示は HSYNC の始まりに表へ写した値を使う（2CLK に 1 語ずつ。DR-027、DR-069）。書いた色は、写しが通った次のラインから効く。
- HBlank の割込みはレベル 6 で、VBlank（BIOS の処理）と同じ線。`video_hblank` はジャンプ表のベクタ 30 に小さな入口（`video_irq.S`）を置く。入口は HBL の保留だけを消し（許可のビットはそのまま）、hook を呼び、BIOS の処理へつなぐ。BIOS は VBlank を受けて `bios_frame` を進めるので、`bios_wait_vbl` もそのまま使える（`bios-abi.md` の「処理を差し替える」）。hook は割込みの中（マスク 6）で呼ばれる C の関数。短くし、ほかの装置の関数（ACRTC の FIFO など）を割り込まれた側と取り合わない。
- ラスタの番号は VSYNC の始まりから。BIOS の案B の画面では、画面の y＝0 はラスタ `VIDEO_TOP_RASTER`（43）で、1 フレームは `VIDEO_RASTERS`（262）ラスタ。
- ラスタ L の HBlank の割込みで書いた色は、ラスタ L＋2 から効く [V]：HBlank の始まりから次のラインの写しまでは 22 ドット（約 4 µs）で、68000 の割込みの応答が間に合わない。画面の y から色を替えたいなら、`VIDEO_TOP_RASTER + y − 2` で比べる。1 フレームに何か所も替えるときは、hook の中で `video_hblank_at` で次のラスタを決め、VBlank のフック（`bios_vbl_hook`）で最初の色と最初のラスタに戻す。
- DIRECT（`video_direct(1)`。Bend、DR-027）では、書いた色が表へ直接入るので、ラスタ L の HBlank の割込みで書いた色は、ラスタ L＋1 の途中から効く [V]（試験では x＝145 の辺り。割り込まれた命令で数ドット動く）。書込みのストローブの間は、そのドットが書いた色になる（点状のノイズ）。DIRECT の間は写しが止まり、書いた色は裏に入らないので、DIRECT を解くと次のラインから裏の色に戻る [P]。

```c
static void split(void)                           /* ラスタ 103 の HBlank */
{
    video_set_color(0, 0x0F00);                   /* 画面の y＝62 から背景を赤に */
}
static void top(void) { video_set_color(0, 0x000F); }
bios_vbl_hook = top;                              /* VBlank で青に戻す */
video_hblank(split, VIDEO_TOP_RASTER + 60);
```

C で書いた手本は SAMPLE 7（`sdk/samples/raster`。8 ラスタごとの帯、DIRECT の入切、DOC の音。DR-111）。

### Temporal RAM（`temporal.h`）

| 関数 | すること | 返す値 |
|---|---|---|
| `tmp_reset(mode)` | 両方のポインタと印を 0 にし、モードを mode にする（`TMP_SYNC`・`TMP_RSYNC`・`TMP_HOLD` の OR） | なし |
| `tmp_mode(mode)` | モードだけを替える（ポインタと印はそのまま） | なし |
| `tmp_get_mode()` | 今のモード | ビット |
| `tmp_seek(wptr, rptr)` | ポインタを置く | なし |
| `tmp_wptr()`・`tmp_rptr()`・`tmp_dist()` | 書込み・読出しのポインタ、WPTR−RPTR（遅延の byte 数） | 値 |
| `tmp_flags()` | 印（`TMP_EQUAL`・`TMP_OVERRUN`・`TMP_UNDERRUN`） | ビット |
| `tmp_clear_flags()` | OVERRUN・UNDERRUN を消す（モードはそのまま） | なし |

- Temporal RAM は 64 KiB の 8 bit の SRAM。ポインタは byte 番地で、Fabric が 1 要素を運ぶたびに 1 進み、64 KiB で回る。遅延は DIST で、読み書きの速さの差は Fabric のチャネルの速さで作る（DR-067）。
- 追越しは止めず、印だけを立てる：書込みで WPTR＝RPTR になったら OVERRUN、読出しで一致したら UNDERRUN。ずれた履歴を混ぜる表現（Temporal feedback）のために、わざと起こしてもよい。書いていない番地は電源投入時の中身（決まらない）を読む。
- CTRL の書込みはモードを毎回決め直す（CLEAR は 1 回の動作、OVERRUN・UNDERRUN は W1C）。関数は値を作って 1 回で書く。`TMP_SYNC` は VBlank の始まりで両方のポインタを、`TMP_RSYNC` は読出しのポインタだけを 0 にし（書いた 1 フィールドを毎フィールド繰り返す）、`TMP_HOLD` は書込みを止める（ポインタは進む。フィールドの保持）。
- C で書いた手本は SAMPLE 8（`sdk/samples/echo`。矩形を 1 語 2 byte で記録し、d・2d・3d 回前を DWT と DMOD の OR で描き戻す。HOLD。DR-114）。

### DSP（`dsp.h`）

| 関数 | すること | 返す値 |
|---|---|---|
| `dsp_load(code, words, zero_rest)` | DSP を RESET にし、PRGBUS を待ち、プログラム窓の 0 番地から code の words 語を書く。zero_rest なら残りを 0 にする。書いた範囲を読み返す。DSP は RESET のまま | 違った語の数（0 で正しい）。PRGBUS が来なければ −1 |
| `dsp_start(ctrl)` | RESET を解いて 0 番地から動かす。ctrl は BIO の選び方（`DSP_CTRL_BIO_*`）と `DSP_CTRL_IRQ_EN` | なし |
| `dsp_reset()` | RESET にする（プログラム窓が開く） | なし |
| `dsp_command(cmd, reply, tries)` | メールボックスに cmd を書き、応答を待って reply に入れる。tries 回（1 回 約 3〜5 µs）見る | 応答が来たら 1、時間切れなら 0 |
| `dsp_status()` | `DSP_STATUS` | 値 |
| `dsp_clear_flags()` | UNDERRUN・OVERRUN の印を消す（W1C） | なし |

- 手順はサンプル（`sdk/samples/dsp/main.s`）と同じ（DR-029）。PRGBUS＝0 の間にプログラム窓に触るとバスエラーなので、`dsp_load` が待つ。音や値の流れは Data Fabric の port 3（入力 FIFO）と port 4（出力 FIFO）で、`fabric.h` で組む。
- DSP のプログラムは dspasm で組み、`--c-array 名前` で C の見出しにする：`py sdk/dspasm/dspasm.py prog.dsp -o prog.h --c-array prog -I sdk/include` は `static const uint16_t prog[]` と語数 `prog_WORDS` を書く。`#include "prog.h"` して `dsp_load(prog, prog_WORDS, 1)`。
- メールボックスの約束（コマンドと応答の意味）は ROM と DSP のプログラムで決める。DSP の側は `IN x,DSP_PA_MAILBOX` で読み、`OUT x,DSP_PA_RESPONSE` で答える（`sdk/include/dsp.inc`。試験の `sdk/c/tests/echo.dsp` が小さな例）。

## 10　組立ての出力と調べ方

`build.py` は `build/` に次を書く。

| ファイル | 中身 |
|---|---|
| `rom.bin` | カートリッジの像（$400000 から） |
| `rom.lst` | 記号の表（関数、大域と static の変数、リンカの記号）。窓口の `load` が横の `.lst` として読むので、`peek`・`break`・`watch` にラベルを使える。関数の中の変数は入らない |
| `rom.elf` | GCC の出力（デバッグの情報つき） |
| `rom.map` | リンカの配置 |
| `rom.dis` | 逆アセンブル。C の行と並べる。`regs` の PC や `trace` の番地を引く |

ひな形の `test_rom.py` はラベルで RAM を読むので、C でもアセンブラでも同じ試験が通る。

## 11　頼ってよいこと・いけないこと

- 頼ってよい [P]：§03 の配置、§04 の見出し、§05 の関数の結果（`sdk/c/test_c.py` が Python の計算と突き合わせる。32 bit 48 組、64 bit 28 組）、§07 の関数が BIOS の約束どおりにレジスタを渡すこと、§09 の関数が規約の手順を守ること（`sdk/c/test_devices.py` が、逆アセンブルで I/O の窓に読み書き一体の命令がないことを、エミュレータで VRAM・Staging RAM・波形 RAM・DOC・音・HBlank で替えた色の区切り・DSP のメールボックスの往復・ACRTC の DMA の読み書き・Temporal RAM の往復を確かめる）。
- 頼ってはいけない：GCC が出す命令の並びと時間。GCC の版や最適化（`-Os`、`-O2`）で変わる。時間が要るところは `VIDEO_LINE` か `bios_frame` で測るか、アセンブラ（`.S`）で書く（`guarantees.md` §05）。
- 確かめた GCC：Ubuntu 22.04 の `gcc-m68k-linux-gnu`（GCC 11.4）[V]。ほかの版と `m68k-elf` はまだ試していない。

## 12　まだないもの

- 浮動小数点、`printf` 相当、ヒープ [O]。
- 周辺の関数は §09 でひととおりそろった（パレットと HBlank の割込み・DIRECT、DSP は v0.3、DR-109。ACRTC の矩形の読み書きと DMA は v0.5、DR-112。Temporal RAM は v0.6、DR-113）。Capture RAM（port 8）は機械の側が [O]（仕様書 §05）なので、関数もない。外の入出力（音声の入力、MIDI）はまだ扱わない。
- C で書いた、DSP と DOC の音楽を見せるサンプル [O]。C で書いたサンプルは、v0.4 で HBlank のラスタのパレットと DIRECT の SAMPLE 7（`sdk/samples/raster`、DR-111）を、v0.7 で ACRTC の DMA と Temporal RAM の残像の SAMPLE 8（`sdk/samples/echo`、DR-114）を足した。
- エミュレータを C から使う窓口（C の ABI。DR-092 (4)）は別のもの。
