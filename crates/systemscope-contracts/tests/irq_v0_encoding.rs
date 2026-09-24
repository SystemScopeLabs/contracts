//! `irq.v0` canonical encoding and strict decoding (`docs/m2-design.md` §7.1).
//!
//! The golden vectors are written byte by byte from the primitive rules of
//! `docs/m0-design.md` §4.5, not produced by the encoder.

use systemscope_contracts::canonical::{DecodeError, Decoder, Encoder};
use systemscope_contracts::protocol::irq_v0::{self, IrqMsg};
use systemscope_contracts::protocol::{Message, ProtocolId, block_v0, mem, mem_v1};

/// `protocol name "irq" · version 0`.
#[rustfmt::skip]
const PROTOCOL: &[u8] = &[
    0x03, 0, 0, 0, b'i', b'r', b'q',   // name
    0x00, 0,                           // version u16
];

#[rustfmt::skip]
const LEVEL_FALSE: &[u8] = &[
    0x00,                              // Level
    0x00,                              //   asserted: false
];

#[rustfmt::skip]
const LEVEL_TRUE: &[u8] = &[
    0x00,                              // Level
    0x01,                              //   asserted: true
];

/// Every message: the one variant with both levels.
fn goldens() -> Vec<(IrqMsg, &'static [u8])> {
    vec![
        (IrqMsg::Level { asserted: false }, LEVEL_FALSE),
        (IrqMsg::Level { asserted: true }, LEVEL_TRUE),
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
fn the_protocol_is_irq_version_0() {
    assert_eq!(
        irq_v0::PROTOCOL,
        ProtocolId {
            name: "irq",
            version: 0
        }
    );
    for other in [mem::PROTOCOL, mem_v1::PROTOCOL, block_v0::PROTOCOL] {
        assert_ne!(irq_v0::PROTOCOL, other);
    }
    for (msg, _) in goldens() {
        assert_eq!(Message::from(msg).protocol(), irq_v0::PROTOCOL);
    }
}

#[test]
fn every_message_matches_hand_written_golden_bytes() {
    for (msg, payload) in goldens() {
        let msg = Message::Irq(msg);
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
fn the_level_tag_and_bool_bytes_are_pinned() {
    assert_eq!(LEVEL_FALSE, [0, 0], "Level tag 0, false");
    assert_eq!(LEVEL_TRUE, [0, 1], "Level tag 0, true");
}

#[test]
fn unknown_tags_are_rejected() {
    for byte in [1, 2, 0x80, 0xFF] {
        let mut bytes = framed(LEVEL_TRUE);
        bytes[PROTOCOL.len()] = byte;
        assert_eq!(decode(&bytes), tag("irq.v0", byte));
    }
}

#[test]
fn non_canonical_bools_are_rejected() {
    for byte in [2, 0x80, 0xFF] {
        let mut bytes = framed(LEVEL_TRUE);
        bytes[PROTOCOL.len() + 1] = byte;
        assert_eq!(decode(&bytes), tag("bool", byte));
    }
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
        for extra in [0x00, 0x01] {
            let mut longer = bytes.clone();
            longer.push(extra);
            assert_eq!(decode(&longer), Err(DecodeError::TrailingBytes));
        }
    }
}

#[test]
fn unknown_versions_and_names_are_rejected() {
    for version in [1u16, 2, u16::MAX] {
        let mut bytes = framed(LEVEL_TRUE);
        bytes[7..9].copy_from_slice(&version.to_le_bytes());
        assert_eq!(decode(&bytes), Err(DecodeError::UnknownProtocol));
    }
    #[rustfmt::skip]
    let names: [&[u8]; 3] = [
        &[0x03, 0, 0, 0, b'I', b'R', b'Q', 0x00, 0],
        &[0x04, 0, 0, 0, b'i', b'r', b'q', b's', 0x00, 0],
        &[0x02, 0, 0, 0, b'i', b'r', 0x00, 0],
    ];
    for name in names {
        let bytes = [name, LEVEL_TRUE].concat();
        assert_eq!(decode(&bytes), Err(DecodeError::UnknownProtocol));
    }
}
