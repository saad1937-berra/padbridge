// src-tauri/src/app.rs
//! Couche Tauri : commandes IPC, événements vers l'interface, cycle de vie.
//! L'interface ne touche jamais directement aux pilotes : tout passe par le moteur.

pub mod commands;
pub mod lifecycle;
pub mod permissions;
pub mod ui_events;
