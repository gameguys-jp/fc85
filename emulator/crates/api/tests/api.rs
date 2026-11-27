//! 窓口（DR-097、DR-101）の試験。Boot ROM を使う試験は、`py firmware/boot/build.py` のビルド結果がなければ
//! 飛ばす（board 層の console_boot.rs と同じ）。

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

use fc85_api::protocol::{Session, handle_line};
use fc85_api::{Config, Machine, RunOptions, Space, Watch};
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
fn a_watch_stops_after_the_write_and_the_trace_shows_the_way_there() {
    let Some(cfg) = config() else { return };
    let mut s = session();
    let mut m = Machine::new(cfg).unwrap();
    // BIOS の VBlank の数（bios_frame、$1006xx の長語）は割込みが 1 足す。ラベルは Boot ROM の .lst から
    let frame_addr = s.call("symbols", &json!({"addr": "bios_frame"}));
    let Ok(v) = frame_addr else {
        eprintln!("bios_frame のラベルがないので飛ばす");
        return;
    };
    let addr = v.value["addr"].as_u64().unwrap() as u32;
    m.run(&RunOptions {
        frames: 10,
        ..Default::default()
    });
    let before = m.peek(Space::Cpu, addr, 4).unwrap();
    let r = m.run(&RunOptions {
        frames: 5,
        watch: vec![Watch {
            space: Space::Cpu,
            addr,
            len: 4,
        }],
        trace: 16,
        ..Default::default()
    });
    assert_eq!(r.stop, "watch");
    assert_eq!(r.frames_run, 0);
    let hit = r.watch_hit.unwrap();
    assert_eq!(hit.watch_addr, addr);
    assert_eq!(hit.old, fc85_api::encode::hex(&before));
    let n = u32::from_str_radix(&hit.new, 16).unwrap();
    assert_eq!(n, u32::from_str_radix(&hit.old, 16).unwrap() + 1);
    assert_eq!(
        fc85_api::encode::hex(&m.peek(Space::Cpu, addr, 4).unwrap()),
        hit.new
    );
    // 書いた命令は記録の最後の命令
    let trace = r.trace.unwrap();
    assert!(!trace.is_empty() && trace.len() <= 16);
    assert_eq!(trace.last().unwrap().pc, hit.pc);
    assert!(trace.windows(2).all(|w| w[0].cpu_clock < w[1].cpu_clock));
    // 続けて run すると次の VBlank でまた止まる
    let r2 = m.run(&RunOptions {
        frames: 5,
        watch: vec![Watch {
            space: Space::Cpu,
            addr,
            len: 4,
        }],
        ..Default::default()
    });
    assert_eq!(r2.stop, "watch");
    assert!(r2.cpu_clock > r.cpu_clock);
    assert!(r2.trace.is_none());
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

#[test]
fn writes_to_a_full_acrtc_fifo_are_counted_as_rule_warnings() {
    // Boot ROM の代わりの小さな ROM：AR＝0（FIFO の入口）にして、WFR を見ずに 20 語を書く。リセットの後の
    // ACRTC は OMR の STR＝0 で FIFO を読まないので、8 語で満杯になり、残りの 12 語は規約の違反（DR-096、DR-121）
    let mut boot = vec![0xFFu8; 0x10000];
    let code: [u16; 15] = [
        0x33FC, 0x0000, 0x0020, 0x0000, // MOVE.W #0,$200000（AR）
        0x303C, 0x0013, // MOVE.W #19,D0
        0x33FC, 0x1234, 0x0020, 0x0002, // MOVE.W #$1234,$200002（FIFO）
        0x51C8, 0xFFF6, // DBRA D0,上へ
        0x60FE, // BRA.S *
        0x4E71, 0x4E71,
    ];
    boot[..8].copy_from_slice(&[0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00]);
    for (i, w) in code.iter().enumerate() {
        boot[0x100 + 2 * i..0x102 + 2 * i].copy_from_slice(&w.to_be_bytes());
    }
    let mut m = Machine::new(Config {
        boot: Some(boot),
        ..Default::default()
    })
    .unwrap();
    let r = m.run(&RunOptions {
        frames: 2,
        ..Default::default()
    });
    assert_eq!(r.stop, "frames");
    assert_eq!(r.bus_errors, 0);
    assert_eq!(r.acrtc_rule_warnings, 12);
    // 次の run は、その間の数だけ
    let r = m.run(&RunOptions {
        frames: 1,
        ..Default::default()
    });
    assert_eq!(r.acrtc_rule_warnings, 0);
}

/// ACIA に来た byte をそのまま送り返す Boot ROM（割込みは使わず、RDRF と TDRE を見る）。
fn midi_echo_rom() -> Vec<u8> {
    let mut boot = vec![0xFFu8; 0x10000];
    let code: [u16; 26] = [
        0x13FC, 0x0003, 0x0026, 0x0014, // MOVE.B #$03,MIDI_CTRL（マスタリセット）
        0x13FC, 0x0015, 0x0026,
        0x0014, // MOVE.B #$15,MIDI_CTRL（÷16、8N1、割込みなし）
        0x0839, 0x0000, 0x0026, 0x0014, // 1: BTST #0,MIDI_STATUS（RDRF）
        0x67F6, // BEQ.S 1
        0x1039, 0x0026, 0x0016, // MOVE.B MIDI_DATA,D0
        0x0839, 0x0001, 0x0026, 0x0014, // 2: BTST #1,MIDI_STATUS（TDRE）
        0x67F6, // BEQ.S 2
        0x13C0, 0x0026, 0x0016, // MOVE.B D0,MIDI_DATA
        0x60DE, // BRA.S 1
        0x4E71,
    ];
    boot[..8].copy_from_slice(&[0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00]);
    for (i, w) in code.iter().enumerate() {
        boot[0x100 + 2 * i..0x102 + 2 * i].copy_from_slice(&w.to_be_bytes());
    }
    boot
}

#[test]
fn midi_bytes_go_through_the_acia_with_their_times() {
    // DR-126：MIDI IN の byte は ACIA が 320 µs ずつ受け、ROM が送り返す。送り終えた時刻が付く
    let mut m = Machine::new(Config {
        boot: Some(midi_echo_rom()),
        ..Default::default()
    })
    .unwrap();
    m.run_frames(1);
    assert!(m.take_midi_out().is_empty());
    let t0 = m.time_us();
    m.midi_in(1000, &[0x90, 60, 100]);
    m.run_frames(1);
    let out = m.take_midi_out();
    assert_eq!(
        out.iter().map(|b| b.byte).collect::<Vec<_>>(),
        [0x90, 60, 100]
    );
    // 線に来て 320 µs で受け、送り返して 320 µs で送り終える（ROM の手間は数 µs）
    let first = out[0].us - t0;
    assert!((1640..1660).contains(&first), "{first}");
    for w in out.windows(2) {
        let d = w[1].us - w[0].us;
        assert!((320..330).contains(&d), "{d}");
    }
    assert!(out.iter().all(|b| b.us <= m.time_us()));
    assert_eq!(m.midi_out_dropped(), 0);
}

#[test]
fn the_json_midi_method_sends_and_takes_bytes() {
    let mut s = Session::with_machine(
        Machine::new(Config {
            boot: Some(midi_echo_rom()),
            ..Default::default()
        })
        .unwrap(),
    );
    // 電源投入の直後は ACIA がマスタリセットの中で、線の byte を受け取らない。ROM が設定してから送る
    call(&mut s, "run", json!({"frames": 1}));
    let r = call(&mut s, "midi", json!({"in": [0xB0, 1, 64]}));
    assert_eq!(r["out"], json!([]));
    assert_eq!(r["pending_in"], 3);
    call(&mut s, "run", json!({"frames": 1}));
    let r = call(&mut s, "midi", json!({"in": "C005"}));
    let bytes: Vec<u64> = r["out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["byte"].as_u64().unwrap())
        .collect();
    assert_eq!(bytes, [0xB0, 1, 64]);
    assert!(r["now_us"].as_u64().unwrap() >= r["out"][2]["us"].as_u64().unwrap());
    assert!(s.call("midi", &json!({"in": [256]})).is_err());
    assert!(s.call("midi", &json!({"in": {"a": 1}})).is_err());
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
