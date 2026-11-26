; xform.dsp：SAMPLE 6 の DSP（TMS32010）のプログラム。dspasm で組む（build.py）。説明は README.md §04。
;
; 1 フレームに 1 回、入力 FIFO（Data Fabric の port 3）から塊（protocol.inc）を読み、頂点を X・Y・Z 軸の順に
; 回して透視し、画面の座標を出力 FIFO（port 4）へ書く。最後に尾の語（番号つき）を書く。
; 正弦と透視の表はプログラム RAM に置き、TBLR で引く（build.py が作る dsptab.inc）。
;
; 待ち：BIO＝入力 FIFO が空でない（68000 が CTRL の BIOSEL＝0 にする）。入力の語は BIOZ で確かめてから IN で
; 読む。出力は状態ポート（IN 2）の「出力に空き」を確かめてから OUT で書く（DR-017：DSP は待てない）。
; 頭の語の印（HEAD_MAGIC）が合わなければ、合うまで読み捨てる（68000 がポートを初期化したあとの同期）。
;
; 68000 との約束は protocol.inc、本機の I/O は dsp.inc。

        INCLUDE "dsp.inc"
        INCLUDE "protocol.inc"

; ------------------------------------------------------------------ データ RAM（ページ 0。DP＝0 のまま）

        RSSET   0
W       RS      1                       ; 入力から読んだ語
STAT    RS      1                       ; IN 2 で読んだ状態
OUTV    RS      1                       ; 出力に書く語
SEQ     RS      1                       ; 塊の番号
ANGA    RS      1
ANGB    RS      1
ANGC    RS      1
NV      RS      1                       ; 頂点の数
CNT     RS      1
SA      RS      1                       ; 正弦・余弦（Q15）
CA      RS      1
SB      RS      1
CB      RS      1
SC      RS      1
CC      RS      1
VX      RS      1                       ; 頂点
VY      RS      1
VZ      RS      1
Y1      RS      1                       ; 途中の値
Z1      RS      1
X2      RS      1
Z2      RS      1
X3      RS      1
Y3      RS      1
K       RS      1                       ; 透視の倍率（Q15）
PX      RS      1
PY      RS      1
KONST   RS      0                       ; ここから定数（KTAB を写す）
ONE     RS      1
K_FF    RS      1
K_64    RS      1
K_128   RS      1
K_HI    RS      1
K_HEAD  RS      1
K_TAIL  RS      1
K_SPACE RS      1
K_SIN   RS      1
K_PERSP RS      1
K_CX    RS      1
K_CY    RS      1
K_MAXV  RS      1
KEND    RS      0
DATA_END RS     0
        ASSERT  DATA_END<=128           ; ページ 0 に収める

; ------------------------------------------------------------------ ベクタ

        ORG     DSP_VEC_RESET
        B       init
        ORG     DSP_VEC_INT
intr    RET                             ; INT は使わない（EINT しないので来ない）

; 定数の表（init が TBLR でデータ RAM の KONST〜に写す）。LACK で番地を作るので 255 番地までに置く
KTAB    DATA    1
        DATA    $00FF
        DATA    64
        DATA    128
        DATA    $FF00
        DATA    HEAD_MAGIC<<8
        DATA    TAIL_MAGIC<<8
        DATA    DSP_IS_SPACE
        DATA    SINTAB
        DATA    PERSP
        DATA    CX
        DATA    CY
        DATA    MAX_VERTS
KTAB_END
        ASSERT  KTAB_END-KTAB=KEND-KONST
        ASSERT  KTAB_END<=256

; ------------------------------------------------------------------ 初期化（台帳 §4.4）

init    DINT
        ROVM                            ; あふれは回り込み（設計の上で起きない）
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
        BANZ    .konst
        LARP    AR1                     ; 頂点の数え（BANZ）は AR1

; ------------------------------------------------------------------ 1 フレーム

frame   CALL    getw                    ; 頭の語を探す
        LAC     W
        AND     K_HI
        SUB     K_HEAD
        BNZ     frame                   ; 頭でなければ読み捨てる
        LAC     W
        AND     K_FF
        SACL    SEQ
        CALL    getw
        LAC     W
        SACL    ANGA
        CALL    getw
        LAC     W
        SACL    ANGB
        CALL    getw
        LAC     W
        SACL    ANGC
        CALL    getw
        LAC     W
        SACL    NV
; 正弦と余弦：s＝SIN[a&255]、c＝SIN[(a＋64)&255]
        LAC     ANGA
        AND     K_FF
        ADD     K_SIN
        TBLR    SA
        LAC     ANGA
        ADD     K_64
        AND     K_FF
        ADD     K_SIN
        TBLR    CA
        LAC     ANGB
        AND     K_FF
        ADD     K_SIN
        TBLR    SB
        LAC     ANGB
        ADD     K_64
        AND     K_FF
        ADD     K_SIN
        TBLR    CB
        LAC     ANGC
        AND     K_FF
        ADD     K_SIN
        TBLR    SC
        LAC     ANGC
        ADD     K_64
        AND     K_FF
        ADD     K_SIN
        TBLR    CC
; 頂点の数：MAX_VERTS より多ければ頭を探し直す。0 以下なら尾だけ
        LAC     NV
        SUB     K_MAXV
        BGZ     frame
        LAC     NV
        BLEZ    tail
        SUB     ONE
        SACL    CNT
        LAR     AR1,CNT                 ; AR1＝N−1（BANZ で N 回）

vert    CALL    getw
        LAC     W
        SACL    VX
        CALL    getw
        LAC     W
        SACL    VY
        CALL    getw
        LAC     W
        SACL    VZ
; X 軸：y1＝(y·cA − z·sA)>>15、z1＝(y·sA ＋ z·cA)>>15
        LT      VY
        MPY     CA
        PAC
        LT      VZ
        MPY     SA
        SPAC
        SACH    Y1,1
        LT      VY
        MPY     SA
        PAC
        LT      VZ
        MPY     CA
        APAC
        SACH    Z1,1
; Y 軸：x2＝(x·cB ＋ z1·sB)>>15、z2＝(z1·cB − x·sB)>>15
        LT      VX
        MPY     CB
        PAC
        LT      Z1
        MPY     SB
        APAC
        SACH    X2,1
        LT      Z1
        MPY     CB
        PAC
        LT      VX
        MPY     SB
        SPAC
        SACH    Z2,1
; Z 軸：x3＝(x2·cC − y1·sC)>>15、y3＝(x2·sC ＋ y1·cC)>>15
        LT      X2
        MPY     CC
        PAC
        LT      Y1
        MPY     SC
        SPAC
        SACH    X3,1
        LT      X2
        MPY     SC
        PAC
        LT      Y1
        MPY     CC
        APAC
        SACH    Y3,1
; 透視：k＝PERSP[(z2＋128)&255]、X＝((x3·k)>>15)＋CX、Y＝((y3·k)>>15)−CY
        LAC     Z2
        ADD     K_128
        AND     K_FF
        ADD     K_PERSP
        TBLR    K
        LT      K
        MPY     X3
        PAC
        SACH    PX,1
        MPY     Y3                      ; T は k のまま
        PAC
        SACH    PY,1
        LAC     PX
        ADD     K_CX
        SACL    OUTV
        CALL    putw
        LAC     PY
        SUB     K_CY
        SACL    OUTV
        CALL    putw
        BANZ    vert                    ; AR1 が 0 になるまで

tail    LAC     K_TAIL
        OR      SEQ
        SACL    OUTV
        CALL    putw
        B       frame

; ------------------------------------------------------------------ 入出力

; 入力 FIFO から 1 語を W に読む。BIO（入力 FIFO が空でない）が来るまで待つ。
getw    BIOZ    .rd
        B       getw
.rd     IN      W,DSP_PA_INPUT
        RET

; OUTV を出力 FIFO に書く。状態ポートの「出力に空き」を待つ。
putw    IN      STAT,DSP_PA_STATUS
        LAC     STAT
        AND     K_SPACE
        BZ      putw
        OUT     OUTV,DSP_PA_OUTPUT
        RET

; ------------------------------------------------------------------ 表（TBLR で読む）

        INCLUDE "dsptab.inc"            ; build.py が作る：SINTAB（256 語）、PERSP（256 語）
