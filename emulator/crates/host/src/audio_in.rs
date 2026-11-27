//! Audio IN（背面の RCA。DR-129）にホストの音を入れる（仕様書 §12 の host 層）。
//!
//! 音の源は WAV（`--audio-in FILE`）か、ホストの録音の装置（`--audio-in-device N|default`。番号は
//! `--audio-list`）。どちらも 48 kHz の L・R にして、フレームごとに仮想機の Audio IN の列に足す。
//! 仮想機は LINE OUT の標本と同じ刻みで 1 つずつ使う。録音の装置と仮想機の速さはずれるので、列が
//! `MAX_LAG` を超えたら古いものを捨てる（ホストの安全層。仮想機は変えない）。足りなければ無音になる。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fc85_board::console::Console;
use fc85_board::wav::RATE;

/// 仮想機の列に溜める上限（約 4 フレーム）。これより遅れたら古いものを捨てる。
const MAX_LAG: usize = RATE as usize / 15;
/// WAV は、列がこれを切ったら足す（約 2 フレーム）。
const FILE_AHEAD: usize = RATE as usize / 30;

/// 録音の装置の一覧を出す。
pub fn list() {
    let host = cpal::default_host();
    let default = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();
    match host.input_devices() {
        Ok(devs) => {
            for (i, d) in devs.enumerate() {
                let name = d.name().unwrap_or_default();
                let mark = if name == default { "（既定）" } else { "" };
                println!("AUDIO IN {i}: {name}{mark}");
            }
        }
        Err(e) => eprintln!("録音の装置を探せない: {e}"),
    }
}

/// 線形補間で 48 kHz にする（標本を 1 つずつ入れる）。
struct Resampler {
    /// 入力の 1 標本の間に進む出力の標本の数の逆数（入力の周波数÷48 kHz）。
    step: f64,
    /// 次の出力の標本の位置（前の入力の標本を 0、今の標本を 1 とする）。
    pos: f64,
    prev: (f32, f32),
}

impl Resampler {
    fn new(rate: u32) -> Self {
        Self {
            step: f64::from(rate) / f64::from(RATE),
            pos: 1.0,
            prev: (0.0, 0.0),
        }
    }

    fn push(&mut self, cur: (f32, f32), out: &mut VecDeque<(i16, i16)>) {
        let to16 = |v: f64| (v * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
        while self.pos <= 1.0 {
            let f = self.pos;
            let mix = |a: f32, b: f32| f64::from(a) * (1.0 - f) + f64::from(b) * f;
            out.push_back((to16(mix(self.prev.0, cur.0)), to16(mix(self.prev.1, cur.1))));
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.prev = cur;
    }
}

/// 音の源。
pub enum Source {
    /// WAV（48 kHz の L・R）と、次に渡す位置。終わったら止まる。
    File { pcm: Vec<(i16, i16)>, pos: usize },
    /// 録音の装置。`ring` は録った音（48 kHz）。
    Device {
        _stream: cpal::Stream,
        ring: Arc<Mutex<VecDeque<(i16, i16)>>>,
    },
}

impl Source {
    /// WAV を読む。読めなければ理由を出して `None`。
    pub fn file(path: &str) -> Option<Self> {
        let data = std::fs::read(path)
            .map_err(|e| eprintln!("--audio-in {path}: {e}"))
            .ok()?;
        match fc85_board::wav::read(&data) {
            Ok(pcm) => {
                eprintln!(
                    "Audio IN：{path}（{:.2} 秒）",
                    pcm.len() as f64 / f64::from(RATE)
                );
                Some(Self::File { pcm, pos: 0 })
            }
            Err(e) => {
                eprintln!("--audio-in {path}: {e}");
                None
            }
        }
    }

    /// 録音の装置を開く。`sel` は `default` か `--audio-list` の番号。開けなければ理由を出して `None`。
    pub fn device(sel: &str) -> Option<Self> {
        let host = cpal::default_host();
        let dev = if sel == "default" {
            host.default_input_device()
        } else {
            let n: usize = sel.parse().ok()?;
            host.input_devices().ok()?.nth(n)
        };
        let Some(dev) = dev else {
            eprintln!("--audio-in-device {sel}: 装置がない");
            return None;
        };
        let cfg = match dev.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("--audio-in-device {sel}: {e}");
                return None;
            }
        };
        let channels = usize::from(cfg.channels());
        let rate = cfg.sample_rate().0;
        let config: cpal::StreamConfig = cfg.clone().into();
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        let err = |e| eprintln!("録音: {e}");
        let q = ring.clone();
        let mut rs = Resampler::new(rate);
        let mut take = move |frames: &mut dyn Iterator<Item = (f32, f32)>| {
            let mut q = q.lock().unwrap();
            for s in frames {
                rs.push(s, &mut q);
            }
            let over = q.len().saturating_sub(MAX_LAG);
            q.drain(..over);
        };
        let pair = move |f: &[f32]| (f[0], if f.len() > 1 { f[1] } else { f[0] });
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => dev.build_input_stream(
                &config,
                move |data: &[f32], _| take(&mut data.chunks(channels).map(pair)),
                err,
                None,
            ),
            cpal::SampleFormat::I16 => dev.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    take(&mut data.chunks(channels).map(|f| {
                        let v: Vec<f32> = f.iter().map(|&s| f32::from(s) / 32768.0).collect();
                        pair(&v)
                    }))
                },
                err,
                None,
            ),
            f => {
                eprintln!("--audio-in-device {sel}: 扱えない形式 {f:?}");
                return None;
            }
        };
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("--audio-in-device {sel}: {e}");
                return None;
            }
        };
        if let Err(e) = stream.play() {
            eprintln!("--audio-in-device {sel}: {e}");
            return None;
        }
        eprintln!(
            "Audio IN：{}（{rate} Hz、{channels} チャネル）",
            dev.name().unwrap_or_default()
        );
        Some(Self::Device {
            _stream: stream,
            ring,
        })
    }

    /// フレームの頭に、仮想機の Audio IN の列に足す。
    pub fn feed(&mut self, c: &mut Console) {
        match self {
            Self::File { pcm, pos } => {
                let have = c.sys.audio_in_pending();
                if have < FILE_AHEAD && *pos < pcm.len() {
                    let end = (*pos + 2 * FILE_AHEAD - have).min(pcm.len());
                    c.sys.audio_in(&pcm[*pos..end]);
                    *pos = end;
                }
            }
            Self::Device { ring, .. } => {
                let got: Vec<_> = ring.lock().unwrap().drain(..).collect();
                c.sys.audio_in(&got);
                let over = c.sys.audio_in_pending().saturating_sub(MAX_LAG);
                if over > 0 {
                    // 仮想機の列は先頭からしか捨てられないので、溜まりすぎたら一度空にする
                    c.sys.audio_in_clear();
                    let keep = got.len().min(MAX_LAG / 2);
                    c.sys.audio_in(&got[got.len() - keep..]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_keeps_the_rate() {
        let mut out = VecDeque::new();
        let mut r = Resampler::new(44_100);
        for i in 0..44_100 {
            let v = (i % 100) as f32 / 100.0;
            r.push((v, -v), &mut out);
        }
        assert!((47_999..=48_001).contains(&out.len()), "{}", out.len());
        let mut out = VecDeque::new();
        let mut r = Resampler::new(48_000);
        for v in [0.5f32, -0.5, 0.25] {
            r.push((v, v), &mut out);
        }
        let l: Vec<i16> = out.iter().map(|s| s.0).collect();
        assert_eq!(l, [16384, -16384, 8192]);
    }
}
