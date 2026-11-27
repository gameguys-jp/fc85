//! libretro のコアの試験。フロントエンドの代わりの呼び返しを渡し、libretro の API の順に呼ぶ。コアは
//! 大域の状態なので、1 つの試験の中で順に確かめる。Boot ROM・文字 ROM がなければ飛ばす。

use std::ffi::{CStr, CString, c_char, c_uint, c_void};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};

use super::*;

#[derive(Default)]
struct Seen {
    /// 最後の映像：幅、高さ、pitch、中身の FNV-1a。
    video: Option<(c_uint, c_uint, usize, u64)>,
    videos: usize,
    audio_frames: usize,
    geometry: Option<(c_uint, c_uint)>,
    variables: usize,
    descriptors: usize,
    messages: Vec<String>,
}

static SEEN: Mutex<Seen> = Mutex::new(Seen {
    video: None,
    videos: 0,
    audio_frames: 0,
    geometry: None,
    variables: 0,
    descriptors: 0,
    messages: Vec::new(),
});
static SYSTEM_DIR: Mutex<Option<CString>> = Mutex::new(None);
static AREA: Mutex<&'static CStr> = Mutex::new(c"view");
static UPDATED: AtomicBool = AtomicBool::new(false);
static PAD: AtomicU16 = AtomicU16::new(0);
static POLLS: AtomicUsize = AtomicUsize::new(0);

fn seen() -> MutexGuard<'static, Seen> {
    lock(&SEEN)
}

fn fnv(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

unsafe extern "C" fn environment(cmd: c_uint, data: *mut c_void) -> bool {
    // SAFETY: data は cmd の約束の形（このコアが渡す）
    unsafe {
        match cmd {
            RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY => {
                let dir = lock(&SYSTEM_DIR);
                *data.cast::<*const c_char>() =
                    dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
                true
            }
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
                *data.cast::<c_uint>() == RETRO_PIXEL_FORMAT_XRGB8888
            }
            RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME => true,
            RETRO_ENVIRONMENT_SET_VARIABLES => {
                let mut v = data.cast::<RetroVariable>();
                while !(*v).key.is_null() {
                    seen().variables += 1;
                    v = v.add(1);
                }
                true
            }
            RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS => {
                let mut d = data.cast::<RetroInputDescriptor>();
                while !(*d).description.is_null() {
                    seen().descriptors += 1;
                    d = d.add(1);
                }
                true
            }
            RETRO_ENVIRONMENT_GET_VARIABLE => {
                let v = &mut *data.cast::<RetroVariable>();
                if CStr::from_ptr(v.key) == c"fc85_area" {
                    v.value = lock(&AREA).as_ptr();
                    true
                } else {
                    false
                }
            }
            RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => {
                *data.cast::<bool>() = UPDATED.swap(false, Ordering::SeqCst);
                true
            }
            RETRO_ENVIRONMENT_SET_GEOMETRY => {
                let g = &*data.cast::<RetroGameGeometry>();
                seen().geometry = Some((g.base_width, g.base_height));
                true
            }
            RETRO_ENVIRONMENT_SET_MESSAGE => {
                let m = &*data.cast::<RetroMessage>();
                seen()
                    .messages
                    .push(CStr::from_ptr(m.msg).to_string_lossy().into_owned());
                true
            }
            _ => false,
        }
    }
}

unsafe extern "C" fn video(data: *const c_void, w: c_uint, h: c_uint, pitch: usize) {
    // SAFETY: コアは pitch×h byte を渡す
    let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), pitch * h as usize) };
    let mut s = seen();
    s.video = Some((w, h, pitch, fnv(bytes)));
    s.videos += 1;
}

unsafe extern "C" fn audio_batch(_data: *const i16, frames: usize) -> usize {
    // 一度に 500 組まで受ける（コアは残りを繰り返して渡す）
    let n = frames.min(500);
    seen().audio_frames += n;
    n
}

