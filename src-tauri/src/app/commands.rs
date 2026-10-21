// src-tauri/src/app/commands.rs
//! Contrats IPC (section 7.3). Chaque commande valide ses entrées ; aucune ne lance de
//! commande arbitraire. Les vérifications système (lentes) tournent hors du fil principal.

use super::permissions;
use super::ui_events::{SharedTestSub, TestSub};
use crate::diagnostics::{drift::{self, DriftReport, DriftSample}, export_text, health, DiagnosticsReport};
use crate::engine::{ActivateResult, ControllerView, Engine, SaveOutcome};
use crate::devices::model::{ConnectionState, ControllerModel, Transport};
use crate::devices::rumble::{ensure_output_allowed, RumbleRegistry};
use crate::error::{codes, AppError, AppResult};
use crate::profiles::schema::{AppSettings, OutputMode, Profile, ValidationReport};
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub engine: Arc<Engine>,
    pub test_sub: SharedTestSub,
    pub rumble: Arc<RumbleRegistry>,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(codes::INTERNAL, e.to_string()))
}

#[tauri::command]
pub fn list_controllers(state: State<'_, AppState>) -> Vec<ControllerView> {
    state.engine.list_controllers()
}

#[tauri::command]
pub fn get_controller_state(state: State<'_, AppState>, device_id: String) -> AppResult<ControllerView> {
    state.engine.get_controller(permissions::device_id(&device_id)?)
}

/// Abonne l'interface aux échantillons d'un appareil. `profile` : brouillon d'aperçu facultatif.
#[tauri::command]
pub fn start_input_test(
    state: State<'_, AppState>,
    device_id: String,
    profile: Option<Profile>,
) -> AppResult<()> {
    let id = permissions::device_id(&device_id)?.to_string();
    state.engine.get_controller(&id)?;
    let preview = match profile {
        Some(p) => Some(state.engine.compile_preview(&p).map_err(|report| {
            let issue = report.errors.first();
            AppError::new(
                codes::PROFILE_INVALID,
                issue.map(|i| format!("{}: {}", i.field, i.code)).unwrap_or_default(),
            )
        })?),
        None => None,
    };
    *state.test_sub.lock().map_err(|_| AppError::new(codes::INTERNAL, "verrou"))? =
        Some(TestSub { device_id: id, preview });
    Ok(())
}

#[tauri::command]
pub fn stop_input_test(state: State<'_, AppState>) {
    if let Ok(mut sub) = state.test_sub.lock() {
        *sub = None;
    }
}

#[tauri::command]
pub fn validate_profile(state: State<'_, AppState>, profile: Profile) -> ValidationReport {
    state.engine.validate_profile(&profile)
}

