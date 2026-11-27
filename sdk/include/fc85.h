/* 生成物。直すときは docs/spec/registers.toml（py tools/regmap/regmap.py で作り直す。DR-100）。手で直さない。
 * fc85.h：本体の番地とレジスタ（68000 から見た MMIO）。C（m68k-elf-gcc）用。
 *
 * 番地・幅・アクセス・ビットの名前と値は docs/spec/registers.toml が正（DR-100）。仕様書（docs/spec/hardware.md）
 * §05 のアドレスマップ、§06 の Data Fabric、§09 の入力と、DR-016・DR-019・DR-025・DR-027・DR-028・DR-029・
 * DR-046・DR-053・DR-055 から書き起こした。動作の説明と根拠は仕様書と DR。食い違えば OI にしてそろえる。
 * 記述ステータスは節と見出しの印に従う（ほとんど [P]）。
 *
 * アクセスの規約（仕様書 §05、DR-016・DR-030・DR-033・DR-052・DR-053）：
 * - 16 bit のレジスタは語アクセスだけ。バイトのアクセスはバスエラー。
 * - 8 bit の IC（DOC、ACIA、文字 ROM）は上位バイトレーン（偶数番地）。MOVE.B で偶数番地を使う。
 * - 未割当の番地はバスエラー（64 クロックの時間切れ）。
 * - ROM と読出し専用のレジスタへの書込みは捨て、書込み専用のレジスタは読むと 0（同じ番地にもう一方の向きの
 *   レジスタがあれば、そのレジスタを読み書きする）。
 * - ACRTC の AR/SR とデータポート、DOC の窓には、CLR・TAS・BSET などの読み書き一体の命令を使わない。
 * - ACRTC の FIFO には、SR の WFR（書込み）・RFR（読出し）を確かめてからアクセスする。
 * - DOC の 0xE0・0xE2 は読むと状態が変わる。0xE0 は DOC の割込みを受ける処理（BIOS の既定の処理か、ベクタ 28 を
 *   書き換えたアプリ）だけが割込み 1 回につき 1 回読み、0xE2 は A/D 変換の手順でだけ読む（DR-123）。
 * 書式：名前と値は fc85.inc と同じ。REG_名前 はそのレジスタ（volatile のポインタを通した左辺値）。
 * 並び（PALETTE、AXIS0、DOC のオシレータ、DSP_PROGRAM）は REG_名前(n)、Data Fabric のチャネルの
 * レジスタは REG_名前(ch)。8 bit の装置は上位バイトレーンの偶数番地を 1 byte で読み書きする（FC85_IO8）。
 */
#ifndef FC85_H
#define FC85_H

#include <stdint.h>

/* 番地 a の 8・16・32 bit のレジスタ（volatile のポインタを通した左辺値） */
#define FC85_IO8(a)  (*(volatile uint8_t *)(uintptr_t)(a))
#define FC85_IO16(a) (*(volatile uint16_t *)(uintptr_t)(a))
#define FC85_IO32(a) (*(volatile uint32_t *)(uintptr_t)(a))

/* ------------------------------------------------------------ メモリ（§05）[P] */

#define BOOT_ROM               0x000000UL     /* 64 KiB */
#define CHAR_ROM               0x010000UL     /* 文字 ROM。ROM の offset n は CHAR_ROM+2n（上位レーン） */
#define MAIN_RAM               0x100000UL     /* 128 KiB */
#define MAIN_RAM_END           0x120000UL
#define JUMP_TABLE             0x100000UL     /* 例外のジャンプ表。ベクタ n は JUMP_TABLE+6n（JMP abs.L）（DR-025） */
#define JUMP_TABLE_END         0x100600UL
#define STAGING                0x120000UL     /* 64 KiB の AV staging RAM（Fabric の port 0） */
#define STAGING_END            0x130000UL
#define CART                   0x400000UL     /* カートリッジの窓（最大 1 MiB）。なければバスエラー */
#define CART_END               0x500000UL

/* ------------------------------------------------------------ ACRTC HD63484（§05、DR-016、DR-033）[P] */

#define ACRTC_AR               0x200000UL     /* 書込み：AR（レジスタの番号） */
#define REG_ACRTC_AR           FC85_IO16(ACRTC_AR)
#define ACRTC_SR               0x200000UL     /* 読出し：SR（上位バイトは 1） */
#define REG_ACRTC_SR           FC85_IO16(ACRTC_SR)
#define ACRTC_DATA             0x200002UL     /* AR が指すレジスタ。r00 は FIFO */
#define REG_ACRTC_DATA         FC85_IO16(ACRTC_DATA)

