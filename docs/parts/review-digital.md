# デジタル系部品の見直し（CPU・MIDI・DMA・グルー・メモリ・パレット）
Phase 0 調査 v0.1 ／ 調査日 2026-11-25

凡例：`[D 資料ID 頁]` 出典あり。`[D: 計算]` 出典の値からの計算。`[P]` 本書の推奨（仕様書は未変更）。`[O]` 代替・併記案。`[V]` 未確認。「推定」は本書の推論。資料IDは末尾の表を参照。既存台帳の資料は、その台帳のIDに台帳名を添えて引く（例：[D M68 PDF p.84]（clock-video-1985台帳））。

資料の区分：**A** 1984年以前、**B** 1985年中、**C** 1986年以降（後年資料）、**M** 現代の製品・販売ページ。年代の根拠は A・B だけ。C は IC 復元と傾向の参考、M は現代の入手性の判断にだけ使う。販売ページは「掲載がある」ことしか示さない。在庫・価格・真正性は保証しない。1985年の価格は米国の通信販売の小売価格で、日本の価格ではない。

判断基準（この順）：① 1985年の日本の中堅業務用電子機器メーカー（文字表示・映像・通信が本業）にとって自然か。② 現代の再現で入手できるか。

既存台帳（`HD63484.md`、`supply-HD63484.md`、`TMS32010.md`、`MC6850.md`、`clock-video-1985.md`）で調べた事項は繰り返さず、結論だけ引く。信号系（DSP・映像 ADC・エンコーダ・アナログ部）は `review-signal.md` で扱う。

---

## 0　要旨

### 0.1　現仕様から変えるべきもの

| # | 対象 | 現状 | 変更案 | 理由 |
|---|---|---|---|---|
| 1 | VRAM（ACRTC 側の DRAM） | 型番・速度が未定。日立の設計例は HM50464-12×4（2CLK 5.03 MHz）[D AN86]（HD63484台帳 §3.6） | 64K×1 の 120 ns 品×16（HM4864A-12 など）。読みと書きで CAS の時刻を分ける回路にする [P] | 2CLK 5.369 MHz では、読み書き共通の CAS だと tCAC 46 ns 以下が要る。1985年の DRAM にはない [D: 計算、U85 p.314; AN86 p.84–85; MEM8505 p.192]（§5.1）。64K×4 の 120 ns 品は、1985年以前の資料では TI の製品予告だけ [D TI84M PDF p.147] |
| 2 | DSP プログラム RAM（OI-017） | §05「100 ns 級の要求を別途確認する」 | 80 ns 級。日立 HM6168H-55（4K×4）×4 [P] | 1985年の日立のメモリ資料に量産品として載る [D MEM8505 p.8, p.112]。分離バッファを入れても 80 ns に収まる（推定）[D T1 2-12]（TMS32010台帳） |
| 3 | Staging RAM（64 KiB） | RAM の種類が未定 | SRAM。日立 HM6264-12（8K×8）×8 [P] | 64K×1 の DRAM だと 16-bit 幅で 16 個＝128 KiB になり、半分が余る（推定）。SRAM はリフレッシュが要らず、OI-004 の時分割を組みやすい（推定）。1985年の量産品 [D MEM8505 p.9] |
| 4 | Main RAM のリフレッシュ（OI-007） | 担い手が未定 | グルー（または Data Fabric）の専用カウンタで RAS オンリー・リフレッシュ。1 ラインに 5 行 [P] | HM4864A は 128 行／2 ms [D MEM8505 p.192]。1 ライン 4 行だと一巡 2.03 ms で足りない [D: 計算]。VRAM は ACRTC が自分でリフレッシュする [D U85 p.53]（HD63484台帳） |
| 5 | Data Fabric の実装（§13 項目5） | 回路規模が未見積もり | 最終試作機は PAL＋TTL で 2ch。量産ではゲートアレイ 1 個にする予定だった、とする [P] | 2ch で約 4,000〜5,200 ゲート、TTL で約 140〜180 個（推定、§4.1）。同時代の Atari ST も TTL の試作からカスタム 4 個に移った [D ST85 PDF p.20]。「2ch 実装＋4ch 窓予約」は TTL 基板の規模の制約として説明がつく（推定） |
| 6 | CPU の型番 | MC68000 8 MHz | 搭載品は日立 HD68000-8 と書く。MC68000L8 と同等 [P] | ACRTC・ACIA・DMAC と同じ日立の 1 冊に量産品として載る [D DB85 p.8] |
| 7 | MIDI の ACIA | MC6850 系 | 日立 HD6850（NMOS、1.0 MHz）[P]。CMOS の HD6350 系は [O] | 国産の第二供給元 [D DB85 p.7]。E＝800 kHz は 1.0 MHz 品の範囲（推定） |

### 0.2　そのままでよいもの

| 対象 | 結論 | 理由 |
|---|---|---|
| DMA | 独自の Data Fabric を維持 [P]。HD68450-8 は [O] | HD68450 は 1985年の量産品だが、変換・ポート FIFO・映像同期トリガー・独立バスを持たない（§3） |
| パレットとカラー出力 | TTL＋小容量 RAM＋抵抗 DAC を維持 [P]（OI-008 の案を支持） | 1985年以前に RAMDAC も国産の映像用 DAC も確認できない（§6） |
| ROM | 27256 を使う [P] | 1984〜85年に富士通・NEC・日立が載せている [D FUJ84 PDF p.234; NEC85 p.3-4; MEM8505 p.10] |

---

## 1　CPU

