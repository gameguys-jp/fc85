# SAMPLE 2：DSP（TMS32010）のサンプル

68000 から DSP を動かし、音を加工して鳴らすまでを、一通りソフトウェアで通すサンプル。

記述ステータスは仕様書と同じ：`[D]` 仕様書・台帳・DR に根拠がある、`[P]` このサンプルの暫定の決めごと（エージェントが決めた。要見直し）、`[O]` 代わりの案、`[V]` 要検証。

## 01　概要

起動すると、次のことをする。

1. 試験の音を Staging RAM に写す。音は `build.py` が合成する。ノコギリ波のはじく音 4 つと雑音 2 つで、間に無音を置く。16 kHz・8 bit の符号なしで 2 秒、外の音源は使わない [P]。
2. DSP をリセットし、プログラム（`effects.dsp` を dspasm で組んだ 231 語）を DSP bridge のプログラム窓に書く。読み返して確かめてから走らせる。
3. Data Fabric で音を流す。チャネル 0 が Staging RAM → DSP の入力 FIFO、チャネル 1 が DSP の出力 FIFO → 音の出口。
4. DSP は 1 標本ごとに 3 つの出力を計算し、モードで選んだ 1 つを出す。
   - THRU：そのまま
   - LPF：31 タップの FIR の低域通過（遮断 1.2 kHz）
   - ECHO：遅れ 3072 標本（16 kHz で 192 ms）、帰還 1/2 のエコー
5. A・B・C でモード（THRU・LPF・ECHO）を替える。DSP が生きていることは画面で分かる。標本の数・検査和・山の値をメールボックスで毎フレーム読み、数字と水準計で出す。

音の出口は、DOC の子基板の有無で替わる（§04）。

| 子基板 | 出口 | 標本化 |
|---|---|---|
| ない | 本体 DAC の FIFO（port 2） | 16 kHz（AUDIO_N＝500） |
| ある | DOC の波形 RAM の 256 byte の輪（port 2）を、オシレータ 0・1 が読む | 15.625 kHz（AUDIO_N＝512） |

画面の項目は次のとおり。

| 行 | 中身 |
|---|---|
| MODE | 今のモード。DSP が答えないときは `DSP NG` |
| COUNT | DSP が処理した標本の数（32 bit、16 進）。8 フレームごとに描き直す |
| SUM | COUNT と同じ写しの検査和。出力の語（u·257）の和の下位 16 bit |
| PEAK | 前のフレームからの \|y\| の最大（Q15）。下の水準計の幅は PEAK÷128（240 まで） |
| SW | 最後にモードを替えた標本の番号の下位 16 bit（`CMD_MODE` の応答） |
| LOAD | 書いた語数、読み返しで違った語数（0 でなければ赤）。`ID` は `RD_MAGIC` の応答（$3210） |
| FLAG | 4 桁。千の位は DSP の入力の UNDERRUN、百の位は出力の OVERRUN、十の位は本体 DAC の UNDERRUN、一の位は応答の時間切れの数。0 以外は赤 |
| ROUTE | 音の出口（`DAC 16000 HZ` か `DOC 15625 HZ`） |

## 02　組み方と動かし方

ファームウェアとサンプルを組む（リポジトリの根で）。

```
py firmware/charrom/build.py
py firmware/boot/build.py
py sdk/samples/dsp/build.py
```

`build/` に次のものができる（リポジトリには入れない）。

- `dsp.bin`・`dsp.lst`：カートリッジ
- `effects.bin`・`effects.lst`：DSP のプログラム。リストにはサイクル数が付く
- `fir.inc`：FIR の係数
- `source.bin`・`source.wav`：試験の音

画面付きで動かすときは、`emulator/` で次を実行する。

```
cargo run --release -p fc85-host -- --no-doc --cart ../sdk/samples/dsp/build/dsp.bin
```

`--no-doc` を外すと DOC の子基板がある構成になる。

画面なしで動かし、WAV と画面を書き出すには次のようにする。入力の台本 `pad-demo.txt` は UTF-8 で書く。台本では、120 フレームに B、240 に C、360 に A、480 に B を押す。

```
cargo run --release -p fc85-host -- --headless --no-doc --cart ../sdk/samples/dsp/build/dsp.bin \
    --frames 600 --pad-script ../sdk/samples/dsp/pad-demo.txt \
    --dump <scratchpad>/d.ppm --dump-at 60,200,300,400,600 --wav <scratchpad>/d.wav
```

