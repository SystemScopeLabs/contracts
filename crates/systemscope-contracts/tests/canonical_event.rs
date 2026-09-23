//! `canonical(ev)` and strict decoding (`docs/m0-design.md` §4.5).
//!
//! The golden vectors are written byte by byte from the specification, not produced by the
//! encoder. Strictness means every accepted input is exactly what the encoder writes.

use proptest::prelude::*;
use systemscope_contracts::canonical::{CanonicalEvent, DecodeError, Decoder, Encoder};
use systemscope_contracts::component::{ComponentId, Delivered, PortId};
use systemscope_contracts::event::{EventKey, Phase};
use systemscope_contracts::protocol::Message;
use systemscope_contracts::protocol::mem::{MemMsg, TxnId};
use systemscope_contracts::time::Tick;

fn event(source: ComponentId, delivery: Delivered) -> CanonicalEvent {
    CanonicalEvent {
        key: EventKey {
            tick: Tick(0x0102),
            phase: Phase::Transfer,
            sequence: 5,
        },
        source,
        target: ComponentId(2),
        delivery,
    }
}

fn message(msg: MemMsg) -> CanonicalEvent {
    event(
        ComponentId(7),
        Delivered::Message {
            port: PortId(3),
            msg: Message::Mem(msg),
        },
    )
}

#[rustfmt::skip]
const PREFIX: &[u8] = &[
    0x02, 0x01, 0, 0, 0, 0, 0, 0,   // tick
    0x01,                           // phase: Transfer
    0x05, 0, 0, 0, 0, 0, 0, 0,      // sequence
    0x07, 0, 0, 0,                  // source
    0x02, 0, 0, 0,                  // target
    0x00,                           // delivery: Message
    0x03, 0,                        //   port
    0x03, 0, 0, 0, b'm', b'e', b'm',//   protocol name
    0x00, 0,                        //   protocol version
];

fn golden(payload: &[u8]) -> Vec<u8> {
    [PREFIX, payload].concat()
}

#[rustfmt::skip]
const READ_REQ: &[u8] = &[
    0x00,                           // ReadReq
    0x09, 0, 0, 0, 0, 0, 0, 0,      //   txn
    0x40, 0, 0, 0, 0, 0, 0, 0,      //   addr
    0x08, 0, 0, 0,                  //   len u32
];

#[rustfmt::skip]
const READ_RESP: &[u8] = &[
    0x01,                           // ReadResp
    0x09, 0, 0, 0, 0, 0, 0, 0,      //   txn
    0x02, 0, 0, 0, 0xDE, 0xAD,      //   data
];

#[rustfmt::skip]
const WRITE_REQ: &[u8] = &[
    0x02,                           // WriteReq
    0x01, 0, 0, 0, 0, 0, 0, 0,      //   txn
    0x10, 0, 0, 0, 0, 0, 0, 0,      //   addr
    0x01, 0, 0, 0, 0xAA,            //   data
];

#[rustfmt::skip]
const WRITE_RESP: &[u8] = &[
    0x03,                           // WriteResp
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // txn
];

#[rustfmt::skip]
const WAKE: &[u8] = &[
    0x02, 0x01, 0, 0, 0, 0, 0, 0,   // tick
    0x01,                           // phase: Transfer
    0x05, 0, 0, 0, 0, 0, 0, 0,      // sequence
    0xFF, 0xFF, 0xFF, 0xFF,         // source: the runtime
    0x02, 0, 0, 0,                  // target
    0x01,                           // delivery: Wake
    0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, // token
];

fn goldens() -> Vec<(CanonicalEvent, Vec<u8>)> {
    vec![
        (
            message(MemMsg::ReadReq {
                txn: TxnId(9),
                addr: 0x40,
                len: 8,
            }),
            golden(READ_REQ),
        ),
        (
            message(MemMsg::ReadResp {
                txn: TxnId(9),
                data: vec![0xDE, 0xAD],
            }),
            golden(READ_RESP),
        ),
        (
            message(MemMsg::WriteReq {
                txn: TxnId(1),
                addr: 0x10,
                data: vec![0xAA],
            }),
            golden(WRITE_REQ),
        ),
        (
            message(MemMsg::WriteResp {
                txn: TxnId(u64::MAX),
            }),
            golden(WRITE_RESP),
        ),
        (
            event(
                ComponentId::RUNTIME,
                Delivered::Wake {
                    token: 0x1122_3344_5566_7788,
                },
            ),
            WAKE.to_vec(),
        ),
    ]
}

fn decode_all(bytes: &[u8]) -> Result<CanonicalEvent, DecodeError> {
    let mut d = Decoder::new(bytes);
    let ev = CanonicalEvent::decode(&mut d)?;
    d.finish()?;
    Ok(ev)
}

#[test]
fn every_variant_matches_hand_written_golden_bytes() {
    for (ev, bytes) in goldens() {
        assert_eq!(ev.to_bytes(), bytes, "{ev:?}");
        assert_eq!(decode_all(&bytes), Ok(ev));
    }
}

#[test]
fn every_proper_prefix_is_truncated() {
    for (_, bytes) in goldens() {
        for n in 0..bytes.len() {
            assert_eq!(decode_all(&bytes[..n]), Err(DecodeError::Truncated), "{n}");
        }
    }
}

#[test]
fn trailing_bytes_are_rejected() {
    for (_, mut bytes) in goldens() {
        bytes.push(0);
        assert_eq!(decode_all(&bytes), Err(DecodeError::TrailingBytes));
    }
}

/// Replaces the byte at `at` in the ReadReq golden.
fn patched(at: usize, byte: u8) -> Result<CanonicalEvent, DecodeError> {
    let mut bytes = golden(READ_REQ);
    bytes[at] = byte;
    decode_all(&bytes)
}

#[test]
fn meaningless_tags_are_rejected() {
    let tag = |what, tag| Err(DecodeError::InvalidTag { what, tag });
    assert_eq!(patched(8, 5), tag("phase", 5));
    assert_eq!(patched(25, 2), tag("delivery", 2));
    assert_eq!(patched(PREFIX.len(), 4), tag("mem.v0", 4));
    let mut wake = WAKE.to_vec();
    wake[25] = 2;
    assert_eq!(decode_all(&wake), tag("delivery", 2));
}

#[test]
fn unknown_protocols_and_bad_strings_are_rejected() {
    // Protocol name "mem" is at 32..35; its version at 35..37.
    assert_eq!(patched(34, b'x'), Err(DecodeError::UnknownProtocol));
    assert_eq!(patched(35, 1), Err(DecodeError::UnknownProtocol));
    assert_eq!(patched(32, 0xFF), Err(DecodeError::InvalidUtf8));
}

#[test]
fn primitive_decoding_is_strict() {
    let mut d = Decoder::new(&[0, 1, 2]);
    assert_eq!(d.bool(), Ok(false));
    assert_eq!(d.bool(), Ok(true));
    assert_eq!(
        d.bool(),
        Err(DecodeError::InvalidTag {
            what: "bool",
            tag: 2
        })
    );
    // A length that runs past the end is truncation, not a short read.
    let mut d = Decoder::new(&[5, 0, 0, 0, 1, 2]);
    assert_eq!(d.bytes(), Err(DecodeError::Truncated));
    // Every primitive reads back what the encoder wrote.
    let mut e = Encoder::new();
    e.u16(0xBEEF);
    e.u128(u128::MAX - 1);
    e.i64(i64::MIN);
    e.raw(&[9; 32]);
    let bytes = e.into_bytes();
    let mut d = Decoder::new(&bytes);
    assert_eq!(d.u16(), Ok(0xBEEF));
    assert_eq!(d.u128(), Ok(u128::MAX - 1));
    assert_eq!(d.i64(), Ok(i64::MIN));
    assert_eq!(d.array::<32>(), Ok([9; 32]));
    assert_eq!(d.remaining(), 0);
}

#[test]
fn phase_tags_follow_declaration_order() {
    for (i, p) in Phase::ALL.iter().enumerate() {
        assert_eq!(Phase::from_u8(i as u8), Ok(*p));
    }
}

fn mem_msg() -> impl Strategy<Value = MemMsg> {
    let data = || prop::collection::vec(any::<u8>(), 0..5);
    prop_oneof![
        (any::<u64>(), any::<u64>(), any::<u32>()).prop_map(|(t, addr, len)| MemMsg::ReadReq {
            txn: TxnId(t),
            addr,
            len
        }),
        (any::<u64>(), data()).prop_map(|(t, data)| MemMsg::ReadResp {
            txn: TxnId(t),
            data
        }),
        (any::<u64>(), any::<u64>(), data()).prop_map(|(t, addr, data)| MemMsg::WriteReq {
            txn: TxnId(t),
            addr,
            data
        }),
        any::<u64>().prop_map(|t| MemMsg::WriteResp { txn: TxnId(t) }),
    ]
}

fn canonical_event() -> impl Strategy<Value = CanonicalEvent> {
    let delivery = prop_oneof![
        (any::<u16>(), mem_msg()).prop_map(|(p, m)| Delivered::Message {
            port: PortId(p),
            msg: Message::Mem(m),
        }),
        any::<u64>().prop_map(|token| Delivered::Wake { token }),
    ];
    (
        any::<u64>(),
        0usize..5,
        any::<u64>(),
        any::<u32>(),
        any::<u32>(),
        delivery,
    )
        .prop_map(|(t, p, s, source, target, delivery)| CanonicalEvent {
            key: EventKey {
                tick: Tick(t),
                phase: Phase::ALL[p],
                sequence: s,
            },
            source: ComponentId(source),
            target: ComponentId(target),
            delivery,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn every_event_decodes_back_to_itself(ev in canonical_event()) {
        prop_assert_eq!(decode_all(&ev.to_bytes()), Ok(ev));
    }

    /// Whatever a decoder accepts, the encoder writes back byte for byte, so no event has
    /// two encodings.
    #[test]
    fn accepted_bytes_are_the_canonical_encoding(
        ev in canonical_event(),
        flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..3),
    ) {
        let mut bytes = ev.to_bytes();
        for (i, b) in flips {
            let i = i.index(bytes.len());
            bytes[i] = b;
        }
        let mut d = Decoder::new(&bytes);
        if let Ok(decoded) = CanonicalEvent::decode(&mut d) {
            let used = bytes.len() - d.remaining();
            prop_assert_eq!(decoded.to_bytes(), &bytes[..used]);
        }
    }
}
