//! MCP サーバー（DR-097）。標準入出力の JSON-RPC 2.0 で、`protocol::methods` の手順をツールとして出す。
//! SDK は使わず、使う分（initialize、ping、tools/list、tools/call）だけを書く。
//! 画面（screenshot で path なし）は image の内容で返す。ツールの誤りは `isError` で返す。

use serde_json::{Value, json};

use crate::API_VERSION;
use crate::encode;
use crate::protocol::{Session, methods};

/// 答えられる手順の版（新しい順）。相手の版がこの中にあればそれを返し、なければ最初のものを返す。
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "fc85（1985 年の架空のコンソール）のエミュレータ。load で ROM を読み、run でフレームを進め、\
screenshot で画面、audio で音、regs・peek でレジスタとメモリを見る。同じ ROM・同じ入力なら結果は同じ（決定的）。\
番地はラベル（m68kasm のリスト）でも書ける。仕様は docs/spec/hardware.md、BIOS の約束は sdk/docs/bios-abi.md、\
この窓口は sdk/docs/emulator-api.md。";

fn ok(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn err(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// 1 つの通知か要求を処理する。通知（id なし）には答えない。
pub fn handle(s: &mut Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    Some(match method {
        "initialize" => {
            let want = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("");
            let version = PROTOCOL_VERSIONS
                .iter()
                .find(|v| **v == want)
                .unwrap_or(&PROTOCOL_VERSIONS[0]);
            ok(
                &id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "fc85", "version": format!("{} (api {API_VERSION})", env!("CARGO_PKG_VERSION"))},
                    "instructions": INSTRUCTIONS,
                }),
            )
        }
        "ping" => ok(&id, json!({})),
        "tools/list" => {
            let tools: Vec<Value> = methods()
                .into_iter()
                .map(|(name, description, schema)| {
                    json!({"name": name, "description": description, "inputSchema": schema})
                })
                .collect();
            ok(&id, json!({"tools": tools}))
        }
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(err(&id, -32602, "name がない"));
            };
            if !methods().iter().any(|m| m.0 == name) {
                return Some(err(&id, -32602, &format!("ツール {name:?} はない")));
            }
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match s.call(name, &args) {
                Ok(r) => {
                    let mut content = Vec::new();
                    if let Some(png) = &r.png {
                        content.push(json!({"type": "image", "data": encode::base64(png), "mimeType": "image/png"}));
                    }
                    content.push(json!({"type": "text", "text": r.value.to_string()}));
                    ok(&id, json!({"content": content, "isError": false}))
                }
                Err(e) => ok(
                    &id,
                    json!({"content": [{"type": "text", "text": e}], "isError": true}),
                ),
            }
        }
        _ => err(&id, -32601, &format!("手順 {method:?} はない")),
    })
}

/// 1 行を処理して、答えの行を返す（なければ None）。
pub fn handle_line(s: &mut Session, line: &str) -> Option<String> {
    let line = line.trim_start_matches('\u{feff}').trim();
    if line.is_empty() {
        return None;
    }
    match serde_json::from_str::<Value>(line) {
        Ok(Value::Array(batch)) => {
            let out: Vec<Value> = batch.iter().filter_map(|m| handle(s, m)).collect();
            (!out.is_empty()).then(|| Value::Array(out).to_string())
        }
        Ok(msg) => handle(s, &msg).map(|v| v.to_string()),
        Err(e) => Some(err(&Value::Null, -32700, &format!("JSON を読めない: {e}")).to_string()),
    }
}
