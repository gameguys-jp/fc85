# Fictional Console / 1985
## 第一世代 ハードウェア設計仕様書 v0.1

Fictional Machines / GAME GUYS ｜ 2026.11.24 ｜ WORKING SPECIFICATION

### 01　目的・設計境界

**Heterogeneous Modular Computing — 異種プロセッサによるモジュラーコンピューティング**

1985年に完成した未発売の家庭用多目的コンピュータを設計する。ゲーム、日本語情報処理、教育、音楽、映像制作、通信を想定する。異なる用途の実在ICを接続し、映像・音響・文字・数値・信号を相互に扱える構造を機械固有の特徴とする。1985年末を技術上の基準とし、当時の技術で製造可能な構成を目指すが、家庭用としての採算は成立条件にしない。

| 項目 | 設計原則 |
|---|---|
| 架空機の仕様 | 1985年までの技術を原則とする。独自のTTL・PAL・ゲートアレイ等の設計を許容。規模・タイミングは検証対象 |
| 通常動作 | 自作カートリッジ、ゲーム描画、音源、汎用入力、文字表示。映像入力・通信・制作機能も想定 |
| 表現 | A/V双方向処理、データ・論理・タイミング・回路・符号化のベンディング。コンポジット波形の再現 |
| HCI | コントローラを固定的なゲームパッドに限定しない。外部入力の意味をソフトウェアで割り当てる。物理形状・操作体系は保留 |
| 実装 | PCエミュレータ最優先。後から実IC、FPGA、RP2350による混在PCBへ移行可能 |
| 部品選定 | 同等なら入手性、小型、低価格、PCBA対応を優先。電気特性が表現に重要な箇所は実ICを優先 |

**本書の位置づけ**：旧「Fictional Console ハードウェア設計仕様書 v0.1」（1989〜1993年想定）は第二世代の設計資産として維持する。本書は第一世代専用の新規仕様であり、旧版のICやクロックをそのまま移植しない。

| 表記 | 意味 |
|---|---|
| [D] | メーカー原典・技術資料等に根拠のあるIC仕様（参照番号付き） |
| [P] | 本機独自の暫定設計値。実装開始時の基準 |
| [O] | 代替・拡張候補。初期実装に必須ではない |
| [V] | 未確認の供給条件、電気特性、回路・性能等。検証が必要 |

分類：F1/T2:1985を目標（適合は未検証）、D5・D6・D7、Machine/System、L2〜L7、E3目標、M1先行、S2〜S4重視、V2→V3/V4、N3〜4。製造用回路図や完全なピン割当表ではない。

<div class="pagebreak"></div>

### 02　フィクションと設計文化

| 年 | 架空の開発史 |
|---|---|
| 1981〜82 | 日本の中堅業務用電子機器メーカーが、文字表示・映像・通信技術を家庭向けに転用。カートリッジ式ゲーム機能を追加 |
| 1983 | JIS C 6226改正（9月1日）による文字対応の不整合、原価と市場環境の問題が重なり、年末発売を延期 |
| 1983末〜84 | 欧州出身のデザインディレクターを招聘。技術部門の多機能志向を整理し、外部機器・操作体系を横断する製品を再設計 |
| 1985 | 第一世代最終試作機が完成。低価格化より機能追加を優先した結果、家庭用製品として採算が取れず量産を断念 |

JIS改正は技術的に解決不能な事故ではなく、延期の一因とする。旧規格維持、変換表、ROM再製造といった回避策は存在する。既存開発環境との互換性を残すため、最終試作機でも旧字形ROMを使う構成を暫定採用する。新ROMへの更新案も残す。

メーカーの特徴は「盛り癖」：業務用技術を惜しみなく採用し、用途を広げてしまう。デザインディレクターは、複雑な内部を一貫した操作で扱えるようにする。氏名、社名、製品名、筐体、ロゴ、内蔵UIは確定しない。

#### 機械に反映する設計思想 [P]

- ゲームだけでなく、媒体を作成・変換・交換する装置とする。
- 標準のゲーム入力を残しつつ、外部楽器、ノブ、スイッチ、センサー等を入力として利用できる。
- 内蔵BIOSに文字描画、入出力、転送の共通機能を設ける。カートリッジなしでも診断・基本操作ができることを目標とする。
- HCIの再検討時に、入力端子やイベント仕様を変更できるよう、コントローラとゲームの意味づけを分離する。

将来は第二世代（1989〜92年頃）、第三世代（1994〜96年頃の3D機）などへ設計文化を継承する。後継機の具体的な仕様や発売状況は未決定。

<div class="pagebreak"></div>

### 03　システムアーキテクチャ [P]

<div class="arch">
<div class="node primary">MC68000 / Boot ROM / Main RAM / Cartridge</div>
<div class="connector">16-bit host bus：レジスタ・IRQ・DMA制御</div>
<div class="triple"><div class="node">HD63484<br>Graphics / VRAM</div><div class="node">TMS32010<br>DSP / Program RAM</div><div class="node">ES5503<br>Wave RAM / DAC</div></div>
<div class="connector">各ICのネイティブポート・ラッチ・調停回路</div>
<div class="node accent">Data Fabric — 16-bit DMA・FIFO・アドレス／データ変換</div>
<div class="connector">独立ポート・逐次転送・同期イベント</div>
<div class="triple"><div class="node">Character ROM<br>JIS78 / JIS83</div><div class="node">Temporal RAM<br>Capture / Delay</div><div class="node">MSM5205<br>ADPCM FIFO</div></div>
<div class="connector">D/A・A/D・同期・バッファ回路</div>
<div class="node">CD22101系アナログマトリクス / Audio & Video signal path</div>
<div class="triple"><div class="node">CVBS IN / OUT<br>映像拡張基板</div><div class="node">Audio IN / OUT<br>Mix / Filter</div><div class="node">Controller / MIDI<br>拡張バス</div></div>
</div>

図1　機能構成。全ICが同一RAMへ直接アクセスできることを意味しない。Data Fabricのポートごとに物理的な接続・アクセス制約をモデル化する。外部映像入力は第一世代の機能目標だが、基板分離を許容する。

| 区分 | 構成 |
|---|---|
| 基本 | CPU、ACRTC、DSP、DOC、ADPCM、漢字ROM、Data Fabric、時間処理RAM、MIDI・汎用入力 |
| 基本／拡張 | CD22101系マトリクス、コンポジット入力、音声入力、通信・外部バス |
| 現代実装 | ADV7180 / ADV7391、AD9280（代替・追加候補）、CH446Q、RP2350、FPGA。1985年の搭載ICとはしない |
| 第二世代へ保留 | TMS34010、IMS A110、μPD42280、専用動き推定・JPEG回路 |

<div class="pagebreak"></div>

### 04　実在ICの参照仕様と適用

| IC | [D] 参照仕様 | [P/V] 本機の使い方 |
|---|---|---|
| MC68000 [1] | 16-bit外部データ、24-bitアドレス、非同期バス、DTACK、バス調停信号 | 8 MHz。16 MiB空間。ROM起動、周辺待ち時間、バスエラー |
| HD63484-8 [2][3] | 1984年の資料に8 MHz品、2 MiBグラフィックメモリ空間、38種の描画命令、画面分割・外部同期・DMA。16-bitホストとローカルバス | 8 MHz。128 KiB VRAM。AR/STATUSとデータポート。FIFOを介した描画・転送。ローカルVRAMをCPUに直結しない |
| TMS32010 [4] | 16-bit固定小数点DSP、16×16乗算、32-bit演算器、144ワード内部データRAM、4Kワード外部プログラム、8 I/Oポート。200 ns命令サイクル級 | 20 MHz入力／5 M命令サイクル毎秒級を候補。外部8 KiBプログラムRAMを停止中にロード。I/Oラッチ経由で処理。速度グレード確認 |
| ES5503 [5][6] | 32オシレータ、64 KiB直接波形アドレス、周波数・音量・波形位置等の制御、時分割波形アクセスと内蔵DAC | 約7 MHzを候補。64 KiB波形RAM、初期16有効オシレータを目標。CPU/FabricアクセスはDOCの空き位相に限定。第三者供給は未確認 |
| MSM5205 [7] | 3/4-bit ADPCM、384 kHz基準で4/6/8 kHz、DACと外付けフィルタ | 4-bit、8 kHz。外部FIFOからニブルを供給。復号履歴とリセットをモデル化 |
| CD22101 [8] | 4×4×2アナログクロスポイントスイッチ。1980年RCA資料に掲載 | 初期は1個相当の経路モデル。必要なら複数個。電圧、帯域、RON、制御ピンは実装前に照合 |
| TDC1007 [9] | 1985年TRW資料の8-bit、最大20 MSPS級映像ADC | [O] 当時の生映像取り込み用候補。同期分離、クランプ、色復号、RAM書込みは外部回路 |
| MC6850系 [10] | 非同期シリアルI/O、制御・状態・データレジスタ | MIDI IN/OUT用候補、31.25 kbit/s。オプトアイソレータ等は別設計 |

HD63484のメーカー資料の接続例には68000とDMAコントローラが含まれる。対してDOCは別のCPUバス形式を前提とするため、クロック位相・バイト幅を変換する専用アダプタを置く。DSPも外部RAMを汎用データ空間として直接参照できない。

ES5503は1985年のMirage採用例に基づくが、確認した詳細仕様書は1986年版である。1985年の個体との差異と供給契約は未検証。後年の資料は復元のために参照するもので、後年の機能を無条件で1985年に持ち込まない。

<div class="pagebreak"></div>

### 05　メモリ構成・ホストバス [P]

CPUアドレスはbyte単位。big-endian。リセット時の初期SP/PCはBoot ROMの先頭8 byteに配置する。16-bitレジスタは偶数番地、8-bit ICは上位バイトレーンに配置。未割当・不正幅アクセスは有限時間でバスエラー（初期実装は64 CPUサイクル以内を目標）。

| CPU番地（hex） | 領域 | 用途 |
|---|---|---|
| 000000–00FFFF | 64 KiB Boot ROM | 診断、BIOS、ROM起動 |
| 010000–017FFF | 32 KiB Character ROM | 字形バイトを読出し専用で公開 |
| 100000–11FFFF | 128 KiB Main RAM | CPU作業領域 |
| 120000–12FFFF | 64 KiB AV staging RAM | DMA、DSP入出力、素材 |
| 200000–200003 | ACRTC host | +0 AR(write)/STATUS(read)、+2 DATA |
| 210000–2101FF | DOC registers | native reg n → base+2n、上位byte |
| 220000–22001F | DSP bridge | mailbox / FIFO / control / status |
| 230000–23001F | ADPCM bridge | FIFO / clock / reset |
| 240000–24003F | Temporal / Capture | 独立R/Wポインタ・状態 |
| 250000–250FFF | Data Fabric | DMA・IRQ・経路・bend |
| 260000–2600FF | Controller / MIDI / matrix | 入力・ACIA・クロック・マトリクス |
| 270000–27003F | Palette | 16色RGB各4-bit（暫定） |
| 280000–281FFF | DSP program window | 4K×16、DSP reset中のみ書込可 |
| 400000–4FFFFF | Cartridge ROM window | 最大1 MiB窓、初期128 KiB想定 |
| その他 | 予約 | 将来拡張 |

| ローカルメモリ | 容量 | 構成・アクセス |
|---|---:|---|
| ACRTC VRAM | 128 KiB | 16-bit word。表示・描画・DMAは調停。CPUからはACRTC経由 |
| DOC wave RAM | 64 KiB | 8-bit。DOCの読出し位相を優先 |
| DSP program RAM | 8 KiB | 4096×16。外部プログラムメモリ |
| DSP internal RAM | 288 B | 144×16。CPU直接アクセス不可 |
| Temporal RAM | 64 KiB | 独立R/Wカウンタ。リング／同期リセット |
| Capture RAM [O] | 64 KiB | 入力専用、フレーム完了後に所有権を移す |

メモリの物理実装はDRAM/SRAM/ROMとリフレッシュ・バッファを組み合わせる。256 Kbit級DRAMやEPROMは1985年資料に掲載例がある [12]。実際の個数、速度品、価格、基板面積は未確定。DSPプログラムRAMは100 ns級の要求を別途確認する。

<div class="pagebreak"></div>

### 06　Data Fabric：独自転送・接続回路 [P]

通常動作は有限帯域のDMA／FIFOコントローラ。初期は2チャネル実装（アドレス窓は4チャネル分を予約）。分岐は複数転送、フィードバックはRAMまたはFIFOを介す。ゼロ時間の循環接続は禁止。CPU、映像、DOC、DSPのメモリバスを無制約に直結しない。

| ID | ポート | 方向・アクセス条件 |
|---|---|---|
| 0 | Staging RAM | R/W、byte address、16-bit対応 |
| 1 | ACRTC VRAM stream | R/W、native RD/WT等のコマンドとFIFO経由。転送開始前にポインタを設定し、逐次word転送。初期は任意stride不可 |
| 2 | DOC Wave RAM | R/W、byte address。DOC非使用位相でアクセス |
| 3 / 4 | DSP input / output FIFO | W / R、16-bit。32 wordsずつ暫定 |
| 5 | ADPCM input FIFO | W、8-bit packed、16 bytes暫定 |
| 6 / 7 | Temporal write / read | W / R、8-bit逐次、独立ポインタ |
| 8 | Capture RAM | R、byte address、完了バッファのみ |
| 9 | Character ROM | R、byte address、書込み不可 |
| 10 | Audio input FIFO | R、8-bitサンプル（拡張基板） |
| 11–15 | 予約 | 拡張モジュール |

#### チャネルレジスタ（0x250000 + ch×0x40）

16-bit big-endian。32-bit値は上位word→下位word。STARTで設定一式をラッチ。チャネル動作中のレジスタ変更は次回STARTから反映する。

| Offset (hex) | 名称 | 定義 |
|---|---|---|
| 00 / 02 | SRC / DST | ポートID（低4-bit） |
| 04 / 08 | SRC_ADDR / DST_ADDR | 32-bit byte address（逐次ポートは無視） |
| 0C | COUNT | 16-bit入力要素数。0は転送なし |
| 0E / 10 | SRC_STEP / DST_STEP | signed 16-bit byte stride。RAMポートのみ |
| 12 | FORMAT | 0=8→8、1=16→16、2=8→16 zero extend、3=16→8 low byte、4=16→8 high byte。未対応はERROR |
| 14 / 16 | MASK / XOR | データ変換後に AND → XOR（16-bit）。初期値 FFFF / 0000 |
| 18 | SHIFT | signed 5-bit（-16〜15）、論理シフト。出力幅で切捨て |
| 1A | TRIGGER | 0=即時、1=VBlank、2=HBlank、3=audio tick、4=external trigger |
| 1C / 1E | CTRL / STATUS | START/STOP/LOOP/IRQ_EN、BUSY/DONE/STALL/ERROR。終了フラグW1C |
| 20–2A | 予約 | 後続版の2D転送・アドレス変換等 |

