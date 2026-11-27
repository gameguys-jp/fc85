//! 振る舞いの指紋：決まった ROM・カートリッジ・ベンド点・入力で仮想機を動かし、画面・音・状態全体の
//! ハッシュを出す。速さのための書き換えの前後で比べ、振る舞いが変わっていないことを確かめる（DR-105）。
//! ハッシュは ROM とサンプルの組立てで変わるので、同じ組立てで比べる。リポジトリの根で
//! `cargo run --release --manifest-path emulator/Cargo.toml -p fc85-api --example fingerprint -- [フレーム数] [回数]`
use fc85_api::{Area, Config, Machine};

/// FNV-1a（64 bit）。Rust の版に左右されない。
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ u64::from(x)).wrapping_mul(0x100_0000_01b3);
        }
    }
}

fn read(p: &str) -> Option<Vec<u8>> {
    std::fs::read(p).ok()
}

fn main() {
    let frames: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(240);
    // 時間は何回か走らせた中の最短（ハッシュは毎回同じでなければならない）
    let repeat: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
        .max(1);
    let boot = read("firmware/boot/build/boot.bin").expect("firmware/boot/build/boot.bin");
    let chars = read("firmware/charrom/build/char.bin").expect("firmware/charrom/build/char.bin");
    let cases: Vec<(&str, Option<&str>, bool, Vec<&str>)> = vec![
        ("bios", None, true, vec![]),
        ("bios-nodoc", None, false, vec![]),
        (
            "showcase",
            Some("sdk/samples/showcase/build/showcase.bin"),
            true,
            vec![],
        ),
        (
            "game",
            Some("sdk/samples/game/build/game.bin"),
            true,
            vec![],
        ),
        ("dsp", Some("sdk/samples/dsp/build/dsp.bin"), true, vec![]),
        (
            "video",
            Some("sdk/samples/video/build/video.bin"),
            true,
            vec![],
        ),
        (
            "patches",
            Some("sdk/samples/patches/build/patches.bin"),
            true,
            vec![],
        ),
        (
            "bend",
            Some("sdk/samples/bend/build/bend.bin"),
            true,
            vec![],
        ),
        (
            "bend-clocks",
            Some("sdk/samples/showcase/build/showcase.bin"),
            true,
            vec![
                "mode bend",
                "clock doc 7300000",
                "clock video 23000000",
                "clock dsp 21000000",
                "dram temp 70",
            ],
        ),
        (
            "bend-sync",
            Some("sdk/samples/game/build/game.bin"),
            true,
            vec![
                "mode bend",
                "sync doc 1",
                "sync acrtc 0",
                "clock doc 8100000",
            ],
        ),
    ];
    let mut all = Fnv::new();
    let mut total = 0.0;
    let n = cases.len() as f64;
    for (name, cart, doc, bends) in cases {
        let config = Config {
            boot: Some(boot.clone()),
            chars: Some(chars.clone()),
            cart: cart.map(|p| read(p).unwrap_or_else(|| panic!("{p}"))),
            doc,
            bends: bends.iter().map(|s| s.to_string()).collect(),
            line: true,
        };
        let mut best = f64::MAX;
        let mut hashes = None;
        for _ in 0..repeat {
            let (h, el) = run(&config, frames);
            assert!(
                hashes.is_none_or(|x| x == h),
                "{name}：同じ入力でハッシュが変わった"
            );
            hashes = Some(h);
            best = best.min(el);
        }
        let h = hashes.unwrap();
        println!(
            "{name:12} 画面 {:016x} 音 {:016x} 状態 {:016x}  {:.2} ms/フレーム",
            h[0],
            h[1],
            h[2],
            best * 1000.0 / frames as f64
        );
        total += best * 1000.0 / frames as f64;
        for x in h {
            all.bytes(&x.to_le_bytes());
        }
    }
    println!("全体 {:016x}  平均 {:.2} ms/フレーム", all.0, total / n);
}

/// 1 回走らせて、画面・音・状態のハッシュと、かかった秒を返す。
fn run(config: &Config, frames: u64) -> ([u64; 3], f64) {
    let mut m = Machine::new(config.clone()).expect("組立て");
    let (mut screen, mut audio, mut state) = (Fnv::new(), Fnv::new(), Fnv::new());
    let mut pic = Vec::new();
    let start = std::time::Instant::now();
    for f in 0..frames {
        // 入力：数フレームごとに押す・離す
        let pad = [0u16, 0x0001, 0x0010, 0x0100, 0x0004, 0][(f / 7 % 6) as usize];
        m.set_input(pad, (f as u16).wrapping_mul(3) & 0x0F);
        m.run_frames(1);
        for (l, r) in m.take_audio(false) {
            audio.bytes(&l.to_le_bytes());
            audio.bytes(&r.to_le_bytes());
        }
        if f % 4 == 3 {
            m.screen_xrgb(Area::Raster, &mut pic);
            for p in &pic {
                screen.bytes(&p.to_le_bytes());
            }
        }
        if f % 60 == 59 {
            state.bytes(&m.save_state().expect("状態"));
        }
    }
    ([screen.0, audio.0, state.0], start.elapsed().as_secs_f64())
}
