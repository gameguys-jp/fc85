# Data Fabric の回路と部品表（最終試作機）

**v0.12**  
2026年11月27日

> 状態：作業中の版。1985年の最終試作機の Data Fabric（2 チャネル）を、PAL と 74LS・74S で組む回路の案と部品表。動作の定義は [仕様書](hardware.md) §06 と [DR-046](../decisions.md)。この文書は、その動作を満たす回路を示し、部品の数と面積を [board.md](board.md) に渡す。値はすべて [P] か [V]。関連：[DR-010](../decisions.md)、[DR-024](../decisions.md)、[DR-041](../decisions.md)、[DR-043](../decisions.md)、[DR-046](../decisions.md)、[DR-047](../decisions.md)、[OI-050](../open-issues.md)、[OI-051](../open-issues.md)、[OI-052](../open-issues.md)、[DR-048](../decisions.md)、[OI-054](../open-issues.md)、[DR-050](../decisions.md)、[DR-084](../decisions.md)、[DR-085](../decisions.md)、[OI-082](../open-issues.md)、[OI-083](../open-issues.md)、[OI-084](../open-issues.md)、[DR-088](../decisions.md)、[DR-089](../decisions.md)、[DR-090](../decisions.md)、[OI-087](../open-issues.md)、[OI-088](../open-issues.md)、[OI-089](../open-issues.md)、[DR-091](../decisions.md)、[OI-090](../open-issues.md)

## 01　前提と範囲

- 対象は B 基板の Data Fabric のブロック（board.md §02 の「Data Fabric」の行）。A 基板の Staging RAM の調停、ACRTC とのブリッジ（DR-016 の同期化の FF）、DSP・ADPCM・本体 DAC のポートアダプタ、Temporal RAM のカウンタは、それぞれのブロックに数える。FIFO の 67401×14 は数だけを書く。
- 量産計画のゲートアレイ（DR-010）は対象にしない。
- 式は PALASM の書き方（`/` は否定、`*` は AND、`+` は OR、`:=` は登録出力）。L 型の PAL は出力が反転するので、左辺を `/X` と書き、項に入力の否定を入れて、ピンの論理を正にする。
- 1 つの PAL は見出しのコメント `; 名前 (型番)` で始め、`PIN` の行にピン 1 から順に信号を並べる（GND・VCC は電源、NC は使わないピン。登録出力の PAL のピン 1 はクロック、/OE をいつも出すなら GND）。`IF (積)` は 3 ステートの出力の許可。`+` で始まる行は前の式の続き [P]（DR-084）。
- この文書の式のコードブロックは、PAL に焼く式そのもので、[tools/fabric/pal/](../../tools/fabric/pal/) のファイルと同じ。[tools/fabric/fabric_logic.py](../../tools/fabric/fabric_logic.py) が式を読み、ピンと項が型番に収まるかと、仕様の定義と同じ値を出すかを確かめ、エミュレータの `emulator/crates/board/src/fabric_pal.rs` を生成する。ポートの属性の PROM（ATR）と MSK の中身も、同じファイルで作る [P]（DR-084）。
- 変換部（ROT、MSK、ANX）と準備の信号（RDY）の式、ATR の中身は、仕様の定義と突き合わせた [P]。
- 順序と状態の PAL のうち、A ファイルの操作（AFC）と判断と状態（CST、ELG、SEQ、ELS、EVT、ADC、CHS、STS）は式を書いた [P]（DR-085）。式から生成した関数をつないだ門の模型（`emulator/crates/board/tests/fabric_gate/mod.rs`）を、段階 2 のエミュレータのスロットの模型（`fabric.rs`、[emulator.md](../impl/emulator.md) §03）と並べて動かし、スロットごとの記録が一致することを確かめた。
- アクセスの制御（TIM、DPA、OPA、OPB、STB0–1、P1A、P2A、U38、NX・OP）と CPU の窓（HDEC0–1、HACK、PRST）も式を書いた [P]（DR-089）。門の模型はこれらの式もつなぎ、判断・状態・アクセスの制御・CPU の窓のすべてが PAL の式になった。P1A・P2A・STB は、fabric_logic.py の仕様の定義（ポートの模型 acrtc.rs・doc.rs の 16 MHz の縁の動き）と、届きうる入力と状態の全部で突き合わせた。
- スロットの中の時刻は、DR-091 で 1985年の PAL と TTL の遅れ（データブックの最大値）で閉じた（03 の時間の予算）。LCLK を q2 の縁（MCLK と同じ）へ早め、桁上げ（CY）を加算器ではなく A バスの 0 の検出から取り、AFC の番地を SCLK のレジスタにし、q1〜q3 の縁で来たポートの答えを次のスロットに数える。延びたアクセス（port 1、DOC の port 2）の終わりが最大 1 スロット遅れ、スロットの記録（`SlotRecord`）が変わった [P]（OI-087）。
- 部品の型番は MMI の 1983年の PAL ハンドブックと 1985年のデータブック（[review-digital.md](../parts/review-digital.md) の MMIPAL83、MMI85）と、74LS・74S の標準品から選んだ。日立の同等品の有無は [V]（09）。

## 02　構成

```
                     CPU（ホストバス、A 基板から）
                        │ D15–0            │ A8–1、R/W、/UDS、/LDS、/FCS
              ┌─────────┴─────────┐   ┌────┴─────────────┐
              │ U1–U2 書込み       │   │ HDEC0–1、U5–U6   │ 書込みの信号
              │ U3–U4 読出し（反転）│   │ HACK（DTACK）    │ A ファイルの要求 HREQ
              └──┬──────────────┬─┘   └──────────────────┘
          ID（書込み）        RD（読出し、引上げ）
   ┌──────────┼─────────┬─────────┐       ▲ U7–U8、U9–U10、U11
   │設定の      │ B ファイル │ A ファイル │       │
   │レジスタ    │ 74LS670  │ 74S189   ├─A バス─┼──────────────┐
   │SRC・DST・  │ (STEP)   │ (番地・   │        │              │
   │FORMAT →DB │   │B バス  │  COUNT)  │     加算器 74S283 ──→ 和のラッチ SL（→ A ファイル）
   │SHIFT・MASK│   └───────→┴──────────┘                     │
   │・XOR →変換 │                                    PS・PD（番地のラッチ）→ PA バス → ポート
   │TRIGGER    │
   └───────────┘
   判断バス DB（候補のチャネルの SRC・DST・FORMAT）→ U32（次のポート）→ ATR・U33 → SEQ・ELG
                                                               └→ NX → OP（アクセスのレジスタ）→ STB
   FD バス（16 bit）：ポート → IL → ROT0–1 → ROT2–3 → ANX0–3（MSK、MASK、XOR）→ FD バス → ポート
```

| バス | 幅 | 中身 |
|---|---|---|
| ID | 16 | CPU の書込みデータ（U1–U2 から）。設定のレジスタ、B ファイル、A ファイル（U12–U13 で反転）へ |
| RD | 16 | CPU の読出しデータの反転。抵抗で引き上げ、U3–U4 で反転して CPU へ。何も出さないアドレスは 0 が読める（DR-046 の「読むと 0」） |
| A バス | 16 | A ファイルの出力（値そのもの。74S189 は反転して出すので、反転した値を書く）。加算器の A と、PS・PD、U7–U8 へ |
| B バス | 16 | B ファイルの出力。B ファイルを出さないと、引上げで 0xFFFF |
| PA | 16 | ポートの番地。PS（読出し元）か PD（書込み先）が出す。A 基板（port 0、9）と port 2 へ |
| FD | 16 | Fabric のデータ。R のスロットはポートが、W のスロットは ANX が出す |
| DB | 11 | 判断バス。SEQ が選んだチャネル（DBCH）の SRC・DST・FORMAT |
| 変換の設定 | 37 | アクセスのチャネル（OP の CH）の SHIFT（5）・MASK（16）・XOR（16） |

## 03　スロットの中の時刻

Fabric は 16 MHz から作る 4 つの位相（q0〜q3、各 62.5 ns）で動く。スロット（250 ns）の頭は TIM が A 基板の Staging の調停に合わせる（SX が S と X を示す。DR-024）[P]。

| 時点 | 起きること |
|---|---|
| スロットの頭（SCLK↑） | SEQ が判断を取り込む：このスロットの A ファイルの操作（AOP）と、次のスロットのアクセス。AFC0・AFC1 が、同じ入力から A ファイルの番地と信号を取り込む（DR-091）。NX が、次のアクセス（ポート、向き、チャネル、FORMAT、W16、LG、FLAG、TWO）と STALL の評価を、判断バスから取り込む（DR-089）。OP は、空いていれば NX から、このスロットのアクセスを取り込む（判断は 1 つ前のスロットの頭）。P1A・P2A は、前のスロットの q1〜q3 で預かった答えを、このスロットの答え（P1R・P2R）にする。IL は、前のスロットで終わった読出しの FD を取り込む（/EN ＝ DNRQ）[P]（DR-085、DR-091） |
| q0〜q1 | A ファイルを読み、加算器が落ち着く。A バスの 0 の検出（U65–U67）が桁上げ（CY）を出す。W のアクセスでは ANX が FD に出す（OP から約 110 ns で落ち着く。IL 20 ns、ROT 2 段 50 ns、ANX 25 ns、緩衝器 12 ns）。STB がポートのアクセスの信号を出す |
| 中ほど（MCLK↑・LCLK↑） | RDY が、このスロットの判断バスのチャネル（次のスロットの頭の候補）の準備を取り込む。PS か PD が A バスの番地を、SL が和を取り込む。CPU の読出しでは U7–U8 が A バスを取り込む。CHS・STS・ELS が、桁上げ（CY）とこのスロットの出来事（アクセスの完了、ERROR、CPU の要求）から、チャネルと要素の状態を取り込む。OPB が、アクセスの終わり（ODN）、時間切れ（TOR）、読出しの完了（DNRQ）を取り込む。U36 がトリガの 1 段目を取り込む。その後、CPU の要求の FF を消す [P]（DR-085、DR-091） |
| q3（WPH） | A ファイルの書込み（/AWE ＝ AWQN ∨ 16 MHz で q3 の後半）。HACK が、A ファイルの窓の許可のスロットで CPU に DTACK を返す（AK）。FIFO への書込みの信号（SI）は q2〜q3 [V] |
| スロットの終わり | R のアクセスで、ポートが FD に出した値を、次の SCLK で IL が取り込む。FIFO から読むときは、読出しの信号（SO）を q3 に出し、その前に取り込む [V] |

- FIFO の満杯・空の信号は、書込み・読出しの信号の終わりから変わる。スロット m のアクセスの結果は、スロット m+1 の中ほどの取込みで見える。このため、満杯・空の信号を持つポートにアクセスするスロットと、その次のスロットの頭では、新しい要素を始めない（DR-046 (2)）。
- ポートごとのアクセスの信号の位置（67401 の SI・SO の幅、Staging RAM の書込みの信号、ACRTC の DACK）は、部品の AC 特性で確かめる [V]（OI-050）。
- 頭の判断の入力は、LCLK の後に CHS・STS・ELS → CST → ELG と落ち着き、次の SCLK までに入る必要がある。DR-089 までは LCLK が q3 で、62.5 ns に PAL 1 段も入らなかった。DR-091 で LCLK を q2 へ早めて 125 ns にし、下の時間の予算のとおり、1985年の A 品で入る [P]（OI-083、OI-087）。
- TIM（PAL16R8A、16 MHz）が位相 PH1・PH0 を数え、SCLK を q0 の縁の後に、MCLK・LCLK を q2 の縁の後に High にする（次の 16 MHz の縁が、その時点の縁）。MCLK と LCLK は同じ式で、負荷を 2 本のピンに分ける。WPHN は q3 の間 Low で、次の 16 MHz の縁が q0 であることを P1A・P2A・STB1 に示す。AWQN は、書込みのある A ファイルの操作（AFC0 の AWN）のとき q3 の間 Low で、74S32（U68）が 16 MHz と OR して /AWE（q3 の後半、31.25 ns）にする。S・X は q0 の縁で入れ替わる。電源投入の RESN で q3・X に戻す [P]（DR-089、DR-091）。
- ポートの答え（DR-091）：P1A・P2A は、答え（port 1 の同期化した DTACK、port 2 の書込みのラッチと読出しの窓の終わり）が q0 の縁ならそのスロットの答え（P1R・P2R）にし、q1〜q3 の縁なら預かり（PND）にして次のスロットの q0 の縁で P1R・P2R にする。P1R・P2R は q0 の縁の登録なので、OPB・EVT・CHS は LCLK までの 125 ns を使える。延びたアクセスは、答えが q1〜q3 なら終わりが 1 スロット遅れる。打切り（OPA の ABT）は 74S32（U68）で WPH と合わせ（ABQ ＝ ABT∧WPH）、q0 の縁だけで効く。そのとき預かった答えは捨てる。時間切れの 16 スロット目の q1〜q3 で来た答えは数えないが、アクセスは装置の側で済んでいる（P2A がラッチした書込みは窓で書き、ACRTC は語を済ませている）[P]。
- 預かった port 1 の読出しの値は、次のスロットの終わりに IL が取り込むまで、A 基板のブリッジ（Fabric 側のデータの切替）が保つ必要がある。DR-089 までも、q2 の答えをスロットの終わりに取り込むので同じ条件で、ブリッジのラッチの有無は [V]（host-acrtc-interface.md §7.6）。DOC の port 2 の読出しは U58 が保つ。

### 時間の予算（OI-083、OI-087、DR-091）

レジスタからレジスタまでの経路の遅れを、データブックの最大値（商用の温度範囲）で足した [P]（DR-090、DR-091）。PAL の値は [PAL.md](../parts/PAL.md)：24 ピンと 20 ピンの A 品 tPD 25・tCO 15・tSU 25 ns、20 ピンの A-2 品 35・25・35 ns。登録の出力を同じ PAL の中で組合せの出力に帰す経路（AFC0 の CIN、EVT の KIL → BLK）は、tCO＋tPD か tPD＋tPD と数えた。TTL は TI の 1985年のデータブック（TITTL85V2）：74LS173 の許可 27、74LS157 の選択 27、74LS151 の選択 43、74LS377 の CLK→Q 27・/EN の準備 25、74LS74 の CLK→Q 40、74LS175 の CLK→Q 25、74LS670 の読出しの番地→Q 45・読出しの許可の禁止 35、74S02 5.5、74S30 7、74S32 7、74S157 のデータ→Y 7.5・選択→Y 15。74S189 の番地からの読出し 35、書込みの幅 25（NS83IBL）、63S281A 28（MMI85）、74S283 の A・B→C4 12、C0→C4 11、C0→Σ 18（TITTL85V2）、74ALS534 の tSU 10（TITTL84V1）、74F377 の CLK→Q 12（SIGFAST84）。配線の遅れは見ていない。SCLK・MCLK・LCLK は TIM の出力なので、16 MHz の縁より TIM の tCO（15 ns）遅れる。16 MHz の縁で取り込む PAL（TIM、P1A、P2A、HACK）と SCLK・LCLK の間をまたぐ経路は、使える時間にこの差を入れた（クロックの分配 U39 の遅れは、両方の側に同じだけ入ると仮定 [V]）。

