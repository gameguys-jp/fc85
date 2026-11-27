# 本体の番地とレジスタ

> 生成物。直すときは [registers.toml](registers.toml)（`py tools/regmap/regmap.py`。DR-100）。番地・幅・アクセス・ビットの名前と値はこの表（registers.toml）が正で、動作の説明と根拠は[仕様書](hardware.md) §05・§06・§09 と DR。同じ名前と値が `sdk/include/fc85.inc`（m68kasm）、`sdk/include/fc85.h`（C）、`emulator/crates/board/src/regmap.rs`（Rust）に出る。

本体の番地とレジスタ（68000 から見た MMIO）。記述ステータスは見出しの印に従う（多くは [P]）。

## 01　アクセスの規約

アクセスの規約（仕様書 §05、DR-016・DR-030・DR-033・DR-052・DR-053）

- 16 bit のレジスタは語アクセスだけ。バイトのアクセスはバスエラー。
- 8 bit の IC（DOC、ACIA、文字 ROM）は上位バイトレーン（偶数番地）。MOVE.B で偶数番地を使う。
- 未割当の番地はバスエラー（64 クロックの時間切れ）。
- ROM と読出し専用のレジスタへの書込みは捨て、書込み専用のレジスタは読むと 0（同じ番地にもう一方の向きのレジスタがあれば、そのレジスタを読み書きする）。
- ACRTC の AR/SR とデータポート、DOC の窓には、CLR・TAS・BSET などの読み書き一体の命令を使わない。
- ACRTC の FIFO には、SR の WFR（書込み）・RFR（読出し）を確かめてからアクセスする。
- DOC の 0xE0・0xE2 は読むと状態が変わる。0xE0 は DOC の割込みを受ける処理（BIOS の既定の処理か、ベクタ 28 を書き換えたアプリ）だけが割込み 1 回につき 1 回読み、0xE2 は A/D 変換の手順でだけ読む（DR-123）。

| アクセス | 意味 |
|---|---|
| r | 読出し。書込みはバスエラーにならない（ROM と I/O のレジスタは捨てる。DR-052、DR-053） |
| w | 書込み。読出しはバスエラーにならない（I/O のレジスタは 0 が読める。DR-046、DR-053） |
| rw | 読み書き |

| 条件 | 意味 |
|---|---|
| doc | DOC の子基板がある（FABRIC_MODULES の bit 2）。ないと窓はバスエラー（DR-019） |
| no_doc | DOC の子基板がない。あると窓はバスエラー（DR-019） |
| dsp_prgbus | DSP をリセットし PRGBUS＝1 の間。ほかはバスエラー（DR-029） |
| cart | カートリッジが差してある（INPUT_IRQ_STATUS の bit 4）。ないと窓はバスエラー（DR-095） |

| 窓の中で表にない番地 | 意味 |
|---|---|
| berr | 表にない番地はバスエラー（DR-052、DR-053） |
| chip | 窓の中の番地はすべて IC に届く（DOC のネイティブのレジスタ 256 個） |

## 02　アドレスマップ

窓の外の番地はバスエラー。

| 番地（hex） | 窓 | 内容 | 条件 | 表にない番地 |
|---|---|---|---|---|
| 000000–00FFFF | BOOT_ROM | 64 KiB |  | berr |
| 010000–01FFFF | CHAR_ROM | 文字 ROM。ROM の offset n は CHAR_ROM+2n（上位レーン） |  | berr |
| 100000–11FFFF | MAIN_RAM | 128 KiB |  | berr |
| 120000–12FFFF | STAGING | 64 KiB の AV staging RAM（Fabric の port 0） |  | berr |
| 200000–200003 | ACRTC | ACRTC のホストの口：+0 AR（書込み）・SR（読出し）、+2 データ |  | berr |
| 210000–2101FF | DOC | DOC のレジスタ（ネイティブのレジスタ n は先頭＋2n の上位バイト） | doc | chip |
| 220000–22001F | DSP bridge | DSP の制御、状態、メールボックス、応答 |  | berr |
| 230000–23001F | ADPCM bridge | ADPCM の制御と状態。+10〜+1F は本体 DAC（子基板がないとき） |  | berr |
| 240000–24003F | Temporal | Temporal RAM のポインタと状態（Capture RAM は [O]） |  | berr |
| 250000–250FFF | Data Fabric | チャネル 0・1 のレジスタ（4 チャネル分の窓を予約）、PORT_RESET、AUDIO_N、MODULES |  | berr |
| 260000–2600FF | Controller / MIDI | コントローラ、アナログ値、ACIA、入力の割込み（+20〜+3E のマトリクスと拡張は予約で、今はバスエラー） |  | berr |
| 270000–27003F | Palette | パレット、PAL_CTRL、映像の割込みとラスタ |  | berr |
| 280000–281FFF | DSP program | DSP のプログラム RAM（4K×16） | dsp_prgbus | berr |
| 400000–4FFFFF | CART | カートリッジの窓（最大 1 MiB）。なければバスエラー | cart | berr |
| その他 | 予約 | 将来の拡張 |  | berr |

## 03　メモリ（§05）[P]

