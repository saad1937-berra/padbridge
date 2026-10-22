// src-tauri/src/app/lifecycle.rs
//! Démarrage et arrêt : journalisation, moteur, surveillance HID, échantillonnage UI.
//! À la sortie, toutes les sorties virtuelles sont neutralisées et détruites.

use super::commands::{self, AppState};
use super::ui_events;
use crate::engine::Engine;
use crate::devices::rumble::RumbleRegistry;
use crate::profiles::store::SettingsStore;
use crate::virtual_pad::selected_backend;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Conservé dans l'état Tauri : le garde de journalisation et les fils à arrêter.
struct Runtime {
    stop: Arc<AtomicBool>,
    #[cfg(windows)]
    monitor: Mutex<Option<crate::devices::enumeration::HidMonitor>>,
    _log_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

fn init_logging(dir: &Path, retention_days: u32) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::rolling::{Builder, Rotation};
    std::fs::create_dir_all(dir).ok()?;
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("padbridge")
        .filename_suffix("log")
        .max_log_files(retention_days as usize)
        .build(dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .try_init()
        .ok()?;
    Some(guard)
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;

    let retention = SettingsStore::new(data_dir.join("settings.json")).load().log_retention_days;
    let log_guard = init_logging(&data_dir.join("logs"), retention);

    // Faire échouer proprement l'initialisation si la zone de notification ne peut
    // pas être créée, avant de démarrer les lecteurs et threads HID.
    #[cfg(windows)]
    super::tray::install(app)?;

    let engine = Arc::new(Engine::new(&data_dir, selected_backend::select())?);
    let stop = Arc::new(AtomicBool::new(false));
    let test_sub: ui_events::SharedTestSub = Arc::new(Mutex::new(None));
    let rumble = Arc::new(RumbleRegistry::default());

    #[cfg(windows)]
    let monitor = Mutex::new(Some(crate::devices::enumeration::spawn(engine.clone(), stop.clone(), rumble.clone())));

    ui_events::spawn(app.handle().clone(), engine.clone(), test_sub.clone(), stop.clone());

    app.manage(AppState { engine, test_sub, rumble });
    app.manage(Runtime {
        stop,
        #[cfg(windows)]
        monitor,
        _log_guard: log_guard,
    });
    Ok(())
}

fn shutdown(app: &tauri::AppHandle) {
    if let Some(runtime) = app.try_state::<Runtime>() {
        runtime.stop.store(true, Ordering::Relaxed);
        #[cfg(windows)]
        if let Ok(mut guard) = runtime.monitor.lock() {
            if let Some(monitor) = guard.take() {
                monitor.join();
            }
        }
    }
    if let Some(state) = app.try_state::<AppState>() {
        state.engine.shutdown();
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            commands::list_controllers,
            commands::get_controller_state,
            commands::start_input_test,
            commands::stop_input_test,
            commands::inspect_ds4_hid,
            commands::test_ds4_rumble,
            commands::stop_ds4_rumble,
            commands::validate_profile,
            commands::save_profile,
            commands::list_profiles,
            commands::get_profile,
            commands::duplicate_profile,
            commands::rename_profile,
            commands::reset_profile,
            commands::activate_mode,
            commands::stop_virtual_session,
            commands::run_diagnostics,
            commands::analyze_stick_drift,
            commands::preview_diagnostics_export,
            commands::export_diagnostics,
            commands::get_settings,
            commands::save_settings,
        ])
        .build(tauri::generate_context!())
        .expect("échec de la construction de l'application PadBridge")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                shutdown(app);
            }
        });
}
