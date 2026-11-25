# 1985年の日本製機器の部品構成 基準台帳
Phase 0 調査 v0.1 ／ 調査日 2026-11-25

凡例：`[D 資料ID 頁]` 出典あり。`[V]` 未確認。「推定」は本台帳での推論。資料IDは末尾の表を参照。`[D MAME 名前]` は MAME のドライバ（ファイル名）を指す。

資料の区分：**A** は1984年以前、**B** は1985年中の刊行。**C** は1986年以降の刊行。C のうち「実機基板」と書いたものは、実物の基板・回路図の観察記録。

使い方：判断基準①「1985年の日本の中堅業務用電子機器メーカー（文字表示・映像・通信が本業）にとって自然か」の物差しにする。

- 部品が1985年までに存在したことは A・B（半導体メーカーのカタログ）で示す。
- 機種がどの部品を載せたかは、実機基板の記録（C）で示す。後期ロットや改版との差がありうる。
- 機種の発売年は C（Wikipedia・博物館・MAME）による。

既存台帳（`MC6850.md`、`TMS32010.md`、`HD63484.md`、`supply-HD63484.md`、`supply-ES5503.md`、`review-signal.md` ほか）で調べた事項は繰り返さない。

---

## 0. 要旨

- **自社・系列の半導体を先に使う。** NEC機は µPD で固める。富士通機は MB／MBL。シャープは自社の LH0080A に日立の CRTC と富士通の FDC を組み合わせる（§1）。
- **半導体部門のない（または小さい）メーカーは、国内大手の互換品を集める。** ソニー SMC-777、ローランド、ヤマハ DX7 がこの型（§2）。中堅の業務機メーカーもこの型に近い（推定）。
- **68000 の国内調達先は、1985年時点では日立（HD68000）。** 1985年の国産基板で HD68000P8／HD68000-8 の刻印がある。東芝 TMP68000 は1988年で、1985年機には使えない（§5）。
- **音源はヤマハ FM が標準。** YM2203・YM2151・YM2164 が並ぶ（§1〜§4）。
- **パソコンの映像出力は RGB。** テレビ出力はテレビ向けの機種か外付けカード（§1）。
- **DSP の量産採用は、1985年の国産機では見つからない** [V]。国産ゲーム基板の TMS320 は1987年から（§6）。

---

## 1. パソコン（1984〜85年を中心に）

> 注（2026-11-25）：X1 のテレビとの重ね合わせは、当時の資料（1982年12月の雑誌記事、1983年1月の広告）でも確認した。[sources.md](../sources.md) §04 を参照。

