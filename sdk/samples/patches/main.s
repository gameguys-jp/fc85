; SAMPLE 3：仕様書 §11 の実験パッチを、実際のデータの経路で動かすカートリッジ。m68kasm で組む（build.py）。
; 説明は README.md。左右でページを替える。
;
; 0. GRAPHICS AS WAVE  ACRTC の VRAM → DRD（port 1）→ DOC の波形 RAM（port 2）。動く図形の 2 行（256 byte）を
;                      毎フレーム波形にし、オシレータ 0・1 が鳴らす。MASK $FE・XOR $81 で 0 を避け（0 は DOC を止める）、
;                      色 0 を中央の値にする。
; 1. IMAGE AS ADPCM    VRAM → DRD → Staging RAM（port 0）→ ADPCM の FIFO（port 5）→ MSM5205。画素のニブルを
;                      ADPCM の符号として復号する。前の復号の状態で音が変わる。ニブルの順、速さ、毎フレームの RESET。
; 2. GLYPH AS WAVE     文字 ROM（port 9）→ DOC の波形 RAM。32 字（256 byte）を 1 周期にする。XOR $01 を外すと、
;                      字形の下の余白行（0）でオシレータが止まる（DOC の性質。仕様書 §08）。
; 3. TEMPORAL FEEDBACK VRAM → DRD → Temporal RAM（port 6）、Temporal RAM（port 7）→ DWT・DMOD → VRAM。
;                      d・2d・3d フレーム前の像を並べる。Temporal RAM は 8 bit なので、1 回の転送では語の上位 byte
;                      （右の 2 画素）だけが残る。2 PASS で下位 byte も運ぶ。FEEDBACK は遅れた像を今の像に EOR し、
;                      その結果をまた記録する。HOLD は Temporal RAM の書込みを止める（DR-067）。
;
; 番地とビットの名前は fc85.inc、BIOS の約束は bios.inc。

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"

; ------------------------------------------------------------------ 定数

NPAGES          EQU     4
CANVAS_Y        EQU     16              ; ページ 0・1 の絵（256×64）
CANVAS_H        EQU     64
CANVAS_Y2       EQU     CANVAS_Y+CANVAS_H-1
SCAN_MAX        EQU     CANVAS_H-2      ; 走査の行（2 行）の上端の最大
BOX             EQU     20
NBOX            EQU     3
WAVE_PAGE       EQU     $20             ; DOC の波形 RAM の頁（$2000〜$20FF）
DOC_VOL         EQU     160
ADPCM_BUF       EQU     $2000           ; Staging RAM の 256 byte（port 0 の番地）
GLYPH_Y         EQU     24              ; 字形の帯（256×8）
SCOPE_Y         EQU     40              ; 波形の図（256×64）
SCOPE_H         EQU     64
SCOPE_Y2        EQU     SCOPE_Y+SCOPE_H-1
T_Y             EQU     32              ; Temporal の矩形（64×48）
T_H             EQU     48
T_W             EQU     64
T_WORDS         EQU     T_W/4*T_H       ; 1 回の転送の語数（768）
BALL            EQU     8
DELAY_MAX       EQU     31
FAB_TIMEOUT     EQU     40000           ; STATUS を読む回数の上限（約 0.4 秒）

Y_TITLE         EQU     0
Y_LABEL         EQU     88
Y_P1            EQU     112
Y_P2            EQU     128
Y_ST            EQU     144
Y_ERR           EQU     152
Y_H1            EQU     168
Y_H2            EQU     176

COL_BG          EQU     0
COL_TITLE       EQU     5
COL_NAME        EQU     7
COL_LABEL       EQU     11
COL_TEXT        EQU     15
COL_BAD         EQU     3
COL_SCOPE       EQU     6
COL_LINE        EQU     15
COL_BAR         EQU     12

; ------------------------------------------------------------------ Main RAM（$100800〜、bios-abi.md §03）

        RSSET   APP_RAM
v_page          RS.W    1
v_frame         RS.W    1
v_has_doc       RS.W    1
v_color         RS.W    1               ; ACRTC の CL0・CL1（−1 は不明）
v_dirty         RS.W    1               ; 値を描き直す
v_scan          RS.W    1               ; 走査の行（絵の中の行）
v_pitch         RS.W    1               ; 0〜2
v_opt_a         RS.W    1
v_opt_b         RS.W    1
v_opt_c         RS.W    1
v_rate          RS.W    1               ; ADPCM の速さ（0〜2）
v_glyph         RS.W    1               ; 文字 ROM の 256 byte の区切り（0〜127）
v_delay         RS.W    1               ; Temporal の遅れ（フレーム）
v_tbase         RS.W    1               ; このフレームを記録する Temporal RAM の番地
v_errors        RS.W    1               ; Fabric の誤りと時間切れの数
v_boxes         RS.W    NBOX*4          ; x、y、dx、dy
v_ball          RS.W    4
v_old           RS.W    NBOX*2+1        ; 前のフレームに描いた箱の x、y と線の x
v_drawn         RS.W    1               ; v_old に描いたものがある
v_vars_end      RS.B    0
        ASSERT  v_vars_end<=$110000

; ------------------------------------------------------------------ 見出し（bios-abi.md §04）

        ORG     CART
cart_header:
        DC.L    CART_MAGIC
        DC.W    CART_HDR_VERSION
        DC.W    0
        DC.L    start
        DC.L    rom_end-CART
        DC.B    "SAMPLE 3        "      ; 題名（16 字、空白で埋める）
        ASSERT  *=CART+cart_header_size

; ------------------------------------------------------------------ 入口

start:
        LEA     INITIAL_SSP,SP
        LEA     APP_RAM,A0
        MOVE.W  #(v_vars_end-APP_RAM+3)/4-1,D0
.clr    CLR.L   (A0)+
        DBRA    D0,.clr
        LEA     ACRTC_AR,A6             ; A6 はいつも ACRTC（AR は r00 のまま使う）
        MOVE.W  #-1,v_color
        MOVE.W  #CANVAS_H/2,v_scan
        MOVE.W  #8,v_delay
        MOVE.W  #2,v_rate
        MOVE.W  #1,v_pitch
        MOVE.W  #$30*8/256,v_glyph      ; 数字と英字のあたり
        LEA     v_boxes,A0
        LEA     box_init(PC),A1
        MOVEQ   #NBOX*4+4-1,D0          ; 箱と球
.init   MOVE.W  (A1)+,(A0)+
        DBRA    D0,.init
        MOVE.W  bios_modules,D0
        BTST    #FABRIC_MOD_DOC_B,D0
        BEQ.S   .nodoc
        MOVE.W  #1,v_has_doc
.nodoc  BSR     page_enter

main_loop:
        JSR     BIOS_WAIT_VBL
        ADDQ.W  #1,v_frame
        BSR     input
        BSR     page_frame
        BSR     draw_params
        BSR     draw_errors
        BRA.S   main_loop

; ------------------------------------------------------------------ ページの切替え

