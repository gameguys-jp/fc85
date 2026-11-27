//! Audio IN に渡す音を用意する道具（DR-129）：WAV の読込み、48 kHz への変換、試験の正弦波。
//!
//! ホストの側の便宜で、機械の模型ではない。WAV は PCM の 8・16・24・32 bit と、32 bit の浮動小数点
//! （WAVE_FORMAT_EXTENSIBLE を含む）。1 チャネルは L・R の両方に、3 チャネル以上は最初の 2 つを使う。
//! 48 kHz でなければ線形補間で変える。

/// Audio IN の標本の周波数（LINE OUT と同じ）。
pub const RATE: u32 = 48_000;

fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// WAV のバイト列を読み、48 kHz の L・R の標本にする。
pub fn read(data: &[u8]) -> Result<Vec<(i16, i16)>, String> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("WAV（RIFF/WAVE）ではない".into());
    }
    let mut fmt = None;
    let mut body = None;
    let mut i = 12;
    while i + 8 <= data.len() {
        let id = &data[i..i + 4];
        let len = u32le(data, i + 4) as usize;
        let start = i + 8;
        let end = start.saturating_add(len).min(data.len());
        match id {
            b"fmt " if end - start >= 16 => fmt = Some(&data[start..end]),
            b"data" => body = Some(&data[start..end]),
            _ => {}
        }
        i = start.saturating_add(len + (len & 1));
    }
    let fmt = fmt.ok_or("fmt の塊がない")?;
    let body = body.ok_or("data の塊がない")?;
    let mut tag = u16le(fmt, 0);
    let channels = usize::from(u16le(fmt, 2));
    let rate = u32le(fmt, 4);
    let bits = usize::from(u16le(fmt, 14));
    if tag == 0xFFFE && fmt.len() >= 26 {
        tag = u16le(fmt, 24);
    }
    if channels == 0 || rate == 0 {
        return Err("チャネルか標本の周波数が 0".into());
    }
    let bytes = bits / 8;
    let sample: fn(&[u8]) -> f64 = match (tag, bits) {
        (1, 8) => |b| (f64::from(b[0]) - 128.0) / 128.0,
        (1, 16) => |b| f64::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0,
        (1, 24) => |b| f64::from(i32::from_le_bytes([0, b[0], b[1], b[2]])) / 2_147_483_648.0,
        (1, 32) => |b| f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])) / 2_147_483_648.0,
        (3, 32) => |b| f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        _ => return Err(format!("扱えない形式（format {tag}、{bits} bit）")),
    };
    let frame = bytes * channels;
    let to16 = |v: f64| (v * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
    let pcm: Vec<(i16, i16)> = body
        .chunks_exact(frame)
        .map(|f| {
            let l = sample(&f[..bytes]);
            let r = if channels > 1 {
                sample(&f[bytes..2 * bytes])
            } else {
                l
            };
            (to16(l), to16(r))
        })
        .collect();
    Ok(resample(&pcm, rate, RATE))
}

/// 線形補間で標本の周波数を `from` から `to` にする。
pub fn resample(src: &[(i16, i16)], from: u32, to: u32) -> Vec<(i16, i16)> {
    if from == to || src.is_empty() {
        return src.to_vec();
    }
    let n = (src.len() as u64 * u64::from(to) / u64::from(from)) as usize;
    let step = f64::from(from) / f64::from(to);
    (0..n)
        .map(|k| {
            let x = k as f64 * step;
            let i = x.floor() as usize;
            let f = x - i as f64;
            let a = src[i.min(src.len() - 1)];
            let b = src[(i + 1).min(src.len() - 1)];
            let mix = |p: i16, q: i16| (f64::from(p) * (1.0 - f) + f64::from(q) * f).round() as i16;
            (mix(a.0, b.0), mix(a.1, b.1))
        })
        .collect()
}

/// 試験の正弦波：`hz` Hz、振幅 `amp`（1.0 で ±32767）、`ms` ミリ秒。L・R は同じ。
pub fn tone(hz: f64, amp: f64, ms: u32) -> Vec<(i16, i16)> {
    let n = (u64::from(RATE) * u64::from(ms) / 1000) as usize;
    (0..n)
        .map(|k| {
            let v =
                (amp * 32767.0 * (std::f64::consts::TAU * hz * k as f64 / f64::from(RATE)).sin())
                    .round()
                    .clamp(-32768.0, 32767.0) as i16;
            (v, v)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(rate: u32, channels: u16, bits: u16, tag: u16, body: &[u8]) -> Vec<u8> {
        let mut v = b"RIFF".to_vec();
        v.extend((36 + body.len() as u32).to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend(16u32.to_le_bytes());
        v.extend(tag.to_le_bytes());
        v.extend(channels.to_le_bytes());
        v.extend(rate.to_le_bytes());
        let align = channels * bits / 8;
        v.extend((rate * u32::from(align)).to_le_bytes());
        v.extend(align.to_le_bytes());
        v.extend(bits.to_le_bytes());
        v.extend(b"data");
        v.extend((body.len() as u32).to_le_bytes());
        v.extend(body);
        v
    }

    #[test]
    fn reads_16_bit_stereo_and_8_bit_mono() {
        let body: Vec<u8> = [1000i16, -2000, 32767, -32768]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        assert_eq!(
            read(&wav(48_000, 2, 16, 1, &body)).unwrap(),
            [(1000, -2000), (32767, -32768)]
        );
        assert_eq!(
            read(&wav(48_000, 1, 8, 1, &[128, 255, 0])).unwrap(),
            [(0, 0), (32512, 32512), (-32768, -32768)]
        );
        let f: Vec<u8> = [0.5f32, -0.25]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        assert_eq!(
            read(&wav(48_000, 1, 32, 3, &f)).unwrap(),
            [(16384, 16384), (-8192, -8192)]
        );
        assert!(read(b"RIFF\0\0\0\0WAVE").is_err());
    }

    #[test]
    fn resamples_by_linear_interpolation() {
        let src = [(0, 0), (100, -100)];
        assert_eq!(
            resample(&src, 24_000, 48_000),
            [(0, 0), (50, -50), (100, -100), (100, -100)]
        );
        assert_eq!(
            read(&wav(24_000, 1, 16, 1, &[0, 0, 100, 0])).unwrap().len(),
            4
        );
        assert_eq!(tone(1000.0, 0.5, 10).len(), 480);
    }
}
