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
        // Always attempt close, even if the neutral command fails.
        let neutral_result = self.transport.submit(handle, &neutral);
        let close_result = self.transport.close(handle);
        match (neutral_result, close_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(neutral), Ok(())) => Err(Self::error(format!(
                "neutralization failed: {neutral}"
            ))),
            (Ok(()), Err(close)) => Err(Self::error(format!(
                "close failed: {close}"
            ))),
            (Err(neutral), Err(close)) => Err(Self::error(format!(
                "neutralization failed: {neutral}; close failed: {close}"
            ))),
        }
    }

    /// Explicitly revoke every owned session following service stop, input
    /// disconnect or provider loss. Tries to close all sessions even if one
    /// fails; future providers MUST provide bounded and cancellable I/O.
    pub fn invalidate_all(&mut self) -> AppResult<()> {
        let owned = std::mem::take(&mut self.sessions);
        let mut failures = Vec::new();
        for (_, active) in owned {
            if let Err(error) = self.close_owned(active.handle) {
                failures.push(error.to_string());
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(AppError::new(codes::BACKEND_SESSION, format!(
                "{} Xbox session(s) revoked with cleanup errors: {}",
                failures.len(), failures.join("; ")
            )))
        }
    }

    fn ensure_ready(&mut self) -> AppResult<()> {
        if self.transport.is_ready() {
            return Ok(());
        }
        // Provider loss must never leave another device's old session alive.
        let cleanup = self.invalidate_all();
        Err(AppError::new(codes::BACKEND_UNAVAILABLE, match cleanup {
            Ok(()) => "Xbox/XUSB provider disconnected; sessions revoked".to_owned(),
            Err(error) => format!(
                "Xbox/XUSB provider disconnected; cleanup also failed: {error}"
            ),
        }))
    }

    fn fail_and_revoke(&mut self, failure: io::Error) -> AppError {
        // Treat a write error as global transport failure: every controller
        // sharing the same provider is revoked, not just the failed handle.
        let cleanup = self.invalidate_all();
        match cleanup {
            Ok(()) => Self::error(failure),
            Err(error) => AppError::new(codes::BACKEND_SESSION, format!(
                "Xbox transport failed: {failure}; cleanup also failed: {error}"
            )),
        }
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
        self.ensure_ready()?;
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
        // Never reuse an old session ID, even in an extreme counter-overflow
        // scenario. Check before opening the provider handle.
        let next_serial = self.serial.checked_add(1)
            .ok_or_else(|| AppError::new(codes::BACKEND_SESSION,
                "Xbox session ID sequence exhausted"))?;
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
        self.serial = next_serial;
        let session_id = SessionId(format!("xbox-adapter-{}", next_serial));
        self.sessions.insert(session_id.0.clone(), ActiveSession {
            physical_id: physical_id.to_owned(),
            handle,
        });
        Ok(session_id)
    }

    fn submit(&mut self, session: &SessionId, state: &OutputState) -> AppResult<()> {
        // A provider that disappeared invalidates ALL existing handles before
        // inspecting this packet or attempting any more active writes.
        self.ensure_ready()?;
        let report = validated_xinput_state(state)
            .map_err(|err| AppError::new(codes::INVALID_ARGUMENT,
                format!("refused invalid Xbox state: {err:?}")))?;
        let handle = self.sessions.get(&session.0)
            .ok_or_else(|| AppError::new(codes::BACKEND_SESSION,
                "unknown or revoked Xbox session"))?.handle;
        if let Err(error) = self.transport.submit(handle, &report) {
            return Err(self.fail_and_revoke(error));
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
        // Destruction revokes all sessions, without masking cleanup failures
        // as a successful device-level neutralization.
        let _ = self.invalidate_all();
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
            if !s.ready {
                return Err(io::Error::new(io::ErrorKind::NotConnected, "provider offline"));
            }
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

    #[test]
    fn lost_provider_revokes_every_session_even_when_neutral_fails() {
        let (mut adapter, trace) = rig(true);
        let old_a = adapter.create("a").unwrap();
        let old_b = adapter.create("b").unwrap();
        adapter.submit(&old_a, &pressed()).unwrap();
        trace.lock().unwrap().ready = false;
        assert!(!adapter.status().available);
        // Offline neutral commands fail, but both close operations must run.
        let error = adapter.submit(&old_a, &pressed()).unwrap_err();
        assert_eq!(error.code, codes::BACKEND_UNAVAILABLE);
        assert!(error.detail.contains("cleanup also failed"));
        let state = trace.lock().unwrap();
        assert!(state.live.is_empty());
        assert_eq!(state.messages.iter().filter(|x| x.1 == "close").count(), 2);
        drop(state);

        trace.lock().unwrap().ready = true;
        let new_id = adapter.create("a").unwrap();
        assert_ne!(new_id.0, old_a.0);
        assert_eq!(adapter.submit(&old_b, &pressed()).unwrap_err().code,
                   codes::BACKEND_SESSION);
        assert!(adapter.submit(&new_id, &pressed()).is_ok());
        adapter.invalidate_all().unwrap();
        assert!(trace.lock().unwrap().live.is_empty());
    }

    #[test]
    fn one_active_submit_error_invalidates_all_controller_handles() {
        let (mut adapter, trace) = rig(true);
        let a = adapter.create("one").unwrap();
        let b = adapter.create("two").unwrap();
        trace.lock().unwrap().fail_next_submit = true;
        assert_eq!(adapter.submit(&a, &pressed()).unwrap_err().code, codes::BACKEND_SESSION);
        assert!(trace.lock().unwrap().live.is_empty());
        assert_eq!(trace.lock().unwrap().messages.iter()
                   .filter(|m| m.1 == "close").count(), 2);
        assert_eq!(adapter.submit(&b, &pressed()).unwrap_err().code,
                   codes::BACKEND_SESSION);
    }

    #[test]
    fn explicit_stop_releases_every_session_and_is_idempotent() {
        let (mut adapter, trace) = rig(true);
        adapter.create("a").unwrap();
        adapter.create("b").unwrap();
        adapter.invalidate_all().unwrap();
        let n = trace.lock().unwrap().messages.len();
        adapter.invalidate_all().unwrap();
        assert_eq!(trace.lock().unwrap().messages.len(), n);
        assert!(trace.lock().unwrap().live.is_empty());
    }

    #[test]
    fn neutral_and_close_errors_are_both_reported_and_other_handles_closed() {
        let (mut adapter, trace) = rig(true);
        adapter.create("a").unwrap();
        adapter.create("b").unwrap();
        {
            let mut state = trace.lock().unwrap();
            state.fail_next_submit = true;
            state.fail_next_close = true;
        }
        let error = adapter.invalidate_all().unwrap_err();
        assert!(error.detail.contains("neutralization failed"));
        assert!(error.detail.contains("close failed"));
        assert!(trace.lock().unwrap().live.is_empty());
        assert_eq!(trace.lock().unwrap().messages.iter()
                   .filter(|m| m.1 == "close").count(), 2);
    }

    #[test]
    fn exhausted_session_counter_fails_before_new_device_open() {
        let (mut adapter, trace) = rig(true);
        let _ = adapter.create("one").unwrap();
        adapter.serial = u64::MAX;
        let old_messages = trace.lock().unwrap().messages.len();
        assert_eq!(adapter.create("two").unwrap_err().code, codes::BACKEND_SESSION);
        assert_eq!(trace.lock().unwrap().messages.len(), old_messages);
        drop(adapter);
        assert!(trace.lock().unwrap().live.is_empty());
    }

    #[test]
    fn disconnect_reconnect_never_resurrects_stale_controller_sessions() {
        let (mut adapter, trace) = rig(true);
        let first = adapter.create("first").unwrap();
        let second = adapter.create("second").unwrap();
        adapter.submit(&first, &pressed()).unwrap();
        adapter.submit(&second, &pressed()).unwrap();
        trace.lock().unwrap().ready = false;
        assert_eq!(adapter.submit(&first, &pressed()).unwrap_err().code,
                   codes::BACKEND_UNAVAILABLE);
        assert!(trace.lock().unwrap().live.is_empty());
        trace.lock().unwrap().ready = true;
        let replacement = adapter.create("first").unwrap();
        assert_ne!(first.0, replacement.0);
        let previous = trace.lock().unwrap().messages.len();
        assert_eq!(adapter.submit(&first, &pressed()).unwrap_err().code,
                   codes::BACKEND_SESSION);
        assert_eq!(adapter.submit(&second, &pressed()).unwrap_err().code,
                   codes::BACKEND_SESSION);
        assert_eq!(trace.lock().unwrap().messages.len(), previous,
                   "stale session must not write to recovered provider");
        adapter.submit(&replacement, &pressed()).unwrap();
        adapter.destroy(&replacement).unwrap();
        assert!(trace.lock().unwrap().live.is_empty());
    }

    #[test]
    fn failed_submit_releases_other_controller_and_recovery_requires_new_ids() {
        let (mut adapter, trace) = rig(true);
        let first = adapter.create("first").unwrap();
        let second = adapter.create("second").unwrap();
        trace.lock().unwrap().fail_next_submit = true;
        assert_eq!(adapter.submit(&second, &pressed()).unwrap_err().code,
                   codes::BACKEND_SESSION);
        assert!(trace.lock().unwrap().live.is_empty());
        let count = trace.lock().unwrap().messages.len();
        assert_eq!(adapter.submit(&first, &pressed()).unwrap_err().code,
                   codes::BACKEND_SESSION);
        assert_eq!(trace.lock().unwrap().messages.len(), count);
        let new_first = adapter.create("first").unwrap();
        let new_second = adapter.create("second").unwrap();
        assert_ne!(first.0, new_first.0);
        assert_ne!(second.0, new_second.0);
        adapter.invalidate_all().unwrap();
        assert!(trace.lock().unwrap().live.is_empty());
    }

    #[test]
    fn invalid_device_identity_never_opens_a_provider_handle() {
        let (mut adapter, trace) = rig(true);
        assert_eq!(adapter.create("").unwrap_err().code, codes::INVALID_ARGUMENT);
        assert!(trace.lock().unwrap().messages.is_empty());
    }

    #[test]
    fn provider_disconnected_before_invalid_input_also_revokes_handles() {
        let (mut adapter, trace) = rig(true);
        let id = adapter.create("one").unwrap();
        trace.lock().unwrap().ready = false;
        let mut invalid = OutputState::neutral();
        invalid.lx = f32::NAN;
        assert_eq!(adapter.submit(&id, &invalid).unwrap_err().code,
                   codes::BACKEND_UNAVAILABLE);
        assert!(trace.lock().unwrap().live.is_empty());
    }
}
