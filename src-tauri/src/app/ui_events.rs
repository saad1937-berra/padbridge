// src-tauri/src/app/ui_events.rs
//! Événements vers l'interface. L'échantillonnage UI (~30 Hz) est indépendant des
//! boucles d'acquisition : un flux haute fréquence ne sature jamais l'IPC.

use crate::engine::Engine;
use crate::pipeline::mapping::CompiledProfile;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
pub struct TestSub {
    pub device_id: String,
    /// Profil brouillon pour l'aperçu en direct (sinon le profil actif de l'appareil).
    pub preview: Option<CompiledProfile>,
}

pub type SharedTestSub = Arc<Mutex<Option<TestSub>>>;

const FRAME_INTERVAL: Duration = Duration::from_millis(33);

pub fn spawn(app: AppHandle, engine: Arc<Engine>, sub: SharedTestSub, stop: Arc<AtomicBool>) {
    let _ = thread::Builder::new()
        .name("padbridge-ui-events".into())
        .spawn(move || {
            let mut last_generation = 0;
            while !stop.load(Ordering::Relaxed) {
                let generation = engine.generation();
                if generation != last_generation {
                    last_generation = generation;
                    let _ = app.emit("controllers-changed", generation);
                }
                let current = sub.lock().ok().and_then(|g| g.clone());
                if let Some(sub) = current {
                    if let Some(sample) = engine.sample(&sub.device_id, sub.preview.as_ref()) {
                        let _ = app.emit("input-sample", sample);
                    }
                }
                thread::sleep(FRAME_INTERVAL);
            }
        });
}