| 機種（メーカー、発売） | CPU | 映像 | 音 | 周辺（DMA・FDC・シリアルほか） | 映像出力 |
|---|---|---|---|---|---|
| PC-8801mkIISR（NEC、1985-01） | µPD780C-1（NEC、Z80A 相当）。FDD 側のサブ CPU も µPD780C-1 [D JW-PC88; GW-PM88] | µPD3301（NEC の CRTC）[D MAME pc8801] | YM2203（ヤマハ）[D JW-PC88] | DMA は 8257 相当（メーカー [V]）、RTC µPD1990A（NEC）[D MAME pc8801] | アナログ RGB。テレビ出力は1988年の PC-8801FE から [D JW-PC88] |
| PC-9801VM（NEC、1985-07） | V30（µPD70116、NEC）8／10 MHz [D JW-PC98] | µPD7220 ×2（NEC の GDC。テキスト用とグラフィック用）＋ GRCG [D JW-PC98] | 本体に FM 音源はなく、ビープ音のみ [D JW-PC98]。FM は拡張ボード PC-9801-26（YM2203 1個）[D MAME pc9801_26] | 8237・8259 ×2・8253・8251・8255・765（初代 PC-9801 の回路図）[D OGU-98]。VM の定義も同構成 [D MAME pc9801]。NEC は µPD8237A・8251A・8253・8259A を自社で出していた [D NEC85SG] | アナログ RGB（4,096色中16色）[D JW-PC98] |
| X1turbo（シャープ、1984-10） | Z80A 相当。初代 X1（1982）と X1D は シャープ LH0080A [D AKB-X1; AKB-X1D]。turbo の品番 [V] | 初代 X1・X1D は HD46505SP-2（日立）[D AKB-X1; AKB-X1D]。turbo の品番 [V] | AY-3-8910（GI）[D AKB-X1]。FM（YM2151）は拡張 [D MAME x1] | Z80 系の CTC・DMA・SIO を標準搭載 [D JW-X1]（メーカー [V]）。FDC は富士通 MB8877／MB8877A [D AKB-X1D; MAME x1]。サブ CPU はシャープ M80C49-22（初代）[D AKB-X1] | テレビと重ね合わせ（デジタルテロッパー、スーパーインポーズ）[D JW-X1] |
| FM77AV（富士通、1985-10 発表） | 68B09E ×2（メイン 2 MHz、MMR 使用時 1.6 MHz）[D IPSJ-AV]。品番 [V]。前身の FM-7 はメインが富士通 MBL68B09、サブがモトローラ MC6809 [D JW-FM7] | 4,096色同時表示 [D IPSJ-AV]。FM-7 以来、表示はサブ CPU 側 [D JW-FM7] | YM2203（FM＋PSG）を標準 [D IPSJ-AV]。FM-7 は YM2203 カード（MB22459）が別売 [D JW-FM7] | FDC MB8877A、キーボード MCU MB88401、マスク ROM MB83256 ほか（すべて富士通）[D MAME fm7] | RGB。NTSC 変換・ビデオデジタイズは別売カード（後継 AV20EX の一覧）[D HB158] |
| MZ-2500（シャープ、1985） | Z80 系（品番 [V]） | [V] | YM2203 [D MAME mz2500] | FDC MB8876 ＋ VFO MB4107（富士通）、RTC RP5C15（リコー）[D MAME mz2500] | [V] |
| MSX2 各社（1985〜） | Z80A 相当、3.58 MHz（規格）[D JW-MSX]。品番 [V]。MSX1 のパイオニア PX-7 は LH0080A [V]（PX7） | V9938（ヤマハ）[D MAME msx2] | YM2149 相当。ヤマハの MSX-ENGINE（S3527 または S1985）に内蔵 [D MAME msx2] | S3527：ソニー HB-F500、ヤマハ CX7M/128、三菱 ML-G30、ナショナル FS-4500 ほか。S1985：日立 MB-H70、ナショナル FS-5000F2。東芝 HX-23F は自社の TCX-1012 と AY-3-8910。ビクター HC-90 は第2 CPU に日立 HD64B180 [D MAME msx2] | テレビ接続が前提（推定）。エンコーダの品番 [V] |
| ソード M68（1983） | 68000 ＋ Z80A [D EW-SORD]。品番 [V] | [V] | [V] | [V] | [V] |

参考（後年）：シャープ X68000（1987）は日立 HD68HC000、日立 HD63450 DMAC、富士通 MB89352A、ザイログ Z85C30、沖 MSM80C51、リコー RP5C15、NEC 72065 [D JW-X68K]。

---

## 2. 業務機・ワープロ・音楽機器

