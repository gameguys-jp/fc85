; 効果音：MSM5205 の ADPCM を Data Fabric で鳴らす（game.s から INCLUDE）。手順は README.md §04。
;
; 道：カートリッジ ROM ─(CPU が起動時に写す)→ Staging RAM ─(Fabric のチャネル 0、port 0 → port 5)→
;     ADPCM の FIFO（64 byte）─(bridge が V̄C̄K̄ ごとにニブルを渡す)→ MSM5205 → DAC
;
; - Fabric は Staging RAM を読めるが、カートリッジは読めない（ポートがない）。そこで起動時に CPU が写す。
; - TRIGGER は 0（即時）で、ブロック（COUNT 個）を 1 回で送る。FIFO が満杯の間、チャネルは STALL で待つ
;   （仕様書 §06。DR-018 の「ADPCM への供給は FIFO の満杯待ちで足りる」）[P]。V̄C̄K̄（TRIGGER 5）で 1 byte
;   ずつ送ると、1 byte に 2 ニブルあるので、送る速さが使う速さの 2 倍になり、FIFO が満ちた後は引き金の
;   取りこぼし（MISS）が続くはず（試していない）[V]。
; - FORMAT 0（8→8）：port 0 の 8 bit の読出し（偶数番地が上位 byte、DR-046）を、8 bit の port 5 に書く。
; - S1・S2（ADPCM の CTRL）で、音ごとに 8・6・4 kHz を選ぶ（sfx.inc の SFXn_CTRL）。
; - MSM5205 の RESET は取込み（V̄C̄K̄ の立下りの 6 クロック後）の時点で効く（台帳 MSM5205.md §3.3、エミュレータ
;   も同じ）。そこで RESET を 1 フレーム保ってから外す。符号は RESET の直後（予測値 0、ステップ 0）から作る。
; - RESET の間、bridge はニブルを置かない。D3〜D0 には RESET の前に置いた最後のニブルが残り、RESET を外した
;   のが V̄C̄K̄ の立上りと取込みの間なら、それが 1 つ復号される。外す時点を V̄C̄K̄ に合わせる手段は CPU にない
;   （ADPCM の STATUS に V̄C̄K̄ も取込みもない）。そこで RESET をかけるのは FIFO が空のとき（bridge が 0x08 を
;   置いている）だけにし、残るニブルを 0 か 8 にする。ずれは予測値の ±2 にとどまる [P]。鳴っている音を止める
;   ときは、FIFO を空にして 1 フレーム待ってから RESET をかける（FLUSH）。
; - 音の頭の 0x08 の 5 byte は、それでも残ったニブルが大きかったときにステップ番号を 0 にそろえる保険
;   （build.py）[P]。
;
; 1 フレームに 1 回 sfx_update を呼ぶ。状態：
;   IDLE    RESET。チャネル 0 は止まっている。
;   FLUSH   鳴っている音を止めた：チャネル 0 を止め、FIFO を空にした。RESET はまだかけない。
;   ARMED   RESET をかけた（S1・S2 は次の音のもの）。次の sfx_update で始める。
;   PLAYING チャネル 0 を始め、FIFO に 16 byte たまったら RESET を外した。
;           チャネル 0 が DONE で FIFO が空になったら IDLE に戻す（RESET をかける）。
; 求めから音が出るまで：IDLE からは 1〜2 フレーム、鳴っている音を替えるときは 2〜3 フレーム。

SFX_IDLE        EQU     0
SFX_ARMED       EQU     1
SFX_PLAYING     EQU     2
SFX_FLUSH       EQU     3
SFX_STAGE       EQU     $1000           ; Staging RAM の中の置き場所（port 0 の番地）[P]
SFX_ENTRY       EQU     8               ; sfx_table の 1 行
SFX_PRIME       EQU     16              ; RESET を外す前に FIFO にためる byte 数 [P]
        ASSERT  SFX_STAGE+SFX_BYTES+4<=$10000

