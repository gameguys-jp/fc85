# 信号系部品の見直し（DSP・映像・アナログ）
Phase 0 調査 v0.1 ／ 調査日 2026-11-25

凡例：`[D 資料ID 頁]` 出典あり。`[P]` 本書の推奨（仕様書は未変更）。`[O]` 代替・併記案。`[V]` 未確認。「推定」は本書の推論。資料IDは末尾の表を参照。既存台帳の資料は、その台帳のIDに台帳名を添えて引く（例：[D T85 D127]（TDC1007台帳））。

資料の区分：**A** 1984年以前、**B** 1985年中、**C** 1986年以降（後年資料）、**M** 現代の製品・販売ページ。年代の根拠は A・B だけ。C は IC 復元と傾向の参考、M は現代の入手性の判断にだけ使う。販売ページは「掲載がある」ことしか示さない。在庫・価格・真正性は保証しない。

判断基準（この順）：① 1985年の日本の中堅業務用電子機器メーカー（文字表示・映像・通信が本業）にとって自然か。② 現代の再現で入手できるか。

既存台帳（`TMS32010.md`、`TDC1007.md`、`CD22101.md`、`clock-video-1985.md`、`MSM5205.md`、`ES5503.md`、`HD63484.md`、`supply-ES5503.md`、`supply-HD63484.md`）で調べた事項は繰り返さず、結論だけ引く。

---

## 0. 要旨

### 0.1 現仕様・既存案から変えるべきもの

| # | 部品 | 現状 | 変更案 | 理由 |
|---|---|---|---|---|
| 1 | NTSC エンコーダ | MC1377 案（OI-008、clock-video-1985 §6.2） | ローム BA7230LS を第一候補 [P]。MC1377 は併記 [O] | 国産で +5 V 単一。1985年10月発売の国産ゲーム機（セガ マークIII）が採用 [D SMR; SMS]。映像入力と RGB の重ね合わせ（スーパーインポーズ）機能を持ち、用途に「キャプテンシステム」「パソコン」を挙げる [D BA p.1]。本機の外部映像入力と、文字表示・通信の本業に合う（推定）。ただし 1985年以前のローム資料は未確認 [V] |
| 2 | 映像入力 ADC | 仕様書 §04・§10 は TDC1007 | TDC1048 [P]（OI-010 の案を支持） | 1985年に使える国産の 8-bit 映像用 ADC は見つからない。国産で確認できたのは 4-bit の日立 HA19202（10 MSPS）だけ [D H85L p.132]。日立の 8-bit 品（HA19209、+5 V、250 mW）は 1986年の資料 [D HY8607] |
| 3 | 同期分離 | 未定（LM1881 は 1985年以前の資料にない、OI-008） | 個別部品（クランプ＋トランジスタまたは比較器）[P]。ライン同期の PLL が要るなら TDA2593／LM1880 [O] | 1985年以前の国産データブックでは、同期分離は TV 用の信号処理・偏向 IC に内蔵された形（11〜13 V 系）しか見つからない [D M82V p.136; N83C p.565, p.574] |
| 4 | 音響アナログ部のオペアンプ | 仕様書に記載なし | ES5503 の I/V 変換と音量帰還は LM318 級、S/H の保持は TL072/TL082 級、加算・LPF・出力と MSM5205 の LPF は 4558 級 [P] | Mirage（1985）は DOC 出力の変換に LM318 を使った（1985-10-17 付け回路図についての後年の証言）[D SDY]。3 種とも 1985年以前に国産の同等品がある [D N81L; H85L] |
| 5 | 電源（OI-025 への追記） | +5 V 以外が未定 | アナログ部に ±12 V、映像 ADC に −5.2 V、DOC に −5 V を見込む [P] | LM318・TL072・4558 級は両電源で使うのが普通（推定）。TDC1048 は −5.2 V [D T85 D4]（TDC1007台帳）。MC1377 を使う場合は +12 V [D AMS; MLI]（clock-video-1985） |
| 6 | 仕様書 §04 の CD22101 の根拠 | 1980年 RCA 資料（Preliminary）[D R1]（CD22101台帳） | 1983年 RCA CMOS データブックを年代の根拠に加える [P] | 1983年版では正式品として掲載 [D R83 PDF p.642–649] |
| 7 | 仕様書 §13 の現代候補 | IA63484 は「入手性未確認」。CD22101 は「実ICまたはCH446Q等」 | IA63484 は「実在するが現在の取扱は見つからない」に改める。CD22101 は実チップ（CD22101F）の流通在庫を第一にする。8×16 規模の代替には CD22M3494 を加える [P] | IA63484 のデータシートはドロップイン互換をうたうが、現在の取扱業者は 0 件 [D IA; FCIA]。CD22101F は流通在庫の掲載あり [D DKCD]。CD22M3494 は MT8816 とピン・機能互換の現行品 [D CD3494]。CH446Q は MT8816 ともピン互換でない [D CH446] |

### 0.2 そのままでよいもの

| 部品 | 結論 | 理由 |
|---|---|---|
| DSP | TMS32010 を維持 [P]。国産の代替として富士通 MB8764 を併記 [O] | 外部メモリのプログラムを全速で実行できる 1985年以前の DSP は、本調査では TMS32010 と MB8764 だけ [D T2; FJ8764 PDF p.62–63]。国産で普及した µPD7720 と HD61810 は内蔵 ROM 方式で、要件に合わない [D N84M; H618]。TMS32010 はピン互換の後継 TMS320C10 の流通在庫と FPGA コアがあり、再現しやすい [D DKC10; IKA] |
| アナログマトリクス | CD22101 系を維持 [P]。国産の 4052／4066＋ラッチを併記 [O] | CD22101 は 1983年の RCA データブックに正式掲載（1980年版は Preliminary だった）[D R83 PDF p.642–649]。現代も実チップの流通在庫がある [D DKCD] |

---

## 1. DSP

要件：DSP のプログラムを外部 RAM に置き、ホストが書き込んでから実行する。カートリッジや利用者がプログラムを差し替えられること（仕様書 §04、TMS32010台帳）。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| TI TMS32010（NMOS、200 ns、40 ピン DIP） | 1985年3月の User's Guide と 1985年10月改訂のデータシート（PRODUCTION DATA）がある [D T1; T2]（TMS32010台帳） | 米国製の輸入品。1985年以前の国内採用例・日本語資料・国内製造は見つからない [V]。TI の国内技術拠点（東京・大阪・名古屋）は 1986年資料で確認 [D TI86]（C）。業務用の映像・通信機器メーカーが輸入 DSP を使うのは不自然ではない（推定） | 外部プログラムメモリ 4K 語をウェイトなしで実行できる [D T2]（TMS32010台帳）。ホストが RAM に書いてからリセットを解く方式で要件を満たす | NMOS 品の在庫は見つからない。CMOS の後継 TMS320C10 は TI が「ピン・オブジェクトコード互換」と記載 [D T4; TI86]（C）。C10 の流通在庫あり [D DKC10; EBC10]（M）。AC タイミングの差は未照合 [V]。FPGA コア IKA32010（BSD-2、実機で検証）[D IKA]。MAME tms320c1x（BSD-3）[D MAME] | [P] |
| 富士通 MB8764（CMOS、命令 100 ns、88 ピン セラミック PGA、命令 24-bit） | データシート第 2 版が 1985年12月付 [D FJ8764 PDF p.62]（B）。1983年2月の ISSCC の富士通論文が本チップの発表と推定 [D ISSCC83]（書誌のみ確認） | 国産。クロスアセンブラと評価ボードがあると記載 [D FJ8764 PDF p.62]。価格・国内採用例は見つからない [V] | 内蔵マスク ROM（1K×24）と外部 ROM を IRM ピンで切り替える。外部 ROM は専用のアドレス・命令バスで読む [D FJ8764 PDF p.62–63]。外部 ROM の位置に SRAM を置けば外部 RAM から実行できる（推定）。直接見えるのは 1K 語。4K 語にはバンク切替が要り、そのとき外部データ RAM が 256 語に減る（DSP 調査担当の報告。頁は未照合 [V]）。8-bit 幅の SRAM が 3 個要る（推定） | 在庫・エミュレータとも見つからない [V]。再現は命令セットからの新規実装になる | [O]（国産案） |
| NEC µPD7720／µPD77P20（NMOS、250 ns、28 ピン） | 1982年の NEC カタログに 7720 [D N82M p.551]。77P20 のデータシートは 1983年7月付 [D N84M p.6-123]。1985年の選択ガイドにも両方が載る [D N85G] | 国産で最も普及した DSP。開発環境 ASM77・SIM77・EVAKIT-7720 がある [D N84M]。CMOS 版 µPD77C20 は 1985年第4四半期に 3000 円 [D DQ85] | プログラムは内蔵マスク ROM（512×23）で、外部プログラムバスがない [D N84M p.6-115]。77P20 は EPROM で、書込みに 21 V、消去に紫外線が要る [D N84M p.6-123]。差し替えの要件に合わない | 77P20D の流通在庫あり [D FCP20]（M）。MAME upd7725（BSD-3）[D MAME]、MiSTer SNES の DSPn（GPL-3）[D MSNES] | 不採用 |
| 日立 HD61810B（CMOS、250 ns、浮動小数点、40 ピン） | 1983年2月の IEEE Trans. ASSP の論文 [D ASSP83]（書誌のみ確認）。1984年の日立カタログに掲載 [D H84Q]。エミュレータは 1985年下期予定と報じられた [D DQ85] | 国産。ACRTC と同じ日立の部品（推定：調達をまとめやすい） | 命令 ROM（512×22）はマスク ROM。外部プログラムの記述はない [D H618] | 在庫・エミュレータとも見つからない [V] | 不採用 |
| AMI S2811／S28212 | S2811 は 1979年発表 [D EL79]。外部命令版 S28212 は 1984年のカタログでも予告段階 [D AMI84 pp.3.92–3.95]。量産されなかったという後年の回想がある [D SHMJ] [V] | 米国製。国内採用例は見つからない [V] | S2811 は外部 ROM 時に半速（600 ns）[D EL79]。S28212 は 512 命令まで [D AMI84] | [V] | 不採用 |
| Intel 2920 | 1979年 [D I2920] | 米国製 [D I2920] | 内蔵 EPROM（192×24）。乗算器がなく、外部プログラムも使えない [D I2920] | [V] | 不採用 |
| 1986年以降の国産 DSP（µPD77230、沖 MSM6992、松下 MN1900 など） | 1986年 [D U230; DQ86]（C） | 国産 [D U230; DQ86] | µPD77230 は外部命令メモリ 4K×32、MSM6992 は外部 64K 語まで [D U230; DQ86] | — | 対象外（T2:1985 の範囲外。後継機の候補） |

推奨 [P]：TMS32010 を維持する。外部 RAM のプログラムを全速で実行でき、後継 C10 と FPGA コアで再現しやすい。国産で要件を満たすのは MB8764 だけで、データシートは 1985年12月付と遅く、88 ピン PGA・24-bit 命令と周辺が重い。国産にこだわる場合の代替 [O] とする。

---

## 2. 映像入力 ADC

前提：128×96・8-bit 輝度。1ラインに128サンプルで約 2.4〜2.7 MSPS（clock-video-1985 §7.4）。4fsc の生波形（14.3 MSPS）は別用途。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| TRW TDC1048（8-bit、20 MSPS、フラッシュ、28 ピン、+5/−5.2 V、1.6 W） | 1984・1985年の TRW 資料に掲載。用途に「Low-Cost Video Digitizing」[D T84 PDF p.114; T85 D4, D127]（TDC1007台帳） | 米国製。国内の代理店・価格は未確認 [V]。1985年に国産の同等品がないので、輸入品を使うのは自然（推定） | 2.7 MSPS の輝度取り込みに十分。TTL 出力。負電源と放熱が要る [D T85 D4]（TDC1007台帳） | （§7.4） | [P] |
| TRW TDC1007（8-bit、20 MSPS、64 ピン、+5/−6 V、2.7 W） | 1984・1985年資料 [D T84; T85 D15]（TDC1007台帳） | 同上 | 4fsc の生波形も取れるが、128×96 には過剰 [D T85 D4]（TDC1007台帳） | （§7.4） | [O]（4fsc 生波形用） |
| TRW TDC1014（6-bit、25 MSPS、1.1 W） | 1985年資料。新設計には TDC1046 を推奨と記載 [D T85 PDF p.68]（TDC1007台帳） | 同上 | 6-bit に下げる廉価案 [D T85 PDF p.68]（TDC1007台帳） | [V] | [O] |
| 日立 HA19202（4-bit 並列型、10 MSPS、TTL オープンコレクタ出力、DP-22）。2 個で 5-bit | 1985年の日立リニア IC データブックに掲載。カラー TV の映像信号処理向けと明記 [D H85L p.132–133] | 国産 [D H85L p.132]。1985年に入手できた唯一の国産映像用 ADC（本調査の範囲） | 5-bit（32 階調）止まり [D H85L p.132]。8-bit 輝度の要件を満たさない | [V] | [O]（国産廉価案。仕様を 5-bit に下げる場合） |
| 日立 HA19209／HA19210（8-bit、30 MSPS typ、250 mW、+5 V 単一、TTL/CMOS、DP28） | 1986年7月の日立評論で紹介、「一部は既に量産」[D HY8607 表2]（C）。試作の発表は 1984年9月の VLSI シンポジウムと 1985年の電子通信学会全国大会（同記事の参考文献）。1985年中の出荷は未確認 [V] | 国産で +5 V 単一 [D HY8607 表2]。条件は最も良い（推定） | 電源・電力・TTL とも本機に最も合う [D HY8607 表2] | [V] | [O]（後継機または 1986年以降の改版） |
| ソニー CX20052A（8-bit、20 MSPS、直並列型、28 ピン DIP、−5 V 単一、700 mW typ） | 1985年以前の資料では未確認 [V]。1987年のソニー資料に掲載 [D SN87 p.116–118]（C）。1986年発表の Tek 2230 が採用 [D TWCX]（C） | 国産。ただしクロックと出力が ECL レベルで、外付けの S/H とクロック回路が要る [D SN87 p.116, p.183] | TTL の Capture RAM との間にレベル変換が要り、周辺回路が重い [D SN87 p.116] | [V] | 不採用（年代未確認） |
| 東芝・松下の 8-bit 並列型 ADC | 学会発表のみ（東芝 1985年 ESSCIRC、松下 1981年 SSDM）[D ESS85; SSDM81]。製品は未確認 [V] | — | — | — | 対象外 |

推奨 [P]：TDC1048。国産で 8-bit の映像用 ADC が 1985年に確認できないため、米国製を輸入して使う設定になる。国産にこだわるなら HA19202×2（5-bit）[O]。日立の 8-bit 品（HA19209）は 1986年なので、後継機の候補とする。

---

## 3. NTSC エンコーダ（RGB→コンポジット）

前提：RGB 各 4-bit のパレット出力（抵抗 DAC）を NTSC コンポジットにする（clock-video-1985 §6）。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| ローム BA7230LS（RGB マトリクス、平衡変調、3.58 MHz VCXO、Y/R−Y/B−Y、映像入力との高速切替、SZIP-24、+5 V） | 1985年10月20日発売のセガ マークIII が採用 [D SMR; SMS; SRM3]（いずれも C）。ローム自身の 1985年以前の資料は未確認 [V] | 国産（京都）で +5 V 単一。用途に「文字多重 TV、キャプテンシステム、ビデオカメラ、パソコン」を挙げる [D BA p.1]。文字表示・映像・通信の会社には自然（推定） | RGB 入力は 0.7 Vp-p [D SMS; BA p.7]。SYNC・BFP（バーストフラグ）・PCP（ペデスタルクランプ）を TTL レベルで入れる [D BA p.1, p.7]。これらは ACRTC の同期出力と TTL で作る（推定 [P]）。映像入力とのスーパーインポーズには RGB が映像入力に同期している必要がある [D BA p.8]。ACRTC の外部同期（EXSYNC）[D U85 p.26, p.44]（HD63484台帳）とライン同期のクロックが要る [O] | 製造中止（DigiKey は Obsolete 表示）[D DKBA]。代理店・ブローカーの掲載はある [D DKBA]（M）。真正性は未確認 [V] | [P] |
| Motorola MC1377（RGB→PAL/NTSC、20 ピン DIP、+12 V±2 V、35 mA） | 1983年 Motorola 資料（Advance Information）[D MLI 10-78〜10-80]（clock-video-1985）。1985年の Amiga 1000 が U7A に搭載 [D AMS]（C） | 米国製。国産パソコンでの採用例は本調査では見つからない [V] | RGB 1.0 Vp-p、外部副搬送波も可 [D MLI]（clock-video-1985）。+12 V が要る [D AMS] | （§7.4） | [O] |
| ソニー V7040 | 1986年の初期 Master System が採用 [D SMS]（C）。1985年は未確認 [V] | 国産 [D SMS] | RGB 1.0 Vp-p [D SMS] | [V] | 対象外（年代未確認） |
| ソニー CXA1145、富士通 MB3514 | 後年の Master System II・Mega Drive などが採用 [D SMS]（C） | 国産 [D SMS] | — | （§7.4） | 対象外（1985年に使えない） |
| National LM1886＋LM1889 | 1982年 National 資料 [D N82 11-23, 11-28]（clock-video-1985） | 米国製 [D N82] | LM1886 は RGB 各 3-bit 入力で本機の 4-bit に合わない（clock-video-1985 §6.2） | [V] | 不採用 |

推奨 [P]：BA7230LS。国産・+5 V で、1985年の国産家庭用機に実績がある。スーパーインポーズ機能は「Composite feedback」や外部映像入力（仕様書 §10・§11）と相性が良い。年代の根拠が後年の解析資料だけなので、1985年以前のローム資料（データブック、技術記事）で確認するまで [V] を付ける。確認できない場合は MC1377 に戻す。

---

## 4. 同期分離

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| 個別部品（75 Ω 終端、クランプ、トランジスタまたは比較器でシンクチップを切り出し、積分で垂直同期） | 汎用部品だけで組める（推定） | 映像機器メーカーには最も普通の方法（推定） | 128×96 の取り込みには H・V とクランプパルスがあれば足りる（clock-video-1985 §7.4）。ライン同期のクロックは別に要る | 汎用部品で常に入手できる（推定） | [P] |
| National TDA2591/TDA2593（同期分離、雑音分離、垂直同期分離、水平発振器との位相比較）、LM1880 | 1982年 National 資料 [D N82 11-90, 11-16]（clock-video-1985） | 米国製 [D N82]（TDA2593 は欧州の TV 用 IC の系統。推定） | 水平発振器との位相比較があり、ライン同期のサンプリングクロックを作りやすい（clock-video-1985 §7.4） | [V] | [O]（ライン同期 PLL が要る場合） |
| NEC µPC1377C（カラー TV 用同期・偏向処理。水平・垂直の同期分離を別々に持つ）、µPC1379C（白黒・小型カラー TV 用） | 1983/1984年の NEC 民生用 IC データブック [D N83C p.565, p.574] | 国産 [D N83C] | TV の偏向回路を駆動する IC で、水平部は高圧系からシャント電源で動く設計 [D N83C p.574]。コンピュータの基板には過剰（推定） | [V] | 不採用 |
| 三菱 M5195P（TV 映像信号処理＋同期分離） | 1982年の三菱ビデオデータブック [D M82V p.136] | 国産 [D M82V] | 電源 11〜13 V の TV 用 IC [D M82V p.136] | [V] | 不採用 |
| National LM1881 | 1982・1984年の National 資料になく、1988年資料に載る [D N82; N84; N88]（clock-video-1985） | — | 1985年機には使えない [D N84; N88] | TI が現行品として掲載 [D TILM1881]（M） | 現代実装のみ [O] |
| ローム BA7046／BA7071F（5 V の同期分離） | 年代未確認（資料は 2010年の技術資料）[D R46] [V] | 国産 [D R46] | — | BA7046（DIP）は製造中止、BA7046F は掲載あり（検索結果の要約）[D R46] [V] | 対象外 |

