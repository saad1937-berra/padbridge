//! Pure XINPUT_GAMEPAD *state* contract for a future validated Xbox backend.
//!
//! This 12-byte structure is the Win32 API's gamepad payload. It is NOT a
//! packet accepted by an XUSB bus, a HID report or a driver IOCTL. Never send
//! these bytes to the current generic VHF driver.
//! No external DLLs, kernel code, driver installation or device I/O here.

use super::backend::{OutputState, XInputReport};

/// Source data must be finite and within the normalized contract before it
/// crosses a future privileged backend boundary. Do not silently translate
/// non-finite or out-of-range values into controller input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidState {
    NonFinite,
    OutOfRange,
}

pub fn validated_xinput_state(state: &OutputState) -> Result<XInputReport, InvalidState> {
    let sticks = [state.lx, state.ly, state.rx, state.ry];
    let triggers = [state.lt, state.rt];
    if sticks.iter().chain(triggers.iter()).any(|n| !n.is_finite()) {
        return Err(InvalidState::NonFinite);
    }
    if sticks.iter().any(|n| !(-1.0..=1.0).contains(n))
        || triggers.iter().any(|n| !(0.0..=1.0).contains(n)) {
        return Err(InvalidState::OutOfRange);
    }
    Ok(state.to_xinput())
}

/// Stable *test fixture* layout equivalent to XINPUT_GAMEPAD on Windows.
/// Use explicit LE conversion rather than serializing a padded memory image.
/// These bytes MUST NOT be treated as XUSB or VHF protocol frames.
pub fn gamepad_fixture_bytes(state: &XInputReport) -> [u8; 12] {
    let mut bytes = [0u8; 12];
    bytes[0..2].copy_from_slice(&state.buttons.to_le_bytes());
    bytes[2] = state.left_trigger;
    bytes[3] = state.right_trigger;
    bytes[4..6].copy_from_slice(&state.thumb_lx.to_le_bytes());
    bytes[6..8].copy_from_slice(&state.thumb_ly.to_le_bytes());
    bytes[8..10].copy_from_slice(&state.thumb_rx.to_le_bytes());
    bytes[10..12].copy_from_slice(&state.thumb_ry.to_le_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::backend::XButton;
    use std::mem::{align_of, size_of};

    #[test]
    fn win32_gamepad_abi_size_and_alignment() {
        assert_eq!(size_of::<XInputReport>(), 12);
        assert_eq!(align_of::<XInputReport>(), 2);
    }

    #[test]
    fn all_standard_face_shoulder_and_dpad_bits_are_mapped() {
        let mut source = OutputState::neutral();
        for button in [
            XButton::A, XButton::B, XButton::X, XButton::Y,
            XButton::LB, XButton::RB, XButton::Back, XButton::Start,
            XButton::LS, XButton::RS, XButton::DpadUp, XButton::DpadDown,
            XButton::DpadLeft, XButton::DpadRight,
        ] {
            source.buttons.set(button, true);
        }
        let result = validated_xinput_state(&source).unwrap();
        assert_eq!(result.buttons, 0xF3FF);
        assert_eq!(gamepad_fixture_bytes(&result)[..2], [0xFF, 0xF3]);
    }

    #[test]
    fn gamepad_fixture_respects_signed_axes_triggers_and_le_order() {
        let mut state = OutputState::neutral();
        state.buttons.set(XButton::A, true);
        state.buttons.set(XButton::DpadUp, true);
        state.lt = 1.0;
        state.rt = 0.5;
        state.lx = 1.0;
        state.ly = -1.0;
        let report = validated_xinput_state(&state).unwrap();
        assert_eq!(gamepad_fixture_bytes(&report), [
            0x01, 0x10, 255, 128,
            0xFF, 0x7F, 0x01, 0x80,
            0, 0, 0, 0,
        ]);
    }

    #[test]
    fn nonfinite_or_overshoot_never_crosses_backend_boundary() {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut source = OutputState::neutral();
            source.lx = invalid;
            assert_eq!(validated_xinput_state(&source), Err(InvalidState::NonFinite));
        }
        let mut source = OutputState::neutral();
        source.lt = 1.01;
        assert_eq!(validated_xinput_state(&source), Err(InvalidState::OutOfRange));
        source.lt = 0.5;
        source.rx = -1.01;
        assert_eq!(validated_xinput_state(&source), Err(InvalidState::OutOfRange));
    }

    #[test]
    fn neutral_state_is_exactly_zero_and_stable() {
        let neutral = validated_xinput_state(&OutputState::neutral()).unwrap();
        assert_eq!(gamepad_fixture_bytes(&neutral), [0u8; 12]);
    }
}
