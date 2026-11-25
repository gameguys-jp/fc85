# クロック系と映像入出力の時代考証（1985）
Phase 0 調査 v0.1 ／ 調査日 2026-11-24

凡例：`[D]` 原典に根拠あり（出典ID＋ページ）。`[D: 計算]` は原典の数値から本書で算出した値。`[V]` 未確認・推定。`[P]` 本機の暫定設計案（本書内の提案。仕様書は未変更）。`[O]` 代替案。出典IDは末尾の表を参照。1986年以降の資料には「後年資料」と書く。

対象：仕様書 §03（構成）、§04（実在IC）、§07（クロック・映像）、§09（MIDI・入力）、§10（映像・音響回路）、§13（未確定事項）、§14（参照資料）。IC個別の詳細は同じディレクトリの `MC6850.md`、`CD22101.md`、`TDC1007.md` を参照。

---

## 0. 要旨

1. 1985年前後の機種は、色副搬送波 fsc＝3.579545 MHz の整数倍の水晶から CPU・画素・色信号を分周する方式（TMS9918A、Amiga 等）と、CPU・映像を同じ非NTSC水晶から作り、周辺用に別クロックを持つ方式（Atari ST）が並んでいた。
2. 256画素／ラインの家庭用NTSC表示には 5.37 MHz（＝1.5 fsc）の画素クロック、342クロック／ライン、262ライン・ノンインタレースの前例がある（TMS9918A、1982年資料）。fc85の256×192はこれとそっくり同じ。
3. 262ライン・ノンインタレースのフィールド周波数は約59.92 Hzで、59.94 Hzにはならない。仕様書§07の「NTSC 525/59.94i目標」「内部更新59.94 Hz目標」は、192ラインの順次走査と食い違う。
4. HD63484は「2CLK＝ドットクロックの2倍、MCYC＝2CLK/2、水平レジスタはメモリサイクル単位」なので、ACRTCのクロックは映像クロックと整数比で結ばれる。ACRTC 8 MHz・16-bit VRAM・4bppのまま256画素をNTSCの画面幅に広げられるかは未確認で、HD63484台帳との照合が必要。
5. 1985年以前の資料で確認できたRGB→NTSCエンコーダは Motorola MC1377（1983年資料）と National LM1886＋LM1889（1982年資料）。LM1881は1982年版・1984年補遺になく1988年版に初出（＊印付き）で、1985年機には使わない方がよい。パレット内蔵DAC（RAMDAC）は1985年以前の資料で確認できなかった。
6. 1985年の民生用デジタイザは数秒〜1分かかるスロースキャン方式が主流だった。実時間（1/30秒）のフレームグラバはIBM PC用拡張ボード（16階調〜6ビット）とMac用の外付け機に限られていた。fc85の「128×96・8-bit・15 Hz」は1985年の部品で組めるが、家庭用としては業務用相当の上位仕様になる（フィクションの「盛り癖」とは整合する）。

---

## 1. 1985年前後の機種のクロック構成

### 1.1 一次資料で確認できた例

| 機種・IC | 年 | 主水晶・クロック | CPU | 画素・映像 | 色副搬送波 | 根拠 |
|---|---|---|---|---|---|---|
| TI TMS9918A VDP（MSX等が採用） | 資料1982 | 10.738635 MHz（±0.005%）＝3 fsc | VDPがCPUCLK＝水晶/3＝3.58 MHzを出力（TMS9918Aのみ） | 水晶/2＝画素クロック5.3 MHz | 主クロックから6相の3.579545 MHz（±10 Hz）を内部生成 | [D TI82 §3.6（3-8）] |
| Commodore-Amiga（Amiga 1000） | 1985 | 未確認（28.63636 MHz＝8 fsc と推定） | 68000 7.15909 MHz（＝2 fsc）。メモリはその2倍の速度 | 色レジスタ32本、各12ビット（4096色中32色） | — | CPU・色 [D BY85 PDF p.90, 93, 95]。2 fscは [D: 計算]。水晶値 [V] |
| Atari ST | 原著1985 | MMUに32 MHz入力。GLUEが8 MHz、2 MHz（音源）、500 kHz（キーボード・MIDI）を生成 | 68000 8 MHz、E＝800 kHz | SHIFTERにパレットレジスタ16本 | 未確認（NTSC版の水晶値・エンコーダ） | [D ST85 PDF p.16, 22, 24, 25, 54, 75]。色系 [V] |
| Ensoniq Mirage のES5503 | 1984 | DOCに8 MHz | — | — | — | [D MM enmirage.cpp L628]（MAMEのエミュレーション参照。実機回路は未照合） |
| Apple IIGS のES5503 | 1986（後年） | 28.636363 MHz/4＝7.16 MHz をDOCへ | — | — | — | [D MM apple2gs.cpp L96–98, L4148]（後年・エミュレーション参照） |

### 1.2 一次資料で確認していない例（参考） [V]

| 機種 | 一般に言われる構成 | 状態 |
|---|---|---|
| Apple II（1977）、Commodore 64（NTSC）、IBM PC（1981） | 14.31818 MHz（4 fsc）から分周。IBM PCはCPU 4.77 MHz（÷3） | [V] 本調査で原典を未照合 |
| ファミコン／NES（1983） | 21.47727 MHz（6 fsc）。CPU ÷12、PPU ÷4（5.37 MHz）、256画素・262ライン | [V] 後年の解析資料のみで、1985年以前の公開資料はない |

### 1.3 傾向

