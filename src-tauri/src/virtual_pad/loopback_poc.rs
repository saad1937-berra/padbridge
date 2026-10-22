//! Test-only end-to-end contract for a prospective VHF output backend.
//! Synthetic DS4 HID reports -> Engine mapping -> BrokerSession -> VHF bytes.
//! Never compiled into normal Tauri builds; no actual driver, Bluetooth, or HID write.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::sync::{Arc, Mutex};

use crate::devices::model::{ControllerDevice, ControllerModel, PsButton, Transport};
use crate::engine::Engine;
use crate::error::{codes, AppError, AppResult};
use crate::profiles::schema::{OutputMode, DEFAULT_PROFILE_ID};
use super::backend::{BackendStatus, OutputState, SessionId, VirtualPadBackend, XButton};
use super::vhf_broker::{BrokerSession, Report, ReportTransport};
use super::vhf_protocol::encode;

#[derive(Default)]
struct Recording {
    reports: Vec<(String, Report)>,
    live: BTreeSet<String>,
    fail_next_active: bool,
}

struct RecordingTransport {
    key: String,
    shared: Arc<Mutex<Recording>>,
}

impl ReportTransport for RecordingTransport {
    fn send(&mut self, report: &Report) -> io::Result<()> {
        let mut state = self.shared.lock().expect("recording mutex");
        if state.fail_next_active && *report != encode(&OutputState::neutral()) {
            state.fail_next_active = false;
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "simulated output failure"));
        }
        state.reports.push((self.key.clone(), *report));
        Ok(())
    }
}

// This backend exists ONLY for tests; selected_backend.rs remains NullBackend.
struct LoopbackBackend {
    shared: Arc<Mutex<Recording>>,
    sessions: BTreeMap<String, BrokerSession<RecordingTransport>>,
    serial: usize,
}

impl LoopbackBackend {
    fn new(shared: Arc<Mutex<Recording>>) -> Self {
        Self { shared, sessions: BTreeMap::new(), serial: 0 }
    }
}

impl VirtualPadBackend for LoopbackBackend {
    fn name(&self) -> &'static str { "test-vhf-loopback" }

    fn status(&self) -> BackendStatus {
        BackendStatus { available: true, name: self.name().into(), code: None, reason: None }
    }

    fn create(&mut self, physical_id: &str) -> AppResult<SessionId> {
        self.serial += 1;
        let key = format!("loopback-{}-{}", physical_id, self.serial);
        let transport = RecordingTransport { key: key.clone(), shared: Arc::clone(&self.shared) };
        let session = BrokerSession::begin(transport)
            .map_err(|err| AppError::new(codes::BACKEND_SESSION, err.to_string()))?;
        self.shared.lock().unwrap().live.insert(key.clone());
        self.sessions.insert(key.clone(), session);
        Ok(SessionId(key))
    }

    fn submit(&mut self, session: &SessionId, state: &OutputState) -> AppResult<()> {
        self.sessions.get_mut(&session.0)
            .ok_or_else(|| AppError::new(codes::BACKEND_SESSION, "unknown test session"))?
            .submit(state)
            .map_err(|err| AppError::new(codes::BACKEND_SESSION, err.to_string()))
    }

    fn destroy(&mut self, session: &SessionId) -> AppResult<()> {
        if let Some(mut current) = self.sessions.remove(&session.0) {
            let result = current.close();
            self.shared.lock().unwrap().live.remove(&session.0);
            result.map_err(|err| AppError::new(codes::BACKEND_SESSION, err.to_string()))?;
        }
        Ok(())
    }
}

fn harness() -> (tempfile::TempDir, Engine, Arc<Mutex<Recording>>) {
    let dir = tempfile::tempdir().unwrap();
    let shared = Arc::new(Mutex::new(Recording::default()));
    let engine = Engine::new(dir.path(), Box::new(LoopbackBackend::new(shared.clone()))).unwrap();
    (dir, engine, shared)
}

fn ds4(id: &str, transport: Transport) -> ControllerDevice {
    let pad = ControllerDevice::new(id.into(), 0x054C, 0x09CC, transport);
    assert_eq!(pad.model, ControllerModel::DualShock4);
    pad
}

fn usb(cross: bool) -> Vec<u8> {
    let mut report = vec![0u8; 64];
    report[0] = 0x01;
    report[1..5].copy_from_slice(&[255, 0, 128, 128]);
    report[5] = 0x08 | if cross { 0x20 } else { 0 };
    report[6] = 0x01; // L1
    report[8] = 255; // L2
    report
}

fn bluetooth(cross: bool) -> Vec<u8> {
    let mut report = vec![0u8; 78];
    report[0] = 0x11;
    report[3..7].copy_from_slice(&[128, 128, 128, 128]);
    report[7] = 0x08 | if cross { 0x20 } else { 0 };
    report
}

