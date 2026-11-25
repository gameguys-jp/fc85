; 内蔵のデモ（bios.s から INCLUDE）。カートリッジがないときに動く。
;
; 画面（256×192）:
;   y 0〜15    帯（色 2）と見出し
;   y 20       FRAME（VBlank の数、16 進 8 桁）と PAD（PORT_A、16 進 4 桁）
;   y 30〜47   音の道（DOC か DAC）、ボタンの 8 つの箱（押すと黄）と名前
;   y 56〜171  枠と扇の線。中を円が回り、方向キーで箱が動く（どちらも XOR で描き、XOR で消す）
;   y 176〜187 パレットの 16 色
; A・B・C・START で音（C5、E5、G5、C6）。START は背景の色も替える。

        RSSET   snd_vars_end
d_pad           RS.W    1
d_pad_drawn     RS.W    1
d_drawn         RS.W    1               ; 動く物を描いてあるか
d_cx            RS.W    1               ; 描いてある円の中心と半径
d_cy            RS.W    1
d_cr            RS.W    1
d_bx            RS.W    1               ; 描いてある箱の左上
d_by            RS.W    1
d_px            RS.W    1               ; 方向キーで動かす箱の左上
d_py            RS.W    1
d_chime         RS.W    1
d_bg            RS.W    1
d_vars_end      RS.B    0
        ASSERT  d_vars_end<=BIOS_VARS_END

FIELD_X0        EQU     4
FIELD_Y0        EQU     56
FIELD_X1        EQU     251
FIELD_Y1        EQU     171
ORBIT_X         EQU     128
ORBIT_Y         EQU     114
BOX             EQU     10              ; 箱の大きさ
COLOR_TEXT      EQU     12
COLOR_BG        EQU     1

demo:
        MOVE.W  #-1,d_pad_drawn
        MOVE.W  #0,d_drawn
        MOVE.W  #60,d_px
        MOVE.W  #100,d_py
        MOVE.W  #1,d_chime
        MOVE.W  #0,d_bg
        BSR     demo_screen
        BSR     snd_init
        BSR     demo_sound_label
.loop   BSR     bios_wait_vbl
        BSR     demo_input
        BSR     demo_chime
        BSR     snd_update
        BSR     demo_objects
        BSR     demo_status
        BRA.S   .loop

; ------------------------------------------------------------------ 動かない絵

demo_screen:
        MOVEQ   #COLOR_BG,D0
        BSR     bios_cls
        MOVEQ   #ACRTC_OPM_REP,D4
        ; 帯
        MOVEQ   #2,D0
        BSR     set_color
        MOVEQ   #0,D0
        MOVEQ   #0,D1
        MOVE.W  #255,D2
        MOVEQ   #15,D3
        BSR     draw_frect
        ; 見出し
        LEA     str_title(PC),A0
        MOVEQ   #8,D0
        MOVEQ   #4,D1
        MOVEQ   #15,D2
        MOVEQ   #2,D3
        BSR     bios_puts
        LEA     str_hello(PC),A0
        MOVE.W  #216,D0
        BSR     bios_puts
        ; 名前
        MOVEQ   #COLOR_TEXT,D2
        MOVEQ   #COLOR_BG,D3
        LEA     str_frame(PC),A0
        MOVEQ   #8,D0
        MOVEQ   #20,D1
        BSR     bios_puts
        LEA     str_pad(PC),A0
        MOVE.W  #144,D0
        BSR     bios_puts
        LEA     str_sound(PC),A0
        MOVEQ   #8,D0
        MOVEQ   #32,D1
        BSR     bios_puts
        LEA     str_buttons(PC),A0
        MOVE.W  #128,D0
        MOVEQ   #40,D1
        BSR     bios_puts
        LEA     str_press(PC),A0
        MOVEQ   #8,D0
        MOVEQ   #44,D1
        BSR     bios_puts
        ; 枠
        MOVEQ   #7,D0
        BSR     set_color
        MOVEQ   #FIELD_X0,D0
        MOVEQ   #FIELD_Y0,D1
        MOVE.W  #FIELD_X1,D2
        MOVEQ   #FIELD_Y0,D3
        BSR     draw_line
        MOVE.W  #FIELD_X1,D0
        MOVEQ   #FIELD_Y0,D1
        MOVE.W  #FIELD_X1,D2
        MOVE.W  #FIELD_Y1,D3
        BSR     draw_line
        MOVE.W  #FIELD_X1,D0
        MOVE.W  #FIELD_Y1,D1
        MOVEQ   #FIELD_X0,D2
        MOVE.W  #FIELD_Y1,D3
        BSR     draw_line
        MOVEQ   #FIELD_X0,D0
        MOVE.W  #FIELD_Y1,D1
        MOVEQ   #FIELD_X0,D2
        MOVEQ   #FIELD_Y0,D3
        BSR     draw_line
        ; 扇：下の真ん中から上の辺へ 9 本
        MOVEQ   #9,D0
        BSR     set_color
        MOVEQ   #12,D5                  ; 上の辺の x
.fan    MOVE.W  #ORBIT_X,D0
        MOVE.W  #FIELD_Y1-1,D1
        MOVE.W  D5,D2
        MOVEQ   #FIELD_Y0+1,D3
        BSR     draw_line
        ADDI.W  #29,D5
        CMP.W   #FIELD_X1,D5
        BLO.S   .fan
        ; パレットの 16 色
        MOVEQ   #0,D5
.pal    MOVE.W  D5,D0
        BSR     set_color
        MOVE.W  D5,D0
        LSL.W   #4,D0
        MOVE.W  #176,D1
        MOVE.W  D0,D2
        ADDI.W  #15,D2
        MOVE.W  #187,D3
        BSR     draw_frect
        ADDQ.W  #1,D5
        CMP.W   #16,D5
        BLO.S   .pal
        RTS

; 音の道を書く（snd_init の後）。
demo_sound_label:
        LEA     str_off(PC),A0
        MOVE.W  snd_mode,D0
        BEQ.S   .draw
        LEA     str_doc(PC),A0
        CMP.W   #1,D0
        BEQ.S   .draw
        LEA     str_dac(PC),A0
.draw   MOVE.W  #56,D0
        MOVEQ   #32,D1
        MOVEQ   #15,D2
        MOVEQ   #COLOR_BG,D3
        BRA     bios_puts

; ------------------------------------------------------------------ 毎フレーム

; パッド：押したボタンで音、方向キーで箱を動かす。
demo_input:
        BSR     bios_read_pad
        MOVE.W  D0,d_pad
        BTST    #PAD_A_B,D1
        BEQ.S   .b
        MOVEQ   #0,D0
        BSR     snd_note
.b      BTST    #PAD_B_B,D1
        BEQ.S   .c
        MOVEQ   #1,D0
        BSR     snd_note
.c      BTST    #PAD_C_B,D1
        BEQ.S   .start
        MOVEQ   #2,D0
        BSR     snd_note
.start  BTST    #PAD_START_B,D1
        BEQ.S   .move
        MOVEQ   #3,D0
        BSR     snd_note
        MOVE.W  d_bg,D0                 ; 背景の色を替える（パレットの 1 番）
        ADDQ.W  #1,D0
        ANDI.W  #3,D0
        MOVE.W  D0,d_bg
        ADD.W   D0,D0
        MOVE.W  bg_colors(PC,D0.W),PALETTE+2*COLOR_BG
.move   MOVE.W  d_pad,D0
        MOVE.W  d_px,D2
        MOVE.W  d_py,D3
        BTST    #PAD_LEFT_B,D0
        BEQ.S   .r
        SUBQ.W  #2,D2
.r      BTST    #PAD_RIGHT_B,D0
        BEQ.S   .u
        ADDQ.W  #2,D2
.u      BTST    #PAD_UP_B,D0
        BEQ.S   .d
        SUBQ.W  #2,D3
.d      BTST    #PAD_DOWN_B,D0
        BEQ.S   .clamp
        ADDQ.W  #2,D3
.clamp  CMP.W   #FIELD_X0+2,D2
        BGE.S   .x1
        MOVEQ   #FIELD_X0+2,D2
.x1     CMP.W   #FIELD_X1-1-BOX,D2
        BLE.S   .y0
        MOVE.W  #FIELD_X1-1-BOX,D2
.y0     CMP.W   #FIELD_Y0+2,D3
        BGE.S   .y1
        MOVEQ   #FIELD_Y0+2,D3
.y1     CMP.W   #FIELD_Y1-1-BOX,D3
        BLE.S   .done
        MOVE.W  #FIELD_Y1-1-BOX,D3
.done   MOVE.W  D2,d_px
        MOVE.W  D3,d_py
        RTS

bg_colors:
        DC.W    $013,$031,$301,$222

; 起動の音：C5、少し後に G5。
demo_chime:
        MOVE.W  d_chime,D0
        BEQ.S   .ret
        ADDQ.W  #1,d_chime
        CMP.W   #1,D0
        BNE.S   .second
        MOVEQ   #0,D0
        BRA     snd_note
.second CMP.W   #13,D0
        BNE.S   .ret
        MOVE.W  #0,d_chime
        MOVEQ   #2,D0
        BRA     snd_note
.ret    RTS

; 円と箱：前のフレームの絵を XOR で消し、新しい位置に XOR で描く。
demo_objects:
        MOVEQ   #ACRTC_OPM_EOR,D4
        TST.W   d_drawn
        BEQ.S   .new
        BSR.S   .draw
.new    ; 円の中心：VBlank の数で正弦の表を引く
        MOVE.L  bios_frame,D0
        ANDI.W  #63,D0
        LEA     sine64(PC),A0
        MOVE.B  0(A0,D0.W),D1
        EXT.W   D1
        MULS    #88,D1
        ASR.L   #7,D1
        ADDI.W  #ORBIT_X,D1
        MOVE.W  D1,d_cx
        ADDI.W  #16,D0
        ANDI.W  #63,D0
        MOVE.B  0(A0,D0.W),D1
        EXT.W   D1
        MULS    #36,D1
        ASR.L   #7,D1
        ADDI.W  #ORBIT_Y,D1
        MOVE.W  D1,d_cy
        ; 半径は音の包絡で膨らむ
        MOVE.W  snd_env,D0
        LSR.W   #5,D0                   ; 0〜7
        ADDI.W  #10,D0
        MOVE.W  D0,d_cr
        MOVE.W  d_px,d_bx
        MOVE.W  d_py,d_by
        MOVE.W  #1,d_drawn
; d_cx・d_cy・d_cr の円と、d_bx・d_by の箱を XOR で描く。
.draw   MOVEQ   #15,D0
        BSR     set_color
        MOVE.W  d_cx,D0
        MOVE.W  d_cy,D1
        MOVE.W  d_cr,D2
        BSR     draw_circle
        MOVEQ   #2,D0
        BSR     set_color
        MOVE.W  d_bx,D0
        MOVE.W  d_by,D1
        MOVE.W  D0,D2
        ADDI.W  #BOX-1,D2
        MOVE.W  D1,D3
        ADDI.W  #BOX-1,D3
        BRA     draw_frect

; 数とボタンの箱。
demo_status:
        MOVEQ   #COLOR_TEXT+3,D2
        MOVEQ   #COLOR_BG,D3
        MOVE.L  bios_frame,D4
        MOVEQ   #8,D5
        MOVEQ   #56,D0
        MOVEQ   #20,D1
        BSR     bios_put_hex
        MOVEQ   #0,D4
        MOVE.W  d_pad,D4
        MOVEQ   #4,D5
        MOVE.W  #176,D0
        BSR     bios_put_hex
        ; ボタンの箱（変わったときだけ）
        MOVE.W  d_pad,D6
        CMP.W   d_pad_drawn,D6
        BEQ.S   .ret
        MOVE.W  D6,d_pad_drawn
        MOVEQ   #ACRTC_OPM_REP,D4
        MOVEQ   #0,D5                   ; ビット
.box    MOVEQ   #11,D0
        BTST    D5,D6
        BEQ.S   .col
        MOVEQ   #5,D0
.col    BSR     set_color
        MOVE.W  D5,D0
        LSL.W   #4,D0
        ADDI.W  #128,D0
        MOVEQ   #30,D1
        MOVE.W  D0,D2
        ADDQ.W  #6,D2
        MOVEQ   #37,D3
        BSR     draw_frect
        ADDQ.W  #1,D5
        CMP.W   #8,D5
        BLO.S   .box
.ret    RTS

; 1 周 64 の正弦（−127〜127）
sine64:
        DC.B    0,12,25,37,49,60,71,81,90,98,106,112,117,122,125,126
        DC.B    127,126,125,122,117,112,106,98,90,81,71,60,49,37,25,12
        DC.B    0,-12,-25,-37,-49,-60,-71,-81,-90,-98,-106,-112,-117,-122,-125,-126
        DC.B    -127,-126,-125,-122,-117,-112,-106,-98,-90,-81,-71,-60,-49,-37,-25,-12

; 文字列（JIS X 0201）。字形のない字（小文字、記号の多く）は使わない。
str_title:      DC.B    "BOOT ROM DEMO",0
str_hello:      DC.B    "ﾊﾛｰ",0
str_frame:      DC.B    "FRAME",0
str_pad:        DC.B    "PAD",0
str_sound:      DC.B    "SOUND",0
str_doc:        DC.B    "DOC",0
str_dac:        DC.B    "DAC",0
str_off:        DC.B    "OFF",0
str_buttons:    DC.B    "U D L R A B C S",0
str_press:      DC.B    "ABC ｷｰ ｦ ｵｽ",0
        EVEN
