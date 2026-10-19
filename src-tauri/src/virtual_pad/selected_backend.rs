// src-tauri/src/virtual_pad/selected_backend.rs
//! Point unique de sélection du backend XInput.
//!
//! ADR en attente (POC B) : tant qu'un backend signé, sous licence redistribuable et
//! maintenu n'est pas validé, on renvoie `NullBackend` et PadBridge reste en
//! « natif / diagnostic ». ViGEmBus est archivé : ne pas l'ajouter par défaut.
//!
//! Pour brancher un backend validé : créer `mon_backend.rs` qui implémente
//! `VirtualPadBackend` (voir `OutputState::to_xinput`), puis le retourner ici.

use super::backend::VirtualPadBackend;
use super::null_backend::NullBackend;

pub fn select() -> Box<dyn VirtualPadBackend> {
    Box::new(NullBackend)
}
