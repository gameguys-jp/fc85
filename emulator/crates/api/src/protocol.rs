//! JSON の手順（DR-097）。要求は `{"id": …, "method": "run", "params": {…}}`、答えは
//! `{"jsonrpc": "2.0", "id": …, "result": {…}}` か `{"jsonrpc": "2.0", "id": …, "error": {"code": …, "message": …}}`。
//! 1 行に 1 つ（JSON Lines）。MCP（`mcp`）も同じ `Session::call` を使う。
//!
//! 番地は数か文字列（`$400000`、`0x400000`、10 進、ラベル、`ラベル+差`）。ファイルの道は、手順を動かして
//! いるプログラムの今のフォルダから。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use fc85_board::console::{AUDIO_RATE, Console};
use fc85_chips::hd68000::Size;
use serde_json::{Value, json};

use crate::encode;
use crate::{
    API_VERSION, Area, Config, MAX_FRAMES, MAX_TRACE, MAX_WATCH_BYTES, Machine, RunOptions, Space,
    Watch, parse_pad,
};

/// 1 つの答え：JSON の値と、付ける PNG（画面）。
#[derive(Debug, Default)]
pub struct Reply {
    pub value: Value,
    pub png: Option<Vec<u8>>,
}

impl Reply {
    fn json(value: Value) -> Self {
        Self { value, png: None }
    }
}

/// 手順の相手：仮想機（最初に使うときに既定の ROM で作る）と、保存の枠。
#[derive(Default)]
pub struct Session {
    machine: Option<Machine>,
    /// 保存の枠（`save_state` の slot）。
    slots: HashMap<String, Console>,
    /// 既定の ROM を探すときの起点（なければ今のフォルダと実行ファイルの場所）。
    pub root: Option<PathBuf>,
}

/// 既定の置き場所（リポジトリの firmware のビルド結果）から探す。起点、今のフォルダとその上 2 段、
/// 実行ファイルのフォルダとその上 4 段の順。
pub fn default_path(root: Option<&Path>, rel: &str) -> Option<PathBuf> {
    let mut bases: Vec<PathBuf> = root.into_iter().map(Path::to_path_buf).collect();
    bases.extend(["", "..", "../.."].iter().map(PathBuf::from));
    if let Ok(exe) = std::env::current_exe() {
        bases.extend(exe.ancestors().skip(1).take(5).map(Path::to_path_buf));
    }
    bases.iter().map(|b| b.join(rel)).find(|p| p.exists())
}

fn read_file(p: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(p).map_err(|e| format!("{} を読めない: {e}", p.display()))
}

fn write_file(p: &str, data: &[u8]) -> Result<(), String> {
    std::fs::write(p, data).map_err(|e| format!("{p} に書けない: {e}"))
}

/// `a.bin` → `a.lst`（あれば）。
fn listing_beside(p: &Path) -> Option<PathBuf> {
    let l = p.with_extension("lst");
    l.exists().then_some(l)
}

// ---------------------------------------------------------------------- 引数

fn opt_str<'a>(p: &'a Value, k: &str) -> Result<Option<&'a str>, String> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(v) => Err(format!("{k} は文字列（{v}）")),
    }
}

fn opt_bool(p: &Value, k: &str) -> Result<Option<bool>, String> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(v) => Err(format!("{k} は true か false（{v}）")),
    }
}

fn opt_u64(p: &Value, k: &str) -> Result<Option<u64>, String> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{k} は 0 以上の整数（{n}）")),
        Some(Value::String(s)) => crate::symbols::parse_number(s.trim())
            .map(|v| Some(u64::from(v)))
            .ok_or_else(|| format!("{k} は数（{s}）")),
        Some(v) => Err(format!("{k} は数（{v}）")),
    }
}

fn str_list(p: &Value, k: &str) -> Result<Vec<String>, String> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(s)) => Ok(vec![s.clone()]),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("{k} は文字列の列"))
            })
            .collect(),
        Some(v) => Err(format!("{k} は文字列の列（{v}）")),
    }
}

