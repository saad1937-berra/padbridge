// src-tauri/src/lib.rs
//! PadBridge — moteur local de compatibilité et de personnalisation des manettes PlayStation.
//!
//! Le moteur (tout sauf `app`) ne dépend ni de Tauri ni de l'interface : il se teste avec
//! `cargo test --no-default-features`.

pub mod devices;
pub mod diagnostics;
pub mod engine;
pub mod error;
pub mod pipeline;
pub mod profiles;
pub mod virtual_pad;

#[cfg(feature = "desktop")]
pub mod app;

#[cfg(feature = "desktop")]
pub fn run() {
    app::lifecycle::run();
}
