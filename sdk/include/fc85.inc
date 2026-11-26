; fc85.inc：本体の番地とレジスタ（68000 から見た MMIO）。m68kasm 用。
;
; 仕様書（docs/spec/hardware.md）§05 のアドレスマップ、§06 の Data Fabric、§09 の入力と、DR-016・
; DR-019・DR-025・DR-027・DR-028・DR-029・DR-046・DR-053・DR-055 から手で書き起こした。仕様書が正で、
; ここは写し。食い違えば仕様書に合わせて直す。記述ステータスは仕様書の該当箇所に従う（ほとんど [P]）。
;
; アクセスの規約（仕様書 §05、DR-016・DR-030・DR-033・DR-052）：
; - 16 bit のレジスタは語アクセスだけ。バイトのアクセスはバスエラー。
; - 8 bit の IC（DOC、ACIA、文字 ROM）は上位バイトレーン（偶数番地）。MOVE.B で偶数番地を使う。
; - 未割当の番地はバスエラー（64 クロックの時間切れ）。
; - ACRTC の AR/SR とデータポート、DOC の窓には、CLR・TAS・BSET などの読み書き一体の命令を使わない。
; - ACRTC の FIFO には、SR の WFR（書込み）・RFR（読出し）を確かめてからアクセスする。
; - DOC の 0xE0・0xE2 は読むと状態が変わる。BIOS の決まった処理だけが読む。
; 書式：番地は絶対番地（.L で参照する）。

; ------------------------------------------------------------------ メモリ（§05）

BOOT_ROM        EQU     $000000         ; 64 KiB
CHAR_ROM        EQU     $010000         ; 文字 ROM。ROM の offset n は CHAR_ROM+2n（上位レーン）
MAIN_RAM        EQU     $100000         ; 128 KiB
MAIN_RAM_END    EQU     $120000
JUMP_TABLE      EQU     $100000         ; 例外のジャンプ表。ベクタ n は JUMP_TABLE+6n（JMP abs.L）（DR-025）
JUMP_TABLE_END  EQU     $100600
STAGING         EQU     $120000         ; 64 KiB の AV staging RAM（Fabric の port 0）
STAGING_END     EQU     $130000
CART            EQU     $400000         ; カートリッジの窓（最大 1 MiB）。なければバスエラー
CART_END        EQU     $500000

; ------------------------------------------------------------------ ACRTC HD63484（§05、DR-016、DR-033）

ACRTC_AR        EQU     $200000         ; 書込み：AR（レジスタの番号）
ACRTC_SR        EQU     $200000         ; 読出し：SR（上位バイトは 1）
ACRTC_DATA      EQU     $200002         ; AR が指すレジスタ。r00 は FIFO

; SR のビット
ACRTC_SR_CER    EQU     $80             ; コマンドの誤り
ACRTC_SR_ARD    EQU     $40             ; 領域の検出
ACRTC_SR_CED    EQU     $20             ; コマンドの終わり
ACRTC_SR_LPD    EQU     $10             ; ライトペン
ACRTC_SR_RFF    EQU     $08             ; 読出し FIFO が満杯
ACRTC_SR_RFR    EQU     $04             ; 読出し FIFO に語がある
ACRTC_SR_WFR    EQU     $02             ; 書込み FIFO に空きがある
ACRTC_SR_WFE    EQU     $01             ; 書込み FIFO が空
ACRTC_SR_CED_B  EQU     5               ; BTST 用のビット番号
ACRTC_SR_RFR_B  EQU     2
ACRTC_SR_WFR_B  EQU     1
ACRTC_SR_WFE_B  EQU     0

