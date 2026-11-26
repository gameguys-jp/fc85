; Boot ROM（BIOS）版 1。m68kasm で組む（firmware/boot/build.py）。
;
; 起動の順（sdk/docs/bios-abi.md §02）:
;   1. 例外のジャンプ表（Main RAM の $100000＋6n）を埋める（DR-025）
;   2. Data Fabric の MASK・XOR などを初期値にする（DR-046）
;   3. ACRTC を案B（256×192、4 bpp）で始め、パレットを書き、画面を消す
;   4. 音を黙らせる（DOC の子基板があれば全オシレータを止める。なければ本体 DAC を中央に）
;   5. ACIA をマスタリセットし、MIDI（31.25 kbit/s、8N1、受信の割込み）にする。入力の割込みを許す
;   6. VBlank の割込み（レベル 6）を許す
;   7. 割込みのマスクを 0 にする
;   8. $400000 にカートリッジの見出しがあれば、その入口へ飛ぶ
;   9. なければ内蔵のデモ

        INCLUDE "fc85.inc"
        INCLUDE "bios.inc"

; ------------------------------------------------------------------ 例外ベクタ（ROM の先頭 1 KiB）

        ORG     $000000
        DC.L    INITIAL_SSP             ; 0：リセットの SSP
        DC.L    reset                   ; 1：リセットの PC
V       SET     2
        REPT    254                     ; 2〜255：ジャンプ表の項目（DR-025）
        DC.L    JUMP_TABLE+6*V
V       SET     V+1
        ENDR

; ------------------------------------------------------------------ 呼出しの入口（bios.inc）

        ORG     BIOS_WAIT_VBL
        JMP     (bios_wait_vbl).L
        JMP     (bios_acrtc_send).L
        JMP     (bios_cls).L
        JMP     (bios_puts).L
        JMP     (bios_put_hex).L
        JMP     (bios_set_pal).L
        JMP     (bios_read_pad).L
        ASSERT  *=BIOS_READ_PAD+6

        ORG     BIOS_INFO
        DC.W    BIOS_VERSION

; ------------------------------------------------------------------ リセット

        ORG     $000500
reset:
        MOVE.W  #$2700,SR
        LEA     INITIAL_SSP,SP
        ; BIOS の変数を 0 にする
        LEA     BIOS_VARS,A0
        MOVE.W  #(BIOS_VARS_END-BIOS_VARS)/4-1,D0
.clr    CLR.L   (A0)+
        DBRA    D0,.clr
        ; 1. ジャンプ表
        BSR     fill_jump_table
        ; 2. Data Fabric
        BSR     fabric_init
        MOVE.W  FABRIC_MODULES,bios_modules
        ; 3. 映像
        BSR     acrtc_init
        LEA     default_palette(PC),A0
        BSR     bios_set_pal
        MOVEQ   #0,D0
        BSR     bios_cls
        ; 4. 音
        BSR     sound_silence
        ; 5. ACIA と入力
        MOVE.B  #ACIA_RESET,MIDI_CTRL
        MOVE.B  #ACIA_MIDI,MIDI_CTRL
        MOVE.W  #INPUT_IRQ_PORT_A|INPUT_IRQ_PORT_B|INPUT_IRQ_EXTRIG,INPUT_IRQ_STATUS
        MOVE.W  #INPUT_IRQ_PORT_A|INPUT_IRQ_PORT_B,INPUT_IRQ_ENABLE
        ; 6. VBlank の割込み（古い保留を消してから許す）
        MOVE.W  #VIRQ_VBL|VIRQ_HBL|VIRQ_VBL_EN,VIDEO_IRQ
        ; 7. 割込みを受ける
        MOVE.W  #$2000,SR
        ; 8. カートリッジ
        BSR     cart_probe
        ; 9. デモ
        BRA     demo

; ------------------------------------------------------------------ ジャンプ表

; 全部の項目を JMP abs.L にする。既定は ROM の小さな入口（ベクタ番号を積んで exc_fatal へ）で、
; 割込みのいくつかは BIOS の処理にする。
fill_jump_table:
        LEA     JUMP_TABLE,A0
        LEA     halt,A1                 ; 0・1（リセット）は使わない。止まる先にしておく
        MOVEQ   #2-1,D0
.unused MOVE.W  #$4EF9,(A0)+
        MOVE.L  A1,(A0)+
        DBRA    D0,.unused
        LEA     exc_stubs,A1
        MOVE.W  #254-1,D0