| 機種（メーカー、年） | CPU | 映像・音源 | 周辺 | 出典 |
|---|---|---|---|---|
| APC（NEC、1982） | 8086 系 | µPD7220、音 µPD1771C | 8237 互換、µPD765A、µPD1990A | [D MAME apc] |
| QX-10（エプソン、1983） | Z80 系 | µPD7220 | µPD765、µPD7201 | [D MAME qx10] |
| SMC-777（ソニー、1983） | Z80 系 [D JW-X1] | HD46505SP-1 または HD68A45SP（日立）、音 SN76489A（TI） | FDC MB8877A（富士通）、RTC MSM58321RS（沖） | [D MAME smc777] |
| MULTI 16-II（三菱、1984） | 8086 系 | HD6845S（日立） | FDC MB8866（富士通） | [D MAME multi16] |
| PASOPIA 1600（東芝、年 [V]） | 8086 系 | 6845 系 | 8237 互換 | [D MAME paso1600] |
| ワープロ：富士通 OASYS Lite（1984-05）、NEC PWP-100（1984-03）、カシオ HW-100（1985-05）、ソニー HW-30（1985-06）、NEC 文豪mini 3/5/7（1985-10）、東芝 Rupo JW-R10（1985） | [V] | [V] | [V] | 発売時期のみ [D IPSJ-WP] |
| DX7（ヤマハ、1983） | HD63B03RP ＋ HD6805S1P（日立） | 自社の音源 LSI | MIDI は CPU 内蔵シリアル | [D MAME ymdx7] |
| DX21/27/100（ヤマハ、1985） | HD6303（日立） | YM2164 | ADC M58990（三菱）、LCD HD44780（日立） | [D MAME ymdx100] |
| CZ-101（カシオ、1984） | µPD7810（NEC） | µPD933（NEC 品番の専用音源。推定） | MIDI は µPD7810 内蔵シリアル、LCD HD44780 | [D MAME cz101] |
| JX-8P（ローランド、1985） | HD6303R（日立） | — | MB63H149（キースキャン。富士通品番の専用 LSI、推定）、ADC µPD7001（NEC） | [D MAME roland_jx8p] |
| α-JUNO（ローランド、1985） | 8052 系 | — | MB62H195・MB63H149（同上）、µPD7001、HD44780 | [D MAME alphajuno] |
| MPU-401（ローランド、年 [V]） | 6801 系 ＋ 専用 LSI | — | MIDI は 6801 内蔵シリアル | [D MAME mpu401] |
| SFG-01（ヤマハ、MSX 用、年 [V]） | — | YM2151 | MIDI・キーボード用 UART YM2148（ヤマハ） | [D MAME sfg; ym2148] |
| FB-01（ヤマハ、1986） | Z80 系 | YM2164 | MIDI は µPD71051（NEC、8251 系 USART） | [D MAME fb01] |

注：MAME のデバイス名（I8086、AM9517A など）は機能の名前で、実物のメーカーを示さない。品番まで書いたものは MAME のコメント（基板の刻印）による。

---

## 3. 業務用ゲーム基板（1984〜87年）

| 基板（メーカー、年） | CPU（刻印） | 音 | その他・カスタム | 出典 |
|---|---|---|---|---|
| ハングオン（セガ、1985） | **HD68000P8（日立）**×2、Z80 | YM2203 ＋ セガ PCM | 8255、ADC0804 | [D MAME segahang; EW-SEGA] |
| System 16A（セガ、1985〜） | 68000 10 MHz（一部は日立 FD1094 暗号化版）、Z80A | YM2151 ＋ YM3012、µPD7751C（NEC）＋ D8243C | SRAM MB8128・MBM2148（富士通）、TC5565（東芝）。カスタム 315-5011／5012／5049／5155、Signetics CK2605、PAL16R6 | [D MAME segas16a; EW-68K] |
| グラディウス（コナミ、1985、GX400） | 68000（メーカー [V]）、Z80 | AY-3-8910 ×2、K005289、VLM5030（三洋） | バブルメモリ制御 005297 | [D MAME nemesis] |
| テラクレスタ（日本物産、1985） | 68000-8（メーカー [V]）、Z80A | YM3526（後に YM2203）、DAC | カスタム 1412M2 | [D MAME terracre] |
| スプレンダーブラスト／ハイボルテージ（アルファ電子、1985） | **HD68000-8（日立）** | 音源基板：TMP8085AP（東芝）、AY-3-8910A、M5232（沖）、M5L8155P（三菱） | — | [D MAME splendor] |
| エクイテス（アルファ電子、1984） | **HD68000-8（日立）** | — | — | [D MAME equites] |
| 激走（イースタン、1985） | **MC68000P8（モトローラ）** | — | — | [D MAME equites] |
| System 86（ナムコ、1985） | 6809 ×2、HD63701（日立 MCU） | YM2151、CUS30 | カスタム CUS27・35/48・39・41・42・43・47 | [D MAME namcos86] |
| リブルラブル（ナムコ、1983） | 68000（国内のゲーム初採用）。メーカー [V] | — | — | [D JW-68K] |
| ダライアス（タイトー、1986） | MC68000P8 ×2、Z80 ×2 | YM2203C ×2、MSM5205 | — | [D MAME darius] |
| ニンジャウォーリアーズ、オペレーションウルフ（タイトー、1987） | **TS68000CP8（トムソン）** | — | — | [D MAME ninjaw; opwolf] |
| ミッドナイトランディング（タイトー、1987） | TS68000CP8 | — | **TMS32020** | [D MAME mlanding] |
| 究極タイガー（東亜プラン、1987） | **SCN68000（シグネティクス）** | — | **TMS320C10** | [D MAME twincobr] |

