; SAMPLE 2：DSP（TMS32010）を動かすサンプルのカートリッジ。m68kasm で組む（build.py）。説明は README.md。
;
; 1. 起動時に、試験の音（build.py が合成した 8 bit の PCM、2 秒）を Staging RAM に写す。
; 2. DSP をリセットし（DSP_CTRL の RESET）、PRGBUS を待ち、DSP のプログラム（effects.dsp を dspasm で組んだ語）を
;    プログラム窓（$280000〜）に書く。残りの語（エコーの遅延線）は 0 にし、全部を読み返して確かめる。
;    BIOSEL＝出力 FIFO の空きにして RESET を解く。メールボックスで RD_MAGIC を読み、動いたことを確かめる。
; 3. Data Fabric：チャネル 0 が Staging RAM（port 0）→ DSP の入力 FIFO（port 3）。8 bit → 16 bit（FORMAT 2）、
;    SHIFT 8、XOR $8000 で符号付きの Q15 にする。LOOP で 2 秒を繰り返す。
;    チャネル 1 が DSP の出力 FIFO（port 4）→
;      DOC の子基板がない：本体 DAC の FIFO（port 2）。DAC は音声クロック（16 kHz）の拍ごとに 1 語を取る。
;      ある　　　　　　：DOC の波形 RAM の 256 byte の輪（port 2、上位 byte）。拍（15.625 kHz）ごとに 1 要素書き、
;                        オシレータ 0（L）と 1（R）が同じ速さで読む。
; 4. 主ループ（1 フレームに 1 回）：A・B・C でモード（THRU・LPF・ECHO）を DSP に送る。CMD_SNAP と CMD_READ で
;    標本の数・検査和・山を読み、数を BIOS_PUT_HEX で、山を ACRTC の矩形の水準計で描く。
;
; 番地とビットの名前は fc85.inc、BIOS の約束は bios.inc、DSP との約束は protocol.inc。

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"
        INCLUDE "protocol.inc"

; ------------------------------------------------------------------ 定数

ROUTE_DAC       EQU     1
ROUTE_DOC       EQU     2
AUDIO_N_DAC     EQU     500             ; 8 MHz÷500＝16 kHz（WAV の 48 kHz のちょうど 1/3）
AUDIO_N_DOC     EQU     512             ; 8 MHz÷512＝15.625 kHz（DOC の FC＝144 とそろう）
SRC_STAGE       EQU     $0000           ; 試験の音の Staging RAM の番地（port 0）
FILL_STAGE      EQU     $FF00           ; DOC の輪を埋める 256 byte の $80
DOC_RING        EQU     $1000           ; DOC の波形 RAM の輪（頁 $10、256 byte）
DOC_FC          EQU     144             ; 55.556 kHz×144÷512＝15.625 kHz（1 標本に 0.28125 byte 進む）
DOC_VOL         EQU     255
DOC_LEAD        EQU     128             ; オシレータを走らせる前に書く標本の数（輪の半分）
OUT_COUNT       EQU     4096            ; チャネル 1 の COUNT（LOOP で繰り返す。DAC の道）
DSP_WAIT        EQU     400             ; 応答を待つ回数（1 回 約 5 µs）
CLEAR_FRAME     EQU     8               ; このフレームで UNDERRUN・OVERRUN の印を一度消す（立ち上がりの分）

; 画面（左上が 0,0。文字は 8×8）
X_LABEL         EQU     8
X_VALUE         EQU     64
Y_TITLE         EQU     8
Y_MODE          EQU     32
Y_COUNT         EQU     48
Y_SUM           EQU     64
Y_PEAK          EQU     80
METER_X         EQU     8
METER_Y         EQU     92
METER_W         EQU     240
METER_H         EQU     8
Y_SWITCH        EQU     112
Y_LOAD          EQU     128
Y_FLAGS         EQU     144
Y_ROUTE         EQU     160
Y_HELP          EQU     176
COL_BG          EQU     0
COL_METER_BG    EQU     1
COL_TITLE       EQU     5
COL_METER       EQU     6
COL_MODE        EQU     7
COL_LABEL       EQU     11
COL_BAD         EQU     3
COL_TEXT        EQU     15

; ------------------------------------------------------------------ Main RAM（$100800〜、bios-abi.md §03）

        RSSET   APP_RAM