推奨 [P]：1985年の入力基板は個別部品で同期分離する。サンプリングクロックをラインに同期させる場合は TDA2593 系の位相比較を使う [O]。現代の再現は LM1881 で置き換えてよい（実装側の差として記録する）。

---

## 5. アナログマトリクス／スイッチ

前提：低帯域の音声・CV・変調信号を切り替える（仕様書 §10）。コンポジット全帯域は対象外（CD22101台帳 §3.3）。

| 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|
| RCA CD22101／CD22102（4×4×2、ラッチ付き、24 ピン） | 1980年版は Preliminary [D R1 589]（CD22101台帳）。1983年の RCA CMOS データブックに正式掲載。分類は「Telecommunication Circuits — Crosspoint Switches」[D R83 PDF p.10, p.642–649] | 米国製。国内の代理店は未確認 [V]。通信用の部品で、通信が本業の会社なら知っていてもおかしくない（推定） | 音声・CV に適する（CD22101台帳 §3.3）。2 組が同時に切り替わるのでステレオに向く | （§7.4） | [P] |
| RCA CD22100（4×4、ラッチ付き、16 ピン） | 1983年の RCA CMOS データブック [D R83 PDF p.637–641] | 同上 | モノラルの経路切替用。CV の分配に向く [D R83 PDF p.637] | （§7.4） | [O] |
| Mitel MT8816（8×16、7→128 デコーダとラッチ、ISO-CMOS、PBX・キーシステム用） | 確認できた最古の資料は 1988年11月の Issue 2 [D MT88]（C）。1985年の存在は未確認 [V] | 通信用。日本での流通は未確認 [V] | 規模が大きく、本機には余る [D MT88] | （§7.4） | 対象外（年代未確認） |
| 4051／4052／4053／4066／4016＋ラッチ（74LS273 など） | 東芝 TC4051BP・TC4052BP・TC4053BP・TC4066BP・TC4016BP が 1985年の東芝 C²MOS データブックに掲載 [D T85C PDF p.9 索引, p.210, p.227]。RCA CD4051B・CD4066B も 1983年資料に掲載 [D R83 PDF p.10 索引] | 国産のセカンドソースが多く、最も自然。1985年の Mirage は声部の多重に 4051 を使う [D SDY]（C） | ラッチを外付けすれば CD22101 と同じ論理モデルにできる（推定 [P]） | TI CD4052B などは現行品 [D TICD4052]（M） | [O]（国産案） |
| 東芝 TC9164 など（音響用アナログスイッチアレイ、シリアル制御、8〜16 V） | 年代未確認 [V]（後年の機器の採用例のみ）[D RMTC] | 国産 [D RMTC] | — | [V] | 対象外 |

推奨 [P]：仕様書の「CD22101 系」をそのまま論理モデルとし、1985年の搭載品は CD22101（1983年資料で正式品）。国産部品で組む場合は 4052＋ラッチで同じ論理モデルを作る [O]。現代の再現は §7.4 を参照。

---

## 6. 音響のアナログ部

ES5503 の出力は差動電流（SIG±）で、音量 DAC の電流出力（VOL−）を電圧にして波形 DAC の基準に戻す [D E2 p.3–4]（ES5503台帳）。出力はオシレータごとに時分割され、CA0–CA3・CHSTB で外部の S/H に分配する（ES5503台帳 §3.6）。MSM5205 は DA 出力の後にボルテージフォロワと LPF（8 kHz で fc 3.4 kHz）を置く（MSM5205台帳 §3.5）。

| 用途 | 候補 | 1985年の入手性 | 日本の中堅メーカーにとっての自然さ | 本機の要件への適合 | 現代の入手性 | 推奨 |
|---|---|---|---|---|---|---|
| ES5503 の I/V 変換（SIG±）と音量帰還（VOL−→WVREF） | LM318 級の高速オペアンプ：National LM318、NEC µPC159（「LM318 Direct Replacement」）[D N81L PDF p.51] | µPC159 は 1981年の NEC リニア IC データブック [D N81L PDF p.51] | 国産のセカンドソースあり。Mirage（1985）が同じ箇所に LM318 を使う [D SDY]（C）。IIGS 用の自作ステレオ基板も LM318 を使う [D CSA]（C） | 時分割出力のセトリングに速度が要る（推定）。4558 級では遅い（推定） | TI LM318 は現行品 [D TILM318]（M） | [P] |
| 時分割出力の S/H と保持バッファ | 4051／4016＋保持コンデンサ＋TL072/TL082 級の JFET 入力オペアンプ：日立 HA17080 系（「TL080 系とピン互換」）[D H85L p.22]、NEC µPC4081/4082/4084 | HA17082 は 1985年の日立資料 [D H85L p.22]。µPC4082 は 1981年の NEC 資料 [D N81L PDF p.8 索引] | 国産あり。IIGS 用自作基板は 4016＋TL072 [D CSA]（C） | JFET 入力で保持コンデンサの漏れが小さい（推定） | TI TL072CP は現行品 [D TITL072]（M） | [P] |
| 加算・LPF・出力、MSM5205 の LPF | 4558 級の 2 回路オペアンプ：JRC NJM4558、NEC µPC4558、日立 HA17558（低雑音と記載）[D N81L PDF p.8; H85L p.51] | µPC4558 は 1981年の NEC 資料 [D N81L PDF p.8 索引]。HA17558 は 1985年の日立資料 [D H85L p.51]。JRC 4558 は 1979年の国産エフェクタ（TS808）に使われたとされる [D WTS]（C） | 国産の定番 [D N81L; H85L]。最も自然（推定） | 音声帯域の LPF・加算には十分（推定） | NJM4558 の DIP 品は 2025年6月に新規設計非推奨、面実装品は掲載あり [D NSN]（M） | [P] |
| （参考）MSM5205 の LPF を省いた RC だけの構成 | — | — | 1980年代のアーケード基板に例が多いとされる [V] | 折返し雑音が残る（MSM5205台帳 §3.5） | — | 不採用 |

推奨 [P]：I/V 変換は LM318 級（国産なら µPC159）、S/H 保持は TL082 級（HA17082 など）、それ以外は 4558 級。いずれも 1985年以前に国産の同等品があり、現代も入手できる。電源は ±12 V を基本にする（推定。OI-025 に追記する案）。

---

## 7. 現代の入手性（ES5503・MSM5205・HD63484 ほか）

1985年の供給は `supply-ES5503.md`・`supply-HD63484.md` を参照。ここは 2026-11-25 時点の現代の入手性だけを扱う（区分 M）。数量・価格は参照日に見えた値。

### 7.1 本機の搭載 IC

| 部品 | 旧在庫（NOS）・流通 | 偽物・再刻印 | 後継・互換品 | FPGA・ソフト実装 | 再現での扱い |
|---|---|---|---|---|---|
| Ensoniq ES5503 | ほぼない。中古の取り外し品が中心 [D MBOX; EMU]（日付なし） | 報告は見つからない | ピン互換品は見つからない | MiSTer Apple IIgs コアの `es5503.v`（GPLv3+、2026-07 公開）[D MIIGS]。MAME es5503（BSD-3）[D MAME] | 実チップは確保できる前提にしない。FPGA・ソフトを主とする [P] |
| OKI MSM5205 | 多い。専門店に掲載あり [D APR] | MSM5205 固有の確かな報告は見つからない。旧型 IC の再刻印は一般に多い [D HAD] | OKI は新規設計に MSM6585 を推奨。ただし差し替えはできない（640 kHz、4-bit のみ、VCK の立上りで取り込む、LPF 内蔵）[D OKI6585] | jt5205（Verilog、GPL-3）[D JT5205]、MAME msm5205 [D MAME] | 実チップを使える [P]。入手品の真正性は検査する |
| 日立 HD63484 | 少ない。個人売買・ブローカーの掲載のみ [D EBHD] | 報告は見つからない | innovASIC IA63484（ドロップイン互換をうたう、68 ピン PLCC、25 MHz）[D IA]。現在の取扱業者は 0 件 [D FCIA] | HDL 実装は見つからない [D GHACRTC]。MAME hd63484 [D MAME] | 実チップの確保が課題。FPGA 化は新規作業になる [P] |
| TI TMS32010／TMS320C10／C15 | NMOS 品は見つからない。C10・C15 は流通在庫あり [D DKC10; EBC10] | 報告は見つからない | C10・C15 は TMS32010 とピン・オブジェクトコード互換と TI が記載 [D T4; TI86]（C）。AC タイミングは未照合 [V] | IKA32010（BSD-2、実機で検証）[D IKA]、TMS32010_sv（作業途中）[D T32SV]、MAME [D MAME] | C10 を第一とし、FPGA を併用 [P] |
| RCA CD22100／CD22101 | CD22101F の流通在庫あり [D DKCD]。CD22100 はブローカーの掲載のみ [V] | 報告は見つからない | Intersil の 1999年資料で「OBSOLETE」、推奨後継なし [D IS18] | 論理は単純で、ソフト実装は容易（推定） | CD22101F の実チップを第一とする [P] |
| TRW TDC1048／TDC1007 | ブローカーのみ。高額な店もある [D TDCM] | 報告は見つからない | Raytheon・Fairchild が型番のまま継承したが廃番。代替供給の ADI AD9048 も廃番で、ピン互換の後継はない [D TDCM]。TDC1007 も廃番 [D TWTDC] | 不要（現代の ADC で置換） | 現代版は仕様書 §13 の実装側候補で置き換え、差を記録する [P] |

### 7.2 本書で比較した周辺部品

| 部品 | 旧在庫（NOS）・流通 | 偽物・再刻印 | 後継・互換品 | FPGA・ソフト実装 | 再現での扱い |
|---|---|---|---|---|---|
| ローム BA7230LS | 製造中止。代理店に在庫の掲載あり [D DKBA] | 未調査 [V] | なし [V] | 不要 | 実チップを使える見込み（推定）[P] |
| Motorola MC1377 | ブローカーの在庫表示のみ [D MC77M] | デートコード「2021+」の掲載があり、再刻印か互換品の疑い（推定）[D MC77M] | MC13077 はピン互換と書いた資料が見つからない [D MC13077]。単体エンコーダ AD724／AD725 も 2026-07 に廃番告知（検索結果の抜粋で確認）[D PDN724] [V] | 不要 | [V] |
| Mitel MT8816 | 2025-11 の PCN で廃番。最終出荷 2027-03（検索結果の抜粋で確認）[D PCN8816] [V] | — | Renesas CD22M3494 は MT8816 とピン・機能互換の現行品 [D CD3494]。WCH CH446Q は並列アドレス時の機能はほぼ互換だが、パッケージ・ピン・一部の X 端子の割当が違う [D CH446] | — | 8×16 規模が要る場合は CD22M3494 [O] |
| ソニー CX20052A／CXA1145 | CX20052A はブローカー表示のみ [V]。CXA1145 は個人売買に多数 [D CXA45] | 報告は見つからない | CXA1145 には互換品 ES71145 と、CXA2075 への変換基板がある [D CXA45] | — | 本機では使わない |
| LM318、TL072、4558 | LM318 [D TILM318]、TL072CP [D TITL072] は現行品。NJM4558 は DIP 品が 2025-06 に新規設計非推奨 [D NSN] | — | 4558 級は各社に同等品がある（推定） | — | 現行品で再現できる [P] |
| LM1881 | 現行品 [D TILM1881] | — | — | — | 現代版の同期分離に使う [O] |
| 4051／4052／4066 | 現行品 [D TICD4052] | — | — | — | 現行品で再現できる [P] |

---

## 8. 未解決事項

1. BA7230LS の 1985年以前のローム資料（データブック、業界紙）。マークIII より前の採用例。見つからなければ MC1377 に戻す。
2. 1985年に出た国産パソコン（MSX2、FM77AV、X1 など）の NTSC エンコーダ。本調査では特定できなかった。
3. TDC1048 の 1985年の国内代理店と価格（TDC1007台帳の未解決と同じ）。
4. 日立 HA19209 の量産開始時期。1985年中なら国産 8-bit ADC の案が成り立つ。ソニー CX20052A の発表時期。
5. TMS32010 の 1985年以前の国内採用例、日本語資料、TI 日本法人の扱い。
6. TMS320C10 と TMS32010 の AC タイミングの照合（TMS32010台帳の未解決 9 と同じ）。
7. MB8764：価格、国内採用、外部 ROM の読出しタイミング（SRAM で足りるか）、命令エンコード、バンク切替の頁。ISSCC 1983 論文と同じチップか。
8. MT8816 の 1985年以前の存在。CD22101 の 1985年の国内代理店。
9. Mirage の DOC 出力に LM318 を使ったこと。1985-10-17 付け回路図の現物で確認する。
10. ES5503 の I/V 変換に要る速度（時分割出力のセトリング時間）の定量化。LM318 級が必要かどうか。
11. アナログ電源（±12 V、−5 V、−5.2 V）の構成。OI-025 で検討する。
12. IA63484 の現在の入手先。HD63484 の FPGA 実装の有無。
13. MC1377・MT8816・AD724 の廃番情報は検索結果の抜粋でしか確認していない。PCN・PDN の本文で確認する。

---

## 資料一覧

