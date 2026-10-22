//! Windows SCM preview: STOP-cancellable, dry-run local IPC only.
//!
//! Opt-in compilation; no CreateService, no INF, no kernel IOCTL or auto-start.
//! Starts a neutral-only in-memory worker; never forwards authenticated IPC input
//! to it or to the experimental driver.
//! Missing admin-provisioned SID in HKLM => fail closed before opening a pipe.
//! The window/Tauri frontend always retains NullBackend.
//! Despite SCM integration this is NOT a production-deployable service:
//! pipe ACL/configuration and stop races need isolated Windows VM testing.

use std::{io, ptr, sync::atomic::{AtomicBool, Ordering}};
use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
    SERVICE_ACCEPT_STOP, SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
    SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOPPED, SERVICE_STOP_PENDING,
    SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
};

use super::broker_config_windows::read_authorized_sid;
use super::broker_pipe_windows::PreviewPipe;
use super::broker_preview_runtime::PreviewRuntime;

pub const SERVICE_NAME: &str = "PadBridgeBrokerPreview";
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn status(handle: SERVICE_STATUS_HANDLE, state: u32, win32_code: u32) {
    let controls = if state == SERVICE_RUNNING { SERVICE_ACCEPT_STOP } else { 0 };
    let report = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: controls,
        dwWin32ExitCode: win32_code,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: if state == SERVICE_START_PENDING || state == SERVICE_STOP_PENDING {
            3000
        } else {
            0
        },
    };
    // SAFETY: handle is from SCM; failures are verified during real VM tests.
    unsafe { SetServiceStatus(handle, &report); }
}

extern "system" fn control_handler(
    control: u32, _event_type: u32,
    _event_data: *mut core::ffi::c_void,
    _context: *mut core::ffi::c_void,
) -> u32 {
    if control == SERVICE_CONTROL_STOP {
        // Only a lock-free signal: no raw event handle and no Win32 calls from
        // callback, avoiding callback-vs-teardown use-after-close races.
        STOP_REQUESTED.store(true, Ordering::Release);
    }
    NO_ERROR
}

extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    STOP_REQUESTED.store(false, Ordering::Release);
    let name = wide(SERVICE_NAME);
    // SAFETY: invoked by the Windows service dispatcher.
    let status_handle = unsafe {
        RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control_handler), ptr::null_mut())
    };
    if status_handle.is_null() {
        return;
    }
    status(status_handle, SERVICE_START_PENDING, 0);

    // If config fails, never report RUNNING or expose a privileged pipe.
    let sid = read_authorized_sid();
    let error_code = match sid {
        Err(error) => error.raw_os_error().unwrap_or(5) as u32,
        Ok(sid) => {
            if STOP_REQUESTED.load(Ordering::Acquire) {
                0
            } else {
                match PreviewPipe::bind_for_sid(&sid) {
                    Err(error) => error.raw_os_error().unwrap_or(5) as u32,
                    Ok(mut pipe) => {
                        // Fail closed: if the autonomous preview worker cannot
                        // start, SCM never reports RUNNING or accepts IPC.
                        match PreviewRuntime::start() {
                            Err(error) => error.raw_os_error().unwrap_or(1) as u32,
                            Ok(runtime) => {
                                // The runtime owns NO device handle and has NO
                                // producer exposed: the pipe keeps rejecting
                                // SUBMIT / NEUTRAL in its dry-run policy.
                                if !STOP_REQUESTED.load(Ordering::Acquire) {
                                    status(status_handle, SERVICE_RUNNING, 0);
                                }
                                let mut failure = 0;
                                while !STOP_REQUESTED.load(Ordering::Acquire) {
                                    match pipe.serve_one_message_until(&STOP_REQUESTED) {
                                        Ok(()) => {}
                                        Err(ref error) if error.kind() == io::ErrorKind::TimedOut => {}
                                        Err(ref error) if error.kind() == io::ErrorKind::Interrupted
                                            && STOP_REQUESTED.load(Ordering::Acquire) => break,
                                        Err(error) => {
                                            failure = error.raw_os_error().unwrap_or(1) as u32;
                                            break;
                                        }
                                    }
                                }
                                // No driver write is possible here: two neutral
                                // frames are counted in RAM (start and STOP).
                                // Surface worker failure instead of hiding it.
                                if let Err(error) = runtime.shutdown() {
                                    if failure == 0 {
                                        failure = error.raw_os_error().unwrap_or(1) as u32;
                                    }
                                }
                                failure
                            }
                        }
                    }
                }
            }
        }
    };
    status(status_handle, SERVICE_STOP_PENDING, 0);
    status(status_handle, SERVICE_STOPPED, error_code);
}

/// Dispatch through SCM only: no installation, config writer or elevation.
/// Calling this executable interactively returns the usual SCM-connect error.
pub fn dispatch_preview_service() -> io::Result<()> {
    let name = wide(SERVICE_NAME);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: name.as_ptr() as *mut u16,
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: ptr::null_mut(),
            lpServiceProc: None,
        },
    ];
    // SAFETY: dispatcher executes callback before returning.
    if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_name_is_padbridge_specific() {
        assert_eq!(SERVICE_NAME, "PadBridgeBrokerPreview");
        assert_ne!(SERVICE_NAME, "PlugPlay");
    }
}
