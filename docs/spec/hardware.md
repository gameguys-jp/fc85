# Fictional Console / 1985
## 第一世代 ハードウェア設計仕様書 v0.2

Fictional Machines / GAME GUYS ｜ 2026.11.25 ｜ WORKING SPECIFICATION

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
| 部品選定 | 1985年の日本の中堅業務用電子機器メーカーとして自然な部品を優先し、現代の入手性は再現側で補う [P]（DR-012）。再現では、同等なら入手性、小型、低価格、PCBA対応を優先。電気特性が表現に重要な箇所は実ICを優先 |

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

日本の中堅業務用電子機器メーカーが、業務用の文字表示・映像・通信の技術を家庭向けに転用し、1985年に最終試作機を完成させた。機能を盛り込みすぎて家庭用の採算が合わず、音源IC（ES5503）の量産供給も確保できずに量産を断念した、という架空の開発史を前提とする [P]。開発史、メーカーの気質、JIS改正の扱い、確定しない項目は [story/main.md](../story/main.md) を正とし、物語が設計に効いた点は [decisions.md](../decisions.md) に決定として記録する。

#### 機械に反映する設計思想 [P]

- ゲームだけでなく、媒体を作成・変換・交換する装置とする。
- 標準のゲーム入力を残しつつ、外部楽器、ノブ、スイッチ、センサー等を入力として利用できる。
- 内蔵BIOSに文字描画、入出力、転送の共通機能を設ける。カートリッジなしでも診断・基本操作ができることを目標とする。
- HCIの再検討時に、入力端子やイベント仕様を変更できるよう、コントローラとゲームの意味づけを分離する。

<div class="pagebreak"></div>

### 03　システムアーキテクチャ [P]

<div class="arch">
<div class="node primary">HD68000-8 / Boot ROM / Main RAM / Cartridge</div>
<div class="connector">16-bit host bus：レジスタ・IRQ・DMA制御</div>
<div class="triple"><div class="node">HD63484-8<br>Graphics / VRAM</div><div class="node">TMS32010<br>DSP / Program RAM</div><div class="node">ES5503（子基板）<br>Wave RAM / DAC</div></div>
<div class="connector">各ICのネイティブポート・ラッチ・調停回路</div>
<div class="node accent">Data Fabric — 16-bit DMA・FIFO・アドレス／データ変換</div>
<div class="connector">独立ポート・逐次転送・同期イベント</div>
<div class="triple"><div class="node">Character ROM<br>JIS78 / JIS83</div><div class="node">Temporal RAM<br>Capture / Delay</div><div class="node">MSM5205<br>ADPCM FIFO</div></div>
<div class="connector">D/A・A/D・同期・バッファ回路</div>
<div class="node">CD22101系アナログマトリクス / Audio & Video signal path</div>
<div class="triple"><div class="node">CVBS IN / OUT<br>映像拡張基板</div><div class="node">Audio IN / OUT<br>Mix / Filter</div><div class="node">Controller / MIDI<br>拡張バス</div></div>
</div>

図1　機能構成。全ICが同一RAMへ直接アクセスできることを意味しない。Data Fabricのポートごとに物理的な接続・アクセス制約をモデル化する。外部映像入力は第一世代の機能目標だが、基板分離を許容する。ES5503は子基板に載せ、外しても動作する構成とする [P]（DR-011）。

| 区分 | 構成 |
|---|---|
| 基本 | CPU、ACRTC、DSP、DOC（子基板）[P]（DR-011）、ADPCM、漢字ROM、Data Fabric、時間処理RAM、MIDI・汎用入力 |
| 基本／拡張 | CD22101系マトリクス、コンポジット入力、音声入力、通信・外部バス |
| 現代実装 | ADV7180 / ADV7391、AD9280（代替・追加候補）、CD22M3494・CH446Q [O]（§13）、RP2350、FPGA。1985年の搭載ICとはしない |
| 第二世代へ保留 | TMS34010、IMS A110、μPD42280、専用動き推定・JPEG回路 |

<div class="pagebreak"></div>

### 04　実在ICの参照仕様と適用

| IC | [D] 参照仕様 | [P/V] 本機の使い方 |
|---|---|---|
| HD68000-8 [1][25][26][31] | 16-bit外部データ、24-bitアドレス、非同期バス、DTACK、バス調停信号。日立HD68000-8はMC68000L8と同等品として1985年の日立データブックに載り、同年の国産基板に刻印例がある [D]（[25] §5、[26] §1） | 8 MHz（16 MHz÷2）[P]（DR-003）。16 MiB空間。ROM起動、周辺待ち時間、バスエラー。日立品を使う [P]（DR-008） |
| HD63484-8 [2][3][15][16] | 1984年の資料に8 MHz品、2 MiBグラフィックメモリ空間、コマンド38種（うち図形描画23種）、画面分割・外部同期・DMA。16-bitホストとローカルバス。-8品の2CLK周期は125 ns以上。1985年8月付けの-8品データシートと、同年の米国製ボードの採用例がある [D]（[15] §1、[16] §1–2） | 2CLK 5.369318 MHz（21.47727 MHz÷4、周期186 ns）[P]（DR-004）。1985年後半の量産初期品を使い、-4／-6品は使わない [P]（DR-004）。128 KiB VRAM。AR/STATUSとデータポート。FIFOを介した描画・転送。ローカルVRAMをCPUに直結しない |
| TMS32010 [4][17] | 16-bit固定小数点DSP、16×16乗算、32-bit演算器、144ワード内部データRAM、4Kワード外部プログラム、入力8・出力8ポート。200 ns命令サイクル級 | 20 MHz入力（専用の発振源）[P]（DR-003）。外部クロックをジャンパで選べる [P]（DR-035）／5 M命令サイクル毎秒級。外部8 KiB（4K×16）プログラムRAMを停止中にロード。分離回路で68000から切り替え、ロードは約6 ms [P]（DR-029）。サイクル数はTIの資料を正とし、MAMEとの差は既知の差として扱う [P]（DR-032）。I/Oラッチ経由で処理。速度グレード確認。国産の代替候補は富士通MB8764 [O]（DR-007、[27] §1） |
| ES5503 [5][6][18][19] | 32オシレータ、64 KiB直接波形アドレス、周波数・音量・波形位置等の制御、時分割波形アクセスと内蔵DAC。入力クロックの周期は100〜125 ns（8〜10 MHz）[D]（[18] §3.1） | 8 MHz（既存の16 MHz÷2を流用）[P]（DR-003、DR-011）。DOC、64 KiB波形RAM、アダプタ、出力アンプを子基板に載せる [P]（DR-011）。音量DACの基準−5 Vは子基板で−12 Vから作る [P]（DR-020）。CLKは子基板のジャンパで外部入力も選べる [P]（DR-035）。子基板がないときは本体のDACでDSP合成の音を出す [P]（DR-019、§06 port 2、§10）。初期16有効オシレータを目標。CPU/Fabricのアクセスは、DOCサイクルごとに来るホスト側の位相（E High）に限る [D]（[18]、§06 port 2）。入手は試作用の少量サンプルという架空の設定 [P]（DR-006、[音源の差し替え](../story/sound-redesign.md)） |
| MSM5205 [7][20] | 3/4-bit ADPCM、384 kHz基準で4/6/8 kHz、DACと外付けフィルタ。内部12-bit、10-bit DAC。予測値が範囲を超えると回り込む（+5 V→0 V）[D]（[20] §4.4） | 4-bit、8 kHz。外部FIFOからニブルを上位から供給 [P]（DR-017）。復号履歴とリセットをモデル化。エミュレータは回り込み（12-bitのmod 4096）を標準とし、クリップ（MAME、jt5205）は比較用 [P]（DR-021） |
| CD22101 [8][21] | 4×4×2アナログクロスポイントスイッチ。1980年RCA資料に掲載。1983年RCA資料では正式品として載る [D]（[27] §5） | CD22101を維持する [P]（DR-009）。初期は1個相当の経路モデル。必要なら複数個。電圧、帯域、RON、制御ピンは実装前に照合 |
| TDC1048 [9][22] | 1984・1985年TRW資料の8-bit、20 MSPSフラッシュADC。28ピン、+5／−5.2 V、1.6 W。用途に「Low-Cost Video Digitizing」[D]（[22]、[27] §2） | 映像入力基板の輝度取り込み用 [P]（DR-009）。同期分離、クランプ、色復号、RAM書込みは外部回路。TDC1007（64ピン、2.7 W、−6 V）は採らない [P]（DR-009）。−5.2 Vは映像入力基板で−12 Vから作る [P]（DR-020） |
| HD6850 [10][23][26] | 非同期シリアルI/O、制御・状態・データレジスタ。MC6850と互換の国産品として1985年の日立データブックに載る [D]（[26] §2） | MIDI IN/OUT用 [P]（DR-008）。68000とはVPA同期でつなぎ、1アクセス10〜19クロック [P]（DR-028、[31] §2.5）。31.25 kbit/s（500 kHz＝16 MHz÷32）[P]（DR-003）。オプトアイソレータ等は別設計 |
| BA7230LS [27] | ロームのRGB→NTSCエンコーダ。+5 V単一、SZIP-24。映像入力との高速切替（スーパーインポーズ）を持つ。1984年9月の雑誌記事と、同年10月・12月のローム自身の広告に載る [D]（[29] §1。誌面の画像は未確認 [V]）。1985年10月発売の国産家庭用ゲーム機が採用（後年の解析資料による）[D]（[27] §3） | NTSCエンコーダ [P]（DR-009、DR-015）。色副搬送波は21.47727 MHz÷6をVC端子に入れる [P][V]（DR-015、[29] §2.5–2.6）。入れられない場合は内蔵発振のまま使う。MC1377（1983年Motorola資料、+12 V、外部副搬送波入力あり）は代替候補 [O]。本体では映像入力との重ね合わせを使わない。ゲンロックは映像拡張基板の候補 [O]（DR-013） |

