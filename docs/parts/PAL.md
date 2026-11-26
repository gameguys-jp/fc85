# PAL の速度の品と AC 特性（1983〜1986年の資料）
調査 v0.1 ／ 調査日 2026-11-26

凡例：`[D 資料ID 頁]` 出典あり。`[D: 計算]` 出典の値からの計算。`[V]` 未確認。「推定」は本書の推論。資料IDは末尾の表を参照。頁は印刷の頁で、PDF の頁を（PDF n）と添える。

資料の区分：**A** 1984年以前、**B** 1985年中、**C** 1986年以降（後年資料）。年代の根拠は A・B だけ。C は値の確認と傾向の参考にだけ使う。

この台帳は事実だけを書く。本機での使い方（どの PAL をどの品にするか、時間の予算）は [fabric.md](../spec/fabric.md) §03 と §09。

---

## 1　要旨

| 問い | 答え | 出典 |
|---|---|---|
| 20 ピンの B 品（tPD 15 ns）は 1985年にあったか | あった。TI の TIBPAL16xx-15C は 1984年2月の版を 1985年1月に改めたデータシートが 1985年の本に予告の印なしで載る。National は 1985年の設計の手引きの表に B を予告の印なしで載せる（B-2 には予告の印）。MMI は 1985年の本では ADVANCE INFORMATION | [D TITTL85V4 p.3-37, p.3-46]、[D NSPLDG85 p.42 表 4.2.4]、[D MMI85 p.15-7] |
| 24 ピンの B 品（PAL20xxB）は 1985年にあったか | 1985年の資料には見つからない。最初は MMI の 1986年の手引きと TI の 1986年6月のデータシート | [D MMIPAL86 PDF99]、[D TITTL86V4 p.3-69] |
| 24 ピンの半電力の A-2 品は 1985年にあったか | TI の 1985年の本に PRODUCT PREVIEW（典型値だけ）として載るだけ。MMI と National の 1985年の資料にはない。MMI の最初は 1986年 | [D TITTL85V4 p.3-17]、[D NSPLDG85 表 4.2.5]、[D MMIPAL86 p.2-75] |
| 24 ピンの A 品（PAL20L8A・20R8A・20R6A・20R4A） | 1983年から量産品 | [D MMIPAL83 p.3-13]、[D MMI85 p.5-31] |
| 20 ピンの A 品と A-2 品 | 1983年から量産品 | [D MMIPAL83 p.3-12, p.3-17]、[D MMI85 p.5-30, p.5-33] |

## 2　AC 特性（商用の温度範囲、最大値）

単位は ns。fMAX は最小で MHz、ICC は最大で mA。OE は「ピン 11（24 ピンは 13）からの許可・禁止；積の項からの許可・禁止」。負荷は MMI が R1 200 Ω・R2 390 Ω・50 pF、TI が RL 500 Ω・50 pF。