| 名前 | 番地（hex） | 大きさ | アクセス | 幅 | 条件 | 内容 |
|---|---|---|---|---|---|---|
| `BOOT_ROM` | 000000–00FFFF | 64 KiB | r | any |  | 64 KiB |
| `CHAR_ROM` | 010000–01FFFF | 64 KiB | r | upper |  | 文字 ROM。ROM の offset n は CHAR_ROM+2n（上位レーン）（DR-031） |
| `MAIN_RAM`（終わりの次は `MAIN_RAM_END`） | 100000–11FFFF | 128 KiB | rw | any |  | 128 KiB |
| `JUMP_TABLE`（終わりの次は `JUMP_TABLE_END`） | 100000–1005FF | 1,536 byte | rw | any |  | 例外のジャンプ表。ベクタ n は JUMP_TABLE+6n（JMP abs.L）（DR-025）。MAIN_RAM の中 |
| `STAGING`（終わりの次は `STAGING_END`） | 120000–12FFFF | 64 KiB | rw | any |  | 64 KiB の AV staging RAM（Fabric の port 0） |
| `CART`（終わりの次は `CART_END`） | 400000–4FFFFF | 1 MiB | r | any | cart | カートリッジの窓（最大 1 MiB）。なければバスエラー |

## 04　ACRTC HD63484（§05、DR-016、DR-033）[P]

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `ACRTC_AR` | 0x200000 | 16 | w |  |  |  | 書込み：AR（レジスタの番号） |
| `ACRTC_SR` | 0x200000 | 16 | r |  |  |  | 読出し：SR（上位バイトは 1） |
| `ACRTC_DATA` | 0x200002 | 16 | rw |  | r00 では読出し FIFO から 1 語を取る |  | AR が指すレジスタ。r00 は FIFO |

### SR のビット（[15]）[D]

対象：`ACRTC_SR`。

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_SR_CER` | 0x80 | コマンドの誤り |
| `ACRTC_SR_ARD` | 0x40 | 領域の検出 |
| `ACRTC_SR_CED` | 0x20 | コマンドの終わり |
| `ACRTC_SR_LPD` | 0x10 | ライトペン |
| `ACRTC_SR_RFF` | 0x08 | 読出し FIFO が満杯 |
| `ACRTC_SR_RFR` | 0x04 | 読出し FIFO に語がある |
| `ACRTC_SR_WFR` | 0x02 | 書込み FIFO に空きがある |
| `ACRTC_SR_WFE` | 0x01 | 書込み FIFO が空 |

### SR のビット番号（BTST 用）（[15]）[D]

対象：`ACRTC_SR`。

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_SR_CER_B` | 7 |  |
| `ACRTC_SR_CED_B` | 5 |  |
| `ACRTC_SR_RFR_B` | 2 |  |
| `ACRTC_SR_WFR_B` | 1 |  |
| `ACRTC_SR_WFE_B` | 0 |  |

### レジスタの番号（AR に書く値）。r80 以降は語ごとに AR が 2 進む（[15]）[D]

対象：`ACRTC_AR`。

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_FIFO` | 0x00 |  |
| `ACRTC_CCR` | 0x02 | コマンドの制御 |
| `ACRTC_OMR` | 0x04 | 動作モード |
| `ACRTC_DCR` | 0x06 | 表示の制御 |
| `ACRTC_RCR` | 0x80 | ラスタの数（読出し） |
| `ACRTC_HSR` | 0x82 | 水平同期 |
| `ACRTC_HDR` | 0x84 | 水平表示 |
| `ACRTC_VSR` | 0x86 | 垂直同期 |
| `ACRTC_VDR` | 0x88 | 垂直表示 |
| `ACRTC_SSW` | 0x8A | 分割画面の幅 1 |
| `ACRTC_SSW0` | 0x8C | 上画面のラスタの数（U85 p.91） |
| `ACRTC_SSW2` | 0x8E | 下画面のラスタの数 |
| `ACRTC_BCR` | 0x90 | 属性の点滅：BON1・BOFF1・BON2・BOFF2（4 bit ずつ、U85 p.92–93） |
| `ACRTC_HWR` | 0x92 | ウィンドウの横：上位 HWS−1（HSYNC の終わりから）、下位 HWW−1 |
| `ACRTC_VWS` | 0x94 | ウィンドウの縦の始まり−1（VSYNC の終わりから、U85 p.89–90） |
| `ACRTC_VWW` | 0x96 | ウィンドウのラスタの数 |
| `ACRTC_GCRX` | 0x98 | グラフィックカーソル：上位 CXE、下位 CXS（HSYNC の始まりからのメモリサイクル） |
| `ACRTC_GCRYS` | 0x9A | CYS（VSYNC の終わりからのラスタ、U85 p.94） |
| `ACRTC_GCRYE` | 0x9C | CYE |
| `ACRTC_MWR0` | 0xC2 | 上画面の記憶の幅 |
| `ACRTC_SAR0H` | 0xC4 |  |
| `ACRTC_SAR0L` | 0xC6 |  |
| `ACRTC_MWR1` | 0xCA | 基本画面の記憶の幅 |
| `ACRTC_SAR1H` | 0xCC | 基本画面の開始番地（bit 11–8 は SDA：横のスムーズスクロール） |
| `ACRTC_SAR1L` | 0xCE |  |
| `ACRTC_MWR2` | 0xD2 | 下画面 |
| `ACRTC_SAR2H` | 0xD4 |  |
| `ACRTC_SAR2L` | 0xD6 |  |
| `ACRTC_MWR3` | 0xDA | ウィンドウ |
| `ACRTC_SAR3H` | 0xDC |  |
| `ACRTC_SAR3L` | 0xDE |  |
| `ACRTC_CDR` | 0xE8 | カーソルの定義（U85 p.107–109） |
| `ACRTC_ZFR` | 0xEA | ズーム：bit 15–12 HZF、bit 11–8 VZF（倍率−1、基本画面だけ、U85 p.110） |

### CCR のビット（[15]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_CCR_ABT` | 0x8000 | 打切り（リセットの後は 1） |
| `ACRTC_CCR_PSE` | 0x4000 |  |
| `ACRTC_CCR_DDM` | 0x2000 | データの DMA（DWT・DRD・DMOD を Fabric の port 1 で運ぶ） |
| `ACRTC_CCR_CDM` | 0x1000 |  |
| `ACRTC_CCR_DRC` | 0x0800 | DMA の要求：1 はサイクルスチール |
| `ACRTC_CCR_GBM4` | 0x0200 | 4 bit/画素 |

