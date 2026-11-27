//! fc85 のエミュレータの libretro のコア（DR-104）。RetroArch などのフロントエンドで動かす。
//!
//! - ROM：フロントエンドの system のフォルダに Boot ROM `fc85_boot.bin` と文字 ROM `fc85_char.bin`
//!   （`py firmware/boot/build.py`・`py firmware/charrom/build.py` の結果を名前を変えて置く）。ゲームは
//!   カートリッジの `.bin`。ゲームなしで始めると Boot ROM だけで動く。
//! - カートリッジの横に同じ名前の `.bends` があれば、1 行に 1 つのベンド点の命令（仕様書 §11。`#` から
//!   後ろは注）として電源を入れるときに入れる（ホストの `--bend` と同じ）。
//! - 入力：RetroPad の十字、Y → A、B → B、A → C、START → START。ポート 1 は PORT_A、2 は PORT_B。
//! - 映像は XRGB8888。範囲（view 320×240、active 256×192、raster 342×262）はコアの選択肢。
//!   画素の縦横比は 8:7（約 1.14。ドットは 2CLK、NTSC のテレビ。`docs/parts/HD63484.md`）。
//! - 音は 48 kHz のステレオ。DOC の子基板と、音を取る点（LINE OUT かデジタル）は選択肢で、リセットで効く。
//! - 状態の保存は窓口の `pack_state` の形に長さを付けたもの。大きさは読み込んだときに上限を見積もる
//!   （`pack_state_bound`）。
//! - Main RAM（128 KiB）を `RETRO_MEMORY_SYSTEM_RAM` で見せる。状態を戻してもリセットしても同じ番地。
//!
//! 68000 が止まっても（halt）フレームは進む。中で panic したら、そのゲームを止めて記録に書き、
//! フロントエンドは止めない。

mod sys;
#[cfg(test)]
mod tests;