| 品 | tPD | tCO | tSU | tH | tW（H/L） | fMAX | OE | ICC | 出典と区分 |
|---|---:|---:|---:|---:|---|---:|---|---:|---|
| MMI 16L8・16R8・16R6・16R4（標準） | 35 | 25 | 35 | 0 | 25 | 16 | 25/25；35/35 | 180 | [D MMIPAL83 p.3-10]（A）。MMI85 p.5-28 も同じ |
| MMI 16xxA | 25 | 15 | 25（注文で 20） | 0 | 15 | 28.5 | 20/20；25/25 | 180 | [D MMIPAL83 p.3-12]（A）。MMI85 p.5-30 も同じ |
| MMI 16xxA-2 | 35 | 25 | 35 | 0 | 25 | 16 | 25/25；35/35 | 90 | [D MMIPAL83 p.3-17]（A）。MMI85 p.5-33 も同じ |
| MMI 16xxA-4 | 55 | 35 | 60 | 0 | 30 | 11 | 30；50 | 50 | [D MMIPAL83 p.3-18]、[D MMI85 p.5-34] |
| MMI 20L8A・20R8A・20R6A・20R4A | 25 | 15 | 25 | 0 | 15 | 28.5 | 20/20；25/25 | 210 | [D MMIPAL83 p.3-13]（A）、[D MMI85 p.5-31]（B） |
| MMI 16xxB（1985年の予告） | 15 | 12 | — | — | — | 40 | — | 180 | [D MMI85 p.15-7]（B、ADVANCE INFORMATION） |
| National 16xxB | 15 | 12 | 15 | 0 | — | 40 | 15/15；20/15 | 180 | [D NSPLDG85 p.42 表 4.2.4]（B）。同じ本のデータシート（p.282–283）は tSU 35・tW 25 で、表と食い違う [V] |
| National 16xxB-2 | 25 | 15 | 25 | — | — | — | — | 90 | [D NSPLDG85 p.42]（B、Preliminary information） |
| AMD AmPAL16xxA | 25 | 15 | 20 | 0 | 15 | 28.5 | 20/20；25/25 | 180（16R8A）、155（16L8A） | [D AMDBMLI85 p.12-7, p.12-8]（B） |
| TI PAL16xxA | 25 | 15 | 20 | 0 | 12/16 | 35 | 22/15；25/25 | 180 | 1985年1月に改めたデータシート [D TITTL85V4 p.3-3]（B）。値は 1986年の同じデータシート [D TITTL86V4 PDF57] |
| TI PAL16xxA-2 | 35 | 25 | 28 | 0 | 25/25 | 18 | 25/20；35/30 | 90 | 同上。値は [D TITTL86V4 PDF58] |
| TI PAL20xxA | 25 | 15 | 25 | 0 | 15/15 | 30 | 20/20；25/25 | 210 | [D TITTL85V4 p.3-17]（B、PRODUCT PREVIEW）。値は [D TITTL86V4 PDF71] |
| TI TIBPAL16xx-15C | 15 | 12 | 15 | 0 | 8/9 | 50 | 12/10；15/15 | 180 | [D TITTL85V4 p.3-37, p.3-46]（B、予告の印なし）。値は [D TITTL86V4 p.3-40] |
| TI TIBPAL16xx-25C（低電力） | 25 | 15 | 20 | 0 | 10/15 | 30 | 20/20；25/25 | 100 | [D TITTL85V4 p.3-47]（B、PRODUCT PREVIEW） |
| MMI 20xxA-2 | 35 | 25 | 35 | 0 | 25 | 16 | 25；35 | 105 | [D MMIPAL86 p.2-75]（C） |
| MMI 20xxB | 15 | 12 | 15 | 0 | 12/10 | 37（帰還あり） | 15/12；18/15 | 210 | [D MMIPAL86 PDF99]（C） |
| TI TIBPAL20xx-15C | 15 | 12 | 15 | 0 | 10/12 | 45 | 15/12；18/15 | 180 | [D TITTL86V4 p.3-69, p.3-70]（C、1986年6月） |

- TITTL85V4 の表は走査が崩れて読めない。TI の値は、同じデータシートの 1986年の版から取った。1985年の版と照合できたのは TIBPAL16-15C の ICC 180 mA だけ [V]。
- 「最初」は、調べた本の中での最初。業界誌の発表日は調べていない。

## 3　本機の部品表に関係する付記

| 部品 | 事実 | 出典 |
|---|---|---|
| 74LS534 | TI の 1985年の TTL の本（第 2 巻）、1984年の第 1 巻の索引（ALS534 と AS534 だけ）、Motorola の 1983年の本（ALS534 と、F534 の Advance Information）、National の 1984年の本に見つからない | [D TITTL85V2]、[D TITTL84V1]、[D MOTSTTL83 PDF628]、[D NSLOG84V1] |
| 日立の HD74LS | 1985年以前の日立の TTL の本は、bitsavers・archive.org に見つからない [V] | — |
| 74LS 系の AC 特性 | 74LS374 CLK→Q 28、tSU 20。74LS377 CLK→Q 27、/EN の準備 25。74LS173 tPZL 27。74LS157 選択 27。74LS151 選択→Y 43。74LS74 CLK→Q 25/40。74LS175 CLK→Q 25。74LS393 A→QD 60。いずれも VCC 5 V、25 °C の最大値 | [D TITTL85V2 p.3-1024, 3-1036, 3-688, 3-594, 3-567, 3-302, 3-693, 3-1061] |
| 74S189、74S283、63S281A | 74S189 tAA 35、tWP 25、tDSW 25（0〜70 °C）。74S283 A・B→C4 12、C0→C4 11、C0→Σ 18。63S281A tAA 28 | [D NS83IBL p.22-4, 22-5, 22-7]、[D TITTL85V2 p.3-921]、[D MMI85 p.3-18] |
| 74S02、74S30、74S32、74S157（2026-11-27 追記） | 74S02 5.5（tPLH・tPHL）。74S30 6（tPLH）・7（tPHL）。74S32 7（tPLH）。74S157 データ→Y 7.5、選択→Y 15。ICC の典型・最大（ICCH と ICCL の平均）は 74S02 22・37、74S30 4・8、74S32 28・50、74S157 50・78 mA。いずれも VCC 5 V、25 °C | [D TITTL85V2 p.3-22, 3-146, 3-155, 3-595] |
| 74LS670（2026-11-27 追記） | 読出しの番地→Q 45、読出しの許可→出力の禁止 35 | [D TITTL85V2 p.3-1273] |
| 74ALS534（2026-11-27 追記） | 74ALS534A：tSU 10、tH 0（0〜70 °C）。CLK→Q 16、tPZL 18 は、走査の表の列の読み方が確かでない [V]。1984年の第 1 巻の索引に載る | [D TITTL84V1 p.3-131] |
| 74F377（2026-11-27 追記） | fMAX 80、CP→Q 11（tPLH）・12（tPHL）、tSU（D・/CE）3、tH（D・/CE）1、クロックの Low の幅 4（0〜70 °C）。Fairchild の 1985年の本は 25 °C の典型だけで、商用の範囲の欄は空き | [D SIGFAST84 p.5-286, 5-287]、[D FSFAST85 p.4-237] |
| 登録の出力の帰還（2026-11-27 追記） | 登録の PAL の出力を同じ PAL の組合せの出力に使うと、遅れは tCO＋tPD（A 品で 15＋25＝40）。資料は帰還の遅れを別に書かない（推定） | — |