/* SR のビット（[15]）[D] */
#define ACRTC_SR_CER           0x80U          /* コマンドの誤り */
#define ACRTC_SR_ARD           0x40U          /* 領域の検出 */
#define ACRTC_SR_CED           0x20U          /* コマンドの終わり */
#define ACRTC_SR_LPD           0x10U          /* ライトペン */
#define ACRTC_SR_RFF           0x08U          /* 読出し FIFO が満杯 */
#define ACRTC_SR_RFR           0x04U          /* 読出し FIFO に語がある */
#define ACRTC_SR_WFR           0x02U          /* 書込み FIFO に空きがある */
#define ACRTC_SR_WFE           0x01U          /* 書込み FIFO が空 */

/* SR のビット番号（BTST 用）（[15]）[D] */
#define ACRTC_SR_CER_B         7
#define ACRTC_SR_CED_B         5
#define ACRTC_SR_RFR_B         2
#define ACRTC_SR_WFR_B         1
#define ACRTC_SR_WFE_B         0

/* レジスタの番号（AR に書く値）。r80 以降は語ごとに AR が 2 進む（[15]）[D] */
#define ACRTC_FIFO             0x00U
#define ACRTC_CCR              0x02U          /* コマンドの制御 */
#define ACRTC_OMR              0x04U          /* 動作モード */
#define ACRTC_DCR              0x06U          /* 表示の制御 */
#define ACRTC_RCR              0x80U          /* ラスタの数（読出し） */
#define ACRTC_HSR              0x82U          /* 水平同期 */
#define ACRTC_HDR              0x84U          /* 水平表示 */
#define ACRTC_VSR              0x86U          /* 垂直同期 */
#define ACRTC_VDR              0x88U          /* 垂直表示 */
#define ACRTC_SSW              0x8AU          /* 分割画面の幅 1 */
#define ACRTC_SSW0             0x8CU          /* 上画面のラスタの数（U85 p.91） */
#define ACRTC_SSW2             0x8EU          /* 下画面のラスタの数 */
#define ACRTC_BCR              0x90U          /* 属性の点滅：BON1・BOFF1・BON2・BOFF2（4 bit ずつ、U85 p.92–93） */
#define ACRTC_HWR              0x92U          /* ウィンドウの横：上位 HWS−1（HSYNC の終わりから）、下位 HWW−1 */
#define ACRTC_VWS              0x94U          /* ウィンドウの縦の始まり−1（VSYNC の終わりから、U85 p.89–90） */
#define ACRTC_VWW              0x96U          /* ウィンドウのラスタの数 */
#define ACRTC_GCRX             0x98U          /* グラフィックカーソル：上位 CXE、下位 CXS（HSYNC の始まりからのメモリサイクル） */
#define ACRTC_GCRYS            0x9AU          /* CYS（VSYNC の終わりからのラスタ、U85 p.94） */
#define ACRTC_GCRYE            0x9CU          /* CYE */
#define ACRTC_MWR0             0xC2U          /* 上画面の記憶の幅 */
#define ACRTC_SAR0H            0xC4U
#define ACRTC_SAR0L            0xC6U
#define ACRTC_MWR1             0xCAU          /* 基本画面の記憶の幅 */
#define ACRTC_SAR1H            0xCCU          /* 基本画面の開始番地（bit 11–8 は SDA：横のスムーズスクロール） */
#define ACRTC_SAR1L            0xCEU
#define ACRTC_MWR2             0xD2U          /* 下画面 */
#define ACRTC_SAR2H            0xD4U
#define ACRTC_SAR2L            0xD6U
#define ACRTC_MWR3             0xDAU          /* ウィンドウ */
#define ACRTC_SAR3H            0xDCU
#define ACRTC_SAR3L            0xDEU
#define ACRTC_CDR              0xE8U          /* カーソルの定義（U85 p.107–109） */
#define ACRTC_ZFR              0xEAU          /* ズーム：bit 15–12 HZF、bit 11–8 VZF（倍率−1、基本画面だけ、U85 p.110） */

/* CCR のビット（[15]）[D] */
#define ACRTC_CCR_ABT          0x8000U        /* 打切り（リセットの後は 1） */
#define ACRTC_CCR_PSE          0x4000U
#define ACRTC_CCR_DDM          0x2000U        /* データの DMA（DWT・DRD・DMOD を Fabric の port 1 で運ぶ） */
#define ACRTC_CCR_CDM          0x1000U
#define ACRTC_CCR_DRC          0x0800U        /* DMA の要求：1 はサイクルスチール */
#define ACRTC_CCR_GBM4         0x0200U        /* 4 bit/画素 */

/* OMR のビット（[15]）[D] */
#define ACRTC_OMR_MS           0x8000U        /* マスタ */
#define ACRTC_OMR_STR          0x4000U        /* 表示と描画を始める */
#define ACRTC_OMR_ACM_I        0x0008U        /* インタリーブ */
#define ACRTC_OMR_ACP          0x2000U        /* 単一アクセスで描画を優先 */
#define ACRTC_OMR_CSK1         0x0400U        /* カーソルのスキュー（×1〜3 メモリサイクル。十字では 0 にしない） */
#define ACRTC_OMR_DSK1         0x0100U        /* DISP のスキュー（×1〜3 メモリサイクル） */
#define ACRTC_OMR_RAM          0x0080U        /* 1：HSYNC の間に DRAM をリフレッシュしない */