use std::ffi::{CStr, CString, c_char, c_uint, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use fc85_api::{AUDIO_RATE, Area, Config, FRAME_RATE, Machine};
use sys::*;

/// system のフォルダの Boot ROM と文字 ROM の名前。
pub const BOOT_FILE: &str = "fc85_boot.bin";
pub const CHAR_FILE: &str = "fc85_char.bin";

/// 画素の縦横比（幅÷高さ）。
pub const PIXEL_ASPECT: f64 = 8.0 / 7.0;

/// 状態の頭の長さ（u32 LE）。
const STATE_HEAD: usize = 4;

// ---------------------------------------------------------------------- 呼び返し

struct Callbacks {
    env: Option<EnvironmentFn>,
    video: Option<VideoRefreshFn>,
    batch: Option<AudioSampleBatchFn>,
    poll: Option<InputPollFn>,
    input: Option<InputStateFn>,
    log: Option<LogPrintfFn>,
}

static CB: Mutex<Callbacks> = Mutex::new(Callbacks {
    env: None,
    video: None,
    batch: None,
    poll: None,
    input: None,
    log: None,
});

static CORE: Mutex<Option<Core>> = Mutex::new(None);

/// 毒になった（中で panic した）錠も開ける。中身は呼ぶ側が確かめる。
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn env(cmd: c_uint, data: *mut c_void) -> bool {
    let f = lock(&CB).env;
    // SAFETY: フロントエンドが渡した関数。data は cmd の約束の形
    f.is_some_and(|f| unsafe { f(cmd, data) })
}

fn log(level: c_uint, msg: &str) {
    let f = lock(&CB).log;
    let text = CString::new(format!("[fc85] {}\n", msg.replace('\0', " "))).unwrap_or_default();
    match f {
        // SAFETY: printf の形。書式は固定の "%s"
        Some(f) => unsafe { f(level, c"%s".as_ptr(), text.as_ptr()) },
        None => eprint!("{}", text.to_string_lossy()),
    }
}

/// 画面の隅に短く出す（英語。フロントエンドの字形に日本語がないことがある）。
fn message(msg: &str) {
    let text = CString::new(msg).unwrap_or_default();
    let mut m = RetroMessage {
        msg: text.as_ptr(),
        frames: 240,
    };
    env(RETRO_ENVIRONMENT_SET_MESSAGE, (&raw mut m).cast());
}

/// 生のポインタを含む静的な表（中身は読むだけ）。
struct Table<T>(T);
// SAFETY: 中のポインタは静的な文字列を指し、書き換えない
unsafe impl<T> Sync for Table<T> {}

static VARIABLES: Table<[RetroVariable; 4]> = Table([
    RetroVariable {
        key: c"fc85_area".as_ptr(),
        value: c"Picture area; view|active|raster".as_ptr(),
    },
    RetroVariable {
        key: c"fc85_doc".as_ptr(),
        value: c"DOC sound board (applies on reset); enabled|disabled".as_ptr(),
    },
    RetroVariable {
        key: c"fc85_audio_tap".as_ptr(),
        value: c"Audio tap (applies on reset); line|digital".as_ptr(),
    },
    RetroVariable {
        key: std::ptr::null(),
        value: std::ptr::null(),
    },
]);

const fn pad(port: c_uint, id: c_uint, description: &'static CStr) -> RetroInputDescriptor {
    RetroInputDescriptor {
        port,
        device: RETRO_DEVICE_JOYPAD,
        index: 0,
        id,
        description: description.as_ptr(),
    }
}

/// RetroPad の釦と PORT_A・PORT_B のビット。
const BUTTONS: [(c_uint, u16); 8] = [
    (RETRO_DEVICE_ID_JOYPAD_UP, 0x01),
    (RETRO_DEVICE_ID_JOYPAD_DOWN, 0x02),
    (RETRO_DEVICE_ID_JOYPAD_LEFT, 0x04),
    (RETRO_DEVICE_ID_JOYPAD_RIGHT, 0x08),
    (RETRO_DEVICE_ID_JOYPAD_Y, 0x10),
    (RETRO_DEVICE_ID_JOYPAD_B, 0x20),
    (RETRO_DEVICE_ID_JOYPAD_A, 0x40),
    (RETRO_DEVICE_ID_JOYPAD_START, 0x80),
];

static DESCRIPTORS: Table<[RetroInputDescriptor; 17]> = Table([
    pad(0, RETRO_DEVICE_ID_JOYPAD_UP, c"Up"),
    pad(0, RETRO_DEVICE_ID_JOYPAD_DOWN, c"Down"),
    pad(0, RETRO_DEVICE_ID_JOYPAD_LEFT, c"Left"),
    pad(0, RETRO_DEVICE_ID_JOYPAD_RIGHT, c"Right"),
    pad(0, RETRO_DEVICE_ID_JOYPAD_Y, c"A"),
    pad(0, RETRO_DEVICE_ID_JOYPAD_B, c"B"),
    pad(0, RETRO_DEVICE_ID_JOYPAD_A, c"C"),
    pad(0, RETRO_DEVICE_ID_JOYPAD_START, c"Start"),
    pad(1, RETRO_DEVICE_ID_JOYPAD_UP, c"Up"),
    pad(1, RETRO_DEVICE_ID_JOYPAD_DOWN, c"Down"),
    pad(1, RETRO_DEVICE_ID_JOYPAD_LEFT, c"Left"),
    pad(1, RETRO_DEVICE_ID_JOYPAD_RIGHT, c"Right"),
    pad(1, RETRO_DEVICE_ID_JOYPAD_Y, c"A"),
    pad(1, RETRO_DEVICE_ID_JOYPAD_B, c"B"),
    pad(1, RETRO_DEVICE_ID_JOYPAD_A, c"C"),
    pad(1, RETRO_DEVICE_ID_JOYPAD_START, c"Start"),
    RetroInputDescriptor {
        port: 0,
        device: 0,
        index: 0,
        id: 0,
        description: std::ptr::null(),
    },
]);

// ---------------------------------------------------------------------- 選択肢

#[derive(Clone, Copy, Debug, PartialEq)]
struct Options {
    area: Area,
    doc: bool,
    line: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            area: Area::View,
            doc: true,
            line: true,
        }
    }
}

