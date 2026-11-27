# fc85

**v0.2**  
2026年11月25日

> 状態：作業中の版。分類記号の定義は [Fictional Machines 基本構想](https://github.com/gameguys-jp/fictional-machines) を参照。文書は [文書管理ルール](https://github.com/gameguys-jp/fictional-machines/blob/master/Documentation_Rules.md) に従う。

1985年に最終試作機まで完成したが、発売されなかった日本の家庭用多目的コンピュータ。その機械仕様、開発史、実装（エミュレータから始める）を作るプロジェクト。

*English: fc85 is a fictional Japanese home computer that reached its final prototype in 1985 but was never released. It is built only from parts that existed by 1985 (Hitachi HD68000 / HD63484 ACRTC, TI TMS32010 DSP, Ensoniq ES5503 DOC, OKI MSM5205, and others). This repository holds its specification, its fictional history, and research notes on the real parts. Documentation is in Japanese.*

## 基本情報

| 項目 | 内容 |
|---|---|
| 基本情報 | ID：fc85。名称：未確定。1985年に完成した未発売の家庭用多目的コンピュータ。用途：ゲーム、日本語情報処理、教育、音楽、映像制作、通信。開発状況：仕様と資料の段階（段階0）。コードはまだない |
| 設計境界 | F1／T2:1985（適合は未検証）／D5・D6・D7。1985年末までに入手できた実在の部品と技術に限る。家庭用としての採算は成立条件にしない。Given：実在の IC と1985年の製造技術。Designed：IC 間の接続回路（Data Fabric）、アドレスマップ、BIOS、A/V 経路 |
| 機械仕様 | Scope：Machine／System。L2〜L7／E3（目標）。異なる用途の実在 IC を独自の転送回路でつなぎ、映像・音響・文字・数値・信号を相互に扱う。詳細は [仕様書](docs/spec/hardware.md) |
| SF前提 | 物理法則・技術水準からの逸脱はない。架空なのはメーカー、開発史、ES5503 を試作用に入手できた経緯（[DR-006](docs/decisions.md)） |
| 実装・検証 | M1（PC エミュレータ）から始め、のちに実 IC・FPGA・PCB の混在へ。S2〜S4 重視。現状は MSM5205、TMS32010、ES5503、HD6850、HD68000（SingleStepTests との照合 99.4%）のコアと、HD63484 のコア（描画と表示）、Data Fabric のスロットの模型と、ACRTC の DMA・音声クロック・本体 DAC・ADPCM・DSP のアダプタ・DOC 子基板の port 2・68000 から DOC へのブリッジ、本体のバス（アドレスの解読、待ち、バスエラー、割込み）と映像の出力段が V2（単体試験）。段階 3（自作の Boot ROM とカートリッジで、入力・描画・文字 ROM・PCM/ADPCM の音声）はエミュレータの上で動く。目標は全体の V2（正常系のビット列一致）、その後 V3／V4 |
| フィクション | N3〜4。開発史と設計文化が部品構成と回路に表れる。詳細は [物語](docs/story/main.md) |
| 成果物 | 仕様書、部品台帳、物語資料、決定記録、課題台帳。エミュレータのコードとテスト（作業中） |

## 主な構成

| 区分 | 部品 |
|---|---|
| CPU | 日立 HD68000-8（8 MHz） |
| 描画 | 日立 HD63484-8 ACRTC（2CLK 5.369 MHz）、VRAM 128 KiB |
| 信号処理 | TI TMS32010（20 MHz） |
| 音源 | Ensoniq ES5503 DOC（子基板、8 MHz）、OKI MSM5205（ADPCM） |
| 接続 | Data Fabric（独自 DMA・FIFO。試作は PAL＋TTL） |
| 映像 | 256×192・4 bpp、ノンインタレース 262 ライン・約 59.92 Hz |

## 文書

| 種類 | 場所 |
|---|---|
| 仕様 | [docs/spec/hardware.md](docs/spec/hardware.md)、基板と外形の見積り [docs/spec/board.md](docs/spec/board.md)、Data Fabric の回路と部品表 [docs/spec/fabric.md](docs/spec/fabric.md) |
| 物語 | [docs/story/](docs/story/)：[本筋](docs/story/main.md)、[メーカーの出自](docs/story/company-origin.md)、[音源の差し替え](docs/story/sound-redesign.md)、[設計方式の由来](docs/story/architecture.md) |
| 部品 | [docs/parts/](docs/parts/)：IC ごとの台帳、1985年の供給調査、部品の見直し |
| デザイン | [docs/design/](docs/design/)：[書体（文字 ROM の字形）](docs/design/main.md)、[筐体の外観の大枠](docs/design/enclosure.md)（[仮]）。製品名・ロゴ・UI・意匠の規則は未確定 |
| 実装 | [emulator/](emulator/)（Rust。段階 1 の MSM5205、TMS32010、ES5503 と、HD63484 の描画コマンドまで。段階 2 の Data Fabric のスロットの模型と、ACRTC の DMA（port 1）、音声クロック・本体 DAC・ADPCM・DSP のアダプタ、DOC 子基板の port 2。段階 3 の HD68000、本体のバス、68000 から DOC のレジスタへのブリッジ、ACRTC の表示と映像の出力段、ACIA。ホスト（窓・音声・キーボード）は `cargo run --release -p fc85-host`）。Boot ROM（BIOS 版 1 と内蔵のデモ）と文字 ROM は [firmware/](firmware/)、68000 と TMS32010 のアセンブラと BIOS の ABI は [sdk/](sdk/)（[sdk/docs/bios-abi.md](sdk/docs/bios-abi.md)）。段階 3 のサンプルのカートリッジ（ゲームと ADPCM の効果音）は [sdk/samples/game/](sdk/samples/game/)、DSP をソフトから動かすサンプル（低域通過とエコー）は [sdk/samples/dsp/](sdk/samples/dsp/)。設計と仕様との差は [docs/impl/emulator.md](docs/impl/emulator.md) |
| 決定記録 | [docs/decisions.md](docs/decisions.md) |
| 課題台帳 | [docs/open-issues.md](docs/open-issues.md) |

## 動かし方

Rust（1.88 以降。edition 2024 と let-chains を使う）と Python 3 が要ります。ROM のイメージはリポジトリに入れていないので、先に組みます。

```
py firmware/charrom/build.py
py firmware/boot/build.py
py sdk/samples/game/build.py
py sdk/samples/dsp/build.py
py sdk/samples/patches/build.py
py sdk/samples/video/build.py
py sdk/samples/bend/build.py
py sdk/samples/showcase/build.py
py sdk/samples/raster/build.py      # C のサンプル。68000 の GCC が要る（sdk/docs/c.md §02）
py sdk/samples/echo/build.py        # 同じく C
py sdk/samples/midi/build.py        # 同じく C
cd emulator
cargo run --release -p fc85-host                                              # Boot ROM の内蔵のデモ
cargo run --release -p fc85-host -- --cart ../sdk/samples/game/build/game.bin  # サンプルのカートリッジ
cargo run --release -p fc85-host -- --cart ../sdk/samples/dsp/build/dsp.bin    # DSP のサンプル（A・B・C で切替え）
cargo run --release -p fc85-host -- --cart ../sdk/samples/patches/build/patches.bin  # 実験パッチ（左右でページ）
cargo run --release -p fc85-host -- --cart ../sdk/samples/video/build/video.bin      # ACRTC の表示（分割画面・ウィンドウ・ズーム）
cargo run --release -p fc85-host -- --cart ../sdk/samples/bend/build/bend.bin        # ベンド点を試す試験信号（F2・F3 で Bend）
cargo run --release -p fc85-host -- --cart ../sdk/samples/showcase/build/showcase.bin  # 映像と描画の予算（ラスタのパレット・DSP の立体・玉）
cargo run --release -p fc85-host -- --cart ../sdk/samples/raster/build/raster.bin      # C で書いたラスタの帯（A で DIRECT、B で音）
cargo run --release -p fc85-host -- --cart ../sdk/samples/echo/build/echo.bin          # C で書いた Temporal RAM の残像（上下で遅れ、A で HOLD）
cargo run --release -p fc85-host -- --cart ../sdk/samples/midi/build/midi.bin --midi-in 0  # C で書いた MIDI で弾く DOC のシンセ（A で弾く、C で音色）
```

| キー | 働き |
|---|---|
| 矢印 | 方向（PORT_A の上・下・左・右） |
| Z・X・C | A・B・C |
| Enter | START |
| F5・F9 | 状態の保存・復元 |
| Esc | 終わり |

`--no-doc` で DOC 子基板なしの構成、`--help` でほかの起動オプション（外部クロックの Bend、MIDI、窓なしの実行など）。詳しくは [docs/impl/emulator.md](docs/impl/emulator.md) §02。

## ライセンス

ソフトウェアと文書は [MIT](LICENSE)、ハードウェアの設計データ（PCB、FPGA の HDL、機構）は CERN-OHL-P v2（`hardware/` を作るときに `hardware/LICENSE` を置きます）です。外部のコードは元のライセンスのままで、GPL のコアを組み込んだ成果物は GPL に従います（[DR-045](docs/decisions.md)）。
