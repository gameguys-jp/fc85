; dsp.inc：DSP（TMS32010）の側から見た本機の I/O。dspasm 用。68000 の側の番地は fc85.inc。
;
; 台帳 docs/parts/TMS32010.md §2.1・§2.2、仕様書 §05・§07、DR-017・DR-029・DR-049・DR-053 から手で書き起こした。
; 仕様書が正で、ここは写し。記述ステータスはすべて [P]（本機の暫定設計値）。
;
; 規約（DR-017、DR-029）：
; - DSP は待てない。空の入力 FIFO を IN で読むと前の語が返り（UNDERRUN）、満杯の出力 FIFO への OUT は捨てられる
;   （OVERRUN）。IN・OUT の前に、状態ポートか BIO で確かめる。
; - TBLW で 0〜7 番地に書くと OUT と同じになる（プログラム RAM の /WE は 8〜4095 番地だけ）。SDK の規約で禁じる。
; - 0 番地がリセット、2 番地が INT の飛び先（台帳 §4.4）。INT はメールボックスの着信の間 Low。

; ------------------------------------------------------------------ I/O ポート（PA。台帳 §2.2、DR-053）

DSP_PA_INPUT    EQU     0               ; IN：入力 FIFO（Data Fabric の port 3）から 1 語取り出す
DSP_PA_OUTPUT   EQU     1               ; OUT：出力 FIFO（Data Fabric の port 4）へ 1 語入れる
DSP_PA_STATUS   EQU     2               ; IN：状態（下のビット）。読むと拍のラッチが消える（DR-049）
DSP_PA_RESPONSE EQU     2               ; OUT：68000 への応答（bridge の RESPONSE。IRQ_EN ならレベル 2）
DSP_PA_MAILBOX  EQU     3               ; IN：68000 からのメールボックス。読むと COMMAND（と INT）が下がる

; 状態ポート（IN 2）のビット（DR-053）
DSP_IS_INPUT    EQU     $0001           ; 入力 FIFO に語がある
DSP_IS_SPACE    EQU     $0002           ; 出力 FIFO に空きがある
DSP_IS_COMMAND  EQU     $0004           ; メールボックスに着信（まだ読んでいない）
DSP_IS_RESPONSE EQU     $0008           ; 68000 が応答をまだ読んでいない
DSP_IS_UNDERRUN EQU     $0010           ; 空の入力 FIFO を読んだ（消すのは 68000）
DSP_IS_OVERRUN  EQU     $0020           ; 満杯の出力 FIFO に書いた（消すのは 68000）
DSP_IS_TICK     EQU     $0040           ; 音声クロックの拍のラッチ（読むと消える。DR-049）
DSP_IS_COUNT_SH EQU     8               ; bit 15–8：入力 FIFO の語数

; BIO（BIOZ が分岐する条件）は、68000 が bridge の CTRL の BIOSEL で選ぶ（DR-029 (3)）：
; 0 入力 FIFO が空でない（既定）、1 出力 FIFO に空き、2 メールボックスに着信、3 拍のラッチ。

; ------------------------------------------------------------------ メモリ

DSP_PROG_WORDS  EQU     4096            ; 外部プログラム RAM（4K×16）
DSP_TBLW_MIN    EQU     8               ; TBLW で書ける最小の番地（DR-029 (2)）
DSP_DATA_WORDS  EQU     144             ; 内部データ RAM（ページ 0：0〜127、ページ 1：128〜143）
DSP_VEC_RESET   EQU     0
DSP_VEC_INT     EQU     2
