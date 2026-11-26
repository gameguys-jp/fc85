; SAMPLE 3：ACRTC の表示の機能を見せるカートリッジ。m68kasm で組む（build.py）。説明は README.md。
;
; 画面（案B の 192 ラスタを 3 つに分ける。台帳 HD63484.md §3.4、U85 p.30・p.75–111）：
;   上画面   48 ラスタ  縦のスムーズスクロール（SAR0 を MW ずつ動かす）。ウィンドウを重ねる
;   基本画面 120 ラスタ 縦横 2 倍のズーム（ZFR）。点滅するグラフィックカーソル（GCR・CDR）
;   下画面   24 ラスタ  横のスムーズスクロール（SAR2 の語と SDA の画素）
; ウィンドウ（64 画素×32 ラスタ）は上画面の上に固定して出す。基本画面の横のズームはラスタ全体の属性
; （HZ）なので、ズームしたラスタにウィンドウを置くと崩れる（emulator.md §03）。それで上画面に重ねる。
;
; 番地とビットの名前は fc85.inc、BIOS の約束は bios.inc（sdk/docs/bios-abi.md）。

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"

; ------------------------------------------------------------------ 配置

UPPER_H         EQU     48              ; ラスタ
BASE_H          EQU     120
LOWER_H         EQU     24
        ASSERT  UPPER_H+BASE_H+LOWER_H=192

V_TOP           EQU     3+40            ; 表示の最初のラスタ（VSW＋VDS、台帳 §3.4）
H_TOP           EQU     13+19           ; 表示の最初のメモリサイクル（HSW＋HDS）

; VRAM（語番地）。上画面とウィンドウは MW 64（BIOS_PUTS の y＝番地/64 で文字を描ける）
UPPER_VRAM      EQU     $4000           ; 64 語×112 行（48 行を見せ、64 行ぶん縦に動かす）
UPPER_ROWS      EQU     112
LOWER_VRAM      EQU     $6000           ; 128 語×24 行（64 語を見せ、256 画素ぶん横に動かす）
LOWER_MW        EQU     128
WIN_VRAM        EQU     $7000           ; 64 語幅のうち左の 16 語×32 行

; ウィンドウ：画面の x 160〜223、ラスタ 51〜82（上画面の 9〜40 行目）
WIN_X           EQU     160
WIN_W           EQU     64
WIN_Y           EQU     V_TOP+8
WIN_H           EQU     32
; HWR：HWS（HSYNC の終わりから）−1、HWW−1（メモリサイクル。1 画素は 1/2 メモリサイクル）
WIN_HWR         EQU     ((H_TOP+WIN_X/2-13-1)<<8)|(WIN_W/2-1)
WIN_VWS         EQU     WIN_Y-3-1       ; VSYNC の終わりから −1

; カーソル：画面の x 112〜127（メモリサイクル 88〜95）、ラスタ 140〜155
CUR_X           EQU     112
CUR_Y           EQU     140
CUR_GCRX        EQU     ((H_TOP+CUR_X/2+7)<<8)|(H_TOP+CUR_X/2)
CUR_CYS         EQU     CUR_Y-3
CUR_CYE         EQU     CUR_Y-3+15
CUR_CDR         EQU     ACRTC_CDR_GRAPHIC|ACRTC_CDR_CON1*1|ACRTC_CDR_COFF1*1   ; 8 フィールド点き 8 消える

DCR_ALL         EQU     ACRTC_DCR_BASE|ACRTC_DCR_UPPER|ACRTC_DCR_LOWER|ACRTC_DCR_WIN
ZOOM_2X         EQU     $1100           ; HZF＝1、VZF＝1

; ------------------------------------------------------------------ Main RAM

        RSSET   APP_RAM
v_frame         RS.W    1
v_vars_end      RS.B    0

; ------------------------------------------------------------------ 見出し（bios-abi.md §04）

        ORG     CART
cart_header:
        DC.L    CART_MAGIC
        DC.W    CART_HDR_VERSION
        DC.W    0
        DC.L    start
        DC.L    rom_end-CART
        DC.B    "SAMPLE 3        "
        ASSERT  *=CART+cart_header_size

; ------------------------------------------------------------------ 入口

start:
        LEA     INITIAL_SSP,SP
        CLR.W   v_frame
        LEA     palette(PC),A0
        JSR     BIOS_SET_PAL
        BSR     setup_screens           ; 先に MW を決める（CLR は RWP の画面の MW で行を進める）
        BSR     draw_base
        BSR     draw_upper
        BSR     draw_window
        BSR     draw_lower

main_loop:
        JSR     BIOS_WAIT_VBL
        ADDQ.W  #1,v_frame
        BSR     scroll
        BRA.S   main_loop

; ------------------------------------------------------------------ 毎フレーム：スクロール

; 上画面：SAR0＝UPPER_VRAM＋(f mod 64)×64。下画面：x＝f mod 256 画素、SAR2＝LOWER_VRAM＋x/4、SDA＝x mod 4。
; VBlank の中で書く。ACRTC はラスタの頭にレジスタを読むので、次のフレームの頭から効く。
scroll:
        MOVE.W  v_frame,D0
        MOVE.W  D0,D1
        ANDI.W  #63,D1
        LSL.W   #6,D1
        ADDI.W  #UPPER_VRAM,D1
        MOVE.W  #ACRTC_SAR0H,ACRTC_AR
        MOVE.W  #0,ACRTC_DATA
        MOVE.W  D1,ACRTC_DATA           ; AR は r80 から先で語ごとに 2 進む（SAR0L）
        MOVE.W  D0,D1
        ANDI.W  #255,D1
        MOVE.W  D1,D2
        ANDI.W  #3,D2
        LSL.W   #8,D2                   ; SDA（bit 11–8）
        LSR.W   #2,D1
        ADDI.W  #LOWER_VRAM,D1
        MOVE.W  #ACRTC_SAR2H,ACRTC_AR
        MOVE.W  D2,ACRTC_DATA
        MOVE.W  D1,ACRTC_DATA
        MOVE.W  #ACRTC_FIFO,ACRTC_AR
        RTS

; ------------------------------------------------------------------ 画面の設定

setup_screens:
        LEA     screen_regs(PC),A0
.next   MOVE.W  (A0)+,D0
        BMI.S   .done
        MOVE.W  D0,ACRTC_AR
        MOVE.W  (A0)+,ACRTC_DATA
        BRA.S   .next
.done   MOVE.W  #ACRTC_FIFO,ACRTC_AR
        RTS

; レジスタの番号と値の組。最後に DCR（使う画面を決める）
screen_regs:
        DC.W    ACRTC_SSW0,UPPER_H
        DC.W    ACRTC_SSW,BASE_H
        DC.W    ACRTC_SSW2,LOWER_H
        DC.W    ACRTC_MWR0,64
        DC.W    ACRTC_SAR0H,0
        DC.W    ACRTC_SAR0L,UPPER_VRAM
        DC.W    ACRTC_MWR2,LOWER_MW
        DC.W    ACRTC_SAR2H,0
        DC.W    ACRTC_SAR2L,LOWER_VRAM
        DC.W    ACRTC_MWR3,64
        DC.W    ACRTC_SAR3H,0
        DC.W    ACRTC_SAR3L,WIN_VRAM
        DC.W    ACRTC_HWR,WIN_HWR
        DC.W    ACRTC_VWS,WIN_VWS
        DC.W    ACRTC_VWW,WIN_H
        DC.W    ACRTC_ZFR,ZOOM_2X
        DC.W    ACRTC_GCRX,CUR_GCRX
        DC.W    ACRTC_GCRYS,CUR_CYS
        DC.W    ACRTC_GCRYE,CUR_CYE
        DC.W    ACRTC_CDR,CUR_CDR
        DC.W    ACRTC_DCR,DCR_ALL
        DC.W    -1

; ------------------------------------------------------------------ 描く

; 基本画面（2 倍で見えるのは左上の 128×60 画素）
draw_base:
        MOVEQ   #1,D0
        JSR     BIOS_CLS
        LEA     base_cmds(PC),A0
        MOVE.W  #(base_cmds_end-base_cmds)/2,D1
        JSR     BIOS_ACRTC_SEND
        LEA     base_text(PC),A1
        BRA     puts_list

; 上画面：8 行ごとの色の帯と文字
draw_upper:
        LEA     upper_cmds(PC),A0
        MOVE.W  #(upper_cmds_end-upper_cmds)/2,D1
        JSR     BIOS_ACRTC_SEND
        LEA     upper_text(PC),A1
        BRA     puts_list

draw_window:
        LEA     window_cmds(PC),A0
        MOVE.W  #(window_cmds_end-window_cmds)/2,D1
        JSR     BIOS_ACRTC_SEND
        LEA     window_text(PC),A1
        BRA     puts_list

; 下画面（MW 128）：16 画素ごとの目盛りと、64 画素ごとの太い印
draw_lower:
        LEA     lower_cmds(PC),A0
        MOVE.W  #(lower_cmds_end-lower_cmds)/2,D1
        JMP     BIOS_ACRTC_SEND

; (A1) の並び（x, y, 前景, 背景, 文字列の番地。x が −1 で終わり）を BIOS_PUTS で描く。
puts_list:
.next   MOVE.W  (A1)+,D0
        BMI.S   .done
        MOVE.W  (A1)+,D1
        MOVE.W  (A1)+,D2
        MOVE.W  (A1)+,D3
        MOVEA.L (A1)+,A0
        JSR     BIOS_PUTS
        BRA.S   .next
.done   RTS

; ------------------------------------------------------------------ 描画のコマンド

; RWP を語番地 a（画面 dn の MW で行が進む）にする
RWP_H           EQU     ACRTC_WPR|ACRTC_P_RWPH
RWP_L           EQU     ACRTC_WPR|ACRTC_P_RWPL

base_cmds:
        ; 箱（論理座標の +Y は画面の上。BIOS は ORG を基本画面の左上に置く）
        DC.W    ACRTC_WPR|ACRTC_P_CL0,$AAAA
        DC.W    ACRTC_AMOVE,80,-30
        DC.W    ACRTC_AFRCT,119,-52
        DC.W    ACRTC_WPR|ACRTC_P_CL0,$7777
        DC.W    ACRTC_AMOVE,88,-36
        DC.W    ACRTC_AFRCT,111,-46
        DC.W    ACRTC_WPR|ACRTC_P_CL0,$FFFF
        DC.W    ACRTC_AMOVE,0,-58
        DC.W    ACRTC_ALINE,127,-58
base_cmds_end:

UPPER_BAND      SET     0
upper_cmds:
        REPT    UPPER_ROWS/8
        DC.W    RWP_H,$4000|((UPPER_VRAM+UPPER_BAND*8*64)>>12)
        DC.W    RWP_L,((UPPER_VRAM+UPPER_BAND*8*64)&$FFF)<<4
        DC.W    ACRTC_CLR,$1111*(2+UPPER_BAND%6),63,-7
UPPER_BAND      SET     UPPER_BAND+1
        ENDR
upper_cmds_end:

window_cmds:
        DC.W    RWP_H,$4000|(WIN_VRAM>>12),RWP_L,(WIN_VRAM&$FFF)<<4
        DC.W    ACRTC_CLR,$CCCC,15,-(WIN_H-1)
        DC.W    RWP_H,$4000|(WIN_VRAM>>12),RWP_L,(WIN_VRAM&$FFF)<<4
        DC.W    ACRTC_CLR,$FFFF,15,0
        DC.W    RWP_H,$4000|((WIN_VRAM+(WIN_H-1)*64)>>12),RWP_L,((WIN_VRAM+(WIN_H-1)*64)&$FFF)<<4
        DC.W    ACRTC_CLR,$FFFF,15,0
        DC.W    RWP_H,$4000|(WIN_VRAM>>12),RWP_L,(WIN_VRAM&$FFF)<<4
        DC.W    ACRTC_CLR,$CCCF,0,-(WIN_H-1)            ; 左端の画素（語の下位ニブル）
        DC.W    RWP_H,$4000|((WIN_VRAM+15)>>12),RWP_L,((WIN_VRAM+15)&$FFF)<<4
        DC.W    ACRTC_CLR,$FCCC,0,-(WIN_H-1)            ; 右端の画素
window_cmds_end:

LOWER_TICK      SET     0
lower_cmds:
        DC.W    RWP_H,$8000|(LOWER_VRAM>>12),RWP_L,(LOWER_VRAM&$FFF)<<4
        DC.W    ACRTC_CLR,$9999,LOWER_MW-1,-(LOWER_H-1)
        REPT    LOWER_MW/4
        DC.W    RWP_H,$8000|((LOWER_VRAM+LOWER_TICK*4)>>12)
        DC.W    RWP_L,((LOWER_VRAM+LOWER_TICK*4)&$FFF)<<4
        IF      LOWER_TICK%4=0
        DC.W    ACRTC_CLR,$7777,1,-(LOWER_H-1)
        ELSE
        DC.W    ACRTC_CLR,$9997,0,-(LOWER_H/2-1)
        ENDIF
LOWER_TICK      SET     LOWER_TICK+1
        ENDR
lower_cmds_end:

; ------------------------------------------------------------------ 文字

base_text:
        DC.W    4,4,15,1
        DC.L    s_base
        DC.W    4,16,7,1
        DC.L    s_zoom
        DC.W    4,26,15,1
        DC.L    s_blink
        DC.W    -1
upper_text:
        DC.W    8,256+0,15,2
        DC.L    s_upper
        DC.W    8,256+32,15,6
        DC.L    s_upper
        DC.W    8,256+64,15,4
        DC.L    s_upper
        DC.W    8,256+96,15,2
        DC.L    s_upper
        DC.W    -1
window_text:
        DC.W    8,WIN_VRAM/64+12,15,12
        DC.L    s_window
        DC.W    -1

s_base          DC.B    "BASE SCREEN",0
s_zoom          DC.B    "ZOOM X2",0
s_blink         DC.B    "CURSOR",0
s_upper         DC.B    "UPPER  V SCROLL",0
s_window        DC.B    "WINDOW",0
        EVEN

; 黒、紺、青、赤、紫、緑、水、黄、灰、濃紺、橙、青緑、オリーブ、桃、白に近い灰、白
palette:
        DC.W    $000,$124,$03C,$C22,$A2A,$2A4,$2AC,$EE2
        DC.W    $888,$236,$E82,$088,$664,$E8C,$CCC,$FFF

rom_end:
