//! Deliberately dormant Windows named-pipe endpoint for PadBridge broker POC.
//!
//! Uses a *Windows token ACL*, not a shared password. Only SYSTEM,
//! Administrators, and the explicitly provisioned SID can access it.
//! The allowed user gets READ/WRITE DATA + SYNCHRONIZE, **not**
//! FILE_CREATE_PIPE_INSTANCE, which is aliased to FILE_APPEND_DATA.
//!
//! No SCM installation or driver IOCTL; all input commands are handled by
//! the dry-run policy, which denies even a well-formed non-neutral report.
//! An overlapped one-shot proof-of-concept with cancellation/timeouts; do
//! not deploy it as a Windows service (no SCM integration or STOP callback).

use std::io;
use std::mem::size_of;
use std::ptr;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, HANDLE, ERROR_PIPE_CONNECTED, ERROR_IO_PENDING,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::{SECURITY_ATTRIBUTES};
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Storage::FileSystem::{
    ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
    PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_WAIT,
};

use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use super::broker_ipc::{self, REQUEST_LEN};
use super::broker_auth_windows::verify_last_sender;

pub const PIPE_NAME: &str = r"\\.\pipe\PadBridge.Broker.POC.v1";
// Win32 CreateNamedPipeW open-mode flag, PIPE_ACCESS_DUPLEX.
const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
/// Per-operation bound. The preview is not a production service.
pub const IO_TIMEOUT_MS: u32 = 2500;

// A user's SID may come ONLY from future privileged provisioning.
// No names, wildcards, SID aliases or SDDL escape sequences are accepted.
pub fn is_numeric_sid(sid: &str) -> bool {
    if sid.len() < 7 || sid.len() > 184 || !sid.starts_with("S-1-") {
        return false;
    }
    let mut pieces = sid.split('-');
    pieces.next() == Some("S")
        && pieces.next() == Some("1")
        && pieces.clone().count() >= 2
        && pieces.all(|part| !part.is_empty() && part.len() <= 20
            && part.as_bytes().iter().all(u8::is_ascii_digit))
}

fn utf16_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

struct Descriptor(*mut core::ffi::c_void);

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: ConvertStringSecurityDescriptorToSecurityDescriptorW allocates
        // using LocalAlloc; LocalFree is the corresponding deallocator.
        unsafe { LocalFree(self.0); }
    }
}

struct IoEvent(HANDLE);
impl Drop for IoEvent {
    fn drop(&mut self) {
        // SAFETY: owns the CreateEventW handle; no OVERLAPPED is pending.
        unsafe { CloseHandle(self.0); }
    }
}

pub struct PreviewPipe {
    handle: HANDLE,
    provisioned_user_sid: String,
}

