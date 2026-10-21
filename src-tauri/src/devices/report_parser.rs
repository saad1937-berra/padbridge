// src-tauri/src/devices/report_parser.rs
//! Décodage des rapports HID bruts DualShock 4 / DualSense.
//!
//! ⚠ Les dispositions d'octets ci-dessous suivent les formats publiquement documentés
//! (pilotes Linux hid-sony / hid-playstation, SDL). Elles doivent être confirmées sur
//! matériel réel pendant le POC A (USB + Bluetooth, DS4 + DualSense) avant la recette.

use super::model::{Battery, ButtonSet, ControllerModel, PsButton};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnsupportedModel,
    UnknownReport { id: u8 },
    TooShort { id: u8, len: usize, need: usize },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "rapport vide"),
            Self::UnsupportedModel => write!(f, "modèle non pris en charge"),
            Self::UnknownReport { id } => write!(f, "identifiant de rapport inconnu: {id:#04x}"),
            Self::TooShort { id, len, need } => {
                write!(f, "rapport {id:#04x} trop court: {len} octets, {need} attendus")
            }
        }
    }
}

/// Valeurs brutes décodées (octets non normalisés).
#[derive(Debug, Clone, PartialEq)]
pub struct RawReport {
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    pub l2: u8,
    pub r2: u8,
    pub buttons: ButtonSet,
    pub battery: Option<Battery>,
}

pub fn parse(model: ControllerModel, report: &[u8]) -> Result<RawReport, ParseError> {
    if model == ControllerModel::Unknown {
        return Err(ParseError::UnsupportedModel);
    }
    let id = *report.first().ok_or(ParseError::Empty)?;
    let dualsense = model.is_dualsense_family();

    match id {
        // Rapport « simple » (Bluetooth par défaut) : même disposition pour les deux familles.
        0x01 if report.len() < 20 => ds4_layout(report, id),
        // DualShock 4 — USB complet
        0x01 if !dualsense => ds4_layout(report, id),
        // DualShock 4 — Bluetooth complet : 2 octets d'en-tête supplémentaires
        0x11 if !dualsense => {
            if report.len() < 3 {
                return Err(ParseError::TooShort { id, len: report.len(), need: 3 });
            }
            ds4_layout(&report[2..], id)
        }
        // DualSense — USB complet
        0x01 => dualsense_layout(report, id),
        // DualSense — Bluetooth complet : 1 octet d'en-tête supplémentaire
        0x31 if dualsense => {
            if report.len() < 2 {
                return Err(ParseError::TooShort { id, len: report.len(), need: 2 });
            }
            dualsense_layout(&report[1..], id)
        }
        _ => Err(ParseError::UnknownReport { id }),
    }
}

fn need(d: &[u8], id: u8, min: usize) -> Result<(), ParseError> {
    if d.len() < min {
        Err(ParseError::TooShort { id, len: d.len(), need: min })
    } else {
        Ok(())
    }
}

/// `d[0]` = position de l'identifiant de rapport (USB) ; sticks à partir de `d[1]`.
fn ds4_layout(d: &[u8], id: u8) -> Result<RawReport, ParseError> {
    need(d, id, 10)?;
    let mut buttons = ButtonSet::default();
    face_and_hat(d[5], &mut buttons);
    shoulders(d[6], &mut buttons);
    misc(d[7], false, &mut buttons);
    Ok(RawReport {
        lx: d[1],
        ly: d[2],
        rx: d[3],
        ry: d[4],
        l2: d[8],
        r2: d[9],
        buttons,
        battery: if d.len() >= 31 { ds4_battery(d[30]) } else { None },
    })
}

fn dualsense_layout(d: &[u8], id: u8) -> Result<RawReport, ParseError> {
    need(d, id, 11)?;
    let mut buttons = ButtonSet::default();
    face_and_hat(d[8], &mut buttons);
    shoulders(d[9], &mut buttons);
    misc(d[10], true, &mut buttons);
    Ok(RawReport {
        lx: d[1],
        ly: d[2],
        rx: d[3],
        ry: d[4],
        l2: d[5],
        r2: d[6],
        buttons,
        battery: if d.len() >= 54 { dualsense_battery(d[53]) } else { None },
    })
}

