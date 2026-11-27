; BIOS の約束（sdk/docs/bios-abi.md、DR-094）を確かめる試験のカートリッジ。firmware/boot/build.py が
; build/abitest.bin（$400000 から）に組む。結果を Main RAM の APP_RAM からの表に書き、board の試験
; console_boot.rs の the_bios_keeps_its_promises が読む。画面と色は試験用で、本機の UI の意匠ではない。
;
; 順：入口の値を残す → USP に印 → レベル 6 の項目を差し替え（BIOS の処理へつなぐ）、フックを置く →
; 呼出しごとにレジスタを保つか → 打切りの返し方 → BIOS_GLYPH → 時間（VBlank の直後からのライン数）→
; 照合用の字を描く → 割込みを受けながら 3 フレームの間レジスタを保つか → 終わりの印

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"

CASES           EQU     9               ; 呼出しの表の項目の数
GLYPH_TESTS     EQU     6
USP_MARK        EQU     $5A5AA5A5
DONE_MARK       EQU     $600D

; 照合用の字の位置（console_boot.rs と同じ）
CHECK_X         EQU     16
CHECK_PUTS_Y    EQU     8
CHECK_KUTEN_Y   EQU     24

; ------------------------------------------------------------------ 結果（APP_RAM から。番地は試験が使う）

        RSSET   APP_RAM
r_done          RS.W    1               ; +$00 終わったら $600D
r_version       RS.W    1               ; +$02 BIOS_INFO_VERSION
r_entry_d0      RS.L    1               ; +$04 入口の D0
r_entry_status  RS.W    1               ; +$08 入口で読んだ INPUT_IRQ_STATUS
r_regs_bad      RS.W    1               ; +$0A レジスタが約束と違った呼出し（bit n が表の n 番目）
r_sp_bad        RS.W    1               ; +$0C SP が戻らなかった呼出し
r_spin_bad      RS.W    1               ; +$0E 割込みを受けながら 3 フレーム回して、変わったレジスタの数
r_usp           RS.L    1               ; +$10 終わりの USP（USP_MARK のはず）
r_send_ok_ccr   RS.W    1               ; +$14 正しい列の BIOS_ACRTC_SEND の後の SR
r_send_bad_ccr  RS.W    1               ; +$16 誤りの列の後の SR
r_aborts        RS.W    1               ; +$18 誤りの列で増えた bios_acrtc_aborts
r_abort_sr      RS.W    1               ; +$1A bios_acrtc_abort_sr
r_irq6          RS.W    1               ; +$1C アプリのレベル 6 の処理に入った数
r_hook          RS.W    1               ; +$1E VBlank のフックの数
r_lines         RS.W    6               ; +$20 ライン数：PUTS 1 字、PUTS 10 字、PUT_HEX 6 桁、その描画が
                                        ;      終わるまで、PUTS_KUTEN 6 字、ACRTC_SEND 64 語
r_glyph_a0      RS.L    GLYPH_TESTS     ; +$2C BIOS_GLYPH の A0
r_glyph_sr      RS.W    GLYPH_TESTS     ; +$44 BIOS_GLYPH の後の SR
r_case_sr       RS.W    CASES           ; +$50 呼出しごとの後の SR
r_end           RS.B    0
        ASSERT  r_lines=APP_RAM+$20
        ASSERT  r_glyph_a0=APP_RAM+$2C
        ASSERT  r_glyph_sr=APP_RAM+$44
        ASSERT  r_case_sr=APP_RAM+$50

; 作業の変数
        RSSET   APP_RAM+$100
regs_in         RS.L    15              ; D0–D7/A0–A6
regs_out        RS.L    15              ; 戻った値
regs_want       RS.L    15              ; 約束の値（regs_out のすぐ後）
bad_out         RS.L    30              ; 最初に違った呼出しの regs_out と regs_want（APP_RAM＋$1B4）
v_entry         RS.L    1
v_sp            RS.L    1
v_save          RS.L    2               ; D7/A5
v_irq6_prev     RS.L    1
v_vars_end      RS.B    0

; ------------------------------------------------------------------ 見出し（bios-abi.md §04）

        ORG     CART
        DC.L    CART_MAGIC
        DC.W    CART_HDR_VERSION
        DC.W    0
        DC.L    start
        DC.L    rom_end-CART
        DC.B    "ABI TEST        "
        ASSERT  *=CART+cart_header_size

; ------------------------------------------------------------------ 入口

start:
        MOVE.L  D0,r_entry_d0
        MOVE.W  INPUT_IRQ_STATUS,r_entry_status
        MOVE.W  BIOS_INFO_VERSION,r_version
        CLR.W   r_done
        LEA     r_regs_bad,A0
        MOVE.W  #(r_end-r_regs_bad)/2-1,D0
.clr    CLR.W   (A0)+
        DBRA    D0,.clr
        ; USP に印。BIOS は USP を使わない（bios-abi.md §03）
        MOVE.L  #USP_MARK,D0
        MOVEA.L D0,A0
        MOVE    A0,USP
        ; レベル 6 の項目を差し替えて BIOS の処理へつなぎ、フックを置く
        MOVE.W  #$2700,SR
        MOVE.L  JUMP_TABLE+6*(VEC_AUTO1+IRQ_VIDEO-1)+2,v_irq6_prev
        MOVE.L  #irq6,JUMP_TABLE+6*(VEC_AUTO1+IRQ_VIDEO-1)+2
        MOVE.L  #hook,bios_vbl_hook
        MOVE.W  #$2000,SR
        ; 版 1 の BIOS では、版 1 にある呼出しの時間だけを測る（DR-094 の前と後の比べ。bios-abi.md §11 の確かめ方）
        CMPI.W  #2,BIOS_INFO_VERSION
        BHS.S   .v2
        BSR     timing
        BRA.S   .end
.v2     BSR     run_cases
        BSR     abort_test
        BSR     glyph_test
        BSR     timing
        BSR     check_draw
        BSR     spin
.end    MOVE    USP,A0
        MOVE.L  A0,r_usp
        MOVE.W  #DONE_MARK,r_done
.idle   JSR     BIOS_WAIT_VBL
        BRA.S   .idle

; ------------------------------------------------------------------ 割込み

; レベル 6：数えて、入ったときのレジスタとスタックのまま、前の項目（BIOS の処理）へつなぐ。
irq6:
        ADDQ.W  #1,r_irq6
        MOVE.L  v_irq6_prev,-(SP)
        RTS

; VBlank のフック：数えて、壊してよい D0–D1/A0–A1 を壊す。
hook:
        ADDQ.W  #1,r_hook
        MOVEQ   #-1,D0
        MOVEQ   #-1,D1
        MOVEA.L D0,A0
        MOVEA.L D0,A1
        RTS

; ------------------------------------------------------------------ 呼出しごとにレジスタを保つか

; 表の項目：入口（L。0 で終わり）、D0–D7/A0–A6 に入れる値（15 L）、出力の（番号 W、値 L）の並び（−1 で終わり）。
; 全部のレジスタを入れて呼び、戻った値を、入れた値に出力を重ねたものと比べる。
run_cases:
        LEA     cases,A5
        MOVEQ   #0,D7                   ; 表の何番目か
.next   MOVE.L  (A5)+,D0
        BEQ     .end
        MOVE.L  D0,v_entry
        LEA     regs_in,A0
        LEA     regs_want,A1
        MOVEQ   #15-1,D0
.copy   MOVE.L  (A5),(A0)+
        MOVE.L  (A5)+,(A1)+
        DBRA    D0,.copy
        LEA     regs_want,A1
.outs   MOVE.W  (A5)+,D0
        BMI.S   .call
        ADD.W   D0,D0
        ADD.W   D0,D0
        MOVE.L  (A5)+,0(A1,D0.W)
        BRA.S   .outs
.call   MOVEM.L D7/A5,v_save
        MOVE.L  SP,v_sp
        PEA     .back(PC)
        MOVE.L  v_entry,-(SP)
        MOVEM.L regs_in,D0-D7/A0-A6
        RTS                             ; 入口へ（戻る先は .back）
.back   MOVE.W  SR,-(SP)
        MOVEM.L D0-D7/A0-A6,regs_out
        MOVE.W  (SP)+,D0
        MOVEM.L v_save,D7/A5
        LEA     r_case_sr,A0
        MOVE.W  D7,D1
        ADD.W   D1,D1
        MOVE.W  D0,0(A0,D1.W)
        CMPA.L  v_sp,SP
        BEQ.S   .sp_ok
        MOVEA.L v_sp,SP
        MOVE.W  r_sp_bad,D0
        BSET    D7,D0
        MOVE.W  D0,r_sp_bad
.sp_ok  LEA     regs_out,A0
        LEA     regs_want,A1
        MOVEQ   #15-1,D0
.cmp    CMPM.L  (A0)+,(A1)+
        DBNE    D0,.cmp
        BEQ.S   .same
        MOVE.W  r_regs_bad,D0
        BNE.S   .mark
        LEA     regs_out,A0             ; 最初に違った呼出しのレジスタを残す（試験が誤りの文に出す）
        LEA     bad_out,A1
        MOVEQ   #30-1,D1
.keep   MOVE.L  (A0)+,(A1)+
        DBRA    D1,.keep
.mark   BSET    D7,D0
        MOVE.W  D0,r_regs_bad
.same   ADDQ.W  #1,D7
        BRA     .next
.end    RTS

PD0     EQU     $D0D0D0D0
PD1     EQU     $D1D1D1D1
PD2     EQU     $D2D2D2D2
PD3     EQU     $D3D3D3D3
PD4     EQU     $D4D4D4D4
PD5     EQU     $D5D5D5D5
PD6     EQU     $D6D6D6D6
PD7     EQU     $D7D7D7D7
PA0     EQU     $A0A0A0A0
PA1     EQU     $A1A1A1A1
PA2     EQU     $A2A2A2A2
PA3     EQU     $A3A3A3A3
PA4     EQU     $A4A4A4A4
PA5     EQU     $A5A5A5A5
PA6     EQU     $A6A6A6A6
RA0     EQU     8                       ; 出力の番号：A0

cases:
        ; 0：BIOS_WAIT_VBL
        DC.L    BIOS_WAIT_VBL
        DC.L    PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PA0,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    -1
        ; 1：BIOS_ACRTC_SEND（A0 は次の語）
        DC.L    BIOS_ACRTC_SEND
        DC.L    PD0,$D1D10002,PD2,PD3,PD4,PD5,PD6,PD7,cmd_ok,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    RA0
        DC.L    cmd_ok+4
        DC.W    -1
        ; 2：BIOS_CLS（色 0。上位の語は見ない）
        DC.L    BIOS_CLS
        DC.L    $D0D00000,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PA0,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    -1
        ; 3：BIOS_PUTS（x 16、y 160、前景 15、背景 1）
        DC.L    BIOS_PUTS
        DC.L    $D0D00010,$D1D100A0,$D2D2000F,$D3D30001,PD4,PD5,PD6,PD7
        DC.L    s_case,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    -1
        ; 4：BIOS_PUT_HEX（6 桁、y 168）
        DC.L    BIOS_PUT_HEX
        DC.L    $D0D00010,$D1D100A8,$D2D2000F,$D3D30001,$00ABCDEF,$D5D50006,PD6,PD7
        DC.L    PA0,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    -1
        ; 5：BIOS_SET_PAL
        DC.L    BIOS_SET_PAL
        DC.L    PD0,PD1,PD2,PD3,PD4,PD5,PD6,PD7,palette,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    -1
        ; 6：BIOS_READ_PAD（パッドは押していない：D0＝0、D1＝0）
        DC.L    BIOS_READ_PAD
        DC.L    0,0,PD2,PD3,PD4,PD5,PD6,PD7,PA0,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    -1
        ; 7：BIOS_PUTS_KUTEN（y 176）
        DC.L    BIOS_PUTS_KUTEN
        DC.L    $D0D00010,$D1D100B0,$D2D2000E,$D3D30002,PD4,PD5,PD6,PD7
        DC.L    k_case,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    -1
        ; 8：BIOS_GLYPH（16 区 1 点 → 文字 ROM の $1F80）
        DC.L    BIOS_GLYPH
        DC.L    $D0D01001,PD1,PD2,PD3,PD4,PD5,PD6,PD7,PA0,PA1,PA2,PA3,PA4,PA5,PA6
        DC.W    RA0
        DC.L    CHAR_ROM+$1F80*2
        DC.W    -1
        DC.L    0
        ASSERT  CASES=9

; ------------------------------------------------------------------ 打切りの返し方（DR-080、DR-082、DR-094）

; ACRTC が止まって FIFO が空のときに、未定義のコマンド（$0000）を 8 語送ると、ACRTC は CER を立てて止まる。
; 9 語目の前に空きを待つところで BIOS が打ち切り、残りの 2 語（正しい WPR）を送る。続けて正しい列を送ると、
; 打ち切らない。
abort_test:
        BSR     acrtc_idle
        MOVE.W  bios_acrtc_aborts,D2
        LEA     cmd_bad,A0
        MOVEQ   #(cmd_bad_end-cmd_bad)/2,D1
        JSR     BIOS_ACRTC_SEND
        MOVE.W  SR,r_send_bad_ccr
        MOVE.W  bios_acrtc_aborts,D0
        SUB.W   D2,D0
        MOVE.W  D0,r_aborts
        MOVE.W  bios_acrtc_abort_sr,r_abort_sr
        LEA     cmd_ok,A0
        MOVEQ   #(cmd_ok_end-cmd_ok)/2,D1
        JSR     BIOS_ACRTC_SEND
        MOVE.W  SR,r_send_ok_ccr
        RTS

; ------------------------------------------------------------------ BIOS_GLYPH

glyph_test:
        LEA     glyph_codes,A2
        LEA     r_glyph_a0,A3
        LEA     r_glyph_sr,A4
        MOVEQ   #GLYPH_TESTS-1,D7
.next   MOVE.W  (A2)+,D0
        JSR     BIOS_GLYPH
        MOVE.W  SR,(A4)+
        MOVE.L  A0,(A3)+
        DBRA    D7,.next
        RTS

; A、16 区 1 点、47 区 94 点、9 区 20 点（ROM にない）、48 区 1 点（ない）、1 区 0 点（ない）
glyph_codes:
        DC.W    $0041,$1001,$2F5E,$0914,$3001,$0100

; ------------------------------------------------------------------ 時間

; VBlank の割込みの直後から、呼出しが戻るまでのライン数（VIDEO_LINE の差）を測る。
timing:
        LEA     r_lines,A5
        ; PUTS 1 字
        BSR     vbl_mark
        LEA     s_one,A0
        BSR     puts_at_128
        BSR     lines_since
        ; PUTS 10 字
        BSR     vbl_mark
        LEA     s_ten,A0
        BSR     puts_at_128
        BSR     lines_since
        ; PUT_HEX 6 桁と、その描画が終わるまで
        BSR     vbl_mark
        MOVE.L  #$123456,D4
        MOVEQ   #6,D5
        MOVEQ   #CHECK_X,D0
        MOVE.W  #136,D1
        MOVEQ   #15,D2
        MOVEQ   #1,D3
        JSR     BIOS_PUT_HEX
        BSR     lines_since
        BSR     acrtc_idle
        BSR     lines_since
        ; PUTS_KUTEN 6 字（版 2 から）
        CMPI.W  #2,BIOS_INFO_VERSION
        BHS.S   .kuten
        ADDQ.L  #2,A5
        BRA.S   .send
.kuten  BSR     vbl_mark
        LEA     k_six,A0
        MOVEQ   #CHECK_X,D0
        MOVE.W  #144,D1
        MOVEQ   #15,D2
        MOVEQ   #1,D3
        JSR     BIOS_PUTS_KUTEN
        BSR     lines_since
        ; ACRTC_SEND 64 語（WPR を 32 回）
.send   BSR     vbl_mark
        LEA     cmd_64,A0
        MOVEQ   #64,D1
        JSR     BIOS_ACRTC_SEND
        BSR     lines_since
        RTS

; FIFO が空でコマンドが終わる（SR の CED・WFE）まで待つ。D0 を壊す。
acrtc_idle:
        MOVE.W  ACRTC_SR,D0
        ANDI.W  #ACRTC_SR_CED|ACRTC_SR_WFE,D0
        CMPI.W  #ACRTC_SR_CED|ACRTC_SR_WFE,D0
        BNE.S   acrtc_idle
        RTS

puts_at_128:
        MOVEQ   #CHECK_X,D0
        MOVE.W  #128,D1
        MOVEQ   #15,D2
        MOVEQ   #1,D3
        JMP     BIOS_PUTS

; ACRTC が前のコマンド（起動の画面の消去など）を終えてから、次の VBlank を待ち、そのときの VIDEO_LINE を
; D6 に置く。
vbl_mark:
        BSR     acrtc_idle
        JSR     BIOS_WAIT_VBL
        MOVE.W  VIDEO_LINE,D6
        RTS

; D6 からのライン数（0〜261）を (A5)+ に書く。
lines_since:
        MOVE.W  VIDEO_LINE,D0
        SUB.W   D6,D0
        BPL.S   .pos
        ADDI.W  #262,D0
.pos    MOVE.W  D0,(A5)+
        RTS

; ------------------------------------------------------------------ 照合用の字

; 試験が VRAM を文字 ROM から作った語と比べる（console_boot.rs）。
check_draw:
        LEA     s_check,A0
        MOVEQ   #CHECK_X,D0
        MOVEQ   #CHECK_PUTS_Y,D1
        MOVEQ   #15,D2
        MOVEQ   #1,D3
        JSR     BIOS_PUTS
        LEA     k_check,A0
        MOVEQ   #CHECK_X,D0
        MOVEQ   #CHECK_KUTEN_Y,D1
        MOVEQ   #3,D2
        MOVEQ   #0,D3
        JMP     BIOS_PUTS_KUTEN

; ------------------------------------------------------------------ 割込みを受けながらレジスタを保つか

; D0–D6/A0–A6 に値を入れ、D7 だけで 3 フレーム回る。その間に VBlank の割込み（アプリの処理 → BIOS の処理 →
; フック）が入る。
spin:
        MOVEM.L spin_regs,D0-D6/A0-A6
        MOVE.L  bios_frame,D7
        ADDQ.L  #3,D7
.spin   CMP.L   bios_frame,D7
        BHI.S   .spin
        MOVEM.L D0-D6/A0-A6,regs_out
        LEA     regs_out,A0
        LEA     spin_regs,A1
        MOVEQ   #0,D1
        MOVEQ   #14-1,D0
.cmp    CMPM.L  (A0)+,(A1)+
        BEQ.S   .same
        ADDQ.W  #1,D1
.same   DBRA    D0,.cmp
        MOVE.W  D1,r_spin_bad
        RTS

spin_regs:
        DC.L    PD0,PD1,PD2,PD3,PD4,PD5,PD6,PA0,PA1,PA2,PA3,PA4,PA5,PA6

; ------------------------------------------------------------------ データ

cmd_ok:
        DC.W    ACRTC_WPR|ACRTC_P_MASK,$FFFF
cmd_ok_end:
cmd_bad:
        DCB.W   8,$0000                 ; 未定義のコマンド
        DC.W    ACRTC_WPR|ACRTC_P_MASK,$FFFF
cmd_bad_end:
cmd_64:
        REPT    32
        DC.W    ACRTC_WPR|ACRTC_P_MASK,$FFFF
        ENDR

palette:
        DC.W    $000,$013,$247,$F44,$F84,$FE4,$4D4,$4CE
        DC.W    $48F,$A6F,$F6C,$888,$CCC,$842,$FFA,$FFF

s_case:         DC.B    "CASE",0
s_one:          DC.B    "A",0
s_ten:          DC.B    "ABCDEFGHIJ",0
s_check:        DC.B    "A1",$B1,"Z ",0        ; A、1、ｱ、Z、空白
        EVEN
k_case:         DC.W    $1001,$0041,0
k_six:          DC.W    $1001,$1002,$1003,$1004,$1005,$1006,0
k_check:        DC.W    $0041,$1001,$0914,$2422,$0100,0     ; A、16 区 1 点、〓（9 区）、4 区 2 点、〓（点 0）
        EVEN
rom_end:
