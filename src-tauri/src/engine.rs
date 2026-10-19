// src-tauri/src/engine.rs
//! Moteur central : registre des manettes, routage, profils, réglages, diagnostics.
//! Indépendant de Tauri et de HID : les lecteurs appellent `device_connected`,
//! `ingest_report` / `ingest_frame` et `device_lost`.

use crate::devices::model::{
    Battery, Bit, ConnectionState, ControllerDevice, ControllerState, InputFrame, PsButton, Stick,
};
use crate::devices::report_parser;
use crate::diagnostics::double_input::{self, DoubleInputAssessment, DoubleInputFacts};
use crate::diagnostics::events::{EventLog, Severity};
use crate::diagnostics::health::SystemReport;
use crate::diagnostics::{export_text, DiagnosticsReport};
use crate::error::{codes, AppError, AppResult};
use crate::pipeline::mapping::CompiledProfile;
use crate::pipeline::normalize;
use crate::pipeline::router::Router;
use crate::profiles::schema::{
    AppSettings, OutputMode, Profile, ValidationReport, DEFAULT_PROFILE_ID,
};
use crate::profiles::store::{now_ms, ProfileStore, SettingsStore};
use crate::virtual_pad::backend::{BackendStatus, OutputState, VirtualPadBackend};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

const DISCONNECTED_TTL_MS: u64 = 600_000;
const FLAP_WINDOW_MS: u64 = 30_000;
const FLAP_THRESHOLD: usize = 3;

struct DeviceEntry {
    device: ControllerDevice,
    state: ControllerState,
    last_frame: Option<InputFrame>,
    seq: u64,
    profile_id: String,
    /// Mode voulu par l'utilisateur, restauré au retour de l'appareil.
    desired_mode: OutputMode,
    connects_ms: VecDeque<u64>,
}

struct Inner {
    devices: BTreeMap<String, DeviceEntry>,
    router: Router,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerView {
    pub device: ControllerDevice,
    pub state: ControllerState,
    pub mode: OutputMode,
    pub virtual_session_id: Option<String>,
    pub profile_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSample {
    pub device_id: String,
    pub seq: u64,
    pub timestamp_us: u64,
    pub pressed: Vec<PsButton>,
    pub left_raw: Stick,
    pub right_raw: Stick,
    pub l2_raw: f32,
    pub r2_raw: f32,
    pub output: OutputState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateResult {
    pub ok: bool,
    pub requested: OutputMode,
    /// Mode réellement actif (natif si la sortie virtuelle a échoué).
    pub mode: OutputMode,
    pub session_id: Option<String>,
    pub error_code: Option<String>,
    pub warnings: Vec<String>,
    pub double_input: DoubleInputAssessment,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveOutcome {
    pub profile: Profile,
    pub report: ValidationReport,
}

pub struct Engine {
    started: Instant,
    inner: Mutex<Inner>,
    events: EventLog,
    profiles: ProfileStore,
    settings: SettingsStore,
    generation: AtomicU64,
}

impl Engine {
    pub fn new(data_dir: &Path, backend: Box<dyn VirtualPadBackend>) -> AppResult<Self> {
        let profiles = ProfileStore::new(data_dir.join("profiles"));
        profiles.init()?;
        let engine = Self {
            started: Instant::now(),
            inner: Mutex::new(Inner { devices: BTreeMap::new(), router: Router::new(backend) }),
            events: EventLog::new(500),
            profiles,
            settings: SettingsStore::new(data_dir.join("settings.json")),
            generation: AtomicU64::new(1),
        };
        engine.events.push(Severity::Info, "PB-I001", "engine", "moteur démarré");
        Ok(engine)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn bump(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Compteur incrémenté à chaque changement structurel (l'interface se rafraîchit).
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn events(&self) -> &EventLog {
        &self.events
    }

    fn now_us(&self) -> u64 {
        self.started.elapsed().as_micros() as u64
    }

    // ----- profils ---------------------------------------------------------

    fn compiled_profile(&self, id: &str) -> AppResult<CompiledProfile> {
        let outcome = self.profiles.load(id)?;
        if outcome.recovered {
            self.events.push(
                Severity::Warning,
                codes::PROFILE_CORRUPT,
                "profiles",
                "profil corrompu : copie conservée, profil par défaut restauré",
            );
        }
        CompiledProfile::compile(&outcome.profile).map_err(|report| {
            let issue = report.errors.first();
            AppError::new(
                codes::PROFILE_INVALID,
                issue.map(|i| format!("{}: {}", i.field, i.code)).unwrap_or_default(),
            )
        })
    }

    /// Profil global actif ; retombe sur le profil par défaut, puis sur le préréglage interne.
    fn global_profile(&self) -> (String, CompiledProfile) {
        let wanted = self.settings.load().active_profile_id;
        for id in [wanted.as_str(), DEFAULT_PROFILE_ID] {
            if let Ok(compiled) = self.compiled_profile(id) {
                return (id.to_string(), compiled);
            }
        }
        let fallback = Profile::default_profile();
        (
            fallback.id.clone(),
            CompiledProfile::compile(&fallback).expect("le profil par défaut est valide"),
        )
    }

    pub fn compile_preview(&self, profile: &Profile) -> Result<CompiledProfile, ValidationReport> {
        CompiledProfile::compile(profile)
    }

    pub fn validate_profile(&self, profile: &Profile) -> ValidationReport {
        profile.validate()
    }

    pub fn list_profiles(&self) -> AppResult<Vec<Profile>> {
        self.profiles.list()
    }

    pub fn get_profile(&self, id: &str) -> AppResult<Profile> {
        Ok(self.profiles.load(id)?.profile)
    }

    /// Enregistre puis applique atomiquement aux appareils qui utilisent ce profil.
    pub fn save_profile(&self, profile: &Profile) -> AppResult<SaveOutcome> {
        let stored = self.profiles.save(profile)?;
        self.apply_profile_to_devices(&stored);
        self.events.push(Severity::Info, "PB-I010", "profiles", "profil enregistré");
        let report = stored.validate();
        Ok(SaveOutcome { profile: stored, report })
    }

    pub fn duplicate_profile(&self, id: &str) -> AppResult<Profile> {
        self.profiles.duplicate(id)
    }

    pub fn rename_profile(&self, id: &str, name: &str) -> AppResult<Profile> {
        let renamed = self.profiles.rename(id, name)?;
        self.bump();
        Ok(renamed)
    }

    pub fn reset_profile(&self, id: &str) -> AppResult<Profile> {
        let fresh = self.profiles.reset(id)?;
        self.apply_profile_to_devices(&fresh);
        Ok(fresh)
    }

    fn apply_profile_to_devices(&self, profile: &Profile) {
        let Ok(compiled) = CompiledProfile::compile(profile) else { return };
        let mut g = self.lock();
        let Inner { devices, router } = &mut *g;
        for (id, entry) in devices.iter() {
            if entry.profile_id == profile.id {
                router.set_profile(id, compiled.clone());
            }
        }
        drop(g);
        self.bump();
    }

    // ----- réglages ---------------------------------------------------------

    pub fn get_settings(&self) -> AppSettings {
        self.settings.load()
    }

    pub fn save_settings(&self, settings: &AppSettings) -> AppResult<AppSettings> {
        self.settings.save(settings)
    }

    // ----- appareils --------------------------------------------------------

    pub fn device_connected(&self, mut device: ControllerDevice, readable: bool) {
        let id = device.device_id.clone();
        let now = now_ms();
        device.last_seen_ms = now;
        let (profile_id, compiled) = self.global_profile();

        let mut g = self.lock();
        let Inner { devices, router } = &mut *g;

        // Purge des appareils déconnectés depuis longtemps.
        devices.retain(|_, e| {
            e.state.connection_state != ConnectionState::Disconnected
                || now.saturating_sub(e.device.last_seen_ms) < DISCONNECTED_TTL_MS
        });

        let entry = devices.entry(id.clone()).or_insert_with(|| DeviceEntry {
            state: ControllerState {
                device_id: id.clone(),
                connection_state: ConnectionState::Disconnected,
                battery: None,
                last_input_at_ms: None,
            },
            device: device.clone(),
            last_frame: None,
            seq: 0,
            profile_id: profile_id.clone(),
            desired_mode: OutputMode::Native,
            connects_ms: VecDeque::new(),
        });

        let already_ok = matches!(
            entry.state.connection_state,
            ConnectionState::Connected | ConnectionState::Unstable
        ) && readable;
        if already_ok {
            entry.device.last_seen_ms = now;
            return;
        }

        entry.device = device;
        entry.last_frame = None;
        entry.profile_id = profile_id;
        entry.connects_ms.push_back(now);
        while entry
            .connects_ms
            .front()
            .map_or(false, |t| now.saturating_sub(*t) > FLAP_WINDOW_MS)
        {
            entry.connects_ms.pop_front();
        }
        entry.state.connection_state = if !readable {
            ConnectionState::Unreadable
        } else if entry.connects_ms.len() >= FLAP_THRESHOLD {
            ConnectionState::Unstable
        } else {
            ConnectionState::Connected
        };

        router.add_device(&id, compiled);

        // Restauration du mode voulu au retour de l'appareil (PB-010 / parcours en échec).
        let desired = entry.desired_mode;
        let mut restore_failed = false;
        if readable && desired == OutputMode::Xinput && router.activate(&id, desired).is_err() {
            entry.desired_mode = OutputMode::Native;
            restore_failed = true;
        }
        drop(g);

        if readable {
            self.events.push(Severity::Info, "PB-I002", "devices", "manette détectée");
        } else {
            self.events.push(
                Severity::Warning,
                codes::DEVICE_UNREADABLE,
                "devices",
                "manette détectée mais illisible",
            );
        }
        if restore_failed {
            self.events.push(
                Severity::Warning,
                codes::BACKEND_UNAVAILABLE,
                "virtual_pad",
                "sortie virtuelle non restaurée : retour au mode natif",
            );
        }
        self.bump();
    }

    /// Idempotent : neutralise la sortie virtuelle et marque l'appareil déconnecté.
    pub fn device_lost(&self, device_id: &str) {
        let mut g = self.lock();
        let Inner { devices, router } = &mut *g;
        let Some(entry) = devices.get_mut(device_id) else { return };
        if entry.state.connection_state == ConnectionState::Disconnected {
            return;
        }
        router.remove_device(device_id);
        entry.state.connection_state = ConnectionState::Disconnected;
        entry.last_frame = None;
        entry.device.last_seen_ms = now_ms();
        drop(g);
        self.events.push(Severity::Info, "PB-I003", "devices", "manette déconnectée");
        self.bump();
    }

    /// Décode un rapport HID brut puis le traite.
    pub fn ingest_report(&self, device_id: &str, report: &[u8]) -> AppResult<()> {
        let mut g = self.lock();
        let Inner { devices, router } = &mut *g;
        let entry = devices
            .get_mut(device_id)
            .ok_or_else(|| AppError::new(codes::DEVICE_NOT_FOUND, "appareil inconnu"))?;
        if entry.state.connection_state == ConnectionState::Disconnected {
            return Ok(());
        }
        let raw = report_parser::parse(entry.device.model, report)
            .map_err(|e| AppError::new(codes::REPORT_INVALID, e.to_string()))?;
        entry.seq += 1;
        let frame = normalize::frame_from_raw(&raw, entry.seq, self.now_us());
        let bump = Self::apply_frame(entry, router, device_id, frame, &self.events);
        drop(g);
        if bump {
            self.bump();
        }
        Ok(())
    }

    /// Point d'entrée pour une trame déjà normalisée (tests, autres sources).
    pub fn ingest_frame(&self, device_id: &str, frame: InputFrame) {
        let mut g = self.lock();
        let Inner { devices, router } = &mut *g;
        let Some(entry) = devices.get_mut(device_id) else { return };
        if entry.state.connection_state == ConnectionState::Disconnected {
            return;
        }
        let bump = Self::apply_frame(entry, router, device_id, frame, &self.events);
        drop(g);
        if bump {
            self.bump();
        }
    }

    /// Retourne vrai si un changement visible par l'interface a eu lieu.
    fn apply_frame(
        entry: &mut DeviceEntry,
        router: &mut Router,
        id: &str,
        frame: InputFrame,
        events: &EventLog,
    ) -> bool {
        let mut changed = false;
        entry.last_frame = Some(frame);
        entry.state.last_input_at_ms = Some(now_ms());
        if let Some(battery) = frame.battery {
            if entry.state.battery != Some(battery) {
                entry.state.battery = Some(battery);
                changed = true;
            }
        }
        if let Some(result) = router.process(id, &frame) {
            if let Some(err) = result.backend_error {
                events.push(Severity::Error, err.code, "virtual_pad", &err.detail);
                entry.desired_mode = OutputMode::Native;
                changed = true;
            }
        }
        changed
    }

    fn view_of(inner: &Inner, id: &str, entry: &DeviceEntry) -> ControllerView {
        ControllerView {
            device: entry.device.clone(),
            state: entry.state.clone(),
            mode: inner.router.mode_of(id).unwrap_or(OutputMode::Native),
            virtual_session_id: inner.router.session_of(id).map(|s| s.0.clone()),
            profile_id: entry.profile_id.clone(),
        }
    }

    pub fn list_controllers(&self) -> Vec<ControllerView> {
        let g = self.lock();
        g.devices.iter().map(|(id, e)| Self::view_of(&g, id, e)).collect()
    }

    pub fn get_controller(&self, device_id: &str) -> AppResult<ControllerView> {
        let g = self.lock();
        g.devices
            .get(device_id)
            .map(|e| Self::view_of(&g, device_id, e))
            .ok_or_else(|| AppError::new(codes::DEVICE_NOT_FOUND, "appareil inconnu"))
    }

    /// Dernier échantillon, transformé par le profil actif ou par un profil d'aperçu.
    pub fn sample(&self, device_id: &str, preview: Option<&CompiledProfile>) -> Option<InputSample> {
        let g = self.lock();
        let entry = g.devices.get(device_id)?;
        let frame = entry.last_frame?;
        let profile = preview.or_else(|| g.router.profile_of(device_id))?;
        Some(InputSample {
            device_id: device_id.to_string(),
            seq: frame.seq,
            timestamp_us: frame.timestamp_us,
            pressed: frame.buttons.iter().collect(),
            left_raw: frame.left,
            right_raw: frame.right,
            l2_raw: frame.l2,
            r2_raw: frame.r2,
            output: profile.apply(&frame),
        })
    }

    // ----- modes ------------------------------------------------------------

    pub fn activate_mode(
        &self,
        device_id: &str,
        mode: OutputMode,
        profile_id: Option<&str>,
        steam_running: bool,
    ) -> AppResult<ActivateResult> {
        let compiled = match profile_id {
            Some(id) => Some((id.to_string(), self.compiled_profile(id)?)),
            None => None,
        };

        let mut g = self.lock();
        let Inner { devices, router } = &mut *g;
        let entry = devices
            .get_mut(device_id)
            .filter(|e| e.state.connection_state != ConnectionState::Disconnected)
            .ok_or_else(|| AppError::new(codes::DEVICE_NOT_FOUND, "appareil absent"))?;

        let mut warnings = Vec::new();
        if let Some((id, c)) = compiled {
            warnings.extend(c.warnings().iter().map(|w| w.code.clone()));
            router.set_profile(device_id, c);
            entry.profile_id = id;
        }

        let outcome = router.activate(device_id, mode);
        let (ok, effective, error_code) = match &outcome {
            Ok(m) => (true, *m, None),
            Err(e) => (
                false,
                router.mode_of(device_id).unwrap_or(OutputMode::Native),
                Some(e.code.to_string()),
            ),
        };
        entry.desired_mode = effective;
        let session_id = router.session_of(device_id).map(|s| s.0.clone());
        let double_input = double_input::assess(&DoubleInputFacts {
            virtual_sessions: router.session_count(),
            steam_running,
            physical_hidden: false,
        });
        drop(g);

        match &outcome {
            Ok(_) => self.events.push(Severity::Info, "PB-I020", "modes", &format!("mode actif : {effective:?}")),
            Err(e) => self.events.push(Severity::Warning, e.code, "modes", &e.detail),
        }
        self.bump();

        Ok(ActivateResult {
            ok,
            requested: mode,
            mode: effective,
            session_id,
            error_code,
            warnings,
            double_input,
        })
    }

    pub fn stop_virtual_session(&self, session_id: &str) -> AppResult<()> {
        let mut g = self.lock();
        let Inner { devices, router } = &mut *g;
        let device_id = router
            .stop_session(session_id)
            .ok_or_else(|| AppError::new(codes::DEVICE_NOT_FOUND, "session inconnue"))?;
        if let Some(entry) = devices.get_mut(&device_id) {
            entry.desired_mode = OutputMode::Native;
        }
        drop(g);
        self.events.push(Severity::Info, "PB-I021", "modes", "sortie virtuelle arrêtée");
        self.bump();
        Ok(())
    }

    // ----- diagnostics ------------------------------------------------------

    pub fn backend_status(&self) -> BackendStatus {
        self.lock().router.backend_status()
    }

    pub fn diagnostics(&self, system: SystemReport) -> DiagnosticsReport {
        let g = self.lock();
        let controllers: Vec<_> = g.devices.iter().map(|(id, e)| Self::view_of(&g, id, e)).collect();
        let backend = g.router.backend_status();
        let sessions = g.router.session_count();
        drop(g);
        let double_input = double_input::assess(&DoubleInputFacts {
            virtual_sessions: sessions,
            steam_running: system.steam_running,
            physical_hidden: false,
        });
        DiagnosticsReport {
            system,
            backend,
            controllers,
            double_input,
            events: self.events.snapshot(),
        }
    }

    pub fn export_diagnostics(&self, path: &Path, system: SystemReport) -> AppResult<()> {
        let text = export_text(&self.diagnostics(system));
        std::fs::write(path, text)
            .map_err(|e| AppError::new(codes::EXPORT_FAILED, e.to_string()))?;
        self.events.push(Severity::Info, "PB-I030", "diagnostics", "diagnostic exporté");
        Ok(())
    }

    /// Arrêt propre : neutralise et détruit toutes les sorties virtuelles.
    pub fn shutdown(&self) {
        self.lock().router.shutdown();
        self.events.push(Severity::Info, "PB-I004", "engine", "moteur arrêté");
    }
}

// Évite un avertissement d'import inutilisé selon les features.
#[allow(dead_code)]
fn _assert_types(_: Battery) {
    let _ = PsButton::ALL;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::model::{ButtonSet, Transport};
    use crate::diagnostics::health::SystemReport;
    use crate::virtual_pad::mock::MockBackend;
    use crate::virtual_pad::null_backend::NullBackend;

    fn system() -> SystemReport {
        SystemReport {
            os: "windows".into(),
            arch: "x86_64".into(),
            os_version: None,
            windows_build: None,
            is_windows_11: None,
            bluetooth_present: None,
            elevated: None,
            steam_running: false,
        }
    }

    fn pad(id: &str) -> ControllerDevice {
        ControllerDevice::new(id.into(), 0x054C, 0x0CE6, Transport::Usb)
    }

    fn engine_with_mock() -> (tempfile::TempDir, Engine, MockBackend) {
        let dir = tempfile::tempdir().unwrap();
        let mock = MockBackend::default();
        let engine = Engine::new(dir.path(), Box::new(mock.clone())).unwrap();
        (dir, engine, mock)
    }

    fn held_l2_and_cross() -> InputFrame {
        let mut buttons = ButtonSet::default();
        buttons.set(PsButton::Cross, true);
        InputFrame { buttons, l2: 1.0, left: Stick { x: 1.0, y: 0.0 }, ..InputFrame::default() }
    }

    #[test]
    fn t16_loss_while_l2_held_leaves_nothing_stuck() {
        let (_d, engine, mock) = engine_with_mock();
        engine.device_connected(pad("aaaa"), true);
        let r = engine.activate_mode("aaaa", OutputMode::Xinput, None, false).unwrap();
        assert!(r.ok);
        let session = r.session_id.unwrap();

        engine.ingest_frame("aaaa", held_l2_and_cross());
        assert!(!mock.shared.lock().unwrap().last[&session].is_neutral());

        engine.device_lost("aaaa");
        let shared = mock.shared.lock().unwrap();
        assert!(shared.last[&session].is_neutral(), "dernier état doit être neutre");
        assert!(shared.live.is_empty(), "la sortie virtuelle doit être détruite");
    }

    #[test]
    fn t17_fifty_reconnections_leave_no_ghost() {
        let (_d, engine, mock) = engine_with_mock();
        for _ in 0..50 {
            engine.device_connected(pad("bbbb"), true);
            engine.activate_mode("bbbb", OutputMode::Xinput, None, false).unwrap();
            engine.ingest_frame("bbbb", held_l2_and_cross());
            engine.device_lost("bbbb");
        }
        let shared = mock.shared.lock().unwrap();
        assert!(shared.live.is_empty());
        assert_eq!(shared.created, shared.destroyed);
    }

    #[test]
    fn one_virtual_pad_per_physical_pad() {
        let (_d, engine, mock) = engine_with_mock();
        engine.device_connected(pad("cccc"), true);
        engine.activate_mode("cccc", OutputMode::Xinput, None, false).unwrap();
        engine.activate_mode("cccc", OutputMode::Xinput, None, false).unwrap();
        assert_eq!(mock.shared.lock().unwrap().created, 1);
    }

    #[test]
    fn reconnect_restores_the_wanted_virtual_mode() {
        let (_d, engine, mock) = engine_with_mock();
        engine.device_connected(pad("dddd"), true);
        engine.activate_mode("dddd", OutputMode::Xinput, None, false).unwrap();
        engine.device_lost("dddd");
        assert!(mock.shared.lock().unwrap().live.is_empty());
        engine.device_connected(pad("dddd"), true);
        assert_eq!(mock.shared.lock().unwrap().live.len(), 1);
        assert_eq!(engine.get_controller("dddd").unwrap().mode, OutputMode::Xinput);
    }

    #[test]
    fn c2_missing_backend_keeps_native_mode_usable() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path(), Box::new(NullBackend)).unwrap();
        engine.device_connected(pad("eeee"), true);
        let r = engine.activate_mode("eeee", OutputMode::Xinput, None, false).unwrap();
        assert!(!r.ok);
        assert_eq!(r.mode, OutputMode::Native);
        assert_eq!(r.error_code.as_deref(), Some(codes::BACKEND_UNAVAILABLE));

        engine.ingest_frame("eeee", held_l2_and_cross());
        let sample = engine.sample("eeee", None).unwrap();
        assert_eq!(sample.pressed, vec![PsButton::Cross]);
        assert!(!engine.backend_status().available);
    }

    #[test]
    fn backend_failure_falls_back_to_native_and_logs() {
        let (_d, engine, mock) = engine_with_mock();
        engine.device_connected(pad("ffff"), true);
        engine.activate_mode("ffff", OutputMode::Xinput, None, false).unwrap();
        mock.shared.lock().unwrap().fail_submit = true;
        engine.ingest_frame("ffff", held_l2_and_cross());
        assert_eq!(engine.get_controller("ffff").unwrap().mode, OutputMode::Native);
        assert!(mock.shared.lock().unwrap().live.is_empty());
        assert!(engine.events().snapshot().iter().any(|e| e.error_code == codes::BACKEND_SESSION));
    }

    #[test]
    fn saving_a_profile_updates_live_devices_atomically() {
        let (_d, engine, _m) = engine_with_mock();
        engine.device_connected(pad("1111"), true);
        let frame = InputFrame { left: Stick { x: 0.1, y: 0.0 }, ..InputFrame::default() };
        engine.ingest_frame("1111", frame);
        assert!(engine.sample("1111", None).unwrap().output.lx > 0.0);

        let mut profile = engine.get_profile(DEFAULT_PROFILE_ID).unwrap();
        profile.deadzones.left_stick = 0.2;
        engine.save_profile(&profile).unwrap();
        assert_eq!(engine.sample("1111", None).unwrap().output.lx, 0.0);
        // l'entrée brute n'est pas modifiée
        assert_eq!(engine.sample("1111", None).unwrap().left_raw.x, 0.1);
    }

    #[test]
    fn preview_profile_does_not_touch_the_active_one() {
        let (_d, engine, _m) = engine_with_mock();
        engine.device_connected(pad("2222"), true);
        engine.ingest_frame(
            "2222",
            InputFrame { left: Stick { x: 0.1, y: 0.0 }, ..InputFrame::default() },
        );
        let mut draft = Profile::default_profile();
        draft.deadzones.left_stick = 0.3;
        let preview = engine.compile_preview(&draft).unwrap();
        assert_eq!(engine.sample("2222", Some(&preview)).unwrap().output.lx, 0.0);
        assert!(engine.sample("2222", None).unwrap().output.lx > 0.0);
    }

    #[test]
    fn unreadable_device_is_listed_with_its_state() {
        let (_d, engine, _m) = engine_with_mock();
        engine.device_connected(pad("3333"), false);
        let view = engine.get_controller("3333").unwrap();
        assert_eq!(view.state.connection_state, ConnectionState::Unreadable);
        assert!(view.state.battery.is_none());
    }

    #[test]
    fn flapping_device_is_reported_unstable() {
        let (_d, engine, _m) = engine_with_mock();
        for _ in 0..3 {
            engine.device_connected(pad("4444"), true);
            engine.device_lost("4444");
        }
        engine.device_connected(pad("4444"), true);
        assert_eq!(
            engine.get_controller("4444").unwrap().state.connection_state,
            ConnectionState::Unstable
        );
    }

    #[test]
    fn frames_after_loss_are_ignored() {
        let (_d, engine, mock) = engine_with_mock();
        engine.device_connected(pad("5555"), true);
        let r = engine.activate_mode("5555", OutputMode::Xinput, None, false).unwrap();
        let session = r.session_id.unwrap();
        engine.device_lost("5555");
        engine.ingest_frame("5555", held_l2_and_cross());
        assert!(mock.shared.lock().unwrap().last[&session].is_neutral());
    }

    #[test]
    fn raw_report_flows_to_the_ui_sample() {
        let (_d, engine, _m) = engine_with_mock();
        let device = ControllerDevice::new("6666".into(), 0x054C, 0x09CC, Transport::Usb);
        engine.device_connected(device, true);
        let mut report = vec![0u8; 64];
        report[0] = 0x01;
        report[1..5].copy_from_slice(&[128, 128, 128, 128]);
        report[5] = 0x08 | 0x20;
        engine.ingest_report("6666", &report).unwrap();
        let sample = engine.sample("6666", None).unwrap();
        assert_eq!(sample.pressed, vec![PsButton::Cross]);
        assert!(engine.ingest_report("6666", &[0x7F; 8]).is_err());
    }

    #[test]
    fn export_has_no_ids_or_vid_pid() {
        let (dir, engine, _m) = engine_with_mock();
        engine.device_connected(pad("deadbeefdeadbeef"), true);
        let path = dir.path().join("diag.txt");
        engine.export_diagnostics(&path, system()).unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("DualSense"));
        assert!(!text.contains("deadbeefdeadbeef"));
        assert!(!text.to_lowercase().contains("054c"));
    }
}