HD63484のメーカー資料の接続例には68000とDMAコントローラが含まれる。対してDOCは別のCPUバス形式を前提とするため、クロック位相・バイト幅を変換する専用アダプタを置く。DSPは外部RAMを汎用データ空間としては参照できないが、TBLR／TBLW（各3サイクル、ACCの下位12 bitで番地を指定）で外部プログラムRAMと内部データRAMの間を1語ずつ転送できる [D]（[4]、[17]）。

HD63484とES5503は他のICに置き換えない [P]（DR-001）。HD63484-8は1985年1月の米国広告、同年6月以降の採用ボード、8月付けの正式データシートがあり、1985年の部品として扱える [D]（[16] §1–2）。音源にヤマハFMは使わない。録音した音や文字・画像のデータをそのまま波形として鳴らすためで、DSPにはプログラムをRAMに読み込んで差し替えられるTMS32010を使う [P]（DR-007）。

ES5503は1985年のMirage採用例に基づくが、確認した詳細仕様書は1986年版である。1985年の個体との差異は未検証 [V]。1985年にEnsoniq以外へ供給した記録はなく、本機の入手経路は物語で扱う [P]（DR-006）。後年の資料は復元のために参照するもので、後年の機能を無条件で1985年に持ち込まない。

<div class="pagebreak"></div>

### 05　メモリ構成・ホストバス [P]

CPUアドレスはbyte単位。big-endian。リセット時の初期SP/PCはBoot ROMの先頭8 byteに配置する。16-bitレジスタは偶数番地、8-bit ICは上位バイトレーンに配置。未割当・不正幅アクセスは有限時間でバスエラー（初期実装は64 CPUサイクル以内を目標）。ACRTCのAR/STATUS・データポートには、CLR・TASなどの読み書き一体の命令を使わない。DMA中はCPUからACRTCのFIFOにアクセスしない。どちらもソフトの規約とし、BIOS・SDKの文書に書き、エミュレータで警告する [P]（DR-016）。ACRTCのFIFOにはSRのWFR・RFRを確かめてからアクセスする [P]（DR-033）。DOCの窓もMOVE.Bの1回の読みか書きだけで触り、0xE0・0xE2は読むと状態が変わるのでBIOSの決まった処理だけが読む [P]（DR-030）。規約に反するアクセスは、通常モードでは警告し、Bendモードでは実行する。実チップでの挙動がわからない部分は近似と明記する（§11）。\n\n例外ベクタ2〜255は、Main RAMのジャンプ表（0x100000＋6n、各項目は`JMP abs.L`）を指す。表はBIOSが起動時に埋め、アプリケーションはBIOSの呼出しか表の書き換えで処理を差し替える。レベル7の要因は、表を埋めるまで回路で止めておく [P]（DR-025）。\n\n割込みはすべてオートベクタ（VPA）で応える。IACKサイクルもバスエラーのタイマーの対象とし、応答のない割込みはスプリアス割込みで終わる。レベルは期限の短い順に割り当てる：6＝映像（VBlank・HBlank、ACRTCのIRQ）、5＝ACIA（MIDI。1バイト320 µs）、4＝DOC、3＝Data Fabric・ADPCM bridge、2＝DSP bridge、1＝入力（INPUT_IRQ_STATUS）、7＝予約 [P]（DR-028）。割込みの受付からジャンプ表の先の処理まで62〜71クロック（7.8〜8.9 µs）（計算、[31] §2.4）。

| CPU番地（hex） | 領域 | 用途 |
|---|---|---|
| 000000–00FFFF | 64 KiB Boot ROM | 診断、BIOS、ROM起動 |
| 010000–01FFFF | Character ROM（32 KiB） | 27256×1を上位バイトレーンに置き、64 KiBの窓の偶数番地に32 KiBを見せる。読出し専用、1ウェイト [P]（DR-031） |
| 100000–11FFFF | 128 KiB Main RAM | CPU作業領域。先頭1,536 byte（0x100000–0x1005FF）は例外のジャンプ表（BIOSのABI）[P]（DR-025） |
| 120000–12FFFF | 64 KiB AV staging RAM | DMA、DSP入出力、素材 |
| 200000–200003 | ACRTC host | +0 AR(write)/STATUS(read)、+2 DATA |
| 210000–2101FF | DOC registers | native reg n → base+2n、上位byte。子基板がないときはバスエラー [P]（DR-019） |
| 220000–22001F | DSP bridge | mailbox / FIFO / control / status。CTRLにRESET、BIOSEL[1:0] [P]（DR-029）。STATUSにUNDERRUN／OVERRUN（W1C）[P]（DR-017） |
| 230000–23001F | ADPCM bridge | FIFO / clock / reset。FIFOの残量、半分空きの割込み、UNDERRUN（W1C）[P]（DR-017）。230010–23001Fは本体DAC（子基板がないとき）のCTRL／STATUS／DATA [P]（DR-019） |
| 240000–24003F | Temporal / Capture | 独立R/Wポインタ・状態 |
| 250000–250FFF | Data Fabric | DMA・IRQ・経路・bend |
| 260000–2600FF | Controller / MIDI / matrix | 入力・ACIA・クロック・マトリクス |
| 270000–27003F | Palette | `$0RGB`の語×16（270000–27001F）、語アクセスのみ。裏と表の2面で、書込みは次のラインから反映。270020はPAL_CTRL（bit0 DIRECT：表へ直接書き、ライン途中から効く。Bend）。残りは予約 [P]（DR-027） |
| 280000–281FFF | DSP program window | 4K×16。DSPをリセットしPRGBUS＝1の間だけR/W可。それ以外はバスエラー [P]（DR-029） |
| 400000–4FFFFF | Cartridge ROM window | 最大1 MiB窓、初期128 KiB想定。既定は1ウェイト（tACC 375 ns以下のROM）。カートリッジがDTACKを返す方式は [O]（DR-026） |
| その他 | 予約 | 将来拡張 |

| ローカルメモリ | 容量 | 構成・アクセス |
|---|---:|---|
| ACRTC VRAM | 128 KiB | 16-bit word。HM4864A-12（64K×1、120 ns）×16。読み出しと書き込みでCASの時刻を分ける [P]（DR-005）。リフレッシュはACRTCがHSYNC中に行う [D]（[15] §3.1）。表示・描画・DMAは調停。CPUからはACRTC経由 |
| DOC wave RAM | 64 KiB | 8-bit。DOC側（E Low）とホスト側（E High）の位相が固定で交互に来る [D]（[18]、§06 port 2）。DOCの子基板に載せる [P]（DR-011）。HM4864A-15（64K×1）×8。RAS／CASとリフレッシュはDOCが出す。ホストの書込み用に番地のラッチと行・列の切替を置く [P]（DR-031）。オシレータを30個以上使うとリフレッシュの周期が2 msを超えるおそれがある [V]（DR-031） |
| DSP program RAM | 8 KiB | 4096×16。外部プログラムメモリ。HM6168H-55（4K×4、55 ns）×4 [P]（DR-008） |
| DSP internal RAM | 288 B | 144×16。CPU直接アクセス不可 |
| Temporal RAM | 64 KiB | 独立R/Wカウンタ。リング／同期リセット。HM6264-12×8（8-bit幅）。Fabricだけが読み書きする [P]（DR-031） |
| Capture RAM [O] | 64 KiB | 入力専用、フレーム完了後に所有権を移す。採るならHM6264-12×8を32 KiB×2面とし、入力基板が片方に書く間にport 8がもう片方を読む [O]（DR-031） |

| ホストバスのメモリ | 部品 [P]（DR-008） | 備考 |
|---|---|---|
| Boot ROM 64 KiB | 27256（32K×8 EPROM）×2 | HN27256-30相当（300 ns）、1ウェイト [P]（DR-026） |
| Main RAM 128 KiB | 4164-15（64K×1、150 ns）×16 | 専用カウンタで1ラインに5行リフレッシュし、128行を約1.63 msで一巡する [P]（DR-008、計算は[26] §5.2）。HSYNCごとにRASオンリーで5行（1.56 µs）。その間のCPUのMain RAMアクセスは最大約13クロック待つ。CPUは0ウェイト [P]（DR-026） |
| Character ROM 32 KiB | 27256×1 | 300 ns品、1ウェイト。Fabricのport 9とは小さな調停器で分け合い、Fabricが使用中ならCPUは3〜5クロック待つ [P][V]（DR-031） |
| パレット | 74LS189（16×4）×6 | 裏3個・表3個 [P]（DR-027） |
| AV staging RAM 64 KiB | HM6264-12（8K×8 CMOS SRAM）×8 | リフレッシュ不要。Staging側の固定の時分割：500 nsの枠の前半をFabric、後半をCPU。CPUは0〜2ウェイト（平均0.75）。Fabricは68000のバスを要求しない [P]（DR-024）。位相によっては余裕が12.5 nsしかないので、HM6264-10は代替候補 [O][V] |

上記の部品は1985年の資料に量産品として載る [D]（[12]、[26] §5）。Character ROM、Temporal RAM、Capture RAMの部品とウェイト数はDR-031で決めた。価格と基板面積は未確定 [V]。DSPプログラムRAMは、アドレスから100 ns以内（切替回路込み）の要求に対し、HM6168H-55で約25 nsの余裕がある（計算）[P]（DR-029、[26] §5.4）。VRAMのCASを分ける回路は回路図とシミュレーションで確認する [V]（[26] §5.1）。VRAMにHM4864A-15を使う案も成り立つが余裕が薄い [O]（DR-005）。

<div class="pagebreak"></div>

### 06　Data Fabric：独自転送・接続回路 [P]

通常動作は有限帯域のDMA／FIFOコントローラ。初期は2チャネル実装（アドレス窓は4チャネル分を予約）。分岐は複数転送、フィードバックはRAMまたはFIFOを介す。ゼロ時間の循環接続は禁止。CPU、映像、DOC、DSPのメモリバスを無制約に直結しない。

実装手段 [P]（DR-010）：1985年の最終試作機の2チャネルはPALと74LS／ALSで組み、FIFOはMMI 67401（64×4）を使う。量産計画では国産CMOSゲートアレイ（NEC µPD65060級、約6,500ゲート）1個に置き換える。2チャネルの規模は約4,000〜5,200ゲート、TTLで約140〜180個と推定する [V]（[26] §4.1）。市販のDMAC HD68450-8は代替候補とする [O]（DR-010）。

ACRTCとのDMAは1語ごとのサイクルスチールとし、各語の間でCPUのアクセスを優先する。まとめて転送するとCPUを28〜48クロック待たせ、バスエラー判定の64クロックに対して余裕が小さいため [P]（DR-016、[30] §7.4）。

| ID | ポート | 方向・アクセス条件 |
|---|---|---|
| 0 | Staging RAM | R/W、byte address、16-bit対応。500 nsごとに1語を保証（4 MB/s）[P]（DR-024） |
| 1 | ACRTC VRAM stream | R/W。ACRTCのネイティブな連続転送（DWT／DRD／DMODコマンドと、DREQ／DACK／DONE信号）とFIFOを使う [D]（[15]）。矩形（x語×y行）単位。形はCPUが転送の前に出すDWT／DRD／DMODのAX・AYで決まり、行の間隔はRWPのDNが選ぶ画面のMWR [D]（[15]）。Fabric側は逐次ポートで、COUNT＝x·y。DONEとCOUNTが合わなければERROR [P]（DR-022）。表示しない画面のMWRで別の行間隔を使う方法は [O][V] |
| 2 | DOC Wave RAM | R/W、byte address。DOCサイクルごとのホスト側の窓（E High）でアクセスし、DOCレジスタのアクセスと同じ窓を取り合う。上限は1サイクル1 byte（8 MHzで1.0 MB/s）[D]（[18]）。子基板がないときは本体DACのFIFO（W、16-bit、上位byteがL・下位byteがR、64 words、67401×4）。音声クロック（TRIGGER 3と同じ）で読み、空なら直前の値を保ちUNDERRUNを記録 [P]（DR-019） |
| 3 / 4 | DSP input / output FIFO | W / R、16-bit、64 wordsずつ（67401×4）[P]（DR-017）。DSPが空の入力FIFOを読むと最後の語が返り、満杯の出力FIFOへの書込みは捨てられる。どちらもUNDERRUN／OVERRUNを記録 [P]（DR-017） |
| 5 | ADPCM input FIFO | W、8-bit packed、64 bytes（67401×2）[P]（DR-017）。ニブルは上位から。空のときは0x08（0000と1000が交互）を供給しUNDERRUNを記録。CTRLで下位からの順と、置換しない動作（Bend）を選べる [P]（DR-017） |
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
| 0E / 10 | SRC_STEP / DST_STEP | signed 16-bit byte stride。RAMポートのみ（port 1では無視、DR-022） |
| 12 | FORMAT | 0=8→8、1=16→16、2=8→16 zero extend、3=16→8 low byte、4=16→8 high byte。未対応はERROR |
| 14 / 16 | MASK / XOR | データ変換後に AND → XOR（16-bit）。初期値 FFFF / 0000 |
| 18 | SHIFT | signed 5-bit（-16〜15）、論理シフト。出力幅で切捨て |
| 1A | TRIGGER | 0=即時、1=VBlank、2=HBlank、3=audio tick（音声クロック8 MHz÷N）、4=external trigger（拡張端子のEXTRIG、§09と共通）、5=ADPCM V̄C̄K̄。いずれも2段FFで同期化し、立上りで1件だけ保留する。保留中に来た分は捨て、取りこぼしをSTATUSに記録 [P]（DR-018） |
| 1C / 1E | CTRL / STATUS | START/STOP/LOOP/IRQ_EN、BUSY/DONE/STALL/ERROR。終了フラグW1C |
| 20–2A | 予約 | 後続版の2D転送・アドレス変換等 |

0x250100以降：グローバルIRQ、チャネル状態、ポートリセット、クロック制御（音声クロックのN [P]（DR-018））、実装されているモジュールのビット（DOC子基板、映像入力、音声入力、拡張11〜15）[P]（DR-019）を予約。実装されていないポートをSTARTするとERROR [P]（DR-019）。初期のベンディングは8/16-bitデータ変換、RAM低位アドレスマスク・XOR、逐次ポインタリセット。アドレス変換はエミュレータの拡張設定として開始し、実機レジスタ割当は次版で定義する。

Data Fabric側はFIFOのfull/emptyで待機し、データを勝手に生成しない。一定の速さで読み書きするIC（DSP、MSM5205）の側は待てないので、ポートアダプタが空・満杯のときの動作を決め、UNDERRUN／OVERRUNを記録する [P]（DR-017）。STOPで未コミットの転送を破棄。LOOPは元のアドレス・個数で再開し、同期トリガ設定時は次のトリガを待つ。タイムアウトを計測し、DMA異常がCPU・ホスト全体の停止に波及しないようにする。

<div class="pagebreak"></div>

### 07　クロック・帯域・映像・DSP [P/V]

| クロック領域 | 初期値・方針 |
|---|---|
| 発振源 | 16 MHz、21.47727 MHz（6fsc）、20 MHz、384 kHzの4つ [P]（DR-003） |
| CPU / Fabric / DOC / ACIA | 16 MHz系。CPUとFabricは8 MHz（÷2）、DOCは8 MHz（÷2を子基板へ流用）、ACIAは500 kHz（÷32）。CPUとDOCは同じ源だが、DOCのブリッジとport 2のアダプタは、DOCのEを2段のFFで16 MHz系へ同期化して待つ。本体のクロックでは待ちが一定で、DOCのクロックを外部に替えても正しく動く。port 2の上限（1.0 MB/s）はDOCのクロックに比例する [P]（DR-035）。音声クロックは8 MHz÷N（Nは12-bit。÷144で55.6 kHz＝DOCの16オシレータのレート、÷1000で8 kHz）[P]（DR-018） |
| ACRTC / Video | 21.47727 MHz系。ACRTCの2CLKとドットクロックは5.369318 MHz（÷4）、色副搬送波3.579545 MHz（÷6）、映像取り込み2.6847 MHz（÷8）[P]（DR-003、DR-004）。色副搬送波はBA7230LSのVC端子へ入れ、1ライン228 fsc・ドットクロック1.5 fscで画素と色信号の位相を固定する [P][V]（DR-015） |
| DSP | 20 MHz（専用の発振源）[P]（DR-003）、200 ns命令サイクル級。外部プログラムメモリ速度を検証 [V]。外部クロックをジャンパで選べる（6.67〜20.5 MHz）[P]（DR-035） |
| ADPCM | MSM5205は384 kHz（専用の発振子）、出力8 kHz [P]（DR-003）。XTに外部クロックをジャンパで選べる（384〜768 kHz）[P]（DR-035） |
| 外部クロック | DOC、MSM5205、音声クロックの分周器、DSP、映像系の5か所に、本体の発振源と外部入力を選ぶジャンパと3ピンのヘッダ（CLK IN、GND、+5 V）を置く。外部の発振器やVCOをつなぐとピッチや速さを連続して変えられる。VCOは本体に載せない。16 MHzには置かない [P]（DR-035、§11） |
| 領域間 | 16 MHz系と映像系・DSP・ADPCMの間の受け渡しには同期化回路を置く。同期・待ち時間は別モデル [P]（[24] §5）。68000とACRTCの間は、CSを68000のS4の立上りで登録し、DTACKはCPUへ直結する（2〜4ウェイト、1アクセス6〜8クロック）。DREQとDMA時のDTACKに2段、DONEに1段の同期化FFを置く（TTL約8〜11個）[P]（DR-016、[30] §4–§7） |
| Video | ノンインタレース、262ライン×342ドット（ドットクロック5.369318 MHz）、水平約15.700 kHz、フレーム約59.92 Hz。NTSC互換のコンポジットとして出す（240p相当）[P]（DR-002）。ACRTCではRSM＝00、VC＝262で直接設定できる [D]（[15] §3.5） |
| Host | PC音声48 kHz（現代側のみ）。整数または有理数時刻で各クロックを独立管理 |

Fabricの理論予算：16-bit幅、8 MHz、1要素の読み書き計4クロック、待ちなしで最大4 MB/s。利用率60%仮定で2.4 MB/s。ポート幅、ACRTC FIFO待機、DRAMリフレッシュ等を含まない上限試算であり、実効帯域を保証しない。

| 処理 | 転送量の目安 |
|---|---:|
| 256×192、4bpp 1画面 | 24,576 B（24 KiB） |
| 同60 Hz片方向／往復 | 約1.47 / 2.95 MB/s |
| 同30 Hz片方向／往復 | 約0.74 / 1.47 MB/s |
| 128×96、8-bit、15 Hz捕捉 | 約0.18 MB/s |
| ADPCM 4-bit、8 kHz | 4 kB/s |
| DOC wave RAM 64 KiBの書換え | 上限1.0 MB/s、65.5 ms以上（§06 port 2）[D]（[18]） |
| 生CVBS 8-bit、4fsc | 約14.32 MB/s。第一世代の目標に含めない。入力のバーストにロックしたVCXO（ゲンロックと共通、DR-013）と専用RAMで取り込む映像拡張基板の候補 [O]（DR-023）。4fscは21.47727 MHzの整数分周で作れない（3fscは÷2）（計算） |

上の表はFabric側の量。ACRTC側にも上限がある。DWTで256×192・4bppの全画面を書くと、表示との競合を除いても2CLK 5.369 MHzで約14.0 msかかる [D]（[15]の値からの計算）。1フレーム約16.7 msのうち表示との競合を考えると、全画面を60 Hzで転送する運用は成り立たない見込み [V]。

#### 映像基本仕様

256×192、4bpp packed（16色）を初期標準。16-bit word内の画素順はACRTCネイティブ仕様に従い、4bppでは左端の画素がLSB側（bit3–0が画素0）[D]（[15]）。VRAM 128 KiBに24 KiBの画面2面と素材を格納。8bppは同解像度での拡張候補。1行32字×24行の8×8フォント表示に対応する。RGB各4-bitパレット、NTSCコンポジットとRGB出力を想定。外付けラッチ・パレット・DAC・エンコーダのタイミングは別設計。パレットは裏と表の2面で、各ラインのHSYNCの間に裏から表へ16語を写す（3.0 µs）[P]（DR-027）。エンコーダはBA7230LS [P][V]（DR-009、§10）。スプライト専用ハードは前提にせず、描画／COPYと背景復元でゲームを構成する。

ドットクロックと2CLKを同じにするため、ACRTCはデュアル（インタリーブ）アクセス、GAI＝+1で使う。256画素の有効幅は約47.7 µsになる [P]（[15] §3.3。レジスタ値の案は[15] §3.4）。

HD63484の1984年資料にある直線描画速度は8 MHz時に1画素0.5 µs級。本機の2CLK 5.369 MHzでは比例して約0.75 µsになる [D]（[2]の値からの計算、[15] §3.3）。全画面描画を60 Hzで更新する性能は保証されない。ローカルコピー、部分更新、低頻度の効果を活用する [2]。

#### DSPと時間軸

TMS32010は専用FIRチップではない。16-bit固定小数点のDSPプログラムでFIR・相関・波形処理を実装し、外部I/O転送時間を含めて評価する。内部RAMが小さいため行・ブロック単位とし、全画面のリアルタイム畳み込みは初期要件にしない。DSP I/Oは port0 入力、port1 出力、port2 状態／コマンド等のアダプタを設計する。ポートは0〜7（A11〜A3＝0）で、プログラムRAMへの書込みは8〜4095番地だけに通す。TBLWで0〜7番地に書くとOUTと同じになるので、SDKで禁じ、エミュレータで警告する（Bendの候補）。BIOはCTRLのBIOSELで4条件から選ぶ（入力FIFOが空でない〔既定〕／出力FIFOに空きがある／メールボックスに着信／音声クロックの拍）[P]（DR-029）。実命令のI/Oタイミングは原典を照合する。

Temporal RAMは後年のフィールドメモリICではなくDRAM/SRAM＋独立R/Wカウンタ・調停で実現。初期は8-bit、64 KiBのリング。未書込読出し、同番地競合、オーバーランの動作を定義する。フィールド保持、繰返し、遅延、読み書きクロック差を操作可能にする。

<div class="pagebreak"></div>

### 08　漢字ROM・文字コードの不整合 [P]

既存「試作機の文字ROM フォントの試み v0.1」（[design/main.md](../design/main.md)）を継承する。32 KiB（256 Kbit）に8×8ドット字形を格納し、7×7を字形本体、右・下1ドットを余白とする。第一水準2965字に加え非漢字・英数かなの領域を持つ。全字形は未完成であり、現状は英数36字・かなと記号102字・漢字30字の168字を起点とする（[design/main.md](../design/main.md) §03、データは [glyphs-v0.1.tsv](../design/glyphs-v0.1.tsv)）。以下は既存資料のROM内オフセットを維持した設計。

| ROM内offset (hex) | 内容 | 容量 |
|---|---|---:|
| 0000–07FF | 英数かな 256字枠 | 2,048 B |
| 0800–1F7F | 非漢字1〜8区（752枠） | 6,016 B |
| 1F80–7D7F | 第一水準16〜47区（3008枠、空き枠含む） | 24,064 B |
| 7D80–7FFF | 予約 | 640 B |
| 合計 | 32 KiB | 32,768 B |

JIS78基準では8区（罫線素片32字）は空き枠で、2区にも空きがある。JIS83で記号39字・罫線素片32字・漢字4字が追加された [D]（[28] §01）。8区と2区の追加分はJIS83の差分ROMイメージで埋まる [P]。

区点→第一水準字形offset = 0x1F80 + ((区−16)×94 + (点−1))×8。字形は1行1 byte、MSBが左。空き枠・未作成字は診断用プレースホルダと区別し、ROM生成時に状態を記録する。CPUの字形アドレスは0x010000＋offset×2（上位バイトレーン）[P]（DR-031）。Data Fabricのport 9はoffsetをそのままbyte addressとして読む。

旧JIS78の字形ROMを最終試作機の基準とし、新JIS83の配置・字形は差分ROMイメージとして保存する。双方の切替はエミュレータのROM差替え／仮想基板改造として提供し、1985年型に両方のROMが常時搭載されていたとはしない。実際の改正範囲・空き区点と字形は一次資料により再確認する。

| 実験 | 実装 |
|---|---|
| 正常表示 | BIOSが区点を字形に変換し、ACRTCへビット展開・描画転送 |
| 規格不一致 | 同一の区点列にJIS78/83の別ROMを適用。字形差・位置入替を観察 |
| Address bending | ROM参照アドレスの低位線固定・XOR。範囲外は定義済みopen bus等 |
| Data bending | 行データのビット反転・並べ替え・断線モデル |
| 字形の音響化 | Character ROM → DOC wave RAMまたはADPCM FIFO。ゼロ値で停止するDOC固有挙動も利用。各字形の8 byte目（下の余白行）は必ず0x00なので、そのまま再生すると8 byte以内で止まる。ビット0（右の余白列）も常に0なので、FabricでXOR 0x01をかければ止まらずに鳴らせる [P]（[18]） |
| 字形の再描画 | Character ROM → staging → DSP/ACRTC。加工後はRAMに保存（ROMは書き換えない） |

8×8全漢字はテレビ上の可読性が保証されない。フォントROMの容量制約と読めなさを機械の特性として扱う。高解像度字形・別規格は後継機または拡張候補。フォントのライセンス・由来・作成過程を管理し、他社ROMを配布物に含めない。

<div class="pagebreak"></div>

### 09　外部入力・コントローラ拡張・HCI [P]

MIDIは1983年に規格化され、1985年のAtari STにも内蔵端子の例がある [10][11]。本機の特徴はMIDIそのものではなく、外部機器がゲーム・創作・機械制御の共通入力になり得ること。1985年型の標準コントローラを残しつつ、将来のHCI設計に備えて入力の意味を固定しない。

| 端子／経路 | 1985年機の設計目標 | 現代PC／実機 |
|---|---|---|
| Controller A/B | 方向・ボタン等のデジタル状態、汎用アナログ値。端子ピン・物理形状は保留 | キーボード、ゲームパッド、MIDI等を割当 |
| MIDI IN/OUT | 5-pin DIN、31.25 kbit/s、8N1。HD6850 [P]（DR-008）＋絶縁入力。THRU候補 | PC MIDI、USB-MIDI変換（RP2350等） |
| Expansion | 16-bitホストバスの外部アクセス、IRQ、外部trigger（EXTRIG。Data FabricのTRIGGER 4とINPUT_IRQ_STATUSの両方に入る [P]（DR-018））。ピン割当・バッファ未定 | FPGA／マイコンブリッジ |
| Audio IN/OUT | レベル調整付きアナログ入出力、ミキサ。DACは子基板がないとき本体のHA17408×2 [P]（DR-019）、ADCは音声入力基板（port 10、拡張） | Audio interface、外部モジュラー |
| Video IN/OUT | NTSCコンポジット、75 Ω。入力基板の詳細は次頁 | ADV7180/ADV7391等、PCキャプチャ |
| CV/Gate | [O] アナログ制御端子、外部trigger、ADC入力。電圧範囲未確定 | RP2350＋保護・レベル変換・必要な外付けADC/DAC |

#### 入力の論理仕様（初期BIOS案）

| 0x260000からのoffset | レジスタ／用途 |
|---|---|
| 00 / 02 | PORT_A / PORT_B：16-bitデジタル状態（active-high論理値） |
| 04–12 | AXIS0–7：8個の16-bitレジスタ。初期は8-bitアナログ値を下位に格納 |
| 14 / 16 | MIDI_STATUS / MIDI_DATA：ACIAの制御・状態、送受信データ、上位byte lane |
| 18 | INPUT_IRQ_STATUS：入力変化、受信、外部trigger。W1C。ただしMIDIビットはACIAのIRQ̄の写しで読み専用。ACIAの状態→受信データの順に読んで解除する [P]（DR-028） |
| 1A | INPUT_IRQ_ENABLE：イベント割込みの許可 |
| 20–3E | 予約：アナログマトリクス、外部trigger／拡張 |

BIOS/SDKではデジタル状態、連続値、タイムスタンプ付きイベント（MIDI Note/CC/Clock等）を抽象化する。入力の意味付けはゲーム／制作ソフトが行う。MIDIバイト列はネイティブACIAを通し、エミュレータのホストイベントはシリアル転送時間も再現する。3-byteメッセージは約0.96 ms（連続送信時、待ちなし）。高頻度の連続制御は帯域制限を考慮する。

外部機器からのゲーム入力と、DMA起動・音源・描画パラメータへの割当はソフトウェアで設定する。通常モードではMIDIから任意バスへ直接書き込ませない。仮想ベンディングの直接介入は別機能とする。将来の新しいコントローラ、操作体系、物理パッチ、光学・身体入力を検討する際もこの境界を維持する。

<div class="pagebreak"></div>

### 10　映像・音響・アナログ回路 [P/O]

#### 外部映像入力

標準出力はNTSCコンポジット。外部映像入力は重要な拡張機能として第一世代から想定する。最初の1985年モデルは専用入力基板による輝度取得を優先し、128×96、8-bit、15 Hz相当を暫定目標とする（間引き・同期・帯域の成立性は未検証）。色復号付き取り込みは次段階。ADCはTRW TDC1048 [P]（DR-009）[9]。サンプリングは21.47727 MHz÷8（2.6847 MHz、出力の2画素に1サンプル）[P]（DR-003、[24] §7.4）。同期分離は専用ICを使わず、クランプとトランジスタまたは比較器による個別部品で組む [P]（DR-009）。これだけでは映像デコーダにならない。

<div class="arch compact">
<div class="node">CVBS IN → 75 Ω終端・クランプ・同期分離・帯域制限</div>
<div class="connector">↓</div>
<div class="triple"><div class="node">1985年案<br>TDC1048＋輝度・間引き</div><div class="node">現代版<br>ADV7180 復号</div><div class="node">代替／追加<br>AD9280 生波形</div></div>
<div class="connector">↓ 独立捕捉経路・FIFO・所有権切替 ↓</div>
<div class="node accent">Capture RAM → Data Fabric → VRAM / DSP / DOC / ADPCM</div>
</div>

ADV7180は復号済み映像、AD9280はADC（生波形を取得し外部で復号）であり、直接互換ではない。両方を必須にしない。ADV7391は現代PCBの出力候補。[13] 1985年型のNTSCエンコーダはローム BA7230LS とする [P]（DR-009、DR-015）。色副搬送波は21.47727 MHz÷6をVC端子に入れる [P][V]（DR-015）。本体では映像入力との重ね合わせを使わない。外部映像の取り込みとComposite feedback（§11）は取り込み経路で成り立つ。ゲンロック（ACRTCの外部同期と、映像系クロックのVCXO／PLL）は映像拡張基板の候補 [O]（DR-013）。4fscの生波形の取り込みも同じ基板の候補とし、第一世代の目標に含めない。3fsc（21.47727 MHz÷2）で自機の出力を取る方式も候補 [O]（DR-023）。RGB/輝度・色差、色副搬送波、同期合成、75 Ω出力回路を別途設計する。

コンポジット信号モデルは水平・垂直同期、ブランキング、輝度、色副搬送波（NTSC約3.58 MHz）、バースト、帯域制限を含む。4fsc（約14.32 MS/s）を基準にした生成・復号モデルをPC上で段階実装。復号済みPCカメラと生波形は区別する。外部映像フィードバックは仮想波形または実キャプチャから形成する。

#### 音響回路・信号ルーティング

ES5503の内蔵DAC・チャンネル選択をモデル化し、外部サンプル保持、ミキサ、LPF、出力アンプの回路を定義する。初期ステレオ出力。MSM5205の復号状態、DAC、LPFも再現する。CD22101系は低帯域の音声・CV・変調信号を優先し、コンポジット全帯域の通過は保証しない。映像の高周波経路は専用バッファ・スイッチを別設計する。

DOCの差動電流出力（SIG±）は両側にアンプを入れて加算する。初期の片側だけの配線をジャンパで選べる形で残し、Bendモードの材料とする [P]（DR-011）。オペアンプは、I/V変換にLM318級、サンプルホールドにTL082級、加算・フィルタに4558級を使う [P]（DR-009）。DOCの子基板がないときは、DSPのソフト合成で音を出す構成を残す [P]（DR-011）。そのときは本体のDAC（日立HA17408×2、8-bitステレオ）で出す。経路はDSP → port 4 → Fabric → port 2（本体DACのFIFO）→ DAC → I/V変換（4558級）→ LPF（約20 kHz [V]）→ ミキサ。子基板は検出ピンで判別し、挿さっているときはport 2とDOCの窓を子基板に渡す [P]（DR-019）。12-bitのNEC µPC648は代替候補 [O]。

複数の低インピーダンス出力をスイッチで直結しない。加算抵抗、バッファ、クランプ、DC遮断、電圧範囲を設計する。実機の電気的故障と仮想的な故障モデルは区別する。CV/Gateの現代拡張も電圧・閾値・過電圧保護を確認する。

#### 電源

電源ユニットは+5／+12／−12 Vの3出力とする [P]（DR-020）。DOCの音量DACの基準−5 Vは子基板で、TDC1048の−5.2 Vは映像入力基板で、それぞれ−12 Vから3端子レギュレータ（NEC µPC7900系。日立の1985年の資料には負電圧品が見当たらない [V]）で作る [P]（DR-020）。−5.2 V側は約0.3 Aで、−12 Vから落とすと約2 Wの損失になり、放熱板が要る（推定 [V]）。±12 VはCD22101、オペアンプ、HA17408（VEE −12 Vで使う [V]）に使う。MSM5205・BA7230LS・映像の抵抗DACの出力段は、+12 Vから作るアナログ+5 Vから給電する [P]（DR-020）。MSM5205の入力（VIH 4.2 V以上）はHD74HC系か、プルアップを付けたTTLで駆動し、V̄C̄K̄（±50 µA）はHD74HC系で受ける [P]（DR-020）。

+5 Vの負荷は67401×14（最大2.2〜2.8 A [V]）、TTL 140〜180個、DRAM 32個、NMOSの大型IC、PALなどで、合計5〜10 A級と推定する [V]。容量は試作時に実測する。

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
| Circuit | 信号経路、ゲイン、フィルタ、帰還。DOC出力配線のジャンパ（片側／両側）[P]（DR-011） | デジタル・アナログモデルを分離 |
| Codec | ADPCM復号状態、入力欠落、データ境界 | 正常系と仕様外挙動を区別 |

通常モードとBendモードを分ける。実ICの外部ピンで可能な操作、専用グルーロジックで可能な操作、エミュレータ限定の内部状態介入を区別する。仕様外動作は近似と明記し、実測可能な場合に更新する。

#### ベンド点 [P]（DR-034、DR-035）

最終試作機に残る評価用のテストピン、クロックのジャンパ、EPROMのソケットを、実機でベンディングする入口とする。

| 操作 | 場所 | 区分 | 結果 | 資料の範囲・注意 |
|---|---|---|---|---|
| DOCのピッチ | 子基板のCLKジャンパ→外部の発振器・VCO | 実ICのピン | 全オシレータの音程が一斉に動く | 8〜10 MHz [D]（[18]）。後年のApple IIGSは7.16 MHzで量産。範囲外は [V]。下げすぎるとレジスタと波形RAMの内容が崩れる [V] |
| ADPCMのピッチ | MSM5205のXTジャンパ→外部クロック | 実ICのピン | ADPCMの音程と速さ。S1/S2の3段の切替はソフト | 384〜768 kHz [D]（[20]）。XTを外部クロックで駆動できるかは [V] |
| DSP合成・本体DACのピッチ | 音声クロックの分周器の入力ジャンパ | つなぎ回路 | DSP合成と本体DACの音程、TRIGGER 3、BIOの拍が一緒に動く | 同期化FFの条件（パルス幅250 ns以上）まで |
| DSPの速さ | TMS32010のCLKINジャンパ | 実ICのピン | 処理の速さ。合成の音程は変わらず、間に合わないとFIFOが空になって崩れる | 6.67〜20.5 MHz [D]（[17]）。速すぎるとプログラムRAMの読出しが間に合わず暴走する |
| 映像のクロック | 21.47727 MHzの外部入力（ゲンロック基板と共通の口） | 実ICのピン | 同期と色が崩れる | テレビが追従できないことがある。ACRTCの2CLKは125 ns以上 [D]（[15]）。Main RAMのリフレッシュはHSYNCごとなので（DR-026）、約8割より下げるか止めると2 msの規格を外れ、Main RAMの内容が崩れていく。崩れ方は個体と温度による [V] |
| 文字ROMの線 | ソケットとテストピン | 実ICのピン | 線の固定・交換・浮かせで字形が崩れる。FabricのXOR・マスクでも同じ操作ができる（つなぎ回路） | 浮かせた線の値は不定 [V] |
| DOC波形RAMの線 | 子基板のテストピン | 実ICのピン | 読む番地と値が崩れ、波形が変わる | |
| FIFOの状態 | EMPTY・FULLのテストピン | つなぎ回路 | UNDERRUN／OVERRUNを起こす | |
| DOC出力の片側 | SIG±のジャンパ | 回路 | 音色と直流分が変わる | DR-011 |
| パレットの直結 | PAL_CTRLのDIRECT | レジスタ | ライン途中の色の変化と点状のノイズ | DR-027 |
| アナログ帰還 | CD22101のマトリクス、映像出力→取り込み | 回路 | 帰還による発振・変質 | 出力同士を直結しない（§10） |
| CPU・Fabricのクロック | ジャンパを置かない | エミュレータのみ | 機械全体の時間が変わる | MIDIの速度とバスのタイミングが崩れ、止まりやすい |

テストピンは検査用で、保護抵抗はない。線をGNDや+5 Vに固定するときは1 kΩ程度の抵抗を挟み、出力同士を直結しない。エミュレータは各ベンド点を同じ名前の仮想の信号線・パラメータとして持ち、各クロック領域の周波数を実行時に変えられるようにする。資料の範囲外の動作は近似と明記する [P]（DR-034、DR-035）。

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
| 1　Chip sandbox | HD63484描画・COPY・FIFO、DSP乗算/FIR/外部I/O（サイクル数はTIの資料どおり。BIOZのBIOは1サイクル目で見る [V]、DR-032）、DOC発音と停止、ADPCM既知列を単体試験。ADPCMの既知列には、範囲外の動作を見分ける列（リセット直後に0111を6個。6個目が回り込みなら−1586、クリップなら+2047）を含める（DR-021） |
| 2　Data Fabric | 2 DMAチャネル、待機、IRQ、変換、FIFO、競合、周期トリガ。traceで順序と所要時間を確認 |
| 3　Console | 自作ROM起動、入力、ゲーム描画、文字ROM、PCM/ADPCM音声。256×192、4bppのデモ |
| 4　AV / HCI | 異種データパッチ4種以上、外部映像・MIDI、クロック・ROMベンディング、複数入力割当。30分の連続試験 |
| 5　PCB / 拡張 | HDL独立モデル、合成・回路検証、実ICの波形測定、現代I/O基板 |

正常系はビット列一致（V2）と独立実装比較（V3）を目指す。描画FIFO・描画処理時間、DSP命令、DOC波形読み出し時刻を順次詳細化。アナログは既知入力に対するゲイン・周波数応答・同期・ノイズを測定（V4）。全ゲート／トランジスタ再現を初期条件にしない。PC上で仮想機の約59.92 Hzのフレームと48 kHz音声を実時間で動かすことを目標とし、負荷はプロファイルで評価する [P]（DR-002）。ホスト表示のリフレッシュとの差は、仮想機の時刻を変えずにホスト側の表示で間引き・複製して吸収する [P]（[24] §3.3）。

**デザインへ戻る条件**：入力・音響・描画・パッチが動いた時点で、標準コントローラに依存しない新しいHCIを小規模に試す。入力ポート・イベント・拡張バスの要件に影響するならPhase 2以前でも設計を見直す。筐体や外観の完成は必要条件にしない。

<div class="pagebreak"></div>

### 13　将来のPCB化・未確定事項

1985年の最終試作機と量産計画の基板の分け方、外形・高さ・端子の位置の見積りは [board.md](board.md) に置く [P]（DR-036）。以下は現代の再現の候補。

| 部品／機能 | 現代版の実装候補 |
|---|---|
| HD68000 / TMS32010 | 旧在庫、互換IC、FPGA。TMS32010はピン互換の後継TMS320C10の流通在庫とFPGA実装がある [O]（[27] §7.1） |
| HD63484 | 実チップを確保する。市販のHD63484P98（ピン配置はHD63484P-8と同じDP-64）を含め、真贋を問わず5.369 MHzで動作を確認できれば使う [P]（DR-012）。互換品IA63484は実在するが、現在の取扱は見つからない [D]（[27] §7.1）。HDL実装は見つからない [V] |
| ES5503 | 取り外し品を確保する前提とし、FPGA（MiSTerの実装）・ソフト実装を併用する [P]（DR-012） |
| MSM5205 | 実IC、FPGA、RP2350等。DAC・固有挙動を活用する場合は実IC優先。FPGAでjt5205を使う場合は、飽和を回り込みに変える改変が要る（DR-021。GPL-3の扱いはOI-014） |
| Data Fabric / Temporal RAM | FPGA、SRAM/SDRAM等。1985年コア仕様と現代実装を区別 |
| CD22101 | 実チップ（CD22101Fの流通在庫）を第一とする [P]。8×16規模の代替にはCD22M3494（MT8816とピン・機能互換の現行品）[O]。CH446Qは機能代替で、MT8816ともピン互換でない。マトリクス規模・電源・制御差を吸収する [O]（[27] §7） |
| ADV7180 / ADV7391 | 現代の映像I/O候補。LCSC掲載例、AliExpress等も候補 |
| AD9280 | 代替または追加の高速ADC。単独では色復号不可 |
| RP2350 | MIDI、CV/Gate、USB、設定・外部I/O |

前版の調達メモ：LCSC CH446Q C109471、ADV7180 C11164/C395114、ADV7391 C443874、AD9280 C44935。これらは過去の掲載情報であり、現在の在庫、価格、JLCPCB PCBA可否や真正性を保証しない。旧世代ICすべての互換品がLCSC/AliExpressにあるかは未確認。

DIP/SOP/LQFP等の手はんだ、QFN/LFCSP/BGAのPCBA・リフロー、変換基板を使い分ける。5 V系と3.3/1.8 V系のレベル変換、電源・熱設計、75 Ω映像終端、アナログリターン経路を設計する。全ICをFPGAに集約することは必須とせず、合成結果・実測・費用で判断する。

#### 次版の優先事項 [V]

1. HD63484のレジスタ／描画命令・VRAMシーケンス（CASを分ける回路）・DMAハンドシェイク・外部表示回路の精査。特にData Fabricからの連続RD/WT転送と描画との競合を確認する。68000からの連続アクセスと書込み直後の読出しで増える待ち、FIFOが満杯・空のときの動作は実チップで確かめる（OI-047）。
2. TMS32010の外部プログラムRAMのロード回路（DR-029）を回路図で確かめ、サイクル単位のDSP演算予算を見積もる。
3. ES5503の波形RAMアクセス位相、DAC／外部多重化、音源出力のタイミングを検証する。子基板がないときの本体DACの出力段（I/V変換、LPF、ミキサでのレベル合わせ）を確かめる（DR-019）。
4. BA7230LSのVC端子への副搬送波の入れ方を実チップで確認する。映像入力基板の間引き・同期・RAMアクセスを確認する。4fscの生波形は、映像拡張基板の候補としてゲンロックと合わせて検討する（DR-023）。
5. Data Fabricの帯域を見積もる（FIFOの深さはDR-017）。0x250100以降のIRQ・port status・clock register・モジュールのビットの番地と、初期ROMフォーマットを確定する。
6. コントローラ物理仕様、拡張端子、BIOS ABI、漢字ROMの全字形とJIS改正差分を確定する。HCI試作結果に応じて修正する。
7. 電源の容量を試作時に実測する（構成はDR-020、§10）。
8. 評価用のテストピン・ジャンパ・ソケットの位置を基板ごとに決める（DR-034）。MSM5205のXTを外部クロックで駆動できるか、DOCを資料の範囲外のクロックで動かしたときの挙動を確かめる [V]（DR-035）。

本書は上記の未確定事項を残した開発開始用仕様。1985年当時の量産性、総BOM原価、リアルタイム性能を検証済みとするものではない。

<div class="pagebreak"></div>

### 14　参照資料

[1]〜[14]は2026.11.24参照。[15]〜[27]はリポジトリ内の部品台帳（2026.11.24〜25調査）で、原典の書誌・頁・参照日と年代の区分は各台帳に記録している。1985年以前の資料を年代の根拠とし、後年の資料はIC復元・実装参考に限定する。原典の全ピン／タイミング表は未精査。リンク先はアーカイブを含む。

