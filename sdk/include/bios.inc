; bios.inc：Boot ROM（BIOS）の ABI。m68kasm 用。説明は sdk/docs/bios-abi.md。
;
; ここにある番地と値は、BIOS の版 2 の暫定の約束 [P]。仕様書（docs/spec/hardware.md）の DR-025（ジャンプ表）の
; ほかは、仕様書に書かれていない。決めるときは DR にする（DR-057、DR-094）。
;
; 版（DR-094）：BIOS_INFO_VERSION の語。版 n の約束は、版 n＋1 以降もすべて守る（足すだけ）。版 2 で足した
; 呼出し（下の「版 2」）を使うカートリッジは、使う前に CMPI.W #2,BIOS_INFO_VERSION で確かめる。版 1 の ROM では
; その番地は消去したままの $FFFF で、呼ぶと F ライン例外で止まる。

BIOS_VERSION    EQU     $0002

; ------------------------------------------------------------------ 呼出しの入口（ROM の固定番地）
; 各項目は JMP abs.L（6 byte）。JSR で呼ぶ。スーパバイザで呼ぶ。出力と書いたレジスタのほかは保つ（USP も）。
; 割込みのマスクは変えない。CCR は、Z を出力と書いた呼出しのほかは不定。ACRTC を使う呼出しは再入できない。

BIOS_WAIT_VBL   EQU     $000400         ; 次の VBlank まで待つ（STOP。戻ると SR は元に戻る）
BIOS_ACRTC_SEND EQU     $000406         ; (A0) から D1.W 語（0〜65535）を ACRTC の FIFO に送る。出力：A0 は次の語、
                                        ; Z＝1 なら打ち切らなかった、Z＝0 なら打ち切った（版 2）
BIOS_CLS        EQU     $00040C         ; 画面（256×192）を色 D0.W で消す。出力：Z（版 2）
BIOS_PUTS       EQU     $000412         ; (A0) の文字列（JIS X 0201、0 で終わる、32 字まで）を x＝D0.W、y＝D1.W
                                        ; （画面の左上が 0,0、x は 4 の倍数）に、前景 D2.W・背景 D3.W で描く。出力：Z（版 2）
BIOS_PUT_HEX    EQU     $000418         ; D4.L を D5.W 桁（1〜8）の 16 進で、BIOS_PUTS と同じ D0〜D3 で描く。出力：Z（版 2）
BIOS_SET_PAL    EQU     $00041E         ; (A0) の 16 語（$0RGB）をパレットに書く
BIOS_READ_PAD   EQU     $000424         ; 出力：D0.W＝PORT_A、D1.W＝前の呼出しから新しく押したボタン
; 版 2
BIOS_PUTS_KUTEN EQU     $00042A         ; (A0) の語の列（$00xx は JIS X 0201、$KKTT は区 KK・点 TT、0 で終わる、
                                        ; 32 字まで）を BIOS_PUTS と同じ D0〜D3 で描く。ROM にない区点は〓。出力：Z
BIOS_GLYPH      EQU     $000430         ; D0.W の符号（BIOS_PUTS_KUTEN と同じ）→ A0＝字形の 1 行目の CPU 番地
                                        ; （行は +2 ずつ）。Z＝1 なら ROM にある、Z＝0 なら〓に替えた
BIOS_CALLS_END  EQU     $000436         ; 版 2 の呼出しの終わり（ここから BIOS_INFO の前までは予約）
BIOS_INFO       EQU     $000480         ; 版の情報（下）

; BIOS_INFO の中身。$000482〜$0004FF は予約（読んで頼らない）
BIOS_INFO_VERSION EQU   BIOS_INFO+0     ; 語：BIOS_VERSION（約束の版）

; ------------------------------------------------------------------ Main RAM の使い方
; $100000–$1005FF  例外のジャンプ表（DR-025）。ベクタ n は JUMP_TABLE+6n。中身は JMP abs.L（$4EF9＋番地）
; $100600–$1007FF  BIOS の変数（下）。カートリッジも読んでよいが、約束するのは「内部」の印のないもの
;                  （bios-abi.md §03 の表）だけ。書くのは「書いてよい」としたものだけ
; $100800–$11FFFF  アプリケーション。スタックは $120000 から下へ（初期の SSP）

BIOS_VARS       EQU     $100600
BIOS_VARS_END   EQU     $100800
APP_RAM         EQU     $100800
INITIAL_SSP     EQU     $120000

        RSSET   BIOS_VARS
bios_frame      RS.L    1               ; VBlank の数（VBlank の割込みが 1 足す）
bios_pad_a      RS.W    1               ; VBlank ごとに読んだ PORT_A
bios_pad_b      RS.W    1               ; VBlank ごとに読んだ PORT_B
bios_modules    RS.W    1               ; 起動時の FABRIC_MODULES
bios_vbl_hook   RS.L    1               ; 0 でなければ、VBlank の割込みが JSR する（書いてよい）。D0–D1/A0–A1 は壊してよい
bios_input_events RS.W  1               ; 入力の割込み（レベル 1）の数
bios_input_status RS.W  1               ; 入力の割込みで消した INPUT_IRQ_STATUS のビット（OR で貯める。書いてよい）
bios_midi_head  RS.W    1               ; MIDI の受信のリング（64 byte）。head は書く位置（割込みが進める）
bios_midi_tail  RS.W    1               ; tail は読む位置（アプリが進める。書いてよい）
bios_midi_buf   RS.B    64
bios_midi_lost  RS.W    1               ; リングが満杯で捨てた数
bios_spurious   RS.W    1               ; スプリアス割込み、レベル 7、既定の処理が受けた DOC の割込み（レベル 4）の数
bios_pad_prev   RS.W    1               ; 内部：BIOS_READ_PAD の前の値（版で変わる。頼らない）
bios_exc_vector RS.W    1               ; 致命的な例外：ベクタ番号
bios_exc_sr     RS.W    1               ; 　SR
bios_exc_pc     RS.L    1               ; 　PC
bios_exc_addr   RS.L    1               ; 　アクセスの番地（バスエラーとアドレスエラー）
bios_exc_busy   RS.W    1               ; 内部：致命的な例外の処理中
bios_probe_sp   RS.L    1               ; 内部：カートリッジを調べる間の SP
bios_hexbuf     RS.B    10              ; 内部：16 進の字の作業場所
; 版 2
bios_acrtc_aborts RS.W  1               ; BIOS が ACRTC のコマンドを打ち切った数（DR-080、DR-082。16 bit で一巡する）
bios_acrtc_abort_sr RS.W 1              ; 最後に打ち切る前に読んだ ACRTC の SR（下位 byte。CER が立っていれば誤り、
                                        ; でなければ空きを待って時間切れ）
bios_vars_used  RS.B    0               ; ここまでが版 2 の BIOS の変数（後ろは BIOS の内部）

; ------------------------------------------------------------------ カートリッジの見出し [P]
; $400000 に置く。BIOS は CART_MAGIC と版を確かめ、入口が窓の中の偶数番地なら飛ぶ。

CART_MAGIC      EQU     $43415254       ; 'CART'
CART_HDR_VERSION EQU    $0001
        RSSET   0
cart_magic      RS.L    1               ; 'CART'
cart_version    RS.W    1               ; 見出しの版（1）
cart_flags      RS.W    1               ; 予約（0）
cart_entry      RS.L    1               ; 入口の絶対番地（$400000〜$4FFFFF、偶数）
cart_size       RS.L    1               ; ROM の大きさ（byte、参考）
cart_title      RS.B    16              ; 題名（JIS X 0201、空白で埋める。BIOS は使わない）
cart_header_size RS.B   0               ; 32 byte