/* DCR のビット（U85 p.75–78）。SE は 0x 使わない、10 ブランク、11 表示（基本画面の SE1 は 1 bit）（[15]）[D] */
#define ACRTC_DCR_DSP          0x8000U
#define ACRTC_DCR_BASE         0x4000U        /* 基本画面を表示（BIOS が書く値。DR-071） */
#define ACRTC_DCR_UPPER        0x3000U        /* 上画面を表示（ブランクは $2000） */
#define ACRTC_DCR_LOWER        0x0C00U        /* 下画面を表示（ブランクは $0800） */
#define ACRTC_DCR_WIN          0x0300U        /* ウィンドウを表示（ブランクは $0200） */

/* CDR のビット（U85 p.107–109）。CON・COFF は 4 フィールド単位（[15]）[D] */
#define ACRTC_CDR_GRAPHIC      0x8000U        /* GCR のカーソルを CUD1 に */
#define ACRTC_CDR_CROSS        0xC000U        /* 十字（横が CUD1、縦が CUD2） */
#define ACRTC_CDR_CON1         0x0800U        /* ×1〜7：ON は 4×(n＋1) フィールド。0 は出さない */
#define ACRTC_CDR_COFF1        0x0100U        /* ×1〜7：OFF は 4×(n＋1) フィールド。0 は常に ON */

/* 案B の表示（256×192、4 bpp、262 ライン、台帳 HD63484.md §3.4）[P] */
#define ACRTC_B_HSR            0xAA0DU
#define ACRTC_B_HDR            0x127FU
#define ACRTC_B_VSR            0x0106U
#define ACRTC_B_VDR            0x2703U
#define ACRTC_B_SSW            0x00C0U
#define ACRTC_B_MW             0x0040U        /* 1 行 64 語（256 画素） */
#define ACRTC_B_OMR            0xC008U        /* M/S＝1、STR＝1、ACM＝10（DSK・CSK は 0） */

/* コマンド（U85 p.173）。下位ビット：OPM（2–0）、COL（4–3）、AREA（7–5）（[15]）[D] */
#define ACRTC_ORG              0x0400U        /* DPH, DPL */
#define ACRTC_WPR              0x0800U        /* | rn、data */
#define ACRTC_RPR              0x0C00U        /* | rn */
#define ACRTC_WPTN             0x1800U        /* | pra、n、data×n */
#define ACRTC_RPTN             0x1C00U        /* | pra、n */
#define ACRTC_DRD              0x2400U        /* AX, AY */
#define ACRTC_DWT              0x2800U        /* AX, AY、data×(|AX|+1)(|AY|+1) */
#define ACRTC_DMOD             0x2C00U        /* | mm、AX, AY */
#define ACRTC_RD               0x4400U
#define ACRTC_WT               0x4800U        /* data */
#define ACRTC_MOD              0x4C00U        /* | mm、data */
#define ACRTC_CLR              0x5800U        /* data, AX, AY（RWP から） */
#define ACRTC_SCLR             0x5C00U        /* | mm、data, AX, AY */
#define ACRTC_CPY              0x6000U        /* | S | DSD、Sah, Sal, AX, AY */
#define ACRTC_SCPY             0x7000U
#define ACRTC_AMOVE            0x8000U        /* X, Y */
#define ACRTC_RMOVE            0x8400U        /* dX, dY */
#define ACRTC_ALINE            0x8800U        /* X, Y */
#define ACRTC_RLINE            0x8C00U        /* dX, dY */
#define ACRTC_ARCT             0x9000U        /* X, Y */
#define ACRTC_RRCT             0x9400U        /* dX, dY */
#define ACRTC_APLL             0x9800U        /* n, X1, Y1, ... */
#define ACRTC_RPLL             0x9C00U
#define ACRTC_APLG             0xA000U
#define ACRTC_RPLG             0xA400U
#define ACRTC_CRCL             0xA800U        /* r（| ACRTC_CW で時計回り） */
#define ACRTC_ELPS             0xAC00U        /* a, b, dX */
#define ACRTC_AARC             0xB000U
#define ACRTC_RARC             0xB400U
#define ACRTC_AEARC            0xB800U
#define ACRTC_REARC            0xBC00U
#define ACRTC_AFRCT            0xC000U        /* X, Y（CP から X・Y までを塗る） */
#define ACRTC_RFRCT            0xC400U        /* dX, dY */
#define ACRTC_PAINT            0xC800U
#define ACRTC_DOT              0xCC00U
#define ACRTC_PTN              0xD000U        /* | S | SD、SZ */
#define ACRTC_AGCPY            0xE000U
#define ACRTC_RGCPY            0xF000U
#define ACRTC_CW               0x0100U        /* 円・弧：時計回り */