fn frames(shared: &Arc<Mutex<Recording>>, key: &str) -> Vec<Report> {
    shared.lock().unwrap().reports.iter()
        .filter(|(id, _)| id == key).map(|(_, frame)| *frame).collect()
}

#[test]
fn usb_hid_to_virtual_wire_and_disconnect() {
    let (_dir, engine, shared) = harness();
    engine.device_connected(ds4("usb-one", Transport::Usb), true);
    let result = engine.activate_mode("usb-one", OutputMode::Xinput, None, false).unwrap();
    assert!(result.ok);
    let key = result.session_id.unwrap();
    engine.ingest_report("usb-one", &usb(true)).unwrap();
    let sent = frames(&shared, &key);
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0], encode(&OutputState::neutral()));
    assert_eq!(sent[1][0], 1);
    assert_eq!(sent[1][1], 0x11, "Cross -> A and L1 -> LB");
    assert_eq!(sent[1][3], 8, "D-pad neutral");
    // Radial deadzone preserves direction but normalizes diagonal magnitude:
    // full upper-right diagonal has components ~127 / sqrt(2), not 127 each.
    assert_eq!(sent[1][4] as i8, 90);
    assert_eq!(sent[1][5] as i8, 90);
    assert_eq!(sent[1][8], 255);

    engine.device_lost("usb-one");
    assert_eq!(frames(&shared, &key).last().copied(), Some(encode(&OutputState::neutral())));
    assert!(shared.lock().unwrap().live.is_empty());
}

#[test]
fn bluetooth_reconnect_never_replays_old_buttons() {
    let (_dir, engine, shared) = harness();
    engine.device_connected(ds4("bt-one", Transport::Bluetooth), true);
    let first = engine.activate_mode("bt-one", OutputMode::Xinput, None, false).unwrap();
    let old_key = first.session_id.unwrap();
    engine.ingest_report("bt-one", &bluetooth(true)).unwrap();
    assert_eq!(frames(&shared, &old_key).last().unwrap()[1] & 1, 1);
    engine.device_lost("bt-one");
    assert_eq!(frames(&shared, &old_key).last().copied(), Some(encode(&OutputState::neutral())));
    assert!(shared.lock().unwrap().live.is_empty());

    engine.device_connected(ds4("bt-one", Transport::Bluetooth), true);
    let restored = engine.get_controller("bt-one").unwrap();
    assert_eq!(restored.mode, OutputMode::Xinput);
    let fresh_key = restored.virtual_session_id.unwrap();
    assert_ne!(old_key, fresh_key);
    assert_eq!(frames(&shared, &fresh_key), vec![encode(&OutputState::neutral())]);

    engine.ingest_report("bt-one", &bluetooth(false)).unwrap();
    assert_eq!(frames(&shared, &fresh_key).last().copied(), Some(encode(&OutputState::neutral())));
    engine.shutdown();
    assert!(shared.lock().unwrap().live.is_empty());
}

#[test]
fn profile_change_updates_wire_bytes_not_only_preview() {
    let (_dir, engine, shared) = harness();
    engine.device_connected(ds4("usb-map", Transport::Usb), true);
    let key = engine.activate_mode("usb-map", OutputMode::Xinput, None, false)
        .unwrap().session_id.unwrap();
    let mut profile = engine.get_profile(DEFAULT_PROFILE_ID).unwrap();
    profile.mapping.insert(PsButton::Cross, XButton::Y);
    engine.save_profile(&profile).unwrap();
    engine.ingest_report("usb-map", &usb(true)).unwrap();
    let last = *frames(&shared, &key).last().unwrap();
    assert_eq!(last[1] & 0x09, 0x08, "Cross -> Y instead of A");
    engine.shutdown();
    assert!(shared.lock().unwrap().live.is_empty());
}

#[test]
fn broken_transport_neutralizes_then_falls_back_to_native() {
    let (_dir, engine, shared) = harness();
    engine.device_connected(ds4("usb-fail", Transport::Usb), true);
    let key = engine.activate_mode("usb-fail", OutputMode::Xinput, None, false)
        .unwrap().session_id.unwrap();
    shared.lock().unwrap().fail_next_active = true;
    engine.ingest_report("usb-fail", &usb(true)).unwrap();
    let view = engine.get_controller("usb-fail").unwrap();
    assert_eq!(view.mode, OutputMode::Native);
    assert!(view.virtual_session_id.is_none());
    assert_eq!(frames(&shared, &key).last().copied(), Some(encode(&OutputState::neutral())));
    assert!(shared.lock().unwrap().live.is_empty());
    assert!(engine.events().snapshot().iter()
        .any(|e| e.error_code == codes::BACKEND_SESSION));
}