## 資料

調査日はすべて 2026-11-26。bitsavers の基点は `http://bitsavers.org/components/`。

| ID | 区分 | 資料と使った頁 | 年 | URL |
|---|---|---|---|---|
| MMIPAL83 | A | MMI PAL Handbook, 3rd ed.；3-3、3-7、3-10、3-12、3-13、3-15〜3-18 | 1983 | …/mmi/_dataBooks/1983_MMI_PAL_Handbook_3ed.pdf |
| MMI85 | B | MMI LSI Databook, 6th ed.；3-17〜3-19、5-20、5-28〜5-34、7-17〜7-19、15-2、15-7 | 1985 | …/mmi/_dataBooks/1985_MMI_LSI_Databook_6ed.pdf |
| MMIPAL86 | C | MMI PAL/PLE Handbook, 5th ed.；2-26〜2-28、2-75、PDF99 | 1986 | …/mmi/_dataBooks/1986_MMI_PAL_PLE_Handbook_5ed.pdf |
| TITTL84V1 | A | TI The TTL Data Book Vol. 1；索引、3-131（2026-11-27） | 1984 | …/ti/_dataBooks/1984_TI_The_TTL_Data_Book_Vol_1.pdf |
| TITTL85V2 | B | TI The TTL Data Book Vol. 2；上の表の頁 | 1985 | …/ti/_dataBooks/1985_TI_The_TTL_Data_Book_Vol_2.pdf |
| TITTL85V4 | B | TI The TTL Data Book Vol. 4（SDZD001）；索引、3-3、3-17、3-35、3-37、3-46、3-47 | 1985 | https://archive.org/details/texas-instruments-the-ttl-data-book-volume-4-bipolar-programmable-logic-and-memory-1985 |
| TITTL86V4 | C | TI The TTL Data Book Vol. 4（SDZD001B）；PDF57–58、71–72、85–86、114–117 | 1986 | …/ti/_dataBooks/1986_TI_The_TTL_Data_Book_Vol_4.pdf |
| NS83IBL | A | National Interface/Bipolar LSI/Bipolar Memory/Programmable Logic Data Book；22-4、22-5、22-7 | 1983 | …/national/_dataBooks/1983_National_Interface-Bipolar_LSI-Bipolar_Memory_Programmable_Logic_Data_Book.pdf |
| NSPLDG85 | B | National Programmable Logic Design Guide；42–43、282–283 | 1985 | …/national/_dataBooks/1985_National_Programmable_Logic_Design_Guide.pdf |
| NSLOG84V1 | A | National Logic Databook Vol. 1；74LS534 がないことの確認 | 1984 | …/national/_dataBooks/1984_National_Logic_Databook_Volume_1.pdf |
| AMDBMLI85 | B | AMD Bipolar Microprocessor Logic and Interface；12-7、12-8 | 1985 | …/amd/_dataBooks/1985_AMD_Bipolar_Microprocessor_Logic_and_Interface.pdf |
| MOTSTTL83 | A | Motorola Schottky TTL Data；索引、PDF628 | 1983 | …/motorola/_dataBooks/1983_Motorola_Schottky_TTL_Data.pdf |
| SIGFAST84 | A | Signetics FAST Data Manual；5-284〜5-287（2026-11-27） | 1984 | …/signetics/_dataBooks/1984_Signetics_FAST_Data_Manual.pdf |
| FSFAST85 | B | Fairchild FAST Data Book；4-235〜4-237（2026-11-27） | 1985 | …/fairchild/_dataBooks/1985_Fairchild_FAST_Data_Book.pdf |