; 音を止め、画面を消し、ページの字を描いて、ページの準備をする。
page_enter:
        BSR     silence
        MOVEQ   #COL_BG,D0
        JSR     BIOS_CLS
        MOVE.W  #0,v_opt_a
        MOVE.W  #0,v_opt_b
        MOVE.W  #0,v_opt_c
        MOVE.W  #1,v_dirty
        MOVE.W  #0,v_drawn
        BSR     draw_static
        MOVE.W  v_page,D2
        ADD.W   D2,D2
        MOVE.W  pg_enter(PC,D2.W),D2
        JMP     pg_enter(PC,D2.W)
pg_enter:
        DC.W    en_wave-pg_enter
        DC.W    en_adpcm-pg_enter
        DC.W    en_glyph-pg_enter
        DC.W    en_temporal-pg_enter

page_frame:
        MOVE.W  v_page,D2
        ADD.W   D2,D2
        MOVE.W  pg_frame(PC,D2.W),D2
        JMP     pg_frame(PC,D2.W)
pg_frame:
        DC.W    fr_wave-pg_frame
        DC.W    fr_adpcm-pg_frame
        DC.W    fr_glyph-pg_frame
        DC.W    fr_temporal-pg_frame

; 両方のチャネルを止め、ADPCM をリセットに置き、DOC のオシレータ 0・1 を止める。Temporal の HOLD を外す。
silence:
        LEA     FABRIC_CH0,A1
        BSR     fab_prep
        LEA     FABRIC_CH1,A1
        BSR     fab_prep
        MOVE.W  #ADPCM_CTL_RESET|ADPCM_CTL_8K,ADPCM_CTRL
        MOVE.W  #1<<PORT_ADPCM,FABRIC_PORT_RESET
        MOVE.W  #0,TEMPORAL_CTRL
        BSR     ddm_off
        TST.W   v_has_doc
        BEQ.S   .ret
        MOVE.B  #0,DOC_VOLUME
        MOVE.B  #0,DOC_VOLUME+DOC_OSC_STRIDE
        MOVE.B  #DOC_CTL_HALT,DOC_CONTROL
        MOVE.B  #(1<<DOC_CTL_CA_SH)|DOC_CTL_HALT,DOC_CONTROL+DOC_OSC_STRIDE
.ret    RTS

; ------------------------------------------------------------------ 入力

; 左右でページ、ほかはページごと（D0＝押している釦、D1＝新しく押した釦）。
input:
        JSR     BIOS_READ_PAD
        MOVE.W  v_page,D2
        BTST    #PAD_LEFT_B,D1
        BEQ.S   .nl
        SUBQ.W  #1,D2
        BPL.S   .go
        MOVEQ   #NPAGES-1,D2
        BRA.S   .go
.nl     BTST    #PAD_RIGHT_B,D1
        BEQ.S   .page
        ADDQ.W  #1,D2
        CMP.W   #NPAGES,D2
        BLO.S   .go
        MOVEQ   #0,D2
.go     MOVE.W  D2,v_page
        BRA     page_enter
.page   ADD.W   D2,D2
        MOVE.W  pg_input(PC,D2.W),D2
        JMP     pg_input(PC,D2.W)
pg_input:
        DC.W    in_wave-pg_input
        DC.W    in_adpcm-pg_input
        DC.W    in_glyph-pg_input
        DC.W    in_temporal-pg_input

; 上下を押している間、走査の行を 1 ずつ動かす。
scan_ud:
        BTST    #PAD_UP_B,D0
        BEQ.S   .nu
        TST.W   v_scan
        BEQ.S   .nu
        SUBQ.W  #1,v_scan
        MOVE.W  #1,v_dirty
.nu     BTST    #PAD_DOWN_B,D0
        BEQ.S   .ret
        CMP.W   #SCAN_MAX,v_scan
        BHS.S   .ret
        ADDQ.W  #1,v_scan
        MOVE.W  #1,v_dirty
.ret    RTS

; ------------------------------------------------------------------ 0. GRAPHICS AS WAVE

en_wave:
        TST.W   v_has_doc
        BEQ     no_doc
        BSR     anim_canvas
        BSR     wave_xfer               ; 鳴らす前に 256 byte を埋める
        BRA     doc_start

in_wave:
        BSR     scan_ud
        BTST    #PAD_A_B,D1
        BEQ.S   .na
        BSR     next_pitch
.na     BTST    #PAD_B_B,D1
        BEQ.S   .nb
        EORI.W  #1,v_opt_b
        MOVE.W  #1,v_dirty
.nb     BTST    #PAD_C_B,D1
        BEQ.S   .ret
        EORI.W  #1,v_opt_c
        MOVE.W  #1,v_dirty
.ret    RTS

fr_wave:
        BSR     anim_canvas
        TST.W   v_has_doc
        BEQ.S   .ret
        BSR     wave_xfer
        TST.W   v_opt_b
        BEQ.S   .ret
        BSR     doc_restart             ; GUARD OFF：0 で止まったオシレータを毎フレーム動かし直す
.ret    RTS

; 走査の 2 行（128 語）を DOC の波形 RAM の 256 byte に。語の上位 byte を偶数番地、下位 byte を奇数番地に、
; 2 回の DRD で運ぶ。
wave_xfer:
        BSR     ddm_on
        MOVE.W  #FMT_16_8_HI,D3
        MOVEQ   #0,D4
        BSR.S   .pass
        MOVE.W  #FMT_16_8_LO,D3
        MOVEQ   #1,D4
        BSR.S   .pass
        BRA     ddm_off
.pass   BSR     drd_scan
        LEA     FABRIC_CH0,A1
        MOVE.W  #PORT_ACRTC,FAB_SRC(A1)
        MOVE.W  #PORT_DOC,FAB_DST(A1)
        MOVE.W  #0,FAB_SRC_ADDR_L(A1)
        MOVE.W  #WAVE_PAGE<<8,D0
        ADD.W   D4,D0
        MOVE.W  D0,FAB_DST_ADDR_L(A1)
        MOVE.W  #128,FAB_COUNT(A1)
        MOVE.W  #0,FAB_SRC_STEP(A1)
        MOVE.W  #2,FAB_DST_STEP(A1)
        MOVE.W  D3,FAB_FORMAT(A1)
        MOVE.W  #$FEFE,D0               ; GUARD：AND $FE、XOR $81。どの byte も 0 にならず、色 0 の 2 画素は
        MOVE.W  #$8181,D1               ; 中央の $81 になる
        TST.W   v_opt_b
        BEQ.S   .guard
        MOVE.W  #$FFFF,D0
        MOVEQ   #0,D1
.guard  MOVE.W  D0,FAB_MASK(A1)
        MOVE.W  D1,FAB_XOR(A1)
        MOVE.W  #0,FAB_SHIFT(A1)
        MOVEQ   #TRG_NOW,D0
        BRA     fab_go_wait

; ------------------------------------------------------------------ 1. IMAGE AS ADPCM

en_adpcm:
        BSR     anim_canvas
        BSR     adpcm_xfer
        LEA     FABRIC_CH1,A1
        BSR     fab_prep
        MOVE.W  #1<<PORT_ADPCM,FABRIC_PORT_RESET
        MOVE.W  #PORT_STAGING,FAB_SRC(A1)
        MOVE.W  #PORT_ADPCM,FAB_DST(A1)
        MOVE.W  #ADPCM_BUF,FAB_SRC_ADDR_L(A1)
        MOVE.W  #0,FAB_DST_ADDR_L(A1)
        MOVE.W  #256,FAB_COUNT(A1)
        MOVE.W  #1,FAB_SRC_STEP(A1)
        MOVE.W  #0,FAB_DST_STEP(A1)
        MOVE.W  #FMT_8_8,FAB_FORMAT(A1)
        MOVE.W  #$FFFF,FAB_MASK(A1)
        MOVE.W  #0,FAB_XOR(A1)
        MOVE.W  #0,FAB_SHIFT(A1)
        MOVE.W  #TRG_NOW|TRG_LOOP,FAB_TRIGGER(A1)       ; FIFO の空きを待ちながら 256 byte を回る
        MOVE.W  #FAB_START,FAB_CTRL(A1)
        BRA     adpcm_ctrl

in_adpcm:
        BSR     scan_ud
        BTST    #PAD_A_B,D1
        BEQ.S   .na
        EORI.W  #1,v_opt_a
        MOVE.W  #1,v_dirty
        BSR     adpcm_ctrl
.na     BTST    #PAD_B_B,D1
        BEQ.S   .nb
        MOVE.W  v_rate,D0
        ADDQ.W  #1,D0
        CMP.W   #3,D0
        BLO.S   .r
        MOVEQ   #0,D0
.r      MOVE.W  D0,v_rate
        MOVE.W  #1,v_dirty
        BSR     adpcm_ctrl
.nb     BTST    #PAD_C_B,D1
        BEQ.S   .ret
        EORI.W  #1,v_opt_c
        MOVE.W  #1,v_dirty
.ret    RTS

fr_adpcm:
        TST.W   v_opt_c
        BEQ.S   .run
        BSR     adpcm_ctrl              ; RESET：転送の間（約 0.5 ms）押さえる
        ORI.W   #ADPCM_CTL_RESET,D0
        MOVE.W  D0,ADPCM_CTRL
.run    BSR     anim_canvas
        BSR     adpcm_xfer
        BRA     adpcm_ctrl

; 走査の 2 行（128 語）を Staging RAM の 256 byte に（語は big-endian のまま）。
adpcm_xfer:
        BSR     ddm_on
        BSR     drd_scan
        LEA     FABRIC_CH0,A1
        MOVE.W  #PORT_ACRTC,FAB_SRC(A1)
        MOVE.W  #PORT_STAGING,FAB_DST(A1)
        MOVE.W  #0,FAB_SRC_ADDR_L(A1)
        MOVE.W  #ADPCM_BUF,FAB_DST_ADDR_L(A1)
        MOVE.W  #128,FAB_COUNT(A1)
        MOVE.W  #0,FAB_SRC_STEP(A1)
        MOVE.W  #2,FAB_DST_STEP(A1)
        MOVE.W  #FMT_16_16,FAB_FORMAT(A1)
        MOVE.W  #$FFFF,FAB_MASK(A1)
        MOVE.W  #0,FAB_XOR(A1)
        MOVE.W  #0,FAB_SHIFT(A1)
        MOVEQ   #TRG_NOW,D0
        BSR     fab_go_wait
        BRA     ddm_off

; CTRL を今の速さとニブルの順にする（RESET を外す）。出力：D0＝書いた値。
adpcm_ctrl:
        MOVE.W  v_rate,D0
        ADD.W   D0,D0
        MOVE.W  rate_bits(PC,D0.W),D0
        TST.W   v_opt_a
        BEQ.S   .out
        ORI.W   #ADPCM_CTL_LOW_FIRST,D0
.out    MOVE.W  D0,ADPCM_CTRL
        RTS
rate_bits:
        DC.W    ADPCM_CTL_4K,ADPCM_CTL_6K,ADPCM_CTL_8K

; ------------------------------------------------------------------ 2. GLYPH AS WAVE

en_glyph:
        TST.W   v_has_doc
        BEQ     no_doc
        BSR     glyph_draw
        BSR     glyph_xfer
        BRA     doc_start

in_glyph:
        BTST    #PAD_UP_B,D1
        BEQ.S   .nu
        SUBQ.W  #1,v_glyph
        BRA.S   .wrap
.nu     BTST    #PAD_DOWN_B,D1
        BEQ.S   .na
        ADDQ.W  #1,v_glyph
.wrap   ANDI.W  #$7F,v_glyph
        BRA.S   .changed
.na     BTST    #PAD_A_B,D1
        BEQ.S   .nb
        BSR     next_pitch
.nb     BTST    #PAD_B_B,D1
        BEQ.S   .nc
        EORI.W  #1,v_opt_b
        BRA.S   .changed
.nc     BTST    #PAD_C_B,D1
        BEQ.S   .ret
        EORI.W  #1,v_opt_c
.changed
        MOVE.W  #1,v_dirty
        TST.W   v_has_doc
        BEQ.S   .ret
        BSR     glyph_draw
        BSR     glyph_xfer
.ret    RTS

fr_glyph:
        TST.W   v_has_doc
        BEQ.S   .ret
        TST.W   v_opt_b
        BEQ.S   .ret
        BSR     doc_restart
.ret    RTS

; Fabric の XOR の値：INVERT で $FF、GUARD で $01（GUARD OFF なら 0）。出力：D0。
glyph_xor:
        MOVEQ   #0,D0
        TST.W   v_opt_c
        BEQ.S   .g
        MOVE.W  #$FF,D0
.g      TST.W   v_opt_b
        BNE.S   .ret
        EORI.W  #$01,D0
.ret    RTS

; 文字 ROM の v_glyph×256 から 256 byte を、DOC の波形 RAM に。
glyph_xfer:
        LEA     FABRIC_CH0,A1
        MOVE.W  #PORT_CHAR_ROM,FAB_SRC(A1)
        MOVE.W  #PORT_DOC,FAB_DST(A1)
        MOVE.W  v_glyph,D0
        LSL.W   #8,D0
        MOVE.W  D0,FAB_SRC_ADDR_L(A1)
        MOVE.W  #WAVE_PAGE<<8,FAB_DST_ADDR_L(A1)
        MOVE.W  #256,FAB_COUNT(A1)
        MOVE.W  #1,FAB_SRC_STEP(A1)
        MOVE.W  #1,FAB_DST_STEP(A1)
        MOVE.W  #FMT_8_8,FAB_FORMAT(A1)
        MOVE.W  #$FFFF,FAB_MASK(A1)
        BSR     glyph_xor
        MOVE.W  D0,FAB_XOR(A1)
        MOVE.W  #0,FAB_SHIFT(A1)
        MOVEQ   #TRG_NOW,D0
        BRA     fab_go_wait