; 音ごとに：sfx.bin の中の offset、byte 数、ADPCM の CTRL（S1・S2）、強さ（強い音は弱い音に消されない）
sfx_table:
        DC.W    SFX0_OFF,SFX0_LEN,SFX0_CTRL,4   ; 始まり
        DC.W    SFX1_OFF,SFX1_LEN,SFX1_CTRL,1   ; 拾う
        DC.W    SFX2_OFF,SFX2_LEN,SFX2_CTRL,2   ; 当たる
        DC.W    SFX3_OFF,SFX3_LEN,SFX3_CTRL,3   ; 終わり
        ASSERT  SFX_COUNT=4

; 起動時：ADPCM を RESET にし、ADPCM の byte をカートリッジから Staging RAM に写す。
sfx_init:
        MOVE.W  #ADPCM_CTL_RESET|ADPCM_CTL_8K,ADPCM_CTRL
        MOVE.W  #FAB_STOP,FABRIC_CH0+FAB_CTRL
        MOVE.W  #FAB_ST_W1C,FABRIC_CH0+FAB_STATUS
        LEA     sfx_data,A0
        LEA     STAGING+SFX_STAGE,A1
        MOVE.W  #(SFX_BYTES+3)/4-1,D0
.copy   MOVE.L  (A0)+,(A1)+
        DBRA    D0,.copy
        MOVE.W  #1<<PORT_ADPCM,FABRIC_PORT_RESET
        MOVE.W  #ADPCM_ST_UNDERRUN,ADPCM_STATUS
        CLR.W   sfx_state
        CLR.W   sfx_prio
        CLR.W   sfx_req
        RTS

; 音 D0.W（SFX_*）を鳴らす。鳴っている音か、待っている音のほうが強ければ何もしない。
; 実際に鳴らすのは次の sfx_update から（1〜2 フレームの後）。
sfx_play:
        MOVEM.L D0-D1/A0,-(SP)
        LEA     sfx_table(PC),A0
        MOVE.W  D0,D1
        LSL.W   #3,D1
        MOVE.W  6(A0,D1.W),D1           ; 強さ
        TST.W   sfx_req
        BEQ.S   .noreq
        CMP.W   sfx_req_prio,D1
        BLO.S   .ret
        BRA.S   .take
.noreq  TST.W   sfx_state
        BEQ.S   .take
        CMP.W   sfx_prio,D1
        BLO.S   .ret
.take   ADDQ.W  #1,D0
        MOVE.W  D0,sfx_req
        MOVE.W  D1,sfx_req_prio
.ret    MOVEM.L (SP)+,D0-D1/A0
        RTS

; 1 フレームに 1 回。
sfx_update:
        MOVEM.L D0-D2/A0-A1,-(SP)
        LEA     FABRIC_CH0,A0
        MOVE.W  sfx_req,D0
        BEQ.S   .noreq
        CLR.W   sfx_req
        SUBQ.W  #1,D0
        MOVE.W  D0,sfx_cur
        MOVE.W  sfx_req_prio,sfx_prio
        MOVE.W  sfx_state,D0
        CMP.W   #SFX_FLUSH,D0
        BEQ     .ret                    ; 次のフレームで RESET（S1・S2 は新しい音のもの）
        CMP.W   #SFX_PLAYING,D0
        BNE.S   .arm
        ; ---- 0. 鳴っている音を止める：チャネル 0 を止め、FIFO を空にする。bridge は 0x08 を置きはじめる
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #1<<PORT_ADPCM,FABRIC_PORT_RESET
        MOVE.W  #SFX_FLUSH,sfx_state
        BRA     .ret
.noreq  MOVE.W  sfx_state,D0
        BEQ     .ret
        CMP.W   #SFX_FLUSH,D0
        BNE.S   .notfl
        ; ---- 1. RESET をかける（IDLE・ARMED からの求めはここから）。S1・S2 は次の音のもの。FIFO を空にする
.arm    BSR     sfx_entry
        MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  4(A1),D1
        ORI.W   #ADPCM_CTL_RESET,D1
        MOVE.W  D1,ADPCM_CTRL
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #1<<PORT_ADPCM,FABRIC_PORT_RESET
        MOVE.W  #SFX_ARMED,sfx_state
        BRA     .ret
.notfl  CMP.W   #SFX_ARMED,D0
        BNE     .playing
        ; ---- 2. 始める：RESET を 1 フレーム保った。チャネル 0 を設定して START
        BSR     sfx_entry
        MOVE.W  #PORT_STAGING,FAB_SRC(A0)
        MOVE.W  #PORT_ADPCM,FAB_DST(A0)
        MOVE.W  (A1),D1
        ADDI.W  #SFX_STAGE,D1
        MOVE.W  D1,FAB_SRC_ADDR_L(A0)   ; Staging RAM の byte 番地
        MOVE.W  #0,FAB_DST_ADDR_L(A0)   ; port 5 は逐次（番地は使わない）
        MOVE.W  2(A1),FAB_COUNT(A0)     ; byte 数
        MOVE.W  #1,FAB_SRC_STEP(A0)
        MOVE.W  #0,FAB_DST_STEP(A0)
        MOVE.W  #FMT_8_8,FAB_FORMAT(A0)
        MOVE.W  #$FFFF,FAB_MASK(A0)
        MOVE.W  #0,FAB_XOR(A0)
        MOVE.W  #0,FAB_SHIFT(A0)
        MOVE.W  #TRG_NOW,FAB_TRIGGER(A0)        ; 即時、ブロック、LOOP なし、割込みなし
        MOVE.W  #FAB_START,FAB_CTRL(A0)
        ; FIFO に SFX_PRIME byte たまるのを待つ（1 byte は約 1 µs。上限あり）
        MOVE.W  #400,D2
.prime  MOVE.W  ADPCM_STATUS,D1
        ANDI.W  #ADPCM_ST_COUNT,D1
        CMP.W   #SFX_PRIME,D1
        DBHS    D2,.prime
        ; RESET を外す。S1・S2 はこの音の標本化
        MOVE.W  #ADPCM_ST_UNDERRUN,ADPCM_STATUS
        MOVE.W  4(A1),ADPCM_CTRL
        MOVE.W  #SFX_PLAYING,sfx_state
        ADDQ.W  #1,sfx_plays
        BRA.S   .ret
.playing
        ; ---- 3. 終わりを待つ：DONE（全部を FIFO に書いた）の後、FIFO が空になったら止める
        MOVE.W  FAB_STATUS(A0),D1
        BTST    #3,D1                   ; ERROR
        BNE.S   .stop
        BTST    #1,D1                   ; DONE
        BNE.S   .drain
        MOVE.W  ADPCM_STATUS,D1         ; 送っている間に FIFO が空になったか（診断）
        BTST    #8,D1
        BEQ.S   .ret
        ADDQ.W  #1,sfx_underruns
        MOVE.W  #ADPCM_ST_UNDERRUN,ADPCM_STATUS
        BRA.S   .ret
.drain  MOVE.W  ADPCM_STATUS,D1
        ANDI.W  #ADPCM_ST_COUNT,D1
        BNE.S   .ret
.stop   MOVE.W  #FAB_STOP,FAB_CTRL(A0)
        MOVE.W  #FAB_ST_W1C,FAB_STATUS(A0)
        MOVE.W  #ADPCM_CTL_RESET|ADPCM_CTL_8K,ADPCM_CTRL
        MOVE.W  #ADPCM_ST_UNDERRUN,ADPCM_STATUS
        CLR.W   sfx_state
        CLR.W   sfx_prio
.ret    MOVEM.L (SP)+,D0-D2/A0-A1
        RTS

; A1 ＝ sfx_table の sfx_cur の行。D1 を壊す。
sfx_entry:
        MOVE.W  sfx_cur,D1
        LSL.W   #3,D1
        LEA     sfx_table(PC),A1
        ADDA.W  D1,A1
        RTS
