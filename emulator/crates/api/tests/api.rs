//! 窓口（DR-097、DR-101）の試験。Boot ROM を使う試験は、`py firmware/boot/build.py` のビルド結果がなければ
//! 飛ばす（board 層の console_boot.rs と同じ）。

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

use fc85_api::protocol::{Session, handle_line};
use fc85_api::{Config, Machine, RunOptions};
use serde_json::{Value, json};

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))
}

fn rom(rel: &str) -> Option<Vec<u8>> {
    std::fs::read(root().join(rel)).ok()
}

/// Boot ROM と文字 ROM（あれば）の組み立て。Boot ROM がなければ None。
fn config() -> Option<Config> {
    let boot = rom("firmware/boot/build/boot.bin");
    if boot.is_none() {
        eprintln!("firmware/boot/build/boot.bin がないので飛ばす");
    }
    Some(Config {
        boot: Some(boot?),
        chars: rom("firmware/charrom/build/char.bin"),
        ..Default::default()
    })
}

#[test]
fn run_ends_in_the_same_state_as_the_host_frame_loop() {
    let Some(cfg) = config() else { return };
    let mut m = Machine::new(cfg.clone()).unwrap();
    let mut c = Machine::new(cfg).unwrap().console;
    let r = m.run(&RunOptions {
        frames: 90,
        input: vec![(30, 0x80), (40, 0)],
        ..Default::default()
    });
    assert_eq!(r.stop, "frames");
    assert_eq!(r.frames_run, 90);
    // ホスト（main.rs の headless）と同じ手順：台本の値を毎フレーム入れて run_frame
    for i in 0..90u64 {
        let v = match i {
            30..=39 => Some(0x80),
            40.. => Some(0),
            _ => None,
        };
        if let Some(v) = v {
            c.sys.set_ports(v, 0);
        }
        c.run_frame();
    }
    c.sys.audio_out.clear();
    c.sys.line_out.clear();
    assert!(
        m.console == c,
        "窓口の run とホストの run_frame で状態が違う"
    );
    assert!(r.audio.samples > 0);
    assert!((0.0..=1.0).contains(&r.budget.cpu_busy));
}

#[test]
fn the_voice_log_sees_oscillators_start_and_stop_without_changing_the_run() {
    let Some(mut cfg) = config() else { return };
    let Some(cart) = rom("sdk/samples/patches/build/patches.bin") else {
        eprintln!("sdk/samples/patches/build/patches.bin がないので飛ばす");
        return;
    };
    cfg.cart = Some(cart);
    let mut a = Machine::new(cfg.clone()).unwrap();
    let mut b = Machine::new(cfg).unwrap();
    // 100 フレーム目に右（次の音色）を押すと、鳴っている音が止まる
    let o = RunOptions {
        frames: 120,
        input: vec![(100, 0x08), (110, 0)],
        ..Default::default()
    };
    let ra = a.run(&RunOptions {
        voices: true,
        ..o.clone()
    });
    let rb = b.run(&o);
    assert!(rb.voices.is_none());
    assert!(a.console == b.console, "記録を取ると状態が変わる");
    let v = ra.voices.unwrap();
    // 最初の音（0 と 1 のオシレータの組）が鳴り始め、右を押したときに止まる
    let on: Vec<_> = v.iter().filter(|e| e.on).map(|e| e.osc).collect();
    let off: Vec<_> = v.iter().filter(|e| !e.on).map(|e| e.osc).collect();
    assert_eq!((&on[..2], &off[..2]), (&[0, 1][..], &[0, 1][..]));
    let first = &v[0];
    assert!(first.freq > 0 && first.vol > 0 && first.control & 1 == 0);
    let stop = v.iter().find(|e| !e.on).unwrap();
    assert!(stop.control & 1 == 1 && stop.frame > first.frame);
    assert!(v.windows(2).all(|w| w[0].cpu_clock <= w[1].cpu_clock));
}

#[test]
fn a_breakpoint_stops_before_the_instruction_and_run_resumes_past_it() {
    let Some(cfg) = config() else { return };
    let mut probe = Machine::new(cfg.clone()).unwrap();
    let mut pcs = Vec::new();
    for _ in 0..300 {
        pcs.push(probe.console.cpu.pc);
        probe.step(1);
    }
    let target = pcs[250];
    let mut m = Machine::new(cfg).unwrap();
    let o = RunOptions {
        frames: 10,
        break_pc: vec![target],
        ..Default::default()
    };
    let r = m.run(&o);
    assert_eq!(r.stop, "breakpoint");
    assert_eq!(r.pc, target);
    let clock = r.cpu_clock;
    // 同じ番地からもう一度：最初の 1 命令は止めない
    let r2 = m.run(&o);
    assert!(r2.cpu_clock > clock);
}

#[test]
fn a_rom_of_ff_halts_and_run_stops_at_once() {
    let mut m = Machine::new(Config::default()).unwrap();
    let r = m.run(&RunOptions {
        frames: 5,
        ..Default::default()
    });
    assert_eq!(r.stop, "halted");
    assert_eq!(r.frames_run, 0);
    assert_eq!(m.regs().state, "halted");
    let r = m.run(&RunOptions {
        frames: 2,
        run_when_halted: true,
        ..Default::default()
    });
    assert_eq!(r.stop, "frames");
    assert_eq!(r.frames_run, 2);
}

fn session() -> Session {
    let mut s = Session::new();
    s.root = Some(root());
    s
}

fn call(s: &mut Session, method: &str, p: Value) -> Value {
    s.call(method, &p)
        .unwrap_or_else(|e| panic!("{method}: {e}"))
        .value
}

#[test]
fn the_json_protocol_runs_peeks_and_restores_slots() {
    let mut s = session();
    let info = call(&mut s, "info", json!({}));
    assert_eq!(info["api_version"], fc85_api::API_VERSION);
    let r = call(
        &mut s,
        "run",
        json!({"frames": 3, "pad": "START", "run_when_halted": true}),
    );
    assert!(r["stop"].is_string());
    call(&mut s, "poke", json!({"addr": "$100F00", "hex": "CAFE"}));
    let p = call(
        &mut s,
        "peek",
        json!({"addr": 0x10_0F00, "len": 2, "words": true}),
    );
    assert_eq!(p["hex"], "CAFE");
    assert_eq!(p["words"], json!([0xCAFE]));
    call(&mut s, "save_state", json!({"slot": "a"}));
    let before = call(&mut s, "regs", json!({}));
    call(&mut s, "run", json!({"frames": 2, "run_when_halted": true}));
    call(&mut s, "load_state", json!({"slot": "a"}));
    let after = call(&mut s, "regs", json!({}));
    assert_eq!(before, after);
    let shot = s.call("screenshot", &json!({})).unwrap();
    assert_eq!(shot.value["width"], 512);
    assert!(shot.png.is_some());
    assert!(s.call("nothing", &json!({})).is_err());
    assert!(s.call("peek", &json!({"addr": "no_such_label"})).is_err());

    // 1 行の要求と答え
    let line = handle_line(&mut s, r#"{"id": 7, "method": "regs"}"#).unwrap();
    let v: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["id"], 7);
    assert!(v["result"]["pc"].is_number());
    let bad: Value = serde_json::from_str(&handle_line(&mut s, "{oops").unwrap()).unwrap();
    assert_eq!(bad["error"]["code"], -32700);
    let shot: Value = serde_json::from_str(
        &handle_line(
            &mut s,
            r#"{"id":1,"method":"screenshot","params":{"scale":1}}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let png = fc85_api::encode::unbase64(shot["result"]["png_base64"].as_str().unwrap()).unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
}

/// 1 つの要求を送り、(状態の番号, 本文) を返す（Connection: close）。
fn http(port: u16, host: &str, method: &str, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let status = out[9..12].parse().unwrap();
    let body = out
        .split_once("\r\n\r\n")
        .map(|x| x.1)
        .unwrap_or("")
        .to_string();
    (status, body)
}

#[test]
fn the_web_gui_serves_the_page_the_api_and_frames_only_to_this_host() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || fc85_api::http::serve(listener, session()));
    let host = format!("127.0.0.1:{port}");
    let (st, page) = http(port, &host, "GET", "/", "");
    assert_eq!(st, 200);
    assert!(page.contains("<canvas"));
    let (st, info) = http(port, &host, "POST", "/api", r#"{"method":"info"}"#);
    assert_eq!(st, 200);
    let v: Value = serde_json::from_str(&info).unwrap();
    assert_eq!(v["result"]["api_version"], fc85_api::API_VERSION);
    let (st, f) = http(
        port,
        &host,
        "POST",
        "/frame",
        r#"{"pad":"START","frames":2}"#,
    );
    assert_eq!(st, 200);
    let v: Value = serde_json::from_str(&f).unwrap();
    let png = fc85_api::encode::unbase64(v["png"].as_str().unwrap()).unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
    assert!(v["report"]["frame"].is_number());
    // ほかの名前の Host（DNS の付け替え）と、知らない道
    assert_eq!(http(port, "evil.example", "GET", "/", "").0, 403);
    assert_eq!(http(port, &host, "GET", "/nothing", "").0, 404);
}

#[test]
fn the_mcp_server_lists_the_tools_and_returns_images() {
    use fc85_api::mcp::handle;
    let mut s = session();
    let init = handle(
        &mut s,
        &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}}),
    )
    .unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(
        handle(
            &mut s,
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .is_none()
    );
    let list = handle(
        &mut s,
        &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .unwrap();
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "run"));
    assert!(tools.iter().all(|t| t["inputSchema"]["type"] == "object"));
    let shot = handle(
        &mut s,
        &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "screenshot", "arguments": {}}}),
    )
    .unwrap();
    assert_eq!(shot["result"]["isError"], false);
    assert_eq!(shot["result"]["content"][0]["type"], "image");
    let bad = handle(
        &mut s,
        &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": {"name": "peek", "arguments": {"addr": "$230000"}}}),
    )
    .unwrap();
    assert_eq!(bad["result"]["isError"], true);
    let unknown = handle(
        &mut s,
        &json!({"jsonrpc": "2.0", "id": 5, "method": "resources/list"}),
    )
    .unwrap();
    assert_eq!(unknown["error"]["code"], -32601);
}
