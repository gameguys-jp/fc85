# エミュレータの窓口

**v0.2**  
2026年11月27日

> 状態：作業中の版。エミュレータを外から動かす手順（JSON の手順と MCP サーバー）と、ほかのソフトに組み込む口（C の ABI、WebAssembly、libretro。§08）を書く。仮想機の挙動は [仕様書](../../docs/spec/hardware.md) が正で、この窓口は仮想機を変えない。方針は DR-092 (4)・(5)、この版の形は DR-097・DR-104。エージェントが決めた（ユーザーが方向を承認。細部は要見直し）。

## 01　概要

| 項目 | 内容 |
|---|---|
| コード | `emulator/crates/api`（crate `fc85-api`）。`Machine`（仮想機と、外から見る道具）、`protocol`（JSON の手順）、`mcp`（MCP サーバー） |
| 実行ファイル | `fc85-api`。標準入力から 1 行に 1 つの要求を読み、標準出力に 1 行の答えを書く。知らせは標準エラー |
| 動かし方 | `emulator/` で `cargo run --release -p fc85-api -- [--mcp | --http] [--cart cart.bin]`。`--mcp` を付けると MCP サーバー、`--http` で Web の GUI（§06）、付けなければ JSON の手順 |
| ROM | `load` の `boot`・`char` を省くと、リポジトリの `firmware/*/build/` のビルド結果を読む（先に `py firmware/boot/build.py` と `py firmware/charrom/build.py`）。`--root DIR` は探す起点 |
| Python | `sdk/tools/fc85emu.py` の `Emulator` が窓口のプログラムを子のプロセスで動かし、`call("run", frames=60)` のように手順を呼ぶ。ROM の試験の見本は `sdk/template/test_rom.py`（DR-102） |
| 組込み | 子のプロセスでなく、同じプロセスの中で仮想機を回すときは §08（C の ABI、WebAssembly、libretro。DR-104） |
| 決定的 | 同じ ROM・同じ設定・同じ要求の列なら、同じ結果になる。`run` は、止める条件に当たらなければ、ホスト（`fc85`）で同じフレーム数を同じ入力で進めたときと同じ状態になる（試験 `run_ends_in_the_same_state_as_the_host_frame_loop`） |
| 版 | 手順の版は `info` の `api_version`（今は 0.1）。答えに項目を足すだけなら小さい方の数を上げ、要求や答えの形を変えたら大きい方を上げる |

## 02　JSON の手順

1 行に 1 つの JSON（JSON Lines）。形は JSON-RPC 2.0 にそろえる。

```
→ {"id": 1, "method": "run", "params": {"frames": 60, "pad": "START"}}
← {"jsonrpc": "2.0", "id": 1, "result": {"stop": "frames", "frame": 60, …}}
← {"jsonrpc": "2.0", "id": 2, "error": {"code": -32000, "message": "ラベル main がない（…）"}}
```

- 誤りの code：-32700 は JSON を読めない、-32600 は method がない、-32000 は手順の誤り（引数、番地、ファイル）。
- 番地は、数か文字列（`"$400000"`、`"0x400000"`、10 進、ラベル、`"ラベル+4"`）。ラベルは m68kasm のリスト（`.lst`）の末尾の表から読む。
- ファイルの道は、`fc85-api` を動かしているフォルダから。
- 画面（`screenshot` で `path` なし）は、答えの `png_base64` に入る。
- 最初に仮想機を使う要求で、既定の ROM で電源を入れる（`load` を先に送らなくてもよい）。

## 03　手順

