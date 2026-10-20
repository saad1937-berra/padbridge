//! POC B / broker core: validates and forwards PadBridge HID reports.
//!
//! This is NOT a Windows service, an IPC endpoint, an XInput backend or an
//! auto-starting driver client. The real transport is feature-gated and cannot
//! be selected by the Tauri app. No automatic elevation.
//!
//! The kernel watchdog expires held buttons in 800ms. A future authenticated
//! broker must call heartbeat() at least every 200ms even if nothing changed.

use std::io;
use std::time::{Duration, Instant};

use super::backend::OutputState;
use super::vhf_protocol::{encode, validate_report, REPORT_LEN, WATCHDOG_INTERVAL_MS};

pub type Report = [u8; REPORT_LEN];
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(WATCHDOG_INTERVAL_MS as u64);

/// A trusted transport (Windows DeviceIoControl, or a simulated unit-test sink).
/// Sending a report must be synchronous. The kernel independently validates it.
pub trait ReportTransport {
    fn send(&mut self, report: &Report) -> io::Result<()>;
}

/// Single-session core; keeping exclusive access is the future service's job.
/// Dropping the session ALWAYS attempts to send neutral before dropping transport.
pub struct BrokerSession<T: ReportTransport> {
    transport: T,
    last_report: Report,
    last_sent: Instant,
    closed: bool,
}

impl<T: ReportTransport> BrokerSession<T> {
    /// Fail closed if the initial neutral report could not be submitted.
    pub fn begin(mut transport: T) -> io::Result<Self> {
        let neutral = encode(&OutputState::neutral());
        transport.send(&neutral)?;
        Ok(Self { transport, last_report: neutral, last_sent: Instant::now(), closed: false })
    }

    pub fn submit(&mut self, state: &OutputState) -> io::Result<()> {
        self.send_validated(&encode(state))
    }

    /// Bounded to a known ABI. Never forwards an arbitrary byte buffer to kernel.
    pub fn send_validated(&mut self, report: &Report) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "broker session closed"));
        }
        validate_report(report).map_err(|msg| io::Error::new(io::ErrorKind::InvalidInput, msg))?;
        match self.transport.send(report) {
            Ok(()) => {
                self.last_report = *report;
                self.last_sent = Instant::now();
                Ok(())
            }
            Err(err) => {
                // Best effort release if the device is still reachable.
                self.force_close();
                Err(err)
            }
        }
    }

    /// Re-send the current *active* report periodically. The kernel watchdog
    /// will neutralize controls on a broker crash or disconnected input source.
    /// The caller must track input disconnection and call close() promptly.
    pub fn heartbeat(&mut self) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "broker session closed"));
        }
        let neutral = encode(&OutputState::neutral());
        if self.last_report != neutral && self.last_sent.elapsed() >= HEARTBEAT_INTERVAL {
            let report = self.last_report;
            self.send_validated(&report)?;
        }
        Ok(())
    }

    pub fn close(&mut self) -> io::Result<()> {
        if self.closed { return Ok(()); }
        self.closed = true;
        self.last_report = encode(&OutputState::neutral());
        self.transport.send(&self.last_report)
    }

    pub fn is_closed(&self) -> bool { self.closed }

    fn force_close(&mut self) {
        let _ = self.close();
    }
}

impl<T: ReportTransport> Drop for BrokerSession<T> {
    fn drop(&mut self) {
        self.force_close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_pad::backend::XButton;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct SimulatedState { reports: Vec<Report>, fail_at: Option<usize> }

    struct SimulatedTransport(Arc<Mutex<SimulatedState>>);

    impl ReportTransport for SimulatedTransport {
        fn send(&mut self, report: &Report) -> io::Result<()> {
            let mut state = self.0.lock().unwrap();
            if state.fail_at == Some(state.reports.len()) {
                // Only fail this command, so the reset attempt can still succeed.
                state.fail_at = None;
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "simulated disconnect"));
            }
            state.reports.push(*report);
            Ok(())
        }
    }

    fn setup() -> (BrokerSession<SimulatedTransport>, Arc<Mutex<SimulatedState>>) {
        let state = Arc::new(Mutex::new(SimulatedState::default()));
        let session = BrokerSession::begin(SimulatedTransport(state.clone())).unwrap();
        (session, state)
    }

    #[test]
    fn start_sends_neutral_and_drop_releases_buttons() {
        let (mut session, log) = setup();
        let mut press = OutputState::neutral();
        press.buttons.set(XButton::A, true);
        session.submit(&press).unwrap();
        drop(session);
        let reports = &log.lock().unwrap().reports;
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0], encode(&OutputState::neutral()));
        assert_eq!(reports[1][1], 1);
        assert_eq!(reports[2], encode(&OutputState::neutral()));
    }

    #[test]
    fn close_is_idempotent_and_rejects_new_input() {
        let (mut session, log) = setup();
        session.close().unwrap();
        session.close().unwrap();
        assert!(session.is_closed());
        assert_eq!(session.submit(&OutputState::neutral()).unwrap_err().kind(),
            io::ErrorKind::BrokenPipe);
        drop(session);
        assert_eq!(log.lock().unwrap().reports.len(), 2);
    }

    #[test]
    fn a_transport_failure_closes_and_attempts_reset() {
        let (mut session, log) = setup();
        log.lock().unwrap().fail_at = Some(1);
        let mut press = OutputState::neutral();
        press.buttons.set(XButton::A, true);
        assert!(session.submit(&press).is_err());
        assert!(session.is_closed());
        assert_eq!(log.lock().unwrap().reports.len(), 2);
        assert_eq!(log.lock().unwrap().reports[1], encode(&OutputState::neutral()));
    }

    #[test]
    fn invalid_wire_data_never_reaches_transport() {
        let (mut session, log) = setup();
        let mut report = encode(&OutputState::neutral());
        report[3] = 0x9;
        assert_eq!(session.send_validated(&report).unwrap_err().kind(),
            io::ErrorKind::InvalidInput);
        assert_eq!(log.lock().unwrap().reports.len(), 1);
        assert!(!session.is_closed());
    }

    #[test]
    fn heartbeat_on_neutral_does_not_write() {
        let (mut session, log) = setup();
        session.heartbeat().unwrap();
        assert_eq!(log.lock().unwrap().reports.len(), 1);
    }
}