.fill   MOVE.W  #$4EF9,(A0)+
        MOVE.L  A1,(A0)+
        ADDQ.L  #8,A1
        DBRA    D0,.fill
        MOVE.L  #irq_count,JUMP_TABLE+VEC_SPURIOUS*6+2
        MOVE.L  #irq_input,JUMP_TABLE+(VEC_AUTO1-1+IRQ_INPUT)*6+2
        MOVE.L  #irq_acia,JUMP_TABLE+(VEC_AUTO1-1+IRQ_ACIA)*6+2
        MOVE.L  #irq_video,JUMP_TABLE+(VEC_AUTO1-1+IRQ_VIDEO)*6+2
        MOVE.L  #irq_doc,JUMP_TABLE+(VEC_AUTO1-1+IRQ_DOC)*6+2
        MOVE.L  #irq_count,JUMP_TABLE+(VEC_AUTO1+6)*6+2
        RTS

; ------------------------------------------------------------------ Data Fabric

; 両チャネルを止め、データの変換を素通しにし、状態を消す。MASK・XOR はリセットで変わらないので、
; 電源投入の後に BIOS が書く（仕様書 §06、DR-046）。AUDIO_N は 500（16 kHz）にしておく [P]。
fabric_init:
        LEA     FABRIC_CH0,A0
        BSR.S   .ch
        LEA     FABRIC_CH1,A0
        BSR.S   .ch
        MOVE.W  #500,FABRIC_AUDIO_N
        RTS
.ch     MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #$FFFF,FAB_MASK(A0)
        MOVE.W  #0,FAB_XOR(A0)
        MOVE.W  #0,FAB_SHIFT(A0)
        MOVE.W  #FMT_8_8,FAB_FORMAT(A0)
        MOVE.W  #TRG_NOW,FAB_TRIGGER(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        RTS

; ------------------------------------------------------------------ カートリッジ

; 見出しを読む。カートリッジがなければ窓はバスエラーなので、その間だけバスエラーの項目を差し替える。
; 見出しが正しければ戻らない。
cart_probe:
        MOVE.L  JUMP_TABLE+VEC_BUS_ERROR*6+2,-(SP)
        MOVE.L  #probe_berr,JUMP_TABLE+VEC_BUS_ERROR*6+2
        MOVE.L  SP,bios_probe_sp
        MOVE.L  CART+cart_magic,D0      ; ない：バスエラー → probe_berr
        MOVE.W  CART+cart_version,D1
        MOVEA.L CART+cart_entry,A1
        MOVE.L  (SP)+,JUMP_TABLE+VEC_BUS_ERROR*6+2
        CMP.L   #CART_MAGIC,D0
        BNE.S   .none
        CMP.W   #CART_HDR_VERSION,D1
        BNE.S   .none
        MOVE.L  A1,D0
        BTST    #0,D0
        BNE.S   .none
        CMP.L   #CART,D0
        BLO.S   .none
        CMP.L   #CART_END,D0
        BHS.S   .none
        ; 入口の条件（bios-abi.md §04）
        MOVE.W  #$2700,SR
        LEA     INITIAL_SSP,SP
        MOVEQ   #0,D0
        BSR     bios_cls
        LEA     CART,A0
        MOVEQ   #0,D0
        MOVE.W  #BIOS_VERSION,D0
        MOVEQ   #0,D1
        MOVE.W  bios_modules,D1
        MOVE.W  #$2000,SR
        JMP     (A1)
.none   RTS

; カートリッジがない：SP を戻し、元の項目に戻して、cart_probe から帰る。
probe_berr:
        MOVEA.L bios_probe_sp,SP
        MOVE.L  (SP)+,JUMP_TABLE+VEC_BUS_ERROR*6+2
        RTS

; ------------------------------------------------------------------ 割込み

; レベル 6：映像。VBlank なら数を 1 足し、パッドを読み、フックを呼ぶ。
irq_video:
        MOVEM.L D0-D1/A0-A1,-(SP)
        MOVE.W  VIDEO_IRQ,D0
        MOVE.W  D0,D1
        ANDI.W  #VIRQ_VBL|VIRQ_HBL,D1
        BEQ.S   .out
        ANDI.W  #VIRQ_VBL_EN|VIRQ_HBL_EN|VIRQ_LINE_CMP,D0
        OR.W    D1,D0
        MOVE.W  D0,VIDEO_IRQ            ; 見た保留だけを消し、許可はそのまま
        BTST    #8,D1
        BEQ.S   .out
        ADDQ.L  #1,bios_frame
        MOVE.W  PORT_A,bios_pad_a
        MOVE.W  PORT_B,bios_pad_b
        MOVE.L  bios_vbl_hook,D0
        BEQ.S   .out
        MOVEA.L D0,A0
        JSR     (A0)
.out    MOVEM.L (SP)+,D0-D1/A0-A1
        RTE

; レベル 5：ACIA（MIDI の受信）。状態→データの順に読んで解除する（DR-028）。
irq_acia:
        MOVEM.L D0-D1/A0,-(SP)
        MOVE.B  MIDI_STATUS,D0
        BPL.S   .out                    ; IRQ のビットが 0
        MOVE.B  MIDI_DATA,D1
        BTST    #0,D0                   ; RDRF
        BEQ.S   .out
        LEA     bios_midi_buf,A0
        MOVE.W  bios_midi_head,D0
        MOVE.B  D1,0(A0,D0.W)
        ADDQ.W  #1,D0
        ANDI.W  #63,D0
        CMP.W   bios_midi_tail,D0
        BEQ.S   .full
        MOVE.W  D0,bios_midi_head
        BRA.S   .out
.full   ADDQ.W  #1,bios_midi_lost
.out    MOVEM.L (SP)+,D0-D1/A0
        RTE

; レベル 1：入力。見たビットを消して貯める。
irq_input:
        MOVE.W  D0,-(SP)
        MOVE.W  INPUT_IRQ_STATUS,D0
        ANDI.W  #INPUT_IRQ_PORT_A|INPUT_IRQ_PORT_B|INPUT_IRQ_EXTRIG,D0
        MOVE.W  D0,INPUT_IRQ_STATUS
        OR.W    D0,bios_input_status
        ADDQ.W  #1,bios_input_events
        MOVE.W  (SP)+,D0
        RTE

; レベル 4：DOC。アプリが項目を書いていないときの既定（DR-076）。DOC の割込みには基板の許可がなく、
; ベンドで崩れたレジスタ（DR-074）や線（DR-073）が IE を立てると要求が来る。要求を 1 つ取り、その
; オシレータの IE を落として数える。DOC が遅すぎて応えない（バスの時間切れ）ときは、戻る先の割込みの
; マスクを 4 以上にする（レベル 5・6 はそのまま受ける）。
irq_doc:
        MOVEM.L D0-D1/A0,-(SP)
        ADDQ.W  #1,bios_spurious
        MOVE.L  JUMP_TABLE+VEC_BUS_ERROR*6+2,-(SP)
        MOVE.L  #.berr,JUMP_TABLE+VEC_BUS_ERROR*6+2
        MOVE.B  DOC_IRQ,D0              ; bit 7＝0 で要求あり。bit 5–1 がオシレータの番号
        BMI.S   .done
        ANDI.W  #$003E,D0               ; 番号×2＝レジスタの番地の差（DOC_OSC_STRIDE）
        LEA     DOC_CONTROL,A0
        MOVE.B  0(A0,D0.W),D1
        ANDI.B  #$FF-DOC_CTL_IE,D1
        MOVE.B  D1,0(A0,D0.W)
.done   MOVE.L  (SP)+,JUMP_TABLE+VEC_BUS_ERROR*6+2
        MOVEM.L (SP)+,D0-D1/A0
        RTE
.berr   LEA     14(SP),SP               ; バスエラーの 7 語を捨てる
        MOVE.L  (SP)+,JUMP_TABLE+VEC_BUS_ERROR*6+2
        MOVE.W  12(SP),D0               ; 戻る先の SR（D0–D1/A0 の 12 byte の上）
        ANDI.W  #$0700,D0
        CMPI.W  #$0400,D0
        BHS.S   .keep
        ANDI.W  #$F8FF,12(SP)
        ORI.W   #$0400,12(SP)
.keep   MOVEM.L (SP)+,D0-D1/A0
        RTE

; スプリアス割込みとレベル 7（予約）：数えるだけ。
irq_count:
        ADDQ.W  #1,bios_spurious
        RTE

; ------------------------------------------------------------------ 呼出し

; 次の VBlank まで待つ。STOP で割込みを待ち、SR を元に戻す。
bios_wait_vbl:
        MOVE.L  D0,-(SP)
        MOVE.W  SR,-(SP)
        MOVE.L  bios_frame,D0
.wait   STOP    #$2000
        CMP.L   bios_frame,D0
        BEQ.S   .wait
        MOVE.W  (SP)+,SR
        MOVE.L  (SP)+,D0
        RTS

; パッド A を読む。D0＝今の値、D1＝前の呼出しから新しく押したビット。
bios_read_pad:
        MOVE.W  PORT_A,D0
        MOVE.W  bios_pad_prev,D1
        NOT.W   D1
        AND.W   D0,D1
        MOVE.W  D0,bios_pad_prev
        RTS

; (A0) の 16 語をパレットに書く。
bios_set_pal:
        MOVEM.L D0/A0-A1,-(SP)
        LEA     PALETTE,A1
        MOVEQ   #15,D0
.copy   MOVE.W  (A0)+,(A1)+
        DBRA    D0,.copy
        MOVEM.L (SP)+,D0/A0-A1
        RTS

; 既定のパレット [P]。0 黒、1 背景、2 帯、3〜10 色、11・12 灰、13 茶、14 淡い黄、15 白。
default_palette:
        DC.W    $000,$013,$247,$F44,$F84,$FE4,$4D4,$4CE
        DC.W    $48F,$A6F,$F6C,$888,$CCC,$842,$FFA,$FFF

; ------------------------------------------------------------------ 致命的な例外

; ジャンプ表の既定の入口から来る。(SP) にベクタ番号、その上に例外のフレーム。
; 情報を BIOS の変数に残し、画面に出して止まる。
exc_fatal:
        MOVE.W  #$2700,SR
        TST.W   bios_exc_busy
        BNE     halt                    ; 処理の中でまた例外：画面に出さずに止まる
        MOVE.W  #1,bios_exc_busy
        MOVE.W  (SP)+,D0
        MOVE.W  D0,bios_exc_vector
        CMP.W   #VEC_ADDR_ERROR,D0
        BHI.S   .short
        CMP.W   #VEC_BUS_ERROR,D0
        BLO.S   .short
        ; バスエラー・アドレスエラー：状態語、番地、命令の語、SR、PC
        MOVE.L  2(SP),bios_exc_addr
        MOVE.W  8(SP),bios_exc_sr
        MOVE.L  10(SP),bios_exc_pc
        BRA.S   .show
.short  MOVE.W  (SP),bios_exc_sr
        MOVE.L  2(SP),bios_exc_pc
        CLR.L   bios_exc_addr
.show   LEA     INITIAL_SSP,SP
        BSR     acrtc_init
        LEA     default_palette(PC),A0
        BSR     bios_set_pal
        MOVE.W  #$600,PALETTE           ; 色 0 を暗い赤に
        MOVEQ   #0,D0
        BSR     bios_cls
        MOVEQ   #15,D2
        MOVEQ   #0,D3
        LEA     str_exception(PC),A0
        MOVEQ   #16,D0
        MOVEQ   #16,D1
        BSR     bios_puts
        MOVEQ   #0,D4
        MOVE.W  bios_exc_vector,D4
        MOVEQ   #2,D5
        MOVE.W  #104,D0
        BSR     bios_put_hex
        LEA     str_pc(PC),A0
        MOVEQ   #16,D0
        MOVEQ   #32,D1
        BSR     bios_puts
        MOVE.L  bios_exc_pc,D4
        MOVEQ   #8,D5
        MOVEQ   #56,D0
        BSR     bios_put_hex
        LEA     str_sr(PC),A0
        MOVEQ   #16,D0
        MOVEQ   #44,D1
        BSR     bios_puts
        MOVEQ   #0,D4
        MOVE.W  bios_exc_sr,D4
        MOVEQ   #4,D5
        MOVEQ   #56,D0
        BSR     bios_put_hex
        LEA     str_addr(PC),A0
        MOVEQ   #16,D0
        MOVEQ   #56,D1
        BSR     bios_puts
        MOVE.L  bios_exc_addr,D4
        MOVEQ   #8,D5
        MOVEQ   #56,D0
        BSR     bios_put_hex
        LEA     str_halted(PC),A0
        MOVEQ   #16,D0
        MOVEQ   #80,D1
        BSR     bios_puts
halt:   STOP    #$2700
        BRA.S   halt

str_exception:  DC.B    "EXCEPTION",0
str_pc:         DC.B    "PC",0
str_sr:         DC.B    "SR",0
str_addr:       DC.B    "ADDR",0
str_halted:     DC.B    "SYSTEM HALTED",0
        EVEN

        INCLUDE "video.s"
        INCLUDE "sound.s"
        INCLUDE "demo.s"

; ------------------------------------------------------------------ ジャンプ表の既定の入口

; ベクタ n の入口：MOVE.W #n,-(SP)、BRA.W exc_fatal（8 byte）。ベクタ 2 から順に並ぶ。
        EVEN
exc_stubs:
V       SET     2
        REPT    254
        MOVE.W  #V,-(SP)
        BRA.W   exc_fatal
V       SET     V+1
        ENDR
exc_stubs_end:
        ASSERT  exc_stubs_end-exc_stubs=254*8