参照日はすべて 2026-11-25。区分は冒頭の A／B／C／M。B 列の `BS` は `https://ftp.mirrorservice.org/sites/www.bitsavers.org/components` の略。取得した PDF は `<scratchpad>\parts\books\`、`<scratchpad>\parts\video\`、`<scratchpad>\parts\dsp\`、`<scratchpad>\parts\modern\` に置いた（リポジトリには入れない）。頁は、断りがなければ資料に印刷された頁（ノンブル）。

### 既存台帳から引いた資料

| ID | 区分 | 書誌（詳細は各台帳） | URL |
|---|---|---|---|
| T1 | B | TI, *TMS32010 User's Guide* SPRU001B（March 1985）（TMS32010台帳） | TMS32010台帳を参照 |
| T2 | B | TI, TMS32010 データシート（REVISED OCTOBER 1985）（TMS32010台帳） | TMS32010台帳を参照 |
| T4 | C | TI, *First-Generation TMS320 User's Guide* SPRU013（1987）（TMS32010台帳） | TMS32010台帳を参照 |
| T84 | A | TRW, *VLSI Data Book*（1984）（TDC1007台帳） | http://www.bitsavers.org/components/trw/_dataBooks/1984_TRW_VLSI_Data_Book.pdf |
| T85 | B | TRW, *VLSI Data Book*（1985）（TDC1007台帳） | https://bitsavers.trailing-edge.com/components/trw/_dataBooks/1985_TRW_VLSI_Data_Book.pdf |
| MLI | A | Motorola, *Linear and Interface Integrated Circuits* DL128（1983）（clock-video-1985） | http://www.bitsavers.org/components/motorola/_dataBooks/1983_DL128_Motorola_Linear_and_Interface_Integrated_Circuits.pdf |
| N82 | A | National, *Linear Databook*（1982）（clock-video-1985） | http://www.bitsavers.org/components/national/_dataBooks/1982_National_Linear_Databook.pdf |
| N84 | A | National, *Linear Supplement*（1984）（clock-video-1985） | http://www.bitsavers.org/components/national/_dataBooks/1984_National_Linear_Supplement.pdf |
| N88 | C | National, *Linear Databook Vol.3*（1988）（clock-video-1985） | http://www.bitsavers.org/components/national/_dataBooks/1988_National_Linear_Databook_Vol_3.pdf |
| R1 | A | RCA, *COS/MOS Integrated Circuits*（1980）（CD22101台帳） | http://www.bitsavers.org/components/rca/_dataBooks/1980_RCA_COS_MOS_Integrated_Circuits.pdf |
| E2 | 年不明 | Ensoniq, *5503 Digital Oscillator Chip* データシート（ES5503台帳） | ES5503台帳を参照 |
| U85 | B | Hitachi, *HD63484 ACRTC User's Manual* #U75（1985年5月）（HD63484台帳） | HD63484台帳を参照 |

### 本書で加えた資料

| ID | 区分 | 書誌 | 日付 | URL |
|---|---|---|---|---|
| H85L | B | Hitachi, *Linear IC Data Book* C10。HA17080 系 p.22、HA17558 p.51、HA19202 p.132–134 | 1985 | BS/hitachi/_dataBooks/1985_C10_Hitachi_Linear_IC_Data_Book.pdf |
| N81L | A | NEC, *Linear Integrated Circuits Data Book* 1st ed.。索引 PDF p.8、µPC159 PDF p.51 | 1981 | BS/nec/_dataBooks/1981_NEC_Linear_Integrated_Circuits_Data_Book_1st_ed.pdf |
| R83 | A | RCA, *CMOS Databook*。索引 PDF p.10、CD22100 PDF p.637–641、CD22101/CD22102 PDF p.642–649 | 1983 | BS/rca/_dataBooks/1983_RCA_CMOS_Databook.pdf |
| T85C | B | Toshiba, *C²MOS Integrated Circuits*。索引 PDF p.9、TC4051/52/53 PDF p.210、TC4066 PDF p.227 | 1985 | BS/toshiba/_dataBook/1985_Toshiba_C2MOS_Integrated_Circuits.pdf |
| M82V | A | Mitsubishi, *Video Data Book*。M5195P p.136 | 1982 | BS/mitsubishi/_dataBooks/1982_Mitsubishi_Video_Data_Book.pdf |
| N83C | A | NEC, *Integrated Circuits for Consumer Use*。µPC1377C p.565、µPC1379C p.574 | 1983 | BS/nec/_dataBooks/1983_NEC_Integrated_Circuits_for_Consumer_Use.pdf |
| N85G | B | NEC, *Product Selection Guide* | 1985 | BS/nec/_dataBooks/1985_Product_Selection_Guide.pdf |
| N82M | A | NEC, *Microcomputer Catalog*。µPD7720 p.551〜 | 1982 | BS/nec/_dataBooks/1982_NEC_Microcomputer_Catalog.pdf |
| N84M | A | NEC, *Microcomputer Catalog*。µPD7720 p.6-115〜、µPD77P20 p.6-123〜（1983年7月付） | 1984 | BS/nec/_dataBooks/1984_NEC_Microcomputer_Catalog.pdf |
| H618 | 年不明 | Hitachi, *HD61810B Digital Signal Processor User's Manual* | 日付なし | BS/hitachi/_dataBooks/HD61810_Digital_Signal_Processor_Users_Manual.pdf |
| H84Q | A | Hitachi, *IC and Discrete Quick Reference Guide* | 1984 | BS/hitachi/_dataBooks/1984_Hitachi_IC_and_Discrete_Quick_Reference_Guide.pdf |
| ASSP83 | A | Hagiwara ほか, IEEE Trans. ASSP 31(1), pp.339–346（書誌のみ確認） | 1983-02 | doi:10.1109/TASSP.1983.1164010 |
| FJ8764 | B（データシート）／C（本文） | Fujitsu, *MB8764 DSP Applications Manual* Edition 1.0（1986-08）。付録データシート Edition 2.0（December 1985）PDF p.62–63 | 1985-12／1986-08 | BS/fujitsu/_dataSheets/1986_Fujitsu_MB8764_DSP_Applications_Manual.pdf |
| ISSCC83 | A | Kikuchi ほか, ISSCC Digest XXVI, pp.128–129（書誌のみ確認） | 1983-02 | doi:10.1109/ISSCC.1983.1156548 |
| EL79 | A | Blasco, "V-MOS chip joins microprocessor…", *Electronics* 52(18), p.131〜 | 1979-08-30 | https://www.worldradiohistory.com/Archive-Electronics/70s/79/Electronics-1979-08-30.pdf |
| AMI84 | A | AMI, *MOS Products Catalog*, pp.3.92–3.95 | 1984 | BS/ami/_dataBooks/1984_AMI_MOS_Products_Catalog.pdf |
| I2920 | A | Intel, *2920 Analog Signal Processor Design Handbook* | 1980-08 | BS/intel/2920/1980_2920_Analog_Signal_Processor_Design_Handbook.pdf |
| TI86 | C | TI, *TMS320 Development Support Reference Guide* | 1986 | BS/ti/TMS320xx/tools/TI_TMS320_Development_Support_Reference_Guide_1986.pdf |
| DQ85 | B〜C | Dataquest, JSIS Newsletters 1985–86（"Technology Trends 4Q1985" ほか） | 1985–86 | https://archive.computerhistory.org/resources/access/text/2013/04/102723423-05-01-acc.pdf |
| DQ86 | C | Dataquest, JSIS Vol.II（1986年5月号・10月号。表の出典は日経エレクトロニクス） | 1986 | https://archive.computerhistory.org/resources/access/text/2013/04/102723432-05-01-acc.pdf |
| U230 | C | NEC, *µPD77230 Advanced Signal Processor* | 1986-03 | BS/nec/_dataSheets/uPD77230_Advanced_Signal_Processor_198603.pdf |
| SHMJ | C | 川上雄一「デジタル信号処理用プロセッサの開発」半導体産業人協会 会報 No.75, pp.13–16 | 2012-04 | https://www.shmj.or.jp/dev_story/pdf/develop45.pdf |
| HY8607 | C | 日立評論 Vol.68 No.7、特集「情報産業を推進するVLSI技術」の「A-D,D-Aコンバータ系列」（HA19209 ほか。表2、参考文献） | 1986-07 | https://www.hitachihyoron.com/jp/pdf/1986/07/1986_07_11.pdf |
| SN87 | C | Sony, *A/D, D/A Converters and DSP Data Book*。CX20052A p.116–118、CXA1008P/1009P p.174〜、p.183 | 1987 | https://bitsavers.mirrorservice.org/components/sony/1987_Sony_A-D_D-A_Converters_and_DSP_Data_Book.pdf |
| TWCX | C | TekWiki「Sony CX20052A」 | 参照日 | https://w140.com/tekwiki/wiki/Sony_CX20052A |
| TWTDC | C | TekWiki「TRW TDC1007」 | 参照日 | https://w140.com/tekwiki/wiki/TRW_TDC1007 |
| ESS85 | B | Yoshizawa ほか, "High Speed 8-bit A/D Converter for Video Signal Processing", ESSCIRC '85, pp.49–52（要旨のみ確認） | 1985 | https://ieeexplore.ieee.org/document/5467989 |
| SSDM81 | A | "Low Power 8 Bit Parallel A/D Converter", SSDM 1981 A-2-3（ページは HTTP 403。検索結果の書誌のみ） | 1981 | https://confit.atlas.jp/guide/organizer/ssdm/ssdm1981/subject/A-2-3/search?lang=en |
| BA | 年不明 | ROHM, BA7230LS データシート（日付なし）。Applications p.1、Input waveform p.7、Operation notes p.8 | 日付なし | https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/504/BA7230LS.pdf |
| SMR | C | Sega Retro「Sega Mark-III Hardware Notes (2008-11-14)」（Color TV signal encoder の節） | 2008-11-14 | https://segaretro.org/Sega_Mark-III_Hardware_Notes_(2008-11-14) |
| SRM3 | C | Sega Retro「Mark III」（日本発売 1985-10-20） | 参照日 | https://segaretro.org/Mark_III |
| SMS | C | SMS Power!「Video Output」（BA7230LS、V7040、CXA1145 などの比較） | 参照日 | https://www.smspower.org/Development/VideoOutput |
| AMS | C | amiga-stuff.com「MC1377」 | 参照日 | https://www.amiga-stuff.com/hardware/mc1377.html |
| MT88 | C | Mitel, MT8816 データシート Issue 2 | 1988-11 | https://hubtronics.in/docs/MT8816AE.pdf |
| RMTC | C | Radiomuseum「TC9164」 | 参照日 | https://radiomuseum.org/tubes/tube_tc9164.html |
| R46 | C | ROHM BA7046F／BA7071F データシートの転載 | 2010 ほか | https://datasheetspdf.com/datasheet/BA7046F.html ／ https://datasheetspdf.com/datasheet/BA7071F.html |
| SDY | C | synth-diy メーリングリスト（Mirage の回路図についての投稿） | 2016-09 | https://synth-diy.org/pipermail/synth-diy/2016-September/153437.html ／ https://synth-diy.org/pipermail/synth-diy/2016-September/153442.html |
| CSA | C | Rubywand ほか, "Apple II Csa2 FAQs: Sound & Music, Part 19/25"（項目 005、自作ステレオ基板の部品表） | 2004-05-04 投稿 | https://discmaster.textfiles.com/view/11940/ftp-pasteur-org-FAQ.zip/FAQ/apple2/faq/part19 |
| WTS | C | Wikipedia「Ibanez Tube Screamer」 | 参照日 | https://en.wikipedia.org/wiki/Ibanez_Tube_Screamer |
| DKBA | M | DigiKey「BA7230LS」（Obsolete、在庫掲載あり） | 参照日 | https://www.digikey.de/en/products/detail/rohm-semiconductor/BA7230LS/4003431 |
| TILM318 | M | TI「LM318」製品ページ（ACTIVE） | 参照日 | https://www.ti.com/product/LM318 |
| TITL072 | M | TI「TL072CP」（ACTIVE） | 参照日 | https://www.ti.com/store/ti/en/p/product/?p=TL072CP |
| TILM1881 | M | TI「LM1881」製品ページ（ACTIVE） | 参照日 | https://www.ti.com/product/LM1881 |
| TICD4052 | M | TI「CD4052B」製品ページ | 参照日 | https://www.ti.com/product/CD4052B |
| NSN | M | 日清紡マイクロデバイス NRND 通知 MNG-2025-0081 | 2025-06-16 | https://mm.digikey.com/Volume0/opasdata/d220001/medias/docus/7136/NRND_MNG-2025-0081.pdf |
| MAME | M | MAME ソース（es5503、msm5205、hd63484、tms320c1x、upd7725） | 参照日 | https://github.com/mamedev/mame |
| MIIGS | M | MiSTer-devel/Apple-IIgs_MiSTer（`rtl/es5503.v`） | 2026-07〜11 | https://github.com/MiSTer-devel/Apple-IIgs_MiSTer |
| MSNES | M | MiSTer-devel/SNES_MiSTer（`rtl/chip/DSP/DSPn.vhd`） | 参照日 | https://github.com/MiSTer-devel/SNES_MiSTer |
| IKA | M | ika-musume/IKA32010 | 2023-06〜2025-12 | https://github.com/ika-musume/IKA32010 |
| T32SV | M | birdybro/TMS32010_sv | 参照日 | https://github.com/birdybro/TMS32010_sv |
| JT5205 | M | jotego/jt5205 | 2025-01 | https://github.com/jotego/jt5205 |
| MBOX | M | MIDIbox フォーラム「ES-5503 DOC newbie」 | 日付なし | https://midibox.org/forums/topic/4031-es-5503-doc-dumb-newbie-alert |
| EMU | M | electro-music.com 売買投稿 | 日付なし | https://electro-music.com/forum/post-429703.html |
| APR | M | Arcade Parts and Repair「MSM5205」 | 参照日 | https://www.arcadepartsandrepair.com/store/integrated-circuits/ttl-chips/msm5205/ |
| OKI6585 | M | OKI MSM6585 データシート（2004-08） | 2004-08 | https://manualmachine.com/oki/msm6585/8341275-technical-data |
| HAD | M | Hackaday「What to know when buying chips that haven't been made for three decades」 | 2020-01-15 | https://hackaday.com/2020/01/15/what-to-know-when-buying-chips-that-havent-been-made-for-three-decades |
| IA | M | innovASIC IA63484 データシート | 日付なし | https://datasheetspdf.com/pdf/382407/InnovASIC/IA63484/2 |
| FCIA | M | Findchips「IA63484-PLC68I」 | 参照日 | https://www.findchips.com/detail/ia63484-plc68i/Innovasic-Inc |
| EBHD | M | eBay.de・Findchips の HD63484 掲載 | 参照日 | https://www.ebay.de/p/1328894401 ／ https://www.findchips.com/detail/hd63484cp98/Hitachi-Ltd |
| GHACRTC | M | GitHub 検索「hd63484」「acrtc」（例：gapux/ACRTC_MIVAC_BT450） | 参照日 | https://github.com/gapux/ACRTC_MIVAC_BT450 |
| DKC10 | M | DigiKey「TMS320C10FNL」（Rochester） | 参照日 | https://www.digikey.de/en/products/detail/rochester-electronics-llc/TMS320C10FNL/12128546 |
| EBC10 | M | eBay.de「TMS320C10NL」 | 参照日 | https://www.ebay.de/itm/115672726130 |
| FCP20 | M | Findchips「UPD77P20D」 | 参照日 | https://www.findchips.com/detail/upd77p20d/NEC-Electronics-Group |
| IS18 | C | Intersil, CD22100／CD22101 データシート（1999） | 1999 | https://manualmachine.com/intersil/cd22100/3935117-datasheet ／ https://manualmachine.com/intersil/cd22101/3935114-datasheet |
| DKCD | M | DigiKey「CD22101F」（Rochester） | 参照日 | https://www.digikey.de/en/products/detail/rochester-electronics-llc/CD22101F/12114125 |
| PCN8816 | M | Microchip PCN MFOL-05GIFW725（検索結果の抜粋で確認） | 2025-11-17 | https://www.mouser.com/PCN/Microchip_Technology_PCN_MFOL_05GIFW725.pdf |
| CD3494 | M | Renesas「CD22M3494」 | 参照日 | https://www.renesas.com/en/products/CD22M3494 |
| CH446 | M | WCH CH446 データシート（中文 v1C §7.4、英語版） | 参照日 | https://atta.szlcsc.com/upload/public/pdf/source/20170515/1494827270940.PDF ／ https://bitsavers.mirrorservice.org/components/wch/_dataSheets/CH446DS1.PDF |
| TDCM | M | TDC1048・AD9048 の掲載と ADI の回答 | 参照日 | https://datasheets360.com/part/detail/tdc1048b6v/-7659013018360648385/ ／ https://www.analog.com/en/products/ad9048.html ／ https://ez.analog.com/data_converters/high-speed_adcs/f/q-a/21325/ad9048---any-pin-to-pin-replacement |
| MC77M | M | MC1377P のブローカー掲載 | 参照日 | https://datasheets360.com/part/detail/mc1377p/205309411900931250/ ／ https://ic-phoenix.com/M/M101/MC1377P.html |
| MC13077 | C | Motorola MC13077 データシート | 1996 | https://manualmachine.com/motorola/mc13077p/4161041-datasheet |
| PDN724 | M | ADI PDN 26_0046（AD724／AD725。検索結果の抜粋で確認） | 2026-07-29 | https://www.analog.com/media/en/PCN/ADI_PDN_26_0046_Rev_-_Form.pdf |
| CXA45 | M | CXA1145 の掲載と互換品の情報 | 参照日 | https://www.ebay.com/itm/323552499894 ／ https://wiki.neogeodev.org/index.php/CXA1145 |
