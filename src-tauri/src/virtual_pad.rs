// src-tauri/src/virtual_pad.rs
//! Couche « sortie virtuelle » : interface interchangeable, facultative.
//! L'application fonctionne entièrement sans backend (mode natif / diagnostic).

pub mod backend;
pub mod null_backend;
pub mod selected_backend;
// Opt-in Win32 XInput API reader for isolated tests, not a selectable backend.
#[cfg(all(windows, feature = "xinput-probe-poc"))]
pub mod xinput_probe_windows;
// POC B : protocole HID virtuel pur, sans pilotes ni IOCTL actifs.
pub mod vhf_protocol;
// Coeur de broker testable, inactif tant qu'aucun service privilegie n'existe.
pub mod vhf_broker;
// Opt-in executor POC: in tests and neutral-only SCM dry-run preview.
// Never created by selected_backend or the frontend; no driver access.
#[cfg(any(test, feature = "broker-service-poc"))]
pub mod broker_worker;
// Protocol v1 IPC (aucun acces materiel, tests deterministes).
pub mod broker_ipc;
// Windows-only opt-in PING probe, no driver access or Tauri hooks.
#[cfg(all(windows, feature = "broker-service-poc"))]
pub mod broker_client_windows;
#[cfg(all(windows, feature = "broker-service-poc"))]
pub mod broker_pipe_windows;
#[cfg(all(windows, feature = "broker-service-poc"))]
pub mod broker_auth_windows;
#[cfg(all(windows, feature = "broker-scm-poc"))]
pub mod broker_scm_windows;
// SCM preview owns a neutral-only memory-backed worker. Never opens a driver.
#[cfg(all(windows, feature = "broker-scm-poc"))]
pub mod broker_preview_runtime;
#[cfg(all(windows, feature = "broker-scm-poc"))]
pub mod broker_config_windows;
#[cfg(all(windows, feature = "broker-poc"))]
pub mod vhf_broker_windows;

/// End-to-end DS4 HID -> mapping -> in-memory broker -> 10-byte VHF wire.
// Tests only, never linked into the Tauri app or selected backend.
#[cfg(test)]
mod loopback_poc;

// Backend de test en mémoire : permet de vérifier les invariants (PB-010, PB-018).
#[cfg(test)]
pub mod mock {
    use super::backend::*;
    use crate::error::{codes, AppError, AppResult};
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    pub struct MockShared {
        pub live: BTreeSet<String>,
        pub last: BTreeMap<String, OutputState>,
        pub created: usize,
        pub destroyed: usize,
        pub fail_submit: bool,
    }

    #[derive(Clone, Default)]
    pub struct MockBackend {
        pub shared: Arc<Mutex<MockShared>>,
    }

    impl VirtualPadBackend for MockBackend {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn status(&self) -> BackendStatus {
            BackendStatus {
                available: true,
                name: "mock".into(),
                code: None,
                reason: None,
            }
        }

        fn create(&mut self, physical_id: &str) -> AppResult<SessionId> {
            let mut s = self.shared.lock().unwrap();
            let id = format!("mock-{}-{}", physical_id, s.created);
            s.created += 1;
            s.live.insert(id.clone());
            s.last.insert(id.clone(), OutputState::neutral());
            Ok(SessionId(id))
        }

        fn submit(&mut self, session: &SessionId, state: &OutputState) -> AppResult<()> {
            let mut s = self.shared.lock().unwrap();
            if s.fail_submit {
                return Err(AppError::new(codes::BACKEND_SESSION, "échec simulé"));
            }
            s.last.insert(session.0.clone(), *state);
            Ok(())
        }

        fn destroy(&mut self, session: &SessionId) -> AppResult<()> {
            let mut s = self.shared.lock().unwrap();
            s.live.remove(&session.0);
            s.destroyed += 1;
            Ok(())
        }
    }
}