| 経路 | 起点 → 終点 | 使える時間 | 遅れ | 余裕 |
|---|---|---:|---|---:|
| 段階 → 頭の判断 | CHS（LCLK）→ CST → ELG → SEQ（SCLK） | 125 | 15＋25＋25＋25＝90 | 35 |
| 要素の状態 → 頭の判断 | ELS（LCLK）→ ELG → SEQ | 125 | 15＋25＋25＝65 | 60 |
| トリガの許可 → 頭の判断 | STS（LCLK）→ ARMX → CST → ELG → SEQ | 125 | 15＋25＋25＋25＋25＝115 | 10 |
| アクセスの終わり → OP の取込み | OPB の ODN（LCLK）→ OPA の OPLD → OP（74LS377 の /EN） | 125 | 15＋25＋25＝65 | 60 |
| アクセスの終わり → 頭の判断 | ODN → OPA の CD → ELG → SEQ | 125 | 15＋25＋25＋25＝90 | 35 |
| 時間切れ → 打切り | OPB の TOR（LCLK）→ OPA の ABT → 74S32（ABQ）→ P1A・P2A（q0 の縁、tSU 25） | 125−15＝110 | 15＋25＋7＋25＝72 | 38 |
| q3 の印 → 打切り | TIM の WPHN（q3 の縁）→ 74S32（ABQ）→ P1A・P2A（q0 の縁） | 62.5 | 15＋7＋25＝47 | 15.5 |
| アクセスの始まり → port 2 | OP0（SCLK、74F377）の P0・W → P2A（q1 の縁） | 62.5−15＝47.5 | 12＋25＝37 | 10.5 |
| 延びたアクセス → port 2 | OPA の LA（SCLK）→ P2A（q1 の縁） | 47.5 | 15＋25＝40 | 7.5 |
| ポートの答え → 状態 | P1A・P2A の P1R・P2R（q0 の縁）→ OPB の ERRK → EVT の KIL → EVT の BLK → CHS・STS（LCLK） | 125＋15＝140 | 15＋25＋25＋25＋25＝115 | 25 |
| A ファイルの番地 → A バス | AFC0（SCLK）の AR0N → U64（74S157）→ 74S189 | — | 15＋7.5＋35＝57.5 | — |
| A バス → 桁上げ → 状態 | A バス → 74S02 → 74S30（CY）→ CHS・ELS（LCLK） | 125 | 57.5＋5.5＋7＋25＝95 | 30 |
| 桁上げ → ブロックの終わり → 状態 | CY → EVT の BLK → CHS・STS（LCLK） | 125 | 70＋25＋25＝120 | 5 |
| A ファイル → 和のラッチ | A バス → 74S283×4（A→C4、C0→C4×2、C0→Σ）→ SL（74ALS534、LCLK、tSU 10） | 125 | 57.5＋12＋11＋11＋18＋10＝119.5 | 5.5 |
| B ファイル → 和のラッチ | AFC1（SCLK）→ 74LS670 の読出しの番地 → 74S283×4 → SL | 125 | 15＋45＋52＋10＝122 | 3 |
| Cin → 和のラッチ | AFC0 の CIN（登録の帰還）→ 74S283 の C0 → SL | 125 | 15＋25＋11＋11＋11＋18＋10＝101 | 24 |
| A ファイルの書込み | 書きの番地（bit 0 は q2 の縁から TIM 15＋U64 の選択 15）〜 /AWE（q3 の後半）の終わり（次の 16 MHz の縁から 74S32 の 7 以内）。番地は同じ縁の後の TIM・AFC0 の tCO と U64 の選択の後に変わる | — | 幅 31.25（tWP 25） | 保持 [V] |
| 判断バス → 準備 | SEQ（SCLK）→ 74LS173 の許可 → RDY（MCLK） | 125 | 15＋27＋25＝67 | 58 |
| 準備 → 頭の判断 | RDY（MCLK）→ READY → ELG → SEQ（SCLK） | 125 | 15＋25＋25＋25＝90 | 35 |
| 判断バス → 属性 → 頭の判断 | SEQ（SCLK）→ 74LS173 → DPA → ELG → SEQ | 250 | 15＋27＋25＋25＋25＝117 | 133 |
| START の検査 | CHS（LCLK）→ CST（CRLD、CK0）→ つなぎ（USEL）→ U32 → ATR → U33 → つなぎ（OKS）→ CHS（次の LCLK） | 250 | 15＋25＋15＋27＋28＋43＋15＋25＝193 | 57 |

- DR-090 の表では、LCLK を q3 に置き、LCLK から次の SCLK までの 62.5 ns に、PAL を 1 段通すだけの経路（24 ピンの A 品で 15＋25＋25＝65 ns）も入らなかった。1985年には 24 ピンの B 品も A-2 品もない（PAL.md）。DR-091 で、LCLK を q2 の縁（MCLK と同じ）へ早めて 125 ns にし、ほかを次のように直した [P]。
  - 桁上げ（CY）：加算器の C4 は、A ファイル → 74S283×4 → EVT → CHS で LCLK に入らないので、A バスの 0 の検出（74S02×2・74S30、U65–U67）から取る。CY を使うのは COUNT の更新と再設定の 3 段目（B ＝ 0xFFFF、Cin ＝ 0）だけで、そこでは CY ＝ A ≠ 0 が加算器の桁上げと同じ（05）。
  - AFC0・AFC1：組合せの PAL（PAL16L8A）を、SCLK で同じ入力（ELG の AC、CST の CK、次の ACH ＝ DBCH）を取り込むレジスタの PAL（AFC0 は PAL20R6A、AFC1 は PAL16R8A）にした。A ファイルの番地は SCLK の 15 ns 後に出る。語の bit 0 は、読み（AR0N）と書き（AW0N）を AFC0 が別に登録し、U64（74S157）が PH1 で選ぶ。AFC0 の中で組み合わせると、登録の帰還を通るので 15＋25＝40 ns になり、A ファイルの読みと和のラッチが入らない。
  - SL（和のラッチ）：74LS534 の tSU 20 では 129.5 ns で入らないので、74ALS534（tSU 10）にした。
  - ポートの答え：q1〜q3 の縁で来た答えは P1A・P2A が預かり、次のスロットの q0 の縁で P1R・P2R にする（03 の上）。DR-090 の「port 1 の答え（q2 の縁）→ CHS」「port 2 の答え（q3 の縁）→ 同じ LCLK」は、q0 の縁から LCLK までの経路（上の表の「ポートの答え → 状態」）になった。
  - CHS の NBSY・NRUN（次の段階）は、EVT の BLK を通すと CY → EVT → CHS → STS が 70＋25＋25＋25＝145 ns で入らない。BLK を除いた部分（NBSYX、NRUN0）を CHS が出し、STS が BLK と合わせる。
  - P1A（PAL16R6A）、P2A（PAL16R8A）、HACK（PAL16R4A）を、A-2 品から A 品（tSU 25）にした。A-2 品（tSU 35）では、OPA の LA から P2A の q1 の縁までが 15＋35＝50 ns で 2.5 ns 足りない。同期化の後の入力（下の表）と、q3 の印からの打切りの余裕も小さい（5.5 ns）。
  - OP の 2 個の 74LS377 のうち、ポートと向きを持つ OP0 を 74F377 にした。P2A は q1 の縁でアクセスを見るので、74LS377（CLK→Q 27）では 15＋27＋25＝67 ns で入らない（DR-089 の回路からあった不足で、DR-090 の表になかった）。74F377 は 1984年の Signetics、1985年の Fairchild の本にある。日立の品と国内の入手性は [V]。使えなければ、P2A が q2 の縁から見るようにする（DOC の窓を取る時刻が 16 MHz の 1 周期遅れ、port 2 の速さが E の位相で変わる）[O]。
- START の検査は 2 スロットにまたがる経路として数えた。判断バスは、再設定を待つチャネルを DBCH に選んだ次のスロットから再設定の間 SEQ が DBCH を保つので変わらない。U32 の選択（USEL ＝ INF ∨ CRLD∧CK0）は、前の LCLK に登録した ELS の INF と CHS の段階から決まる。START の要求が、すでに DBCH のチャネルに来たとき（そのスロットで再設定の段 0 を始める）は、CRLD が要求の FF から来る（OI-088）。
- B ファイルを出さないスロット（B ＝ 0xFFFF：COUNT の更新と再設定）は、74LS670 の出力の禁止（35）の後、B バスが引上げで High になる。15＋35＋立上り＋52＋10 で、立上りが 13 ns 以下なら入る。引上げの抵抗と B バスの負荷で決まる（680 Ω・20 pF なら約 6 ns）[V]。
- A ファイルの書込みの保持は、TIM・AFC0・74S157 の最小の遅れ − 74S32 の最大の遅れ（7 ns）で、資料に最小の遅れがない [V]。データ（SL、U12–U13）は /AWE の終わりの 100 ns 以上前に落ち着く（tDSW 25）。
- 余裕が 10 ns 以下の経路（STS → 頭の判断、桁上げ → BLK、和のラッチ、B ファイル、延びたアクセス → port 2）は、配線の遅れとクロックの分配のずれで足りなくなりうる [V]。
- CPU の側の要求（STRQ・STPQ・W1C の FF、HDEC の AREQ）は、CPU のストローブで変わり、SCLK と同期化しないまま CST・ELG に入る。ストローブと SCLK の位相は、ウェイトの数で半スロットずつずれる [V]（OI-088）。

非同期の入力（1 段で Fabric のクロックに取り込むもの）の余裕。余裕は、取り込む縁から次のレジスタまでの時間 − 取り込む FF の CLK→Q − 間の組合せ − 次のレジスタの tSU で、メタステーブルが収まるのに使える時間 [P]（DR-088、DR-090、DR-091）。

| 入力 | 取り込むところ | 次のレジスタ | 余裕 | |
|---|---|---|---:|---|
| IR3、IR5、OR4、OR10（FIFO の向こうは DSP・ADPCM・音声入力のクロック） | RDY（MCLK、PAL20R4A） | READY → ELG → SEQ（SCLK） | 125−15−25−25−25＝35 | |
| RDY2（DOC 子基板がなければ本体 DAC の FIFO の IR） | RDY（MCLK） | 同じ | 35 | |
| XRDY（拡張。向こうのクロックはモジュールが決める） | RDY（MCLK） | 同じ | 35 | |
| トリガのレベル | U36（74LS74、LCLK） | STS の SYN2（次の LCLK） | 250−40−25＝185 | |
| DOC の E | SYN1（74ALS74、16 MHz。DR-088） | P2A の SYN2 と 3 つの預かりの段、HACK の BW・BC・BDT（次の縁、tSU 25） | 62.5−18−25＝19.5 | **30 未満** |
| ACRTC の DTACK・DONE | ブリッジの 74LS175（2 段目、16 MHz） | P1A（次の縁、tSU 25） | 62.5−25−25＝12.5 | **30 未満** |
| CPU の /UDS・/LDS・R/W・番地（16 MHz と同期。位相はウェイトでずれる） | HDEC0 → HACK（16 MHz） | AK | 位相による | **OI-088** |
| CPU の CTRL・STATUS の書込み | 要求の FF（74LS74・74LS175） | CST・CHS・STS | 位相による | **OI-088** |

- 74ALS74 の CLK→Q は 18 ns（最大）と仮定した [V]。DOC の E は、外部クロックのジャンパで 16 MHz と無関係になるときだけ非同期になる（DR-088）。
- DR-091 で P1A・P2A・HACK を A 品にし、ACRTC のブリッジの 2 段目から P1A までは 2.5 ns から 12.5 ns に、DOC の E の SYN1 から P2A・HACK までは 9.5 ns から 19.5 ns になった [P]。エミュレータの同期化の模型（DR-088）の PAL の判定点と遅れは、A 品の tSU（最大 25 ns）の範囲に入るので変えていない（OI-086）。
- トリガのレベルの U36 は、LCLK（q2 の縁）で取り込み、STS が次の LCLK で 2 段目を取る。LCLK が q3 から q2 に移っても、間は 1 スロットのまま。

## 04　変換部

読んだ値（IL）を、回転 → マスク → MASK → XOR の順に通す。回転器は 2 段の PAL で、空いたビットを PROM のマスクで消してゼロ詰めのシフトにする（DR-046 (5)）。

- 回す量 T（4 bit）は SHIFT の下位 4 bit。FORMAT 4（上位バイトを下位へ移してからシフト）は、8 だけ余分に回すことと同じなので、T3 ＝ S3 ⊕ F4 とする（74LS86。F4 は ATR の出力を OP に取り込んだもの）。
- 右へのシフト（SHIFT が負）は、16 から引いた量の左回転と同じ。SHIFT の下位 4 bit をそのまま T にすればよい。
- MSK0・MSK1（63S281A×2、番地は SHIFT 5 bit と FORMAT 3 bit）が、出力の位置で有効なビットを出す。FORMAT 1 は SHIFT ≥ 0 で j ≥ s、負で j ≤ 15+s。FORMAT 0・2〜4 は s ≤ j ≤ s+7。FORMAT 5 はすべて。8 bit の出力（FORMAT 0・3・4）は上位 8 bit を 0 にする。FORMAT 6・7 は 0（START の検査で ERROR になるので使わない）。
- ANX は W のスロットだけ FD に出す（WEN。STB が出す）。8 bit のポートは FD の下位バイトを使う。上位バイトには XOR の上位 8 bit が出るが、ポートは使わない。

変換の全体を、下の式そのもの（ROT0–3、ANX0–3）と MSK の中身を評価して、仕様の定義（hardware.md §06、DR-046）と 61,056 通りの値で突き合わせた（FORMAT 0〜5 × SHIFT −16〜15 × 入力 318 通り、MASK・XOR は乱数）[P]。エミュレータは、この式から生成した関数で変換する（DR-084）。生成した経路は、それまでの変換の式と、FORMAT 0〜7 × SHIFT 32 通り × 4,096 通りの値で同じ値を出し、ROT は 16 bit の全部の値と回す量 16 通りのすべてで正しく回る（emulator.md §04）[P]。