0x250100以降：グローバルIRQ、チャネル状態、ポートリセット、クロック制御を予約。初期のベンディングは8/16-bitデータ変換、RAM低位アドレスマスク・XOR、逐次ポインタリセット。アドレス変換はエミュレータの拡張設定として開始し、実機レジスタ割当は次版で定義する。

FIFO full/emptyでは待機し、データを勝手に生成しない。STOPで未コミットの転送を破棄。LOOPは元のアドレス・個数で再開し、同期トリガ設定時は次のトリガを待つ。タイムアウトを計測し、DMA異常がCPU・ホスト全体の停止に波及しないようにする。

<div class="pagebreak"></div>

### 07　クロック・帯域・映像・DSP [P/V]

| クロック領域 | 初期値・方針 |
|---|---|
| CPU / Fabric / ACRTC | 各8 MHz候補。同期・待ち時間は別モデル |
| DSP | 20 MHz入力級、200 ns命令サイクル級。外部プログラムメモリ速度を検証 |
| DOC / ADPCM | DOC 約7 MHz候補、MSM5205 384 kHz基準、出力8 kHz |
| Video | NTSC 525/59.94i目標。内部更新59.94 Hz目標。走査タイミングのカウンタ・分周設計は未確定 |
| Host | PC音声48 kHz（現代側のみ）。整数または有理数時刻で各クロックを独立管理 |

Fabricの理論予算：16-bit幅、8 MHz、1要素の読み書き計4クロック、待ちなしで最大4 MB/s。利用率60%仮定で2.4 MB/s。ポート幅、ACRTC FIFO待機、DRAMリフレッシュ等を含まない上限試算であり、実効帯域を保証しない。

| 処理 | 転送量の目安 |
|---|---:|
| 256×192、4bpp 1画面 | 24,576 B（24 KiB） |
| 同60 Hz片方向／往復 | 約1.47 / 2.95 MB/s |
| 同30 Hz片方向／往復 | 約0.74 / 1.47 MB/s |
| 128×96、8-bit、15 Hz捕捉 | 約0.18 MB/s |
| ADPCM 4-bit、8 kHz | 4 kB/s |
| 生CVBS 8-bit、4fsc | 約14.32 MB/s（専用捕捉経路が必要） |

#### 映像基本仕様

256×192、4bpp packed（16色）を初期標準。16-bit word内の画素順はACRTCネイティブ仕様に従う。VRAM 128 KiBに24 KiBの画面2面と素材を格納。8bppは同解像度での拡張候補。1行32字×24行の8×8フォント表示に対応する。RGB各4-bitパレット、NTSCコンポジットとRGB出力を想定。外付けラッチ・パレット・DAC・エンコーダのタイミングは別設計。スプライト専用ハードは前提にせず、描画／COPYと背景復元でゲームを構成する。

HD63484の1984年資料にある直線描画速度は8 MHz時に1画素0.5 µs級。全画面描画を60 Hzで更新する性能は保証されない。ローカルコピー、部分更新、低頻度の効果を活用する [2]。

#### DSPと時間軸

TMS32010は専用FIRチップではない。16-bit固定小数点のDSPプログラムでFIR・相関・波形処理を実装し、外部I/O転送時間を含めて評価する。内部RAMが小さいため行・ブロック単位とし、全画面のリアルタイム畳み込みは初期要件にしない。DSP I/Oは port0 入力、port1 出力、port2 状態／コマンド等のアダプタを設計し、BIO等で空き・到着を通知。実命令のI/Oタイミングは原典を照合する。

Temporal RAMは後年のフィールドメモリICではなくDRAM/SRAM＋独立R/Wカウンタ・調停で実現。初期は8-bit、64 KiBのリング。未書込読出し、同番地競合、オーバーランの動作を定義する。フィールド保持、繰返し、遅延、読み書きクロック差を操作可能にする。

<div class="pagebreak"></div>

### 08　漢字ROM・文字コードの不整合 [P]

既存「試作機の文字ROM フォントの試み v0.1」を継承する。32 KiB（256 Kbit）に8×8ドット字形を格納し、7×7を字形本体、右・下1ドットを余白とする。第一水準2965字に加え非漢字・英数かなの領域を持つ。全字形は未完成であり、現状30字の漢字試作を起点とする。以下は既存資料のROM内オフセットを維持した設計。

| ROM内offset (hex) | 内容 | 容量 |
|---|---|---:|
| 0000–07FF | 英数かな 256字枠 | 2,048 B |
| 0800–1F7F | 非漢字1〜8区（752枠） | 6,016 B |
| 1F80–7D7F | 第一水準16〜47区（3008枠、空き枠含む） | 24,064 B |
| 7D80–7FFF | 予約 | 640 B |
| 合計 | 32 KiB | 32,768 B |

区点→第一水準字形offset = 0x1F80 + ((区−16)×94 + (点−1))×8。字形は1行1 byte、MSBが左。空き枠・未作成字は診断用プレースホルダと区別し、ROM生成時に状態を記録する。CPUの字形アドレスは0x010000+offset。

旧JIS78の字形ROMを最終試作機の基準とし、新JIS83の配置・字形は差分ROMイメージとして保存する。双方の切替はエミュレータのROM差替え／仮想基板改造として提供し、1985年型に両方のROMが常時搭載されていたとはしない。実際の改正範囲・空き区点と字形は一次資料により再確認する。

| 実験 | 実装 |
|---|---|
| 正常表示 | BIOSが区点を字形に変換し、ACRTCへビット展開・描画転送 |
| 規格不一致 | 同一の区点列にJIS78/83の別ROMを適用。字形差・位置入替を観察 |
| Address bending | ROM参照アドレスの低位線固定・XOR。範囲外は定義済みopen bus等 |
| Data bending | 行データのビット反転・並べ替え・断線モデル |
| 字形の音響化 | Character ROM → DOC wave RAMまたはADPCM FIFO。ゼロ値で停止するDOC固有挙動も利用 |
| 字形の再描画 | Character ROM → staging → DSP/ACRTC。加工後はRAMに保存（ROMは書き換えない） |

8×8全漢字はテレビ上の可読性が保証されない。フォントROMの容量制約と読めなさを機械の特性として扱う。高解像度字形・別規格は後継機または拡張候補。フォントのライセンス・由来・作成過程を管理し、他社ROMを配布物に含めない。

<div class="pagebreak"></div>

### 09　外部入力・コントローラ拡張・HCI [P]

MIDIは1983年に規格化され、1985年のAtari STにも内蔵端子の例がある [10][11]。本機の特徴はMIDIそのものではなく、外部機器がゲーム・創作・機械制御の共通入力になり得ること。1985年型の標準コントローラを残しつつ、将来のHCI設計に備えて入力の意味を固定しない。

| 端子／経路 | 1985年機の設計目標 | 現代PC／実機 |
|---|---|---|
| Controller A/B | 方向・ボタン等のデジタル状態、汎用アナログ値。端子ピン・物理形状は保留 | キーボード、ゲームパッド、MIDI等を割当 |
| MIDI IN/OUT | 5-pin DIN、31.25 kbit/s、8N1。MC6850系＋絶縁入力。THRU候補 | PC MIDI、USB-MIDI変換（RP2350等） |
| Expansion | 16-bitホストバスの外部アクセス、IRQ、trigger候補。ピン割当・バッファ未定 | FPGA／マイコンブリッジ |
| Audio IN/OUT | レベル調整付きアナログ入出力、ADC/DAC、ミキサ | Audio interface、外部モジュラー |
| Video IN/OUT | NTSCコンポジット、75 Ω。入力基板の詳細は次頁 | ADV7180/ADV7391等、PCキャプチャ |
| CV/Gate | [O] アナログ制御端子、外部trigger、ADC入力。電圧範囲未確定 | RP2350＋保護・レベル変換・必要な外付けADC/DAC |

#### 入力の論理仕様（初期BIOS案）

| 0x260000からのoffset | レジスタ／用途 |
|---|---|
| 00 / 02 | PORT_A / PORT_B：16-bitデジタル状態（active-high論理値） |
| 04–12 | AXIS0–7：8個の16-bitレジスタ。初期は8-bitアナログ値を下位に格納 |
| 14 / 16 | MIDI_STATUS / MIDI_DATA：ACIAの制御・状態、送受信データ、上位byte lane |
| 18 | INPUT_IRQ_STATUS：入力変化、受信、外部trigger。W1C、MIDIはACIA側も処理 |
| 1A | INPUT_IRQ_ENABLE：イベント割込みの許可 |
| 20–3E | 予約：アナログマトリクス、外部trigger／拡張 |

BIOS/SDKではデジタル状態、連続値、タイムスタンプ付きイベント（MIDI Note/CC/Clock等）を抽象化する。入力の意味付けはゲーム／制作ソフトが行う。MIDIバイト列はネイティブACIAを通し、エミュレータのホストイベントはシリアル転送時間も再現する。3-byteメッセージは約0.96 ms（連続送信時、待ちなし）。高頻度の連続制御は帯域制限を考慮する。

外部機器からのゲーム入力と、DMA起動・音源・描画パラメータへの割当はソフトウェアで設定する。通常モードではMIDIから任意バスへ直接書き込ませない。仮想ベンディングの直接介入は別機能とする。将来の新しいコントローラ、操作体系、物理パッチ、光学・身体入力を検討する際もこの境界を維持する。

<div class="pagebreak"></div>

### 10　映像・音響・アナログ回路 [P/O]

#### 外部映像入力

標準出力はNTSCコンポジット。外部映像入力は重要な拡張機能として第一世代から想定する。最初の1985年モデルは専用入力基板による輝度取得を優先し、128×96、8-bit、15 Hz相当を暫定目標とする（間引き・同期・帯域の成立性は未検証）。色復号付き取り込みは次段階。時代相当の生波形取得候補はTRW TDC1007等 [9]。これだけでは映像デコーダにならない。

<div class="arch compact">
<div class="node">CVBS IN → 75 Ω終端・クランプ・同期分離・帯域制限</div>
<div class="connector">↓</div>
<div class="triple"><div class="node">1985年案<br>ADC＋輝度・間引き</div><div class="node">現代版<br>ADV7180 復号</div><div class="node">代替／追加<br>AD9280 生波形</div></div>
<div class="connector">↓ 独立捕捉経路・FIFO・所有権切替 ↓</div>
<div class="node accent">Capture RAM → Data Fabric → VRAM / DSP / DOC / ADPCM</div>
</div>

ADV7180は復号済み映像、AD9280はADC（生波形を取得し外部で復号）であり、直接互換ではない。両方を必須にしない。ADV7391は現代PCBの出力候補。1985年型はRGB/輝度・色差、色副搬送波、同期合成、75 Ω出力回路を別途設計する。[13]

コンポジット信号モデルは水平・垂直同期、ブランキング、輝度、色副搬送波（NTSC約3.58 MHz）、バースト、帯域制限を含む。4fsc（約14.32 MS/s）を基準にした生成・復号モデルをPC上で段階実装。復号済みPCカメラと生波形は区別する。外部映像フィードバックは仮想波形または実キャプチャから形成する。

#### 音響回路・信号ルーティング

ES5503の内蔵DAC・チャンネル選択をモデル化し、外部サンプル保持、ミキサ、LPF、出力アンプの回路を定義する。初期ステレオ出力。MSM5205の復号状態、DAC、LPFも再現する。CD22101系は低帯域の音声・CV・変調信号を優先し、コンポジット全帯域の通過は保証しない。映像の高周波経路は専用バッファ・スイッチを別設計する。

複数の低インピーダンス出力をスイッチで直結しない。加算抵抗、バッファ、クランプ、DC遮断、電圧範囲を設計する。実機の電気的故障と仮想的な故障モデルは区別する。CV/Gateの現代拡張も電圧・閾値・過電圧保護を確認する。

<div class="pagebreak"></div>

### 11　A/V処理とベンディング [P]

| 実験パッチ | 実際のデータ経路と表現 |
|---|---|
| Graphics as wave | ACRTC VRAM → staging → DOC wave RAM。図形描画・論理演算が波形を変形 |
| Image as ADPCM | 4-bit画素列 → MSM5205。前の復号状態に依存して音が変化。ニブル順・入力切替・欠落・resetを操作 |
| DSP shared processing | VRAM/Capture/DOC → DSP FIFO → DSPプログラム → RAM。波形・画像に同一の固定小数点演算を適用 |
| Temporal feedback | VRAM/Capture → Temporal RAM → staging/ACRTC。読書きポインタ・クロック差で履歴を変形 |
| Sound-driven graphics | 音声ADCまたは波形RAM → DSP/CPU → 描画命令・コピー・パレット制御 |
| Glyph as wave | 文字ROM → DOC/ADPCM、またはDSPを経由して再描画 |
| Composite feedback | 映像出力 → 内外の信号回路 → 入力。同期・色・帯域の変質を再入力 |
| Delta update [O] | 前フレームの領域コピー＋部分差分更新。参照画像・更新命令の破損によるモッシュ的表現 |

差分更新はACRTCのコピー、CPUプログラム、独自回路を使う案。1985年の専用動画圧縮ICを搭載したことにはしない。動画コーデックの参照フレーム／動き補償を改変する厳密なデータモッシングは後継機または独自の予測符号化拡張に残す。

| 種類 | 初期操作 | 忠実度の考え方 |
|---|---|---|
| Data | XOR、マスク、ニブル交換、字形・波形破損 | 実データを変更し後段で解釈 |
| Logic | アドレス線交換・固定、制御信号の欠落 | 仮想信号線とアクセス結果を変更 |
| Timing | 分周、クロック欠落、リセット時刻 | 各ICの状態を維持しイベントを変形 |
| Circuit | 信号経路、ゲイン、フィルタ、帰還 | デジタル・アナログモデルを分離 |
| Codec | ADPCM復号状態、入力欠落、データ境界 | 正常系と仕様外挙動を区別 |

通常モードとBendモードを分ける。実ICの外部ピンで可能な操作、専用グルーロジックで可能な操作、エミュレータ限定の内部状態介入を区別する。仕様外動作は近似と明記し、実測可能な場合に更新する。

ライブ運用：モジュール単位reset、DMA中止、パッチundo、状態snapshot、有限バス待機。仮想CPUや回路が停止することは許容するが、PCアプリは停止させない。ホスト表示保持・音声リミッタ・DC除去・安全なバイパスは仮想機の挙動を変更しない別層とする。

<div class="pagebreak"></div>

### 12　エミュレータ開発・検証計画

C++20等の独立コア＋SDL等のホスト入出力を候補とする。MAME等の既存実装（68000、HD63484、TMS32010系、ES5503、MSM5205、ACIA）を参照する。移植前に各ファイルのライセンスと依存性、命令・タイミング精度を確認する [14]。既存コアを使うことと、基板の回路やベンディングを再現することは別の課題である。

| 層 | 主な実装 |
|---|---|
| core | 時刻・イベント、バス、信号線、IRQ、trace、snapshot |
| chips | CPU、ACRTC、DSP、DOC、ADPCM、ACIA、マトリクス、RAM |
| board | アドレスマップ、Data Fabric、ポートアダプタ、クロック、文字ROM、映像・音響経路 |
| host | PC表示・音声、MIDI/OSC、外部映像、仮想パッチ、操作UI、復帰 |
| SDK | m68k assembler/compiler、リンカ、ROM builder、DSP assembler、MMIOヘッダ、BIOS ABI |
| tests | 単体試験、独立参照モデル、ゴールデンデータ、イベントtrace、帯域計測 |

| 段階 | 成果物／合格条件（暫定） |
|---|---|
| 0　仕様・資料 | 各ICのpin/timing/register台帳、ROMとテストの権利確認。実装上の未確定項目を記録 |
| 1　Chip sandbox | HD63484描画・COPY・FIFO、DSP乗算/FIR/外部I/O、DOC発音と停止、ADPCM既知列を単体試験 |
| 2　Data Fabric | 2 DMAチャネル、待機、IRQ、変換、FIFO、競合、周期トリガ。traceで順序と所要時間を確認 |
| 3　Console | 自作ROM起動、入力、ゲーム描画、文字ROM、PCM/ADPCM音声。256×192、4bppのデモ |
| 4　AV / HCI | 異種データパッチ4種以上、外部映像・MIDI、クロック・ROMベンディング、複数入力割当。30分の連続試験 |
| 5　PCB / 拡張 | HDL独立モデル、合成・回路検証、実ICの波形測定、現代I/O基板 |

正常系はビット列一致（V2）と独立実装比較（V3）を目指す。描画FIFO・描画処理時間、DSP命令、DOC波形読み出し時刻を順次詳細化。アナログは既知入力に対するゲイン・周波数応答・同期・ノイズを測定（V4）。全ゲート／トランジスタ再現を初期条件にしない。PC上で59.94 Hz出力・48 kHz音声の実時間動作を目標とし、負荷はプロファイルで評価する。

**デザインへ戻る条件**：入力・音響・描画・パッチが動いた時点で、標準コントローラに依存しない新しいHCIを小規模に試す。入力ポート・イベント・拡張バスの要件に影響するならPhase 2以前でも設計を見直す。筐体や外観の完成は必要条件にしない。

<div class="pagebreak"></div>

### 13　将来のPCB化・未確定事項

| 部品／機能 | 現代版の実装候補 |
|---|---|
| MC68000 / HD63484 / TMS32010 | 旧在庫、互換IC、FPGA。IA63484は機能互換資料のある候補だが入手性は未確認 |
| ES5503 / MSM5205 | 実IC、FPGA、RP2350等。DAC・固有挙動を活用する場合は実IC優先 |
| Data Fabric / Temporal RAM | FPGA、SRAM/SDRAM等。1985年コア仕様と現代実装を区別 |
| CD22101 | 実ICまたはCH446Q等の機能代替。マトリクス規模・電源・制御差を吸収（ピン互換ではない） |
| ADV7180 / ADV7391 | 現代の映像I/O候補。LCSC掲載例、AliExpress等も候補 |
| AD9280 | 代替または追加の高速ADC。単独では色復号不可 |
| RP2350 | MIDI、CV/Gate、USB、設定・外部I/O |

前版の調達メモ：LCSC CH446Q C109471、ADV7180 C11164/C395114、ADV7391 C443874、AD9280 C44935。これらは過去の掲載情報であり、現在の在庫、価格、JLCPCB PCBA可否や真正性を保証しない。旧世代ICすべての互換品がLCSC/AliExpressにあるかは未確認。

DIP/SOP/LQFP等の手はんだ、QFN/LFCSP/BGAのPCBA・リフロー、変換基板を使い分ける。5 V系と3.3/1.8 V系のレベル変換、電源・熱設計、75 Ω映像終端、アナログリターン経路を設計する。全ICをFPGAに集約することは必須とせず、合成結果・実測・費用で判断する。

#### 次版の優先事項 [V]

1. HD63484の速度品・レジスタ／描画命令・VRAMシーケンス・DMAハンドシェイク・外部表示回路の精査。特にData Fabricからの連続RD/WT転送と描画との競合を確認する。
2. TMS32010のI/Oポート・BIO/割込み、外部プログラムRAMのロード回路、サイクル単位のDSP演算予算を確認する。
3. ES5503の1985年採用品・供給条件、波形RAMアクセス位相、DAC／外部多重化、音源出力のタイミングを検証する。
4. 1985年映像入力基板のADC、間引き・同期・RAMアクセス、出力のDAC／エンコーダ候補を選定し、時代整合性を確認する。
5. Data FabricのTTL/PAL/ゲートアレイ規模と帯域を見積もる。IRQ・port status・clock registerの詳細と初期ROMフォーマットを確定する。
6. コントローラ物理仕様、拡張端子、BIOS ABI、漢字ROMの全字形とJIS改正差分を確定する。HCI試作結果に応じて修正する。

本書は上記の未確定事項を残した開発開始用仕様。1985年当時の量産性、総BOM原価、リアルタイム性能を検証済みとするものではない。

<div class="pagebreak"></div>

### 14　参照資料

2026.11.24参照。1985年以前の資料を年代の根拠とし、後年の資料はIC復元・実装参考に限定する。原典の全ピン／タイミング表は未精査。リンク先はアーカイブを含む。

