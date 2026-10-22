//! Explicitly invoked, read-only XInput 1.4 slot snapshot.
//! No installation, controller input injection, rumble, service or HID access.

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    use padbridge_lib::virtual_pad::xinput_probe_windows::{snapshot, SlotStatus};

    if std::env::args().len() != 2
        || std::env::args().nth(1).as_deref() != Some("--snapshot")
    {
        eprintln!("PadBridge XInput probe: opt-in only; pass --snapshot to read the four slots once.");
        eprintln!("Not connected to the Tauri app, service or experimental driver.");
        return std::process::ExitCode::from(2);
    }
    let slots = snapshot();
    let any_error = slots.iter().any(|slot|
        matches!(slot.state, SlotStatus::Error { .. })
    );
    match serde_json::to_string_pretty(&slots) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("Failed to serialize XInput slot snapshot: {err}");
            return std::process::ExitCode::from(1);
        }
    }
    eprintln!("WARNING: XInput slot numbers do not identify a PadBridge device.");
    eprintln!("Compare against an empty VM baseline; a generic VHF HID device is not automatically XInput.");
    if any_error {
        std::process::ExitCode::from(1)
    } else {
        std::process::ExitCode::SUCCESS
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The XInput 1.4 snapshot probe is available on Windows only.");
}
