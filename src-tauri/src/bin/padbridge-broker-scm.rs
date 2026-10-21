//! SCM IPC preview: dry-run only, gated by protected HKLM SID configuration.
//! Neither installed nor launched by PadBridge or CI.
//! Optionally binds restricted local IPC only under an externally provisioned
//! machine SID. NEVER accesses devices, injects HID or elevates Tauri.
#[cfg(windows)]
fn main() -> std::process::ExitCode {
    if std::env::args().len() != 1 {
        eprintln!("PadBridge SCM preview accepts no CLI configuration or arguments.");
        return std::process::ExitCode::from(2);
    }
    match padbridge_lib::virtual_pad::broker_scm_windows::dispatch_preview_service() {
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
