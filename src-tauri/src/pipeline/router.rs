// src-tauri/src/pipeline/router.rs
//! Routage par appareil : applique le profil, alimente la sortie virtuelle si elle existe,
//! et garantit l'état neutre à la perte d'un appareil.

use super::mapping::CompiledProfile;
use crate::devices::model::InputFrame;
use crate::error::{codes, AppError, AppResult};
use crate::profiles::schema::OutputMode;
use crate::virtual_pad::backend::{BackendStatus, OutputState, SessionId, VirtualPadBackend};
use std::collections::BTreeMap;

struct Slot {
    mode: OutputMode,
    profile: CompiledProfile,
    session: Option<SessionId>,
}

pub struct ProcessResult {
    pub output: OutputState,
    /// Renseigné si la sortie virtuelle a échoué : l'appareil est alors repassé en natif.
    pub backend_error: Option<AppError>,
}

pub struct Router {
    backend: Box<dyn VirtualPadBackend>,
    slots: BTreeMap<String, Slot>,
}

impl Router {
    pub fn new(backend: Box<dyn VirtualPadBackend>) -> Self {
        Self { backend, slots: BTreeMap::new() }
    }

    pub fn backend_status(&self) -> BackendStatus {
        self.backend.status()
    }

    pub fn session_count(&self) -> usize {
        self.slots.values().filter(|s| s.session.is_some()).count()
    }

    pub fn mode_of(&self, device_id: &str) -> Option<OutputMode> {
        self.slots.get(device_id).map(|s| s.mode)
    }

    pub fn session_of(&self, device_id: &str) -> Option<&SessionId> {
        self.slots.get(device_id).and_then(|s| s.session.as_ref())
    }

    pub fn profile_of(&self, device_id: &str) -> Option<&CompiledProfile> {
        self.slots.get(device_id).map(|s| &s.profile)
    }

    /// Ajoute un appareil en mode natif (aucune sortie virtuelle).
    pub fn add_device(&mut self, device_id: &str, profile: CompiledProfile) {
        self.slots.entry(device_id.to_string()).or_insert(Slot {
            mode: OutputMode::Native,
            profile,
            session: None,
        });
    }

    /// Remplacement atomique : la trame suivante utilise entièrement le nouveau profil.
    pub fn set_profile(&mut self, device_id: &str, profile: CompiledProfile) -> bool {
        match self.slots.get_mut(device_id) {
            Some(slot) => {
                slot.profile = profile;
                true
            }
            None => false,
        }
    }

    /// Retourne le mode effectivement actif. `Xinput` échoue si le backend refuse.
    pub fn activate(&mut self, device_id: &str, mode: OutputMode) -> AppResult<OutputMode> {
        let backend = &mut self.backend;
        let slot = self
            .slots
            .get_mut(device_id)
            .ok_or_else(|| AppError::new(codes::DEVICE_NOT_FOUND, "appareil inconnu"))?;

        match mode {
            OutputMode::Native | OutputMode::Diagnostic => {
                Self::close_session(backend.as_mut(), slot);
                slot.mode = mode;
                Ok(mode)
            }
            OutputMode::Xinput => {
                if slot.session.is_none() {
                    // Au plus une sortie virtuelle par appareil physique (PB-018).
                    let session = backend.create(device_id)?;
                    slot.session = Some(session);
                }
                slot.mode = OutputMode::Xinput;
                Ok(OutputMode::Xinput)
            }
        }
    }

    /// Traite une trame. `None` si l'appareil n'est pas connu.
    pub fn process(&mut self, device_id: &str, frame: &InputFrame) -> Option<ProcessResult> {
        let backend = &mut self.backend;
        let slot = self.slots.get_mut(device_id)?;
        let output = slot.profile.apply(frame);
        let mut backend_error = None;

        if let Some(session) = slot.session.clone() {
            if let Err(err) = backend.submit(&session, &output) {
                let _ = backend.destroy(&session);
                slot.session = None;
                slot.mode = OutputMode::Native;
                backend_error = Some(err);
            }
        }
        Some(ProcessResult { output, backend_error })
    }

    /// Ferme la session portant cet identifiant ; retourne l'appareil concerné.
    pub fn stop_session(&mut self, session_id: &str) -> Option<String> {
        let backend = &mut self.backend;
        for (device_id, slot) in self.slots.iter_mut() {
            if slot.session.as_ref().map(|s| s.0.as_str()) == Some(session_id) {
                Self::close_session(backend.as_mut(), slot);
                slot.mode = OutputMode::Native;
                return Some(device_id.clone());
            }
        }
        None
    }

    /// Perte d'un appareil : état neutre envoyé, sortie détruite, emplacement supprimé.
    pub fn remove_device(&mut self, device_id: &str) {
        if let Some(mut slot) = self.slots.remove(device_id) {
            Self::close_session(self.backend.as_mut(), &mut slot);
        }
    }

    pub fn shutdown(&mut self) {
        let ids: Vec<String> = self.slots.keys().cloned().collect();
        for id in ids {
            self.remove_device(&id);
        }
    }

    fn close_session(backend: &mut dyn VirtualPadBackend, slot: &mut Slot) {
        if let Some(session) = slot.session.take() {
            // Aucun bouton ne doit rester enfoncé côté jeu (PB-010).
            let _ = backend.submit(&session, &OutputState::neutral());
            let _ = backend.destroy(&session);
        }
    }
}