本機の要件：8 MHz、16-bit バス、24-bit アドレス。ACIA 用の E クロック（CPU の 1/10、800 kHz）。Data Fabric 用のバス要求（BR/BG/BGACK）。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 日立 HD68000-8（-4〜-12。DIP-64 ほか） | 1984年の日立の早見表で -4〜-12 が「A：最終仕様」[D QRG84 PDF p.16]。1985年の #U70 の早見表に開発中の印はない。MC68000L8 と同等と書く [D DB85 p.8] | 国産の第二供給元。ACRTC・ACIA・DMAC と同じ 1 冊に載る [D DB85 p.8, p.517, p.585, p.712]。同じ1985年に国内の業務用機（セガ Hang-On、コナミ Nemesis。どちらもアーケード）が 68000 を使う [D MAME-HANG; MAME-NEM]（C） | 8 MHz 品で足りる。MC68000 と同等で、仕様書と同じ [D DB85 p.8] | 68000 の DIP 品は販売ページに多い [D FC68K]（M）。FPGA は fx68k（サイクル単位で再現、GPL-3.0）[D FX68K]。MiSTer の ST・X68000 コアが使う [D MST-ST; MST-X68K]（M） | [P] |
| Motorola MC68000L8 | 1983年の資料 [D M68]（clock-video-1985台帳）。米国の小売で 8 MHz 品 $39.95 [D JDR8508 p.424] | 原設計元の輸入品。日本で選ぶ理由は日立より弱い（推定） | 同上 [D M68]（clock-video-1985台帳） | 同上 [D FC68K]（M） | [O]（同等品） |
| 東芝 TMP68000 | 1984年の東芝マイコン LSI データブックの製品一覧にない [D TOS84 p.v]。1988年の資料にはある [D TLCS88]（C）。1985年は未確認 [V] | 国産だが、1985年の入手は弱い（推定） | 同等品（推定） | [V] | 不採用 |
| Thomson EF68000 ほか（Mostek、Rockwell、Signetics） | 1985年以前の資料で未確認 [V] | 日本で選ぶ理由が薄い（推定） | 同等品（推定） | [V] | 対象外 |

推奨 [P]：HD68000-8。ACRTC・ACIA・DMAC と日立で揃い、資料も 1 冊にまとまる。日立の日本語資料と開発ツールの 1985年の状況は未確認 [V]。

---

## 2　MIDI 用シリアル（ACIA）

本機の要件：31.25 kbps。68000 の E 同期サイクル（VPA）で接続する（OI-012）。8-bit IC は上位バイトレーンに置く。MC6850 そのものの調査は `MC6850.md` にある。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 日立 HD6850（NMOS、1.0 MHz）／HD68A50（1.5 MHz） | 1984年の早見表で HD6850P・HD68A50P が「A」[D QRG84 PDF p.16]。1985年の #U70 で MC6850／MC68A50 と互換 [D DB85 p.7] | 国産の第二供給元。68000 と同じ日立。Atari ST（1985）も 6850 を 2 個使う [D A1]（MC6850台帳）。米国の小売で 6850 は $3.25 [D JDR8508 p.424] | E＝800 kHz は 1.0 MHz 品の範囲（推定）。動作は MC6850 と同じで、MAME の実装を使える [D MAME1]（MC6850台帳） | MC6850P の販売ページあり [D WILT]（M）。FPGA は AtariST_MiSTer に ACIA の実装（acia.v）がある [D MST-ST]（M）。ライセンス表示は未確認 [V] | [P] |
| 日立 HD6350／HD63A50／HD63B50（CMOS、1.0／1.5／2.0 MHz） | 1985年の #U70 に開発中の印なしで載る。「NMOS の HD6850 と互換」「最大 500 kbps」[D DB85 p.7, p.517]。1984年の早見表での扱いは未確認 [V] | 国産で低消費電力。日立の中で選べる [D DB85 p.517] | HD6850 と同等 [D DB85 p.517] | HD6350P の販売ページあり（1 個 $15〜20 の表示）[D FCHD6350]（M） | [O] |
| Motorola MC6850 | 1983年の資料 [D M1]（MC6850台帳） | 原設計元の輸入品（推定） | 仕様書の想定そのもの [D M1]（MC6850台帳） | HD6850 と同じ [D WILT]（M） | [O]（同等品） |
| ヤマハ YM2148（MIDI／キーボード・インタフェース） | 1985年以前の資料で未確認 [V]。MAME に実装がある [D MAME-2148]（C） | 外販の有無が未確認 [V] | 68000 への接続方法が未確認 [V] | [V] | 不採用 |
| NEC µPD8251A／µPD71051（CMOS） | 1985年の NEC 選択ガイドに載る [D NEC85 p.4-4–4-5]。米国の小売で 8251 は $6.95 [D JDR8508 p.424] | 国産で広く使われた（推定） | Intel 系の RD/WR 信号なので、68000 との間に変換ロジックが要る（推定） | 8251 系は多い（推定）[V] | [O] |

参考：ローランド MPU-401 は、日立 HD6801 の内蔵シリアル（SCI）で MIDI を扱う [D MAME-MPU]（C）。CPU 内蔵の SCI は本機の構成に合わない（推定）。

推奨 [P]：HD6850。仕様書の「MC6850 系」と同じ動作で、国産・日立で揃う。

---

## 3　DMA（HD68450 と独自 Data Fabric）

本機の要件（仕様書 §06）：初期 2ch（4ch 分の窓を予約）。ポート 0〜15。SRC/DST・COUNT・STEP・FORMAT・MASK/XOR・SHIFT・TRIGGER。FIFO の full/empty で待つ。DMA の異常がホストを止めない。理論 4 MB/s。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 日立 HD68450-8（4ch DMAC、68000 バス用） | 1984年の早見表で -4/-6/-8 が「A」、-10 が「B：暫定仕様」、-12 が「C：開発中」[D QRG84 PDF p.16]。1985年の #U70 も同じ扱いで、MC68450 と同等 [D DB85 p.8]。CMOS の HD63450 は 1987年が初出 [D AMG]（C） | 国産。ACRTC の DMA ハンドシェイクは HD68450 の単一アドレス・モードに合わせてある [D U85 p.49–50]。1987年の国産パソコン X68000 も HD63450 を使う [D MST-X68K]（C） | 単純な転送だけ。MASK/XOR・SHIFT・FORMAT の変換、ポート FIFO、映像同期のトリガーはない（推定）。ホストバスを借りて転送するので、ホストを止めない不変条件と相性が悪い（推定） | HD68450-8 の販売ページあり（1 個 $11〜24 の表示）[D FCHD68450]（M）。MAME に hd63450.cpp（BSD-3）[D MAME-63450]。FPGA は X68000_MiSTer に DMA63450.vhd（GPL-3.0）[D MST-X68K] | [O]（ホスト↔ACRTC／Staging の単純転送用） |
| NEC µPD8237A-5／µPD71071（8-bit DMAC） | 1985年の NEC 選択ガイドに載る [D NEC85 p.4-4–4-5] | 国産。8-bit 系の部品（推定） | NEC の表では 8-bit 品 [D NEC85 p.4-5]。変換・ポート FIFO はない（推定） | [V] | 不採用 |
| 独自 Data Fabric（試作は PAL＋TTL、量産はゲートアレイ） | 部品は 1985年にそろう（§4） | 同時代の Atari ST も DMA を独自チップにした。TTL の試作からカスタム 4 個（GLUE・MMU・DMA・SHIFTER）に移った [D ST85 PDF p.20] | 仕様書 §06 の機能と独立バスをそのまま満たす（推定） | FPGA／CPLD で再現できる（推定） | [P] |