WAV と画面の数をモデルと比べるには、`verify_wav.py` に画面の SW・COUNT・SUM を写して渡す（§08）。

```
py sdk/samples/dsp/verify_wav.py <scratchpad>/d.wav --sw 0x76BB --count 0x31E0 --sum 0xE4F7 ...
py sdk/samples/dsp/verify_wav.py <scratchpad>/e.wav --doc ...
```

試験は `py -m unittest discover -s sdk/samples/dsp` で動かす。見出しの形、DSP のプログラムの置き場所、FIR、モデルの値の範囲、1 標本のサイクル数を確かめる。dspasm の試験は `py -m unittest discover -s sdk/dspasm`。

## 03　ファイル

| ファイル | 中身 |
|---|---|
| `main.s` | 68000 の側（m68kasm）。読込み、Data Fabric の道、メールボックス、画面 |
| `effects.dsp` | DSP の側（dspasm）。3 つの効果と、コマンドの処理 |
| `protocol.inc` | 68000 と DSP の約束。両方のアセンブラが INCLUDE する |
| `../../include/dsp.inc` | DSP から見た I/O の名前（dspasm 用） |
| `build.py` | 試験の音の合成、FIR の設計、2 つのアセンブル |
| `model.py` | DSP のプログラムとビットまで同じ Python のモデル |
| `verify_wav.py` | エミュレータの WAV と画面の数を、モデルと標本ごとに比べる |
| `test_build.py` | 試験 |
| `pad-demo.txt` | `--pad-script` の台本 |

## 04　音の道（Data Fabric）

**子基板がない（本体 DAC）**

```
Staging RAM ──ch0──▶ port 3 ──▶ DSP ──▶ port 4 ──ch1──▶ port 2（本体 DAC の FIFO）──▶ 16 kHz の拍で 1 語
 8 bit・LOOP   8→16、<<8、^$8000   入力 FIFO   出力 FIFO   16→16、LOOP、STALL で待つ
```

- 歩調を決めるのは本体 DAC だけ。DAC は拍（8 MHz÷500＝16 kHz）ごとに FIFO から 1 語取る [D 仕様書 §07、DR-019]。DAC の FIFO が満杯なら ch1 が STALL で待つ。DSP の出力 FIFO も満杯になり、DSP は空きを待つ。DSP が入力を読まないので入力 FIFO が満杯になり、ch0 が待つ。
- 待つのはすべて FIFO と STALL で、ゼロ時間の循環はない [D DR-017]。
- ch0 と ch1 は即時（`TRG_NOW`）と `TRG_LOOP`。ch0 は 2 秒（32000 byte）の終わりで頭に戻る。ch1 の COUNT（4096）は意味を持たず、LOOP で回り続ける [P]。

**子基板がある（DOC）**

```
Staging RAM ──ch0──▶ port 3 ──▶ DSP ──▶ port 4 ──ch1──▶ port 2（波形 RAM $1000〜$10FF の輪）◀── オシレータ 0（L）・1（R）
                                                16→8（上位 byte）、TRG_AUDIO・EACH・LOOP
```

- ch1 は拍（8 MHz÷512＝15.625 kHz）ごとに 1 要素を書く。DST_STEP＝1、COUNT＝256、LOOP なので、256 byte の輪を回る [D DR-046、DR-054]。
- オシレータ 0・1 は輪の頁 $10 を読む。設定は T＝0（256 byte）、R＝0、FC＝144 の自走。16 個を走査すると標本化は 55.556 kHz で、1 回に 144/512＝9/32 byte 進むので、15.625 kHz で読む [D 台帳 ES5503 §4]。書き手と読み手が同じ 8 MHz から作られるので、間隔は保たれる [P]。
- 輪の初めの中身は $80 で埋める。$00 はオシレータを止める値なので使わない [D]。DSP は u＝0 を 1 にする（§06）。
- 読み手は書き手が 128 標本（輪の半分）進んでから走らせる [P]。
- オシレータ 0 と 1 は、CONTROL への 2 回の書込みで走らせるので、位相が少し違う。L と R は完全には同じにならない [P]。

## 05　レジスタの手順（68000）

番地とビットは `sdk/include/fc85.inc` の名前で書く。（　）は値。

**1. 入口**

- `INITIAL_SSP` を SP に置き、変数を 0 にする。
- `bios_modules` の bit 2（DOC）で道を選ぶ。
- 固定の文字を BIOS_PUTS で描く。

**2. 試験の音を写す**

