//! Explicit named-pipe client identity verification (Windows, no driver I/O).
//!
//! ReadFile must have succeeded before this check, so impersonation binds to
//! the client who sent the message. The pipe DACL is an additional boundary:
//! we check the client's actual TokenUser SID, never an identity in the frame.

use std::{ffi::c_void, io, mem::size_of, ptr};
use windows_sys::Win32::Foundation::{
    CloseHandle, LocalFree, HANDLE, ERROR_INSUFFICIENT_BUFFER,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, RevertToSelf, TokenUser, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::System::Pipes::ImpersonateNamedPipeClient;
use windows_sys::Win32::System::Threading::{GetCurrentThread, OpenThreadToken};

use super::broker_pipe_windows::is_numeric_sid;

struct ImpersonationGuard;
impl Drop for ImpersonationGuard {
    fn drop(&mut self) {
        // Critical: continuing as an impersonated user is not acceptable.
        // A failed revert is fatal to this privileged process.
        if unsafe { RevertToSelf() } == 0 {
            std::process::abort();
        }
    }
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0); }
    }
}

struct LocalWide(*mut u16);
impl Drop for LocalWide {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0.cast::<c_void>()); }
    }
}

/// Requires a successful request read on this pipe immediately beforehand.
/// Impersonation failure never authorizes an administrative caller.
pub fn verify_last_sender(pipe: HANDLE, expected_sid: &str) -> io::Result<bool> {
    if !is_numeric_sid(expected_sid) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid configured SID"));
    }
    // SAFETY: only the server end calls this after a successful ReadFile.
    if unsafe { ImpersonateNamedPipeClient(pipe) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _revert = ImpersonationGuard;
    let mut raw_token = ptr::null_mut();
    // SAFETY: GetCurrentThread is a pseudo-handle; opening the thread's
    // *impersonation* token does not fall back to the privileged process token.
    if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut raw_token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = OwnedHandle(raw_token);

    let mut size = 0u32;
    // SAFETY: first call queries the required bytes, not the SID itself.
    let query = unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut size)
    };
    if query != 0 || io::Error::last_os_error().raw_os_error()
        != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected token query result"));
    }
    if size < size_of::<TOKEN_USER>() as u32 || size > 65536 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid token user size"));
    }
    // TokenUser must be suitably aligned for TOKEN_USER and its SID fields.
    let mut storage = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: buffer size bounded and memory aligned for TOKEN_USER.
    if unsafe {
        GetTokenInformation(token.0, TokenUser,
            storage.as_mut_ptr().cast(), size, &mut size)
    } == 0 {
        return Err(io::Error::last_os_error());
    }
    let user = unsafe { &*(storage.as_ptr().cast::<TOKEN_USER>()) };
    if user.User.Sid.is_null() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "missing token SID"));
    }
    let mut converted = ptr::null_mut();
    // SAFETY: SID pointer is owned by TokenUser storage until conversion ends.
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut converted) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let sid = LocalWide(converted);
    // SID strings are small. Never scan an unbounded UTF-16 pointer.
    let len = (0..184).find(|&n| unsafe { *sid.0.add(n) } == 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "oversized token SID"))?;
    let actual = String::from_utf16(
        unsafe { std::slice::from_raw_parts(sid.0, len) }
    ).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad token SID"))?;
    Ok(actual.eq_ignore_ascii_case(expected_sid))
}
