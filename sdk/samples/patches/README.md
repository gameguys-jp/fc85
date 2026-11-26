# SAMPLE 3（実験パッチのサンプルのカートリッジ）

**v0.1**  
2026年11月26日

> 状態：作業中の版。仕様書 §11 の「実験パッチ」のうち 4 つを、本機の実際のデータの経路（ACRTC のデータの DMA、Data Fabric、DOC の波形 RAM、ADPCM の FIFO、文字 ROM、Temporal RAM）でつないで動かすサンプル。機械仕様は [仕様書](../../../docs/spec/hardware.md)、BIOS の約束は [bios-abi.md](../../docs/bios-abi.md) が正。このサンプルで決めた値はすべて [P] で、エージェントが決めた（ユーザーが判断を任せた。要見直し）。題名の `SAMPLE 3` は中立な仮の名前で、製品名ではない。

記述ステータス：`[D]` 原典に根拠のある IC の仕様、`[P]` 本機独自の暫定設計値、`[O]` 代替・拡張候補、`[V]` 要検証（ここではエミュレータで測った値も [V] とする）。

## 01　概要

| ページ | 仕様書 §11 のパッチ | データの経路 | 操作 |
|---|---|---|---|
| 0 GRAPHICS AS WAVE | Graphics as wave | VRAM →（DRD、port 1）→ DOC の波形 RAM（port 2）の頁 $20 | 上下：走査の行。A：音程。B：GUARD。C：EOR で描く |
| 1 IMAGE AS ADPCM | Image as ADPCM | VRAM →（DRD）→ Staging RAM（port 0）の 256 byte → ADPCM の FIFO（port 5）→ MSM5205 | 上下：走査の行。A：ニブルの順。B：速さ（4・6・8 kHz）。C：毎フレームの RESET |
| 2 GLYPH AS WAVE | 字形の音響化（§08） | 文字 ROM（port 9）の 256 byte（32 字）→ DOC の波形 RAM | 上下：次・前の 32 字。A：音程。B：GUARD。C：INVERT |
| 3 TEMPORAL FEEDBACK | Temporal feedback | VRAM →（DRD）→ Temporal RAM（port 6）、Temporal RAM（port 7）→（DWT・DMOD）→ VRAM | 上下：遅れ。A：BITS（8・16）。B：MODE（COPY・EOR）。C：HOLD |

左右でページを替える。ページを替えると、両方のチャネルを止め、ADPCM をリセットに置き、DOC のオシレータ 0・1 を止めてから、次のページを始める。

- 像は約 4.3 KB。`$400000` から置く。DOC の子基板があるときに、ページ 0 と 2 が鳴る（ないときは `NO DOC BOARD` と出す）。
- 使う BIOS の呼出し：`BIOS_WAIT_VBL`、`BIOS_CLS`、`BIOS_PUTS`、`BIOS_PUT_HEX`、`BIOS_READ_PAD`。描画は ACRTC に直接送る。
- 外の音源・画像は使わない。絵はプログラムが描き、音はその絵と文字 ROM から作る。

## 02　組み方と動かし方

リポジトリの根で：

```
py firmware/charrom/build.py
py firmware/boot/build.py
py sdk/samples/patches/build.py
```

`sdk/samples/patches/build/` に `patches.bin`（像）と `patches.lst`（リストと記号の表）ができる。

窓で動かす（`emulator/` で）：

```
cargo run --release -p fc85-host -- --cart ../sdk/samples/patches/build/patches.bin
```

画面なしで動かし、WAV と画面を書き出す。入力の台本 `pad-demo.txt` は、4 つのページを順に回り、各ページの釦を一度ずつ押す（台本の頭の説明を参照）。

```
cargo run --release -p fc85-host -- --headless --cart ../sdk/samples/patches/build/patches.bin \
    --frames 1000 --pad-script ../sdk/samples/patches/pad-demo.txt \
    --dump <scratchpad>/p.ppm --dump-at 80,170,350,470,650,770,890,990 --wav <scratchpad>/p.wav
```

## 03　ページごとの仕組み

