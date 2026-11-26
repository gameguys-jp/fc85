; SAMPLE 5：仕様書 §11 のベンド点を試すための、止まらない試験信号のカートリッジ。m68kasm で組む（build.py）。
; 説明は README.md。
;
; 音：DOC の子基板があれば、波形 RAM の全 256 頁に 4 つの波形（頁 p は p&3：正弦・三角・のこぎり・矩形）を
;     置き、A2 の持続音（オシレータ 0・1）、低音（2・3）、分散和音（4・5、L と R に交互）を約 120 BPM で鳴らす。
;     オシレータのレジスタは、その声の音の頭でだけ書く（持続音は 8 歩＝約 1 秒ごと）。ベンドで崩れたレジスタは
;     次の音で戻り、音と音の間は崩れたまま聞こえる。太鼓は MSM5205 の ADPCM（Data Fabric のチャネル 0）。
; 絵：静止画（色の帯、市松と方眼、文字 ROM の字の表）は一度だけ描く。崩れはそのまま残り、START で描き直す。
;     毎フレーム動かすのは、箱 1 つと状態の行と、字を描き直す 1 行だけ。
; 待ち：どの待ちにも上限を置く。VBlank が来なければ（映像のクロックを止めた）、描画を飛ばして音だけ進める。
;
; 番地とビットの名前は fc85.inc、BIOS の約束は bios.inc（sdk/docs/bios-abi.md）。

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"
        INCLUDE "tables.inc"            ; build.py が作る（太鼓の offset・長さ、曲の定数）

; ------------------------------------------------------------------ 定数

; 拍：8 分音符を 15 フレーム（59.92 Hz で 119.8 BPM）。16 分音符の歩は 0 と 8 のフレーム（8・7 フレームの交互）
TICKS           EQU     15
ODD_TICK        EQU     8
FLUSH_TICK      EQU     13              ; まだ鳴っている太鼓を止める
ARM_TICK        EQU     14              ; 次の太鼓のために ADPCM を RESET にする
SONG_STEPS      EQU     SONG_BARS*16
MAX_CATCHUP     EQU     4               ; 1 回の待ちで進めるフレームの上限

; VBlank の待ちの上限：1 回 32 クロックの読出しを 6,250 回（約 25 ms）
WAIT_LOOPS      EQU     6250

; DOC
PAGE_SINE       EQU     0               ; 波形の頁（4 頁ごとに繰り返す）
PAGE_TRI        EQU     1
PAGE_SAW        EQU     2
PAGE_SQUARE     EQU     3
OSC_DRONE_L     EQU     0               ; オシレータ。CA は同じ番号（偶数は L、奇数は R。DR-054）
OSC_DRONE_R     EQU     1
OSC_BASS_L      EQU     2
OSC_BASS_R      EQU     3
OSC_LEAD_L      EQU     4
OSC_LEAD_R      EQU     5
OSC_FIRST_FREE  EQU     6               ; ここから DOC_OSCS−1 までは止めて CA 15 に置く
CA_UNUSED       EQU     15
DRONE_VOL       EQU     48              ; 音量 [P]（太鼓の ADPCM と釣り合うように小さめ）
BASS_VOL        EQU     56
LEAD_VOL        EQU     64

; Staging RAM（port 0 の番地）
WAVE_STAGE      EQU     $0000           ; 波形 1 KiB を 4 回（4 KiB＝16 頁）
DRUM_STAGE      EQU     $1000           ; 太鼓の ADPCM
        ASSERT  DRUM_STAGE+DRUM_BYTES+4<=$10000

; 波形 RAM を埋める：4 KiB を 16 回（1 フレームに 1 回）
FILL_BYTES      EQU     $1000
FILL_BLOCKS     EQU     16
FILL_MAX_FRAMES EQU     4               ; 1 回の転送の上限（工場出荷の CLK では 1 回 4〜6 ms）
FAB_TIMEOUT     EQU     40000           ; STATUS を読む回数の上限（約 0.4 秒）

; 太鼓
DR_IDLE         EQU     0
DR_ARMED        EQU     1
DR_PLAYING      EQU     2
DR_FLUSH        EQU     3
DRUM_PRIME      EQU     16              ; RESET を外す前に FIFO にためる byte 数
PRIME_LOOPS     EQU     400

; 画面（左上が 0,0）
TITLE_Y         EQU     0
BAR_LINE0       EQU     10              ; 色の帯の上下の白い線
BAR_Y0          EQU     12
BAR_Y1          EQU     51
BAR_LINE1       EQU     53
LABEL_Y         EQU     56
GRID_Y          EQU     66              ; 市松と方眼（pattern.bin、48 行）
GRID_ROWS       EQU     48
GRID_CHUNK      EQU     12              ; 1 フレームに描く行
TEXT_Y          EQU     118             ; 字の表（3 行）
LIVE_Y          EQU     144             ; 描き直す行
LANE_Y0         EQU     154             ; 箱の走る帯
LANE_Y1         EQU     167
BOX_Y0          EQU     156
BOX_Y1          EQU     165
BOX_W           EQU     12
BOX_SPEED       EQU     2
STATUS_Y        EQU     172
HELP_Y          EQU     184
LIVE_HOLD       EQU     16              ; 描き直す行を替えるまでの回数（1 回は 4 片）

; 色（palette の番号）
COL_BG          EQU     0
COL_RED         EQU     1
COL_YELLOW      EQU     3
COL_GREEN       EQU     5
COL_DARK        EQU     13
COL_GRAY        EQU     14
COL_WHITE       EQU     15
COL_LANE        EQU     COL_DARK
COL_BOX         EQU     COL_WHITE
COL_LIVE        EQU     COL_YELLOW

; v_dirty のビット
DIRTY_STEP      EQU     $0001           ; 歩と FC
DIRTY_FLAGS     EQU     $0002           ; 釦の行

; ------------------------------------------------------------------ Main RAM（$100800〜、bios-abi.md §03）

        RSSET   APP_RAM
v_frame         RS.W    1               ; 主ループの回数（VBlank か待ちの時間切れ）
v_has_doc       RS.W    1
v_novideo       RS.W    1               ; 最後の待ちが時間切れ
v_vbl_miss      RS.W    1               ; 待ちの時間切れの数
v_errors        RS.W    1               ; Fabric の誤りと時間切れの数
v_tick          RS.W    1               ; 0〜14（8 分音符の中のフレーム）
v_step          RS.W    1               ; 0〜63（次に鳴らす歩）
v_solo          RS.W    1               ; B：持続音だけ
v_mute          RS.W    1               ; C：太鼓を止める
v_lead_fc       RS.W    1               ; 最後に書いた分散和音の FC
v_fill          RS.W    1               ; 次に埋める波形 RAM の 4 KiB（16 で終わり）
v_fill_age      RS.W    1
v_dr_state      RS.W    1               ; DR_*
v_dr_cur        RS.W    1               ; 鳴らす太鼓の番号
v_dr_plays      RS.W    1
v_phase         RS.W    1               ; 静止画を描く段（0 は描き終わった）
v_dirty         RS.W    1
v_help          RS.W    1               ; 釦の行の残りの項目
v_err_shown     RS.W    1
v_miss_shown    RS.W    1
v_box_x         RS.W    1
v_box_dx        RS.W    1
v_box_old       RS.W    1               ; 描いてある箱の x（−1 はなし）
v_live          RS.W    1               ; 描き直す行の片（0〜3）
v_live_row      RS.W    1               ; 描き直す行（字の表の 0〜2）
v_live_age      RS.W    1
v_last          RS.L    1               ; 最後に見た bios_frame
v_txt           RS.B    18              ; BIOS_PUTS に渡す 16 字＋0
v_cmd           RS.W    200             ; ACRTC の語の並び（BIOS_ACRTC_SEND で送る）
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
        DC.B    "SAMPLE 5        "      ; 題名（16 字、空白で埋める）
        ASSERT  *=CART+cart_header_size

; ------------------------------------------------------------------ 入口

; 入口の条件（bios-abi.md §04）：画面は色 0、Data Fabric の両チャネルは止まっている、音は黙らせてある。
start:
        LEA     INITIAL_SSP,SP
        LEA     APP_RAM,A0
        MOVE.W  #(v_vars_end-APP_RAM+3)/4-1,D0
.clr    CLR.L   (A0)+
        DBRA    D0,.clr
        MOVE.W  bios_modules,D0
        BTST    #FABRIC_MOD_DOC_B,D0
        BEQ.S   .nodoc
        MOVE.W  #1,v_has_doc
.nodoc  MOVE.W  #FILL_BLOCKS,v_fill     ; 埋めていない（fill_start で 0 にする）
        MOVE.W  #-1,v_box_old
        MOVE.W  #BOX_SPEED,v_box_dx
        MOVE.W  #TICKS-2,v_tick         ; 最初のフレームで太鼓を準備し、次のフレームで歩 0
        MOVE.W  #1,v_phase              ; 静止画を描く
        LEA     palette(PC),A0
        JSR     BIOS_SET_PAL
        BSR     drums_init
        TST.W   v_has_doc
        BEQ.S   .go
        BSR     doc_init
        BSR     fill_start              ; 波形 RAM の最初の 4 KiB（使う頁 0〜3 を含む）は待つ
        LEA     FABRIC_CH1,A0
        MOVE.L  #FAB_TIMEOUT,D1
.first  MOVE.W  FAB_STATUS(A0),D0
        ANDI.W  #FAB_ST_DONE|FAB_ST_ERROR,D0
        BNE.S   .go
        SUBQ.L  #1,D1
        BNE.S   .first
.go     MOVE.L  bios_frame,v_last

main_loop:
        BSR     wait_frame              ; D0＝進めるフレームの数
        MOVE.W  D0,D7
        SUBQ.W  #1,D7
.tick   BSR     seq_frame               ; 音（描画より先）
        DBRA    D7,.tick
        BSR     input
        BSR     fill_poll
        TST.W   v_novideo
        BNE.S   main_loop               ; 映像がない間は ACRTC に触れない
        BSR     draw_frame
        BRA.S   main_loop

; 次の VBlank を待つ（bios_frame が変わる）。上限は約 25 ms。出力：D0.W＝進めるフレームの数（1〜4）。
; 時間切れ（映像のクロックを止めたか、大きく下げた）なら 1 で、v_novideo を立てる。
wait_frame:
        MOVE.L  v_last,D1
        MOVE.W  #WAIT_LOOPS-1,D2
.wait   CMP.L   bios_frame,D1
        DBNE    D2,.wait
        BNE.S   .vbl
        MOVE.W  #1,v_novideo
        ADDQ.W  #1,v_vbl_miss
        MOVEQ   #1,D0
        BRA.S   .ret
.vbl    CLR.W   v_novideo
        MOVE.L  bios_frame,D0
        MOVE.L  D0,v_last
        SUB.L   D1,D0                   ; 前に見てから来た VBlank の数（描画が長引いたときに追いつく）
        CMP.L   #MAX_CATCHUP,D0
        BLS.S   .ret
        MOVEQ   #MAX_CATCHUP,D0
.ret    ADDQ.W  #1,v_frame
        RTS

; ------------------------------------------------------------------ 入力

input:
        JSR     BIOS_READ_PAD           ; D1＝新しく押した釦（PORT_A を直接読む）
        MOVE.W  D1,D6                   ; fill_start は D1 を壊す
        BTST    #PAD_A_B,D6             ; A：波形 RAM を埋め直す
        BEQ.S   .b
        BSR     fill_start
.b      BTST    #PAD_B_B,D6             ; B：持続音だけにする・戻す
        BEQ.S   .c
        EORI.W  #1,v_solo
        BSR     help_dirty
.c      BTST    #PAD_C_B,D6             ; C：太鼓を止める・戻す
        BEQ.S   .s
        EORI.W  #1,v_mute
        BSR     help_dirty
.s      BTST    #PAD_START_B,D6         ; START：静止画を描き直す
        BEQ.S   .ret
        MOVE.W  #1,v_phase
.ret    RTS

help_dirty:
        ORI.W   #DIRTY_FLAGS,v_dirty
        RTS

; ------------------------------------------------------------------ 拍

; 1 フレームぶん拍を進める。D7 は保つ。
seq_frame:
        MOVEM.L D0-D7/A0-A2,-(SP)
        MOVE.W  v_tick,D0
        ADDQ.W  #1,D0
        CMP.W   #TICKS,D0
        BLO.S   .t
        MOVEQ   #0,D0
.t      MOVE.W  D0,v_tick
        BSR     drum_tick
        MOVE.W  v_tick,D0
        BEQ.S   .step
        CMP.W   #ODD_TICK,D0
        BNE.S   .ret
.step   BSR     step_event
.ret    MOVEM.L (SP)+,D0-D7/A0-A2
        RTS

; 16 分音符の歩。持続音（8 歩ごと）、分散和音、低音の順に、音の頭でだけレジスタを書く。
step_event:
        MOVE.W  v_step,D6
        TST.W   v_has_doc
        BEQ.S   .next
        MOVE.W  D6,D0
        ANDI.W  #7,D0
        BNE.S   .notes
        BSR     drone_refresh
.notes  BSR     lead_note
        BSR     bass_note
.next   ADDQ.W  #1,D6
        ANDI.W  #SONG_STEPS-1,D6
        MOVE.W  D6,v_step
        ORI.W   #DIRTY_STEP,v_dirty
        RTS

; ------------------------------------------------------------------ DOC（DR-030：窓には MOVE.B の 1 回の書込みだけ。読まない）

; 起動時：16 個のオシレータを止めて CA 15 に置き、走査を 16 個にする。
doc_init:
        LEA     DOC,A0
        MOVEQ   #DOC_OSCS-1,D0
.osc    MOVE.B  #0,DOC_VOLUME-DOC(A0)
        MOVE.B  #(CA_UNUSED<<DOC_CTL_CA_SH)|DOC_CTL_HALT,DOC_CONTROL-DOC(A0)
        ADDQ.L  #DOC_OSC_STRIDE,A0
        DBRA    D0,.osc
        MOVE.B  #(DOC_OSCS-1)*2,DOC_OSC_ENABLE
        RTS

; 持続音（A2、のこぎり波、L と R）を書き直し、走査の数と使わないオシレータも書き直す（約 1 秒ごと）。
; D6 は保つ。
drone_refresh:
        MOVEQ   #DRONE_NOTE,D0
        BSR     note_fc
        MOVE.W  #DRONE_VOL,D2
        MOVEQ   #PAGE_SAW,D3
        MOVEQ   #OSC_DRONE_L,D0
        MOVE.W  #(OSC_DRONE_L<<DOC_CTL_CA_SH)|DOC_CTL_FREE,D4
        BSR     osc_write
        MOVEQ   #OSC_DRONE_R,D0
        MOVE.W  #(OSC_DRONE_R<<DOC_CTL_CA_SH)|DOC_CTL_FREE,D4
        BSR     osc_write
        MOVE.B  #(DOC_OSCS-1)*2,DOC_OSC_ENABLE
        LEA     DOC+2*OSC_FIRST_FREE,A0
        MOVEQ   #DOC_OSCS-OSC_FIRST_FREE-1,D0
.free   MOVE.B  #0,DOC_VOLUME-DOC(A0)
        MOVE.B  #(CA_UNUSED<<DOC_CTL_CA_SH)|DOC_CTL_HALT,DOC_CONTROL-DOC(A0)
        ADDQ.L  #DOC_OSC_STRIDE,A0
        DBRA    D0,.free
        RTS

; 分散和音：偶数の歩はオシレータ 4（L、三角波）、奇数は 5（R、正弦波）。次の同じ側の音まで鳴り続ける。
; D6＝歩（0〜63）。D6 は保つ。
lead_note:
        MOVE.W  D6,D5
        ANDI.W  #15,D5
        LEA     lead_pat(PC),A1
        MOVEQ   #0,D0
        MOVE.B  0(A1,D5.W),D0           ; 和音の音の番号（0〜4）
        MOVE.W  D6,D1
        LSR.W   #4,D1                   ; 小節
        LSL.W   #3,D1
        ADD.W   D1,D0
        LEA     bar_tones(PC),A1
        MOVEQ   #0,D1
        MOVE.B  0(A1,D0.W),D1
        MOVE.W  D1,D0                   ; MIDI の音
        BSR     note_fc
        MOVE.W  D1,v_lead_fc
        MOVE.W  #LEAD_VOL,D2
        TST.W   v_solo
        BEQ.S   .side
        MOVEQ   #0,D2
.side   BTST    #0,D6
        BNE.S   .right
        MOVEQ   #OSC_LEAD_L,D0
        MOVEQ   #PAGE_TRI,D3
        MOVE.W  #(OSC_LEAD_L<<DOC_CTL_CA_SH)|DOC_CTL_FREE,D4
        BRA     osc_write
.right  MOVEQ   #OSC_LEAD_R,D0
        MOVEQ   #PAGE_SINE,D3
        MOVE.W  #(OSC_LEAD_R<<DOC_CTL_CA_SH)|DOC_CTL_FREE,D4
        BRA     osc_write

; 低音：オシレータ 2（L）と 3（R）、矩形波。休みは音量 0 を書く。D6＝歩。D6 は保つ。
bass_note:
        MOVE.W  D6,D5
        ANDI.W  #15,D5
        LEA     bass_pat(PC),A1
        MOVEQ   #0,D0
        MOVE.B  0(A1,D5.W),D0
        CMP.B   #HOLD,D0
        BEQ.S   .ret
        MOVE.W  #BASS_VOL,D2
        CMP.B   #REST,D0
        BNE.S   .solo
        MOVEQ   #0,D0                   ; 休み：根音で音量 0
        MOVEQ   #0,D2
.solo   TST.W   v_solo
        BEQ.S   .root
        MOVEQ   #0,D2
.root   MOVE.W  D6,D1
        LSR.W   #4,D1
        LEA     bar_root(PC),A1
        MOVEQ   #0,D3
        MOVE.B  0(A1,D1.W),D3
        ADD.W   D3,D0
        BSR     note_fc
        MOVEQ   #PAGE_SQUARE,D3
        MOVEQ   #OSC_BASS_L,D0
        MOVE.W  #(OSC_BASS_L<<DOC_CTL_CA_SH)|DOC_CTL_FREE,D4
        BSR     osc_write
        MOVEQ   #OSC_BASS_R,D0
        MOVE.W  #(OSC_BASS_R<<DOC_CTL_CA_SH)|DOC_CTL_FREE,D4
        BRA     osc_write
.ret    RTS

; D0.W＝MIDI の音 → D1.W＝FC（song.inc の pitch_fc）。A1 を壊す。
note_fc:
        LEA     pitch_fc(PC),A1
        MOVE.W  D0,D1
        ADD.W   D1,D1
        MOVE.W  0(A1,D1.W),D1
        RTS

; オシレータ D0.W のレジスタを全部書く：FC＝D1.W、音量 D2、頁 D3、制御 D4（最後に書く。HALT＝0 で動く）。
; 表は 256 byte（SIZE＝0：T＝0、R＝0）。A0・D5 を壊す。
osc_write:
        LEA     DOC,A0
        ADDA.W  D0,A0
        ADDA.W  D0,A0
        MOVE.B  D1,DOC_FREQ_LO-DOC(A0)
        MOVE.W  D1,D5
        LSR.W   #8,D5
        MOVE.B  D5,DOC_FREQ_HI-DOC(A0)
        MOVE.B  D2,DOC_VOLUME-DOC(A0)
        MOVE.B  D3,DOC_POINTER-DOC(A0)
        MOVE.B  #0,DOC_SIZE-DOC(A0)
        MOVE.B  D4,DOC_CONTROL-DOC(A0)
        RTS

; ------------------------------------------------------------------ 波形 RAM を埋める（Data Fabric のチャネル 1）

; 波形 1 KiB を Staging RAM の $0000〜$0FFF に 4 回写し（16 頁。頁 p は波形 p&3）、その 4 KiB を
; 波形 RAM の 16 か所に 1 フレームに 1 回ずつ運ぶ。全 256 頁に同じ並びを置くので、DOC の AD のバスの線
; （頁の番地の線を兼ねる）を崩しても、読む頁は波形のある頁になる。A で埋め直す。
fill_start:
        TST.W   v_has_doc
        BEQ.S   .ret
        LEA     STAGING+WAVE_STAGE,A1
        MOVEQ   #4-1,D1
.rep    LEA     wave_data,A0
        MOVE.W  #1024/4-1,D0
.copy   MOVE.L  (A0)+,(A1)+
        DBRA    D0,.copy
        DBRA    D1,.rep
        CLR.W   v_fill
        BSR     help_dirty
        BRA     fill_block
.ret    RTS

; v_fill 番目の 4 KiB を始める。
fill_block:
        LEA     FABRIC_CH1,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_DOC,FAB_DST(A0)
        MOVE.W  #WAVE_STAGE,FAB_SRC_ADDR_L(A0)
        MOVE.W  v_fill,D0
        MOVEQ   #12,D1
        LSL.W   D1,D0
        MOVE.W  D0,FAB_DST_ADDR_L(A0)   ; 波形 RAM の byte 番地
        MOVE.W  #FILL_BYTES,FAB_COUNT(A0)
        MOVE.W  #1,FAB_SRC_STEP(A0)
        MOVE.W  #1,FAB_DST_STEP(A0)
        MOVE.W  #FMT_8_8,FAB_FORMAT(A0)
        MOVE.W  #$FFFF,FAB_MASK(A0)
        MOVE.W  #0,FAB_XOR(A0)
        MOVE.W  #0,FAB_SHIFT(A0)
        MOVE.W  #TRG_NOW,FAB_TRIGGER(A0)
        MOVE.W  #FAB_START,FAB_CTRL(A0)
        CLR.W   v_fill_age
        RTS

; 1 フレームに 1 回：終わっていれば次の 4 KiB。ERROR と時間切れは v_errors に数えて次へ進む。
fill_poll:
        CMP.W   #FILL_BLOCKS,v_fill
        BHS.S   .ret
        LEA     FABRIC_CH1,A0
        MOVE.W  FAB_STATUS(A0),D1
        BTST    #3,D1                   ; ERROR
        BNE.S   .err
        BTST    #1,D1                   ; DONE
        BNE.S   .done
        ADDQ.W  #1,v_fill_age
        CMP.W   #FILL_MAX_FRAMES,v_fill_age
        BLO.S   .ret
.err    ADDQ.W  #1,v_errors
.done   MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        ADDQ.W  #1,v_fill
        CMP.W   #FILL_BLOCKS,v_fill
        BLO     fill_block
        BRA     help_dirty              ; 埋め終わった（釦の行の A を戻す）
.ret    RTS

; ------------------------------------------------------------------ 太鼓（ADPCM。sdk/samples/game/sfx.s と同じ手順）
;
; 8 分音符（15 フレーム）ごとに 1 つ鳴らす。フレーム 13 でまだ鳴っていれば止めて FIFO を空にし、
; フレーム 14 で RESET をかけ（1 フレーム保つ）、フレーム 0 で Fabric のチャネル 0 を始めて RESET を外す。

; 起動時：ADPCM を RESET にし、太鼓の byte をカートリッジから Staging RAM に写す。
drums_init:
        MOVE.W  #ADPCM_CTL_RESET|ADPCM_CTL_8K,ADPCM_CTRL
        LEA     FABRIC_CH0,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        LEA     drum_data,A0
        LEA     STAGING+DRUM_STAGE,A1
        MOVE.W  #(DRUM_BYTES+3)/4-1,D0
.copy   MOVE.L  (A0)+,(A1)+
        DBRA    D0,.copy
        MOVE.W  #1<<PORT_ADPCM,FABRIC_PORT_RESET
        MOVE.W  #ADPCM_ST_UNDERRUN,ADPCM_STATUS
        CLR.W   v_dr_state
        RTS

drum_tick:
        MOVE.W  v_tick,D0
        CMP.W   #FLUSH_TICK,D0
        BEQ.S   drum_flush
        CMP.W   #ARM_TICK,D0
        BEQ.S   drum_arm
        TST.W   D0
        BEQ     drum_start
        BRA     drum_poll

; まだ鳴っていれば（ADPCM のクロックを下げたときなど）チャネル 0 を止め、FIFO を空にする。RESET は次のフレーム。
drum_flush:
        CMP.W   #DR_PLAYING,v_dr_state
        BNE.S   .ret
        ADDQ.W  #1,v_errors             ; 時間切れ（工場出荷のクロックでは、どの太鼓も 13 フレームより短い）
        LEA     FABRIC_CH0,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #1<<PORT_ADPCM,FABRIC_PORT_RESET
        MOVE.W  #DR_FLUSH,v_dr_state
.ret    RTS

; RESET をかけ、次の 8 分音符の太鼓を選ぶ。
drum_arm:
        LEA     FABRIC_CH0,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #ADPCM_CTL_RESET|ADPCM_CTL_8K,ADPCM_CTRL
        MOVE.W  #1<<PORT_ADPCM,FABRIC_PORT_RESET
        CLR.W   v_dr_state
        TST.W   v_mute
        BNE.S   .ret
        MOVE.W  v_step,D0               ; 次に鳴らす歩（偶数）
        LSR.W   #1,D0
        ANDI.W  #7,D0
        LEA     drum_pat(PC),A1
        MOVEQ   #0,D1
        MOVE.B  0(A1,D0.W),D1
        CMP.B   #NO_DRUM,D1
        BEQ.S   .ret
        MOVE.W  D1,v_dr_cur
        MOVE.W  #DR_ARMED,v_dr_state
.ret    RTS

; RESET を 1 フレーム保った。チャネル 0 を設定して始め、FIFO に 16 byte たまったら RESET を外す。
drum_start:
        CMP.W   #DR_ARMED,v_dr_state
        BNE     .ret
        MOVE.W  v_dr_cur,D1
        LSL.W   #2,D1
        LEA     drum_table(PC),A1
        ADDA.W  D1,A1
        LEA     FABRIC_CH0,A0
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_ADPCM,FAB_DST(A0)
        MOVE.W  (A1)+,D1
        ADDI.W  #DRUM_STAGE,D1
        MOVE.W  D1,FAB_SRC_ADDR_L(A0)
        MOVE.W  #0,FAB_DST_ADDR_L(A0)
        MOVE.W  (A1)+,FAB_COUNT(A0)
        MOVE.W  #1,FAB_SRC_STEP(A0)
        MOVE.W  #0,FAB_DST_STEP(A0)
        MOVE.W  #FMT_8_8,FAB_FORMAT(A0)
        MOVE.W  #$FFFF,FAB_MASK(A0)
        MOVE.W  #0,FAB_XOR(A0)
        MOVE.W  #0,FAB_SHIFT(A0)
        MOVE.W  #TRG_NOW,FAB_TRIGGER(A0)
        MOVE.W  #FAB_START,FAB_CTRL(A0)
        MOVE.W  #PRIME_LOOPS,D2
.prime  MOVE.W  ADPCM_STATUS,D1
        ANDI.W  #ADPCM_ST_COUNT,D1
        CMP.W   #DRUM_PRIME,D1
        DBHS    D2,.prime
        MOVE.W  #ADPCM_ST_UNDERRUN,ADPCM_STATUS
        MOVE.W  #ADPCM_CTL_8K,ADPCM_CTRL
        MOVE.W  #DR_PLAYING,v_dr_state
        ADDQ.W  #1,v_dr_plays
.ret    RTS

; 鳴り終わり（DONE で FIFO が空）か ERROR なら止めて RESET をかける。
drum_poll:
        CMP.W   #DR_PLAYING,v_dr_state
        BNE.S   .ret
        LEA     FABRIC_CH0,A0
        MOVE.W  FAB_STATUS(A0),D1
        BTST    #3,D1                   ; ERROR
        BNE.S   .err
        BTST    #1,D1                   ; DONE
        BEQ.S   .ret
        MOVE.W  ADPCM_STATUS,D1
        ANDI.W  #ADPCM_ST_COUNT,D1
        BNE.S   .ret
        BRA.S   .stop
.err    ADDQ.W  #1,v_errors
.stop   MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #ADPCM_CTL_RESET|ADPCM_CTL_8K,ADPCM_CTRL
        CLR.W   v_dr_state
.ret    RTS

; ------------------------------------------------------------------ 描画（VBlank が来たフレームだけ）
;
; ACRTC には BIOS_ACRTC_SEND で送る（WFR の待ちに上限があり、CER ですぐ打ち切る。DR-080、DR-082）。
; 語の並びは v_cmd に作る。文字は BIOS_PUTS・BIOS_PUT_HEX。

draw_frame:
        TST.W   v_phase
        BNE     draw_phase
        BSR     box_draw                ; VBlank の中で終わる
        BSR     status_frame            ; 毎フレーム：主ループの回数
        ; ほかの字は 1 フレームに 1 つだけ（BIOS_PUTS は 1 字に約 9〜12 ライン）
        MOVE.W  v_dirty,D0
        BTST    #0,D0                   ; DIRTY_STEP
        BNE     status_step
        MOVE.W  v_errors,D0
        CMP.W   v_err_shown,D0
        BNE     status_counts
        MOVE.W  v_vbl_miss,D0
        CMP.W   v_miss_shown,D0
        BNE     status_counts
        BTST    #1,v_dirty+1            ; DIRTY_FLAGS
        BEQ.S   .help
        BCLR    #1,v_dirty+1
        MOVE.W  #4,v_help
.help   TST.W   v_help
        BNE     help_item
        BRA     live_piece

; ---- 静止画（1 フレームに 1 段）

draw_phase:
        MOVE.W  v_phase,D0
        LSL.W   #2,D0
        LEA     phase_table(PC),A1
        MOVEA.L -4(A1,D0.W),A1
        JSR     (A1)
        ADDQ.W  #1,v_phase
        CMP.W   #PHASES,v_phase
        BLS.S   .ret
        ; 描き終わった：動く物と字を描き直す
        CLR.W   v_phase
        MOVE.W  #-1,v_box_old
        MOVE.W  #DIRTY_STEP|DIRTY_FLAGS,v_dirty
        MOVE.W  #-1,v_err_shown
        MOVE.W  #-1,v_miss_shown
.ret    RTS

phase_table:
        DC.L    ph_clear, ph_title, ph_bars, ph_labels0, ph_labels1
        DC.L    ph_grid, ph_grid, ph_grid, ph_grid
        DC.L    ph_text, ph_text, ph_text, ph_text, ph_text, ph_text
        DC.L    ph_lane
phase_table_end:
PHASES          EQU     (phase_table_end-phase_table)/4

; パレットを戻し、画面を色 0 で消す。
ph_clear:
        LEA     palette(PC),A0
        JSR     BIOS_SET_PAL
        MOVEQ   #COL_BG,D0
        JMP     BIOS_CLS

; 題名と、DOC の子基板の有無。
ph_title:
        LEA     str_title(PC),A0
        MOVEQ   #0,D0
        MOVEQ   #TITLE_Y,D1
        MOVEQ   #COL_WHITE,D2
        BSR     puts
        TST.W   v_has_doc
        BEQ.S   .nodoc
        LEA     str_doc(PC),A0
        MOVE.W  #184,D0
        MOVEQ   #COL_GREEN,D2
        BRA.S   .put
.nodoc  LEA     str_no_doc(PC),A0
        MOVE.W  #160,D0
        MOVEQ   #COL_RED,D2
.put    MOVEQ   #TITLE_Y,D1
        BRA     puts

; 16 色の帯（パレット 0〜15、16 画素ずつ）と、上下の白い線。
ph_bars:
        BSR     cmd_begin
        MOVEQ   #COL_WHITE,D0
        BSR     cmd_color
        MOVEQ   #0,D0
        MOVEQ   #BAR_LINE0,D1
        MOVE.W  #255,D2
        MOVEQ   #BAR_LINE0,D3
        BSR     cmd_rect
        MOVEQ   #0,D0
        MOVEQ   #BAR_LINE1,D1
        MOVE.W  #255,D2
        MOVEQ   #BAR_LINE1,D3
        BSR     cmd_rect
        MOVEQ   #0,D6                   ; 色
.bar    MOVE.W  D6,D0
        BSR     cmd_color
        MOVE.W  D6,D0
        LSL.W   #4,D0
        MOVEQ   #BAR_Y0,D1
        MOVE.W  D0,D2
        ADDI.W  #15,D2
        MOVEQ   #BAR_Y1,D3
        BSR     cmd_rect
        ADDQ.W  #1,D6
        CMP.W   #16,D6
        BLO.S   .bar
        BRA     cmd_send

; 帯の番号（16 進）。8 本ずつ。
ph_labels0:
        LEA     str_labels0(PC),A0
        MOVEQ   #4,D0
        BRA.S   labels
ph_labels1:
        LEA     str_labels1(PC),A0
        MOVE.W  #132,D0
labels:
        MOVEQ   #LABEL_Y,D1
        MOVEQ   #COL_WHITE,D2
        BRA     puts

; 市松と方眼（pattern.bin）の 12 行を DWT で描く。段 6〜9 が 0〜3 の片。
ph_grid:
        MOVE.W  v_phase,D6
        SUBQ.W  #6,D6                   ; 片（0〜3）
        MOVE.W  D6,D0
        MULU    #GRID_CHUNK*64,D0
        ADDI.L  #GRID_Y*64,D0           ; 語の番地
        BSR     cmd_begin
        BSR     cmd_rwp
        MOVE.W  #ACRTC_DWT,(A2)+
        MOVE.W  #64-1,(A2)+             ; AX：64 語
        MOVE.W  #-(GRID_CHUNK-1),(A2)+  ; AY：12 行、下へ
        BSR     cmd_send
        LEA     pattern_data,A0
        MOVE.W  D6,D0
        MULU    #GRID_CHUNK*64*2,D0
        ADDA.L  D0,A0
        MOVE.W  #GRID_CHUNK*64,D1
        JMP     BIOS_ACRTC_SEND

; 字の表：3 行×32 字を 16 字ずつ。段 10〜15。
ph_text:
        MOVE.W  v_phase,D0
        SUB.W   #10,D0                  ; 0〜5
        MOVE.W  D0,D1
        LSR.W   #1,D1                   ; 行
        ANDI.W  #1,D0                   ; 左・右
        LSL.W   #4,D0                   ; 字の位置（0・16）
        LEA     text_rows(PC),A0
        MOVE.W  D1,D2
        LSL.W   #5,D2
        ADDA.W  D2,A0
        ADDA.W  D0,A0
        LSL.W   #3,D1
        ADDI.W  #TEXT_Y,D1              ; y
        LSL.W   #3,D0                   ; x
        MOVEQ   #COL_WHITE,D2
        MOVEQ   #16,D4
        BRA     put_part

; 箱の帯と、状態の行の名前。
ph_lane:
        BSR     cmd_begin
        MOVEQ   #COL_LANE,D0
        BSR     cmd_color
        MOVEQ   #0,D0
        MOVE.W  #LANE_Y0,D1
        MOVE.W  #255,D2
        MOVE.W  #LANE_Y1,D3
        BSR     cmd_rect
        BSR     cmd_send
        LEA     status_names(PC),A3
.name   MOVE.W  (A3)+,D0                ; x（−1 で終わり）
        BMI.S   .ret
        MOVEA.L A3,A0
        MOVE.W  #STATUS_Y,D1
        MOVEQ   #COL_GRAY,D2
        BSR     puts
        ADDQ.L  #4,A3
        BRA.S   .name
.ret    RTS

; ---- 毎フレーム

; 帯の中の箱を 2 画素ずつ動かす：前の位置を帯の色で塗り、今の位置に箱を塗る。
box_draw:
        BSR     cmd_begin
        MOVE.W  v_box_old,D6
        BMI.S   .move
        MOVEQ   #COL_LANE,D0
        BSR     cmd_color
        MOVE.W  D6,D0
        MOVE.W  #BOX_Y0,D1
        MOVE.W  D6,D2
        ADDI.W  #BOX_W-1,D2
        MOVE.W  #BOX_Y1,D3
        BSR     cmd_rect
.move   MOVE.W  v_box_x,D6
        ADD.W   v_box_dx,D6
        BPL.S   .right
        MOVEQ   #0,D6
        MOVE.W  #BOX_SPEED,v_box_dx
.right  CMP.W   #256-BOX_W,D6
        BLE.S   .draw
        MOVE.W  #256-BOX_W,D6
        MOVE.W  #-BOX_SPEED,v_box_dx
.draw   MOVE.W  D6,v_box_x
        MOVE.W  D6,v_box_old
        MOVEQ   #COL_BOX,D0
        BSR     cmd_color
        MOVE.W  D6,D0
        MOVE.W  #BOX_Y0,D1
        MOVE.W  D6,D2
        ADDI.W  #BOX_W-1,D2
        MOVE.W  #BOX_Y1,D3
        BSR     cmd_rect
        BRA     cmd_send

; 状態の行：FR（主ループの回数）。
status_frame:
        MOVEQ   #0,D4
        MOVE.W  v_frame,D4
        MOVEQ   #4,D5
        MOVEQ   #24,D0
        BRA     put_hex

; ST（次に鳴らす歩）と FC（最後に書いた分散和音の FC）。
status_step:
        BCLR    #0,v_dirty+1
        MOVEQ   #0,D4
        MOVE.W  v_step,D4
        MOVEQ   #2,D5
        MOVEQ   #88,D0
        BSR     put_hex
        MOVEQ   #0,D4
        MOVE.W  v_lead_fc,D4
        MOVEQ   #4,D5
        MOVE.W  #132,D0
        BRA     put_hex

; ER（Fabric の誤りと時間切れ）と NV（VBlank を待って時間切れになった数）の下位 8 bit。
status_counts:
        MOVE.W  v_errors,v_err_shown
        MOVE.W  v_vbl_miss,v_miss_shown
        MOVEQ   #0,D4
        MOVE.W  v_err_shown,D4
        MOVEQ   #2,D5
        MOVE.W  #196,D0
        BSR     put_hex
        MOVEQ   #0,D4
        MOVE.W  v_miss_shown,D4
        MOVEQ   #2,D5
        MOVE.W  #240,D0
        BRA     put_hex

; 釦の行の 1 項目（v_help が 4〜1）。色で状態を示す。
help_item:
        MOVE.W  v_help,D0
        SUBQ.W  #1,v_help
        NEG.W   D0
        ADDQ.W  #4,D0                   ; 項目（0〜3）
        MOVE.W  D0,D6
        LSL.W   #3,D0
        LEA     help_items(PC),A0
        ADDA.W  D0,A0                   ; 7 字＋0
        MOVEQ   #COL_WHITE,D2
        TST.W   D6
        BNE.S   .b
        CMP.W   #FILL_BLOCKS,v_fill     ; A：埋めている間は黄
        BHS.S   .put
        MOVEQ   #COL_YELLOW,D2
        BRA.S   .put
.b      CMP.W   #1,D6
        BNE.S   .c
        TST.W   v_solo                  ; B：持続音だけの間は黄
        BEQ.S   .put
        MOVEQ   #COL_YELLOW,D2
        BRA.S   .put
.c      CMP.W   #2,D6
        BNE.S   .put
        TST.W   v_mute                  ; C：太鼓を止めている間は暗い灰
        BEQ.S   .put
        MOVEQ   #COL_DARK,D2
.put    MOVE.W  D6,D0
        LSL.W   #6,D0                   ; x＝64×項目
        MOVE.W  #HELP_Y,D1
        BRA     puts

; 字の表の 1 行を、毎回文字 ROM から描き直す（8 字ずつ 4 片）。文字 ROM の線のベンドは、ここにすぐ出て、
; 上の表（描いたときの字）と比べられる。LIVE_HOLD 回ごとに次の行へ。
live_piece:
        MOVE.W  v_live,D0               ; 片（0〜3）
        LSL.W   #3,D0                   ; 字の位置
        LEA     text_rows(PC),A0
        MOVE.W  v_live_row,D1
        LSL.W   #5,D1
        ADDA.W  D1,A0
        ADDA.W  D0,A0
        LSL.W   #3,D0                   ; x
        MOVE.W  #LIVE_Y,D1
        MOVEQ   #COL_LIVE,D2
        MOVEQ   #8,D4
        BSR     put_part
        MOVE.W  v_live,D0
        ADDQ.W  #1,D0
        ANDI.W  #3,D0
        MOVE.W  D0,v_live
        BNE.S   .ret
        ADDQ.W  #1,v_live_age
        CMP.W   #LIVE_HOLD,v_live_age
        BLO.S   .ret
        CLR.W   v_live_age
        MOVE.W  v_live_row,D0
        ADDQ.W  #1,D0
        CMP.W   #3,D0
        BLO.S   .row
        MOVEQ   #0,D0
.row    MOVE.W  D0,v_live_row
.ret    RTS

; ------------------------------------------------------------------ 字と ACRTC の語の並び

; (A0) の D4.W 字（16 字まで）を v_txt に写して、(D0,D1) に色 D2 で描く。
put_part:
        LEA     v_txt,A1
        SUBQ.W  #1,D4
.copy   MOVE.B  (A0)+,(A1)+
        DBRA    D4,.copy
        CLR.B   (A1)
        LEA     v_txt,A0
; (A0) を (D0,D1) に色 D2 で（背景は COL_BG）。
puts:
        MOVEQ   #COL_BG,D3
        JMP     BIOS_PUTS

; D4.L を D5.W 桁の 16 進で (D0, STATUS_Y) に白で。
put_hex:
        MOVE.W  #STATUS_Y,D1
        MOVEQ   #COL_WHITE,D2
        MOVEQ   #COL_BG,D3
        JMP     BIOS_PUT_HEX

; A2 を v_cmd の頭にする。
cmd_begin:
        LEA     v_cmd,A2
        RTS

; v_cmd から A2 までの語を送る。
cmd_send:
        LEA     v_cmd,A0
        MOVE.L  A2,D1
        SUB.L   A0,D1
        LSR.W   #1,D1
        JMP     BIOS_ACRTC_SEND

; 描く色（CL0 と CL1）を D0.W（0〜15）にする。D5 を壊す。
cmd_color:
        MOVE.W  D0,D5
        ANDI.W  #15,D5
        MULU    #$1111,D5
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL0,(A2)+
        MOVE.W  D5,(A2)+
        MOVE.W  #ACRTC_WPR|ACRTC_P_CL1,(A2)+
        MOVE.W  D5,(A2)+
        RTS

; (D0,D1)〜(D2,D3)（画面の座標、両端を含む）を置き換えで塗る。y は符号を変える（bios-abi.md §06）。D5 を壊す。
cmd_rect:
        MOVE.W  #ACRTC_AMOVE,(A2)+
        MOVE.W  D0,(A2)+
        MOVE.W  D1,D5
        NEG.W   D5
        MOVE.W  D5,(A2)+
        MOVE.W  #ACRTC_AFRCT|ACRTC_OPM_REP,(A2)+
        MOVE.W  D2,(A2)+
        MOVE.W  D3,D5
        NEG.W   D5
        MOVE.W  D5,(A2)+
        RTS

; RWP を基本画面（DN＝1）の語の番地 D0.L にする。D5 を壊す。
cmd_rwp:
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPH,(A2)+
        MOVE.L  D0,D5
        LSR.L   #8,D5
        LSR.L   #4,D5
        ANDI.W  #$00FF,D5
        ORI.W   #$4000,D5
        MOVE.W  D5,(A2)+
        MOVE.W  #ACRTC_WPR|ACRTC_P_RWPL,(A2)+
        MOVE.W  D0,D5
        LSL.W   #4,D5
        MOVE.W  D5,(A2)+
        RTS

; ------------------------------------------------------------------ 表と文字列（文字 ROM に字形のある字だけ）

; パレット：0 黒、1〜12 は色相を 30°ずつ回した飽和色（赤から）、13・14 灰、15 白 [P]
palette:
        DC.W    $000,$F00,$F80,$FF0,$8F0,$0F0,$0F8,$0FF
        DC.W    $08F,$00F,$80F,$F0F,$F08,$444,$888,$FFF

str_title:      DC.B    "SAMPLE 5  BEND TEST",0
str_doc:        DC.B    "DOC BOARD",0
str_no_doc:     DC.B    "NO DOC BOARD",0
str_labels0:    DC.B    "0 1 2 3 4 5 6 7 ",0
str_labels1:    DC.B    "8 9 A B C D E F ",0
        EVEN

; 状態の行の名前：x、名前（2 字＋0、偶数にそろえる）。−1 で終わり
status_names:
        DC.W    0
        DC.B    "FR",0,0
        DC.W    64
        DC.B    "ST",0,0
        DC.W    108
        DC.B    "FC",0,0
        DC.W    172
        DC.B    "ER",0,0
        DC.W    216
        DC.B    "NV",0,0
        DC.W    -1

; 釦の行（7 字＋0）
help_items:
        DC.B    "A WAVE ",0
        DC.B    "B SOLO ",0
        DC.B    "C DRUM ",0
        DC.B    "S DRAW ",0

; 字の表：文字 ROM に字形のある 86 字（数字、英大文字、。、ー、カナ）を 32 字ずつ
text_rows:
        DC.B    "0123456789ABCDEFGHIJKLMNOPQRSTUV"
        DC.B    "WXYZ｡､ｰｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉ"
        DC.B    "ﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜｦﾝｯ          "
text_rows_end:
        ASSERT  text_rows_end-text_rows=3*32
        EVEN

; 太鼓：drums.bin の中の offset と byte 数
drum_table:
        DC.W    DRUM0_OFF,DRUM0_LEN
        DC.W    DRUM1_OFF,DRUM1_LEN
        DC.W    DRUM2_OFF,DRUM2_LEN
        ASSERT  DRUM_COUNT=3

        INCLUDE "song.inc"              ; build.py が作る（pitch_fc、bar_root、bar_tones、lead_pat、bass_pat、drum_pat）

; ------------------------------------------------------------------ build.py が作るデータ

        EVEN
wave_data:
        INCBIN  "waves.bin"             ; 正弦・三角・のこぎり・矩形、256 byte ずつ
wave_data_end:
        ASSERT  wave_data_end-wave_data=1024
pattern_data:
        INCBIN  "pattern.bin"           ; 市松と方眼、48 行×64 語
pattern_data_end:
        ASSERT  pattern_data_end-pattern_data=GRID_ROWS*64*2
drum_data:
        INCBIN  "drums.bin"
drum_data_end:
        ASSERT  drum_data_end-drum_data=DRUM_BYTES
        DCB.B   4,$08                   ; 写すときに 4 byte 単位で読み過ぎる分
        EVEN
rom_end:
        ASSERT  ((rom_end-CART)&1)=0