; レジスタの番号（AR に書く値）。r80 以降は語ごとに AR が 2 進む
ACRTC_FIFO      EQU     $00
ACRTC_CCR       EQU     $02             ; コマンドの制御
ACRTC_OMR       EQU     $04             ; 動作モード
ACRTC_DCR       EQU     $06             ; 表示の制御
ACRTC_RCR       EQU     $80             ; ラスタの数（読出し）
ACRTC_HSR       EQU     $82             ; 水平同期
ACRTC_HDR       EQU     $84             ; 水平表示
ACRTC_VSR       EQU     $86             ; 垂直同期
ACRTC_VDR       EQU     $88             ; 垂直表示
ACRTC_SSW       EQU     $8A             ; 分割画面の幅 1
ACRTC_SSW0      EQU     $8C             ; 上画面のラスタの数（U85 p.91）
ACRTC_SSW2      EQU     $8E             ; 下画面のラスタの数
ACRTC_BCR       EQU     $90             ; 属性の点滅：BON1・BOFF1・BON2・BOFF2（4 bit ずつ、U85 p.92–93）
ACRTC_HWR       EQU     $92             ; ウィンドウの横：上位 HWS−1（HSYNC の終わりから）、下位 HWW−1
ACRTC_VWS       EQU     $94             ; ウィンドウの縦の始まり−1（VSYNC の終わりから、U85 p.89–90）
ACRTC_VWW       EQU     $96             ; ウィンドウのラスタの数
ACRTC_GCRX      EQU     $98             ; グラフィックカーソル：上位 CXE、下位 CXS（HSYNC の始まりからのメモリサイクル）
ACRTC_GCRYS     EQU     $9A             ; CYS（VSYNC の終わりからのラスタ、U85 p.94）
ACRTC_GCRYE     EQU     $9C             ; CYE
ACRTC_MWR0      EQU     $C2             ; 上画面の記憶の幅
ACRTC_SAR0H     EQU     $C4
ACRTC_SAR0L     EQU     $C6
ACRTC_MWR1      EQU     $CA             ; 基本画面の記憶の幅
ACRTC_SAR1H     EQU     $CC             ; 基本画面の開始番地（bit 11–8 は SDA：横のスムーズスクロール）
ACRTC_SAR1L     EQU     $CE
ACRTC_MWR2      EQU     $D2             ; 下画面
ACRTC_SAR2H     EQU     $D4
ACRTC_SAR2L     EQU     $D6
ACRTC_MWR3      EQU     $DA             ; ウィンドウ
ACRTC_SAR3H     EQU     $DC
ACRTC_SAR3L     EQU     $DE
ACRTC_CDR       EQU     $E8             ; カーソルの定義（U85 p.107–109）
ACRTC_ZFR       EQU     $EA             ; ズーム：bit 15–12 HZF、bit 11–8 VZF（倍率−1、基本画面だけ、U85 p.110）

; CCR のビット
ACRTC_CCR_ABT   EQU     $8000           ; 打切り（リセットの後は 1）
ACRTC_CCR_PSE   EQU     $4000
ACRTC_CCR_DDM   EQU     $2000           ; データの DMA（DWT・DRD・DMOD を Fabric の port 1 で運ぶ）
ACRTC_CCR_CDM   EQU     $1000
ACRTC_CCR_DRC   EQU     $0800           ; DMA の要求：1 はサイクルスチール
ACRTC_CCR_GBM4  EQU     $0200           ; 4 bit/画素
; OMR のビット
ACRTC_OMR_MS    EQU     $8000           ; マスタ
ACRTC_OMR_STR   EQU     $4000           ; 表示と描画を始める
ACRTC_OMR_ACM_I EQU     $0008           ; インタリーブ
ACRTC_OMR_ACP   EQU     $2000           ; 単一アクセスで描画を優先
ACRTC_OMR_CSK1  EQU     $0400           ; カーソルのスキュー（×1〜3 メモリサイクル。十字では 0 にしない）
ACRTC_OMR_DSK1  EQU     $0100           ; DISP のスキュー（×1〜3 メモリサイクル）
ACRTC_OMR_RAM   EQU     $0080           ; 1：HSYNC の間に DRAM をリフレッシュしない
; DCR のビット（U85 p.75–78）。SE は 0x 使わない、10 ブランク、11 表示（基本画面の SE1 は 1 bit）
ACRTC_DCR_DSP   EQU     $8000
ACRTC_DCR_BASE  EQU     $4000           ; 基本画面を表示（BIOS が書く値。DR-071）
ACRTC_DCR_UPPER EQU     $3000           ; 上画面を表示（ブランクは $2000）
ACRTC_DCR_LOWER EQU     $0C00           ; 下画面を表示（ブランクは $0800）
ACRTC_DCR_WIN   EQU     $0300           ; ウィンドウを表示（ブランクは $0200）
; CDR のビット（U85 p.107–109）。CON・COFF は 4 フィールド単位
ACRTC_CDR_GRAPHIC EQU   $8000           ; GCR のカーソルを CUD1 に
ACRTC_CDR_CROSS EQU     $C000           ; 十字（横が CUD1、縦が CUD2）
ACRTC_CDR_CON1  EQU     $0800           ; ×1〜7：ON は 4×(n＋1) フィールド。0 は出さない
ACRTC_CDR_COFF1 EQU     $0100           ; ×1〜7：OFF は 4×(n＋1) フィールド。0 は常に ON

