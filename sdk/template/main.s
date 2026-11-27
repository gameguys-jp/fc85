; ひな形のカートリッジ。m68kasm で組む（build.py）。作り方と試し方は AGENTS.md。
;
; 画面に題、フレームの数、PORT_A の値を出し、方向キーで四角（「*」と呼ぶ）を 8 画素ずつ動かす。START で
; 「*」を真ん中に戻す。文字 ROM の記号の字形はまだそろっていない（描いていない字は市松模様になる）ので、
; 文字列には英字・数字・空白を使う。ここから書き足して自分の ROM にする。音・ACRTC の図形・Data Fabric・DSP の使い方は
; sdk/samples/ の各サンプル（README.md）を見る。
;
; 番地とビットの名前は fc85.inc、BIOS の約束は bios.inc（sdk/docs/bios-abi.md）、ROM が守ることは
; sdk/docs/guarantees.md。

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"

; ------------------------------------------------------------------ 定数

COL_BG          EQU     0               ; 色はパレットの番号（BIOS の既定のパレット。bios-abi.md §06）
COL_DIM         EQU     11
COL_TEXT        EQU     15

CELL            EQU     8               ; 文字は 8×8 画素。BIOS_PUTS の x は 4 の倍数
FIELD_X0        EQU     0               ; 「*」が動く範囲（両端を含む）
FIELD_Y0        EQU     40
FIELD_X1        EQU     248
FIELD_Y1        EQU     184
HOME_X          EQU     128
HOME_Y          EQU     112

; ------------------------------------------------------------------ Main RAM

        RSSET   APP_RAM
v_frame         RS.L    1               ; 入口からのフレームの数
v_x             RS.W    1               ; 「*」の位置
v_y             RS.W    1
v_vars_end      RS.B    0

; ------------------------------------------------------------------ 見出し（bios-abi.md §04）

        ORG     CART
cart_header:
        DC.L    CART_MAGIC
        DC.W    CART_HDR_VERSION
        DC.W    0
        DC.L    start
        DC.L    rom_end-CART
        DC.B    "TEMPLATE        "      ; 題（16 byte。new_rom.py が書き換える）
        ASSERT  *=CART+cart_header_size

; ------------------------------------------------------------------ 入口

start:
        LEA     INITIAL_SSP,SP
        CLR.L   v_frame
        MOVE.W  #HOME_X,v_x
        MOVE.W  #HOME_Y,v_y
        MOVEQ   #COL_BG,D0
        JSR     BIOS_CLS
        MOVEQ   #COL_BG,D3              ; 以下、BIOS_PUTS の背景（BIOS は出力のほかのレジスタを保つ）
        MOVE.W  #COL_TEXT,D2
        LEA     s_title(PC),A0
        MOVEQ   #8,D0
        MOVEQ   #8,D1
        JSR     BIOS_PUTS
        MOVE.W  #COL_DIM,D2
        LEA     s_frame(PC),A0
        MOVEQ   #8,D0
        MOVEQ   #24,D1
        JSR     BIOS_PUTS
        LEA     s_pad(PC),A0
        MOVE.W  #136,D0
        JSR     BIOS_PUTS
        BSR     draw_cursor

main_loop:
        JSR     BIOS_WAIT_VBL           ; 描画は VBlank の直後にまとめる
        ADDQ.L  #1,v_frame
        JSR     BIOS_READ_PAD           ; D0.W＝PORT_A、D1.W＝新しく押したビット
        MOVE.W  D0,D6
        MOVE.W  D1,D7
        BSR     show_numbers
        BSR     move_cursor
        BRA.S   main_loop

; ------------------------------------------------------------------ 数字の表示

; 入力：D6.W＝PORT_A
show_numbers:
        MOVE.W  #COL_TEXT,D2
        MOVEQ   #COL_BG,D3
        MOVE.L  v_frame,D4
        MOVEQ   #8,D5                   ; 8 桁
        MOVEQ   #56,D0
        MOVEQ   #24,D1
        JSR     BIOS_PUT_HEX
        MOVEQ   #0,D4
        MOVE.W  D6,D4
        MOVEQ   #4,D5
        MOVE.W  #168,D0
        JSR     BIOS_PUT_HEX
        RTS

; ------------------------------------------------------------------ 「*」

; 入力：D6.W＝PORT_A（押している間、毎フレーム動く）、D7.W＝新しく押したビット
move_cursor:
        MOVE.W  v_x,D4
        MOVE.W  v_y,D5
        BTST    #PAD_START_B,D7
        BEQ.S   .keys
        MOVE.W  #HOME_X,D4
        MOVE.W  #HOME_Y,D5
        BRA.S   .moved
.keys   BTST    #PAD_LEFT_B,D6
        BEQ.S   .right
        CMPI.W  #FIELD_X0,D4
        BLE.S   .right
        SUBQ.W  #CELL,D4
.right  BTST    #PAD_RIGHT_B,D6
        BEQ.S   .up
        CMPI.W  #FIELD_X1,D4
        BGE.S   .up
        ADDQ.W  #CELL,D4
.up     BTST    #PAD_UP_B,D6
        BEQ.S   .down
        CMPI.W  #FIELD_Y0,D5
        BLE.S   .down
        SUBQ.W  #CELL,D5
.down   BTST    #PAD_DOWN_B,D6
        BEQ.S   .moved
        CMPI.W  #FIELD_Y1,D5
        BGE.S   .moved
        ADDQ.W  #CELL,D5
.moved  CMP.W   v_x,D4
        BNE.S   .redraw
        CMP.W   v_y,D5
        BEQ.S   .done
.redraw MOVEQ   #COL_BG,D3              ; 前の位置を背景の色で消す
        BSR.S   put_at_cursor
        MOVE.W  D4,v_x
        MOVE.W  D5,v_y
        BSR.S   draw_cursor
.done   RTS

; 「*」は空白の字を前景の色の背景で描いた 8×8 の四角（文字 ROM にまだない記号を使わない）
draw_cursor:
        MOVE.W  #COL_TEXT,D3
put_at_cursor:                          ; 入力：D3.W＝塗る色
        LEA     s_blank(PC),A0
        MOVE.W  v_x,D0
        MOVE.W  v_y,D1
        MOVEQ   #COL_BG,D2
        JMP     BIOS_PUTS

; ------------------------------------------------------------------ 文字列

s_title:        DC.B    "TEMPLATE",0    ; 画面の題（new_rom.py が書き換える）
s_frame:        DC.B    "FRAME",0
s_pad:          DC.B    "PAD",0
s_blank:        DC.B    " ",0
        EVEN

rom_end:
