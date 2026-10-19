// src-tauri/src/virtual_pad/null_backend.rs
//! Backend « absent » : refuse toute création, l'application reste en mode natif.

use super::backend::{BackendStatus, OutputState, SessionId, VirtualPadBackend};
use crate::error::{codes, AppError, AppResult};

#[derive(Debug, Default)]
pub struct NullBackend;

const REASON: &str = "Aucun pilote de manette virtuelle validé n'est installé.";

impl VirtualPadBackend for NullBackend {
    fn name(&self) -> &'static str {
        "none"
    }

    fn status(&self) -> BackendStatus {
        BackendStatus {
            available: false,
            name: "none".into(),
            code: Some(codes::BACKEND_UNAVAILABLE.into()),
            reason: Some(REASON.into()),
        }
    }

    fn create(&mut self, _physical_id: &str) -> AppResult<SessionId> {
        Err(AppError::new(codes::BACKEND_UNAVAILABLE, REASON))
    }

    fn submit(&mut self, _session: &SessionId, _state: &OutputState) -> AppResult<()> {
        Err(AppError::new(codes::BACKEND_UNAVAILABLE, REASON))
    }

    fn destroy(&mut self, _session: &SessionId) -> AppResult<()> {
        Ok(())
    }
}
