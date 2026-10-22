//! Provider-neutral private PadBridge IPC contract, NOT a native XUSB frame.
//!
//! This module has no Windows I/O and never starts a driver. Its binary packet
//! carries a packed XINPUT_GAMEPAD-like value ONLY between app and a future
//! independently audited local provider. A real channel must enforce peer
//! identity, ACL, bounded I/O, cancellation, and a provider-side watchdog.

use std::io;

use super::backend::XInputReport;
use super::xbox_provider_adapter::XboxTransport;
use super::xinput_state_contract::gamepad_fixture_bytes;

pub const REQUEST_LEN: usize = 36;
pub const RESPONSE_LEN: usize = 24;
const MAGIC: &[u8; 4] = b"PBXB";
const VERSION: u8 = 1;
const NEUTRAL: [u8; 12] = [0; 12];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Operation { Probe = 1, Open = 2, Submit = 3, Close = 4 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReplyCode {
    Ok = 0, Invalid = 1, Unauthorized = 2, Unavailable = 3,
    UnknownSession = 4, Busy = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub operation: Operation,
    pub handle: u64,
    pub sequence: u64,
    pub state: [u8; 12],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Response {
    pub code: ReplyCode,
    pub handle: u64,
    pub sequence: u64,
}

fn malformed(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}
fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, reason)
}

pub fn encode_request(req: Request) -> io::Result<[u8; REQUEST_LEN]> {
    if req.sequence == 0 { return Err(invalid("sequence zero forbidden")); }
    match req.operation {
        Operation::Probe | Operation::Open if req.handle == 0 && req.state == NEUTRAL => {}
        Operation::Submit if req.handle != 0 => {}
        Operation::Close if req.handle != 0 && req.state == NEUTRAL => {}
        _ => return Err(invalid("invalid handle or state for request")),
    }
    let mut frame = [0u8; REQUEST_LEN];
    frame[..4].copy_from_slice(MAGIC);
    frame[4] = VERSION;
    frame[5] = req.operation as u8;
    frame[8..16].copy_from_slice(&req.handle.to_le_bytes());
    frame[16..24].copy_from_slice(&req.sequence.to_le_bytes());
    frame[24..36].copy_from_slice(&req.state);
    Ok(frame)
}

pub fn decode_request(frame: &[u8]) -> io::Result<Request> {
    if frame.len() != REQUEST_LEN { return Err(malformed("wrong request length")); }
    if &frame[..4] != MAGIC || frame[4] != VERSION || frame[6] != 0 || frame[7] != 0 {
        return Err(malformed("invalid request header"));
    }
    let operation = match frame[5] {
        1 => Operation::Probe, 2 => Operation::Open,
        3 => Operation::Submit, 4 => Operation::Close,
        _ => return Err(malformed("invalid request operation")),
    };
    let req = Request {
        operation,
        handle: u64::from_le_bytes(frame[8..16].try_into().expect("exact 8")),
        sequence: u64::from_le_bytes(frame[16..24].try_into().expect("exact 8")),
        state: frame[24..36].try_into().expect("exact 12"),
    };
    encode_request(req).map_err(|_| malformed("invalid request fields"))?;
    Ok(req)
}

pub fn encode_response(resp: Response) -> [u8; RESPONSE_LEN] {
    let mut frame = [0u8; RESPONSE_LEN];
    frame[..4].copy_from_slice(MAGIC);
    frame[4] = VERSION;
    frame[5] = resp.code as u8;
    frame[8..16].copy_from_slice(&resp.handle.to_le_bytes());
    frame[16..24].copy_from_slice(&resp.sequence.to_le_bytes());
    frame
}

pub fn decode_response(frame: &[u8]) -> io::Result<Response> {
    if frame.len() != RESPONSE_LEN { return Err(malformed("wrong response length")); }
    if &frame[..4] != MAGIC || frame[4] != VERSION || frame[6] != 0 || frame[7] != 0 {
        return Err(malformed("invalid response header"));
    }
    let code = match frame[5] {
        0 => ReplyCode::Ok, 1 => ReplyCode::Invalid,
        2 => ReplyCode::Unauthorized, 3 => ReplyCode::Unavailable,
        4 => ReplyCode::UnknownSession, 5 => ReplyCode::Busy,
        _ => return Err(malformed("unrecognized response status")),
    };
    Ok(Response {
        code,
        handle: u64::from_le_bytes(frame[8..16].try_into().expect("exact 8")),
        sequence: u64::from_le_bytes(frame[16..24].try_into().expect("exact 8")),
    })
}

