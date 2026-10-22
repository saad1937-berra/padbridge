//! Opt-in, non-elevated health probe for an existing PadBridge broker.
//! Only PING is possible. No SCM installation, no service start, no driver,
//! no HID writes, no Bluetooth changes. Not launched from Tauri.

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    use padbridge_lib::virtual_pad::broker_client_windows::ping_existing_broker;
    use padbridge_lib::virtual_pad::broker_ipc::Status;
    use std::process::ExitCode;

    if std::env::args().collect::<Vec<_>>().as_slice().len() != 2
        || std::env::args().nth(1).as_deref() != Some("--ping-only")
    {
        eprintln!("Disabled by default. Use --ping-only only in the isolated broker test environment.");
        eprintln!("No service is installed or started and no device is opened.");
        return ExitCode::from(2);
    }

    match ping_existing_broker() {
        Ok(Status::Ok) => {
            println!("PadBridge broker: local PING accepted (IPC only; not XInput).");
            ExitCode::SUCCESS
        }
        Ok(status) => {
            eprintln!("PadBridge broker: PING refused ({status:?}).");
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("PadBridge broker: unavailable or invalid local response: {err}");
            ExitCode::from(1)
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("PadBridge broker probe is Windows-only.");
}
