//! MIDI の入出力（仕様書 §12 の host 層。DR-061）。
//!
//! ホストの MIDI IN の byte を ACIA の受信線に流し、ACIA が送り終えた byte を MIDI OUT に出す。
//! ACIA が通信速度（31.25 kbps）どおりに取り込むので、ホストは byte の並びだけを渡す。
//! MIDI OUT の装置はメッセージ単位でしか送れないので、byte の列をメッセージに組み立てる。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

/// 装置の一覧を出す。
pub fn list() {
    match MidiInput::new("fc85") {
        Ok(m) => {
            for (i, p) in m.ports().iter().enumerate() {
                println!("IN  {i}: {}", m.port_name(p).unwrap_or_default());
            }
        }
        Err(e) => eprintln!("MIDI IN を開けない: {e}"),
    }
    match MidiOutput::new("fc85") {
        Ok(m) => {
            for (i, p) in m.ports().iter().enumerate() {
                println!("OUT {i}: {}", m.port_name(p).unwrap_or_default());
            }
        }
        Err(e) => eprintln!("MIDI OUT を開けない: {e}"),
    }
}

pub struct Midi {
    _input: Option<MidiInputConnection<()>>,
    received: Arc<Mutex<VecDeque<u8>>>,
    output: Option<MidiOutputConnection>,
    out: Assembler,
}

impl Midi {
    /// `inp`・`out` は装置の番号（`--midi-list` の番号）。開けなければ警告を出してなしで動く。
    pub fn open(inp: Option<usize>, out: Option<usize>) -> Self {
        let received = Arc::new(Mutex::new(VecDeque::new()));
        let input = inp.and_then(|n| {
            let m = MidiInput::new("fc85 in").ok()?;
            let port = m.ports().get(n)?.clone();
            let q = received.clone();
            m.connect(
                &port,
                "fc85-in",
                move |_, msg, _| q.lock().unwrap().extend(msg.iter().copied()),
                (),
            )
            .map_err(|e| eprintln!("MIDI IN {n} を開けない: {e}"))
            .ok()
        });
        let output = out.and_then(|n| {
            let m = MidiOutput::new("fc85 out").ok()?;
            let port = m.ports().get(n)?.clone();
            m.connect(&port, "fc85-out")
                .map_err(|e| eprintln!("MIDI OUT {n} を開けない: {e}"))
                .ok()
        });
        if inp.is_some() && input.is_none() {
            eprintln!("MIDI IN {} が見つからない（--midi-list）", inp.unwrap_or(0));
        }
        if out.is_some() && output.is_none() {
            eprintln!(
                "MIDI OUT {} が見つからない（--midi-list）",
                out.unwrap_or(0)
            );
        }
        Self {
            _input: input,
            received,
            output,
            out: Assembler::default(),
        }
    }

    /// 受けた byte を取り出す。
    pub fn take_received(&self) -> Vec<u8> {
        self.received.lock().unwrap().drain(..).collect()
    }

    /// ACIA が送った byte を、メッセージにそろったものから送る。
    pub fn send(&mut self, b: u8) {
        if let Some(msg) = self.out.push(b)
            && let Some(o) = &mut self.output
        {
            let _ = o.send(&msg);
        }
    }
}

/// byte の列を MIDI のメッセージに組み立てる（ランニングステータス、リアルタイム、SysEx）。
#[derive(Default)]
pub struct Assembler {
    status: Option<u8>,
    data: Vec<u8>,
    sysex: Option<Vec<u8>>,
}

impl Assembler {
    /// 1 byte を足し、メッセージがそろえば返す。
    pub fn push(&mut self, b: u8) -> Option<Vec<u8>> {
        if b >= 0xF8 {
            // リアルタイム：ほかのメッセージの途中にも入る
            return Some(vec![b]);
        }
        if let Some(sx) = &mut self.sysex {
            sx.push(b);
            if b == 0xF7 {
                return self.sysex.take();
            }
            if b & 0x80 == 0 {
                return None;
            }
            // F7 の前に別のステータスが来た：SysEx を捨てて読み直す
            self.sysex = None;
        }
        if b == 0xF0 {
            self.sysex = Some(vec![b]);
            self.status = None;
            return None;
        }
        if b & 0x80 != 0 {
            self.data.clear();
            if b >= 0xF0 {
                // システムコモン：ランニングステータスを消す
                self.status = None;
                return match b {
                    0xF6 => Some(vec![b]),
                    0xF1..=0xF3 => {
                        self.status = Some(b);
                        None
                    }
                    _ => None,
                };
            }
            self.status = Some(b);
            return None;
        }
        let s = self.status?;
        self.data.push(b);
        let need = match s & 0xF0 {
            0xC0 | 0xD0 => 1,
            0xF0 => match s {
                0xF2 => 2,
                _ => 1,
            },
            _ => 2,
        };
        if self.data.len() < need {
            return None;
        }
        let mut m = vec![s];
        m.append(&mut self.data);
        if s >= 0xF0 {
            self.status = None;
        }
        Some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::Assembler;

    fn feed(bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut a = Assembler::default();
        bytes.iter().filter_map(|&b| a.push(b)).collect()
    }

    #[test]
    fn running_status_and_realtime() {
        assert_eq!(
            feed(&[0x90, 60, 100, 62, 0xF8, 90, 0xC1, 5]),
            vec![
                vec![0x90, 60, 100],
                vec![0xF8],
                vec![0x90, 62, 90],
                vec![0xC1, 5]
            ]
        );
    }

    #[test]
    fn sysex_and_system_common() {
        assert_eq!(
            feed(&[0xF0, 0x7E, 0x01, 0xF7, 0xF2, 0x10, 0x20, 0xF6]),
            vec![
                vec![0xF0, 0x7E, 0x01, 0xF7],
                vec![0xF2, 0x10, 0x20],
                vec![0xF6]
            ]
        );
        // データだけ（ステータスなし）は捨てる
        assert!(feed(&[0x40, 0x41]).is_empty());
    }
}
