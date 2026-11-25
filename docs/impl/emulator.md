# エミュレータ

**v0.9**  
2026年11月25日

> 状態：作業中の版。機械仕様は [仕様書](../spec/hardware.md) が正。この文書は実装の設計と、仕様との差を書く。決定記録は [DR-040](../decisions.md)。TMS32010 のサイクル数は [DR-032](../decisions.md)。Data Fabric の動作は [DR-046](../decisions.md)、回路は [fabric.md](../spec/fabric.md)。

## 01　概要

| 項目 | 内容 |
|---|---|
| M（実装媒体） | M1（PC 上のソフト） |
| S（再現忠実度、側面ごと） | chips 層は S1・S2・S3（外部ピンで見える動作、内部状態、IC のクロック単位の時刻）。board 層の Data Fabric は S1・S2 と、スロット（250 ns）単位の S3。S4（DAC の電圧、LPF、アナログ経路）は後の層で足す |
| V（検証、現状→目標） | MSM5205、TMS32010、ES5503 のコアと、HD63484 のコア（表示を除く）は V2（単体試験）。Data Fabric のスロットの模型と、ACRTC の DMA（port 1）のつなぎ、音声クロック・本体 DAC・ADPCM・DSP のアダプタ（DOC のポートを除く）も V2。全体は V0→V2、その後 V3（独立実装との比較） |
| 参照する仕様 | [仕様書](../spec/hardware.md) §12（段階と合格条件）、IC ごとの台帳（[docs/parts/](../parts/)） |
| 言語 | Rust（edition 2024）、cargo の workspace（DR-040） |
| コード | [emulator/](../../emulator/) |

## 02　構成

| 層（仕様書 §12） | crate | 状況 |
|---|---|---|
| core | `fc85-core`（`emulator/crates/core`） | 時刻の単位（1/15.12 GHz。4 つの発振源の周期がすべて整数）とクロックの立上りの列（`time`）。イベント、信号線、trace、snapshot はまだ。Data Fabric の trace は、スロットの記録（`SlotRecord`）で始めた |
| chips | `fc85-chips`（`emulator/crates/chips`） | MSM5205、TMS32010、ES5503、HD63484（データの DMA のピンまで。表示はまだ） |
| board | `fc85-board`（`emulator/crates/board`） | Data Fabric のスロットの模型、67401 の FIFO、ポートの側（Staging RAM、文字 ROM、Temporal RAM、FIFO）、ACRTC の DMA のつなぎ（port 1）、音声クロックと本体 DAC（port 2）、ADPCM のアダプタ（port 5）、DSP のアダプタ（port 3・4）。DOC のポートはまだ |
| host | 未作成 | 表示・音声・操作 UI のライブラリは未定（SDL3、egui が候補） |

### 約束

- IC のコアは、その IC のクロック 1 つを `tick` の 1 回とする。TMS32010 は命令サイクル（CLKOUT の 1 周期＝CLKIN の 4 周期）、ES5503 は DOC サイクル（E の 1 周期＝CLK の 8 周期）、HD63484 は 2CLK の 1 周期を 1 回とする。外部ピンへの入力は `set_*`、出力は `tick` の戻り値と読み出しの関数で返す。IC の外のメモリ（ES5503 の波形 RAM、HD63484 の VRAM）はトレイトの向こうに置き、board 層（または試験）が実装する。クロック領域どうしの合わせ方は core の仕事で、コアは知らない。
- 状態はすべて serde で保存・復元できる（snapshot、Bend のパッチの undo）。
- MAME など既存の実装のコードは写さない。台帳に書き起こした式とタイミングから書く。V3（独立実装との比較）で、比べる相手と独立であるため。
- 台帳で [V] の動作は、どう扱ったかをコードの注釈と §03 に書く。比べたい動作（例：回り込みとクリップ）は設定で切り替えられるようにする。

### コマンド

`emulator/` で実行する。

| 目的 | コマンド |
|---|---|
| 全部の試験 | `cargo test` |
| 1 件だけ | `cargo test -p fc85-chips --test <IC の名前> <試験の名前>`（例：`--test tms32010`）。board 層は `cargo test -p fc85-board --test fabric <試験の名前>` |
| 書式 | `cargo fmt --all` |
| lint | `cargo clippy --all-targets -- -D warnings` |

### IC ごとの状況

| IC | 段階 1 の範囲（仕様書 §12） | 状況 |
|---|---|---|
| MSM5205 | ADPCM の既知列（DR-021 の列を含む） | 実装した。試験 12 件 |
| HD63484 | 描画・COPY・FIFO | ホストとのやり取りと、コマンド 38 種（レジスタ・データ転送、直線・矩形・多角形・円・楕円・弧・塗り・パターン・PAINT・図形コピー）、演算・色・領域モードを実装した。段階 2 でデータの DMA のピン（DREQ・DACK・DTACK・DONE）を足した。試験 63 件。表示（同期、表示アドレス、カーソル）とコマンドの DMA はまだ |
| TMS32010 | 乗算・FIR・外部 I/O。サイクル数は TI の資料どおり | 実装した。試験 33 件 |
| ES5503 | 発音と停止 | 実装した。試験 20 件 |
| HD68000、HD6850 | 段階 1 の対象外 | 段階 3 で |

### 基板の回路の状況

| 回路 | 段階 2 の範囲（仕様書 §12） | 状況 |
|---|---|---|
| Data Fabric | 2 DMA チャネル、待機、IRQ、変換、FIFO、競合、周期トリガ。trace で順序と所要時間を確認 | スロットの模型を実装した：2 チャネル、再設定と START の検査、変換、トリガ（EACH、LOOP、保留、MISS）、STALL・WAIT・ERROR・IRQ、CPU の A ファイルのアクセス、延びるアクセスの時間切れ。試験 26 件。port 1 は、ACRTC のつなぎ（ブリッジの同期化の FF と P1A）と、DONE と COUNT の照合を足した（試験 8 件）。DOC 子基板の port 2 はまだ |
| ポートの側 | 同上 | Staging RAM（port 0）、本体 DAC・DSP・ADPCM・音声入力の FIFO（port 2・3・4・5・10）、Temporal RAM（port 6・7）、文字 ROM（port 9）。本体 DAC（port 2）と ADPCM（port 5）のアダプタ、音声クロック（TRIGGER 3）と V̄C̄K̄（TRIGGER 5）は実装した（試験 12 件）。DSP のアダプタ（port 3・4、プログラム RAM、メールボックス、BIO の選択）も実装した（試験 9 件）。ブリッジと Temporal RAM の CPU 側のレジスタの番地はまだ |

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

### HD63484

