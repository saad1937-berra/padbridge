//! Opt-in, one-shot PadBridge broker *preview*, NOT an installed Windows service.
//! Does not load/open a driver, does not escalate privileges, and rejects
//! all report submission commands. This is for future isolated VM review.

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    use padbridge_lib::virtual_pad::broker_pipe_windows::PreviewPipe;
    use std::process::ExitCode;

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 || args[1] != "--preview-one-message-for-sid" {
        eprintln!("PadBridge broker preview is inactive by default.");
        eprintln!("No service is installed, no driver is opened.");
        eprintln!("This binary is a code-only, one-message IPC experiment.");
        return ExitCode::from(2);
    }
    // The SID is a test-only argument, NOT a secure provisioning mechanism.
    // A future SCM installer must read an admin-controlled, integrity-protected
    // SID and enforce an authenticated per-user session policy.
    let mut endpoint = match PreviewPipe::bind_for_sid(&args[2]) {
        Ok(pipe) => pipe,
        Err(error) => {
            eprintln!("Local test pipe was not created: {error}");
            return ExitCode::from(1);
        }
    };
    // WARNING: blocking, no cancellation or timeout. Not SCM service-ready.
    // The handling policy answers only PING/CLOSE; ALL input injection denied.
    match endpoint.serve_one_message() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Local test IPC ended without a response: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("PadBridge broker preview supports Windows only.");
}
