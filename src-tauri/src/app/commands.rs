// src-tauri/src/app/commands.rs
//! Contrats IPC (section 7.3). Chaque commande valide ses entrées ; aucune ne lance de
//! commande arbitraire. Les vérifications système (lentes) tournent hors du fil principal.

use super::permissions;
use super::ui_events::{SharedTestSub, TestSub};
use crate::diagnostics::{drift::{self, DriftReport, DriftSample}, export_text, health, DiagnosticsReport};
use crate::engine::{ActivateResult, ControllerView, Engine, SaveOutcome};
use crate::error::{codes, AppError, AppResult};
use crate::profiles::schema::{AppSettings, OutputMode, Profile, ValidationReport};
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub engine: Arc<Engine>,
    pub test_sub: SharedTestSub,
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

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.engine.get_settings()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: AppSettings) -> AppResult<AppSettings> {
    state.engine.save_settings(&settings)
}