; 字形の帯（32 字を 256×8 に、CPU が DWT で書く）と、DOC が読む 256 byte の図（縦の線 256 本）。
glyph_draw:
        BSR     acrtc_wait_idle
        MOVE.L  #GLYPH_Y*64,D0
        BSR     set_rwp
        MOVE.W  #ACRTC_DWT,D5
        BSR     ac_put
        MOVE.W  #63,D5
        BSR     ac_put
        MOVE.W  #-7,D5
        BSR     ac_put
        LEA     CHAR_ROM,A3
        LEA     expand(PC),A4
        MOVE.W  v_glyph,D0
        LSL.W   #8,D0                   ; ROM の offset の頭
        MOVEQ   #0,D6                   ; 行
.row    MOVEQ   #0,D4                   ; 字
.char   MOVE.W  D4,D1
        LSL.W   #3,D1
        ADD.W   D6,D1
        ADD.W   D0,D1
        MOVEQ   #0,D2
        MOVE.W  D1,D2
        ADD.L   D2,D2
        MOVE.B  0(A3,D2.L),D1           ; 字形の 1 行（MSB が左）
        MOVE.W  D1,D2
        LSR.W   #3,D2
        ANDI.W  #$1E,D2
        MOVE.W  0(A4,D2.W),D5           ; 左の 4 画素
        BSR     ac_put
        MOVE.W  D1,D2
        ADD.W   D2,D2
        ANDI.W  #$1E,D2
        MOVE.W  0(A4,D2.W),D5           ; 右の 4 画素
        BSR     ac_put
        ADDQ.W  #1,D4
        CMP.W   #32,D4
        BLO.S   .char
        ADDQ.W  #1,D6
        CMP.W   #8,D6
        BLO.S   .row
        ; 図：消してから、byte ごとに下から値/4 の高さの線
        MOVEQ   #COL_BG,D0
        BSR     set_color
        MOVEQ   #0,D0
        MOVE.W  #SCOPE_Y,D1
        MOVE.W  #255,D2
        MOVE.W  #SCOPE_Y2,D3
        MOVEQ   #ACRTC_OPM_REP,D4
        BSR     rect
        MOVEQ   #COL_SCOPE,D0
        BSR     set_color
        BSR     glyph_xor
        MOVE.W  D0,D3
        MOVE.W  v_glyph,D0
        LSL.W   #8,D0
        MOVEQ   #0,D6                   ; byte の番号＝x
.col    MOVE.W  D0,D1
        ADD.W   D6,D1
        MOVEQ   #0,D2
        MOVE.W  D1,D2
        ADD.L   D2,D2
        MOVEQ   #0,D1
        MOVE.B  0(A3,D2.L),D1
        EOR.W   D3,D1
        ANDI.W  #$FF,D1
        LSR.W   #2,D1                   ; 0〜63
        MOVE.W  #ACRTC_AMOVE,D5
        BSR     ac_put
        MOVE.W  D6,D5
        BSR     ac_put
        MOVE.W  #-SCOPE_Y2,D5
        BSR     ac_put
        MOVE.W  #ACRTC_ALINE,D5
        BSR     ac_put
        MOVE.W  D6,D5
        BSR     ac_put
        MOVE.W  #SCOPE_Y2,D5
        SUB.W   D1,D5
        NEG.W   D5
        BSR     ac_put
        ADDQ.W  #1,D6
        CMP.W   #256,D6
        BLO.S   .col
        RTS

; 4 bit（MSB が左の画素）→ 4 画素の語（左の画素が下位ニブル）。1 は色 15。
expand:
        DC.W    $0000,$F000,$0F00,$FF00,$00F0,$F0F0,$0FF0,$FFF0
        DC.W    $000F,$F00F,$0F0F,$FF0F,$00FF,$F0FF,$0FFF,$FFFF

; ------------------------------------------------------------------ 3. TEMPORAL FEEDBACK

en_temporal:
        MOVE.W  #TMP_CLEAR,TEMPORAL_CTRL
        MOVE.W  #0,v_tbase
        RTS

in_temporal:
        BTST    #PAD_UP_B,D1
        BEQ.S   .nu
        CMP.W   #DELAY_MAX,v_delay
        BHS.S   .nu
        ADDQ.W  #1,v_delay
        MOVE.W  #1,v_dirty
.nu     BTST    #PAD_DOWN_B,D1
        BEQ.S   .na
        CMP.W   #1,v_delay
        BLS.S   .na
        SUBQ.W  #1,v_delay
        MOVE.W  #1,v_dirty
.na     BTST    #PAD_A_B,D1
        BEQ.S   .nb
        EORI.W  #1,v_opt_a
        MOVE.W  #1,v_dirty
.nb     BTST    #PAD_B_B,D1
        BEQ.S   .nc
        EORI.W  #1,v_opt_b
        MOVE.W  #1,v_dirty
        BSR     clear_copies
.nc     BTST    #PAD_C_B,D1
        BEQ.S   .ret
        EORI.W  #1,v_opt_c
        MOVE.W  #1,v_dirty
        MOVEQ   #0,D0
        TST.W   v_opt_c
        BEQ.S   .h
        MOVE.W  #TMP_HOLD,D0
.h      MOVE.W  D0,TEMPORAL_CTRL
.ret    RTS

; COPY：記録してから、d・2d・3d 前の像を右の 3 つの矩形に。FEEDBACK：d 前の像を今の像に EOR してから記録する。
fr_temporal:
        BSR     anim_live
        BSR     ddm_on
        TST.W   v_opt_b
        BNE.S   .fb
        BSR     t_capture
        MOVEQ   #1,D6
.k      MOVE.W  v_delay,D0
        MULU    D6,D0
        BSR     t_seek
        MOVE.W  D6,D0
        LSL.W   #4,D0
        ADD.W   #T_Y*64,D0
        ANDI.L  #$FFFF,D0
        MOVE.W  #ACRTC_DWT,D2
        BSR     t_play
        ADDQ.W  #1,D6
        CMP.W   #3,D6
        BLS.S   .k
        BRA.S   .done
.fb     MOVE.W  v_delay,D0
        BSR     t_seek
        MOVE.L  #T_Y*64,D0
        MOVE.W  #ACRTC_DMOD|ACRTC_OPM_EOR,D2
        BSR     t_play
        BSR     t_capture
.done   BSR     ddm_off
        BSR     t_record_bytes
        ADD.W   D0,v_tbase
        MOVE.W  v_frame,D0
        ANDI.W  #15,D0
        BNE.S   .ret
        BSR     t_status
.ret    RTS

; 1 フレームの記録の byte 数。出力：D0。
t_record_bytes:
        MOVE.W  #T_WORDS,D0
        TST.W   v_opt_a
        BEQ.S   .ret
        ADD.W   D0,D0
.ret    RTS

; RPTR を D0.W フレーム前の記録の頭に。D0・D1 を壊す。
t_seek:
        MOVE.W  D0,D1
        BSR     t_record_bytes
        MULU    D1,D0
        MOVE.W  v_tbase,D1
        SUB.W   D0,D1                   ; 64 KiB で回る
        MOVE.W  D1,TEMPORAL_RPTR
        RTS

; 今の矩形を Temporal RAM の v_tbase から記録する（8 bit：上位 byte。2 PASS：続けて下位 byte）。
t_capture:
        MOVE.W  v_tbase,TEMPORAL_WPTR
        MOVE.W  #FMT_16_8_HI,D3
        BSR.S   .pass
        TST.W   v_opt_a
        BEQ.S   .ret
        MOVE.W  #FMT_16_8_LO,D3
.pass   MOVE.L  #T_Y*64,D0
        BSR     set_rwp
        MOVE.W  #ACRTC_DRD,D5
        BSR     ac_put
        MOVE.W  #T_W/4-1,D5
        BSR     ac_put
        MOVE.W  #-(T_H-1),D5
        BSR     ac_put
        LEA     FABRIC_CH0,A1
        MOVE.W  #PORT_ACRTC,FAB_SRC(A1)
        MOVE.W  #PORT_TEMPORAL_W,FAB_DST(A1)
        MOVE.W  #0,FAB_SHIFT(A1)
        BRA     t_fabric
.ret    RTS

; RPTR から、語番地 D0.L の矩形へ。D2＝コマンド（DWT か DMOD|EOR）。2 PASS の 2 回目は下位 byte を
; DMOD の OR（FEEDBACK では EOR）で重ねる。D6 を保つ。
t_play:
        MOVE.L  D0,-(SP)
        MOVE.W  D2,-(SP)
        MOVEQ   #8,D4                   ; 8 bit → 語の上位 byte
        BSR.S   .pass
        MOVE.W  (SP)+,D2
        MOVE.L  (SP)+,D0
        TST.W   v_opt_a
        BEQ.S   .ret
        CMP.W   #ACRTC_DWT,D2
        BNE.S   .lo
        MOVE.W  #ACRTC_DMOD|ACRTC_OPM_OR,D2
.lo     MOVEQ   #0,D4                   ; 下位 byte
.pass   BSR     set_rwp
        MOVE.W  D2,D5
        BSR     ac_put
        MOVE.W  #T_W/4-1,D5
        BSR     ac_put
        MOVE.W  #-(T_H-1),D5
        BSR     ac_put
        LEA     FABRIC_CH0,A1
        MOVE.W  #PORT_TEMPORAL_R,FAB_SRC(A1)
        MOVE.W  #PORT_ACRTC,FAB_DST(A1)
        MOVE.W  D4,FAB_SHIFT(A1)
        MOVE.W  #FMT_8_16,D3
        BRA     t_fabric
.ret    RTS

; チャネル 0 の残りを書いて走らせる（SRC・DST・SHIFT は呼ぶ側）。D3＝FORMAT。
t_fabric:
        MOVE.W  #0,FAB_SRC_ADDR_L(A1)
        MOVE.W  #0,FAB_DST_ADDR_L(A1)
        MOVE.W  #T_WORDS,FAB_COUNT(A1)
        MOVE.W  #0,FAB_SRC_STEP(A1)
        MOVE.W  #0,FAB_DST_STEP(A1)
        MOVE.W  D3,FAB_FORMAT(A1)
        MOVE.W  #$FFFF,FAB_MASK(A1)
        MOVE.W  #0,FAB_XOR(A1)
        MOVEQ   #TRG_NOW,D0
        BRA     fab_go_wait

; 右の 3 つの矩形を消す（FEEDBACK に切り替えたとき）。
clear_copies:
        MOVEQ   #COL_BG,D0
        BSR     set_color
        MOVEQ   #T_W,D0
        MOVE.W  #T_Y,D1
        MOVE.W  #255,D2
        MOVE.W  #T_Y+T_H-1,D3
        MOVEQ   #ACRTC_OPM_REP,D4
        BRA     rect

; 今の矩形：消して、球と縦の棒を描く。
anim_live:
        MOVEQ   #COL_BG,D0
        BSR     set_color
        MOVEQ   #0,D0
        MOVE.W  #T_Y,D1
        MOVEQ   #T_W-1,D2
        MOVE.W  #T_Y+T_H-1,D3
        MOVEQ   #ACRTC_OPM_REP,D4
        BSR     rect
        MOVEQ   #COL_BAR,D0
        BSR     set_color
        MOVE.W  v_frame,D0
        ANDI.W  #T_W-1,D0
        MOVE.W  #T_Y,D1
        MOVE.W  D0,D2
        ADDQ.W  #1,D2
        CMP.W   #T_W-1,D2
        BLS.S   .bar
        MOVE.W  D0,D2
.bar    MOVE.W  #T_Y+T_H-1,D3
        MOVEQ   #ACRTC_OPM_REP,D4
        BSR     rect
        LEA     v_ball,A2
        MOVE.W  #T_W-BALL,D2
        MOVE.W  #T_Y,D3
        MOVE.W  #T_Y+T_H-BALL,D4
        BSR     move_obj
        MOVE.W  v_frame,D0
        LSR.W   #3,D0
        BSR     cycle_color
        BSR     set_color
        MOVE.W  (A2),D0
        MOVE.W  2(A2),D1
        MOVE.W  D0,D2
        ADDQ.W  #BALL-1,D2
        MOVE.W  D1,D3
        ADDQ.W  #BALL-1,D3
        MOVEQ   #ACRTC_OPM_REP,D4
        BRA     rect

; DIST と CTRL（EQUAL・OVERRUN・UNDERRUN）を描く。
t_status:
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BG,D3
        MOVEQ   #4,D5
        MOVEQ   #0,D4
        MOVE.W  TEMPORAL_DIST,D4
        MOVEQ   #48,D0
        MOVE.W  #Y_ST,D1
        JSR     BIOS_PUT_HEX
        MOVE.W  TEMPORAL_CTRL,D4
        MOVE.W  #136,D0
        JSR     BIOS_PUT_HEX
        RTS

; ------------------------------------------------------------------ 共通：DOC

; オシレータ 0（L）と 1（R）を、波形 RAM の頁 WAVE_PAGE の 256 byte（T＝0、R＝0）で自走させる。
doc_start:
        BSR     doc_freq
        LEA     DOC,A0
        MOVEQ   #2-1,D1
        MOVEQ   #DOC_CTL_FREE,D2        ; CA は 0 から
.osc    MOVE.B  #WAVE_PAGE,DOC_POINTER-DOC(A0)
        MOVE.B  #0,DOC_SIZE-DOC(A0)
        MOVE.B  D2,DOC_CONTROL-DOC(A0)
        MOVE.B  #DOC_VOL,DOC_VOLUME-DOC(A0)
        ADDI.B  #1<<DOC_CTL_CA_SH,D2
        ADDQ.L  #DOC_OSC_STRIDE,A0
        DBRA    D1,.osc
        RTS

; 0 の byte で止まったオシレータを動かし直す（HALT を 0 に書く）。
doc_restart:
        MOVE.B  #DOC_CTL_FREE,DOC_CONTROL
        MOVE.B  #(1<<DOC_CTL_CA_SH)|DOC_CTL_FREE,DOC_CONTROL+DOC_OSC_STRIDE
        RTS

