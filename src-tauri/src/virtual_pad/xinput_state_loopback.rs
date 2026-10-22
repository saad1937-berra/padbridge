//! Synthetic DualShock 4 -> real Engine -> XINPUT_GAMEPAD contract tests.
//! NEVER linked into the app, never connects to any XUSB/HID driver.
//! This tests the consumer-facing Xbox state, not a Windows XInput slot.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use crate::devices::model::{ControllerDevice, ControllerModel, PsButton, Transport};
use crate::engine::Engine;
use crate::error::{codes, AppError, AppResult};
use crate::profiles::schema::{OutputMode, DEFAULT_PROFILE_ID};
use super::backend::{
    BackendStatus, OutputState, SessionId, VirtualPadBackend, XButton, XInputReport,
};
use super::xinput_state_contract::{gamepad_fixture_bytes, validated_xinput_state};

#[derive(Default)]
struct Record {
    live: BTreeSet<String>,
    history: BTreeMap<String, Vec<XInputReport>>,
    created: u32,
    fail_active_once: bool,
}

struct XboxMemoryBackend(Arc<Mutex<Record>>);

impl VirtualPadBackend for XboxMemoryBackend {
    fn name(&self) -> &'static str { "test-xinput-state-memory-only" }

    fn status(&self) -> BackendStatus {
        BackendStatus {
            name: self.name().into(), available: true, code: None, reason: None,
        }
    }

    fn create(&mut self, physical_id: &str) -> AppResult<SessionId> {
        let mut state = self.0.lock().unwrap();
        state.created += 1;
        let key = format!("xinput-test-{physical_id}-{}", state.created);
        state.live.insert(key.clone());
        state.history.insert(
            key.clone(),
            vec![validated_xinput_state(&OutputState::neutral()).unwrap()],
        );
        Ok(SessionId(key))
    }

    fn submit(&mut self, session: &SessionId, state: &OutputState) -> AppResult<()> {
        let report = validated_xinput_state(state)
            .map_err(|err| AppError::new(codes::BACKEND_SESSION, format!("invalid Xbox state: {err:?}")))?;
        let mut shared = self.0.lock().unwrap();
        if shared.fail_active_once && !state.is_neutral() {
            shared.fail_active_once = false;
            return Err(AppError::new(codes::BACKEND_SESSION, "simulated Xbox-state write failure"));
        }
        if !shared.live.contains(&session.0) {
            return Err(AppError::new(codes::BACKEND_SESSION, "unknown virtual session"));
        }
        shared.history.get_mut(&session.0).unwrap().push(report);
        Ok(())
    }

    fn destroy(&mut self, session: &SessionId) -> AppResult<()> {
        let mut shared = self.0.lock().unwrap();
        if shared.live.remove(&session.0) {
            shared.history.get_mut(&session.0).unwrap()
                .push(validated_xinput_state(&OutputState::neutral()).unwrap());
        }
        Ok(())
    }
}

fn engine() -> (tempfile::TempDir, Engine, Arc<Mutex<Record>>) {
    let dir = tempfile::tempdir().unwrap();
    let shared = Arc::new(Mutex::new(Record::default()));
    let engine = Engine::new(dir.path(), Box::new(XboxMemoryBackend(shared.clone()))).unwrap();
    (dir, engine, shared)
}

fn ds4(id: &str, transport: Transport) -> ControllerDevice {
    let device = ControllerDevice::new(id.into(), 0x054C, 0x09CC, transport);
    assert_eq!(device.model, ControllerModel::DualShock4);
    device
}

fn usb(cross: bool) -> Vec<u8> {
    let mut report = vec![0u8; 64];
    report[0] = 0x01;
    report[1..5].copy_from_slice(&[128, 128, 128, 128]);
    report[5] = 0x08 | if cross { 0x20 } else { 0 };
    report[6] = 0x01;  // L1
    report[8] = 255;   // L2 analog
    report
}

fn bluetooth(cross: bool) -> Vec<u8> {
    let mut report = vec![0u8; 78];
    report[0] = 0x11;
    report[3..7].copy_from_slice(&[128, 128, 128, 128]);
    report[7] = 0x08 | if cross { 0x20 } else { 0 };
    report
}

fn states(shared: &Arc<Mutex<Record>>, key: &str) -> Vec<XInputReport> {
    shared.lock().unwrap().history.get(key).cloned().unwrap_or_default()
}

#[test]
fn usb_ds4_cross_l1_and_l2_reach_xinput_gamepad_state() {
    let (_dir, engine, shared) = engine();
    engine.device_connected(ds4("usb", Transport::Usb), true);
    let key = engine.activate_mode("usb", OutputMode::Xinput, None, false)
        .unwrap().session_id.unwrap();

    engine.ingest_report("usb", &usb(true)).unwrap();
    let sent = states(&shared, &key);
    assert_eq!(sent.len(), 2);
    assert_eq!(gamepad_fixture_bytes(&sent[0]), [0; 12]);
    assert_eq!(sent[1].buttons & 0x1100, 0x1100, "Cross->A and L1->LB");
    assert_eq!(sent[1].left_trigger, 255);
    assert_eq!(sent[1].right_trigger, 0);
    assert!(sent[1].thumb_lx.abs() < 1024, "center stick should remain near neutral");

    engine.device_lost("usb");
    assert_eq!(gamepad_fixture_bytes(states(&shared, &key).last().unwrap()), [0; 12]);
    assert!(shared.lock().unwrap().live.is_empty());
}

#[test]
fn bt_ds4_disconnect_then_reconnect_does_not_hold_cross() {
    let (_dir, engine, shared) = engine();
    engine.device_connected(ds4("bt", Transport::Bluetooth), true);
    let old = engine.activate_mode("bt", OutputMode::Xinput, None, false)
        .unwrap().session_id.unwrap();
    engine.ingest_report("bt", &bluetooth(true)).unwrap();
    assert!(states(&shared, &old).last().unwrap().buttons & 0x1000 != 0);

    engine.device_lost("bt");
    assert_eq!(gamepad_fixture_bytes(states(&shared, &old).last().unwrap()), [0; 12]);
    engine.device_connected(ds4("bt", Transport::Bluetooth), true);
    let new_key = engine.get_controller("bt").unwrap().virtual_session_id.unwrap();
    assert_ne!(old, new_key);
    assert_eq!(states(&shared, &new_key).len(), 1);
    assert_eq!(gamepad_fixture_bytes(states(&shared, &new_key).last().unwrap()), [0; 12]);

    engine.ingest_report("bt", &bluetooth(false)).unwrap();
    assert_eq!(states(&shared, &new_key).last().unwrap().buttons & 0x1000, 0);
    engine.shutdown();
    assert!(shared.lock().unwrap().live.is_empty());
}

#[test]
fn custom_mapping_cross_to_y_reaches_xinput_state() {
    let (_dir, engine, shared) = engine();
    engine.device_connected(ds4("map", Transport::Usb), true);
    let key = engine.activate_mode("map", OutputMode::Xinput, None, false)
        .unwrap().session_id.unwrap();
    let mut profile = engine.get_profile(DEFAULT_PROFILE_ID).unwrap();
    profile.mapping.insert(PsButton::Cross, XButton::Y);
    engine.save_profile(&profile).unwrap();
    engine.ingest_report("map", &usb(true)).unwrap();
    let buttons = states(&shared, &key).last().unwrap().buttons;
    assert_eq!(buttons & 0x9000, 0x8000, "Y must replace A");
    engine.shutdown();
}

#[test]
fn failing_memory_output_reverts_to_native_with_neutral_close() {
    let (_dir, engine, shared) = engine();
    engine.device_connected(ds4("err", Transport::Usb), true);
    let key = engine.activate_mode("err", OutputMode::Xinput, None, false)
        .unwrap().session_id.unwrap();
    shared.lock().unwrap().fail_active_once = true;
    engine.ingest_report("err", &usb(true)).unwrap();
    let view = engine.get_controller("err").unwrap();
    assert_eq!(view.mode, OutputMode::Native);
    assert!(view.virtual_session_id.is_none());
    assert_eq!(gamepad_fixture_bytes(states(&shared, &key).last().unwrap()), [0; 12]);
    assert!(shared.lock().unwrap().live.is_empty());
}