- 色副搬送波に同期させる機種は、fscの3倍（10.74 MHz）・4倍（14.32 MHz）・6倍（21.48 MHz）・8倍（28.64 MHz）の水晶を使い、CPUもそこから分周する。CPUがちょうど8 MHzにはならない（7.16 MHz、3.58 MHz、4.77 MHz等）[D TI82; BY85]、[V]（4 fsc系の例）。
- Atari STはCPU 8 MHzを優先し、ACIA用の500 kHzも同じ系から作る [D ST85 PDF p.22, 54, 75]。
- 民生用の映像ICでは、主クロックの1/2を画素クロックにする例（TMS9918A）がある [D TI82 §3.6]。

---

## 2. fc85 のクロック要求

| 領域 | 要求 | 根拠 |
|---|---|---|
| CPU（MC68000） | 8 MHz（仕様書§04・§07の暫定値）。8 MHz品のCLKは2.0〜8.0 MHz、パルス幅tCL/tCH≥55 ns | 仕様書 [P]、[D M68 §8.5（PDF p.83）] |
| 68000 E | CLK/10、6クロック低・4クロック高。8 MHzで800 kHz | [D M68 §4.1.7.1, §6.1]、[D: 計算] |
| Data Fabric | 8 MHz候補 | 仕様書§07 [P] |
| ACRTC（HD63484-8） | 2CLK 8 MHz以下。2CLKはドットクロックの2倍、MCYCは2CLKの1/2。水平表示レジスタはメモリサイクル単位 | [D HDS 端子説明・タイミング制御]（年代不明、後年の可能性） |
| DSP（TMS32010） | 水晶またはCLKIN 20 MHz級（TMS32010は20.5 MHzまで）。CLKOUTは1/4 | [D TMS32010台帳 T2 A-3, A-13] |
| DOC（ES5503） | 約7 MHz候補。前例はMirage 8 MHz、IIGS 7.16 MHz | 仕様書§07 [P]、[D MM]（エミュレーション参照） |
| ADPCM（MSM5205） | 384 kHzの水晶またはセラロック | [D OKI87 PDF p.29–30]（後年資料。仕様書[7]と同系統） |
| ACIA（MC6850） | 500 kHz（÷16で31.25 kbit/s）。÷16/÷64時の上限は0.8 MHz（MC6850） | [D M8B 3-496]。詳細は `MC6850.md` |
| MIDI | 31.25 kbit/s ±1% | [D MI1]（後年の要約。1983年規格原典は未照合 [V]） |
| 色副搬送波 | 3.579545 MHz | [D TI82 §3.6.1]。315/88 MHz は [D: 計算] |
| 画素クロック | 256画素を画面幅に収める値。前例は5.37 MHz | [D TI82 §3.6, Table 3-3] |

---

## 3. 256画素のNTSC表示と240p

### 3.1 前例：TMS9918A [D TI82 §3.6.2, Table 3-3（3-8）]

| 項目 | 画素クロック数・ライン数 |
|---|---|
| 水平有効表示（Graphics/Multicolor） | 256 |
| 1ラインの合計 | 342 |
| 垂直有効表示 | 192 |
| 1フレームの合計 | 262（ノンインタレース、約60フレーム/秒） |
| 垂直の内訳 | 有効192、下ボーダー24、下ブランキング3、垂直同期3、上ブランキング13、上ボーダー27 |

fc85の256×192はTMS9918Aと同じ画面構成で、1985年の家庭用として自然である。

### 3.2 fc85の走査候補 [D: 計算]

| 案 | ドットクロック | 1ライン | 水平周波数 | フィールド | 256画素の幅 | fsc/ライン |
|---|---|---|---|---|---|---|
| 6 fsc÷4、342ドット×262ライン | 5.369318 MHz | 63.695 µs | 15.700 kHz | 59.923 Hz | 47.68 µs | 228.000 |
| 6 fsc÷4、341ドット×262ライン | 5.369318 MHz | 63.509 µs | 15.746 kHz | 60.098 Hz | 47.68 µs | 227.333 |
| 4 fsc÷2、455ドット×262ライン | 7.159091 MHz | 63.556 µs | 15.734 kHz | 60.054 Hz | 35.76 µs | 227.500 |
| 16 MHz÷3、340ドット×262ライン | 5.333333 MHz | 63.750 µs | 15.686 kHz | 59.871 Hz | 48.00 µs | 228.196 |
| 参考：NTSC標準 525/59.94i | — | 63.556 µs | 15.734 kHz | 59.940 Hz | — | 227.5 |

### 3.3 仕様書§07との関係

- NTSC標準は1ライン227.5 fsc、525ライン2:1インタレースで59.94フィールド/秒 [D: 計算]。
- 192ラインを毎フィールド同じ位置に出す順次走査（240p）は、1フィールド262ラインの整数になるため、フィールド周波数は59.92 Hz（342ドット案）または60.05〜60.10 Hz になり、59.94 Hzちょうどにはならない [D: 計算]。
- 59.94 Hzを守るには262.5ラインのインタレース同期を出し、両フィールドに同じ192ラインを描く方法になるが、半ライン分の上下揺れが出る [V]。
- 家庭用のノンインタレース出力は1982年のTMS9918Aで明記されている [D TI82 §3.6.2]。MC1377は「NTSCではブロック状の垂直同期でよい」と書いており、簡略化した同期を受け付ける [D MLI 10-80]。1988年のLM1881資料には「ビデオゲーム・ホームコンピュータの非標準映像信号」向けの機能が書かれている [D N88 PDF p.513]（後年資料）。
- 修正案 [P]：§07を「NTSC互換コンポジット、262ライン・ノンインタレース（240p相当）。6 fsc÷4＝5.369 MHz、342ドット×262ラインで約59.92 Hz」とし、「525/59.94i」は外す。
- ホスト出力（§12の59.94 Hz）との差は0.0173 Hzで、約57.8秒ごとに1フレームずれる [D: 計算]。エミュレータは仮想機の時刻を有理数で持ち、ホスト表示側で間引き／複製する（§12の方針どおり）[P]。
- 仕様書§10の4 fsc（14.32 MS/s）基準のコンポジットモデルと、ドット1.5 fscは整数比にならない。12 fsc（42.95 MHz）を共通の時間格子にすると、ドット＝12 fsc/8、4 fsc＝12 fsc/3 と両方整数になる [D: 計算]／[P]。

