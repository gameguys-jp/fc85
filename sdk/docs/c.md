# C で ROM を書く

**v0.1**  
2026年11月27日

> fc85 のカートリッジを C で書くための手引き（DR-103）。GCC は外の道具として使い、SDK には入れない（DR-092 (3)）。SDK が持つのは、起動のコード、リンカの設定、小さな実行時ライブラリ、BIOS を呼ぶ関数、組立ての道具。番地とレジスタは `sdk/include/fc85.h`（生成物。DR-100）、BIOS は `sdk/c/include/bios.h`、BIOS の約束は `bios-abi.md`、ROM が守ることは `guarantees.md` が正。アセンブラ（m68kasm）で書く ROM もそのまま使える。

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

`m68k-linux-gnu` は Linux 向けの GCC だが、組立ての道具が 68000 の命令だけ（`-m68000`）、OS なし（`-ffreestanding`）、位置固定（`-fno-pie`）、スタック保護なしで呼ぶ。GCC の実行時ライブラリ（libgcc）と C の標準ライブラリはつながない。Linux 向けの libgcc は 68020 向けに組まれていて、68000 にない命令（MULU.L、DIVU.L など）を含むため（逆アセンブルで確かめた）。

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

- **ヘッダ**：GCC の OS なしのヘッダ（`stdint.h`、`stddef.h`、`stdbool.h`、`stdarg.h`、`stdalign.h`、`stdnoreturn.h`、`iso646.h`）と、SDK の `limits.h`、`string.h`、`bios.h`、`fc85.h`。`stdio.h`・`stdlib.h` などはない（インクルードで止まる）。
- **string.h**：`memcpy`、`memmove`、`memset`、`memcmp`、`strlen`、`strcmp`、`strcpy`（`sdk/c/string.c`）。両方が偶数番地なら長語ずつ写す。Main RAM と Staging RAM だけに使う（MMIO の窓には幅と回数が決まらないので使わない）。
- **整数**：32 bit の掛け算・割り算（`sdk/c/runtime.S`）と 64 bit（`long long`）のシフト・掛け算・割り算（`sdk/c/runtime64.c`）は、GCC が呼ぶ関数を SDK が持つ。名前は GCC の実行時ライブラリと同じで、中身はここで書いた。16×16 の掛け算と 32÷16 の割り算は 68000 の命令（MULU・DIVU）になる。64 bit の割り算は 1 bit ずつ引くので遅い。
- **0 で割る**：DIVU の 0 除算の例外（ベクタ 5）になり、BIOS が赤い画面で止まる（`bios-abi.md` §03）。
- **ないもの**：浮動小数点（`float`・`double` の演算はリンクで止まる）、`malloc`、`printf`、C++、起動時の構築子。固定小数点の整数で書く [P]。

## 06　呼び方とアセンブラ

GCC の m68k の呼び方 [D]：

- 引数はスタックに 4 byte ずつ積む（`int` と `short` はどちらも 4 byte。値は下位の語が +2）。
- 結果は D0（64 bit は D0:D1。ポインタは D0 と A0）。
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

- 16 bit のレジスタは語で読み書きする（バイトのアクセスはバスエラー）。`REG_名前` は語の型なので、ふつうに読み書きすれば語になる。試験の ROM でビットの検査と 0 の書込みを確かめた [V]（GCC 11.4。§10）。
- 0 を書く `REG_ACRTC_AR = 0;` は MOVE になり、CLR（書く前に読む）にならない（GCC は 68000 の volatile のメモリに CLR を使わない）。試験が逆アセンブルで確かめる。
- ACRTC の AR/SR とデータポート、DOC の窓には `|=`・`&=`・`^=` を使わない。C でも「読んでから書く」になり、読むことに意味がある窓では規約に反する（`guarantees.md` §03）。値を作ってから `=` で書く。
- 8 bit の装置（DOC、ACIA、文字 ROM）は `REG_DOC_…(n)` などの 1 byte の左辺値（上位バイトレーンの偶数番地）。DOC の窓は 1 回の読みか書きだけ（DR-030）。
- ACRTC の FIFO は SR の WFR・RFR を見てから読み書きする（DR-096）。待ちの数や命令の速さに頼らない。

## 09　組立ての出力と調べ方

`build.py` は `build/` に次を書く。

| ファイル | 中身 |
|---|---|
| `rom.bin` | カートリッジの像（$400000 から） |
| `rom.lst` | 記号の表（関数、大域と static の変数、リンカの記号）。窓口の `load` が横の `.lst` として読むので、`peek`・`break`・`watch` にラベルを使える。関数の中の変数は入らない |
| `rom.elf` | GCC の出力（デバッグの情報つき） |
| `rom.map` | リンカの配置 |
| `rom.dis` | 逆アセンブル。C の行と並べる。`regs` の PC や `trace` の番地を引く |

ひな形の `test_rom.py` はラベルで RAM を読むので、C でもアセンブラでも同じ試験が通る。

## 10　頼ってよいこと・いけないこと

- 頼ってよい [P]：§03 の配置、§04 の見出し、§05 の関数の結果（`sdk/c/test_c.py` が Python の計算と突き合わせる。32 bit 48 組、64 bit 28 組）、§07 の関数が BIOS の約束どおりにレジスタを渡すこと。
- 頼ってはいけない：GCC が出す命令の並びと時間。GCC の版や最適化（`-Os`、`-O2`）で変わる。時間が要るところは `VIDEO_LINE` か `bios_frame` で測るか、アセンブラ（`.S`）で書く（`guarantees.md` §05）。
- 確かめた GCC：Ubuntu 22.04 の `gcc-m68k-linux-gnu`（GCC 11.4）[V]。ほかの版と `m68k-elf` はまだ試していない。

## 11　まだないもの

- 浮動小数点、`printf` 相当、ヒープ [O]。
- ACRTC のコマンド、Data Fabric、DOC、ADPCM を C から使う関数 [O]。今は `fc85.h` のレジスタと `bios_acrtc_send` で書き、サンプル（`sdk/samples/`、アセンブラ）を手本にする。
- エミュレータを C から使う窓口（C の ABI。DR-092 (4)）は別のもの。