---

## 4. MAME の基板メモの集計（目安）

対象：国内のゲームメーカー19社のドライバのうち、収録ゲームがすべて1983〜1987年のもの 176 ファイル。刻印はコメント中の品番、音源はファイル中の言及を数えた。基板の数ではなくファイルの数。網羅的ではない [D MAME]。

| 部品 | ファイル数（メーカー別） |
|---|---|
| Z80 の刻印 | NEC 11、シャープ 6、ザイログ 4、モステック 1、東芝 0 |
| 6809 の刻印 | 日立 12、モトローラ 11、富士通 2 |
| 68000 の刻印 | モトローラ 5、日立 2、トムソン 1 |
| SRAM の刻印 | 東芝 7、富士通 6、三菱 5、日立 4、NEC 1、沖 1、ソニー 1 |
| MCU | モトローラ 6801／6803／68705 系 20、8751 8、日立 HD63701 ほか 2 |
| 音源 | AY-3-8910 56、YM2203 43、MSM5205 25、YM2149 24、SN76489 22、YM2151 18、YM3526 16 |

読み方（推定）：Z80 は国産の互換品が主。6809 は日立とモトローラが半々。SRAM は国内5社に分散。音源はヤマハと GI（AY-3-8910）が中心で、ADPCM は沖 MSM5205。

---

## 5. 68000 の国内供給

| 時期 | 事項 | 出典 |
|---|---|---|
| 1970年代末 | モトローラと日立が技術交換 | [D EW-68K] |
| 1982 | 日立のカタログに HD68000（PRELIMINARY）。対照表に HD68000-4／-6／-8。DMAC HD68450 も載る | [D HIT82 p.81, p.82] |
| 1983 | ソード M68（68000＋Z80A）。ナムコ「リブルラブル」が国内ゲーム初の 68000 | [D EW-SORD; JW-68K] |
| 1984-07 | 東芝のマイコン LSI データブックに 68000 はない。Z80 は TMPZ84C00 系、8080 系周辺は TMP8251A・8253・8255A・8259A | [D TOS84] |
| 1985 | 日立のデータブックに HD68000-4〜-12（DIP・PGA・LCC）、HD68450、HD6845S、HD6850／HD68A50、HD6350。HD63484 は開発中 | [D DB85 p.444, p.506, p.517, p.585, p.664, p.712] |
| 1985 | 国産ゲーム基板に HD68000P8（セガ）、HD68000-8（アルファ電子）、MC68000P8（イースタン） | [D MAME segahang; splendor; equites] |
| 1985 | CMOS の 68HC000（日立の設計）をモトローラと日立が発表 | [D EW-68K; JW-68K] |
| 1986〜87 | 日本の電機メーカーで 68000 系 EWS の開発が流行（ソニー NEWS、オムロン LUNA など）。68000 を載せたパソコンはほとんどなかった | [D JW-X68K] |
| 1987 | X68000 は日立 HD68HC000（当時 CMOS 版を作るのは日立だけ）。ゲーム基板にトムソン TS68000、シグネティクス SCN68000 | [D JW-X68K; MAME ninjaw; twincobr] |
| 1988 | 東芝 TMP68000 | [D JW-68K] |

---

## 6. DSP（1985年の日本）

- NEC µPD7720 は1980年発表、1982年末に市販（2万円）。用途は電話、ディスク・グラフィック制御、音声合成、モデム [D EW-7720]。国内の音声合成の需要から開発された [D SHMJ702]。
- 1985年の NEC カタログに µPD7720・µPD77P20 が載る [D NEC85SG]。
- 1985年の国産機で µPD7720 または TMS32010 を量産採用した例は見つからない [V]。
- 国産ゲーム基板の TMS320 は1987年から（タイトー TMS32020、東亜プラン TMS320C10）[D MAME mlanding; twincobr]。

---

## 7. 傾向のまとめ

1. **自社・系列の半導体を先に使う。**
   - NEC は CPU から周辺まで µPD でそろえる（PC-8001mkII の µPD780C-1・µPD3301 [D AKB-8001]、PC-8801・PC-9801・APC）。
   - 富士通は MBL68B09・MB8877A・MB88401・MB83256。ただし FM-7 のサブ CPU はモトローラ MC6809 [D JW-FM7]。自社にない品は他社品を使う。
   - シャープは自社の LH0080A・M80C49 に、日立の CRTC と富士通の FDC を組み合わせる [D AKB-X1; AKB-X1D]。
2. **半導体部門のない（または小さい）メーカーは、国内大手の互換品を集める。** 例：ソニー SMC-777（日立 CRTC、富士通 FDC、沖 RTC、TI 音源）、ローランド（日立 HD6303、NEC µPD7001）、ヤマハ DX7（日立 CPU）[D MAME smc777; roland_jx8p; ymdx7]。中堅の業務機メーカーもこの型に近い（推定）。
3. **汎用 LSI は、インテル・ザイログ・モトローラ互換の国産品が標準。**
   - Z80：NEC・シャープ・東芝 [D TOS84; §4]
   - 8080 系の周辺：NEC µPD82xx、東芝 TMP82xx [D NEC85SG; TOS84]
   - 6800 系の周辺：日立 HD46505／HD6845S [D AKB-X1; MAME multi16]
   - FDC：富士通の MB8877A／MB8876／MB8866 がよく出る [D MAME x1; fm7; mz2500; multi16; smc777]
4. **音源はヤマハ FM が1985年の標準。**
   - YM2203：PC-8801mkIISR、FM77AV、MZ-2500、PC-9801-26
   - YM2151：X1 の拡張、System 16
   - YM2164：DX21、FB-01
   - PSG は AY-3-8910 または YM2149。ゲームの ADPCM は MSM5205 [§1〜§4]
5. **68000 は、1985年には日立品を国内で調達できた。**
   - 業務用ゲーム基板と EWS で使われた。パソコンではソード M68 などの少数だった [§5]。
   - 業務用電子機器メーカーが 68000 を選ぶのは、無理のない範囲（推定）。
6. **専用 LSI を起こすのは、量の出る会社。**
   - 例：セガ 315-xxxx、コナミ 005289／005297、ナムコ CUSxx、日本物産 1412M2、ヤマハ MSX-ENGINE [§3]
   - 国内の半導体各社は1985年にゲートアレイを提供していた（日立 HD61 系 CMOS [D DB85 p.732]、NEC は CMOS 12 品種、最大 11,000 ゲート [D NEC85SG]）。
   - 中堅メーカーは、汎用 LSI に PAL と少数のゲートアレイを足す形が自然（推定）。
7. **パソコンは RGB モニタが前提。**
   - テレビ接続や合成映像は、テレビ向けの機種（X1 のテロッパー、MSX、ゲーム機）か、外付けカード（FM77AV 系）だけ [D JW-X1; HB158; SMSP]。
   - セガ マークIII のエンコーダは `review-signal.md` で扱う。
8. **MIDI は、CPU 内蔵のシリアルか、専用 UART を使う。**
   - CPU 内蔵の例：HD63B03／HD6303、µPD7810、6801。専用 UART の例：µPD71051（8251 系）、ヤマハ YM2148 [§2]。
   - 6850 系 ACIA を使った国産楽器は、今回の範囲では見つからない [V]。
   - 日立は1985年に HD6850／HD68A50（NMOS）と HD6350（CMOS）を出していた [D DB85 p.506, p.517]。`MC6850.md` の「国内の第二ソースの品番」はこれで一部埋まる（入手時期・価格は未確認）。
9. **DSP は1985年の国産機ではまだ珍しい** [§6]。

---

## 8. fc85 への示唆（推定。仕様書は未変更）

- 68000 は「日立 HD68000-8」と書くのが、1985年の国産機として最も自然。MC68000 のままでも誤りではない。同年に MC68000P8 を使う国産基板がある。
- HD68000・HD63484・HD6850 は同じ日立のデータブックに載る [D DB85]。調達先を日立にそろえる筋書きが組める。
- MIDI に 6850 系を使うのは、68000 系のバスに合う点で自然。ただし国産楽器の主流ではない（§7-8）。
- TMS32010（DSP）と ES5503（DOC）は米国品で、1985年の国産機には採用例が見えない。仕様書に採用理由を書く必要がある（例：国産品に同等の機能がない）。DSP の国産の対案は µPD7720。
- 本機は音源にヤマハ FM を使わない。1985年の国産機としては珍しい。
- 独自回路（Data Fabric など）は、国内半導体メーカーのゲートアレイで作ったとするのが自然。汎用の DMAC には日立 HD68450 が1985年にあった [D DB85 p.664]。独自 DMA にした理由も書くとよい。

---

## 9. 未解決事項

1. 機種表の大半は C 区分の資料による。1984〜85年の雑誌の基板写真や分解記事（ASCII、I/O、Oh!X など）では確かめていない。
2. メーカーが未確認の品番：X1turbo の Z80 系 LSI と CRTC、FM77AV の 68B09E、PC-8801mkIISR の DMA（8257 相当）、MSX2 の Z80、ソード M68 の 68000、グラディウスとリブルラブルの 68000。
3. ワープロ・業務端末（文字表示が本業のメーカー）の CPU・CRTC・ゲートアレイが未確認。本機のメーカー像に一番近い分野なので、優先して調べる。
4. 1985年の国産機で DSP を量産採用した例。日立 HD61810 など、µPD7720 以外の国産 DSP も未調査。
5. 6850 系 ACIA を MIDI に使った国産機器。今回は MAME に載る楽器だけを見た。
6. MSX2 のビデオエンコーダの品番。
7. MAME の集計は網羅的でない。コメントに刻印を書くかどうかはドライバによる。
8. PX7：ManualsLib の頁本体は参照日に 403 で取得できず、検索結果の抜粋だけを見た。
9. MPU-401・SFG-01・PASOPIA 1600 の発売年。

---

## 出典

参照日はすべて 2026-11-25。取得した PDF・テキストはリポジトリ外の `<scratchpad>\parts\` に置いた。リポジトリには入れていない。Wikipedia は参照日の版。

| ID | 区分 | 書誌 | 日付 | URL |
|---|---|---|---|---|
| HIT82 | A | Hitachi, Full Line Condensed Catalog。p.81 HD68000（PRELIMINARY）、p.82 HD68450、互換品の対照表 | 1982 | http://bitsavers.org/components/hitachi/_dataBooks/1982_Hitachi_Full_Line_Condensed_Catalog.pdf |
| TOS84 | A | Toshiba, Microcomputer LSI Data Book。目次：TMPZ84C00 系、TMP8085A p.32、TMP8251A p.65、TMP8253 p.80、TMP8255A p.95、TMP8259A p.102。68000 の記載なし | 1984-07 | https://bitsavers.org/components/toshiba/_dataBook/1984_Toshiba_Microcomputer_LSI_Data_Book.pdf |
| PX7 | A | Pioneer PX-7 サービスマニュアル p.73（ManualsLib 掲載）。検索結果の抜粋のみ確認 | 1984 [V] | https://www.manualslib.com/manual/1255707/Pioneer-Px-7.html?page=73 |
| DB85 | B | Hitachi, 8/16-Bit Multi-Chip Microcomputer Data Book #U70。`HD63484.md` の DB85 と同じ資料。目次の頁：HD6845S p.444、HD6850/HD68A50 p.506、HD6350 p.517、HD68000 p.585、HD68450 p.664、HD63484 p.712、ゲートアレイ p.732 | 1985（月は未確認） | http://bitsavers.org/components/hitachi/_dataBooks/1985_U70_Hitachi_8_16-Bit_Multi-Chip_Microcomputer_Data_Book.pdf |
| NEC85SG | B | NEC Electronics, 1985 Product Selection Guide。µPD780-1/-2、µPD70108/70116、µPD7220A、µPD7720/77P20、µPD8237A-5・8251A・8253・8257・8259A、µPD7201A、µPD4164・41256、ゲートアレイ | 1985-05 | https://bitsavers.org/components/nec/_dataBooks/1985_Product_Selection_Guide.pdf |
| MAME | C（実機基板を含む） | MAME ソースコード（master）。本文の名前はドライバのファイル名（src/mame/ 以下。pc9801_26・mpu401・sfg・ym2148 は src/devices/ 以下）。コメント中の基板の刻印・部品表を使った | 参照日の版 | https://github.com/mamedev/mame |
| AKB-X1 | C（実機基板） | AKIBA PC Hotline!、懐かしパソコンギャラリー「X1」 | 2013-02-26 | https://akiba-pc.watch.impress.co.jp/docs/wakiba/retro/588974.html |
| AKB-X1C | C（実機基板） | 同「X1C」 | 2014-05-28 | https://akiba-pc.watch.impress.co.jp/docs/wakiba/retro/650113.html |
| AKB-X1D | C（実機基板） | 同「X1D」 | 2015-05-11 | https://akiba-pc.watch.impress.co.jp/docs/wakiba/retro/698478.html |
| AKB-8001 | C（実機基板） | 同「PC-8001mkII」 | 2015-06-05 | https://akiba-pc.watch.impress.co.jp/docs/wakiba/retro/704675.html |
| OGU-98 | C（実機回路図） | 初代 PC-9801 の回路図（PDF） | 不明 | https://www.oguchi-rd.com/PC9801/PC9801%20Schematics.pdf |
| SMSP | C（実機基板） | SMS Power!「Video Output」 | 不明 | https://www.smspower.org/Development/VideoOutput |
| GW-PM88 | C | GAME Watch、PasocomMini PC-8801mkIISR の発表記事（仕様欄） | 2024-08-08 | https://game.watch.impress.co.jp/docs/news/1614735.html |
| IPSJ-AV | C | 情報処理学会 コンピュータ博物館「FM77AV」 | 不明 | https://museum.ipsj.or.jp/computer/personal/0017.html |
| IPSJ-WP | C | 同 日本語ワードプロセッサの各頁（0001 HW-100、0011 OASYS Lite、0022 文豪mini、0029 PWP-100、0043 HW-30、0055 Rupo JW-R10） | 不明 | https://museum.ipsj.or.jp/computer/word/0001.html ほか同形式 |
| HB158 | C | 八ビット（hachibitto）、FM77AV20EX のシステムディスク紹介（英文）。オプション一覧 | 不明 | https://hachibitto.com/jvcc/articles/view_article.php?id=158 |
| SHMJ702 | C | 半導体産業人協会 日本半導体歴史館、展示「µPD7720」英語版 | 不明 | https://shmj.or.jp/english/pdf/ic/exhibi702E.pdf |
| JW-PC88 | C | Wikipedia 日本語版「PC-8800シリーズ」 | 参照日の版 | https://ja.wikipedia.org/wiki/PC-8800シリーズ |
| JW-PC98 | C | 同「PC-9800シリーズ」 | 参照日の版 | https://ja.wikipedia.org/wiki/PC-9800シリーズ |
| JW-X1 | C | 同「X1 (コンピュータ)」 | 参照日の版 | https://ja.wikipedia.org/wiki/X1_(コンピュータ) |
| JW-FM7 | C | 同「FM-7」 | 参照日の版 | https://ja.wikipedia.org/wiki/FM-7 |
| JW-MSX | C | 同「MSX」 | 参照日の版 | https://ja.wikipedia.org/wiki/MSX |
| JW-X68K | C | 同「X68000」 | 参照日の版 | https://ja.wikipedia.org/wiki/X68000 |
| JW-68K | C | 同「MC68000」 | 参照日の版 | https://ja.wikipedia.org/wiki/MC68000 |
| EW-68K | C | Wikipedia 英語版「Motorola 68000」 | 参照日の版 | https://en.wikipedia.org/wiki/Motorola_68000 |
| EW-7720 | C | 同「NEC μPD7720」 | 参照日の版 | https://en.wikipedia.org/wiki/NEC_μPD7720 |
| EW-SEGA | C | 同「List of Sega arcade system boards」 | 参照日の版 | https://en.wikipedia.org/wiki/List_of_Sega_arcade_system_boards |
| EW-SORD | C | 同「Sord Computer Corporation」 | 参照日の版 | https://en.wikipedia.org/wiki/Sord_Computer_Corporation |
