//! Explicit, read-only Windows client for the local broker POC.
//!
//! This code sends only a PING control frame and validates the response.
//! It cannot send SUBMIT/NEUTRAL, install a service, access a device,
//! elevate the caller, or load a driver. It is NOT used by Tauri.

use std::{io, mem, ptr};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, ERROR_IO_PENDING, INVALID_HANDLE_VALUE,
    WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::WaitNamedPipeW;
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use super::broker_ipc::{self, Command, Status, RESPONSE_LEN};
use super::broker_pipe_windows::PIPE_NAME;

/// Timeout per overlapped read/write. The service has its own bounded I/O.
const CLIENT_IO_TIMEOUT_MS: u32 = 1500;
const PIPE_CONNECT_TIMEOUT_MS: u32 = 1000;
// FILE_READ_DATA | FILE_WRITE_DATA | SYNCHRONIZE: no pipe creation right.
// Do NOT request GENERIC_WRITE: it includes FILE_APPEND_DATA, which aliases
// FILE_CREATE_PIPE_INSTANCE for named pipes.
const MINIMUM_PIPE_CLIENT_ACCESS: u32 = 0x0010_0003;

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0); }
    }
}

/// Completes or drains pending I/O before freeing an OVERLAPPED buffer.
fn exchange_io(handle: HANDLE, buf: &mut [u8], writing: bool) -> io::Result<usize> {
    let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
    if event.is_null() {
        return Err(io::Error::last_os_error());
    }
    let event = OwnedHandle(event);
    let mut overlapped: OVERLAPPED = unsafe { mem::zeroed() };
    overlapped.hEvent = event.0;
    let mut count = 0u32;
    let len = u32::try_from(buf.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "IPC buffer too large"))?;

    let ok = if writing {
        unsafe { WriteFile(handle, buf.as_ptr().cast(), len, &mut count, &mut overlapped) }
    } else {
        unsafe { ReadFile(handle, buf.as_mut_ptr().cast(), len, &mut count, &mut overlapped) }
    };
    if ok != 0 {
        return Ok(count as usize);
    }
    let err = unsafe { GetLastError() };
    if err != ERROR_IO_PENDING {
        return Err(io::Error::from_raw_os_error(err as i32));
    }

    let result = unsafe { WaitForSingleObject(event.0, CLIENT_IO_TIMEOUT_MS) };
    if result == WAIT_OBJECT_0 {
        if unsafe { GetOverlappedResult(handle, &overlapped, &mut count, 0) } == 0 {
            return Err(io::Error::last_os_error());
        }
        return Ok(count as usize);
    }

    // Regardless of timeout or wait failure, the outstanding operation must
    // be cancelled and drained before its stack buffers are destroyed.
    let wait_error = if result == WAIT_TIMEOUT {
        io::Error::new(io::ErrorKind::TimedOut, "PadBridge broker probe timed out")
    } else {
        io::Error::last_os_error()
    };
    unsafe {
        CancelIoEx(handle, &overlapped);
        let mut drained = 0u32;
        GetOverlappedResult(handle, &overlapped, &mut drained, 1);
    }
    Err(wait_error)
}

/// Sends a single PING to an already-running local broker preview.
///
/// No path or command argument is accepted; the only target is the constant
/// local PadBridge pipe. The server checks Windows token identity separately.
pub fn ping_existing_broker() -> io::Result<Status> {
    let pipe = wide_z(PIPE_NAME);
    if unsafe { WaitNamedPipeW(pipe.as_ptr(), PIPE_CONNECT_TIMEOUT_MS) } == 0 {
        return Err(io::Error::last_os_error());
    }

    let raw = unsafe {
        CreateFileW(
            pipe.as_ptr(), MINIMUM_PIPE_CLIENT_ACCESS, 0, ptr::null_mut(),
            OPEN_EXISTING, FILE_FLAG_OVERLAPPED, ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let handle = OwnedHandle(raw);

    // The one-shot POC server disconnects after a single message.
    let mut request = broker_ipc::encode_request(Command::Ping);
    if exchange_io(handle.0, &mut request, true)? != request.len() {
        return Err(io::Error::new(io::ErrorKind::WriteZero, "partial PING frame"));
    }
    let mut reply = [0u8; RESPONSE_LEN];
    if exchange_io(handle.0, &mut reply, false)? != reply.len() {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "partial PING response"));
    }
    broker_ipc::decode_response(&reply)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData,
            format!("invalid broker IPC response: {error:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::broker_pipe_windows::PreviewPipe;
    use std::{ffi::c_void, sync::mpsc, thread, time::Duration};
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_INSUFFICIENT_BUFFER};
    use windows_sys::Win32::Security::{
        GetTokenInformation, OpenProcessToken, TokenUser, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    fn current_user_sid() -> io::Result<String> {
        let mut raw_token = ptr::null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle(raw_token);
        let mut size = 0u32;
        let probe = unsafe {
            GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut size)
        };
        if probe != 0 || io::Error::last_os_error().raw_os_error()
            != Some(ERROR_INSUFFICIENT_BUFFER as i32) || size == 0 || size > 65536
        {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid token size"));
        }
        let mut storage = vec![0u64; (size as usize).div_ceil(8)];
        if unsafe {
            GetTokenInformation(
                token.0, TokenUser, storage.as_mut_ptr().cast(), size, &mut size
            )
        } == 0 {
            return Err(io::Error::last_os_error());
        }
        let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
        if user.User.Sid.is_null() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "missing token SID"));
        }
        let mut sid_ptr = ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid_ptr) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if sid_ptr.is_null() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "empty SID"));
        }
        let len = (0..184).find(|&n| unsafe { *sid_ptr.add(n) } == 0);
        let sid = if let Some(len) = len {
            String::from_utf16(unsafe { std::slice::from_raw_parts(sid_ptr, len) })
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid SID encoding"))
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidData, "oversized SID"))
        };
        unsafe { LocalFree(sid_ptr.cast::<c_void>()); }
        sid
    }

    #[test]
    fn authenticated_ping_round_trip_uses_real_local_windows_pipe() {
        let _serial = super::super::broker_pipe_windows::PIPE_TEST_MUTEX.lock().unwrap();
        // Runs on the disposable Windows CI runner, NEVER on a user's PC.
        // Does not touch a device, Bluetooth, SCM or kernel-mode driver.
        let sid = current_user_sid().expect("runner token");
        let (ready, received) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || -> io::Result<()> {
            let mut server = PreviewPipe::bind_for_sid(&sid)?;
            ready.send(()).map_err(|_| io::Error::other("receiver gone"))?;
            server.serve_one_message()
        });
        received.recv_timeout(Duration::from_secs(4)).expect("preview pipe ready");
        assert_eq!(ping_existing_broker().expect("local broker PING"), Status::Ok);
        worker.join().expect("preview thread").expect("server completed");
    }
}