/* 演算（OPM）（[15]）[D] */
#define ACRTC_OPM_REP          0              /* 置き換え */
#define ACRTC_OPM_OR           1
#define ACRTC_OPM_AND          2
#define ACRTC_OPM_EOR          3

/* 色（COL）（[15]）[D] */
#define ACRTC_COL_PAT          0x00U          /* パターンのビットで CL0・CL1 */
#define ACRTC_COL_CL1          0x08U          /* 1 は CL1、0 は描かない */
#define ACRTC_COL_CL0          0x10U          /* 0 は CL0、1 は描かない */
#define ACRTC_COL_DIR          0x18U          /* パターン RAM の語をそのまま */

/* パラメータレジスタ（WPR の rn）（[15]）[D] */
#define ACRTC_P_CL0            0x00U
#define ACRTC_P_CL1            0x01U
#define ACRTC_P_CCMP           0x02U
#define ACRTC_P_EDG            0x03U
#define ACRTC_P_MASK           0x04U
#define ACRTC_P_PRC0           0x05U
#define ACRTC_P_PRC1           0x06U
#define ACRTC_P_PRC2           0x07U
#define ACRTC_P_ADR_XMIN       0x08U
#define ACRTC_P_ADR_YMIN       0x09U
#define ACRTC_P_ADR_XMAX       0x0AU
#define ACRTC_P_ADR_YMAX       0x0BU
#define ACRTC_P_RWPH           0x0CU          /* DN<<14 | 語番地の bit 19–12 */
#define ACRTC_P_RWPL           0x0DU          /* 語番地の bit 11–0 << 4 */

/* ------------------------------------------------------------ DOC ES5503（§05、DR-019、DR-030）[P] */
/* 子基板。ネイティブのレジスタ n は DOC+2n の上位バイト。MOVE.B の 1 回の読みか書きだけ。子基板がなければバスエラー（FABRIC_MODULES の bit 2 で確かめる）。波形 RAM は Fabric の port 2 からだけ読み書きできる。 */

#define DOC                    0x210000UL
#define DOC_FREQ_LO            (DOC+2*0x00)   /* +2×osc */
#define REG_DOC_FREQ_LO(n)     FC85_IO8(DOC_FREQ_LO + 2UL * (n))
#define DOC_FREQ_HI            (DOC+2*0x20)
#define REG_DOC_FREQ_HI(n)     FC85_IO8(DOC_FREQ_HI + 2UL * (n))
#define DOC_VOLUME             (DOC+2*0x40)
#define REG_DOC_VOLUME(n)      FC85_IO8(DOC_VOLUME + 2UL * (n))
#define DOC_DATA               (DOC+2*0x60)   /* 最後に読んだ波形の値（読出し） */
#define REG_DOC_DATA(n)        FC85_IO8(DOC_DATA + 2UL * (n))
#define DOC_POINTER            (DOC+2*0x80)   /* 波形テーブルの頁（番地の bit 15–8） */
#define REG_DOC_POINTER(n)     FC85_IO8(DOC_POINTER + 2UL * (n))
#define DOC_CONTROL            (DOC+2*0xA0)
#define REG_DOC_CONTROL(n)     FC85_IO8(DOC_CONTROL + 2UL * (n))
#define DOC_SIZE               (DOC+2*0xC0)   /* bit 6 BS、bit 5–3 テーブルの大きさ T、bit 2–0 分解能 R */
#define REG_DOC_SIZE(n)        FC85_IO8(DOC_SIZE + 2UL * (n))
#define DOC_IRQ                (DOC+2*0xE0)   /* 読むと割込みの要求を 1 つ取る（DOC の割込みを受ける処理だけが、1 回につき 1 回読む。DR-123） */
#define REG_DOC_IRQ            FC85_IO8(DOC_IRQ)
#define DOC_OSC_ENABLE         (DOC+2*0xE1)   /* (N−1)×2。書くと走査を始める */
#define REG_DOC_OSC_ENABLE     FC85_IO8(DOC_OSC_ENABLE)
#define DOC_ADC                (DOC+2*0xE2)   /* 読むと A/D の変換を始め、前の変換の結果を返す。読んで 30 µs 以上待ってから読み直す（DR-123） */
#define REG_DOC_ADC            FC85_IO8(DOC_ADC)

/* オシレータの並び */
#define DOC_OSC_STRIDE         2              /* オシレータ 1 つぶんの番地の差 */

/* CONTROL のビット（[18]）[D] */
#define DOC_CTL_HALT           0x01U
#define DOC_CTL_FREE           0x00U          /* モード（bit 2–1） */
#define DOC_CTL_ONESHOT        0x02U
#define DOC_CTL_SYNC           0x04U
#define DOC_CTL_SWAP           0x06U
#define DOC_CTL_IE             0x08U
#define DOC_CTL_CA_SH          4              /* CA3–0（bit 7–4）。偶数は L、奇数は R（DR-054） */

