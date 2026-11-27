//! ゲームパッド（DR-132）：gilrs でホストのゲームパッドを読み、PORT_A・PORT_B のビットにする（仕様書 §12 の
//! host 層）。
//!
//! 番号の小さい順に、1 台目を PORT_A（キーボードと OR）、2 台目を PORT_B にする。配置は Web の頁（DR-116）と
//! libretro（DR-104）と同じ：十字か左の棒が方向、下の釦（Xbox の A）が A、右（B）が B、左（X）が C、START が
//! START。棒は半分より倒したら押したことにする。つなぎ外しはフレームごとに見る。

use gilrs::{Axis, Button, EventType, Gilrs};

use crate::{PAD_A, PAD_B, PAD_C, PAD_DOWN, PAD_LEFT, PAD_RIGHT, PAD_START, PAD_UP};

/// 棒を押したことにする倒し方。
const STICK: f32 = 0.5;

/// 押している釦と左の棒（x は右、y は上が正。−1〜1）を、PORT_A・B のビットにする。
fn bits(pressed: impl Fn(Button) -> bool, x: f32, y: f32) -> u16 {
    let mut v = 0;
    for (b, bit) in [
        (Button::DPadUp, PAD_UP),
        (Button::DPadDown, PAD_DOWN),
        (Button::DPadLeft, PAD_LEFT),
        (Button::DPadRight, PAD_RIGHT),
        (Button::South, PAD_A),
        (Button::East, PAD_B),
        (Button::West, PAD_C),
        (Button::Start, PAD_START),
    ] {
        if pressed(b) {
            v |= bit;
        }
    }
    if y > STICK {
        v |= PAD_UP;
    }
    if y < -STICK {
        v |= PAD_DOWN;
    }
    if x < -STICK {
        v |= PAD_LEFT;
    }
    if x > STICK {
        v |= PAD_RIGHT;
    }
    v
}

/// ホストのゲームパッド。読めなければ（`--no-gamepad`、OS が対応しない）いつも 0。
pub struct Pads {
    g: Option<Gilrs>,
}

impl Pads {
    pub fn open(on: bool) -> Self {
        if !on {
            return Self { g: None };
        }
        let g = match Gilrs::new() {
            Ok(g) => g,
            Err(gilrs::Error::NotImplemented(_)) => {
                eprintln!("この OS ではゲームパッドを読めない");
                return Self { g: None };
            }
            Err(e) => {
                eprintln!("ゲームパッドを読めない: {e}");
                return Self { g: None };
            }
        };
        for (id, p) in g.gamepads() {
            eprintln!("ゲームパッド {}: {}", usize::from(id), p.name());
        }
        Self { g: Some(g) }
    }

    /// 今の PORT_A・PORT_B のビット。
    pub fn read(&mut self) -> [u16; 2] {
        let Some(g) = &mut self.g else {
            return [0; 2];
        };
        while let Some(ev) = g.next_event() {
            match ev.event {
                EventType::Connected => eprintln!(
                    "ゲームパッド {} をつないだ: {}",
                    usize::from(ev.id),
                    g.gamepad(ev.id).name()
                ),
                EventType::Disconnected => {
                    eprintln!("ゲームパッド {} を外した", usize::from(ev.id))
                }
                _ => {}
            }
        }
        let mut pads: Vec<_> = g.gamepads().collect();
        pads.sort_by_key(|(id, _)| usize::from(*id));
        let mut out = [0; 2];
        for (slot, (_, p)) in out.iter_mut().zip(pads) {
            *slot = bits(
                |b| p.is_pressed(b),
                p.value(Axis::LeftStickX),
                p.value(Axis::LeftStickY),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_and_the_stick_map_like_the_web_page() {
        let none = |_| false;
        assert_eq!(bits(none, 0.0, 0.0), 0);
        assert_eq!(bits(|b| b == Button::South, 0.0, 0.0), PAD_A);
        assert_eq!(bits(|b| b == Button::East, 0.0, 0.0), PAD_B);
        assert_eq!(bits(|b| b == Button::West, 0.0, 0.0), PAD_C);
        assert_eq!(
            bits(|b| b == Button::North, 0.0, 0.0),
            0,
            "上の釦は使わない"
        );
        assert_eq!(
            bits(|b| b == Button::Start || b == Button::DPadLeft, 0.0, 0.0),
            PAD_START | PAD_LEFT
        );
        assert_eq!(bits(none, 0.8, 0.9), PAD_RIGHT | PAD_UP, "棒の y は上が正");
        assert_eq!(bits(none, -0.6, -0.7), PAD_LEFT | PAD_DOWN);
        assert_eq!(bits(none, 0.4, -0.4), 0, "半分より倒さなければ押していない");
    }

    #[test]
    fn a_closed_pad_reads_nothing() {
        assert_eq!(Pads::open(false).read(), [0, 0]);
    }
}
