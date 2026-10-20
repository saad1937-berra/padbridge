//! V1 framing for a future authenticated **local** Windows named-pipe broker.
//!
//! Authentication is NOT a password or a field in this frame. The pipe's
//! Windows ACL and (future) service-side token verification establish identity.
//! This module deliberately has no I/O, elevation, device access or Tauri hooks.

use super::vhf_broker::Report;
use super::vhf_protocol::{encode, validate_report, REPORT_LEN};
use super::backend::OutputState;

pub const REQUEST_LEN: usize = 16;
pub const RESPONSE_LEN: usize = 8;
pub const VERSION: u8 = 1;
const MAGIC: &[u8; 4] = b"PBIP";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Ping,
    Neutral,
    Submit(Report),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Ok = 0,
    Malformed = 1,
    Unauthorized = 2,
    Disabled = 3,
    DeviceUnavailable = 4,
    Throttled = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidLength,
    InvalidMagic,
    InvalidVersion,
    ReservedField,
    InvalidCommand,
    InvalidReport,
}

pub fn decode(frame: &[u8]) -> Result<Command, ProtocolError> {
    if frame.len() != REQUEST_LEN {
        return Err(ProtocolError::InvalidLength);
    }
    if &frame[..4] != MAGIC {
        return Err(ProtocolError::InvalidMagic);
    }
    if frame[4] != VERSION {
        return Err(ProtocolError::InvalidVersion);
    }
    if frame[6] != 0 || frame[7] != 0 {
        return Err(ProtocolError::ReservedField);
    }
    let report: Report = frame[8..].try_into().expect("10 report bytes");
    match frame[5] {
        1 if report == [0; REPORT_LEN] => Ok(Command::Ping),
        2 if report == [0; REPORT_LEN] => Ok(Command::Neutral),
        3 => {
            validate_report(&report).map_err(|_| ProtocolError::InvalidReport)?;
            Ok(Command::Submit(report))
        }
        4 if report == [0; REPORT_LEN] => Ok(Command::Close),
        1 | 2 | 4 => Err(ProtocolError::ReservedField),
        _ => Err(ProtocolError::InvalidCommand),
    }
}

pub fn encode_request(command: Command) -> [u8; REQUEST_LEN] {
    let mut result = [0u8; REQUEST_LEN];
    result[..4].copy_from_slice(MAGIC);
    result[4] = VERSION;
    match command {
        Command::Ping => result[5] = 1,
        Command::Neutral => result[5] = 2,
        Command::Submit(report) => {
            result[5] = 3;
            result[8..].copy_from_slice(&report);
        }
        Command::Close => result[5] = 4,
    }
    result
}

pub fn response(status: Status) -> [u8; RESPONSE_LEN] {
    let mut buf = [0u8; RESPONSE_LEN];
    buf[..4].copy_from_slice(MAGIC);
    buf[4] = VERSION;
    buf[5] = status as u8;
    buf
}

pub fn neutral_request() -> [u8; REQUEST_LEN] {
    encode_request(Command::Submit(encode(&OutputState::neutral())))
}

/// Initial service policy: parse local frames, but NEVER write to a driver.
/// No user (including admin) can toggle this at runtime.
pub fn handle_dry_run(frame: &[u8]) -> (Status, bool) {
    match decode(frame) {
        Ok(Command::Ping) => (Status::Ok, false),
        Ok(Command::Close) => (Status::Ok, true),
        Ok(Command::Neutral | Command::Submit(_)) => (Status::Disabled, false),
        Err(_) => (Status::Malformed, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_ping_close_and_neutral() {
        for command in [Command::Ping, Command::Close, Command::Neutral,
            Command::Submit(encode(&OutputState::neutral()))] {
            assert_eq!(decode(&encode_request(command)), Ok(command));
        }
        assert_eq!(decode(&neutral_request()), Ok(Command::Submit(
            encode(&OutputState::neutral()))));
    }

    #[test]
    fn malformed_and_reserved_frames_are_rejected() {
        let src = encode_request(Command::Ping);
        assert_eq!(decode(&src[..15]), Err(ProtocolError::InvalidLength));
        let mut invalid = src;
        invalid[0] = 0;
        assert_eq!(decode(&invalid), Err(ProtocolError::InvalidMagic));
        invalid = src;
        invalid[4] = 2;
        assert_eq!(decode(&invalid), Err(ProtocolError::InvalidVersion));
        invalid = src;
        invalid[6] = 1;
        assert_eq!(decode(&invalid), Err(ProtocolError::ReservedField));
        invalid = src;
        invalid[8] = 1;
        assert_eq!(decode(&invalid), Err(ProtocolError::ReservedField));
        invalid = src;
        invalid[5] = 99;
        assert_eq!(decode(&invalid), Err(ProtocolError::InvalidCommand));
        invalid = neutral_request();
        invalid[11] = 9;
        assert_eq!(decode(&invalid), Err(ProtocolError::InvalidReport));
    }

    #[test]
    fn dry_run_never_forwards_active_input() {
        let mut active = encode(&OutputState::neutral());
        active[1] = 1;
        assert_eq!(handle_dry_run(&encode_request(Command::Submit(active))),
            (Status::Disabled, false));
        assert_eq!(handle_dry_run(&encode_request(Command::Neutral)),
            (Status::Disabled, false));
        assert_eq!(handle_dry_run(&encode_request(Command::Ping)),
            (Status::Ok, false));
        assert_eq!(handle_dry_run(&encode_request(Command::Close)),
            (Status::Ok, true));
    }

    #[test]
    fn response_has_version_and_fixed_size() {
        assert_eq!(response(Status::Unauthorized), [b'P', b'B', b'I', b'P', 1, 2, 0, 0]);
    }
}