```
; ROT0 (PAL20L8A)  入力 D0–D7、D13–D15、T0、T1。細かい段：0〜3 だけ左へ回す
PIN D0 D1 D2 D3 D4 D5 D6 D7 D13 D14 D15 GND T0 T1 F0 F1 F2 F3 F4 F5 F6 F7 NC VCC
/F0 = /T1*/T0*/D0 + /T1*T0*/D15 + T1*/T0*/D14 + T1*T0*/D13
/F1 = /T1*/T0*/D1 + /T1*T0*/D0 + T1*/T0*/D15 + T1*T0*/D14
/F2 = /T1*/T0*/D2 + /T1*T0*/D1 + T1*/T0*/D0 + T1*T0*/D15
/F3 = /T1*/T0*/D3 + /T1*T0*/D2 + T1*/T0*/D1 + T1*T0*/D0
/F4 = /T1*/T0*/D4 + /T1*T0*/D3 + T1*/T0*/D2 + T1*T0*/D1
/F5 = /T1*/T0*/D5 + /T1*T0*/D4 + T1*/T0*/D3 + T1*T0*/D2
/F6 = /T1*/T0*/D6 + /T1*T0*/D5 + T1*/T0*/D4 + T1*T0*/D3
/F7 = /T1*/T0*/D7 + /T1*T0*/D6 + T1*/T0*/D5 + T1*T0*/D4

; ROT1 (PAL20L8A)  入力 D5–D15、T0、T1
PIN D5 D6 D7 D8 D9 D10 D11 D12 D13 D14 D15 GND T0 T1 F8 F9 F10 F11 F12 F13 F14 F15 NC VCC
/F8 = /T1*/T0*/D8 + /T1*T0*/D7 + T1*/T0*/D6 + T1*T0*/D5
/F9 = /T1*/T0*/D9 + /T1*T0*/D8 + T1*/T0*/D7 + T1*T0*/D6
/F10 = /T1*/T0*/D10 + /T1*T0*/D9 + T1*/T0*/D8 + T1*T0*/D7
/F11 = /T1*/T0*/D11 + /T1*T0*/D10 + T1*/T0*/D9 + T1*T0*/D8
/F12 = /T1*/T0*/D12 + /T1*T0*/D11 + T1*/T0*/D10 + T1*T0*/D9
/F13 = /T1*/T0*/D13 + /T1*T0*/D12 + T1*/T0*/D11 + T1*T0*/D10
/F14 = /T1*/T0*/D14 + /T1*T0*/D13 + T1*/T0*/D12 + T1*T0*/D11
/F15 = /T1*/T0*/D15 + /T1*T0*/D14 + T1*/T0*/D13 + T1*T0*/D12

; ROT2 (PAL20L8A)  入力 F0、F1、F4、F5、F8、F9、F12、F13、T2、T3。粗い段：4 の倍数だけ回す
PIN F0 F1 F4 F5 F8 F9 F12 F13 T2 T3 NC GND NC NC R0 R1 R4 R5 R8 R9 R12 R13 NC VCC
/R0 = /T3*/T2*/F0 + /T3*T2*/F12 + T3*/T2*/F8 + T3*T2*/F4
/R1 = /T3*/T2*/F1 + /T3*T2*/F13 + T3*/T2*/F9 + T3*T2*/F5
/R4 = /T3*/T2*/F4 + /T3*T2*/F0 + T3*/T2*/F12 + T3*T2*/F8
/R5 = /T3*/T2*/F5 + /T3*T2*/F1 + T3*/T2*/F13 + T3*T2*/F9
/R8 = /T3*/T2*/F8 + /T3*T2*/F4 + T3*/T2*/F0 + T3*T2*/F12
/R9 = /T3*/T2*/F9 + /T3*T2*/F5 + T3*/T2*/F1 + T3*T2*/F13
/R12 = /T3*/T2*/F12 + /T3*T2*/F8 + T3*/T2*/F4 + T3*T2*/F0
/R13 = /T3*/T2*/F13 + /T3*T2*/F9 + T3*/T2*/F5 + T3*T2*/F1

; ROT3 (PAL20L8A)  入力 F2、F3、F6、F7、F10、F11、F14、F15、T2、T3
PIN F2 F3 F6 F7 F10 F11 F14 F15 T2 T3 NC GND NC NC R2 R3 R6 R7 R10 R11 R14 R15 NC VCC
/R2 = /T3*/T2*/F2 + /T3*T2*/F14 + T3*/T2*/F10 + T3*T2*/F6
/R3 = /T3*/T2*/F3 + /T3*T2*/F15 + T3*/T2*/F11 + T3*T2*/F7
/R6 = /T3*/T2*/F6 + /T3*T2*/F2 + T3*/T2*/F14 + T3*T2*/F10
/R7 = /T3*/T2*/F7 + /T3*T2*/F3 + T3*/T2*/F15 + T3*T2*/F11
/R10 = /T3*/T2*/F10 + /T3*T2*/F6 + T3*/T2*/F2 + T3*T2*/F14
/R11 = /T3*/T2*/F11 + /T3*T2*/F7 + T3*/T2*/F3 + T3*T2*/F15
/R14 = /T3*/T2*/F14 + /T3*T2*/F10 + T3*/T2*/F6 + T3*T2*/F2
/R15 = /T3*/T2*/F15 + /T3*T2*/F11 + T3*/T2*/F7 + T3*T2*/F3

; ANX0 (PAL20L8A)  入力 R・M（MSK）・K（MASK）・X（XOR）の 4 bit 分と WEN。Q = (R*M*K) ⊕ X
PIN R0 R1 R2 R3 M0 M1 M2 M3 K0 K1 K2 GND K3 X0 Q0 Q1 X1 X2 X3 WEN Q2 Q3 NC VCC
IF (WEN) /Q0 = R0*M0*K0*X0 + /R0*/X0 + /M0*/X0 + /K0*/X0
IF (WEN) /Q1 = R1*M1*K1*X1 + /R1*/X1 + /M1*/X1 + /K1*/X1
IF (WEN) /Q2 = R2*M2*K2*X2 + /R2*/X2 + /M2*/X2 + /K2*/X2
IF (WEN) /Q3 = R3*M3*K3*X3 + /R3*/X3 + /M3*/X3 + /K3*/X3

; ANX1 (PAL20L8A)  bit 4–7
PIN R4 R5 R6 R7 M4 M5 M6 M7 K4 K5 K6 GND K7 X4 Q4 Q5 X5 X6 X7 WEN Q6 Q7 NC VCC
IF (WEN) /Q4 = R4*M4*K4*X4 + /R4*/X4 + /M4*/X4 + /K4*/X4
IF (WEN) /Q5 = R5*M5*K5*X5 + /R5*/X5 + /M5*/X5 + /K5*/X5
IF (WEN) /Q6 = R6*M6*K6*X6 + /R6*/X6 + /M6*/X6 + /K6*/X6
IF (WEN) /Q7 = R7*M7*K7*X7 + /R7*/X7 + /M7*/X7 + /K7*/X7

; ANX2 (PAL20L8A)  bit 8–11
PIN R8 R9 R10 R11 M8 M9 M10 M11 K8 K9 K10 GND K11 X8 Q8 Q9 X9 X10 X11 WEN Q10 Q11 NC VCC
IF (WEN) /Q8 = R8*M8*K8*X8 + /R8*/X8 + /M8*/X8 + /K8*/X8
IF (WEN) /Q9 = R9*M9*K9*X9 + /R9*/X9 + /M9*/X9 + /K9*/X9
IF (WEN) /Q10 = R10*M10*K10*X10 + /R10*/X10 + /M10*/X10 + /K10*/X10
IF (WEN) /Q11 = R11*M11*K11*X11 + /R11*/X11 + /M11*/X11 + /K11*/X11

; ANX3 (PAL20L8A)  bit 12–15
PIN R12 R13 R14 R15 M12 M13 M14 M15 K12 K13 K14 GND K15 X12 Q12 Q13 X13 X14 X15 WEN Q14 Q15 NC VCC
IF (WEN) /Q12 = R12*M12*K12*X12 + /R12*/X12 + /M12*/X12 + /K12*/X12
IF (WEN) /Q13 = R13*M13*K13*X13 + /R13*/X13 + /M13*/X13 + /K13*/X13
IF (WEN) /Q14 = R14*M14*K14*X14 + /R14*/X14 + /M14*/X14 + /K14*/X14
IF (WEN) /Q15 = R15*M15*K15*X15 + /R15*/X15 + /M15*/X15 + /K15*/X15
```

ROT0・ROT1 は入力 13 本・出力 8 本、ROT2・ROT3 は入力 10 本・出力 8 本、ANX は入力 17 本・出力 4 本（3 ステート）で、PAL20L8 の入力（出力 8 本で 14 本、出力 4 本で 18 本）と項の数（1 出力 7 項）に収まる。PIN の行のピンの割当て（出力は 15〜22、ANX の出力は帰還のない 15・22 と 16・21、入力に使う I/O は 17〜20）も、fabric_logic.py が型番のピンの働きと突き合わせて確かめる [P]（DR-084）。

## 05　番地と COUNT の経路

### A ファイル（74S189×4）

| 番地 | ch0 | 番地 | ch1 |
|---|---|---|---|
| 0 | SRC_ADDR の設定値 | 8 | SRC_ADDR の設定値 |
| 1 | SRC_ADDR の作業値 | 9 | SRC_ADDR の作業値 |
| 2 | DST_ADDR の設定値 | 10 | DST_ADDR の設定値 |
| 3 | DST_ADDR の作業値 | 11 | DST_ADDR の作業値 |
| 4 | COUNT の設定値 | 12 | COUNT の設定値 |
| 5 | COUNT の作業値 | 13 | COUNT の作業値 |
| 6–7 | 空き | 14–15 | 空き |

74S189 は書いた値を反転して出す。A バスに値そのものが出るように、反転した値を書く。加算器の和は反転のラッチ SL（74ALS534。DR-091 で 74LS534 から）を通し、CPU の書込みは反転の緩衝器 U12–U13（74LS240）を通す。B ファイル（74LS670×4）は 4 語（ch0・ch1 の SRC_STEP・DST_STEP）で、CPU は別の書込みポートから書く。読出しの番地はチャネルと読出し元・書込み先で、出さないときは B バスが引上げで 0xFFFF になる。

### A ファイルの操作

1 スロットに 1 回。AFC0・AFC1 が SEQ の操作の番号（AOP。06）を番地と信号に直す。

| 操作 | 読む語 | B | Cin | 書く語 | 桁上げ（CY） |
|---|---|---|---|---|---|
| 読出し元の歩進 | SRC の作業値 | SRC_STEP | 0 | SRC の作業値 | 使わない。A バスを PS に取り込む |
| 書込み先の歩進 | DST の作業値 | DST_STEP | 0 | DST の作業値 | 使わない。A バスを PD に取り込む |
| COUNT の更新 | COUNT の作業値 | 0xFFFF | 0 | COUNT の作業値（W−1） | 0 なら W＝0、つまり最後の要素 |
| 再設定（番地） | SRC か DST の設定値 | 0xFFFF | 1 | 作業値（P のまま） | 使わない |
| 再設定（COUNT） | COUNT の設定値 | 0xFFFF | 0 | 作業値（P−1） | 0 なら P＝0、転送せずに DONE |
| CPU の読出し | 作業値 | — | — | なし | A バスを U7–U8 に取り込む |
| CPU の書込み | — | — | — | 設定値（ID から） | — |

- ポートのアクセスに使う番地は、歩進の前の値（読み出した作業値）。PS・PD は LCLK（DR-091 で 3/4 から中ほどの q2 の縁へ）で取り込み、次のスロットのアクセスの間ずっと保つ。
- 作業値の COUNT は「残りの数−1」。最後の要素の更新で 0xFFFF になる（DR-046 (3)）。DR-091 で、桁上げ（CY）は加算器の C4 ではなく、A バスの 0 の検出（U65–U66 の 74S02 で 2 bit ずつ NOR、U67 の 74S30 で NAND）から取る：CY ＝ A ≠ 0。CY を使う COUNT の更新と再設定（COUNT）は B ＝ 0xFFFF・Cin ＝ 0 なので、加算器の桁上げと同じ。加算器の C4 を待たないので、LCLK を q2 の縁に早められる（03）[P]。
- 再設定は、START と LOOP の再開で SRC・DST・COUNT の順に 3 スロット（DR-046 (3)）。最初の 2 スロットで、ポートの組合せの検査も行う（06）。
- 番地を持たないポート（逐次のポート）では歩進をしないので、その A ファイルの空きで COUNT を更新する。読出し元が逐次なら R の判断のとき、読出し元が番地を持ち書込み先が逐次なら W の判断のとき、両方が番地を持てば W の後に 1 スロット足す。この置き方はスロットの列に表れるので、エミュレータも同じにする [P]。port 2 は、DOC 子基板がないとき（本体 DAC の FIFO）も番地を持つポートとして扱い、歩進する（番地は使わない）。
- 要素を始めたときに COUNT を更新したか（読出し元が逐次だったか）は、ELS の EPC に取り込む。W のときに COUNT を更新するかは、そのときの SRC ではなく EPC で決めるので、動作中に SRC を書き換えても、COUNT の更新は 1 要素に 1 回 [P]（DR-085）。

AFC0・AFC1 の式 [P]（DR-085、DR-091）。DR-091 で、組合せの PAL16L8A から、SCLK で SEQ と同じ入力を取り込むレジスタの PAL（AFC0 は PAL20R6A、AFC1 は PAL16R8A）にした。番地と信号は SCLK の 15 ns 後に出て、スロットの間変わらない。A ファイルの番地（AA3–AA0）は、負の論理のまま 74S189 の番地につなぐ（語の番号は番地のピンの反転）。語の bit 0 は、読み（AR0N）と書き（AW0N）を別に登録し、U64（74S157）が PH1 で選ぶ（q0〜q1 は読み、q2〜q3 は書き）。B ファイルの番地は BA1 がチャネル、BA0 が書込み先（CPU の書込みポートと同じ並び）。AOP の 8 通り × チャネル × CPU の窓の語と向き、読みと書きの相のすべてで、上の表の語、B、Cin、取込みを出すことを確かめた。

```
; AFC0 (PAL20R6A)  SCLK で A ファイルの番地を登録する（SEQ と同じ入力：ELG の AC2–AC0、CST の CK1・CK0、次の ACH ＝ DBCH）。語の bit 3–1（AA3N–AA1N、負の論理のまま 74S189 へ）、語の bit 0 の読みの側（AR0N）と書きの側（AW0N。74S157 の U64 が PH1 で、読み（q0〜q1）と書き（q2〜q3）を選んで AA0N にする）、書込みのある操作（AWN。TIM の AWQN へ）。CIN は加算器の桁上げの入力
PIN SCLK AC2N AC1N AC0N CK1N CK0N DBCHN HCH HA3 HA2 HRW GND GND NC NC AA3N AA2N AA1N AR0N AW0N AWN CIN NC VCC
/AA3N := /AC2N*/DBCHN + /AC1N*/DBCHN + AC2N*AC1N*HCH
/AA2N := /AC2N*/CK1N + /AC2N*AC0N + AC2N*AC1N*HA3*HA2
/AA1N := /AC1N*/AC0N + /AC2N*/AC0N*/CK0N + AC2N*AC1N*/HA2
/AR0N := AC0N + AC2N
/AW0N := /AC2N + /AC1N
/AWN := /AC2N + /AC1N + /AC0N*/HRW
/CIN = /AR0N + /AA2N

; AFC1 (PAL16R8A)  SCLK で B ファイルの読出し（BEN、番地 BA1・BA0）、A ファイルへのデータ（SLOE：和のラッチ、HWOE：CPU の書込み）、PS・PD・U7–U8 の取込み（PSC、PDC、HRC）を登録する。入力は AFC0 と同じ
PIN SCLK AC2N AC1N AC0N DBCHN HRW NC NC NC GND GND BENN BA1 BA0 SLOEN HWOEN PSCN PDCN HRCN VCC
/BENN := AC2N*/AC1N
/BA1 := DBCHN
/BA0 := AC0N
/SLOEN := /AC2N + /AC1N
/HWOEN := AC2N*AC1N*/AC0N*/HRW
/PSCN := AC2N*/AC1N*AC0N
/PDCN := AC2N*/AC1N*/AC0N
/HRCN := AC2N*AC1N*/AC0N*HRW
```

AFC の出力の働き（PSC・PDC・HRC の取込みの時点は LCLK、/AWE は q3 の後半）は TIM の位相と組み合わせる。AWN は TIM の AWQN（q3 の間 Low）になり、74S32（U68）で 16 MHz と OR して /AWE にする。SLOE・HWOE は SCLK からスロットの間出す。A ファイルの /CS は常に選ぶ [P]（DR-091）。

## 06　順序と状態

### 判断バスと候補

SEQ が、スロットごとに判断バスのチャネル（DBCH）を選ぶ。要素が動いている間はそのチャネル、再設定を待つチャネルがあればそれ、なければ動いている 2 チャネルを交互に選ぶ（DR-046 (2)）。ただし、候補の読出し元が port 0 で今が S のスロットなら、HOLD で入れ替えない。入れ替えが S・X と同じ周期なので、これがないと port 0 から読むチャネルの候補が S の頭にだけ来て止まる（[OI-052](../open-issues.md)、[DR-048](../decisions.md)）。DBCH のチャネルの SRC・DST・FORMAT（74LS173 の 3 ステート出力）が判断バスに出る。

- RDY が SRC・DST の準備を、スロットの中ほどで取り込む（下の式）。
- U32（74LS157）が、次のアクセスのポートを選ぶ：要素が動いていなければ SRC（R）、動いていれば DST（W）。再設定の 2 段目（START の検査で書込み先を調べる）も DST。選択は USEL ＝ INF ∨ CRLD∧CK0（下の CST・ELS）[P]（DR-085）。
- ATR（63S281A）が、次のポートの属性を出す。番地は ポートの番号 4 bit、向き 1 bit、FORMAT 3 bit。
- U33（74LS151）が、port 8〜15 のモジュールの /PRES を選ぶ（port 9 は常にある。port 0〜7 では使わない側に固定し、「ある」を出す）。
- NX（74LS374×2）が、判断のスロットの頭で、次のアクセス（ポート、向き、チャネル、FORMAT、W16、LG、FLAG、TWO、有効）と、STALL の評価（STV、STL）を取り込む（DR-089 で F4 を LG に替えた）。OP（74LS377×2）は、空いていれば次のスロットの頭で NX を取り込む。STB と変換部は OP を見る [P]（DR-085）。

### ポートの属性（ATR の中身）

| 出力 | 意味 | 1 になるポート |
|---|---|---|
| SONLY | S のスロットでだけアクセスする | 0 |
| ADDR | 番地を持つ（歩進する） | 0、2、8、9 |
| FLAG | 満杯・空の信号を持つ（アクセスの後 2 回の判断で新しい要素を始めない） | 1〜5、10〜15 |
| OKD | DOC 子基板があるとき、この向きと FORMAT の幅で使える | 下の表 |
| OKN | DOC 子基板がないとき、使える（port 2 だけが OKD と違う） | 下の表 |
| W16 | この側の幅が 16 bit（読出し元は FORMAT の入力の幅、書込み先は出力の幅）。port 0 のバイトのアクセスに使う | FORMAT 1・3・4・5（読出し元）、1・2・5（書込み先） |
| F4 | FORMAT 4 | — |
| TWO | 2 スロットの読出し | 9 |

| ポート | 読出し | 書込み | 幅 |
|---|---|---|---|
| 0 | ○ | ○ | 8・16 |
| 1 | ○ | ○ | 16 |
| 2（DOC 子基板あり） | ○ | ○ | 8 |
| 2（なし：本体 DAC の FIFO） | — | ○ | 16 |
| 3 | — | ○ | 16 |
| 4 | ○ | — | 16 |
| 5 | — | ○ | 8 |
| 6 | — | ○ | 8 |
| 7 | ○ | — | 8 |
| 8 | ○ | — | 8 |
| 9 | ○ | — | 8 |
| 10 | ○ | — | 8 |
| 11〜15（拡張） | ○ | ○ | 8・16（モジュールが決める）[P] |

FORMAT 6・7 は、どのポートでも OKD・OKN が 0。

### 準備の信号（RDY、PAL20R4A）