/* ------------------------------------------------------------ DSP bridge（§05、DR-029、DR-049、DR-053）[P] */

#define DSP_CTRL               0x220000UL     /* bit 0 RESET、bit 2–1 BIOSEL、bit 3 IRQ_EN */
#define REG_DSP_CTRL           FC85_IO16(DSP_CTRL)
#define DSP_STATUS             0x220002UL     /* bit 0 PRGBUS、1 入力の語あり、2 出力に空き、3 COMMAND、4 RESPONSE、5 UNDERRUN（W1C）、6 OVERRUN（W1C） */
#define REG_DSP_STATUS         FC85_IO16(DSP_STATUS)
#define DSP_MAILBOX            0x220004UL
#define REG_DSP_MAILBOX        FC85_IO16(DSP_MAILBOX)
#define DSP_RESPONSE           0x220006UL
#define REG_DSP_RESPONSE       FC85_IO16(DSP_RESPONSE)
#define DSP_PROGRAM            0x280000UL     /* 4K 語。DSP をリセットし PRGBUS＝1 の間だけ（語アクセス） */
#define REG_DSP_PROGRAM(n)     FC85_IO16(DSP_PROGRAM + 2UL * (n))

/* CTRL のビット */
#define DSP_CTRL_RESET         0x0001U
#define DSP_CTRL_IRQ_EN        0x0008U

/* CTRL の BIOSEL（bit 2–1）：DSP の BIO に出す条件（DR-029 (3)、DR-049） */
#define DSP_CTRL_BIO_INPUT     0x0000U        /* 入力 FIFO が空でない（既定） */
#define DSP_CTRL_BIO_SPACE     0x0002U        /* 出力 FIFO に空き */
#define DSP_CTRL_BIO_MAIL      0x0004U        /* メールボックスに着信 */
#define DSP_CTRL_BIO_TICK      0x0006U        /* 音声クロックの拍のラッチ */

/* STATUS のビット番号（BTST 用。MOVE.W で読んでから調べる。バイトのアクセスはバスエラー） */
#define DSP_ST_PRGBUS_B        0
#define DSP_ST_INPUT_B         1
#define DSP_ST_SPACE_B         2
#define DSP_ST_COMMAND_B       3
#define DSP_ST_RESPONSE_B      4

/* STATUS の W1C のビット */
#define DSP_ST_UNDERRUN        0x0020U        /* W1C */
#define DSP_ST_OVERRUN         0x0040U        /* W1C */

/* プログラム RAM の大きさ */
#define DSP_PROGRAM_WORDS      4096

/* ------------------------------------------------------------ ADPCM bridge と本体 DAC（§05、DR-017、DR-019、DR-053）[P] */

#define ADPCM_CTRL             0x230000UL     /* bit 0 RESET、1 S1、2 S2、3 下位ニブルから、4 置換しない、5 半分空きの割込み */
#define REG_ADPCM_CTRL         FC85_IO16(ADPCM_CTRL)
#define ADPCM_STATUS           0x230002UL     /* bit 6–0 FIFO の byte 数（0〜64）、bit 7 は 0、bit 8 UNDERRUN（W1C）、bit 9 半分空き、bit 10 V̄C̄K̄ */
#define REG_ADPCM_STATUS       FC85_IO16(ADPCM_STATUS)
#define DAC_CTRL               0x230010UL     /* bit 0 DIRECT。DOC の子基板があるときはバスエラー */
#define REG_DAC_CTRL           FC85_IO16(DAC_CTRL)
#define DAC_STATUS             0x230012UL     /* bit 6–0 FIFO の語数（0〜64）、bit 7 は 0、bit 8 UNDERRUN（W1C） */
#define REG_DAC_STATUS         FC85_IO16(DAC_STATUS)
#define DAC_DATA               0x230014UL     /* DIRECT のとき：上位バイト L、下位バイト R（符号なし、$80 が中央） */
#define REG_DAC_DATA           FC85_IO16(DAC_DATA)

/* 本体 DAC の CTRL と DATA */
#define DAC_CTRL_DIRECT        0x0001U
#define DAC_CENTER             0x8080U        /* DATA の中央（L・R とも $80） */

/* ADPCM の CTRL のビット（DR-053）。S1・S2 は MSM5205 の標本化：S1＝1・S2＝0 で 8 kHz、S1＝0・S2＝1 で 6 kHz、両方 0 で 4 kHz（384 kHz の 1/48・1/64・1/96、台帳 MSM5205.md §3.1）。両方 1 は Bend（V̄C̄K̄ が入力になる。エミュレータでは取込みが起きない [V]）。RESET は電源投入時 1 */
#define ADPCM_CTL_RESET        0x0001U
#define ADPCM_CTL_S1           0x0002U
#define ADPCM_CTL_S2           0x0004U
#define ADPCM_CTL_LOW_FIRST    0x0008U        /* ニブルを下位から */
#define ADPCM_CTL_NO_REPLACE   0x0010U        /* FIFO が空でも 0x08 に置き換えない（Bend） */
#define ADPCM_CTL_HALF_IRQ     0x0020U        /* 半分空きの割込み（レベル 3） */
#define ADPCM_CTL_8K           (ADPCM_CTL_S1)
#define ADPCM_CTL_6K           (ADPCM_CTL_S2)
#define ADPCM_CTL_4K           0x0000U