- カートリッジの `source.bin`（32000 byte）を `STAGING`＋$0000 へ、MOVE.L で写す。
- Fabric はカートリッジを読めないので、CPU が写す。port 0 の 8 bit の読出しは偶数番地が先 [D DR-046]。

**3. DSP のプログラムを書く** [D DR-029 (1)]

1. `DSP_CTRL`（$220000）←$0001（RESET）。
2. `DSP_STATUS`（$220002）の bit 0 PRGBUS が 1 になるまで読む（1000 回まで。来なければ `DSP NG`）。
3. `DSP_PROGRAM`（$280000〜）に `effects.bin` の 231 語を MOVE.W で書く。続く 3865 語（エコーの遅延線）は 0 にする。CLR は読んでから書くので、MOVE で 0 を書く。
4. 4096 語を読み返して比べ、違った数を LOAD に出す。
5. `DSP_CTRL`←$0002 で走らせる。BIOSEL＝1（BIO＝出力 FIFO に空き）、RESET＝0、IRQ_EN＝0。
6. `DSP_MAILBOX`（$220004）←$0305（`CMD_READ`・`RD_MAGIC`）。`DSP_STATUS` の bit 4 RESPONSE を待ち（400 回まで）、`DSP_RESPONSE`（$220006）を読む。$3210 でなければ `DSP NG`。

**4a. 子基板がない：ch1 と本体 DAC**

1. `FABRIC_AUDIO_N`（$250102）←500。
2. ch1（`FABRIC_CH1`＝$250040）を設定する。
   - CTRL←$0002（STOP）、STATUS←$001A（W1C）。
   - `FABRIC_PORT_RESET`（$250100）←$001C で、port 2・3・4 の FIFO を初期化する。
   - SRC←4、DST←2、SRC_ADDR_L←0、DST_ADDR_L←0、COUNT←4096、SRC_STEP←0、DST_STEP←0。
   - FORMAT←1（16→16）、MASK←$FFFF、XOR←0、SHIFT←0、TRIGGER←$0010（NOW・LOOP）。
   - CTRL←$0001（START）。
3. `DAC_CTRL`（$230010）←0。DIRECT を下げ、拍ごとに FIFO から読む [D DR-019]。

**4b. 子基板がある：輪と DOC と ch1**

1. `FABRIC_AUDIO_N`←512。
2. `STAGING`＋$FF00 の 256 byte を $80 で埋める。
3. ch1 で波形 RAM の輪を埋める。
   - CTRL←STOP、STATUS←$001A、`FABRIC_PORT_RESET`←$0018（port 3・4）。
   - SRC←0、DST←2、SRC_ADDR_L←$FF00、DST_ADDR_L←$1000、COUNT←256、SRC_STEP←1、DST_STEP←1。
   - FORMAT←0（8→8）、MASK←$FFFF、XOR←0、SHIFT←0、TRIGGER←0（NOW）、CTRL←START。
   - STATUS の bit 1 DONE を待つ（bit 3 ERROR なら `DSP NG`）。待ち終えたら CTRL←STOP、STATUS←$001A。
4. DOC を設定する。
   - `DOC_OSC_ENABLE`（$2101C2）←30。16 個を走査する（BIOS と同じ値）。
   - オシレータ 0・1 のそれぞれに、VOLUME←0、POINTER←$10、SIZE←$00、FREQ_LO←144、FREQ_HI←0 を書く。
   - CONTROL はオシレータ 0 に $03、1 に $13 を書く（ワンショット＋停止、CA＝0・1）。止めておくと、次に訪れたときにアキュムレータが 0 になる [D 台帳 ES5503]。
5. ch1 を DSP の出口にする。
   - SRC←4、DST←2、SRC_ADDR_L←0、DST_ADDR_L←$1000、COUNT←256、SRC_STEP←0、DST_STEP←1。
   - FORMAT←4（16→上位 8）、TRIGGER←$001B（AUDIO・EACH・LOOP）、CTRL←START。

**5. ch0（両方の道で同じ）**

`FABRIC_CH0`（$250000）を設定する。

- CTRL←STOP、STATUS←$001A。
- SRC←0、DST←3、SRC_ADDR_L←$0000、DST_ADDR_L←0、COUNT←32000、SRC_STEP←1、DST_STEP←0。
- FORMAT←2（8→16）、MASK←$FFFF、XOR←$8000、SHIFT←8。変換は「左へ 8 → AND MASK → XOR」の順で、x＝(b−128)×256（Q15）になる [D DR-046 (5)]。
- TRIGGER←$0010（NOW・LOOP）、CTRL←START。