### OMR のビット（[15]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_OMR_MS` | 0x8000 | マスタ |
| `ACRTC_OMR_STR` | 0x4000 | 表示と描画を始める |
| `ACRTC_OMR_ACM_I` | 0x0008 | インタリーブ |
| `ACRTC_OMR_ACP` | 0x2000 | 単一アクセスで描画を優先 |
| `ACRTC_OMR_CSK1` | 0x0400 | カーソルのスキュー（×1〜3 メモリサイクル。十字では 0 にしない） |
| `ACRTC_OMR_DSK1` | 0x0100 | DISP のスキュー（×1〜3 メモリサイクル） |
| `ACRTC_OMR_RAM` | 0x0080 | 1：HSYNC の間に DRAM をリフレッシュしない |

### DCR のビット（U85 p.75–78）。SE は 0x 使わない、10 ブランク、11 表示（基本画面の SE1 は 1 bit）（[15]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_DCR_DSP` | 0x8000 |  |
| `ACRTC_DCR_BASE` | 0x4000 | 基本画面を表示（BIOS が書く値。DR-071） |
| `ACRTC_DCR_UPPER` | 0x3000 | 上画面を表示（ブランクは $2000） |
| `ACRTC_DCR_LOWER` | 0x0C00 | 下画面を表示（ブランクは $0800） |
| `ACRTC_DCR_WIN` | 0x0300 | ウィンドウを表示（ブランクは $0200） |

### CDR のビット（U85 p.107–109）。CON・COFF は 4 フィールド単位（[15]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_CDR_GRAPHIC` | 0x8000 | GCR のカーソルを CUD1 に |
| `ACRTC_CDR_CROSS` | 0xC000 | 十字（横が CUD1、縦が CUD2） |
| `ACRTC_CDR_CON1` | 0x0800 | ×1〜7：ON は 4×(n＋1) フィールド。0 は出さない |
| `ACRTC_CDR_COFF1` | 0x0100 | ×1〜7：OFF は 4×(n＋1) フィールド。0 は常に ON |

### 案B の表示（256×192、4 bpp、262 ライン、台帳 HD63484.md §3.4）[P]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_B_HSR` | 0xAA0D |  |
| `ACRTC_B_HDR` | 0x127F |  |
| `ACRTC_B_VSR` | 0x0106 |  |
| `ACRTC_B_VDR` | 0x2703 |  |
| `ACRTC_B_SSW` | 0x00C0 |  |
| `ACRTC_B_MW` | 0x0040 | 1 行 64 語（256 画素） |
| `ACRTC_B_OMR` | 0xC008 | M/S＝1、STR＝1、ACM＝10（DSK・CSK は 0） |

### コマンド（U85 p.173）。下位ビット：OPM（2–0）、COL（4–3）、AREA（7–5）（[15]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_ORG` | 0x0400 | DPH, DPL |
| `ACRTC_WPR` | 0x0800 | \| rn、data |
| `ACRTC_RPR` | 0x0C00 | \| rn |
| `ACRTC_WPTN` | 0x1800 | \| pra、n、data×n |
| `ACRTC_RPTN` | 0x1C00 | \| pra、n |
| `ACRTC_DRD` | 0x2400 | AX, AY |
| `ACRTC_DWT` | 0x2800 | AX, AY、data×(\|AX\|+1)(\|AY\|+1) |
| `ACRTC_DMOD` | 0x2C00 | \| mm、AX, AY |
| `ACRTC_RD` | 0x4400 |  |
| `ACRTC_WT` | 0x4800 | data |
| `ACRTC_MOD` | 0x4C00 | \| mm、data |
| `ACRTC_CLR` | 0x5800 | data, AX, AY（RWP から） |
| `ACRTC_SCLR` | 0x5C00 | \| mm、data, AX, AY |
| `ACRTC_CPY` | 0x6000 | \| S \| DSD、Sah, Sal, AX, AY |
| `ACRTC_SCPY` | 0x7000 |  |
| `ACRTC_AMOVE` | 0x8000 | X, Y |
| `ACRTC_RMOVE` | 0x8400 | dX, dY |
| `ACRTC_ALINE` | 0x8800 | X, Y |
| `ACRTC_RLINE` | 0x8C00 | dX, dY |
| `ACRTC_ARCT` | 0x9000 | X, Y |
| `ACRTC_RRCT` | 0x9400 | dX, dY |
| `ACRTC_APLL` | 0x9800 | n, X1, Y1, ... |
| `ACRTC_RPLL` | 0x9C00 |  |
| `ACRTC_APLG` | 0xA000 |  |
| `ACRTC_RPLG` | 0xA400 |  |
| `ACRTC_CRCL` | 0xA800 | r（\| ACRTC_CW で時計回り） |
| `ACRTC_ELPS` | 0xAC00 | a, b, dX |
| `ACRTC_AARC` | 0xB000 |  |
| `ACRTC_RARC` | 0xB400 |  |
| `ACRTC_AEARC` | 0xB800 |  |
| `ACRTC_REARC` | 0xBC00 |  |
| `ACRTC_AFRCT` | 0xC000 | X, Y（CP から X・Y までを塗る） |
| `ACRTC_RFRCT` | 0xC400 | dX, dY |
| `ACRTC_PAINT` | 0xC800 |  |
| `ACRTC_DOT` | 0xCC00 |  |
| `ACRTC_PTN` | 0xD000 | \| S \| SD、SZ |
| `ACRTC_AGCPY` | 0xE000 |  |
| `ACRTC_RGCPY` | 0xF000 |  |
| `ACRTC_CW` | 0x0100 | 円・弧：時計回り |

