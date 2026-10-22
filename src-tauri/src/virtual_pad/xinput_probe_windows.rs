//! Read-only XInput 1.4 API probe for the future isolated Hyper-V validation.
//!
//! Reads four XInput slots exactly once. No injection, rumble/XInputSetState,
//! driver access/installation, Bluetooth or physical HID access.
//! This module is never linked into PadBridge's desktop backend.

use serde::Serialize;

const ERROR_SUCCESS: u32 = 0;
const ERROR_DEVICE_NOT_CONNECTED: u32 = 1167;
pub const MAX_SLOTS: usize = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct XInputGamepad {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    thumb_rx: i16,
    thumb_ry: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct XInputState {
    packet_number: u32,
    gamepad: XInputGamepad,
}

#[link(name = "xinput1_4")]
extern "system" {
    fn XInputGetState(user_index: u32, state: *mut XInputState) -> u32;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadSnapshot {
    pub packet_number: u32,
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub thumb_lx: i16,
    pub thumb_ly: i16,
    pub thumb_rx: i16,
    pub thumb_ry: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SlotStatus {
    Connected { gamepad: GamepadSnapshot },
    Disconnected,
    Error { win32_code: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotSnapshot {
    pub slot: u32,
    #[serde(flatten)]
    pub state: SlotStatus,
}

/// An XInput slot is NOT a device identity. A connected slot can belong to
/// another gamepad or software; no PadBridge attribution is performed.
fn classify(code: u32, state: XInputState) -> SlotStatus {
    match code {
        ERROR_SUCCESS => SlotStatus::Connected {
            gamepad: GamepadSnapshot {
                packet_number: state.packet_number,
                buttons: state.gamepad.buttons,
                left_trigger: state.gamepad.left_trigger,
                right_trigger: state.gamepad.right_trigger,
                thumb_lx: state.gamepad.thumb_lx,
                thumb_ly: state.gamepad.thumb_ly,
                thumb_rx: state.gamepad.thumb_rx,
                thumb_ry: state.gamepad.thumb_ry,
            }
        },
        ERROR_DEVICE_NOT_CONNECTED => SlotStatus::Disconnected,
        code => SlotStatus::Error { win32_code: code },
    }
}

/// Snapshot of exactly four slots (indices 0..3), with no retry or polling.
pub fn snapshot() -> [SlotSnapshot; MAX_SLOTS] {
    std::array::from_fn(|i| {
        let mut state = XInputState::default();
        // SAFETY: XInput 1.4 writes to an initialized, correctly laid out
        // XINPUT_STATE buffer. User index is always within the documented 0..3.
        let code = unsafe { XInputGetState(i as u32, &mut state) };
        SlotSnapshot { slot: i as u32, state: classify(code, state) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn xinput_abi_sizes_match_windows_sdk() {
        assert_eq!(size_of::<XInputGamepad>(), 12);
        assert_eq!(align_of::<XInputGamepad>(), 2);
        assert_eq!(size_of::<XInputState>(), 16);
        assert_eq!(align_of::<XInputState>(), 4);
    }

    #[test]
    fn disconnected_state_never_exposes_uninitialized_gamepad_data() {
        let sample = XInputState {
            packet_number: 42,
            gamepad: XInputGamepad { buttons: 0xffff, ..XInputGamepad::default() },
        };
        assert_eq!(classify(ERROR_DEVICE_NOT_CONNECTED, sample), SlotStatus::Disconnected);
        assert_eq!(
            serde_json::to_string(&classify(ERROR_DEVICE_NOT_CONNECTED, sample)).unwrap(),
            r#"{"status":"disconnected"}"#
        );
    }

    #[test]
    fn unexpected_win32_failure_is_not_misread_as_connected() {
        assert_eq!(
            classify(5, XInputState::default()),
            SlotStatus::Error { win32_code: 5 }
        );
    }

    #[test]
    fn connected_state_preserves_raw_buttons_triggers_and_axes() {
        let sample = XInputState {
            packet_number: 3,
            gamepad: XInputGamepad {
                buttons: 0x1001,
                left_trigger: 255,
                right_trigger: 128,
                thumb_lx: -32768,
                thumb_ly: 32767,
                thumb_rx: -1,
                thumb_ry: 1,
            }
        };
        let status = classify(ERROR_SUCCESS, sample);
        let SlotStatus::Connected { gamepad } = status else { panic!("should be connected"); };
        assert_eq!(gamepad.packet_number, 3);
        assert_eq!(gamepad.buttons, 0x1001);
        assert_eq!((gamepad.left_trigger, gamepad.right_trigger), (255, 128));
        assert_eq!((gamepad.thumb_lx, gamepad.thumb_ly), (-32768, 32767));
        assert_eq!((gamepad.thumb_rx, gamepad.thumb_ry), (-1, 1));
    }

    #[test]
    fn all_four_slots_are_the_only_indices_probed() {
        assert_eq!(MAX_SLOTS, 4);
        let slots = std::array::from_fn::<_, MAX_SLOTS, _>(|i| SlotSnapshot {
            slot: i as u32,
            state: SlotStatus::Disconnected,
        });
        assert_eq!(slots.iter().map(|s| s.slot).collect::<Vec<_>>(), [0, 1, 2, 3]);
    }
}