| 手順 | 引数 | 答え |
|---|---|---|
| `info` | なし | `api_version`、`emulator_version`、`bios_version`（$000480 の語。DR-094）、`cart_bytes`、`doc_board`、`symbols`（ラベルの数）、`frame`、`cpu_clock`、`mode`（normal・bend）、`bends`、`tv`、`audio_rate` |
| `methods` | なし | 手順の一覧と、引数の JSON Schema |
| `load` | `cart`、`boot`、`char`（道）、`listing`（足して読むリスト）、`doc`（子基板。既定 true）、`bends`（ベンド点の命令の列）、`audio_tap`（`line`・`digital`） | 電源を入れ直し、`info` と、読んだリスト。カートリッジと Boot ROM の横の `.lst` は自動で読む |
| `reset` | なし | 同じ ROM と設定で電源を入れ直す（ラベルは残す） |
| `run` | `frames`（既定 1、最大 36,000）、`pad`（PORT_A を最初から）、`input`（台本 `[[フレーム, 値], …]`。フレームはこの実行の何フレーム目か。次の行まで保つ）、`port_b`、`break`（番地の列。その命令の前で止める）、`idle`（待ちのループ。番地か `[from, to)` の列）、`stop_on_bus_error`、`run_when_halted`、`voices`（DOC の発音の記録。§04）、`watch`（メモリの見張り。番地かラベルなら CPU の空間の 2 byte、`{"space", "addr", "len"}` で空間と長さ。合計 4,096 byte まで）、`trace`（最後に実行した命令をいくつ覚えるか。最大 4,096） | §04 |
| `step` | `count`（既定 1） | 命令を実行し、`regs` と同じ形 |
| `regs` | なし | `d`、`a`（A7 は今の SP）、`usp`、`ssp`、`sr`、`pc`、`pc_label`、`state`（running・stopped・halted）、`clock`、`frame` |
| `doc` | なし | DOC の今の様子：`enabled`（有効なオシレータの数 N）と `oscillators`（0〜N−1 の `on`、`freq`、`vol`、`wave_ptr`、`wave_size`、`control`）。子基板がなければ null。読んでも DOC は変わらない |
| `screenshot` | `path`、`area`（`active`：表示の 256×192、`view`：320×240、`raster`：342×262）、`scale`（1〜8。既定は `path` があれば 1、なければ 2） | `width`、`height`、`hash`、`frame`。`path` があれば PNG を書き、なければ PNG を答えに付ける |
| `audio` | `path`（WAV）、`keep` | 溜めた音（最大 60 秒。超えると古い方を捨てる）の `samples`、`seconds`、`peak`（L・R）、`rms`。`path` があれば 48 kHz・16 bit・ステレオの WAV に書く。`keep` が false なら取り出した分を捨てる |
| `peek` | `space`、`addr`、`len`（最大 65,536）、`words` | `hex`（大文字の 16 進）。`words` なら big-endian の語の列も |
| `poke` | `space`、`addr`、`hex` | 書いた byte の数 |
| `bus_read` | `addr`、`size`（`word`・`byte`） | `value` |
| `bus_write` | `addr`、`value`、`size` | — |
| `save_state` | `slot`（この過程の中の名前）か `path`（JSON のファイル） | `frame` |
| `load_state` | `slot` か `path` | `frame`、`pc` |
| `bend` | `cmd`（仕様書 §11 のベンド点の命令。`mode bend`、`clock video 18000000` など） | `summary`、`mode` |
| `symbols` | `prefix`・`limit`、または `addr` | ラベルの一覧、または番地の名前 |

`peek`・`poke` の空間：

| 空間 | 中身 |
|---|---|
| `cpu` | CPU から見た番地のうちメモリの窓：Boot ROM（$000000〜）、Main RAM（$100000〜）、Staging RAM（$120000〜）、カートリッジ（$400000〜）。I/O の番地は誤りになるので、`bus_read`・`bus_write` を使う |
| `vram` | ACRTC の VRAM（128 KiB。語は big-endian） |
| `wave` | DOC の波形 RAM（64 KiB） |
| `dsp_prog` | DSP のプログラム RAM（4K 語） |
| `temporal` | Temporal RAM |
| `char_rom` | 文字 ROM |
| `palette` | いま表示に使っているパレットの 16 語（読むだけ） |

`peek`・`poke` はバスを通さないので、時刻も状態も変えない（ROM も書き換えられる。試験の道具）。`bus_read`・`bus_write` は CPU と同じバスサイクルを今の CPU の時刻で起こすので、I/O の副作用（FIFO を進める、W1C のビットを消す）が起き、バスエラーは記録に数える。

