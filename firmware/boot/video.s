; ACRTC と文字（bios.s から INCLUDE）。
;
; 座標：BIOS は ORG を基本画面（DN＝1）の語番地 0、ドット 0 に置く。論理座標の +Y は画面の上なので、
; 画面の左上を (0,0)、下を +y とする画面座標から、ACRTC には y の符号を変えて渡す。
; 画面の語番地は y×64＋x/4。左の画素が語の下位ニブル（台帳 HD63484.md §4.5）。
;
; FIFO の規約（DR-016、DR-033）：書く前に SR の WFR を確かめる。AR/SR とデータポートには読み書き一体の
; 命令を使わない。ここの処理は、AR を r00（FIFO）にしたまま戻る。

; ------------------------------------------------------------------ 初期化

; 打ち切ってから、案B の表示を始め、パターン RAM・MASK・原点を決める。
; CCR は DDM＝0（DWT のデータを CPU が FIFO に書く）[P]。Fabric の port 1 で DMA するアプリは DDM を立てる。
acrtc_init:
        MOVEM.L D1/A0,-(SP)
        MOVE.W  #ACRTC_CCR,ACRTC_AR
        MOVE.W  #ACRTC_CCR_ABT|ACRTC_CCR_GBM4,ACRTC_DATA
        MOVE.W  #ACRTC_HSR,ACRTC_AR     ; r82〜r8A（AR は語ごとに 2 進む）
        MOVE.W  #ACRTC_B_HSR,ACRTC_DATA
        MOVE.W  #ACRTC_B_HDR,ACRTC_DATA
        MOVE.W  #ACRTC_B_VSR,ACRTC_DATA
        MOVE.W  #ACRTC_B_VDR,ACRTC_DATA
        MOVE.W  #ACRTC_B_SSW,ACRTC_DATA
        MOVE.W  #ACRTC_MWR1,ACRTC_AR    ; rCA〜rCE
        MOVE.W  #ACRTC_B_MW,ACRTC_DATA
        MOVE.W  #0,ACRTC_DATA
        MOVE.W  #0,ACRTC_DATA
        MOVE.W  #ACRTC_CCR,ACRTC_AR
        MOVE.W  #ACRTC_CCR_GBM4,ACRTC_DATA
        MOVE.W  #ACRTC_OMR,ACRTC_AR
        MOVE.W  #ACRTC_B_OMR,ACRTC_DATA
        LEA     acrtc_init_cmds(PC),A0
        MOVE.W  #(acrtc_init_cmds_end-acrtc_init_cmds)/2,D1
        BSR     bios_acrtc_send
        MOVEM.L (SP)+,D1/A0
        RTS

acrtc_init_cmds:
        DC.W    ACRTC_WPTN|0,16         ; パターン RAM を 0 に：COL＝00 の描画は CL0 の色になる
        DCB.W   16,0
        DC.W    ACRTC_WPR|ACRTC_P_MASK,$FFFF
        DC.W    ACRTC_ORG,$4000,$0000   ; DN＝1（基本画面の MWR）、語番地 0、ドット 0
        DC.W    ACRTC_WPR|ACRTC_P_CL0,$0000
        DC.W    ACRTC_WPR|ACRTC_P_CL1,$0000
acrtc_init_cmds_end:

; ------------------------------------------------------------------ FIFO

; (A0) から D1.W 語を FIFO に送る。出力：A0 は送った次の語。
bios_acrtc_send:
        MOVEM.L D0-D1/D7,-(SP)
        MOVE.W  #ACRTC_FIFO,ACRTC_AR
        SUBQ.W  #1,D1
        BMI.S   .done
.next   MOVE.W  (A0)+,D0
.wait   MOVE.W  ACRTC_SR,D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   .wait
        MOVE.W  D0,ACRTC_DATA
        DBRA    D1,.next
.done   MOVEM.L (SP)+,D0-D1/D7
        RTS

; D0.W を FIFO に書く（AR は r00 のはず）。
acrtc_put:
        MOVE.W  D7,-(SP)
.wait   MOVE.W  ACRTC_SR,D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   .wait
        MOVE.W  D0,ACRTC_DATA
        MOVE.W  (SP)+,D7
        RTS

; RWP を基本画面（DN＝1）の語番地 D0.L にする。
acrtc_rwp:
        MOVEM.L D0-D1,-(SP)
        MOVE.L  D0,D1
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPH,D0
        BSR     acrtc_put
        MOVE.L  D1,D0
        LSR.L   #8,D0
        LSR.L   #4,D0
        ANDI.W  #$00FF,D0
        ORI.W   #$4000,D0
        BSR     acrtc_put
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPL,D0
        BSR     acrtc_put
        MOVE.W  D1,D0
        LSL.W   #4,D0
        BSR     acrtc_put
        MOVEM.L (SP)+,D0-D1
        RTS

; ------------------------------------------------------------------ 画面

; 画面（256×192）を色 D0.W で消す。
bios_cls:
        MOVEM.L D0-D1,-(SP)
        MOVE.W  #ACRTC_FIFO,ACRTC_AR
        ANDI.W  #15,D0
        MULU    #$1111,D0
        MOVE.W  D0,D1
        MOVEQ   #0,D0
        BSR     acrtc_rwp
        MOVE.W  #ACRTC_CLR,D0
        BSR     acrtc_put
        MOVE.W  D1,D0
        BSR     acrtc_put
        MOVE.W  #SCREEN_W/4-1,D0        ; AX：64 語
        BSR     acrtc_put
        MOVE.W  #-(SCREEN_H-1),D0       ; AY：192 行、下へ
        BSR     acrtc_put
        MOVEM.L (SP)+,D0-D1
        RTS

; 描く色（CL0 と CL1）を D0.W（0〜15）にする。
set_color:
        MOVEM.L D0-D1,-(SP)
        ANDI.W  #15,D0
        MULU    #$1111,D0
        MOVE.W  D0,D1
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL0,D0
        BSR     acrtc_put
        MOVE.W  D1,D0
        BSR     acrtc_put
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL1,D0
        BSR     acrtc_put
        MOVE.W  D1,D0
        BSR     acrtc_put
        MOVEM.L (SP)+,D0-D1
        RTS

; 図形。画面座標で、D4.W が演算（ACRTC_OPM_*）。
; draw_frect：(D0,D1)〜(D2,D3) を塗る（両端を含む）。
draw_frect:
        MOVE.W  #ACRTC_AFRCT,-(SP)
        BRA.S   draw_two_points
; draw_line：(D0,D1) から (D2,D3) へ線（終点は描かない）。
draw_line:
        MOVE.W  #ACRTC_ALINE,-(SP)
draw_two_points:
        MOVE.L  D0,-(SP)
        MOVE.W  D0,-(SP)
        MOVE.W  #ACRTC_AMOVE,D0
        BSR     acrtc_put
        MOVE.W  (SP)+,D0
        BSR     acrtc_put
        MOVE.W  D1,D0
        NEG.W   D0
        BSR     acrtc_put
        MOVE.W  4(SP),D0                ; コマンド
        OR.W    D4,D0
        BSR     acrtc_put
        MOVE.W  D2,D0
        BSR     acrtc_put
        MOVE.W  D3,D0
        NEG.W   D0
        BSR     acrtc_put
        MOVE.L  (SP)+,D0
        ADDQ.L  #2,SP
        RTS

; draw_circle：中心 (D0,D1)、半径 D2 の円。
draw_circle:
        MOVE.L  D0,-(SP)
        MOVE.W  D0,-(SP)
        MOVE.W  #ACRTC_AMOVE,D0
        BSR     acrtc_put
        MOVE.W  (SP)+,D0
        BSR     acrtc_put
        MOVE.W  D1,D0
        NEG.W   D0
        BSR     acrtc_put
        MOVE.W  #ACRTC_CRCL,D0
        OR.W    D4,D0
        BSR     acrtc_put
        MOVE.W  D2,D0
        BSR     acrtc_put
        MOVE.L  (SP)+,D0
        RTS

; ------------------------------------------------------------------ 文字

; (A0) の文字列（JIS X 0201、0 で終わる、32 字まで）を、左上 (D0,D1) に、前景 D2・背景 D3 で描く。
; x は 4 の倍数。文字 ROM の字形（CHAR_ROM＋(符号×8＋行)×2）を CPU が読み、1 行 1 byte を 2 語
; （4 bpp の 8 画素）に広げ、1 本の DWT（AX＝2n−1、AY＝−7）で送る。
bios_puts:
        MOVEM.L D0-D7/A0-A3,-(SP)
        MOVE.W  #ACRTC_FIFO,ACRTC_AR
        ; 字の数
        MOVEA.L A0,A1
        MOVEQ   #0,D4
.len    TST.B   (A1)+
        BEQ.S   .counted
        ADDQ.W  #1,D4
        CMP.W   #32,D4
        BLO.S   .len
.counted
        TST.W   D4
        BEQ     .ret
        ; 色の語
        ANDI.W  #15,D2
        MULU    #$1111,D2
        ANDI.W  #15,D3
        MULU    #$1111,D3
        ; RWP＝y×64＋x/4
        MOVEQ   #0,D5
        MOVE.W  D1,D5
        LSL.L   #6,D5
        MOVEQ   #0,D6
        MOVE.W  D0,D6
        LSR.W   #2,D6
        ADD.L   D6,D5
        MOVE.L  D5,D0
        BSR     acrtc_rwp
        ; DWT
        MOVE.W  #ACRTC_DWT,D0
        BSR     acrtc_put
        MOVE.W  D4,D0
        ADD.W   D0,D0
        SUBQ.W  #1,D0
        BSR     acrtc_put
        MOVEQ   #-7,D0
        BSR     acrtc_put
        ; 8 行×字の数×2 語
        LEA     nibble_masks(PC),A2
        LEA     CHAR_ROM,A3
        MOVEQ   #0,D6                   ; 行×2
.row    MOVEA.L A0,A1
        MOVE.W  D4,D7
        SUBQ.W  #1,D7
.char   MOVEQ   #0,D5
        MOVE.B  (A1)+,D5
        LSL.W   #4,D5
        ADD.W   D6,D5
        MOVEQ   #0,D1
        MOVE.B  0(A3,D5.W),D1           ; 字形の 1 行（MSB が左）
        MOVE.W  D1,D5
        LSR.W   #4,D5
        ADD.W   D5,D5
        MOVE.W  0(A2,D5.W),D0           ; 左の 4 画素
        BSR.S   .emit
        ANDI.W  #15,D1
        ADD.W   D1,D1
        MOVE.W  0(A2,D1.W),D0           ; 右の 4 画素
        BSR.S   .emit
        DBRA    D7,.char
        ADDQ.W  #2,D6
        CMP.W   #16,D6
        BLO.S   .row
.ret    MOVEM.L (SP)+,D0-D7/A0-A3
        RTS
; D0＝画素のマスク → (マスク∧前景)∨(¬マスク∧背景) を FIFO へ
.emit   MOVE.W  D0,D5
        AND.W   D2,D0
        NOT.W   D5
        AND.W   D3,D5
        OR.W    D5,D0
        BRA     acrtc_put

; 字形の 4 bit（bit 3 が左）→ 4 画素の語のマスク（左の画素が下位ニブル）
nibble_masks:
        DC.W    $0000,$F000,$0F00,$FF00,$00F0,$F0F0,$0FF0,$FFF0
        DC.W    $000F,$F00F,$0F0F,$FF0F,$00FF,$F0FF,$0FFF,$FFFF

; D4.L の下位 D5.W 桁（1〜8）を 16 進で、bios_puts と同じ D0〜D3 で描く。
bios_put_hex:
        MOVEM.L D4-D6/A0,-(SP)
        MOVEQ   #0,D6
        MOVE.W  D5,D6
        BNE.S   .nz
        MOVEQ   #1,D6
.nz     CMP.W   #8,D6
        BLS.S   .ok
        MOVEQ   #8,D6
.ok     LEA     bios_hexbuf,A0
        CLR.B   0(A0,D6.W)
.digit  SUBQ.W  #1,D6
        BMI.S   .draw
        MOVE.W  D4,D5
        ANDI.W  #15,D5
        MOVE.B  hex_chars(PC,D5.W),0(A0,D6.W)
        LSR.L   #4,D4
        BRA.S   .digit
.draw   BSR     bios_puts
        MOVEM.L (SP)+,D4-D6/A0
        RTS
hex_chars:
        DC.B    "0123456789ABCDEF"
        EVEN
