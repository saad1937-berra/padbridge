//! Vendor-neutral Xbox backend lifecycle, with NO concrete driver transport.
//!
//! Compiles as ordinary Rust but is never selected by the application.
//! The caller must separately audit a signed Xbox/XUSB provider, its license,
//! process/IPC privileges, device provenance and stop/timeout behavior.
//! A 12-byte XINPUT_GAMEPAD state is NOT an XUSB transport wire packet.

use std::collections::BTreeMap;
use std::io;

use crate::error::{codes, AppError, AppResult};
use super::backend::{
    BackendStatus, OutputState, SessionId, VirtualPadBackend, XInputReport,
};
use super::xinput_state_contract::validated_xinput_state;

/// Future implementations MUST provide a bounded, authenticated transport
/// to an independently installed and authorized Xbox/XUSB provider.
/// Handle ownership is exclusive to a single XboxAdapter instance.
pub trait XboxTransport: Send {
    fn is_ready(&self) -> bool;
    fn open(&mut self) -> io::Result<u64>;
    fn submit(&mut self, handle: u64, state: &XInputReport) -> io::Result<()>;
    fn close(&mut self, handle: u64) -> io::Result<()>;
}

struct ActiveSession {
    physical_id: String,
    handle: u64,
}

/// At most four independently owned sessions. This is a defensive cap, not
/// proof that Windows has free XInput slots or that this backend is XUSB.
pub const MAX_SESSIONS: usize = 4;

pub struct XboxAdapter<T: XboxTransport> {
    transport: T,
    sessions: BTreeMap<String, ActiveSession>,
    serial: u64,
}

impl<T: XboxTransport> XboxAdapter<T> {
    pub fn new(transport: T) -> Self {
        Self { transport, sessions: BTreeMap::new(), serial: 0 }
    }

    fn error(error: impl std::fmt::Display) -> AppError {
        AppError::new(codes::BACKEND_SESSION, error.to_string())
    }

    fn close_owned(&mut self, handle: u64) -> AppResult<()> {
        let neutral = validated_xinput_state(&OutputState::neutral())
            .expect("neutral is always valid");
        // Even if neutral fails, always close. Return the first failure.
        let neutral_result = self.transport.submit(handle, &neutral);
        let close_result = self.transport.close(handle);
        neutral_result.and(close_result).map_err(Self::error)
    }

    fn fail_and_revoke(&mut self, session: &SessionId, handle: u64, failure: io::Error) -> AppError {
        // An unreliable provider must never stay owned after a failed submit.
        self.sessions.remove(&session.0);
        let cleanup = self.close_owned(handle);
        if let Err(cleanup) = cleanup {
            return AppError::new(codes::BACKEND_SESSION, format!(
                "Xbox transport failed: {failure}; cleanup also failed: {cleanup}"
            ));
        }
        Self::error(failure)
    }
}

impl<T: XboxTransport> VirtualPadBackend for XboxAdapter<T> {
    fn name(&self) -> &'static str { "unselected-xbox-adapter" }

    fn status(&self) -> BackendStatus {
        if self.transport.is_ready() {
            BackendStatus {
                available: true,
                name: self.name().into(),
                code: None,
                reason: None,
            }
        } else {
            BackendStatus {
                available: false,
                name: self.name().into(),
                code: Some(codes::BACKEND_UNAVAILABLE.into()),
                reason: Some("No verified external Xbox/XUSB transport".into()),
            }
        }
    }

