// src-tauri/src/main.rs
// Empêche l'ouverture d'une console supplémentaire sous Windows en version release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "desktop")]
    padbridge_lib::run();
}
