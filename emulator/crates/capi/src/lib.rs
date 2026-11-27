//! fc85 のエミュレータを C の ABI でほかのソフトに組み込む窓口（DR-092 (4)、DR-104）。宣言は
//! `include/fc85emu.h`、説明は `sdk/docs/emulator-api.md` §08。
//!
//! 窓口の `Machine`（DR-097）の上に、組み込みでよく使う口（ROM をバイト列で渡して作る、フレームを進める、
//! 入力、画面、音、状態の保存と復元、メモリ）を C の関数で出す。ほかのことは `fc85_call` で JSON の手順
//! （1 行の要求と答え）をそのまま使う。同じものを wasm32-unknown-unknown に組むと WebAssembly の窓口になる
//! （`emulator/wasm/`）。
//!
//! 誤りは戻り値（-1 か NULL）で知らせ、文は `fc85_last_error`。関数の中で Rust が panic しても、呼んだ
//! ソフトを止めず誤りとして返す（WebAssembly では panic は止まる）。

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use fc85_api::protocol::{Session, handle_line};
use fc85_api::{Area, Config, Machine, MidiByte, Space};

/// C の ABI の版。関数の形を変えたら上げる（足すだけなら変えない）。
pub const ABI_VERSION: u32 = 1;

/// `fc85_new` の flags：DOC の子基板を挿さない。
pub const FLAG_NO_DOC: u32 = 0x01;
/// `fc85_new` の flags：音をチップの出力のデジタルの和にする（既定は LINE OUT。DR-054）。
pub const FLAG_DIGITAL_AUDIO: u32 = 0x02;

/// 仮想機と窓口の手順の相手。C からは中を見ない。
pub struct Fc85 {
    session: Session,
    pic: Vec<u32>,
    /// `fc85_audio` で取り残した音。
    pending: Vec<(i16, i16)>,
    /// `fc85_midi_out` で取り残した MIDI OUT の byte。
    midi: Vec<MidiByte>,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn set_error(msg: &str) {
    let c = CString::new(msg.replace('\0', " ")).unwrap_or_default();
    LAST_ERROR.with(|e| *e.borrow_mut() = c);
}

fn clear_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = CString::default());
}

/// `f` を走らせ、誤りと panic を `default` と `fc85_last_error` に替える。
fn guard<T>(default: T, f: impl FnOnce() -> Result<T, String>) -> T {
    clear_error();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            set_error(&e);
            default
        }
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "不明".into());
            set_error(&format!("エミュレータの中で panic：{msg}"));
            default
        }
    }
}

/// ポインタと長さをバイト列に。NULL か 0 なら None。
///
/// # Safety
/// `p` は `n` byte 読める。
unsafe fn bytes<'a>(p: *const u8, n: usize) -> Option<&'a [u8]> {
    if p.is_null() || n == 0 {
        None
    } else {
        // SAFETY: 呼ぶ側が p から n byte を読めると約束する
        Some(unsafe { std::slice::from_raw_parts(p, n) })
    }
}

/// # Safety
/// `m` は `fc85_new` が返したもので、まだ `fc85_free` していない（か NULL）。
unsafe fn handle<'a>(m: *mut Fc85) -> Result<&'a mut Fc85, String> {
    // SAFETY: 上の約束
    unsafe { m.as_mut() }.ok_or_else(|| "仮想機が NULL".into())
}

impl Fc85 {
    fn machine(&mut self) -> Result<&mut Machine, String> {
        self.session.current().ok_or_else(|| "仮想機がない".into())
    }
}

fn area(n: i32) -> Result<Area, String> {
    match n {
        0 => Ok(Area::Active),
        1 => Ok(Area::View),
        2 => Ok(Area::Raster),
        _ => Err(format!("area {n} はない（0 ACTIVE、1 VIEW、2 RASTER）")),
    }
}

fn space(n: i32) -> Result<Space, String> {
    Ok(match n {
        0 => Space::Cpu,
        1 => Space::Vram,
        2 => Space::Wave,
        3 => Space::DspProg,
        4 => Space::Temporal,
        5 => Space::CharRom,
        6 => Space::Palette,
        _ => return Err(format!("space {n} はない（0〜6。fc85emu.h）")),
    })
}