**6. 子基板がある：オシレータを走らせる**

1. ch1 の COUNT（作業値＝残り−1）が 127 以下になるまで待つ。書き手が 128 標本進んだところ。
2. オシレータ 0・1 の VOLUME←255 を書く。
3. CONTROL を、オシレータ 0 に $00、1 に $10 にする（自走、CA＝0・1）。

**7. 主ループ（1 フレームに 1 回）**

1. `BIOS_WAIT_VBL`。
2. メールボックスを 5 往復する：`CMD_SNAP`（$0200）、`CMD_READ` の 0〜3（$0300〜$0303）。どれも、`DSP_STATUS` の bit 4 を待ってから `DSP_RESPONSE` を読む。
3. 水準計を描く。ACRTC の AMOVE と AFRCT を使い、色は WPR の CL0・CL1 で替える（`sdk/samples/game` と同じ書き方）。
4. `BIOS_READ_PAD` で新しく押したボタンを見る。A・B・C なら `CMD_MODE`（$0100＋モード）を送り、応答を SW に出す。
5. 数字を描く。8 フレームを周期とし、0 番目に COUNT と SUM、4 番目に PEAK、6 番目に FLAG を描く。FLAG を作るときは `DSP_STATUS` の bit 5・6 と `DAC_STATUS` の bit 8 を読む。
6. 6 フレーム目に一度だけ、立ち上がりの印を W1C で消す：`DSP_STATUS`←$0060、`DAC_STATUS`←$0100 [P]。

## 06　DSP のプログラム（`effects.dsp`）

**番地**

| プログラム RAM | 中身 |
|---|---|
| 0 | `B init`（リセット） |
| 2 | `RET`（INT。EINT しないので来ない [P]） |
| 3〜13 | 定数の表（11 語）。init が TBLR でデータ RAM に写す |
| 14〜230 | コード（init から。全部で 231 語） |
| $400〜$FFF | エコーの遅延線（3072 語）。68000 が 0 で埋める。TBLW は 8 番地より上だけに書く [D DR-029 (2)] |

データ RAM はページ 0 だけを使い、DP＝0 のままにする。割当ては `effects.lst` の記号の表にある。主なものは次のとおり。

- XN（0〜30）：FIR の遅延線
- 状態と作業域：STAT、CMD、OPC、ARG、TMP、YL、DL、W、Y、OUTW、RPTR
- 数：CNT_HI、CNT_LO、SUM、PEAK
- 写し（`CMD_READ` の並び）：S_CNT_HI、S_CNT_LO、S_SUM、S_PEAK、MODE、MAGIC
- 定数：ONE、K_FF ほか

**流れ**

1. init：DINT、ROVM、LDPK 0、LARP 0 のあと、データ RAM を 143 番地から 0 番地まで 0 にする（リセットでは消えない [D 台帳 §4.4]）。続いて定数を写し、RPTR＝$400 にする。
2. idle：`IN STAT,PA2` で状態を読む。bit 2（COMMAND）が立っていればコマンドを処理する。BIOZ（BIO＝出力 FIFO に空き）でなければ idle に戻る。bit 0（入力あり）でなければ idle に戻る。
3. 1 標本の処理：
   - `IN XN,PA0` で x[n] を読む。
   - LPF：`ZAC`、`LT XN+30`、`MPYK H30` のあと、`LTD XN+k`・`MPYK Hk` を 30 回繰り返し、`APAC` で終える。`SACH YL,4` は (ACC<<4)>>16＝ACC>>12。
   - ECHO：遅延線から `TBLR` で d を読み、w＝(x·2^15＋d·2^15) の上位 16 bit＝(x＋d)>>1 を求める。w を同じ番地に `TBLW` で書き戻し、番地を進める（$1000 で $400 に戻る）。
   - MODE で y を選ぶ。
   - u＝((y>>8) AND $FF) XOR $80 を求め、0 なら 1 にする。出力の語は u·257（L＝R）で、`OUT OUTW,PA1` で書く。空きは BIOZ で確かめてある。
   - 数を進める：数（32 bit）、検査和（＋語）、山（\|y\| を符号なしで比べる）。
4. コマンド：`IN CMD,PA3` で読む。読むと COMMAND が下がる。上位 byte で分け、`OUT TMP,PA2` で応答する。

3 つの出力は、モードによらず毎回すべて計算する。遅延線（XN とエコー）の中身はモードによらないので、どのモードの出力も、そこまでの入力だけで決まる [P]。このため、モデルと標本ごとに比べられる。