## 04　run の答え

| 項目 | 内容 |
|---|---|
| `stop` | 止まった理由：`frames`（数え終えた）、`breakpoint`、`halted`（68000 の二重バスフォルト。`run_when_halted` で続けられる）、`bus_error`（`stop_on_bus_error`）、`watch`（見張りの中身が変わった） |
| `frames_run` | 最後まで進めたフレームの数 |
| `frame`、`cpu_clock`、`pc`、`pc_label` | 止まったときの値。`frame` は電源投入からの ACRTC のフレームの数 |
| `bus_errors`、`last_bus_error`、`bridge_timeouts`、`doc_rule_warnings`、`acrtc_rule_warnings` | この実行の間の数。`last_bus_error` は番地、ラベル、書込みか。`acrtc_rule_warnings` は ACRTC の FIFO の規約（DR-096）に反したアクセス（満杯の書込み FIFO への書込み、空の読出し FIFO の読出し）の数（DR-121） |
| `budget` | 働いた割合（0〜1）の全体の値と、いちばん重いフレームの値（`_max`）：`cpu_busy`、`acrtc_busy`、`fabric_busy` |
| `audio` | この実行の音の `samples`、`seconds`、`peak`、`rms` |
| `screen_hash` | 表示の範囲（256×192）の画素の FNV-1a（64 bit）。同じ画面かを比べる（暗号の強さはない） |
| `tv` | テレビの模型の状態（同期と色が取れているか。映像のクロックのベンド点） |
| `voices`、`voices_dropped` | `voices` を付けたときだけ。DOC のオシレータが鳴り始めた・止まった出来事の列（`frame`、`cpu_clock`、`osc`、`on`、そのときの `freq`、`vol`、`wave_ptr`、`wave_size`、`control`）。最大 2,000 件で、超えた数は `voices_dropped` |

| `watch_hit` | 見張りで止まったときだけ。`space`、`watch_addr`（見張りの頭）、`addr` と `label`（変わった最初の byte）、`old`・`new`（見張りの範囲の前と後。16 進）、`pc` と `pc_label`（変わったのを見つけた命令） |
| `trace` | `trace` を付けたときだけ。最後に実行した命令の `pc`、`label`、`cpu_clock`（古い順。STOP で待つ間は数えない） |

見張りの見方 [P]：命令を 1 つ実行するごとに、見張りの範囲をバスを通さずに読み、前と比べる。同じ値を書いた書込みと、読むだけのアクセスでは止まらない。Data Fabric・ACRTC・DSP が書いたときも、次の命令の後で見つかるので、`pc` はその命令になる（`trace` の最後の命令と同じ）。止まった後に続けて `run` を送ると、今の中身から見張り直す。

発音の記録の見方 [P]：基板を CPU に追いつかせる区切り（64 クロック＝8 µs）ごとに、有効なオシレータの制御のレジスタの停止ビットを見て、変わったものを記録する。8 µs より短く鳴って止まった音と、止めずにレジスタだけ替えた音（同じオシレータで音程を変える）は出来事にならない。記録を取っても仮想機の状態は変わらない（試験で確かめている）。

予算の数え方 [P]：

- `cpu_busy`：68000 が STOP で割込みを待つ間と、`idle` で示した番地の命令の時間を空きとして数え、残りを働いた時間とする。BIOS の `BIOS_WAIT_VBL` は STOP で待つので、何も示さなくても空きに入る。自分で VBlank のフラグを回して待つループは、`idle` で示さないと働いた時間に入る。
- `acrtc_busy`：ACRTC がコマンドを実行している（`Hd63484::idle` でない）時間。命令の切れ目ごとに見る。
- `fabric_busy`：Data Fabric のどれかのチャネルが動いている時間。

ブレークポイント：番地の命令を実行する前に止める。続けて `run` を送ると、止まった番地の最初の 1 命令は止めずに進む。止まった後は基板を追いつかせる区切りがずれるので、止めずに進めたときとクロック単位で同じ結果になるとは限らない（同じ要求の列なら同じ結果になる）。

