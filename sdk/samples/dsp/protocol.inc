; protocol.inc：SAMPLE 2 の 68000 と DSP の約束。m68kasm（main.s）と dspasm（effects.dsp）の両方が INCLUDE する。
; 値はすべてこのサンプルの [P]（エージェントが決めた。要見直し）。説明は README.md §05。
;
; メールボックス（68000 → DSP）の語：上位バイトがコマンド、下位バイトが引数。DSP は 1 つのコマンドに必ず 1 回
; 応答（OUT 2 → bridge の RESPONSE）を返す。68000 は応答を読んでから次のコマンドを書く。

CMD_MODE        EQU     $01             ; 引数＝モード（MODE_*）。応答＝新しいモードで処理する最初の標本の番号（下位 16 bit）
CMD_SNAP        EQU     $02             ; 標本の数・検査和・山を写しに取り、山を 0 に戻す。応答＝写した数の下位 16 bit
CMD_READ        EQU     $03             ; 引数＝番号（RD_*）。応答＝その値
RESP_ERROR      EQU     $FFFF           ; 知らないコマンド、範囲の外の引数

MODE_THRU       EQU     0               ; そのまま
MODE_LPF        EQU     1               ; 31 タップの FIR の低域通過
MODE_ECHO       EQU     2               ; 192 ms の帰還のエコー
MODE_COUNT      EQU     3

RD_COUNT_HI     EQU     0               ; 写した標本の数（上位 16 bit）
RD_COUNT_LO     EQU     1               ; 　　　　　　　（下位 16 bit）
RD_SUM          EQU     2               ; 写したときの検査和（出力の語の和の下位 16 bit）
RD_PEAK         EQU     3               ; 前の CMD_SNAP からの |y| の最大
RD_MODE         EQU     4               ; 今のモード
RD_MAGIC        EQU     5               ; DSP_MAGIC（プログラムが動いている印）
RD_COUNT        EQU     6

DSP_MAGIC       EQU     $3210

; エコーの遅延線（プログラム RAM の後ろ）。長さ RING_END−RING_START 標本
RING_START      EQU     $0400
RING_END        EQU     $1000