### 演算（OPM）（[15]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_OPM_REP` | 0 | 置き換え |
| `ACRTC_OPM_OR` | 1 |  |
| `ACRTC_OPM_AND` | 2 |  |
| `ACRTC_OPM_EOR` | 3 |  |

### 色（COL）（[15]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_COL_PAT` | 0x00 | パターンのビットで CL0・CL1 |
| `ACRTC_COL_CL1` | 0x08 | 1 は CL1、0 は描かない |
| `ACRTC_COL_CL0` | 0x10 | 0 は CL0、1 は描かない |
| `ACRTC_COL_DIR` | 0x18 | パターン RAM の語をそのまま |

### パラメータレジスタ（WPR の rn）（[15]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `ACRTC_P_CL0` | 0x00 |  |
| `ACRTC_P_CL1` | 0x01 |  |
| `ACRTC_P_CCMP` | 0x02 |  |
| `ACRTC_P_EDG` | 0x03 |  |
| `ACRTC_P_MASK` | 0x04 |  |
| `ACRTC_P_PRC0` | 0x05 |  |
| `ACRTC_P_PRC1` | 0x06 |  |
| `ACRTC_P_PRC2` | 0x07 |  |
| `ACRTC_P_ADR_XMIN` | 0x08 |  |
| `ACRTC_P_ADR_YMIN` | 0x09 |  |
| `ACRTC_P_ADR_XMAX` | 0x0A |  |
| `ACRTC_P_ADR_YMAX` | 0x0B |  |
| `ACRTC_P_RWPH` | 0x0C | DN<<14 \| 語番地の bit 19–12 |
| `ACRTC_P_RWPL` | 0x0D | 語番地の bit 11–0 << 4 |

## 05　DOC ES5503（§05、DR-019、DR-030）[P]

子基板。ネイティブのレジスタ n は DOC+2n の上位バイト。MOVE.B の 1 回の読みか書きだけ。子基板がなければバスエラー（FABRIC_MODULES の bit 2 で確かめる）。波形 RAM は Fabric の port 2 からだけ読み書きできる。

先頭：`DOC`＝0x210000。

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `DOC_FREQ_LO` | 0x210000〜0x21003E（32 個、2 ごと） | 8（上位レーン） | rw |  |  | doc | +2×osc |
| `DOC_FREQ_HI` | 0x210040〜0x21007E（32 個、2 ごと） | 8（上位レーン） | rw |  |  | doc |  |
| `DOC_VOLUME` | 0x210080〜0x2100BE（32 個、2 ごと） | 8（上位レーン） | rw |  |  | doc |  |
| `DOC_DATA` | 0x2100C0〜0x2100FE（32 個、2 ごと） | 8（上位レーン） | r |  |  | doc | 最後に読んだ波形の値（読出し） |
| `DOC_POINTER` | 0x210100〜0x21013E（32 個、2 ごと） | 8（上位レーン） | rw |  |  | doc | 波形テーブルの頁（番地の bit 15–8） |
| `DOC_CONTROL` | 0x210140〜0x21017E（32 個、2 ごと） | 8（上位レーン） | rw |  |  | doc |  |
| `DOC_SIZE` | 0x210180〜0x2101BE（32 個、2 ごと） | 8（上位レーン） | rw |  |  | doc | bit 6 BS、bit 5–3 テーブルの大きさ T、bit 2–0 分解能 R |
| `DOC_IRQ` | 0x2101C0（native 0xE0） | 8（上位レーン） | r |  | 割込みの要求を 1 つ取る | doc | 読むと割込みの要求を 1 つ取る（DOC の割込みを受ける処理だけが、1 回につき 1 回読む。DR-123） |
| `DOC_OSC_ENABLE` | 0x2101C2（native 0xE1） | 8（上位レーン） | rw |  |  | doc | (N−1)×2。書くと走査を始める |
| `DOC_ADC` | 0x2101C4（native 0xE2） | 8（上位レーン） | r |  | A/D の変換を始める | doc | 読むと A/D の変換を始め、前の変換の結果を返す。読んで 30 µs 以上待ってから読み直す（DR-123） |

### オシレータの並び

| 名前 | 値 | 内容 |
|---|---|---|
| `DOC_OSC_STRIDE` | 2 | オシレータ 1 つぶんの番地の差 |