推奨 [P]：独自 Data Fabric を維持する。HD68450-8 は、ホストから ACRTC や Staging RAM への単純転送を受け持つ併用案 [O]。

OI-004（Staging RAM の調停）への材料：
- 方式 1：BR/BG/BGACK で CPU を止める。ST の DMA は GLUE がバス要求を出す [D ST85]。
- 方式 2：RAM 側の時分割。ST の MMU は映像と CPU にメモリを交互に割り当てる [D ST85]。
- Staging RAM を SRAM にすると、方式 2 を組みやすい（推定、§5.3）。決定は OI-004 で行う。

---

## 4　Data Fabric の実装とグルーロジック

### 4.1　回路規模の見積もり（推定）

ゲート数は 2 入力 NAND 換算。D-FF 1 個を 6〜8 ゲート、全加算器 1 bit を 8〜10 ゲートと置いた（推定）。FIFO 用の IC は含めない。

| ブロック | ゲート数（1ch） | TTL の目安（1ch） |
|---|---|---|
| SRC/DST アドレスレジスタ（24-bit×2） | 約 340 | 8-bit レジスタ 6 個 |
| STEP（16-bit×2）と 24-bit 加算器（SRC/DST で共用） | 約 440 | レジスタ 4 個、4-bit 加算器 6 個 |
| COUNT（16-bit ダウンカウンタ） | 約 160 | 4-bit カウンタ 4 個 |
| MASK/XOR（16-bit×2）と論理 | 約 290 | レジスタ 4 個、ゲート 8 個 |
| SHIFT（16-bit、4 段） | 約 220 | マルチプレクサ 8〜12 個 |
| FORMAT・TRIGGER・CTRL/STATUS（約 32-bit） | 約 220 | レジスタ 4 個 |
| データラッチと順序回路 | 約 240 | レジスタ 2 個、PAL 2〜4 個ほか |
| 小計（1ch） | 約 1,700〜2,100 | 約 55〜65 個 |
| 共通部（ホスト I/F、デコード、調停、ポート選択、トリガー同期、リフレッシュ、割り込み、バス切替） | 約 600〜1,000 | 約 30〜50 個 |
| **合計（2ch）** | **約 4,000〜5,200** | **約 140〜180 個** |
| 合計（4ch） | 約 7,400〜9,400 | 約 250〜310 個 |

- 2ch は NEC µPD65060（6,500 ゲート）1 個に入る（推定。使えるゲートを 7〜8 割と仮定）[D NEC85 p.8-1]。
- 4ch は µPD65100（11,000 ゲート）級が要る（推定）[D CD8408 p.237]。
- 日立の HD61 系は最大 2,496 ゲート（1984年）。2ch に 2〜3 個要る（推定）[D QRG84 PDF p.39]。
- TTL 140〜180 個は、大きめの基板 1 枚になる。ST の試作機も大きさの大半が TTL のためだった [D ST85 PDF p.20]。

### 4.2　グルー部品とゲートアレイ

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| MMI PAL16L8／16R8 ほか（A 品） | 1983年の PAL ハンドブック第 3 版に標準品と A 品。A 品の tPD は商用で 25 ns [D MMIPAL83 p.3-12]。1985年の MMI データブックに PAL とマスク版の HAL [D MMI85] | 米国製。MMI の日本法人の記載がある [D MMI85]。国産の PAL 相当品は未確認 [V] | 順序回路・デコード・調停を小さくまとめられる（推定） | Microchip ATF16V8B・ATF22V10C が現行品。一部の DIP 品は終息 [D ATF16; ATF22]（M） | [P]（試作） |
| 74LS／74ALS（日立 HD74LS 126 品種、HD74ALS 16 品種） | 1984年の日立の早見表に載る [D QRG84 PDF p.40] | 国産で品種が多い。日立の ACRTC 設計例も 74ALS／74LS を使う [D AN86 p.84–85]（C） | レジスタ・加算器・マルチプレクサの主力（推定） | 74HC／74HCT で置き換えられる（推定） | [P]（試作） |
| 74S（日立 HD74S 35 品種） | 1984年の早見表 [D QRG84 PDF p.40] | 国産。消費電力が大きい（推定） | 速い経路だけに使う（推定） | NOS のみ（推定）[V] | [O] |
| 74F（FAST） | 日立の 1984年の早見表にない [D QRG84 PDF p.40]。日本での 1985年の入手は未確認 [V] | 輸入品になる（推定） | 速さは最も有利（推定） | [V] | 不採用（[V] が解ければ [O]） |
| 74HC（日立 HD74HC 99 品種） | 1984年の早見表 [D QRG84 PDF p.40] | 国産。低消費電力（推定） | 速さの要らない部分（レジスタ保持、デコード）（推定） | 現行品が多い（推定） | [O] |
| NEC CMOS ゲートアレイ µPD650xx | 3 µm 系 400〜2,100 ゲート、2 µm 系 800〜11,000 ゲート。µPD65060 は 6,500 ゲート [D NEC85 p.8-1]。µPD65100 は 5,000 個時に 1 個約 $200、開発費約 $80,000、検証後 10〜12 週 [D CD8408 p.237] | 国産。中堅メーカーでも手の届く費用と期間（推定） | 2ch を 1 個に収められる（推定、§4.1） | FPGA で再現する（推定） | [P]（量産計画） |
| 日立ゲートアレイ：CMOS HD61J／K／L／MM（504〜2,496 ゲート）、Bi-CMOS HD27（200〜1,530 ゲート） | 1984年の早見表。「短い開発期間と手頃な開発費」とうたう [D QRG84 PDF p.39–40] | 国産で日立に揃う。1985年に大きい品種が出たかは未確認 [V] | 2ch に 2〜3 個（推定） | FPGA で再現する（推定） | [O] |
| MMI HAL（PAL のマスク版） | 1985年の MMI データブック [D MMI85] | 量産で PAL を置き換える中間案（推定） | PAL と同じ（推定） | ATF16V8B などで代替（推定）[D ATF16]（M） | [O] |