---

## 4. ACRTC（HD63484）と画素クロックの関係（要確認）

HD63484の端子説明 [D HDS]：
- 「2CLK (Dot Clock)：ACRTCの基本動作クロックで、ドットクロックの2倍の周波数」。
- 「MCYC (Memory Cycle)：フレームバッファのアクセスタイミング出力。2CLKの1/2の周波数」。
- 「水平表示制御レジスタはメモリサイクル単位、垂直はラスタ単位で設定」。
- 特長欄に「シングルアクセス／デュアルアクセス」「表示と重ね合わせのためのインタリーブアクセスモード」がある。

fc85のVRAMは16-bit wordで、4bppなら1ワード＝4画素。256画素は64ワード／ライン [D: 計算]。

| 2CLK | MCYC | 毎MCYC表示（4画素/MCYC） | 2MCYCに1ワード | 3MCYCに1ワード |
|---|---|---|---|---|
| 8.000 MHz | 4.000 MHz | 16.0 MHz、256画素＝16.0 µs | 8.0 MHz、32.0 µs | 5.333 MHz、48.0 µs |
| 7.159 MHz（6 fsc÷3） | 3.580 MHz | 14.32 MHz、17.9 µs | 7.16 MHz、35.8 µs | 4.77 MHz、53.6 µs |
| 5.369 MHz（6 fsc÷4） | 2.685 MHz | 10.74 MHz、23.8 µs | 5.369 MHz、47.7 µs | 3.58 MHz、71.5 µs |
| 5.333 MHz（16 MHz÷3） | 2.667 MHz | 10.67 MHz、24.0 µs | 5.333 MHz、48.0 µs | 3.56 MHz、72.0 µs |

[D: 計算]

- ACRTC 8 MHzで毎メモリサイクル表示すると、256画素は16 µsしかなく、NTSCの有効期間（約52 µs）の3割になる [D: 計算]。
- 256画素を約48 µsに広げるには、「3メモリサイクルに1ワード表示（2CLK 8 MHz）」か「2メモリサイクルに1ワード表示（2CLK 5.33〜5.37 MHz）」が必要。ACRTCがこの比を作れるか（アクセスモード・ズーム・表示幅の設定）は未確認 [V]。
- 2CLKを下げると描画速度も下がる（HD63484の描画時間は2CLKサイクル数で決まる [D HDS 描画時間表の注]）。仕様書§07の「ACRTC 8 MHz候補」と「256×192のNTSC全幅表示」は同時に成り立たない可能性がある。
- 照合結果（同ディレクトリ `HD63484.md` §3.3〜3.6、別担当の台帳）：デュアルアクセスかつGAI＝+1なら4×2CLK（＝2メモリサイクル）ごとに4画素で、ドットクロック＝2CLKになる。同台帳は2CLK＝6 fsc÷4＝5.369318 MHz（案B）を推奨し、262ラインのノンインタレース（RSM＝00、VC＝262）も直接設定できるとしている。また2CLK 8 MHzでは1周期125 nsで、1985年のDRAMで組める根拠がないとしている。本書の「2メモリサイクルに1ワード（2CLK 5.369 MHz）」と一致する。「3メモリサイクルに1ワード（2CLK 8 MHz）」は同台帳で扱われておらず未確認 [V]。

---

## 5. クロックツリー候補

水晶の数には、MSM5205用の384 kHz発振子も含めて書く。

### 案1（推奨）：ホスト系16 MHz＋映像系6 fsc＋DSP 20 MHz

| 発振源 | 分周 | 供給先 | 周波数 | 備考 |
|---|---|---|---|---|
| X1 16.000 MHz | ÷2 | CPU、Data Fabric | 8.000 MHz（デューティ50%） | 仕様書どおり [P] |
| X1 | ÷2 | DOC（案1a） | 8.000 MHz | Mirage前例（MAME） [D MM]／[P] |
| X1 | ÷32 | ACIA Tx/Rx CLK | 500.000 kHz（MIDI誤差0） | [D: 計算] |
| （CPU内部） | ÷10 | E（ACIA等） | 800 kHz | [D M68 §4.1.7.1] |
| X2 21.47727 MHz（6 fsc） | ÷4 | ドットクロック | 5.369318 MHz | TMS9918Aと同じ画素クロック [D TI82] |
| X2 | ÷4（ドットクロックと共用） | ACRTC 2CLK | 5.369 MHz（デュアルアクセスでドット＝2CLK） | [P]。根拠は `HD63484.md` §3.3（案B） |
| X2 | ÷6 | 色副搬送波（MC1377の外部入力） | 3.579545 MHz | MC1377は0.25 Vrmsの外部副搬送波を受けられる [D MLI 10-79, 10-80] |
| X2 | ÷8 | 映像入力のサンプリング | 2.684659 MHz | §7.4 [P] |
| X2 | ÷3 | DOC（案1b） | 7.159 MHz | IIGS前例（後年）。÷3はデューティ補正が必要 [D: 計算] |
| X3 20.000 MHz | そのまま | TMS32010 CLKIN | 20 MHz（CLKOUT 5 MHz、200 ns） | [D TMS32010台帳 T2] |
| X4 384 kHz発振子 | そのまま | MSM5205 | 384 kHz | [D OKI87 PDF p.29–30]（後年資料） |

