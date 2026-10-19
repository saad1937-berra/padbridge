// src-tauri/src/pipeline/normalize.rs
//! Passage des octets bruts (0..255) aux valeurs normalisées.

use crate::devices::model::{InputFrame, Stick};
use crate::devices::report_parser::RawReport;

/// Octet de stick → [-1, 1]. Le HID place « haut » à 0 : on inverse Y pour que haut soit positif.
pub fn stick(raw_x: u8, raw_y: u8) -> Stick {
    Stick {
        x: (f32::from(raw_x) - 127.5) / 127.5,
        y: (127.5 - f32::from(raw_y)) / 127.5,
    }
}

pub fn trigger(raw: u8) -> f32 {
    f32::from(raw) / 255.0
}

pub fn frame_from_raw(raw: &RawReport, seq: u64, timestamp_us: u64) -> InputFrame {
    InputFrame {
        seq,
        timestamp_us,
        buttons: raw.buttons,
        left: stick(raw.lx, raw.ly),
        right: stick(raw.rx, raw.ry),
        l2: trigger(raw.l2),
        r2: trigger(raw.r2),
        battery: raw.battery,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stick_extremes() {
        let top_left = stick(0, 0);
        assert_eq!((top_left.x, top_left.y), (-1.0, 1.0));
        let bottom_right = stick(255, 255);
        assert_eq!((bottom_right.x, bottom_right.y), (1.0, -1.0));
    }

    #[test]
    fn rest_position_is_close_to_center() {
        let s = stick(128, 127);
        assert!(s.x.abs() < 0.01 && s.y.abs() < 0.01);
    }

    #[test]
    fn trigger_range() {
        assert_eq!(trigger(0), 0.0);
        assert_eq!(trigger(255), 1.0);
    }
}
