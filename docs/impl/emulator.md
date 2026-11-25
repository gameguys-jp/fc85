# エミュレータ

**v0.3**  
2026年11月25日

> 状態：作業中の版。機械仕様は [仕様書](../spec/hardware.md) が正。この文書は実装の設計と、仕様との差を書く。決定記録は [DR-040](../decisions.md)。TMS32010 のサイクル数は [DR-032](../decisions.md)。

## 01　概要

| 項目 | 内容 |
|---|---|
| M（実装媒体） | M1（PC 上のソフト） |
| S（再現忠実度、側面ごと） | chips 層は S1・S2・S3（外部ピンで見える動作、内部状態、IC のクロック単位の時刻）。S4（DAC の電圧、LPF、アナログ経路）は後の層で足す |
| V（検証、現状→目標） | MSM5205、TMS32010、ES5503 のコアは V2（単体試験）。全体は V0→V2、その後 V3（独立実装との比較） |
| 参照する仕様 | [仕様書](../spec/hardware.md) §12（段階と合格条件）、IC ごとの台帳（[docs/parts/](../parts/)） |
| 言語 | Rust（edition 2024）、cargo の workspace（DR-040） |
| コード | [emulator/](../../emulator/) |

## 02　構成

| 層（仕様書 §12） | crate | 状況 |
|---|---|---|
| core | 未作成 | 段階 2 で作る（クロック領域ごとの時刻、イベント、trace、snapshot） |
| chips | `fc85-chips`（`emulator/crates/chips`） | MSM5205、TMS32010、ES5503 |
| board | 未作成 | 段階 2（Data Fabric、ポートアダプタ）から |
| host | 未作成 | 表示・音声・操作 UI のライブラリは未定（SDL3、egui が候補） |

### 約束

- IC のコアは、その IC のクロック 1 つを `tick` の 1 回とする。TMS32010 は命令サイクル（CLKOUT の 1 周期＝CLKIN の 4 周期）、ES5503 は DOC サイクル（E の 1 周期＝CLK の 8 周期）を 1 回とする。外部ピンへの入力は `set_*`、出力は `tick` の戻り値と読み出しの関数で返す。クロック領域どうしの合わせ方は core の仕事で、コアは知らない。
- 状態はすべて serde で保存・復元できる（snapshot、Bend のパッチの undo）。
- MAME など既存の実装のコードは写さない。台帳に書き起こした式とタイミングから書く。V3（独立実装との比較）で、比べる相手と独立であるため。
- 台帳で [V] の動作は、どう扱ったかをコードの注釈と §03 に書く。比べたい動作（例：回り込みとクリップ）は設定で切り替えられるようにする。

### コマンド

`emulator/` で実行する。

| 目的 | コマンド |
|---|---|
| 全部の試験 | `cargo test` |
| 1 件だけ | `cargo test -p fc85-chips --test <IC の名前> <試験の名前>`（例：`--test tms32010`） |
| 書式 | `cargo fmt --all` |
| lint | `cargo clippy --all-targets -- -D warnings` |

### IC ごとの状況

| IC | 段階 1 の範囲（仕様書 §12） | 状況 |
|---|---|---|
| MSM5205 | ADPCM の既知列（DR-021 の列を含む） | 実装した。試験 12 件 |
| HD63484 | 描画・COPY・FIFO | 未着手 |
| TMS32010 | 乗算・FIR・外部 I/O。サイクル数は TI の資料どおり | 実装した。試験 33 件 |
| ES5503 | 発音と停止 | 実装した。試験 20 件 |
| HD68000、HD6850 | 段階 1 の対象外 | 段階 3 で |

## 03　仕様との差

### MSM5205

コードは `emulator/crates/chips/src/msm5205.rs`。節の番号は [台帳](../parts/MSM5205.md) のもの。