/* ADPCM の STATUS のビット（DR-053、DR-096） */
#define ADPCM_ST_COUNT         0x007FU        /* FIFO の byte 数 */
#define ADPCM_ST_UNDERRUN      0x0100U        /* W1C */
#define ADPCM_ST_HALF          0x0200U        /* 残りが 32 byte 以下 */
#define ADPCM_ST_VCK           0x0400U        /* V̄C̄K̄ のピンのレベル（DR-096）。RESET を外すのは、1→0 を見てから 16 µs より後、次の 0→1 の前（取込みと立上りの間。OI-064） */

/* ------------------------------------------------------------ Temporal（§05、DR-053、DR-067）[P] */
/* 0x240020〜0x24003F は Capture RAM [O]。実装しないので、今はバスエラー。 */

#define TEMPORAL_WPTR          0x240000UL
#define REG_TEMPORAL_WPTR      FC85_IO16(TEMPORAL_WPTR)
#define TEMPORAL_RPTR          0x240002UL
#define REG_TEMPORAL_RPTR      FC85_IO16(TEMPORAL_RPTR)
#define TEMPORAL_CTRL          0x240004UL     /* 下の TMP_*（DR-067） */
#define REG_TEMPORAL_CTRL      FC85_IO16(TEMPORAL_CTRL)
#define TEMPORAL_DIST          0x240006UL     /* WPTR−RPTR */
#define REG_TEMPORAL_DIST      FC85_IO16(TEMPORAL_DIST)

/* CTRL のビット */
#define TMP_CLEAR              0x0001U        /* 両方のポインタと印を 0 に */
#define TMP_SYNC               0x0002U        /* VBlank の始まりで両方のポインタを 0 に */
#define TMP_RSYNC              0x0004U        /* VBlank の始まりで読出しのポインタだけを 0 に（繰返し） */
#define TMP_HOLD               0x0008U        /* 書込みの /WE を止める。ポインタは進む（保持） */
#define TMP_EQUAL              0x0010U        /* 読出し専用：WPTR＝RPTR */
#define TMP_OVERRUN            0x0100U        /* 書く側が追いついた。W1C */
#define TMP_UNDERRUN           0x0200U        /* 読む側が追いついた。W1C */

/* ------------------------------------------------------------ Data Fabric（§06、DR-046）[P] */

#define FABRIC                 0x250000UL
#define FABRIC_CH0             0x250000UL
#define FABRIC_CH1             0x250040UL
#define FABRIC_CH_SIZE         0x40U
/* チャネルの中の offset。チャネル ch のレジスタは FABRIC_CH0＋FABRIC_CH_SIZE×ch＋offset。チャネル 2・3 の窓（予約）と、各チャネルの 0x20〜0x3E はバスエラー。 */
#define FAB_SRC                0x00U          /* ポートの ID */
#define REG_FAB_SRC(ch)        FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_SRC)
#define FAB_DST                0x02U
#define REG_FAB_DST(ch)        FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_DST)
#define FAB_SRC_ADDR           0x04U          /* 32 bit（下位語 $06 だけを使う） */
#define FAB_SRC_ADDR_L         0x06U
#define REG_FAB_SRC_ADDR(ch)   FC85_IO32(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_SRC_ADDR)
#define REG_FAB_SRC_ADDR_L(ch) FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_SRC_ADDR_L)
#define FAB_DST_ADDR           0x08U
#define FAB_DST_ADDR_L         0x0AU
#define REG_FAB_DST_ADDR(ch)   FC85_IO32(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_DST_ADDR)
#define REG_FAB_DST_ADDR_L(ch) FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_DST_ADDR_L)
#define FAB_COUNT              0x0CU
#define REG_FAB_COUNT(ch)      FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_COUNT)
#define FAB_SRC_STEP           0x0EU
#define REG_FAB_SRC_STEP(ch)   FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_SRC_STEP)
#define FAB_DST_STEP           0x10U
#define REG_FAB_DST_STEP(ch)   FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_DST_STEP)
#define FAB_FORMAT             0x12U
#define REG_FAB_FORMAT(ch)     FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_FORMAT)
#define FAB_MASK               0x14U
#define REG_FAB_MASK(ch)       FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_MASK)
#define FAB_XOR                0x16U
#define REG_FAB_XOR(ch)        FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_XOR)
#define FAB_SHIFT              0x18U
#define REG_FAB_SHIFT(ch)      FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_SHIFT)
#define FAB_TRIGGER            0x1AU
#define REG_FAB_TRIGGER(ch)    FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_TRIGGER)
#define FAB_CTRL               0x1CU
#define REG_FAB_CTRL(ch)       FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_CTRL)
#define FAB_STATUS             0x1EU
#define REG_FAB_STATUS(ch)     FC85_IO16(FABRIC_CH0 + FABRIC_CH_SIZE * (ch) + FAB_STATUS)
#define FABRIC_PORT_RESET      0x250100UL     /* bit n：port n の FIFO・ポインタを初期化（書込み専用） */
#define REG_FABRIC_PORT_RESET  FC85_IO16(FABRIC_PORT_RESET)
#define FABRIC_AUDIO_N         0x250102UL     /* 音声クロック＝8 MHz÷N（12 bit） */
#define REG_FABRIC_AUDIO_N     FC85_IO16(FABRIC_AUDIO_N)
#define FABRIC_MODULES         0x250104UL     /* 読出し専用 */
#define REG_FABRIC_MODULES     FC85_IO16(FABRIC_MODULES)