コードは `emulator/crates/chips/src/hd63484.rs`。節の番号は [台帳](../parts/HD63484.md) のもので、「追補 2 の (n)」「追補 3 の (n)」は台帳の追補の節。動作は日立のユーザーズマニュアル（1985年5月、台帳の U85）に従った。MAME（M1）は解釈を比べるためだけに読んだ。段階 1 の範囲（描画・COPY・FIFO）のコマンドはすべて作った。表示はまだ作っていない。

| 仕様の項目 | 実装での扱い | 理由 | 影響 |
|---|---|---|---|
| 時間の単位（§3.3） | `tick` の 1 回を 2CLK の 1 周期（本機では 5.369318 MHz）にする | U85 のコマンドの所要時間は 2CLK で数える | — |
| 所要時間（追補 2 の (9)） | U85 p.173 の式どおり。コマンド語を取り出したサイクルも含める。DWT・DMOD・DRD は語ごとに 4、行の終わりに 8、8 語ごとに 16（DRD は 12）、最初に 34（DRD は 62）を割り振る。直線は 1 画素 P、多角形は 1 点 16 | 式の項を、それが生じる時点に置いた | 合計は式と一致する。途中の時刻は近似 |
| パラメータの待ち | FIFO にパラメータが足りなければ、1 サイクルずつ待つ。待ちは式の外に足す | U85 は書かない [V] | ホストの書き込みが遅いと所要時間が延びる |
| 表示とのメモリの取り合い（§7 の 4） | 数えない。描画は式の速さで進む | U85 はインタリーブで「描画の速さを落とさない」と書く。実チップの測定はない [V] | 表示を作ったときに見直す |
| VRAM に書く時期 | CLR・SCLR・CPY・SCPY・直線・矩形・円・楕円・弧・塗り・PTN・PAINT・図形コピーは、パラメータがそろったサイクルに全部書き、その後に所要時間だけ待つ。DWT・DMOD・DRD・WPTN・RPTN と、多角形の各点は、1 語・1 点ずつ | 段階 1 は結果と所要時間を比べる | 描画中の VRAM を表示が読むと、実チップと違う。表示を作るときに語ごと・画素ごとにする |
| ホストのバス（§2） | 16 bit バスだけ。`read`・`write` の 1 回が CS の 1 サイクル。DTACK の待ちは扱わない | 本機の接続 | — |
| 満杯の書込み FIFO・空の読出し FIFO | 書込みは捨て、読出しは 0 を返し、回数を数える | U85 に書いてない [V] | ソフトが SR を見ずにアクセスしたとき |
| SR の上位バイト | 1（$FF） | MAME と同じ [V] | — |
| CED（追補 2 の (1)） | ホストが書込み FIFO に書くと 0。コマンド語を取り出しても 0。コマンドが終わると 1 | U85 p.62 と、MAME の取り出しで 0 にする動作を合わせた | — |
| 未定義のコマンド | CER を 1 にし、ABT まで何も実行しない | U85 は CER が立つとだけ書く [V] | — |
| 読出し専用・アクセスできない番号への WPR | 捨てる | U85 に書いてない [V] | — |
| RES、電源投入（追補 2 の (1)） | RES は CCR、SR、OMR の M/S・STR、FIFO と実行中のコマンドだけを戻す。電源投入時のレジスタは 0 | 実チップの電源投入時は不定 [V] | — |
| ABT を 1 にしたままのとき | コマンドを実行しない。FIFO への書込みは受け付ける | U85 に書いてない [V] | — |
| GBM＝101〜111 | 1 bit/画素とする | U85 は無効とだけ書く。MAME と同じ [V] | — |
| 演算モード 110・111 の CL（追補 2 の (4)） | 描こうとしている色データ（COL で選んだ CL0・CL1、またはパターン RAM の語）の、画素の位置のビット | U85 p.145–146 の「the color data（CL）」 | MAME（P＜CL0、P＞CL1）と比べるとき（V3）は既知の差 |
| 色レジスタの画素の位置（追補 2 の (4)） | 描く画素の物理の位置（語の中のビット）で選ぶ | U85 p.148 | MAME は論理の X で選ぶ。ORG の DPD が 0 でないときに差が出る |
| パターンの位置（追補 2 の (5)） | PPX・PPY・PZCX・PZCY から始める。線は描いた画素ごとに X を進める。面は各行の頭で X をコマンドの開始時の値に戻し、行ごとに Y を進める | U85 p.168–170 の図と例 | MAME は描き始めからの距離で、PSX・PSY から数える（V3 の既知の差） |
| コマンドの後のパターンの位置 | 線のコマンドは進んだまま残す。面のコマンドは X を開始時の値に戻し、Y は進んだまま残す | U85 に書いてない [V] | 続けて描くときの模様のつながり |
| DOT のパターンの位置 | 線のコマンドとして X を 1 つ進める | U85 p.168 は DOT を線のコマンドに入れる。進めるかは書いてない [V] | — |
| COL＝11（追補 2 の (4)） | パターン RAM の語 4×PPY＋PPX（PPX・PPY の下位 2 bit）を色データにする | U85 p.152–153 の例 | MAME は止まる |
| 直線の画素（追補 2 の (8)） | Bresenham。誤差がちょうど半分のときは主の向きにだけ進む | U85 に画素の選び方は書いてない [V] | 実チップと 1 画素ずれる線がありうる。V4 で確かめる |
| 領域モードで止まったとき（追補 2 の (6)） | 外へ出た（101 は中へ入った）最初の画素で止め、CP をその画素にする。ARCT は残りの辺を描かない。APLL などは残りの点を読み捨てる。所要時間はその画素までで数える | U85 に書いてない [V] | 止まった後の CP を使うソフト |
| 領域モードを MAME で | — | MAME は判定しない | V3 の比較では領域モードを使わない |
| AFRCT・RFRCT の終点（追補 2 の (8)） | CP の列で、最後の行の 1 つ先 | U85 p.266、p.268 | — |
| AGCPY・RGCPY | 転送元の画素も ORG の画面から読む。COL は見ない。領域モードは転送先で判定する | U85 p.293–299 | — |
| CPY・SCPY で転送元と転送先が重なるとき | 走査の順に 1 語ずつ読み書きする | 実チップの順と同じと考えた [V] | 重ねて写すソフト |
| CLR・SCLR の RWPe（追補 2 の (7)） | 開始の列で、最後の行の 1 つ先（CLR の例で $C6） | DWT の図と CPY の式（図 C14-2）に合わせた。CLR の図のラベル（$B6）とは違う [V] | 続けて CLR を出すソフト |
| DRD の所要時間 | 最初に 62 を数える（式の 62〜68 の最小） | 幅の中身が資料にない [V] | — |
| データの DMA のバスのサイクル | DACK を 2CLK の立上りで取り込んだサイクルを T1、次の立上りを T3 として DTACK を出し、DACK＝H を取り込んだ立上り（Tn）で FIFO と受け渡す。ピンの遅れは扱わず、立上りで変える（board 層のつなぎが足す） | U85 p.320–321 の図。T の状態は 2CLK の半周期（host-acrtc-interface.md §2.1、§7.3） | — |
| DREQ | DDM＝1 で DWT・DMOD・DRD を始めたとき、矩形の語の数を覚える。受け渡した語がそれに満たず、書込みなら書込み FIFO に余地、読出しなら読出し FIFO に語がある間に出す。サイクルスチール（DRC＝1）は T1 で戻し、Tn で出し直す。バースト（DRC＝0）は保ち、1 要求で最大 8 語の上限は扱わない。PSE・ABT・STR＝0・CDM＝1 の間は出さない | U85 p.66–67 は方式だけを書く [V] | バーストを使うソフト |
| DONE | 転送の最後の語のサイクルの T1 から Tn まで出す | U85 p.50、p.315 No.40 | — |
| DMA の DRD の終わり | プログラム転送と違い止まらない。最後の語を VRAM から読んだら CED を 1 にし、読出し FIFO に残った語は DMA で渡す | U85 p.128 はプログラム転送の場合だけを書く [V] | CED を見て次のコマンドを書くソフト |
| DMA と描画の停止・DDM | STR＝0・PSE・ABT の間も、動いているバスのサイクルは DACK＝H まで進める。ABT は要求をやめる。DDM は転送を始めたときの値で決め、途中で 0 にすると DREQ を出さない | 資料にない [V] | — |
| 空・満杯の FIFO の DMA | プログラム転送と同じく、空の読出しは 0 を渡し、満杯への書込みは捨て、回数を数える | [V] | — |
| コマンドの DMA（CDM） | 作っていない。CDM＝1 の間は DREQ を出さない | 本機はデータの DMA だけを使う（DR-022） | — |
| DRD をプログラム転送で（DDM＝0） | 最後の語の後も止まったままにし、ABT で終わる | U85 p.128 | — |
| DMA（DDM＝1） | ハンドシェイク（DREQ、DACK、DONE）はまだない。プログラム転送と同じ速さで FIFO を通し、DRD も最後の語で終わる | 段階 2（Data Fabric）で足す | — |
| 円・楕円・弧の画素（追補 3 の (1)） | 中心からの相対座標で、整数の式 kx·x²＋ky·y²＝k をたどる。円は x²＋y²＝r²、楕円は b·x²＋a·y²＝b·dX²、弧は CP で取る値。8 近傍で 1 画素ずつ、接線の主の向きに 1 進め、副の向きは式の値が 0 に近いほうを選ぶ（同じなら進めない） | U85 に画素の選び方は書いてない [V] | 実チップと 1 画素ずれる点がありうる。V4 で確かめる |
| 弧の終わり（追補 3 の (1)） | 終点と同じ区分（主に X が動く上・下、主に Y が動く左・右）に入り、主の向きの座標が終点に達するか越えた点で、その点を描かずに終える。CRCL・ELPS は始点を終点とする。終点が曲線から外れていても止まり、1 周しても達しなければ始点で止める | U85 は終点が曲線の上にあることを求めるだけ（p.262、p.303）[V] | 終点を丸めずに渡すソフト |
| 弧のパラメータの幅 | AARC・RARC は下位 13 bit を符号付きとして広げる。CRCL の r は下位 13 bit。AEARC・REARC と ELPS は 16 bit | U85 は AARC・RARC・CRCL で「下位 13 bit が有効」とだけ書く [V] | — |
| 半径が 0、a・b が 0 の曲線 | 描かない（d＝0） | U85 に書いてない [V] | — |
| 曲線とパターン | 線のコマンドとして、描いた点ごとにパターンの X を進める | U85 p.168 | — |
| PTN の走査（追補 3 の (2)） | 表 C36-1 のとおり、行の向きは SD×45°、行を送る向きは SL＝0 で＋90°、SL＝1 で＋45°。SL＝0 で SD が奇数（45° 回した矩形）のときは、行と行の間に市松に描かない画素が残る | 表の向きをそのまま 1 画素ずつ進めた結果 [V] | 回した模様のすき間 |
| PTN のパターンの位置 | AFRCT と同じ（各行の頭で X を開始の値に戻し、行ごとに Y を進める） | U85 p.168 | — |
| PAINT の順（追補 3 の (3)） | CP の行の区間を左右に広げ、左から右へ塗る。その上の行、下の行の順に、区間の下で見つけた塗れる区間の左端を種としてスタックに積む。スタックの最後の種から続ける | U85 は手順の概略だけを書く。種の選び方と順は [V] | 塗り残しが FIFO に出る条件が、実チップと違いうる |
| PAINT のスタック | 4 点。PAINT を実行するときに読出し FIFO にデータがあれば 2 点。あふれた種は X、Y、パターンの位置の 3 語を FIFO へ出す。パターンの位置は Pr05 と同じ形（PPY、PZCY、PPX、PZCX） | U85 p.274 は「2 点以下」「パターンポインタの値」とだけ書く [V] | — |
| PAINT で塗った画素 | 同じコマンドの中で、同じ物理の画素を 2 度塗らない | 塗った色が縁にならないとき（COL＝01・10・11、条件付きの演算モード）、実チップは塗り続けるかもしれない [V] | エミュレータは必ず終わる |
| PAINT のパターンの位置 | CP からのずれで決まる（CP の左や下でも周期で続く）。PS〜PE の外の PP は周期の中へ折り返す。コマンドの後は元の値に戻す | U85 p.272 [V] | — |
| PAINT の終点 Pe | 最後に塗った区間の右端。領域の判定で止まったときはその点 | U85 は「Pe に移る」とだけ書く [V] | — |
| PAINT の所要時間 | 区間ごとに 18×画素数＋102 を足し、58 を引く（最小 44）。FIFO へ出すのは 1 語 1 サイクル | U85 の式は矩形の場合だけ [V] | — |
| PAINT の領域モード | 010・011・110・111 は、領域の境を縁と同じに扱う。001・101 は、塗る範囲が外（101 は中）へ出ようとした点で止める | U85 p.276–277 | — |
| PAINT の時刻 | 実行のサイクルに全部を塗り、その後で種を FIFO へ出す。FIFO が満杯なら、ホストが読むまで出すのを待つ | 実チップは塗りながら出し、満杯なら塗るのを止める（U85 p.274）。塗る順が同じなら、最後の VRAM と FIFO の中身は変わらない | 塗っている途中の VRAM を表示が読むと違う |
| 円・楕円・弧・PAINT を MAME で | — | MAME は浮動小数点で近似し、ELPS は別のパラメータを dX に使い、PAINT は FIFO に出さない（追補 3 の (4)） | V3 の比較は、PTN だけが使える |
| 表示（同期、表示アドレス、カーソル、ライトペン） | まだない。タイミング・表示制御のレジスタ（r80〜rFE）は値を保つだけ。RCR は 0 を返す。LPAR を読むと LPD を 0 にする | 段階 1 の後半で作る | — |

### Data Fabric

コードは `emulator/crates/board/src/fabric.rs`（Fabric）、`fifo.rs`（67401）、`ports.rs`（ポートの側）、`acrtc.rs`（port 1 のつなぎ）。動作は仕様書 §06 と DR-046、回路は [fabric.md](../spec/fabric.md) に従った。fabric.md の順序と状態の PAL の式（OI-050）は、この模型のスロットの列に合わせて書く。

ポートの外側（ポート、モジュールの /PRES、PORT_RESET と AUDIO_N の書込みの信号）は `Ports` トレイトの向こう側に置き、board 層（または試験）が実装する。

| 仕様の項目 | 実装での扱い | 理由 | 影響 |
|---|---|---|---|
| 時間の単位（DR-046 (1)） | `step_slot` の 1 回を 1 スロット（8 MHz の 2 クロック、250 ns）にする。スロット 0 を S にする | スロットの中の時刻（q0〜q3）は、順序（頭の判断、中ほどの準備の取込み、アクセス、4 分の 3 の A ファイルの操作、終わり）としてだけ扱う（fabric.md §03） | CPU やほかの IC との時刻の合わせ方は core を作るときに決める |
| 判断と A ファイル（DR-046 (2)、fabric.md §05） | 各スロットの頭で、このスロットの A ファイルの操作と、次のスロットのアクセスを決める。COUNT の更新は、逐次の読出し元なら R の判断、番地を持つ読出し元と逐次の書込み先なら W の判断、両方が番地を持てば W のスロットに置く | fabric.md §05 [P] | `SlotRecord` の列が、PAL の式を書くときの基準になる |
| 2 スロット以上のアクセス | port 9 の読出しは、2 スロット目の頭で次のアクセスを決める。port 1 と DOC の port 2 は、答えたスロットの次の頭で決める（答えが早くても 2 スロット） | port 9 は終わりが分かっている。fabric.md §06 の「長いアクセスの間は決めない」を、この形に詳しくした（fabric.md v0.2）[P] | port 9 から読む要素は 3 スロットに 1 つ |
| 満杯・空の信号を持つポート（DR-046 (2)） | アクセスのスロットと、その次のスロットの頭では、新しい要素を始めない。port 2 は DOC 子基板があっても対象 | ATR の FLAG は port 2 の両方の中身で 1 | DOC への書込みの後も 1 スロット空く |
| 候補の入れ替えと S のスロット（DR-046 (2)） | 規則のとおり、要素がなく 2 チャネルとも動いていれば候補をスロットごとに入れ替える | 仕様どおり | 入れ替えが S・X と同じ周期なので、port 0 から読むチャネルは、相手が要素を始めない限り候補が S の頭にだけ来て、要素を始められない（相手がトリガや空の FIFO を待つだけで止まる）。OI-052 |
| 動作中の書換え（DR-046 (4)） | 設定のレジスタは、使うスロットの値を使う。R と W の間に SHIFT・MASK・XOR・FORMAT・DST を書き換えると、その要素の W から効く | 回路では、設定のレジスタが変換部と判断バスに直接つながる（fabric.md §02）。仕様書の「次の要素から効く」はおおよその言い方 [P] | 動作中の書換えは Bend の操作 |
| トリガ（DR-046 (8)） | 2 段の同期化と立上りの検出はスロット単位で、入力の立上りから 2〜3 スロットで保留になる。チャネルが止まっている間の立上りは数えない。保留は START で消す | 同期化の FF のクロックは fabric.md で決めていない [V]。止まっている間に MISS が立つのを避けた [P] | — |
| START と状態のビット | START は DONE・ERROR・MISS を変えない（W1C でだけ消える） | 仕様書に書いてない [P] | ソフトは START の前に消す |
| LOOP で COUNT の設定値が 0 | 再設定のたびに DONE にし、また再設定する（3 スロットごと） | 回路の動きをそのまま [V] | 動作中に COUNT を 0 に書き換える Bend |
| 時間切れ（DR-046 (9)） | 延びたアクセスが 16 スロット（32 クロック）で終わらなければ、そのスロットの終わりに ERROR にし、アクセスを打ち切る | DR-046 (9) | — |
| CPU のアクセス（fabric.md §07） | A ファイルの語は、次のスロットの頭で許可し、許可のスロットの後に結果を返す（`HostReply::Wait` と `host_poll`）。68000 のクロック単位のウェイトは数えない | 68000 をまだつないでいない | 段階 3 でウェイトのクロック数を決める [V] |
| 電源投入時 | A ファイルの値と設定のレジスタは 0。MASK は 0xFFFF、XOR は 0 | 資料にない [V]。MASK・XOR は DR-046 (4) | — |
| MODULES への書込み | 捨てる（DTACK は返す） | 読出し専用 [P] | — |
| 67401 の FIFO（review-digital.md 付録 A.1） | 通り抜けの時間を扱わない。書いた次のスロットの中ほどで準備の信号に出る。空の読出しは前の値、満杯の書込みは捨てる。4 個を並べた FIFO を 1 つとして扱う | 通り抜けの時間を資料で確かめていない [V] | 空の FIFO に書いてすぐ読む転送が、実機より早い可能性 |
| まだないポート | DOC 子基板の port 2 は答えない（時間切れで ERROR）。port 8・11〜15 は読むと 0 | 段階 2 の後半で足す | — |
| ポートの向こうの回路の時間 | `Ports::run_slot` を、スロットの頭の判断の後に毎スロット呼び、このスロットで始まるアクセスを渡す。ACRTC のつなぎは、そこでスロットの間の 16 MHz と 2CLK の立上りを時刻の順に進める。準備の信号とアクセスの応答は、その後に問う | 中ほどの準備の取込みと、延びるアクセスの終わりを、スロットの中の時刻に合わせるため | core を作るとき、時刻の管理を core に移す |
| port 1 の時刻（DR-016、host-acrtc-interface.md §2.3・§7） | 時刻の単位は 1 スロットの 1/3780（16 MHz の周期が 945、2CLK が 2816）。Fabric の出力（DACK、R/W、データ）は P1A のクロックの 20 ns 後に変わる。ACRTC は 40 ns 前の DACK を見て（tDAKS）、DREQ・DONE を 50 ns 後、DTACK を 70 ns 後に変える。ブリッジの FF は 16 MHz で 10 ns 前の値を取り、DREQ・DTACK は 2 段、DONE は 1 段。P1A は 2 段目の DTACK を次の立上りで見て DACK を戻し、データと DONE を取り込む。RDY1 は中ほどの立上りの前の 2 段目の DREQ | ACRTC の遅れは U85 の表の値。Fabric の出力の遅れと FF のセットアップは [P]。2 つの発振源の位相は `set_phase` で決め（既定 0）、水晶のずれは扱わない | 1 回のアクセスは位相によらず 3 スロット（DACK の L は 480〜668 ns。§7.2 の 375〜688 ns の範囲） |
| port 1 の速さ | 1 語は、Temporal RAM から 6〜7 スロット、Staging RAM から 6 か 8 スロット（1.5〜2.0 µs）。ACRTC の DWT（約 6 サイクル＝1.12 µs/語）より遅く、port 1 の速さは Fabric の側で決まる | 次の DREQ の同期化と中ほどの取込み、満杯・空の信号の規則、要素の R と W の順による（試験 `one_word_takes_six_or_seven_slots_from_temporal_ram`） | host-acrtc-interface.md §7.2 の「DWT の速度を制限しない」と食い違う（OI-051） |
| DONE と COUNT（DR-022） | port 1 のアクセスが終わったとき、DONE の有無と、要素が最後か（COUNT の桁上げ）が食い違えば ERROR にし、時間切れと同じくチャネルを止める（書いていない要素は捨てる）。DONE は `PortReply::Last` で渡す | DR-022 は ERROR にするとだけ書く [P] | COUNT が矩形より大きいと、DONE の付いた最後の語も書かれない |
| ACRTC の VRAM | 64K 語。ACRTC の 20 bit の語番地の下位 16 bit を使い、上は折り返す | 仕様書 §05 は 128 KiB とだけ書く [V] | — |

### 時刻（core）

コードは `emulator/crates/core/src/time.rs`。仕様書 §12 の「各クロック領域は整数または有理数の時刻で独立に管理する」を、1 つの整数の単位で行う。

| 仕様の項目 | 実装での扱い | 理由 | 影響 |
|---|---|---|---|
| 時間の単位 | 1 単位＝1/15.12 GHz（約 66.1 ps）。16 MHz は 945、8 MHz は 1890、21.47727 MHz は 704、ACRTC の 2CLK は 2816、20 MHz は 756、DSP の命令サイクルは 3024、384 kHz は 39375、DOC の E は 15120、スロットは 3780 | 4 つの発振源（DR-003）の周期がすべて整数になる最小の単位。21.47727 MHz は 945/44 MHz（6fsc）とする | u64 であふれるまで約 1.2×10⁹ 秒 |
| クロック | `Clock` は周期と次の立上りの時刻を持つ。周期は途中で変えられる（外部クロックのジャンパ。DR-035） | — | 水晶の誤差とずれは扱わない |
| 進め方 | 今は Data Fabric のスロットが主で、`Ports::run_slot` が各回路を 1 スロットずつ進める | CPU をまだつないでいない | 段階 3 で、CPU とイベントの順を core に移す |

### 音声クロック・本体 DAC・ADPCM のアダプタ

コードは `emulator/crates/board/src/audio.rs`（音声クロック、本体 DAC）と `adpcm.rs`（ADPCM のアダプタ）。動作は DR-017、DR-018、DR-019、DR-035、台帳 MSM5205.md §3・§4.6 に従った。

| 仕様の項目 | 実装での扱い | 理由 | 影響 |
|---|---|---|---|
| 音声クロック（DR-018） | 8 MHz を N で割る。N＝0 は 4096 として数える。N の書換えは次の拍から効く。拍の出力（TRIGGER 3）は入力の 2 クロック High | 分周器の作りは仕様にない [P]。N＝0 は [V] | N が 2 以下では High のままで、トリガの立上りが出ない [V] |
| 本体 DAC（DR-019） | 拍ごとに port 2 の FIFO から 1 語を取り込む。空なら保ち、UNDERRUN を記録する。DOC 子基板があれば読まない。電源投入時のラッチは 0 | DR-019 (2)(3)。初期値は [V] | — |
| FIFO の受け渡しの時刻 | 本体 DAC と ADPCM のアダプタの取り出しは、スロットの頭（Fabric のアクセスの前）にまとめる | 簡単のため [P] | ずれは 1 スロット（250 ns）以内 |
| ADPCM のニブル（DR-017 (2)(3)） | V̄C̄K̄ の立上りで次のニブルを D3〜D0 に置く（取込みは立下りの 6 クロック後）。byte の 1 つ目のニブルで FIFO から取り出す。既定は上位から。空なら 0x08（0000 と 1000 が交互）で UNDERRUN。置換しない Bend では、FIFO の出力に残った前の byte を使う | 立上りで置くのは [P]（台帳 §3.2 の取込みの時刻に間に合う） | — |
| ADPCM の RESET | 電源投入時は CTRL の RESET＝1。RESET の間は FIFO から取り出さず、解除の後と PORT_RESET の bit 5 の後は、byte の 1 つ目のニブルから始める | [P] | 解除の直後の 1 回は、前に置いたニブル（電源投入時は 0）を取り込むことがある |
| 半分空きの割込み（DR-017 (4)） | FIFO の残りが 32 byte 以下の間のレベル | 「半分空いたら」をレベルと読んだ [P] | 書き足して 33 byte 以上になると下がる |
| S1＝S2＝H | V̄C̄K̄ を駆動しないので、取込みは起きない | Bend のモード。外から V̄C̄K̄ を入れる回路は決めていない [V] | — |
| ブリッジのレジスタ | CTRL を構造体の値（RESET、S1、S2、ニブルの順、置換しない、半分空きの割込み）で持つ。CPU から見た番地とビット（0x230000〜）は段階 3 で決める | CPU をつないでいない | — |
| ポートの向こうのトリガ（DR-018） | `Ports::trigger_levels` が TRIGGER 3（音声クロック）と 5（V̄C̄K̄）のレベルを出し、`Fabric::set_trigger_inputs` の値と OR をとる。スロットの終わりのレベルを取り込む | — | 250 ns 以上のパルスは、必ず 1 回取り込まれる |
| トリガの発生源の選択（fabric.md §06） | 選択器（74LS151）が同期化の前にあるので、選んだ入力が High なら立上りとして伝わる。START の直前に TRIGGER を書くと、最初の要素がすぐ始まる | 回路どおり [P] | ソフトは TRIGGER を書いてから 3 スロット以上あけて START する |

### DSP のアダプタ

コードは `emulator/crates/board/src/dsp.rs`。動作は DR-017、DR-029、DR-035、台帳 TMS32010.md §2.1・§2.2 に従った。

| 仕様の項目 | 実装での扱い | 理由 | 影響 |
|---|---|---|---|
| 時間 | 命令サイクル（CLKOUT、200 ns＝3024 単位）ごとに進める。外部クロックのジャンパは周期の変更（DR-035） | — | スロットの中の DSP のサイクルは、そのスロットの Fabric のアクセスより前に済ませる [P] |
| RESET と PRGBUS（DR-029 (1)） | CTRL.RESET＝1 で次の CLKOUT から RS を Low にし、8 サイクル目に PRGBUS＝1。0 に戻すと次の CLKOUT で PRGBUS を下げ、その次で RS を解く。電源投入時は RESET＝1 | 台帳 §2.1。電源投入時の値は [P] | PRGBUS＝0 の間のプログラム RAM の読み書きはバスエラー（`None`・`false`）。電源投入時の中身は 0 [V] |
| I/O（台帳 §2.2） | IN 0 は入力 FIFO（port 3）から取り出し、空なら前の値で UNDERRUN。OUT 1 は出力 FIFO（port 4）へ入れ、満杯なら捨てて OVERRUN（DR-017 (2)）。IN 2 は状態、IN 3 はメールボックス（読むと着信を下げる）、OUT 2 は応答（68000 への要求を上げる） | — | ほかのポートの IN は 0、OUT は捨てる [V]。DSP の側から UNDERRUN・OVERRUN を消す手段はまだない（68000 の W1C だけ）|
| IN 2 の状態（OI-053） | bit 0 入力 FIFO が空でない、bit 1 出力 FIFO に空き、bit 2 コマンド着信、bit 3 応答が未読、bit 4 UNDERRUN、bit 5 OVERRUN、bit 15〜8 入力 FIFO の語数（アダプタのカウンタ） | 仕様にない。仮の配置 [P] | 仕様で決めたら直す |
| TBLW と OUT（DR-029 (2)） | プログラム RAM の /WE は 8〜4095 番地だけ。TBLW で 0〜7 番地に書くと OUT と同じになり、回数を `io.port_tblw` に数える（警告） | DR-029 (2) | — |
| BIO（DR-029 (3)） | CTRL.BIOSEL の条件を CLKOUT で 1 段の FF に取って BIO に入れる。音声クロックの拍は、その時刻の TRIGGER 3 のレベル（250 ns のパルス） | FF の段数は [P] | 待ちのループでは拍を取りこぼす。N が 8 の倍数だと拍と DSP の位相が固定され、全部取るか全部落とす（OI-053） |
| INT | メールボックスの着信の間 Low | [P] | 立下りで INTF。IN 3 を読むまで Low のまま |
| bridge のレジスタ | CTRL（RESET、BIOSEL）、STATUS、メールボックス、応答を構造体の値と関数で持つ。CPU から見た番地（0x220000〜）とビットは段階 3 で決める | CPU をつないでいない | — |

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

### HD63484

試験は `emulator/crates/chips/tests/hd63484.rs`。期待値は台帳の追補 2・追補 3 と、そこに引いた U85 の例から取る。VRAM は 1 M 語の配列にし、4 bit/画素で試す。

| 分野 | テスト | 判定基準 | 状況 |
|---|---|---|---|
| ホスト | `reset_state`、`address_register_increments_only_from_r80`、`fifo_flags_follow_the_contents`、`abort_clears_the_fifos_and_sets_sr_23`、`str_zero_and_pause_hold_the_command`、`undefined_command_sets_cer_until_abort`、`command_end_interrupt` | RES の後は SR＝$23、CCR＝$8000。AR は r80 から 2 ずつ進む。WFE・WFR・CED。ABT で SR＝$23。STR＝0 と PSE＝1 で止まる。CER と IRQ（追補 2 の (1)） | 合格 |
| レジスタのコマンド | `wpr_and_rpr_take_6_cycles`、`wptn_and_rptn_example`、`read_fifo_full_makes_the_command_wait`、`org_example_sets_the_origin_and_clears_cp`、`parameters_arriving_late_add_wait_cycles` | U85 の例（CL1 に $1111、番地 $B に $2314・$5713、ORG $0000・$0254 で語 $25 の bit 7–4）。6、16、18、8 サイクル。読出し FIFO が満杯なら待つ（追補 2 の (2)、(3)、(9)） | 合格 |
| データ転送 | `rd_wt_mod_step_rwp`、`clr_example`、`sclr_modifies_only_mask_bits`、`cpy_example_rotates_columns_into_rows`、`dwt_program_transfer_and_rwpe`、`dmod_applies_mm_and_mask`、`drd_hangs_after_the_last_word_until_abort` | CLR の例：$52〜$56 の 5 語×7 行で (2·5＋8)·7＋12 サイクル。CPY の例：転送元の列が転送先の行になり、RWPe＝$70。MASK と MM。DRD は最後の語の後で止まる（追補 2 の (4)、(7)） | 合格 |
| 演算と色 | `operation_modes`、`color_modes` | OPM の 8 通り（CCMP は同じ位置の画素と比べる）。COL の 4 通り（追補 2 の (4)） | 合格 |
| 線と図形 | `aline_example`、`arct_example`、`apll_example`、`relative_polyline_matches_absolute`、`aplg_closes_back_to_cp`、`line_pattern_with_zoom`、`moves_take_56_cycles` | U85 の例の点で、終点を描かず、CP と画素数とサイクル数が式どおり。PZX＝1 で '1111101010' が '11111111110011001100'（追補 2 の (5)、(8)、(9)） | 合格 |
| 塗り | `filled_rectangle_example`、`rfrct_paints_toward_negative_corner` | 図 6.17(b) を書き起こした 10 行と一致し、(4·11＋8)·10＋18 サイクル。Pe は CP の列で最後の行の 1 つ先（追補 2 の (5)、(8)） | 合格 |
| 領域 | `area_modes` | 7 通りの AREA で、描く画素、止まった CP、ARD。RPR で ARD が 0（追補 2 の (6)） | 合格 |
| 図形コピー | `agcpy_example`、`rgcpy_mirrors_with_dsd` | U85 の例：Pe＝(4, 16)、転送先 (4＋i, 2＋j) が転送元 (18＋j, 2＋i)。DSD で左右を反転（追補 2 の (8)） | 合格 |
| 円・楕円・弧 | `crcl_example_draws_a_closed_circle_around_cp`、`crcl_of_radius_zero_draws_nothing`、`elps_example`、`aarc_example_draws_three_quarters`、`aarc_clockwise_takes_the_short_way`、`rarc_example`、`arc_with_pe_equal_to_cp_draws_the_whole_circle`、`aearc_quarter_and_rearc_agree`、`curves_advance_the_line_pattern` | U85 の例（r＝7、a＝9・b＝4・dX＝9、AARC・RARC の 4 分の 3 の円）。どの点も曲線から半画素以内で、上下・左右・対角に対称で、閉じている。時計回りでも同じ画素。弧は円の画素のうち、通る範囲だけで、終点を描かない。CP と 8d＋66、10d＋90、8d＋18、10d＋96 サイクル（追補 3 の (1)） | 合格 |
| パターン | `ptn_example_copies_the_pattern`、`ptn_smaller_and_larger_than_the_pattern`、`ptn_zoom_doubles_each_dot`、`ptn_scan_directions` | U85 の例 (1)・(3)・(4)・(5) の SZ と拡大。SL・SD の 16 通りの向きと Pe（表 C36-1）。(4·6＋10)·8＋20 サイクル（追補 3 の (2)） | 合格 |
| PAINT | `paint_fills_a_rectangle_inside_the_edge`、`paint_pattern_follows_the_offset_from_cp`、`paint_inside_a_circle`、`paint_treats_cl0_and_cl1_as_edges`、`paint_e1_fills_the_region_of_the_edg_color`、`paint_overflow_seeds_go_to_the_read_fifo`、`paint_with_data_in_the_read_fifo_has_a_two_point_stack`、`paint_area_modes` | 矩形で (18·10＋102)·7−58 サイクル。円の中だけを塗る。CL0・CL1 も縁。E＝1。10 個の部屋のうち 6 個の種が FIFO に出て（18 語で FIFO が満杯になり、読むまで待つ）、図 C34-6 の手順でやり直すと全部塗れる。領域モード（追補 3 の (3)） | 合格 |
| データの DMA | `dma_dwt_one_word_per_request_and_done_on_the_last`、`dma_cycle_takes_t1_t3_and_waits_for_dack_high`、`dma_drd_hands_out_the_read_fifo_and_does_not_hang`、`dma_drd_requests_only_while_the_read_fifo_has_data`、`dma_burst_keeps_dreq_through_the_cycle`、`dma_write_fifo_full_holds_dreq`、`dma_needs_ddm_and_stops_on_pause_and_abort`、`dma_state_survives_a_snapshot` | 1 語ごとの DREQ（サイクルスチールは T1 で戻す）、T1・T3・Tn、最後の語の DONE、DMA の DRD は止まらない、満杯の書込み FIFO の間は要求しない、DDM・PSE・ABT（U85 p.49–50、p.66–67、p.320–321） | 合格 |
| 保存 | `snapshot_round_trip_mid_transfer` | DWT の途中で保存して戻すと、同じ VRAM と状態になる | 合格 |