### CONTROL のビット（[18]）[D]

対象：`DOC_CONTROL`。

| 名前 | 値 | 内容 |
|---|---|---|
| `DOC_CTL_HALT` | 0x01 |  |
| `DOC_CTL_FREE` | 0x00 | モード（bit 2–1） |
| `DOC_CTL_ONESHOT` | 0x02 |  |
| `DOC_CTL_SYNC` | 0x04 |  |
| `DOC_CTL_SWAP` | 0x06 |  |
| `DOC_CTL_IE` | 0x08 |  |
| `DOC_CTL_CA_SH` | 4 | CA3–0（bit 7–4）。偶数は L、奇数は R（DR-054） |

## 06　DSP bridge（§05、DR-029、DR-049、DR-053）[P]

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `DSP_CTRL` | 0x220000 | 16 | rw |  |  |  | bit 0 RESET、bit 2–1 BIOSEL、bit 3 IRQ_EN |
| `DSP_STATUS` | 0x220002 | 16 | r | 0x0060 |  |  | bit 0 PRGBUS、1 入力の語あり、2 出力に空き、3 COMMAND、4 RESPONSE、5 UNDERRUN（W1C）、6 OVERRUN（W1C） |
| `DSP_MAILBOX` | 0x220004 | 16 | w |  |  |  |  |
| `DSP_RESPONSE` | 0x220006 | 16 | r |  | RESPONSE と割込みの要求を下げる（DR-053） |  |  |
| `DSP_PROGRAM` | 0x280000〜0x281FFE（4096 個、2 ごと） | 16 | rw |  |  | dsp_prgbus | 4K 語。DSP をリセットし PRGBUS＝1 の間だけ（語アクセス） |

### CTRL のビット

対象：`DSP_CTRL`。

| 名前 | 値 | 内容 |
|---|---|---|
| `DSP_CTRL_RESET` | 0x0001 |  |
| `DSP_CTRL_IRQ_EN` | 0x0008 |  |

### CTRL の BIOSEL（bit 2–1）：DSP の BIO に出す条件（DR-029 (3)、DR-049）

対象：`DSP_CTRL`。

| 名前 | 値 | 内容 |
|---|---|---|
| `DSP_CTRL_BIO_INPUT` | 0x0000 | 入力 FIFO が空でない（既定） |
| `DSP_CTRL_BIO_SPACE` | 0x0002 | 出力 FIFO に空き |
| `DSP_CTRL_BIO_MAIL` | 0x0004 | メールボックスに着信 |
| `DSP_CTRL_BIO_TICK` | 0x0006 | 音声クロックの拍のラッチ |

### STATUS のビット番号（BTST 用。MOVE.W で読んでから調べる。バイトのアクセスはバスエラー）

対象：`DSP_STATUS`。

| 名前 | 値 | 内容 |
|---|---|---|
| `DSP_ST_PRGBUS_B` | 0 |  |
| `DSP_ST_INPUT_B` | 1 |  |
| `DSP_ST_SPACE_B` | 2 |  |
| `DSP_ST_COMMAND_B` | 3 |  |
| `DSP_ST_RESPONSE_B` | 4 |  |

### STATUS の W1C のビット

対象：`DSP_STATUS`。

| 名前 | 値 | 内容 |
|---|---|---|
| `DSP_ST_UNDERRUN` | 0x0020 | W1C |
| `DSP_ST_OVERRUN` | 0x0040 | W1C |

### プログラム RAM の大きさ

| 名前 | 値 | 内容 |
|---|---|---|
| `DSP_PROGRAM_WORDS` | 4096 |  |

## 07　ADPCM bridge と本体 DAC（§05、DR-017、DR-019、DR-053）[P]

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `ADPCM_CTRL` | 0x230000 | 16 | rw |  |  |  | bit 0 RESET、1 S1、2 S2、3 下位ニブルから、4 置換しない、5 半分空きの割込み |
| `ADPCM_STATUS` | 0x230002 | 16 | r | 0x0100 |  |  | bit 6–0 FIFO の byte 数（0〜64）、bit 7 は 0、bit 8 UNDERRUN（W1C）、bit 9 半分空き、bit 10 V̄C̄K̄ |
| `DAC_CTRL` | 0x230010 | 16 | rw |  |  | no_doc | bit 0 DIRECT。DOC の子基板があるときはバスエラー |
| `DAC_STATUS` | 0x230012 | 16 | r | 0x0100 |  | no_doc | bit 6–0 FIFO の語数（0〜64）、bit 7 は 0、bit 8 UNDERRUN（W1C） |
| `DAC_DATA` | 0x230014 | 16 | rw |  |  | no_doc | DIRECT のとき：上位バイト L、下位バイト R（符号なし、$80 が中央） |

### 本体 DAC の CTRL と DATA

| 名前 | 値 | 内容 |
|---|---|---|
| `DAC_CTRL_DIRECT` | 0x0001 |  |
| `DAC_CENTER` | 0x8080 | DATA の中央（L・R とも $80） |

### ADPCM の CTRL のビット（DR-053）

対象：`ADPCM_CTRL`。S1・S2 は MSM5205 の標本化：S1＝1・S2＝0 で 8 kHz、S1＝0・S2＝1 で 6 kHz、両方 0 で 4 kHz（384 kHz の 1/48・1/64・1/96、台帳 MSM5205.md §3.1）。両方 1 は Bend（V̄C̄K̄ が入力になる。エミュレータでは取込みが起きない [V]）。RESET は電源投入時 1