// ---------------------------------------------------------------------- 版と定数

#[unsafe(no_mangle)]
pub extern "C" fn fc85_abi_version() -> u32 {
    ABI_VERSION
}

/// エミュレータの版（"0.1.0" など）。静的な文字列。
#[unsafe(no_mangle)]
pub extern "C" fn fc85_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// 音の標本化の頻度（Hz）。
#[unsafe(no_mangle)]
pub extern "C" fn fc85_audio_rate() -> u32 {
    fc85_api::AUDIO_RATE
}

/// 仮想機のフレームの頻度（約 59.92 Hz）。
#[unsafe(no_mangle)]
pub extern "C" fn fc85_frame_rate() -> f64 {
    fc85_api::FRAME_RATE
}

/// 画面の幅・高さ（area：0 ACTIVE 256×192、1 VIEW 320×240、2 RASTER 342×262）。誤りなら 0。
#[unsafe(no_mangle)]
pub extern "C" fn fc85_screen_width(area_id: i32) -> u32 {
    area(area_id).map(|a| a.size().0 as u32).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn fc85_screen_height(area_id: i32) -> u32 {
    area(area_id).map(|a| a.size().1 as u32).unwrap_or(0)
}

/// 最後の誤りの文（UTF-8。同じスレッドの次の呼出しまで使える）。なければ空。
#[unsafe(no_mangle)]
pub extern "C" fn fc85_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ptr())
}

// ---------------------------------------------------------------------- 作る・壊す

/// 仮想機を作って電源を入れる。boot（Boot ROM）と chars（文字 ROM）は NULL でもよい（Boot ROM がなければ
/// 68000 はすぐ止まる）。flags は FC85_NO_DOC・FC85_DIGITAL_AUDIO の OR。誤りなら NULL。
///
/// # Safety
/// boot・chars は NULL か、それぞれ boot_len・chars_len byte を読める。呼んだ後は手放してよい（写す）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_new(
    boot: *const u8,
    boot_len: usize,
    chars: *const u8,
    chars_len: usize,
    flags: u32,
) -> *mut Fc85 {
    guard(std::ptr::null_mut(), || {
        let config = Config {
            // SAFETY: 上の約束
            boot: unsafe { bytes(boot, boot_len) }.map(<[u8]>::to_vec),
            // SAFETY: 上の約束
            chars: unsafe { bytes(chars, chars_len) }.map(<[u8]>::to_vec),
            cart: None,
            doc: flags & FLAG_NO_DOC == 0,
            bends: Vec::new(),
            line: flags & FLAG_DIGITAL_AUDIO == 0,
        };
        let m = Machine::new(config)?;
        Ok(Box::into_raw(Box::new(Fc85 {
            session: Session::with_machine(m),
            pic: Vec::new(),
            pending: Vec::new(),
            midi: Vec::new(),
        })))
    })
}

/// 壊す。NULL なら何もしない。
///
/// # Safety
/// `m` は `fc85_new` が返したもので、まだ壊していない（か NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_free(m: *mut Fc85) {
    if !m.is_null() {
        // SAFETY: 上の約束。Box::into_raw で作った
        drop(unsafe { Box::from_raw(m) });
    }
}

/// カートリッジを挿して（len が 0 なら抜いて）電源を入れ直す。0 か -1。
///
/// # Safety
/// `m` は生きている仮想機。cart は NULL か len byte を読める（写す）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_load_cart(m: *mut Fc85, cart: *const u8, len: usize) -> i32 {
    guard(-1, || {
        // SAFETY: 上の約束
        let h = unsafe { handle(m) }?;
        let mm = h.machine()?;
        // SAFETY: 上の約束
        mm.config.cart = unsafe { bytes(cart, len) }.map(<[u8]>::to_vec);
        mm.reset()?;
        h.pending.clear();
        h.midi.clear();
        Ok(0)
    })
}

/// 同じ ROM で電源を入れ直す。0 か -1。
///
/// # Safety
/// `m` は生きている仮想機。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_reset(m: *mut Fc85) -> i32 {
    guard(-1, || {
        // SAFETY: 上の約束
        let h = unsafe { handle(m) }?;
        h.machine()?.reset()?;
        h.pending.clear();
        h.midi.clear();
        Ok(0)
    })
}

// ---------------------------------------------------------------------- 進める

/// 入力を置く。port_a は PAD の bit（UP 1、DOWN 2、LEFT 4、RIGHT 8、A $10、B $20、C $40、START $80）。
///
/// # Safety
/// `m` は生きている仮想機。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_set_input(m: *mut Fc85, port_a: u16, port_b: u16) -> i32 {
    guard(-1, || {
        // SAFETY: 上の約束
        unsafe { handle(m) }?.machine()?.set_input(port_a, port_b);
        Ok(0)
    })
}

/// MIDI IN の線に data の len byte を置く（DR-126）。今の時刻から delay_us µs 後に線に来たとし、ACIA が
/// 31.25 kbit/s で 1 byte ずつ受け取る。前に置いた byte より早くはならない。0 か -1。
///
/// # Safety
/// `m` は生きている仮想機。data は NULL か len byte を読める。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_midi_in(
    m: *mut Fc85,
    data: *const u8,
    len: usize,
    delay_us: u32,
) -> i32 {
    guard(-1, || {
        // SAFETY: 上の約束
        let d = unsafe { bytes(data, len) }.unwrap_or(&[]);
        // SAFETY: 上の約束
        unsafe { handle(m) }?.machine()?.midi_in(delay_us, d);
        Ok(0)
    })
}

/// MIDI OUT：ACIA が送り終えた byte を古い順に out へ最大 cap 個書き、取り出す。time_us が NULL でなければ、
/// 送り終えた時刻（電源を入れてからの µs。fc85_time_us と同じ数え方）を同じ順に書く。書いた数か -1。
/// 残りは次に回す。
///
/// # Safety
/// `m` は生きている仮想機。out は cap byte、time_us は NULL か cap 個の u64 を書ける。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_midi_out(
    m: *mut Fc85,
    out: *mut u8,
    time_us: *mut u64,
    cap: usize,
) -> i32 {
    guard(-1, || {
        // SAFETY: 上の約束
        let h = unsafe { handle(m) }?;
        let new = h.machine()?.take_midi_out();
        h.midi.extend(new);
        let n = h.midi.len().min(cap).min(i32::MAX as usize);
        if n > 0 && out.is_null() {
            return Err("out が NULL".into());
        }
        for (i, b) in h.midi[..n].iter().enumerate() {
            // SAFETY: 上の約束（i < cap）。time_us は境界にそろっていなくてもよい
            unsafe {
                *out.add(i) = b.byte;
                if !time_us.is_null() {
                    time_us.add(i).write_unaligned(b.us);
                }
            }
        }
        h.midi.drain(..n);
        Ok(n as i32)
    })
}

/// 仮想機の今の時刻（電源を入れてからの µs。68000 のクロックから）。誤りなら 0。
///
/// # Safety
/// `m` は生きている仮想機。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_time_us(m: *mut Fc85) -> u64 {
    guard(0, || {
        // SAFETY: 上の約束
        Ok(unsafe { handle(m) }?.machine()?.time_us())
    })
}

/// n フレーム進める（ホストと同じ進め方。68000 が止まってもフレームは進む）。0 か -1。
///
/// # Safety
/// `m` は生きている仮想機。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_run_frames(m: *mut Fc85, n: u32) -> i32 {
    guard(-1, || {
        // SAFETY: 上の約束
        unsafe { handle(m) }?.machine()?.run_frames(u64::from(n));
        Ok(0)
    })
}

/// 電源を入れてからのフレームの数。
///
/// # Safety
/// `m` は生きている仮想機。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_frame(m: *mut Fc85) -> u64 {
    guard(0, || {
        // SAFETY: 上の約束
        Ok(unsafe { handle(m) }?.machine()?.frame())
    })
}

// ---------------------------------------------------------------------- 画面と音