### 0. GRAPHICS AS WAVE

- 絵（y＝16〜79、256×64）に、跳ね返る箱 3 つと斜めの線を描く。C で EOR にすると、重なりが別の色になり、波形も変わる。
- 毎フレーム、走査の 2 行（64 語×2）を `DRD` で読み、Data Fabric のチャネル 0 で波形 RAM の 256 byte に書く。Temporal RAM と同じく、16 bit の語を 8 bit の口に入れるので、2 回に分ける：FORMAT 4（上位 byte）を偶数番地へ、FORMAT 3（下位 byte）を奇数番地へ（DST_STEP 2）。1 byte は 2 画素で、左の画素が下位ニブル（bios-abi.md §06）なので、1 語の 4 画素は byte の列で「3・2、1・0」の順になる。
- GUARD：MASK $FE・XOR $81。どの byte も奇数になって 0 にならない（0 は DOC のオシレータを止める）。色 0 の 2 画素は $81（中央）になる。B で GUARD を外すと、色 0 の画素が 0 の byte になり、オシレータはそこで止まる。毎フレーム `CONTROL` の HALT を 0 に書いて動かし直すので、約 60 Hz で切れる音になる。
- オシレータ 0（L）と 1（R）が、頁 $20 の 256 byte（T＝0、R＝0）を自走で読む。FC は 130・260・520（A）。1 周期は 55.556 kHz×FC÷512÷256 で、55・110・220 Hz。オシレータ 1 は FC を 1 だけ高くして、うなりを出す。

### 1. IMAGE AS ADPCM

- 絵はページ 0 と同じ。走査の 2 行を `DRD` で Staging RAM の 256 byte（`$2000`）に写す（FORMAT 1）。
- チャネル 1 は、その 256 byte を `TRG_NOW|TRG_LOOP` で ADPCM の FIFO に送り続ける。Data Fabric の側は FIFO の空きを待つので、MSM5205 の速さ（4・6・8 kHz で 1 ニブル）で進む。チャネル 0 の写しと、チャネル 1 の読出しはそろえない。
- 画素のニブルは、そのまま ADPCM の符号になる。色 0 は「少し上げる」符号で、色 8 は「少し下げる」符号。ADPCM は差分なので、同じ 256 byte を回しても、前の復号の状態（値と刻み）によって毎周違う音になる。
- A でニブルの順（上位から・下位から）、B で速さ、C で毎フレームの RESET（写しの間、約 0.5 ms 押さえる。復号の状態が毎フレーム初期値に戻るので、約 60 Hz の周期の音になる）。

### 2. GLYPH AS WAVE

- 文字 ROM の offset `v_glyph×256` から 256 byte（8×8 の字形 32 字）を、チャネル 0 で波形 RAM の頁 $20 に写す（port 9 は offset をそのまま byte の番地として読む。仕様書 §08）。
- 画面には、その 32 字を 256×8 の帯に描き（CPU が文字 ROM を読み、`DWT` で書く）、下に DOC が読む 256 byte を縦の線（値÷4 の高さ）で描く。
- GUARD：XOR $01。字形の 8 行目（下の余白）は必ず 0 で、bit 0（右の余白）も必ず 0 なので、XOR $01 で 0 がなくなる（仕様書 §08 の「字形の音響化」）。B で外すと、最初の字の 8 byte 以内でオシレータが止まる。ページ 0 と同じく、毎フレーム動かし直す。
- C の INVERT は XOR に $FF を足す（GUARD と合わせて $FE）。右の余白のほかが全部点の行（$FE）が 0 になって止まる。GUARD を外した INVERT（$FF）は、bit 0 がいつも 1 になるので止まらない。

### 3. TEMPORAL FEEDBACK