#[tauri::command]
pub fn save_profile(state: State<'_, AppState>, profile: Profile) -> AppResult<SaveOutcome> {
    state.engine.save_profile(&profile)
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> AppResult<Vec<Profile>> {
    state.engine.list_profiles()
}

#[tauri::command]
pub fn get_profile(state: State<'_, AppState>, id: String) -> AppResult<Profile> {
    state.engine.get_profile(permissions::profile_id(&id)?)
}

#[tauri::command]
pub fn duplicate_profile(state: State<'_, AppState>, id: String) -> AppResult<Profile> {
    state.engine.duplicate_profile(permissions::profile_id(&id)?)
}

#[tauri::command]
pub fn rename_profile(state: State<'_, AppState>, id: String, name: String) -> AppResult<Profile> {
    state.engine.rename_profile(permissions::profile_id(&id)?, &name)
}

#[tauri::command]
pub fn reset_profile(state: State<'_, AppState>, id: String) -> AppResult<Profile> {
    state.engine.reset_profile(permissions::profile_id(&id)?)
}

#[tauri::command]
pub async fn activate_mode(
    state: State<'_, AppState>,
    device_id: String,
    mode: OutputMode,
    profile_id: Option<String>,
) -> AppResult<ActivateResult> {
    let device_id = permissions::device_id(&device_id)?.to_string();
    if let Some(p) = &profile_id {
        permissions::profile_id(p)?;
    }
    let engine = state.engine.clone();
    let steam = blocking(health::steam_running).await?;
    engine.activate_mode(&device_id, mode, profile_id.as_deref(), steam)
}

#[tauri::command]
pub fn stop_virtual_session(state: State<'_, AppState>, session_id: String) -> AppResult<()> {
    state.engine.stop_virtual_session(permissions::session_id(&session_id)?)
}

#[tauri::command]
pub async fn run_diagnostics(state: State<'_, AppState>) -> AppResult<DiagnosticsReport> {
    let engine = state.engine.clone();
    let system = blocking(health::collect).await?;
    Ok(engine.diagnostics(system))
}

/// Texte exact qui sera écrit par `export_diagnostics` (aperçu avant consentement).
#[tauri::command]
pub async fn preview_diagnostics_export(state: State<'_, AppState>) -> AppResult<String> {
    let engine = state.engine.clone();
    let system = blocking(health::collect).await?;
    Ok(export_text(&engine.diagnostics(system)))
}

#[tauri::command]
pub async fn export_diagnostics(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let path = permissions::export_path(&path)?;
    let engine = state.engine.clone();
    let system = blocking(health::collect).await?;
    engine.export_diagnostics(&path, system)
}

/// Analyse uniquement des positions brutes saisies durant un test volontaire.
/// Taille et valeurs vérifiées dans le moteur, sans effets de bord.
#[tauri::command]
pub fn analyze_stick_drift(samples: Vec<DriftSample>) -> AppResult<DriftReport> {
    drift::analyze(&samples)
}

/// Impulsion manuelle DualShock 4 : 10–60 %, 500–2000 ms, et arrêt HID vérifié.
#[tauri::command]
pub async fn test_ds4_rumble(
    state: State<'_, AppState>,
    device_id: String,
    intensity: u8,
    duration_ms: u16,
) -> AppResult<()> {
    ensure_output_allowed()?;
    let id = permissions::device_id(&device_id)?.to_string();
    let controller = state.engine.get_controller(&id)?;
    if controller.device.model != ControllerModel::DualShock4
        || !matches!(controller.device.transport, Transport::Bluetooth | Transport::Usb)
        || !matches!(controller.state.connection_state, ConnectionState::Connected | ConnectionState::Unstable)
    {
        return Err(AppError::new(codes::DEVICE_UNREADABLE, "une DualShock 4 lisible doit être connectée"));
    }
    if !(10..=60).contains(&intensity) || !(500..=2000).contains(&duration_ms) {
        return Err(AppError::new(codes::INVALID_ARGUMENT, "intensité 10–60 %, durée 500–2000 ms"));
    }
    let rumble = state.rumble.clone();
    blocking(move || rumble.pulse(&id, intensity * 2, intensity * 3, duration_ms)).await?
}

#[tauri::command]
pub async fn stop_ds4_rumble(state: State<'_, AppState>, device_id: String) -> AppResult<()> {
    ensure_output_allowed()?;
    let id = permissions::device_id(&device_id)?.to_string();
    let rumble = state.rumble.clone();
    blocking(move || rumble.stop(&id)).await?
}

/// Mesure en lecture seule des capacités de sortie HID de la collection choisie.
/// L'identifiant local est vérifié ; aucun chemin ou numéro de série n'est retourné.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HidOutputInfo {
    pub model: String,
    pub transport: String,
    pub requested_report_id: String,
    pub requested_length: usize,
    pub windows_output_report_byte_length: Option<u16>,
    pub windows_caps_error: Option<String>,
}

#[tauri::command]
pub async fn inspect_ds4_hid(
    state: State<'_, AppState>,
    device_id: String,
) -> AppResult<HidOutputInfo> {
    let id = permissions::device_id(&device_id)?.to_string();
    let controller = state.engine.get_controller(&id)?;
    if controller.device.model != ControllerModel::DualShock4 {
        return Err(AppError::new(codes::INVALID_ARGUMENT, "diagnostic réservé aux DualShock 4"));
    }
    let transport = controller.device.transport;
    let (requested_report_id, requested_length) = match transport {
        Transport::Bluetooth => ("0x11", 78),
        Transport::Usb => ("0x05", 32),
        Transport::Unknown => return Err(AppError::new(codes::INVALID_ARGUMENT, "transport inconnu")),
    };

    #[cfg(windows)]
    {
        let (len, error) = blocking(move || {
            let api = hidapi::HidApi::new().map_err(|e| {
                AppError::new(codes::DEVICE_UNREADABLE, format!("énumération HID : {e}"))
            })?;
            let info = api.device_list()
                .find(|dev| crate::devices::model::device_id_from_path(dev.path().to_bytes()) == id)
                .ok_or_else(|| AppError::new(codes::DEVICE_NOT_FOUND, "collection HID introuvable"))?;
            let result = crate::devices::hid_caps::output_report_byte_length(
                &info.path().to_string_lossy()
            );
            Ok::<_, AppError>(match result {
                Ok(length) => (Some(length), None),
                Err(error) => (None, Some(error)),
            })
        }).await??;
        Ok(HidOutputInfo {
            model: "DualShock 4".into(),
            transport: format!("{transport:?}"),
            requested_report_id: requested_report_id.into(),
            requested_length,
            windows_output_report_byte_length: len,
            windows_caps_error: error,
        })
    }

    #[cfg(not(windows))]
    {
        let _ = id;
        Err(AppError::new(codes::DEVICE_UNREADABLE, "diagnostic réservé à Windows"))
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.engine.get_settings()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> AppResult<AppSettings> {
    state.engine.save_settings(&settings)
}
