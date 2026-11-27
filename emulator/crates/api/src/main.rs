//! fc85 の窓口のプログラム（DR-097、DR-101）。標準入力から 1 行に 1 つの要求を読み、標準出力に答えを書く。
//!
//! 使い方：`fc85-api [--mcp | --http [ADDR]] [--root DIR] [--cart cart.bin] [--boot boot.bin] [--char char.bin]`
//! - 既定は JSON の手順（`protocol`）。`--mcp` で MCP サーバー（`mcp`）。
//! - `--http` は Web の GUI（`http`）。既定の口は 127.0.0.1:8085。手元の番地でだけ待つ。
//! - `--cart` などを付けると、最初の要求の前にその ROM で `load` する（付けなければ、最初に仮想機を使う
//!   ときに既定の ROM で作る）。
//! - `--root` は既定の ROM（`firmware/*/build/`）を探す起点。
//!
//! 標準出力には答えだけを書く。知らせは標準エラーに出す。

use std::io::{BufRead, Write};
use std::net::{SocketAddr, TcpListener};

use fc85_api::protocol::{Session, handle_line};
use serde_json::{Map, Value};

const HELP: &str = "fc85-api [--mcp | --http [ADDR]] [--root DIR] [--cart cart.bin] [--boot boot.bin] [--char char.bin]\n\
標準入力に 1 行 1 つの JSON の要求。手順の一覧は {\"id\":1,\"method\":\"methods\"}。\
--http は Web の GUI（既定 127.0.0.1:8085）。説明は sdk/docs/emulator-api.md";

enum Mode {
    Json,
    Mcp,
    Http(SocketAddr),
}

fn main() {
    let mut mode = Mode::Json;
    let mut session = Session::new();
    let mut load = Map::new();
    let mut args = std::env::args().skip(1).peekable();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--mcp" => mode = Mode::Mcp,
            "--http" => {
                let addr = match args.peek() {
                    Some(v) if !v.starts_with("--") => args.next().unwrap_or_default(),
                    _ => "127.0.0.1:8085".to_string(),
                };
                let addr: SocketAddr = addr.parse().unwrap_or_else(|_| {
                    eprintln!("--http の口を読めない：{addr}（例 127.0.0.1:8085）");
                    std::process::exit(2);
                });
                if !fc85_api::http::is_loopback(&addr) {
                    eprintln!("--http は手元の番地（127.0.0.1 か ::1）でだけ待つ");
                    std::process::exit(2);
                }
                mode = Mode::Http(addr);
            }
            "--root" => session.root = args.next().map(Into::into),
            "--cart" | "--boot" | "--char" => {
                if let Some(v) = args.next() {
                    load.insert(a.trim_start_matches("--").to_string(), Value::String(v));
                }
            }
            "-h" | "--help" => {
                eprintln!("{HELP}");
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
    let mcp = match mode {
        Mode::Http(addr) => {
            let listener = TcpListener::bind(addr).unwrap_or_else(|e| {
                eprintln!("{addr} で待てない: {e}");
                std::process::exit(1);
            });
            eprintln!("Web の GUI：http://{addr}/ （Ctrl+C で終わる）");
            if let Err(e) = fc85_api::http::serve(listener, session) {
                eprintln!("{e}");
            }
            return;
        }
        Mode::Mcp => true,
        Mode::Json => false,
    };
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
