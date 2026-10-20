//! Windows-only PadBridge KMDF transport, behind opt-in "broker-poc".
//!
//! There is NO invocation path from Tauri and NO automatic elevation.
//! The future secured service can own one WindowsVhfTransport per device.
//! Unsafe code is limited to Win32 SetupAPI and DeviceIoControl boundaries.

use std::io;
use std::mem::size_of;
use std::ptr;

use windows_sys::core::GUID;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, ERROR_INSUFFICIENT_BUFFER,
    ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING};
use windows_sys::Win32::System::IO::DeviceIoControl;

use super::vhf_broker::{Report, ReportTransport};
use super::vhf_protocol::{validate_report, IOCTL_SUBMIT};

/// Private PadBridge KMDF interface, NOT the virtual HID child or a Sony/Xbox GUID.
/// Matches GUID_DEVINTERFACE_PADBRIDGE_VHF in driver/poc-vhf/PadBridgeVhf.c.
pub const DEVICE_INTERFACE: GUID = GUID {
    data1: 0xe23bd620,
    data2: 0x6064,
    data3: 0x4879,
    data4: [0x9c, 0x10, 0x1a, 0x5d, 0x42, 0x2b, 0xcc, 0x21],
};

struct DeviceSet(HANDLE);
impl Drop for DeviceSet {
    fn drop(&mut self) {
        // SAFETY: SetupDiGetClassDevsW returned this valid HDEVINFO.
        unsafe { SetupDiDestroyDeviceInfoList(self.0); }
    }
}

/// A single exclusive handle, usable ONLY by an authorized future broker.
/// This does not grant admin privileges or install a driver.
pub struct WindowsVhfTransport {
    handle: HANDLE,
}

// HANDLE is an opaque pointer; sharing between threads is intentionally
// prohibited. The ReportTransport interface requires a mutable owner only.

impl WindowsVhfTransport {
    /// Enumerate by exact PadBridge device-interface GUID; reject ambiguous
    /// registrations. The Windows driver ACL decides whether access is allowed.
    /// Opening this handle must happen in an independently authorized service.
    pub fn open_preinstalled() -> io::Result<Self> {
        let path = find_unique_padbridge_interface()?;
        const GENERIC_WRITE_ACCESS: u32 = 0x4000_0000;
        // SAFETY: null-terminated UTF-16 path produced by SetupAPI, no
        // inheritance, no sharing, OPEN_EXISTING (never create a file).
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                GENERIC_WRITE_ACCESS,
                0,
                ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle })
    }
}

impl ReportTransport for WindowsVhfTransport {
    fn send(&mut self, report: &Report) -> io::Result<()> {
        validate_report(report)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let mut bytes_returned = 0u32;
        // SAFETY: valid synchronous handle, fixed-size input, no output buffer;
        // the report remains alive until DeviceIoControl has returned.
        let result = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_SUBMIT,
                report.as_ptr().cast(),
                report.len() as u32,
                ptr::null_mut(),
                0,
                &mut bytes_returned,
                ptr::null_mut(),
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for WindowsVhfTransport {
    fn drop(&mut self) {
        // The higher-level BrokerSession Drop sends neutral before this runs;
        // the kernel's FileCleanup and watchdog protect crash scenarios.
        // SAFETY: handle obtained from CreateFileW and owned exclusively.
        unsafe { CloseHandle(self.handle); }
    }
}

/// Returns null-terminated UTF-16 path, never a user-supplied arbitrary path.
fn find_unique_padbridge_interface() -> io::Result<Vec<u16>> {
    // SAFETY: GUID and all output buffers remain valid during SetupAPI calls.
    let raw = unsafe {
        SetupDiGetClassDevsW(
            &DEVICE_INTERFACE, ptr::null(), ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let device_set = DeviceSet(raw);

    // SAFETY: zeroed POD Win32 structure; cbSize assigned before use.
    let mut iface: SP_DEVICE_INTERFACE_DATA = unsafe { std::mem::zeroed() };
    iface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;

    // SAFETY: device_set valid, interface metadata is mutable and correctly sized.
    let found = unsafe {
        SetupDiEnumDeviceInterfaces(device_set.0, ptr::null(), &DEVICE_INTERFACE, 0, &mut iface)
    };
    if found == 0 {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
            Err(io::Error::new(io::ErrorKind::NotFound, "PadBridge VHF driver interface not found"))
        } else { Err(error) };
    }

    // Refuse more than one PadBridge device; binding must be unambiguous.
    // SAFETY: same device set, valid mutable interface structure.
    let extra = unsafe {
        SetupDiEnumDeviceInterfaces(device_set.0, ptr::null(), &DEVICE_INTERFACE, 1, &mut iface)
    };
    if extra != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData,
            "multiple PadBridge VHF interfaces detected"));
    }
    // SAFETY: GetLastError belongs to immediate preceding Win32 call.
    if unsafe { GetLastError() } != ERROR_NO_MORE_ITEMS {
        return Err(io::Error::last_os_error());
    }

    // Reacquire the first element because the probe above reused the buffer.
    iface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
    // SAFETY: valid buffer.
    if unsafe { SetupDiEnumDeviceInterfaces(device_set.0, ptr::null(), &DEVICE_INTERFACE, 0, &mut iface) } == 0 {
        return Err(io::Error::last_os_error());
    }

    let mut required = 0u32;
    // SAFETY: sizing call; NULL detail pointer and zero length are required.
    let probe = unsafe {
        SetupDiGetDeviceInterfaceDetailW(device_set.0, &iface, ptr::null_mut(), 0, &mut required, ptr::null_mut())
    };
    if probe != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "SetupAPI detail size query failed"));
    }
    // Bound allocations even on compromised/erroneous metadata.
    if required < 8 || required > 65536 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad interface path size"));
    }

    // Vec<u64> gives >= 8-byte alignment to hold the Win32 detail structure.
    let mut storage = vec![0u64; (required as usize).div_ceil(8)];
    let detail = storage.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
    // SAFETY: buffer is 8-byte aligned and holds at least required bytes.
    // x64 Unicode SetupAPI requires cbSize == 8.
    unsafe { (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32 };
    // SAFETY: buffer contains initialized header; size exactly as queried.
    if unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            device_set.0, &iface, detail, required, ptr::null_mut(), ptr::null_mut())
    } == 0 {
        return Err(io::Error::last_os_error());
    }

    // In SP_DEVICE_INTERFACE_DETAIL_DATA_W, WCHAR DevicePath starts at byte 4.
    // Avoid reading beyond the allocated WCHAR portion; require a terminator.
    let wide = unsafe {
        std::slice::from_raw_parts(
            (detail.cast::<u8>()).add(4).cast::<u16>(),
            (required as usize - 4) / 2,
        )
    };
    let end = wide.iter().position(|c| *c == 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "unterminated device path"))?;
    if end == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty device path"));
    }
    let mut path = wide[..end].to_vec();
    path.push(0);
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_matches_private_kernel_source() {
        assert_eq!(DEVICE_INTERFACE.data1, 0xe23bd620);
        assert_eq!(DEVICE_INTERFACE.data4, [0x9c, 0x10, 0x1a, 0x5d, 0x42, 0x2b, 0xcc, 0x21]);
    }
}
