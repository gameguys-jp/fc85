# ES5503 DOC 供給・外販 調査メモ
Phase 0 調査 v0.1 ／ 調査日 2026-11-25

凡例：`[D 資料ID 頁]` 出典あり。`[V]` 未確認。「推定」は本メモでの推論。資料IDは末尾の表を参照。E1〜E12 は `ES5503.md` と同じ資料、S1 以降は本メモで追加した資料。

頁表記：PDF は PDF 上の頁番号（印刷番号と違う場合は注記）。Web 記事は頁を付けない。

年代区分：

- **1985年以前の資料**：1985-12-31 までの日付をもつ資料。年代の根拠に使う。
- **後年資料**：1986 年以降の文書、取材、回想、Wikipedia など。回想は「後年の証言」と書く。
- 1986 年の Apple 社内文書（S3〜S7）は後年資料だが、1985〜86 年の出来事を当時に記録した一次資料として扱う。

---

## 0. 最重要：1985年中に Ensoniq 以外へ DOC が出ていたか

| 事実 | 区分 | 根拠 |
|---|---|---|
| Apple 社内メモ（1985-06-17、Fern Bachman「Tools Overview/Notes」）に「the ENSONIC sound chip Columbia uses」とある。ROM のサウンドツールで、このチップとのやり取りを簡単にする計画。Columbia は IIGS の開発名の一つ（同時期の「Columbia AppleTalk ERS」1985-06-12 がある） | 1985以前 | [D S1 p.1, p.8][D S8] |
| 「CORTLAND BLOCK DIAGRAM」（1985-08-08、署名は D.L.H. のイニシャルのみ）に「SOUND GLUE」「64K×4 RAM」2 個「ENSONIQ」が描かれている。この音源部は破線の枠で囲まれ、ほかの機能ブロック（MEGA II、VGC、SCC、IWM など）は実線 | 1985以前 | [D S2] |
| 破線は「オプション扱い」を表すと読める。後年の証言「DOC と 64K RAM は直前までオプション」と合う | 推定 | [S2][S10] |
| 1986 年の Apple 文書 2 点が、典拠に「Sound Glu ERS, June 21, 1985」を挙げる。Sound GLU は 65C816 と DOC・音源 RAM の間をつなぐ Apple のカスタム IC。制御レジスタのビットで「DOC をアクセス／RAM をアクセス」を選ぶ | 後年資料（ERS の日付は 1985） | [D S4 p.3][D S5 p.2]。GLU の機能は [D S4 PDF p.120–128][D S5 p.51] |
| ERS 本体（1985-06-21 版）は未入手。1985 年の時点で DOC 向けの内容だったかは確認できない | — | [V] |
| 1986-02-25〜26、Apple の 3 名（Rob Moore、Gus Andrade、Allen Watson）が Ensoniq 工場を訪問。Apple 側は DOC を使う回路を説明し、「基板設計はほぼ凍結済み」と述べた。出力の取り方などを以前から試していた | 後年資料（1986-03-05） | [D S3 p.2–4] |
| DOC の現物サンプルが 1985 年中に Apple に渡ったことを直接示す、1985 年の日付の資料 | — | 見つからない [V] |
| 1986-02 の時点で試作と試行錯誤の話があるので、1985 年中に現物が Apple にあった可能性は高い | 推定 | [S3] |
| 「Ensoniq が DOC のサンプルを Apple に送った」 | 後年の証言（1997、日付なし） | [D S10] |
| Ensoniq の欧州部門（ブリュッセル）がイタリアに生産拠点を設け、Mirage を「ライセンス生産」する（1985-07 の記事） | 1985以前 | [D E6]。生産会社の名前は不明 [V] |
| 上の結果、Q-chip は 1985 年に米国外の工場へ出荷されたことになる。ただし Ensoniq 製品（Mirage）の生産であり、他社製品への供給ではない | 推定 | [E6] |
| 1985 年に Apple 以外の会社が DOC を評価・採用した記録 | — | 見つからない [V] |

**まとめ**：1985 年 6〜8 月の日付をもつ Apple 社内文書 2 点（S1、S2）に Ensoniq の音源チップが載っている。少なくとも DOC の情報は 1985 年中に Ensoniq の外へ出ていた。現物サンプルの受け渡しの日付は、資料では確認できない。正式な契約は 1986 年とされる [D E12]。

---

## 1. Ensoniq の外販の意思と時期

### 1985年以前の資料

- 1985 年の資料で、Ensoniq が DOC を他社に売る意思を示した記述は見つからなかった [V]。
- E&MM 1985-02：Q Chip は「Mirage のために設計した」チップ。前身 PVI の DrumKey もカスタムの音源 ROM と DAC チップを使う。同じ技術で 200 ドル未満のドラムマシンを出すという噂 [D S13]。
- One Two Testing 1985-06：Q Chip の開発に 100 万ドル以上かかり、Mirage の販売で回収する賭け、と書く [D E5]。
- IEEE Spectrum 1985-03：創業者の前職 MOS Technology は「merchant semiconductor house」（外販する半導体メーカー）だった。1979〜80 年ごろ、Commodore のほか Atari、GE、HP などにも供給していた。C64 の 2 チップも当初は「世界最高のビデオゲーム」を作る誰にでも売るつもりで設計した [D S12]。

### 後年資料

- 1988 年の取材で、市販品ではない部品を使う危険を問われた Yannes は「このチップはほぼどの会社でも作れる。単一ソースに縛られておらず、量産できる会社が複数ある」と答えた [D E7]。後年の証言。自社の調達先の話で、他社への外販の話ではない。
- 1996 年の取材で Yannes は、SID は MOS が外販メーカーだったので、シンセメーカーに売るつもりで設計した、と述べた [D S19]。後年の証言。
- Ensoniq と Apple の契約は 1986 年 [D E12]（二次資料）。
- Ensoniq の Bill Mauchly が IIGS の Note Synthesizer の文書（v0.5、1986-09-25）を書いている [D S8]。1986 年には技術協力があった。
- 「semiconductor group」という部門名は E2 の表紙にしか見つからない。実在と時期は不明 [V]。
- inCider 1986-11「The IIgs Hardware Report」：MDIdeas が予告した IIgs 用デジタイザ SuperSonic（ハードとソフトで 100 ドル）は「IIgs と同じ Ensoniq サウンドチップを自前で持つ」と書く [D S26]。実物の構成は未確認 [V]。1997 年の FAQ は SuperSonic を「最初の IIgs 用ステレオカード」と書くだけ [D S10]。

### E2 の発行年の手がかり

| 手がかり | 根拠 |
|---|---|
| E2 の住所は「263 Great Valley Pkwy., Malvern, Pa 19355」 | [D E2 p.1] |
| 同じ住所が Mirage の Service Bulletin No.1（1985-06-01）から No.9（1986-03-01）まで載っている | [D S16 各号 p.1] |
| 1984 年の PVI の住所は「Great Valley Parkway, Malvern」（番地なし） | [D S14] |
| 後年の Ensoniq の住所は「155 Great Valley Parkway」 | 検索結果のみ [V] |
| 1986-02 に 26,000 平方フィートの工場へ拡張、約 1 年後に 70,000 平方フィートへ移転 | [D E7] 後年資料 |
| E2 は 6809E 用の E/Q クロックを前提とする。Mirage の CPU は 68B09E。65816 や Apple の記述はない | [D E2 p.1][D E5] |
| 用途に「professional music products」から「arcade or home video games」まで挙げる | [D E2 p.1] |

推定：E2 は 1985 年半ばから 1987 年ごろの発行。1985 年以前と断定する根拠はない。

---

## 2. ICS1261 の正体

| 事実 | 根拠 |
|---|---|
| ICS1261（DOC1）は「the leading U.S. manufacturer of electronic keyboard musical instruments」である Ensoniq が設計した、と書く | [D E3 p.1] |
| ICS の住所：Valley Forge Corporate Center, 2626 Van Buren Ave., P.O. Box 968, Valley Forge PA 19482-0968。電話 (215) 666-1900、FAX (215) 666-1099 | [D E3 p.12] |
| ICS は DOC II（ES5504 相当）も ICS1399「Digital Sound Generator (DOCII)」として出している | [D S25 p.1] |
| ICS は 1976 年にペンシルベニア州で設立 | [D S18] 1998 年の SEC 提出書類 |
| ICS は「OEM 向けのカスタム IC 設計と製品調達（product sourcing）サービス」の会社として設立。マスク、ウェハ製造、組立は外部に委託し、生産管理、試験、品質管理、製品技術は自社で行う | [D S17 会社概要、ASIC の章] 1994 年 |
| ASIC の顧客には、開発から量産の手配（production sourcing）までを請け負う。国内外のファウンドリ、マスク会社、組立会社と取引があり、複数ソース化（multi-sourcing）ができると書く | [D S17 ASIC の章] 1994 年 |
| 1988 年に ICS1394（ビデオ用タイミング発生器）を出し、標準品の柱になった | [D S17 会社概要] |
| 1994 年のデータブックの住所は 2435 Boulevard of the Generals、電話 (215) 244-4000。ICS1261 は載っていない | [D S17] |
| 1998 年時点で ICS は自社ファブを持たず、外部ファウンドリ 3 社に頼る | [D S18] |
| 日本語の個人サイトは「DOC の IC 自体は ICS 社で製造」と書く | [D S23] 後年の証言。裏付けなし [V] |

推定：

- E3 は ICS が Boulevard of the Generals に移る前（1994 年より前）のもの。「leading U.S. manufacturer」という書き方から、Ensoniq が大きくなった 1986 年以降の可能性が高い。1985 年の根拠にはならない。
- ICS がライセンスを受けた第二ソースか、Ensoniq のチップに自社名を付けて売った（リバッジ）かは不明 [V]。
- ICS は製造を外注する会社なので、仮に ICS 経由でも、実際のウェハ製造会社はわからない [V]。

1988 年の Yannes の発言の全体は §1 を参照 [D E7]。「単一ソースに縛られない」は Q-chip を含む自社カスタムチップ全般について。ICS の名前は出ていない。

---

## 3. Apple の採用の経緯と日付

| 日付 | 出来事 | 根拠 | 区分 |
|---|---|---|---|
| 〜1984-03 | 前計画 IIx が 1984-03 に中止。65816 の初期サンプルにも問題があった | [D S9][D S11] | 後年資料 |
| 1984 夏の終わり | 「Phoenix」として 16 bit Apple II を再開 | [D S9] | 後年資料 |
| 1984-01 | PVI（Ensoniq の前身）が NAMM で Apple II 用ドラムカードを展示 | [D E7] | 後年の証言 |
| 1984-08 | PVI の Apple II 用ドラムカード「Drum-Key」の新製品記事 | [D S14] | 1985以前 |
| 1985-06-17 | Apple のツール計画メモに「ENSONIC sound chip」 | [D S1 p.8] | 1985以前 |
| 1985-06-21 | 「Sound Glu ERS」の日付（題名と日付のみ確認） | [D S4 p.3][D S5 p.2] | 後年資料の引用 |
| 1985-08-08 | ブロック図に「ENSONIQ」と音源 RAM、Sound GLU（破線） | [D S2] | 1985以前 |
| 1986-02-14 | Custom ICs の予備ノートに Sound GLU の章 | [D S5 p.51–52] | 後年資料 |
| 1986-02-25〜26 | Apple の 3 名が Ensoniq を訪問。基板設計は「ほぼ凍結」 | [D S3 p.2, p.4] | 後年資料 |
| 1986-06-25 | Ensoniq DOC ERS Rev 1.0（E1）、Cortland Sound ERS Rev 1 | [D E1 p.1][D S7 p.1] | 後年資料 |
| 1986 | Apple と Ensoniq の契約 | [D E12] | 後年資料 |
| 1986-09-15 | IIGS 発売 | [D S11] | 後年資料 |
| 日付なし | 「Sound GLU fix」：緑色の量産前基板だけに適用する改修 | [D S6 p.4] | 不明 |

**当初予定の音源チップ**

- Apple II History：当初案のチップは「典型的なアーケードゲーム程度の音質」だった。Phoenix のハードウェア責任者 Rob Moore が Mirage の Ensoniq チップを提案し、「enabling technology」だと言って経営陣を説得した [D S9]。典拠は A+ 誌 1986-11 の記事（原典は未読）。後年の証言。
- Sound FAQ（1997）：当初は Mac II 系のチップに似たものを予定していた。4 音同時、左右 2 音ずつ、22,050 Hz 固定。Ensoniq が DOC のサンプルを送り、音楽好きの技術者が経営陣と戦って採用させた。DOC と 64K RAM は直前まで別売りオプションの予定だった [D S10]。後年の証言。
- 当初案のチップの品名は見つからない [V]。Mac II の ASC は 1987 年の製品で、FAQ は性能を比べているだけ。
- 1986 年の訪問者 3 名に Rob Moore が入っている [D S3 p.2]。

**1985 年中にサンプルが社外に出ていたか**：§0 のとおり。情報が出ていたことは 1985 年の資料で言える。現物の受け渡しの日付は資料にない [V]。

---

## 4. 1985年に日本のメーカーが DOC を知り得た経路

### 1985年以前の資料

| 経路 | 内容 | 根拠 |
|---|---|---|
| 英国誌の予告 | E&MM 1985-02「フランクフルトの前に」Malvern の会社のサンプリング鍵盤 Mirage（1,695 ドル）と Q Chip を紹介 | [D S13] |
| フランクフルト楽器見本市（1985-02） | 欧州初公開。小さなブースに 1 週間人だかり。英国で 2,000 ポンド未満と説明 | [D E6] |
| NAMM（1985-02） | 正式発表 | [D E5]。E7 も「February '85 NAMM」と書く。会期の日付は未確認 [V] |
| Summer NAMM（1985-06、ニューオーリンズ） | Mirage が「Apple ベースの音響解析ソフト」とともに展示 | [D S15] |
| 英国誌のレビュー | 1985-04、06、07 | [D E4][D E5][D E6] |
| 英国の輸入元 | Syco Systems Ltd | [D E5] |
| 販売店向け資料 | Service Bulletin 1985-06〜12 | [D S16] |
| 日本企業との取引 | Mirage に Epson 製のフロッピードライブが使われている（1985-09） | [D S16 No.3 p.1]。§7c |

- 1985 年に日本で Mirage が発売されたか、その代理店と価格：見つからない [V]。
- 1985 年の日本の雑誌での紹介：見つからない [V]。

### 後年資料