### 4.3　FIFO

本機の要件：DSP 入出力 FIFO が 32 語×16-bit ずつ、ADPCM 入力 FIFO が 16 バイト（いずれも暫定）[仕様書 §06]。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| MMI 67401／67402（64×4／64×5、非同期） | 1985年の MMI データブックに 10／15／16.7 MHz 品 [D MMI85 p.7-16]。同書の解説は「数年前から出ている」と書く [D MMI85 p.7-4] | 米国製。国産品は未確認 [V] | 4 個で 64×16。DSP の 1 方向分を満たす。DSP の入出力で 8 個、ADPCM で 2 個（推定） | NOS のみ（推定）[V]。FPGA で再現する（推定） | [P] |
| IDT7201（512×9） | 1985年の資料が見つからない [V] | — | 深い FIFO が要るなら有利（推定） | Renesas の製品ページあり。一部は終息予告 [D REN7201]（M） | [V] |

### 4.4　「試作は PAL＋TTL、量産はゲートアレイ」は自然か

自然と判断する（推定）。
- Atari ST（1985）が同じ道をたどった。試作は TTL が大半で、製品はカスタム 4 個になった [D ST85 PDF p.20]。
- 1984〜85年に、国内 2 社が CMOS ゲートアレイを品揃えしていた [D QRG84 PDF p.39; NEC85 p.8-1]。
- NEC の 11,000 ゲート品で、開発費約 $80,000、検証後 10〜12 週 [D CD8408 p.237]。
- 本機は未発売。最終試作機は PAL＋TTL のままで、ゲートアレイ化の前に止まった、という設定にできる（推定）。
- 「2ch 実装＋4ch 窓予約」は、TTL 基板の規模で 2ch に絞った結果と説明できる（推定）。

---

## 5　メモリ

### 5.1　VRAM（ACRTC、2CLK 5.369 MHz）

前提：
- ACRTC は HD63484-8 [仕様書 §04]。2CLK 1 周期は 186.2 ns。メモリサイクルは 2CLK 2 周期で 372.5 ns [D: 計算]。
- HD63484-8 の書き込みデータの遅れ tMWDD は最大 70 ns。読み込みデータのセットアップ tMRDS は最小 40 ns [D U85 p.314]。
- 外付けの遅れは、書き込みバッファ 20 ns、読み出しマルチプレクサ（74ALS253）10 ns とした [D AN86 p.84–85]（C）。
- 時刻はサイクル中央の 2CLK↓ を 0 とする。次のサイクルの始まり S' は 186.2 ns。

**読み書き共通の CAS（不成立）**：CAS↓ は書き込みデータが届く 90 ns より後。読み出しは CAS↓＋tCAC＋10 ≤ 186.2−40。よって tCAC ≤ 46 ns が要る [D: 計算]。日立の 1985年の 120 ns 品で tCAC は 60 ns [D MEM8505 p.192]。1985年以前の資料で、この条件を満たす DRAM は見つからない。日立の応用ノートも、150 ns 品で 2CLK 約 4.4 MHz までと計算している [D AN86 p.84–85]（C、HD63484台帳 §3.6）。

**読みと書きで CAS を分ける（成立、推定）**：AN86 は分けられると書くが、上限は示さない [D AN86 p.85]（C）。

| DRAM | tRAC／tCAC／tRSH／tRP（ns） | 読みの CAS↓ の最遅 | 書きの後、次の RAS↓ を置ける範囲 | 判定（推定） |
|---|---|---|---|---|
| HM4864A-12 | 120／60／60／90 [D MEM8505 p.192] | 中央＋76 ns | S'＋54〜S'＋202 ns（幅 148 ns） | 余裕あり |
| HM4864A-15 | 150／75／75／100 [D MEM8505 p.192] | 中央＋61 ns | S'＋79〜S'＋172 ns（幅 94 ns） | 成立するが薄い |

- 計算 [D: 計算]：書きの CAS↓＝中央＋90。RAS↑ ≥ CAS↓＋tRSH。次の RAS↓ ≥ RAS↑＋tRP。次のサイクルの読みは RAS↓＋tRAC＋10 ≤ S'＋372.5−40。
- ACRTC を -6 品にすると（tMWDD 80、tMRDS 50 [D U85 p.314]）、-12 品の範囲は S'＋64〜S'＋192 ns。まだ成立する（推定）。
- 遅延線の誤差、tCP・tCRP・tDH などの細かい条件は見ていない。実回路で確かめる [V]。
- リフレッシュ：ACRTC が HSYNC 中にリフレッシュアドレスを出す [D U85 p.53]。1 ライン 12 回、256 行を約 1.4 ms で一巡する（HD63484台帳）。HM4864A の 128 行／2 ms を満たす（推定）[D MEM8505 p.192]。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 64K×1・120 ns×16（日立 HM4864A-12、NEC µPD4164-12、TI TMS4164-12） | 1985年の日立・NEC の資料に載る [D MEM8505 p.9; NEC85 p.3-1]。TI は 1984年 [D TI84M]。米国の小売で 4164-120 は $3.95 [D JDR8508 p.424] | 国産・複数社の量産品 [D MEM8505 p.9; NEC85 p.3-1] | 16-bit×64K＝128 KiB ちょうど。CAS を分ければ 5.369 MHz で余裕あり（推定、上表） | -12 品（KM4164B-12）の販売ページあり [D JAM4164B]（M） | [P] |
| 64K×1・150 ns×16（HM4864A-15 ほか） | 同上。米国の小売で 4164-150 は 9 個 $9.95 [D JDR8508 p.424] | 同上 | CAS を分ければ成立するが余裕が薄い（推定、上表） | 4164-150 の販売ページ（$3.95）[D JAM4164]（M） | [O]（費用優先） |
| 64K×4・150 ns×4（富士通 MB81464-15、NEC µPD41464-15、TI TMS4464-15、日立 HM50464-15） | NEC の 1985年版に -15／-20 [D NEC85 p.3-1]。TI は 1984年に製品予告 [D TI84M PDF p.147]。日立の 1985年版と富士通の 1984年版にはない [D MEM8505; FUJ84]。1985年8月の米国製品の部品表に MB81464-15 [D IRON8508 p.115]（supply-HD63484台帳） | 1985年は出始めで入手が弱い（推定） | 4 個で済む。余裕は 150 ns の 64K×1 と同程度（推定） | [V] | [O] |
| 64K×4・120 ns×4（HM50464-12 ほか） | 1985年以前の資料では TI の製品予告（tCAC 70 ns）だけ [D TI84M PDF p.147]。日立の設計例は HM50464-12×4 [D AN86]（C、HD63484台帳 §3.6） | 1985年は弱い（推定） | 4 個で済む（推定） | [V] | [O]（後年の改版） |
| デュアルポート VRAM（NEC µPD41264-12、TI TMS4161） | NEC の 1985年版に載る [D NEC85 p.3-1]。TI は 1984年に製品予告 [D TI84M PDF p.55] | 新しすぎる（推定） | ACRTC はシリアルポートを使わないので、利点が小さい（推定） | [V] | 不採用 |

### 5.2　Main RAM（128 KiB）

68000-8 のゼロウェイト条件（推定）：
- AS↓ は S2 の立ち上がりから最大 60 ns。サイクル先頭から 185 ns [D M68 PDF p.84]（clock-video-1985台帳）。
- データは S6 の終わりの立ち下がり（437.5 ns）の 15 ns 前までに要る。期限は 422.5 ns [D M68 PDF p.84]（clock-video-1985台帳）。
- AS↓ からデコード 20 ns で RAS↓、tRAC 150 ns、バッファ 15 ns とすると 370 ns。約 50 ns 残る [D: 計算]。150 ns 品でゼロウェイトにできる（推定）。

リフレッシュ（OI-007）：NTSC の 1 ラインは約 63.6 µs。2 ms は約 31.5 ライン。1 ライン 4 行では 128 行に 32 ライン（2.03 ms）かかり、規格を超える。1 ライン 5 行なら 25.6 ライン（1.63 ms）[D: 計算、MEM8505 p.192]。水平帰線期間に 5 行、または 15 µs ごとのタイマーにする [P]。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 64K×1・150 ns×16（日立 HM4864A-15、NEC µPD4164-15、TI TMS4164-15、東芝 TMM4164） | 1985年以前に国内 3 社以上 [D MEM8505 p.9; NEC85 p.3-1; TOS83 p.29]。米国の小売で 9 個 $9.95 [D JDR8508 p.424] | 最も普通の部品（推定） | 128 KiB ちょうど。ゼロウェイト（推定） | 4164-150 の販売ページ（$3.95）[D JAM4164]（M） | [P] |
| 256K×1・150 ns×16（日立 HM50256、NEC µPD41256） | 1985年の資料に載る [D MEM8505 p.9; NEC85 p.3-1]。米国の小売で $3.95 [D JDR8508 p.424] | 普通の部品（推定） | 512 KiB になる。仕様の 4 倍（推定） | 中古・NOS が多い（推定）[V] | [O]（増設版） |
| 64K×4・150 ns×4 | §5.1 と同じ [D NEC85 p.3-1; IRON8508 p.115] | 1985年は入手が弱い（推定） | 4 個で済む（推定） | [V] | [O] |

### 5.3　Staging RAM（64 KiB）

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 日立 HM6264-12（8K×8 CMOS）×8 | 1985年に -10／-12／-15 [D MEM8505 p.9]。同等の NEC µPD4364 [D NEC85 p.3-2]。米国の小売で HM6264LP-12 は $9.50、-15 は $7.95 [D JDR8508 p.424] | 国産・複数社（推定） | 64 KiB ちょうど。リフレッシュが要らない。Fabric の 1 アクセス 250 ns（仕様書 §06 の 4 クロック／要素）にも足りる（推定） | 5 V の 32K×8 SRAM（Renesas 71256SA、Alliance AS7C256B）で代替 [D REN71256; FAR256]（M） | [P] |
| NEC µPD43256（32K×8）×2 | 1985年の NEC 選択ガイドに -10／-12／-15 [D NEC85 p.3-2]。出荷状況は未確認 [V] | 国産（推定） | 2 個で済む（推定） | 同上 [D REN71256]（M） | [O] |
| 64K×1 DRAM×16 | §5.2 [D MEM8505 p.9] | 普通の部品（推定） | 128 KiB になり半分が余る。リフレッシュが要る（推定） | §5.2 [D JAM4164]（M） | 不採用 |

### 5.4　DSP プログラム RAM（4K×16）

要件：アドレスからデータまで 100 ns 以下（20 MHz）。分離バッファの遅れを引くと 80 ns 級 [D T1 2-12]（TMS32010台帳）。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 日立 HM6168H-55（4K×4 CMOS、20 ピン）×4 | 1985年に -45／-55／-70 [D MEM8505 p.8]。tAA・tACS は 55 ns、tHZ は 20 ns [D MEM8505 p.112] | 国産で日立に揃う（推定） | 4 個で 4K×16。バッファに 25 ns 残る（推定） | NOS は未確認 [V]。5 V の高速 SRAM（71256SA15 など）の一部を使って代替 [D REN71256]（M） | [P] |
| 日立 HM6168H-45×4 | 同上 [D MEM8505 p.8] | 同上 | 余裕が増える（推定） | 同上 | [O] |
| 富士通 MB8168-55（4K×4 NMOS）×4 | 1984年の富士通資料 [D FUJ84 PDF p.158]。INMOS IMS1420 の互換品と書く [D FUJ84 PDF p.6] | 国産（推定） | 同上（推定） | [V] | [O] |
| 東芝 TMM2018D-45（2K×8 NMOS）×4 | 1983年の東芝資料 [D TOS83 p.89] | 国産（推定） | 2 個×2 組。デコードが 1 段増える（推定） | [V] | [O] |