/// 画面を 0x00RRGGBB で out に書く（左上から行ごと）。書いた画素の数（幅×高さ）か -1。
///
/// # Safety
/// `m` は生きている仮想機。out は out_len 個の u32 を書ける。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_screen(
    m: *mut Fc85,
    area_id: i32,
    out: *mut u32,
    out_len: usize,
) -> i32 {
    guard(-1, || {
        let a = area(area_id)?;
        // SAFETY: 上の約束
        let h = unsafe { handle(m) }?;
        let mut pic = std::mem::take(&mut h.pic);
        let (w, hh) = h.machine()?.screen_xrgb(a, &mut pic);
        let n = w * hh;
        let r = if out.is_null() || out_len < n {
            Err(format!("画面は {w}×{hh}＝{n} 画素（out_len は {out_len}）"))
        } else {
            // SAFETY: 上の約束（out は out_len ≥ n 個）
            unsafe { std::ptr::copy_nonoverlapping(pic.as_ptr(), out, n) };
            Ok(n as i32)
        };
        h.pic = pic;
        r
    })
}

/// 画面を R・G・B・A（A は 255）の byte の順で out に書く（ブラウザの ImageData の形。DR-107）。書いた画素の
/// 数（幅×高さ）か -1。
///
/// # Safety
/// `m` は生きている仮想機。out は out_len byte を書ける。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_screen_rgba(
    m: *mut Fc85,
    area_id: i32,
    out: *mut u8,
    out_len: usize,
) -> i32 {
    guard(-1, || {
        let a = area(area_id)?;
        // SAFETY: 上の約束
        let h = unsafe { handle(m) }?;
        let mut pic = std::mem::take(&mut h.pic);
        let (w, hh) = h.machine()?.screen_xrgb(a, &mut pic);
        let n = w * hh;
        let r = if out.is_null() || out_len < 4 * n {
            Err(format!(
                "画面は {w}×{hh}＝{n} 画素で {} byte（out_len は {out_len}）",
                4 * n
            ))
        } else {
            // SAFETY: 上の約束（out は out_len ≥ 4n byte）
            let dst = unsafe { std::slice::from_raw_parts_mut(out, 4 * n) };
            for (d, &p) in dst.chunks_exact_mut(4).zip(&pic) {
                let [_, r, g, b] = p.to_be_bytes();
                d.copy_from_slice(&[r, g, b, 255]);
            }
            Ok(n as i32)
        };
        h.pic = pic;
        r
    })
}

/// 溜まった音を out に L・R の順の s16 で書き、取り出す。書いた組の数（max_frames まで）。残りは次に回す。
///
/// # Safety
/// `m` は生きている仮想機。out は 2×max_frames 個の i16 を書ける。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_audio(m: *mut Fc85, out: *mut i16, max_frames: usize) -> i32 {
    guard(-1, || {
        // SAFETY: 上の約束
        let h = unsafe { handle(m) }?;
        let new = h.machine()?.take_audio(false);
        h.pending.extend(new);
        let n = h.pending.len().min(max_frames);
        if n > 0 && out.is_null() {
            return Err("out が NULL".into());
        }
        for (i, &(l, r)) in h.pending[..n].iter().enumerate() {
            // SAFETY: 上の約束（i < max_frames）
            unsafe {
                *out.add(2 * i) = l;
                *out.add(2 * i + 1) = r;
            }
        }
        h.pending.drain(..n);
        Ok(n as i32)
    })
}

// ---------------------------------------------------------------------- 状態

/// 状態を保存する。Rust の側で取った領域を返し、大きさを *len に書く。fc85_free_bytes で返す。誤りなら NULL。
/// 形は `STATE_MAGIC` ＋ deflate した JSON（窓口の `pack_state`）。同じ版のエミュレータで読める。
///
/// # Safety
/// `m` は生きている仮想機。len は書ける。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_save_state(m: *mut Fc85, len: *mut usize) -> *mut u8 {
    guard(std::ptr::null_mut(), || {
        if len.is_null() {
            return Err("len が NULL".into());
        }
        // SAFETY: 上の約束
        let data = unsafe { handle(m) }?
            .machine()?
            .pack_state()?
            .into_boxed_slice();
        // SAFETY: 上の約束
        unsafe { *len = data.len() };
        Ok(Box::into_raw(data).cast::<u8>())
    })
}

/// 保存した状態に戻す。0 か -1。
///
/// # Safety
/// `m` は生きている仮想機。data は len byte を読める。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_load_state(m: *mut Fc85, data: *const u8, len: usize) -> i32 {
    guard(-1, || {
        // SAFETY: 上の約束
        let d = unsafe { bytes(data, len) }.ok_or("状態が空")?;
        // SAFETY: 上の約束
        let h = unsafe { handle(m) }?;
        h.machine()?.unpack_state(d)?;
        h.pending.clear();
        h.midi.clear();
        Ok(0)
    })
}