; 案B の表示（256×192、4 bpp、262 ライン、台帳 HD63484.md §3.4）[P]
ACRTC_B_HSR     EQU     $AA0D
ACRTC_B_HDR     EQU     $127F
ACRTC_B_VSR     EQU     $0106
ACRTC_B_VDR     EQU     $2703
ACRTC_B_SSW     EQU     $00C0
ACRTC_B_MW      EQU     $0040           ; 1 行 64 語（256 画素）
ACRTC_B_OMR     EQU     $C008           ; M/S＝1、STR＝1、ACM＝10（DSK・CSK は 0）

; コマンド（U85 p.173）。下位ビット：OPM（2–0）、COL（4–3）、AREA（7–5）
ACRTC_ORG       EQU     $0400           ; DPH, DPL
ACRTC_WPR       EQU     $0800           ; | rn、data
ACRTC_RPR       EQU     $0C00           ; | rn
ACRTC_WPTN      EQU     $1800           ; | pra、n、data×n
ACRTC_RPTN      EQU     $1C00           ; | pra、n
ACRTC_DRD       EQU     $2400           ; AX, AY
ACRTC_DWT       EQU     $2800           ; AX, AY、data×(|AX|+1)(|AY|+1)
ACRTC_DMOD      EQU     $2C00           ; | mm、AX, AY
ACRTC_RD        EQU     $4400
ACRTC_WT        EQU     $4800           ; data
ACRTC_MOD       EQU     $4C00           ; | mm、data
ACRTC_CLR       EQU     $5800           ; data, AX, AY（RWP から）
ACRTC_SCLR      EQU     $5C00           ; | mm、data, AX, AY
ACRTC_CPY       EQU     $6000           ; | S | DSD、Sah, Sal, AX, AY
ACRTC_SCPY      EQU     $7000
ACRTC_AMOVE     EQU     $8000           ; X, Y
ACRTC_RMOVE     EQU     $8400           ; dX, dY
ACRTC_ALINE     EQU     $8800           ; X, Y
ACRTC_RLINE     EQU     $8C00           ; dX, dY
ACRTC_ARCT      EQU     $9000           ; X, Y
ACRTC_RRCT      EQU     $9400           ; dX, dY
ACRTC_APLL      EQU     $9800           ; n, X1, Y1, ...
ACRTC_RPLL      EQU     $9C00
ACRTC_APLG      EQU     $A000
ACRTC_RPLG      EQU     $A400
ACRTC_CRCL      EQU     $A800           ; r（| ACRTC_CW で時計回り）
ACRTC_ELPS      EQU     $AC00           ; a, b, dX
ACRTC_AARC      EQU     $B000
ACRTC_RARC      EQU     $B400
ACRTC_AEARC     EQU     $B800
ACRTC_REARC     EQU     $BC00
ACRTC_AFRCT     EQU     $C000           ; X, Y（CP から X・Y までを塗る）
ACRTC_RFRCT     EQU     $C400           ; dX, dY
ACRTC_PAINT     EQU     $C800
ACRTC_DOT       EQU     $CC00
ACRTC_PTN       EQU     $D000           ; | S | SD、SZ
ACRTC_AGCPY     EQU     $E000
ACRTC_RGCPY     EQU     $F000
ACRTC_CW        EQU     $0100           ; 円・弧：時計回り
; 演算（OPM）
ACRTC_OPM_REP   EQU     0               ; 置き換え
ACRTC_OPM_OR    EQU     1
ACRTC_OPM_AND   EQU     2
ACRTC_OPM_EOR   EQU     3
; 色（COL）
ACRTC_COL_PAT   EQU     $00             ; パターンのビットで CL0・CL1
ACRTC_COL_CL1   EQU     $08             ; 1 は CL1、0 は描かない
ACRTC_COL_CL0   EQU     $10             ; 0 は CL0、1 は描かない
ACRTC_COL_DIR   EQU     $18             ; パターン RAM の語をそのまま
; パラメータレジスタ（WPR の rn）
ACRTC_P_CL0     EQU     $00
ACRTC_P_CL1     EQU     $01
ACRTC_P_CCMP    EQU     $02
ACRTC_P_EDG     EQU     $03
ACRTC_P_MASK    EQU     $04
ACRTC_P_PRC0    EQU     $05
ACRTC_P_PRC1    EQU     $06
ACRTC_P_PRC2    EQU     $07
ACRTC_P_ADR_XMIN EQU    $08
ACRTC_P_ADR_YMIN EQU    $09
ACRTC_P_ADR_XMAX EQU    $0A
ACRTC_P_ADR_YMAX EQU    $0B
ACRTC_P_RWPH    EQU     $0C             ; DN<<14 | 語番地の bit 19–12
ACRTC_P_RWPL    EQU     $0D             ; 語番地の bit 11–0 << 4

