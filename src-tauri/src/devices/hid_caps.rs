//! Lecture seule des capacités HID Windows. Aucune commande envoyée à la manette.
//! API Windows natives : HidD_GetPreparsedData / HidP_GetCaps.
//! Ne jamais retourner le chemin HID, un identifiant série ou le MAC dans un diagnostic.

#![cfg(windows)]

use std::ffi::c_void;
use std::iter::once;
use std::ptr::{null, null_mut};

type Handle = *mut c_void;
const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const OPEN_EXISTING: u32 = 3;

#[repr(C)]
#[derive(Default)]
#[allow(non_snake_case)]
struct HidpCaps {
    Usage: u16,
    UsagePage: u16,
    InputReportByteLength: u16,
    OutputReportByteLength: u16,
    FeatureReportByteLength: u16,
    Reserved: [u16; 17],
    NumberLinkCollectionNodes: u16,
    NumberInputButtonCaps: u16,
    NumberInputValueCaps: u16,
    NumberInputDataIndices: u16,
    NumberOutputButtonCaps: u16,
    NumberOutputValueCaps: u16,
    NumberOutputDataIndices: u16,
    NumberFeatureButtonCaps: u16,
    NumberFeatureValueCaps: u16,
    NumberFeatureDataIndices: u16,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(name: *const u16, desired_access: u32, share_mode: u32, security: *const c_void, creation: u32, flags: u32, template: Handle) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
}

#[link(name = "hid")]
extern "system" {
    fn HidD_GetPreparsedData(handle: Handle, data: *mut *mut c_void) -> u8;
    fn HidD_FreePreparsedData(data: *mut c_void) -> u8;
    fn HidP_GetCaps(data: *mut c_void, caps: *mut HidpCaps) -> i32;
}

/// OutputReportByteLength décrit la collection HID Windows et peut être différent
/// de la longueur d'un rapport 0x11 Bluetooth particulier.
pub fn output_report_byte_length(path: &str) -> Result<u16, String> {
    let utf16: Vec<u16> = path.encode_utf16().chain(once(0)).collect();
    // SAFETY : chemin NUL-terminé, pointeurs valides ; HANDLE fermé systématiquement.
    unsafe {
        let handle = CreateFileW(utf16.as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, null(), OPEN_EXISTING, 0, null_mut());
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("CreateFileW : {}", std::io::Error::last_os_error()));
        }
        struct Guard(Handle);
        impl Drop for Guard {
            fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
        }
        let _guard = Guard(handle);

        let mut pp_data: *mut c_void = null_mut();
        if HidD_GetPreparsedData(handle, &mut pp_data) == 0 || pp_data.is_null() {
            return Err(format!("HidD_GetPreparsedData : {}", std::io::Error::last_os_error()));
        }
        struct PrepGuard(*mut c_void);
        impl Drop for PrepGuard {
            fn drop(&mut self) { unsafe { HidD_FreePreparsedData(self.0); } }
        }
        let _prep_guard = PrepGuard(pp_data);

        let mut caps = HidpCaps::default();
        let ntstatus = HidP_GetCaps(pp_data, &mut caps);
        if ntstatus < 0 {
            return Err(format!("HidP_GetCaps NTSTATUS 0x{:08X}", ntstatus as u32));
        }
        Ok(caps.OutputReportByteLength)
    }
}