fn pad_value(v: &Value) -> Result<u16, String> {
    match v {
        Value::Number(n) => n
            .as_u64()
            .and_then(|x| u16::try_from(x).ok())
            .ok_or_else(|| format!("入力の値 {n} は 0〜$FFFF")),
        Value::String(s) => parse_pad(s),
        _ => Err(format!("入力の値は数か釦の名前（{v}）")),
    }
}

fn size_of(p: &Value) -> Result<Size, String> {
    match opt_str(p, "size")?.unwrap_or("word") {
        "word" => Ok(Size::Word),
        "byte" => Ok(Size::Byte),
        s => Err(format!("size は word か byte（{s}）")),
    }
}

use encode::hex;

fn unhex(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if !s.len().is_multiple_of(2) {
        return Err("hex の桁が奇数".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| format!("hex ではない：{s}")))
        .collect()
}

// ---------------------------------------------------------------------- 手順の表

/// 手順の名前、説明、引数の JSON Schema。MCP の tools/list もこれを使う。
pub fn methods() -> Vec<(&'static str, &'static str, Value)> {
    let addr = json!({"type": ["integer", "string"], "description": "番地：数、\"$400000\"、\"0x400000\"、ラベル、\"ラベル+4\""});
    let space = json!({"type": "string", "enum": ["cpu", "vram", "wave", "dsp_prog", "temporal", "char_rom", "palette"], "default": "cpu"});
    let pad = json!({"type": ["integer", "string"], "description": "PORT_A：数か釦の名前（\"UP+A\"。UP DOWN LEFT RIGHT A B C START）"});
    vec![
        (
            "info",
            "仮想機と窓口の版、BIOS の版、カートリッジ、フレーム、モード、ベンド点、テレビの状態",
            json!({"type": "object", "properties": {}}),
        ),
        (
            "load",
            "ROM を読んで電源を入れ直す。boot と char を省くとリポジトリの firmware のビルド結果。カートリッジの横の .lst と boot.lst のラベルも読む",
            json!({"type": "object", "properties": {
                "cart": {"type": "string", "description": "カートリッジの ROM（なければ挿さない）"},
                "boot": {"type": "string"}, "char": {"type": "string"},
                "listing": {"type": ["string", "array"], "items": {"type": "string"}, "description": "足して読む m68kasm のリスト"},
                "doc": {"type": "boolean", "default": true, "description": "DOC の子基板を挿す"},
                "bends": {"type": "array", "items": {"type": "string"}, "description": "ベンド点の命令（仕様書 §11。\"clock doc 4000000\" など）"},
                "audio_tap": {"type": "string", "enum": ["line", "digital"], "default": "line"}
            }}),
        ),
        (
            "reset",
            "同じ ROM と設定で電源を入れ直す",
            json!({"type": "object", "properties": {}}),
        ),
        (
            "run",
            "フレームを進める。止める条件（ブレークポイント、二重バスフォルト、バスエラー、メモリの見張り）に当たると途中で止まる。予算（CPU・ACRTC・Fabric の働いた割合）、音の要約、画面の要約の値を返す",
            json!({"type": "object", "properties": {
                "frames": {"type": "integer", "minimum": 0, "maximum": MAX_FRAMES, "default": 1},
                "pad": pad,
                "input": {"type": "array", "description": "入力の台本 [[この実行の何フレーム目から, PORT_A], …]。次の行まで保つ", "items": {"type": "array"}},
                "port_b": {"type": "integer"},
                "break": {"type": "array", "items": addr, "description": "この番地の命令の前で止める"},
                "idle": {"type": "array", "description": "待ちのループ。番地か [from, to)。ここの時間を CPU の空きとして数える"},
                "stop_on_bus_error": {"type": "boolean", "default": false},
                "run_when_halted": {"type": "boolean", "default": false},
                "voices": {"type": "boolean", "default": false, "description": "DOC のオシレータが鳴り始め・止まったときを記録して voices で返す（8 µs ごとに停止ビットを見る。最大 2000 件）"},
                "watch": {"type": "array", "description": "メモリの見張り。中身が変わったら、その命令の後で止めて watch_hit で返す。番地かラベルだけなら CPU の空間の 2 byte。{\"space\", \"addr\", \"len\"} で空間（cpu、vram、wave など）と長さ。合計 4096 byte まで", "items": {}},
                "trace": {"type": "integer", "minimum": 0, "maximum": MAX_TRACE, "default": 0, "description": "最後に実行した命令をこの数だけ覚え、trace で返す（PC、ラベル、クロック。古い順）"}
            }}),
        ),
        (
            "step",
            "68000 の命令を count 個実行し、レジスタを返す",
            json!({"type": "object", "properties": {"count": {"type": "integer", "minimum": 1, "maximum": 100000, "default": 1}}}),
        ),
        (
            "regs",
            "68000 のレジスタ、状態、クロック、フレーム",
            json!({"type": "object", "properties": {}}),
        ),
        (
            "doc",
            "DOC（ES5503）の今の様子：有効なオシレータの数と、各オシレータの鳴っているか・周波数・音量・波形の表の頭と大きさ・制御のレジスタ。子基板がなければ null。読んでも DOC は変わらない",
            json!({"type": "object", "properties": {}}),
        ),
        (
            "screenshot",
            "画面（テレビの模型の絵）の PNG。path があればファイルに書き、なければ答えに付ける。hash は画素の FNV-1a",
            json!({"type": "object", "properties": {
                "path": {"type": "string"},
                "area": {"type": "string", "enum": ["active", "view", "raster"], "default": "active", "description": "active は表示の 256×192、view は 320×240、raster は帰線を含む 342×262"},
                "scale": {"type": "integer", "minimum": 1, "maximum": 8, "description": "既定は path があれば 1、なければ 2"}
            }}),
        ),
        (
            "audio",
            "溜めた音（最大 60 秒）の要約。path があれば 48 kHz・16 bit・ステレオの WAV に書く。keep が false なら取り出した分を捨てる",
            json!({"type": "object", "properties": {"path": {"type": "string"}, "keep": {"type": "boolean", "default": false}}}),
        ),
        (
            "peek",
            "メモリを読む（バスを通さず、副作用なし）。cpu は Boot ROM・Main RAM・Staging RAM・カートリッジの窓",
            json!({"type": "object", "required": ["addr"], "properties": {
                "space": space, "addr": addr,
                "len": {"type": "integer", "minimum": 1, "maximum": 65536, "default": 16},
                "words": {"type": "boolean", "default": false, "description": "big-endian の語の列も返す"}
            }}),
        ),
        (
            "poke",
            "メモリを書く（バスを通さない。ROM も書ける）",
            json!({"type": "object", "required": ["addr", "hex"], "properties": {"space": space, "addr": addr, "hex": {"type": "string"}}}),
        ),
        (
            "bus_read",
            "CPU と同じバスサイクルで読む（I/O の副作用あり。バスエラーは記録に数える）",
            json!({"type": "object", "required": ["addr"], "properties": {"addr": addr, "size": {"type": "string", "enum": ["word", "byte"], "default": "word"}}}),
        ),
        (
            "bus_write",
            "CPU と同じバスサイクルで書く",
            json!({"type": "object", "required": ["addr", "value"], "properties": {"addr": addr, "value": {"type": ["integer", "string"]}, "size": {"type": "string", "enum": ["word", "byte"], "default": "word"}}}),
        ),
        (
            "save_state",
            "状態を保存する。slot（この過程の中の名前）か path（JSON のファイル）",
            json!({"type": "object", "properties": {"slot": {"type": "string"}, "path": {"type": "string"}}}),
        ),
        (
            "load_state",
            "保存した状態に戻す。slot か path",
            json!({"type": "object", "properties": {"slot": {"type": "string"}, "path": {"type": "string"}}}),
        ),
        (
            "bend",
            "ベンド点の命令を実行する（仕様書 §11。\"mode bend\"、\"clock video 18000000\" など）",
            json!({"type": "object", "required": ["cmd"], "properties": {"cmd": {"type": "string"}}}),
        ),
        (
            "symbols",
            "ラベルを探す（prefix で始まる名前）か、addr の名前を返す",
            json!({"type": "object", "properties": {"prefix": {"type": "string"}, "addr": addr, "limit": {"type": "integer", "default": 50}}}),
        ),
    ]
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// 仮想機（なければ既定の ROM で作る）。
    pub fn machine(&mut self) -> Result<&mut Machine, String> {
        if self.machine.is_none() {
            self.load(&json!({}))?;
        }
        Ok(self.machine.as_mut().expect("作った"))
    }

    fn load(&mut self, p: &Value) -> Result<Value, String> {
        let root = self.root.clone();
        let find = |k: &str, rel: &str| -> Result<Option<PathBuf>, String> {
            Ok(match opt_str(p, k)? {
                Some(s) => Some(PathBuf::from(s)),
                None => default_path(root.as_deref(), rel),
            })
        };
        let boot = find("boot", "firmware/boot/build/boot.bin")?;
        let chars = find("char", "firmware/charrom/build/char.bin")?;
        let cart = opt_str(p, "cart")?.map(PathBuf::from);
        let line = match opt_str(p, "audio_tap")?.unwrap_or("line") {
            "line" => true,
            "digital" => false,
            s => return Err(format!("audio_tap は line か digital（{s}）")),
        };
        let config = Config {
            boot: boot.as_deref().map(read_file).transpose()?,
            chars: chars.as_deref().map(read_file).transpose()?,
            cart: cart.as_deref().map(read_file).transpose()?,
            doc: opt_bool(p, "doc")?.unwrap_or(true),
            bends: str_list(p, "bends")?,
            line,
        };
        let mut m = Machine::new(config)?;
        let mut listings: Vec<PathBuf> = Vec::new();
        listings.extend(boot.as_deref().and_then(listing_beside));
        listings.extend(cart.as_deref().and_then(listing_beside));
        listings.extend(str_list(p, "listing")?.into_iter().map(PathBuf::from));
        let mut read = Vec::new();
        for l in &listings {
            let text = String::from_utf8_lossy(&read_file(l)?).into_owned();
            let n = m.symbols.add_listing(&text);
            read.push(json!({"path": l.display().to_string(), "symbols": n}));
        }
        let warn = if boot.is_none() {
            Some(
                "Boot ROM が見つからない（py firmware/boot/build.py で作るか boot で指定する）。68000 はすぐ止まる",
            )
        } else {
            None
        };
        self.machine = Some(m);
        let mut info = self.info()?;
        info["listings"] = json!(read);
        info["boot"] = json!(boot.map(|b| b.display().to_string()));
        if let Some(w) = warn {
            info["warning"] = json!(w);
        }
        Ok(info)
    }

    fn info(&mut self) -> Result<Value, String> {
        let m = self.machine()?;
        Ok(json!({
            "api_version": API_VERSION,
            "emulator_version": env!("CARGO_PKG_VERSION"),
            "bios_version": m.bios_version(),
            "cart_bytes": m.console.sys.cart.as_ref().map(Vec::len),
            "doc_board": m.console.sys.ports.doc,
            "symbols": m.symbols.len(),
            "frame": m.frame(),
            "cpu_clock": m.console.cpu.clock,
            "mode": m.mode(),
            "bends": m.console.sys.bends.summary(),
            "tv": m.tv_status(),
            "audio_rate": AUDIO_RATE,
        }))
    }

    fn addr(&mut self, p: &Value, k: &str) -> Result<u32, String> {
        let m = self.machine()?;
        match p.get(k) {
            Some(Value::Number(n)) => n
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or_else(|| format!("{k} は 0〜$FFFFFFFF（{n}）")),
            Some(Value::String(s)) => m.symbols.parse_addr(s),
            Some(v) => Err(format!("{k} は数か文字列（{v}）")),
            None => Err(format!("{k} がない")),
        }
    }

    fn addr_value(&mut self, v: &Value) -> Result<u32, String> {
        self.addr(&json!({"a": v}), "a")
    }

    fn run(&mut self, p: &Value) -> Result<Value, String> {
        let mut o = RunOptions {
            frames: opt_u64(p, "frames")?.unwrap_or(1),
            ..Default::default()
        };
        if o.frames > MAX_FRAMES {
            return Err(format!("frames は {MAX_FRAMES} まで"));
        }
        if let Some(v) = p.get("pad").filter(|v| !v.is_null()) {
            o.input.push((0, pad_value(v)?));
        }
        if let Some(list) = p.get("input").filter(|v| !v.is_null()) {
            let list = list.as_array().ok_or("input は [[フレーム, 値], …]")?;
            for e in list {
                let pair = e
                    .as_array()
                    .filter(|a| a.len() == 2)
                    .ok_or_else(|| format!("input の行は [フレーム, 値]（{e}）"))?;
                let f = pair[0]
                    .as_u64()
                    .ok_or_else(|| format!("input のフレームは整数（{e}）"))?;
                o.input.push((f, pad_value(&pair[1])?));
            }
            o.input.sort_by_key(|e| e.0);
        }
        if let Some(b) = opt_u64(p, "port_b")? {
            o.port_b = Some(u16::try_from(b).map_err(|_| "port_b は 0〜$FFFF")?);
        }
        if let Some(list) = p.get("break").filter(|v| !v.is_null()) {
            let list = list
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![list.clone()]);
            for v in &list {
                let a = self.addr_value(v)?;
                o.break_pc.push(a);
            }
        }
        if let Some(list) = p.get("idle").filter(|v| !v.is_null()) {
            let list = list
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![list.clone()]);
            for v in &list {
                let r = match v.as_array() {
                    Some(a) if a.len() == 2 => (self.addr_value(&a[0])?, self.addr_value(&a[1])?),
                    _ => {
                        let a = self.addr_value(v)?;
                        (a, a + 1)
                    }
                };
                o.idle_pc.push(r);
            }
        }
        o.stop_on_bus_error = opt_bool(p, "stop_on_bus_error")?.unwrap_or(false);
        o.run_when_halted = opt_bool(p, "run_when_halted")?.unwrap_or(false);
        o.voices = opt_bool(p, "voices")?.unwrap_or(false);
        if let Some(list) = p.get("watch").filter(|v| !v.is_null()) {
            let list = list
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![list.clone()]);
            let mut bytes = 0;
            for v in &list {
                // 番地だけなら CPU の空間の 2 byte（語）
                let (space, at, len) = match v.as_object() {
                    Some(m) => (
                        m.get("space").and_then(Value::as_str).unwrap_or("cpu"),
                        m.get("addr").ok_or("watch の addr がない")?,
                        m.get("len").and_then(Value::as_u64).unwrap_or(2),
                    ),
                    None => ("cpu", v, 2),
                };
                let w = Watch {
                    space: Space::parse(space)?,
                    addr: self.addr_value(at)?,
                    len: u32::try_from(len).map_err(|_| "watch の len が大きすぎる")?,
                };
                bytes += w.len;
                if w.len == 0 || bytes > MAX_WATCH_BYTES {
                    return Err(format!(
                        "watch は 1 byte 以上、合計 {MAX_WATCH_BYTES} byte まで"
                    ));
                }
                // 範囲が空間の中にあるかを先に確かめる
                self.machine()?.peek(w.space, w.addr, w.len as usize)?;
                o.watch.push(w);
            }
        }
        o.trace = opt_u64(p, "trace")?.unwrap_or(0) as usize;
        if o.trace > MAX_TRACE {
            return Err(format!("trace は {MAX_TRACE} まで"));
        }
        let r = self.machine()?.run(&o);
        serde_json::to_value(r).map_err(|e| e.to_string())
    }

    fn screenshot(&mut self, p: &Value) -> Result<Reply, String> {
        let path = opt_str(p, "path")?;
        let area = match opt_str(p, "area")?.unwrap_or("active") {
            "active" => Area::Active,
            "view" => Area::View,
            "raster" => Area::Raster,
            s => return Err(format!("area は active、view、raster（{s}）")),
        };
        let scale = opt_u64(p, "scale")?.unwrap_or(if path.is_some() { 1 } else { 2 }) as usize;
        if !(1..=8).contains(&scale) {
            return Err("scale は 1〜8".into());
        }
        let (w, h, rgb) = self.machine()?.screen(area);
        let hash = format!("{:016x}", encode::fnv1a(&rgb));
        let (sw, sh) = (w * scale, h * scale);
        let mut big = Vec::with_capacity(sw * sh * 3);
        for y in 0..sh {
            for x in 0..sw {
                let i = ((y / scale) * w + x / scale) * 3;
                big.extend_from_slice(&rgb[i..i + 3]);
            }
        }
        let png = encode::png_rgb(sw, sh, &big);
        let mut v =
            json!({"width": sw, "height": sh, "hash": hash, "frame": self.machine()?.frame()});
        if let Some(p) = path {
            write_file(p, &png)?;
            v["path"] = json!(p);
            Ok(Reply::json(v))
        } else {
            Ok(Reply {
                value: v,
                png: Some(png),
            })
        }
    }

    fn audio(&mut self, p: &Value) -> Result<Value, String> {
        let keep = opt_bool(p, "keep")?.unwrap_or(false);
        let pcm = self.machine()?.take_audio(keep);
        let mut v = serde_json::to_value(crate::AudioStats::of(&pcm)).map_err(|e| e.to_string())?;
        if let Some(path) = opt_str(p, "path")? {
            write_file(path, &encode::wav(AUDIO_RATE, &pcm))?;
            v["path"] = json!(path);
        }
        Ok(v)
    }

    fn space(p: &Value) -> Result<Space, String> {
        Space::parse(opt_str(p, "space")?.unwrap_or("cpu"))
    }

    fn peek(&mut self, p: &Value) -> Result<Value, String> {
        let space = Self::space(p)?;
        let addr = self.addr(p, "addr")?;
        let len = opt_u64(p, "len")?.unwrap_or(16) as usize;
        if !(1..=65536).contains(&len) {
            return Err("len は 1〜65536".into());
        }
        let data = self.machine()?.peek(space, addr, len)?;
        let mut v = json!({"addr": addr, "len": len, "hex": hex(&data)});
        if opt_bool(p, "words")?.unwrap_or(false) {
            let words: Vec<u16> = data
                .chunks(2)
                .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
                .collect();
            v["words"] = json!(words);
        }
        Ok(v)
    }

    fn state_target(p: &Value) -> Result<(Option<String>, Option<String>), String> {
        let slot = opt_str(p, "slot")?.map(str::to_string);
        let path = opt_str(p, "path")?.map(str::to_string);
        if slot.is_none() && path.is_none() {
            return Err("slot か path を指定する".into());
        }
        Ok((slot, path))
    }

    /// 要求を 1 つ実行する。
    pub fn call(&mut self, method: &str, p: &Value) -> Result<Reply, String> {
        let p = if p.is_null() { &json!({}) } else { p };
        if !p.is_object() {
            return Err("params はオブジェクト".into());
        }
        let v = match method {
            "info" => self.info()?,
            "methods" => json!(
                methods()
                    .into_iter()
                    .map(|(n, d, s)| json!({"name": n, "description": d, "params": s}))
                    .collect::<Vec<_>>()
            ),
            "load" => self.load(p)?,
            "reset" => {
                self.machine()?.reset()?;
                self.info()?
            }
            "run" => self.run(p)?,
            "step" => {
                let n = opt_u64(p, "count")?.unwrap_or(1);
                if !(1..=100_000).contains(&n) {
                    return Err("count は 1〜100000".into());
                }
                serde_json::to_value(self.machine()?.step(n)).map_err(|e| e.to_string())?
            }
            "regs" => serde_json::to_value(self.machine()?.regs()).map_err(|e| e.to_string())?,
            "doc" => serde_json::to_value(self.machine()?.doc()).map_err(|e| e.to_string())?,
            "screenshot" => return self.screenshot(p),
            "audio" => self.audio(p)?,
            "peek" => self.peek(p)?,
            "poke" => {
                let space = Self::space(p)?;
                let addr = self.addr(p, "addr")?;
                let data = unhex(opt_str(p, "hex")?.ok_or("hex がない")?)?;
                self.machine()?.poke(space, addr, &data)?;
                json!({"addr": addr, "len": data.len()})
            }
            "bus_read" => {
                let addr = self.addr(p, "addr")?;
                let size = size_of(p)?;
                let value = self.machine()?.bus_read(addr, size)?;
                json!({"addr": addr, "value": value})
            }
            "bus_write" => {
                let addr = self.addr(p, "addr")?;
                let size = size_of(p)?;
                let value = opt_u64(p, "value")?.ok_or("value がない")?;
                let value = u16::try_from(value).map_err(|_| "value は 0〜$FFFF")?;
                self.machine()?.bus_write(addr, size, value)?;
                json!({"addr": addr, "value": value})
            }
            "save_state" => {
                let (slot, path) = Self::state_target(p)?;
                let m = self.machine()?;
                if let Some(path) = &path {
                    write_file(path, &m.save_state()?)?;
                }
                let c = m.console.clone();
                let frame = m.frame();
                if let Some(s) = &slot {
                    self.slots.insert(s.clone(), c);
                }
                json!({"slot": slot, "path": path, "frame": frame})
            }
            "load_state" => {
                let (slot, path) = Self::state_target(p)?;
                if let Some(s) = &slot {
                    let c = self
                        .slots
                        .get(s)
                        .cloned()
                        .ok_or_else(|| format!("枠 {s:?} はない"))?;
                    self.machine()?.restore(c);
                } else if let Some(path) = &path {
                    let data = read_file(Path::new(path))?;
                    self.machine()?.load_state(&data)?;
                }
                let m = self.machine()?;
                json!({"frame": m.frame(), "pc": m.console.cpu.pc})
            }
            "bend" => {
                let cmd = opt_str(p, "cmd")?.ok_or("cmd がない")?.to_string();
                let m = self.machine()?;
                let summary = m.bend(&cmd)?;
                json!({"summary": summary, "mode": m.mode()})
            }
            "symbols" => {
                if p.get("addr").is_some_and(|v| !v.is_null()) {
                    let a = self.addr(p, "addr")?;
                    json!({"addr": a, "label": self.machine()?.label(a)})
                } else {
                    let prefix = opt_str(p, "prefix")?.unwrap_or("").to_string();
                    let limit = opt_u64(p, "limit")?.unwrap_or(50) as usize;
                    let m = self.machine()?;
                    let found: Vec<Value> = m
                        .symbols
                        .find(&prefix, limit)
                        .into_iter()
                        .map(|(n, a)| json!({"name": n, "addr": a}))
                        .collect();
                    json!({"total": m.symbols.len(), "found": found})
                }
            }
            _ => return Err(format!("手順 {method:?} はない（methods で一覧）")),
        };
        Ok(Reply::json(v))
    }
}

/// JSON Lines の 1 行を処理して、答えの行を返す。
pub fn handle_line(s: &mut Session, line: &str) -> Option<String> {
    // Windows の PowerShell などが先頭に付ける BOM は読み飛ばす
    let line = line.trim_start_matches('\u{feff}').trim();
    if line.is_empty() {
        return None;
    }
    let req: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return Some(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("JSON を読めない: {e}")}})
                    .to_string(),
            );
        }
    };
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = req.get("method").and_then(Value::as_str) else {
        return Some(
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": "method がない"}})
                .to_string(),
        );
    };
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let out = match s.call(method, &params) {
        Ok(r) => {
            let mut v = r.value;
            if let Some(png) = r.png {
                v["png_base64"] = json!(encode::base64(&png));
            }
            json!({"jsonrpc": "2.0", "id": id, "result": v})
        }
        Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": e}}),
    };
    Some(out.to_string())
}