; ------------------------------------------------------------------ DOC ES5503（子基板、DR-019、DR-030）
; ネイティブのレジスタ n は DOC+2n の上位バイト。MOVE.B の 1 回の読みか書きだけ。子基板がなければバスエラー
; （FABRIC_MODULES の bit 2 で確かめる）。波形 RAM は Fabric の port 2 からだけ読み書きできる。

DOC             EQU     $210000
DOC_FREQ_LO     EQU     DOC+2*$00       ; +2×osc
DOC_FREQ_HI     EQU     DOC+2*$20
DOC_VOLUME      EQU     DOC+2*$40
DOC_DATA        EQU     DOC+2*$60       ; 最後に読んだ波形の値（読出し）
DOC_POINTER     EQU     DOC+2*$80       ; 波形テーブルの頁（番地の bit 15–8）
DOC_CONTROL     EQU     DOC+2*$A0
DOC_SIZE        EQU     DOC+2*$C0       ; bit 6 BS、bit 5–3 テーブルの大きさ T、bit 2–0 分解能 R
DOC_IRQ         EQU     DOC+2*$E0       ; 読むと割込みの要求を 1 つ取る（BIOS だけが読む）
DOC_OSC_ENABLE  EQU     DOC+2*$E1       ; (N−1)×2。書くと走査を始める
DOC_ADC         EQU     DOC+2*$E2       ; 読むと A/D の変換を始める（BIOS だけが読む）
DOC_OSC_STRIDE  EQU     2               ; オシレータ 1 つぶんの番地の差
; CONTROL のビット
DOC_CTL_HALT    EQU     $01
DOC_CTL_FREE    EQU     $00             ; モード（bit 2–1）
DOC_CTL_ONESHOT EQU     $02
DOC_CTL_SYNC    EQU     $04
DOC_CTL_SWAP    EQU     $06
DOC_CTL_IE      EQU     $08
DOC_CTL_CA_SH   EQU     4               ; CA3–0（bit 7–4）。偶数は L、奇数は R（DR-054）

; ------------------------------------------------------------------ DSP bridge（DR-029、DR-049、DR-053）

DSP_CTRL        EQU     $220000         ; bit 0 RESET、bit 2–1 BIOSEL、bit 3 IRQ_EN
DSP_STATUS      EQU     $220002         ; bit 0 PRGBUS、1 入力の語あり、2 出力に空き、3 COMMAND、4 RESPONSE、
                                        ; 5 UNDERRUN（W1C）、6 OVERRUN（W1C）
