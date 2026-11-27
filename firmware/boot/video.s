; ACRTC と文字（bios.s から INCLUDE）。
;
; 座標：BIOS は ORG を基本画面（DN＝1）の語番地 0、ドット 0 に置く。論理座標の +Y は画面の上なので、
; 画面の左上を (0,0)、下を +y とする画面座標から、ACRTC には y の符号を変えて渡す。
; 画面の語番地は y×64＋x/4。左の画素が語の下位ニブル（台帳 HD63484.md §4.5）。
;
; FIFO の規約（DR-016、DR-033）：書く前に SR の WFR を確かめる。SR の WFE（FIFO が空）を見たときは、8 語を
; 続けて書く（FIFO は 8 語。DR-094）。AR/SR とデータポートには読み書き一体の命令を使わない。ここの処理は、
; AR を r00（FIFO）にしたまま戻る。

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
        MOVE.W  #ACRTC_DCR,ACRTC_AR     ; 基本画面だけを出す。電源投入時の DCR は不定（DR-071）
        MOVE.W  #ACRTC_DCR_BASE,ACRTC_DATA
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

ACRTC_WATCH     EQU     40960           ; 空きを待って SR を読む回数の上限（1 回 約 78 クロック）

; (A0) から D1.W 語（0〜65535）を FIFO に送る。出力：A0 は送った次の語。CCR の Z＝1 ならこの呼出しの間に
; 打ち切らなかった、Z＝0 なら打ち切った（DR-094）。FIFO が空（WFE）なら 8 語を続けて書き、空でなく空きが
; ある（WFR）なら 1 語を書く。
bios_acrtc_send:
        MOVEM.L D1/D6-D7/A1,-(SP)
        MOVE.W  bios_acrtc_aborts,-(SP)
        LEA     ACRTC_DATA,A1
        MOVE.W  #ACRTC_FIFO,ACRTC_AR
        MOVEQ   #0,D7                   ; 続けて書いてよい語の数
        TST.W   D1
        BEQ.S   .done
        SUBQ.W  #1,D1
.next   SUBQ.W  #1,D7
        BPL.S   .put
        BSR.S   acrtc_room
        SUBQ.W  #1,D7
.put    MOVE.W  (A0)+,(A1)
        DBRA    D1,.next
.done   MOVE.W  (SP)+,D1
        CMP.W   bios_acrtc_aborts,D1    ; MOVEM と RTS は CCR を変えない
        MOVEM.L (SP)+,D1/D6-D7/A1
        RTS

; D0.W を FIFO に書く（AR は r00 のはず）。
acrtc_put:
        MOVE.L  D7,-(SP)
        BSR.S   acrtc_room
        MOVE.W  D0,ACRTC_DATA
        MOVE.L  (SP)+,D7
        RTS

; FIFO に続けて書いてよい語の数を D7 に返す：空（WFE）なら 8、空でなく空きがある（WFR）なら 1（FIFO は
; 8 語。台帳 HD63484.md）。D7 のほかは保つ。ベンドで書込みが抜けて（DR-079）コマンドの列がずれると、
; ACRTC はパラメタをコマンドとして読み、巨大な図形を描き続けるか、誤り（CER）で止まる（ABT まで動かない）。
; CER が立っていればすぐに、40960 回（工場出荷のクロックで約 0.4 秒）読んでも空きがなければ、打ち切って
; 続ける（DR-080、DR-082）。打ち切った後の FIFO は空なので 8。
acrtc_room:
        MOVE.W  D6,-(SP)
        MOVE.W  #ACRTC_WATCH-1,D6
.poll   MOVE.W  ACRTC_SR,D7
        BTST    #ACRTC_SR_CER_B,D7
        BNE.S   .abort
        BTST    #ACRTC_SR_WFE_B,D7
        BNE.S   .empty
        BTST    #ACRTC_SR_WFR_B,D7
        DBNE    D6,.poll
        BEQ.S   .abort                  ; 空きが来ない
        MOVEQ   #1,D7
        MOVE.W  (SP)+,D6
        RTS
.abort  BSR.S   acrtc_abort
.empty  MOVEQ   #8,D7
        MOVE.W  (SP)+,D6
        RTS

; コマンドを打ち切り（CCR の ABT。FIFO も空になる）、AR を r00 にする。CCR のほかのビット（アプリが
; 立てた DDM など）は保つ。D7（打ち切る前の SR）を bios_acrtc_abort_sr に残して数える。D7 を壊す。
acrtc_abort:
        ADDQ.W  #1,bios_acrtc_aborts
        MOVE.W  D7,bios_acrtc_abort_sr
        MOVE.W  #ACRTC_CCR,ACRTC_AR
        MOVE.W  ACRTC_DATA,D7
        ORI.W   #ACRTC_CCR_ABT,D7
        MOVE.W  #ACRTC_CCR,ACRTC_AR
        MOVE.W  D7,ACRTC_DATA
        ANDI.W  #$FFFF-ACRTC_CCR_ABT,D7
        MOVE.W  #ACRTC_CCR,ACRTC_AR
        MOVE.W  D7,ACRTC_DATA
        MOVE.W  #ACRTC_FIFO,ACRTC_AR
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

