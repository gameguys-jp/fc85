//! fc85 の窓口のプログラム（DR-097）。標準入力から 1 行に 1 つの要求を読み、標準出力に答えを書く。
//!
//! 使い方：`fc85-api [--mcp] [--root DIR] [--cart cart.bin] [--boot boot.bin] [--char char.bin]`
//! - 既定は JSON の手順（`protocol`）。`--mcp` で MCP サーバー（`mcp`）。
//! - `--cart` などを付けると、最初の要求の前にその ROM で `load` する（付けなければ、最初に仮想機を使う
//!   ときに既定の ROM で作る）。
//! - `--root` は既定の ROM（`firmware/*/build/`）を探す起点。
//!
//! 標準出力には答えだけを書く。知らせは標準エラーに出す。

use std::io::{BufRead, Write};

use fc85_api::protocol::{Session, handle_line};
use serde_json::{Map, Value};

fn main() {
    let mut mcp = false;
    let mut session = Session::new();
    let mut load = Map::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mcp" => mcp = true,
            "--root" => session.root = it.next().map(Into::into),
            "--cart" | "--boot" | "--char" => {
                if let Some(v) = it.next() {
                    load.insert(a.trim_start_matches("--").to_string(), Value::String(v));
                }
            }
            "-h" | "--help" => {
                eprintln!(
                    "fc85-api [--mcp] [--root DIR] [--cart cart.bin] [--boot boot.bin] [--char char.bin]\n\
                     標準入力に 1 行 1 つの JSON の要求。手順の一覧は {{\"id\":1,\"method\":\"methods\"}}。\
                     説明は sdk/docs/emulator-api.md"
                );
                return;
            }
            _ => {
                eprintln!("知らない引数 {a}（--help）");
                std::process::exit(2);
            }
        }
    }
    if !load.is_empty()
        && let Err(e) = session.call("load", &Value::Object(load))
    {
        eprintln!("load: {e}");
        std::process::exit(1);
    }
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let reply = if mcp {
            fc85_api::mcp::handle_line(&mut session, &line)
        } else {
            handle_line(&mut session, &line)
        };
        if let Some(r) = reply
            && writeln!(out, "{r}").and_then(|_| out.flush()).is_err()
        {
            break;
        }
    }
}