DSP_MAILBOX     EQU     $220004
DSP_RESPONSE    EQU     $220006
DSP_PROGRAM     EQU     $280000         ; 4K 語。DSP をリセットし PRGBUS＝1 の間だけ（語アクセス）
DSP_CTRL_RESET  EQU     $0001
DSP_CTRL_IRQ_EN EQU     $0008
; CTRL の BIOSEL（bit 2–1）：DSP の BIO に出す条件（DR-029 (3)、DR-049）
DSP_CTRL_BIO_INPUT EQU  $0000           ; 入力 FIFO が空でない（既定）
DSP_CTRL_BIO_SPACE EQU  $0002           ; 出力 FIFO に空き
DSP_CTRL_BIO_MAIL EQU   $0004           ; メールボックスに着信
DSP_CTRL_BIO_TICK EQU   $0006           ; 音声クロックの拍のラッチ
; STATUS のビット番号（BTST 用。MOVE.W で読んでから調べる。バイトのアクセスはバスエラー）
DSP_ST_PRGBUS_B EQU     0
DSP_ST_INPUT_B  EQU     1
DSP_ST_SPACE_B  EQU     2
DSP_ST_COMMAND_B EQU    3
DSP_ST_RESPONSE_B EQU   4
DSP_ST_UNDERRUN EQU     $0020           ; W1C
DSP_ST_OVERRUN  EQU     $0040           ; W1C
DSP_PROGRAM_WORDS EQU   4096

; ------------------------------------------------------------------ ADPCM bridge と本体 DAC（DR-017、DR-019、DR-053）

ADPCM_CTRL      EQU     $230000         ; bit 0 RESET、1 S1、2 S2、3 下位ニブルから、4 置換しない、5 半分空きの割込み
ADPCM_STATUS    EQU     $230002         ; bit 7–0 残量、bit 8 UNDERRUN（W1C）、bit 9 半分空き
DAC_CTRL        EQU     $230010         ; bit 0 DIRECT。DOC の子基板があるときはバスエラー
DAC_STATUS      EQU     $230012         ; bit 7–0 FIFO の残量、bit 8 UNDERRUN（W1C）
DAC_DATA        EQU     $230014         ; DIRECT のとき：上位バイト L、下位バイト R（符号なし、$80 が中央）
DAC_CTRL_DIRECT EQU     $0001
DAC_CENTER      EQU     $8080
; ADPCM の CTRL と STATUS のビット（DR-053）。S1・S2 は MSM5205 の標本化：S1＝1・S2＝0 で 8 kHz、
; S1＝0・S2＝1 で 6 kHz、両方 0 で 4 kHz（384 kHz の 1/48・1/64・1/96、台帳 MSM5205.md §3.1）。
; 両方 1 は Bend（V̄C̄K̄ が入力になる。エミュレータでは取込みが起きない [V]）。RESET は電源投入時 1
ADPCM_CTL_RESET EQU     $0001
ADPCM_CTL_S1    EQU     $0002
ADPCM_CTL_S2    EQU     $0004
ADPCM_CTL_LOW_FIRST EQU $0008           ; ニブルを下位から
ADPCM_CTL_NO_REPLACE EQU $0010          ; FIFO が空でも 0x08 に置き換えない（Bend）
ADPCM_CTL_HALF_IRQ EQU  $0020           ; 半分空きの割込み（レベル 3）
ADPCM_CTL_8K    EQU     ADPCM_CTL_S1
ADPCM_CTL_6K    EQU     ADPCM_CTL_S2
ADPCM_CTL_4K    EQU     $0000
ADPCM_ST_COUNT  EQU     $007F           ; FIFO の byte 数
ADPCM_ST_UNDERRUN EQU   $0100           ; W1C
ADPCM_ST_HALF   EQU     $0200           ; 残りが 32 byte 以下

; ------------------------------------------------------------------ Temporal（DR-053）

TEMPORAL_WPTR   EQU     $240000
TEMPORAL_RPTR   EQU     $240002
TEMPORAL_CTRL   EQU     $240004         ; 下の TMP_*（DR-067）
TEMPORAL_DIST   EQU     $240006
TMP_CLEAR       EQU     $0001           ; 両方のポインタと印を 0 に
TMP_SYNC        EQU     $0002           ; VBlank の始まりで両方のポインタを 0 に
TMP_RSYNC       EQU     $0004           ; VBlank の始まりで読出しのポインタだけを 0 に（繰返し）
TMP_HOLD        EQU     $0008           ; 書込みの /WE を止める。ポインタは進む（保持）
TMP_EQUAL       EQU     $0010           ; 読出し専用：WPTR＝RPTR
TMP_OVERRUN     EQU     $0100           ; 書く側が追いついた。W1C
TMP_UNDERRUN    EQU     $0200           ; 読む側が追いついた。W1C

