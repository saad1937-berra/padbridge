//! Opt-in prototype of an autonomous broker heartbeat executor.
//!
//! Pure Rust, memory-backed in CI. It is NOT a Windows service and does not
//! open an HID device, start a driver, elevate, or expose an IPC endpoint.
//! Nothing in Tauri/selected_backend.rs constructs this executor.
//!
//! Queue operations mean "accepted for processing", NOT "delivered to HID".
//! Future privileged callers must provide a bounded synchronous transport,
//! authenticated producer, disconnect notification and verified stop behavior.

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::backend::OutputState;
use super::vhf_broker::{BrokerSession, Report, ReportTransport, HEARTBEAT_INTERVAL, INPUT_LEASE};
use super::vhf_protocol::{encode, validate_report};

/// Ticks four times per heartbeat. The kernel's independent watchdog still
/// protects against broker crash, I/O stalls or scheduler starvation.
pub const WORKER_POLL: Duration = Duration::from_millis(50);
const MAX_QUEUED_INPUTS: usize = 8;

struct FreshInput {
    report: Report,
    observed_at: Instant, // Measured on arrival, not when the worker dequeues.
}

/// Privileged broker executor preview. A future service must NOT use it until
/// its real I/O transport is verified to terminate promptly on cancellation.
pub struct BrokerWorker {
    sender: Option<SyncSender<FreshInput>>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl BrokerWorker {
    /// Sends the initial neutral report synchronously and starts an idle timer.
    pub fn start<T: ReportTransport + Send + 'static>(transport: T) -> io::Result<Self> {
        let session = BrokerSession::begin(transport)?;
        let (tx, rx) = mpsc::sync_channel(MAX_QUEUED_INPUTS);
        let thread = thread::Builder::new()
            .name("padbridge-broker-preview-tick".into())
            .spawn(move || run_loop(session, rx))?;
        Ok(Self { sender: Some(tx), thread: Some(thread) })
    }

    /// Enqueues fresh state. This confirms acceptance only, not successful
    /// delivery or acknowledgement by the driver. Never waits on a full queue.
    pub fn queue_fresh(&self, state: &OutputState) -> io::Result<()> {
        let report = encode(state);
        validate_report(&report)
            .map_err(|reason| io::Error::new(io::ErrorKind::InvalidInput, reason))?;
        let sender = self.sender.as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "worker already shut down"))?;
        sender.try_send(FreshInput { report, observed_at: Instant::now() })
            .map_err(|error| match error {
                TrySendError::Full(_) => io::Error::new(
                    io::ErrorKind::WouldBlock, "broker input queue full; report discarded"
                ),
                TrySendError::Disconnected(_) => io::Error::new(
                    io::ErrorKind::BrokenPipe, "broker worker no longer running"
                ),
            })
    }

    /// Close the input channel and wait for the worker to release controls.
    /// The transport must itself guarantee bounded I/O.
    pub fn shutdown(mut self) -> io::Result<()> {
        self.stop_inner()
    }

    fn stop_inner(&mut self) -> io::Result<()> {
        self.sender.take(); // closing producer wakes recv_timeout
        if let Some(join) = self.thread.take() {
            join.join().map_err(|_| io::Error::other("broker worker panicked"))?
        } else {
            Ok(())
        }
    }
}

impl Drop for BrokerWorker {
    fn drop(&mut self) {
        let _ = self.stop_inner();
    }
}

fn run_loop<T: ReportTransport>(
    mut session: BrokerSession<T>, rx: Receiver<FreshInput>
) -> io::Result<()> {
    loop {
        // Never allow a steady stream of queued reports to starve the lease
        // check. Expiration always wins over a late-arriving input packet.
        session.heartbeat()?;
        match rx.recv_timeout(WORKER_POLL) {
            Ok(message) => {
                // Reject a message that waited too long in the bounded queue.
                // Its original arrival time must remain authoritative.
                if message.observed_at.elapsed() >= INPUT_LEASE {
                    let _ = session.close();
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut, "queued controller input expired"
                    ));
                }
                session.send_observed(&message.report, message.observed_at)?;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return session.close(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_pad::backend::XButton;
    use std::sync::{Arc, Mutex};

    struct MemoryTransport(Arc<Mutex<Vec<Report>>>);
    impl ReportTransport for MemoryTransport {
        fn send(&mut self, report: &Report) -> io::Result<()> {
            self.0.lock().expect("memory sink poisoned").push(*report);
            Ok(())
        }
    }
    fn test_worker() -> (BrokerWorker, Arc<Mutex<Vec<Report>>>) {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let worker = BrokerWorker::start(MemoryTransport(frames.clone())).unwrap();
        (worker, frames)
    }

    fn until_frames(frames: &Arc<Mutex<Vec<Report>>>, wanted: usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if frames.lock().unwrap().len() >= wanted { return; }
            assert!(Instant::now() < deadline, "worker made no progress");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn automatic_tick_repeats_only_active_input_then_shutdown_neutralizes() {
        let (worker, frames) = test_worker();
        let mut pressed = OutputState::neutral();
        pressed.buttons.set(XButton::A, true);
        worker.queue_fresh(&pressed).unwrap();
        until_frames(&frames, 2);
        // The worker independently calls heartbeat, with no manual test tick.
        until_frames(&frames, 3);
        assert_eq!(frames.lock().unwrap()[2][1] & 1, 1);
        worker.shutdown().unwrap();
        let sent = frames.lock().unwrap();
        assert_eq!(sent.first().copied(), Some(encode(&OutputState::neutral())));
        assert_eq!(sent.last().copied(), Some(encode(&OutputState::neutral())));
    }

    #[test]
    fn disconnected_producer_releases_held_button_without_manual_tick() {
        let (worker, frames) = test_worker();
        let mut pressed = OutputState::neutral();
        pressed.buttons.set(XButton::B, true);
        worker.queue_fresh(&pressed).unwrap();
        until_frames(&frames, 2);
        drop(worker);
        assert_eq!(frames.lock().unwrap().last().copied(), Some(encode(&OutputState::neutral())));
    }

    #[test]
    fn stale_input_is_closed_by_automatic_lease_check() {
        let (worker, frames) = test_worker();
        let mut pressed = OutputState::neutral();
        pressed.buttons.set(XButton::A, true);
        worker.queue_fresh(&pressed).unwrap();
        until_frames(&frames, 2);
        // No fresh packets. Independent 50ms tick detects 600ms lease expiry.
        assert_eq!(worker.shutdown_after_lease_for_test().unwrap_err().kind(),
                   io::ErrorKind::TimedOut);
        assert_eq!(frames.lock().unwrap().last().copied(), Some(encode(&OutputState::neutral())));
    }

    #[test]
    fn no_active_input_needs_no_heartbeat_write() {
        let (worker, frames) = test_worker();
        thread::sleep(Duration::from_millis(120));
        worker.shutdown().unwrap();
        assert_eq!(frames.lock().unwrap().len(), 2, "initial and shutdown neutral only");
    }

    impl BrokerWorker {
        // Tests may wait for the internal 600ms lease without an extra command.
        fn shutdown_after_lease_for_test(mut self) -> io::Result<()> {
            thread::sleep(INPUT_LEASE + WORKER_POLL + Duration::from_millis(60));
            self.stop_inner()
        }
    }
}