/// Transport itself MUST authenticate the server and bound every operation.
/// connected() is advisory, never proof of an actual Windows XInput device.
pub trait XboxExchange: Send {
    fn connected(&self) -> bool;
    fn exchange(&mut self, frame: &[u8; REQUEST_LEN]) -> io::Result<[u8; RESPONSE_LEN]>;
}

/// Not selected by Tauri; tested only against a memory-backed fake service.
pub struct XboxWireClient<E: XboxExchange> {
    exchange: E,
    sequence: u64,
}

impl<E: XboxExchange> XboxWireClient<E> {
    pub fn new(exchange: E) -> Self {
        Self { exchange, sequence: 0 }
    }

    fn request(&mut self, operation: Operation, handle: u64, state: [u8; 12]) -> io::Result<u64> {
        let next = self.sequence.checked_add(1)
            .ok_or_else(|| invalid("Xbox IPC sequence exhausted"))?;
        self.sequence = next;
        let bytes = encode_request(Request {
            operation, handle, sequence: next, state,
        })?;
        // Never automatically retry ambiguous Create/Submit writes.
        let raw = self.exchange.exchange(&bytes)?;
        let reply = decode_response(&raw)?;
        if reply.sequence != next {
            return Err(malformed("stale or replayed provider response"));
        }
        if reply.code != ReplyCode::Ok {
            return Err(match reply.code {
                ReplyCode::Unauthorized => io::Error::new(io::ErrorKind::PermissionDenied,
                    "provider rejected caller identity"),
                ReplyCode::Unavailable => io::Error::new(io::ErrorKind::NotConnected,
                    "provider disconnected"),
                ReplyCode::UnknownSession => io::Error::new(io::ErrorKind::NotFound,
                    "provider session no longer exists"),
                ReplyCode::Busy => io::Error::new(io::ErrorKind::WouldBlock,
                    "provider busy"),
                ReplyCode::Invalid => malformed("provider rejected request"),
                ReplyCode::Ok => unreachable!(),
            });
        }
        match operation {
            Operation::Probe if reply.handle == 0 => Ok(0),
            Operation::Open if reply.handle != 0 => Ok(reply.handle),
            Operation::Submit | Operation::Close if reply.handle == handle => Ok(handle),
            _ => Err(malformed("wrong provider response handle")),
        }
    }

    pub fn probe(&mut self) -> io::Result<()> {
        self.request(Operation::Probe, 0, NEUTRAL).map(|_| ())
    }
}

impl<E: XboxExchange> XboxTransport for XboxWireClient<E> {
    fn is_ready(&self) -> bool { self.exchange.connected() }

    fn open(&mut self) -> io::Result<u64> {
        if !self.exchange.connected() {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "Xbox provider offline"));
        }
        self.probe()?;
        self.request(Operation::Open, 0, NEUTRAL)
    }

    fn submit(&mut self, handle: u64, state: &XInputReport) -> io::Result<()> {
        self.request(Operation::Submit, handle, gamepad_fixture_bytes(state))
            .map(|_| ())
    }

    fn close(&mut self, handle: u64) -> io::Result<()> {
        self.request(Operation::Close, handle, NEUTRAL).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::backend::{OutputState, VirtualPadBackend, XButton};
    use super::super::xbox_provider_adapter::XboxAdapter;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Service {
        online: bool,
        next_handle: u64,
        sessions: BTreeMap<u64, [u8; 12]>,
        trace: Vec<Request>,
        corrupt: Option<u8>,
        reject_submit: bool,
    }
    struct MemoryExchange(Arc<Mutex<Service>>);
    impl XboxExchange for MemoryExchange {
        fn connected(&self) -> bool { self.0.lock().unwrap().online }
        fn exchange(&mut self, raw: &[u8; REQUEST_LEN]) -> io::Result<[u8; RESPONSE_LEN]> {
            let mut s = self.0.lock().unwrap();
            if !s.online { return Err(io::ErrorKind::NotConnected.into()); }
            let r = decode_request(raw)?;
            s.trace.push(r);
            let mut status = ReplyCode::Ok;
            let handle = match r.operation {
                Operation::Probe => 0,
                Operation::Open => {
                    s.next_handle += 1;
                    let handle = s.next_handle;
                    s.sessions.insert(handle, NEUTRAL);
                    handle
                }
                Operation::Submit => {
                    if s.reject_submit {
                        s.reject_submit = false;
                        status = ReplyCode::Unavailable;
                    } else if let Some(current) = s.sessions.get_mut(&r.handle) {
                        *current = r.state;
                    } else { status = ReplyCode::UnknownSession; }
                    r.handle
                }
                Operation::Close => {
                    if s.sessions.remove(&r.handle).is_none() {
                        status = ReplyCode::UnknownSession;
                    }
                    r.handle
                }
            };
            let mut resp = encode_response(Response { code: status, handle, sequence: r.sequence });
            if let Some(bit) = s.corrupt.take() { resp[bit as usize] ^= 1; }
            Ok(resp)
        }
    }

    fn rig() -> (XboxAdapter<XboxWireClient<MemoryExchange>>, Arc<Mutex<Service>>) {
        let service = Arc::new(Mutex::new(Service {
            online: true, ..Service::default()
        }));
        (XboxAdapter::new(XboxWireClient::new(MemoryExchange(service.clone()))), service)
    }

    fn pressed() -> OutputState {
        let mut state = OutputState::neutral();
        state.buttons.set(XButton::A, true);
        state.lt = 1.0;
        state.lx = -1.0;
        state
    }

    #[test]
    fn roundtrip_valid_and_rejects_malformed_requests() {
        let good = Request {
            operation: Operation::Probe, handle: 0, sequence: 1, state: NEUTRAL,
        };
        let frame = encode_request(good).unwrap();
        assert_eq!(decode_request(&frame).unwrap(), good);
        assert!(decode_request(&frame[..35]).is_err());
        for i in [0usize, 4, 5, 6, 7] {
            let mut corrupt = frame;
            corrupt[i] = 255;
            assert!(decode_request(&corrupt).is_err());
        }
        for wrong in [
            Request { handle: 2, ..good },
            Request { sequence: 0, ..good },
            Request { state: [1; 12], ..good },
            Request { operation: Operation::Submit, ..good },
            Request { operation: Operation::Close, ..good },
        ] { assert!(encode_request(wrong).is_err()); }
    }

    #[test]
    fn exact_little_endian_wire_layout_and_response_validation() {
        let req = Request {
            operation: Operation::Submit,
            handle: 0x0102_0304_0506_0708,
            sequence: 0x1112_1314_1516_1718,
            state: [0xA5; 12],
        };
        let data = encode_request(req).unwrap();
        assert_eq!(&data[..8], b"PBXB\x01\x03\x00\x00");
        assert_eq!(&data[8..16], &[8,7,6,5,4,3,2,1]);
        assert_eq!(&data[16..24], &[0x18,0x17,0x16,0x15,0x14,0x13,0x12,0x11]);
        assert_eq!(&data[24..], &[0xA5; 12]);
        assert_eq!(decode_request(&data).unwrap(), req);
        let resp = encode_response(Response { code: ReplyCode::Ok, handle: 3, sequence: 4 });
        assert_eq!(decode_response(&resp).unwrap().handle, 3);
        assert!(decode_response(&resp[..23]).is_err());
        let mut bad = resp;
        bad[5] = 255;
        assert!(decode_response(&bad).is_err());
    }

    #[test]
    fn end_to_end_session_open_neutral_active_neutral_close() {
        let (mut adapter, service) = rig();
        let id = adapter.create("ds4").unwrap();
        adapter.submit(&id, &pressed()).unwrap();
        let g = service.lock().unwrap();
        assert_eq!(g.sessions.len(), 1);
        let last = *g.sessions.values().next().unwrap();
        assert_eq!(&last[..4], &[0x00, 0x10, 255, 0]);
        assert_eq!(&last[4..6], &[0x01, 0x80]); // -32767 LE
        assert_eq!(g.trace.iter().map(|r| r.operation).collect::<Vec<_>>(),
            [Operation::Probe, Operation::Open, Operation::Submit, Operation::Submit]);
        drop(g);
        adapter.destroy(&id).unwrap();
        let g = service.lock().unwrap();
        assert!(g.sessions.is_empty());
        assert_eq!(g.trace[g.trace.len()-2].operation, Operation::Submit);
        assert_eq!(g.trace[g.trace.len()-2].state, NEUTRAL);
        assert_eq!(g.trace.last().unwrap().operation, Operation::Close);
    }

    #[test]
    fn two_controllers_keep_independent_provider_handles() {
        let (mut adapter, srv) = rig();
        let one = adapter.create("one").unwrap();
        let two = adapter.create("two").unwrap();
        adapter.submit(&one, &pressed()).unwrap();
        assert_eq!(srv.lock().unwrap().sessions.len(), 2);
        adapter.destroy(&one).unwrap();
        assert_eq!(srv.lock().unwrap().sessions.len(), 1);
        adapter.submit(&two, &pressed()).unwrap();
        adapter.destroy(&two).unwrap();
        assert!(srv.lock().unwrap().sessions.is_empty());
    }

    #[test]
    fn replayed_and_corrupt_replies_never_succeed() {
        for bit in [0u8, 4, 5, 6, 7, 16] {
            let srv = Arc::new(Mutex::new(Service { online: true, ..Service::default() }));
            let mut client = XboxWireClient::new(MemoryExchange(srv.clone()));
            srv.lock().unwrap().corrupt = Some(bit);
            assert_eq!(client.probe().unwrap_err().kind(), io::ErrorKind::InvalidData);
        }
    }

    #[test]
    fn wrong_provider_handle_rejected() {
        let srv = Arc::new(Mutex::new(Service { online: true, ..Service::default() }));
        let mut client = XboxWireClient::new(MemoryExchange(srv.clone()));
        srv.lock().unwrap().corrupt = Some(8);
        assert_eq!(client.probe().unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn provider_error_revokes_session_and_neutralizes() {
        let (mut adapter, srv) = rig();
        let id = adapter.create("ds4").unwrap();
        srv.lock().unwrap().reject_submit = true;
        assert!(adapter.submit(&id, &pressed()).is_err());
        assert!(srv.lock().unwrap().sessions.is_empty());
        assert!(adapter.submit(&id, &pressed()).is_err());
    }

    #[test]
    fn offline_provider_blocks_create_and_revokes_old_sessions() {
        let (mut adapter, srv) = rig();
        let old = adapter.create("ds4").unwrap();
        srv.lock().unwrap().online = false;
        assert!(adapter.submit(&old, &pressed()).is_err());
        srv.lock().unwrap().online = true;
        let replacement = adapter.create("ds4").unwrap();
        assert_ne!(old.0, replacement.0);
        assert!(adapter.submit(&old, &pressed()).is_err());
        adapter.destroy(&replacement).unwrap();
        // Service-side cleanup must be independent to eliminate ghosts if it crashed.
    }

    #[test]
    fn sequence_overflow_refused_without_any_request() {
        let srv = Arc::new(Mutex::new(Service { online: true, ..Service::default() }));
        let mut client = XboxWireClient::new(MemoryExchange(srv.clone()));
        client.sequence = u64::MAX;
        assert_eq!(client.probe().unwrap_err().kind(), io::ErrorKind::InvalidInput);
        assert!(srv.lock().unwrap().trace.is_empty());
    }

    #[test]
    fn malformed_request_lengths_and_reserved_fields_fail_closed() {
        let original = encode_request(Request {
            operation: Operation::Probe, handle: 0, sequence: 1, state: NEUTRAL,
        }).unwrap();
        for length in 0..REQUEST_LEN {
            assert!(decode_request(&original[..length]).is_err(),
                "truncated request accepted at {length} bytes");
        }
        let mut oversized = original.to_vec();
        oversized.push(0);
        assert!(decode_request(&oversized).is_err());
        for index in [6usize, 7] {
            let mut modified = original;
            modified[index] = 1;
            assert!(decode_request(&modified).is_err(),
                "nonzero reserved request byte {index} accepted");
        }
        let mut modified = original;
        modified[16..24].fill(0);
        assert!(decode_request(&modified).is_err(), "zero sequence accepted");
        modified = original;
        modified[24] = 1;
        assert!(decode_request(&modified).is_err(), "non-neutral Probe accepted");
    }

    #[test]
    fn malformed_response_lengths_status_and_reserved_fields_fail_closed() {
        let original = encode_response(Response {
            code: ReplyCode::Ok, handle: 0, sequence: 1,
        });
        for length in 0..RESPONSE_LEN {
            assert!(decode_response(&original[..length]).is_err(),
                "truncated response accepted at {length} bytes");
        }
        let mut oversized = original.to_vec();
        oversized.push(0);
        assert!(decode_response(&oversized).is_err());
        for index in [0usize, 4, 5, 6, 7] {
            let mut modified = original;
            modified[index] = 255;
            assert!(decode_response(&modified).is_err(),
                "corrupted response header byte {index} accepted");
        }
    }

    #[test]
    fn unavailable_channel_never_opens_a_session() {
        let srv = Arc::new(Mutex::new(Service::default()));
        let mut client = XboxWireClient::new(MemoryExchange(srv.clone()));
        assert!(!client.is_ready());
        assert!(client.open().is_err());
        assert!(srv.lock().unwrap().sessions.is_empty());
    }
}