; ------------------------------------------------------------------ Data Fabric（§06、DR-046）

FABRIC          EQU     $250000
FABRIC_CH0      EQU     $250000
FABRIC_CH1      EQU     $250040
FABRIC_CH_SIZE  EQU     $40
; チャネルの中の offset
FAB_SRC         EQU     $00             ; ポートの ID
FAB_DST         EQU     $02
FAB_SRC_ADDR    EQU     $04             ; 32 bit（下位語 $06 だけを使う）
FAB_SRC_ADDR_L  EQU     $06
FAB_DST_ADDR    EQU     $08
FAB_DST_ADDR_L  EQU     $0A
FAB_COUNT       EQU     $0C
FAB_SRC_STEP    EQU     $0E
FAB_DST_STEP    EQU     $10
FAB_FORMAT      EQU     $12
FAB_MASK        EQU     $14
FAB_XOR         EQU     $16
FAB_SHIFT       EQU     $18
FAB_TRIGGER     EQU     $1A
FAB_CTRL        EQU     $1C
FAB_STATUS      EQU     $1E
FABRIC_PORT_RESET EQU   $250100         ; bit n：port n の FIFO・ポインタを初期化（書込み専用）
FABRIC_AUDIO_N  EQU     $250102         ; 音声クロック＝8 MHz÷N（12 bit）
FABRIC_MODULES  EQU     $250104         ; 読出し専用
FABRIC_MOD_DOC  EQU     $0004           ; bit 2：DOC 子基板
FABRIC_MOD_DOC_B EQU    2
; ポートの ID
PORT_STAGING    EQU     0
PORT_ACRTC      EQU     1
PORT_DOC        EQU     2               ; 子基板がないときは本体 DAC の FIFO（語、上位 L・下位 R）
PORT_DSP_IN     EQU     3
PORT_DSP_OUT    EQU     4
PORT_ADPCM      EQU     5
PORT_TEMPORAL_W EQU     6
PORT_TEMPORAL_R EQU     7
PORT_CAPTURE    EQU     8
PORT_CHAR_ROM   EQU     9
PORT_AUDIO_IN   EQU     10
; FORMAT
FMT_8_8         EQU     0
FMT_16_16       EQU     1
FMT_8_16        EQU     2               ; ゼロ拡張
FMT_16_8_LO     EQU     3
FMT_16_8_HI     EQU     4
FMT_16_ROT      EQU     5               ; SHIFT で回転
; TRIGGER
TRG_NOW         EQU     0
TRG_VBLANK      EQU     1
TRG_HBLANK      EQU     2
TRG_AUDIO       EQU     3
TRG_EXTRIG      EQU     4
TRG_ADPCM_VCK   EQU     5
TRG_EACH        EQU     $0008           ; 1 回のトリガで 1 要素
TRG_LOOP        EQU     $0010
TRG_IRQ_EN      EQU     $0020           ; DONE か ERROR で割込み（レベル 3）
; CTRL
FAB_START       EQU     $0001
FAB_STOP        EQU     $0002
; STATUS
FAB_ST_BUSY     EQU     $0001
FAB_ST_DONE     EQU     $0002           ; W1C
FAB_ST_STALL    EQU     $0004
FAB_ST_ERROR    EQU     $0008           ; W1C
FAB_ST_MISS     EQU     $0010           ; W1C
FAB_ST_WAIT     EQU     $0020
FAB_ST_W1C      EQU     $001A

; ------------------------------------------------------------------ 入力と ACIA（§09、DR-028、DR-053、DR-055）

