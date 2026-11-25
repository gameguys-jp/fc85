; bios.inc：Boot ROM（BIOS）の ABI。m68kasm 用。説明は sdk/docs/bios-abi.md。
;
; ここにある番地と値は、BIOS の版 1 の暫定の約束 [P]。仕様書（docs/spec/hardware.md）の DR-025（ジャンプ表）の
; ほかは、仕様書に書かれていない。決めるときは DR にする。

BIOS_VERSION    EQU     $0001

; ------------------------------------------------------------------ 呼出しの入口（ROM の固定番地）
; 各項目は JMP abs.L（6 byte）。JSR で呼ぶ。出力と書いたレジスタのほかは保つ。スーパバイザで呼ぶ。

BIOS_WAIT_VBL   EQU     $000400         ; 次の VBlank まで待つ（STOP。戻ると SR は元に戻る）
BIOS_ACRTC_SEND EQU     $000406         ; (A0) から D1.W 語を ACRTC の FIFO に送る。出力：A0 は次の語
BIOS_CLS        EQU     $00040C         ; 画面（256×192）を色 D0.W で消す
BIOS_PUTS       EQU     $000412         ; (A0) の文字列（JIS X 0201、0 で終わる、32 字まで）を x＝D0.W、y＝D1.W
                                        ; （画面の左上が 0,0、x は 4 の倍数）に、前景 D2.W・背景 D3.W で描く
BIOS_PUT_HEX    EQU     $000418         ; D4.L を D5.W 桁（1〜8）の 16 進で、BIOS_PUTS と同じ D0〜D3 で描く
BIOS_SET_PAL    EQU     $00041E         ; (A0) の 16 語（$0RGB）をパレットに書く
BIOS_READ_PAD   EQU     $000424         ; 出力：D0.W＝PORT_A、D1.W＝前の呼出しから新しく押したボタン
BIOS_INFO       EQU     $000480         ; 版の情報（下）

; BIOS_INFO の中身
BIOS_INFO_VERSION EQU   BIOS_INFO+0     ; 語：BIOS_VERSION

; ------------------------------------------------------------------ Main RAM の使い方
; $100000–$1005FF  例外のジャンプ表（DR-025）。ベクタ n は JUMP_TABLE+6n。中身は JMP abs.L（$4EF9＋番地）
; $100600–$1007FF  BIOS の変数（下）。カートリッジも読んでよい。書くのは「書いてよい」としたものだけ
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
bios_spurious   RS.W    1               ; スプリアス割込みとレベル 7 の数
bios_pad_prev   RS.W    1               ; BIOS_READ_PAD の前の値
bios_exc_vector RS.W    1               ; 致命的な例外：ベクタ番号
bios_exc_sr     RS.W    1               ; 　SR
bios_exc_pc     RS.L    1               ; 　PC
bios_exc_addr   RS.L    1               ; 　アクセスの番地（バスエラーとアドレスエラー）
bios_exc_busy   RS.W    1               ; 致命的な例外の処理中
bios_probe_sp   RS.L    1               ; カートリッジを調べる間の SP
bios_hexbuf     RS.B    10
bios_vars_used  RS.B    0               ; ここまでが版 1 の BIOS の変数（後ろは BIOS の内部）

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
