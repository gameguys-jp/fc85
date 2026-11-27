# エミュレータの窓口

**v0.1**  
2026年11月27日

> 状態：作業中の版。エミュレータを外から動かす手順（JSON の手順と MCP サーバー）を書く。仮想機の挙動は [仕様書](../../docs/spec/hardware.md) が正で、この窓口は仮想機を変えない。方針は DR-092 (4)・(5)、この版の形は DR-097。エージェントが決めた（ユーザーが方向を承認。細部は要見直し）。

## 01　概要

| 項目 | 内容 |
|---|---|
| コード | `emulator/crates/api`（crate `fc85-api`）。`Machine`（仮想機と、外から見る道具）、`protocol`（JSON の手順）、`mcp`（MCP サーバー） |
| 実行ファイル | `fc85-api`。標準入力から 1 行に 1 つの要求を読み、標準出力に 1 行の答えを書く。知らせは標準エラー |
| 動かし方 | `emulator/` で `cargo run --release -p fc85-api -- [--mcp | --http] [--cart cart.bin]`。`--mcp` を付けると MCP サーバー、`--http` で Web の GUI（§06）、付けなければ JSON の手順 |
| ROM | `load` の `boot`・`char` を省くと、リポジトリの `firmware/*/build/` のビルド結果を読む（先に `py firmware/boot/build.py` と `py firmware/charrom/build.py`）。`--root DIR` は探す起点 |
| Python | `sdk/tools/fc85emu.py` の `Emulator` が窓口のプログラムを子のプロセスで動かし、`call("run", frames=60)` のように手順を呼ぶ。ROM の試験の見本は `sdk/template/test_rom.py`（DR-102） |
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
| `run` | `frames`（既定 1、最大 36,000）、`pad`（PORT_A を最初から）、`input`（台本 `[[フレーム, 値], …]`。フレームはこの実行の何フレーム目か。次の行まで保つ）、`port_b`、`break`（番地の列。その命令の前で止める）、`idle`（待ちのループ。番地か `[from, to)` の列）、`stop_on_bus_error`、`run_when_halted`、`voices`（DOC の発音の記録。§04） | §04 |
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
| `stop` | 止まった理由：`frames`（数え終えた）、`breakpoint`、`halted`（68000 の二重バスフォルト。`run_when_halted` で続けられる）、`bus_error`（`stop_on_bus_error`） |
| `frames_run` | 最後まで進めたフレームの数 |
| `frame`、`cpu_clock`、`pc`、`pc_label` | 止まったときの値。`frame` は電源投入からの ACRTC のフレームの数 |
| `bus_errors`、`last_bus_error`、`bridge_timeouts`、`doc_rule_warnings` | この実行の間の数。`last_bus_error` は番地、ラベル、書込みか |
| `budget` | 働いた割合（0〜1）の全体の値と、いちばん重いフレームの値（`_max`）：`cpu_busy`、`acrtc_busy`、`fabric_busy` |
| `audio` | この実行の音の `samples`、`seconds`、`peak`、`rms` |
| `screen_hash` | 表示の範囲（256×192）の画素の FNV-1a（64 bit）。同じ画面かを比べる（暗号の強さはない） |
| `tv` | テレビの模型の状態（同期と色が取れているか。映像のクロックのベンド点） |
| `voices`、`voices_dropped` | `voices` を付けたときだけ。DOC のオシレータが鳴り始めた・止まった出来事の列（`frame`、`cpu_clock`、`osc`、`on`、そのときの `freq`、`vol`、`wave_ptr`、`wave_size`、`control`）。最大 2,000 件で、超えた数は `voices_dropped` |

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
| 操作 | 矢印・Z（A）・X（B）・C（C）・Enter（START）が釦（ホストの `fc85` と同じ）。動かす・止める、1 フレーム、リセット、音（ブラウザの決まりで、押してから鳴る）、画面の保存（PNG）、カートリッジを読む、状態の保存と復元（枠 `gui`）、Bend モード、ベンド点の命令 |
| 見るもの | フレーム、速さ、PC とラベル、モード、バスエラー（数と最後の番地）、テレビの模型の状態、1 フレームの予算（CPU・ACRTC・Fabric）、DOC の 32 のオシレータ（鳴っているか、音量。ますに指を置くとレジスタ）、68000 のレジスタ（20 フレームごと） |
| 速さ | ページが時刻から進めるフレームの数を決め、1 回に最大 3 フレームを頼む。遅れが 6 フレームを超えたら飛ばす。音はブラウザの WebAudio に 0.06 秒先から並べ、0.3 秒より先にたまったら捨てる（ホストの安全層と同じく、仮想機は変えない） |
| 口 | `GET /`（ページ。プログラムに埋め込み）、`POST /api`（本文は `{"method", "params"}`。答えは §02 と同じ）、`POST /frame`（GUI の 1 歩。本文は `{"pad", "frames"}`。`run` の答え、表示の範囲の PNG、溜まった音（16 bit・ステレオ・リトルエンディアン）を base64 で返し、`mode` と `doc`（`doc` の手順と同じ）を添える） |
| 守り | 手元の番地（127.0.0.1 か ::1）でだけ待つ。Host の見出しが待っている口の名前（127.0.0.1・localhost・[::1]）でなければ 403、Origin が付いていて同じ口でなければ 403（ほかのサイトのページや、DNS の付け替えから使われないように）。`/api` はファイルを読み書きする手順（`load`、`screenshot` の `path` など）も通すので、手元の人だけが使う前提 |

## 07　これから足すもの

DR-092 (5) のうち、この版にないもの [O]：記録（trace）とラベル、メモリの見張り（書いたら止める）、レジスタ表の 1 ファイル（DR-092 (5)）からの名前、C の ABI・WebAssembly・libretro（DR-092 (4)）。Web の GUI の残り：ゲームパッド、メモリの表示、ブレークポイント、波形の表示。