/// `fc85_save_state` の領域を返す。
///
/// # Safety
/// p と len は `fc85_save_state` が返した組（か NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_free_bytes(p: *mut u8, len: usize) {
    if !p.is_null() {
        // SAFETY: 上の約束。Box<[u8]> を into_raw で作った
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(p, len)) });
    }
}

// ---------------------------------------------------------------------- メモリ

/// メモリを読む（バスを通さない。space は FC85_SPACE_*）。0 か -1。
///
/// # Safety
/// `m` は生きている仮想機。out は len byte を書ける。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_peek(
    m: *mut Fc85,
    space_id: i32,
    addr: u32,
    out: *mut u8,
    len: usize,
) -> i32 {
    guard(-1, || {
        let s = space(space_id)?;
        // SAFETY: 上の約束
        let data = unsafe { handle(m) }?.machine()?.peek(s, addr, len)?;
        if len > 0 {
            if out.is_null() {
                return Err("out が NULL".into());
            }
            // SAFETY: 上の約束
            unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), out, len) };
        }
        Ok(0)
    })
}

/// メモリを書く（バスを通さない。試験と改造の道具）。0 か -1。
///
/// # Safety
/// `m` は生きている仮想機。data は len byte を読める。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_poke(
    m: *mut Fc85,
    space_id: i32,
    addr: u32,
    data: *const u8,
    len: usize,
) -> i32 {
    guard(-1, || {
        let s = space(space_id)?;
        // SAFETY: 上の約束
        let d = unsafe { bytes(data, len) }.unwrap_or(&[]);
        // SAFETY: 上の約束
        unsafe { handle(m) }?.machine()?.poke(s, addr, d)?;
        Ok(0)
    })
}

// ---------------------------------------------------------------------- JSON の手順

/// JSON の手順の要求を 1 つ（1 行。sdk/docs/emulator-api.md）実行し、答えの行を返す。誤りの答え（error）も
/// 答えの行。fc85_free_string で返す。要求が空か文字列が読めなければ NULL。
///
/// # Safety
/// `m` は生きている仮想機。request は 0 で終わる UTF-8。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_call(m: *mut Fc85, request: *const c_char) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        if request.is_null() {
            return Err("要求が NULL".into());
        }
        // SAFETY: 上の約束
        let req = unsafe { CStr::from_ptr(request) }
            .to_str()
            .map_err(|_| "要求が UTF-8 ではない")?;
        // SAFETY: 上の約束
        let h = unsafe { handle(m) }?;
        let out = handle_line(&mut h.session, req).ok_or("要求が空")?;
        Ok(CString::new(out).map_err(|e| e.to_string())?.into_raw())
    })
}

/// `fc85_call` の答えを返す。
///
/// # Safety
/// s は `fc85_call` が返したもの（か NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_free_string(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: 上の約束。CString::into_raw で作った
        drop(unsafe { CString::from_raw(s) });
    }
}

// ---------------------------------------------------------------------- 領域（WebAssembly の呼ぶ側のため）

const ALIGN: usize = 8;

/// n byte の領域を取る（8 byte の境界）。WebAssembly の呼ぶ側が ROM や画面の置き場を作るため。誤りなら NULL。
#[unsafe(no_mangle)]
pub extern "C" fn fc85_alloc(n: usize) -> *mut u8 {
    match std::alloc::Layout::from_size_align(n.max(1), ALIGN) {
        // SAFETY: 大きさは 1 以上
        Ok(l) => unsafe { std::alloc::alloc(l) },
        Err(_) => std::ptr::null_mut(),
    }
}

/// `fc85_alloc` の領域を返す（n は取ったときと同じ）。
///
/// # Safety
/// p と n は `fc85_alloc` の組（か NULL）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fc85_dealloc(p: *mut u8, n: usize) {
    if p.is_null() {
        return;
    }
    if let Ok(l) = std::alloc::Layout::from_size_align(n.max(1), ALIGN) {
        // SAFETY: 上の約束
        unsafe { std::alloc::dealloc(p, l) };
    }
}
