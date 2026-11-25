; effects.dsp：SAMPLE 2 の DSP（TMS32010）のプログラム。dspasm で組む（build.py）。説明は README.md。
;
; 1 標本ごとに：入力 FIFO（Data Fabric の port 3）から x[n]（Q15）を 1 語読み、3 つの出力を毎回すべて計算し、
; MODE で 1 つを選んで、本体 DAC の語（上位 L・下位 R、符号なし 8 bit）にして出力 FIFO（port 4）へ書く。
;   THRU  y＝x[n]
;   LPF   y＝(Σ h[k]·x[n−k]) >> 12。31 タップ、係数は Q12 の MPYK（build.py が設計して fir.inc に書く）
;   ECHO  w[n]＝(x[n]＋w[n−D]) >> 1、y＝w[n]。D＝3072 標本。遅延線はプログラム RAM（TBLR・TBLW）
; 遅延線（FIR の XN と、エコーのプログラム RAM）は MODE によらず毎回進める。どのモードの出力も、そこまでの入力
; だけで決まる（モデル model.py と標本ごとに比べられる）。
;
; 歩調：BIO＝出力 FIFO に空き（68000 が CTRL の BIOSEL＝1 にする）。空きがあり、入力があるときだけ 1 標本を処理
; する。出力 FIFO が満杯の間は待ち、その間もメールボックスを見る。処理の速さは、出力を読む側（本体 DAC の拍）
; が決める（DR-029 (3)）。
;
; 68000 との約束は protocol.inc、本機の I/O は dsp.inc。

        INCLUDE "dsp.inc"
        INCLUDE "protocol.inc"
        INCLUDE "fir.inc"               ; build.py が作る：NTAPS、H0〜H30

        ASSERT  NTAPS=31
        ASSERT  MODE_THRU=0
        ASSERT  MODE_LPF=1

; ------------------------------------------------------------------ データ RAM（ページ 0。DP＝0 のまま）

        RSSET   0
XN      RS      NTAPS                   ; FIR の遅延線：XN＋0 が x[n]、XN＋k が x[n−k]
STAT    RS      1                       ; IN 2 で読んだ状態
CMD     RS      1                       ; メールボックスの語
OPC     RS      1                       ; その上位バイト（コマンド）
ARG     RS      1                       ; 　　　下位バイト（引数）
TMP     RS      1
YL      RS      1                       ; LPF の出力
DL      RS      1                       ; エコー：遅延線から読んだ w[n−D]
W       RS      1                       ; エコー：w[n]
Y       RS      1                       ; 選んだ出力
OUTW    RS      1                       ; DAC の語
RPTR    RS      1                       ; エコーの遅延線の番地（RING_START〜RING_END−1）
CNT_HI  RS      1                       ; 処理した標本の数（32 bit）
CNT_LO  RS      1
SUM     RS      1                       ; 出力の語の和（下位 16 bit）
PEAK    RS      1                       ; 前の CMD_SNAP からの |y| の最大
; CMD_READ で返す並び（RD_* の順）
SNAPS   RS      0
S_CNT_HI RS     1
S_CNT_LO RS     1
S_SUM   RS      1
S_PEAK  RS      1
MODE    RS      1                       ; RD_MODE
KONST   RS      0                       ; ここから定数（KTAB を写す）。最初が RD_MAGIC
MAGIC   RS      1
ONE     RS      1
K_FF    RS      1
K_80    RS      1
K_CMD   RS      1
K_INPUT RS      1
K_RSTART RS     1
K_REND  RS      1
K_SNAPS RS      1
K_NREAD RS      1
K_NMODE RS      1
KEND    RS      0
DATA_END RS     0
        ASSERT  MODE=SNAPS+RD_MODE
        ASSERT  MAGIC=SNAPS+RD_MAGIC
        ASSERT  DATA_END<=128           ; ページ 0 に収める

; ------------------------------------------------------------------ ベクタ

        ORG     DSP_VEC_RESET
        B       init
        ORG     DSP_VEC_INT
intr    RET                             ; INT は使わない（EINT しないので来ない）

; 定数の表（init が TBLR でデータ RAM の KONST〜に写す）。LACK で番地を作るので 255 番地までに置く
KTAB    DATA    DSP_MAGIC
        DATA    1
        DATA    $00FF
        DATA    $0080
        DATA    DSP_IS_COMMAND
        DATA    DSP_IS_INPUT
        DATA    RING_START
        DATA    RING_END
        DATA    SNAPS
        DATA    RD_COUNT
        DATA    MODE_COUNT
KTAB_END
        ASSERT  KTAB_END-KTAB=KEND-KONST
        ASSERT  KTAB_END<=256

; ------------------------------------------------------------------ 初期化（台帳 §4.4）

init    DINT
        ROVM                            ; あふれは回り込み（設計の上で起きない。test_build.py が確かめる）
        LDPK    0
        LARP    AR0
        LARK    AR0,DSP_DATA_WORDS-1    ; データ RAM を 143 番地から 0 番地まで 0 にする
        ZAC
.clear  SACL    *
        BANZ    .clear
        LACK    1
        SACL    ONE
        LARK    AR0,KONST               ; 定数を写す：AR0 が写し先、AR1 が数
        LARK    AR1,KEND-KONST-1
        LACK    KTAB
.konst  LARP    AR0
        TBLR    *+,AR1                  ; data[AR0++] ← prog[ACC]、次は AR1
        ADD     ONE
        BANZ    .konst                  ; AR1 が 0 になるまで
        LARP    AR0
        LAC     K_RSTART
        SACL    RPTR                    ; MODE は 0（THRU）、数・和・山は 0

; ------------------------------------------------------------------ 待つ

idle    IN      STAT,DSP_PA_STATUS
        LAC     STAT
        AND     K_CMD
        BNZ     command                 ; メールボックスに着信
        BIOZ    .space                  ; BIO：出力 FIFO に空き
        B       idle
.space  LAC     STAT
        AND     K_INPUT
        BZ      idle                    ; 入力がまだない

; ------------------------------------------------------------------ 1 標本

        IN      XN,DSP_PA_INPUT         ; x[n]（Q15。Fabric が 8 bit の符号なしから作る）

; LPF：古い方から LTD（T に入れ、前の積を足し、1 つ後ろへ写す）と MPYK（係数は Q12）を 30 回（T1 5-16 の形）
        ZAC
        LT      XN+30
        MPYK    H30
        LTD     XN+29
        MPYK    H29
        LTD     XN+28
        MPYK    H28
        LTD     XN+27
        MPYK    H27
        LTD     XN+26
        MPYK    H26
        LTD     XN+25
        MPYK    H25
        LTD     XN+24
        MPYK    H24
        LTD     XN+23
        MPYK    H23
        LTD     XN+22
        MPYK    H22
        LTD     XN+21
        MPYK    H21
        LTD     XN+20
        MPYK    H20
        LTD     XN+19
        MPYK    H19
        LTD     XN+18
        MPYK    H18
        LTD     XN+17
        MPYK    H17
        LTD     XN+16
        MPYK    H16
        LTD     XN+15
        MPYK    H15
        LTD     XN+14
        MPYK    H14
        LTD     XN+13
        MPYK    H13
        LTD     XN+12
        MPYK    H12
        LTD     XN+11
        MPYK    H11
        LTD     XN+10
        MPYK    H10
        LTD     XN+9
        MPYK    H9
        LTD     XN+8
        MPYK    H8
        LTD     XN+7
        MPYK    H7
        LTD     XN+6
        MPYK    H6
        LTD     XN+5
        MPYK    H5
        LTD     XN+4
        MPYK    H4
        LTD     XN+3
        MPYK    H3
        LTD     XN+2
        MPYK    H2
        LTD     XN+1
        MPYK    H1
        LTD     XN+0
        MPYK    H0
        APAC
        SACH    YL,4                    ; (ACC << 4) >> 16 ＝ ACC >> 12

; ECHO：遅延線は RING_START〜RING_END−1。同じ番地を読んでから書くので、遅れは RING_END−RING_START
        LAC     RPTR
        TBLR    DL                      ; w[n−D]
        LAC     XN,15
        ADD     DL,15
        SACH    W                       ; (x＋w[n−D]) >> 1
        LAC     RPTR
        TBLW    W                       ; RPTR は 8 番地より上（OUT と重ならない）
        ADD     ONE
        SACL    RPTR
        SUB     K_REND
        BLZ     .inring
        LAC     K_RSTART
        SACL    RPTR
.inring

; 選ぶ
        LAC     MODE
        BZ      .thru
        SUB     ONE
        BZ      .lpf
        LAC     W
        B       .chosen
.thru   LAC     XN
        B       .chosen
.lpf    LAC     YL
.chosen SACL    Y

; DAC の語：u＝(y >> 8)＋128（0 は 1 にする。DOC の波形 RAM では 0 が止める値）、語＝u·257
        LAC     Y,8
        SACH    TMP                     ; y >> 8（符号付き）
        LAC     TMP
        AND     K_FF
        XOR     K_80
        BNZ     .nonzero
        LACK    1
.nonzero
        SACL    TMP
        LAC     TMP,8
        ADD     TMP
        SACL    OUTW
        OUT     OUTW,DSP_PA_OUTPUT      ; 空きは BIO で確かめてある

; 数える
        ZALS    CNT_LO
        ADDH    CNT_HI
        ADD     ONE
        SACL    CNT_LO
        SACH    CNT_HI
        LAC     SUM
        ADD     OUTW
        SACL    SUM
        LAC     Y
        ABS
        SUBS    PEAK                    ; 符号なしで比べる（|y| は 32768 まで）
        BLEZ    idle
        ADDS    PEAK
        SACL    PEAK
        B       idle

; ------------------------------------------------------------------ コマンド（protocol.inc）

command IN      CMD,DSP_PA_MAILBOX      ; 読むと COMMAND が下がる
        LAC     CMD,8
        SACH    OPC
        LAC     OPC
        AND     K_FF
        SACL    OPC
        LAC     CMD
        AND     K_FF
        SACL    ARG
        LAC     OPC
        SUB     ONE
        BZ      cmd_mode
        SUB     ONE
        BZ      cmd_snap
        SUB     ONE
        BZ      cmd_read
error   ZAC
        SUB     ONE                     ; RESP_ERROR（$FFFF）
reply   SACL    TMP
        OUT     TMP,DSP_PA_RESPONSE
        B       idle

cmd_mode
        LAC     ARG
        SUB     K_NMODE
        BGEZ    error
        LAC     ARG
        SACL    MODE
        LAC     CNT_LO                  ; 次に処理する標本から新しいモード
        B       reply

cmd_snap
        LAC     CNT_HI
        SACL    S_CNT_HI
        LAC     CNT_LO
        SACL    S_CNT_LO
        LAC     SUM
        SACL    S_SUM
        LAC     PEAK
        SACL    S_PEAK
        ZAC
        SACL    PEAK
        LAC     S_CNT_LO
        B       reply

cmd_read
        LAC     ARG
        SUB     K_NREAD
        BGEZ    error
        LAC     ARG
        ADD     K_SNAPS
        SACL    TMP
        LAR     AR0,TMP
        LAC     *                       ; data[SNAPS＋引数]（ARP は 0）
        B       reply

code_end
        ASSERT  code_end<=RING_START    ; エコーの遅延線と重ならない
        ASSERT  RING_START>=DSP_TBLW_MIN
        END
