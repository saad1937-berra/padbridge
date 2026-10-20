//! Inert SCM lifecycle test binary. Not installed by PadBridge or the CI.
//! Does NOT bind IPC, access devices, inject HID or elevate privileges.
#[cfg(windows)]
fn main() -> std::process::ExitCode {
    if std::env::args().len() != 1 {
        eprintln!("PadBridge SCM preview accepts no interactive arguments.");
        return std::process::ExitCode::from(2);
    }
    match padbridge_lib::virtual_pad::broker_scm_windows::dispatch_inert_service() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("SCM preview not started (expected outside SCM): {e}");
            std::process::ExitCode::from(1)
        }
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only.");
}
