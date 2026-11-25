; 拍の低音（game.s から INCLUDE）。A2・G2・F2・E2 を 1 拍ずつ繰り返し、石が増えると速くなる。
;
; BIOS と同じく FABRIC_MODULES（起動時の写しは bios_modules）の bit 2 で道を分ける（DR-019、bios-abi.md §06）。
; - DOC の子基板がある：矩形波 256 byte を Staging RAM に置き、Fabric のチャネル 1（port 0 → port 2、
;   FORMAT 0、即時）で波形 RAM の頁 0 に運ぶ。オシレータ 0（CA 0、L）と 1（CA 1、R）を自走させ、FC で音程、
;   音量で包絡。DOC の窓には MOVE.B の 1 回の書込みだけを使う（DR-030）。
; - ない：本体 DAC。Staging RAM の 64 語（1 周期の矩形波、上位 L・下位 R）を、チャネル 1 が LOOP で port 2
;   の FIFO に送り続ける。音程は AUDIO_N（f＝8 MHz÷N÷64）、包絡は CPU が 64 語を書き直す。
; チャネル 0 は効果音（sfx.s）が使う。

MU_WAVE         EQU     STAGING         ; DOC の波形（port 0 の番地 $0000）
MU_DAC_BUF      EQU     STAGING+$100    ; 本体 DAC の 64 語
MU_DAC_OFF      EQU     $0100           ; その port 0 の番地
MU_ENV_START    EQU     255             ; 包絡（0〜255）。DOC の音量は ÷4、DAC の振幅は ×127÷4096 [P]
        ASSERT  MU_DAC_OFF+128<=SFX_STAGE

music_init:
        CLR.W   mu_mode
        CLR.W   mu_on
        CLR.W   mu_env
        MOVE.W  #-1,mu_env_out
        MOVE.W  bios_modules,D0
        BTST    #FABRIC_MOD_DOC_B,D0
        BEQ     music_init_dac
; ---- DOC
        LEA     MU_WAVE,A1              ; 矩形波（$C0 と $40。0 はオシレータを止めるので使わない）
        MOVEQ   #128/4-1,D0
.hi     MOVE.L  #$C0C0C0C0,(A1)+
        DBRA    D0,.hi
        MOVEQ   #128/4-1,D0
.lo     MOVE.L  #$40404040,(A1)+
        DBRA    D0,.lo
        LEA     FABRIC_CH1,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_DOC,FAB_DST(A0)
        MOVE.W  #MU_WAVE-STAGING,FAB_SRC_ADDR_L(A0)
        MOVE.W  #$0000,FAB_DST_ADDR_L(A0)       ; 波形 RAM の頁 0
        MOVE.W  #256,FAB_COUNT(A0)
        MOVE.W  #1,FAB_SRC_STEP(A0)
        MOVE.W  #1,FAB_DST_STEP(A0)
        MOVE.W  #FMT_8_8,FAB_FORMAT(A0)
        MOVE.W  #$FFFF,FAB_MASK(A0)
        MOVE.W  #0,FAB_XOR(A0)
        MOVE.W  #0,FAB_SHIFT(A0)
        MOVE.W  #TRG_NOW,FAB_TRIGGER(A0)
        MOVE.W  #FAB_START,FAB_CTRL(A0)
        MOVE.L  #20000,D1               ; 待つ上限（約 0.1 秒）
.wait   MOVE.W  FAB_STATUS(A0),D0
        BTST    #3,D0                   ; ERROR
        BNE.S   .fail
        BTST    #1,D0                   ; DONE
        BNE.S   .loaded
        SUBQ.L  #1,D1
        BNE.S   .wait
.fail   MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        RTS
.loaded MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        ; オシレータ 0（L）と 1（R）：頁 0 の 256 byte、分解能 0、自走、音量 0
        LEA     DOC,A0
        MOVEQ   #2-1,D1
        MOVEQ   #0,D2                   ; CA
.osc    MOVE.B  #0,2*$40(A0)            ; VOLUME
        MOVE.B  #0,2*$80(A0)            ; POINTER
        MOVE.B  #$00,2*$C0(A0)          ; SIZE：T＝0（256 byte）、R＝0
        MOVE.B  #0,2*$00(A0)            ; FREQ_LO
        MOVE.B  #0,2*$20(A0)            ; FREQ_HI
        MOVE.B  D2,2*$A0(A0)            ; CONTROL：CA、自走、動かす
        ADDI.B  #1<<DOC_CTL_CA_SH,D2
        ADDQ.L  #DOC_OSC_STRIDE,A0
        DBRA    D1,.osc
        MOVE.W  #1,mu_mode
        RTS

music_init_dac:
        LEA     MU_DAC_BUF,A1
        MOVEQ   #64-1,D0
.fill   MOVE.W  #DAC_CENTER,(A1)+
        DBRA    D0,.fill
        MOVE.W  mu_n(PC),FABRIC_AUDIO_N
        MOVE.W  #1<<PORT_DOC,FABRIC_PORT_RESET
        MOVE.W  #0,DAC_CTRL             ; DIRECT を下げる：拍で FIFO から読む
        LEA     FABRIC_CH1,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_DOC,FAB_DST(A0)
        MOVE.W  #MU_DAC_OFF,FAB_SRC_ADDR_L(A0)
        MOVE.W  #0,FAB_DST_ADDR_L(A0)
        MOVE.W  #64,FAB_COUNT(A0)
        MOVE.W  #2,FAB_SRC_STEP(A0)
        MOVE.W  #0,FAB_DST_STEP(A0)
        MOVE.W  #FMT_16_16,FAB_FORMAT(A0)
        MOVE.W  #$FFFF,FAB_MASK(A0)
        MOVE.W  #0,FAB_XOR(A0)
        MOVE.W  #0,FAB_SHIFT(A0)
        MOVE.W  #TRG_NOW|TRG_LOOP,FAB_TRIGGER(A0)
        MOVE.W  #FAB_START,FAB_CTRL(A0)
        MOVE.W  #2,mu_mode
        RTS

music_start:
        MOVE.W  #1,mu_on
        MOVE.W  #1,mu_timer
        CLR.W   mu_step
        RTS

music_stop:
        CLR.W   mu_on
        CLR.W   mu_env
        RTS

; 1 フレームに 1 回。
music_update:
        MOVEM.L D0-D4/A0-A1,-(SP)
        TST.W   mu_mode
        BEQ     .ret
        TST.W   mu_on
        BEQ     .env
        SUBQ.W  #1,mu_timer
        BGT     .env
        ; 拍：間隔は 36−3×(石の数) フレーム（12 まで）[P]
        LEA     g_rocks,A0
        MOVEQ   #ROCK_MAX-1,D1
        MOVEQ   #36,D0
.count  TST.W   OB_ON(A0)
        BEQ.S   .nx
        SUBQ.W  #3,D0
.nx     LEA     OB_SIZE(A0),A0
        DBRA    D1,.count
        CMP.W   #12,D0
        BGE.S   .tempo
        MOVEQ   #12,D0
.tempo  MOVE.W  D0,mu_timer
        MOVE.W  mu_step,D0
        ADDQ.W  #1,mu_step
        ANDI.W  #3,mu_step
        ADD.W   D0,D0
        CMP.W   #1,mu_mode
        BNE.S   .dacn
        LEA     mu_fc(PC),A0
        MOVE.W  0(A0,D0.W),D1
        MOVE.B  D1,DOC_FREQ_LO
        MOVE.B  D1,DOC_FREQ_LO+DOC_OSC_STRIDE
        LSR.W   #8,D1
        MOVE.B  D1,DOC_FREQ_HI
        MOVE.B  D1,DOC_FREQ_HI+DOC_OSC_STRIDE
        BRA.S   .hit
.dacn   LEA     mu_n(PC),A0
        MOVE.W  0(A0,D0.W),FABRIC_AUDIO_N
.hit    MOVE.W  #MU_ENV_START,mu_env
        BRA.S   .out
        ; 包絡：毎フレーム 7/8 にして 1 を引く
.env    MOVE.W  mu_env,D0
        BEQ.S   .out
        MOVE.W  D0,D1
        LSR.W   #3,D1
        SUB.W   D1,D0
        SUBQ.W  #1,D0
        BPL.S   .pos
        MOVEQ   #0,D0
.pos    MOVE.W  D0,mu_env
.out    MOVE.W  mu_env,D0
        CMP.W   mu_env_out,D0
        BEQ.S   .ret
        MOVE.W  D0,mu_env_out
        CMP.W   #1,mu_mode
        BNE.S   .dac
        LSR.W   #2,D0                   ; DOC の音量 0〜63
        MOVE.B  D0,DOC_VOLUME
        MOVE.B  D0,DOC_VOLUME+DOC_OSC_STRIDE
        BRA.S   .ret
.dac    MULU    #127,D0                 ; 振幅 0〜7（8 bit の DAC の段）
        LSR.L   #8,D0
        LSR.L   #4,D0
        MOVE.W  #128,D1
        ADD.W   D0,D1                   ; 上の半周期
        MOVE.W  #128,D2
        SUB.W   D0,D2                   ; 下の半周期
        MOVE.W  D1,D3
        LSL.W   #8,D3
        OR.W    D1,D3                   ; L と R に同じ値
        MOVE.W  D3,D1
        SWAP    D3
        MOVE.W  D1,D3
        MOVE.W  D2,D4
        LSL.W   #8,D4
        OR.W    D2,D4
        MOVE.W  D4,D2
        SWAP    D4
        MOVE.W  D2,D4
        LEA     MU_DAC_BUF,A1
        MOVEQ   #16-1,D1
.up     MOVE.L  D3,(A1)+
        DBRA    D1,.up
        MOVEQ   #16-1,D1
.down   MOVE.L  D4,(A1)+
        DBRA    D1,.down
.ret    MOVEM.L (SP)+,D0-D4/A0-A1
        RTS

; A2・G2・F2・E2。DOC：f＝(1 MHz÷(16＋2))×FC÷2^17（16 オシレータ、256 byte の表）。DAC：f＝8 MHz÷N÷64。
mu_fc:
        DC.W    260,231,206,194
mu_n:
        DC.W    1136,1276,1432,1517