- 水晶・発振子：4個（16 MHz、21.47727 MHz、20 MHz、384 kHz）。
- クロック領域：ホスト（16 MHz系）、映像（6 fsc系）、DSP（20 MHz）、ADPCM（384 kHz）の4つ。CPU↔ACRTC、Fabric↔ACRTC、Fabric↔DSP、Fabric↔ADPCMで同期化回路が要る [P]。68000バスは非同期（DTACK）なので、ホスト↔映像の非同期自体は前例どおり扱える [D M68 §6.1 の非同期バスの説明]。
- 長所：CPU 8 MHz・MIDI誤差0を保ったまま、映像をfscに同期できる（ドット＝1.5 fsc、1ライン228 fsc）[D: 計算]。
- 短所：ACRTC 2CLKが8 MHzより低くなる可能性（§4）。水晶が多い。

### 案2：6 fsc 単一マスター（Amiga／IIGS型）＋DSP 20 MHz

| 発振源 | 分周 | 供給先 | 周波数 | 備考 |
|---|---|---|---|---|
| X1 21.47727 MHz（6 fsc） | ÷3（デューティ50%化） | CPU、Fabric、DOC、ACRTC 2CLK（候補） | 7.159091 MHz | 単純な÷3では高レベル46.6 nsで68000の55 nsを満たさない。両エッジを使う÷3で各69.8 nsにする [D M68 §8.5]、[D: 計算] |
| X1 | ÷4 | ドットクロック | 5.369318 MHz | |
| X1 | ÷6 | 色副搬送波 | 3.579545 MHz | |
| X1 | ÷43 | ACIA | 499.471 kHz（MIDI −0.11%） | ±1%以内 [D: 計算]。÷43はプリセット付きカウンタ |
| X1 | ÷56 | MSM5205（外部クロック駆動できる場合） | 383.523 kHz（−0.12%） | MSM5205を外部クロックで駆動できるかは未確認 [V]。できなければ384 kHz発振子を追加 |
| X2 20.000 MHz | そのまま | TMS32010 | 20 MHz | 21.48 MHzはTMS32010の上限20.5 MHzを超える [D TMS32010台帳 T2] |

- 水晶・発振子：2〜3個。
- 長所：CPU・映像・DOCが同期し、エミュレーションの決定性が高い。DOCが仕様書の「約7 MHz」に一致する。
- 短所：CPUが7.16 MHz（8 MHz比 −10.5%）になり、仕様書§04・§07の8 MHzを変更する。÷3と÷43の特殊な分周器が要る。

### 案3：16 MHz 単一系（Atari ST型、映像をfscに同期させない）＋エンコーダ専用水晶

| 発振源 | 分周 | 供給先 | 周波数 | 備考 |
|---|---|---|---|---|
| X1 16.000 MHz | ÷2 | CPU、Fabric、DOC、ACRTC 2CLK | 8.000 MHz | |
| X1 | ÷3 | ドットクロック | 5.333333 MHz（340ドット×262ラインで59.87 Hz） | ACRTCが3MCYCに1ワード表示できれば2CLK 8 MHzのまま256画素＝48 µs（§4、[V]） |
| X1 | ÷32 | ACIA | 500.000 kHz | |
| X2 3.579545 MHz | MC1377内蔵発振 | 色副搬送波 | 3.579545 MHz | MC1377は内蔵コルピッツ発振で自走できる [D MLI 10-80] |
| X3 20.000 MHz | — | TMS32010 | 20 MHz | |
| X4 384 kHz | — | MSM5205 | 384 kHz | |

- 水晶・発振子：4個。
- 長所：CPU・ACRTC・Fabric・DOC・MIDIが1つの系にそろい、ACRTCを8 MHzで使える可能性がある。
- 短所：画素と色副搬送波が非同期で、コンポジットでは画素境界と色信号のうなり（ドット妨害の動き）が出る [V]。水平周波数15.686 kHzはNTSC標準から−0.3%で、受像機の追従範囲内かは未確認 [V]。

### 比較

| 項目 | 案1 | 案2 | 案3 |
|---|---|---|---|
| 水晶・発振子 | 4 | 2〜3 | 4 |
| CPU | 8.000 MHz | 7.159 MHz | 8.000 MHz |
| DOC | 8.000 または 7.159 MHz | 7.159 MHz | 8.000 MHz |
| ACRTC 2CLK | 5.369 MHz | 5.369 MHz（ドット＝2CLK） | 8.000 MHz（要確認） |
| ドット | 5.369 MHz | 5.369 MHz | 5.333 MHz |
| fsc同期 | あり | あり | なし |
| MIDI誤差 | 0 | −0.11% | 0 |
| 仕様書からの変更 | ACRTCクロックのみ | CPU 8→7.16 MHz | 映像のfsc非同期 |

推奨 [P]：案1。仕様書のCPU 8 MHzとMIDIを保ったまま、映像系をTMS9918Aと同じ5.37 MHz／262ラインに合わせられる。ACRTCの2CLKは、HD63484台帳の案B（デュアルアクセスでドット＝2CLK）に合わせて5.369 MHz（X2÷4、ドットクロックと共用）とする。エミュレーションの単純さを優先するなら案2。

---

## 6. 映像出力系（RGB→NTSCコンポジット）

### 6.1 パレット

| 方式 | 前例・資料 | 判定 |
|---|---|---|
| カスタムチップ内蔵パレット | Atari ST（SHIFTERにパレットレジスタ16本）[D ST85 PDF p.25]、Amiga（12ビットの色レジスタ32本）[D BY85 PDF p.95] | 1985年の前例あり |
| パレットRAMとDACを1チップにしたIC（RAMDAC） | 1985年以前の資料では確認できなかった。Brooktree Bt451（256×12のLUT＋3ch 4-bit DAC）の登場年は未確認。IBM VGA（1987）がInmos G171を使ったという二次情報のみ | [V]（1985年には使わない） |
| 映像用DAC単体 | TRW TDC1334：3ch 4-bit、100 MSPS、同期・ブランク・ブライト入力、ECL互換（TTLでも動作可）、−5.2 V単電源、28ピンセラミックDIP。1985年版は「Advance Information」（Rev. A-1/85） | [D T85 PDF p.24]。供給実態 [V] |
| TTL＋SRAM＋抵抗DAC | 16語×12ビットの高速SRAM（または同等のレジスタ）と抵抗ラダー。fc85は独自TTL・PAL・ゲートアレイを許容（仕様書§01） | 構成は [P]、具体品番（74S189等）は未照合 [V] |

画素周期は5.369 MHzで186 ns [D: 計算]。パレット読出し＋ラッチをこの周期に収める必要がある [P]。

推奨 [P]：TTL＋高速SRAM（またはゲートアレイ内レジスタ）でRGB各4ビットを出し、抵抗DAC＋エミッタフォロワでアナログRGBにする。TDC1334は性能過剰で負電源が要るため代替 [O]。

### 6.2 エンコーダ

| IC | 年代根拠 | 内容 | 本機での判定 |
|---|---|---|---|
| Motorola MC1377「Color Television RGB to PAL/NTSC Encoder」 | 1983年Motorola資料（Advance Information） | RGB・同期入力→コンポジット。色副搬送波発振器（内蔵または外部駆動）、90°移相器、DSB変調器2個、RGBマトリクス、ブランキングレベルクランプ。PAL/NTSC切替。電源12±2 V。RGB 1.0 Vp-pで飽和度100%。コンポジット出力約2.5 Vp-p（直流基準付き）、出力インピーダンス≤100 Ω。輝度帯域8 MHz（遅延線なし）。外部副搬送波は0.25 Vrms。NTSCではブロック状の垂直同期でよい | 第一候補。75 Ω出力段（バッファ）を別に付ける [D MLI 10-78〜10-80]／[P] |
| National LM1886「TV Video Matrix D to A」 | 1982年National資料 | 3ビットのR・G・B入力からY、R−Y、B−Yを作る。NTSC行列。TTL入力。LM1889と組んでRF変調まで完結 | RGB各3ビットのためfc85の4ビットには合わない。不採用 [D N82 11-23] |
| National LM1889「TV Video Modulator」 | 1982年National資料 | 音声副搬送波・色副搬送波発振器、直交色変調器、RF発振・変調器（低VHF 2チャネル）。12〜18 V。「コンポジット映像の生成にも使える」 | [O] 自前のY/R−Y/B−Y行列と組む場合の代替。RF出力が要る場合も候補 [D N82 11-28] |

同期：ACRTCのHSYNC・VSYNCから複合同期を作りMC1377の同期入力へ入れる [P]。ACRTCの同期出力の仕様はHD63484台帳で確認する [V]。

---

## 7. 映像入力系

### 7.1 同期分離IC

| IC | 1985年以前の資料 | 内容 | 判定 |
|---|---|---|---|
| National LM1881「Video Sync Separator」 | 1982年Linear Databookと1984年Linear Supplementには載っていない（全文検索）。1988年Linear Databook Vol.3の目次に「＊」付きで載る | 複合同期・垂直同期・バースト／バックポーチ・奇偶フィールド出力。8ピン | 1985年機には使わない。登場は1985〜1988年の間と推定（「＊」は新規掲載の印と推定）[D N82, N84（不在）; N88 PDF p.7, 461, 513]、年 [V] |
| National TDA2591/TDA2593「Line Oscillator Combination」 | 1982年 | 同期分離、雑音分離、垂直同期分離、バーストキーイング、同期と発振器の位相比較 | 1985年の同期分離候補 [D N82 11-90] |
| National LM1880「No-Holds Vertical/Horizontal」 | 1982年 | 水平周波数32倍のVCO（セラミック発振子）、負極性同期へのロック、垂直同期ウィンドウ、バーストゲート出力 | 同期・タイミング生成の候補 [D N82 11-16] |
| 個別部品（クランプ＋比較器） | — | 同期チップを比較器で切り出す一般的な方法 | [V] |

### 7.2 ADC

| IC | 内容 | 判定 |
|---|---|---|
| TRW TDC1007 | 8ビット20 MSPSフラッシュ、64ピン、+5/−6 V、2.7 W | 4 fscの生波形取得用の上位候補 [D T85 D4, D15] |
| TRW TDC1048 | 8ビット20 MSPSフラッシュ、28ピン、+5/−5.2 V、1.6 W（1984年版は1.4 W）、用途に「Low-Cost Video Digitizing」 | 128×96輝度取得の第一候補 [D T85 D4, D127; T84 PDF p.114] |
| TRW TDC1014 | 6ビット25 MSPS、1.1 W、「Low-Cost Video Digitizing」 | 6ビット仕様に下げる場合 [D T85 D4, PDF p.68] |
| TRW TDC1001 | 8ビット2.5 MSPS逐次比較、0.7 W | 128画素の速度（約2.4〜2.7 MSPS）にはぎりぎりで、サンプルホールドも必要 [D T85 D4]、[V] |

詳細は `TDC1007.md`。

### 7.3 1985年前後の民生用デジタイザとフレームグラバ

| 製品 | 年 | 方式・性能 | 価格 | 根拠 |
|---|---|---|---|---|
| 一般論（1985年の解説記事） | 1985 | 多くのデジタイザは逐次比較型A/Dで、1ラインに1画素しか取れない。3ビット階調なら3回走査。高価な機種はフラッシュA/Dを使う。256×256・8ビットは高級機の標準で、約66 KBのメモリが要り家庭用には過剰 | — | [D CC85a] |
| ComputerEyes（Digital Vision、Apple II版は1984年10月に誌上レビュー、C64版1985） | 1984–85 | スロースキャン。高コントラスト画像は6秒未満。階調画像はしきい値を変えた多重走査で、8階調なら最大50秒 | — | [D CC85b] |
| ComputerEyes（Atari版） | 1985 | 「1分ほどで」画像が出る | 白黒カメラ付きで399.95ドル | [D AN85] |
| Koala MacVision（Macintosh） | 1985 | 320×240、1画素8ビット。小窓で約5秒、全画面22秒 | — | [D CC85b; CP85] |
| Micro Works Digisector DS-88（IBM PC） | 1985 | 512×512、最大256階調。256×256で4秒 | — | [D CC85b] |
| Scion FAX640（IBM PC用ボード） | 1985 | RS-170白黒1フレームを1/30秒で16階調・640×408または544×480に取り込む実時間フレームグラバ。画像メモリはボード上 | — | [D CC85b] |
| PCvision Frame Grabber（IBM PC/XT用ボード） | 1985 | 実時間取り込み。1画素6ビット、ボード上に512×512のフレームメモリ。毎秒30フレームの取り込みと表示を同時に行える。出力に書換え可能なルックアップテーブル。内蔵水晶で同期信号を生成でき、3枚でカラー処理 | — | [D CC85b] |
| Chorus PC-Eye PC-1200（IBM PC用ボード） | 1985 | 最大640×512×8ビット。640×480×8ビットを0.6秒で取り込み | — | [D CC85b] |
| Private Eye（Macintosh用） | 1985 | 1フレームを実時間（1/30秒）で取り込み、512×512画素の画像にする。階調数は記事に記載なし | — | [D CC85b] |
| NewTek Digi-View（Amiga） | 1986 | （1985年の比較対象外） | 199.95ドルとの二次情報 | 年 [D DV]（後年資料）、価格 [V] |
| 富士通FM77AV用ビデオディジタイズカード | 未確認 | FM77AV本体は1985年10月発表。カードの発売時期は未確認 | — | [V] |

### 7.4 fc85「専用入力基板で128×96・8-bit・15 Hz」の妥当性

計算 [D: 計算]：
- 1ラインの有効期間（約48〜52.6 µs）に128サンプルなら2.43〜2.68 MSPS。
- 1画面12,288 B、15 Hzで184,320 B/s（仕様書§07の約0.18 MB/sと一致）。
- 6 fsc÷8＝2.684659 MHzでサンプルすると、出力の256画素（6 fsc÷4）のちょうど2画素に1サンプルになる。

構成案 [P]：
1. 75 Ω終端 → クランプ（バックポーチまたはシンクチップ） → 約1.2 MHzのLPF（2.68 MSPSのナイキスト1.34 MHz以下。3.58 MHzの色信号も同時に落ちる） → 利得2のバッファ → ADC（TDC1048推奨）。
2. 同期分離（TDA2593、LM1880、または個別回路）でH・Vを取り出し、サンプリングクロックをHごとに位相リセット（最大1クロック＝373 nsの揺れ）するか、ラインロックPLLで作る [V]。
3. 1フィールドの192ライン中、1ライン置きに96ラインを取り込み、4フィールドに1回（60/4＝15 Hz）取り込む。残り3フィールドでCapture RAMの所有権を移す（仕様書§05・§10の「フレーム完了後に所有権を移す」と整合）。
4. 書込みは2.68 MB/sのバースト（373 ns/バイト）で、1985年のSRAMで足りる [V]（品番未選定）。Capture RAM 64 KiBに5画面入る。

判定：
- 技術的には1985年以前の資料にある部品（TDC1048等、TDA2593、SRAM、TTL）で組める [D T85; N82]／構成 [P]。
- ただし1985年の民生用は数秒〜1分のスロースキャンが主流で、実時間取り込みはPC用拡張ボード（16階調〜6ビット、毎秒30フレーム）とMac用外付け機に限られていた [D CC85a; CC85b; AN85]。8ビット・15 Hzの実時間取り込みは家庭用としては業務用相当の上位仕様で、フラッシュADCの電力（1.6〜2.7 W）と負電源も必要になる [D T85 D4]。
- フィクション設定（業務用電子機器メーカーの「盛り癖」、採算を問わない）とは整合する。リアリティを上げるなら、(a) ADCをTDC1007からTDC1048に替える、(b) 6ビット（TDC1014）を廉価案として併記する、(c) 入力基板を別売／別基板とする（仕様書§03の「基板分離を許容」と同じ）[P]。

---

## 8. 仕様書との照合

| 仕様書の節・記述 | 判定（一致／不一致／要確認） | 根拠・修正案 |
|---|---|---|
| §07「Video：NTSC 525/59.94i目標。内部更新59.94 Hz目標」 | 不一致 | 192ラインの順次走査は262ライン・ノンインタレースで約59.92 Hz（342ドット案）になり、525/59.94iにはならない [D TI82 §3.6.2]、[D: 計算]。「NTSC互換240p、6 fsc÷4、342×262、約59.92 Hz」に改める案 |
| §07「走査タイミングのカウンタ・分周設計は未確定」 | 要確認（候補提示） | §3.2、§5の案1〜3 |
| §07「CPU / Fabric / ACRTC 各8 MHz候補」 | 要確認 | ACRTCは2CLKとドットクロックが結ばれる [D HDS]。HD63484台帳§3.3は、デュアルアクセスでドット＝2CLKとなるため2CLK 5.369 MHz（6 fsc÷4）を推奨し、8 MHzは1985年のDRAMで組める根拠がないとしている（§4）。CPU・Fabricは8 MHzのまま、ACRTCだけ「2CLK 5.369 MHz（映像系クロック）」に改める案 |
| §07「DOC 約7 MHz候補」 | 要確認 | 1984年のMirageは8 MHz、7.16 MHzは1986年のIIGSの値（いずれもMAMEのエミュレーション参照）[D MM]。案1ならDOC 8 MHzも選べる |
| §07「DSP 20 MHz入力級」 | 一致 | TMS32010は20.5 MHzまで [D TMS32010台帳 T2]。6 fsc（21.48 MHz）からは供給できないので独立水晶 |
| §07 クロック表 | 要確認 | ACIA 500 kHzとMSM5205 384 kHzの発振源が表にない。§5の表を追加する案 |
| §07「256×192、4bpp…RGB各4-bitパレット、NTSCコンポジットとRGB出力」 | 一致 | Amiga（1985）が12ビット色レジスタを持つ [D BY85]。STは16本のパレット [D ST85] |
| §07「外付けラッチ・パレット・DAC・エンコーダのタイミングは別設計」 | 要確認（候補提示） | パレットはTTL＋SRAM、DACは抵抗ラダー、エンコーダはMC1377（1983年資料）[D MLI]。RAMDACは1985年以前の資料で確認できない [V] |
| §10「1985年型はRGB/輝度・色差、色副搬送波、同期合成、75 Ω出力回路を別途設計する。[13]」 | 要確認 | [13]は現代部品の資料。1985年側の根拠としてMC1377（Motorola 1983）とLM1886/LM1889（National 1982）を§14に追加する案 |
| §10「最初の1985年モデルは専用入力基板による輝度取得を優先し、128×96、8-bit、15 Hz相当を暫定目標」 | 要確認 | 部品は1985年以前の資料にあるが、民生用としては上位仕様（§7.4）。成立性の根拠と構成案を本書§7.4に記載 |
| §10「時代相当の生波形取得候補はTRW TDC1007等 [9]」 | 一致（補足） | TDC1007は1984・1985年資料に掲載 [D T84; T85]。輝度128×96用にはTDC1048がより適合 |
| §10 入力基板の図「75 Ω終端・クランプ・同期分離・帯域制限」 | 一致 | 同期分離ICは1982年のTDA2593／LM1880が候補。LM1881は1985年以前の資料にない [D N82; N84; N88] |
| §10「コンポジット信号モデル…4fsc（約14.32 MS/s）を基準」 | 要確認 | ドット（1.5 fsc）と4 fscは整数比にならない。12 fscを共通格子にする案（§3.3） |
| §09「MIDI 31.25 kbit/s」 | 一致 | 500 kHz÷16。発振源は§5 [D M8B 3-496] |
| §12「PC上で59.94 Hz出力」 | 要確認 | 仮想機59.92 Hzとの差で約58秒に1フレームずれる。ホスト側で吸収する方針を明記する案 |
| §13 次版の優先事項4「1985年映像入力基板のADC…出力のDAC／エンコーダ候補を選定し、時代整合性を確認」 | 一部完了 | ADC：TDC1048／TDC1007、同期分離：TDA2593／LM1880、エンコーダ：MC1377、DAC：抵抗ラダー（TDC1334は代替） |
| §14 参照資料 | 要確認 | TI TMS9918A Data Manual（1982）、Motorola DL128（1983）、National Linear Databook（1982）、Creative Computing 1985-07 を追加する案 |

---

## 9. 未確定事項 [V]

1. HD63484で「3メモリサイクルに1ワード表示」（案3の2CLK 8 MHz）が設定できるか（2メモリサイクルに1ワードはHD63484台帳§3.3で成立）。68000 8 MHzとACRTC 2CLK 5.37 MHzの非同期接続時のウェイト数。ACRTCの同期出力でMC1377を駆動できるか。
2. Amiga 1000の水晶値（28.63636 MHz）と、Atari ST NTSC版の水晶値・エンコーダ構成の一次資料。
3. LM1881の初出年（1985〜1988年のNational資料）と、1988年版目次の「＊」の意味。
4. RAMDAC（Brooktree Bt451等、Inmos G17x）の登場年。
5. パレット用の高速SRAM・TTLの具体品番と1985年の入手性。MC1377の1985年の量産状態（1983年はAdvance Information）。
6. 案3の水平周波数15.686 kHzが当時の受像機で問題ないか。fsc非同期時の画質。
7. MSM5205を外部クロックで駆動できるか（案2で384 kHz発振子を省く場合）。
8. 日本国内の1985年の民生用映像デジタイザ（FM77AV用カード等）の発売時期・方式。
9. 入力基板のサンプリングクロック方式（H同期ごとの位相リセットかラインロックPLLか）と、その揺れが128画素で許容できるか。

---

## 出典

| ID | 資料（年、ページ、URL） |
|---|---|
| TI82 | Texas Instruments, *TMS9918A/TMS9928A/TMS9929A Video Display Processors Data Manual*（1982年11月）。§3.6 Oscillator and Clock Generation、§3.6.1、§3.6.2、Table 3-3（3-8、PDF p.51）。http://www.bitsavers.org/components/ti/TMS9900/TMS9918A_TMS9928A_TMS9929A_Video_Display_Processors_Data_Manual_Nov82.pdf |
| M68 | Motorola, *MC68000 16-Bit Microprocessor*（1983年4月）。§4.1.7（PDF p.32）、§6.1–6.2（PDF p.66–67）、§8.5 Clock Timing（PDF p.83）。http://www.bitsavers.org/components/motorola/68000/68000/MC68000_16-Bit_Microprocessor_Apr83.pdf |
| M8B | Motorola, *8-Bit Microprocessor and Peripheral Data*（1983）。MC6850 3-494〜3-502（PDF p.538–546）。http://www.bitsavers.org/components/motorola/_dataBooks/1983_Motorola_8-Bit_Microprocessor_and_Peripheral_Data.pdf |
| MLI | Motorola, *Linear and Interface Integrated Circuits* DL128（1983）。MC1377 10-78〜10-80（PDF p.1227–1229）、選択ガイド（PDF p.75）。http://www.bitsavers.org/components/motorola/_dataBooks/1983_DL128_Motorola_Linear_and_Interface_Integrated_Circuits.pdf |
| N82 | National Semiconductor, *Linear Databook*（1982）。TV Circuits：LM1880 11-16（PDF p.1751）、LM1886 11-23（PDF p.1758）、LM1889 11-28（PDF p.1763）、TDA2591/TDA2593 11-90（PDF p.1825）。LM1881は全文検索で該当なし。http://www.bitsavers.org/components/national/_dataBooks/1982_National_Linear_Databook.pdf |
| N84 | National Semiconductor, *Linear Supplement*（1984）。LM1881は全文検索で該当なし。http://www.bitsavers.org/components/national/_dataBooks/1984_National_Linear_Supplement.pdf |
| N88 | National Semiconductor, *Linear Databook Vol.3*（1988）。**後年資料**。目次（PDF p.7, p.461）、LM1881（PDF p.513）。http://www.bitsavers.org/components/national/_dataBooks/1988_National_Linear_Databook_Vol_3.pdf |
| T85 | TRW LSI Products, *VLSI Data Book*（1985）。TDC1334（PDF p.24）、A/D選択ガイドD4（PDF p.45）、TDC1007 D15〜D26、TDC1014（PDF p.68）、TDC1048 D127〜。https://bitsavers.trailing-edge.com/components/trw/_dataBooks/1985_TRW_VLSI_Data_Book.pdf （仕様書§14 [9]） |
| T84 | TRW LSI Products, *VLSI Data Book*（1984）。TDC1007（PDF p.32）、TDC1048（PDF p.114）。http://www.bitsavers.org/components/trw/_dataBooks/1984_TRW_VLSI_Data_Book.pdf |
| ST85 | K. Gerits, L. Englisch, R. Bruckmann, *Atari ST Internals*（Data Becker／Abacus。Copyright 1985、英語版第3刷1986年7月）。PDF p.16, 22, 24, 25, 54, 75。https://www.atarimania.com/documents/Atari-ST-Internals.pdf |
| BY85 | BYTE Vol.10 No.8（1985年8月）、G. Williams ほか "The Amiga Personal Computer"。PDF p.90（68000 7.15909 MHz）、p.93、p.95（12ビット色レジスタ、4096色中32色）。https://www.vintageapple.org/byte/pdf/198508_Byte_Magazine_Vol_10-08_The_Amiga.pdf |
| CC85a | Creative Computing Vol.11 No.7（1985年7月）p.65 "Digital image processors"。https://www.atarimagazines.com/creative/v11n7/65_Digital_image_processors.php （原誌スキャン：https://archive.org/details/creativecomputing-1985-07/page/n68/mode/1up ） |
| CC85b | Creative Computing Vol.11 No.7（1985年7月）p.70 "Choosing an image processor"。https://www.atarimagazines.com/creative/v11n7/70_Choosing_an_image_process.php （原誌スキャン：https://archive.org/details/creativecomputing-1985-07/page/n75/mode/1up ） |
| CP85 | COMPUTE! Issue 60（1985年5月）p.60 "MacVision For Apple Macintosh"。https://www.atarimagazines.com/compute/issue60/188_1_REVIEWS_MacVision_For_Apple_Macintosh.php |
| AN85 | ANTIC Vol.4 No.8（1985年12月）"Video Star Atari"（ComputerEyes）。https://atarimagazines.com/v4n8/VideoStar.php |
| HDS | Hitachi, HD63484 ACRTC データシート（仕様書§14 [3] の mega-micros 掲載版。発行年不明、9.8 MHz版の記述を含むため1986年以降の可能性＝**後年資料扱い**）。端子説明（2CLK、MCYC）、タイミング制御レジスタ、描画時間表の注。https://mega-micros.co.uk/datasheets/datasheet%20-%2063484%20%28hitachi%29%20acrtc.pdf （本調査では別担当がOCRしたテキストを参照） |
| TMS32010台帳 | 同ディレクトリ `TMS32010.md`（出典 T2：TMS32010 User's Guide 1985、A-3, A-13） |
| OKI87 | OKI, *Voice Synthesis LSI Data Book*（初版1987年8月）。**後年資料**。MSM5205（PDF p.26–30）。http://www.bitsavers.org/components/oki/_dataBooks/1987_OKI_Voice_Synthesis_LSI_Data_Book.pdf |
| MM | MAME `src/mame/ensoniq/enmirage.cpp`（L628、ES5503 8000000）、`src/mame/apple/apple2gs.cpp`（L96–98、L4148、A2GS_7M＝28.636363 MHz/4）。**エミュレーション参照**（実機回路の一次資料ではない）。https://github.com/mamedev/mame |
| DV | Amiga Hardware Database「Digi-View」（NewTek、1986、最古の広告1986年5月）。**後年資料**。https://amiga.resource.cx/exp/digiview |
| MI1 | David Van Brink, MIDI仕様の要約（1992/1995改訂、**後年資料**）。https://www.cs.cmu.edu/~music/cmp/archives/cmsip/readings/davids-midi-spec.htm |
| S | fc85 仕様書 `docs/Fictional_Console_1985_Hardware_v0.1.md`（§01, §03, §04, §06, §07, §09, §10, §12, §13, §14） |

調査用に取得したPDFは、リポジトリ外の `<scratchpad>\video\` に置いた。リポジトリには入れていない。