/* MODULES のビット */
#define FABRIC_MOD_DOC         0x0004U        /* bit 2：DOC 子基板 */
#define FABRIC_MOD_DOC_B       2

/* ポートの ID */
#define PORT_STAGING           0
#define PORT_ACRTC             1
#define PORT_DOC               2              /* 子基板がないときは本体 DAC の FIFO（語、上位 L・下位 R） */
#define PORT_DSP_IN            3
#define PORT_DSP_OUT           4
#define PORT_ADPCM             5
#define PORT_TEMPORAL_W        6
#define PORT_TEMPORAL_R        7
#define PORT_CAPTURE           8
#define PORT_CHAR_ROM          9
#define PORT_AUDIO_IN          10

/* FORMAT */
#define FMT_8_8                0
#define FMT_16_16              1
#define FMT_8_16               2              /* ゼロ拡張 */
#define FMT_16_8_LO            3
#define FMT_16_8_HI            4
#define FMT_16_ROT             5              /* SHIFT で回転 */

/* TRIGGER の発生源（bit 2–0）（DR-018） */
#define TRG_NOW                0
#define TRG_VBLANK             1
#define TRG_HBLANK             2
#define TRG_AUDIO              3
#define TRG_EXTRIG             4
#define TRG_ADPCM_VCK          5

/* TRIGGER のビット */
#define TRG_EACH               0x0008U        /* 1 回のトリガで 1 要素 */
#define TRG_LOOP               0x0010U
#define TRG_IRQ_EN             0x0020U        /* DONE か ERROR で割込み（レベル 3） */

/* CTRL */
#define FAB_START              0x0001U
#define FAB_STOP               0x0002U

/* STATUS */
#define FAB_ST_BUSY            0x0001U
#define FAB_ST_DONE            0x0002U        /* W1C */
#define FAB_ST_STALL           0x0004U
#define FAB_ST_ERROR           0x0008U        /* W1C */
#define FAB_ST_MISS            0x0010U        /* W1C */
#define FAB_ST_WAIT            0x0020U
#define FAB_ST_W1C             0x001AU

/* ------------------------------------------------------------ 入力と ACIA（§09、DR-028、DR-053、DR-055）[P] */

#define INPUT                  0x260000UL
#define PORT_A                 0x260000UL     /* コントローラ A（active-high） */
#define REG_PORT_A             FC85_IO16(PORT_A)
#define PORT_B                 0x260002UL     /* コントローラ B（同じ割当て） */
#define REG_PORT_B             FC85_IO16(PORT_B)
#define AXIS0                  0x260004UL     /* AXIS0〜7：$260004〜$260012（下位 8 bit にアナログ値） */
#define REG_AXIS0(n)           FC85_IO16(AXIS0 + 2UL * (n))
#define MIDI_CTRL              0x260014UL     /* ACIA の制御（書込み）／状態（読出し）。上位レーン、MOVE.B */
#define REG_MIDI_CTRL          FC85_IO8(MIDI_CTRL)
#define MIDI_STATUS            0x260014UL     /* ACIA の状態 */
#define REG_MIDI_STATUS        FC85_IO8(MIDI_STATUS)
#define MIDI_DATA              0x260016UL     /* ACIA の送受信データ。上位レーン、MOVE.B */
#define REG_MIDI_DATA          FC85_IO8(MIDI_DATA)
#define INPUT_IRQ_STATUS       0x260018UL     /* bit 0、1、3 は W1C。bit 2 は ACIA の IRQ の写し、bit 4 はカートリッジ（読出し専用） */
#define REG_INPUT_IRQ_STATUS   FC85_IO16(INPUT_IRQ_STATUS)
#define INPUT_IRQ_ENABLE       0x26001AUL
#define REG_INPUT_IRQ_ENABLE   FC85_IO16(INPUT_IRQ_ENABLE)

/* INPUT_IRQ_STATUS・INPUT_IRQ_ENABLE のビット */
#define INPUT_IRQ_PORT_A       0x0001U
#define INPUT_IRQ_PORT_B       0x0002U
#define INPUT_IRQ_MIDI         0x0004U
#define INPUT_IRQ_EXTRIG       0x0008U
#define INPUT_IRQ_CART         0x0010U        /* INPUT_IRQ_STATUS だけ：カートリッジがある（端子 /CART。読出し専用、割込みにはならない）[P]（DR-095） */
#define INPUT_IRQ_CART_B       4

/* PORT_A・PORT_B のビット（DR-055）。bit 15–8 は予約（0）[P] */
#define PAD_UP                 0x0001U
#define PAD_DOWN               0x0002U
#define PAD_LEFT               0x0004U
#define PAD_RIGHT              0x0008U
#define PAD_A                  0x0010U
#define PAD_B                  0x0020U
#define PAD_C                  0x0040U
#define PAD_START              0x0080U

/* PORT_A・PORT_B のビット番号（BTST 用）[P] */
#define PAD_UP_B               0
#define PAD_DOWN_B             1
#define PAD_LEFT_B             2
#define PAD_RIGHT_B            3
#define PAD_A_B                4
#define PAD_B_B                5
#define PAD_C_B                6
#define PAD_START_B            7

/* HD6850 ACIA の制御（[23]）[D] */
#define ACIA_RESET             0x03U          /* マスタリセット */
#define ACIA_DIV16             0x01U          /* 500 kHz÷16＝31.25 kbit/s */
#define ACIA_8N1               0x14U
#define ACIA_RIE               0x80U          /* 受信の割込み */
#define ACIA_MIDI              (ACIA_RIE|ACIA_8N1|ACIA_DIV16) /* $95 */

/* HD6850 ACIA の状態（[23]）[D] */
#define ACIA_ST_RDRF           0x01U
#define ACIA_ST_TDRE           0x02U
#define ACIA_ST_OVRN           0x20U
#define ACIA_ST_IRQ            0x80U

/* ------------------------------------------------------------ パレットと映像（§05、DR-027、DR-053）[P] */

#define PALETTE                0x270000UL     /* $0RGB×16。書込み専用、語アクセス。次のラインから効く */
#define REG_PALETTE(n)         FC85_IO16(PALETTE + 2UL * (n))
#define PAL_CTRL               0x270020UL     /* bit 0 DIRECT（Bend） */
#define REG_PAL_CTRL           FC85_IO16(PAL_CTRL)
#define VIDEO_IRQ              0x270022UL     /* 割込みの許可と保留（W1C）、今のレベル。下の VIRQ_* */
#define REG_VIDEO_IRQ          FC85_IO16(VIDEO_IRQ)
#define VIDEO_LINE             0x270024UL     /* 今のラスタ */
#define REG_VIDEO_LINE         FC85_IO16(VIDEO_LINE)
#define VIDEO_LINE_CMP         0x270026UL     /* 12 bit */
#define REG_VIDEO_LINE_CMP     FC85_IO16(VIDEO_LINE_CMP)

/* VIDEO_IRQ のビット */
#define VIRQ_VBL_EN            0x0001U
#define VIRQ_HBL_EN            0x0002U
#define VIRQ_LINE_CMP          0x0004U        /* HBlank の割込みを LINE_CMP の 1 ラインだけにする */
#define VIRQ_VBL               0x0100U        /* 保留（W1C） */
#define VIRQ_HBL               0x0200U        /* 保留（W1C） */
#define VIRQ_HBLANK            0x4000U        /* 今のレベル */
#define VIRQ_VBLANK            0x8000U

/* 画面の大きさ */
#define SCREEN_W               256
#define SCREEN_H               192

/* ------------------------------------------------------------ 割込み（§05、DR-028）[P] */
/* すべてオートベクタ。レベル L のベクタは 24＋L。 */

/* レベル */
#define IRQ_VIDEO              6              /* VBlank・HBlank、ACRTC */
#define IRQ_ACIA               5
#define IRQ_DOC                4
#define IRQ_FABRIC             3              /* Data Fabric、ADPCM bridge */
#define IRQ_DSP                2
#define IRQ_INPUT              1

/* 68000 の例外のベクタの番号（[31]）[D] */
#define VEC_BUS_ERROR          2
#define VEC_ADDR_ERROR         3
#define VEC_ILLEGAL            4
#define VEC_ZERO_DIV           5
#define VEC_CHK                6
#define VEC_TRAPV              7
#define VEC_PRIVILEGE          8
#define VEC_TRACE              9
#define VEC_LINE_A             10
#define VEC_LINE_F             11
#define VEC_SPURIOUS           24
#define VEC_AUTO1              25             /* レベル 1〜7：25〜31 */
#define VEC_TRAP0              32             /* TRAP #0〜15：32〜47 */

#endif /* FC85_H */
