# ROM の作業の手引き（エージェントと人のため）

**v0.7**  
2026年11月27日

> このフォルダは fc85 のカートリッジ（ROM）を 1 本作るところ。`sdk/tools/new_rom.py` がひな形（`sdk/template/`）から作った。ここに書くのは作り方・試し方・守ることの要点で、正は SDK の文書（§06）。エージェント（Claude Code、ほかの道具）も人も、この順で作業する。

## 01　このフォルダ

| ファイル | 中身 |
|---|---|
| `main.c` か `main.s` | ROM の本体。`main.c` は C（`sdk/docs/c.md`）、`main.s` は 68000 のアセンブラ（m68kasm の書き方は `sdk/docs/bios-abi.md` §10）。見出し、入口、毎フレームの処理。`new_rom.py` の `--lang` で決まる |
| `build.py` | 組む。`build/rom.bin`（$400000 から、最大 1 MiB）と `build/rom.lst`（ラベルの表）。C なら `rom.elf`・`rom.map`・`rom.dis`（逆アセンブル）も。このフォルダに `.c` があれば C の ROM として、ここの `.c` と `.S`（GNU as）を全部組む |
| `test_rom.py` | 試験。組めるか、見出し、エミュレータで動かしてバスエラーがないか・入力に応えるか |
| `fc85-root` | リポジトリの根（`sdk/`・`emulator/`・`firmware/` があるところ）への道。根の中にこのフォルダがあれば要らない |
| `.mcp.json` | エミュレータの窓口を MCP サーバーとして使う設定（§03） |
| `build/` | 組んだ結果。リポジトリに入れない |

## 02　組む・試す

```
py build.py            # build/rom.bin と build/rom.lst
py -m unittest         # 試験
```

C の ROM には 68000 の GCC が要る（`sdk/docs/c.md` §02。Ubuntu・WSL なら `sudo apt install gcc-m68k-linux-gnu`。使う GCC は根で `py sdk/tools/m68kgcc.py --which`）。

試験でエミュレータを動かすには、根で一度だけ次を用意する（なければその試験は飛ばす）：

```
py firmware/charrom/build.py
py firmware/boot/build.py
cd emulator && cargo build --release -p fc85-api -p fc85-host
```

人が遊んで確かめるとき：`emulator/target/release/fc85 --cart build/rom.bin`（窓。矢印・Z・X・C・Enter）か、`emulator/target/release/fc85-api --http --cart build/rom.bin` で http://127.0.0.1:8085/ を開く（Web の GUI。予算、DOC のオシレータ、レジスタも見える）。

## 03　エミュレータで確かめる

エミュレータは同じ ROM と同じ入力の台本で同じ結果を返す（決定的）。窓口の手順は `sdk/docs/emulator-api.md` が正。

- **MCP**（エージェント）：`.mcp.json` の `fc85` サーバーの道具が手順と同じ名前で並ぶ。最初に `load`（`{"cart": "build/rom.bin"}`。横の `rom.lst` も読むので、番地をラベルで書ける）。組み直したら `load` し直す。
- **Python**（試験と台本）：`sdk/tools/fc85emu.py` の `Emulator`。`test_rom.py` が使い方の見本。
- **JSON の手順**（ほかの道具）：`fc85-api` の標準入出力に 1 行 1 つの要求。

よく使う手順：

| したいこと | 手順 |
|---|---|
| 進める | `run`（`frames`、`input` の台本 `[[フレーム, "RIGHT"], [フレーム, 0], …]`、`stop_on_bus_error`、`break`） |
| 画面を見る | `screenshot`（PNG。`area` は `active`＝256×192） |
| 音を聞く・測る | `audio`（WAV、ピーク、RMS）、`run` の `voices`（DOC のオシレータが鳴り始め・止まったとき）、`doc`（今のオシレータ） |
| 中を見る | `regs`、`peek`（`space` と番地かラベル）、`symbols`、`step` |
| 試行を速く回す | `save_state`・`load_state`（`slot` の名前） |
| 重さを見る | `run` の `budget`（CPU・ACRTC・Fabric の働いた割合と、いちばん重いフレーム） |

## 04　回し方

1. `main.c`（か `main.s`）を変える。
2. `py build.py`。誤りは行番号つきで出る。
3. `load` → `run`（入力の台本つき）→ `screenshot`・`audio`・`voices` で確かめる。`bus_errors`、`doc_rule_warnings`、`budget` を見る。
4. 確かめたことを `test_rom.py` の試験にする（画面の `screen_hash`、Main RAM の値、音のピークなど）。
5. 終える前に `py -m unittest` を通す。

止まったとき：`run` の `stop` が `halted`（二重バスフォルト）か `bus_error` なら、`last_bus_error` の番地とラベル、`regs` の PC を見る（C なら PC を `build/rom.dis` で引くと C の行が分かる）。例外で止まると BIOS は赤い画面にベクタ番号を出す（`sdk/docs/bios-abi.md` §03）。

## 05　守ること

詳しくは `sdk/docs/guarantees.md`（保証の手引き）。要点：

- **番地**：アプリの Main RAM は `APP_RAM`（$100800）から。スタックは `INITIAL_SSP`（$120000）から下へ。Staging RAM は $120000〜（64 KiB）。名前は C なら `fc85.h` と `bios.h`、アセンブラなら `fc85.inc` と `bios.inc` を使い、番地を数で書かない。
- **BIOS**：スーパバイザで `JSR` で呼ぶ。出力のほかのレジスタは保たれる。ACRTC を使う呼出しは再入できない。版 2 の呼出しを使うなら `BIOS_INFO_VERSION` を確かめる（`bios-abi.md` §05・§11）。
- **DOC の窓**：`MOVE.B` の 1 回の読みか書きだけで触る（DR-030）。読み書き一体の命令（CLR、BSET など）を使わない。
- **ACRTC**：FIFO は SR の WFR・RFR を見てから読み書きし、待ちの数に頼らない（DR-096）。描画は VBlank の直後にまとめる。
- **時間**：呼出しの時間、起動のフレーム数、命令の待ちの数に頼らない。時間が要るなら `VIDEO_LINE` か `bios_frame` で測る。
- **字**：文字 ROM の記号と一部の漢字の字形はまだそろっていない（描いていない字は市松模様）。
- **C**：浮動小数点・`malloc`・`printf` はない（整数と固定小数点で書く）。ACRTC・DOC の窓に `|=`・`&=` を使わない（値を作って `=` で書く）。割込みの関数は `fc85_set_vector` と `__attribute__((interrupt_handler))`、VBlank のフックはふつうの関数でよい。GCC が出す命令の時間に頼らない（`sdk/docs/c.md` §05〜§11）。ACRTC の図形と矩形の DMA・Data Fabric・DOC・ADPCM・パレットと HBlank の割込み・DSP・Temporal RAM は `acrtc.h`・`fabric.h`・`doc.h`・`adpcm.h`・`video.h`・`dsp.h`・`temporal.h` の関数を使うと、規約の手順を関数が守る（`sdk/docs/c.md` §09）。HBlank の割込みで替えた色は、比べたラスタの 2 つ後から効く。

## 06　読むもの

| 文書 | 中身 |
|---|---|
| `sdk/docs/c.md` | C で書く：GCC、メモリと起動、使えるもの、BIOS の関数（`sdk/c/include/bios.h`）、レジスタの扱い、ACRTC・Data Fabric・DOC・ADPCM の関数 |
| `sdk/docs/bios-abi.md` | BIOS の呼出し、メモリ、カートリッジの見出しと入口、m68kasm |
| `sdk/docs/guarantees.md` | ROM が守ること、頼ってよいこと・いけないこと |
| `sdk/docs/emulator-api.md` | エミュレータの窓口の手順 |
| `sdk/include/fc85.h`・`fc85.inc` | 番地とビットの名前（`docs/spec/registers.toml` から生成） |
| `docs/spec/hardware.md` | 機械の仕様（§05 メモリ、§06 Data Fabric、§07 クロックと映像、§11 ベンド） |
| `sdk/samples/*/README.md` | サンプル：game（SAMPLE 1：ACRTC の XOR の動く物、ADPCM の効果音、DOC か本体 DAC の音楽）、dsp（SAMPLE 2：DSP で音を加工）、patches（SAMPLE 3：実験パッチ）、video（SAMPLE 4：ACRTC の分割画面・ウィンドウ・ズーム・スクロール）、bend（SAMPLE 5：ベンド点の試験信号）、showcase（SAMPLE 6：ラスタのパレット、DSP と ACRTC の線画、ソフトのスプライト、漢字、描画の予算）、raster（SAMPLE 7：C で書いた HBlank のラスタのパレットと DIRECT。C の ROM の手本）、echo（SAMPLE 8：C で書いた Temporal RAM の残像。ACRTC の DMA と Fabric） |

## 07　しないこと

- SDK（`sdk/include/`、`sdk/c/`、`sdk/m68kasm/`）やエミュレータを、この ROM のために書き換えない。足りないものや食い違いは、リポジトリの課題（`docs/open-issues.md`）に挙げる。
- `build/` をリポジトリに入れない。`fc85-root`・`.mcp.json` に絶対の道が入っていたら、公開する前に直す。
- 製品名・ロゴ・UI の意匠を決めない（題は仮のもの）。
