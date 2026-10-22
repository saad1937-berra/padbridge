//! Fail-closed, machine-wide SID provisioning for the opt-in SCM preview.
//!
//! The service NEVER accepts an SID from argv, an IPC frame, or environment.
//! The registry key must be created by a trusted future administrator-only
//! installer with a protected ACL. Without it the service refuses to run.
//! No installer writes the key in this POC.

use std::{ffi::c_void, io, ptr};
use windows_sys::Win32::System::Registry::{
    RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
};
use super::broker_pipe_windows::is_numeric_sid;

const REGISTRY_SUBKEY: &str = r"SOFTWARE\PadBridge\BrokerPreview";
const REGISTRY_VALUE: &str = "AuthorizedClientSid";

fn wide_z(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn read_authorized_sid() -> io::Result<String> {
    let key = wide_z(REGISTRY_SUBKEY);
    let value = wide_z(REGISTRY_VALUE);
    let mut bytes = 0u32;

    // SAFETY: first call requests size of a REG_SZ, not its contents.
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE, key.as_ptr(), value.as_ptr(),
            RRF_RT_REG_SZ, ptr::null_mut(), ptr::null_mut(), &mut bytes,
        )
    };
    if result != 0 {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,
            format!("machine SID configuration unavailable (Win32 {result})")));
    }
    if !(8..=512).contains(&bytes) || bytes % 2 != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid machine SID size"));
    }

    let mut utf16 = vec![0u16; bytes as usize / 2];
    let mut received_bytes = bytes;
    // SAFETY: correctly aligned UTF-16 storage of exactly the requested
    // byte capacity, bounded to 512 bytes, expected REG_SZ only.
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE, key.as_ptr(), value.as_ptr(),
            RRF_RT_REG_SZ, ptr::null_mut(),
            utf16.as_mut_ptr().cast::<c_void>(), &mut received_bytes,
        )
    };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    if received_bytes > bytes || received_bytes % 2 != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "inconsistent machine SID size"));
    }
    let len = utf16.iter().position(|unit| *unit == 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unterminated SID"))?;
    if utf16[len..].iter().any(|unit| *unit != 0) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "embedded SID terminator"));
    }
    let sid = String::from_utf16(&utf16[..len])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "non UTF16 SID"))?;
    if !is_numeric_sid(&sid) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid provisioned SID"));
    }
    Ok(sid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_is_machine_scoped_not_a_user_supplied_path() {
        assert_eq!(REGISTRY_SUBKEY, r"SOFTWARE\PadBridge\BrokerPreview");
        assert_eq!(REGISTRY_VALUE, "AuthorizedClientSid");
    }
}