### 5.5　ROM

68000-8 では、アドレス確定がサイクル先頭から最大 132.5 ns、データの期限が 422.5 ns [D M68 PDF p.84]（clock-video-1985台帳）。デコードとバッファを引くと、使えるのは約 250〜270 ns [D: 計算]。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 27256（32K×8 EPROM）×2：富士通 MBM27256-20／-25／-30、NEC µPD27256（250 ns）、日立 HN27256-30 | 1984〜85年の資料に載る [D FUJ84 PDF p.234; NEC85 p.3-4; MEM8505 p.10]。日立の -25 品は 1985年に開発中 [D MEM8505 p.10]。米国の小売で 250 ns 品 $12.95 [D JDR8508 p.424] | 国産・複数社 | ブート ROM 64 KiB を 2 個で組める。200 ns 品ならゼロウェイト。250 ns 品は余裕がなく、1 ウェイトが無難（推定、OI-007） | 27C256 系で代替（推定）[V] | [P] |
| マスク ROM：日立 HN61256（200 ns）、NEC µPD23256A | 1985年の資料に載る [D MEM8505 p.10; NEC85 p.3-3] | 量産品の普通の選択（推定） | 同上（推定） | 再現では EPROM で代替（推定） | [O]（量産計画） |
| 27128×4：日立 HN4827128 | 1985年の資料に載る [D MEM8505 p.10] | 普通の部品（推定） | 個数が倍になる（推定） | [V] | [O] |

Character ROM（32 KiB）も 27256 で組める（推定）。配置は仕様書 §08 に従う。

---

## 6　パレットとカラー出力

本機の要件：16 色、RGB 各 4-bit [仕様書 §05]。1 画素は約 186 ns（推定）。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| TTL＋小容量 RAM＋抵抗 DAC（パレット RAM は 74LS189（16×4）×3 または 日立 HM6148H（1K×4）×3。後段にラッチ、抵抗ラダー、エミッタフォロワ） | 部品は 1985年にそろう。米国の小売で 74LS189 は $8.95 [D JDR8508 p.424]。HM6148H は 1985年の日立の資料に載る [D MEM8505 p.8] | 文字表示・映像が本業なら自社で設計できる範囲（推定） | 16 色×12-bit にちょうど合う。RAM の速さは 186 ns／画素に十分（推定） | 抵抗と 74HC で再現できる。FPGA でも容易（推定） | [P] |
| TRW TDC1334（3ch・4-bit の映像用 DAC） | 1985年1月の「Advance Information」[D T85]（clock-video-1985台帳） | 米国製で −5.2 V が要る [D T85]（clock-video-1985台帳） | 4-bit×3 で要件に合う。パレット RAM は別に要る（推定） | [V] | [O] |
| RAMDAC（パレット RAM と DAC の 1 チップ品） | 1985年以前の資料にない（clock-video-1985台帳 §6.1） | — | — | — | 対象外 |
| 国産の映像用 DAC | 日立の 1984年の早見表には映像用 DAC がない。あるのは 4-bit 高速 A/D（HA19202）と 8-bit D/A（HA16642、暫定仕様）[D QRG84 PDF p.8–9]。ソニー CX20051A（10-bit、30 MHz、映像用）は 1986年9月の資料 [D SONY86 p.104]（C） | 1985年の国産品は未確認 [V] | — | [V] | 対象外（後継機の候補） |

書き換えのタイミング（OI-008 への材料）[P]：
- CPU の書き込みはいったん保持レジスタに入れる。次の水平帰線期間にパレット RAM へ移す。
- 表示中の RAM の取り合いと、ラインの途中での色の乱れを避けられる（推定）。ラインごとの色替えはできる。
- HM6148H を使うと 64 組のパレットを持てる。拡張の余地になる（推定）。
- RGB の後段（エンコーダ）は `review-signal.md` §3 を参照する。

---

## 7　未解決事項

1. HD68000・HD6850・HD68450 の 1985年の国内価格 [V]。
2. 日立の HD68000 の日本語資料・開発ツールの 1985年の状況 [V]。
3. HD6350／HD63B50 の 1985年の国内出荷 [V]。
4. TMP68000・EF68000 の 1985年の状況 [V]。
5. YM2148 の外販の有無と、68000 への接続 [V]。
6. 日本での PAL の供給元（MMI 日本法人のほか、国産品）[V]。
7. 74F の 1985年の国内入手 [V]。
8. IDT7201 の発売時期。国産 FIFO の有無 [V]。
9. 64K×4 DRAM の 120 ns 品の 1985年の入手 [V]。
10. HM6168H の 1985年の価格 [V]。
11. TMS32010台帳は IMS1420 を「16K×1」と書く。富士通の互換表は IMS1420 を MB8168（4K×4）に対応させる [D FUJ84 PDF p.6]。台帳の記述を確かめる [V]。
12. 1985年以前の国産の映像用 DAC [V]。
13. 4,000〜6,500 ゲート級のゲートアレイの国内の開発費・単価 [V]。
14. §5.1 の VRAM タイミング（CAS を分ける案）を回路図とシミュレーションで確かめる。遅延線の誤差、tCP・tCRP・tDH を含める [V]。
15. Main RAM のリフレッシュとホスト・Fabric の調停の詳細（OI-004・OI-007）。
16. HD68450 と Data Fabric の転送速度の比較 [V]。

---

## 出典