unsafe extern "C" fn input_poll() {
    POLLS.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "C" fn input_state(port: c_uint, device: c_uint, _index: c_uint, id: c_uint) -> i16 {
    if port != 0 || device != RETRO_DEVICE_JOYPAD {
        return 0;
    }
    let bit = BUTTONS.iter().find(|b| b.0 == id).map_or(0, |b| b.1);
    i16::from(PAD.load(Ordering::SeqCst) & bit != 0)
}

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(rel)
}

fn run(n: usize) {
    for _ in 0..n {
        retro_run();
    }
}

fn last_video() -> (c_uint, c_uint, usize, u64) {
    seen().video.expect("映像が来ない")
}

fn pc() -> u32 {
    lock(&CORE).as_ref().expect("ゲーム").m.console.cpu.pc
}

#[test]
fn a_frontend_session() {
    let (Ok(boot), Ok(chars)) = (
        std::fs::read(repo("firmware/boot/build/boot.bin")),
        std::fs::read(repo("firmware/charrom/build/char.bin")),
    ) else {
        eprintln!("Boot ROM か文字 ROM がない（firmware を組む）。飛ばす");
        return;
    };
    let dir = std::env::temp_dir().join(format!("fc85-libretro-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(BOOT_FILE), &boot).unwrap();
    std::fs::write(dir.join(CHAR_FILE), &chars).unwrap();
    *lock(&SYSTEM_DIR) = Some(CString::new(dir.to_string_lossy().as_bytes()).unwrap());

    assert_eq!(retro_api_version(), 1);
    retro_set_environment(Some(environment));
    assert_eq!(seen().variables, 3, "選択肢");
    retro_set_video_refresh(Some(video));
    retro_set_audio_sample_batch(Some(audio_batch));
    retro_set_input_poll(Some(input_poll));
    retro_set_input_state(Some(input_state));
    retro_init();

    // ゲームなし：Boot ROM だけ
    // SAFETY: NULL はゲームなし
    assert!(unsafe { retro_load_game(std::ptr::null()) });
    assert_eq!(seen().descriptors, 16);
    let mut av = RetroSystemAvInfo {
        geometry: geometry(Area::Active),
        timing: RetroSystemTiming {
            fps: 0.0,
            sample_rate: 0.0,
        },
    };
    // SAFETY: 書ける
    unsafe { retro_get_system_av_info(&mut av) };
    assert_eq!(
        (av.geometry.base_width, av.geometry.base_height),
        (320, 240)
    );
    assert_eq!((av.geometry.max_width, av.geometry.max_height), (342, 262));
    assert!((f64::from(av.geometry.aspect_ratio) - 320.0 * 8.0 / 7.0 / 240.0).abs() < 1e-4);
    assert!((av.timing.fps - FRAME_RATE).abs() < 1e-9 && av.timing.sample_rate == 48_000.0);
    run(60);
    let (w, h, pitch, _) = last_video();
    assert_eq!((w, h, pitch), (320, 240, 1280));
    assert_eq!(seen().videos, 60);
    assert_eq!(POLLS.load(Ordering::SeqCst), 60);
    let per = seen().audio_frames as f64 / 60.0;
    assert!(
        (per - 48_000.0 / FRAME_RATE).abs() < 2.0,
        "音 {per} 組／フレーム"
    );
    retro_unload_game();

    // カートリッジ（中身で渡す）
    let Ok(cart) = std::fs::read(repo("sdk/samples/game/build/game.bin")) else {
        eprintln!("game.bin がない（py sdk/samples/game/build.py）。ここまで");
        return;
    };
    let info = RetroGameInfo {
        path: std::ptr::null(),
        data: cart.as_ptr().cast(),
        size: cart.len(),
        meta: std::ptr::null(),
    };
    // SAFETY: 中身は cart
    assert!(unsafe { retro_load_game(&info) });
    let ram = retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM);
    assert!(!ram.is_null());
    assert_eq!(retro_get_memory_size(RETRO_MEMORY_SYSTEM_RAM), 0x2_0000);
    let size = retro_serialize_size();
    assert!(size > 4 && size < 8 << 20, "{size}");
    run(120);

    // 状態：保存して、入力を変えて進め、戻して同じ入力で進めると同じ画面
    let mut st = vec![0u8; size];
    // SAFETY: size byte
    assert!(unsafe { retro_serialize(st.as_mut_ptr().cast(), st.len()) });
    PAD.store(0x08, Ordering::SeqCst); // 右
    run(30);
    let a = (last_video(), pc());
    // SAFETY: size byte
    assert!(unsafe { retro_unserialize(st.as_ptr().cast(), st.len()) });
    assert_eq!(
        retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM),
        ram,
        "Main RAM の番地が動いた"
    );
    run(30);
    assert_eq!((last_video(), pc()), a, "戻して同じ入力で進めた結果が違う");
    PAD.store(0, Ordering::SeqCst);
    // 短すぎる領域・壊れた状態は誤り（ゲームは続く）
    // SAFETY: 長さどおり
    assert!(!unsafe { retro_serialize(st.as_mut_ptr().cast(), 10) });
    // SAFETY: 長さどおり
    assert!(!unsafe { retro_unserialize(b"\x10\0\0\0garbage".as_ptr().cast(), 11) });
    // 大きさの上限は変わらない
    assert_eq!(retro_serialize_size(), size);

    // 範囲を変える：形を知らせ、次のフレームからその大きさ
    *lock(&AREA) = c"active";
    UPDATED.store(true, Ordering::SeqCst);
    run(1);
    assert_eq!(seen().geometry, Some((256, 192)));
    let (w, h, pitch, _) = last_video();
    assert_eq!((w, h, pitch), (256, 192, 1024));

    // リセットしても Main RAM の番地は同じ
    retro_reset();
    assert_eq!(retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM), ram);
    run(2);
    retro_unload_game();
    assert!(retro_get_memory_data(RETRO_MEMORY_SYSTEM_RAM).is_null());

    // ファイルの場所で渡し、横の .bends を読む
    let game = dir.join("game.bin");
    std::fs::write(&game, &cart).unwrap();
    std::fs::write(
        dir.join("game.bends"),
        "# DOC のクロックを上げる\nclock doc 9000000  # 試し\n\n",
    )
    .unwrap();
    let path = CString::new(game.to_string_lossy().as_bytes()).unwrap();
    let info = RetroGameInfo {
        path: path.as_ptr(),
        data: std::ptr::null(),
        size: 0,
        meta: std::ptr::null(),
    };
    // SAFETY: path は 0 で終わる
    assert!(unsafe { retro_load_game(&info) });
    {
        let core = lock(&CORE);
        let m = &core.as_ref().unwrap().m;
        assert_eq!(m.config.bends, ["clock doc 9000000"]);
        assert_eq!(m.mode(), "bend", "工場出荷と違うベンド点は Bend モード");
    }
    run(5);
    std::fs::write(dir.join("game.bends"), "clock nothing 1\n").unwrap();
    // SAFETY: path は 0 で終わる
    assert!(!unsafe { retro_load_game(&info) }, "読めないベンド点は誤り");

    // Boot ROM がない：読み込みに失敗し、画面に知らせる
    std::fs::remove_file(dir.join(BOOT_FILE)).unwrap();
    // SAFETY: NULL はゲームなし
    assert!(!unsafe { retro_load_game(std::ptr::null()) });
    assert!(
        seen().messages.iter().any(|m| m.contains(BOOT_FILE)),
        "{:?}",
        seen().messages
    );
    retro_run(); // ゲームがなくても止まらない
    retro_deinit();
    let _ = std::fs::remove_dir_all(&dir);
}
