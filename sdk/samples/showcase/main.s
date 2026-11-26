; SAMPLE 6：通常モードの映像と描画の力を見せ、1 フレームの描画の予算を測るカートリッジ。m68kasm で組む
; （build.py）。説明は README.md。ベンドは使わない。
;
; 画面（案B の 192 ラスタを 3 つに分ける）：
;   上画面   16 ラスタ  題、モードの印、文字 ROM の漢字の行（CPU が字形を読んで DWT で描く）と半角カナ
;   基本画面 160 ラスタ 2 頁を交互に描き、VBlank の中で SAR1 を替える（頁の切替え）。上の 96 行は空：
;                       DSP（または 68000）が回した立方体を ACRTC の折れ線で描く。下の 64 行は海：16×16 の
;                       玉を N 個、海の元の絵からの CPY で背景を戻してから、型（AND）と絵（OR）の SCPY で描く
;   下画面   16 ラスタ  段ごとのライン数と、玉の数・フレームの速さ・誤りの数
; ラスタのパレット：レベル 6 の項目を差し替え、基本画面の 160 ラスタで毎ライン HBlank の割込みを受けて、
; パレット 1（空・海）と 2（立方体・波頭）を書く。コッパーバー 2 本は VBlank のフックが表を書き換える。
; 時間の帯：B で、段の始まりごとにパレット 0（上下の画面の地と、基本画面の左右の縁）を段の色にする。
; 待ち：どの待ちにも上限を置き、時間切れと誤りは数える（README.md §05 の表。試験が読む）。
;
; 番地とビットの名前は fc85.inc、BIOS の約束は bios.inc（sdk/docs/bios-abi.md）、DSP との約束は
; protocol.inc、表の大きさは build.py が作る consts.inc。

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"
        INCLUDE "protocol.inc"
        INCLUDE "consts.inc"

; ------------------------------------------------------------------ 定数

; 画面の分け方（ラスタ）。表示の最初のラスタは 43（VSW＋VDS、台帳 HD63484.md §3.4）
UPPER_H         EQU     16
LOWER_H         EQU     16
        ASSERT  UPPER_H+BASE_ROWS+LOWER_H=192
V_TOP           EQU     43
BASE_RASTER     EQU     V_TOP+UPPER_H   ; 基本画面の最初のラスタ（59）
RASTER_ARM      EQU     BASE_RASTER-2   ; この HBlank で表の 0 番を書く（2 ライン後のラスタから効く）
VBL_LINE        EQU     235             ; VBlank の始まり（LINE の値）
RASTERS         EQU     262

; VRAM（語番地。どの画面も 1 行 64 語）
UPPER_VRAM      EQU     0               ; 行 0〜15
PAGE0_VRAM      EQU     16*64           ; 行 16〜175
LOWER_VRAM      EQU     176*64          ; 行 176〜191
PAGE1_VRAM      EQU     192*64          ; 行 192〜351
SEA_VRAM        EQU     352*64          ; 海の元の絵（64 行）
BANK_VRAM       EQU     416*64          ; 玉の型と絵（16 行）
DIGIT_VRAM      EQU     432*64          ; 「0123456789 」（8 行）
VRAM_ROWS       EQU     440
LOWER_ROW       EQU     176             ; BIOS_PUTS の y（VRAM の行）
DIGIT_ROW       EQU     432
KANJI_ROW       EQU     8
KANJI_X         EQU     0
KANA_X          EQU     72

; ACRTC のコマンドの組立て
ACRTC_FIFO_WORDS EQU    8               ; 書込み FIFO の深さ（台帳 HD63484.md §4.1）
CPY_DOWN        EQU     $0100           ; DSD＝001：先は X＋、行は下へ
AREA_CLIP       EQU     $0040           ; AREA＝010：領域の外は描かずに続ける
BORDER_W        EQU     1               ; 左右の縁（語）
SKY_WORDS       EQU     64-2*BORDER_W   ; 空の幅（語）
SPR_H           EQU     16              ; 玉の行
CUBE_CMD_WORDS  EQU     5*CUBE_PATHS+2*CUBE_SEGMENTS

; 色（パレットの番号）
COL_BG          EQU     0
COL_SKY         EQU     1
COL_CUBE        EQU     2
COL_GREY        EQU     11
COL_YELLOW      EQU     12
COL_WHITE       EQU     15

; 段（時間の帯と下画面の数）
PH_K            EQU     0               ; DSP へ送る
PH_R            EQU     1               ; 玉の背景を戻す
PH_E            EQU     2               ; 前の立方体を消す
PH_X            EQU     3               ; 座標を受け取る（68000 なら計算する）
PH_C            EQU     4               ; 立方体を描く
PH_S            EQU     5               ; 玉を動かして描く
PH_T            EQU     6               ; 入力と文字
PH_IDLE         EQU     7               ; 待ち
PH_BUSY         EQU     PHASE_COUNT     ; v_dur の 7 番：K から待ちまで
PH_PERIOD       EQU     PHASE_COUNT+1   ; 8 番：ループのフレーム数
DUR_WORDS       EQU     PHASE_COUNT+2
WINDOW          EQU     16              ; この回数のループの最大を出す
FIELDS_PER_LOOP EQU     2               ; 1 ループに描き直す数の欄

; 待ちの上限
WAIT_LOOPS      EQU     6250            ; VBlank：1 回 32 クロックで約 25 ms
FAB_WAIT        EQU     6250            ; DSP の結果：約 25 ms
PRG_WAIT        EQU     1000            ; DSP の PRGBUS

; Staging RAM（port 0 の byte 番地）
IN_STAGE        EQU     $0000
IN_WORDS        EQU     IN_HEAD_WORDS+3*CUBE_VERTS
OUT_STAGE       EQU     $0100
OUT_WORDS       EQU     2*CUBE_VERTS+1
        ASSERT  CUBE_VERTS<=MAX_VERTS
        ASSERT  IN_STAGE+2*IN_WORDS<=OUT_STAGE

; 入力
INIT_OBJ        EQU     16
REPEAT_DELAY    EQU     20              ; 上下を押し続けて繰り返すまでのループ
REPEAT_MASK     EQU     3               ; その後は 4 ループごと
DIRS            EQU     PAD_UP|PAD_DOWN|PAD_LEFT|PAD_RIGHT

; v_flags のビット（上画面の印）
FL_DSP          EQU     0
FL_RASTER       EQU     1
FL_BARS         EQU     2
FL_SPRITES      EQU     3
FL_COUNT        EQU     4

; ------------------------------------------------------------------ Main RAM（$100800〜、bios-abi.md §03）

        RSSET   APP_RAM
; 試験と測定が読む（README.md §05。番地を変えない）
v_loops         RS.W    1               ; +0  主ループの回数
v_err_fabric    RS.W    1               ; +2  Fabric の時間切れと ERROR
v_err_dsp       RS.W    1               ; +4  DSP の読込みの誤り、UNDERRUN・OVERRUN
v_err_tail      RS.W    1               ; +6  尾の語の食い違い
v_err_xcheck    RS.W    1               ; +8  起動時の DSP と 68000 の突合せの食い違い
v_err_acrtc     RS.W    1               ; +10 ACRTC の打切り（CER か時間切れ）
v_vbl_miss      RS.W    1               ; +12 VBlank の待ちの時間切れ
ERR_COUNT       EQU     6
v_dsp_ok        RS.W    1               ; +14 DSP を読み込んで確かめた
v_nobj          RS.W    1               ; +16 玉の数（C で止めていても数は保つ）
v_period        RS.W    1               ; +18 最後のループのフレーム数（1 は 60 Hz、2 は 30 Hz）
v_dur           RS.W    DUR_WORDS       ; +20 最後のループの段ごとのライン数、BUSY、フレーム数
v_engine        RS.W    1               ; +38 1：DSP、0：68000
v_raster        RS.W    1               ; +40 ラスタのパレット
v_bars          RS.W    1               ; +42 時間の帯
v_sprites       RS.W    1               ; +44 玉を出す
        ASSERT  v_dur=APP_RAM+20
        ASSERT  v_sprites=APP_RAM+44
; ここから内部
v_bios_irq6     RS.L    1               ; 差し替える前のレベル 6 の処理
v_draw          RS.W    1               ; 描いている頁（0・1）
v_start         RS.L    1               ; このループの始まりの bios_frame
v_seq           RS.W    1               ; DSP への塊の番号
v_kicked        RS.W    1               ; このループで DSP に送った
v_ang_a         RS.W    1
v_ang_b         RS.W    1
v_ang_c         RS.W    1
v_trig          RS.W    6               ; sA, cA, sB, cB, sC, cC
v_proj          RS.W    2*CUBE_VERTS    ; 68000 の計算の結果
v_bar_t         RS.W    1               ; コッパーバーの位相
v_bar_a         RS.W    1               ; 塗ってあるバーの空の行
v_bar_b         RS.W    1
v_hold          RS.W    1               ; 上下左右を押し続けたループ
v_flags_shown   RS.W    1
v_window        RS.W    1
v_hz            RS.W    1
v_err_sum       RS.W    1
v_obj_shown     RS.W    1
v_mark          RS.L    PHASE_COUNT+1   ; 段の始まりの時刻（上位語は bios_frame、下位語は VBlank からのライン）
v_peak          RS.W    DUR_WORDS
v_show          RS.W    DUR_WORDS
CELLS           EQU     25
FIELDS          EQU     11
v_cells         RS.B    CELLS+1         ; 下画面に出している字（0〜9、10 は空白）
v_field_val     RS.W    FIELDS          ; 欄ごとに描いた値
v_cube_len      RS.W    2               ; 頁ごとの立方体のコマンドの語数
v_cube          RS.W    2*CUBE_CMD_WORDS
REC_BYTES       EQU     2+8*MAX_OBJ
v_rec           RS.B    2*REC_BYTES     ; 頁ごと：玉の数、(描いた先の RWPH・RWPL、海の元の Sah・Sal)×数
RTPL_WORDS      EQU     9
STPL_WORDS      EQU     18
v_rtpl          RS.W    RTPL_WORDS      ; 背景を戻す CPY の型（tpl_rom を写す）
v_stpl          RS.W    STPL_WORDS      ; 玉を描く 2 つの SCPY の型
v_obj           RS.W    5*MAX_OBJ       ; x, y, dx, dy, 種類
v_rtab          RS.W    3*BASE_ROWS     ; ラスタの表：パレット 1、パレット 2、VIDEO_IRQ に書く語
v_cmd           RS.W    160             ; ACRTC の語の並び
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
        DC.B    "SAMPLE 6        "      ; 題名（16 字、空白で埋める）
        ASSERT  *=CART+cart_header_size

; ------------------------------------------------------------------ 入口

; 入口の条件（bios-abi.md §04）：画面は色 0、Data Fabric の両チャネルは止まっている、DSP はリセットのまま。
; A6 はずっと ACRTC_AR（AR は r00 のまま使う）。
start:
        LEA     INITIAL_SSP,SP
        LEA     APP_RAM,A0
        MOVE.W  #(v_vars_end-APP_RAM+3)/4-1,D0
.clr    MOVE.L  #0,(A0)+
        DBRA    D0,.clr
        LEA     ACRTC_AR,A6
        MOVE.W  #ACRTC_FIFO,(A6)
        LEA     palette,A0
        JSR     BIOS_SET_PAL
        BSR     setup_screens
        BSR     vram_init
        BSR     text_init
        BSR     objects_init
        BSR     rtab_init
        BSR     dsp_load
        BSR     xcheck
        ; 既定のモード
        MOVE.W  #INIT_OBJ,v_nobj
        MOVE.W  #1,v_sprites
        MOVE.W  #1,v_raster
        MOVE.W  v_dsp_ok,v_engine
        LEA     v_cells,A0
        MOVEQ   #CELLS-1,D0
.cells  MOVE.B  #$FF,(A0)+
        DBRA    D0,.cells
        LEA     v_field_val,A0
        MOVEQ   #FIELDS-1,D0
.vals   MOVE.W  #$FFFF,(A0)+
        DBRA    D0,.vals
        BSR     flags_value
        MOVE.W  D4,v_flags_shown
        MOVEQ   #(1<<FL_COUNT)-1,D5
        BSR     flags_draw
        BSR     irq_install
        MOVE.L  bios_frame,v_start

; ------------------------------------------------------------------ 主ループ

; 1 回で 1 頁を描く。前のループで描いた頁を VBlank の中で見せてから、もう一方の頁を描く。
main_loop:
        BSR     flip
        ADDQ.W  #1,v_loops
        MOVEQ   #PH_K,D0
        BSR     mark
        BSR     ph_kick
        MOVEQ   #PH_R,D0
        BSR     mark
        BSR     ph_restore
        MOVEQ   #PH_E,D0
        BSR     mark
        BSR     ph_erase
        MOVEQ   #PH_X,D0
        BSR     mark
        BSR     ph_xform
        MOVEQ   #PH_C,D0
        BSR     mark
        BSR     ph_cube
        MOVEQ   #PH_S,D0
        BSR     mark
        BSR     ph_sprites
        MOVEQ   #PH_T,D0
        BSR     mark
        BSR     ph_text
        MOVEQ   #PH_IDLE,D0
        BSR     mark
        BSR     account
        BSR     wait_frame
        BRA     main_loop

; 次の VBlank を待つ（bios_frame が変わる）。上限は約 25 ms。描画が 1 フレームを越えたら、越えた後の
; VBlank を待つ（頁はいつも VBlank の中で替える）。v_period にループのフレーム数を入れる。
wait_frame:
        MOVE.L  bios_frame,D1
        MOVE.W  #WAIT_LOOPS-1,D2
.wait   CMP.L   bios_frame,D1
        DBNE    D2,.wait
        BNE     .vbl
        ADDQ.W  #1,v_vbl_miss
        MOVE.W  #VIRQ_VBL_EN,VIDEO_IRQ  ; 許可が落ちていたら戻す
.vbl    MOVE.L  bios_frame,D0
        MOVE.L  D0,D1
        SUB.L   v_start,D1
        MOVE.L  D0,v_start
        MOVE.W  D1,v_period
        RTS

; 描いた頁（v_draw）を見せ、もう一方を描く頁にする。SAR1 は次のフレームの基本画面の頭から効く。
flip:
        MOVE.W  v_draw,D0
        ADD.W   D0,D0
        LEA     page_vram,A0
        MOVE.W  0(A0,D0.W),D1
        MOVE.W  #ACRTC_SAR1H,(A6)
        MOVE.W  #0,2(A6)
        MOVE.W  D1,2(A6)                ; AR は r80 から先で語ごとに 2 進む（SAR1L）
        MOVE.W  #ACRTC_FIFO,(A6)
        EORI.W  #1,v_draw
        RTS

; ------------------------------------------------------------------ 時刻

; 段 D0.W の始まり。時刻を v_mark に入れ、時間の帯が出ていればパレット 0 を段の色にする。D0〜D3・A0 を壊す。
mark:
        MOVE.W  D0,D2
        ADD.W   D2,D2
        TST.W   v_bars
        BEQ     .time
        LEA     phase_colors,A0
        MOVE.W  0(A0,D2.W),PALETTE
.time   BSR     now
        ADD.W   D2,D2
        LEA     v_mark,A0
        MOVE.L  D0,0(A0,D2.W)
        RTS

; 今の時刻。D0.L の上位語は bios_frame の下位語、下位語は VBlank の始まりからのライン（0〜261）。
; VBlank の割込みがまだ bios_frame を足していなければ（VIDEO_IRQ の VBL の保留）、足した後の値にする。
; D1・D3 を壊す。
now:
        MOVE.W  D2,-(SP)
        MOVEQ   #3,D3
.again  MOVE.L  bios_frame,D0
        MOVE.W  VIDEO_LINE,D1
        MOVE.W  VIDEO_IRQ,D2
        CMP.L   bios_frame,D0
        DBEQ    D3,.again
        SUBI.W  #VBL_LINE,D1
        BPL     .pos
        ADDI.W  #RASTERS,D1
.pos    BTST    #8,D2
        BEQ     .pack
        CMPI.W  #RASTERS/2,D1
        BHS     .pack
        ADDQ.L  #1,D0
.pack   SWAP    D0
        MOVE.W  D1,D0
        MOVE.W  (SP)+,D2
        RTS

; D0.L から D1.L までのライン数を D2.W に（0〜999）。D1・D4 を壊す。
lines_between:
        MOVE.L  D1,D2
        SWAP    D2
        MOVE.L  D0,D4
        SWAP    D4
        SUB.W   D4,D2
        MULS    #RASTERS,D2
        SUB.W   D0,D1
        EXT.L   D1
        ADD.L   D1,D2
        BPL     .pos
        MOVEQ   #0,D2
.pos    CMPI.L  #999,D2
        BLS     .ret
        MOVE.W  #999,D2
.ret    RTS

; 段の長さを v_dur に、WINDOW 回の最大を v_peak に入れ、WINDOW 回ごとに v_show（下画面に出す値）へ写す。
account:
        LEA     v_mark,A3
        LEA     v_dur,A4
        MOVEQ   #PHASE_COUNT-1,D3
.ph     MOVE.L  (A3)+,D0
        MOVE.L  (A3),D1
        BSR     lines_between
        MOVE.W  D2,(A4)+
        DBRA    D3,.ph
        MOVE.L  v_mark,D0
        MOVE.L  v_mark+4*PH_IDLE,D1
        BSR     lines_between
        MOVE.W  D2,(A4)+                ; BUSY
        MOVE.W  v_period,(A4)+
        LEA     v_dur,A3
        LEA     v_peak,A4
        MOVEQ   #DUR_WORDS-1,D3
.peak   MOVE.W  (A3)+,D0
        CMP.W   (A4),D0
        BLS     .keep
        MOVE.W  D0,(A4)
.keep   ADDQ.L  #2,A4
        DBRA    D3,.peak
        ADDQ.W  #1,v_window
        CMPI.W  #WINDOW,v_window
        BLO     .ret
        MOVE.W  #0,v_window
        LEA     v_peak,A3
        LEA     v_show,A4
        MOVEQ   #DUR_WORDS-1,D3
.latch  MOVE.W  (A3),(A4)+
        MOVE.W  #0,(A3)+
        DBRA    D3,.latch
.ret    RTS

; ------------------------------------------------------------------ 段 K：DSP へ送る

ph_kick:
        MOVE.W  #0,v_kicked
        TST.W   v_engine
        BEQ     .ret
        BSR     dsp_kick
        MOVE.W  #1,v_kicked
.ret    RTS

; 入力の塊の頭（番号と角）を Staging RAM に書き、チャネル 1（port 4 → Staging）、チャネル 0（Staging →
; port 3）の順に始める。頂点は起動時に書いてある。D0・A0・A1 を壊す。
dsp_kick:
        LEA     STAGING+IN_STAGE,A0
        MOVE.W  v_seq,D0
        ADDQ.W  #1,D0
        ANDI.W  #255,D0
        MOVE.W  D0,v_seq
        ORI.W   #HEAD_MAGIC<<8,D0
        MOVE.W  D0,(A0)+
        MOVE.W  v_ang_a,(A0)+
        MOVE.W  v_ang_b,(A0)+
        MOVE.W  v_ang_c,(A0)+
        MOVE.W  #CUBE_VERTS,(A0)+
        MOVE.W  #0,STAGING+OUT_STAGE+4*CUBE_VERTS      ; 古い尾の語を受け取らない
        LEA     FABRIC_CH1,A1
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A1)
        MOVE.W  #FAB_START,FAB_CTRL(A1)
        LEA     FABRIC_CH0,A1
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A1)
        MOVE.W  #FAB_START,FAB_CTRL(A1)
        RTS

; DSP の結果を待つ。出力：D0.W＝0 なら A0＝座標（Staging RAM の X, Y×CUBE_VERTS）。0 でなければ誤りで、
; 数えて DSP と Fabric を初期化し直した。D1・A1 を壊す。
dsp_fetch:
        LEA     FABRIC_CH1,A1
        MOVE.W  #FAB_WAIT-1,D1
.wait   MOVE.W  FAB_STATUS(A1),D0
        ANDI.W  #FAB_ST_DONE|FAB_ST_ERROR,D0
        DBNE    D1,.wait
        BEQ     .ferr                   ; 時間切れ
        ANDI.W  #FAB_ST_ERROR,D0
        BNE     .ferr
        MOVE.W  FABRIC_CH0+FAB_STATUS,D0
        ANDI.W  #FAB_ST_ERROR,D0
        BNE     .ferr
        MOVE.W  DSP_STATUS,D0
        ANDI.W  #DSP_ST_UNDERRUN|DSP_ST_OVERRUN,D0
        BNE     .derr
        MOVE.W  v_seq,D0
        ORI.W   #TAIL_MAGIC<<8,D0
        CMP.W   STAGING+OUT_STAGE+4*CUBE_VERTS,D0
        BNE     .terr
        LEA     STAGING+OUT_STAGE,A0
        MOVEQ   #0,D0
        RTS
.ferr   ADDQ.W  #1,v_err_fabric
        BRA     .reset
.derr   ADDQ.W  #1,v_err_dsp
        BRA     .reset
.terr   ADDQ.W  #1,v_err_tail
.reset  BSR     dsp_restart
        MOVEQ   #1,D0
        RTS

; 両チャネルを止め、DSP をリセットし、port 3・4 の FIFO を初期化して、DSP を解く（プログラムは RAM に残る）。
dsp_restart:
        MOVE.W  #FAB_STOP,FABRIC_CH0+FAB_CTRL
        MOVE.W  #FAB_STOP,FABRIC_CH1+FAB_CTRL
        MOVE.W  #FAB_ST_W1C,FABRIC_CH0+FAB_STATUS
        MOVE.W  #FAB_ST_W1C,FABRIC_CH1+FAB_STATUS
        MOVE.W  #DSP_CTRL_RESET,DSP_CTRL
        MOVE.W  #(1<<PORT_DSP_IN)|(1<<PORT_DSP_OUT),FABRIC_PORT_RESET
        MOVE.W  #DSP_ST_UNDERRUN|DSP_ST_OVERRUN,DSP_STATUS
        MOVE.W  #DSP_CTRL_BIO_INPUT,DSP_CTRL
        RTS

; ------------------------------------------------------------------ 段 R：玉の背景を戻す

; この頁に 2 ループ前に描いた玉の場所へ、海の元の絵から 5 語×16 行を CPY する。記録（段 S が作る）の
; RWP と元の番地を、コマンドの型（v_rtpl）に入れて送る。
ph_restore:
        BSR     rec_ptr
        MOVE.W  (A5)+,D3
        BEQ     .done
        SUBQ.W  #1,D3
        LEA     v_rtpl,A2
.obj    MOVE.W  (A5)+,2(A2)             ; RWPH
        MOVE.W  (A5)+,6(A2)             ; RWPL
        MOVE.L  (A5)+,10(A2)            ; Sah・Sal
        MOVEA.L A2,A0
        MOVEQ   #RTPL_WORDS,D1
        BSR     send
        DBRA    D3,.obj
.done   BRA     wait_idle

; A5＝描いている頁の玉の記録。D0 を壊す。
rec_ptr:
        LEA     v_rec,A5
        TST.W   v_draw
        BEQ     .ret
        LEA     REC_BYTES(A5),A5
.ret    RTS

; ------------------------------------------------------------------ 段 E・X・C：立方体

; 頁の原点（ORG）を置き、この頁に 2 ループ前に描いた線を空の色で描き直して消す。
ph_erase:
        LEA     v_cmd,A2
        MOVE.W  #ACRTC_ORG,(A2)+
        MOVE.W  v_draw,D0
        LSL.W   #2,D0
        LEA     page_org,A0
        MOVE.L  0(A0,D0.W),(A2)+
        MOVEQ   #COL_SKY,D0
        BSR     cmd_color
        BSR     flush
        BSR     cube_send
        BRA     wait_idle

; DSP の結果を受け取るか、68000 で計算して、この頁の線のコマンドを作る。角を進める。
ph_xform:
        SUBA.L  A0,A0
        TST.W   v_kicked
        BEQ     .m68k
        BSR     dsp_fetch
        TST.W   D0
        BEQ     .list
        SUBA.L  A0,A0                   ; 誤り：このループは描かない
        BRA     .list
.m68k   BSR     xform68
        LEA     v_proj,A0
.list   BSR     cube_list
        ADDQ.W  #2,v_ang_a
        ADDQ.W  #3,v_ang_b
        ADDQ.W  #1,v_ang_c
        RTS

; 線の色にして、この頁の線を描く（ORG は段 E で置いた）。
ph_cube:
        LEA     v_cmd,A2
        MOVEQ   #COL_CUBE,D0
        BSR     cmd_color
        BSR     flush
        BSR     cube_send
        BRA     wait_idle

; 頂点の画面座標（A0。0 なら描かない）から、辺の道（cube_paths）ごとに AMOVE と APLL を作り、この頁の
; v_cube に置く。
cube_list:
        MOVE.W  v_draw,D0
        MOVE.W  D0,D2
        MULU    #CUBE_CMD_WORDS*2,D0
        LEA     v_cube,A2
        ADDA.L  D0,A2
        MOVEA.L A2,A3
        MOVE.L  A0,D0
        BEQ     .done
        LEA     cube_paths,A1
.path   MOVE.W  (A1)+,D0                ; 線分の数
        BEQ     .done
        MOVE.W  (A1)+,D1                ; 最初の頂点
        LSL.W   #2,D1
        MOVE.W  #ACRTC_AMOVE,(A2)+
        MOVE.L  0(A0,D1.W),(A2)+
        MOVE.W  #ACRTC_APLL|AREA_CLIP,(A2)+
        MOVE.W  D0,(A2)+
        SUBQ.W  #1,D0
.pt     MOVE.W  (A1)+,D1
        LSL.W   #2,D1
        MOVE.L  0(A0,D1.W),(A2)+
        DBRA    D0,.pt
        BRA     .path
.done   SUBA.L  A3,A2
        MOVE.L  A2,D0
        LSR.W   #1,D0
        ADD.W   D2,D2
        LEA     v_cube_len,A0
        MOVE.W  D0,0(A0,D2.W)
        RTS

; この頁の v_cube を送る。
cube_send:
        MOVE.W  v_draw,D0
        MOVE.W  D0,D1
        ADD.W   D1,D1
        LEA     v_cube_len,A0
        MOVE.W  0(A0,D1.W),D1
        MULU    #CUBE_CMD_WORDS*2,D0
        LEA     v_cube,A0
        ADDA.L  D0,A0
        BRA     send

; 68000 で回して透視する（protocol.inc の式。DSP と同じ結果になる）。出力は v_proj。D0・D1・A0・A1 を壊す。
xform68:
        MOVEM.L D2-D7/A2-A3,-(SP)
        LEA     sin_table,A0
        LEA     v_trig,A1
        MOVE.W  v_ang_a,D0
        BSR     .sc
        MOVE.W  v_ang_b,D0
        BSR     .sc
        MOVE.W  v_ang_c,D0
        BSR     .sc
        LEA     cube_verts,A0
        LEA     v_proj,A1
        LEA     v_trig,A2
        LEA     persp_table,A3
        MOVEQ   #CUBE_VERTS-1,D7
.vert   MOVEM.W (A0)+,D0-D2             ; x, y, z
        ; X 軸：y1＝(y·cA − z·sA)>>15、z1＝(y·sA ＋ z·cA)>>15
        MOVE.W  D1,D3
        MULS    2(A2),D3
        MOVE.W  D2,D4
        MULS    (A2),D4
        SUB.L   D4,D3
        ADD.L   D3,D3
        SWAP    D3                      ; y1
        MULS    (A2),D1
        MULS    2(A2),D2
        ADD.L   D1,D2
        ADD.L   D2,D2
        SWAP    D2                      ; z1
        ; Y 軸：x2＝(x·cB ＋ z1·sB)>>15、z2＝(z1·cB − x·sB)>>15
        MOVE.W  D0,D4
        MULS    6(A2),D4
        MOVE.W  D2,D5
        MULS    4(A2),D5
        ADD.L   D5,D4
        ADD.L   D4,D4
        SWAP    D4                      ; x2
        MULS    6(A2),D2
        MULS    4(A2),D0
        SUB.L   D0,D2
        ADD.L   D2,D2
        SWAP    D2                      ; z2
        ; Z 軸：x3＝(x2·cC − y1·sC)>>15、y3＝(x2·sC ＋ y1·cC)>>15
        MOVE.W  D4,D0
        MULS    10(A2),D0
        MOVE.W  D3,D1
        MULS    8(A2),D1
        SUB.L   D1,D0
        ADD.L   D0,D0
        SWAP    D0                      ; x3
        MULS    8(A2),D4
        MULS    10(A2),D3
        ADD.L   D4,D3
        ADD.L   D3,D3
        SWAP    D3                      ; y3
        ; 透視：k＝PERSP[(z2＋128)&255]
        ADDI.W  #128,D2
        ANDI.W  #255,D2
        ADD.W   D2,D2
        MOVE.W  0(A3,D2.W),D2
        MULS    D2,D0
        ADD.L   D0,D0
        SWAP    D0
        ADDI.W  #CX,D0
        MULS    D2,D3
        ADD.L   D3,D3
        SWAP    D3
        SUBI.W  #CY,D3
        MOVE.W  D0,(A1)+
        MOVE.W  D3,(A1)+
        DBRA    D7,.vert
        MOVEM.L (SP)+,D2-D7/A2-A3
        RTS
; 角 D0 の正弦と余弦を (A1)+ に
.sc     MOVE.W  D0,D1
        ANDI.W  #255,D1
        ADD.W   D1,D1
        MOVE.W  0(A0,D1.W),(A1)+
        ADDI.W  #64,D0
        ANDI.W  #255,D0
        ADD.W   D0,D0
        MOVE.W  0(A0,D0.W),(A1)+
        RTS

; ------------------------------------------------------------------ 段 S：玉

; 玉を動かし（端で跳ね返る）、型の AND と絵の OR の SCPY で描く。コマンドの型（v_stpl）の変わる語だけを
; 書いて送る。描いた先の RWP と海の元の番地（Sah・Sal）を、段 R のために記録する。
ph_sprites:
        BSR     rec_ptr
        MOVEQ   #0,D3
        TST.W   v_sprites
        BEQ     .count
        MOVE.W  v_nobj,D3
.count  MOVE.W  D3,(A5)+
        BEQ     .done
        SUBQ.W  #1,D3
        LEA     v_obj,A4
        LEA     v_stpl,A2
        LEA     spr_src,A3
        MOVE.W  v_draw,D0
        ADD.W   D0,D0
        LEA     page_vram,A0
        MOVE.W  0(A0,D0.W),D6
.obj    MOVE.W  (A4),D0                 ; x
        ADD.W   4(A4),D0
        CMPI.W  #SPR_XMIN,D0
        BGE     .x0
        MOVE.W  #SPR_XMIN,D0
        NEG.W   4(A4)
.x0     CMPI.W  #SPR_XMAX,D0
        BLE     .x1
        MOVE.W  #SPR_XMAX,D0
        NEG.W   4(A4)
.x1     MOVE.W  D0,(A4)
        MOVE.W  2(A4),D1                ; y
        ADD.W   6(A4),D1
        CMPI.W  #SPR_YMIN,D1
        BGE     .y0
        MOVE.W  #SPR_YMIN,D1
        NEG.W   6(A4)
.y0     CMPI.W  #SPR_YMAX,D1
        BLE     .y1
        MOVE.W  #SPR_YMAX,D1
        NEG.W   6(A4)
.y1     MOVE.W  D1,2(A4)
        ; 頁の中の語番地＝y×64＋x/4
        LSL.W   #6,D1
        MOVE.W  D0,D2
        LSR.W   #2,D2
        ADD.W   D2,D1
        MOVE.W  D1,D2
        ADD.W   D6,D2                   ; 描く先
        ; RWP（DN＝1）：RWPH＝$4000｜番地>>12、RWPL＝番地<<4
        MOVE.W  D2,D4
        ROL.W   #4,D4
        ANDI.W  #$000F,D4
        ORI.W   #$4000,D4
        LSL.W   #4,D2
        MOVE.W  D4,2(A2)
        MOVE.W  D2,6(A2)
        MOVE.W  D4,20(A2)
        MOVE.W  D2,24(A2)
        MOVE.W  D4,(A5)+
        MOVE.W  D2,(A5)+
        ; 海の元の番地（Sah・Sal）
        ADDI.W  #SEA_VRAM-SKY_ROWS*64,D1
        MOVE.W  D1,D4
        ROL.W   #4,D4
        ANDI.W  #$000F,D4
        MOVE.W  D4,(A5)+
        LSL.W   #4,D1
        MOVE.W  D1,(A5)+
        ; 型と絵：横のずれ s＝x&3 と種類で spr_src を引く
        ANDI.W  #3,D0
        MOVE.W  8(A4),D1
        LSL.W   #2,D1
        ADD.W   D1,D0
        LSL.W   #3,D0
        MOVE.L  0(A3,D0.W),10(A2)       ; 型の Sah・Sal
        MOVE.L  4(A3,D0.W),28(A2)       ; 絵の Sah・Sal
        MOVEA.L A2,A0
        MOVEQ   #STPL_WORDS,D1
        BSR     send
        LEA     10(A4),A4
        DBRA    D3,.obj
.done   BRA     wait_idle

; ------------------------------------------------------------------ 段 T：入力と文字

ph_text:
        BSR     input
        BSR     flags_value
        MOVE.W  v_flags_shown,D5
        EOR.W   D4,D5
        BEQ     .num
        MOVE.W  D4,v_flags_shown
        BSR     flags_draw
.num    BSR     status_values
        BSR     status_draw
        BRA     wait_idle

; A：立体の計算を DSP と 68000 で切り替える。B：時間の帯。C：玉を出す・消す。START：ラスタのパレット。
; 上・下：玉の数を 1 つ増やす・減らす。右・左：8 つ。押し続けると繰り返す。
input:
        JSR     BIOS_READ_PAD           ; D0＝PORT_A、D1＝新しく押したビット
        MOVE.W  D0,D6
        MOVE.W  D1,D5
        BTST    #PAD_A_B,D5
        BEQ     .b
        TST.W   v_dsp_ok
        BEQ     .b
        EORI.W  #1,v_engine
.b      BTST    #PAD_B_B,D5
        BEQ     .c
        EORI.W  #1,v_bars
        BNE     .c
        MOVE.W  #0,PALETTE              ; 帯を止めたら黒に戻す
.c      BTST    #PAD_C_B,D5
        BEQ     .s
        EORI.W  #1,v_sprites
.s      BTST    #PAD_START_B,D5
        BEQ     .dirs
        EORI.W  #1,v_raster
.dirs   MOVE.W  D6,D0
        ANDI.W  #DIRS,D0
        BNE     .held
        MOVE.W  #0,v_hold
        RTS
.held   ADDQ.W  #1,v_hold
        MOVE.W  D5,D1
        ANDI.W  #DIRS,D1
        BNE     .act
        CMPI.W  #REPEAT_DELAY,v_hold
        BLO     .ret
        MOVE.W  v_hold,D1
        ANDI.W  #REPEAT_MASK,D1
        BNE     .ret
        MOVE.W  D0,D1
.act    MOVE.W  v_nobj,D2
        BTST    #PAD_UP_B,D1
        BEQ     .down
        ADDQ.W  #1,D2
.down   BTST    #PAD_DOWN_B,D1
        BEQ     .right
        SUBQ.W  #1,D2
.right  BTST    #PAD_RIGHT_B,D1
        BEQ     .left
        ADDQ.W  #8,D2
.left   BTST    #PAD_LEFT_B,D1
        BEQ     .clamp
        SUBQ.W  #8,D2
.clamp  TST.W   D2
        BPL     .max
        MOVEQ   #0,D2
.max    CMPI.W  #MAX_OBJ,D2
        BLE     .set
        MOVE.W  #MAX_OBJ,D2
.set    MOVE.W  D2,v_nobj
.ret    RTS

; 上画面の印の今の値を D4.W に（v_flags のビット）。
flags_value:
        MOVEQ   #0,D4
        TST.W   v_engine
        BEQ     .r
        BSET    #FL_DSP,D4
.r      TST.W   v_raster
        BEQ     .b
        BSET    #FL_RASTER,D4
.b      TST.W   v_bars
        BEQ     .s
        BSET    #FL_BARS,D4
.s      TST.W   v_sprites
        BEQ     .ret
        BSET    #FL_SPRITES,D4
.ret    RTS

; D5.W のビットの印を、D4.W の値で描き直す（BIOS_PUTS）。
flags_draw:
        LEA     flag_items,A4
        MOVEQ   #0,D6
.item   BTST    D6,D5
        BEQ     .next
        MOVE.W  (A4),D0                 ; x
        MOVEQ   #0,D1                   ; y
        MOVEQ   #COL_BG,D3
        BTST    D6,D4
        BEQ     .off
        MOVE.W  2(A4),D2
        MOVEA.L 6(A4),A0
        BRA     .put
.off    MOVE.W  4(A4),D2
        MOVEA.L 10(A4),A0
.put    JSR     BIOS_PUTS
.next   LEA     14(A4),A4
        ADDQ.W  #1,D6
        CMPI.W  #FL_COUNT,D6
        BLO     .item
        RTS

; 下画面に出す値を作る：フレームの速さ、誤りの合計、出している玉の数。
status_values:
        MOVEQ   #0,D0
        MOVE.W  v_show+2*PH_PERIOD,D1
        BEQ     .hz
        MOVEQ   #60,D0
        DIVU    D1,D0
.hz     MOVE.W  D0,v_hz
        LEA     v_err_fabric,A0
        MOVEQ   #0,D0
        MOVEQ   #ERR_COUNT-1,D1
.err    ADD.W   (A0)+,D0
        DBRA    D1,.err
        MOVE.W  D0,v_err_sum
        MOVEQ   #0,D0
        TST.W   v_sprites
        BEQ     .obj
        MOVE.W  v_nobj,D0
.obj    MOVE.W  D0,v_obj_shown
        RTS

; 数の欄（status_fields）を描く。値の変わった欄の、変わった桁だけを、数字の帯から CPY する。1 回に描く
; のは FIELDS_PER_LOOP 欄まで（段 T の長さをそろえ、予算の端で 60 Hz と 30 Hz の間を揺れないように）。
status_draw:
        MOVE.L  D6,-(SP)
        MOVEQ   #FIELDS_PER_LOOP-1,D6
        LEA     v_cells,A3
        LEA     status_fields,A4
        LEA     v_field_val,A1
.field  MOVE.W  (A4)+,D2                ; 最上位の桁の語番地（−1 で終わり）
        BMI     .ret
        MOVE.W  (A4)+,D1                ; 桁の数（2 か 3）
        MOVEA.L (A4)+,A0
        MOVE.W  (A0),D0
        CMP.W   (A1),D0
        BEQ     .same
        MOVE.W  D0,(A1)+
        BSR     put_number
        DBRA    D6,.field               ; 残りの欄は次のループで描く
        BRA     .ret
.same   ADDA.W  D1,A3
        ADDQ.L  #2,A1
        BRA     .field
.ret    MOVE.L  (SP)+,D6
        RTS

; D0.W を D1.W 桁で、最上位の桁が語番地 D2.W の欄に描く。上の桁の 0 は空白。A3 は表示中の字（進める）。
; D0〜D5・D7・A0・A2 を壊す。
put_number:
        MOVE.W  #99,D4
        CMPI.W  #2,D1
        BEQ     .cap
        MOVE.W  #999,D4
.cap    CMP.W   D4,D0
        BLS     .ok
        MOVE.W  D4,D0
.ok     MOVE.W  D1,D4
        SUBQ.W  #1,D4
        ADD.W   D4,D2
        ADD.W   D4,D2                   ; 一の位の語番地（字は 2 語）
        ANDI.L  #$FFFF,D0
        MOVEQ   #0,D5
.digit  TST.W   D5
        BEQ     .num
        TST.W   D0
        BNE     .num
        MOVEQ   #10,D3                  ; 空白
        BRA     .cell
.num    DIVU    #10,D0
        MOVE.L  D0,D3
        SWAP    D3                      ; 余り
        ANDI.L  #$FFFF,D0               ; 商
.cell   BSR     put_cell
        SUBQ.W  #2,D2
        MOVEQ   #1,D5
        DBRA    D4,.digit
        RTS

; 字 D3.W（0〜10）を語番地 D2.W に。(A3)+ と同じなら描かない。D1・D7・A0・A2 を壊す。
put_cell:
        CMP.B   (A3)+,D3
        BEQ     .ret
        MOVE.B  D3,-1(A3)
        MOVE.L  D0,-(SP)
        LEA     v_cmd,A2
        MOVE.W  D2,D0
        BSR     cmd_rwp
        MOVE.W  #ACRTC_CPY|CPY_DOWN,(A2)+
        MOVE.W  D3,D0
        ADD.W   D0,D0
        ADDI.W  #DIGIT_VRAM,D0
        BSR     cmd_src
        MOVE.W  #1,(A2)+
        MOVE.W  #-7,(A2)+
        BSR     flush
        MOVE.L  (SP)+,D0
.ret    RTS

; ------------------------------------------------------------------ ACRTC

; (A0) から D1.W 語を FIFO に書く。SR の WFE（FIFO が空）が立っていれば、FIFO の深さの 8 語（台帳
; HD63484.md §4.1）までは確かめずに続けて書く。そうでなければ語ごとに WFR を確かめ、空かなければ wait_wfr
; で待ち、打ち切ったら残りの語は捨てる（DR-080、DR-082）。A0 は次の語。D1・D7 を壊す。
send:
        SUBQ.W  #1,D1
        BMI     .ret
.word   MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFE_B,D7
        BEQ     .one
        CMPI.W  #ACRTC_FIFO_WORDS-1,D1
        BLS     .last
        REPT    ACRTC_FIFO_WORDS
        MOVE.W  (A0)+,2(A6)
        ENDR
        SUBQ.W  #ACRTC_FIFO_WORDS,D1
        BRA     .word
.last   MOVE.W  (A0)+,2(A6)
        DBRA    D1,.last
        RTS
.one    BTST    #ACRTC_SR_WFR_B,D7
        BEQ     .wait
.put    MOVE.W  (A0)+,2(A6)
        DBRA    D1,.word
.ret    RTS
.wait   BSR     wait_wfr
        TST.W   D7
        BNE     .put
        ADDQ.W  #1,D1
        ADD.W   D1,D1
        ADDA.W  D1,A0
        RTS

; WFR を待つ。CER が立つか、65536 回（約 0.4 秒）読んでも空かなければ打ち切る。出力：D7.W＝0 は打ち切った。
wait_wfr:
        MOVE.L  D6,-(SP)
        MOVEQ   #-1,D6
.poll   MOVE.W  (A6),D7
        BTST    #ACRTC_SR_CER_B,D7
        BNE     .abort
        BTST    #ACRTC_SR_WFR_B,D7
        DBNE    D6,.poll
        BEQ     .abort
        MOVE.L  (SP)+,D6
        MOVEQ   #1,D7
        RTS
.abort  BSR     acrtc_abort
        MOVE.L  (SP)+,D6
        MOVEQ   #0,D7
        RTS

; ACRTC が終わるのを待つ（FIFO が空で、コマンドが終わった）。段の長さを分けて測るため、段の終わりで呼ぶ。
; 上限と CER は wait_wfr と同じ。D7 を壊す。
wait_idle:
        MOVE.L  D6,-(SP)
        MOVEQ   #-1,D6
.poll   MOVE.W  (A6),D7
        BTST    #ACRTC_SR_CER_B,D7
        BNE     .abort
        ANDI.W  #ACRTC_SR_CED|ACRTC_SR_WFE,D7
        CMPI.W  #ACRTC_SR_CED|ACRTC_SR_WFE,D7
        DBEQ    D6,.poll
        BEQ     .ok
.abort  BSR     acrtc_abort
.ok     MOVE.L  (SP)+,D6
        RTS

; CCR の ABT でコマンドを打ち切り（FIFO も空になる）、AR を r00 に戻して数える。CCR のほかのビットは保つ。
acrtc_abort:
        MOVE.W  #ACRTC_CCR,(A6)
        MOVE.W  2(A6),D7
        ORI.W   #ACRTC_CCR_ABT,D7
        MOVE.W  #ACRTC_CCR,(A6)
        MOVE.W  D7,2(A6)
        ANDI.W  #$FFFF-ACRTC_CCR_ABT,D7
        MOVE.W  #ACRTC_CCR,(A6)
        MOVE.W  D7,2(A6)
        MOVE.W  #ACRTC_FIFO,(A6)
        ADDQ.W  #1,v_err_acrtc
        RTS

; v_cmd から A2 までを送り、A2 を v_cmd に戻す。D1・D7・A0 を壊す。
flush:
        LEA     v_cmd,A0
        MOVE.L  A2,D1
        SUB.L   A0,D1
        LSR.W   #1,D1
        BSR     send
        LEA     v_cmd,A2
        RTS

; RWP を語番地 D0.W にする（DN＝1）。D1 を壊す。
cmd_rwp:
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPH,(A2)+
        MOVE.W  D0,D1
        ROL.W   #4,D1
        ANDI.W  #$000F,D1
        ORI.W   #$4000,D1
        MOVE.W  D1,(A2)+
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPL,(A2)+
        MOVE.W  D0,D1
        LSL.W   #4,D1
        MOVE.W  D1,(A2)+
        RTS

; CPY の元の番地 D0.W（Sah、Sal）を置く。D1 を壊す。
cmd_src:
        MOVE.W  D0,D1
        ROL.W   #4,D1
        ANDI.W  #$000F,D1
        MOVE.W  D1,(A2)+
        MOVE.W  D0,D1
        LSL.W   #4,D1
        MOVE.W  D1,(A2)+
        RTS

; 描く色（CL0 と CL1）を D0.W（0〜15）にする。
cmd_color:
        ANDI.W  #15,D0
        MULU    #$1111,D0
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL0,(A2)+
        MOVE.W  D0,(A2)+
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL1,(A2)+
        MOVE.W  D0,(A2)+
        RTS

; ------------------------------------------------------------------ 起動時の準備

; 上 16、基本 160、下 16 ラスタに分ける。どの画面も 1 行 64 語。基本画面は頁 0 から見せる。
setup_screens:
        LEA     screen_regs,A0
.next   MOVE.W  (A0)+,D0
        BMI     .done
        MOVE.W  D0,(A6)
        MOVE.W  (A0)+,2(A6)
        BRA     .next
.done   MOVE.W  #ACRTC_FIFO,(A6)
        RTS

; VRAM を 0 にし、海の元の絵と玉を DWT で、数字の帯を BIOS_PUTS で置いて、2 つの頁に空と海を描く。
; 立方体の線は空の中だけに描く（ADR の外は描かずに続ける）。
vram_init:
        LEA     v_cmd,A2
        MOVEQ   #0,D0
        BSR     cmd_rwp
        MOVE.W  #ACRTC_CLR,(A2)+
        MOVE.W  #0,(A2)+
        MOVE.W  #63,(A2)+
        MOVE.W  #-(VRAM_ROWS-1),(A2)+
        MOVE.W  #SEA_VRAM,D0
        BSR     cmd_rwp
        MOVE.W  #ACRTC_DWT,(A2)+
        MOVE.W  #63,(A2)+
        MOVE.W  #-(SEA_ROWS-1),(A2)+
        BSR     flush
        LEA     sea_data,A0
        MOVE.W  #64*SEA_ROWS,D1
        BSR     send
        MOVE.W  #BANK_VRAM,D0
        BSR     cmd_rwp
        MOVE.W  #ACRTC_DWT,(A2)+
        MOVE.W  #63,(A2)+
        MOVE.W  #-(SPR_H-1),(A2)+
        BSR     flush
        LEA     bank_data,A0
        MOVE.W  #64*SPR_H,D1
        BSR     send
        BSR     wait_idle
        LEA     str_digits,A0
        MOVEQ   #0,D0
        MOVE.W  #DIGIT_ROW,D1
        MOVEQ   #COL_YELLOW,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
        ; 頁：空（左右の縁を除く）と海
        LEA     page_vram,A3
        MOVEQ   #1,D3
.page   MOVE.W  (A3),D0
        ADDQ.W  #BORDER_W,D0
        BSR     cmd_rwp
        MOVE.W  #ACRTC_CLR,(A2)+
        MOVE.W  #COL_SKY*$1111,(A2)+
        MOVE.W  #SKY_WORDS-1,(A2)+
        MOVE.W  #-(SKY_ROWS-1),(A2)+
        MOVE.W  (A3)+,D0
        ADDI.W  #SKY_ROWS*64,D0
        BSR     cmd_rwp
        MOVE.W  #ACRTC_CPY|CPY_DOWN,(A2)+
        MOVE.W  #SEA_VRAM,D0
        BSR     cmd_src
        MOVE.W  #63,(A2)+
        MOVE.W  #-(SEA_ROWS-1),(A2)+
        BSR     flush
        DBRA    D3,.page
        ; 線を描く領域（論理座標。頁の左上が 0,0、+Y が上）
        LEA     clip_cmds,A0
        MOVEQ   #(clip_cmds_end-clip_cmds)/2,D1
        BSR     send
        BRA     wait_idle

; 上画面の題と字、下画面の欄の名前。
text_init:
        LEA     str_title,A0
        MOVEQ   #0,D0
        MOVEQ   #0,D1
        MOVEQ   #COL_WHITE,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
        BSR     kanji_line_draw
        LEA     str_kana,A0
        MOVEQ   #KANA_X,D0
        MOVEQ   #KANJI_ROW,D1
        MOVEQ   #COL_GREY,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
        LEA     status_labels,A4
.label  MOVE.W  (A4)+,D0                ; x（−1 で終わり）
        BMI     .done
        MOVE.W  (A4)+,D1                ; y
        MOVEA.L (A4)+,A0
        MOVEQ   #COL_GREY,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
        BRA     .label
.done   RTS

; 漢字の行：文字 ROM の字形（JIS X 0208 の区点の枠、仕様書 §08）を CPU が読み、DWT で描く。BIOS_PUTS は
; JIS X 0201 の枠だけを描くので、ここでは BIOS_PUTS と同じ方法（bios-abi.md §07）を区点の offset で行う。
kanji_line_draw:
        LEA     v_cmd,A2
        MOVE.W  #UPPER_VRAM+KANJI_ROW*64+KANJI_X/4,D0
        BSR     cmd_rwp
        MOVE.W  #ACRTC_DWT,(A2)+
        MOVE.W  #2*KANJI_COUNT-1,(A2)+
        MOVE.W  #-7,(A2)+
        LEA     CHAR_ROM,A3
        LEA     nibble_masks,A4
        MOVEQ   #0,D6                   ; 字形の行
.row    LEA     kanji_line,A1
        MOVEQ   #KANJI_COUNT-1,D7
.char   MOVEQ   #0,D5
        MOVE.W  (A1)+,D5                ; 字の offset
        ADD.W   D6,D5
        ADD.L   D5,D5                   ; 上位レーン：番地は offset×2
        MOVEQ   #0,D1
        MOVE.B  0(A3,D5.L),D1           ; 字形の 1 行（MSB が左）
        MOVE.W  D1,D2
        LSR.W   #4,D2
        ADD.W   D2,D2
        MOVE.W  0(A4,D2.W),D0
        ANDI.W  #COL_WHITE*$1111,D0
        MOVE.W  D0,(A2)+
        ANDI.W  #15,D1
        ADD.W   D1,D1
        MOVE.W  0(A4,D1.W),D0
        ANDI.W  #COL_WHITE*$1111,D0
        MOVE.W  D0,(A2)+
        DBRA    D7,.char
        ADDQ.W  #1,D6
        CMPI.W  #8,D6
        BLO     .row
        BRA     flush

; 玉の初めの位置と速さを RAM に写し、玉のコマンドの型を RAM に置く。
objects_init:
        LEA     obj_init,A0
        LEA     v_obj,A1
        MOVE.W  #5*MAX_OBJ-1,D0
.copy   MOVE.W  (A0)+,(A1)+
        DBRA    D0,.copy
        LEA     tpl_rom,A0
        LEA     v_rtpl,A1
        MOVEQ   #RTPL_WORDS+STPL_WORDS-1,D0
.tpl    MOVE.W  (A0)+,(A1)+
        DBRA    D0,.tpl
        RTS

; ラスタの表：基本画面の行ごとに (パレット 1, パレット 2, VIDEO_IRQ に書く語)。VIDEO_IRQ の語は HBL の保留を
; 消し、毎ラインの割込みにする。最後の行は HBL_EN を下げる（次の VBlank のフックまで HBlank を止める）。
rtab_init:
        LEA     grad_table,A0
        LEA     v_rtab,A1
        MOVE.W  #BASE_ROWS-2,D0
.row    MOVE.L  (A0)+,(A1)+
        MOVE.W  #VIRQ_VBL_EN|VIRQ_HBL_EN|VIRQ_HBL,(A1)+
        DBRA    D0,.row
        MOVE.L  (A0)+,(A1)+
        MOVE.W  #VIRQ_VBL_EN|VIRQ_HBL,(A1)+
        RTS

; DSP のプログラムを読み込んで読み返し、Fabric のチャネルを決め、頂点を Staging RAM に置いて DSP を解く。
dsp_load:
        MOVE.W  #DSP_CTRL_RESET,DSP_CTRL
        MOVE.W  #PRG_WAIT-1,D1
.prg    MOVE.W  DSP_STATUS,D0
        BTST    #DSP_ST_PRGBUS_B,D0
        DBNE    D1,.prg
        BEQ     .fail
        LEA     dsp_code,A0
        LEA     DSP_PROGRAM,A1
        MOVE.W  #DSP_CODE_WORDS-1,D1
.copy   MOVE.W  (A0)+,(A1)+
        DBRA    D1,.copy
        LEA     dsp_code,A0
        LEA     DSP_PROGRAM,A1
        MOVE.W  #DSP_CODE_WORDS-1,D1
        MOVEQ   #0,D2
.cmp    CMPM.W  (A0)+,(A1)+
        BEQ     .same
        ADDQ.W  #1,D2
.same   DBRA    D1,.cmp
        TST.W   D2
        BNE     .bad
        BSR     fabric_setup
        LEA     cube_verts,A0
        LEA     STAGING+IN_STAGE+2*IN_HEAD_WORDS,A1
        MOVEQ   #3*CUBE_VERTS-1,D0
.vert   MOVE.W  (A0)+,(A1)+
        DBRA    D0,.vert
        MOVE.W  #(1<<PORT_DSP_IN)|(1<<PORT_DSP_OUT),FABRIC_PORT_RESET
        MOVE.W  #DSP_ST_UNDERRUN|DSP_ST_OVERRUN,DSP_STATUS
        MOVE.W  #DSP_CTRL_BIO_INPUT,DSP_CTRL    ; 解く。BIO＝入力 FIFO が空でない
        MOVE.W  #1,v_dsp_ok
        RTS
.bad    ADD.W   D2,v_err_dsp
        RTS
.fail   ADDQ.W  #1,v_err_dsp
        RTS

; チャネル 0：Staging（IN_STAGE から語）→ port 3、IN_WORDS 語。チャネル 1：port 4 → Staging（OUT_STAGE へ語）、
; OUT_WORDS 語。どちらも即時の 1 回。毎フレーム STATUS を消して START だけを書く。
fabric_setup:
        LEA     FABRIC_CH0,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_DSP_IN,FAB_DST(A0)
        MOVE.W  #IN_STAGE,FAB_SRC_ADDR_L(A0)
        MOVE.W  #0,FAB_DST_ADDR_L(A0)
        MOVE.W  #IN_WORDS,FAB_COUNT(A0)
        MOVE.W  #2,FAB_SRC_STEP(A0)
        MOVE.W  #0,FAB_DST_STEP(A0)
        BSR     .common
        LEA     FABRIC_CH1,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #PORT_DSP_OUT,FAB_SRC(A0)
        MOVE.W  #PORT_STAGING,FAB_DST(A0)
        MOVE.W  #0,FAB_SRC_ADDR_L(A0)
        MOVE.W  #OUT_STAGE,FAB_DST_ADDR_L(A0)
        MOVE.W  #OUT_WORDS,FAB_COUNT(A0)
        MOVE.W  #0,FAB_SRC_STEP(A0)
        MOVE.W  #2,FAB_DST_STEP(A0)
.common MOVE.W  #FMT_16_16,FAB_FORMAT(A0)
        MOVE.W  #$FFFF,FAB_MASK(A0)
        MOVE.W  #0,FAB_XOR(A0)
        MOVE.W  #0,FAB_SHIFT(A0)
        MOVE.W  #TRG_NOW,FAB_TRIGGER(A0)
        RTS

; 起動時の突合せ：いくつかの角で DSP と 68000 の結果を比べ、食い違った語を v_err_xcheck に数える。
xcheck:
        TST.W   v_dsp_ok
        BEQ     .ret
        LEA     xcheck_angles,A5
.set    MOVE.W  (A5)+,D0
        BMI     .ret
        MOVE.W  D0,v_ang_a
        MOVE.W  (A5)+,v_ang_b
        MOVE.W  (A5)+,v_ang_c
        BSR     dsp_kick
        BSR     dsp_fetch
        TST.W   D0
        BNE     .set                    ; 誤りは dsp_fetch が数えた
        MOVEA.L A0,A4
        BSR     xform68
        LEA     v_proj,A1
        MOVEQ   #2*CUBE_VERTS-1,D1
.cmp    CMPM.W  (A4)+,(A1)+
        BEQ     .same
        ADDQ.W  #1,v_err_xcheck
.same   DBRA    D1,.cmp
        BRA     .set
.ret    MOVE.W  #0,v_ang_a
        MOVE.W  #0,v_ang_b
        MOVE.W  #0,v_ang_c
        RTS

; レベル 6 のジャンプ表の項目を差し替え、VBlank のフックを置く（bios-abi.md §03）。
irq_install:
        MOVE.W  #$2700,SR
        MOVE.L  JUMP_TABLE+6*(VEC_AUTO1+IRQ_VIDEO-1)+2,v_bios_irq6
        MOVE.L  #irq6,JUMP_TABLE+6*(VEC_AUTO1+IRQ_VIDEO-1)+2
        MOVE.L  #vbl_hook,bios_vbl_hook
        MOVE.W  #$2000,SR
        RTS

; ------------------------------------------------------------------ 割込み

; レベル 6。HBlank の保留なら、ラスタの表（USP が指す）の 1 行ぶんをパレット 1・2 と VIDEO_IRQ に書く。
; HBlank の保留が立つのはラスタ r の表示の終わり（ドット 320）で、書いた色はラスタ r＋2 の頭の写しから効く。
; そうでなければ（VBlank）、差し替える前の BIOS の処理へ同じ例外のフレームのまま進む。
irq6:
        MOVE.L  D0,-(SP)
        MOVE.W  VIDEO_IRQ,D0
        BTST    #9,D0                   ; VIRQ_HBL
        BEQ     .bios
        MOVE.L  A0,-(SP)
        MOVE    USP,A0
        MOVE.L  (A0)+,PALETTE+2         ; パレット 1・2
        MOVE.W  (A0)+,VIDEO_IRQ         ; 保留を消す。最後の行は HBL_EN を下げる
        MOVE    A0,USP
        MOVEA.L (SP)+,A0
        MOVE.L  (SP)+,D0
        RTE
.bios   MOVE.L  (SP)+,D0
        MOVE.L  v_bios_irq6,-(SP)
        RTS

; VBlank のフック（BIOS の VBlank の処理から JSR。D0–D1/A0–A1 は壊してよい）。ラスタのパレットが入って
; いれば、表の頭を USP に置き、RASTER_ARM の HBlank だけで割込むようにして、コッパーバーを動かす。
; 入っていなければ HBlank を止め、パレット 1・2 を固定の色にする。
vbl_hook:
        TST.W   v_raster
        BEQ     .off
        LEA     v_rtab,A0
        MOVE    A0,USP
        MOVE.W  #RASTER_ARM,VIDEO_LINE_CMP
        MOVE.W  #VIRQ_VBL_EN|VIRQ_HBL_EN|VIRQ_LINE_CMP|VIRQ_HBL,VIDEO_IRQ
        BRA     bars_update
.off    MOVE.W  #VIRQ_VBL_EN|VIRQ_HBL,VIDEO_IRQ
        MOVE.L  #(FIXED_C1<<16)|FIXED_C2,PALETTE+2
        RTS

; コッパーバー：前のフレームの 2 本の行を空の色に戻し、位相を進めて、奥のバー、手前のバーの順に塗る。
; バー A は余弦が 0 以上のとき手前（B は反対の側を回る）。
bars_update:
        MOVEM.L D2-D3/A2,-(SP)
        LEA     v_rtab,A0
        LEA     grad_table,A1
        MOVE.W  v_bar_a,D0
        BSR     bar_clear
        MOVE.W  v_bar_b,D0
        BSR     bar_clear
        MOVE.W  v_bar_t,D2
        ADDQ.W  #BAR_SPEED,D2
        ANDI.W  #255,D2
        MOVE.W  D2,v_bar_t
        LEA     bar_pos,A2
        MOVE.W  D2,D0
        ADD.W   D0,D0
        MOVE.W  0(A2,D0.W),v_bar_a
        MOVE.W  D2,D0
        ADDI.W  #128,D0
        ANDI.W  #255,D0
        ADD.W   D0,D0
        MOVE.W  0(A2,D0.W),v_bar_b
        LEA     sin_table,A2
        ADDI.W  #64,D2
        ANDI.W  #255,D2
        ADD.W   D2,D2
        TST.W   0(A2,D2.W)
        BMI     .a_back
        MOVE.W  v_bar_b,D0
        LEA     bar_colors_b,A2
        BSR     bar_paint
        MOVE.W  v_bar_a,D0
        LEA     bar_colors_a,A2
        BSR     bar_paint
        BRA     .ret
.a_back MOVE.W  v_bar_a,D0
        LEA     bar_colors_a,A2
        BSR     bar_paint
        MOVE.W  v_bar_b,D0
        LEA     bar_colors_b,A2
        BSR     bar_paint
.ret    MOVEM.L (SP)+,D2-D3/A2
        RTS

; 空の行 D0.W から BAR_H 行のパレット 1 を grad_table（A1）の色に戻す。A0＝v_rtab。D0・D1・D3 を壊す。
bar_clear:
        MOVE.W  D0,D1
        LSL.W   #2,D1
        MULU    #6,D0
        MOVEQ   #BAR_H-1,D3
.row    MOVE.W  0(A1,D1.W),0(A0,D0.W)
        ADDQ.W  #4,D1
        ADDQ.W  #6,D0
        DBRA    D3,.row
        RTS

; 空の行 D0.W から BAR_H 行のパレット 1 を (A2)+ の色にする。A0＝v_rtab。D0・D3 を壊す。
bar_paint:
        MULU    #6,D0
        MOVEQ   #BAR_H-1,D3
.row    MOVE.W  (A2)+,0(A0,D0.W)
        ADDQ.W  #6,D0
        DBRA    D3,.row
        RTS

; ------------------------------------------------------------------ 表と文字列（文字 ROM に字形のある字だけ）

page_vram:
        DC.W    PAGE0_VRAM,PAGE1_VRAM
; 頁の ORG：DPH＝DN<<14｜番地>>12、DPL＝(番地 & $FFF)<<4（ドット 0）
page_org:
        DC.W    $4000|(PAGE0_VRAM>>12),(PAGE0_VRAM&$FFF)<<4
        DC.W    $4000|(PAGE1_VRAM>>12),(PAGE1_VRAM&$FFF)<<4

screen_regs:
        DC.W    ACRTC_SSW0,UPPER_H
        DC.W    ACRTC_SSW,BASE_ROWS
        DC.W    ACRTC_SSW2,LOWER_H
        DC.W    ACRTC_MWR0,64
        DC.W    ACRTC_SAR0H,0
        DC.W    ACRTC_SAR0L,UPPER_VRAM
        DC.W    ACRTC_MWR1,64
        DC.W    ACRTC_SAR1H,0
        DC.W    ACRTC_SAR1L,PAGE0_VRAM
        DC.W    ACRTC_MWR2,64
        DC.W    ACRTC_SAR2H,0
        DC.W    ACRTC_SAR2L,LOWER_VRAM
        DC.W    ACRTC_DCR,ACRTC_DCR_BASE|ACRTC_DCR_UPPER|ACRTC_DCR_LOWER
        DC.W    -1

; 玉のコマンドの型。0 の語は段 R・S が毎回書く（RWP、元の Sah・Sal）
tpl_rom:
        DC.W    ACRTC_WPR|ACRTC_P_RWPH,0,ACRTC_WPR|ACRTC_P_RWPL,0
        DC.W    ACRTC_CPY|CPY_DOWN,0,0,SPR_WORDS-1,-(SPR_H-1)
        DC.W    ACRTC_WPR|ACRTC_P_RWPH,0,ACRTC_WPR|ACRTC_P_RWPL,0
        DC.W    ACRTC_SCPY|CPY_DOWN|ACRTC_OPM_AND,0,0,SPR_WORDS-1,-(SPR_H-1)
        DC.W    ACRTC_WPR|ACRTC_P_RWPH,0,ACRTC_WPR|ACRTC_P_RWPL,0
        DC.W    ACRTC_SCPY|CPY_DOWN|ACRTC_OPM_OR,0,0,SPR_WORDS-1,-(SPR_H-1)
tpl_rom_end:
        ASSERT  tpl_rom_end-tpl_rom=2*(RTPL_WORDS+STPL_WORDS)

; 玉の型と絵の元（Sah・Sal）：種類 v、横のずれ s ごとに (型, 絵)。型は BANK_MASK＋5s、絵は
; BANK_IMAGE＋20v＋5s（build.py の make_bank）
SRC_HI          EQU     BANK_VRAM>>12
SRC_LO          EQU     BANK_VRAM&$FFF
        ASSERT  (BANK_VRAM+64*SPR_H-1)>>12=SRC_HI
spr_src:
        DC.W    SRC_HI,(SRC_LO+BANK_MASK+0)<<4,SRC_HI,(SRC_LO+BANK_IMAGE+0)<<4
        DC.W    SRC_HI,(SRC_LO+BANK_MASK+5)<<4,SRC_HI,(SRC_LO+BANK_IMAGE+5)<<4
        DC.W    SRC_HI,(SRC_LO+BANK_MASK+10)<<4,SRC_HI,(SRC_LO+BANK_IMAGE+10)<<4
        DC.W    SRC_HI,(SRC_LO+BANK_MASK+15)<<4,SRC_HI,(SRC_LO+BANK_IMAGE+15)<<4
        DC.W    SRC_HI,(SRC_LO+BANK_MASK+0)<<4,SRC_HI,(SRC_LO+BANK_IMAGE+20)<<4
        DC.W    SRC_HI,(SRC_LO+BANK_MASK+5)<<4,SRC_HI,(SRC_LO+BANK_IMAGE+25)<<4
        DC.W    SRC_HI,(SRC_LO+BANK_MASK+10)<<4,SRC_HI,(SRC_LO+BANK_IMAGE+30)<<4
        DC.W    SRC_HI,(SRC_LO+BANK_MASK+15)<<4,SRC_HI,(SRC_LO+BANK_IMAGE+35)<<4

clip_cmds:
        DC.W    ACRTC_WPR|ACRTC_P_ADR_XMIN,4*BORDER_W
        DC.W    ACRTC_WPR|ACRTC_P_ADR_YMIN,-(SKY_ROWS-1)
        DC.W    ACRTC_WPR|ACRTC_P_ADR_XMAX,255-4*BORDER_W
        DC.W    ACRTC_WPR|ACRTC_P_ADR_YMAX,0
clip_cmds_end:

; 起動時の突合せの角（A, B, C）。−1 で終わり
xcheck_angles:
        DC.W    0,0,0
        DC.W    17,85,200
        DC.W    100,33,250
        DC.W    255,128,64
        DC.W    -1

; 上画面の印：x、点いているときの色、消えているときの色、文字列（点いている、消えている）
flag_items:
        DC.W    16*8,COL_WHITE,COL_WHITE        ; 黄は時間の帯の段 X の色と重なる
        DC.L    str_dsp,str_68k
        DC.W    20*8,COL_WHITE,COL_GREY
        DC.L    str_ras,str_ras
        DC.W    24*8,COL_WHITE,COL_GREY
        DC.L    str_bar,str_bar
        DC.W    28*8,COL_WHITE,COL_GREY
        DC.L    str_spr,str_spr

; 下画面の欄の名前：x、y、文字列。−1 で終わり
status_labels:
        DC.W    0*8,LOWER_ROW
        DC.L    str_obj
        DC.W    8*8,LOWER_ROW
        DC.L    str_hz
        DC.W    15*8,LOWER_ROW
        DC.L    str_busy
        DC.W    25*8,LOWER_ROW
        DC.L    str_err
        DC.W    0*8,LOWER_ROW+8
        DC.L    str_phases
        DC.W    -1

; 下画面の数の欄：最上位の桁の語番地、桁の数、値の番地。−1 で終わり（v_cells は合わせて 25 字）
ROW0            EQU     LOWER_VRAM
ROW1            EQU     LOWER_VRAM+8*64
status_fields:
        DC.W    ROW0+2*4,2
        DC.L    v_obj_shown
        DC.W    ROW0+2*11,2
        DC.L    v_hz
        DC.W    ROW0+2*20,3
        DC.L    v_show+2*PH_BUSY
        DC.W    ROW0+2*29,2
        DC.L    v_err_sum
        DC.W    ROW1+2*1,2
        DC.L    v_show+2*PH_K
        DC.W    ROW1+2*5,3
        DC.L    v_show+2*PH_R
        DC.W    ROW1+2*10,2
        DC.L    v_show+2*PH_E
        DC.W    ROW1+2*14,2
        DC.L    v_show+2*PH_X
        DC.W    ROW1+2*18,2
        DC.L    v_show+2*PH_C
        DC.W    ROW1+2*22,3
        DC.L    v_show+2*PH_S
        DC.W    ROW1+2*27,2
        DC.L    v_show+2*PH_T
        DC.W    -1

str_title:      DC.B    "SAMPLE 6",0
str_kana:       DC.B    "ﾊﾝｶｸ ｶﾀｶﾅ",0
str_digits:     DC.B    "0123456789 ",0
str_dsp:        DC.B    "DSP",0
str_68k:        DC.B    "68K",0
str_ras:        DC.B    "RAS",0
str_bar:        DC.B    "BAR",0
str_spr:        DC.B    "SPR",0
str_obj:        DC.B    "OBJ",0
str_hz:         DC.B    "HZ",0
str_busy:       DC.B    "BUSY",0
str_err:        DC.B    "ERR",0
str_phases:     DC.B    "K   R    E   X   C   S    T",0
        EVEN

        INCLUDE "tables.inc"            ; build.py が作る（ラスタの表、正弦、透視、立方体、玉、漢字の offset）
        EVEN

sea_data:
        INCBIN  "sea.bin"               ; 64 行×64 語
bank_data:
        INCBIN  "bank.bin"              ; 16 行×64 語
dsp_code:
        INCBIN  "xform.bin"
dsp_code_end:
DSP_CODE_WORDS  EQU     (dsp_code_end-dsp_code)/2
        ASSERT  DSP_CODE_WORDS<=DSP_PROGRAM_WORDS
        EVEN
rom_end:
