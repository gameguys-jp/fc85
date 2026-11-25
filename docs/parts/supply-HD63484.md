# HD63484 ACRTC 供給・入手性 台帳
Phase 0 調査 v0.1 ／ 調査日 2026-11-25

凡例：`[D 資料ID 頁]` 原典に根拠あり。`[V]` 未確認。「推定」は本台帳での推論。出典IDは末尾の表を参照。
資料の区分：**A** は1984年以前の刊行、**B** は1985年中の刊行。A・B は年代の根拠に使える。**C** は1986年以降の刊行（後年資料）。C は傾向の参考にとどめ、年代の根拠には使わない。
`HD63484.md` で調べ済みの事項（日立評論1984年7月、1984年11月と1985年5月のマニュアル、1985年データブックの「Under development」、1986年の採用率22%）は繰り返さない。同台帳の出典ID（H1, U84, U85, DB85, AN86）はそのまま参照する。

---

## 1. 結論の要約

| 問い | 結論 |
|---|---|
| 1985年中にサンプルまたは製品が出ていたか | **出ていた証拠がある。** 日立は1985年1月の米国誌広告で「すぐに使える」と宣伝した [D ED850110 p.191–192]。1985年6月に HD63484 を使う PC 用ボードが「7月下旬発売」で発表された [D EL850617 p.61]。1985年8月には複数社のボードが価格付きで載った [D MMS8508]。同じ8月付けで -8 品の正式データシートが出た [D HDS8508]。ただし1985年中の出荷を直接示す資料はない [V] |
| 1 サンプル・量産・価格・速度品 | 日本での発表は1984年7月の製品紹介まで遡れる [D HY8407P p.562]。サンプル開始日・量産開始日・サンプル価格は見つからない [V]。価格は1986年初めに1000個時 $38.50 [D EDN860206 p.111]。最初に出回ったのは -8 品とみられる（推定） |
| 2 初期の採用製品 | 米国の PC 用グラフィックボードが最初。Number Nine（1985年6月発表）、Modgraph・ID Systems・Frontier・C.S.D.（1985年8月）、Imagraph・Basu（1985年12月）。英・伊にも1985年の製品がある。日本製品は1985年には見つからない [V] |
| 3 日本メーカーのサンプル入手 | 直接の資料はない [V]。日立は1984年に国内で製品紹介と技術記事を出していた [D HY8407P; IO8410]。1985年7月の国内誌は「すでに発表されている」と書く [D IO8507] |
| 4 HM50464 | 日立自身の1985年5月メモリデータブックには載っていない [D MEM8505]。1985年の部品表に代替品として名前がある [D IRON8508 p.115]。日立のデータブックに載るのは1987年3月が最初 [D MEM8703 p.300]。1985年の入手は弱い（推定） |
| 5 Amagaki | ACRTC と HM50464 の入手性を扱ったログはまだない [D AMG] |

---

## 2. 年表

| 年月 | できごと | 根拠 | 区分 |
|---|---|---|---|
| 1984年6月 | 日本語ユーザーズマニュアル第1版の発行（個人サイトの記述） | [D ELR]、事実は [V] | C（後年の二次資料） |
| 1984年6月14日頃 | Electronic Design 誌に日立技術者の技術記事。日本語訳が I/O 1984年10月号に載る | [D IO8410 pp.230–233] | A |
| 1984年7月 | 日立評論の「製品紹介」に HD63484 が載る。表1 にクロック周波数として -4（4 MHz）、-6（6 MHz）、-8（8 MHz）。セラミック DIP の写真付き。価格・サンプル時期の記載はない | [D HY8407P p.562] | A |
| 1984年8月 | 米 Computer Design 誌の新製品欄に HD63484 が載る（Hitachi America）。価格の記載はない | [D CD8408 System Components 欄] | A |
| 1984年11月 | 米国版ユーザーズマニュアル | [D U84 表紙]（HD63484.md） | A |
| 1985年1月 | Hitachi America の見開き広告「Ready for Immediate Application」。用途に「アーケードゲーム機」「高度なパーソナルコンピュータ」を挙げる。8 MHz で最大 200万画素/秒。価格の記載はない | [D ED850110 pp.191–192; EW850121; EW850218 p.81] | B |
| 1985年5月 | 米誌のグラフィックチップ記事で HD63484 を紹介（8 MHz） | [D DD8505 図3] | B |
| 1985年5月 | 日立の米国向けメモリデータブック #M11。HM50464 は載っていない | [D MEM8505 目次] | B |
| 1985年5月 | 米誌の特集で「日立の新しいグラフィックチップ」、約11万7千トランジスタ、500 ns/画素 | [D EW850527] | B |
| 1985年6月17日 | Number Nine の Revolution ボード4種を発表。「HD63484 を使う最初の製品群の一つ」。7月下旬発売予定。$1,995〜$3,995 | [D EL850617 p.61] | B |
| 1985年7月 | 国内誌の HD64180 インタビューで「すでに発表されている ACRTC(HD63484)」 | [D IO8507] | B |
| 1985年7月 | SIGGRAPH（7月）の PC AT 用高解像度ボードは「大半が NEC 7220 か日立 HD63484 を使う」 | [D EL850729] | B |
| 1985年7月 | NCC（シカゴ、7月15〜18日）で Microware・日立・Graphic Software Systems が ACRTC 用ソフトの共同発表。OS-9 ドライバは JIS 漢字に対応 | [D MW85; RB8510] | B |
| 1985年8月 | 米誌の記事と表で、HD63484 を使うボードを C.S.D.、Frontier、Modgraph、ID Systems、Number Nine が出す | [D MMS8508] | B |
| 1985年8月 | 英国の BBC Micro 用 PRISMA II（1 MB、832×575）が「最新の日立 63484」を使う | [D ABC8508] | B |
| 1985年8月 | 日立 #U149「HD63484-8 Advanced CRT Controller (ACRTC)」データシート（August 1985）。品名は HD63484-8（DC-64）、HD63484P-8（DP-64）、HD63484CP-8（CP-68）で、すべて 8 MHz。初頁に「ADVANCE INFORMATION」の表示はない | [D HDS8508 leaf n1142] | B（1986年版 IC Master への転載で確認） |
| 1985年8月 | 米 Ironics 社の部品表で、64K×4 DRAM の代替品に HM50464-15 | [D IRON8508 p.115] | B |
| 1985年10月 | 伊 Datatech 社の PC 用グラフィックボード SA001 が「新しい日立 ACRTC」を使う | [D STE8510] | B |
| 1985年10月 | BYTE 誌の製品欄に Number Nine Revolution 2048×4（$1,495〜$3,995） | [D BY8510] | B |
| 1985年12月 | Imagraph の AGC シリーズ（$1,995〜$4,495）。「納期30日」 | [D EL851223] | B |
| 1985年12月 | Basu の VMEbus ボード（$1,795／$2,885）。「日立チップを使う最初期の基板メーカーの一つ」 | [D MMS8512] | B |
| 1985年12月 | 「日立が最近 63484 を出した」 | [D MC8512] | B |
| 1986年1月 | PRISMA 2 が Millipede 社から £2,195 で入手可能 | [D AU8601] | C |
| 1986年2月 | EDN の比較表で HD63484-8 が 1000個時 $38.50。代替供給元は Signetics。「未発売」「サンプルのみ」の印はない | [D EDN860206 pp.110–111] | C |
| 1986年2月 | Pericom のカラーグラフィック端末 MX-4000 が HD63484 を使う | [D CW860217] | C |
| 1986年4月 | 日立のアプリケーションノートの設計例が HM50464-12 を4個使う | [D AN86 p.130]（HD63484.md） | C |
| 1986年5月 | 日立マイクロコンピュータエンジニアリングのグラフィックボード M80HGD01。ACRTC と HM50464×12。定価未定 | [D IO8605 p.183] | C |
| 1986年秋 | 米ボード会社の投書「ACRTC は出て1年半、$50、PC 用は1月から」 | [D PCTJ8611 投書欄] | C |
| 1987年3月 | 日立メモリデータブックに HM50464P/CP-12/-15/-20 | [D MEM8703 p.300] | C |
| 1987年8月 | 日立データブック #U70-A で HD63484-4/-6/-8 に開発中の印がない | [D U70A87 p.472 ほか] | C |

---

## 3. 問1　サンプル・量産の時期、価格、速度品

### 3.1 わかったこと

- 日本では1984年7月に「製品紹介」として出ている。表1 に -4／-6／-8 の3品種が並ぶ [D HY8407P p.562]。
- 米国では1984年8月に新製品として紹介された [D CD8408]。1985年1月には見開き広告で「すぐに使える」と宣伝した [D ED850110 pp.191–192]。
- 1985年8月付けの米国データシートは -8 品だけを載せる。外形は DC-64、DP-64、CP-68 の3種 [D HDS8508]。初頁に「ADVANCE INFORMATION」の表示はない。
- 1986年2月の比較表では、HD63484-8 が普通に買える部品として載る。1000個時 $38.50 [D EDN860206 p.111]。
- 1986年秋の米ボード会社の投書では「出て1年半、$50」 [D PCTJ8611]。逆算すると1985年初めに出回り始めたことになる（推定）。
- 1987年8月のデータブックでは -4／-6／-8 のどれにも開発中の印がない [D U70A87]。
- 日立自身の後年の記述は「1984年に製品化」 [D HY8707]。

### 3.2 速度品の順番（推定）

- 米国で最初に出回ったのは -8 品とみられる。根拠は、1985年1月の広告が 8 MHz の性能を示すこと [D ED850110]、1985年8月のデータシートが -8 だけを載せること [D HDS8508]、1986年の比較表が -8 だけを載せること [D EDN860206]。
- -4 と -6 が1985年に出荷されていたかは確認できない [V]。

### 3.3 「開発中」の印が外れた時期

- 1985年のデータブック（月不明）では3品種とも「Under development」 [D DB85 p.8]（HD63484.md）。
- -8 品は1985年8月のデータシートで「ADVANCE INFORMATION」が消えている [D HDS8508]。-8 品はこの頃に開発中を外れたとみられる（推定）。
- 3品種そろって印がないのが確認できるのは1987年8月 [D U70A87]。1986年の日立データブックは見つけられなかった [V]。

### 3.4 わからなかったこと

- サンプル出荷の開始日と量産開始日 [V]。
- 1984〜85年のサンプル価格と量産価格（日本・米国とも） [V]。
- 日経エレクトロニクス 1984年5月21日号（No.343）の記事本文。サンプル価格が載っている可能性があるが、読めなかった [V]。

---

## 4. 問2　初期の採用製品

| 製品 | 会社（国） | 初出 | 内容 | 根拠 | 区分 |
|---|---|---|---|---|---|
| Revolution 1280×4 ほか4種 | Number Nine Computer（米） | 1985年6月17日 | IBM PC 用。1 MB。「7月下旬発売」。$1,995〜$3,995 | [D EL850617 p.61; DD8508; BY8510] | B |
| IHR 2 | Frontier Technologies（米） | 1985年8月 | PC 用。価格は表で NA | [D MMS8508 表] | B |
| ID-1501 | ID Systems（米） | 1985年8月 | 表で $2,950 | [D MMS8508 表] | B |
| GX2250／GX2150 | Modgraph（米） | 1985年8月 | 表で $2,500／$2,400。表の「*」の意味は未確認 | [D MMS8508 表]、注記は [V] | B |
| 製品名不明 | C.S.D.（米） | 1985年8月 | 本文で HD63484 採用社として名前だけ | [D MMS8508 本文] | B |
| PRISMA II（PRISMA 2） | Millipede（英） | 1985年8月 | BBC Micro 用、1 MB、832×575。1986年1月に £2,195 | [D ABC8508; AU8601] | B／C |
| SA001 | Datatech（伊） | 1985年10月 | PC 用。512×512、最大 2048×2048 | [D STE8510] | B |
| AGC-1024P ほか | Imagraph（米） | 1985年12月 | PC/XT/AT 用。$1,995〜$4,495。納期30日 | [D EL851223; DD8512] | B |
| VMEbus ボード | Basu（米） | 1985年12月 | 512 KB で $1,795、2 MB で $2,885 | [D MMS8512] | B |
| MX-4000 | Pericom（米） | 1986年2月 | カラーグラフィック端末。68000 と 6809 | [D CW860217] | C |
| DC1-4 | Dolen Computer（米） | 1986年1月（自社の主張） | PC 用 | [D PCTJ8611] | C |
| M80HGD01 | 日立マイクロコンピュータエンジニアリング（日） | 1986年5月 | カード型 PC M80HMP01（HD64180）用。HM50464×12。640×400、8色、4画面。定価未定 | [D IO8605 p.183] | C |
| クラウン用ナビ | デンソー／トヨタ（日） | 1987年 | ACRTC を使用 | [D SHMJ] | C |

- 1985年7月の SIGGRAPH では、PC AT 用高解像度ボードの大半が NEC 7220 か HD63484 だった [D EL850729]。
- 1985年12月の記事は「多くのグラフィックボードが使う」と書く [D DD8512]。
- 1985年中に実際に出荷された製品は、記事の書き方（「発売予定」「納期30日」）からはあったとみられる（推定）。出荷実績を直接示す資料はない [V]。
- 1985年の日本製品は見つからない [V]。日本で最初に確認できたのは日立グループの M80HGD01（1986年5月、定価未定） [D IO8605]。

---

## 5. 問3　日本メーカーのサンプル入手（1984〜85年）

### 5.1 わかったこと

- 日立は1984年7月に日立評論の「製品紹介」で国内向けに紹介した [D HY8407P p.562]。
- 日立技術者の技術記事が1984年10月に国内誌に翻訳で載った [D IO8410 pp.230–233]。入手方法・価格の記載はない。
- 1985年7月の国内誌インタビューは「すでに発表されている ACRTC」と書く [D IO8507]。
- 1985年7月、日立は米国で OS-9 用ドライバの共同発表に加わった。ドライバは JIS 漢字に対応する [D MW85]。日本市場も意識していたとみられる（推定）。
- 個人サイトによれば、初期のマスク版（R、S）には多くの制限事項があり、後に U マスクになった [D ELR]（事実としては [V]）。

### 5.2 推定

- 1984年中に製品紹介とマニュアルが出ているので、日立の特約店や営業経由で国内メーカーがサンプルを得ることはできたとみられる（推定）。
- 1985年の試作機に載るのは、R または S マスクの初期品だった可能性がある（推定。マスク版ごとの時期は [V]）。

### 5.3 わからなかったこと

- 国内のサンプル価格、サンプル受付の条件（数量・契約）、国内での量産開始時期 [V]。
- 1985年に ACRTC を載せた国内試作機・製品の実例 [V]。

---

## 6. 問4　HM50464（64K×4 DRAM）の1985年の入手性

### 6.1 わかったこと

- 日立の米国向けメモリデータブックには、1984年3月版 [D MEM8403]、1984年 M10 [D MEM84]、1985年5月 M11 [D MEM8505] のどれにも HM50464 がない。1984年の日立クイックリファレンスにもない [D QRG84]。
- IC Master 1985（第2巻）の全文検索で「HM50464-12, -15, -20 65,536 x 4 Bits Dynamic Random Access … Hitachi」が出る [D IOM85]。頁の画像は確認できていない。
- 1985年8月の米国製ボードの部品表で、富士通 MB81464-15 の代替品として TI TMS4464-15、NEC µPD41254-15、日立 HM50464-15 が並ぶ [D IRON8508 p.115]。代替品の一覧なので、日立品が実際に使われた証拠ではない。
- 1986年4月の日立アプリケーションノートの設計例は HM50464-12 を4個使う [D AN86 p.130]（HD63484.md）。
- 1986年5月の日立グループのボードは HM50464 を12個使う [D IO8605 p.183]。
- 日立のメモリデータブックに HM50464P／CP（-12／-15／-20）が載るのは1987年3月が最初 [D MEM8703 p.300]。

### 6.2 推定

- 1985年の HM50464 は、発表済みでサンプル程度の段階だったとみられる。量産品としての裏付けは弱い。
- 64K×4 の -15（150 ns）品は、1985年8月の時点で富士通・TI・NEC・日立の4社の品番が部品表に並ぶ [D IRON8508 p.115]。-15 品は1985年に複数社から入手できたとみられる。
- -12（120 ns）品が1985年に入手できたかは確認できない [V]。

---

## 7. 問5　Amagaki（hackaday.io）のログ

- 2026-11-25 時点のログは、CPU の入手性（2025-05-08）、DMAC の入手性（2025-05-13）、画像生成の話の3本。ACRTC と HM50464 を扱うログはない [D AMG]。
- Amagaki は1987年の機械という設定で、HD63484 を 6.275 MHz で使う [D AMG]。fc85（1985年）より2年遅い。
- DMAC のログは、1985年の #U70 で HD68450-12 が「under development」、HD63450 は1987年8月の #U70-A に初めて載る、と調べている [D AMG DMAC ログ]。データブックの開発中の印で時期を判断する方法は本台帳と同じ。

---

## 8. fc85 への示唆（推定。仕様書は未変更）

1. 1985年の最終試作で ACRTC を使う設定は無理がない。1985年1月の米国広告と、1985年6月以降の基板製品が裏付けになる。
2. 速度品は -8 品が最も早く出回った形跡がある。2CLK 5.37 MHz を -8 品で動かす設定が最も裏付けが厚い。-6 品の1985年の入手性は未確認。
3. 試作機の ACRTC は初期マスク品で、制限事項があった可能性がある（[V]）。
4. 表示用 DRAM は、1985年の実績が確認できる -15 品（富士通 MB81464、TI TMS4464、NEC µPD41254、日立 HM50464）から選ぶ設定が安全。日立品だけに頼る設定は裏付けが弱い。

---

## 9. 未解決事項

1. HD63484 のサンプル出荷開始日と量産開始日（日本・米国）。
2. 1984〜85年のサンプル価格・量産価格。日経エレクトロニクス 1984年5月21日号（No.343 pp.221–254）が候補。
3. -4／-6 品が1985〜86年に出荷されていたか。1986年の日立データブック（#U70 系）が見つかっていない。
4. 1985年のデータブック DB85 の発行月。これが分かれば「開発中」の時期が絞れる。
5. 1984年6月の日本語マニュアル第1版の実物と、R／S／U マスクの時期・制限事項の内容。
6. 1985年中に出荷された製品の直接の証拠（出荷記事、製品写真のデートコードなど）。
7. 1985年の国内での採用例・試作例。
8. HM50464 の日本国内での1985年の入手性と、-12 品の時期。IC Master 1985 の該当頁の画像確認。
9. MMS8508 の表で Modgraph の価格に付いた「*」の意味。
10. I/O 1986年6月号の ACRTC 製作記事の詳細（OCR が崩れており未精読）。

---

## 出典

参照日はすべて 2026-11-25。取得した PDF・画像はリポジトリ外の `<scratchpad>\supply\` に置いた。リポジトリには入れていない。

| ID | 区分 | 書誌 | 日付 | URL |
|---|---|---|---|---|
| HY8407P | A | 日立評論 Vol.66 No.7、製品紹介「グラフィック描画・表示を行なう高性能CRTコントローラ」、p.562（日立製作所 半導体事業部） | 1984-07-25 発行 | https://www.hitachihyoron.com/jp/pdf/1984/07/1984_07_00_sinseihin.pdf |
| IO8410 | A | I/O 1984年10月号（工学社）pp.230–233。Electronic Design 1984年6月14日号の日立技術者（Yonezawa ほか）の記事の翻訳 | 1984-10 | https://archive.org/details/Io198410 |
| CD8408 | A | Computer Design Vol.23 No.9、System Components 欄「Controller provides graphics functions in CMOS VLSI」（頁は未確認） | 1984-08 | https://archive.org/details/bitsavers_ComputerDe23N09198408_170603952 |
| MEM8403 | A | Hitachi IC Memory Data Book（March 1984） | 1984-03 | http://bitsavers.org/components/hitachi/_dataBooks/1984_Hitachi_IC_Memory_Data_Book_Mar84.pdf |
| MEM84 | A | Hitachi IC Memories Data Book #M10 | 1984 | http://bitsavers.org/components/hitachi/_dataBooks/1984_M10_Hitachi_IC_Memories_Data_Book.pdf |
| QRG84 | A | Hitachi IC and Discrete Quick Reference Guide | 1984 | http://bitsavers.org/components/hitachi/_dataBooks/1984_Hitachi_IC_and_Discrete_Quick_Reference_Guide.pdf |
| ED850110 | B | Electronic Design Vol.33 No.1、pp.191–192、Hitachi America 広告 | 1985-01-10 | https://archive.org/details/bitsavers_ElectronicignV33N0119850110_258040560 |
| EW850121 | B | ElectronicsWeek、同じ日立広告（頁は未確認） | 1985-01-21 | https://archive.org/details/electronics_week-1985_01_21 |
| EW850218 | B | ElectronicsWeek、p.81、同じ日立広告 | 1985-02-18 | https://archive.org/details/electronics_week-1985_02_18 |
| DD8505 | B | Digital Design Vol.15 No.5、グラフィックチップ記事、図3「The 63484 from Hitachi」 | 1985-05 | https://archive.org/details/bitsavers_DigitalDesN05198505_74324840 |
| MEM8505 | B | Hitachi IC Memories Data Book #M11（Hitachi America、裏表紙に MAY 1985） | 1985-05 | http://bitsavers.org/components/hitachi/_dataBooks/1985_M11_Hitachi_IC_Memories_Data_Book.pdf |
| EW850527 | B | ElectronicsWeek、コンピュータグラフィックス特集 | 1985-05-27 | https://archive.org/details/electronics_week-1985_05_27 |
| EL850617 | B | Electronics、p.61、Number Nine Revolution の記事（Tobias Naegele） | 1985-06-17 | https://archive.org/details/electronics-1985_06_17 |
| IO8507 | B | I/O 1985年7月号、HD64180 に関するインタビュー（頁は未確認） | 1985-07 | https://archive.org/details/Io19857 |
| EL850729 | B | Electronics、SIGGRAPH の報告記事 | 1985-07-29 | https://archive.org/details/electronics-1985_07_29 |
| MW85 | B | Microware, Pipelines Vol.1 No.1、NCC '85 の報告と ACRTC 記事 | 1985 秋 | https://archive.org/details/microware-pipelines-v01n01 |
| RB8510 | B | The Rainbow 1985年10月号、同じ共同発表の報告 | 1985-10 | https://archive.org/details/rainbowmagazine-1985-10 |
| MMS8508 | B | Mini-Micro Systems、PC 用グラフィックボードの記事と表 | 1985-08 | https://archive.org/details/bitsavers_MiniMicroS_107798794 |
| DD8508 | B | Digital Design Vol.15 No.8、グラフィック記事（Number Nine が ACRTC を使う）。1985年9月号にも同文 | 1985-08 | https://archive.org/details/bitsavers_DigitalDesN08198508_59877862 |
| ABC8508 | B | A&B Computing、PRISMA II の紹介 | 1985-08 | https://archive.org/details/AB_Computing_1985-08_OCR |
| IRON8508 | B | Ironics IV-1624 シリアルカードのマニュアル、部品表 p.115（archive.org の leaf 番号） | 1985-08 | https://archive.org/details/bitsavers_ironicsIV1SerialCard198508_6736989 |
| HDS8508 | B | Hitachi #U149「HD63484-8 Advanced CRT Controller (ACRTC)」August 1985。IC Master 1986 Vol.1 への転載、leaf n1142 | 1985-08（転載は1986） | https://archive.org/details/bitsavers_icMaster19_273501026 |
| IOM85 | B | IC Master 1985 Vol.2。全文検索の抜粋のみ（貸出資料のため頁画像は未確認） | 1985 | https://archive.org/details/bwb_S0-EGU-558_2 |
| STE8510 | B | Selezione di Tecniche Elettroniche 1985年10月号、Datatech SA001 | 1985-10 | https://archive.org/details/Selezione_di_Tecniche_Elettroniche_1985_10 |
| BY8510 | B | BYTE Vol.10 No.10、製品欄（Number Nine Revolution 2048×4） | 1985-10 | https://archive.org/details/byte-magazine-1985-10 |
| EL851223 | B | Electronics、Imagraph AGC の記事（Tom Manuel） | 1985-12-23 | https://archive.org/details/electronics-1985_12_23 |
| MMS8512 | B | Mini-Micro Systems、Basu の VMEbus ボードの記事（Mike Seither） | 1985-12 | https://archive.org/details/bitsavers_MiniMicroS_91321993 |
| DD8512 | B | Digital Design Vol.15 No.12、Imagraph AGC-1024P の記事 | 1985-12 | https://archive.org/details/bitsavers_DigitalDesN12198512_58032951 |
| MC8512 | B | Micro Cornucopia 1985年12月号 | 1985-12 | https://archive.org/details/micro-cornucopia-magazine-1985-12 |
| AU8601 | C | Acorn User No.42、ニュース欄（PRISMA 2、Millipede） | 1986-01 | https://archive.org/details/AcornUser042-Jan86 |
| EDN860206 | C | EDN、Special Report: Graphics-controller ICs（pp.104–）、本文 p.110–111、表 p.111 | 1986-02-06 | https://archive.org/details/edn-1986_02_06 |
| CW860217 | C | Computerworld、Pericom MX-4000 の記事 | 1986-02-17 | https://archive.org/details/computerworld207unse |
| IO8605 | C | I/O 1986年5月号 p.183、Big New Products（M80HGD01） | 1986-05 | https://archive.org/details/Io19865 |
| IO8606 | C | I/O 1986年6月号、ACRTC の製作記事 | 1986-06 | https://archive.org/details/Io19866 |
| PCTJ8611 | C | PC Tech Journal Vol.4 No.11、投書（Phillip Woellhof, Dolen Computer Corp.） | 1986-11 | https://archive.org/details/PC_Tech_Journal_vol04_n11 |
| MEM8703 | C | Hitachi IC Memories Data Book #M11、p.300（HM50464） | 1987-03 | http://bitsavers.org/components/hitachi/_dataBooks/1987_M11_Hitachi_IC_Memories_Data_Book.pdf |
| HY8707 | C | 日立評論 Vol.69 No.7、表示系 LSI ファミリーの記事 | 1987-07 | https://www.hitachihyoron.com/jp/pdf/1987/07/1987_07_08.pdf |
| U70A87 | C | Hitachi 8/16-Bit Peripheral LSI Data Book #U70-A、p.472 ほか | 1987-08 | http://bitsavers.org/components/hitachi/_dataBooks/1987_U70-A_Hitachi_8_16_Bit_Peripheral_LSI_Data_Book.pdf |
| SHMJ | C | 半導体産業人協会 日本半導体歴史館「Advanced CRT controller」展示（英語版、Version 2022/4/27） | 2022 | https://www.shmj.or.jp/english/pdf/ic/exhibi759E.pdf |
| ELR | C | Electrelic「Hitachi HD63484 ACRTC」（個人サイト、二次資料） | 不明 | https://electrelic.com/electrelic/node/9 |
| ITO | C | itofamily のコレクション頁「HD63484」（個人サイト、二次資料。1984年頃の発表、R→S→U のマスク版） | 不明 | http://www.itofamily.com/ito/collections/peripherals/hd63484/index.html |
| AMG | C | Amagaki（hackaday.io project 203046）と、ログ「Were the chips available? CPU」「同 DMAC」 | 2025-05 | https://hackaday.io/project/203046-amagaki-the-macintosh-killer ／ https://hackaday.io/project/203046-amagaki-the-macintosh-killer/log/240476-were-the-chips-available-cpu ／ https://hackaday.io/project/203046-amagaki-the-macintosh-killer/log/240601-were-the-chips-available-dmac |
