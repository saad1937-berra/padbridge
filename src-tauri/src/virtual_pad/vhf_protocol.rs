//! Contrat binaire d'un gamepad HID virtuel PadBridge, v0 (POC B).
//!
//! Aucun acces au noyau ni a une vraie manette : serialisation pure et testable.
//! Ce rapport HID *n'est pas* un paquet XUSB/XInput. Le pilote Windows VHF
//! experimental doit le recevoir via IOCTL une fois signe, installe et audite en VM.

use super::backend::{OutputState, XButton};
use crate::devices::model::Bit;

pub const REPORT_ID: u8 = 1;
pub const REPORT_LEN: usize = 10;
pub const HAT_NEUTRAL: u8 = 8;
/// CTL_CODE(FILE_DEVICE_UNKNOWN, 0x800, METHOD_BUFFERED, FILE_WRITE_DATA)
/// dans PadBridgeVhf.h. Constante pure : n'ouvre ni n'ecrit de handle Windows.
pub const IOCTL_SUBMIT: u32 = 0x0022_A000;
pub const WATCHDOG_TIMEOUT_MS: u32 = 800;
pub const WATCHDOG_INTERVAL_MS: u32 = 200;

/// Verification en espace utilisateur pour le futur broker (le noyau valide
/// les memes champs, meme si le broker est compromis).
pub fn validate_report(bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.len() != REPORT_LEN {
        return Err("invalid report length");
    }
    if bytes[0] != REPORT_ID {
        return Err("invalid report id");
    }
    if u16::from_le_bytes([bytes[1], bytes[2]]) & !0x07ff != 0 {
        return Err("reserved button bits");
    }
    if bytes[3] > HAT_NEUTRAL {
        return Err("invalid hat value");
    }
    if bytes[4..8].contains(&0x80) {
        return Err("invalid signed axis -128");
    }
    Ok(())
}

/// Ordre des bits dans PadBridgeVhf.h, distinct du masque XINPUT_GAMEPAD.
/// Les quatre directions sont representees par un hat switch.
const BUTTON_ORDER: [XButton; 11] = [
    XButton::A,
    XButton::B,
    XButton::X,
    XButton::Y,
    XButton::LB,
    XButton::RB,
    XButton::Back,
    XButton::Start,
    XButton::Guide,
    XButton::LS,
    XButton::RS,
];

fn axis(v: f32) -> u8 {
    (v.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8
}

fn trigger(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn hat(state: &OutputState) -> u8 {
    let up = state.buttons.contains(XButton::DpadUp);
    let right = state.buttons.contains(XButton::DpadRight);
    let down = state.buttons.contains(XButton::DpadDown);
    let left = state.buttons.contains(XButton::DpadLeft);

    // Annuler les directions opposees pour ne jamais injecter une
    // direction contradictoire dans le rapport HID.
    let y = (up as i8) - (down as i8);
    let x = (right as i8) - (left as i8);
    match (x, y) {
        (0, 1) => 0,
        (1, 1) => 1,
        (1, 0) => 2,
        (1, -1) => 3,
        (0, -1) => 4,
        (-1, -1) => 5,
        (-1, 0) => 6,
        (-1, 1) => 7,
        _ => HAT_NEUTRAL,
    }
}

/// [ID][boutons_lo][boutons_hi][hat][LX][LY][RX][RY][LT][RT]
pub fn encode(state: &OutputState) -> [u8; REPORT_LEN] {
    let mut buttons = 0u16;
    for (idx, button) in BUTTON_ORDER.iter().enumerate() {
        if state.buttons.contains(*button) {
            buttons |= 1u16 << idx;
        }
    }
    let [low, high] = buttons.to_le_bytes();
    [
        REPORT_ID,
        low,
        high,
        hat(state),
        axis(state.lx),
        axis(state.ly),
        axis(state.rx),
        axis(state.ry),
        trigger(state.lt),
        trigger(state.rt),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_pad::backend::OutputState;

    #[test]
    fn neutral_has_neutral_hat_and_10_bytes() {
        assert_eq!(encode(&OutputState::neutral()), [1, 0, 0, 8, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn button_bit_order_matches_c_wire_contract() {
        let mut s = OutputState::neutral();
        s.buttons.set(XButton::A, true);
        s.buttons.set(XButton::Y, true);
        s.buttons.set(XButton::Guide, true);
        s.buttons.set(XButton::RS, true);
        let data = encode(&s);
        assert_eq!(u16::from_le_bytes([data[1], data[2]]), 0x0509);
    }

    #[test]
    fn hats_cover_all_eight_directions() {
        const DIRS: [(&[XButton], u8); 8] = [
            (&[XButton::DpadUp], 0),
            (&[XButton::DpadUp, XButton::DpadRight], 1),
            (&[XButton::DpadRight], 2),
            (&[XButton::DpadDown, XButton::DpadRight], 3),
            (&[XButton::DpadDown], 4),
            (&[XButton::DpadDown, XButton::DpadLeft], 5),
            (&[XButton::DpadLeft], 6),
            (&[XButton::DpadUp, XButton::DpadLeft], 7),
        ];
        for (buttons, expected) in DIRS {
            let mut s = OutputState::neutral();
            for &button in buttons {
                s.buttons.set(button, true);
            }
            assert_eq!(encode(&s)[3], expected);
        }
    }

    #[test]
    fn opposite_directions_are_neutral() {
        let mut s = OutputState::neutral();
        s.buttons.set(XButton::DpadLeft, true);
        s.buttons.set(XButton::DpadRight, true);
        assert_eq!(encode(&s)[3], HAT_NEUTRAL);
    }

    #[test]
    fn analog_axes_and_triggers_saturate() {
        let mut s = OutputState::neutral();
        s.lx = -2.0;
        s.ly = 1.0;
        s.rx = -0.5;
        s.ry = 2.0;
        s.lt = 2.0;
        s.rt = -1.0;
        let data = encode(&s);
        assert_eq!(data[4] as i8, -127);
        assert_eq!(data[5] as i8, 127);
        assert_eq!(data[6] as i8, -64);
        assert_eq!(data[7] as i8, 127);
        assert_eq!((data[8], data[9]), (255, 0));
    }

    #[test]
    fn ioctl_and_watchdog_match_kernel_contract() {
        assert_eq!(IOCTL_SUBMIT, (0x22u32 << 16) | (2 << 14) | (0x800 << 2));
        assert_eq!((WATCHDOG_INTERVAL_MS, WATCHDOG_TIMEOUT_MS), (200, 800));
    }

    #[test]
    fn encoded_report_is_valid_and_bad_reports_are_rejected() {
        let valid = encode(&OutputState::neutral());
        assert_eq!(validate_report(&valid), Ok(()));
        assert!(validate_report(&valid[..9]).is_err());
        let mut wrong_id = valid;
        wrong_id[0] = 2;
        assert!(validate_report(&wrong_id).is_err());
        let mut reserved_bit = valid;
        reserved_bit[2] = 0x80;
        assert!(validate_report(&reserved_bit).is_err());
        let mut invalid_hat = valid;
        invalid_hat[3] = 9;
        assert!(validate_report(&invalid_hat).is_err());
        let mut invalid_axis = valid;
        invalid_axis[4] = 0x80;
        assert!(validate_report(&invalid_axis).is_err());
    }

    #[test]
    fn no_emission_or_side_effects_in_encoder() {
        let s = OutputState::neutral();
        assert_eq!(encode(&s), encode(&s));
        assert!(s.is_neutral());
        // Le vrai backend reste NullBackend tant qu'un pilote signe
        // et son broker ne sont pas valides sur une VM isolee.
    }
}
