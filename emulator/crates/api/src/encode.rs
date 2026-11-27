//! 外へ渡す形：PNG（画面）、WAV（音）、base64（JSON の中のバイト列）、画面の要約の値。

use std::io::Write;

/// RGB（1 画素 3 byte、行は上から）を PNG にする。8 bit・RGB、フィルタなし、zlib は既定の圧縮。
pub fn png_rgb(width: usize, height: usize, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(rgb.len(), width * height * 3);
    let mut raw = Vec::with_capacity(height * (width * 3 + 1));
    for row in rgb.chunks(width * 3) {
        raw.push(0); // フィルタの種類：なし
        raw.extend_from_slice(row);
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&raw).expect("メモリへの書込み");
    let idat = z.finish().expect("メモリへの書込み");

    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bit、RGB、deflate、フィルタ 0、インタレースなし
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = flate2::Crc::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(&crc.sum().to_be_bytes());
}

/// 16 bit・ステレオの WAV。
pub fn wav(rate: u32, pcm: &[(i16, i16)]) -> Vec<u8> {
    let bytes = (pcm.len() * 4) as u32;
    let mut d = Vec::with_capacity(44 + bytes as usize);
    d.extend_from_slice(b"RIFF");
    d.extend_from_slice(&(36 + bytes).to_le_bytes());
    d.extend_from_slice(b"WAVEfmt ");
    d.extend_from_slice(&16u32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&2u16.to_le_bytes());
    d.extend_from_slice(&rate.to_le_bytes());
    d.extend_from_slice(&(rate * 4).to_le_bytes());
    d.extend_from_slice(&4u16.to_le_bytes());
    d.extend_from_slice(&16u16.to_le_bytes());
    d.extend_from_slice(b"data");
    d.extend_from_slice(&bytes.to_le_bytes());
    for (l, r) in pcm {
        d.extend_from_slice(&l.to_le_bytes());
        d.extend_from_slice(&r.to_le_bytes());
    }
    d
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// base64（RFC 4648、詰め物 `=` あり）。
pub fn base64(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// base64 を戻す。空白は読み飛ばす。
pub fn unbase64(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for ch in s.bytes() {
        if ch.is_ascii_whitespace() || ch == b'=' {
            continue;
        }
        let v = B64
            .iter()
            .position(|&b| b == ch)
            .ok_or_else(|| format!("base64 ではない文字 {:?}", ch as char))?;
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// 大文字の 16 進（区切りなし）。
pub fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02X}")).collect()
}

/// FNV-1a（64 bit）。画面が前と同じかをすばやく比べる値（暗号の強さはない）。
pub fn fnv1a(data: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn base64_round_trips_and_matches_the_rfc_examples() {
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        let data: Vec<u8> = (0..=255).collect();
        assert_eq!(unbase64(&base64(&data)).unwrap(), data);
        assert!(unbase64("Zm9v!").is_err());
    }

    #[test]
    fn png_has_valid_chunks_and_the_pixels_come_back() {
        let (w, h) = (3, 2);
        let rgb: Vec<u8> = (0..(w * h * 3) as u8).collect();
        let png = png_rgb(w, h, &rgb);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let mut pos = 8;
        let mut idat = Vec::new();
        let mut kinds = Vec::new();
        while pos < png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let kind = &png[pos + 4..pos + 8];
            let data = &png[pos + 8..pos + 8 + len];
            let crc = u32::from_be_bytes(png[pos + 8 + len..pos + 12 + len].try_into().unwrap());
            let mut c = flate2::Crc::new();
            c.update(kind);
            c.update(data);
            assert_eq!(c.sum(), crc);
            if kind == b"IDAT" {
                idat.extend_from_slice(data);
            }
            kinds.push(String::from_utf8(kind.to_vec()).unwrap());
            pos += 12 + len;
        }
        assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(&idat[..])
            .read_to_end(&mut raw)
            .unwrap();
        let rows: Vec<u8> = raw
            .chunks(w * 3 + 1)
            .flat_map(|r| {
                assert_eq!(r[0], 0);
                r[1..].to_vec()
            })
            .collect();
        assert_eq!(rows, rgb);
    }

    #[test]
    fn wav_header_has_the_sizes() {
        let w = wav(48_000, &[(1, -1), (2, -2)]);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(w[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 8);
        assert_eq!(w.len(), 44 + 8);
    }
}