準備の信号を持つのは port 1（ACRTC の DREQ。ブリッジで同期化したもの）、port 2（P2A の RDY2：DOC 子基板があれば常に 1 で、読出しと、P2A の預かりが空いていないときの書込みは、アクセスそのものが延びる（DR-050）。なければ DAC の FIFO の IR）、port 3・5（FIFO の IR）、port 4・10（FIFO の OR）、port 11〜15（拡張の XRDY、1 本で共通 [P]）。ほかのポートは常に準備ができている。使えない向きのポート（例：読出し元の port 3）は START の検査で ERROR になるので、準備は見ない。

```
; RDY (PAL20R4A)  MCLK で取り込む。入力 S3–S0、D3–D0（判断バス）、RDY1、RDY2、OR4、OR10、IR3、IR5、XRDY。READY は両方の準備（今の値）
PIN MCLK S0 S1 S2 S3 D0 D1 D2 D3 RDY1 RDY2 GND GND OR4 OR10 IR3 SRDY DRDY NC NC IR5 READY XRDY VCC
/SRDY := /S3*/S2*/S1*S0*/RDY1 + /S3*/S2*S1*/S0*/RDY2 + /S3*S2*/S1*/S0*/OR4 + S3*/S2*S1*/S0*/OR10 + S3*/S2*S1*S0*/XRDY + S3*S2*/XRDY
/DRDY := /D3*/D2*/D1*D0*/RDY1 + /D3*/D2*D1*/D0*/RDY2 + /D3*/D2*D1*D0*/IR3 + /D3*D2*/D1*D0*/IR5 + D3*/D2*D1*D0*/XRDY + D3*D2*/XRDY
/READY = /SRDY + /DRDY
```

ポートの番号の 16 通りと、準備の信号の 128 通りのすべてで、仕様と一致することを確かめた [P]。RDY の入力の 15 本のうち 3 本（OR10、IR3、IR5）は、組合せの I/O のピン（15、16、21）を入力に使う。READY（ピン 22）は SRDY∧DRDY で、ELG の入力の数を減らす。XRDY はピン 23 へ移した [P]（DR-085）。エミュレータは、この式から生成した関数で準備を取り込み、ATR の中身もそのまま引く（DR-084）[P]。

### 順序と状態の PAL

判断と状態の PAL は 8 種 10 個 [P]（DR-085）。式は下のブロック（[tools/fabric/pal/seq.pal](../../tools/fabric/pal/seq.pal)）。出力はすべて負の論理で、名前の末尾を N にする（`/XN = 積和` のピン XN は、積和が 1 のとき 0）。CHS・STS は 2 個ずつで、同じ式を焼く。

| PAL | 型番 | クロック | 働き |
|---|---|---|---|
| CST | PAL20L8A | — | 判断バスのチャネル（DBCH）と、もう一方のチャネルの状態：動作中（CRUN）、新しい要素を始められる動作中（CRA）、再設定を待つ（CRLD。START の要求を含む）、トリガが許す（CARM）、再設定の段（CK1・CK0）、もう一方の再設定（ORLD）・動作中（ORUN）。STOP の要求のあるチャネルは止まっているとみなす |
| ELG | PAL20L8A | — | 頭の判断：A ファイルの操作の種類（AC2–AC0。再設定は 101 で、段は SEQ が付ける）、新しい要素（GR）、書込み（GW）、要素が残るか（KEEP）、STALL の評価（STV、STL）。ポートの属性は DPA から（DR-089） |
| SEQ | PAL20R8A | SCLK | 判断の登録：DBCH、判断のチャネル（ACH）、AOP（3）、決めた読出し（ACCR）・書込み（ACCW） |
| ELS | PAL20R8A | LCLK | 要素の状態：動いている要素（INF）、段階（Q1・Q0）、COUNT の更新を W に回す（EPC）、最後（ELAST）、書込みの残り（TL、そのチャネル TCH、最後 TLAST） |
| EVT | PAL20L8A | — | このスロットの出来事：止める（KIL0・1：STOP か ERROR）、ERROR（ER0・1：時間切れか port 1 の DONE の食い違い）、ブロックの終わり（BLK0・1）、要素と書込みの残りのチャネルが止まる（KE、KT） |
| ADC | PAL16L8A-2 | — | AOP と ACH をチャネルごとの信号に：再設定（RLD）、COUNT の更新（CNT）、EACH の要素の始め（STE）、STALL の評価（SV） |
| CHS0・1 | PAL20R6A | LCLK | チャネルの段階（P2–P0）、START の検査（OK）、DONE、ERROR。次の段階のうち、ブロックの終わり（BLK）を除いた部分（NBSYX、NRUN0）を STS へ（DR-091） |
| STS0・1 | PAL20R6A | LCLK | トリガの同期化の 2 段目（SYN2）、立上り（RISE）、許可（ARM）、保留（PEND）、MISS、STALL。次の段階が止まっていないか（NBSYX ∨ NRUN0∧¬BLK）と動作か（NRUN0∧¬BLK）は、CHS の出力と EVT の BLK から作る（DR-091）。ARMX（即時か ARM）と WAIT を出す |

| 段階（CHS） | P2 P1 P0 | | AOP（SEQ） | 操作 |
|---|---|---|---|---|
| 止まっている | 000 | | 000 | なし |
| 再設定の段 0（SRC） | 100 | | 001 | CPU |
| 再設定の段 1（DST） | 101 | | 010 | 読出し元の歩進 |
| 再設定の段 2（COUNT） | 110 | | 011 | 書込み先の歩進 |
| 動作 | 111 | | 100 | COUNT の更新 |
| 動作（COUNT の更新で最後と分かった。新しい要素を始めない） | 011 | | 101・110・111 | 再設定の段 0・1・2 |

- 要素（ELS）：INF の間、Q1＝0 は W を決めていない段階（Q0 は読出しが済んだ）、Q1＝1 は W を決め、W の後の COUNT の更新を待つ段階（Q0 は W が済んだ）。W と COUNT の更新を決めた要素は、書込みの残り（TL）に移る。TL は W が済むと消え、最後の要素なら BLK を出す。要素があるとき DBCH は要素のチャネルで、ACH も同じ。
- つなぎ（74LS 系）[P]：USEL ＝ INF ∨ CRLD∧CK0。HOLD ＝ S ∧ CRUN ∧（判断バスの SRC が 0）。OKS ＝（DOC があれば OKD、なければ OKN）∧ U33。IMM ＝ TRIGGER の発生源が 0。STATUS の BUSY ＝ P2 ∨ P1。CPU への許可は AOP＝001（HACK が見る）。
- DR-089 で、S のスロットの規則・番地の有無・FB は DPA が判断バスの SRC・DST から直接作る（下の「アクセスの制御」）。U32 → ATR を通らないので、頭の判断の経路が短くなる。ELG の入力の SOK・ADDR・FB は、読出し元の側の GOKS（S の規則と FB の待ち）・ADDRS と、書込み先の側の SOKD・ADDRD になった（新しい要素を始めてよいときは USEL＝0、要素があるときは USEL＝1 なので、どちらの側かは決まっている）。CD は OPA が出す。
- CPU の要求の FF [P]：CTRL の書込みは、STOP（ID1）なら STPQ、START（ID0）で段階が 000 なら STRQ に入る（チャネルごとに 74LS74）。STATUS の W1C は WDQ・WEQ・WMQ（チャネルごとに 74LS175）。CST・CHS・STS・EVT が次のスロットで使い、LCLK（DR-091 で q2 の縁）の後に消す。STOP の要求のあるチャネルのアクセスは、次の頭で OP と NX から捨て、延びたアクセスは打ち切る（OPA の ABT を P1A・P2A へ）。68000 のバスサイクルは 2 スロット以上なので、CPU の次のアクセスは、要求を使った LCLK の後になる。要求の FF が SCLK と同期していないことは OI-088 [V]。
- 確かめたこと：CHS の段階・OK・DONE・ERROR と STS（2^17 通り）は届きうる入力と状態の組の全部、SEQ の AOP は操作の種類と段の全部で、fabric_logic.py の仕様の定義と一致する。全体は、式から生成した関数をつないだ門の模型（`emulator/crates/board/tests/fabric_gate/mod.rs`）とエミュレータのスロットの模型（`fabric.rs`）を並べて動かし、スロットの記録、STATUS、段階、割込み、CPU への応答、ポートへの働きかけが、すべてのスロットで一致した（fabric.rs の試験と同じ転送の筋書き、DOC 子基板の延びるアクセス、擬似乱数の CPU の手順とポート 64 通り × 8,000 スロット）[P]。DR-089 で、アクセスの制御と CPU の窓も式にし、延びたアクセスの時間切れ（port 1、DOC の port 2）、port 1 の DONE の印の食い違い、延びたアクセスの途中の STOP、port 9 の 2 スロットのアクセス、転送中の PORT_RESET の筋書きを足した。fabric.rs の側は 1 か所を直した：NX が判断のときの FORMAT を持つので、アクセスの幅と変換の FORMAT を判断のときの値にした（OI-084 (1)）[P]。

```
; CST (PAL20L8A)  候補の状態。CHS の段階と、START・STOP の要求の FF から、判断バスのチャネル（DBCH）ともう一方の状態を出す。出力は負の論理
PIN P2N0 P1N0 P0N0 P2N1 P1N1 P0N1 STRQ0 STRQ1 STPQ0 STPQ1 DBCHN GND ARMXN0 ARMXN1 CRUNN CRAN CRLDN CARMN CK1N CK0N ORLDN ORUNN NC VCC
/CRUNN = DBCHN*/P1N0*/P0N0*/STPQ0 + /DBCHN*/P1N1*/P0N1*/STPQ1
/CRAN = DBCHN*/P2N0*/P1N0*/P0N0*/STPQ0 + /DBCHN*/P2N1*/P1N1*/P0N1*/STPQ1
/CRLDN = DBCHN*/P2N0*P1N0*/STPQ0 + DBCHN*/P2N0*P0N0*/STPQ0 + DBCHN*STRQ0*/STPQ0
+ /DBCHN*/P2N1*P1N1*/STPQ1 + /DBCHN*/P2N1*P0N1*/STPQ1 + /DBCHN*STRQ1*/STPQ1
/CARMN = DBCHN*/ARMXN0 + /DBCHN*/ARMXN1
/CK1N = DBCHN*/P1N0 + /DBCHN*/P1N1
/CK0N = DBCHN*/P0N0 + /DBCHN*/P0N1
/ORLDN = /DBCHN*/P2N0*P1N0*/STPQ0 + /DBCHN*/P2N0*P0N0*/STPQ0 + /DBCHN*STRQ0*/STPQ0
+ DBCHN*/P2N1*P1N1*/STPQ1 + DBCHN*/P2N1*P0N1*/STPQ1 + DBCHN*STRQ1*/STPQ1
/ORUNN = /DBCHN*/P1N0*/P0N0*/STPQ0 + DBCHN*/P1N1*/P0N1*/STPQ1

; ELG (PAL20L8A)  このスロットの頭の判断：A ファイルの操作の種類（AC2–AC0）、新しい要素（GR）、書込み（GW）、要素が残るか（KEEP）、STALL の評価（STV、STL）。出力は負の論理
PIN INFN Q1N EPCN CRUNN CRAN CRLDN CARMN READY GOKS ADDRS SOKD GND CD HREQ GRN AC2N AC1N AC0N GWN KEEPN STVN STLN ADDRD VCC
/GRN = INFN*CD*/CRAN*/CARMN*READY*GOKS*/HREQ
/AC2N = INFN*CD*/CRAN*/CARMN*READY*GOKS*/HREQ*/ADDRS
+ /INFN*/CRUNN*Q1N*CD*SOKD*/ADDRD*/EPCN*/HREQ
+ /INFN*/CRUNN*/Q1N*/HREQ
+ /CRLDN*CD*/HREQ
/AC1N = INFN*CD*/CRAN*/CARMN*READY*GOKS*/HREQ*ADDRS
+ /INFN*/CRUNN*Q1N*CD*SOKD*ADDRD*/HREQ
/AC0N = HREQ + /INFN*/CRUNN*Q1N*CD*SOKD*ADDRD + CD*/CRLDN
/GWN = /INFN*/CRUNN*Q1N*CD*SOKD*/ADDRD*EPCN + /INFN*/CRUNN*Q1N*CD*SOKD*/HREQ
/KEEPN = INFN*CD*/CRAN*/CARMN*READY*GOKS*/HREQ
+ /INFN*/CRUNN*/Q1N*HREQ
+ /INFN*/CRUNN*Q1N*/CD
+ /INFN*/CRUNN*Q1N*/SOKD
+ /INFN*/CRUNN*Q1N*ADDRD*HREQ
+ /INFN*/CRUNN*Q1N*/EPCN*HREQ
+ /INFN*/CRUNN*Q1N*/EPCN*ADDRD*/HREQ
/STVN = INFN*CD*/CRAN
/STLN = /CARMN*/READY

; SEQ (PAL20R8A)  SCLK で判断を登録する：次の候補（DBCH）、判断のチャネル（ACH）、A ファイルの操作（AOP2–AOP0）、決めたアクセス（ACCR、ACCW）。出力は負の論理
PIN SCLK AC2N AC1N AC0N GRN GWN KEEPN CK1N CK0N CRLDN ORLDN GND GND ORUNN DBCHN ACHN AOP2N AOP1N AOP0N ACCRN ACCWN NC HOLD VCC
/DBCHN := /DBCHN*/KEEPN + /DBCHN*/CRLDN + /DBCHN*ORLDN*HOLD + /DBCHN*ORLDN*ORUNN
+ DBCHN*KEEPN*CRLDN*/ORLDN + DBCHN*KEEPN*CRLDN*/HOLD*/ORUNN
/ACHN := /DBCHN
/AOP2N := /AC2N
/AOP1N := /AC1N + /AC2N*AC1N*/AC0N*/CK0N + /AC2N*AC1N*/AC0N*/CK1N
/AOP0N := /AC0N*AC2N + /AC0N*/AC1N + /AC0N*CK0N
/ACCRN := /GRN
/ACCWN := /GWN

; ELS (PAL20R8A)  LCLK で要素の状態を登録する：動いている要素（INF、段階 Q1・Q0、COUNT の更新の待ち EPC、最後 ELAST）と、書込みの残り（TL、TCH、TLAST）。出力は負の論理
PIN LCLK ACCRN ACCWN AOP2N AOP1N CY DNR DNW ACHN KEN KTN GND GND NC INFN Q1N Q0N EPCN ELASTN TLN TCHN TLASTN NC VCC
/INFN := KEN*/ACCRN + KEN*/INFN*ACCWN*Q1N + KEN*/INFN*ACCWN*AOP2N + KEN*/ACCWN*/EPCN*/AOP1N
/Q1N := /ACCWN*/EPCN*/AOP1N + /INFN*/Q1N*AOP2N
/Q0N := /INFN*Q1N*ACCWN*/Q0N + /INFN*Q1N*ACCWN*DNR + /INFN*/Q1N*AOP2N*/Q0N + /INFN*/Q1N*AOP2N*DNW
/EPCN := /ACCRN*/AOP1N + ACCRN*/EPCN
/ELASTN := /ACCRN*AOP1N*/CY + ACCRN*/ELASTN
/TLN := /TLN*/DNW*KTN + KEN*/ACCWN*EPCN + KEN*/ACCWN*AOP1N + KEN*/INFN*/Q1N*/AOP2N*Q0N*/DNW
/TCHN := /ACCWN*/ACHN + /INFN*/Q1N*/AOP2N*/ACHN + ACCWN*INFN*/TCHN + ACCWN*Q1N*/TCHN + ACCWN*AOP2N*/TCHN
/TLASTN := /ACCWN*/EPCN*AOP1N*/CY + /ACCWN*EPCN*/ELASTN + /INFN*/Q1N*/AOP2N*/CY
+ ACCWN*INFN*/TLASTN + ACCWN*Q1N*/TLASTN + ACCWN*AOP2N*/TLASTN

; EVT (PAL20L8A)  このスロットのチャネルの出来事：止める（KIL）、ERROR で止める（ER）、ブロックの終わり（BLK）、要素と書込みの残りのチャネルが止まる（KE、KT）。出力は負の論理
PIN STPQ0 STPQ1 ERRKN OPCH TLN TCHN TLASTN DNW INFN Q1N Q0N GND AOP2N CY ERN0 KILN0 KILN1 KEN KTN BLKN0 BLKN1 ERN1 ACHN VCC
/ERN0 = /ERRKN*/OPCH
/KILN0 = STPQ0 + /ERRKN*/OPCH
/KILN1 = STPQ1 + /ERRKN*OPCH
/KEN = ACHN*/KILN0 + /ACHN*/KILN1
/KTN = TCHN*/KILN0 + /TCHN*/KILN1
/BLKN0 = KILN0*/TLN*DNW*/TLASTN*TCHN + KILN0*/INFN*/Q1N*/AOP2N*/CY*/Q0N*ACHN
+ KILN0*/INFN*/Q1N*/AOP2N*/CY*DNW*ACHN
/BLKN1 = KILN1*/TLN*DNW*/TLASTN*/TCHN + KILN1*/INFN*/Q1N*/AOP2N*/CY*/Q0N*/ACHN
+ KILN1*/INFN*/Q1N*/AOP2N*/CY*DNW*/ACHN
/ERN1 = /ERRKN*OPCH

; ADC (PAL16L8A-2)  AOP と ACH をチャネルごとの信号に直す：再設定（RLD）、COUNT の更新（CNT）、EACH の要素の始め（STE）、STALL の評価（SV）。出力は負の論理
PIN AOP2N AOP1N AOP0N ACHN ACCRN STVN EACH0 EACH1 IMM0 GND IMM1 RLDN0 RLDN1 CNTN0 CNTN1 STEN0 STEN1 SVN0 SVN1 VCC
/RLDN0 = /AOP2N*/AOP1N*ACHN + /AOP2N*/AOP0N*ACHN
/RLDN1 = /AOP2N*/AOP1N*/ACHN + /AOP2N*/AOP0N*/ACHN
/CNTN0 = /AOP2N*AOP1N*AOP0N*ACHN
/CNTN1 = /AOP2N*AOP1N*AOP0N*/ACHN
/STEN0 = /ACCRN*ACHN*EACH0*/IMM0
/STEN1 = /ACCRN*/ACHN*EACH1*/IMM1
/SVN0 = /STVN*ACHN
/SVN1 = /STVN*/ACHN

; CHS (PAL20R6A)  LCLK でチャネルの段階（P2–P0：止まっている 000、再設定 100・101・110、動作 111、最後と分かった動作 011）、START の検査（OK）、DONE、ERROR を登録する。CHS0・CHS1 は同じ式。NBSYX・NRUN0 は次の段階の、ブロックの終わり（BLK）を除いた部分：次が止まっていない ＝ NBSYX ∨ NRUN0∧¬BLK、次が動作 ＝ NRUN0∧¬BLK（STS で合わせる）
PIN LCLK RLDN CNTN CY OKS BLKN KILN ERN STRQ LOOP WDQ GND GND WEQ NBSYXN P2N P1N P0N OKN DONEN ERRN NRUN0N NC VCC
/P2N := KILN*/P2N*P1N + KILN*STRQ
+ KILN*/RLDN*/P2N*/P1N*P0N*/OKN*CY + KILN*/RLDN*/P2N*/P1N*P0N*/OKN*LOOP
+ KILN*/BLKN*LOOP + KILN*RLDN*/P2N*P0N
+ KILN*RLDN*BLKN*/P2N*CNTN + KILN*RLDN*BLKN*/P2N*CY
/P1N := KILN*/RLDN*/P2N*P1N*/P0N + KILN*/RLDN*/P2N*/P1N*P0N*/OKN*CY + KILN*RLDN*BLKN*/P1N
/P0N := KILN*/RLDN*/P2N*P1N*P0N + KILN*/RLDN*STRQ + KILN*/RLDN*/P2N*/P1N*P0N*/OKN*CY
+ KILN*RLDN*BLKN*/P0N
/OKN := /RLDN*/P2N*P1N*P0N*OKS + /RLDN*STRQ*OKS + /RLDN*/P2N*P1N*/P0N*/OKN*OKS
+ RLDN*/OKN + /P1N*/OKN
/DONEN := /DONEN*/WDQ + /RLDN*/P2N*/P1N*P0N*/OKN*/CY + /BLKN
/ERRN := /ERRN*/WEQ + /RLDN*/P2N*/P1N*P0N*OKN + /ERN
/NBSYXN = KILN*/P2N*P1N + KILN*STRQ + KILN*/RLDN*/P2N*/P1N*P0N*/OKN*LOOP + KILN*RLDN*/P2N*P0N
+ KILN*RLDN*/P1N*/P0N*LOOP
/NRUN0N = KILN*/RLDN*/P2N*/P1N*P0N*/OKN*CY + KILN*RLDN*/P1N*/P0N

; STS (PAL20R6A)  LCLK でトリガの同期化の 2 段目（SYN2）と立上り（RISE）、許可（ARM）、保留（PEND）、MISS、STALL を登録する。STS0・STS1 は同じ式。次が動作（NRUN0∧¬BLK）と次が止まっていない（NBSYX ∨ NRUN0∧¬BLK）は CHS の NBSYX・NRUN0 と EVT の BLK から（BLK から CHS を通さない）。ARMX・WAIT は今の値
PIN LCLK S1Q NBSYXN NRUN0N STRQ STEN WMQ IMM SVN STLN P1N GND GND P0N ARMXN SYN2N RISEN ARMN PENDN MISSN STALN WAITN BLKN VCC
/SYN2N := S1Q
/RISEN := S1Q*SYN2N
/ARMN := /NRUN0N*BLKN*/ARMN*STEN + /NRUN0N*BLKN*/PENDN + /NRUN0N*BLKN*/RISEN
/PENDN := /NRUN0N*BLKN*/PENDN*/ARMN*STEN + /NRUN0N*BLKN*/RISEN*/ARMN*STEN + NRUN0N*/PENDN*/STRQ
+ /BLKN*/PENDN*/STRQ + NRUN0N*/RISEN*/NBSYXN + /BLKN*/RISEN*/NBSYXN
/MISSN := /MISSN*/WMQ + /RISEN*/NBSYXN*/PENDN*/STRQ + /RISEN*/NRUN0N*BLKN*/PENDN*/STRQ
/STALN := /NRUN0N*BLKN*/SVN*/STLN + /NRUN0N*BLKN*SVN*/STALN
/ARMXN = IMM + /ARMN
/WAITN = /P1N*/P0N*/IMM*ARMN
```