| 名前 | 値 | 内容 |
|---|---|---|
| `ADPCM_CTL_RESET` | 0x0001 |  |
| `ADPCM_CTL_S1` | 0x0002 |  |
| `ADPCM_CTL_S2` | 0x0004 |  |
| `ADPCM_CTL_LOW_FIRST` | 0x0008 | ニブルを下位から |
| `ADPCM_CTL_NO_REPLACE` | 0x0010 | FIFO が空でも 0x08 に置き換えない（Bend） |
| `ADPCM_CTL_HALF_IRQ` | 0x0020 | 半分空きの割込み（レベル 3） |
| `ADPCM_CTL_8K` | 0x0002（ADPCM_CTL_S1） |  |
| `ADPCM_CTL_6K` | 0x0004（ADPCM_CTL_S2） |  |
| `ADPCM_CTL_4K` | 0x0000 |  |

### ADPCM の STATUS のビット（DR-053、DR-096）

対象：`ADPCM_STATUS`。

| 名前 | 値 | 内容 |
|---|---|---|
| `ADPCM_ST_COUNT` | 0x007F | FIFO の byte 数 |
| `ADPCM_ST_UNDERRUN` | 0x0100 | W1C |
| `ADPCM_ST_HALF` | 0x0200 | 残りが 32 byte 以下 |
| `ADPCM_ST_VCK` | 0x0400 | V̄C̄K̄ のピンのレベル（DR-096）。RESET を外すのは、1→0 を見てから 16 µs より後、次の 0→1 の前（取込みと立上りの間。OI-064） |

## 08　Temporal（§05、DR-053、DR-067）[P]

0x240020〜0x24003F は Capture RAM [O]。実装しないので、今はバスエラー。

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `TEMPORAL_WPTR` | 0x240000 | 16 | rw |  |  |  |  |
| `TEMPORAL_RPTR` | 0x240002 | 16 | rw |  |  |  |  |
| `TEMPORAL_CTRL` | 0x240004 | 16 | rw | 0x0300 |  |  | 下の TMP_*（DR-067） |
| `TEMPORAL_DIST` | 0x240006 | 16 | r |  |  |  | WPTR−RPTR |

### CTRL のビット

対象：`TEMPORAL_CTRL`。

| 名前 | 値 | 内容 |
|---|---|---|
| `TMP_CLEAR` | 0x0001 | 両方のポインタと印を 0 に |
| `TMP_SYNC` | 0x0002 | VBlank の始まりで両方のポインタを 0 に |
| `TMP_RSYNC` | 0x0004 | VBlank の始まりで読出しのポインタだけを 0 に（繰返し） |
| `TMP_HOLD` | 0x0008 | 書込みの /WE を止める。ポインタは進む（保持） |
| `TMP_EQUAL` | 0x0010 | 読出し専用：WPTR＝RPTR |
| `TMP_OVERRUN` | 0x0100 | 書く側が追いついた。W1C |
| `TMP_UNDERRUN` | 0x0200 | 読む側が追いついた。W1C |

## 09　Data Fabric（§06、DR-046）[P]

先頭：`FABRIC`＝0x250000。

チャネルの中の offset：`FABRIC_CH0`＝0x250000、`FABRIC_CH1`＝0x250040（間隔 `FABRIC_CH_SIZE`＝0x40）。チャネル ch のレジスタは FABRIC_CH0＋FABRIC_CH_SIZE×ch＋offset。チャネル 2・3 の窓（予約）と、各チャネルの 0x20〜0x3E はバスエラー。

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `FAB_SRC` | +0x00 | 16 | w |  |  |  | ポートの ID |
| `FAB_DST` | +0x02 | 16 | w |  |  |  |  |
| `FAB_SRC_ADDR`、`FAB_SRC_ADDR_L`（下位語） | +0x04 | 32（2 語） | rw |  |  |  | 32 bit（下位語 $06 だけを使う） |
| `FAB_DST_ADDR`、`FAB_DST_ADDR_L`（下位語） | +0x08 | 32（2 語） | rw |  |  |  |  |
| `FAB_COUNT` | +0x0C | 16 | rw |  |  |  |  |
| `FAB_SRC_STEP` | +0x0E | 16 | w |  |  |  |  |
| `FAB_DST_STEP` | +0x10 | 16 | w |  |  |  |  |
| `FAB_FORMAT` | +0x12 | 16 | w |  |  |  |  |
| `FAB_MASK` | +0x14 | 16 | w |  |  |  |  |
| `FAB_XOR` | +0x16 | 16 | w |  |  |  |  |
| `FAB_SHIFT` | +0x18 | 16 | w |  |  |  |  |
| `FAB_TRIGGER` | +0x1A | 16 | w |  |  |  |  |
| `FAB_CTRL` | +0x1C | 16 | w |  |  |  |  |
| `FAB_STATUS` | +0x1E | 16 | r | 0x001A |  |  |  |

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `FABRIC_PORT_RESET` | 0x250100 | 16 | w |  |  |  | bit n：port n の FIFO・ポインタを初期化（書込み専用） |
| `FABRIC_AUDIO_N` | 0x250102 | 16 | w |  |  |  | 音声クロック＝8 MHz÷N（12 bit） |
| `FABRIC_MODULES` | 0x250104 | 16 | r |  |  |  | 読出し専用 |