fn face_and_hat(b: u8, set: &mut ButtonSet) {
    // Croix directionnelle : 0 = haut, sens horaire, 8 (ou plus) = neutre.
    let (up, right, down, left) = match b & 0x0F {
        0 => (true, false, false, false),
        1 => (true, true, false, false),
        2 => (false, true, false, false),
        3 => (false, true, true, false),
        4 => (false, false, true, false),
        5 => (false, false, true, true),
        6 => (false, false, false, true),
        7 => (true, false, false, true),
        _ => (false, false, false, false),
    };
    set.set(PsButton::DpadUp, up);
    set.set(PsButton::DpadRight, right);
    set.set(PsButton::DpadDown, down);
    set.set(PsButton::DpadLeft, left);
    set.set(PsButton::Square, b & 0x10 != 0);
    set.set(PsButton::Cross, b & 0x20 != 0);
    set.set(PsButton::Circle, b & 0x40 != 0);
    set.set(PsButton::Triangle, b & 0x80 != 0);
}

fn shoulders(b: u8, set: &mut ButtonSet) {
    // 0x04 / 0x08 = L2 / R2 numériques : ignorés, on lit la valeur analogique.
    set.set(PsButton::L1, b & 0x01 != 0);
    set.set(PsButton::R1, b & 0x02 != 0);
    set.set(PsButton::Share, b & 0x10 != 0);
    set.set(PsButton::Options, b & 0x20 != 0);
    set.set(PsButton::L3, b & 0x40 != 0);
    set.set(PsButton::R3, b & 0x80 != 0);
}

fn misc(b: u8, has_mute: bool, set: &mut ButtonSet) {
    set.set(PsButton::Ps, b & 0x01 != 0);
    set.set(PsButton::Touchpad, b & 0x02 != 0);
    if has_mute {
        set.set(PsButton::Mute, b & 0x04 != 0);
    }
}

/// Niveau approximatif : la télémétrie varie selon le transport (voir PB-009).
fn ds4_battery(b: u8) -> Option<Battery> {
    let level = b & 0x0F;
    let cable = b & 0x10 != 0;
    if cable {
        match level {
            0..=10 => Some(Battery {
                percent: Some((level * 10 + 5).min(100)),
                charging: true,
            }),
            11 => Some(Battery { percent: Some(100), charging: false }),
            _ => None,
        }
    } else if level <= 8 {
        Some(Battery {
            percent: Some(((u16::from(level) * 100 + 4) / 8) as u8),
            charging: false,
        })
    } else {
        None
    }
}

fn dualsense_battery(b: u8) -> Option<Battery> {
    let level = b & 0x0F;
    match b >> 4 {
        0 => Some(Battery { percent: Some((level * 10 + 5).min(100)), charging: false }),
        1 => Some(Battery { percent: Some((level * 10 + 5).min(100)), charging: true }),
        2 => Some(Battery { percent: Some(100), charging: false }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ds4_usb(buttons1: u8, buttons2: u8, l2: u8) -> Vec<u8> {
        let mut r = vec![0u8; 64];
        r[0] = 0x01;
        r[1..5].copy_from_slice(&[128, 128, 128, 128]);
        r[5] = buttons1;
        r[6] = buttons2;
        r[8] = l2;
        r
    }

    #[test]
    fn ds4_usb_buttons_and_trigger() {
        let r = parse(ControllerModel::DualShock4, &ds4_usb(0x08 | 0x20, 0x01, 255)).unwrap();
        let pressed: Vec<_> = r.buttons.iter().collect();
        assert_eq!(pressed, vec![PsButton::Cross, PsButton::L1]);
        assert_eq!(r.l2, 255);
        assert_eq!(r.r2, 0);
    }

    #[test]
    fn ds4_dpad_diagonal() {
        let r = parse(ControllerModel::DualShock4, &ds4_usb(0x01, 0, 0)).unwrap();
        let pressed: Vec<_> = r.buttons.iter().collect();
        assert_eq!(pressed, vec![PsButton::DpadUp, PsButton::DpadRight]);
    }

    #[test]
    fn ds4_bluetooth_full_report_has_offset() {
        let mut r = vec![0u8; 78];
        r[0] = 0x11;
        r[3..7].copy_from_slice(&[0, 255, 128, 128]);
        r[7] = 0x08 | 0x80; // triangle
        let parsed = parse(ControllerModel::DualShock4, &r).unwrap();
        assert_eq!((parsed.lx, parsed.ly), (0, 255));
        assert!(parsed.buttons.contains(PsButton::Triangle));
    }

    #[test]
    fn ds4_bluetooth_press_release_and_reconnect_reports() {
        // Pure synthetic reports: never open a Bluetooth device or write HID.
        let mut pressed = vec![0u8; 78];
        pressed[0] = 0x11;
        pressed[3..7].copy_from_slice(&[128, 128, 128, 128]);
        pressed[7] = 0x08 | 0x20; // D-pad neutral + Cross
        let first = parse(ControllerModel::DualShock4, &pressed).unwrap();
        assert!(first.buttons.contains(PsButton::Cross));

        let mut released = pressed.clone();
        released[7] = 0x08; // no pressed face button
        let idle = parse(ControllerModel::DualShock4, &released).unwrap();
        assert!(!idle.buttons.contains(PsButton::Cross));

        // A fresh stream after reconnect must not preserve the previous press.
        let after_reconnect = parse(ControllerModel::DualShock4, &released).unwrap();
        assert!(after_reconnect.buttons.is_empty());
        assert_eq!((after_reconnect.lx, after_reconnect.ly), (128, 128));
    }

    #[test]
    fn dualsense_usb_layout_and_battery() {
        let mut r = vec![0u8; 64];
        r[0] = 0x01;
        r[1..5].copy_from_slice(&[10, 20, 30, 40]);
        r[5] = 200; // L2
        r[6] = 100; // R2
        r[8] = 0x08 | 0x40; // cercle
        r[9] = 0x10 | 0x80; // create + R3
        r[10] = 0x04 | 0x01; // mute + PS
        r[53] = 0x1A; // en charge, niveau 10
        let parsed = parse(ControllerModel::DualSense, &r).unwrap();
        assert_eq!((parsed.lx, parsed.ly, parsed.rx, parsed.ry), (10, 20, 30, 40));
        assert_eq!((parsed.l2, parsed.r2), (200, 100));
        for b in [PsButton::Circle, PsButton::Share, PsButton::R3, PsButton::Mute, PsButton::Ps] {
            assert!(parsed.buttons.contains(b), "{b:?} attendu");
        }
        assert_eq!(parsed.battery, Some(Battery { percent: Some(100), charging: true }));
    }

    #[test]
    fn simple_report_has_no_battery() {
        let mut r = vec![0u8; 10];
        r[0] = 0x01;
        let parsed = parse(ControllerModel::DualSense, &r).unwrap();
        assert_eq!(parsed.battery, None);
    }

    #[test]
    fn errors_are_explicit() {
        assert_eq!(parse(ControllerModel::DualSense, &[]), Err(ParseError::Empty));
        assert_eq!(
            parse(ControllerModel::Unknown, &[1, 2, 3]),
            Err(ParseError::UnsupportedModel)
        );
        assert_eq!(
            parse(ControllerModel::DualShock4, &[0x7F; 12]),
            Err(ParseError::UnknownReport { id: 0x7F })
        );
        assert!(matches!(
            parse(ControllerModel::DualShock4, &[0x01, 1, 2]),
            Err(ParseError::TooShort { .. })
        ));
    }
}