## 05　MCP サーバー

`fc85-api --mcp` は MCP（Model Context Protocol）のサーバーで、§03 の手順をそのままツールとして出す（ツールの名前は手順の名前、引数は `params`）。標準入出力の JSON-RPC 2.0 で、`initialize`、`ping`、`tools/list`、`tools/call` に答える。`screenshot` で `path` を省くと、PNG を image の内容で返すので、エージェントが画面を見られる。ツールの誤りは `isError` で返す。

Claude Code に登録する例（リポジトリの根の `.mcp.json`。先に `cargo build --release -p fc85-api` で作っておく）：

```json
{
  "mcpServers": {
    "fc85": {
      "command": "emulator/target/release/fc85-api",
      "args": ["--mcp", "--root", "."]
    }
  }
}
```

## 06　Web の GUI

`fc85-api --http [ADDR]`（既定は 127.0.0.1:8085）で、ブラウザから人が遊び、調べる画面を出す（DR-092 (6)、DR-101）。仮想機は手元のプログラムが回し、ブラウザには画面と音だけを送る。

```
cargo run --release -p fc85-api -- --http --cart ../sdk/samples/game/build/game.bin
→ http://127.0.0.1:8085/ を開く
```

| 項目 | 内容 |
|---|---|
| 操作 | 矢印・Z（A）・X（B）・C（C）・Enter（START）が釦（ホストの `fc85` と同じ）。ゲームパッド（Gamepad API の標準の配置。十字か左の棒、下の釦が A、右が B、左が C、START）も、フレームを頼むたびに読んでキーと OR にする（DR-125）。動かす・止める、1 フレーム、リセット、音（ブラウザの決まりで、押してから鳴る）、画面の保存（PNG）、カートリッジを読む、状態の保存と復元（枠 `gui`）、Bend モード、ベンド点の命令 |
| 見るもの | フレーム、速さ、PC とラベル、モード、バスエラー（数と最後の番地）、テレビの模型の状態、1 フレームの予算（CPU・ACRTC・Fabric）、DOC の 32 のオシレータ（鳴っているか、音量。ますに指を置くとレジスタ）、68000 のレジスタ（20 フレームごと） |
| 速さ | ページが時刻から進めるフレームの数を決め、1 回に最大 3 フレームを頼む。遅れが 6 フレームを超えたら飛ばす。音はブラウザの WebAudio に 0.06 秒先から並べ、0.3 秒より先にたまったら捨てる（ホストの安全層と同じく、仮想機は変えない） |
| 口 | `GET /`（ページ。プログラムに埋め込み）、`POST /api`（本文は `{"method", "params"}`。答えは §02 と同じ）、`POST /frame`（GUI の 1 歩。本文は `{"pad", "frames"}`。`run` の答え、表示の範囲の PNG、溜まった音（16 bit・ステレオ・リトルエンディアン）を base64 で返し、`mode` と `doc`（`doc` の手順と同じ）を添える） |
| 守り | 手元の番地（127.0.0.1 か ::1）でだけ待つ。Host の見出しが待っている口の名前（127.0.0.1・localhost・[::1]）でなければ 403、Origin が付いていて同じ口でなければ 403（ほかのサイトのページや、DNS の付け替えから使われないように）。`/api` はファイルを読み書きする手順（`load`、`screenshot` の `path` など）も通すので、手元の人だけが使う前提 |

## 07　これから足すもの

DR-092 (5) のうち、この版にないもの [O]：I/O のレジスタの読み書きの記録（どの命令がどのレジスタに何を書いたか）、レジスタ表の 1 ファイル（DR-092 (5)）からの名前。Web の GUI の残り：メモリの表示、ブレークポイント、波形の表示（ゲームパッドは DR-125 で足した）。C の ABI・WebAssembly・libretro（DR-092 (4)）は v0.2 で足した（§08。DR-104）。

## 08　組み込む（C の ABI・WebAssembly・libretro）

ほかのソフトの同じプロセスの中で仮想機を回す口（DR-092 (4)、DR-104）。どれも窓口の `Machine` の上の薄い層で、仮想機を変えない。1 フレームの進め方はホスト（`Console::run_frame`）と同じで、68000 が止まって（halt）もフレーム・画面・音は進む。同じ ROM・同じ設定・同じ入力の列なら、C の ABI と WebAssembly で画面・音・状態がビットまで同じになる（試験 `emulator/embed/test_embed.py`）[P]。

| 口 | 置き場所 | 組み方 |
|---|---|---|
| C の ABI | crate `fc85-capi`（`emulator/crates/capi`）。見出し `emulator/crates/capi/include/fc85emu.h`（ROM の中で使う `fc85.h` とは別物） | `emulator/` で `cargo build --release -p fc85-capi`（か `py emulator/embed/build.py --native`）。`target/release/` に共有ライブラリ（`fc85emu.dll`・`libfc85emu.so`・`libfc85emu.dylib`）と静的なライブラリ（`fc85emu.lib`・`libfc85emu.a`） |
| WebAssembly | 同じ crate を wasm32-unknown-unknown に組む。糊 `emulator/embed/fc85emu.js`、試しの頁 `emulator/embed/index.html`（Web Worker `worker.js`、AudioWorklet `audio.js`。DR-107） | `py emulator/embed/build.py`（`emulator/embed/build/fc85emu.wasm`。Rust の `wasm32-unknown-unknown` の target が要る。binaryen の wasm-opt が PATH か環境変数 `WASM_OPT` にあれば -O3 をかける。`--wasm-opt PATH`・`--no-wasm-opt`。DR-108。Boot ROM・文字 ROM・組んであるサンプルも `build/` に写す。写すだけなら `--roms`。DR-115）。頁は `py emulator/embed/serve.py` で配り、http://127.0.0.1:8000/ を開く |
| libretro | crate `fc85-libretro`（`emulator/crates/libretro`） | `cargo build --release -p fc85-libretro`（か `build.py --libretro`）。`fc85_libretro.dll`・`.so`・`.dylib` をフロントエンド（RetroArch など）のコアのフォルダに置く |

### C の ABI

| 関数 | すること |
|---|---|
| `fc85_new(boot, boot_len, chars, chars_len, flags)` | 仮想機を作って電源を入れる。ROM は NULL でもよい（Boot ROM がなければ 68000 はすぐ止まる）。`flags` は `FC85_NO_DOC`（DOC の子基板を挿さない）・`FC85_DIGITAL_AUDIO`（音をチップの出力のデジタルの和にする。既定は LINE OUT）。`fc85_free` で壊す |
| `fc85_load_cart`・`fc85_reset` | カートリッジを挿して（長さ 0 で抜いて）電源を入れ直す。同じ ROM で入れ直す |
| `fc85_set_input(m, port_a, port_b)` | PORT_A・PORT_B（`FC85_PAD_*` の OR。上 $01、下 $02、左 $04、右 $08、A $10、B $20、C $40、START $80） |
| `fc85_run_frames(m, n)`・`fc85_frame` | n フレーム進める。電源投入からのフレームの数 |
| `fc85_screen(m, area, out, out_len)` | 画面を 0x00RRGGBB で左上から行ごとに書き、画素の数を返す。`area` は `FC85_AREA_ACTIVE`（256×192）・`VIEW`（320×240）・`RASTER`（342×262）。大きさは `fc85_screen_width`・`fc85_screen_height` |
| `fc85_screen_rgba(m, area, out, out_len)` | 同じ画面を R・G・B・A（A は 255）の byte の順で書く（ブラウザの ImageData の形）。`out_len` は byte の数（画素の数の 4 倍以上）。画素の数を返す（DR-107） |
| `fc85_audio(m, out, max_frames)` | 溜まった音を L・R の順の s16 で書き、組の数を返す（48 kHz、1 フレームに約 801 組。残りは次に回す）。取り出さないと 60 秒分まで溜め、古い方から捨てる |
| `fc85_save_state`・`fc85_load_state` | 状態を保存して返す（`fc85_free_bytes` で返す）・戻す。形は印 `FC85ST01` と、`save_state` の JSON を deflate したもの。同じ版のエミュレータで読めることだけを約束する [P]（OI-103） |
| `fc85_peek`・`fc85_poke` | メモリをバスを通さずに読む・書く。空間は `FC85_SPACE_*`（§03 の表と同じ順：CPU、VRAM、WAVE、DSP_PROG、TEMPORAL、CHAR_ROM、PALETTE） |
| `fc85_call(m, request)` | §02 の JSON の手順の要求を 1 行、同じ仮想機に送り、答えの行を返す（`fc85_free_string` で返す）。ブレークポイント、見張り、ベンド点、レジスタなど、ほかの手順はこちらで使う |
| `fc85_abi_version`・`fc85_version`・`fc85_audio_rate`・`fc85_frame_rate`・`fc85_last_error` | ABI の版（今は 1）、エミュレータの版、48000、約 59.92、最後の誤りの文 |