### MODULES のビット

対象：`FABRIC_MODULES`。

| 名前 | 値 | 内容 |
|---|---|---|
| `FABRIC_MOD_DOC` | 0x0004 | bit 2：DOC 子基板 |
| `FABRIC_MOD_DOC_B` | 2 |  |

### ポートの ID

対象：`FAB_SRC`。

| 名前 | 値 | 内容 |
|---|---|---|
| `PORT_STAGING` | 0 |  |
| `PORT_ACRTC` | 1 |  |
| `PORT_DOC` | 2 | 子基板がないときは本体 DAC の FIFO（語、上位 L・下位 R） |
| `PORT_DSP_IN` | 3 |  |
| `PORT_DSP_OUT` | 4 |  |
| `PORT_ADPCM` | 5 |  |
| `PORT_TEMPORAL_W` | 6 |  |
| `PORT_TEMPORAL_R` | 7 |  |
| `PORT_CAPTURE` | 8 |  |
| `PORT_CHAR_ROM` | 9 |  |
| `PORT_AUDIO_IN` | 10 |  |

### FORMAT

対象：`FAB_FORMAT`。

| 名前 | 値 | 内容 |
|---|---|---|
| `FMT_8_8` | 0 |  |
| `FMT_16_16` | 1 |  |
| `FMT_8_16` | 2 | ゼロ拡張 |
| `FMT_16_8_LO` | 3 |  |
| `FMT_16_8_HI` | 4 |  |
| `FMT_16_ROT` | 5 | SHIFT で回転 |

### TRIGGER の発生源（bit 2–0）（DR-018）

対象：`FAB_TRIGGER`。

| 名前 | 値 | 内容 |
|---|---|---|
| `TRG_NOW` | 0 |  |
| `TRG_VBLANK` | 1 |  |
| `TRG_HBLANK` | 2 |  |
| `TRG_AUDIO` | 3 |  |
| `TRG_EXTRIG` | 4 |  |
| `TRG_ADPCM_VCK` | 5 |  |

### TRIGGER のビット

対象：`FAB_TRIGGER`。

| 名前 | 値 | 内容 |
|---|---|---|
| `TRG_EACH` | 0x0008 | 1 回のトリガで 1 要素 |
| `TRG_LOOP` | 0x0010 |  |
| `TRG_IRQ_EN` | 0x0020 | DONE か ERROR で割込み（レベル 3） |

### CTRL

対象：`FAB_CTRL`。

| 名前 | 値 | 内容 |
|---|---|---|
| `FAB_START` | 0x0001 |  |
| `FAB_STOP` | 0x0002 |  |

### STATUS

対象：`FAB_STATUS`。

| 名前 | 値 | 内容 |
|---|---|---|
| `FAB_ST_BUSY` | 0x0001 |  |
| `FAB_ST_DONE` | 0x0002 | W1C |
| `FAB_ST_STALL` | 0x0004 |  |
| `FAB_ST_ERROR` | 0x0008 | W1C |
| `FAB_ST_MISS` | 0x0010 | W1C |
| `FAB_ST_WAIT` | 0x0020 |  |
| `FAB_ST_W1C` | 0x001A |  |

## 10　入力と ACIA（§09、DR-028、DR-053、DR-055）[P]

先頭：`INPUT`＝0x260000。

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `PORT_A` | 0x260000 | 16 | r |  |  |  | コントローラ A（active-high） |
| `PORT_B` | 0x260002 | 16 | r |  |  |  | コントローラ B（同じ割当て） |
| `AXIS0` | 0x260004〜0x260012（8 個、2 ごと） | 16 | r |  |  |  | AXIS0〜7：$260004〜$260012（下位 8 bit にアナログ値） |
| `MIDI_CTRL` | 0x260014 | 8（上位レーン） | w |  |  |  | ACIA の制御（書込み）／状態（読出し）。上位レーン、MOVE.B |
| `MIDI_STATUS` | 0x260014 | 8（上位レーン） | r |  |  |  | ACIA の状態 |
| `MIDI_DATA` | 0x260016 | 8（上位レーン） | rw |  | 受けた byte を取り、RDRF を下げる |  | ACIA の送受信データ。上位レーン、MOVE.B |
| `INPUT_IRQ_STATUS` | 0x260018 | 16 | r | 0x000B |  |  | bit 0、1、3 は W1C。bit 2 は ACIA の IRQ の写し、bit 4 はカートリッジ（読出し専用） |
| `INPUT_IRQ_ENABLE` | 0x26001A | 16 | rw |  |  |  |  |

### INPUT_IRQ_STATUS・INPUT_IRQ_ENABLE のビット

対象：`INPUT_IRQ_STATUS`。

| 名前 | 値 | 内容 |
|---|---|---|
| `INPUT_IRQ_PORT_A` | 0x0001 |  |
| `INPUT_IRQ_PORT_B` | 0x0002 |  |
| `INPUT_IRQ_MIDI` | 0x0004 |  |
| `INPUT_IRQ_EXTRIG` | 0x0008 |  |
| `INPUT_IRQ_CART` | 0x0010 | INPUT_IRQ_STATUS だけ：カートリッジがある（端子 /CART。読出し専用、割込みにはならない）[P]（DR-095） |
| `INPUT_IRQ_CART_B` | 4 |  |

