//! Real Windows named-pipe PROBE for the *first-party dry-run Xbox preview*.
//!
//! This sends only PBXB Probe to a fixed local endpoint. The server uses an
//! explicitly provisioned numeric user SID in the DACL and impersonates the
//! last sender before responding. It NEVER routes to an XUSB/HID driver,
//! activates Xbox output, elevates, or creates a Windows service.
//!
//! No generic provider exchange is exported here on purpose: Open, Submit and
//! Close remain unavailable even to an authenticated user.

use std::io;

use super::broker_client_windows::exchange_existing_endpoint_for_pid;
use super::broker_pipe_windows::XBOX_PROBE_PIPE_NAME;
use super::xbox_wire::{self, Operation, ReplyCode, Request, RESPONSE_LEN};

/// Check that an already-running local preview accepts a read-only Probe.
/// A *trusted* source must provide the expected server PID (not the pipe or
/// its response). A PID check is not proof of code signature or service origin.
/// This does NOT imply a usable Xbox provider, XInput slot or installed driver.
pub fn probe_existing_xbox_preview(expected_server_pid: u32) -> io::Result<()> {
    let mut request = xbox_wire::encode_request(Request {
        operation: Operation::Probe, handle: 0, sequence: 1, state: [0; 12],
    })?;
    let mut response = [0u8; RESPONSE_LEN];
    exchange_existing_endpoint_for_pid(
        XBOX_PROBE_PIPE_NAME, &mut request, &mut response, expected_server_pid,
    )?;
    let status = xbox_wire::decode_response(&response)?;
    if status.sequence != 1 || status.handle != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData,
            "Xbox preview returned a replayed or invalid Probe response"));
    }
    match status.code {
        ReplyCode::Ok => Ok(()),
        ReplyCode::Unauthorized => Err(io::Error::new(io::ErrorKind::PermissionDenied,
            "Windows token is not the provisioned Xbox preview SID")),
        _ => Err(io::Error::new(io::ErrorKind::NotConnected,
            "Xbox preview rejected Probe; no driver-backed provider is available")),
    }
}