参照日はすべて 2026-11-25。取得した PDF・テキスト・画像はリポジトリ外の `<scratchpad>\parts\` に置いた。既存台帳の作業で取得したもの（`<scratchpad>\supply\`、`<scratchpad>\hd63484\`、`<scratchpad>\video\`）も使った。リポジトリには入れていない。bitsavers はミラーから取得したが、URL は本家を書く。頁は特記のない限り資料の印刷頁。

| ID | 区分 | 書誌 | 日付 | URL | 参照日 |
|---|---|---|---|---|---|
| QRG84 | A | Hitachi IC and Discrete Quick Reference Guide。PDF p.8–9（リニア）、p.16（MPU・周辺）、p.39–40（ゲートアレイ・ロジック品種数）。状態記号 A／B／C は PDF p.4（supply-HD63484台帳と同じ） | 1984 | http://bitsavers.org/components/hitachi/_dataBooks/1984_Hitachi_IC_and_Discrete_Quick_Reference_Guide.pdf | 2026-11-25 |
| DB85 | B | Hitachi, 8/16-Bit Multi-Chip Microcomputer Data Book #U70。p.7–8（Quick Reference）、p.517（HD6350 系）、p.585（HD68000）、p.712（HD63484）（HD63484台帳と同じ） | 1985 | http://bitsavers.org/components/hitachi/_dataBooks/1985_U70_Hitachi_8_16-Bit_Multi-Chip_Microcomputer_Data_Book.pdf | 2026-11-25 |
| U85 | B | Hitachi, HD63484 ACRTC User's Manual #U75。p.49–50（DMA）、p.53（リフレッシュ）、p.314（AC 特性）（HD63484台帳と同じ） | 1985-05 | http://bitsavers.org/components/hitachi/_dataBooks/1985_U75_Hitachi_HD68484_ACRTC_Advanced_CRT_Controller_Users_Manual.pdf | 2026-11-25 |
| AN86 | C | Hitachi, HD63484 ACRTC Application Note #U90。p.84–85（DRAM タイミング）。IC 復元用（HD63484台帳と同じ） | 1986-04 | https://bitsavers.org/components/hitachi/_dataBooks/U90_Hitachi_HD63484_ACRTC_Advanced_CRT_Controller_Application_Note_198604.pdf | 2026-11-25 |
| MEM8505 | B | Hitachi IC Memories Data Book #M11。p.8–10（Quick Reference）、p.110–112（HM6168H）、p.191–192（HM4864A）（supply-HD63484台帳と同じ） | 1985-05 | http://bitsavers.org/components/hitachi/_dataBooks/1985_M11_Hitachi_IC_Memories_Data_Book.pdf | 2026-11-25 |
| CD8408 | A | Computer Design Vol.23 No.9。p.237（NEC µPD65100 ゲートアレイの価格・開発費・期間）（supply-HD63484台帳と同じ資料） | 1984-08 | https://archive.org/details/bitsavers_ComputerDe23N09198408_170603952 | 2026-11-25 |
| IRON8508 | B | Ironics IV-1624 シリアルカードのマニュアル、部品表 p.115（supply-HD63484台帳と同じ） | 1985-08 | https://archive.org/details/bitsavers_ironicsIV1SerialCard198508_6736989 | 2026-11-25 |
| M68 | A | Motorola, MC68000 16-Bit Microprocessor。PDF p.84（AC 特性）（clock-video-1985台帳と同じ） | 1983-04 | http://www.bitsavers.org/components/motorola/68000/68000/MC68000_16-Bit_Microprocessor_Apr83.pdf | 2026-11-25 |
| T85 | B | TRW LSI Products, VLSI Data Book。TDC1334（PDF p.24）（clock-video-1985台帳と同じ） | 1985 | https://bitsavers.trailing-edge.com/components/trw/_dataBooks/1985_TRW_VLSI_Data_Book.pdf | 2026-11-25 |
| ST85 | B | K. Gerits ほか, Atari ST Internals（Copyright 1985、英語版第3刷 1986年7月）。PDF p.20（TTL の試作とカスタム 4 個）（clock-video-1985台帳と同じ） | 1985 | https://www.atarimania.com/documents/Atari-ST-Internals.pdf | 2026-11-25 |
| T1 | B | Texas Instruments, TMS32010 User's Guide Rev. B。2-12（TMS32010台帳と同じ） | 1985-03 | https://www.bitsavers.org/components/ti/TMS320xx/3201x/1983_TI_TMS32010_Users_Guide_Digital_Signal_Processor_Products_Revision_B_198503.pdf | 2026-11-25 |
| M1 | A | Motorola, 8-Bit Microprocessor and Peripheral Data。MC6850（MC6850台帳と同じ） | 1983 | http://www.bitsavers.org/components/motorola/_dataBooks/1983_Motorola_8-Bit_Microprocessor_and_Peripheral_Data.pdf | 2026-11-25 |
| A1 | B | Atari ST Internals（ST85 と同じ本。MC6850台帳の ID） | 1985 | https://www.atarimania.com/documents/Atari-ST-Internals.pdf | 2026-11-25 |
| MAME1 | M | MAME `src/devices/machine/6850acia.cpp`（MC6850台帳と同じ） | master | https://github.com/mamedev/mame/blob/master/src/devices/machine/6850acia.cpp | 2026-11-25 |
| AMG | C | Amagaki（hackaday.io project 203046）のログ「Were the chips available? CPU」「同 DMAC」（supply-HD63484台帳と同じ） | 2025-05 | https://hackaday.io/project/203046-amagaki-the-macintosh-killer/log/240601-were-the-chips-available-dmac | 2026-11-25 |
| JDR8508 | B | BYTE Vol.10 No.8、JDR Microdevices の広告 p.424（PDF p.432）。68000-8、6850、8251、6844、4164、41256、HM6264LP、27256、74LS189 の価格 | 1985-08 | https://www.vintageapple.org/byte/pdf/198508_Byte_Magazine_Vol_10-08_The_Amiga.pdf | 2026-11-25 |
| NEC85 | B | NEC Electronics, Product Selection Guide。p.3-1（DRAM）、p.3-2（SRAM）、p.3-3（マスク ROM）、p.3-4（EPROM）、p.4-4–4-5（周辺 LSI）、p.8-1（CMOS ゲートアレイ） | 1985-05 | http://bitsavers.org/components/nec/_dataBooks/1985_Product_Selection_Guide.pdf | 2026-11-25 |
| FUJ84 | A | Fujitsu Memory Data Book。PDF p.6（互換表）、p.158（MB8168-55）、p.234（MBM27256） | 1984 | http://bitsavers.org/components/fujitsu/_dataBooks/1984_Fujitsu_Memory_Data_Book.pdf | 2026-11-25 |
| TOS83 | A | Toshiba MOS Memory Products。p.29（TMM4164）、p.89（TMM2018D）（目次の頁） | 1983 | http://bitsavers.org/components/toshiba/_dataBook/1983_Toshiba_MOS_Memory.pdf | 2026-11-25 |
| TOS84 | A | Toshiba Microcomputer LSI Data Book。p.v（製品一覧） | 1984 | http://bitsavers.org/components/toshiba/_dataBook/1984_Toshiba_Microcomputer_LSI_Data_Book.pdf | 2026-11-25 |
| TI84M | A | Texas Instruments, MOS Memory Data Book。PDF p.55（TMS4161、製品予告）、p.147（TMS4464、製品予告） | 1984 | http://bitsavers.org/components/ti/_dataBooks/1984_TI_MOS_Memory_Data_Book.pdf | 2026-11-25 |
| MMI85 | B | Monolithic Memories, LSI Databook 6th ed.。p.7-4（FIFO 解説）、p.7-16（67401／67402） | 1985 | http://bitsavers.org/components/mmi/_dataBooks/1985_MMI_LSI_Databook_6ed.pdf | 2026-11-25 |
| MMIPAL83 | A | Monolithic Memories, PAL Handbook 3rd ed.。p.3-12（16L8A／16R8A の AC 特性） | 1983 | http://bitsavers.org/components/mmi/_dataBooks/1983_MMI_PAL_Handbook_3ed.pdf | 2026-11-25 |
| SONY86 | C | Sony, A/D・D/A Converters and DSP Data Book（1986年9月 第1版）。p.104（CX20051A） | 1986-09 | http://bitsavers.org/components/sony/1987_Sony_A-D_D-A_Converters_and_DSP_Data_Book.pdf | 2026-11-25 |
| TLCS88 | C | Toshiba, TLCS-68000 User's Manual | 1988 | http://bitsavers.org/components/toshiba/_dataBook/1988_Toshiba_TLCS-68000_Users_Manual.pdf | 2026-11-25 |
| MAME-HANG | C | MAME `src/mame/sega/segahang.cpp`（Hang-On・Space Harrier、1985、68000×2）。BSD-3 | master | https://github.com/mamedev/mame/blob/master/src/mame/sega/segahang.cpp | 2026-11-25 |
| MAME-NEM | C | MAME `src/mame/konami/nemesis.cpp`（Nemesis・TwinBee、1985、68000）。BSD-3 | master | https://github.com/mamedev/mame/blob/master/src/mame/konami/nemesis.cpp | 2026-11-25 |
| MAME-MPU | C | MAME `src/devices/machine/mpu401.cpp`（MPU-401、6801 の SCI で MIDI）。BSD-3 | master | https://github.com/mamedev/mame/blob/master/src/devices/machine/mpu401.cpp | 2026-11-25 |
| MAME-2148 | C | MAME `src/devices/machine/ym2148.cpp`（YM2148）。BSD-3 | master | https://github.com/mamedev/mame/blob/master/src/devices/machine/ym2148.cpp | 2026-11-25 |
| MAME-63450 | C | MAME `src/devices/machine/hd63450.cpp`（HD63450 DMAC）。BSD-3 | master | https://github.com/mamedev/mame/blob/master/src/devices/machine/hd63450.cpp | 2026-11-25 |
| FX68K | M | fx68k（68000 のサイクル単位の FPGA 実装）。GPL-3.0 | 2021-06 最終更新 | https://github.com/ijor/fx68k | 2026-11-25 |
| MST-ST | M | AtariST_MiSTer（`rtl/acia.v`、`rtl/fx68k`）。リポジトリのライセンス表示は API で検出されず [V] | 2026-11 | https://github.com/MiSTer-devel/AtariST_MiSTer | 2026-11-25 |
| MST-X68K | M | X68000_MiSTer（`rtl/DMAC/DMA63450.vhd`、`rtl/FX68K`）。GPL-3.0。X68000（1987）が HD63450 を使うことの傍証（C） | 2026-11 | https://github.com/MiSTer-devel/X68000_MiSTer | 2026-11-25 |
| FC68K | M | Findchips の MC68000P 検索結果（販売掲載） | — | https://www.findchips.com/search/mc68000p | 2026-11-25 |
| FCHD6350 | M | Findchips の HD6350P 検索結果（販売掲載） | — | https://findchips.com/search/HD6350P | 2026-11-25 |
| FCHD68450 | M | Findchips の HD68450 検索結果（販売掲載） | — | https://findchips.com/search/hd68450 | 2026-11-25 |
| WILT | M | Wiltronics の MC6850P 販売ページ | — | https://www.wiltronics.com.au/?p=4527 | 2026-11-25 |
| JAM4164 | M | Jameco の 4164-150 販売ページ | — | https://www.jameco.com/z/4164-150-Major-Brands-64KX1-DIP-16-DRAM-150NS_41662.html | 2026-11-25 |
| JAM4164B | M | Jameco の KM4164B-12 販売ページ | — | https://www.jameco.com/z/KM4164B-12-Samsung-Electronics-IC-4164-120-NMOS-DRAM-64K-Bit-64K-x-1-120ns-w-Page-Mode_2290535.html | 2026-11-25 |
| REN71256 | M | Renesas 71256SA（32K×8、5 V、15〜25 ns）製品ページ。ライフサイクルの表示は未確認 [V] | — | https://www.renesas.com/en/products/71256sa | 2026-11-25 |
| FAR256 | M | Farnell の Alliance AS7C256B-15PIN 販売ページ | — | https://ie.farnell.com/alliance-memory/as7c256b-15pin/sram-256kbit-40-to-85deg-c-dip/dp/4260889 | 2026-11-25 |
| REN7201 | M | Renesas 7201（512×9 非同期 FIFO、5 V）製品ページ | — | https://www.renesas.com/us/en/products/memory-logic/fifo-products/asynchronous-fifos/7201-512-x-9-asyncfifo-50v | 2026-11-25 |
| ATF16 | M | Microchip ATF16V8B 製品ページ | — | https://www.microchip.com/en-us/product/ATF16V8B | 2026-11-25 |
| ATF22 | M | Microchip ATF22V10C 製品ページ | — | https://www.microchip.com/en-us/product/ATF22V10C | 2026-11-25 |
