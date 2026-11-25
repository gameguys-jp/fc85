# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 前提

- ユーザーとの会話は日本語で行う。
- fc85は、Fictional Machinesの第一世代（1985年）架空コンソールの実装プロジェクト。
- エミュレータ（`emulator/`、Rust）を段階 1 から書き始めた（DR-040）。ビルドと試験のコマンドは「エミュレータの実装方針」の節。
- このリポジトリの外（親ディレクトリ）に関連文書がある。
  - `../fictional-machines/Fictional_Machines.md`：全体の基本構想・分類体系（最上位文書。分類記号の定義はここ。公開版は https://github.com/gameguys-jp/fictional-machines ）
  - `../Fictional_Console_Hardware_v0.1.md`：旧版（1989〜93年想定）。第二世代の資産。**fc85へICやクロックを移植しない**。2つの仕様はIC構成もアドレスマップも異なる（例：Data Fabricは旧版がDMA 4ch、1985年版は初期2ch実装＋4ch分の窓を予約）

## リポジトリ

- GitHub の公開リポジトリ `gameguys-jp/fc85`。
- オーナーの環境では、コミットの作成者は gameguys-jp（noreply アドレス）で、このリポジトリのローカル設定に入っている。push には、ローカル設定にある資格情報ヘルパーを使う。gh のキーリングから gameguys-jp のトークンを取り出すので、gh の有効なアカウントは切り替えない。
- 共同作業者は、自分の GitHub アカウント（リポジトリの collaborator）でコミットし、push する。作成者の名前は公開の履歴に残る。
- 公開されるので、ローカルの絶対パス、ユーザー名、メールアドレスなどの個人情報を書かない。リポジトリ外の作業用一時フォルダは `<scratchpad>` と書く。
- 資料のPDFはリポジトリに入れない。台帳には書誌とURLを書く。
- ライセンスはハイブリッド（DR-044、DR-045）。ソフトと文書は MIT（ルートの `LICENSE`）。ハードウェアの設計データ（`hardware/` の PCB、FPGA の HDL と制約、機構）は CERN-OHL-P v2 で、`hardware/` を作るときに `hardware/LICENSE` を置く。外部のコードは `third_party/` に元のライセンスのまま置く。GPL のコア（fx68k、MiSTer の es5503.v、jt5205 など）を組み込んだ成果物を配るときは GPL に従う。組み合わせの条件の確認は OI-014。

## 共同作業

複数の人が、それぞれエージェント（Claude Code など）を使ってこのリポジトリで作業する。一般のルールは `../fictional-machines/Documentation_Rules.md` §11。このリポジトリでは次のとおり。

- **始める前と push の前に `git pull --rebase`。** 作業は小さく区切り、こまめに push する。push した履歴は書き換えない（force push をしない）。
- **DR・OI の番号**は、push の直前に取り込んだ最新の次の番号にする。rebase で番号がぶつかったら、まだ push していない側が自分の番号を振り直し、参照（仕様書、台帳、`CLAUDE.md`、コミットメッセージ）も直す。push 済みの番号は変えない。
- **衝突しやすいファイル**：`docs/decisions.md` と `docs/open-issues.md`（末尾に追記し、既存の行を並べ替えない）、`CLAUDE.md`、`README.md`、仕様書 `docs/spec/hardware.md`、外形（`docs/spec/board.md`、`docs/design/enclosure.md`）、字形（`docs/design/glyphs-v0.1.tsv`）、`docs/impl/emulator.md`。これらで節の組み替え、表の作り直し、全体の置換をするときは、ブランチを切ってプルリクエストにし、相手に知らせる。
- **自分の変更と関係のない行を整形しない**（表の幅、句読点、改行）。コードの整形は `cargo fmt --all` だけ。
- **改行は LF**（`.gitattributes`）。
- **公開前の走査**：pre-commit フック `tools/hooks/pre-commit` が、コミットに追加する行のローカルの絶対パス、メールアドレス、トークンを調べる。クローンしたら一度 `git config core.hooksPath tools/hooks` を実行する。自分の名前やユーザー名など個人を特定する文字列は、リポジトリに入れず `.git/info/private-patterns`（1 行に 1 つの正規表現）に書く。フックが止めたときに `--no-verify` を使うかは人が決める。
- **未確定項目は人が決める。** 氏名・社名・製品名・ロゴ・UI・意匠、外寸の確定などを、エージェントは確定させない。案は DR の案にとどめる。
- **どの IC・どの段階を作業しているか**は `docs/impl/emulator.md` §02 の表で分かるようにする。同じ IC や仕様書の同じ節を同時に触りそうなら、先に知らせる。

## 文書の構成

文書は `../fictional-machines/Documentation_Rules.md`（公開版は https://github.com/gameguys-jp/fictional-machines ）の文書管理ルールに従う。Claude Code では skill `fictional-machines:project-docs` で同じルールと雛形を使える。

| 場所 | 内容 |
|---|---|
| `docs/spec/hardware.md` | 仕様書（以下「仕様書」）。機械が何であるか。正となる仕様 |
| `docs/spec/board.md` | 基板の分け方と外形の見積り（最終試作機と量産計画）。筐体の設計の条件。配置図は `docs/spec/img/` |
| `docs/story/` | 開発史・メーカーの出自・音源の差し替えなど。なぜそうなったか |
| `docs/parts/` | 実在部品の台帳と供給調査。事実だけを書き、本機での使い方は仕様書に書く |
| `docs/decisions.md` | 決定記録（DR-xxx）。理由と却下した案 |
| `docs/open-issues.md` | 課題台帳（OI-xxx） |
| `docs/sources.md` | 部品以外の史実（JIS 改正、キャプテンシステム、Commodore など）の出典台帳 |
| `docs/design/` | 製品デザイン。書体（`main.md`、文字 ROM の字形の試案 v0.1）と字形データ `glyphs-v0.1.tsv`、筐体の外観の大枠（`enclosure.md`、[仮]、DR-038）。製品名・ロゴ・UI・意匠の規則は未確定 |
| `docs/impl/` | 実装ごとの文書。`emulator.md`：構成、仕様との差、テストと判定基準 |

分類：F1／T2:1985（適合は未検証）、D5・D6・D7、Machine/System、L2〜L7、E3目標、M1先行、S2〜S4重視、V2→V3/V4、N3〜4。

## フォルダ構成（実装を始めたら作る）

`../fictional-machines/Documentation_Rules.md` §10 に従う。フォルダは必要になってから作る。仕様書§12 の層構成との対応は次のとおり。

| フォルダ | 中身 |
|---|---|
| `emulator/` | §12 の core・chips・board・host と、その単体テスト |
| `firmware/` | Boot ROM（BIOS）、漢字 ROM の生成元。作中の機械に載る中身 |
| `sdk/` | §12 の SDK：m68k・DSP のアセンブラ設定、リンカ、ROM builder、MMIO ヘッダ、BIOS ABI の文書、サンプル |
| `hardware/fpga/`・`hardware/pcb/` | 段階5以降。PCB は基板ごと（メイン、DOC 子基板、映像入力基板など）に分ける。ライセンスは CERN-OHL-P v2（DR-045） |
| `tests/` | §12 の独立参照モデル・ゴールデンデータ・テスト ROM。エミュレータと FPGA で共通に使う |
| `tools/`・`third_party/` | 補助スクリプト。外部のコード（MAME、fx68k など）は元のライセンスのまま置く |

- アドレスマップとレジスタ表は、機械が読める形で `docs/spec/` に1か所置き、エミュレータ・SDK・ハードウェアはそこから生成する（置き場所と形式は実装を始めるときに決める）。
- 実装ごとに `docs/impl/<名前>.md` を置く。
- ライセンスは「リポジトリ」の節のとおり（ソフトと文書は MIT、ハードウェアは CERN-OHL-P v2、外部のコードは元のライセンス）。

## 仕様書を書くときのルール

- 仕様書は確定版ではなく作業中の版。調査で整合しない点、時代考証やリアリティが薄い点が見つかったら随時更新する前提で扱う。発見した課題は `docs/open-issues.md` に記録し、決めたことは `docs/decisions.md` に DR として書いてから、根拠を付けて仕様書に反映する。
- **部品の選び方**：①1985年の日本の中堅業務用電子機器メーカーにとって自然か（国内大手、とくに日立の品を優先。`docs/parts/baseline-1985.md`）、②現代の再現で入手できるか、の順で判断する。
- **リアリティと表現**：1985年の自然さを優先しても、本機の独自の映像・音響表現を失わせない。両者がぶつかるときは、通常モードを実機どおりにし、表現は Bend モード、切り替えのビット、拡張基板の候補 [O] として残す（例：DR-027 のパレットの直結モード、DR-030 の規約違反）。
- **記述ステータス**：`[D]` 原典に根拠のあるIC仕様（§14の参照番号を付ける）、`[P]` 本機独自の暫定設計値、`[O]` 代替・拡張候補、`[V]` 要検証。追記・変更する記述にも必ず付ける。
- **年代の整合性**：1985年以前の資料を年代の根拠とし、後年の資料（1986年のES5503資料、1994年のMSM5205データブック等）はIC復元にのみ使う。後年の機能を1985年機に持ち込まない。
- 現代部品（ADV7180／ADV7391／AD9280／CH446Q／RP2350／FPGA）は実装側の候補。1985年機の搭載ICとして書かない。
- フィクション側では、氏名・社名・製品名・筐体・ロゴ・内蔵UIを意図的に未確定にしている。勝手に確定させない。名前の案は、求められるまで出さない（以前のコードネーム案は却下済み）。筐体は構成だけを [確定] にし、寸法は見積り（[仮]）として扱う（DR-038、DR-039）。最終試作機の外寸は 430×310×95 mm を目標にし、Data Fabric の部品表ができた時点で確定する（DR-043）。色・仕上げ・ロゴ・名称などの意匠は未確定。
- 物語では実在の人物の名前を出さず、役割で書く。物語の中の事実には `[史実]`（台帳へリンク）か `[架空]` を付け、実在の記録と矛盾させない。メーカーを Mirage の輸入元・製造元にしない。

## アーキテクチャの要点

複数の節にまたがる骨格だけを示す。具体的な値は仕様書の該当節（§05アドレスマップ、§06 Data Fabricポート・レジスタ、§08漢字ROM配置、§09入力MMIO）を参照する。

- **ホスト**：日立 HD68000-8（8 MHz）、16-bitホストバス、big-endian。8-bit ICは上位バイトレーン、16-bitレジスタは偶数番地。未割当・不正幅アクセスは有限時間でバスエラー。
- **異種IC**：HD63484-8 ACRTC（描画、2CLK 5.369318 MHz、VRAM 128 KiB）、TMS32010 DSP（プログラムRAM）、ES5503 DOC（子基板、8 MHz、波形RAM 64 KiB。子基板がないときは本体のDACでDSP合成の音を出す、DR-019）、MSM5205 ADPCM、HD6850 ACIA（MIDI）、CD22101系アナログマトリクス、漢字ROM 32 KiB（JIS78基準、JIS83は差分ROMイメージ）。
- **クロック**：発振源は 16 MHz、21.47727 MHz、20 MHz、384 kHz の4つ（DR-003）。DOC・MSM5205・音声クロック・DSP・映像系には外部クロックのジャンパがあり、ピッチの改造ができる。16 MHz にはない（DR-035）。評価用のテストピンとソケットは仕様書 §11 の「ベンド点」（DR-034）。
- **各ICのローカルメモリは共有RAMではない**。CPUからVRAMへはACRTC経由、DSP内部RAMは直接アクセス不可。IC間のデータ移動は Data Fabric（独自DMA／FIFO、ポートID 0〜15）を通る。ポートごとに方向・幅・アクセス条件が異なる。
- **Data Fabricの不変条件**：ゼロ時間の循環接続は禁止（フィードバックはRAM／FIFO経由）。Data Fabricの側はFIFOのfull/emptyで待機し、データを生成しない。待てないIC（DSP、MSM5205）の側は、ICごとのポートアダプタが空・満杯のときの動作を決める（DR-017）。DMA異常がホスト全体の停止に波及しない。
- **通常モードとBendモードを分ける**。ベンディングは「実ICの外部ピンで可能」「専用グルーロジックで可能」「エミュレータ限定の内部介入」を区別し、仕様外の動作は近似と明記する（§11）。
- **映像**：256×192・4bpp、ノンインタレース 262 ライン・342 ドット、約 59.92 Hz（DR-002）。スプライト専用ハードはなく、描画／COPYと背景復元でゲームを構成する。

## エミュレータの実装方針（仕様書§12、DR-040）

- Rust（edition 2024）の cargo workspace。仕様書§12 の層ごとに crate を分ける（今は chips 層の `fc85-chips` だけ）。ホストのライブラリは未定（SDL3、egui が候補）。
- コマンドは `emulator/` で実行する：`cargo test`（全部）、`cargo test -p fc85-chips --test <IC> <名前>`（1 件。例：`--test tms32010`）、`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`。コミットの前に fmt・clippy・test を通す。
- IC のコアは `crates/chips/src/<IC>.rs`、試験は `crates/chips/tests/<IC>.rs`。コアは IC のクロック 1 つを `tick` の 1 回とし（TMS32010 は命令サイクル＝CLKOUT の 1 周期、ES5503 は DOC サイクル＝E の 1 周期、HD63484 は 2CLK の 1 周期）、外部ピンの入出力だけを見せる。IC の外のメモリ（波形 RAM、VRAM）はトレイトの向こうに置く。状態は serde で保存・復元できるようにする。
- 試験の期待値は台帳（`docs/parts/`）の計算から取り、どの節の値かを書く。台帳で [V] の動作の扱いは `docs/impl/emulator.md` §03 に書く。
- 層構成：`core`（時刻・イベント・バス・信号線・IRQ・trace・snapshot）／`chips`／`board`（アドレスマップ・Data Fabric・ポートアダプタ・映像音響経路）／`host`／`SDK`／`tests`。
- 各クロック領域は整数または有理数の時刻で独立に管理する。ホスト出力の表示レートと音声レートは仕様書§12に従う（仮想機のフレームは約 59.92 Hz）。
- MAMEの既存実装（68000、HD63484、TMS32010、ES5503、MSM5205、ACIA）を参照してよい。コードは写さず、台帳に書き起こした式とタイミングから書く（V3 の独立性のため、DR-040）。
- 仮想機が停止するのは許容するが、PCアプリは停止させない。ホスト側の安全層（音声リミッタ、DC除去、表示保持）は仮想機の挙動を変えない別層にする。
- 開発段階：0 仕様・資料 → 1 Chip sandbox → 2 Data Fabric → 3 Console → 4 AV/HCI → 5 PCB/拡張。各段階の合格条件は仕様書§12の表を参照。
- 検証目標：正常系はビット列一致（V2）と独立実装との比較（V3）。