約束 [P]：

- 誤りは戻り値（-1 か NULL）で知らせ、文は `fc85_last_error()`（UTF-8。同じスレッドの次の呼出しまで使える）。中で Rust が panic しても、ネイティブでは誤りとして返し、呼んだソフトを止めない。
- 1 つの仮想機を同時に 2 つのスレッドから使わない（別々の仮想機なら別のスレッドでよい）。
- 渡したバイト列は関数の中で写す。返された領域は対の関数で返す（`free()` では返さない）。
- ABI の版は、関数の形を変えたら上げる。関数を足すだけなら変えない。

### WebAssembly

C の ABI と同じ関数を出し、取り込む関数はない（どの WebAssembly の実行環境でも読める）。大きさは約 1.5 MB。`fc85emu.js`（ES のモジュール。ブラウザでも Node でも動く）が、領域の受け渡しを包む：

```js
import { load, AREA, PAD } from "./fc85emu.js";
const fc85 = await load("build/fc85emu.wasm");
const m = fc85.create({ boot, chars });          // Uint8Array
m.loadCart(cart);
m.setInput(PAD.RIGHT | PAD.A);
m.runFrames(1);
ctx.putImageData(new ImageData(m.screenRGBA(AREA.VIEW), 320, 240), 0, 0);
const pcm = m.audio();                           // Int16Array（L・R、48 kHz）
const st = m.saveState(); m.loadState(st);
m.call("regs");                                  // JSON の手順
```