    fn create(&mut self, physical_id: &str) -> AppResult<SessionId> {
        if !self.transport.is_ready() {
            return Err(AppError::new(codes::BACKEND_UNAVAILABLE,
                "No verified Xbox/XUSB transport available"));
        }
        if physical_id.is_empty() {
            return Err(AppError::new(codes::INVALID_ARGUMENT, "empty physical device ID"));
        }
        if self.sessions.values().any(|value| value.physical_id == physical_id) {
            return Err(AppError::new(codes::BACKEND_SESSION,
                "this physical device already owns an Xbox session"));
        }
        if self.sessions.len() >= MAX_SESSIONS {
            return Err(AppError::new(codes::BACKEND_SESSION,
                "adapter reached its four-session safety cap"));
        }
        let handle = self.transport.open().map_err(Self::error)?;
        // Never expose a new handle before its first safe neutral command.
        let neutral = validated_xinput_state(&OutputState::neutral())
            .expect("neutral is always valid");
        if let Err(err) = self.transport.submit(handle, &neutral) {
            // Try a final neutral, then close even if the retry fails.
            let cleanup = self.close_owned(handle);
            return Err(AppError::new(codes::BACKEND_SESSION, match cleanup {
                Ok(()) => format!("initial neutral submit failed: {err}"),
                Err(cleanup) => format!(
                    "initial neutral submit failed: {err}; cleanup also failed: {cleanup}"
                ),
            }));
        }
        self.serial = self.serial.wrapping_add(1);
        let session_id = SessionId(format!("xbox-adapter-{}", self.serial));
        self.sessions.insert(session_id.0.clone(), ActiveSession {
            physical_id: physical_id.to_owned(),
            handle,
        });
        Ok(session_id)
    }

    fn submit(&mut self, session: &SessionId, state: &OutputState) -> AppResult<()> {
        let report = validated_xinput_state(state)
            .map_err(|err| AppError::new(codes::INVALID_ARGUMENT,
                format!("refused invalid Xbox state: {err:?}")))?;
        let handle = self.sessions.get(&session.0)
            .ok_or_else(|| AppError::new(codes::BACKEND_SESSION,
                "unknown or revoked Xbox session"))?.handle;
        if let Err(error) = self.transport.submit(handle, &report) {
            return Err(self.fail_and_revoke(session, handle, error));
        }
        Ok(())
    }

    fn destroy(&mut self, session: &SessionId) -> AppResult<()> {
        if let Some(active) = self.sessions.remove(&session.0) {
            self.close_owned(active.handle)?;
        }
        // Idempotent destroy does not re-create a controller.
        Ok(())
    }
}

impl<T: XboxTransport> Drop for XboxAdapter<T> {
    fn drop(&mut self) {
        // Destruction releases every locally owned session best-effort.
        // A future production transport must have bounded I/O.
        let handles: Vec<_> = self.sessions.values().map(|active| active.handle).collect();
        self.sessions.clear();
        for handle in handles {
            let _ = self.close_owned(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::backend::XButton;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Trace {
        next: u64,
        messages: Vec<(u64, &'static str, Option<XInputReport>)>,
        live: Vec<u64>,
        fail_next_submit: bool,
        fail_next_close: bool,
        ready: bool,
    }

    struct MemoryTransport(Arc<Mutex<Trace>>);

    impl XboxTransport for MemoryTransport {
        fn is_ready(&self) -> bool { self.0.lock().unwrap().ready }
        fn open(&mut self) -> io::Result<u64> {
            let mut s = self.0.lock().unwrap();
            s.next += 1;
            let handle = s.next;
            s.live.push(handle);
            s.messages.push((handle, "open", None));
            Ok(handle)
        }
        fn submit(&mut self, handle: u64, state: &XInputReport) -> io::Result<()> {
            let mut s = self.0.lock().unwrap();
            if !s.live.contains(&handle) {
                return Err(io::Error::new(io::ErrorKind::NotConnected, "unknown handle"));
            }
            s.messages.push((handle, "submit", Some(*state)));
            if s.fail_next_submit {
                s.fail_next_submit = false;
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "simulated submit failure"));
            }
            Ok(())
        }
        fn close(&mut self, handle: u64) -> io::Result<()> {
            let mut s = self.0.lock().unwrap();
            s.messages.push((handle, "close", None));
            s.live.retain(|item| *item != handle);
            if s.fail_next_close {
                s.fail_next_close = false;
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "simulated close failure"));
            }
            Ok(())
        }
    }

    fn rig(ready: bool) -> (XboxAdapter<MemoryTransport>, Arc<Mutex<Trace>>) {
        let trace = Arc::new(Mutex::new(Trace { ready, ..Trace::default() }));
        (XboxAdapter::new(MemoryTransport(trace.clone())), trace)
    }

    fn pressed() -> OutputState {
        let mut state = OutputState::neutral();
        state.buttons.set(XButton::A, true);
        state
    }

    #[test]
    fn unavailable_provider_cannot_create_any_handle() {
        let (mut adapter, trace) = rig(false);
        assert!(!adapter.status().available);
        assert_eq!(adapter.create("dev").unwrap_err().code, codes::BACKEND_UNAVAILABLE);
        assert!(trace.lock().unwrap().messages.is_empty());
    }

    #[test]
    fn initial_neutral_active_update_and_final_neutral_are_ordered() {
        let (mut adapter, trace) = rig(true);
        let id = adapter.create("a").unwrap();
        adapter.submit(&id, &pressed()).unwrap();
        adapter.destroy(&id).unwrap();
        adapter.destroy(&id).unwrap();
        let s = trace.lock().unwrap();
        let kinds: Vec<_> = s.messages.iter().map(|m| m.1).collect();
        assert_eq!(kinds, ["open", "submit", "submit", "submit", "close"]);
        assert_eq!(s.messages[1].2.unwrap().buttons, 0);
        assert_eq!(s.messages[2].2.unwrap().buttons, XButton::A.xinput_mask());
        assert_eq!(s.messages[3].2.unwrap().buttons, 0);
        assert!(s.live.is_empty());
    }

    #[test]
    fn rejects_duplicate_device_and_limits_four_sessions() {
        let (mut adapter, trace) = rig(true);
        for index in 0..MAX_SESSIONS {
            adapter.create(&format!("dev-{index}")).unwrap();
        }
        assert!(adapter.create("dev-1").is_err());
        assert!(adapter.create("fifth").is_err());
        assert_eq!(trace.lock().unwrap().live.len(), 4);
        drop(adapter);
        assert!(trace.lock().unwrap().live.is_empty());
    }

    #[test]
    fn invalid_number_is_rejected_before_transport_submission() {
        let (mut adapter, trace) = rig(true);
        let id = adapter.create("a").unwrap();
        let mut state = pressed();
        state.ly = f32::NAN;
        let count = trace.lock().unwrap().messages.len();
        assert_eq!(adapter.submit(&id, &state).unwrap_err().code, codes::INVALID_ARGUMENT);
        assert_eq!(trace.lock().unwrap().messages.len(), count);
        adapter.destroy(&id).unwrap();
    }

    #[test]
    fn failed_active_submit_is_revoked_and_neutralized() {
        let (mut adapter, trace) = rig(true);
        let id = adapter.create("a").unwrap();
        trace.lock().unwrap().fail_next_submit = true;
        assert_eq!(adapter.submit(&id, &pressed()).unwrap_err().code, codes::BACKEND_SESSION);
        assert!(adapter.submit(&id, &pressed()).is_err());
        assert!(trace.lock().unwrap().live.is_empty());
        let last: Vec<_> = trace.lock().unwrap().messages.iter()
            .rev().take(2).map(|value| value.1).collect();
        assert_eq!(last, ["close", "submit"]);
    }

    #[test]
    fn failed_initial_neutral_rolls_back_handle() {
        let (mut adapter, trace) = rig(true);
        trace.lock().unwrap().fail_next_submit = true;
        assert_eq!(adapter.create("a").unwrap_err().code, codes::BACKEND_SESSION);
        assert!(trace.lock().unwrap().live.is_empty());
    }

    #[test]
    fn close_error_is_reported_and_owned_handle_is_removed() {
        let (mut adapter, trace) = rig(true);
        let id = adapter.create("a").unwrap();
        trace.lock().unwrap().fail_next_close = true;
        assert!(adapter.destroy(&id).is_err());
        assert!(trace.lock().unwrap().live.is_empty());
        assert!(adapter.destroy(&id).is_ok());
    }

    #[test]
    fn unexpected_drop_leaves_no_simulated_ghost_controller() {
        let (mut adapter, trace) = rig(true);
        adapter.create("a").unwrap();
        adapter.create("b").unwrap();
        drop(adapter);
        let state = trace.lock().unwrap();
        assert!(state.live.is_empty());
        assert_eq!(state.messages.iter().filter(|m| m.1 == "close").count(), 2);
    }
}