| PAL | 入力 | 出力 | 積（最大） | 空きピン |
|---|---:|---:|---:|---:|
| AFC0・AFC1 | 10・5 | 7・8 | 3／8・2／8 | 3・3 |
| CST | 13 | 8 | 6／7 | 1 |
| ELG | 14 | 8 | 7／7 | 0 |
| SEQ | 12 | 7 | 6／8 | 1 |
| ELS | 10 | 8 | 6／8 | 2 |
| EVT | 14 | 8 | 3／7 | 0 |
| ADC | 10 | 8 | 2／7 | 0 |
| CHS | 11 | 8 | 8／8 | 1 |
| STS | 12 | 8 | 6／8 | 0 |

- 2 スロット以上のアクセスの間は、新しいアクセスを決めない。port 9 は終わりが分かっているので、2 スロット目の頭で次のアクセスを決める（STB が OPDN を出す）。port 1 と DOC の port 2 は、P1A・P2A の OPDN で終わりを知り、その次のスロットの頭で決める（答えが早くても 2 スロット）[P]。port 1 は、エミュレータの模型（ブリッジの同期化の時刻。[emulator.md](../impl/emulator.md) §03）で、DACK を出してから同期化した DTACK を見るまで 3 スロットで、それが 3 スロット目の q1〜q3 の縁なら次のスロットに数えて 4 スロット（DR-091）。1 語は 6〜8 スロットになる（OI-051）。DOC の port 2 は、P2A が書込みを預かってすぐ答える（DR-050）。預かりが空いていても、ラッチはアクセスを始めたスロットの q1 の縁なので答えは次のスロットに数え、書込みは 2 スロットで終わる（DR-091。次の決定まで 3 スロット）。エミュレータの模型では、Staging RAM からの書込みは 1 byte が 6 スロットで、E の位相によらない（DR-091 の前は 4 スロットか、4 スロットと 8 スロットの交互）。読出しは、答えの縁で 1 回のアクセスが 1 スロット延びることがあるが、1 byte 8 スロットは変わらない。U38（74LS393）が 8 MHz を数え、32 クロックで TOUT を出す（DR-046 (9)）。
- DR-089 で、U38 は 74LS163 にした。延びたアクセスの間（LA）だけ LCLK を数え、LA でないと同期で 0 に戻す。15 の RCO が TOUT で、延びたアクセスの 16 スロット目（4 µs）に時間切れになる。STB は OPDN を出さず、終わりは OPA・OPB が決める（下の「アクセスの制御」）[P]。
- 優先は、CPU のアクセス、動いている要素の W、その COUNT の更新、再設定、新しい要素の順（DR-046 (2)）。

### アクセスの制御（DR-089）

NX と OP の制御、アクセスの終わりと時間切れ、ポートごとの信号を、次の PAL で作る [P]。式は下のブロック（[tools/fabric/pal/access.pal](../../tools/fabric/pal/access.pal)）。

| PAL | 型番 | クロック | 働き |
|---|---|---|---|
| TIM | PAL16R8A | 16 MHz | 位相（PH1・PH0）、S・X（SX）、SCLK・MCLK・LCLK、q3 の印（WPHN）、A ファイルの書込みの相（AWQN）（03。DR-091） |
| DPA | PAL16L8A | — | 判断バスの SRC・DST の属性：読出し元で新しい要素を始めてよい（GOKS：port 0 は X のスロットの頭だけ、FB の間は待つ）、番地を持つ（ADDRS、ADDRD）、書込み先の S の規則（SOKD）。FB ＝ OP の FLAG ∨ OP へ入る NX の FLAG |
| OPA | PAL20R4A | SCLK | OP が空くか（OPLD：OP の 74LS377 の /EN）、NX が STOP で消えないか（OPVD：OP の有効の入力）、判断してよいか（CD）、延びたアクセスの打切り（ABT：時間切れか STOP）。このスロットで終わる 1 スロットのアクセス（SD）と延びたアクセス（LA）を登録する |
| OPB | PAL20R4A | LCLK | このスロットの読出し・書込みの完了（DNR、DNW）、ERROR（ERRK：port 1 の DONE の食い違いか時間切れ）。OP のアクセスが終わった（ODN）、時間切れ（TOR）、読出しの完了（DNRQ：IL の /EN。DR-091）を登録する |
| STB0 | PAL16L8A-2 | — | A 基板のポート（0、9）の要求、U45–U47 の緩衝器、Temporal RAM（U56）と拡張（U57）の緩衝器、ANX の出力の許可 |
| STB1 | PAL16L8A-2 | — | 本体 DAC・port 3・5 の FIFO の SI、port 4 の SO と U51–U52、Temporal RAM の書込みと読出し |
| P1A | PAL16R6A | 16 MHz | port 1（ACRTC）：OP に入る q0 の縁で DACK を出し、同期化した DTACK で答え、そのときの DONE を印にする。答えは q0 の縁ならそのスロット（P1R、P1M）、q1〜q3 の縁なら預かり（PND）、次の q0 の縁で P1R。打切り（ABQ、q0 の縁）で DACK を戻し、預かりを捨てる（DR-091 で 16R4A-2 から） |
| P2A | PAL16R8A | 16 MHz | port 2（DOC 子基板）：E の同期化の 2 段目（SYN2）、預かりの段（空き、窓を待つ、窓を取った、取った読出しを捨てる）、預かったものが読出しか、答えの預かり（PND）とこのスロットの答え（P2R、q0 の縁で登録）。U48–U49 の許可（LDQ）（DR-091 で 16R6A-2 から） |

- NX（74LS374×2、16 bit）：ポート 4、向き、チャネル、FORMAT 3、W16、LG（延びたアクセス）、FLAG、TWO、有効、STV、STL。LG はつなぎで作る：判断バスのポートが 1、または DOC 子基板があって 2。F4 は NX に置かず、OP の FORMAT からつなぎで作る（T3 ＝ S3⊕F4 の F4）。
- OP（74LS377×2）：NX と同じ中身から STV・STL を除いた 14 bit。OPLD のときだけ取り込み、有効の bit には OPVD を入れる。DR-091 で、ポートと向きを持つ OP0 を 74F377 にした（P2A が q1 の縁で見る。03）。
- 1 スロットのアクセスは SD、延びたアクセスは LA の間に P1A・P2A の答え（P1R・P2R）で終わる。port 9 は 2 スロット目が SD になる。読出しの完了で IL（74LS377×2、SCLK）が FD を取り込む。DR-091 で、IL の /EN は OPB が LCLK で登録した DNRQ にした（P1R・P2R が q0 の縁で変わるので、組合せの DNR では SCLK の縁で保持が足りない）。時間切れ（U38 の TOUT）は LA の 16 スロット目で、TOR を P1A・P2A の打切りに回す。
- 書込みの値のラッチ U50 は 74LS377（LCLK、/EN ＝ P2R）、番地のラッチ U48–U49 は 74LS377（16 MHz、/EN ＝ LAT）にした（08）。DR-091 で、U48–U49 の /EN は P2A が登録する LDQ（ラッチの次の縁で取り込む）になった。U50 は、答えを数えたスロットの LCLK で取り込む。P2A は q1 以降の縁でラッチするので、それは 2 スロット目以降で、FD は OP が保つ間ずっと出ている。本体のクロック（DOC の CLK 8 MHz）では、U50 の値は窓の E の立上りの前に入る：いちばん近いのはラッチが q1 の縁のときで、E の立上りはラッチの 375 ns 以上後（emulator.md §03）、U50 の LCLK はラッチの 312.5 ns 後（TIM の 15 を足して 327.5）なので、47.5 ns 以上前。データの出力は 74LS377 の CLK→Q（27）の後で、窓の中の波形 RAM の書込みに間に合うかは [V]。外部クロックで E が短いと、U50 が取るのは窓の E の立上りの後になりうる（DOC の CLK が 16 MHz 以下なら窓の終わりの前）[V]。
- 確かめたこと：DPA・OPA・OPB・STB・TIM は入力の全部、P1A・P2A は届きうる入力と状態の全部で、fabric_logic.py の仕様の定義と一致する。全体は門の模型で突き合わせた（上の「確かめたこと」）[P]。DR-091 の P1A・P2A（q1〜q3 の答えの預かり、q0 の縁だけで効く打切り、P2A の LDQ）も同じように突き合わせた。門の模型は、延びたアクセスの答えの縁（q0〜q3）をポートの模型から受け取り、fabric.rs と同じスロットに数えることを、q0〜q3 のそれぞれで確かめた（試験 `fabric_lockstep` の網羅の目安「答え q0」〜「答え q3」）。
- DR-091 の P2A の式は、E の 1 周期が 16 MHz の 8 周期以上（DOC の CLK が 16 MHz 以下）として、届かない組を省いた（fabric_logic.py の `p2a_reachable`：同期化した E の立下りと、預かった答え・このスロットの答えが重ならない）。U50 が窓の終わりの前に値を取るのも同じ前提による。外部クロック（DR-035）で DOC の CLK を 16 MHz より上げたときの P2A の働きは決めていない [V]（DOC は個体 0・25 °C で 12.6 MHz から崩れ始める。impl/emulator.md §04）。
- P2A は、預かった読出しを STOP や時間切れで打ち切ると、窓を取る前でも「捨てる」段に入り、窓を 1 つ使ってから空く。エミュレータの模型（doc.rs）は、窓を取る前なら預かりをすぐ消す。違いは、打ち切った後の次の DOC のアクセスが最大で E の 1 周期遅れることだけ [V]（OI-089）。
- 68000 のブリッジ（/DCS の窓の待ち、E の立下りでの窓の取得、DTACK）の 3 つのレジスタは、P2A のピンが足りないので HACK に置いた。DR-088 の「P2A が E の立下りを使う 3 つのレジスタを持つ」とは置き場所が違う（同じ SYN1・SYN2 を見る）[P]（OI-089）。

