; 音（bios.s から INCLUDE）。
;
; DOC の子基板があるか（FABRIC_MODULES の bit 2）で道を分ける（DR-019）。
; - DOC：波形 RAM は Fabric の port 2 からだけ書ける。CPU が staging RAM に波形を置き、チャネル 0 で
;   波形 RAM の 0〜255 に運ぶ。オシレータ 0（CA 0、L）と 1（CA 1、R）を同じ音程で自走させ、音量で包絡を作る。
;   DOC の窓は MOVE.B の 1 回の書込みだけで触る（DR-030）。
; - 本体 DAC：staging RAM の 64 語（1 周期の三角波、上位 L・下位 R）を、チャネル 1 が LOOP で port 2 の
;   FIFO に送り続ける。音程は AUDIO_N（標本の速さ＝8 MHz÷N）、包絡は CPU が 64 語を毎フレーム書き直す。
;   子基板があるとき DAC の窓（$230010〜）はバスエラーなので触らない。

SND_OSC_COUNT   EQU     16              ; DOC の有効なオシレータの数 N（E1＝(N−1)×2）[P]
; 止めたオシレータも、走査の中にあれば CA のチャンネルに中央の値を出し、そのチャンネルのサンプルホールドを
; 0 に引き戻す（emulator.md §03、ES5503 の試験 halted_oscillator_still_fetches_and_outputs_center）[V]。
; 使わないオシレータは、鳴らすオシレータと別のチャンネル（BIOS は 15）に置く [P]。
SND_IDLE_CTL    EQU     (15<<DOC_CTL_CA_SH)|DOC_CTL_HALT
SND_ENV_START   EQU     240             ; 包絡の初めの値（DOC の音量、0〜255）
SND_ENV_STEP    EQU     8               ; 1 フレームに下げる量
SND_WAVE        EQU     STAGING         ; DOC の波形を置く staging の番地
SND_DAC_BUF     EQU     STAGING+$100    ; DAC の 64 語
SND_DAC_BUF_OFF EQU     $0100           ; その port 0 の番地

; ------------------------------------------------------------------ 起動時：黙らせる

sound_silence:
        MOVE.W  bios_modules,D0
        BTST    #FABRIC_MOD_DOC_B,D0
        BEQ.S   .dac
        ; DOC：全部のオシレータを止めてチャンネル 15 に置き、音量を 0 にしてから走査を始める
        LEA     DOC_CONTROL,A0
        MOVEQ   #32-1,D0
.halt   MOVE.B  #SND_IDLE_CTL,(A0)
        ADDQ.L  #DOC_OSC_STRIDE,A0
        DBRA    D0,.halt
        LEA     DOC_VOLUME,A0
        MOVEQ   #32-1,D0
.vol    MOVE.B  #0,(A0)
        ADDQ.L  #DOC_OSC_STRIDE,A0
        DBRA    D0,.vol
        MOVE.B  #(SND_OSC_COUNT-1)*2,DOC_OSC_ENABLE
        RTS
.dac    MOVE.W  #DAC_CTRL_DIRECT,DAC_CTRL
        MOVE.W  #DAC_CENTER,DAC_DATA
        RTS

; ------------------------------------------------------------------ デモの音

        RSSET   $100700
snd_mode        RS.W    1               ; 0 なし、1 DOC、2 本体 DAC
snd_env         RS.W    1               ; 包絡（0〜SND_ENV_START）
snd_env_out     RS.W    1               ; 最後に出した包絡
snd_vars_end    RS.B    0

; 波形を置き、鳴らす用意をする。
snd_init:
        MOVE.W  #0,snd_mode
        MOVE.W  #0,snd_env
        MOVE.W  #-1,snd_env_out
        MOVE.W  bios_modules,D0
        BTST    #FABRIC_MOD_DOC_B,D0
        BEQ     snd_init_dac
; DOC
        LEA     doc_wave(PC),A0
        LEA     SND_WAVE,A1
        MOVE.W  #256/4-1,D0
.copy   MOVE.L  (A0)+,(A1)+
        DBRA    D0,.copy
        LEA     FABRIC_CH0,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_DOC,FAB_DST(A0)
        MOVE.W  #SND_WAVE-STAGING,FAB_SRC_ADDR_L(A0)
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
        MOVE.W  #1,snd_mode
        RTS

snd_init_dac:
        LEA     SND_DAC_BUF,A1
        MOVEQ   #64-1,D0
.fill   MOVE.W  #DAC_CENTER,(A1)+
        DBRA    D0,.fill
        MOVE.W  #239,FABRIC_AUDIO_N
        MOVE.W  #1<<PORT_DOC,FABRIC_PORT_RESET
        MOVE.W  #0,DAC_CTRL             ; FIFO から読む
        LEA     FABRIC_CH1,A0
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_DOC,FAB_DST(A0)
        MOVE.W  #SND_DAC_BUF_OFF,FAB_SRC_ADDR_L(A0)
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
        MOVE.W  #2,snd_mode
        RTS

; 音を出す。D0.W＝音の番号（0〜3）。
snd_note:
        MOVEM.L D0-D1,-(SP)
        MOVE.W  snd_mode,D1
        BEQ.S   .ret
        ANDI.W  #3,D0
        ADD.W   D0,D0
        CMP.W   #1,D1
        BNE.S   .dac
        MOVE.W  note_fc(PC,D0.W),D1
        MOVE.B  D1,DOC_FREQ_LO
        MOVE.B  D1,DOC_FREQ_LO+DOC_OSC_STRIDE
        LSR.W   #8,D1
        MOVE.B  D1,DOC_FREQ_HI
        MOVE.B  D1,DOC_FREQ_HI+DOC_OSC_STRIDE
        BRA.S   .env
.dac    MOVE.W  note_n(PC,D0.W),FABRIC_AUDIO_N
.env    MOVE.W  #SND_ENV_START,snd_env
.ret    MOVEM.L (SP)+,D0-D1
        RTS

; C5、E5、G5、C6。DOC：f＝(1 MHz÷(N＋2))×FC÷2^17（N＝16）。DAC：f＝(8 MHz÷N)÷64。
note_fc:
        DC.W    1235,1555,1850,2469
note_n:
        DC.W    239,190,159,119

; 毎フレーム：包絡を下げて出す。
snd_update:
        MOVEM.L D0-D3/A0-A1,-(SP)
        MOVE.W  snd_env,D0
        BEQ.S   .out
        SUBQ.W  #SND_ENV_STEP,D0
        BPL.S   .pos
        MOVEQ   #0,D0
.pos    MOVE.W  D0,snd_env
.out    CMP.W   snd_env_out,D0
        BEQ.S   .ret
        MOVE.W  D0,snd_env_out
        MOVE.W  snd_mode,D1
        BEQ.S   .ret
        CMP.W   #1,D1
        BNE.S   .dac
        MOVE.B  D0,DOC_VOLUME
        MOVE.B  D0,DOC_VOLUME+DOC_OSC_STRIDE
        BRA.S   .ret
.dac    LEA     dac_tri(PC),A0
        LEA     SND_DAC_BUF,A1
        MOVEQ   #64-1,D2
.word   MOVE.B  (A0)+,D1
        EXT.W   D1
        MULS    D0,D1
        ASR.L   #8,D1
        ADDI.W  #128,D1
        MOVE.W  D1,D3
        LSL.W   #8,D3
        OR.W    D1,D3
        MOVE.W  D3,(A1)+
        DBRA    D2,.word
.ret    MOVEM.L (SP)+,D0-D3/A0-A1
        RTS

; DOC の波形：256 byte の三角波（1〜255。0 はオシレータを止めるので使わない）
doc_wave:
I       SET     0
        REPT    128
        DC.B    1+2*I
I       SET     I+1
        ENDR
I       SET     0
        REPT    128
        DC.B    255-2*I
I       SET     I+1
        ENDR

; DAC の三角波：64 個の符号付きの値（−124〜124）
dac_tri:
I       SET     0
        REPT    32
        DC.B    -124+8*I
I       SET     I+1
        ENDR
I       SET     0
        REPT    32
        DC.B    124-8*I
I       SET     I+1
        ENDR
        EVEN