期待値は U85 の例・記述と手計算で、実チップの測定ではない。曲線の画素は、曲線への近さと対称性だけを見ている（選び方は §03 の [V]）。直線の画素は、画素の数と列ごとに 1 画素であることだけを見ている（選び方は §03 の [V]）。MAME と比べるとき（V3）は、演算モード 110・111、パターンの位置、色レジスタの位置、COL＝11、領域モード、CED の時期、円・楕円・弧・PAINT が既知の差になる（§03）。

### Data Fabric

試験は `emulator/crates/board/tests/fabric.rs` と、port 1 は `tests/acrtc_port.rs`。期待値は仕様書 §06、DR-022、DR-046、fabric.md の記述と、そこからの手計算で取る。

| 分野 | テスト | 判定基準 | 状況 |
|---|---|---|---|
| 変換とポートの表 | `transform_follows_dr046`、`port_table_matches_fabric_md` | FORMAT 0〜6、SHIFT の正負と −16、MASK・XOR、8 bit の出力の FD の上位（DR-046 (5)、fabric.md §04）。番地を持つポートは 0・2・8・9、満杯・空の信号は 1〜5・10〜15。向きと幅（fabric.md §06） | 合格 |
| レジスタ | `registers_read_zero_and_bad_addresses_get_no_dtack`、`register_file_access_waits_for_the_next_slot`、`start_copies_the_settings_in_three_slots`、`count_zero_is_done_without_a_transfer`、`start_checks_the_ports_and_the_format` | 書込み専用のレジスタは 0、MODULES、バスエラーの番地（DR-046 (10)）。A ファイルは次のスロットで許可。再設定は 3 スロットで、COUNT の作業値は残りの数−1。COUNT＝0 は転送せずに DONE。START の検査で ERROR になる 5 通り（DR-046 (9)） | 合格 |
| 転送の時間 | `staging_to_temporal_moves_one_byte_every_two_slots`、`staging_to_staging_takes_four_slots_per_element`、`count_update_goes_where_the_address_file_is_free`、`fifo_reads_take_three_slots_and_fifo_writes_four`、`char_rom_reads_take_two_slots_and_ignore_bit_15`、`candidate_alternates_while_both_channels_run` | 2 スロットに 1 要素（4 MB/s）、port 0 どうしは 4 スロット、FIFO から読むと 3、FIFO へ書くと 4（仕様書 §06）。port 0 は S のスロットだけ。COUNT の更新の置き場所 4 通り（fabric.md §05）。port 9 は 2 スロット。2 チャネルが交互に候補になり、交互に要素を始める | 合格 |
| 待ちとトリガ | `empty_source_fifo_stalls_and_resumes`、`each_trigger_moves_one_element`、`block_trigger_with_loop_keeps_one_pending_and_counts_a_miss`、`loop_restarts_from_the_settings`、`stop_discards_the_element_not_yet_written` | STALL と再開。EACH＝1 は 1 回で 1 要素。EACH＝0 と LOOP：ブロックの途中の 1 回は保留、2 回目は MISS。LOOP は設定値から。STOP は書いていない要素を捨て、DONE にしない（DR-046 (6)〜(8)） | 合格 |
| 延びるアクセス | `a_long_access_times_out_after_32_clocks`、`doc_writes_wait_for_the_answer` | 答えない port 1 は 16 スロットで ERROR と IRQ になり、打ち切りは 1 回。DOC への書込みは答えまで延び、終わった次の頭は満杯・空の信号の規則で空く（DR-046 (1)(2)(9)） | 合格 |
| CPU と割込み | `cpu_access_to_the_register_file_takes_a_slot_from_the_transfer`、`irq_follows_done_and_irq_en` | 転送中の COUNT の読出しが 1 スロットを取り、転送は正しく終わる。IRQ は IRQ_EN∧DONE で、W1C で消える | 合格 |
| ポートの細部 | `port0_byte_lanes_and_word_alignment`、`mask_xor_and_shift_apply_on_the_way`、`port_reset_clears_fifos_and_pointers` | 8 bit の偶数番地は上位バイト、16 bit は bit 0 を無視（DR-046 (3)）。0x1234 を 8 回して MASK・XOR で 0x3403。PORT_RESET | 合格 |
| 保存 | `snapshot_round_trip_mid_transfer` | 2 チャネルの転送の途中で保存して戻すと、同じスロットの列と状態になる | 合格 |
| 候補の入れ替え | `a_staging_channel_starves_while_the_other_channel_waits` | Staging RAM → Temporal RAM の 100 要素は、ch1 がなければ 400 スロットで終わる。ch1 がトリガを待って動いていると 2 要素で止まる（DR-046 (2) のままの動作。OI-052 で直したら期待値を変える） | 合格 |
| port 1 | `dwt_from_staging_ram_fills_the_rectangle`、`drd_to_staging_ram_reads_the_rectangle`、`one_word_takes_six_or_seven_slots_from_temporal_ram`、`count_larger_than_the_rectangle_is_an_error`、`count_smaller_than_the_rectangle_is_an_error_and_abt_clears_the_acrtc`、`the_channel_stalls_until_the_acrtc_requests`、`stop_in_the_middle_of_a_port1_access_releases_dack`、`snapshot_round_trip_mid_port1_transfer` | DWT・DRD の矩形を COUNT＝x·y で運ぶ（DR-022）。1 回のアクセスは 3 スロット、1 語は Staging RAM から 6 か 8、Temporal RAM から 6〜7 スロット（§03 の時刻からの計算。2CLK の位相を 7 通りに変える）。COUNT が大きくても小さくても ERROR、ABT で ACRTC が止まる。DREQ まで STALL。STOP で DACK を戻す。途中で保存して戻せる | 合格 |

期待値は仕様の記述と手計算で、回路のシミュレーションでも実機の測定でもない。fabric.md の順序と状態の PAL の式ができたら、この試験のスロットの列と突き合わせる（OI-050 (1)）。

### 音声クロック・本体 DAC・ADPCM

試験は `emulator/crates/board/tests/audio.rs`、時刻は `emulator/crates/core/src/time.rs` の単体試験。期待値は DR-017、DR-018、DR-019、台帳 MSM5205.md §3 と、そこからの手計算で取る。

| 分野 | テスト | 判定基準 | 状況 |
|---|---|---|---|
| 時刻 | `periods_are_exact`、`clock_edges_within_a_slot` | 各クロックの周期が単位で割り切れる。1 秒は 4,000,000 スロット。立上りの列と周期の変更 | 合格 |
| 音声クロック | `audio_clock_divides_8mhz_by_n_from_the_next_beat` | N＝0 の最初の拍は 2048 スロット目、N＝100 で 50 スロットごと（8 MHz÷N）。入力を 4 MHz にすると倍（DR-035） | 合格 |
| 本体 DAC | `main_dac_takes_one_word_per_beat_and_holds_when_empty`、`the_fabric_keeps_the_dac_fifo_filled_from_staging_ram` | 拍ごとに 1 語、上位 byte が L。空なら保って UNDERRUN（DR-019）。Fabric が満杯の FIFO で STALL しながら 100 語を順に渡し、UNDERRUN が出ない | 合格 |
| ADPCM | `adpcm_uses_the_high_nibble_first_and_0x08_when_empty`、`adpcm_low_first_and_no_replace`、`adpcm_capture_period_follows_s1_s2`、`adpcm_reset_leaves_the_fifo_alone`、`adpcm_half_empty_irq_is_a_level` | 0x12・0x34 から 1、2、3、4 の後に 0、8、0、8。下位から・置換しないで 2、1、4、3、4、3。S1・S2 で 500・667・1000 スロット（384 kHz÷48・64・96）。RESET の間は取り出さない。32 byte 以下で割込み（DR-017） | 合格 |
| トリガ | `trigger3_paces_one_element_per_beat`、`trigger5_follows_vck`、`selecting_a_high_source_right_before_start_counts_as_an_edge` | 拍（20 スロットごと）と V̄C̄K̄（500 スロットごと）に 1 要素。High の入力を選んで直後に START すると、最初の要素がすぐ始まる | 合格 |
| 保存 | `snapshot_round_trip_with_audio_running` | 音声クロック、本体 DAC、ADPCM が動いている途中で保存して戻すと、同じスロットの列と状態になる | 合格 |

### DSP のアダプタ

試験は `emulator/crates/board/tests/dsp.rs`。期待値は DR-017、DR-029、台帳 TMS32010.md §2.1・§2.2 と、そこからの手計算で取る。DSP のプログラムは試験の中で命令の符号から組む。

| 分野 | テスト | 判定基準 | 状況 |
|---|---|---|---|
| RESET と窓 | `prgbus_follows_reset_and_gates_the_program_window` | PRGBUS は RESET から 8 CLKOUT（6 スロット目）。その前と解除の後の窓はバスエラー。解除すると 0 番地から動き、もう一度 RESET すると止まって始め直す | 合格 |
| データの流れ | `dsp_processes_a_block_through_the_fabric`、`soft_synthesis_paced_by_output_space` | Fabric が Temporal RAM → port 3 → DSP（2 倍）→ port 4 → Temporal RAM で 16 語を渡し、UNDERRUN・OVERRUN が出ない。出力の空きを BIO にしたソフト合成が、本体 DAC に 0〜149 を欠けずに渡す | 合格 |
| 空と満杯 | `empty_input_returns_the_last_word_and_full_output_drops` | 空の IN 0 は前の値で UNDERRUN 1 回。OUT 1 を 100 回で、64 語が入り 36 回が OVERRUN。W1C は書いたビットだけ消す | 合格 |
| 状態とメールボックス | `status_port_and_mailbox`、`a_command_interrupts_the_dsp` | IN 2 の値（仮の配置、OI-053）、IN 3 で着信が下がる、OUT 2 で応答。着信で INT が入り、割込みの中で応答を返す | 合格 |
| TBLW | `tblw_to_addresses_0_to_7_acts_as_out` | 2 番地への TBLW は RAM を書かずに OUT 2 になり、警告を数える。0x80 番地へは書く | 合格 |
| 拍の BIO | `a_polling_loop_catches_all_or_none_of_the_audio_ticks` | 待ちのループで数えた拍が、N＝40 で 400/400、N＝48 で 0/333（OI-053 の問題を示す。仕様を直したら期待値を変える） | 合格 |
| 保存 | `snapshot_round_trip_while_the_dsp_runs` | DSP が動いている途中で保存して戻すと、同じスロットの列と状態になる | 合格 |