- Mirage DSK-8 の日本製の版があり、アジアとオーストラリア向け。日本語の説明書に英語の表紙で「Distributed by Nihon Hammond, Ltd.」と書かれていた [D E12（Ensoniq Mirage）]。Wikipedia でも出典なし（citation needed）。時期は不明 [V]。
- 日本語の個人サイト：機種一覧に「Mirage 国内生産 version」「ESQ-1 国内生産 version」がある。ESQ-1 の国内版はハモンド鈴木製。ENSONIQ JAPAN はイケベ楽器にいた人たちが中心になって設立。キーボード・マガジン 1986-10 号に ESQ-1 の記事（中島安貴彦）[D S23]。後年の証言。裏付けなし [V]。
- 1987 年、Ensoniq は日本に子会社を設立 [D E12][D S24]。1988 年の記事も「日本に進出した」と書く [D E7]。
- 日本ハモンド（Nihon Hammond）は Hammond の日本の分家で、1979 年に X-5 を出した。米国の Hammond Organ Company は 1985 年に廃業し、名前は鈴木楽器が買い取った [D S22]。

---

## 5. Ensoniq の1985年の状況

### 1985年以前の資料

- 1983 年春に Peripheral Visions（PVI）を設立。運転資金を得るために Atari の VCS 用キーボードの仕事を受けたが、ゲーム市場の崩壊で発売されなかった。Commodore が「この仕事は自社のもの」として PVI を訴え、1985-03 時点で係争中 [D S12]。
- 1985-03 時点の役職：Charpentier が技術担当副社長、Yannes が上級設計者、Ziembicki が生産管理マネージャ [D S12]。
- 注文が生産能力を大きく超えた。欧州版の生産をライセンスに出すなどで追いつこうとしている。米国では在庫が少なく、レビュー機を確保できなかった [D E5]。
- Q Chip の開発費は 100 万ドル以上 [D E5]。
- 欧州部門（ブリュッセル）がイタリアにライセンス生産の拠点を設けた。英国の希望小売価格は 1,795 ポンド [D E6]。イタリア製の欧州版は米国版より速いディスクドライブを積んだ [D E12（Ensoniq Mirage）] 後年資料。
- Service Bulletin に見る製造番号：No.1（1985-06-01）で製造番号 10500〜10725 の個体の不具合に触れる。No.7（1985-12-15）で「製造番号 14731 以降」が新仕様。No.8 の表は 00001〜14731 の範囲を挙げる [D S16 No.1, No.7, No.8]。
- 推定：1985 年末までに 1 万 4 千番台後半まで製造番号が付いた。ただし連番かどうかは不明。Wikipedia の「初年度約 8,000 台」とは合わない [V]。
- 1985 年中に、キーパッド配線、ハム、電源の電圧、ドライブの互換性などの不具合と改修が続いた [D S16 No.1〜No.8]。

### 後年資料

- 1986-02 に工場を拡張、約 1 年後に移転。Mirage は 1988 年初めまでに 25,000 台以上。創業者 Crockett は「Mirage の信頼性の記録は誇れない」と述べた。全製品を自社工場で生産（一時は欧州向けをイタリアで生産）。財務責任者は Malcolm Walter。欧州は Felix Visser との提携 [D E7]。
- Mirage は初年度に約 8,000 台。Great Valley の工場は 200 人近くを雇用（時期は不明）[D E12]。
- 検索結果の抜粋のみ：創業資金はコンサルティング収入 100 万ドル、ベンチャー資金 35 万ドル、銀行融資 10 万ドル（Reverb の記事。本文は取得できず）[V]。
- 1985 年に Ensoniq が半導体の外販で収入を得たかった事情を直接示す資料は見つからない [V]。

---

## 6. 類例：開発終盤の差し替えと、基板に残った痕跡

### Apple IIGS（1985〜86）

| 痕跡 | 内容 | 根拠 |
|---|---|---|
| オプション扱いの図 | 1985-08 のブロック図で、Sound GLU、音源 RAM、DOC だけが破線 | [D S2] |
| クロックの流用 | DOC を 7.14 MHz で動かす（文書の表記。公称は 8 MHz）。全 32 オシレータで約 26.3 kHz。波形 DRAM のリフレッシュ周期が DRAM の規格の最大値（70°C で 4 ms）をわずかに超えるが、Yannes は問題ないと答えた | [D S3 p.3]。量産 IIGS の DOC クロックは 7.159 MHz [D E11] |
| 出力回路の試行錯誤 | 最初は差動出力の片方だけを使い、発音の始めと終わりの DC オフセットでクリック音が出た。SIG− に反転アンプを入れて SIG+ と足したが、アンプの遅延で過渡ひずみが出た。Ensoniq は両方にアンプを入れてから足すよう勧めた。Apple は「基板設計はほぼ凍結済みだが、この変更はできる」と答えた | [D S3 p.3–4] |
| S/H の誤解 | Apple はフィルタの前にサンプルホールドを試したがうまくいかなかった。Ensoniq は「推奨したのは S/H ではなくゲート」と説明 | [D S3 p.4] |
| 量産前基板の手直し | 「Sound GLU fix（緑色の量産前基板のみ）」：Sound GLU のタイミング不具合で音源メモリのデータが失われ、クリック音やオシレータの停止が起きる。U35 のパターンを切り、U40 から配線を足す。基板レイアウトの誤りも同時に直す | [D S6 p.4] |
| 拡張端子 | Cortland Sound ERS は「8 チャンネルの独立出力に拡張可能（少しの追加回路が要る）」とし、拡張用に 7 ピンの Molex コネクタを置く | [D S7 p.2, p.7] |
| 〃 | DOC は 16 チャンネルの出力を出せるが、Apple の Molex 拡張コネクタは 8 チャンネルまで | [D S11] 後年資料 |
| 後の基板改訂 | 後年の ROM 3 基板で、Ensoniq の音に入っていたノイズの問題を解決 | [D S11] 後年資料 |

参考：IIGS の外装は、Apple IIc に続いて Hartmut Esslinger の Frog Design と組んだ 2 番目の大型の設計で、Snow White デザイン言語の始まり [D S11]。

### Atari 520ST と 65XEM（1984〜85）

- Atari の加算合成チップ AMY（仕様書は「Atari Semiconductor Group」1983-08-18）。初期の報道と開発ノートは 520ST の音源を AMY としていたが、発売時には市販の Yamaha YM2149 に替わった [D S21]。
- AMY 搭載の 65XEM は 1985-01 の CES で公開されたが、発売されなかった [D S21]。
- S21 は Wikipedia で、典拠は 1984〜85 年の InfoWorld、Practical Computing、Creative Computing。原典は未読 [V]。ST の基板に残った痕跡は見つからない [V]。

### Mirage 自身（1985）：二重調達と改修の痕跡

| 痕跡 | 内容 | 根拠 |
|---|---|---|
| ドライブの二重調達 | Epson 製と Shugart 製のドライブを併用。メイン基板のジャンパ（J1 の 10 番と 16 番）を Epson ならショート、Shugart ならカット。シャーシもドライブごとに専用で互換性なし | [D S16 No.3 p.1–2] |
| 電源の 2 系統 | ＋12 V／−12 V と＋12 V／−8 V の電源があり、メイン基板との組み合わせを確認する | [D S16 No.2 p.1][D S16 No.8 p.2] |
| Q chip の調整 | メイン基板の半固定抵抗 R49 が Q chip（U21）への音量基準電圧を決める。基板や電源の交換時に再調整が要る | [D S16 No.4 p.1] |
| 子基板の廃止 | 製造番号 14731 以降は重い鍵盤、新ケース、ノイズ低減回路、モジュール配置の変更。メイン基板から「tower board」、オペアンプ数個、R49 を廃止。旧機にはアップデートキット（新 EPROM「USA+」など） | [D S16 No.7 p.1][D S16 No.8 p.2] |

「tower board」が何を載せた子基板かは不明 [V]。

---

## 7. Ensoniq と日本・外部企業の接点

### a. 創業者の経歴と日本との接点

> 注（2026-11-25）：「日本に VIC-20 の組立工場を持ち」は、委託先を含む組立拠点と読むのが正確。日本での生産の実態と香港への移転は [sources.md](../sources.md) §03 を参照。

| 人物 | 経歴 | 根拠 |
|---|---|---|
| Albert Charpentier | MOS Technology の LSI グループの長。VIC チップ（1977、端末とゲーム機向け）を設計し、VIC-20 に使われた。C64 のビデオチップ。1985 年に Ensoniq の技術担当副社長 | [D S12][D S20] |
| Robert Yannes | MOS Technology の新人技術者のとき、自宅で「MicroPET」を設計し、Charpentier と Winterble の助けで試作機を完成。これを Tramiel が製品化させ、VIC-20 になった。C64 の SID。1985 年に Ensoniq の上級設計者 | [D S12][D S20] |
| Bruce Crockett | Commodore の製造担当副社長。C64 のデバッグと量産立ち上げを手伝った | [D E5][D S12] |
| Charles Winterble | Commodore の全世界の技術担当ディレクター。PVI の創業メンバーだが、1985-03 までに Coleco の電子部門グループ副社長に移った | [D S12] |
| David Ziembicki | Commodore の生産技術者。Ensoniq の生産管理マネージャ | [D S12] |

日本との接点：

- VIC-20 は日本で先に「VIC-1001」として 1980 年に発売された（日本語のカナを ROM とキーに持つ）[D S20]。
- 低価格カラー機の構想は Commodore Japan の GM、Tony Tokai も支持し、Yash Terakura が率いる Commodore Japan の技術チームが設計の仕上げを手伝った [D S20]。Wikipedia（典拠は Tomczyk の後年の証言）。Yannes や Charpentier が Terakura のチームと直接働いたかは不明 [V]。
- 1985 年の記事：Commodore は日本に VIC-20 の組立工場を持ち、C64 用ディスクドライブもそこで作る予定だった。VIC-20 のディスクドライブは日本で設計された。米国工場は自動挿入機を使い、日本の工場は使わなかったので、C64 の基板レイアウトでもめた（Charpentier の証言）。最終的に香港の新工場へ移った [D S12]。
- 日本のプログラマが書いた初期の C64 ゲームは、SID の（不正確な）仕様書どおりにコードを書いていた、と Yannes が述べている [D S12]（1985）。1996 年の取材でも同じ話をしている [D S19]。
- 創業者が個人として日本企業と取引した記録は見つからない [V]。

### b. 創業時の事業計画

- 1983 年春、PVI の計画は「もう 1 台コンピュータを設計すること」。運転資金のため Atari の VCS 用キーボードを受託したが、ゲーム市場の崩壊で未発売。Commodore が PVI を提訴 [D S12]（1985-03）。
- Wikipedia：創業者は Commodore に新しい家庭用コンピュータを提案したが Tramiel に拒まれ、自分たちで作る資金を得るため Atari の仕事を受けた。DOC の開発は、中止された Atari のキーボードの仕事より進んだ段階にあった（原文の文意はやや不明瞭）[D E12]。後年資料。
- 1982 年から Yannes はカスタムチップ技術のコンサルティング会社にかかわっていた [D E7]。後年の証言。
- 1984-01 の NAMM で PVI は Apple II 用ドラムカードと、サンプリング音の 200 ドルのドラムマシン試作を展示 [D E7]。後年の証言。
- 1984-08：PVI の「Drum-Key」は Apple II 用の電子音楽インターフェース基板。28 音、139 ドル [D S14]。
- 1985-02：PVI と Ensoniq は同じ住所で、同じ会社とみられる。DrumKey はカスタムの音源 ROM と DAC チップを使う [D S13]。
- E2 は DOC の用途に「アーケードや家庭用ビデオゲーム」を挙げる [D E2 p.1]（日付なし）。
- 1983 年の市場崩壊の影響：VCS 用キーボードの中止 [D S12]。

### c. Mirage の部品の調達先と製造委託

| 部品・工程 | 内容 | 根拠 |
|---|---|---|
| フロッピードライブ | Shugart SA300（1985-06）。Epson と Shugart を併用（1985-09） | [D S16 No.1 p.1][D S16 No.3 p.1] |
| 〃 | 初期の Mirage は最初に出荷された Microfloppy ドライブ（Shugart SA300）を積んだ | [D E12] 後年資料 |
| CPU | 8 bit の Rockwell 6500 と Motorola 68B09E | [D E5] |
| 表示 | 2 文字の英数字表示器 | [D E5] |
| 鍵盤 | 初代は Pratt-Reed 製 | [D E12]（出典なし）[V] |
| フィルタ | Curtis CEM3328 | [D E12][D S23] 後年資料 |
| 欧州版の製造 | イタリアでライセンス生産 | [D E6][D E7] |
| アジア・豪州向け | 日本製の DSK-8。「Distributed by Nihon Hammond, Ltd.」 | [D E12]（出典なし）[V] |
| 国内生産版 | Mirage と ESQ-1 の国内生産版。ESQ-1 はハモンド鈴木製 | [D S23] 後年の証言 [V] |

1985 年の資料で確認できた日本製部品は Epson のドライブだけ。直接購入か代理店経由かは不明 [V]。

### d. 1985年前後の日本企業との提携・販売契約・技術供与

- 1985 年の提携・販売契約・技術供与の記録は見つからない [V]。確認できたのは Epson のドライブの採用だけ [D S16 No.3]。
- 時期不明〜1987 年：日本ハモンドの販売、ハモンド鈴木の国内生産 [V]。
- 1987 年：日本子会社を設立 [D E12][D S24]。

### e. DOC を製造したファブ

- 1985 年の記事は「独自のカスタム IC を設計し（作っている）」と書く [D E6]。自社にファブがあったという記述はない。
- 1988 年：社内に VLSI 設計センターがある。チップは複数の会社が作れる、と Yannes。工場には自動組立・試験装置を入れた [D E7]。
- 日本語の個人サイト：DOC は ICS が製造 [D S23] 後年の証言 [V]。
- ICS は製造を外部に委託する会社なので、ウェハの製造会社は不明 [D S17][V]。
- 前職の MOS Technology は自社に製造ラインを持ち、C64 のチップを社内で試作した [D S12]。Ensoniq が自社ラインを持った記録はない [V]。
- チップの刻印や国名、ダイ写真は未確認 [V]。

---

## 8. 未解決事項

1. 「Sound Glu ERS」1985-06-21 版の本文。1985 年の時点で DOC 用だったか。
2. DOC の現物サンプルが Apple に届いた日付。送り主と経路。
3. IIGS の当初案の音源チップの品名。A+ 誌 1986-11「The Making of the Apple IIGS」の原文。
4. Ensoniq が 1985 年に DOC の外販や OEM 供給の意思を示した記事（Keyboard、Electronic Musician、Electronics 誌など 1985 年の米国誌）。
5. 「semiconductor group」の実在と時期。E2 の発行年（現状の推定は 1985 年半ば〜1987 年ごろ）。
6. ICS1261 の時期、ICS と Ensoniq の契約の形（第二ソースかリバッジか）。DOC のウェハの製造会社。
7. Mirage の日本での発売時期、代理店、価格。日本製 DSK-8 と日本ハモンドの関係の一次資料。1985 年の日本の雑誌記事（キーボード・マガジン等。国会図書館の雑誌記事索引は今回アクセス制限で未検索）。
8. ハモンド鈴木の ESQ-1 国内生産の時期と契約。
9. 1985 年の Ensoniq の資金繰りと売上。創業資金の内訳の原典。
10. Mirage の製造番号の付け方（連番か）と、1985 年の実際の出荷台数。
11. Mirage の「tower board」の中身。
12. Mirage のイタリアのライセンス生産会社の名前。
13. 1985 年に Apple 以外の会社が DOC を評価した記録。
14. Atari ST の AMY から YM2149 への切替の原典と、基板に残った痕跡。

---

## 資料一覧

参照日はすべて 2026-11-25。E1〜E12 の詳細は `ES5503.md` の出典表を参照。

| ID | 書誌 | 日付 | URL |
|---|---|---|---|
| E1 | Apple Computer, *Ensoniq DOC ERS*, Rev.1.0 | 1986-06-25 | https://www.brutaldeluxe.fr/documentation/cortland/v4_13_EnsoniqDOC.pdf |
| E2 | Ensoniq semiconductor group, *5503 Digital Oscillator Chip* | 日付なし | https://drive.google.com/open?id=1Gp_cQLwj2HpnbRTD2YzRYCJTPRN41RSn |
| E3 | Integrated Circuit Systems, *ICS 1261 (DOC1) Digital Oscillator Chip* | 日付なし | https://audiopro.r-massive.com/Ensoniq/Schematics/ICS1261%20%28rebadged%205503%20DOC%29.pdf |
| E4 | *International Musician & Recording World*, "Ensoniq Mirage" | 1985-04 | https://www.muzines.co.uk/articles/ensoniq-mirage/12460 |
| E5 | *One Two Testing*, "Ensoniq Mirage" | 1985-06 | https://www.muzines.co.uk/articles/ensoniq-mirage/11768 |
| E6 | *Electronics & Music Maker*, "Ensoniq Mirage"（Paul Wiffen） | 1985-07 | https://www.muzines.co.uk/articles/ensoniq-mirage/9076 |
| E7 | *Sound On Sound*, "Sampling Ensoniq"（Craig Anderton） | 1988-02 | https://www.muzines.co.uk/articles/sampling-ensoniq/3473 |
| E8 | R-Massive, "Ensoniq Technical Documents and Schematics"（資料一覧） | — | https://zine.r-massive.com/ensoniq-technical-documents-and-schematics/ |
| E11 | MAME `src/mame/apple/apple2gs.cpp` | 2026-11-24 取得 | https://github.com/mamedev/mame/blob/master/src/mame/apple/apple2gs.cpp |
| E12 | Wikipedia "Ensoniq"、"Ensoniq Mirage" | 2026-11-25 参照 | https://en.wikipedia.org/wiki/Ensoniq 、https://en.wikipedia.org/wiki/Ensoniq_Mirage |
| S1 | Fern Bachman, "Tools Overview/Notes", Document Version 00:00（Apple 社内） | 1985-06-17 | https://www.brutaldeluxe.fr/documentation/cortland/v3/Tools%20Overview%20and%20Notes%20-%20Fern%20Bachman%20-%20v00.00%20-%2019850617.pdf |
| S2 | "CORTLAND BLOCK DIAGRAM"（D.L.H.）1 枚 | 1985-08-08 | https://www.brutaldeluxe.fr/documentation/cortland/v5/Cortland%20Block%20Diagram%20-%2019850808.pdf |
| S3 | Allen Watson, "Notes on Ensoniq DOC", Apple Technical Publications（訪問 1986-02-25〜26） | 1986-03-05 | https://www.brutaldeluxe.fr/documentation/cortland/v3_04_NotesOnEnsoniqDOC.pdf |
| S4 | *Cortland Hardware Reference Manual*, Beta Draft（典拠 ERS の一覧は p.3） | 1986-09-19 | https://www.brutaldeluxe.fr/documentation/cortland/v3_01_CortlandHardwareReference.pdf |
| S5 | Wayne Lowry, "Cortland Custom ICs, Preliminary Notes" | 1986-02-14 | https://www.brutaldeluxe.fr/documentation/cortland/v5/Cortland%20Custom%20ICs%20-%20Wayne%20Lowry%20-%20Preliminary%20notes%20-%2019860214.pdf |
| S6 | "Sound GLU Fix (Green pre-production boards only)"（PERITEL 資料と合本） | 日付なし | https://www.brutaldeluxe.fr/documentation/cortland/v5/Cortland%20VGC%20Scart%20Sound%20glu%20fix.pdf |
| S7 | Apple, "Cortland Sound ERS", Rev. 1 | 1986-06-25 | https://www.brutaldeluxe.fr/documentation/cortland/v4_13_CortlandSound.pdf |
| S8 | Brutal Deluxe, Cortland 資料の索引（各文書の著者と日付） | — | https://brutaldeluxe.fr/documentation/cortland.html |
| S9 | Steven Weyhrich, *Apple II History*, "10-The Apple IIGS"（典拠：A+ 1986-11 ほか） | — | https://www.apple2history.org/history/ah10/ |
| S10 | Ian Schmidt, "Apple II Sound & Music FAQ", Version 1.7 | 1997-11-03 | https://www.electrickery.nl/comp/apple2c/apple2faq/app2soundfaq.html |
| S11 | Wikipedia "Apple IIGS" | 2026-11-25 参照 | https://en.wikipedia.org/wiki/Apple_IIGS |
| S12 | Tekla S. Perry, Paul Wallich, "Design case history: the Commodore 64", *IEEE Spectrum* 22(3) pp.48–58（Web 再掲。末尾に 1985-03 時点の後日談） | 1985-03 | https://spectrum.ieee.org/commodore-64 |
| S13 | *Electronics & Music Maker*, "Computer Musician – Rumblings" | 1985-02 | https://www.muzines.co.uk/articles/computer-musician-rumblings/1813 |
| S14 | *COMPUTE!* #51, "Percussion Emulator For Apple", p.135 | 1984-08 | https://www.atarimagazines.com/compute/issue51/237_1_NEWS_PRODUCTS_Percussion_Emulator_For_Apple.php |
| S15 | *One Two Testing*, "NAMM (Part 1)" | 1985-08 | https://www.muzines.co.uk/articles/namm/11827 |
| S16 | Ensoniq, Mirage Service Bulletin No.1（1985-06-01）、No.2（1985-08-01）、No.3（1985-09-01）、No.4（1985-10-01）、No.7（1985-12-15）、No.8「Mirage Update」（1985-12-15）、No.9（1986-03-01）。No.5 は未使用、No.6 は一覧になし | 1985-06〜1986-03 | E8 の一覧から Google Drive（No.1 https://drive.google.com/file/d/1ONGEKVMezYTBQ0UoAstTnT7PQCr3zun4 、No.3 https://drive.google.com/file/d/1zgSmL9Ucw2p1FyMxMfi2b1D3nS4uTYSd 、No.7 https://drive.google.com/file/d/1BP-p2UFdpTqCWhUStHn8HjR02AbrvX1w 、No.8 https://drive.google.com/file/d/1SbvynMn9e1DDQkP-NWHjAQc0aEL8VNWZ ほか） |
| S17 | Integrated Circuit Systems, *1994 Data Book* | 1994 | https://ftp.mirrorservice.org/sites/www.bitsavers.org/pdf/integratedCircuitSystems/1994_Integrated_Circuit_Systems_Data_Book.pdf |
| S18 | Integrated Circuit Systems, SEC Form S-3（会社の沿革、外部ファウンドリへの依存） | 1998-02-27 | https://www.sec.gov/Archives/edgar/data/874689/0001036050-98-000295.txt |
| S19 | Andreas Varga, "Interview with Bob Yannes"（電子メール取材） | 1996-08 | https://landley.net/history/mirror/commodore/yannes.html |
| S20 | Wikipedia "VIC-20" | 2026-11-25 参照 | https://en.wikipedia.org/wiki/VIC-20 |
| S21 | Wikipedia "Atari AMY" | 2026-11-25 参照 | https://en.wikipedia.org/wiki/Atari_AMY |
| S22 | Wikipedia "Hammond organ" | 2026-11-25 参照 | https://en.wikipedia.org/wiki/Hammond_organ |
| S23 | 個人サイト「ENSONIQ Synthesizers」（約 20 年前の記事を 2019 年に改訂、と記載） | 2019 改訂 | https://www.asahi-net.or.jp/~wz4k-tnk/MY/ensoniq.html |
| S24 | Wikipedia 日本語版「エンソニック」 | 2026-11-25 参照 | https://ja.wikipedia.org/wiki/エンソニック |
| S25 | Integrated Circuit Systems, *ICS1399 Digital Sound Generator (DOCII)*（13 頁） | 日付なし | https://audiopro.r-massive.com/Ensoniq/Schematics/ICS1399%20%28rebadged%205504%20DOC%20II%29.pdf |
| S26 | *inCider*, "The IIgs Hardware Report"（1986 年 11 月号、CW Communications。Internet Archive の全文テキスト） | 1986-11 | https://archive.org/stream/inCider_86-11/inCider_86-11_djvu.txt |

資料の保存先（リポジトリ外）：`<scratchpad>\supply\pdf\`（S1〜S7、S16、S17、S25 など）。OCR の結果は `<scratchpad>\supply\` の `ocr_sb\`（S16）、`ocr_doc\`（S3）、`ocr_ics\`（E3、S25）。E1〜E3 の PDF は `ES5503.md` と同じく `<scratchpad>\audio\pdf\`。