**サイクル**：idle から `B idle` までの命令を、どの分岐も通ると数えて上限 142 サイクル（`effects.lst` の数。`test_build.py` が確かめる）。1 標本の時間は、16 kHz で 312 サイクル（CLKOUT 5 MHz [D 台帳 §1]）、15.625 kHz で 320 サイクル。

**FIR**：31 タップ、Hamming 窓の sinc、遮断 1200 Hz、Q12 の整数で和は 4096、左右対称 [P]。利得は 500 Hz で −0.3 dB、1200 Hz で −6.0 dB、3000 Hz で −64.6 dB、5000 Hz で −55.8 dB。係数の絶対値の和は 4864 なので、入力の山が 0.6 FS なら ACC>>12 は 16 bit に収まる（ROVM で回り込ませるが、起きない。`test_build.py`）。

**ECHO**：帰還が 1/2 なので、出力の山は入力の山を超えない。インパルスは D、2D、3D… の後に 1/2、1/4、1/8 で返る（`test_build.py`）。

## 07　68000 と DSP の約束（`protocol.inc`） [P]

メールボックスの語は、上位 byte がコマンド、下位 byte が引数。DSP は 1 つのコマンドに必ず 1 回だけ応答する。68000 は、応答を読んでから次のコマンドを書く。

| コマンド | 引数 | 応答 |
|---|---|---|
| `CMD_MODE` $01 | モード（0 THRU、1 LPF、2 ECHO） | 新しいモードで処理する最初の標本の番号（下位 16 bit） |
| `CMD_SNAP` $02 | なし | 数・検査和・山を写し、山を 0 に戻す。応答は写した数の下位 16 bit |
| `CMD_READ` $03 | 0 数の上位、1 数の下位、2 検査和、3 山、4 モード、5 $3210 | その値 |
| その他 | | $FFFF（範囲の外の引数も） |

割込み（bridge の IRQ_EN と DSP の INT）は使わず、両方の側が状態を読んで待つ [P]。待つ上限は、68000 の側が 400 回（1 回が約 5 µs）[P]。DSP の 1 標本の処理（142 サイクル＝28 µs）の後にも、応答は間に合う。

## 08　確かめたこと

`<scratchpad>` に WAV と画面を書き出して確かめた。値は §02 の台本のときのもの。

**子基板がない（`--no-doc`）**

- 600 フレームで、バスエラーは 0、エミュレータの警告もない。FLAG は 0000、LOAD は `00E7 0000`、ID は 3210。
- WAV の L と R は同じで、どの標本も 128 の倍数。WAV を 3 つずつに間引くと、16 kHz の DAC の並びが取れる（3 つがそろわない組は 0）。
- DSP の 0 番目の標本は、WAV の 128.4 ms から始まる。そこから 158187 標本（9.9 秒）が、モデルの出力と 1 つ残らず一致した。モードの切り替えは、画面の SW と同じ番号で起きている。LPF の切り替え（30395 と 126518）は、試験の音の無音の部分にあたる。WAV で違いが見えるのはループの頭（32001 と 128001）からだが、その間は THRU と LPF の出力が同じなので、どちらとも合う。

| フレーム | MODE | COUNT | SUM | SW | モデル |
|---|---|---|---|---|---|
| 60 | THRU | $000031E0 | $E4F7 | $0000 | 一致 |
| 200 | LPF | $0000C812 | $92AF | $76BB | 一致 |
| 300 | ECHO | $00012C33 | $4785 | $F3E4 | 一致 |
| 400 | THRU | $000198AC | $5A7F | $7112 | 一致 |
| 600 | LPF | $00026946 | $2E3F | $EE36 | 一致 |

**子基板がある（`--no-doc` なし）**

- 600 フレームで、バスエラーは 0。FLAG は 0000。
- `verify_wav.py --doc` は、§04 の DOC の読み方を整数で計算する。まず E の位相と書き手との位置の差を探す。次に、WAV の 474138 標本（9.88 秒）を、モデルの出力から計算した値と比べた。1 つ残らず一致した。モードの切り替えは WAV の上で、画面の SW と同じ番号に見えた。
- L と R は少し違う（§04）。

| フレーム | MODE | COUNT | SUM | SW | モデル |
|---|---|---|---|---|---|
| 60 | THRU | $00003173 | $2E77 | $0000 | 一致 |
| 200 | LPF | $0000BBF9 | $D4F8 | $73B0 | 一致 |
| 300 | ECHO | $000125E7 | $78DA | $EDE6 | 一致 |
| 400 | THRU | $000187B0 | $A443 | $6825 | 一致 |
| 600 | LPF | $00025366 | $9C93 | $E25F | 一致 |

`verify_wav.py` の DOC の計算は、エミュレータの混ぜ方（`console.rs` の mix：E のサイクルの値の平均を 4 で割り、48 kHz の時刻の後のスロットの境で取る）に合わせてある。機械の仕様ではない [V]。実機の確かめには、DOC の出力を別に録る必要がある。

## 09　[P] の決めごと

このサンプルでエージェントが決めたこと。どれも要見直し。

1. 音の道：子基板がなければ port 4→port 2（本体 DAC、16 kHz）。あれば port 4→波形 RAM の 256 byte の輪（$1000）を DOC が読む（15.625 kHz、FC＝144、先行 128 標本）。
2. 歩調：BIOSEL＝1（出力 FIFO に空き）。DSP は出口の速さに従い、拍のラッチ（BIOSEL＝3）は使わない。
3. 入力の形：8 bit の符号なし → FORMAT 2・SHIFT 8・XOR $8000 で Q15。
4. 出力の形：u·257（L＝R のモノラル）。u＝0 は 1 にする（DOC の止める値を避ける）。
5. FIR：31 タップ、Hamming 窓、遮断 1200 Hz、Q12、MPYK の直列の形（LTD・MPYK）。
6. ECHO：遅れ 3072 標本、帰還 1/2、遅延線はプログラム RAM の $400〜$FFF（TBLR・TBLW）。
7. 3 つの出力を毎回すべて計算する（モードと状態を切り離す）。
8. メールボックスの約束（§07）と、`DSP_MAGIC`＝$3210。
9. 割込みは使わず、待つのは状態ポートと BIOZ（DSP）、STATUS の読み返し（68000）。
10. データ RAM はページ 0 だけ。定数は 255 番地までの表から TBLR で写す。
11. ROVM（回り込み）。あふれは入力の山 0.6 FS と係数の和で起きないようにする。
12. 試験の音：2 秒、ノコギリ波 4 つと雑音 2 つ、山 0.6 FS、16 kHz。
13. DSP のプログラムの残りの語は 0 で埋め、4096 語を読み返す。
14. 画面：COUNT と SUM は同じ写しを同じフレームに描く（12 字、150 ラインほど）。PEAK・FLAG は別のフレーム。水準計は PEAK÷128。
15. 立ち上がりの UNDERRUN（DAC の FIFO が埋まるまで）は、6 フレーム目に一度だけ消す。
16. 題名は「SAMPLE 2」。名前は付けない。
17. `fc85.inc` に足した名前：`DSP_CTRL_BIO_*`、`DSP_ST_*`、`DSP_PROGRAM_WORDS`。

## 10　見つけた問題

1. **m68kasm：`.w`・`.l` という名前の局所ラベル**。定義は通るが、参照すると誤る。
   - `.w` で参照すると「式がない」の誤りになる。
   - 親の名前を付けて `f.w` と書くと、誤りにならず、`f` に絶対番地の大きさの指定 `.W` を付けたものとして組まれる（分岐先が変わる）。
   - 再現：`f:` / `BRA.S f.w` / `.w RTS`（各行をタブで区切る）を `Assembler(base=0).assemble_source` で組むと、`60FE 4E75` になる（`f` への分岐）。
   - このサンプルではラベルの名前を替えて避けた。直すなら、`.b`・`.w`・`.l`・`.s` をラベルの名前として拒むか、参照の解釈をそろえる [P]。
2. **pre-commit フックのメールアドレスの検査**。行頭が `@` の Python の飾り（`@` の後に `unittest.skipUnless`）を足すと、アドレスとみなして止める。差分の行の先頭の `+` が、アドレスの `@` の前の文字の組に入るため。
   - 再現：行頭に `@` を置き、続けて `a.bc` と書いた行を足してコミットする。
   - `sdk/dspasm/test_dspasm.py` では、飾りをやめて `setUpClass` の中で飛ばすことで避けた。
   - 直すなら、`tools/hooks/pre-commit` の検査で、差分の印の `+` を外してから調べる [P]。
3. エミュレータ・BIOS・仕様書の問題は見つからなかった。DSP の命令、bridge、FIFO、Data Fabric の変換と待ち、本体 DAC、DOC の読み方は、どれも文書から作ったモデルとビットまで合った。