impl PreviewPipe {
    /// Only a strictly formatted SID and the system/admin SIDs can connect.
    ///
    /// Unlike GR/GW, 0x00120083 does not grant FILE_CREATE_PIPE_INSTANCE.
    /// D:P = protected DACL; no access for Everyone, anonymous or network.
    pub fn bind_for_sid(provisioned_user_sid: &str) -> io::Result<Self> {
        if !is_numeric_sid(provisioned_user_sid) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                "an explicitly provisioned numeric user SID is required"));
        }
        let sddl = format!(
            "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x00120083;;;{provisioned_user_sid})");
        let wide_acl = utf16_z(&sddl);
        let mut raw_descriptor = ptr::null_mut();

        // SAFETY: valid NUL-terminated SDDL input, allocated descriptor returned.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide_acl.as_ptr(), 1, &mut raw_descriptor, ptr::null_mut())
        };
        if converted == 0 {
            return Err(io::Error::last_os_error());
        }
        let descriptor = Descriptor(raw_descriptor);
        let mut security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };

        let pipe_name = utf16_z(PIPE_NAME);
        // SAFETY: Windows copies the security descriptor at pipe creation;
        // only this one server instance is allowed and remote clients are
        // rejected by the named-pipe subsystem.
        let handle = unsafe {
            CreateNamedPipeW(
                pipe_name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT |
                    PIPE_REJECT_REMOTE_CLIENTS,
                1,
                broker_ipc::RESPONSE_LEN as u32,
                REQUEST_LEN as u32,
                0,
                &mut security,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle, provisioned_user_sid: provisioned_user_sid.to_owned() })
    }

    /// Handle ONE client within bounded wait times, then disconnect.
    /// Still not SCM-ready; no cancellation event from the service manager.
    pub fn serve_one_message(&mut self) -> io::Result<()> {
        // A client may have connected before this overlapped ConnectNamedPipe.
        self.bounded_io(true, |overlapped, _bytes| unsafe {
            ConnectNamedPipe(self.handle, overlapped)
        })?;
        let result = self.read_and_respond();
        // SAFETY: all overlapped requests have been drained before disconnect.
        unsafe { DisconnectNamedPipe(self.handle); }
        result
    }

    fn bounded_io(
        &self,
        allow_already_connected: bool,
        action: impl FnOnce(*mut OVERLAPPED, *mut u32) -> i32,
    ) -> io::Result<u32> {
        // Manual reset event, unique to this single in-flight request.
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        let event = IoEvent(event);
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = event.0;
        let mut count = 0u32;
        let ok = action(&mut overlapped, &mut count);
        if ok != 0 {
            return Ok(count);
        }
        // SAFETY: GetLastError immediately follows the failed I/O operation.
        let error = unsafe { GetLastError() };
        if allow_already_connected && error == ERROR_PIPE_CONNECTED {
            return Ok(0);
        }
        if error != ERROR_IO_PENDING {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        // The operation is pending; timeout triggers CancelIoEx. Draining
        // completion before dropping OVERLAPPED + event avoids a UAF race.
        let wait = unsafe { WaitForSingleObject(event.0, IO_TIMEOUT_MS) };
        if wait != WAIT_OBJECT_0 {
            unsafe {
                CancelIoEx(self.handle, &overlapped);
                let mut drained = 0u32;
                GetOverlappedResult(self.handle, &overlapped, &mut drained, 1);
            }
            return Err(io::Error::new(io::ErrorKind::TimedOut,
                "PadBridge IPC operation timed out or was cancelled"));
        }
        if unsafe {
            GetOverlappedResult(self.handle, &overlapped, &mut count, 0)
        } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(count)
    }

    fn read_and_respond(&mut self) -> io::Result<()> {
        let mut request = [0u8; REQUEST_LEN];
        let bytes_read = self.bounded_io(false, |overlapped, count| unsafe {
            ReadFile(self.handle, request.as_mut_ptr().cast(),
                REQUEST_LEN as u32, count, overlapped)
        })?;

        // Never authorize solely based on the pipe DACL or a self-reported frame.
        // Identity is checked AFTER reading the last client message.
        let status = if matches!(verify_last_sender(self.handle, &self.provisioned_user_sid), Ok(true)) {
            broker_ipc::handle_dry_run(&request[..bytes_read as usize]).0
        } else {
            broker_ipc::Status::Unauthorized
        };
        let response = broker_ipc::response(status);
        let bytes_written = self.bounded_io(false, |overlapped, count| unsafe {
            WriteFile(self.handle, response.as_ptr().cast(),
                response.len() as u32, count, overlapped)
        })?;
        if bytes_written != response.len() as u32 {
            return Err(io::Error::new(io::ErrorKind::WriteZero,
                "short local IPC response"));
        }
        Ok(())
    }
}

impl Drop for PreviewPipe {
    fn drop(&mut self) {
        // SAFETY: owned CreateNamedPipeW handle; no driver objects involved.
        unsafe { CloseHandle(self.handle); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_numeric_user_sids_are_allowed_in_sddl() {
        assert!(is_numeric_sid("S-1-5-21-123456789-987654321-123456789-1001"));
        assert!(!is_numeric_sid("SY"));
        assert!(!is_numeric_sid("S-1-5-21-1)(A;;GA;;;WD)"));
        assert!(!is_numeric_sid("S-1-5-21-"));
        assert!(!is_numeric_sid("S-1-5-21-123 456"));
    }

    #[test]
    fn known_pipe_name_is_constant_and_local() {
        assert_eq!(PIPE_NAME, r"\\.\pipe\PadBridge.Broker.POC.v1");
    }
}