fn variable(key: &CStr) -> Option<String> {
    let mut v = RetroVariable {
        key: key.as_ptr(),
        value: std::ptr::null(),
    };
    if !env(RETRO_ENVIRONMENT_GET_VARIABLE, (&raw mut v).cast()) || v.value.is_null() {
        return None;
    }
    // SAFETY: フロントエンドが 0 で終わる文字列を返した
    Some(
        unsafe { CStr::from_ptr(v.value) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn read_options() -> Options {
    let mut o = Options::default();
    match variable(c"fc85_area").as_deref() {
        Some("active") => o.area = Area::Active,
        Some("raster") => o.area = Area::Raster,
        _ => {}
    }
    o.doc = variable(c"fc85_doc").as_deref() != Some("disabled");
    o.line = variable(c"fc85_audio_tap").as_deref() != Some("digital");
    o
}

fn geometry(area: Area) -> RetroGameGeometry {
    let (w, h) = area.size();
    let (mw, mh) = Area::Raster.size();
    RetroGameGeometry {
        base_width: w as c_uint,
        base_height: h as c_uint,
        max_width: mw as c_uint,
        max_height: mh as c_uint,
        aspect_ratio: (w as f64 * PIXEL_ASPECT / h as f64) as f32,
    }
}

// ---------------------------------------------------------------------- コア

struct Core {
    m: Machine,
    opts: Options,
    pic: Vec<u32>,
    pcm: Vec<i16>,
    /// `retro_serialize_size` の値（読み込んだときに決め、変えない）。
    state_size: usize,
}

/// Main RAM の領域を動かさずに `f` を走らせる（状態を戻す・電源を入れ直すと `Console` ごと作り直すので、
/// 新しい中身を元の領域に写す）。フロントエンドは `retro_get_memory_data` の番地を覚えている。
fn keep_ram<T>(m: &mut Machine, f: impl FnOnce(&mut Machine) -> T) -> T {
    let mut old = std::mem::take(&mut m.console.sys.main_ram);
    let r = f(m);
    let new = &mut m.console.sys.main_ram;
    if new.is_empty() {
        // 作り直していない（誤り）
        *new = old;
    } else if new.len() == old.len() {
        old.copy_from_slice(new);
        *new = old;
    }
    r
}

fn system_dir() -> Option<PathBuf> {
    let mut p: *const c_char = std::ptr::null();
    if !env(RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY, (&raw mut p).cast()) || p.is_null() {
        return None;
    }
    // SAFETY: フロントエンドが 0 で終わる文字列を返した
    Some(PathBuf::from(
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned(),
    ))
}

/// `.bends` を読む：1 行に 1 つの命令、`#` から後ろは注。
fn read_bends(p: &Path) -> Result<Vec<String>, String> {
    let text =
        std::fs::read_to_string(p).map_err(|e| format!("{} を読めない: {e}", p.display()))?;
    Ok(text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// ゲーム（なければ None）から仮想機を作る。
///
/// # Safety
/// `game` は NULL か、フロントエンドが渡した `retro_game_info`。
unsafe fn load(game: *const RetroGameInfo) -> Result<Core, String> {
    let dir = system_dir().ok_or("system のフォルダが分からない")?;
    let boot_path = dir.join(BOOT_FILE);
    let boot = std::fs::read(&boot_path).map_err(|e| {
        message(&format!(
            "fc85: {BOOT_FILE} is missing in the system directory"
        ));
        format!("{} を読めない: {e}", boot_path.display())
    })?;
    let chars = match std::fs::read(dir.join(CHAR_FILE)) {
        Ok(c) => Some(c),
        Err(e) => {
            log(
                RETRO_LOG_WARN,
                &format!("{CHAR_FILE} がない（字が出ない）: {e}"),
            );
            None
        }
    };
    let (mut cart, mut bends) = (None, Vec::new());
    // SAFETY: 上の約束
    if let Some(g) = unsafe { game.as_ref() } {
        if !g.data.is_null() && g.size > 0 {
            // SAFETY: フロントエンドが data から size byte を読めると約束する
            cart =
                Some(unsafe { std::slice::from_raw_parts(g.data.cast::<u8>(), g.size) }.to_vec());
        } else if !g.path.is_null() {
            // SAFETY: 0 で終わる文字列
            let p = unsafe { CStr::from_ptr(g.path) }
                .to_string_lossy()
                .into_owned();
            cart = Some(std::fs::read(&p).map_err(|e| format!("{p} を読めない: {e}"))?);
        }
        if !g.path.is_null() {
            // SAFETY: 0 で終わる文字列
            let p = PathBuf::from(
                unsafe { CStr::from_ptr(g.path) }
                    .to_string_lossy()
                    .into_owned(),
            );
            let b = p.with_extension("bends");
            if b.exists() {
                bends = read_bends(&b)?;
                log(
                    RETRO_LOG_INFO,
                    &format!("ベンド点 {} 行（{}）", bends.len(), b.display()),
                );
            }
        }
    }
    let opts = read_options();
    let m = Machine::new(Config {
        boot: Some(boot),
        chars,
        cart,
        doc: opts.doc,
        bends,
        line: opts.line,
    })?;
    let state_size = STATE_HEAD + m.pack_state_bound()?;
    Ok(Core {
        m,
        opts,
        pic: Vec::new(),
        pcm: Vec::new(),
        state_size,
    })
}

impl Core {
    fn run(&mut self) {
        let mut changed = false;
        if env(
            RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,
            (&raw mut changed).cast(),
        ) && changed
        {
            let o = read_options();
            if o.area != self.opts.area {
                let mut g = geometry(o.area);
                env(RETRO_ENVIRONMENT_SET_GEOMETRY, (&raw mut g).cast());
            }
            self.opts = o;
        }
        let cb = lock(&CB);
        let (poll, input, video, batch) = (cb.poll, cb.input, cb.video, cb.batch);
        drop(cb);
        if let Some(poll) = poll {
            // SAFETY: フロントエンドの関数
            unsafe { poll() };
        }
        let mut ports = [0u16; 2];
        if let Some(input) = input {
            for (port, v) in ports.iter_mut().enumerate() {
                for &(id, bit) in &BUTTONS {
                    // SAFETY: フロントエンドの関数
                    if unsafe { input(port as c_uint, RETRO_DEVICE_JOYPAD, 0, id) } != 0 {
                        *v |= bit;
                    }
                }
            }
        }
        self.m.set_input(ports[0], ports[1]);
        self.m.run_frames(1);
        let (w, h) = self.m.screen_xrgb(self.opts.area, &mut self.pic);
        if let Some(video) = video {
            // SAFETY: pic は w×h 個。フロントエンドは呼んでいる間だけ読む
            unsafe { video(self.pic.as_ptr().cast(), w as c_uint, h as c_uint, w * 4) };
        }
        self.pcm.clear();
        for (l, r) in self.m.take_audio(false) {
            self.pcm.extend_from_slice(&[l, r]);
        }
        if let Some(batch) = batch {
            let mut done = 0;
            let total = self.pcm.len() / 2;
            while done < total {
                // SAFETY: pcm の done 組目から total - done 組
                let n = unsafe { batch(self.pcm.as_ptr().add(2 * done), total - done) };
                if n == 0 {
                    break;
                }
                done += n;
            }
        }
    }

    fn reset(&mut self) -> Result<(), String> {
        self.m.config.doc = self.opts.doc;
        self.m.config.line = self.opts.line;
        keep_ram(&mut self.m, Machine::reset)
    }

    fn serialize(&self, out: &mut [u8]) -> Result<(), String> {
        let s = self.m.pack_state()?;
        let n = STATE_HEAD + s.len();
        if n > out.len() {
            return Err(format!(
                "状態が {n} byte で、領域（{} byte）に入らない",
                out.len()
            ));
        }
        out[..STATE_HEAD].copy_from_slice(&(s.len() as u32).to_le_bytes());
        out[STATE_HEAD..n].copy_from_slice(&s);
        out[n..].fill(0);
        Ok(())
    }

    fn unserialize(&mut self, data: &[u8]) -> Result<(), String> {
        let head: [u8; STATE_HEAD] = data
            .get(..STATE_HEAD)
            .and_then(|h| h.try_into().ok())
            .ok_or("状態が短い")?;
        let n = u32::from_le_bytes(head) as usize;
        let body = data
            .get(STATE_HEAD..STATE_HEAD + n)
            .ok_or("状態の長さが合わない")?;
        keep_ram(&mut self.m, |m| m.unpack_state(body))
    }
}

/// `f` を走らせ、誤りと panic を記録に書いて `default` を返す。panic したらゲームを止める。
fn guarded<T>(what: &str, default: T, f: impl FnOnce(&mut Option<Core>) -> Result<T, String>) -> T {
    let mut core = lock(&CORE);
    match catch_unwind(AssertUnwindSafe(|| f(&mut core))) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            log(RETRO_LOG_ERROR, &format!("{what}: {e}"));
            default
        }
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "不明".into());
            log(
                RETRO_LOG_ERROR,
                &format!("{what}: エミュレータの中で panic（ゲームを止める）：{msg}"),
            );
            message("fc85: emulator error (see log)");
            *core = None;
            default
        }
    }
}

// ---------------------------------------------------------------------- libretro の API

#[unsafe(no_mangle)]
pub extern "C" fn retro_api_version() -> c_uint {
    RETRO_API_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_set_environment(f: Option<EnvironmentFn>) {
    lock(&CB).env = f;
    let mut cb = RetroLogCallback { log: None };
    if env(RETRO_ENVIRONMENT_GET_LOG_INTERFACE, (&raw mut cb).cast()) {
        lock(&CB).log = cb.log;
    }
    let mut yes = true;
    env(RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME, (&raw mut yes).cast());
    env(
        RETRO_ENVIRONMENT_SET_VARIABLES,
        VARIABLES.0.as_ptr().cast_mut().cast(),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_set_video_refresh(f: Option<VideoRefreshFn>) {
    lock(&CB).video = f;
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_set_audio_sample(_f: Option<AudioSampleFn>) {}

#[unsafe(no_mangle)]
pub extern "C" fn retro_set_audio_sample_batch(f: Option<AudioSampleBatchFn>) {
    lock(&CB).batch = f;
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_set_input_poll(f: Option<InputPollFn>) {
    lock(&CB).poll = f;
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_set_input_state(f: Option<InputStateFn>) {
    lock(&CB).input = f;
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_init() {}

#[unsafe(no_mangle)]
pub extern "C" fn retro_deinit() {
    *lock(&CORE) = None;
}

/// # Safety
/// `info` は書ける `retro_system_info`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_get_system_info(info: *mut RetroSystemInfo) {
    // SAFETY: 上の約束
    if let Some(i) = unsafe { info.as_mut() } {
        *i = RetroSystemInfo {
            library_name: c"fc85".as_ptr(),
            library_version: concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast(),
            valid_extensions: c"bin".as_ptr(),
            need_fullpath: false,
            block_extract: false,
        };
    }
}

/// # Safety
/// `info` は書ける `retro_system_av_info`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_get_system_av_info(info: *mut RetroSystemAvInfo) {
    let area = lock(&CORE).as_ref().map_or(Area::View, |c| c.opts.area);
    // SAFETY: 上の約束
    if let Some(i) = unsafe { info.as_mut() } {
        *i = RetroSystemAvInfo {
            geometry: geometry(area),
            timing: RetroSystemTiming {
                fps: FRAME_RATE,
                sample_rate: f64::from(AUDIO_RATE),
            },
        };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_set_controller_port_device(_port: c_uint, _device: c_uint) {}

#[unsafe(no_mangle)]
pub extern "C" fn retro_reset() {
    guarded("reset", (), |c| match c {
        Some(c) => c.reset(),
        None => Ok(()),
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_run() {
    guarded("run", (), |c| {
        if let Some(c) = c {
            c.run();
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_serialize_size() -> usize {
    lock(&CORE).as_ref().map_or(0, |c| c.state_size)
}

/// # Safety
/// `data` は `size` byte を書ける。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_serialize(data: *mut c_void, size: usize) -> bool {
    guarded("serialize", false, |c| {
        let c = c.as_ref().ok_or("ゲームがない")?;
        if data.is_null() {
            return Err("data が NULL".into());
        }
        // SAFETY: 上の約束
        let out = unsafe { std::slice::from_raw_parts_mut(data.cast::<u8>(), size) };
        c.serialize(out)?;
        Ok(true)
    })
}

/// # Safety
/// `data` は `size` byte を読める。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_unserialize(data: *const c_void, size: usize) -> bool {
    guarded("unserialize", false, |c| {
        let c = c.as_mut().ok_or("ゲームがない")?;
        if data.is_null() {
            return Err("data が NULL".into());
        }
        // SAFETY: 上の約束
        let d = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
        c.unserialize(d)?;
        Ok(true)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_cheat_reset() {}

#[unsafe(no_mangle)]
pub extern "C" fn retro_cheat_set(_index: c_uint, _enabled: bool, _code: *const c_char) {}

/// # Safety
/// `game` は NULL か、フロントエンドが渡した `retro_game_info`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn retro_load_game(game: *const RetroGameInfo) -> bool {
    let mut fmt = RETRO_PIXEL_FORMAT_XRGB8888;
    if !env(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT, (&raw mut fmt).cast()) {
        log(RETRO_LOG_ERROR, "XRGB8888 を使えない");
        return false;
    }
    env(
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS,
        DESCRIPTORS.0.as_ptr().cast_mut().cast(),
    );
    guarded("load_game", false, |c| {
        // SAFETY: 上の約束
        *c = Some(unsafe { load(game) }?);
        Ok(true)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_load_game_special(
    _type: c_uint,
    _info: *const RetroGameInfo,
    _num: usize,
) -> bool {
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_unload_game() {
    *lock(&CORE) = None;
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_get_region() -> c_uint {
    RETRO_REGION_NTSC
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_get_memory_data(id: c_uint) -> *mut c_void {
    match (id, lock(&CORE).as_mut()) {
        (RETRO_MEMORY_SYSTEM_RAM, Some(c)) => c.m.console.sys.main_ram.as_mut_ptr().cast(),
        _ => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn retro_get_memory_size(id: c_uint) -> usize {
    match (id, lock(&CORE).as_ref()) {
        (RETRO_MEMORY_SYSTEM_RAM, Some(c)) => c.m.console.sys.main_ram.len(),
        _ => 0,
    }
}