```
; TIM (PAL16R8A)  16 MHz の位相（PH1・PH0：q0〜q3 が 0〜3）、S・X（SX＝1 で今のスロットが S）、SCLK（q0 の縁で立つ）、MCLK・LCLK（q2 の縁で立つ。同じ信号を負荷で分ける）、WPHN（q3 の間 Low：次の 16 MHz の縁が q0）、A ファイルの書込みの相（AWQN：AW のとき q3 の間 Low。74S32 で C16 と OR して /AWE）。RESN で q3・X に戻す
PIN C16 RESN AWN NC NC NC NC NC NC GND GND PH1 PH0 SX SCLK MCLK LCLK WPHN AWQN VCC
/PH1 := RESN*/PH1*/PH0 + RESN*PH1*PH0
/PH0 := RESN*PH0
/SX := /RESN + PH1*PH0*SX + /PH1*/SX + /PH0*/SX
/SCLK := /RESN + /PH1 + /PH0
/MCLK := /RESN + PH1 + /PH0
/LCLK := /RESN + PH1 + /PH0
/WPHN := /RESN + PH1*/PH0
/AWQN := RESN*PH1*/PH0*/AWN

; DPA (PAL16L8A)  判断バスの読出し元（S3–S0）と書込み先（D3–D0）の属性：新しい要素を始めてよい（GOKS：S のスロットの規則と FLAG の待ち）、番地を持つ（ADDRS、ADDRD）、書込み先の S の規則（SOKD）。X は今が X のスロット
PIN S0 S1 S2 S3 D0 D1 D2 D3 X GND OPV GOKS OPFL OPVD NXFL NC ADDRS SOKD ADDRD VCC
/GOKS = /S3*/S2*/S1*/S0*/X + OPV*OPFL + OPVD*NXFL
/ADDRS = S2 + /S3*S0 + S3*S1
/SOKD = /D3*/D2*/D1*/D0*/X
/ADDRD = D2 + /D3*D0 + D3*D1

; OPA (PAL20R4A)  SCLK の前に OP が空くか（OPLD：OP の 74LS377 の許可）、NX のアクセスが STOP で消えないか（OPVD：OP の有効の入力）、判断してよいか（CD）、延びたアクセスの打切り（ABT）。SCLK で、このスロットで終わる 1 スロットのアクセス（SD）と延びたアクセス（LA）を登録する
PIN SCLK NXV NXCH NXTW NXLG OPV OPCH OPTW OPLG ODN STPQ0 GND GND STPQ1 OPLDN OPVD SDN LAN NC NC CD ABTN TOR VCC
/OPLDN = ODN + /OPV + /OPCH*STPQ0 + OPCH*STPQ1
/OPVD = /NXV + NXCH*STPQ1 + /NXCH*STPQ0
/CD = NXV*NXCH*NXTW*/STPQ1 + NXV*/NXCH*NXTW*/STPQ0 + NXV*NXCH*NXLG*/STPQ1 + NXV*/NXCH*NXLG*/STPQ0
+ OPV*OPCH*/OPTW*/ODN*/STPQ1 + OPV*/OPCH*/OPTW*/ODN*/STPQ0
/ABTN = TOR + OPV*OPCH*OPLG*/ODN*STPQ1 + OPV*/OPCH*OPLG*/ODN*STPQ0
/SDN := NXV*NXCH*/NXTW*/NXLG*/STPQ1 + NXV*/NXCH*/NXTW*/NXLG*/STPQ0
+ OPV*OPCH*OPTW*/ODN*/STPQ1 + OPV*/OPCH*OPTW*/ODN*/STPQ0
/LAN := NXV*NXCH*NXLG*/STPQ1 + NXV*/NXCH*NXLG*/STPQ0 + OPV*OPCH*OPLG*/ODN*/STPQ1 + OPV*/OPCH*OPLG*/ODN*/STPQ0

; OPB (PAL20R4A)  LCLK の前の、このスロットのアクセスの結果：読出し・書込みの完了（DNR、DNW）、ERROR（ERRK：port 1 の DONE の食い違いか時間切れ）。LCLK で、OP のアクセスが終わった（ODN）、時間切れ（TOR）、読出しの完了（DNRQ：IL が次の SCLK で取り込む許可）を登録する。P1R・P2R・P1M は P1A・P2A の答え（q0 の縁で登録）と DONE の印、TOUT は U38
PIN LCLK SDN LAN OPW P1RN P2RN P1MN TOUT INFN Q1N EPCN GND GND ELASTN TLN DNR ODN TOR DNRQN NC DNW ERRKN TLASTN VCC
/DNR = OPW + SDN*P1RN*P2RN + /P1MN*/INFN*Q1N*EPCN*ELASTN + /P1RN*P1MN*/INFN*Q1N*EPCN*/ELASTN
/DNW = /OPW + SDN*P1RN*P2RN + /P1MN*INFN*/TLN*TLASTN + /P1RN*P1MN*INFN*/TLN*/TLASTN
+ /P1MN*Q1N*/TLN*TLASTN + /P1RN*P1MN*Q1N*/TLN*/TLASTN
/ERRKN = /LAN*P1RN*P2RN*TOUT
+ OPW*/P1MN*INFN*/TLN*TLASTN + OPW*/P1RN*P1MN*INFN*/TLN*/TLASTN
+ OPW*/P1MN*Q1N*/TLN*TLASTN + OPW*/P1RN*P1MN*Q1N*/TLN*/TLASTN
+ /OPW*/P1MN*/INFN*Q1N*EPCN*ELASTN + /OPW*/P1RN*P1MN*/INFN*Q1N*EPCN*/ELASTN
/ODN := SDN*P1RN*P2RN*LAN + SDN*P1RN*P2RN*/TOUT
/TOR := LAN + /P1RN + /P2RN + /TOUT
/DNRQN := DNR

; STB0 (PAL16L8A-2)  A 基板のポート（0、1、9）の要求と緩衝器（U45 上位、U46 下位、U47 下位と上位の入替え）、Temporal RAM の緩衝器（U56）、拡張（U57、port 8・10〜15）、ANX の出力の許可（WEN）。OP から（OPWORD は OP の W16）
PIN OPV OPW OPP0 OPP1 OPP2 OPP3 OPWORD PA0 NC GND NC RQ0N RQ9N ABHN ABLN ABSN T67N XSN WEN VCC
/RQ0N = OPV*/OPP3*/OPP2*/OPP1*/OPP0
/RQ9N = OPV*OPP3*/OPP2*/OPP1*OPP0
/ABHN = OPV*OPWORD*/OPP3*/OPP2*/OPP1
/ABLN = OPV*OPWORD*/OPP3*/OPP2*/OPP1 + OPV*/OPWORD*PA0*/OPP3*/OPP2*/OPP1*/OPP0
/ABSN = OPV*/OPWORD*/PA0*/OPP3*/OPP2*/OPP1*/OPP0 + OPV*/OPWORD*OPP3*/OPP2*/OPP1*OPP0
/T67N = OPV*/OPP3*OPP2*OPP1
/XSN = OPV*OPP3*/OPP2*/OPP1*/OPP0 + OPV*OPP3*/OPP2*OPP1 + OPV*OPP3*OPP2
/WEN = /OPV + /OPW

; STB1 (PAL16L8A-2)  B 基板の FIFO と Temporal RAM のアクセスの信号：本体 DAC の FIFO（port 2、DOC 子基板がないとき）・port 3・5 の SI（q2〜q3）、port 4 の SO（q3）と出力の緩衝器（U51–U52）、Temporal RAM の書込み（port 6、q2〜q3）と読出し（port 7）
PIN OPV OPP0 OPP1 OPP2 OPP3 DOC PH1 WPHN NC GND NC SIDN SI3N SI5N SO4N OE4N T6WN T7RN NC VCC
/SIDN = OPV*/OPP3*/OPP2*OPP1*/OPP0*/DOC*PH1
/SI3N = OPV*/OPP3*/OPP2*OPP1*OPP0*PH1
/SI5N = OPV*/OPP3*OPP2*/OPP1*OPP0*PH1
/SO4N = OPV*/OPP3*OPP2*/OPP1*/OPP0*/WPHN
/OE4N = OPV*/OPP3*OPP2*/OPP1*/OPP0
/T6WN = OPV*/OPP3*OPP2*OPP1*/OPP0*PH1
/T7RN = OPV*/OPP3*OPP2*OPP1*OPP0

; P1A (PAL16R6A)  port 1（ACRTC）。16 MHz の縁ごと。NX の port 1 のアクセスが OP に入る q0 の縁（WPHN が Low）で DACK を出し（DACKN）、同期化した DTACK（DTS）で答え、そのときの DONE（DNF）を印にする（DNM）。答えは、q0 の縁ならそのスロットの答え（P1R、印 P1M）、q1〜q3 の縁なら預かり（PND）にして次の q0 の縁で P1R にする。打切り（ABQ ＝ ABT∧WPH。74S32）は q0 の縁だけで効き、DACK を戻し、預かった答えを捨てる
PIN C16 WPHN OPLDN OPVD NXLG NXP0 DTS DNF ABQN GND GND NC DACKN PNDN DNMN P1RN P1MN NC NC VCC
/DACKN := /WPHN*/OPLDN*OPVD*NXLG*NXP0 + /DACKN*/DTS*ABQN
/PNDN := WPHN*/DACKN*DTS + WPHN*/PNDN
/DNMN := /DACKN*DTS*ABQN*DNF + /DNMN*DACKN + /DNMN*/DTS + /DNMN*/ABQN
/P1RN := /WPHN*ABQN*/DACKN*DTS + /WPHN*ABQN*/PNDN + WPHN*/P1RN
/P1MN := /WPHN*ABQN*/DACKN*DTS*DNF + /WPHN*ABQN*/PNDN*/DNMN + WPHN*/P1MN

; P2A (PAL16R8A)  port 2（DOC 子基板）。16 MHz の縁ごと。DOC の E の同期化の 2 段目（SYN2。1 段目 SYN1 は外の 74ALS74。DR-088）、預かりの段（S1・S0：空き 00、窓を待つ 01、窓を取った 11、取った読出しを捨てる 10）、預かったものが読出し（RDH）。空きなら Fabric のアクセスをラッチし（LDQ：次の縁で U48–U49 が番地を取り込む）、E の立下りで窓を取り、次の立下りで空ける。答え（書込みのラッチ、読出しの窓の終わり）は、q0 の縁ならそのスロットの答え（P2R）、q1〜q3 の縁なら預かり（PND）にして次の q0 の縁で P2R にする。打切り（ABQ ＝ ABT∧WPH。74S32）は q0 の縁だけで効く。BWN は 68000 のブリッジが窓を待っていない（HACK）。組合せの出力を帰還しない
PIN C16 SYN1 LAN OPP0 OPW ABQN WPHN BWN NC GND GND SYN2N S1N S0N RDHN PNDN P2RN LDQN NC VCC
/SYN2N := SYN1
/S1N := /S1N*SYN2N + /S1N*SYN1 + /SYN2N*/SYN1*BWN*S1N*/S0N*RDHN + /SYN2N*/SYN1*BWN*S1N*/S0N*ABQN
+ /SYN2N*/SYN1*BWN*/LAN*/OPP0*ABQN*S0N*PNDN*P2RN + /LAN*/OPP0*ABQN*BWN*/S1N*RDHN
/S0N := /S0N*S1N*RDHN + /S0N*SYN2N*ABQN + /S0N*SYN1*ABQN + /S1N*SYN2N*RDHN + /S0N*SYN1*RDHN
+ /LAN*/OPP0*ABQN*S1N*PNDN*P2RN + /LAN*/OPP0*ABQN*/S0N*RDHN + /SYN2N*/SYN1*/LAN*/OPP0*ABQN*/S1N*S0N
/RDHN := /OPW*S0N + /OPW*/RDHN + /SYN2N*/SYN1*/OPW*/S1N + /S1N*S0N*SYN1 + /S1N*S0N*SYN2N
/PNDN := WPHN*/PNDN + WPHN*/SYN2N*/SYN1*/S1N*/S0N*/RDHN + WPHN*/SYN2N*/SYN1*/LAN*/OPP0*OPW*/S1N
+ WPHN*/LAN*/OPP0*OPW*S1N*S0N*P2RN
/P2RN := WPHN*/P2RN + /WPHN*ABQN*/PNDN + /WPHN*ABQN*/SYN2N*/SYN1*/S1N*/S0N*/RDHN
+ /WPHN*ABQN*/SYN2N*/SYN1*/LAN*/OPP0*OPW*/S1N + /WPHN*ABQN*/LAN*/OPP0*OPW*S1N*S0N*P2RN
/LDQN := /LAN*/OPP0*ABQN*S1N*S0N*PNDN*P2RN + /SYN2N*/SYN1*/LAN*/OPP0*ABQN*/S1N*RDHN
+ /SYN2N*/SYN1*/LAN*/OPP0*ABQN*/S1N*S0N
```

| PAL | 入力 | 出力 | 積（最大） | 空きピン |
|---|---:|---:|---:|---:|
| TIM | 2 | 8 | 4／8 | 6 |
| DPA | 13 | 4 | 3／7 | 1 |
| OPA | 12 | 6 | 6／7・4／8 | 2 |
| OPB | 13 | 6 | 7／7・4／8 | 1 |
| STB0・STB1 | 8・8 | 8・7 | 3／7・1／7 | 2・3 |
| P1A | 8 | 5 | 4／8 | 3 |
| P2A | 7 | 7 | 8／8 | 2 |

### チャネルの状態（CHS、STS）

CHS・STS の登録する状態と式は、上の「順序と状態の PAL」の表とブロック [P]（DR-085）。CHS は段階・START の検査・DONE・ERROR を、STS はトリガと STALL を持つ。STATUS の BUSY・WAIT は CHS・STS の出力から作る。

- START の検査：再設定の 1 スロット目で読出し元を、2 スロット目で書込み先を、ATR（DOC の有無で OKD か OKN）と U33 で調べる。3 スロット目で、どちらかが使えなければ ERROR、COUNT の設定値が 0（CY＝0）なら DONE にする（DR-046 (9)）。
- トリガ：U34・U35（74LS151）が TRIGGER の発生源を選び、U36（74LS74）と STS の 2 段で同期化し、立上りで 1 件だけ保留する。保留中の立上りは MISS（DR-018、DR-046 (8)）。発生源 6・7 の入力は Low に固定し、トリガは来ない。選択器が同期化の前にあるので、TRIGGER の発生源を替えたとき、選んだ入力が High なら立上りとして伝わる。チャネルが止まっている間の立上りは数えないので、TRIGGER を書いてから 3 スロット以上あけて START する [P]（エミュレータの模型で確かめた。[emulator.md](../impl/emulator.md) §03）。
- 割込み：IRQ ＝ IRQ_EN0∧(DONE0∨ERROR0) ∨ IRQ_EN1∧(DONE1∨ERROR1) をつなぎのゲートで作り、レベル 3 へ（DR-028）。

## 07　CPU のインタフェース

- 窓は 0x250000〜0x250FFF（仕様書 §05。/FCS は A 基板の解読から）。レジスタは 16-bit のアクセスだけに答える。8-bit のアクセス、チャネル 2・3、各チャネルの 0x20〜0x3E、0x250106 以降には DTACK を返さず、A 基板のバスの時間切れでバスエラーになる（DR-046 (10)）。
- 書込み：SRC・DST・FORMAT・SHIFT・MASK・XOR・TRIGGER は ID からそのまま取り込む（HDEC と U5–U6）。STEP は B ファイルの書込みポートへ（番地は HA6 と HA4）。CTRL は要求の FF（STRQ・STPQ）を通して CST・CHS へ、STATUS の W1C は要求の FF（WDQ・WEQ・WMQ）を通して CHS・STS へ（06。DR-085）。SRC_ADDR・DST_ADDR・COUNT は HACK が SEQ に A ファイルを求め（HREQ）、許可（HGNT）のスロットで書く。PORT_RESET は PRST がポートごとの初期化の信号に直す。AUDIO_N は書込みの信号だけを出し、音声クロックの分周器（board.md の ADPCM・本体 DAC のブロック）が取り込む。
- 読出し：作業値は A ファイルから U7–U8 を通して（2〜3 ウェイト [V]）、STATUS は U9–U10、MODULES は U11 から。ほかのレジスタと 32-bit の上位の語は、RD に何も出さないので 0 が読める。
- DTACK：A ファイルを使わないアクセスは HACK がすぐ返す。A ファイルのアクセスは、許可のスロットの q3 の縁（3/4）で返す。
- DR-089 で式を書いた [P]。式は下のブロック（[tools/fabric/pal/host.pal](../../tools/fabric/pal/host.pal)）。HDEC0 は、窓の解読に 15 本の入力が要るので PAL20L8A にした（HDEC1 は PAL16L8A-2 のまま）。HDEC0 が DTACK をすぐ返すアクセス（VLD）、A ファイルのアクセス（AREQ：0x06・0x0A・0x0C）、チャネルと全体のレジスタの範囲を出し、HDEC1 が書込みの信号と STATUS の読出しの許可を出す。オフセット 0x12〜0x1E の書込みは U5–U6（74LS138。A6 でチャネル、A3–A1 で語）。HACK は AK（許可のスロットの q3 の縁で立ち、AREQ が落ちるまで保つ）と HREQ（AREQ ∧ ¬AK ∧ ¬許可。DR-091 で、許可のスロットの頭から下げ、q3 の縁の AK を待たずに ELG が次の判断をできるようにした）、DTACK、DOC の窓のブリッジの 3 つのレジスタを持つ。AK は MCLK が High の間の縁、つまり q3 の縁で立つ（DR-089 の「LCLK の縁」は、LCLK が q3 の縁で立っていたときの言い方で、時刻は変わらない）。HACK は A 品（PAL16R4A）にした（03）。PRST は PORT_RESET の bit 2–7、10 と電源投入の RESN から、ポートごとの初期化の信号を出す。
- 確かめたこと：HDEC0–1 は窓の番地・幅・向きの全部で仕様書 §06 の窓と、HACK・PRST は入力と状態の全部で、fabric_logic.py の仕様の定義と一致する [P]。

```
; HDEC0 (PAL20L8A)  CPU の窓（/FCS、16-bit のアクセス、A11–A1）の解読：DTACK をすぐ返すアクセス（VLD）、A ファイルのアクセス（AREQ：0x06・0x0A・0x0C）、チャネルのレジスタの範囲（CH）、全体のレジスタの範囲（GL：0x100〜0x104）、A ファイルの読出しの許可（RAF：U7–U8）、MODULES（RMOD：U11）、読出しの緩衝器（RDEN：U3–U4）
PIN FCSN UDSN LDSN RW A11 A10 A9 A8 A7 A6 A5 GND A4 A3 VLDN AREQN CHN GLN RAFN RMODN A1 RDENN A2 VCC
/VLDN = /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5*A4
+ /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5*/A4*/A3*/A2
+ /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5*/A4*/A3*/A1
+ /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5*/A4*/A2*/A1
+ /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5*/A4*A3*A2*A1
+ /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*A8*/A7*/A6*/A5*/A4*/A3*/A2
+ /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*A8*/A7*/A6*/A5*/A4*/A3*/A1
/AREQN = /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5*/A4*/A3*A2*A1
+ /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5*/A4*A3*/A2*A1
+ /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5*/A4*A3*A2*/A1
/CHN = /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*/A8*/A7*/A5
/GLN = /FCSN*/UDSN*/LDSN*/A11*/A10*/A9*A8*/A7*/A6*/A5*/A4*/A3
/RAFN = /AREQN*RW
/RMODN = /GLN*RW*A2*/A1
/RDENN = /FCSN*RW

; HDEC1 (PAL16L8A-2)  チャネル・全体のレジスタの範囲から、書込みの信号と STATUS の読出しの許可：オフセット 0x10〜0x1E の書込み（W1X：U5–U6 の 74LS138 の許可。チャネルは A6）、SRC・DST（WSRC、WDST）、B ファイル（WBF：0x0E・0x10）、PORT_RESET（WPRST）、AUDIO_N（WAUD）、STATUS（RST0・RST1：U9–U10）
PIN CHN GLN RW A6 A4 A3 A2 A1 NC GND NC W1XN WSRCN WDSTN WBFN WPRSTN WAUDN RST0N RST1N VCC
/W1XN = /CHN*/RW*A4
/WSRCN = /CHN*/RW*/A4*/A3*/A2*/A1
/WDSTN = /CHN*/RW*/A4*/A3*/A2*A1
/WBFN = /CHN*/RW*/A4*A3*A2*A1 + /CHN*/RW*A4*/A3*/A2*/A1
/WPRSTN = /GLN*/RW*/A2*/A1
/WAUDN = /GLN*/RW*/A2*A1
/RST0N = /CHN*RW*/A6*A4*A3*A2*A1
/RST1N = /CHN*RW*A6*A4*A3*A2*A1

; HACK (PAL16R4A)  68000 の DTACK（16 MHz の縁ごと）。Fabric の窓：A ファイルを使わないアクセスはすぐ（VLD）、A ファイルのアクセスは SEQ の許可（AOP＝001）のスロットの q3 の縁で返す（AK）。HREQ は ELG への要求で、許可のスロットの頭から下げる（AOP＝001）。DOC の窓のブリッジ（DR-035、DR-050）：/DCS を取り込んで窓を待ち（BW）、同期化した E の立下りで次の窓を取り（BC。P2A より先）、その窓の終わりの立下りで DTACK を出す（BDT）
PIN C16 AREQN VLDN AOP2N AOP1N AOP0N MCLK SYN1 SYN2N GND GND DCSN NC AKN BWN BCN BDTN HREQ DTKN VCC
/AKN := /AREQN*AOP2N*AOP1N*/AOP0N*MCLK + /AREQN*/AKN
/BWN := /DCSN*BWN*BCN*BDTN*SYN2N + /DCSN*BWN*BCN*BDTN*SYN1 + /BWN*/DCSN*SYN2N + /BWN*/DCSN*SYN1
/BCN := /SYN2N*/SYN1*/DCSN*BWN*BCN*BDTN + /SYN2N*/SYN1*/DCSN*/BWN + /BCN*SYN2N + /BCN*SYN1
/BDTN := /BCN*/SYN2N*/SYN1*/DCSN + /BDTN*/DCSN
/HREQ = AREQN + /AKN + AOP2N*AOP1N*/AOP0N
/DTKN = /VLDN + /AKN*/AREQN + /BDTN

; PRST (PAL16L8A-2)  PORT_RESET の書込み（WPRST）と ID のビットから、ポートごとの初期化の信号（2：本体 DAC の FIFO、3・4：DSP の FIFO、5：ADPCM の FIFO とアダプタ、6・7：Temporal RAM のポインタ、10：音声入力の FIFO）。電源投入の RESN でも出す
PIN WPRSTN D2 D3 D4 D5 D6 D7 D10 RESN GND NC RST2N RST3N RST4N RST5N RST6N RST7N RST10N NC VCC
/RST2N = /WPRSTN*D2 + /RESN
/RST3N = /WPRSTN*D3 + /RESN
/RST4N = /WPRSTN*D4 + /RESN
/RST5N = /WPRSTN*D5 + /RESN
/RST6N = /WPRSTN*D6 + /RESN
/RST7N = /WPRSTN*D7 + /RESN
/RST10N = /WPRSTN*D10 + /RESN
```

| PAL | 入力 | 出力 | 積（最大） | 空きピン |
|---|---:|---:|---:|---:|
| HDEC0 | 15 | 7 | 7／7 | 0 |
| HDEC1 | 8 | 8 | 2／7 | 2 |
| HACK | 9 | 6 | 3／7・4／8 | 1 |
| PRST | 9 | 7 | 2／7 | 2 |

## 08　ポートの側

| ポート | B 基板の回路 | つなぎ先 |
|---|---|---|
| 0、1、9 | U45–U47（74LS245×3）：FD の上位・下位と、FD の下位を上位の線へ入れ替える 1 個。port 0 の 8-bit のアクセスの偶数番地（DR-046 (3)）と、port 9（文字 ROM は上位バイトレーン。仕様書 §05）は、上位の線を使う。PA の 16 本と、STB の要求・向き・バイトの信号 | A 基板：Staging RAM の調停、ACRTC のブリッジ、文字 ROM |
| 1 | P1A（PAL16R6A。DR-091 で 16R4A-2 から）：ACRTC の DACK を出し、DONE を受けて OPB に渡す（DR-022）。q1〜q3 の縁の答えは次のスロットに数える（DR-091）。DREQ は A 基板のブリッジで同期化したもの（DR-016） | ACRTC |
| 2 | P2A（PAL16R8A。DR-089 で 16R4A-2 から 16R6A-2、DR-091 で 16R8A）：DOC の E の同期化の 2 段目（1 段目は 74ALS74。DR-088）、窓を待つ（DR-035）。アクセスを 1 つ預かり、書込みはラッチしたときに答える。預かりが空いていれば、ラッチはアクセスを始めたスロットの q1 の縁なので、答えは次のスロットに数え、書込みは 2 スロット（DR-091）。続くアクセスは、預かったものの窓が終わるまで待たせる（DR-050）。U48–U49（番地のラッチ）、U50（書込みのデータのラッチ）、U58（読出しの値のラッチ）。DOC 子基板がなければ、FD を本体 DAC の FIFO（67401×4）に書く | DOC 子基板、本体 DAC の FIFO |
| 3、5 | FIFO（67401×4、×2）の入力は FD に直結。SI は STB から | DSP、MSM5205 のアダプタ |
| 4 | FIFO（67401×4）の出力を U51–U52（74LS244）で FD へ。67401 は出力を切れないため | DSP のアダプタ |
| 6、7 | U56（74LS245）で Temporal RAM のデータとつなぐ。ポインタは Temporal RAM のカウンタ | Temporal RAM |
| 8、10〜15 | U57（74LS245）で拡張の端子とつなぐ。番地は PA、準備は XRDY、ある・なしは /PRES | 映像入力基板、拡張 |

- 67401 を 4 個並べた 16 bit の FIFO は、IR と OR を 4 個の AND にする（U53–U55、74LS21。[review-digital.md](../parts/review-digital.md) 付録 A.1）。port 5 の 2 個の AND はつなぎのゲートで作る。

## 09　部品表

| 区分 | 記号 | 型番 | 個数 | 働き |
|---|---|---|---:|---|
| CPU のインタフェース | U1–U2 | 74LS244 | 2 | CPU のデータを内部の書込みバス ID へ |
| CPU のインタフェース | U3–U4 | 74LS240 | 2 | 内部の読出しバス RD から CPU へ（反転。RD は引上げで、何も出さないと 0 が読める） |
| CPU のインタフェース | HDEC0 | PAL20L8A | 1 | 番地の解読：DTACK をすぐ返すアクセス、A ファイルの要求、レジスタの範囲、読出しの選択（DR-089 で 16L8A-2 から） |
| CPU のインタフェース | HDEC1 | PAL16L8A-2 | 1 | レジスタの書込みの信号、STATUS の読出しの許可 |
| CPU のインタフェース | HACK | PAL16R4A | 1 | DTACK。A ファイルのアクセスでは SEQ の許可を待つ。DOC の窓のブリッジの 3 つのレジスタ（DR-089。DR-091 で 16R4A-2 から） |
| CPU のインタフェース | U5–U6 | 74LS138 | 2 | チャネルごとに、オフセット 10〜1E の書込みの信号 |
| 読出しの経路 | U7–U8 | 74LS534 | 2 | A ファイルの読出しのラッチ（反転。A バス → RD。CPU のサイクルの終わりまで保つ） |
| 読出しの経路 | U9–U10 | 74LS240 | 2 | STATUS（ch0、ch1）→ RD |
| 読出しの経路 | U11 | 74LS244 | 1 | MODULES（モジュールの /PRES 線）→ RD |
| 番地と COUNT | AF0–AF3 | 74S189 | 4 | A ファイル：16 語×16 bit（設定値と作業値の 12 語） |
| 番地と COUNT | BF0–BF3 | 74LS670 | 4 | B ファイル：STEP の 4 語（CPU は別の書込みポートから書く） |
| 番地と COUNT | AD0–AD3 | 74S283 | 4 | 16 bit の加算器 |
| 番地と COUNT | SL0–SL1 | 74ALS534 | 2 | 和のラッチ（反転）→ A ファイルの書込みデータ（DR-091 で 74LS534 から） |
| 番地と COUNT | PS0–PS1, PD0–PD1 | 74LS374 | 4 | ポートの番地のラッチ（読出し元 PA_S、書込み先 PA_D） |
| 番地と COUNT | U12–U13 | 74LS240 | 2 | CPU の書込み → A ファイル（反転） |
| 番地と COUNT | U64 | 74S157 | 1 | A ファイルの語の bit 0：PH1 で AFC0 の読み（AR0N）と書き（AW0N）を選ぶ（DR-091） |
| 番地と COUNT | U65–U66 | 74S02 | 2 | A バスの 0 の検出：2 bit ずつの NOR（DR-091） |
| 番地と COUNT | U67 | 74S30 | 1 | A バスの 0 の検出：8 入力の NAND。出力が桁上げ（CY ＝ A ≠ 0）（DR-091） |
| 変換 | IL0–IL1 | 74LS377 | 2 | 読んだ値のラッチ（SCLK、/EN ＝ DNRQ。DR-089 で 74LS374 から、DR-091 で /EN を DNR から） |
| 変換 | ROT0–ROT3 | PAL20L8A | 4 | 回転：細かい段（0〜3）と粗い段（4 の倍数） |
| 変換 | MSK0–MSK1 | 63S281A | 2 | 有効なビットのマスク（256×8 PROM） |
| 変換 | ANX0–ANX3 | PAL20L8A | 4 | (回転∧マスク∧MASK)⊕XOR。W のスロットでバスへ出す |
| 設定のレジスタ | U14–U17 | 74LS173 | 4 | SRC・DST（ch0、ch1）→ 判断バス |
| 設定のレジスタ | U18–U19 | 74LS173 | 2 | FORMAT（ch0、ch1）→ 判断バス |
| 設定のレジスタ | U20–U21 | 74LS374 | 2 | SHIFT（ch0、ch1） |
| 設定のレジスタ | U22–U29 | 74LS374 | 8 | MASK・XOR（ch0、ch1） |
| 設定のレジスタ | U30–U31 | 74LS174 | 2 | TRIGGER（発生源、EACH、LOOP、IRQ_EN） |
| 順序と状態 | TIM | PAL16R8A | 1 | スロットの時刻（16 MHz）：4 分の 1 の位相、S・X、取込みとラッチの信号、q3 の印、A ファイルの書込みの相（DR-091） |
| 順序と状態 | SEQ | PAL20R8A | 1 | 判断の登録：候補のチャネル、AOP、決めたアクセス（DR-085） |
| 順序と状態 | ELG | PAL20L8A | 1 | 頭の判断：A ファイルの操作、新しい要素、書込み、STALL の評価（DR-085） |
| 順序と状態 | CST | PAL20L8A | 1 | 判断バスのチャネルともう一方の状態（DR-085） |
| 順序と状態 | ELS | PAL20R8A | 1 | 要素の状態と書込みの残り（DR-085） |
| 順序と状態 | EVT | PAL20L8A | 1 | スロットの出来事：止める、ERROR、ブロックの終わり（DR-085） |
| 順序と状態 | ADC | PAL16L8A-2 | 1 | AOP をチャネルごとの信号に（DR-085） |
| 順序と状態 | AFC0 | PAL20R6A | 1 | A ファイルの番地（bit 0 は読みと書き）・書込みのある操作・Cin を SCLK で登録（DR-085 で 20L8A から 16L8A、DR-091 で 20R6A） |
| 順序と状態 | AFC1 | PAL16R8A | 1 | B の選択・A ファイルへのデータ・ラッチの信号を SCLK で登録（DR-085 で 20L8A から 16L8A、DR-091 で 16R8A） |
| 順序と状態 | CHS0–CHS1 | PAL20R6A | 2 | チャネルの段階、START の検査、DONE、ERROR（DR-085） |
| 順序と状態 | STS0–STS1 | PAL20R6A | 2 | トリガの同期化の 2 段目と立上り、ARM、保留、MISS、STALL（DR-085） |
| 順序と状態 | RDY | PAL20R4A | 1 | 候補の読出し元と書込み先の準備（スロットの中ほどで取り込む）、両方の準備（READY） |
| 順序と状態 | STB0–STB1 | PAL16L8A-2 | 2 | ポートごとのアクセスの信号、A 側の緩衝器の向き |
| 順序と状態 | PRST | PAL16L8A-2 | 1 | PORT_RESET：ポートごとの初期化の信号 |
| 順序と状態 | DPA | PAL16L8A | 1 | 判断バスの SRC・DST の属性（S の規則、番地、FB）（DR-089） |
| 順序と状態 | OPA | PAL20R4A | 1 | OP の取込み、NX の有効、判断してよいか、打切り。SD・LA の登録（DR-089） |
| 順序と状態 | OPB | PAL20R4A | 1 | 読出し・書込みの完了、ERRK。ODN・TOR・DNRQ の登録（DR-089、DR-091） |
| 順序と状態 | U32 | 74LS157 | 1 | 次のポート：SRC か DST の番号を選ぶ |
| 順序と状態 | NX0–NX1 | 74LS374 | 2 | 次のアクセスのレジスタ：ポート、向き、チャネル、FORMAT、W16、LG、FLAG、TWO、有効、STALL の評価（DR-085、DR-089） |
| 順序と状態 | OP0 | 74F377 | 1 | アクセスのレジスタ（ポート、向きほか）：NX を空いたときに取り込む（DR-085 で 74LS374 から。DR-091 で 74LS377 から。P2A が q1 の縁で見る） |
| 順序と状態 | OP1 | 74LS377 | 1 | アクセスのレジスタ（残りの bit）（DR-085 で 74LS374 から） |
| 順序と状態 | ATR | 63S281A | 1 | ポートの属性（256×8 PROM） |
| 順序と状態 | U33 | 74LS151 | 1 | ポートがあるか（port 8〜15 の /PRES を選ぶ） |
| 順序と状態 | U34–U35 | 74LS151 | 2 | トリガの発生源を選ぶ（ch0、ch1） |
| 順序と状態 | U36–U37 | 74LS74 | 2 | トリガの同期化の 1 段目（ch0、ch1）、ch0 の CTRL の要求（STRQ0、STPQ0）。桁上げのラッチは DR-085 でなくした |
| 順序と状態 | U59 | 74LS74 | 1 | ch1 の CTRL の要求（STRQ1、STPQ1）（DR-085） |
| 順序と状態 | U60–U61 | 74LS175 | 2 | STATUS の W1C の要求（WDQ、WEQ、WMQ。ch0、ch1）（DR-085） |
| 順序と状態 | U62–U63 | 74LS32・74LS08 ほか | 2 | つなぎ：USEL・OKS・HOLD（SRC＝0）・LG・F4・RDY2（DOC ∨ 本体 DAC の IR）のゲート（DR-085、DR-089。FLAG の FF と SOK・CD は DPA・OPA へ移した。ゲートの数は [V]） |
| 順序と状態 | U68 | 74S32 | 1 | 打切りを q0 の縁だけに（ABQ ＝ ABT∧WPH）、A ファイルの書込み（/AWE ＝ AWQN ∨ 16 MHz）（DR-091） |
| 順序と状態 | U38 | 74LS163 | 1 | 時間切れ（延びたアクセスの間の LCLK を数え、16 スロット目に TOUT。DR-089 で 74LS393 から） |
| 順序と状態 | U39 | 74LS244 | 1 | クロックの分配 |
| 順序と状態 | U40–U44 | 74LS04/08/27/32/86 | 5 | つなぎ（チャネルの選択の反転、即時のトリガの検出、T3＝S3⊕F4、IRQ の AND・OR など） |
| ポートの側 | U45–U47 | 74LS245 | 3 | A 基板のポート（0、1、9）へのバス。上位・下位と、バイトの入れ替え |
| ポートの側 | P1A | PAL16R6A | 1 | port 1：ACRTC の DACK と DONE の扱い、答えの預かり（DR-091 で 16R4A-2 から） |
| ポートの側 | P2A | PAL16R8A | 1 | port 2：DOC の E の同期化の 2 段目と窓の待ち、アクセスの預かり、答えの預かり（DR-050、DR-089。DR-091 で 16R6A-2 から） |
| ポートの側 | SYN1 | 74ALS74 | 1 | port 2：DOC の E の同期化の 1 段目（HD74ALS74。この型番が 1985年の品揃えにあったかは [V]。DR-088） |
| ポートの側 | U48–U49 | 74LS377 | 2 | port 2：DOC への番地のラッチ（16 MHz、/EN ＝ LDQ。DR-050、DR-089、DR-091） |
| ポートの側 | U50 | 74LS377 | 1 | port 2：書込みのデータのラッチ（LCLK、/EN ＝ P2R。DR-050、DR-089） |
| ポートの側 | U58 | 74LS374 | 1 | port 2：読出しの値のラッチ（DR-050） |
| ポートの側 | U51–U52 | 74LS244 | 2 | port 4：FIFO の出力をバスへ（67401 は出力を切れない） |
| ポートの側 | U53–U55 | 74LS21 | 3 | 67401 を 4 個並べた FIFO の IR・OR の AND |
| ポートの側 | U56 | 74LS245 | 1 | port 6・7：Temporal RAM とのデータ |
| ポートの側 | U57 | 74LS245 | 1 | port 8、10〜15：拡張の端子とのデータ |
| **合計** | | | **129** | PAL 33（24 ピン 22、20 ピン 11）、TTL と PROM 96。ほかに 67401×14 |

| 区分 | 個数 | うち PAL |
|---|---:|---:|
| CPU のインタフェース | 9 | 3 |
| 読出しの経路 | 5 | 0 |
| 番地と COUNT | 24 | 0 |
| 変換 | 12 | 8 |
| 設定のレジスタ | 18 | 0 |
| 順序と状態 | 44 | 20 |
| ポートの側 | 17 | 2 |
| 合計 | 129 | 33 |

- 見積り（DR-041、board.md §02 v0.4）の 90〜130 個の中で、B 基板の見積りが外れる目安の約 110 個を下回った（DR-050 で 1 個足して 109 個）。ただし、順序と状態の PAL は式を書くと 1〜3 個増える可能性がある [V]（OI-050）。
- DR-085 で判断と状態の式を書くと、PAL が 4 個（CST、ELS、EVT、ADC）、TTL が 7 個（NX0–NX1、CPU の要求の FF 3 個、つなぎ 2 個）増え、120 個になった。目安の約 110 個を超える。基板への影響（面積、電流、board.md の見積り）は OI-082 [V]。アクセスの制御と STB、HDEC、HACK、TIM、P1A、P2A、PRST の式はまだなので、さらに増える可能性がある。
- DR-088・DR-089 で式を書き終え、124 個になった。PAL が 3 個（DPA、OPA、OPB）、TTL が 1 個（SYN1 の 74ALS74）増え、HDEC0 は 20 ピンから 24 ピンへ、P2A は 16R4A-2 から 16R6A-2 へ替わった。FLAG の FF と SOK・CD のゲートは PAL に入り、LG・F4・RDY2 のゲートが加わって、つなぎ（U62–U63）の数は変わらない [P]。時間の予算（03）の直し方によっては、さらに 2〜3 個増える（OI-087）[V]。
- DR-091 で時間の予算（03）を閉じ、129 個になった。TTL が 5 個増えた：A バスの 0 の検出（U65–U66 の 74S02、U67 の 74S30）、A ファイルの語の bit 0 の選択（U64、74S157）、打切りと /AWE の OR（U68、74S32）。PAL の数は変わらず、AFC0 は 20 ピンから 24 ピン（PAL20R6A）へ、AFC1 は PAL16R8A、P1A・P2A・HACK は A-2 品から A 品へ替わった。SL0–SL1 は 74ALS534、OP0 は 74F377 にした（数は変わらない）[P]。OI-087 の見積り（2〜3 個）より多いのは、AFC0 の中で語の bit 0 を組み合わせると登録の帰還で遅れる（74S157 を足した）のと、打切りと /AWE に 16 MHz の相のゲートが要ったため。
- 74LS534（U7–U8）は、1985年以前の TI・Motorola・National の資料に見つからない（74ALS534 はある。[PAL.md](../parts/PAL.md) の付記）。74LS374 と反転の組か 74ALS534 にするかは [V]（OI-050）。SL0–SL1 は DR-091 で 74ALS534 にした（tSU 10 が要る）。
- 数の大半は、レジスタ（74LS374・173・174 が 30 個）、緩衝器（74LS240・244・245 が 19 個）、A ファイルの経路（20 個）で、PAL にまとめても減らない。PAL をさらに増やして TTL を減らす手当て（board.md §09 の ①）の余地は小さい。
- 部品の出典：PAL20L8A・20R8A・20R6A・20R4A は MMI の 1985年のデータブック p.5-31 [D MMI85]、PAL16L8・16R4・16R8 の A 品と半電力の A-2 品（電源電流 最大 90 mA）は 1983年の PAL ハンドブック [D MMIPAL83]、63S281A（256×8、3 ステート、20 ピン）は同じデータブック p.3-17 [D MMI85]。74S189、74LS670、74LS534、74S283 は 1985年に日立の品があったかを確かめていない [V]（OI-050）。DR-091 で足した 74S02・74S30・74S32・74S157 は TI の 1985年の本 [D TITTL85V2]、74ALS534 は TI の 1984年の本 [D TITTL84V1]、74F377 は Signetics の 1984年と Fairchild の 1985年の本 [D SIGFAST84、FSFAST85]（PAL.md の付記）。74ALS534 と 74F377 の日立の品は [V]。

## 10　面積と消費電流

### 面積

board.md §01 の 1 個あたりの面積を使う。24 ピンの細い DIP（PAL20L8 など）は表にないので、20 ピンより 0.2 インチ長い分を見て 1.2 倍（中央 6.1 cm²）と仮定する [P]。

| | 小さめ | 中央 | 大きめ |
|---|---:|---:|---:|
| 1 個あたり（14〜20 ピン／24 ピン） | 4.8／5.8 cm² | 5.05／6.1 cm² | 5.3／6.4 cm² |
| 係数（端子・縁） | 1.3 | 1.3 | 1.35 |
| Data Fabric（108 個＋67401×14、両面基板） | 約 783 cm² | **約 824 cm²** | 約 898 cm² |

この節の値は 108 個のときのもの。DR-050 で 74LS374 を 1 個足し（109 個）、Data Fabric は中央で約 7 cm²、B 基板は約 6 cm²（枠の約 0.5%）、+5 V の電流は典型で約 30 mA 増える。下の結論は変わらない [P]。

DR-085 で 11 個増えた（120 個。24 ピンの PAL 3 個、20 ピンの PAL 1 個と TTL 7 個。AFC0–AFC1 は 24 ピンから 20 ピンへ）。Data Fabric は中央で約 74 cm² 増える。B 基板（4 層の係数 0.85、1 個あたりの面積が中央）は約 1,125 cm²（枠の約 104%）になり、中央の見積りでも約 45 cm² あふれる。2 枚の合計は約 2,105 cm²（枠の約 97%）で、収まる。+5 V の電流は典型で約 0.6 A、最大で約 0.8 A 増える。下の表は 108 個のときのままにする。手当て（board.md §09 の ②、子基板）と board.md の見積りの直しは OI-082 [V]。

DR-088・DR-089 で 4 個増えた（124 個。24 ピンの PAL 2 個、20 ピンの PAL 1 個、74ALS74 1 個。HDEC0 は 20 ピンから 24 ピンへ）。Data Fabric は中央で約 30 cm²（4 層の係数 0.85 で約 26 cm²）増える。B 基板は、この節の数え方で約 1,151 cm²（枠の約 107%）、board.md の数え方（DOC のブリッジ 4 個と DR-050 の 1 個を含む）で約 1,179 cm²（枠の約 109%）になり、中央の見積りで約 100 cm² あふれる。2 枚の合計は約 2,160 cm² で、枠とほぼ同じになる。+5 V の電流は、典型で約 0.5 A、最大で約 0.7 A 増え、Data Fabric は 67401 を含めて典型 約 8.6 A、最大 約 13.0 A（PAL の典型は最大の約 7 割と仮定 [V]）。手当ては board.md §09 と OI-082 [P]。

DR-091 で 5 個増えた（129 個。74S157・74S02×2・74S30・74S32。AFC0 は 20 ピンから 24 ピンへ）。Data Fabric は中央で約 34 cm²（4 層の係数 0.85 で約 29 cm²）増える。B 基板は、この節の数え方で約 1,180 cm²（枠の約 109%）、board.md の数え方で約 1,208 cm²（枠の約 112%）になり、中央の見積りで約 130 cm² あふれる。2 枚の合計は約 2,190 cm²（枠の約 101%）で、約 30 cm² あふれる。+5 V の電流は、TTL の 5 個で典型 約 125 mA・最大 約 210 mA（74S157 50・78、74S02 22・37、74S30 4・8、74S32 28・50 mA。TITTL85V2 の ICCH と ICCL の平均）、A-2 品から A 品への 3 個と AFC0 で最大 約 300 mA（典型 約 210 mA）、74F377 で典型 約 40 mA 増え（74F377 の ICC は [V]）、Data Fabric は 67401 を含めて典型 約 9.0 A、最大 約 13.5 A になる。74ALS534 は 74LS534 より少し減るが、数えていない [V]。手当ては board.md §09 と OI-082 [P]。

board.md v0.4 の見積り（約 840 cm²）より約 16 cm² 小さい。B 基板（4 層、外形 400×270 mm＝1,080 cm²）は次のとおり。B のほかのブロック（Temporal RAM、DSP、ADPCM・本体 DAC・音声クロック）は board.md §02 の値で、両面基板で約 425 cm²。

| B 基板 | 4 層の係数 0.8 | 0.85（中央） | 0.9 |
|---|---:|---:|---:|
| 1 個あたりの面積が中央 | 約 999 cm²（92%） | **約 1,062 cm²（98%）** | 約 1,124 cm²（104%） |
| 1 個あたりの面積が大きめ | 約 1,089 cm²（101%） | 約 1,157 cm²（107%） | 約 1,225 cm²（113%） |
| ② Temporal RAM を A に移したとき（中央の面積） | 約 895 cm²（83%） | 約 951 cm²（88%） | 約 1,007 cm²（93%） |

- 中央の見積りでは、B は枠の約 98% に収まる。2 枚の合計は、A（約 910 cm²＋カートリッジの場所 約 70 cm²）と合わせて約 2,042 cm²（枠 2,160 cm² の約 95%）。
- 4 層の係数が 0.9 のときは、B が約 44 cm²、2 枚の合計が約 34 cm² あふれる。B の手当て ②（Temporal RAM を A へ）は、A に空きがあるときだけ使える（board.md §09）。係数が 0.9 なら A もほぼいっぱいで、② は使えない。
- 1 個あたりの面積が大きめに外れると、2 枚の合計で 100 cm² 以上あふれる。外寸は変えず（DR-047）、基板を筐体の中で広げるか、B の上に子基板を足して吸収する（board.md §09）。これは部品表ではなく実装密度の仮定の問題で、OI-049 (1)(6) で確かめる。

### 消費電流（+5 V）

| | 典型 | 最大 |
|---|---:|---:|
| PAL（24 ピン 17 個、20 ピン 9 個） | 約 3.3 A | 約 4.5 A |
| TTL と PROM（82 個） | 約 2.7 A | 約 4.3 A |
| 67401×14（典型は 1 個 100 mA と仮定。最大は review-digital.md 付録 A.1） | 約 1.4 A | 約 2.8 A |
| 合計 | **約 7.5 A** | **約 11.5 A** |

- 24 ピンの PAL は標準品だけで、1 個 最大 210 mA [D MMI85 p.5-31]。半電力品は 1985年の資料で見つけていない [V]。20 ピンの PAL は、16 MHz で動く TIM を除いて半電力の A-2 品にした。
- TTL の値は、各品種のデータブックの典型・最大を使った推定 [V]。
- Data Fabric だけで約 37 W（典型）の熱が B 基板で出る。board.md §06 の「本体の中で 35〜70 W、ファンが要る可能性が高い」は変わらず、上の側に寄る [V]。

## 11　未確定の点

OI-050 にまとめる。

1. 順序と状態の PAL（SEQ、ELG、AFC、CHS、STS、STB、HDEC、HACK、TIM、P1A、P2A、PRST）の式。エミュレータのスロットの模型と、スロットごとの判断の列が一致するかを、変換部と同じように Python などで突き合わせる。PAL の入力・出力・項が足りなければ、個数が増える。
   - DR-085 で、AFC と判断と状態（CST、ELG、SEQ、ELS、EVT、ADC、CHS、STS）の式を書き、門の模型で突き合わせた。残りは、アクセスの制御（NX・OP の制御、2 スロットのアクセス、時間切れ、port 1 の DONE の照合）と STB、HDEC、HACK、TIM、P1A、P2A、PRST。基板への影響は OI-082、判断の経路の時間は OI-083、門の模型と実機の差は OI-084。
   - DR-089 で残りの式を書き、門の模型で突き合わせた。残るのは、時間の予算が閉じないこと（OI-087）、CPU の側の要求の同期化（OI-088）、P2A の打切りとブリッジの置き場所（OI-089）。
   - DR-091 で時間の予算を閉じた（OI-087）。残るのは、余裕が 10 ns 以下の経路と配線・クロックの分配の遅れ、A ファイルの書込みの保持、B バスの引上げの立上り（03 の [V]。OI-090）、OI-088、OI-089。
2. スロットの中の時刻（03）：67401 の SI・SO の幅と満杯・空の信号の遅れ、Staging RAM の書込み、ACRTC の DACK、74S189 の書込みの時間を、AC 特性で確かめる。DR-091 の答えの預かりで、port 1 の読出しの値をブリッジが次のスロットの終わりまで保てるかも確かめる。
3. 74S189、74LS670、74LS534、74S283、63S281A、PAL の日立の同等品と、1985年の国内での入手性。DR-091 の 74ALS534（SL）と 74F377（OP0）も。
4. 24 ピンの細い DIP の面積の仮定（1.2 倍）。
5. 拡張のポート（11〜15）の幅、番地の有無、準備の信号の扱い。