| ID | 資料・確認範囲 |
|---|---|
| [1] | Motorola, M68000 User's Manual, bus operation. [バス仕様PDF](https://datasheets.chipdb.org/Motorola/68000/MC68000UM.9.pdf)（前版参照、後年版） |
| [2] | 日立評論 Vol.66 No.7 (1984), pp.43–48, HD63484 ACRTC。表1、図1–3。[原典PDF](https://www.hitachihyoron.com/jp/pdf/1984/07/1984_07_10.pdf) |
| [3] | Hitachi, HD63484 Application Note (1986)。[アーカイブ](https://bitsavers.org/components/hitachi/_dataBooks/U90_Hitachi_HD63484_ACRTC_Advanced_CRT_Controller_Application_Note_198604.pdf)、[データシート](https://mega-micros.co.uk/datasheets/datasheet%20-%2063484%20%28hitachi%29%20acrtc.pdf)。AR/STATUS・FIFO・表示回路の復元用 |
| [4] | TI, TMS32010 User's Guide, Rev.B (1985-03), program memory / I/O。[原典PDF](https://bitsavers.computerhistory.org/components/ti/TMS320xx/3201x/1983_TI_TMS32010_Users_Guide_Digital_Signal_Processor_Products_Revision_B_198503.pdf)。テキスト確認、画像取得制限あり。[1983 Assembly Guide](https://www.bitsavers.org/components/ti/TMS320xx/3201x/1983_TMS32010_Assembly_Language_Programmers_Guide.pdf) |
| [5] | Apple Computer, Ensoniq DOC ERS Rev.1.0 (1986-06-25), pp.1–2、oscillator/register descriptions。[技術資料PDF](https://www.brutaldeluxe.fr/documentation/cortland/v4_13_EnsoniqDOC.pdf)（後年資料） |
| [6] | Ensoniq Mirageの技術資料アーカイブとMAME機種実装。[資料一覧](https://zine.r-massive.com/ensoniq-technical-documents-and-schematics/) / [Mirage実装](https://github.com/mamedev/mame/blob/master/src/mame/ensoniq/enmirage.cpp)。採用実績・復元参考 |
| [7] | OKI Voice Synthesis ICs (1994), MSM5205。[データブック](https://www.bitsavers.org/components/oki/_dataBooks/1994_OKI_Voice_Synthesis_ICs.pdf)（前版参照、後年資料。初期品の差異は要調査） |
| [8] | RCA COS/MOS Integrated Circuits (1980), CD22101。[データブック複製](https://manuals.plus/m/e55f9cfbf186e76de7d6ee64653ba60cdefe735fa4ceb15663fd0e3555f463b5) / [後年データシート](https://dtsheet.com/doc/151567/intersil-cd22102e) |
| [9] | TRW VLSI Data Book (1985), TDC1007。[原典PDF](https://bitsavers.trailing-edge.com/components/trw/_dataBooks/1985_TRW_VLSI_Data_Book.pdf) / [1984資料](https://w140.com/tekwiki/wiki/File%3ATRW_TDC1007_Data_Sheet.pdf) |
| [10] | [MIDI Association: 1983–1985](https://midi.org/midi-history-chapter-7-midi-associations-1983-1985)、[通信形式](https://www.cs.cmu.edu/~music/cmp/archives/cmsip/readings/davids-midi-spec.htm)、[Motorola 1975 applications manual](https://www.bitsavers.org/components/motorola/6800/M6800_Microprocessor_Applications_Manual_1975.pdf) |
| [11] | International Musician & Recording World (Nov. 1985), [Atari 520ST MIDI Computer](https://www.muzines.co.uk/articles/atari-520st-midi-computer/5101) |
| [12] | Maplin Electronics (June 1985), [27256/41256掲載号](https://www.worldradiohistory.com/UK/Mapelin/Maplin-Electronics-1985-06-08.pdf) |
| [13] | 現代I/O資料：ADI [ADV7180](https://www.analog.com/en/products/adv7180.html)、[ADV7391](https://www.analog.com/en/products/adv7391.html)、[AD9280](https://www.analog.com/en/products/ad9280.html)、[RP2350](https://pip.raspberrypi.com/categories/1214-rp2350)、[CH446Q](https://www.lcsc.com/product-detail/C109471.html) |
| [14] | MAME参考実装：[HD63484](https://github.com/mamedev/mame/blob/master/src/devices/video/hd63484.cpp)、[ES5503](https://github.com/mamedev/mame/blob/master/src/devices/sound/es5503.cpp)、[MSM5205](https://github.com/mamedev/mame/blob/master/src/devices/sound/msm5205.cpp)、[MAMEリポジトリ](https://github.com/mamedev/mame)。個別の精度・ライセンスは要確認 |

内部資料：Fictional Machines 基本構想・分類体系 v0.6、先行事例と制作案 v0.1、試作機の文字ROM フォントの試み v0.1、旧ハードウェア設計仕様書 v0.1（1990年前後版）、第一世代基本構想およびDesign Manifesto 1984 暫定案。