- 誤りは Error を投げる。WebAssembly の中の panic は止まる（RuntimeError）ので、その仮想機は捨てる。
- WebAssembly にはファイルがないので、`fc85_call` の手順のうち `path` を使うもの（`load`、`screenshot` の `path` など）は誤りになる。ROM はバイト列で渡す。
- 速さ [V]：開発機の Chrome の Worker の中で 1 フレーム約 13〜18 ms（ネイティブの release は約 10 ms。DR-105、DR-107、DR-108）。wasm-opt の -O3 で約 1 割速くなる（振る舞いは同じ）。読み込んでから約 10 秒は、ブラウザが機械語を組み直す前で遅い。Boot ROM と SAMPLE 1 は実時間（16.7 ms）に入るが、描画や DSP の重い ROM（SAMPLE 6、SAMPLE 2）は実時間の 9 割台で、音が途切れる（OI-103）。
- 試しの頁（DR-107）：仮想機は module の Web Worker（`worker.js`）で回し、頁の主のスレッドは釦と描画だけにする（OffscreenCanvas が使えれば描画も Worker）。音は AudioWorklet（`audio.js`）で鳴らし、進める速さは音の時計に合わせる（鳴っていない音が約 50〜167 ms になるようにフレームを進める。仮想機のフレームは約 59.92 Hz で、画面の 60 Hz とは合わせない）。音が鳴る前と AudioWorklet がないときは時刻に合わせる。実時間に追いつかない機械では実時間より遅く進み、音は途切れる（音の高さは変えない）。1 フレームより短い釦の押下も、次のフレームでは押したことにする。状態の行に、実時間の割合、仮想機に使った時間の割合、音の溜まり、途切れの数を出す。「速さを測る」は、今の場面（動かしている仮想機の状態を写す。まだ動かしていなければ電源投入）から、同じ ROM の別の仮想機を 240 フレーム止まらずに進めた 1 フレームの時間（DR-108）。
- 試しの頁は `build/` の Boot ROM・文字 ROM（`build.py` が firmware の組立ての結果を写す）を読み、なければ欄で選ぶ。カートリッジは「サンプル」の欄（`build.py` が、組んであるサンプル `sdk/samples/<名前>/build/<名前>.bin` を `build/carts/` に写し、見出しの題を一覧 `build/carts.json` に書く。サンプルだけを組み直したら `build.py --roms`。選び直すとその場で読み込み直す。DR-115）か、ファイルの欄で選ぶか、頁の URL に `?sample=<名前>`（例 `?sample=echo`）か `?cart=<URL>`（頁からの相対。例 `?cart=build/carts/game.bin`）を付ける。`?doc=0` で DOC の子基板を外す。釦はホストと同じ（矢印、Z・X・C、Enter）。ゲームパッド（Gamepad API の標準の配置）も読む：十字か左の棒、下の釦が A、右が B、左が C、START。つないだら一度釦を押す（DR-116）。「ベンド」の欄には仕様書 §11 のベンド点の命令を、ホストの `--bend` と同じ書き方で打つ（例 `mode bend`、`clock doc 4000000`）。Worker が JSON の手順の `bend`（§03）を C の ABI の `fc85_call` で呼び、答えの要約を頁に出す。読み込み直すと通常に戻る（DR-124）。`py -m http.server` でも配れるが、Windows では .js を text/plain で返すことがあり、ブラウザがモジュールや Worker を読まないので `serve.py` を使う。

### libretro のコア

| 項目 | 内容 |
|---|---|
| ROM | フロントエンドの system のフォルダに `fc85_boot.bin`（Boot ROM）と `fc85_char.bin`（文字 ROM）を置く（firmware の `boot.bin`・`char.bin` を名前を変えて）。Boot ROM がなければ読み込みに失敗し、画面に知らせる。ゲームはカートリッジの `.bin`。ゲームなしで始めると Boot ROM だけで動く |
| ベンド点 | カートリッジの横に同じ名前の `.bends` があれば、1 行に 1 つのベンド点の命令（仕様書 §11。`#` から後ろは注。ホストの `--bend` と同じ形）を電源を入れるときに入れる。工場出荷と違えば Bend モードで始まる。読めない命令があれば読み込みに失敗する |
| 入力 | RetroPad の十字、Y → A、B → B、A → C、START → START [P]。ポート 1 は PORT_A、ポート 2 は PORT_B |
| 映像 | XRGB8888。範囲は選択肢 `fc85_area`（view 320×240、active 256×192、raster 342×262）で、変えると次のフレームから。画素の縦横比は 8:7（約 1.14。ドットは 2CLK、NTSC のテレビ。`docs/parts/HD63484.md`）をフロントエンドに知らせる [P]（ホストと Web の GUI は補正しない。DR-055 (2)） |
| 音 | 48 kHz のステレオ。選択肢 `fc85_doc`（DOC の子基板）と `fc85_audio_tap`（line・digital）は、リセットか読み直しで効く |
| 状態 | 頭に長さ（4 byte）を付けた `pack_state` の形。`retro_serialize_size` は読み込んだときに、大きなメモリを擬似乱数で埋めて詰めた大きさに余裕を足して決め、ゲームの間は変えない（約 1 MB。中身は約 135 KB）[P] |
| メモリ | Main RAM（128 KiB）を `RETRO_MEMORY_SYSTEM_RAM` で見せる。状態を戻しても、リセットしても同じ番地 |
| 守り | 中で panic したら、そのゲームを止めて記録に書き、フロントエンドは止めない |
| まだないもの [O] | 電池で保つ RAM（本機にない）、チート、キーボード・MIDI、ベンド点の選択肢（`.bends` だけ）、実際のフロントエンドでの確かめ（OI-103） |