| ID | 資料・確認範囲 |
|---|---|
| [1] | Motorola, M68000 User's Manual, bus operation. [バス仕様PDF](https://datasheets.chipdb.org/Motorola/68000/MC68000UM.9.pdf)（前版参照、後年版） |
| [2] | 日立評論 Vol.66 No.7 (1984), pp.43–46, HD63484 ACRTC。表1、図1–3。[原典PDF](https://www.hitachihyoron.com/jp/pdf/1984/07/1984_07_10.pdf) |
| [3] | Hitachi, HD63484 Application Note (1986)。[アーカイブ](https://bitsavers.org/components/hitachi/_dataBooks/U90_Hitachi_HD63484_ACRTC_Advanced_CRT_Controller_Application_Note_198604.pdf)、[データシート](https://mega-micros.co.uk/datasheets/datasheet%20-%2063484%20%28hitachi%29%20acrtc.pdf)。AR/STATUS・FIFO・表示回路の復元用。データシートは-98品を含む後年資料で、データDMAの頁が白紙 |
| [4] | TI, TMS32010 User's Guide, Rev.B (1985-03), program memory / I/O。[原典PDF](https://bitsavers.computerhistory.org/components/ti/TMS320xx/3201x/1983_TI_TMS32010_Users_Guide_Digital_Signal_Processor_Products_Revision_B_198503.pdf)。テキスト確認、画像取得制限あり。付録のデータシートは1986-02改訂（後年資料）。[1983 Assembly Guide](https://www.bitsavers.org/components/ti/TMS320xx/3201x/1983_TMS32010_Assembly_Language_Programmers_Guide.pdf) |
| [5] | Apple Computer, Ensoniq DOC ERS Rev.1.0 (1986-06-25), pp.1–2、oscillator/register descriptions。[技術資料PDF](https://www.brutaldeluxe.fr/documentation/cortland/v4_13_EnsoniqDOC.pdf)（後年資料） |
| [6] | Ensoniq Mirageの技術資料アーカイブとMAME機種実装。[資料一覧](https://zine.r-massive.com/ensoniq-technical-documents-and-schematics/) / [Mirage実装](https://github.com/mamedev/mame/blob/master/src/mame/ensoniq/enmirage.cpp)。採用実績・復元参考 |
| [7] | OKI Voice Synthesis ICs (1994), MSM5205。[データブック](https://www.bitsavers.org/components/oki/_dataBooks/1994_OKI_Voice_Synthesis_ICs.pdf)（前版参照、後年資料。初期品の差異は要調査） |
| [8] | RCA COS/MOS Integrated Circuits (1980), CD22101。[データブック複製](https://manuals.plus/m/e55f9cfbf186e76de7d6ee64653ba60cdefe735fa4ceb15663fd0e3555f463b5) / [後年データシート](https://dtsheet.com/doc/151567/intersil-cd22102e) |
| [9] | TRW VLSI Data Book (1985), TDC1007、TDC1048。[原典PDF](https://bitsavers.trailing-edge.com/components/trw/_dataBooks/1985_TRW_VLSI_Data_Book.pdf) / [1984資料](https://w140.com/tekwiki/wiki/File%3ATRW_TDC1007_Data_Sheet.pdf) |
| [10] | [MIDI Association: 1983–1985](https://midi.org/midi-history-chapter-7-midi-associations-1983-1985)、[通信形式](https://www.cs.cmu.edu/~music/cmp/archives/cmsip/readings/davids-midi-spec.htm)、[Motorola 1975 applications manual](https://www.bitsavers.org/components/motorola/6800/M6800_Microprocessor_Applications_Manual_1975.pdf) |
| [11] | International Musician & Recording World (Nov. 1985), [Atari 520ST MIDI Computer](https://www.muzines.co.uk/articles/atari-520st-midi-computer/5101) |
| [12] | Maplin Electronics (June 1985), [27256/41256掲載号](https://www.worldradiohistory.com/UK/Mapelin/Maplin-Electronics-1985-06-08.pdf) |
| [13] | 現代I/O資料：ADI [ADV7180](https://www.analog.com/en/products/adv7180.html)、[ADV7391](https://www.analog.com/en/products/adv7391.html)、[AD9280](https://www.analog.com/en/products/ad9280.html)、[RP2350](https://pip.raspberrypi.com/categories/1214-rp2350)、[CH446Q](https://www.lcsc.com/product-detail/C109471.html) |
| [14] | MAME参考実装：[HD63484](https://github.com/mamedev/mame/blob/master/src/devices/video/hd63484.cpp)、[ES5503](https://github.com/mamedev/mame/blob/master/src/devices/sound/es5503.cpp)、[MSM5205](https://github.com/mamedev/mame/blob/master/src/devices/sound/msm5205.cpp)、[TMS32010](https://github.com/mamedev/mame/tree/master/src/devices/cpu/tms320c1x)（旧 `tms32010/`）、[MAMEリポジトリ](https://github.com/mamedev/mame)。個別の精度・ライセンスは要確認 |
| [15] | 部品台帳 [HD63484.md](../parts/HD63484.md)。ACRTCのピン・タイミング・レジスタ、2CLKと走査の案、VRAMの速度条件、MAME実装の注意 |
| [16] | 部品台帳 [supply-HD63484.md](../parts/supply-HD63484.md)。HD63484の1984〜85年の供給・速度品・採用例 |
| [17] | 部品台帳 [TMS32010.md](../parts/TMS32010.md)。ピン・タイミング・I/O、外部プログラムRAMのロード回路 |
| [18] | 部品台帳 [ES5503.md](../parts/ES5503.md)。ピン・クロック・位相・レジスタ・出力 |
| [19] | 部品台帳 [supply-ES5503.md](../parts/supply-ES5503.md)。ES5503の1985年の供給、Apple IIGSとMirageの類例 |
| [20] | 部品台帳 [MSM5205.md](../parts/MSM5205.md)。サンプリング・復号・LPF |
| [21] | 部品台帳 [CD22101.md](../parts/CD22101.md)。制御・電気特性・用途の適否 |
| [22] | 部品台帳 [TDC1007.md](../parts/TDC1007.md)。TDC1007と同時代のTRWのADC（TDC1048を含む） |
| [23] | 部品台帳 [MC6850.md](../parts/MC6850.md)。6850系ACIAと68000の接続、MIDIのクロック |
| [24] | 部品台帳 [clock-video-1985.md](../parts/clock-video-1985.md)。1985年前後のクロック構成、走査、映像入出力 |
| [25] | 部品台帳 [baseline-1985.md](../parts/baseline-1985.md)。1985年の日本製機器の部品構成、68000の国内供給 |
| [26] | 部品台帳 [review-digital.md](../parts/review-digital.md)。CPU・ACIA・DMA・グルー・FIFO・メモリ・パレットの見直し |
| [27] | 部品台帳 [review-signal.md](../parts/review-signal.md)。DSP・映像ADC・エンコーダ・同期分離・アナログ部の見直し、現代の入手性 |
| [28] | 出典台帳 [sources.md](../sources.md)。部品以外の史実（JIS C 6226 の改正、キャプテンシステム、Commodore の日本での生産、X1 の重ね合わせ、設計慣行の類例） |
| [29] | 部品台帳 [BA7230LS.md](../parts/BA7230LS.md)。1984年の雑誌記事・広告による年代の根拠、ピン、VC端子への副搬送波の入力 |
| [30] | 部品台帳 [host-acrtc-interface.md](../parts/host-acrtc-interface.md)。68000とHD63484の非同期インタフェース：CSの作り方、待ち数、DTACKとDMAの同期化 |
| [31] | 部品台帳 [HD68000.md](../parts/HD68000.md)。HD68000-8のバスサイクル、DTACK・BERR・HALT、割込み応答、E・VPA・VMA、バス調停、リセット、旧マスクの不具合 |

内部資料：Fictional Machines 基本構想・分類体系 v0.6、試作機の文字ROM フォントの試み v0.1（[design/main.md](../design/main.md) に取り込み）、旧ハードウェア設計仕様書 v0.1（1990年前後版）。決定記録は [decisions.md](../decisions.md)、物語は [story/main.md](../story/main.md)、未決の課題は [open-issues.md](../open-issues.md)。