### PORT_A・PORT_B のビット（DR-055）。bit 15–8 は予約（0）[P]

対象：`PORT_A`。

| 名前 | 値 | 内容 |
|---|---|---|
| `PAD_UP` | 0x0001 |  |
| `PAD_DOWN` | 0x0002 |  |
| `PAD_LEFT` | 0x0004 |  |
| `PAD_RIGHT` | 0x0008 |  |
| `PAD_A` | 0x0010 |  |
| `PAD_B` | 0x0020 |  |
| `PAD_C` | 0x0040 |  |
| `PAD_START` | 0x0080 |  |

### PORT_A・PORT_B のビット番号（BTST 用）[P]

対象：`PORT_A`。

| 名前 | 値 | 内容 |
|---|---|---|
| `PAD_UP_B` | 0 |  |
| `PAD_DOWN_B` | 1 |  |
| `PAD_LEFT_B` | 2 |  |
| `PAD_RIGHT_B` | 3 |  |
| `PAD_A_B` | 4 |  |
| `PAD_B_B` | 5 |  |
| `PAD_C_B` | 6 |  |
| `PAD_START_B` | 7 |  |

### HD6850 ACIA の制御（[23]）[D]

対象：`MIDI_CTRL`。

| 名前 | 値 | 内容 |
|---|---|---|
| `ACIA_RESET` | 0x03 | マスタリセット |
| `ACIA_DIV16` | 0x01 | 500 kHz÷16＝31.25 kbit/s |
| `ACIA_8N1` | 0x14 |  |
| `ACIA_RIE` | 0x80 | 受信の割込み |
| `ACIA_MIDI` | 0x95（ACIA_RIE\|ACIA_8N1\|ACIA_DIV16） | $95 |

### HD6850 ACIA の状態（[23]）[D]

対象：`MIDI_STATUS`。

| 名前 | 値 | 内容 |
|---|---|---|
| `ACIA_ST_RDRF` | 0x01 |  |
| `ACIA_ST_TDRE` | 0x02 |  |
| `ACIA_ST_OVRN` | 0x20 |  |
| `ACIA_ST_IRQ` | 0x80 |  |

## 11　パレットと映像（§05、DR-027、DR-053）[P]

| 名前 | 番地 | 幅 | アクセス | W1C | 読むと | 条件 | 内容 |
|---|---|---|---|---|---|---|---|
| `PALETTE` | 0x270000〜0x27001E（16 個、2 ごと） | 16 | w |  |  |  | $0RGB×16。書込み専用、語アクセス。次のラインから効く |
| `PAL_CTRL` | 0x270020 | 16 | rw |  |  |  | bit 0 DIRECT（Bend） |
| `VIDEO_IRQ` | 0x270022 | 16 | rw | 0x0300 |  |  | 割込みの許可と保留（W1C）、今のレベル。下の VIRQ_* |
| `VIDEO_LINE` | 0x270024 | 16 | r |  |  |  | 今のラスタ |
| `VIDEO_LINE_CMP` | 0x270026 | 16 | rw |  |  |  | 12 bit |

### VIDEO_IRQ のビット

対象：`VIDEO_IRQ`。

| 名前 | 値 | 内容 |
|---|---|---|
| `VIRQ_VBL_EN` | 0x0001 |  |
| `VIRQ_HBL_EN` | 0x0002 |  |
| `VIRQ_LINE_CMP` | 0x0004 | HBlank の割込みを LINE_CMP の 1 ラインだけにする |
| `VIRQ_VBL` | 0x0100 | 保留（W1C） |
| `VIRQ_HBL` | 0x0200 | 保留（W1C） |
| `VIRQ_HBLANK` | 0x4000 | 今のレベル |
| `VIRQ_VBLANK` | 0x8000 |  |

### 画面の大きさ

| 名前 | 値 | 内容 |
|---|---|---|
| `SCREEN_W` | 256 |  |
| `SCREEN_H` | 192 |  |

## 12　割込み（§05、DR-028）[P]

すべてオートベクタ。レベル L のベクタは 24＋L。

### レベル

| 名前 | 値 | 内容 |
|---|---|---|
| `IRQ_VIDEO` | 6 | VBlank・HBlank、ACRTC |
| `IRQ_ACIA` | 5 |  |
| `IRQ_DOC` | 4 |  |
| `IRQ_FABRIC` | 3 | Data Fabric、ADPCM bridge |
| `IRQ_DSP` | 2 |  |
| `IRQ_INPUT` | 1 |  |

### 68000 の例外のベクタの番号（[31]）[D]

| 名前 | 値 | 内容 |
|---|---|---|
| `VEC_BUS_ERROR` | 2 |  |
| `VEC_ADDR_ERROR` | 3 |  |
| `VEC_ILLEGAL` | 4 |  |
| `VEC_ZERO_DIV` | 5 |  |
| `VEC_CHK` | 6 |  |
| `VEC_TRAPV` | 7 |  |
| `VEC_PRIVILEGE` | 8 |  |
| `VEC_TRACE` | 9 |  |
| `VEC_LINE_A` | 10 |  |
| `VEC_LINE_F` | 11 |  |
| `VEC_SPURIOUS` | 24 |  |
| `VEC_AUTO1` | 25 | レベル 1〜7：25〜31 |
| `VEC_TRAP0` | 32 | TRAP #0〜15：32〜47 |
