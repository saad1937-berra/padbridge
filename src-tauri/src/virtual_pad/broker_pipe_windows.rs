//! Deliberately dormant Windows named-pipe endpoint for PadBridge broker POC.
//!
//! Uses a *Windows token ACL*, not a shared password. Only SYSTEM,
//! Administrators, and the explicitly provisioned SID can access it.
//! The allowed user gets READ/WRITE DATA + SYNCHRONIZE, **not**
//! FILE_CREATE_PIPE_INSTANCE, which is aliased to FILE_APPEND_DATA.
//!
//! No SCM installation or driver IOCTL; all input commands are handled by
//! the dry-run policy, which denies even a well-formed non-neutral report.
//! A synchronous one-shot proof-of-concept only; do not deploy it as a
//! production Windows service (there are no timeouts or stop callbacks).

use std::io;
use std::mem::size_of;
use std::ptr;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, HANDLE, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::{SECURITY_ATTRIBUTES};
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Storage::FileSystem::{
    ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
    PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_WAIT,
};

use super::broker_ipc::{self, REQUEST_LEN};

pub const PIPE_NAME: &str = r"\\.\pipe\PadBridge.Broker.POC.v1";
// Win32 CreateNamedPipeW open-mode flag, PIPE_ACCESS_DUPLEX.
const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;

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

pub struct PreviewPipe {
    handle: HANDLE,
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
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
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
        Ok(Self { handle })
    }

    /// Handle precisely ONE bounded client message, then disconnect.
    /// Blocking: not suitable for SCM until cancellation + timeouts exist.
    /// No command can reach DeviceIoControl in this implementation.
    pub fn serve_one_message(&mut self) -> io::Result<()> {
        // SAFETY: valid server pipe handle. ERROR_PIPE_CONNECTED is allowed
        // if the client connected between CreateNamedPipe and ConnectNamedPipe.
        let connected = unsafe { ConnectNamedPipe(self.handle, ptr::null_mut()) };
        if connected == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
            return Err(io::Error::last_os_error());
        }

        // Guarantee the pipe is disconnected even if the request is invalid.
        let result = self.read_and_respond();
        // SAFETY: owned pipe server handle.
        unsafe { DisconnectNamedPipe(self.handle); }
        result
    }

    fn read_and_respond(&mut self) -> io::Result<()> {
        let mut request = [0u8; REQUEST_LEN];
        let mut bytes_read = 0u32;
        // SAFETY: fixed-size writable report buffer, synchronous API.
        let read_ok = unsafe {
            ReadFile(self.handle, request.as_mut_ptr().cast(),
                REQUEST_LEN as u32, &mut bytes_read, ptr::null_mut())
        };
        if read_ok == 0 {
            return Err(io::Error::last_os_error());
        }

        let (status, _close) = broker_ipc::handle_dry_run(&request[..bytes_read as usize]);
        let response = broker_ipc::response(status);
        let mut bytes_written = 0u32;
        // SAFETY: response remains alive throughout this synchronous write.
        let write_ok = unsafe {
            WriteFile(self.handle, response.as_ptr().cast(),
                response.len() as u32, &mut bytes_written, ptr::null_mut())
        };
        if write_ok == 0 {
            return Err(io::Error::last_os_error());
        }
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