; 画面（256×192）を色 D0.W で消す。出力：Z（bios_acrtc_send と同じ。DR-094）。
bios_cls:
        MOVEM.L D0-D1,-(SP)
        MOVE.W  bios_acrtc_aborts,-(SP)
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
        MOVE.W  (SP)+,D1
        CMP.W   bios_acrtc_aborts,D1
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

PUTS_MAX        EQU     32              ; 1 回に描く字の数の上限

; (A0) の文字列（JIS X 0201、0 で終わる、32 字まで）を、左上 (D0,D1) に、前景 D2・背景 D3 で描く。x は 4 の
; 倍数。出力：Z（bios_acrtc_send と同じ。DR-094）。字ごとの字形の 1 行目の番地の差（符号×8×2）をスタックの
; 表にして、puts_draw で描く。
bios_puts:
        MOVEM.L D0-D7/A0-A4,-(SP)
        MOVE.W  bios_acrtc_aborts,-(SP)
        LEA     -4*PUTS_MAX(SP),SP
        MOVEA.L SP,A1
        MOVEQ   #0,D4                   ; 字の数
.len    MOVEQ   #0,D5
        MOVE.B  (A0)+,D5
        BEQ.S   puts_go
        LSL.W   #4,D5
        MOVE.L  D5,(A1)+
        ADDQ.W  #1,D4
        CMP.W   #PUTS_MAX,D4
        BLO.S   .len
puts_go:
        MOVEA.L SP,A1
        BSR     puts_draw
        LEA     4*PUTS_MAX(SP),SP
        MOVE.W  (SP)+,D0
        CMP.W   bios_acrtc_aborts,D0    ; LEA・MOVEM・RTS は CCR を変えない
        MOVEM.L (SP)+,D0-D7/A0-A4
        RTS

; (A0) の語の列（$00xx は JIS X 0201 の符号 xx、$KKTT は区 KK・点 TT。0 で終わる、32 字まで）を、bios_puts と
; 同じ D0〜D3 で描く。ROM にない区点は〓（2 区 14 点）で描く。出力：Z（DR-094）。
bios_puts_kuten:
        MOVEM.L D0-D7/A0-A4,-(SP)
        MOVE.W  bios_acrtc_aborts,-(SP)
        LEA     -4*PUTS_MAX(SP),SP
        MOVEA.L SP,A1
        MOVEQ   #0,D4
.len    MOVE.W  (A0)+,D5
        BEQ.S   puts_go
        BSR.S   glyph_offset
        MOVE.L  D5,(A1)+
        ADDQ.W  #1,D4
        CMP.W   #PUTS_MAX,D4
        BLO.S   .len
        BRA.S   puts_go

; D0.W の符号（bios_puts_kuten と同じ）→ A0＝字形の 1 行目の CPU 番地（行は +2 ずつ、上位レーン）。ROM に
; ない区点は〓の番地。出力：Z＝1 なら ROM にある、Z＝0 なら〓に替えた（DR-094）。
bios_glyph:
        MOVEM.L D5-D6,-(SP)
        MOVE.W  D0,D5
        BSR.S   glyph_offset
        LEA     CHAR_ROM,A0
        ADDA.L  D5,A0                   ; LEA・ADDA・MOVEM は CCR を変えない
        MOVEM.L (SP)+,D5-D6
        RTS

GETA_KU         EQU     2               ; 〓：ROM にない区点の代わり
GETA_TEN        EQU     14

; D5.W の符号 → D5.L＝字形の 1 行目の CHAR_ROM からの差（ROM の offset×2。仕様書 §08 の配置）。ROM にない
; 区点（9〜15 区、48 区から上、点が 1〜94 の外）は〓にして Z＝0、ROM にあれば Z＝1。D6 を壊す。
glyph_offset:
        MOVEQ   #0,D6
        MOVE.W  D5,D6
        LSR.W   #8,D6                   ; 区
        BEQ.S   .x0201
        ANDI.L  #$00FF,D5
        SUBQ.W  #1,D5                   ; 点−1
        CMP.W   #94,D5
        BHS.S   .geta                   ; 点が 0 か 95 以上
        CMP.W   #8,D6
        BLS.S   .frame                  ; 1〜8 区（非漢字）：枠は (区−1)×94＋点−1
        CMP.W   #16,D6
        BLO.S   .geta
        CMP.W   #47,D6
        BHI.S   .geta
        SUBQ.W  #7,D6                   ; 16〜47 区（第一水準）：8 区の後に続く
.frame  SUBQ.W  #1,D6
        MULU    #94,D6
        ADD.W   D6,D5                   ; 枠の番号（0〜3759）
        LSL.L   #4,D5                   ; ×8 byte、×2（上位レーン）
        ADDI.L  #$0800*2,D5             ; 非漢字の枠の始まり（ROM の $0800）
        CMP.W   D5,D5                   ; Z＝1
        RTS
.x0201  ANDI.L  #$00FF,D5
        LSL.W   #4,D5
        CMP.W   D5,D5
        RTS
.geta   MOVE.L  #($0800+((GETA_KU-1)*94+GETA_TEN-1)*8)*2,D5     ; Z＝0
        RTS

; D0.W を FIFO に書く。A4＝ACRTC_DATA、D7＝続けて書いてよい語の数（puts_draw の中で使う）。
fifo_emit:
        SUBQ.W  #1,D7
        BPL.S   .put
        BSR     acrtc_room
        SUBQ.W  #1,D7
.put    MOVE.W  D0,(A4)
        RTS

; (A1) の D4.W 個の長語（字形の 1 行目の CHAR_ROM からの差）の字を、左上 (D0,D1)、前景 D2・背景 D3 で、
; 1 本の DWT（AX＝2n−1、AY＝−7。RWP を左上）で送る。字形の 1 行 1 byte を表 glyph_masks で 2 語のマスクに
; 変え、背景 XOR ((前景 XOR 背景)∧マスク) を書く。FIFO が空なら 8 語を続けて書く（DR-094）。
; D0–D7/A0–A4 を壊す。
puts_draw:
        MOVE.W  #ACRTC_FIFO,ACRTC_AR
        TST.W   D4
        BEQ     .ret
        ANDI.W  #15,D2                  ; D2.L＝(前景 XOR 背景)×$11111111
        ANDI.W  #15,D3                  ; D3.L＝背景×$11111111
        EOR.W   D3,D2
        MULU    #$1111,D2
        MOVE.W  D2,D5
        SWAP    D2
        MOVE.W  D5,D2
        MULU    #$1111,D3
        MOVE.W  D3,D5
        SWAP    D3
        MOVE.W  D5,D3
        LEA     ACRTC_DATA,A4
        MOVEQ   #0,D7                   ; 続けて書いてよい語の数
        ; RWP＝y×64＋x/4（基本画面、DN＝1）
        MOVEQ   #0,D5
        MOVE.W  D1,D5
        LSL.L   #6,D5
        MOVEQ   #0,D6
        MOVE.W  D0,D6
        LSR.W   #2,D6
        ADD.L   D6,D5
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPH,D0
        BSR     fifo_emit
        MOVE.L  D5,D0
        LSR.L   #8,D0
        LSR.L   #4,D0
        ANDI.W  #$00FF,D0
        ORI.W   #$4000,D0
        BSR     fifo_emit
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPL,D0
        BSR     fifo_emit
        MOVE.W  D5,D0
        LSL.W   #4,D0
        BSR     fifo_emit
        ; DWT
        MOVE.W  #ACRTC_DWT,D0
        BSR     fifo_emit
        MOVE.W  D4,D0
        ADD.W   D0,D0
        SUBQ.W  #1,D0
        BSR     fifo_emit
        MOVEQ   #-7,D0
        BSR     fifo_emit
        ; 8 行×字の数×2 語
        LEA     glyph_masks,A2
        LEA     CHAR_ROM,A3             ; 行ごとに 2 進める
        SUBQ.W  #1,D4
        MOVEQ   #8-1,D6                 ; 行
.row    MOVEA.L A1,A0
        MOVE.W  D4,D5
.char   MOVE.L  (A0)+,D1
        MOVEQ   #0,D0
        MOVE.B  0(A3,D1.L),D0           ; 字形の 1 行（MSB が左）
        ADD.W   D0,D0
        ADD.W   D0,D0
        MOVE.L  0(A2,D0.W),D0           ; 上位の語が左の 4 画素、下位の語が右の 4 画素のマスク
        AND.L   D2,D0
        EOR.L   D3,D0                   ; (マスク∧前景)∨(¬マスク∧背景)
        SWAP    D0
        SUBQ.W  #1,D7
        BPL.S   .w1
        BSR     acrtc_room
        SUBQ.W  #1,D7
.w1     MOVE.W  D0,(A4)
        SWAP    D0
        SUBQ.W  #1,D7
        BPL.S   .w2
        BSR     acrtc_room
        SUBQ.W  #1,D7
.w2     MOVE.W  D0,(A4)
        DBRA    D5,.char
        ADDQ.L  #2,A3
        DBRA    D6,.row
.ret    RTS

; 字形の 1 行（8 bit、MSB が左）→ 8 画素のマスクの 2 語（長語の上位が左の 4 画素）。語の中は左の画素が
; 下位ニブル（台帳 HD63484.md §4.5）。256 項目、1 KiB。
glyph_masks:
GM      SET     0
        REPT    256
        DC.W    (((GM>>7)&1)*$000F)|(((GM>>6)&1)*$00F0)|(((GM>>5)&1)*$0F00)|(((GM>>4)&1)*$F000)
        DC.W    (((GM>>3)&1)*$000F)|(((GM>>2)&1)*$00F0)|(((GM>>1)&1)*$0F00)|((GM&1)*$F000)
GM      SET     GM+1
        ENDR

; D4.L の下位 D5.W 桁（1〜8）を 16 進で、bios_puts と同じ D0〜D3 で描く。出力：Z（bios_puts の Z のまま）。
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