v_route         RS.W    1               ; ROUTE_DAC・ROUTE_DOC
v_mode          RS.W    1               ; 今のモード（MODE_*）
v_mode_dirty    RS.W    1               ; モードと SW を描き直す
v_dead          RS.W    1               ; DSP が答えない（0 でなければ問い合わせない）
v_frame         RS.W    1
v_count         RS.L    1               ; 標本の数（CMD_SNAP の写し）
v_sum           RS.W    1
v_peak          RS.W    1
v_switch        RS.W    1               ; 最後にモードを替えた標本の番号（下位 16 bit）
v_load_words    RS.W    1               ; 書いた DSP のプログラムの語数
v_load_errors   RS.W    1               ; 読み返して違った語の数
v_magic         RS.W    1               ; RD_MAGIC の応答
v_timeouts      RS.W    1               ; 応答の時間切れの数
v_meter         RS.W    1               ; 描いてある水準計の幅（−1 は未描画）
v_color         RS.W    1               ; ACRTC の CL0・CL1（−1 は不明）
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
        DC.B    "SAMPLE 2        "      ; 題名（16 字、空白で埋める）
        ASSERT  *=CART+cart_header_size

; ------------------------------------------------------------------ 入口

; 入口の条件（bios-abi.md §04）：スーパバイザ、SR＝$2000、画面は色 0、Fabric の両チャネルは止まっている、
; 音は黙らせてある（子基板がなければ本体 DAC は DIRECT で $8080）、DSP は電源投入時の RESET のまま。
start:
        LEA     INITIAL_SSP,SP
        LEA     APP_RAM,A0
        MOVE.W  #(v_vars_end-APP_RAM+3)/4-1,D0
.clr    CLR.L   (A0)+
        DBRA    D0,.clr
        MOVE.W  #-1,v_color
        MOVE.W  #-1,v_meter
        MOVE.W  #1,v_mode_dirty
        LEA     ACRTC_AR,A6             ; A6 はいつも ACRTC（AR は r00 のまま使う）
        MOVE.W  #ROUTE_DAC,v_route
        MOVE.W  bios_modules,D0
        BTST    #FABRIC_MOD_DOC_B,D0
        BEQ.S   .dac
        MOVE.W  #ROUTE_DOC,v_route
.dac    BSR     draw_static
        BSR     copy_source
        BSR     dsp_load
        BSR     dsp_hello
        BSR     draw_load
        TST.W   v_dead
        BNE.S   main_loop               ; DSP が動かない：音の道は作らない
        CMP.W   #ROUTE_DOC,v_route
        BEQ.S   .doc
        BSR     route_dac
        BSR     fabric_input
        BRA.S   main_loop
.doc    BSR     route_doc
        BSR     fabric_input
        BSR     doc_start

main_loop:
        JSR     BIOS_WAIT_VBL
        ADDQ.W  #1,v_frame
        BSR     dsp_poll                ; 数・検査和・山（メールボックス 5 往復）
        BSR     draw_meter
        BSR     input
        BSR     draw_mode
        BSR     draw_stats
        BRA.S   main_loop

; ------------------------------------------------------------------ 試験の音を Staging RAM に写す

; Fabric はカートリッジを読めない（ポートがない）ので、CPU が写す。port 0 の 8 bit の読出しは偶数番地が
; 上位 byte（DR-046）なので、byte の順は ROM と同じ。
copy_source:
        LEA     src_pcm(PC),A0
        LEA     STAGING+SRC_STAGE,A1
        MOVE.W  #SRC_BYTES/4-1,D0
.copy   MOVE.L  (A0)+,(A1)+
        DBRA    D0,.copy
        RTS

; ------------------------------------------------------------------ DSP のプログラムを書く（DR-029 (1)）

; RESET を書き、PRGBUS（8 CLKOUT＝1.6 µs の後）を待ってから、プログラム窓に語で書く。PRGBUS＝0 の間に窓に
; 触るとバスエラーなので、必ず待つ。書いた後に 4096 語を読み返す。
dsp_load:
        MOVE.W  #DSP_CTRL_RESET,DSP_CTRL
        MOVE.W  #1000,D1
.prg    MOVE.W  DSP_STATUS,D0
        BTST    #DSP_ST_PRGBUS_B,D0
        BNE.S   .open
        DBRA    D1,.prg
        MOVE.W  #1,v_dead               ; PRGBUS が来ない
        RTS
.open   LEA     dsp_code(PC),A0
        LEA     DSP_PROGRAM,A1
        MOVE.W  #DSP_CODE_WORDS-1,D1
.copy   MOVE.W  (A0)+,(A1)+
        DBRA    D1,.copy
        MOVEQ   #0,D2
        MOVE.W  #DSP_PROGRAM_WORDS-DSP_CODE_WORDS-1,D1
.zero   MOVE.W  D2,(A1)+                ; エコーの遅延線を 0 に（CLR は読んでから書くので使わない）
        DBRA    D1,.zero
        MOVE.W  #DSP_CODE_WORDS,v_load_words
        ; 読み返す
        MOVEQ   #0,D3
        LEA     dsp_code(PC),A0
        LEA     DSP_PROGRAM,A1
        MOVE.W  #DSP_CODE_WORDS-1,D1
.cmp    CMPM.W  (A0)+,(A1)+
        BEQ.S   .same
        ADDQ.W  #1,D3
.same   DBRA    D1,.cmp
        MOVE.W  #DSP_PROGRAM_WORDS-DSP_CODE_WORDS-1,D1
.cmpz   TST.W   (A1)+
        BEQ.S   .samez
        ADDQ.W  #1,D3
.samez  DBRA    D1,.cmpz
        MOVE.W  D3,v_load_errors
        ; 解く：BIO＝出力 FIFO に空き（DSP は出力の空きで歩調を合わせる。DR-029 (3)）
        MOVE.W  #DSP_CTRL_BIO_SPACE,DSP_CTRL
        RTS

; RD_MAGIC を読む。違えば DSP は動いていない。
dsp_hello:
        TST.W   v_dead
        BNE.S   .ret
        MOVE.W  #CMD_READ<<8|RD_MAGIC,D0
        BSR     dsp_cmd
        MOVE.W  D0,v_magic
        CMP.W   #DSP_MAGIC,D0
        BEQ.S   .ret
        MOVE.W  #1,v_dead
.ret    RTS

; D0.W のコマンドをメールボックスに書き、応答を待って D0.W に返す。時間切れなら C＝1、D0＝RESP_ERROR。
; D1 を壊す。
dsp_cmd:
        MOVE.L  D2,-(SP)
        MOVE.W  D0,DSP_MAILBOX
        MOVE.W  #DSP_WAIT-1,D1
.wait   MOVE.W  DSP_STATUS,D2
        BTST    #DSP_ST_RESPONSE_B,D2
        BNE.S   .got
        DBRA    D1,.wait
        ADDQ.W  #1,v_timeouts
        MOVE.W  #RESP_ERROR,D0
        MOVE.L  (SP)+,D2
        ORI     #1,CCR
        RTS
.got    MOVE.W  DSP_RESPONSE,D0         ; 読むと RESPONSE が下がる
        MOVE.L  (SP)+,D2                ; MOVE は C を 0 にする
        RTS

; 1 フレームに 1 回：写しを取り、数・検査和・山を読む。
dsp_poll:
        TST.W   v_dead
        BNE.S   .ret
        MOVE.W  #CMD_SNAP<<8,D0
        BSR     dsp_cmd
        BCS.S   .dead
        MOVE.W  #CMD_READ<<8|RD_COUNT_HI,D0
        BSR     dsp_cmd
        MOVE.W  D0,v_count
        MOVE.W  #CMD_READ<<8|RD_COUNT_LO,D0
        BSR     dsp_cmd
        MOVE.W  D0,v_count+2
        MOVE.W  #CMD_READ<<8|RD_SUM,D0
        BSR     dsp_cmd
        MOVE.W  D0,v_sum
        MOVE.W  #CMD_READ<<8|RD_PEAK,D0
        BSR     dsp_cmd
        MOVE.W  D0,v_peak
.ret    RTS
.dead   MOVE.W  #1,v_dead
        MOVE.W  #1,v_mode_dirty
        RTS

; ------------------------------------------------------------------ Data Fabric の道

; チャネル 0：Staging RAM の試験の音（8 bit の符号なし）→ DSP の入力 FIFO（port 3）。
; 8 bit → 16 bit（FORMAT 2）、左へ 8（SHIFT）、XOR $8000 で x＝(b−128)×256（Q15）。即時・LOOP：入力 FIFO が
; 満杯の間は STALL で待ち、2 秒の終わりで頭に戻る（設定値の写しに 3 スロット）。
fabric_input:
        LEA     FABRIC_CH0,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_DSP_IN,FAB_DST(A0)
        MOVE.W  #SRC_STAGE,FAB_SRC_ADDR_L(A0)
        MOVE.W  #0,FAB_DST_ADDR_L(A0)
        MOVE.W  #SRC_BYTES,FAB_COUNT(A0)
        MOVE.W  #1,FAB_SRC_STEP(A0)
        MOVE.W  #0,FAB_DST_STEP(A0)
        MOVE.W  #FMT_8_16,FAB_FORMAT(A0)
        MOVE.W  #$FFFF,FAB_MASK(A0)
        MOVE.W  #$8000,FAB_XOR(A0)
        MOVE.W  #8,FAB_SHIFT(A0)
        MOVE.W  #TRG_NOW|TRG_LOOP,FAB_TRIGGER(A0)
        MOVE.W  #FAB_START,FAB_CTRL(A0)
        RTS

; 子基板がない：チャネル 1 が DSP の出力 FIFO（port 4）→ 本体 DAC の FIFO（port 2）。即時・LOOP。DAC の FIFO
; が満杯の間は STALL で待つ。DAC は拍（16 kHz）ごとに 1 語を取るので、全体の歩調は DAC の拍で決まる。
route_dac:
        MOVE.W  #AUDIO_N_DAC,FABRIC_AUDIO_N
        LEA     FABRIC_CH1,A1
        MOVE.W  #FAB_STOP,FAB_CTRL(A1)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A1)
        MOVE.W  #(1<<PORT_DOC)|(1<<PORT_DSP_IN)|(1<<PORT_DSP_OUT),FABRIC_PORT_RESET
        MOVE.W  #PORT_DSP_OUT,FAB_SRC(A1)
        MOVE.W  #PORT_DOC,FAB_DST(A1)
        MOVE.W  #0,FAB_SRC_ADDR_L(A1)
        MOVE.W  #0,FAB_DST_ADDR_L(A1)
        MOVE.W  #OUT_COUNT,FAB_COUNT(A1)
        MOVE.W  #0,FAB_SRC_STEP(A1)
        MOVE.W  #0,FAB_DST_STEP(A1)
        MOVE.W  #FMT_16_16,FAB_FORMAT(A1)
        MOVE.W  #$FFFF,FAB_MASK(A1)
        MOVE.W  #0,FAB_XOR(A1)
        MOVE.W  #0,FAB_SHIFT(A1)
        MOVE.W  #TRG_NOW|TRG_LOOP,FAB_TRIGGER(A1)
        MOVE.W  #FAB_START,FAB_CTRL(A1)
        MOVE.W  #0,DAC_CTRL             ; DIRECT を下げる：拍で FIFO から読む（空の間は $8080 を保つ）
        RTS

; 子基板がある：DOC の波形 RAM の 256 byte の輪を $80 で埋め、オシレータ 0・1 を止めてアキュムレータを 0 に
; し、チャネル 1 を「port 4 → port 2（上位 byte）、拍ごとに 1 要素、LOOP」にする。オシレータは doc_start で
; 走らせる（書き手が輪の半分だけ先に進んでから）。
route_doc:
        MOVE.W  #AUDIO_N_DOC,FABRIC_AUDIO_N
        LEA     STAGING+FILL_STAGE,A0
        MOVE.W  #256/4-1,D0
.fill   MOVE.L  #$80808080,(A0)+        ; 中央（0 は DOC を止める値なので使わない）
        DBRA    D0,.fill
        LEA     FABRIC_CH1,A1
        MOVE.W  #FAB_STOP,FAB_CTRL(A1)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A1)
        MOVE.W  #(1<<PORT_DSP_IN)|(1<<PORT_DSP_OUT),FABRIC_PORT_RESET
        MOVE.W  #PORT_STAGING,FAB_SRC(A1)
        MOVE.W  #PORT_DOC,FAB_DST(A1)
        MOVE.W  #FILL_STAGE,FAB_SRC_ADDR_L(A1)
        MOVE.W  #DOC_RING,FAB_DST_ADDR_L(A1)
        MOVE.W  #256,FAB_COUNT(A1)
        MOVE.W  #1,FAB_SRC_STEP(A1)
        MOVE.W  #1,FAB_DST_STEP(A1)
        MOVE.W  #FMT_8_8,FAB_FORMAT(A1)
        MOVE.W  #$FFFF,FAB_MASK(A1)
        MOVE.W  #0,FAB_XOR(A1)
        MOVE.W  #0,FAB_SHIFT(A1)
        MOVE.W  #TRG_NOW,FAB_TRIGGER(A1)
        MOVE.W  #FAB_START,FAB_CTRL(A1)
        MOVE.L  #20000,D1               ; 待つ上限（約 0.1 秒）
.wait   MOVE.W  FAB_STATUS(A1),D0
        BTST    #3,D0                   ; ERROR
        BNE.S   .fail
        BTST    #1,D0                   ; DONE
        BNE.S   .filled
        SUBQ.L  #1,D1
        BNE.S   .wait
.fail   MOVE.W  #1,v_dead
.filled MOVE.W  #FAB_STOP,FAB_CTRL(A1)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A1)
        ; オシレータ 0（CA 0、L）と 1（CA 1、R）：頁 DOC_RING、256 byte（T＝0）、R＝0、FC＝144、音量 0。
        ; ワンショットで止める（M0＝1 かつ H＝1 で、DOC が次に訪れたときにアキュムレータが 0 になる）。
        MOVE.B  #(16-1)*2,DOC_OSC_ENABLE ; 16 個を走査（BIOS と同じ値。標本化 55.556 kHz）
        LEA     DOC,A0
        MOVEQ   #2-1,D1
        MOVEQ   #DOC_CTL_ONESHOT|DOC_CTL_HALT,D2        ; CA は 0 から
.osc    MOVE.B  #0,2*$40(A0)            ; VOLUME
        MOVE.B  #DOC_RING>>8,2*$80(A0)  ; POINTER
        MOVE.B  #$00,2*$C0(A0)          ; SIZE：T＝0（256 byte）、R＝0
        MOVE.B  #DOC_FC&$FF,2*$00(A0)   ; FREQ_LO
        MOVE.B  #DOC_FC>>8,2*$20(A0)    ; FREQ_HI
        MOVE.B  D2,2*$A0(A0)            ; CONTROL
        ADDI.B  #1<<DOC_CTL_CA_SH,D2
        ADDQ.L  #DOC_OSC_STRIDE,A0
        DBRA    D1,.osc
        ; チャネル 1：port 4 → 波形 RAM の輪。16 bit → 上位 8 bit（FORMAT 4。DSP の語の上位は u）
        MOVE.W  #PORT_DSP_OUT,FAB_SRC(A1)
        MOVE.W  #PORT_DOC,FAB_DST(A1)
        MOVE.W  #0,FAB_SRC_ADDR_L(A1)
        MOVE.W  #DOC_RING,FAB_DST_ADDR_L(A1)
        MOVE.W  #256,FAB_COUNT(A1)
        MOVE.W  #0,FAB_SRC_STEP(A1)
        MOVE.W  #1,FAB_DST_STEP(A1)
        MOVE.W  #FMT_16_8_HI,FAB_FORMAT(A1)
        MOVE.W  #TRG_AUDIO|TRG_EACH|TRG_LOOP,FAB_TRIGGER(A1)
        MOVE.W  #FAB_START,FAB_CTRL(A1)
        RTS

; 書き手（チャネル 1）が DOC_LEAD 標本を書くまで待ち、オシレータ 0・1 を自走で走らせる。書き手と読み手は
; 同じ 15.625 kHz なので、読み手は輪の半分の遅れを保つ。
doc_start:
        LEA     FABRIC_CH1,A1
        MOVE.L  #20000,D1
.wait   MOVE.W  FAB_COUNT(A1),D0        ; 作業値＝残りの数−1
        CMP.W   #256-DOC_LEAD-1,D0
        BLE.S   .go
        SUBQ.L  #1,D1
        BNE.S   .wait
.go     LEA     DOC,A0
        MOVE.B  #DOC_VOL,DOC_VOLUME-DOC(A0)
        MOVE.B  #DOC_VOL,DOC_VOLUME-DOC+DOC_OSC_STRIDE(A0)
        MOVE.B  #(0<<DOC_CTL_CA_SH)|DOC_CTL_FREE,DOC_CONTROL-DOC(A0)
        MOVE.B  #(1<<DOC_CTL_CA_SH)|DOC_CTL_FREE,DOC_CONTROL-DOC+DOC_OSC_STRIDE(A0)
        RTS

; ------------------------------------------------------------------ 入力

; A・B・C を新しく押したら、そのモードを DSP に送る。応答は新しいモードの最初の標本の番号。
input:
        JSR     BIOS_READ_PAD
        MOVEQ   #MODE_THRU,D2
        BTST    #PAD_A_B,D1
        BNE.S   .send
        MOVEQ   #MODE_LPF,D2
        BTST    #PAD_B_B,D1
        BNE.S   .send
        MOVEQ   #MODE_ECHO,D2
        BTST    #PAD_C_B,D1
        BNE.S   .send
        RTS
.send   TST.W   v_dead
        BNE.S   .ret
        CMP.W   v_mode,D2
        BEQ.S   .ret
        MOVE.W  D2,D0
        ORI.W   #CMD_MODE<<8,D0
        BSR     dsp_cmd
        BCS.S   .ret
        MOVE.W  D0,v_switch
        MOVE.W  D2,v_mode
        MOVE.W  #1,v_mode_dirty
.ret    RTS

; ------------------------------------------------------------------ 描画

draw_static:
        LEA     static_text(PC),A2
.next   MOVE.W  (A2)+,D0                ; x（−1 で終わり）
        BMI.S   .route
        MOVE.W  (A2)+,D1                ; y
        MOVE.W  (A2)+,D2                ; 前景
        MOVEQ   #COL_BG,D3
        MOVE.L  A2,A0
        JSR     BIOS_PUTS
.skip   TST.B   (A2)+                   ; 文字列の終わりの次へ
        BNE.S   .skip
        MOVE.L  A2,D0
        ADDQ.L  #1,D0
        ANDI.W  #$FFFE,D0
        MOVE.L  D0,A2
        BRA.S   .next
.route  LEA     str_dac(PC),A0
        CMP.W   #ROUTE_DOC,v_route
        BNE.S   .r
        LEA     str_doc(PC),A0
.r      MOVE.W  #X_VALUE,D0
        MOVE.W  #Y_ROUTE,D1
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
        RTS

; LOAD の行：書いた語数、読み返しの違い、RD_MAGIC の応答。
draw_load:
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BG,D3
        MOVE.W  #X_VALUE,D0
        MOVE.W  #Y_LOAD,D1
        MOVEQ   #0,D4
        MOVE.W  v_load_words,D4
        MOVEQ   #4,D5
        JSR     BIOS_PUT_HEX
        MOVE.W  #X_VALUE+40,D0
        MOVE.W  v_load_errors,D4
        TST.W   D4
        BEQ.S   .ok
        MOVEQ   #COL_BAD,D2
.ok     JSR     BIOS_PUT_HEX
        MOVEQ   #COL_TEXT,D2
        MOVE.W  #X_VALUE+120,D0
        MOVE.W  v_magic,D4
        JSR     BIOS_PUT_HEX
        RTS

; モードの名前と、切り替えた標本の番号（変わったときだけ）。
draw_mode:
        TST.W   v_mode_dirty
        BEQ.S   .ret
        CLR.W   v_mode_dirty
        MOVE.W  #X_VALUE,D0
        MOVE.W  #Y_MODE,D1
        MOVEQ   #COL_MODE,D2
        MOVEQ   #COL_BG,D3
        LEA     str_dead(PC),A0
        TST.W   v_dead
        BNE.S   .name
        MOVE.W  v_mode,D4
        LSL.W   #3,D4
        LEA     mode_names(PC),A0
        ADDA.W  D4,A0
.name   JSR     BIOS_PUTS
        MOVEQ   #COL_TEXT,D2
        MOVE.W  #Y_SWITCH,D1
        MOVEQ   #0,D4
        MOVE.W  v_switch,D4
        MOVEQ   #4,D5
        JSR     BIOS_PUT_HEX
.ret    RTS

; 数は 8 フレームに 1 つずつ描き直す（BIOS_PUTS は 1 字に約 8.5〜12 ライン。bios-abi.md §09 の 8）。
; COUNT と SUM は同じ CMD_SNAP の組なので、同じフレームに描く（12 字で 150 ラインほど）。
draw_stats:
        MOVE.W  v_frame,D6
        ANDI.W  #7,D6
        MOVE.W  #X_VALUE,D0
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BG,D3
        MOVEQ   #0,D4
        CMP.W   #0,D6
        BNE.S   .peak
        MOVE.W  #Y_COUNT,D1
        MOVE.L  v_count,D4
        MOVEQ   #8,D5
        JSR     BIOS_PUT_HEX
        MOVE.W  #X_VALUE,D0
        MOVE.W  #Y_SUM,D1
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BG,D3
        MOVEQ   #0,D4
        MOVE.W  v_sum,D4
        MOVEQ   #4,D5
        JMP     BIOS_PUT_HEX
.peak   CMP.W   #4,D6
        BNE.S   .flags
        MOVE.W  #Y_PEAK,D1
        MOVE.W  v_peak,D4
        MOVEQ   #4,D5
        JMP     BIOS_PUT_HEX
.flags  CMP.W   #6,D6
        BNE.S   .ret
        BSR     flags
        MOVE.W  #X_VALUE,D0
        MOVE.W  #Y_FLAGS,D1
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BG,D3
        MOVEQ   #4,D5
        TST.W   D4
        BEQ.S   .put
        MOVEQ   #COL_BAD,D2
.put    JMP     BIOS_PUT_HEX
.ret    RTS

; FLAG の 4 桁を D4 に作る：千の位 DSP の UNDERRUN、百 OVERRUN、十 本体 DAC の UNDERRUN、一 時間切れの数（15 まで）。
; CLEAR_FRAME のフレームで、立ち上がりの UNDERRUN を一度だけ消す（W1C）。
flags:
        MOVEQ   #0,D4
        MOVE.W  DSP_STATUS,D0
        BTST    #5,D0
        BEQ.S   .or
        ORI.W   #$1000,D4
.or     BTST    #6,D0
        BEQ.S   .dac
        ORI.W   #$0100,D4
.dac    CMP.W   #ROUTE_DAC,v_route
        BNE.S   .to
        MOVE.W  DAC_STATUS,D0
        BTST    #8,D0
        BEQ.S   .to
        ORI.W   #$0010,D4
.to     MOVE.W  v_timeouts,D0
        CMP.W   #15,D0
        BLS.S   .t
        MOVEQ   #15,D0
.t      OR.W    D0,D4
        CMP.W   #CLEAR_FRAME-1,v_frame
        BHI.S   .ret
        ; 立ち上がり（DAC の FIFO が埋まるまで）の印を消す
        MOVE.W  #DSP_ST_UNDERRUN|DSP_ST_OVERRUN,DSP_STATUS
        CMP.W   #ROUTE_DAC,v_route
        BNE.S   .ret
        MOVE.W  #$0100,DAC_STATUS
.ret    RTS

; 水準計：前のフレームの |y| の最大（v_peak）を 128 で割った幅（240 まで）。変わったときだけ描く。
draw_meter:
        MOVE.W  v_peak,D6
        LSR.W   #7,D6
        CMP.W   #METER_W,D6
        BLS.S   .clip
        MOVE.W  #METER_W,D6
.clip   CMP.W   v_meter,D6
        BEQ.S   .ret
        MOVE.W  D6,v_meter
        TST.W   D6
        BEQ.S   .rest
        MOVEQ   #COL_METER,D0
        BSR     set_color
        MOVE.W  #METER_X,D0
        MOVE.W  #METER_Y,D1
        MOVE.W  #METER_X-1,D2
        ADD.W   D6,D2
        MOVE.W  #METER_Y+METER_H-1,D3
        MOVEQ   #ACRTC_OPM_REP,D4
        BSR     rect
.rest   CMP.W   #METER_W,D6
        BEQ.S   .ret
        MOVEQ   #COL_METER_BG,D0
        BSR     set_color
        MOVE.W  #METER_X,D0
        ADD.W   D6,D0
        MOVE.W  #METER_Y,D1
        MOVE.W  #METER_X+METER_W-1,D2
        MOVE.W  #METER_Y+METER_H-1,D3
        MOVEQ   #ACRTC_OPM_REP,D4
        BSR     rect
.ret    RTS

; ------------------------------------------------------------------ ACRTC（sdk/samples/game と同じ書き方）

; 描く色（CL0 と CL1）を D0.W（0〜15）にする。同じ色なら書かない。D5・D7 を壊す。
set_color:
        CMP.W   v_color,D0
        BEQ.S   .ret
        MOVE.W  D0,v_color
        MOVE.W  D0,-(SP)
        ANDI.W  #15,D0
        MULU    #$1111,D0
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL0,D5
        BSR.S   ac_put
        MOVE.W  D0,D5
        BSR.S   ac_put
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL1,D5
        BSR.S   ac_put
        MOVE.W  D0,D5
        BSR.S   ac_put
        MOVE.W  (SP)+,D0
.ret    RTS

; (D0,D1)〜(D2,D3)（画面の座標、両端を含む）を塗る。D4＝OPM。AMOVE と AFRCT。論理座標の +Y は画面の上
; なので、y の符号を変えて渡す（bios-abi.md §06）。D5・D7 を壊す。
rect:
        MOVE.W  #ACRTC_AMOVE,D5
        BSR.S   ac_put
        MOVE.W  D0,D5
        BSR.S   ac_put
        MOVE.W  D1,D5
        NEG.W   D5
        BSR.S   ac_put
        MOVE.W  #ACRTC_AFRCT,D5
        OR.W    D4,D5
        BSR.S   ac_put
        MOVE.W  D2,D5
        BSR.S   ac_put
        MOVE.W  D3,D5
        NEG.W   D5
        BSR.S   ac_put
        RTS

; D5.W を FIFO に書く。SR の WFR を確かめてから（DR-033）。A6＝ACRTC_AR、AR は r00。D7 を壊す。
ac_put:
        MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   ac_put
        MOVE.W  D5,2(A6)
        RTS

; ------------------------------------------------------------------ 文字列（JIS X 0201。字形のある大文字・数字・空白だけ）

; 固定の文字：x、y、前景、文字列（0 で終わり、偶数にそろえる）。x＝−1 で終わり。
static_text:
        DC.W    X_LABEL,Y_TITLE,COL_TITLE
        DC.B    "SAMPLE 2   DSP TMS32010",0
        EVEN
        DC.W    X_LABEL,Y_MODE,COL_LABEL
        DC.B    "MODE",0
        EVEN
        DC.W    X_LABEL,Y_COUNT,COL_LABEL
        DC.B    "COUNT",0
        EVEN
        DC.W    X_LABEL,Y_SUM,COL_LABEL
        DC.B    "SUM",0
        EVEN
        DC.W    X_LABEL,Y_PEAK,COL_LABEL
        DC.B    "PEAK",0
        EVEN
        DC.W    X_LABEL,Y_SWITCH,COL_LABEL
        DC.B    "SW",0
        EVEN
        DC.W    X_LABEL,Y_LOAD,COL_LABEL
        DC.B    "LOAD",0
        EVEN
        DC.W    X_VALUE+96,Y_LOAD,COL_LABEL
        DC.B    "ID",0
        EVEN
        DC.W    X_LABEL,Y_FLAGS,COL_LABEL
        DC.B    "FLAG",0
        EVEN
        DC.W    X_LABEL,Y_ROUTE,COL_LABEL
        DC.B    "ROUTE",0
        EVEN
        DC.W    X_LABEL,Y_HELP,COL_LABEL
        DC.B    "A THRU  B LPF  C ECHO",0
        EVEN
        DC.W    -1

mode_names:                             ; 8 byte ずつ（MODE_* の順）
        DC.B    "THRU",0,0,0,0
        DC.B    "LPF ",0,0,0,0
        DC.B    "ECHO",0,0,0,0
str_dead:       DC.B    "DSP NG",0
str_dac:        DC.B    "DAC 16000 HZ",0
str_doc:        DC.B    "DOC 15625 HZ",0
        EVEN

; ------------------------------------------------------------------ DSP のプログラムと試験の音（build.py が作る）

dsp_code:
        INCBIN  "effects.bin"
dsp_code_end:
DSP_CODE_WORDS  EQU     (dsp_code_end-dsp_code)/2
        ASSERT  DSP_CODE_WORDS<=RING_START      ; エコーの遅延線を 0 にする範囲と重ならない
        EVEN
src_pcm:
        INCBIN  "source.bin"
src_end:
SRC_BYTES       EQU     src_end-src_pcm
        ASSERT  SRC_BYTES%4=0
        ASSERT  SRC_STAGE+SRC_BYTES<=FILL_STAGE
rom_end:
