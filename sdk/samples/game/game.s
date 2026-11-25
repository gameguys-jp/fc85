; SAMPLE 1：段階 3 のサンプルのカートリッジ。m68kasm で組む（build.py）。説明は README.md。
;
; 遊び方：水色の輪（自機）を方向キーで動かし、黄の菱形を拾い（10 点）、赤の石をよける。石に当たると残りが
; 1 減り、0 で終わる。菱形を 5 個拾うごとに石が 1 個増える（8 個まで）。START で始め、終わった後も START で
; 始め直す。C でデバッグの行（ACRTC の仕事のライン数、ADPCM の UNDERRUN など）を出し入れする。
; デバッグの行を出している間は、B で石を足し、A で拍の低音を止める・戻す（試験用）。
;
; 描画：ACRTC だけを使う。動く物は XOR（OPM＝011）で描き、次のフレームに同じ図形を XOR で描いて消す。
; 下の絵を壊さないので、物が重なっても戻る。ACRTC の仕事は VBlank の直後にまとめ、ゲームの計算はその後。
; 文字は BIOS_PUTS・BIOS_PUT_HEX（点数は BCD で持ち、16 進の表示がそのまま 10 進になる）。
;
; 音：効果音は MSM5205 の ADPCM。カートリッジの ADPCM の byte を起動時に Staging RAM に写し、鳴らすときは
; Data Fabric のチャネル 0 が port 0（Staging）→ port 5（ADPCM の FIFO）へ運ぶ（sfx.s）。拍の低音は
; FABRIC_MODULES を見て、DOC の子基板があれば DOC、なければ本体 DAC で鳴らす（music.s）。
;
; 番地とビットの名前は fc85.inc、BIOS の約束は bios.inc（sdk/docs/bios-abi.md）。

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"
        INCLUDE "sfx.inc"               ; build.py が作る（効果音の offset・長さ・S1/S2）

; ------------------------------------------------------------------ 定数

; 状態
ST_TITLE        EQU     0
ST_PLAY         EQU     1
ST_OVER         EQU     2

; 画面の配置（画面の左上が 0,0）
HUD_Y           EQU     2               ; 上の帯の文字
SCORE_X         EQU     52
HI_X            EQU     132
LIFE_X          EQU     236
FIELD_X0        EQU     4               ; 動く物が入る範囲（両端を含む）
FIELD_Y0        EQU     18
FIELD_X1        EQU     251
FIELD_Y1        EQU     179
DBG_Y           EQU     184             ; デバッグの行

; 物の大きさと速さ
PLAYER_SIZE     EQU     10
PLAYER_SPEED    EQU     2
PLAYER_X0       EQU     123
PLAYER_Y0       EQU     94
ROCK_SIZE       EQU     8
GEM_SIZE        EQU     6
ROCK_MAX        EQU     8
GEM_MAX         EQU     3
ROCKS_START     EQU     2
GEMS_PER_ROCK   EQU     5               ; 拾うと石が増える数
GEM_DELAY       EQU     30              ; 拾ってから次の菱形が出るまで（フレーム）
INVULN_FRAMES   EQU     120             ; 当たった後の無敵（点滅）
FLASH_FRAMES    EQU     8               ; 当たったときに背景を赤くする
OVER_WAIT       EQU     90              ; 終わってから START を受けるまで

; 乱数の位置の幅
GEM_RANGE_X     EQU     FIELD_X1-FIELD_X0-GEM_SIZE+2
GEM_RANGE_Y     EQU     FIELD_Y1-FIELD_Y0-GEM_SIZE+2
ROCK_RANGE_X    EQU     FIELD_X1-FIELD_X0-ROCK_SIZE+2
ROCK_RANGE_Y    EQU     FIELD_Y1-FIELD_Y0-ROCK_SIZE+2

; 色（パレットの番号。パレットは BIOS の既定と同じ値）
COL_BG          EQU     0
COL_BAND        EQU     2
COL_ROCK        EQU     3
COL_GEM         EQU     5
COL_PLAYER      EQU     7
COL_BORDER      EQU     8
COL_DIM         EQU     11
COL_BLINK       EQU     14              ; パレットで点滅させる文字
COL_TEXT        EQU     15

; 書き直す物（g_hud のビット）
HUD_SCORE       EQU     $0001
HUD_HI          EQU     $0002
HUD_LIVES       EQU     $0004
HUD_OVER        EQU     $0008           ; 終わりの枠（次のフレームに T1、その次に T2）
HUD_OVER_T1     EQU     $0010
HUD_OVER_T2     EQU     $0020

; 効果音の番号（build.py の SOUNDS と同じ順）
SFX_START       EQU     0
SFX_COLLECT     EQU     1
SFX_HIT         EQU     2
SFX_OVER        EQU     3

; 物（自機・石・菱形）の形
        RSRESET
OB_ON           RS.W    1               ; 0 なし、1 あり
OB_X            RS.W    1               ; 左上
OB_Y            RS.W    1
OB_VX           RS.W    1               ; 石の速さ
OB_VY           RS.W    1
OB_DRAWN        RS.W    1               ; 画面に描いてあるか
OB_DX           RS.W    1               ; 描いた位置
OB_DY           RS.W    1
OB_TIMER        RS.W    1               ; 菱形：次に出るまで
OB_SIZE         RS.B    0

; ------------------------------------------------------------------ Main RAM（$100800〜、bios-abi.md §03）

        RSSET   APP_RAM
g_state         RS.W    1
g_pad           RS.W    1               ; 今の PORT_A
g_new           RS.W    1               ; 新しく押したボタン
g_seed          RS.W    1               ; 乱数
g_color         RS.W    1               ; ACRTC の CL0・CL1 に入れてある色（−1 は不明）
g_score         RS.L    1               ; BCD 8 桁
g_score_shown   RS.L    1               ; 画面に出してある点数（変わった桁だけ描き直す）
g_hi            RS.L    1               ; BCD 8 桁
g_hi_shown      RS.L    1               ; 画面の帯に出してある最高点
g_digit         RS.B    2               ; 1 字の文字列
g_lives         RS.W    1
g_rock_cd       RS.W    1               ; 石が増えるまでに拾う数
g_hud           RS.W    1               ; 書き直す物（HUD_*）
g_invuln        RS.W    1
g_flash         RS.W    1
g_over_timer    RS.W    1
g_blink         RS.W    1
g_player        RS.B    OB_SIZE         ; 自機・石・菱形の順に続ける（objects_init）
g_rocks         RS.B    OB_SIZE*ROCK_MAX
g_gems          RS.B    OB_SIZE*GEM_MAX
g_objects_end   RS.B    0
dbg_on          RS.W    1               ; デバッグの行を出すか
dbg_dirty       RS.W    1               ; 出し入れを変えた
dbg_line0       RS.W    1               ; VBlank の直後のラスタ
dbg_lines       RS.W    1               ; このフレームの ACRTC の仕事のライン数
dbg_max         RS.W    1               ; その最大
dbg_buf         RS.B    32
; 効果音（sfx.s）
sfx_state       RS.W    1               ; SFX_IDLE・SFX_FLUSH・SFX_ARMED・SFX_PLAYING
sfx_cur         RS.W    1               ; 鳴らしている音の番号
sfx_prio        RS.W    1               ; その強さ（0 は鳴っていない）
sfx_req         RS.W    1               ; 次に鳴らす音の番号＋1（0 はなし）
sfx_req_prio    RS.W    1
sfx_underruns   RS.W    1               ; 本体を送っている間に ADPCM の FIFO が空になった数
sfx_plays       RS.W    1               ; 鳴らし始めた数
; 拍の低音（music.s）
mu_mode         RS.W    1               ; 0 なし、1 DOC、2 本体 DAC
mu_on           RS.W    1
mu_timer        RS.W    1               ; 次の拍までのフレーム
mu_step         RS.W    1               ; 4 つの音のどれか
mu_env          RS.W    1               ; 包絡
mu_env_out      RS.W    1               ; 最後に出した包絡
g_vars_end      RS.B    0
        ASSERT  g_vars_end<=$110000

; ------------------------------------------------------------------ 見出し（bios-abi.md §04）

        ORG     CART
cart_header:
        DC.L    CART_MAGIC
        DC.W    CART_HDR_VERSION
        DC.W    0
        DC.L    start
        DC.L    rom_end-CART
        DC.B    "SAMPLE 1        "      ; 題名（16 字、空白で埋める）
        ASSERT  *=CART+cart_header_size

; ------------------------------------------------------------------ 入口

; 入口の条件（bios-abi.md §04）：スーパバイザ、SR＝$2000、SSP＝$120000、D1.L＝FABRIC_MODULES、画面は色 0、
; Data Fabric の両チャネルは止まっている、音は黙らせてある。
start:
        LEA     INITIAL_SSP,SP
        LEA     APP_RAM,A0
        MOVE.W  #(g_vars_end-APP_RAM+3)/4-1,D0
.clr    CLR.L   (A0)+
        DBRA    D0,.clr
        MOVE.W  #$ACE1,g_seed
        MOVE.W  #-1,g_color
        LEA     ACRTC_AR,A6             ; A6 はいつも ACRTC（AR は r00 のまま使う）
        BSR     sfx_init
        BSR     music_init
        BSR     title_screen

main_loop:
        JSR     BIOS_WAIT_VBL
        MOVE.W  VIDEO_LINE,dbg_line0
        BSR     draw_frame              ; ACRTC の仕事はここだけ（VBlank の中で終える）
        CMP.W   #ST_PLAY,g_state
        BNE.S   .nomeasure
        BSR     measure
.nomeasure
        BSR     dbg_phase
        BSR     input
        BSR     update
        BSR     sfx_update
        BSR     music_update
        BRA.S   main_loop

; ------------------------------------------------------------------ 入力

input:
        JSR     BIOS_READ_PAD
        MOVE.W  D0,g_pad
        MOVE.W  D1,g_new
        BTST    #PAD_C_B,D1
        BEQ.S   .b
        EORI.W  #1,dbg_on
        MOVE.W  #1,dbg_dirty
.b      TST.W   dbg_on                  ; デバッグの行を出している間だけ（試験用）
        BEQ.S   .ret
        CMP.W   #ST_PLAY,g_state
        BNE.S   .ret
        BTST    #PAD_B_B,D1             ; B：石を足す
        BEQ.S   .a
        BSR     rock_add
.a      BTST    #PAD_A_B,g_new+1        ; A：拍の低音を止める・戻す（効果音だけを録るため）
        BEQ.S   .ret
        TST.W   mu_on
        BEQ     music_start
        BRA     music_stop
.ret    RTS

; ------------------------------------------------------------------ 状態ごとの計算

update:
        ADDQ.W  #1,g_blink
        MOVE.W  g_state,D0
        BEQ.S   update_title
        CMP.W   #ST_PLAY,D0
        BEQ.S   update_play
        BRA     update_over

update_title:
        BSR     blink_text
        MOVE.W  g_new,D0
        BTST    #PAD_START_B,D0
        BNE     new_game
        RTS

update_play:
        BSR     player_move
        BSR     rocks_move
        BSR     gems_update
        BSR     rocks_hit
        TST.W   g_invuln
        BEQ.S   .ret
        SUBQ.W  #1,g_invuln
.ret    RTS

update_over:
        BSR     blink_text
        TST.W   g_over_timer
        BEQ.S   .start
        SUBQ.W  #1,g_over_timer
        RTS
.start  MOVE.W  g_new,D0
        BTST    #PAD_START_B,D0
        BNE     new_game
        RTS

; 「PUSH START」の色（パレットの 14 番）を 32 フレームごとに点けたり消したりする。
blink_text:
        MOVE.W  g_blink,D0
        ANDI.W  #31,D0
        BNE.S   .ret
        BTST    #5,g_blink+1
        BEQ.S   .on
        MOVE.W  #$000,PALETTE+2*COL_BLINK
        RTS
.on     MOVE.W  #$FFA,PALETTE+2*COL_BLINK
.ret    RTS

; ------------------------------------------------------------------ ゲームの始まり

new_game:
        MOVE.L  bios_frame,D0           ; 乱数の種に START を押したフレームを混ぜる
        MOVE.W  g_seed,D1
        EOR.W   D0,D1
        BNE.S   .seed
        MOVE.W  #$ACE1,D1
.seed   MOVE.W  D1,g_seed
        CLR.L   g_score
        MOVE.W  #3,g_lives
        CLR.W   g_invuln
        CLR.W   g_flash
        MOVE.W  #GEMS_PER_ROCK,g_rock_cd
        BSR     objects_init
        MOVE.L  #-1,g_score_shown       ; 点数と最高点の 6 桁を全部描く
        MOVE.L  #-1,g_hi_shown
        MOVE.W  #HUD_SCORE|HUD_HI|HUD_LIVES,g_hud
        BSR     play_screen             ; 帯・枠・数・最初の物を描いてから見せる
        MOVE.W  #ST_PLAY,g_state
        CLR.W   dbg_max
        MOVE.W  #1,dbg_dirty
        BSR     music_start
        MOVEQ   #SFX_START,D0
        BRA     sfx_play

; 自機・石・菱形を最初の位置に置く。最初の配置は決まった位置（台本で試せるように）[P]。
objects_init:
        LEA     g_player,A0
        MOVE.W  #(g_objects_end-g_player)/2-1,D0
.clr    CLR.W   (A0)+
        DBRA    D0,.clr
        MOVE.W  #1,g_player+OB_ON
        MOVE.W  #PLAYER_X0,g_player+OB_X
        MOVE.W  #PLAYER_Y0,g_player+OB_Y
        LEA     init_gems(PC),A1
        LEA     g_gems,A0
        MOVEQ   #GEM_MAX-1,D0
.gem    MOVE.W  #1,OB_ON(A0)
        MOVE.W  (A1)+,OB_X(A0)
        MOVE.W  (A1)+,OB_Y(A0)
        LEA     OB_SIZE(A0),A0
        DBRA    D0,.gem
        LEA     init_rocks(PC),A1
        LEA     g_rocks,A0
        MOVEQ   #ROCKS_START-1,D0
.rock   MOVE.W  #1,OB_ON(A0)
        MOVE.W  (A1)+,OB_X(A0)
        MOVE.W  (A1)+,OB_Y(A0)
        MOVE.W  (A1)+,OB_VX(A0)
        MOVE.W  (A1)+,OB_VY(A0)
        LEA     OB_SIZE(A0),A0
        DBRA    D0,.rock
        RTS

init_gems:
        DC.W    60,96, 192,96, 126,40
init_rocks:
        DC.W    12,24,1,1, 232,166,-1,-1

; ------------------------------------------------------------------ 自機・石・菱形の計算

player_move:
        LEA     g_player,A0
        MOVE.W  g_pad,D2
        MOVE.W  OB_X(A0),D0
        MOVE.W  OB_Y(A0),D1
        BTST    #PAD_LEFT_B,D2
        BEQ.S   .r
        SUBQ.W  #PLAYER_SPEED,D0
.r      BTST    #PAD_RIGHT_B,D2
        BEQ.S   .u
        ADDQ.W  #PLAYER_SPEED,D0
.u      BTST    #PAD_UP_B,D2
        BEQ.S   .d
        SUBQ.W  #PLAYER_SPEED,D1
.d      BTST    #PAD_DOWN_B,D2
        BEQ.S   .x0
        ADDQ.W  #PLAYER_SPEED,D1
.x0     CMP.W   #FIELD_X0,D0
        BGE.S   .x1
        MOVEQ   #FIELD_X0,D0
.x1     CMP.W   #FIELD_X1-PLAYER_SIZE+1,D0
        BLE.S   .y0
        MOVE.W  #FIELD_X1-PLAYER_SIZE+1,D0
.y0     CMP.W   #FIELD_Y0,D1
        BGE.S   .y1
        MOVEQ   #FIELD_Y0,D1
.y1     CMP.W   #FIELD_Y1-PLAYER_SIZE+1,D1
        BLE.S   .done
        MOVE.W  #FIELD_Y1-PLAYER_SIZE+1,D1
.done   MOVE.W  D0,OB_X(A0)
        MOVE.W  D1,OB_Y(A0)
        RTS

; 石を動かし、枠で跳ね返す。
rocks_move:
        LEA     g_rocks,A0
        MOVEQ   #ROCK_MAX-1,D7
.next   TST.W   OB_ON(A0)
        BEQ.S   .skip
        MOVE.W  OB_X(A0),D0
        ADD.W   OB_VX(A0),D0
        CMP.W   #FIELD_X0,D0
        BGE.S   .x1
        MOVEQ   #FIELD_X0,D0
        NEG.W   OB_VX(A0)
        BRA.S   .xok
.x1     CMP.W   #FIELD_X1-ROCK_SIZE+1,D0
        BLE.S   .xok
        MOVE.W  #FIELD_X1-ROCK_SIZE+1,D0
        NEG.W   OB_VX(A0)
.xok    MOVE.W  D0,OB_X(A0)
        MOVE.W  OB_Y(A0),D0
        ADD.W   OB_VY(A0),D0
        CMP.W   #FIELD_Y0,D0
        BGE.S   .y1
        MOVEQ   #FIELD_Y0,D0
        NEG.W   OB_VY(A0)
        BRA.S   .yok
.y1     CMP.W   #FIELD_Y1-ROCK_SIZE+1,D0
        BLE.S   .yok
        MOVE.W  #FIELD_Y1-ROCK_SIZE+1,D0
        NEG.W   OB_VY(A0)
.yok    MOVE.W  D0,OB_Y(A0)
.skip   LEA     OB_SIZE(A0),A0
        DBRA    D7,.next
        RTS

; 菱形：自機と重なれば拾う。拾った菱形は GEM_DELAY フレームの後に別の位置に出る。
gems_update:
        LEA     g_gems,A0
        MOVEQ   #GEM_MAX-1,D7
.next   TST.W   OB_ON(A0)
        BNE.S   .active
        TST.W   OB_TIMER(A0)
        BEQ.S   .spawn
        SUBQ.W  #1,OB_TIMER(A0)
        BRA.S   .skip
.spawn  BSR     gem_place
        BRA.S   .skip
.active MOVE.W  g_player+OB_X,D0
        MOVE.W  g_player+OB_Y,D1
        MOVEQ   #PLAYER_SIZE,D2
        MOVE.W  OB_X(A0),D3
        MOVE.W  OB_Y(A0),D4
        MOVEQ   #GEM_SIZE,D5
        BSR     hit_test
        BEQ.S   .skip
        CLR.W   OB_ON(A0)
        MOVE.W  #GEM_DELAY,OB_TIMER(A0)
        BSR     collect
.skip   LEA     OB_SIZE(A0),A0
        DBRA    D7,.next
        RTS

; 菱形（A0）を、自機から離れた乱数の位置に置く。
gem_place:
        MOVEM.L D0-D7,-(SP)
        MOVEQ   #7,D7                   ; 試す回数−1
.try    BSR     rand
        MULU    #GEM_RANGE_X,D0
        SWAP    D0
        ADDQ.W  #FIELD_X0,D0
        MOVE.W  D0,D3
        BSR     rand
        MULU    #GEM_RANGE_Y,D0
        SWAP    D0
        ADDI.W  #FIELD_Y0,D0
        MOVE.W  D0,D4
        MOVEQ   #GEM_SIZE,D5
        MOVE.W  g_player+OB_X,D0
        SUBI.W  #16,D0
        MOVE.W  g_player+OB_Y,D1
        SUBI.W  #16,D1
        MOVEQ   #PLAYER_SIZE+32,D2
        BSR     hit_test
        DBEQ    D7,.try
        MOVE.W  D3,OB_X(A0)
        MOVE.W  D4,OB_Y(A0)
        MOVE.W  #1,OB_ON(A0)
        MOVEM.L (SP)+,D0-D7
        RTS

; 拾った：10 点、拾った数で石を増やす、音。
collect:
        MOVEM.L D0-D1/A0,-(SP)
        MOVEQ   #$10,D0                 ; BCD の 10
        BSR     add_score
        SUBQ.W  #1,g_rock_cd
        BNE.S   .snd
        MOVE.W  #GEMS_PER_ROCK,g_rock_cd
        BSR     rock_add
.snd    MOVEQ   #SFX_COLLECT,D0
        BSR     sfx_play
        MOVEM.L (SP)+,D0-D1/A0
        RTS

; 空いている石の場所に、自機から離れた位置と乱数の向きで石を出す。
rock_add:
        MOVEM.L D0-D7/A0,-(SP)
        LEA     g_rocks,A0
        MOVEQ   #ROCK_MAX-1,D7
.find   TST.W   OB_ON(A0)
        BEQ.S   .found
        LEA     OB_SIZE(A0),A0
        DBRA    D7,.find
        BRA     .ret
.found  MOVEQ   #7,D7
.try    BSR     rand
        MULU    #ROCK_RANGE_X,D0
        SWAP    D0
        ADDQ.W  #FIELD_X0,D0
        MOVE.W  D0,D3
        BSR     rand
        MULU    #ROCK_RANGE_Y,D0
        SWAP    D0
        ADDI.W  #FIELD_Y0,D0
        MOVE.W  D0,D4
        MOVEQ   #ROCK_SIZE,D5
        MOVE.W  g_player+OB_X,D0
        SUBI.W  #40,D0
        MOVE.W  g_player+OB_Y,D1
        SUBI.W  #40,D1
        MOVEQ   #PLAYER_SIZE+80,D2
        BSR     hit_test
        DBEQ    D7,.try
        MOVE.W  D3,OB_X(A0)
        MOVE.W  D4,OB_Y(A0)
        BSR     rand                    ; 速さ：x・y とも ±1 か ±2
        MOVEQ   #1,D1
        BTST    #0,D0
        BEQ.S   .vx
        MOVEQ   #2,D1
.vx     BTST    #1,D0
        BEQ.S   .vxs
        NEG.W   D1
.vxs    MOVE.W  D1,OB_VX(A0)
        MOVEQ   #1,D1
        BTST    #2,D0
        BEQ.S   .vy
        MOVEQ   #2,D1
.vy     BTST    #3,D0
        BEQ.S   .vys
        NEG.W   D1
.vys    MOVE.W  D1,OB_VY(A0)
        CLR.W   OB_DRAWN(A0)
        MOVE.W  #1,OB_ON(A0)
.ret    MOVEM.L (SP)+,D0-D7/A0
        RTS

; 石と自機。当たり判定は見た目より 1〜2 画素小さい。
rocks_hit:
        TST.W   g_invuln
        BNE.S   .ret
        LEA     g_rocks,A0
        MOVEQ   #ROCK_MAX-1,D7
.next   TST.W   OB_ON(A0)
        BEQ.S   .skip
        MOVE.W  g_player+OB_X,D0
        ADDQ.W  #2,D0
        MOVE.W  g_player+OB_Y,D1
        ADDQ.W  #2,D1
        MOVEQ   #PLAYER_SIZE-4,D2
        MOVE.W  OB_X(A0),D3
        ADDQ.W  #1,D3
        MOVE.W  OB_Y(A0),D4
        ADDQ.W  #1,D4
        MOVEQ   #ROCK_SIZE-2,D5
        BSR     hit_test
        BNE.S   player_hit
.skip   LEA     OB_SIZE(A0),A0
        DBRA    D7,.next
.ret    RTS

; 当たった：残りを 1 減らす。0 なら終わり。
player_hit:
        SUBQ.W  #1,g_lives
        ORI.W   #HUD_LIVES,g_hud
        MOVE.W  #FLASH_FRAMES,g_flash
        TST.W   g_lives
        BEQ.S   .over
        MOVE.W  #INVULN_FRAMES,g_invuln
        MOVEQ   #SFX_HIT,D0
        BRA     sfx_play
.over   MOVE.W  #ST_OVER,g_state
        MOVE.W  #OVER_WAIT,g_over_timer
        ORI.W   #HUD_OVER,g_hud
        MOVE.L  g_score,D0
        CMP.L   g_hi,D0                 ; BCD は符号なしの比較で大小がわかる
        BLS.S   .snd
        MOVE.L  D0,g_hi                 ; 帯には HUD_OVER_T1 で描く
.snd    BSR     music_stop
        MOVEQ   #SFX_OVER,D0
        BRA     sfx_play

; (D0,D1) 大きさ D2 の正方形と、(D3,D4) 大きさ D5 の正方形が重なるか。出力：重なれば Z＝0（BNE）。D6 を壊す。
hit_test:
        MOVE.W  D3,D6
        ADD.W   D5,D6
        CMP.W   D6,D0
        BGE.S   .no
        MOVE.W  D0,D6
        ADD.W   D2,D6
        CMP.W   D6,D3
        BGE.S   .no
        MOVE.W  D4,D6
        ADD.W   D5,D6
        CMP.W   D6,D1
        BGE.S   .no
        MOVE.W  D1,D6
        ADD.W   D2,D6
        CMP.W   D6,D4
        BGE.S   .no
        MOVEQ   #1,D6
        RTS
.no     MOVEQ   #0,D6
        RTS

; 点数（BCD）に D0.L（BCD）を足す。ABCD で 1 byte ずつ。
add_score:
        MOVEM.L D0/A0-A1,-(SP)
        MOVE.L  D0,-(SP)
        LEA     4(SP),A1                ; 足す数の後ろ
        LEA     g_score+4,A0
        MOVE.W  #0,CCR                  ; X＝0
        ABCD    -(A1),-(A0)
        ABCD    -(A1),-(A0)
        ABCD    -(A1),-(A0)
        ABCD    -(A1),-(A0)
        ADDQ.L  #4,SP
        ORI.W   #HUD_SCORE,g_hud
        MOVEM.L (SP)+,D0/A0-A1
        RTS

; 乱数（xorshift16、7・9・8）。出力：D0.W。
rand:
        MOVE.L  D1,-(SP)
        MOVE.W  g_seed,D0
        MOVE.W  D0,D1
        LSL.W   #7,D1
        EOR.W   D1,D0
        MOVE.W  D0,D1
        LSR.W   #8,D1
        LSR.W   #1,D1
        EOR.W   D1,D0
        MOVE.W  D0,D1
        LSL.W   #8,D1
        EOR.W   D1,D0
        MOVE.W  D0,g_seed
        MOVE.L  (SP)+,D1
        RTS

; ------------------------------------------------------------------ 毎フレームの描画（VBlank の中）

draw_frame:
        MOVE.W  g_state,D0
        BEQ.S   .ret                    ; 題名の画面：点滅はパレットだけ
        CMP.W   #ST_PLAY,D0
        BNE.S   .hud
        BSR     draw_player
        BSR     draw_rocks
        BSR     draw_gems
.hud    BSR     draw_hud
        BSR     palette_fx
.ret    RTS

; 自機。無敵の間は 4 フレームごとに消す（点滅）。
draw_player:
        LEA     g_player,A0
        LEA     shape_player(PC),A1
        MOVEQ   #1,D6
        MOVE.W  g_invuln,D0
        BEQ.S   .draw
        BTST    #2,D0
        BEQ.S   .draw
        MOVEQ   #0,D6
.draw   BRA     draw_obj

draw_rocks:
        LEA     g_rocks,A0
        LEA     shape_rock(PC),A1
        MOVEQ   #ROCK_MAX-1,D7
.next   MOVE.W  OB_ON(A0),D6
        BSR     draw_obj
        LEA     OB_SIZE(A0),A0
        DBRA    D7,.next
        RTS

draw_gems:
        LEA     g_gems,A0
        LEA     shape_gem(PC),A1
        MOVEQ   #GEM_MAX-1,D7
.next   MOVE.W  OB_ON(A0),D6
        BSR     draw_obj
        LEA     OB_SIZE(A0),A0
        DBRA    D7,.next
        RTS

; 物 A0 を形 A1 で描き直す。D6＝見えるか。描いた位置と今の位置が同じで見えたままなら何もしない。
; 変わっていれば、描いた位置に XOR で描いて消し、今の位置に XOR で描く。消してから描くまでの間も VBlank の
; 中なので、物が消えて見えることはない。D0〜D5・A2・A3 を壊す。
draw_obj:
        TST.W   OB_DRAWN(A0)
        BEQ.S   .draw
        TST.W   D6
        BEQ.S   .erase
        MOVE.W  OB_X(A0),D0
        CMP.W   OB_DX(A0),D0
        BNE.S   .erase
        MOVE.W  OB_Y(A0),D0
        CMP.W   OB_DY(A0),D0
        BEQ.S   .ret
.erase  MOVE.W  OB_DX(A0),D0
        MOVE.W  OB_DY(A0),D1
        BSR     draw_shape
        CLR.W   OB_DRAWN(A0)
.draw   TST.W   D6
        BEQ.S   .ret
        MOVE.W  OB_X(A0),D0
        MOVE.W  OB_Y(A0),D1
        MOVE.W  D0,OB_DX(A0)
        MOVE.W  D1,OB_DY(A0)
        BSR     draw_shape
        MOVE.W  #1,OB_DRAWN(A0)
.ret    RTS

; 形（A1：色、矩形の数、矩形ごとに左上と右下の相対の位置）を (D0,D1) に XOR で描く。
; D0〜D5・A2・A3 を壊す（毎フレーム何度も呼ぶので、退避は D6・D7・A1 だけ）。
draw_shape:
        MOVEM.L D6-D7/A1,-(SP)
        MOVEA.W D0,A2
        MOVEA.W D1,A3
        MOVE.W  (A1)+,D0
        BSR     set_color
        MOVE.W  (A1)+,D6
        SUBQ.W  #1,D6
        MOVEQ   #ACRTC_OPM_EOR,D4
.next   MOVE.W  (A1)+,D0
        ADD.W   A2,D0
        MOVE.W  (A1)+,D1
        ADD.W   A3,D1
        MOVE.W  (A1)+,D2
        ADD.W   A2,D2
        MOVE.W  (A1)+,D3
        ADD.W   A3,D3
        BSR     rect
        DBRA    D6,.next
        MOVEM.L (SP)+,D6-D7/A1
        RTS

; 形。矩形どうしは重ねない（重ねると XOR で消える）。自機は内側の 4×4 をもう一度 XOR して穴にする。
; 石は毎フレーム動くので 1 個の矩形にした（8 個を消して描くと 16 個の矩形）[P]。
shape_player:
        DC.W    COL_PLAYER,2, 0,0,9,9, 3,3,6,6
shape_rock:
        DC.W    COL_ROCK,1, 0,0,7,7
shape_gem:
        DC.W    COL_GEM,5, 2,0,3,0, 1,1,4,1, 0,2,5,3, 1,4,4,4, 2,5,3,5

; 上の帯の数と、終わりの枠。BIOS_PUTS は 1 字に約 12 ライン（0.75 ms）かかる（16 語を CPU が字形から作る。
; エミュレータで測った [V]）。6 桁を描き直すと VBlank（70 ライン）を超えるので、点数と最高点は変わった桁だけ
; 描く。終わりの枠は 3 フレームに分ける（枠と残り → 最高点と「GAME OVER」→「PUSH START」）。文字は表示が
; その行に来る前に描き終わるので、裂けない。
draw_hud:
        MOVE.W  g_hud,D7
        BEQ     .ret
        CLR.W   g_hud
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BAND,D3
        MOVEQ   #HUD_Y,D1
        BTST    #0,D7                   ; HUD_SCORE
        BEQ.S   .hi
        LEA     g_score,A1
        LEA     g_score_shown,A2
        MOVEQ   #SCORE_X,D0
        BSR     draw_digits
.hi     BTST    #1,D7                   ; HUD_HI
        BEQ.S   .lives
        BSR     draw_hi
.lives  BTST    #2,D7                   ; HUD_LIVES
        BEQ.S   .over
        MOVEQ   #0,D4
        MOVE.W  g_lives,D4
        MOVEQ   #1,D5
        MOVE.W  #LIFE_X,D0
        JSR     BIOS_PUT_HEX
.over   BTST    #3,D7                   ; HUD_OVER：枠
        BEQ.S   .t1
        MOVE.W  D7,-(SP)
        BSR     draw_over_box
        MOVE.W  (SP)+,D7
        ORI.W   #HUD_OVER_T1,g_hud
.t1     BTST    #4,D7                   ; HUD_OVER_T1：最高点（帯は VBlank の中に描く）と「GAME OVER」
        BEQ.S   .t2
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BAND,D3
        MOVEQ   #HUD_Y,D1
        BSR     draw_hi
        LEA     str_over(PC),A0
        MOVEQ   #92,D0
        MOVEQ   #OVY0+8,D1
        MOVEQ   #COL_ROCK,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
        ORI.W   #HUD_OVER_T2,g_hud
.t2     BTST    #5,D7                   ; HUD_OVER_T2：「PUSH START」
        BEQ.S   .ret
        LEA     str_push(PC),A0
        MOVEQ   #88,D0
        MOVEQ   #OVY0+24,D1
        MOVEQ   #COL_BLINK,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
.ret    RTS

draw_hi:
        LEA     g_hi,A1
        LEA     g_hi_shown,A2
        MOVE.W  #HI_X,D0
; 6 桁の BCD（(A1).L）のうち、(A2).L と違う桁だけを 1 字ずつ描き、(A2) を新しい値にする。D0＝左端の x。
; D1〜D3（y と色）は呼ぶ側の値。
draw_digits:
        MOVEM.L D0/D4-D7/A0,-(SP)
        MOVE.L  (A1),D4
        MOVE.L  (A2),D6
        MOVE.L  D4,(A2)
        ADDI.W  #5*8,D0                 ; いちばん右の桁から
        MOVEQ   #6-1,D7
.digit  MOVE.W  D4,D5
        EOR.W   D6,D5
        ANDI.W  #15,D5
        BEQ.S   .same
        MOVE.W  D4,D5
        ANDI.W  #15,D5
        ADDI.W  #'0',D5
        MOVE.B  D5,g_digit
        CLR.B   g_digit+1
        LEA     g_digit,A0
        JSR     BIOS_PUTS
.same   LSR.L   #4,D4
        LSR.L   #4,D6
        SUBQ.W  #8,D0
        DBRA    D7,.digit
        MOVEM.L (SP)+,D0/D4-D7/A0
        RTS

; 終わりの枠。置き換え（OPM＝000）で描くので、下の物は壊れる（始め直すと画面ごと消す）。
OVX0            EQU     76
OVY0            EQU     80
OVX1            EQU     179
OVY1            EQU     119
; 縁の 4 本と中を別に塗る（画素は 104×40 の 1 回分、約 49 ライン）。D0〜D7 を壊す。
draw_over_box:
        MOVEQ   #ACRTC_OPM_REP,D4
        MOVEQ   #COL_BORDER,D0
        BSR     set_color
        LEA     over_rects(PC),A1
        MOVEQ   #4-1,D6
.edge   MOVEM.W (A1)+,D0-D3
        BSR     rect
        DBRA    D6,.edge
        MOVEQ   #COL_BG,D0
        BSR     set_color
        MOVEM.W (A1)+,D0-D3
        BRA     rect

over_rects:
        DC.W    OVX0,OVY0,OVX1,OVY0+1
        DC.W    OVX0,OVY1-1,OVX1,OVY1
        DC.W    OVX0,OVY0+2,OVX0+1,OVY1-2
        DC.W    OVX1-1,OVY0+2,OVX1,OVY1-2
        DC.W    OVX0+2,OVY0+2,OVX1-2,OVY1-2     ; 中

; 当たったときの赤い背景（パレットの 0 番）。
palette_fx:
        TST.W   g_flash
        BEQ.S   .ret
        SUBQ.W  #1,g_flash
        BEQ.S   .off
        MOVE.W  #$600,PALETTE+2*COL_BG
        RTS
.off    MOVE.W  #$000,PALETTE+2*COL_BG
.ret    RTS

; ------------------------------------------------------------------ 画面を作る（数フレームかかってよい）

; 題名の画面。描く間はパレットを黒にして見せない。
title_screen:
        BSR     pal_black
        MOVEQ   #COL_BG,D0
        JSR     BIOS_CLS
        MOVEQ   #73,D0                  ; 菱形・石・自機の絵（draw_shape は D0〜D5 を壊す）
        MOVEQ   #73,D1
        LEA     shape_gem(PC),A1
        BSR     draw_shape
        MOVEQ   #72,D0
        MOVEQ   #92,D1
        LEA     shape_rock(PC),A1
        BSR     draw_shape
        MOVEQ   #71,D0
        MOVEQ   #111,D1
        LEA     shape_player(PC),A1
        BSR     draw_shape
        MOVEQ   #COL_BG,D3
        LEA     str_title(PC),A0
        MOVEQ   #96,D0
        MOVEQ   #40,D1
        MOVEQ   #COL_TEXT,D2
        JSR     BIOS_PUTS
        LEA     str_gem(PC),A0
        MOVEQ   #88,D0
        MOVEQ   #72,D1
        MOVEQ   #COL_GEM,D2
        JSR     BIOS_PUTS
        LEA     str_rock(PC),A0
        MOVEQ   #88,D0
        MOVEQ   #92,D1
        MOVEQ   #COL_ROCK,D2
        JSR     BIOS_PUTS
        LEA     str_you(PC),A0
        MOVEQ   #88,D0
        MOVEQ   #112,D1
        MOVEQ   #COL_PLAYER,D2
        JSR     BIOS_PUTS
        LEA     str_push(PC),A0
        MOVEQ   #88,D0
        MOVE.W  #144,D1
        MOVEQ   #COL_BLINK,D2
        JSR     BIOS_PUTS
        LEA     str_hi(PC),A0
        MOVEQ   #96,D0
        MOVE.W  #168,D1
        MOVEQ   #COL_DIM,D2
        JSR     BIOS_PUTS
        MOVE.L  g_hi,D4
        MOVEQ   #6,D5
        MOVE.W  #120,D0
        JSR     BIOS_PUT_HEX
        BSR     show_screen
        MOVE.W  #ST_TITLE,g_state
        RTS

; 遊ぶ画面：帯、名前、枠。
play_screen:
        BSR     pal_black
        MOVEQ   #COL_BG,D0
        JSR     BIOS_CLS
        MOVEQ   #ACRTC_OPM_REP,D4
        MOVEQ   #COL_BAND,D0
        BSR     set_color
        MOVEQ   #0,D0
        MOVEQ   #0,D1
        MOVE.W  #255,D2
        MOVEQ   #11,D3
        BSR     rect
        MOVEQ   #COL_BORDER,D0
        BSR     set_color
        LEA     border_rects(PC),A1
        MOVEQ   #4-1,D6
.border MOVEM.W (A1)+,D0-D3
        BSR     rect
        DBRA    D6,.border
        MOVEQ   #COL_TEXT,D2
        MOVEQ   #COL_BAND,D3
        MOVEQ   #HUD_Y,D1
        LEA     str_score(PC),A0
        MOVEQ   #4,D0
        JSR     BIOS_PUTS
        LEA     str_hi(PC),A0
        MOVEQ   #112,D0
        JSR     BIOS_PUTS
        LEA     str_life(PC),A0
        MOVE.W  #196,D0
        JSR     BIOS_PUTS
        BSR     draw_hud                ; 数（g_hud）
        BSR     draw_player             ; 最初の物
        BSR     draw_rocks
        BSR     draw_gems
        BRA     show_screen

; 枠（太さ 2、物の範囲から 1 画素あける）
BL              EQU     FIELD_X0-3
BR              EQU     FIELD_X1+3
BT              EQU     FIELD_Y0-3
BB              EQU     FIELD_Y1+3
border_rects:
        DC.W    BL,BT,BR,BT+1
        DC.W    BL,BB-1,BR,BB
        DC.W    BL,BT+2,BL+1,BB-2
        DC.W    BR-1,BT+2,BR,BB-2

; ACRTC が描き終わるのを待ち、次の VBlank でパレットを戻す。
show_screen:
        BSR     acrtc_wait_idle
        JSR     BIOS_WAIT_VBL
        LEA     palette(PC),A0
        JSR     BIOS_SET_PAL
        RTS

pal_black:
        LEA     PALETTE,A0
        MOVEQ   #16-1,D0
.clr    MOVE.W  #0,(A0)+
        DBRA    D0,.clr
        RTS

; パレット（BIOS の既定と同じ値。bios-abi.md §06）
palette:
        DC.W    $000,$013,$247,$F44,$F84,$FE4,$4D4,$4CE
        DC.W    $48F,$A6F,$F6C,$888,$CCC,$842,$FFA,$FFF

; ------------------------------------------------------------------ ACRTC

; 描く色（CL0 と CL1）を D0.W（0〜15）にする。同じ色なら書かない。D5・D7 を壊す。
set_color:
        CMP.W   g_color,D0
        BEQ.S   .ret
        MOVE.W  D0,g_color
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

; (D0,D1)〜(D2,D3)（画面の座標、両端を含む）を塗る。D4＝OPM。AMOVE と AFRCT の 6 語。D5・D7 を壊す。
; 論理座標の +Y は画面の上なので、y の符号を変えて渡す（bios-abi.md §06）。毎フレーム何十回も呼ぶので、
; FIFO への書込みを開いて書く（1 語に約 50 クロック。ac_put を呼ぶと約 90）。どの語も WFR を確かめる（DR-033）。
rect:
.w1     MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   .w1
        MOVE.W  #ACRTC_AMOVE,2(A6)
.w2     MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   .w2
        MOVE.W  D0,2(A6)
        MOVE.W  D1,D5
        NEG.W   D5
.w3     MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   .w3
        MOVE.W  D5,2(A6)
        MOVE.W  #ACRTC_AFRCT,D5
        OR.W    D4,D5
.w4     MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   .w4
        MOVE.W  D5,2(A6)
.w5     MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   .w5
        MOVE.W  D2,2(A6)
        MOVE.W  D3,D5
        NEG.W   D5
.w6     MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   .w6
        MOVE.W  D5,2(A6)
        RTS

; D5.W を FIFO に書く。SR の WFR を確かめてから（DR-033）。A6＝ACRTC_AR、AR は r00。D7 を壊す。
ac_put:
        MOVE.W  (A6),D7
        BTST    #ACRTC_SR_WFR_B,D7
        BEQ.S   ac_put
        MOVE.W  D5,2(A6)
        RTS

; ACRTC が仕事を終えるまで待つ（FIFO が空で、コマンドが終わった）。上限あり。D7 を壊す。
acrtc_wait_idle:
        MOVE.L  D1,-(SP)
        MOVE.W  #$7FFF,D1
.wait   MOVE.W  (A6),D7
        ANDI.W  #ACRTC_SR_WFE|ACRTC_SR_CED,D7
        CMP.W   #ACRTC_SR_WFE|ACRTC_SR_CED,D7
        DBEQ    D1,.wait
        MOVE.L  (SP)+,D1
        RTS

; ------------------------------------------------------------------ デバッグの行

; このフレームの ACRTC の仕事が終わったラスタを読み、VBlank の直後からのライン数を数える。
measure:
        BSR     acrtc_wait_idle
        MOVE.W  VIDEO_LINE,D0
        SUB.W   dbg_line0,D0
        BPL.S   .pos
        ADDI.W  #262,D0
.pos    MOVE.W  D0,dbg_lines
        CMP.W   dbg_max,D0
        BLS.S   .ret
        MOVE.W  D0,dbg_max
.ret    RTS

; C で出し入れ。出している間は 16 フレームごとに数だけ書き直す（8 字で約 6 ms。測ったあとに描くので、
; ライン数には入らない。表示が上のほうを走っている間に画面の下の行を描くので、裂けない）。題名の画面では出さない。
dbg_phase:
        TST.W   g_state
        BEQ.S   .ret
        TST.W   dbg_dirty
        BEQ.S   .periodic
        CLR.W   dbg_dirty
        TST.W   dbg_on
        BNE.S   .full
        MOVEQ   #ACRTC_OPM_REP,D4       ; 消す
        MOVEQ   #COL_BG,D0
        BSR     set_color
        MOVEQ   #0,D0
        MOVE.W  #DBG_Y,D1
        MOVE.W  #255,D2
        MOVE.W  #DBG_Y+7,D3
        BRA     rect
.full   LEA     str_dbg(PC),A0          ; 名前と数
        MOVEQ   #4,D0
        MOVE.W  #DBG_Y,D1
        MOVEQ   #COL_DIM,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
        BRA.S   dbg_draw
.periodic
        TST.W   dbg_on
        BEQ.S   .ret
        MOVE.W  bios_frame+2,D0
        ANDI.W  #15,D0
        BEQ.S   dbg_draw
.ret    RTS

; 「LN xx MX xx UR xx SE xx」の数：ACRTC の仕事のライン数（このフレーム、最大）、ADPCM の UNDERRUN の数、
; 鳴らし始めた効果音の数（16 進）。
dbg_draw:
        LEA     dbg_vals(PC),A2
        MOVEQ   #4-1,D4
.val    MOVE.W  (A2)+,D5                ; x
        MOVEA.L (A2)+,A0                ; 値の番地
        MOVE.W  (A0),D0
        LEA     dbg_buf,A1
        BSR.S   hex2
        CLR.B   (A1)
        LEA     dbg_buf,A0
        MOVE.W  D5,D0
        MOVE.W  #DBG_Y,D1
        MOVEQ   #COL_DIM,D2
        MOVEQ   #COL_BG,D3
        JSR     BIOS_PUTS
        DBRA    D4,.val
        RTS

; x、値の番地
dbg_vals:
        DC.W    4+3*8
        DC.L    dbg_lines
        DC.W    4+9*8
        DC.L    dbg_max
        DC.W    4+15*8
        DC.L    sfx_underruns
        DC.W    4+21*8
        DC.L    sfx_plays

; D0 の下位 8 bit を 16 進の 2 字にして (A1)+ に置く。
hex2:
        MOVE.W  D0,D1
        LSR.W   #4,D1
        ANDI.W  #15,D1
        MOVE.B  .hex(PC,D1.W),(A1)+
        ANDI.W  #15,D0
        MOVE.B  .hex(PC,D0.W),(A1)+
        RTS
.hex    DC.B    "0123456789ABCDEF"

; ------------------------------------------------------------------ 文字列（JIS X 0201。字形のある字だけ）

str_title:      DC.B    "SAMPLE 1",0
str_gem:        DC.B    "ｷｲﾛ ｦ ｱﾂﾒﾙ",0
str_rock:       DC.B    "ｱｶ ｦ ﾖｹﾙ",0
str_you:        DC.B    "YOU",0
str_push:       DC.B    "PUSH START",0
str_hi:         DC.B    "HI",0
str_score:      DC.B    "SCORE",0
str_life:       DC.B    "LIFE",0
str_over:       DC.B    "GAME OVER",0
str_dbg:        DC.B    "LN 00 MX 00 UR 00 SE 00",0
        EVEN

        INCLUDE "sfx.s"
        INCLUDE "music.s"

; ------------------------------------------------------------------ 効果音の ADPCM（build.py が作る）

        EVEN
sfx_data:
        INCBIN  "sfx.bin"
sfx_data_end:
        ASSERT  sfx_data_end-sfx_data=SFX_BYTES
        DCB.B   4,$08                   ; 写すときに 4 byte 単位で読み過ぎる分
        IF      SFX_BYTES&1             ; 偶数の大きさにする（m68kasm の初版は末尾の EVEN でバイトを
        DC.B    $08                     ; 出さず、像の長さが見出しの大きさより 1 短くなった）
        ENDIF
rom_end:
        ASSERT  ((rom_end-CART)&1)=0