| 仕様の項目 | 実装での扱い | 理由 | 影響 |
|---|---|---|---|
| オーバーフロー（§4.4） | 12 bit の 2 の補数で回り込むのを標準にし、クリップ（MAME と同じ）を設定で選べる | データシートは回り込み、MAME はクリップ。実 IC の測定はない [V] | Image as ADPCM、字形の ADPCM 化の音 |
| RESET の最小幅（§3.3：V̄C̄K̄ 2 周期以上） | 取込みの時点で RESET が H なら、予測値とステップ番号を 0 にする。最小幅は見ない | 短いパルスのときの動作が資料にない [V] | 短い RESET を使う Bend で、実 IC と違いうる |
| DA OUT の遅れ（§3.2：約 1 周期） | 取込みと同時に予測値を更新する。遅れは扱わない | V2 の比較は取込み列で行う（§6 の注意 5） | 音が 1 サンプル早く出る。アナログの層で足す |
| DAC の精度と LPF（§3.5、§6 の注意 4） | 10 bit のコード（予測値の下位 2 bit を捨てた値）だけを出す | 電圧はアナログの層の仕事 | — |
| 3-bit モード（§4.5） | D0 を 0 にして、4-bit の表に通す（MAME と同じ） | ほかに根拠がない。OKI の数値と合わない可能性がある [V] | 本機は 4-bit が標準 [P] |
| S1＝S2＝H（§4.5） | 内部の分周を止め、V̄C̄K̄ を入力にする（MAME の解釈） | データシートでは禁止 | Bend モードだけで使う [P] |
| 電源投入時の状態 | 予測値とステップ番号は 0、V̄C̄K̄ は H、8 kHz | 資料にない [V] | 使う前に RESET をかける |
| V̄C̄K̄ の位相 | 周期の前半を H、後半を L にする。立下りから 6 クロック後に取り込む | §3.2 | — |
| S1・S2 を周期の途中で変える | 周期の中の位置を保つ。新しい周期を超えていれば、次の周期の頭から数え直す | 実 IC の動作は未確認 [V] | 分周を切り替える Bend |

### TMS32010

コードは `emulator/crates/chips/src/tms32010.rs`。節の番号は [台帳](../parts/TMS32010.md) のもの。命令の意味は TI の資料（台帳の T1・T3）に従った。MAME（台帳の M1）とは照合だけをした。

プログラムメモリ、I/O ポート、BIO ピンは `Bus` トレイトの向こう側に置き、board 層（または試験）が実装する。

| 仕様の項目 | 実装での扱い | 理由 | 影響 |
|---|---|---|---|
| 時間の単位（§3.1） | `tick` の 1 回を 1 命令サイクル（20 MHz の水晶で 200 ns）にする。CLKIN の 4 分周は board 層でする | V2 の比較はサイクル単位で足りる。サイクルの中のストローブの位相（§3.2）は、外から見て効くところが少ない | MEN・DEN・WE のサイクルの中の時刻は出さない。ストローブの幅を使う Bend は、board 層で近似する |
| 命令の先読み（§3.3、§3.4） | フェッチと実行を同じサイクルに置く。サイクル数は §3.7 の表どおり。IN・OUT は 1 サイクル目に、TBLR・TBLW は 2 サイクル目にバスを使う | 外から見えるのは、サイクル数とバスの読み書きの順 | — |
| 条件分岐のサイクル数（追補 A） | 成否によらず 2 | TI の資料を正とした（DR-032） | MAME（不成立は 1）と比べるとき（V3）は既知の差 |
| 割込みの受付（§3.6、追補 A） | 1 ダミーサイクルで PC を積み、2 番地へ移る。次のサイクルで 2 番地の命令を実行する。受け付けない条件は複数サイクル命令の途中と、MPY・MPYK・EINT の次の命令 | TI の図2-12（DR-032） | MAME（3 サイクル）と比べるときは既知の差 |
| INT の入力（§3.6） | 立下りを INTF に保ち、Low の間も要求にする。DINT の間も INTF は残る | §3.6 | ハンドラから戻るとき INT が Low のままなら、すぐ次の割込みになる |
| BIO のサンプル（追補 A） | BIOZ の 1 サイクル目に読む [P] | 資料にない [V]。DR-032 の案 | 外で同期させたとき、実チップとの差は最大 1 サイクル（推定） |
| リセット（§3.5） | PC＝0、INTM＝1、INTF＝0。OVM は変えない。ほかのレジスタとデータ RAM は変えない。電源投入時はすべて 0 | 資料にない [V] | ソフトは §4.4 の初期化をする前提。リセットの前の値を使う Bend は、実チップと違いうる |
| RS の最小幅（§3.5：5 サイクル以上） | 見ない。RS を外した 1 サイクル後に 0 番地から実行する | 短いパルスの動作が資料にない [V] | 短い RS を使う Bend |
| 144〜255 番地（§4.2、§7 の 6） | 読むと 0、書込みは捨てる | 資料にない [V] | 範囲外を指す間接アドレスの Bend |
| DMOV・LTD の番地の境界（§3.8、§7 の 6） | 127 番地からは 128 番地へ写す。143 番地からは写さない | 資料にない [V] | 遅延線をページの境界に置くソフト |
| スタックの POP（§4.1） | 最上段を取り出し、最下段の値はそのまま残す（MAME と同じ） | 資料にない [V] | 5 回目以降の POP・RET の値 |
| SUBC・ABS の OV | 変えない。ABS の 0x80000000 は OVM＝1 なら 0x7FFFFFFF（§4.3） | T1 の各命令の頁は OV に触れない。MAME は SUBC で OV を立てる [V] | BV の分岐 |
| 加減算の OV（ADD 系、SUB 系、APAC、SPAC、LTA、LTD） | 32 bit の符号付きのあふれで OV＝1。OVM＝1 なら最大・最小の値にする。OV は BV か LST でしか消えない | T1 §2.2.1.1、§2.2.2.1 | — |
| SAR・SST を間接で使い、指している AR を増減するとき | 増減の前の値を書く | 資料にない [V]。MAME は増減の後の値を書く（SAR） | AR を自分の指す先へ書く Bend |
| SST の間接での次の ARP | 指定どおりに変える | T1 の書式（`SST *,<ARP>`）。MAME は変えない | — |
| SST の番地 | 直接は DP によらずページ 1（§4.1）。間接は AR の下位 8 bit | 間接の番地の資料はない [V] | — |
| LAR・LST を間接で使い、同じ AR・ARP を変えるとき | 読んだ値が勝つ | 資料にない [V]。MAME と同じ | — |
| SACH のシフト（§4.3：0・1・4） | 2・3・5・6・7 も同じ式（左シフトして上位 16 bit）で扱う | 未定義の符号 [V] | Bend だけで使う |
| 未定義の命令 | 1 サイクルの NOP として実行し、回数を数える | 資料にない [V] | — |
| OUT と 0〜7 番地への TBLW（§2.2） | コアは別の関数（`port_out`、`write_program`）で呼ぶ。区別できないことは board 層で再現する | 外部ピンでは同じ WE と番地 | — |
| データ RAM への直接の書込み（`set_data`） | 試験と Bend の道具。実機にはない | エミュレータだけの内部介入（仕様書 §11） | — |

### ES5503

コードは `emulator/crates/chips/src/es5503.rs`。節の番号は [台帳](../parts/ES5503.md) のもの。動作は Ensoniq のデータシート（台帳の E2）と Apple の ERS（E1）に従い、資料にない細部は台帳に書き起こした MAME（E10）の動作を使った。MAME の細部は IIGS（1986年以降の個体）での測定に基づくので、1985年の個体と同じかは分からない（台帳 §7 の 4）。

波形 RAM は `WaveBus` トレイトの向こう側に置き、board 層（または試験）が実装する。

| 仕様の項目 | 実装での扱い | 理由 | 影響 |
|---|---|---|---|
| 時間の単位（§3.2） | `tick` の 1 回を 1 DOC サイクル（E の 1 周期＝CLK の 8 周期、8 MHz 入力で 1 µs）にする | 1 サイクルに 1 オシレータを訪れ、ホストの窓も 1 回ある。サイクルの中の CLK の位相は扱わない | RAS・CAS・E・Q のサイクルの中の時刻は出さない |
| ホストのアクセス（§3.3） | `read`・`write` は、E High の窓での 1 回のアクセス | 1 サイクル 1 回の制限と、波形 RAM との取り合いは board 層（ブリッジ）でする | — |
| 出力（§3.6、§4.5） | サイクルごとに SIG±（符号付きの波形値×音量）、CA3–CA0、CSTRB を返す。チャンネルごとのサンプルホールドと加算は board 層でする | ピンで見える形にした。MAME はチャンネルごとに足し合わせる | — |
| 出力のパイプライン（§3.6、§7 の 8） | 訪れたサイクルに、そのオシレータの値を出す | 段数は資料にない [V] | 出力が実チップより 1 サイクル以上早い可能性 |
| リフレッシュの 2 サイクルの SIG± | 最後のオシレータの値を保つ。CSTRB は無効、CA は 0xF（E2 p.22） | MAME の「最上位のオシレータは 3 回分聞こえる」（§6 の注意 2）を、出力が 3 サイクル保たれる形と解釈した [V] | 3 回分に聞こえるかは board 層のアナログ経路で決まる。CSTRB でゲートするサンプルホールドなら 1 回分 |
| 止まっているオシレータ（§4.4） | 訪れて読む。SIG± は 0（中心） | 読むことは MAME（IIGS の測定）。E1・E2 にない [V] | — |
| 波形データのレジスタ（§4.1、§4.4） | 0 は入らない | MAME [V] | — |
| 0 で止まったオシレータ（§4.4） | 訪れるたびに 0 を見つけ、IE＝1 なら要求を出し直す | MAME [V] | フリーランで 0 に止めると、割込みが続く |
| M0＝1・H＝1 の巻き戻し（§4.2） | 止まっているのを見つけた訪問で、すぐアキュムレータを 0 にする | E1 p.8 の文のとおり。MAME は 1 訪問遅れて 0 にする（IIGS の測定）[V] | CPU が止めてすぐ動かしたときの開始位置 |
| 同期（§4.2） | 偶数番がテーブルの頭に戻ると、上の奇数番のアキュムレータを 0 にする。偶数番は端数を残す | E1 は奇数番を戻すとだけ書く。MAME は偶数番も 0 にする [V] | 偶数番の位相が端数だけずれる |
| AM（§4.2） | 奇数番は鳴らさず（SIG± は 0）、上の偶数番が動いていれば、その音量を波形値で上書きする | MAME と同じ。E1 は「CSTRB でゲートが要る」と書き、奇数番の枠に何かが出ることを示すが、何が出るかは分からない [V] | 奇数番の枠を聞く Bend は作れない |
| スワップ（§4.2） | 偶数番・奇数番とも、止まったら組の相手（番号の bit0 を反転）の停止を解く | E1 の「次のオシレータ」を組の相手と読んだ。MAME は、偶数番が相手を起こし、止まった奇数番は偶数番を動かし続ける（IIGS の測定）[V] | 交互に鳴る点は同じ。切り替わりのサイクルが違いうる。切り替わりの 0x80（§6 の注意 3）は扱わない |
| 割込み（§4.6） | E1 どおり：IE＝0 で完了した要求はテーブルに残り、IE を 1 にすると OIR へ渡る。OIR は先入れ先出し。制御レジスタに書いても要求は消えない | MAME（IIGS の測定）は、IE＝0 では要求を残さず、制御レジスタへの書込みで消し、走査の順に 1 件ずつ出す。提示と同じサイクルの読出しでは消えない [V] | V3 の比較で差になる |
| OIR に要求がないとき | bit7＝1、bit5–1 は最後の番号 | MAME と同じ [V] | — |
| リセット（§4.8） | 有効数を 1 にし、要求を消し、0xE1 に書くまで走査しない。オシレータのレジスタは変えない | E2 は「内部レジスタをリセット」とだけ書く。走査の停止は MAME の解釈 [V] | — |
| 電源投入（§4.8） | レジスタは全ビット 1 | MAME の解釈 [V] | — |
| レジスタの消失（§4.7、§4.8） | 走査で訪れないまま 1,500,000 サイクル（8 MHz 入力で約 1.5 秒）たつと全ビット 1 になる。書き込んだオシレータは、そのとき新しくなったとみなす。`set_decay` で止められる | E2 は「有効でないオシレータは値を保たない」。時間は MAME の解釈。書込みの扱いは近似 [V] | 有効数を減らしたオシレータの値が消える |
| 有効数の終わり（§6） | 等値比較。走査がすでに終点を過ぎていれば 31 まで回り、リフレッシュなしで 0 に戻る | MAME | 有効数を減らした直後の 1 周 |
| A/D（§3.7） | 読むと変換を始め、30 サイクル後に入力を取り込む。変換中に読むと前の値を返し、変換をやり直す | E2 は 30 µs、E1 は 26 サイクル。失われた値の中身は資料にない [V] | — |
| 存在しない番地（E3〜FF） | 読むと 0xFF。書込みは捨てる | MAME [V] | — |
| 音量（§4.5） | 単純な乗算 | E1 p.6。IIGS で測った非線形の特性はアナログの層で足す | — |
| BS（§2） | 番地の bit 16 に出す。本機の board 層はつながない | 仕様書 §05 [P] | — |

## 04　テストと判定基準

### MSM5205

試験は `emulator/crates/chips/tests/msm5205.rs`。期待値は台帳の計算から取る。

| テスト | 入力 | 判定基準 | 状況 |
|---|---|---|---|
| `step_table_matches_formula` | ステップ表の 49 段 | floor(16×1.1^i) と一致（§4.2） | 合格 |
| `default_overflow_is_wrap` | 初期状態 | 回り込みが標準 | 合格 |
| `rate_pins_follow_datasheet` | S1・S2 の 4 通り | 1/96、1/64、1/48、Slave（§3.1） | 合格 |
| `capture_is_six_clocks_after_vck_fall` | 3 つの分周で 3 周期 | 立下りは周期の中央、立上りは周期の頭、取込みは立下りの 6 クロック後（§3.2） | 合格 |
| `reset_clears_signal_and_step` | 途中で RESET | 予測値とステップ番号が 0 | 合格 |
| `dr021_wrap_vs_clip_sequence` | リセット直後に 0111 を 6 個（DR-021） | 回り込み：30, 93, 229, 522, 1153, −1586。クリップ：6 個目が 2047（追補 A.3） | 合格 |
| `holding_zero_wraps_after_1024_samples` | 0000 を 1024 個 | 1 個 2 LSB ずつ上がり、1024 個目で −2048 に回り込む（§4.6 (a)） | 合格 |
| `alternating_zero_and_minus_zero_stays_near_center` | 0000 と 1000 を交互に 1000 個 | 予測値が 0〜2 に留まる（§4.6 (b)） | 合格 |
| `three_bit_mode_ignores_d0` | 3-bit で D0 だけ違う列 | 同じ取込み列になる | 合格 |
| `dac_code_drops_two_low_bits` | 予測値 30 と −33 | DAC のコードが 7 と −9 | 合格 |
| `slave_mode_captures_after_external_vck_fall` | S1＝S2＝H で外から V̄C̄K̄ を下げる | 内部の分周は止まり、立下りの 6 クロック後に取り込む | 合格 |
| `snapshot_round_trip` | 周期の途中で保存して戻す | 同じ入力から同じ取込み列が出る | 合格 |

期待値はどれも台帳の式（MAME の実装を読んで書き起こしたもの）から計算した値で、実 IC の測定ではない。実 IC と合うかは V4 で確かめる（台帳 §7）。

### TMS32010

試験は `emulator/crates/chips/tests/tms32010.rs`。期待値は台帳と、そこに引いた TI の資料（T1）の命令の説明から取る。試験の中に小さなアセンブラ（命令の符号の定数）を置いた。

| 分野 | テスト | 判定基準 | 状況 |
|---|---|---|---|
| リセット | `reset_sets_pc_and_intm_but_keeps_ovm` | PC＝0、INTM＝1、OVM はそのまま。解除の 1 サイクル後に 0 番地を実行（§3.5） | 合格 |
| サイクル数 | `cycle_counts_follow_ti_table`、`branch_taken_or_not_is_two_cycles` | 1・2・3 サイクルの命令が §3.7 の表どおり。条件分岐は成否によらず 2（追補 A） | 合格 |
| 乗算 | `mpy_of_8000_squared_gives_c0000000`、`mpyk_takes_a_signed_13_bit_constant`、`pac_apac_spac_lta_use_the_product` | 0x8000×0x8000＝0xC0000000（§4.3）。MPYK は 13 bit の符号付き | 合格 |
| 加減算とあふれ | `overflow_sets_ov_and_ovm_saturates`、`ov_stays_set_until_bv`、`abs_of_80000000_depends_on_ovm` | OVM＝0 で回り込み、OVM＝1 で飽和。OV は BV で消える。ABS の特例（§4.3） | 合格 |
| シフトと論理 | `shifts_and_sign_extension`、`logic_ops_touch_only_the_low_half` | ADD・LAC の符号拡張、ADDS・ZALS・LACK の非拡張、SACH のシフト 0・1・4、AND は上位を 0 に | 合格 |
| 除算 | `sixteen_subc_divide` | 1000÷7 を SUBC 16 回で。下位に商 142、上位に余り 6 | 合格 |
| FIR | `three_tap_fir_with_ltd_and_mpy` | 3 タップ（LT・MPY・LTD・APAC）。0.5 のインパルスへの応答が 0x2000、0x1000、0x0800、0。1 標本 15 サイクル（§3.7 から計算） | 合格 |
| アドレス | `indirect_uses_low_8_bits_and_steps_in_9_bits`、`indirect_can_load_the_next_arp`、`banz_tests_9_bits_and_always_decrements`、`dp_selects_page_and_sst_always_uses_page_1` | 番地は AR の下位 8 bit、増減は下位 9 bit（§4.1、§4.2）。SST の直接はページ 1 | 合格 |
| 状態とスタック | `lst_restores_status_but_not_intm`、`stack_is_four_deep_and_loses_the_oldest`、`push_keeps_the_low_12_bits`、`call_and_ret`、`dmov_and_ltd_shift_the_delay_line` | 状態語の bit 配置（§4.1）。スタックは 4 段で、5 段目で最も古い値を失う | 合格 |
| I/O と表 | `in_and_out_use_the_port_field`、`tblr_and_tblw_use_acc_low_12_bits_and_one_stack_level`、`tblr_with_a_full_stack_loses_the_oldest`、`bioz_branches_when_bio_is_low` | ポートは上位バイトの 3 bit（§2.2）。表の番地は ACC の下位 12 bit で、スタックを 1 段使う（§3.4） | 合格 |
| 割込み | `interrupt_jumps_to_2_with_one_dummy_cycle`、`interrupt_waits_one_instruction_after_mpy`、`interrupt_waits_for_a_multi_cycle_instruction`、`int_edge_is_latched_while_disabled`、`int_held_low_requests_again` | 1 ダミーサイクルで 2 番地へ。MPY・EINT の次の命令と、複数サイクル命令の途中では受け付けない（§3.6、追補 A） | 合格 |
| その他 | `undefined_opcode_runs_as_one_cycle_nop`、`snapshot_round_trip` | 未定義の命令は 1 サイクル。TBLR の途中で保存して戻すと、同じ動作を続ける | 合格 |

期待値は TI の資料の記述と、そこからの手計算で、実チップの測定ではない。MAME との比較（V3）はまだしていない。比べるときは、条件分岐と割込みの受付のサイクル数、SUBC の OV、SAR・SST の間接で書く値が既知の差になる（§03）。

### ES5503

試験は `emulator/crates/chips/tests/es5503.rs`。期待値は台帳の記述と計算から取る。

| 分野 | テスト | 判定基準 | 状況 |
|---|---|---|---|
| 電源投入と有効数 | `power_on_waits_for_the_first_e1_write`、`enable_register_holds_n_minus_1_times_2` | 0xE1 に書くまで走査しない（§4.8）。0x1E で 16 個、62 で 32 個、64 で 1 個（§4.7） | 合格 |
| 走査 | `scan_visits_n_oscillators_then_two_refresh_cycles`、`a_scan_past_the_new_end_runs_on_to_31`、`refresh_cycles_hold_the_last_sig_without_strobe` | N 個の後にリフレッシュ 2 サイクル。N＝16 で 55,555.6 Hz（§3.2）。終点を過ぎた走査は 31 まで回る（§6）。リフレッシュ中は CSTRB が無効で CA＝0xF | 合格 |
| 番地 | `address_follows_table_size_and_resolution` | T・R・ポインタ・BS から番地を作る。R を T より 1 大きくすると半分の速さ（§4.3） | 合格 |
| 周波数 | `free_run_wraps_every_256_samples_217_hz` | 8 MHz、N＝16、256 byte、FC＝0x200 で 217.0 Hz（§4.3 の例） | 合格 |
| 停止とモード | `zero_halts_and_data_register_keeps_the_last_nonzero`、`one_shot_halts_at_the_end_and_rewinds`、`swap_pairs_take_turns`、`sync_resets_the_odd_partner`、`am_odd_oscillator_sets_the_volume_of_the_next`、`halted_oscillator_still_fetches_and_outputs_center` | 0 で止まる（§4.4）。ワンショットは端で止まり、次の訪問で 0 に戻る。スワップは交互に鳴る。同期で奇数番が頭に戻る。AM で奇数番が上の偶数番の音量になる（§4.2） | 合格 |
| 割込み | `irq_requests_come_out_first_in_first_out`、`ie_zero_keeps_the_request_until_ie_is_set` | OIR＝0x41＋番号×2、先入れ先出し。要求がなければ bit7＝1。IE を 1 にすると、テーブルに残った要求が渡る（§4.6、E1 p.7） | 合格 |
| レジスタ | `unused_bits_read_as_one`、`adc_converts_after_the_conversion_time`、`registers_of_unscanned_oscillators_decay` | 未使用の bit は 1（§4.1）。A/D は読んで 30 サイクル後（§3.7）。有効でないオシレータの値は消える（§4.7） | 合格 |
| リセットと保存 | `reset_stops_the_scan_and_clears_requests`、`snapshot_round_trip` | リセットで要求が消え、0xE1 に書くまで止まる（§4.8）。途中で保存して戻すと、同じ出力を続ける | 合格 |

期待値は資料の記述と手計算で、実チップの測定ではない。MAME と比べるとき（V3）は、M0＝1・H＝1 の巻き戻しの時期、同期の偶数番の端数、スワップの起こし方、割込みの規則が既知の差になる（§03）。
