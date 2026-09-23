//! `mem.v1` canonical encoding and strict decoding (`docs/m1-design.md` §4.3).
//!
//! The golden vectors are written byte by byte from the primitive rules of
//! `docs/m0-design.md` §4.5, not produced by the encoder.

use proptest::prelude::*;
use systemscope_contracts::canonical::{DecodeError, Decoder, Encoder};
use systemscope_contracts::protocol::mem_v1::{
    self, Access, MemFault, MemMsg, ReadOutcome, TxnId, WriteOutcome,
};
use systemscope_contracts::protocol::{Message, ProtocolId, mem};

/// `protocol name "mem" · version 1`.
#[rustfmt::skip]
const PROTOCOL: &[u8] = &[
    0x03, 0, 0, 0, b'm', b'e', b'm',   // name
    0x01, 0,                           // version u16
];

#[rustfmt::skip]
const READ_REQ: &[u8] = &[
    0x00,                              // ReadReq
    0x09, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0, 0, 0, 0x80, 0, 0, 0, 0,         //   addr 0x8000_0000
    0x04, 0, 0, 0,                     //   len u32
];

#[rustfmt::skip]
const READ_RESP_DATA: &[u8] = &[
    0x01,                              // ReadResp
    0x09, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x00,                              //   outcome: Data
    0x04, 0, 0, 0,                     //     data length
    0xDE, 0xAD, 0xBE, 0xEF,            //     data
];

#[rustfmt::skip]
const READ_RESP_FAULT: &[u8] = &[
    0x01,                              // ReadResp
    0x09, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x01,                              //   outcome: Fault
    0x00,                              //     fault: AccessFault
];

#[rustfmt::skip]
const WRITE_REQ: &[u8] = &[
    0x02,                              // WriteReq
    0x0A, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0, 0, 0, 0x10, 0, 0, 0, 0,         //   addr 0x1000_0000
    0x01, 0, 0, 0,                     //   data length
    b'H',                              //   data
];

#[rustfmt::skip]
const WRITE_RESP_DONE: &[u8] = &[
    0x03,                              // WriteResp
    0x0A, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x00,                              //   outcome: Done
];

#[rustfmt::skip]
const WRITE_RESP_FAULT: &[u8] = &[
    0x03,                              // WriteResp
    0x0A, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x01,                              //   outcome: Fault
    0x00,                              //     fault: AccessFault
];

fn fault() -> MemFault {
    MemFault::AccessFault
}

/// The six required vectors: every message variant and every outcome.
fn goldens() -> Vec<(MemMsg, &'static [u8])> {
    vec![
        (
            MemMsg::ReadReq {
                txn: TxnId(9),
                addr: 0x8000_0000,
                len: 4,
            },
            READ_REQ,
        ),
        (
            MemMsg::ReadResp {
                txn: TxnId(9),
                outcome: ReadOutcome::Data {
                    data: vec![0xDE, 0xAD, 0xBE, 0xEF],
                },
            },
            READ_RESP_DATA,
        ),
        (
            MemMsg::ReadResp {
                txn: TxnId(9),
                outcome: ReadOutcome::Fault { fault: fault() },
            },
            READ_RESP_FAULT,
        ),
        (
            MemMsg::WriteReq {
                txn: TxnId(10),
                addr: 0x1000_0000,
                data: vec![b'H'],
            },
            WRITE_REQ,
        ),
        (
            MemMsg::WriteResp {
                txn: TxnId(10),
                outcome: WriteOutcome::Done,
            },
            WRITE_RESP_DONE,
        ),
        (
            MemMsg::WriteResp {
                txn: TxnId(10),
                outcome: WriteOutcome::Fault { fault: fault() },
            },
            WRITE_RESP_FAULT,
        ),
    ]
}

fn framed(payload: &[u8]) -> Vec<u8> {
    [PROTOCOL, payload].concat()
}

fn encode(msg: &Message) -> Vec<u8> {
    let mut e = Encoder::new();
    msg.encode(&mut e);
    e.into_bytes()
}

fn decode(bytes: &[u8]) -> Result<Message, DecodeError> {
    let mut d = Decoder::new(bytes);
    let msg = Message::decode(&mut d)?;
    d.finish()?;
    Ok(msg)
}

fn tag(what: &'static str, tag: u8) -> Result<Message, DecodeError> {
    Err(DecodeError::InvalidTag { what, tag })
}

#[test]
fn the_protocol_is_mem_version_1() {
    assert_eq!(
        mem_v1::PROTOCOL,
        ProtocolId {
            name: "mem",
            version: 1
        }
    );
    assert_ne!(mem_v1::PROTOCOL, mem::PROTOCOL);
    for (msg, _) in goldens() {
        assert_eq!(Message::from(msg).protocol(), mem_v1::PROTOCOL);
    }
}

#[test]
fn every_variant_matches_hand_written_golden_bytes() {
    for (msg, payload) in goldens() {
        let msg = Message::MemV1(msg);
        let bytes = framed(payload);
        // encode matches the vector; decode gives the message back; each direction
        // round-trips.
        assert_eq!(encode(&msg), bytes, "{msg:?}");
        assert_eq!(decode(&bytes), Ok(msg.clone()), "{msg:?}");
        assert_eq!(encode(&decode(&bytes).unwrap()), bytes, "{msg:?}");
        assert_eq!(decode(&encode(&msg)), Ok(msg));
    }
}

#[test]
fn variant_tags_are_pinned() {
    // Message tags: the first payload byte.
    let tags: Vec<u8> = goldens().iter().map(|(_, p)| p[0]).collect();
    assert_eq!(tags, [0, 1, 1, 2, 3, 3]);
    // Outcome tags follow the 8-byte txn; the fault tag follows the outcome tag.
    assert_eq!(READ_RESP_DATA[9], 0, "ReadOutcome::Data");
    assert_eq!(
        READ_RESP_FAULT[9..],
        [1, 0],
        "ReadOutcome::Fault, AccessFault"
    );
    assert_eq!(WRITE_RESP_DONE[9], 0, "WriteOutcome::Done");
    assert_eq!(
        WRITE_RESP_FAULT[9..],
        [1, 0],
        "WriteOutcome::Fault, AccessFault"
    );
}

#[test]
fn requests_encode_as_in_mem_v0_apart_from_the_version() {
    let v0 = Message::Mem(mem::MemMsg::ReadReq {
        txn: TxnId(9),
        addr: 0x8000_0000,
        len: 4,
    });
    let v1 = Message::MemV1(goldens()[0].0.clone());
    let (a, b) = (encode(&v0), encode(&v1));
    assert_eq!(a.len(), b.len());
    let differing: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
    assert_eq!(differing, [7], "only the version's low byte differs");
}

#[test]
fn boundary_values_round_trip() {
    let cases = [
        MemMsg::ReadReq {
            txn: TxnId(0),
            addr: 0,
            len: 1,
        },
        MemMsg::ReadReq {
            txn: TxnId(u64::MAX),
            addr: u64::MAX,
            len: u32::MAX,
        },
        MemMsg::WriteReq {
            txn: TxnId(u64::MAX),
            addr: u64::MAX,
            data: vec![0xFF],
        },
        MemMsg::ReadResp {
            txn: TxnId(0),
            outcome: ReadOutcome::Fault { fault: fault() },
        },
        MemMsg::WriteResp {
            txn: TxnId(u64::MAX),
            outcome: WriteOutcome::Done,
        },
    ];
    for msg in cases {
        let msg = Message::MemV1(msg);
        assert_eq!(decode(&encode(&msg)), Ok(msg));
    }
    #[rustfmt::skip]
    let max_read: &[u8] = &[
        0x00,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF,
    ];
    assert_eq!(
        decode(&framed(max_read)),
        Ok(Message::MemV1(MemMsg::ReadReq {
            txn: TxnId(u64::MAX),
            addr: u64::MAX,
            len: u32::MAX,
        }))
    );
}

/// Decoding checks bytes, not meaning: an empty request or an empty `Data` response is
/// representable and decodes, and is rejected later by whoever receives it
/// (`docs/m1-design.md` §4.2).
#[test]
fn empty_payloads_are_representable_and_left_to_the_receiver() {
    let empty = [
        MemMsg::ReadReq {
            txn: TxnId(1),
            addr: 0x40,
            len: 0,
        },
        MemMsg::WriteReq {
            txn: TxnId(1),
            addr: 0x40,
            data: vec![],
        },
        MemMsg::ReadResp {
            txn: TxnId(1),
            outcome: ReadOutcome::Data { data: vec![] },
        },
    ];
    for msg in empty {
        assert_eq!(
            decode(&encode(&Message::MemV1(msg.clone()))),
            Ok(Message::MemV1(msg.clone()))
        );
    }
    assert_eq!(
        MemMsg::ReadReq {
            txn: TxnId(1),
            addr: 0x40,
            len: 0
        }
        .access(),
        Some(Access::Empty)
    );
}

#[test]
fn unknown_versions_are_rejected() {
    for version in [2u16, u16::MAX] {
        let mut bytes = framed(READ_REQ);
        bytes[7..9].copy_from_slice(&version.to_le_bytes());
        assert_eq!(decode(&bytes), Err(DecodeError::UnknownProtocol));
    }
}

#[test]
fn unknown_tags_are_rejected() {
    let patched = |payload: &[u8], at: usize, byte: u8| {
        let mut bytes = framed(payload);
        bytes[PROTOCOL.len() + at] = byte;
        decode(&bytes)
    };
    assert_eq!(patched(READ_REQ, 0, 4), tag("mem.v1", 4));
    assert_eq!(patched(READ_REQ, 0, 0xFF), tag("mem.v1", 0xFF));
    assert_eq!(
        patched(READ_RESP_FAULT, 9, 2),
        tag("mem.v1 read outcome", 2)
    );
    assert_eq!(
        patched(WRITE_RESP_DONE, 9, 2),
        tag("mem.v1 write outcome", 2)
    );
    assert_eq!(patched(READ_RESP_FAULT, 10, 1), tag("mem.v1 fault", 1));
    assert_eq!(patched(WRITE_RESP_FAULT, 10, 1), tag("mem.v1 fault", 1));
}

#[test]
fn every_truncation_and_any_trailing_byte_is_rejected() {
    for (_, payload) in goldens() {
        let bytes = framed(payload);
        for cut in 0..bytes.len() {
            assert_eq!(
                decode(&bytes[..cut]),
                Err(DecodeError::Truncated),
                "{payload:?} cut at {cut}"
            );
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(decode(&longer), Err(DecodeError::TrailingBytes));
    }
}

#[test]
fn byte_lengths_must_match_the_input() {
    let with_len = |payload: &[u8], at: usize, len: u32| {
        let mut bytes = framed(payload);
        let at = PROTOCOL.len() + at;
        bytes[at..at + 4].copy_from_slice(&len.to_le_bytes());
        decode(&bytes)
    };
    // A declared length longer than the input is truncation, never a short read.
    assert_eq!(with_len(READ_RESP_DATA, 10, 5), Err(DecodeError::Truncated));
    assert_eq!(
        with_len(READ_RESP_DATA, 10, u32::MAX),
        Err(DecodeError::Truncated)
    );
    assert_eq!(with_len(WRITE_REQ, 17, 2), Err(DecodeError::Truncated));
    // A shorter one leaves bytes over.
    assert_eq!(
        with_len(READ_RESP_DATA, 10, 3),
        Err(DecodeError::TrailingBytes)
    );
    assert_eq!(with_len(WRITE_REQ, 17, 0), Err(DecodeError::TrailingBytes));
}

fn any_fault() -> impl Strategy<Value = MemFault> {
    Just(MemFault::AccessFault)
}

fn any_msg() -> impl Strategy<Value = MemMsg> {
    let data = || proptest::collection::vec(any::<u8>(), 0..16);
    prop_oneof![
        (any::<u64>(), any::<u64>(), any::<u32>()).prop_map(|(t, addr, len)| MemMsg::ReadReq {
            txn: TxnId(t),
            addr,
            len
        }),
        (any::<u64>(), data()).prop_map(|(t, data)| MemMsg::ReadResp {
            txn: TxnId(t),
            outcome: ReadOutcome::Data { data }
        }),
        (any::<u64>(), any_fault()).prop_map(|(t, fault)| MemMsg::ReadResp {
            txn: TxnId(t),
            outcome: ReadOutcome::Fault { fault }
        }),
        (any::<u64>(), any::<u64>(), data()).prop_map(|(t, addr, data)| MemMsg::WriteReq {
            txn: TxnId(t),
            addr,
            data
        }),
        any::<u64>().prop_map(|t| MemMsg::WriteResp {
            txn: TxnId(t),
            outcome: WriteOutcome::Done
        }),
        (any::<u64>(), any_fault()).prop_map(|(t, fault)| MemMsg::WriteResp {
            txn: TxnId(t),
            outcome: WriteOutcome::Fault { fault }
        }),
    ]
}

proptest! {
    #[test]
    fn every_message_round_trips(msg in any_msg()) {
        let msg = Message::MemV1(msg);
        let bytes = encode(&msg);
        prop_assert_eq!(decode(&bytes), Ok(msg));
        prop_assert_eq!(encode(&decode(&bytes).unwrap()), bytes);
    }
}
