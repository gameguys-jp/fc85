; protocol.inc：SAMPLE 6 の 68000 と DSP の約束。m68kasm（main.s）と dspasm（xform.dsp）の両方が INCLUDE する。
; 値はすべてこのサンプルの [P]（エージェントが決めた。要見直し）。説明は README.md §04。
;
; 1 フレームに 1 回、68000 が Staging RAM に入力の塊を置き、Data Fabric のチャネル 0 で DSP の入力 FIFO
; （port 3）へ送る。DSP は頂点を回して透視し、画面の座標を出力 FIFO（port 4）へ書く。チャネル 1 がそれを
; Staging RAM へ運ぶ。メールボックスは使わない。
;
; 入力（16 bit の語）：HEAD、角 A（X 軸）、角 B（Y 軸）、角 C（Z 軸）、頂点の数 N、続けて x・y・z を N 組
;   HEAD＝HEAD_MAGIC<<8｜番号（0〜255）。角は 256 で 1 回り。座標は整数（立方体は ±CUBE_HALF）
; 出力：X・Y（ACRTC の論理座標。X は右、Y は上が正で、基本画面の頁の左上が 0,0）を N 組、最後に
;   TAIL＝TAIL_MAGIC<<8｜番号
; 計算（どちらも 16 bit×16 bit の積の和を 15 bit 右へ。端数は負の向きに切り捨てる）：
;   X 軸：y1＝(y·cA − z·sA)>>15、z1＝(y·sA ＋ z·cA)>>15
;   Y 軸：x2＝(x·cB ＋ z1·sB)>>15、z2＝(z1·cB − x·sB)>>15
;   Z 軸：x3＝(x2·cC − y1·sC)>>15、y3＝(x2·sC ＋ y1·cC)>>15
;   透視：k＝PERSP[(z2＋128)&255]（Q15）、X＝((x3·k)>>15)＋CX、Y＝((y3·k)>>15)−CY
; sA・cA は正弦の表 SIN[A]・SIN[(A＋64)&255]（Q15、±32767）。68000 にも同じ式の版があり（A で切替え）、
; 起動時に両方の結果を突き合わせる。

HEAD_MAGIC      EQU     $53
TAIL_MAGIC      EQU     $AC
IN_HEAD_WORDS   EQU     5               ; HEAD、A、B、C、N
MAX_VERTS       EQU     16
CX              EQU     128             ; 画面の中心（論理座標）
CY              EQU     48              ; 空の 96 行の真ん中。Y＝py−CY
