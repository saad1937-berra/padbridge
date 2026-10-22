//! Windows SCM preview runtime: a real heartbeat executor with a **NO-DRIVER**
//! transport. This is intentionally NOT a kernel broker / XInput backend.
//!
//! The runtime only emits neutral VHF *bytes into an in-memory counter*.
//! In particular it has no device handle and no method that forwards IPC
//! input to the worker. Production adoption requires an independently audited
//! driver transport, authenticated input bridge and lifecycle tests in a VM.

use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use super::backend::OutputState;
use super::broker_worker::BrokerWorker;
use super::vhf_broker::{Report, ReportTransport};
use super::vhf_protocol::encode;

#[derive(Clone)]
struct NoDriverSink {
    neutral_writes: Arc<AtomicU64>,
}

impl ReportTransport for NoDriverSink {
    fn send(&mut self, report: &Report) -> io::Result<()> {
        // Defense in depth: even a future accidental queue path cannot
        // inject a pressed button through the SCM preview.
        if *report != encode(&OutputState::neutral()) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "SCM preview forbids active reports",
            ));
        }
        self.neutral_writes.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

/// Owns the autonomous worker for the lifetime of the dry-run SCM service.
/// The private worker has no producer exposed through PreviewRuntime.
pub struct PreviewRuntime {
    worker: Option<BrokerWorker>,
    neutral_writes: Arc<AtomicU64>,
}

impl PreviewRuntime {
    pub fn start() -> io::Result<Self> {
        let neutral_writes = Arc::new(AtomicU64::new(0));
        let sink = NoDriverSink { neutral_writes: neutral_writes.clone() };
        let worker = BrokerWorker::start(sink)?;
        Ok(Self { worker: Some(worker), neutral_writes })
    }

    /// Close the producer, join the worker and send a final neutral report
    /// *to memory only*. A nonzero error must be surfaced to SCM by the caller.
    pub fn shutdown(mut self) -> io::Result<()> {
        if let Some(worker) = self.worker.take() {
            worker.shutdown()?;
        }
        Ok(())
    }

    /// Diagnostic counters are local to the service and do not identify users.
    /// They count in-memory neutral frames, not successful driver writes.
    pub fn neutral_count(&self) -> u64 {
        self.neutral_writes.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::backend::XButton;
    use super::super::broker_ipc::{encode_request, handle_dry_run, Command, Status};
    use super::super::vhf_protocol::{validate_report, REPORT_LEN};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn runtime_starts_and_stops_only_with_in_memory_neutrals() {
        let runtime = PreviewRuntime::start().unwrap();
        assert_eq!(runtime.neutral_count(), 1, "start sends one neutral to memory");
        thread::sleep(Duration::from_millis(120));
        assert_eq!(runtime.neutral_count(), 1, "neutral state needs no keepalive");
        runtime.shutdown().unwrap();
    }

    #[test]
    fn no_driver_sink_rejects_active_gamepad_reports() {
        let count = Arc::new(AtomicU64::new(0));
        let mut sink = NoDriverSink { neutral_writes: count.clone() };
        let mut active = OutputState::neutral();
        active.buttons.set(XButton::A, true);
        let report = encode(&active);
        assert_eq!(validate_report(&report), Ok(()));
        assert_eq!(sink.send(&report).unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(count.load(Ordering::Relaxed), 0);
        sink.send(&encode(&OutputState::neutral())).unwrap();
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn authorized_syntax_still_cannot_submit_active_input_to_scm_preview() {
        let runtime = PreviewRuntime::start().unwrap();
        let mut report = encode(&OutputState::neutral());
        report[1] = 1; // valid HID button, forbidden in preview.
        assert_eq!(handle_dry_run(&encode_request(Command::Submit(report))),
            (Status::Disabled, false));
        assert_eq!(handle_dry_run(&encode_request(Command::Neutral)),
            (Status::Disabled, false));
        assert_eq!(handle_dry_run(&encode_request(Command::Ping)),
            (Status::Ok, false));
        assert_eq!(runtime.neutral_count(), 1);
        runtime.shutdown().unwrap();
    }

    #[test]
    fn preview_is_not_a_physical_device_transport() {
        assert_eq!(std::mem::size_of::<Report>(), REPORT_LEN);
        let runtime = PreviewRuntime::start().unwrap();
        runtime.shutdown().unwrap();
        // No CreateFile, DeviceIoControl, HID write, driver load or elevation
        // is called by this module or its no-driver transport.
    }
}
