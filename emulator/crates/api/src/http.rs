//! Web の GUI（DR-092 (6)）のための小さな HTTP の口。手元（127.0.0.1）でだけ待ち、同じ `Session` を
//! ブラウザから使う。依存を増やさないよう、HTTP/1.1 の要る分だけを std で書く。
//!
//! - `GET /`：GUI のページ（`web/index.html` を埋め込む）。
//! - `POST /api`：本文は `{"method": …, "params": {…}}`。答えは JSON の手順と同じ形（`protocol::handle_line`）。
//! - `POST /frame`：GUI の 1 歩。本文は `{"pad": …, "frames": 1}`。`run` の答えと、表示の範囲の PNG と、
//!   溜まった音（16 bit・ステレオ・リトルエンディアン）を base64 で返す。
//!
//! 守り：Host の見出しが待っている番地（か localhost）でなければ 403、POST の Origin が同じ口でなければ
//! 403（ほかのサイトのページや DNS の付け替えから使われないように）。

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::encode;
use crate::protocol::{Session, handle_line};
use crate::{Area, RunOptions, parse_pad};

const PAGE: &str = include_str!("web/index.html");

/// 本文の上限（状態の JSON を受けることはないので小さくてよい）。
const MAX_BODY: usize = 1 << 20;

/// 1 つの要求。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub host: Option<String>,
    pub origin: Option<String>,
    pub keep_alive: bool,
    pub body: Vec<u8>,
}

/// 要求を 1 つ読む。接続が閉じていれば None。
pub fn read_request(r: &mut impl BufRead) -> Result<Option<Request>, String> {
    let mut line = String::new();
    if r.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let (Some(method), Some(path), Some(version)) = (parts.next(), parts.next(), parts.next())
    else {
        return Err(format!("要求の行を読めない：{line:?}"));
    };
    let mut req = Request {
        method: method.to_string(),
        path: path.to_string(),
        keep_alive: version == "HTTP/1.1",
        ..Default::default()
    };
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).map_err(|e| e.to_string())? == 0 {
            return Err("見出しの途中で閉じた".into());
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        let Some((k, v)) = h.split_once(':') else {
            continue;
        };
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "host" => req.host = Some(v.to_string()),
            "origin" => req.origin = Some(v.to_string()),
            "content-length" => len = v.parse().map_err(|_| "Content-Length を読めない")?,
            "connection" => {
                let v = v.to_ascii_lowercase();
                if v.contains("close") {
                    req.keep_alive = false;
                } else if v.contains("keep-alive") {
                    req.keep_alive = true;
                }
            }
            _ => {}
        }
    }
    if len > MAX_BODY {
        return Err("本文が大きすぎる".into());
    }
    req.body = vec![0; len];
    r.read_exact(&mut req.body).map_err(|e| e.to_string())?;
    Ok(Some(req))
}

fn respond(
    w: &mut impl Write,
    status: &str,
    kind: &str,
    body: &[u8],
    keep: bool,
) -> std::io::Result<()> {
    write!(
        w,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: {}\r\n\r\n",
        body.len(),
        if keep { "keep-alive" } else { "close" }
    )?;
    w.write_all(body)?;
    w.flush()
}

/// Host と Origin が、待っている口のものか。
pub fn allowed(req: &Request, port: u16) -> bool {
    let hosts = [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ];
    let host_ok = req
        .host
        .as_deref()
        .is_some_and(|h| hosts.iter().any(|x| x.eq_ignore_ascii_case(h)));
    let origin_ok = match &req.origin {
        None => true,
        Some(o) => hosts
            .iter()
            .any(|x| o.eq_ignore_ascii_case(&format!("http://{x}"))),
    };
    host_ok && origin_ok
}

/// GUI の 1 歩：フレームを進め、画面と音を返す。
fn frame(s: &mut Session, p: &Value) -> Result<Value, String> {
    let pad = match p.get("pad") {
        Some(Value::Number(n)) => n.as_u64().and_then(|v| u16::try_from(v).ok()),
        Some(Value::String(t)) => Some(parse_pad(t)?),
        _ => None,
    };
    let frames = p.get("frames").and_then(Value::as_u64).unwrap_or(1).min(10);
    let m = s.machine()?;
    let o = RunOptions {
        frames,
        input: pad.map(|v| vec![(0, v)]).unwrap_or_default(),
        run_when_halted: true,
        ..Default::default()
    };
    let report = m.run(&o);
    let (w, h, rgb) = m.screen(Area::Active);
    let png = encode::png_rgb(w, h, &rgb);
    let pcm = m.take_audio(false);
    let mut raw = Vec::with_capacity(pcm.len() * 4);
    for (l, r) in pcm {
        raw.extend_from_slice(&l.to_le_bytes());
        raw.extend_from_slice(&r.to_le_bytes());
    }
    Ok(json!({
        "report": report,
        "png": encode::base64(&png),
        "pcm": encode::base64(&raw),
        "mode": m.mode(),
    }))
}

fn handle(s: &Mutex<Session>, req: &Request, port: u16) -> (String, &'static str, Vec<u8>) {
    if !allowed(req, port) {
        return ("403 Forbidden".into(), "text/plain", b"forbidden".to_vec());
    }
    let json_reply = |v: Value| {
        (
            "200 OK".to_string(),
            "application/json",
            v.to_string().into_bytes(),
        )
    };
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => (
            "200 OK".into(),
            "text/html; charset=utf-8",
            PAGE.as_bytes().to_vec(),
        ),
        ("POST", "/api") => {
            let mut v: Value = match serde_json::from_slice(&req.body) {
                Ok(v) => v,
                Err(e) => {
                    return json_reply(
                        json!({"error": {"code": -32700, "message": e.to_string()}}),
                    );
                }
            };
            if v.get("id").is_none() {
                v["id"] = json!(0);
            }
            let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
            let out = handle_line(&mut s, &v.to_string()).unwrap_or_default();
            ("200 OK".into(), "application/json", out.into_bytes())
        }
        ("POST", "/frame") => {
            let p: Value = serde_json::from_slice(&req.body).unwrap_or(json!({}));
            let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
            match frame(&mut s, &p) {
                Ok(v) => json_reply(v),
                Err(e) => json_reply(json!({"error": e})),
            }
        }
        _ => ("404 Not Found".into(), "text/plain", b"not found".to_vec()),
    }
}

fn serve_conn(stream: TcpStream, s: Arc<Mutex<Session>>, port: u16) {
    // 開いたままの接続が糸を持ち続けないよう、読みに時間の上限を付ける
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let Ok(write) = stream.try_clone() else {
        return;
    };
    let mut r = BufReader::new(stream);
    let mut w = write;
    loop {
        let req = match read_request(&mut r) {
            Ok(Some(req)) => req,
            Ok(None) => return,
            Err(e) => {
                let _ = respond(&mut w, "400 Bad Request", "text/plain", e.as_bytes(), false);
                return;
            }
        };
        let (status, kind, body) = handle(&s, &req, port);
        if respond(&mut w, &status, kind, &body, req.keep_alive).is_err() || !req.keep_alive {
            return;
        }
    }
}

/// 待ち受けを始める（戻らない）。`addr` は手元の番地に限る。
pub fn serve(listener: TcpListener, session: Session) -> std::io::Result<()> {
    let port = listener.local_addr()?.port();
    let s = Arc::new(Mutex::new(session));
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let s = s.clone();
        std::thread::spawn(move || serve_conn(stream, s, port));
    }
    Ok(())
}

/// 手元の番地か（127.0.0.0/8 と ::1）。
pub fn is_loopback(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_parse_with_headers_and_body() {
        let raw = b"POST /api HTTP/1.1\r\nHost: 127.0.0.1:8085\r\nContent-Length: 4\r\nOrigin: http://127.0.0.1:8085\r\n\r\nabcdGET / HTTP/1.0\r\nHost: x\r\n\r\n";
        let mut r = BufReader::new(&raw[..]);
        let a = read_request(&mut r).unwrap().unwrap();
        assert_eq!(
            (a.method.as_str(), a.path.as_str(), a.body.as_slice()),
            ("POST", "/api", &b"abcd"[..])
        );
        assert!(a.keep_alive);
        assert!(allowed(&a, 8085));
        let b = read_request(&mut r).unwrap().unwrap();
        assert!(!b.keep_alive);
        assert!(!allowed(&b, 8085));
        assert_eq!(read_request(&mut r).unwrap(), None);
    }

    #[test]
    fn other_hosts_and_origins_are_refused() {
        let req = |host: &str, origin: Option<&str>| Request {
            host: Some(host.into()),
            origin: origin.map(Into::into),
            ..Default::default()
        };
        assert!(allowed(&req("localhost:9000", None), 9000));
        assert!(!allowed(&req("evil.example:9000", None), 9000));
        assert!(!allowed(
            &req("127.0.0.1:9000", Some("http://evil.example")),
            9000
        ));
        assert!(!allowed(&req("127.0.0.1:9001", None), 9000));
    }
}