; FC：55.556 kHz×FC÷512 byte/秒。256 byte の 1 周期で、130 は 55 Hz、260 は 110 Hz、520 は 220 Hz。
; オシレータ 1 は 1 だけ高くして、うなりを出す。
doc_freq:
        TST.W   v_has_doc
        BEQ.S   .ret
        BSR     pitch_fc
        MOVE.B  D0,DOC_FREQ_LO
        MOVE.W  D0,D1
        LSR.W   #8,D1
        MOVE.B  D1,DOC_FREQ_HI
        ADDQ.W  #1,D0
        MOVE.B  D0,DOC_FREQ_LO+DOC_OSC_STRIDE
        MOVE.W  D0,D1
        LSR.W   #8,D1
        MOVE.B  D1,DOC_FREQ_HI+DOC_OSC_STRIDE
.ret    RTS

; 出力：D0＝今の FC。
pitch_fc:
        MOVE.W  v_pitch,D0
        ADD.W   D0,D0
        MOVE.W  fc_table(PC,D0.W),D0
        RTS
fc_table:
        DC.W    130,260,520

next_pitch:
        MOVE.W  v_pitch,D0
        ADDQ.W  #1,D0
        CMP.W   #3,D0
        BLO.S   .p
        MOVEQ   #0,D0
.p      MOVE.W  D0,v_pitch
        MOVE.W  #1,v_dirty
        BRA     doc_freq

no_doc:
        LEA     str_no_doc(PC),A0
        MOVEQ   #8,D0
        MOVE.W  #Y_ST,D1
        MOVEQ   #COL_BAD,D2
        BRA     puts

; ------------------------------------------------------------------ 共通：絵

; ページ 0・1 の絵：箱 3 つと斜めの線。C（ページ 0）で EOR で描く。絵の全体を毎フレーム消すと、ACRTC の塗りが
; 表示の走査に追い越されて何も見えないので、前のフレームの箱と線だけを色 0 で消す（背景の復元）。
anim_canvas:
        TST.W   v_drawn
        BEQ.S   .new
        MOVEQ   #COL_BG,D0
        BSR     set_color
        MOVEQ   #ACRTC_OPM_REP,D4
        LEA     v_old,A2
        MOVEQ   #NBOX-1,D6
.erase  MOVE.W  (A2)+,D0
        MOVE.W  (A2)+,D1
        MOVE.W  D0,D2
        ADD.W   #BOX-1,D2
        MOVE.W  D1,D3
        ADD.W   #BOX-1,D3
        BSR     rect
        DBRA    D6,.erase
        MOVE.W  (A2),D0
        BSR     canvas_line
.new    LEA     v_boxes,A2
        LEA     v_old,A3
        MOVEQ   #0,D6                   ; 箱の番号
.box    MOVE.W  #256-BOX,D2
        MOVE.W  #CANVAS_Y,D3
        MOVE.W  #CANVAS_Y2-BOX+1,D4
        BSR     move_obj
        MOVE.W  v_frame,D0
        LSR.W   #4,D0
        MOVE.W  D6,D1
        MULU    #5,D1
        ADD.W   D1,D0
        BSR     cycle_color
        BSR     set_color
        MOVE.W  (A2),D0
        MOVE.W  2(A2),D1
        MOVE.W  D0,(A3)+
        MOVE.W  D1,(A3)+
        MOVE.W  D0,D2
        ADD.W   #BOX-1,D2
        MOVE.W  D1,D3
        ADD.W   #BOX-1,D3
        BSR     canvas_opm
        BSR     rect
        ADDQ.L  #8,A2
        ADDQ.W  #1,D6
        CMP.W   #NBOX,D6
        BLO.S   .box
        MOVEQ   #COL_LINE,D0
        BSR     set_color
        MOVE.W  v_frame,D0
        MULU    #3,D0
        ANDI.W  #255,D0
        MOVE.W  D0,(A3)
        MOVE.W  #1,v_drawn
        BSR     canvas_opm
        ; 続けて canvas_line

; 絵の線：上の (D0, CANVAS_Y) から下の (255−D0, CANVAS_Y2) へ。D4＝OPM。D5・D7 を壊す。
canvas_line:
        MOVE.W  #ACRTC_AMOVE,D5
        BSR     ac_put
        MOVE.W  D0,D5
        BSR     ac_put
        MOVE.W  #-CANVAS_Y,D5
        BSR     ac_put
        MOVE.W  #ACRTC_ALINE,D5
        OR.W    D4,D5
        BSR     ac_put
        MOVE.W  #255,D5
        SUB.W   D0,D5
        BSR     ac_put
        MOVE.W  #-CANVAS_Y2,D5
        BRA     ac_put

; 出力：D4＝絵の OPM（ページ 0 の C で EOR）。
canvas_opm:
        MOVEQ   #ACRTC_OPM_REP,D4
        TST.W   v_page
        BNE.S   .ret
        TST.W   v_opt_c
        BEQ.S   .ret
        MOVEQ   #ACRTC_OPM_EOR,D4
.ret    RTS

; (A2) の x、y を dx、dy だけ動かす。範囲（x は 0〜D2、y は D3〜D4）を出るなら向きを変えて動かさない。
move_obj:
        MOVE.W  (A2),D0
        ADD.W   4(A2),D0
        BMI.S   .xr
        CMP.W   D2,D0
        BGT.S   .xr
        MOVE.W  D0,(A2)
        BRA.S   .y
.xr     NEG.W   4(A2)
.y      MOVE.W  2(A2),D0
        ADD.W   6(A2),D0
        CMP.W   D3,D0
        BLT.S   .yr
        CMP.W   D4,D0
        BGT.S   .yr
        MOVE.W  D0,2(A2)
        RTS
.yr     NEG.W   6(A2)
        RTS

; D0 を 1〜15 の色にする（0 は背景なので使わない）。
cycle_color:
        ANDI.W  #15,D0
        BNE.S   .ret
        MOVEQ   #1,D0
.ret    RTS

; ------------------------------------------------------------------ 共通：ACRTC と Fabric

; CCR の DDM を立てる・下ろす。ACRTC が仕事を終えてから書く（DDM は実行を始めたときの値で決まる）。
ddm_on:
        MOVE.W  #ACRTC_CCR_DDM|ACRTC_CCR_GBM4,D0
        BRA     set_ccr
ddm_off:
        MOVE.W  #ACRTC_CCR_GBM4,D0
set_ccr:
        BSR     acrtc_wait_idle
        MOVE.W  #ACRTC_CCR,(A6)
        MOVE.W  D0,2(A6)
        MOVE.W  #ACRTC_FIFO,(A6)
        RTS

; 走査の 2 行を読む DRD を送る（64 語×2 行、下へ）。
drd_scan:
        MOVEQ   #0,D0
        MOVE.W  v_scan,D0
        ADD.W   #CANVAS_Y,D0
        LSL.L   #6,D0
        BSR     set_rwp
        MOVE.W  #ACRTC_DRD,D5
        BSR     ac_put
        MOVE.W  #63,D5
        BSR     ac_put
        MOVEQ   #-1,D5
        BRA     ac_put

; A1＝チャネル。止めて、印を消す。
fab_prep:
        MOVE.W  #FAB_STOP,FAB_CTRL(A1)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A1)
        RTS

; TRIGGER を D0 にして始め、DONE か ERROR を待って止める。誤りと時間切れは v_errors に数える。D0・D1 を壊す。
fab_go_wait:
        MOVE.W  D0,FAB_TRIGGER(A1)
        MOVE.W  #FAB_START,FAB_CTRL(A1)
        MOVE.L  #FAB_TIMEOUT,D1