INPUT           EQU     $260000
PORT_A          EQU     $260000         ; コントローラ A（active-high）
PORT_B          EQU     $260002         ; コントローラ B（同じ割当て）
AXIS0           EQU     $260004         ; AXIS0〜7：$260004〜$260012（下位 8 bit にアナログ値）
MIDI_CTRL       EQU     $260014         ; ACIA の制御（書込み）／状態（読出し）。上位レーン、MOVE.B
MIDI_STATUS     EQU     $260014
MIDI_DATA       EQU     $260016         ; ACIA の送受信データ。上位レーン、MOVE.B
INPUT_IRQ_STATUS EQU    $260018         ; bit 0、1、3 は W1C。bit 2 は ACIA の IRQ の写し（読出し専用）
INPUT_IRQ_ENABLE EQU    $26001A
INPUT_IRQ_PORT_A EQU    $0001
INPUT_IRQ_PORT_B EQU    $0002
INPUT_IRQ_MIDI  EQU     $0004
INPUT_IRQ_EXTRIG EQU    $0008

; PORT_A・PORT_B のビット（DR-055 [P]）。bit 15–8 は予約（0）
PAD_UP          EQU     $0001
PAD_DOWN        EQU     $0002
PAD_LEFT        EQU     $0004
PAD_RIGHT       EQU     $0008
PAD_A           EQU     $0010
PAD_B           EQU     $0020
PAD_C           EQU     $0040
PAD_START       EQU     $0080
PAD_UP_B        EQU     0               ; BTST 用のビット番号
PAD_DOWN_B      EQU     1
PAD_LEFT_B      EQU     2
PAD_RIGHT_B     EQU     3
PAD_A_B         EQU     4
PAD_B_B         EQU     5
PAD_C_B         EQU     6
PAD_START_B     EQU     7

; HD6850 ACIA
ACIA_RESET      EQU     $03             ; マスタリセット
ACIA_DIV16      EQU     $01             ; 500 kHz÷16＝31.25 kbit/s
ACIA_8N1        EQU     $14
ACIA_RIE        EQU     $80             ; 受信の割込み
ACIA_MIDI       EQU     ACIA_RIE|ACIA_8N1|ACIA_DIV16    ; $95
ACIA_ST_RDRF    EQU     $01
ACIA_ST_TDRE    EQU     $02
ACIA_ST_OVRN    EQU     $20
ACIA_ST_IRQ     EQU     $80

; ------------------------------------------------------------------ パレットと映像（DR-027、DR-053）

PALETTE         EQU     $270000         ; $0RGB×16。書込み専用、語アクセス。次のラインから効く
PAL_CTRL        EQU     $270020         ; bit 0 DIRECT（Bend）
VIDEO_IRQ       EQU     $270022
VIDEO_LINE      EQU     $270024         ; 今のラスタ
VIDEO_LINE_CMP  EQU     $270026
VIRQ_VBL_EN     EQU     $0001
VIRQ_HBL_EN     EQU     $0002
VIRQ_LINE_CMP   EQU     $0004           ; HBlank の割込みを LINE_CMP の 1 ラインだけにする
VIRQ_VBL        EQU     $0100           ; 保留（W1C）
VIRQ_HBL        EQU     $0200           ; 保留（W1C）
VIRQ_HBLANK     EQU     $4000           ; 今のレベル
VIRQ_VBLANK     EQU     $8000

SCREEN_W        EQU     256
SCREEN_H        EQU     192

; ------------------------------------------------------------------ 割込み（DR-028）
; すべてオートベクタ。レベル L のベクタは 24＋L。

IRQ_VIDEO       EQU     6               ; VBlank・HBlank、ACRTC
IRQ_ACIA        EQU     5
IRQ_DOC         EQU     4
IRQ_FABRIC      EQU     3               ; Data Fabric、ADPCM bridge
IRQ_DSP         EQU     2
IRQ_INPUT       EQU     1
VEC_BUS_ERROR   EQU     2
VEC_ADDR_ERROR  EQU     3
VEC_ILLEGAL     EQU     4
VEC_ZERO_DIV    EQU     5
VEC_CHK         EQU     6
VEC_TRAPV       EQU     7
VEC_PRIVILEGE   EQU     8
VEC_TRACE       EQU     9
VEC_LINE_A      EQU     10
VEC_LINE_F      EQU     11
VEC_SPURIOUS    EQU     24
VEC_AUTO1       EQU     25              ; レベル 1〜7：25〜31
VEC_TRAP0       EQU     32              ; TRAP #0〜15：32〜47