- 左の矩形（x＝0〜63、y＝32〜79。16 語×48 行＝768 語）に、跳ね返る球と縦の棒を描く。
- 記録：`DRD` で今の矩形を読み、Temporal RAM の `v_tbase` から書く。Temporal RAM は 8 bit なので、BITS 8 では FORMAT 4（上位 byte）だけを書く。1 フレームの記録は 768 byte で、64 KiB に約 85 フレーム。BITS 16 は、続けて FORMAT 3（下位 byte）も書く（1 フレーム 1,536 byte、約 42 フレーム）。
- COPY：RPTR を d・2d・3d フレーム前の記録の頭に置き、FORMAT 2 と SHIFT 8 で語の上位 byte に戻して、`DWT` で右の 3 つの矩形に書く。BITS 8 では語の下位 byte（左の 2 画素）が 0 になり、縦の縞になる。BITS 16 は、続けて下位 byte を `DMOD` の OR で重ね、元の像に戻る。
- EOR：d フレーム前の記録を、今の矩形に `DMOD` の EOR で重ねてから記録する。重ねた結果をまた記録するので、残像が d フレームおきに続く（帰還）。
- HOLD：Temporal の CTRL の HOLD（DR-067）。書込みの /WE を止め、ポインタだけを進める。履歴が止まり、同じ数フレームを繰り返して見せる。
- 遅れが記録できるフレーム数を超えると、輪の上で新しい記録を読む（BITS 16 で 3d が 42 を超えるとき）。
- 下の行に、DIST（WPTR−RPTR）と CTRL（EQUAL・OVERRUN・UNDERRUN の印）を 16 フレームに 1 回描く。
- 電源投入の直後は、記録のない番地を読むので、SRAM の電源投入時の中身（DR-067 の模様）が矩形に出る。

## 04　確かめたこと

エミュレータの試験 `the_patch_cartridge_runs_every_page`（`emulator/crates/board/tests/console_boot.rs`）が、DOC の子基板ありで 4 つのページを回り、次を確かめる [V]。

| ページ | 確かめること |
|---|---|
| 0 | 波形 RAM の頁 $20 の 256 byte がすべて奇数（GUARD）。音が出ている |
| 1 | 30 フレームで MSM5205 が 300 ニブル以上を取り込む |
| 2 | 波形 RAM の頁 $20 が、文字 ROM の offset $100 からの 256 byte の XOR $01 と一致する |
| 3 | Temporal RAM の WPTR が、1 フレームに 768 byte ずつ進む |
| 全体 | バスエラー 0、DOC の窓の規約違反 0、Fabric の誤りと時間切れ 0（`v_errors`） |

`pad-demo.txt` で 1000 フレーム動かしたときの WAV（DOC の子基板あり）[V]：

| 区間 | 平均の振幅（RMS） | 見たこと |
|---|---|---|
| ページ 0 | 約 2,000 | 箱と線が走査の行を横切るたびに音色が変わる |
| ページ 1 | 約 1,000 | 雑音に近い音。速さとニブルの順で変わる |
| ページ 2 | 約 1,700〜2,300 | 字形ごとに音色が違う。GUARD を外すと約 210 に下がる（止まっては動き直す） |
| ページ 3 | 0 | 音は出さない |

画面では、ページ 3 の BITS 8 の縞、BITS 16 での復元、EOR の残像の連鎖、HOLD での繰返しを見た。BITS 16 では 1 つの遅れに 2 回の転送がいるので、走査がちょうど通るフレームには、上位 byte の画素だけが見える矩形がある（二重のバッファがないための裂け目）。

## 05　[P] の決めごと

| 項目 | 値 | 理由 |
|---|---|---|
| 絵の消し方 | 前のフレームの箱と線だけを色 0 で消す | 256×64 の全体を毎フレーム `AFRCT` で消すと、塗りが表示の走査に追い越され、絵が見えない（試して確かめた） |
| 波形の頁 | $20（$2000〜$20FF） | BIOS と SAMPLE 2 の頁と重ねない |
| DOC の音量 | 160 | 2 つのオシレータの和が振り切れない |
| Fabric の待ちの上限 | STATUS を 40,000 回読む（約 0.4 秒） | 1 回の転送は長くて約 1.3 ms |
| DDM | 転送の間だけ CCR の DDM を立てる | BIOS の文字は CPU が `DWT` のデータを書くので、DDM＝0 で描く（bios-abi.md §06） |