.wait   MOVE.W  FAB_STATUS(A1),D0
        BTST    #3,D0                   ; ERROR
        BNE.S   .bad
        BTST    #1,D0                   ; DONE
        BNE     fab_prep
        SUBQ.L  #1,D1
        BNE.S   .wait
.bad    ADDQ.W  #1,v_errors
        MOVE.W  #1,v_dirty
        BRA     fab_prep

; RWP を基本画面（DN＝1）の語番地 D0.L にする。D5・D7 を壊す。
set_rwp:
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPH,D5
        BSR     ac_put
        MOVE.L  D0,D5
        LSR.L   #8,D5
        LSR.L   #4,D5
        ANDI.W  #$00FF,D5
        ORI.W   #$4000,D5
        BSR     ac_put
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPL,D5
        BSR     ac_put
        MOVE.W  D0,D5
        LSL.W   #4,D5
        BRA     ac_put

; 描く色（CL0 と CL1）を D0.W（0〜15）にする。同じ色なら書かない。D5・D7 を壊す。
set_color:
        CMP.W   v_color,D0
        BEQ.S   .ret
        MOVE.W  D0,v_color
        MOVE.W  D0,-(SP)
        ANDI.W  #15,D0
        MULU    #$1111,D0
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL0,D5
        BSR     ac_put
        MOVE.W  D0,D5
        BSR     ac_put
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL1,D5
        BSR     ac_put
        MOVE.W  D0,D5
        BSR     ac_put
        MOVE.W  (SP)+,D0
.ret    RTS

; (D0,D1)〜(D2,D3)（画面の座標、両端を含む）を塗る。D4＝OPM。y は符号を変えて渡す（bios-abi.md §06）。
; D5・D7 を壊す。
rect:
        MOVE.W  #ACRTC_AMOVE,D5
        BSR     ac_put
        MOVE.W  D0,D5
        BSR     ac_put
        MOVE.W  D1,D5
        NEG.W   D5
        BSR     ac_put
        MOVE.W  #ACRTC_AFRCT,D5
        OR.W    D4,D5
        BSR     ac_put
        MOVE.W  D2,D5
        BSR     ac_put
        MOVE.W  D3,D5
        NEG.W   D5
        BRA     ac_put

; D5.W を FIFO に書く。SR の WFR を確かめてから（DR-033）。A6＝ACRTC_AR、AR は r00。D7 を壊す。
ac_put:
        MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ     ac_put
        MOVE.W  D5,2(A6)
        RTS

; ACRTC が仕事を終えるまで待つ（FIFO が空で、コマンドが終わった）。上限あり。D7 を壊す。
acrtc_wait_idle:
        MOVE.L  D1,-(SP)
        MOVE.W  #$7FFF,D1
.wait   MOVE.W  (A6),D7
        ANDI.W  #ACRTC_SR_WFE|ACRTC_SR_CED,D7
        CMP.W   #ACRTC_SR_WFE|ACRTC_SR_CED,D7
        DBEQ    D1,.wait
        MOVE.L  (SP)+,D1
        RTS

; ------------------------------------------------------------------ 字

; (A0) を (D0,D1) に色 D2 で（背景は COL_BG）。
puts:
        MOVEQ   #COL_BG,D3
        JMP     BIOS_PUTS

; D4.W を D5.W 桁の 16 進で (D0,D1) に。
put_hex:
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BG,D3
        ANDI.L  #$FFFF,D4
        JMP     BIOS_PUT_HEX

; D4.W 番目（0〜）の 4 字の名前を (D0,D1) に。A0＝名前の表（4 字＋0 を 6 byte ずつ）。
put_choice:
        MULU    #6,D4
        ADDA.W  D4,A0
        MOVEQ   #COL_TEXT,D2
        BRA     puts

; ページの題名、見出し、助けの 2 行。
draw_static:
        LEA     str_title(PC),A0
        MOVEQ   #8,D0
        MOVEQ   #Y_TITLE,D1
        MOVEQ   #COL_TITLE,D2
        BSR     puts
        MOVE.W  v_page,D0
        LSL.W   #4,D0
        LEA     page_text(PC),A2
        ADDA.W  D0,A2
        MOVE.L  (A2)+,A0                ; 名前
        MOVEQ   #88,D0
        MOVEQ   #Y_TITLE,D1
        MOVEQ   #COL_NAME,D2
        BSR     puts
        MOVE.L  (A2)+,A0
        MOVEQ   #8,D0
        MOVE.W  #Y_H1,D1
        MOVEQ   #COL_LABEL,D2
        BSR     puts
        MOVE.L  (A2)+,A0
        MOVEQ   #8,D0
        MOVE.W  #Y_H2,D1
        MOVEQ   #COL_LABEL,D2
        BSR     puts
        MOVE.L  (A2)+,A2                ; 見出しの並び：x、y、文字列。x＝−1 で終わり
.next   MOVE.W  (A2)+,D0
        BMI.S   .ret
        MOVE.W  (A2)+,D1
        MOVEQ   #COL_LABEL,D2
        MOVE.L  A2,A0
        BSR     puts
.skip   TST.B   (A2)+
        BNE.S   .skip
        MOVE.L  A2,D0
        ADDQ.L  #1,D0
        ANDI.W  #$FFFE,D0
        MOVE.L  D0,A2
        BRA.S   .next
.ret    RTS

; 変わった値を描く。
draw_params:
        TST.W   v_dirty
        BEQ.S   .ret
        MOVE.W  #0,v_dirty
        MOVE.W  v_page,D2
        ADD.W   D2,D2
        MOVE.W  pg_params(PC,D2.W),D2
        JMP     pg_params(PC,D2.W)
.ret    RTS
pg_params:
        DC.W    pa_wave-pg_params
        DC.W    pa_adpcm-pg_params
        DC.W    pa_glyph-pg_params
        DC.W    pa_temporal-pg_params

pa_wave:
        BSR     pa_row
        BSR     pa_fc
        MOVE.W  v_opt_b,D4
        LEA     names_guard(PC),A0
        MOVEQ   #56,D0
        MOVE.W  #Y_P2,D1
        BSR     put_choice
        MOVE.W  v_opt_c,D4
        LEA     names_draw(PC),A0
        MOVE.W  #168,D0
        MOVE.W  #Y_P2,D1
        BRA     put_choice

pa_adpcm:
        BSR     pa_row
        MOVE.W  v_rate,D4
        LEA     names_rate(PC),A0
        MOVE.W  #136,D0
        MOVE.W  #Y_P1,D1
        BSR     put_choice
        MOVE.W  v_opt_a,D4
        LEA     names_nibble(PC),A0
        MOVEQ   #64,D0
        MOVE.W  #Y_P2,D1
        BSR     put_choice
        MOVE.W  v_opt_c,D4
        LEA     names_reset(PC),A0
        MOVE.W  #176,D0
        MOVE.W  #Y_P2,D1
        BRA     put_choice

pa_glyph:
        MOVE.W  v_glyph,D4
        LSL.W   #8,D4
        MOVEQ   #4,D5
        MOVEQ   #40,D0
        MOVE.W  #Y_P1,D1
        BSR     put_hex
        BSR     pa_fc
        MOVE.W  v_opt_b,D4
        LEA     names_guard(PC),A0
        MOVEQ   #56,D0
        MOVE.W  #Y_P2,D1
        BSR     put_choice
        MOVE.W  v_opt_c,D4
        LEA     names_onoff(PC),A0
        MOVE.W  #184,D0
        MOVE.W  #Y_P2,D1
        BRA     put_choice

pa_temporal:
        MOVE.W  v_delay,D4
        MOVEQ   #2,D5
        MOVEQ   #56,D0
        MOVE.W  #Y_P1,D1
        BSR     put_hex
        MOVEQ   #1,D6                   ; d、2d、3d を矩形の下に
.k      MOVE.W  v_delay,D4
        MULU    D6,D4
        MOVEQ   #2,D5
        MOVE.W  D6,D0
        LSL.W   #6,D0
        ADD.W   #24,D0
        MOVE.W  #Y_LABEL,D1
        BSR     put_hex
        ADDQ.W  #1,D6
        CMP.W   #3,D6
        BLS.S   .k
        MOVE.W  v_opt_a,D4
        LEA     names_bits(PC),A0
        MOVEQ   #48,D0
        MOVE.W  #Y_P2,D1
        BSR     put_choice
        MOVE.W  v_opt_b,D4
        LEA     names_mode(PC),A0
        MOVE.W  #128,D0
        MOVE.W  #Y_P2,D1
        BSR     put_choice
        MOVE.W  v_opt_c,D4
        LEA     names_onoff(PC),A0
        MOVE.W  #216,D0
        MOVE.W  #Y_P2,D1
        BRA     put_choice

pa_row:
        MOVE.W  v_scan,D4
        MOVEQ   #2,D5
        MOVEQ   #40,D0
        MOVE.W  #Y_P1,D1
        BRA     put_hex

pa_fc:
        BSR     pitch_fc
        MOVE.W  D0,D4
        MOVEQ   #4,D5
        MOVE.W  #128,D0
        MOVE.W  #Y_P1,D1
        BRA     put_hex

; Fabric の誤りがあれば数を描く。
draw_errors:
        MOVE.W  v_errors,D4
        BEQ.S   .ret
        MOVE.W  v_frame,D0
        ANDI.W  #31,D0
        BNE.S   .ret
        LEA     str_err(PC),A0
        MOVEQ   #8,D0
        MOVE.W  #Y_ERR,D1
        MOVEQ   #COL_BAD,D2
        BSR     puts
        MOVE.W  v_errors,D4
        MOVEQ   #4,D5
        MOVEQ   #48,D0
        MOVE.W  #Y_ERR,D1
        BRA     put_hex
.ret    RTS

; ------------------------------------------------------------------ 表と文字列（大文字・数字・空白だけ）

box_init:
        DC.W    16,CANVAS_Y+4,3,1
        DC.W    112,CANVAS_Y+30,-2,2
        DC.W    200,CANVAS_Y+12,1,-1
        DC.W    10,T_Y+6,2,1            ; 球

; ページごとに：名前、助け 1、助け 2、見出しの並び（16 byte）
page_text:
        DC.L    pn_wave,h1_scan,h2_wave,lb_wave
        DC.L    pn_adpcm,h1_scan,h2_adpcm,lb_adpcm
        DC.L    pn_glyph,h1_glyph,h2_glyph,lb_glyph
        DC.L    pn_temporal,h1_temporal,h2_temporal,lb_temporal

lb_wave:
        DC.W    8,Y_P1
        DC.B    "ROW",0
        EVEN
        DC.W    96,Y_P1
        DC.B    "FC",0
        EVEN
        DC.W    8,Y_P2
        DC.B    "GUARD",0
        EVEN
        DC.W    128,Y_P2
        DC.B    "DRAW",0
        EVEN
        DC.W    -1
lb_adpcm:
        DC.W    8,Y_P1
        DC.B    "ROW",0
        EVEN
        DC.W    96,Y_P1
        DC.B    "RATE",0
        EVEN
        DC.W    8,Y_P2
        DC.B    "FIRST",0
        EVEN
        DC.W    128,Y_P2
        DC.B    "RESET",0
        EVEN
        DC.W    -1
lb_glyph:
        DC.W    8,Y_P1
        DC.B    "ROM",0
        EVEN
        DC.W    96,Y_P1
        DC.B    "FC",0
        EVEN
        DC.W    8,Y_P2
        DC.B    "GUARD",0
        EVEN
        DC.W    128,Y_P2
        DC.B    "INVERT",0
        EVEN
        DC.W    -1
lb_temporal:
        DC.W    8,Y_LABEL
        DC.B    "LIVE",0
        EVEN
        DC.W    8,Y_P1
        DC.B    "DELAY",0
        EVEN
        DC.W    8,Y_P2
        DC.B    "BITS",0
        EVEN
        DC.W    88,Y_P2
        DC.B    "MODE",0
        EVEN
        DC.W    176,Y_P2
        DC.B    "HOLD",0
        EVEN
        DC.W    8,Y_ST
        DC.B    "DIST",0
        EVEN
        DC.W    96,Y_ST
        DC.B    "CTRL",0
        EVEN
        DC.W    -1

; 4 字＋0 を 6 byte ずつ
names_guard:    DC.B    "ON  ",0,0,"OFF ",0,0
names_onoff:    DC.B    "OFF ",0,0,"ON  ",0,0
names_draw:     DC.B    "REP ",0,0,"EOR ",0,0
names_rate:     DC.B    "4KHZ",0,0,"6KHZ",0,0,"8KHZ",0,0
names_nibble:   DC.B    "HIGH",0,0,"LOW ",0,0
names_reset:    DC.B    "ONCE",0,0,"EACH",0,0
names_bits:     DC.B    "8   ",0,0,"16  ",0,0
names_mode:     DC.B    "COPY",0,0,"EOR ",0,0

str_title:      DC.B    "SAMPLE 3",0
pn_wave:        DC.B    "GRAPHICS AS WAVE",0
pn_adpcm:       DC.B    "IMAGE AS ADPCM",0
pn_glyph:       DC.B    "GLYPH AS WAVE",0
pn_temporal:    DC.B    "TEMPORAL FEEDBACK",0
h1_scan:        DC.B    "LR PAGE  UD ROW",0
h2_wave:        DC.B    "A PITCH  B GUARD  C EOR",0
h2_adpcm:       DC.B    "A NIBBLE  B RATE  C RESET",0
h1_glyph:       DC.B    "LR PAGE  UD GLYPHS",0
h2_glyph:       DC.B    "A PITCH  B GUARD  C INVERT",0
h1_temporal:    DC.B    "LR PAGE  UD DELAY",0
h2_temporal:    DC.B    "A BITS  B MODE  C HOLD",0
str_no_doc:     DC.B    "NO DOC BOARD",0
str_err:        DC.B    "ERR",0
        EVEN
rom_end:
