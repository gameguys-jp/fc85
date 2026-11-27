# SAMPLE 8（C で書いた Temporal RAM の残像のカートリッジ）

**v0.1**  
2026年11月27日

> 状態：作業中の版。C で書いた 2 本目のサンプル（DR-114）。ACRTC の矩形を Data Fabric の port 1 で Temporal RAM と行き来させて、遅れた絵を描き戻す本機の表現を、SDK の C の関数（`acrtc.h` の DMA、`temporal.h`、`fabric.h`。[c.md](../../docs/c.md) §09、DR-112・DR-113）だけで書く。アセンブラで同じことをするのは SAMPLE 3（[patches](../patches/README.md)）の TEMPORAL FEEDBACK。機械仕様は [仕様書](../../../docs/spec/hardware.md)、C の手引きは [c.md](../../docs/c.md) が正。このサンプルで決めた値はすべて [P] で、エージェントが決めた（ユーザーが判断を任せた。要見直し）。題名の `SAMPLE 8` は中立な仮の名前で、製品名ではない。

記述ステータス：`[D]` 原典に根拠のある IC の仕様、`[P]` 本機独自の暫定設計値、`[O]` 代替・拡張候補、`[V]` 要検証（ここではエミュレータで測った値も [V] とする）。

## 01　概要

| 出すもの | 中身 |
|---|---|
| 今の矩形（NOW） | 左の 64×32 画素（y＝56〜87）で、8 画素の玉が跳ねる。色は 8 回ごとに替わる |
| 記録 | 毎回、その矩形を DRD で読み、Fabric で Temporal RAM（port 6）に書く。1 語を上位 byte（`FMT_16_8_HI`）と下位 byte（`FMT_16_8_LO`）の 2 回に分け、1 回 1024 byte。64 KiB で 64 回分 |
| 残像（D・2D・3D） | 右の 3 つの矩形に、d・2d・3d 回前の記録を描き戻す。上位 byte を DWT（下位は 0）、下位 byte を DMOD の OR で（`FMT_8_16`、SHIFT 8 と 0）。語まで同じ絵になる |
| HOLD | A で Temporal RAM の CTRL の HOLD を入れる。書込みは止まり、ポインタは進むので、残像には 64 回前の周の記録が出て、止める前の 64 回が繰り返される |
| 数 | d、HOLD、DIST（WPTR−RPTR。描き戻しの後は 3d 回分の byte）、Fabric と ACRTC の誤りの数 |

- 主ループは VBlank を待ってから、玉を描き、DDM を立て、記録 1 回と描き戻し 3 回（Fabric の転送 8 回、4096 語）をして、DDM を下ろす。エミュレータでは 1 フレームの約 86% で、毎フレーム回る [V]。仕事は VBlank の後から次のフレームの表示の途中まで続くので、描き戻しの途中の絵が見えるフレームがありうる。
- DDM の間は BIOS の字の呼出しを使わない（`acrtc.h`）。字は DDM を下ろした後に、値が変わったときだけ書く（BIOS の字は時間がかかり、毎回書くと 1 フレームに収まらない）。
- 起動の直後、まだ記録していない番地からは、Temporal RAM の電源投入時の中身（エミュレータでは決まった模様。DR-067）が残像に出る。
- 像は約 3 KB。`$400000` から置く。

## 02　組み方と動かし方

リポジトリの根で：

```
py sdk/samples/echo/build.py
cd emulator
cargo run --release -p fc85-host -- --cart ../sdk/samples/echo/build/echo.bin
```

68000 の GCC が要る（[c.md](../../docs/c.md) §02）。GCC がなければ `build.py` は組まずに 2 を返し、試験は飛ばす。

| キー | 働き |
|---|---|
| 上・下 | d（1〜21。3×21×1024 byte が 64 KiB に入る） |
| A | HOLD の入切 |
| START | d を 8 に戻し、HOLD を解く |

## 03　試験

`py -m unittest discover -s sdk/samples/echo`。GCC があれば組んで見出しを確かめる。エミュレータの窓口と Boot ROM もあれば動かし、主ループが 60 フレームで 60 回回ること、誤りが 0 のこと、主ループの 1 回ごとに VRAM の 4 つの矩形を読んで、右の k 番目の矩形が k×d 回前の左の矩形と語まで同じこと、HOLD の後は記録しなかった回の代わりに 64 回前の周の記録が出ることを確かめる。
