//! Inert Windows Service Control Manager (SCM) lifecycle skeleton.
//!
//! Compiles for inspection only. No CreateService call, no installer,
//! no driver handle, no named-pipe accept loop and no automatic elevation.
//! The service does not process commands: RUNNING means "control loop alive",
//! not "PadBridge virtual controller available".

use std::{io, ptr, sync::atomic::{AtomicIsize, Ordering}};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, NO_ERROR, WAIT_OBJECT_0};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
    SERVICE_ACCEPT_STOP, SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
    SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOPPED, SERVICE_STOP_PENDING,
    SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, SetEvent, WaitForSingleObject, INFINITE,
};

pub const SERVICE_NAME: &str = "PadBridgeBrokerPreview";
static STOP_EVENT: AtomicIsize = AtomicIsize::new(0);

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
    // SAFETY: handle was returned by RegisterServiceCtrlHandlerExW.
    unsafe { SetServiceStatus(handle, &report); }
}

extern "system" fn control_handler(
    control: u32, _event_type: u32,
    _event_data: *mut core::ffi::c_void,
    _context: *mut core::ffi::c_void,
) -> u32 {
    if control == SERVICE_CONTROL_STOP {
        let event = STOP_EVENT.load(Ordering::Acquire);
        if event != 0 {
            // SAFETY: main thread keeps this event open until service stopped.
            unsafe { SetEvent(event as HANDLE); }
        }
    }
    NO_ERROR
}

extern "system" fn service_main(
    _argc: u32,
    _argv: *mut *mut u16,
) {
    let name = wide(SERVICE_NAME);
    // SAFETY: service dispatcher calls this entry; name is NUL terminated.
    let status_handle = unsafe {
        RegisterServiceCtrlHandlerExW(name.as_ptr(), Some(control_handler), ptr::null_mut())
    };
    if status_handle.is_null() {
        return;
    }
    status(status_handle, SERVICE_START_PENDING, 0);
    // Manual-reset event; initially unsignaled.
    // SAFETY: unnamed event created with default security.
    let stop_event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
    if stop_event.is_null() {
        status(status_handle, SERVICE_STOPPED, 1);
        return;
    }
    STOP_EVENT.store(stop_event as isize, Ordering::Release);

    // This preview intentionally never binds IPC or opens a device.
    // A future protected configuration and cancellable pipe worker are
    // prerequisites for activating the broker.
    status(status_handle, SERVICE_RUNNING, 0);
    // SAFETY: stop_event remains alive during the wait.
    let wait = unsafe { WaitForSingleObject(stop_event, INFINITE) };
    status(status_handle, SERVICE_STOP_PENDING, 0);
    STOP_EVENT.store(0, Ordering::Release);
    // SAFETY: we own this event and have stopped accepting commands.
    unsafe { CloseHandle(stop_event); }
    status(status_handle, SERVICE_STOPPED, if wait == WAIT_OBJECT_0 { 0 } else { 1 });
}

/// Called *only* by the explicitly feature-gated SCM binary, never Tauri.
/// Requires SCM invocation; does not install or launch a service by itself.
pub fn dispatch_inert_service() -> io::Result<()> {
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
    // SAFETY: table stays alive while SCM invokes service_main.
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
