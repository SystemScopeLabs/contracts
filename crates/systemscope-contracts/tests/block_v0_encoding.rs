//! `block.v0` canonical encoding and strict decoding (`docs/m2-design.md` §8.1).
//!
//! The golden vectors are written byte by byte from the primitive rules of
//! `docs/m0-design.md` §4.5, not produced by the encoder.

use proptest::prelude::*;
use systemscope_contracts::canonical::{DecodeError, Decoder, Encoder};
use systemscope_contracts::protocol::block_v0::{
    self, BLOCK_SIZE, BlockMsg, BlockReadOutcome, BlockWriteOutcome, MediaError, TxnId,
};
use systemscope_contracts::protocol::{Message, ProtocolId, irq_v0, mem, mem_v1};

/// `protocol name "block" · version 0`.
#[rustfmt::skip]
const PROTOCOL: &[u8] = &[
    0x05, 0, 0, 0, b'b', b'l', b'o', b'c', b'k',   // name
    0x00, 0,                                       // version u16
];

#[rustfmt::skip]
const READ_BLOCK: &[u8] = &[
    0x00,                              // ReadBlock
    0x21, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x07, 0, 0, 0, 0, 0, 0, 0,         //   lba 7
];

#[rustfmt::skip]
const WRITE_BLOCK: &[u8] = &[
    0x01,                              // WriteBlock
    0x22, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x02, 0, 0, 0, 0x01, 0, 0, 0,      //   lba 0x1_0000_0002
    0x02, 0, 0, 0,                     //   data length
    0xCA, 0xFE,                        //   data
];

#[rustfmt::skip]
const READ_RESULT_DATA: &[u8] = &[
    0x02,                              // ReadResult
    0x21, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x00,                              //   outcome: Data
    0x04, 0, 0, 0,                     //     data length
    0xDE, 0xAD, 0xBE, 0xEF,            //     data
];

#[rustfmt::skip]
const READ_RESULT_OUT_OF_RANGE: &[u8] = &[
    0x02,                              // ReadResult
    0x21, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x01,                              //   outcome: Error
    0x00,                              //     error: OutOfRange
];

#[rustfmt::skip]
const READ_RESULT_BAD_BLOCK: &[u8] = &[
    0x02,                              // ReadResult
    0x21, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x01,                              //   outcome: Error
    0x01,                              //     error: BadBlock
];

#[rustfmt::skip]
const WRITE_RESULT_DONE: &[u8] = &[
    0x03,                              // WriteResult
    0x22, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x00,                              //   outcome: Done
];

#[rustfmt::skip]
const WRITE_RESULT_OUT_OF_RANGE: &[u8] = &[
    0x03,                              // WriteResult
    0x22, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x01,                              //   outcome: Error
    0x00,                              //     error: OutOfRange
];

#[rustfmt::skip]
const WRITE_RESULT_BAD_BLOCK: &[u8] = &[
    0x03,                              // WriteResult
    0x22, 0, 0, 0, 0, 0, 0, 0,         //   txn
    0x01,                              //   outcome: Error
    0x01,                              //     error: BadBlock
];

/// Every message variant, every outcome, and every error.
fn goldens() -> Vec<(BlockMsg, &'static [u8])> {
    let read_error = |error| BlockMsg::ReadResult {
        txn: TxnId(0x21),
        outcome: BlockReadOutcome::Error { error },
    };
    let write_error = |error| BlockMsg::WriteResult {
        txn: TxnId(0x22),
        outcome: BlockWriteOutcome::Error { error },
    };
    vec![
        (
            BlockMsg::ReadBlock {
                txn: TxnId(0x21),
                lba: 7,
            },
            READ_BLOCK,
        ),
        (
            BlockMsg::WriteBlock {
                txn: TxnId(0x22),
                lba: 0x1_0000_0002,
                data: vec![0xCA, 0xFE],
            },
            WRITE_BLOCK,
        ),
        (
            BlockMsg::ReadResult {
                txn: TxnId(0x21),
                outcome: BlockReadOutcome::Data {
                    data: vec![0xDE, 0xAD, 0xBE, 0xEF],
                },
            },
            READ_RESULT_DATA,
        ),
        (read_error(MediaError::OutOfRange), READ_RESULT_OUT_OF_RANGE),
        (read_error(MediaError::BadBlock), READ_RESULT_BAD_BLOCK),
        (
            BlockMsg::WriteResult {
                txn: TxnId(0x22),
                outcome: BlockWriteOutcome::Done,
            },
            WRITE_RESULT_DONE,
        ),
        (
            write_error(MediaError::OutOfRange),
            WRITE_RESULT_OUT_OF_RANGE,
        ),
        (write_error(MediaError::BadBlock), WRITE_RESULT_BAD_BLOCK),
    ]
}

/// A full block whose byte `i` is `i mod 256`, written without the encoder.
fn pattern_block() -> Vec<u8> {
    (0..=255u8).chain(0..=255u8).collect()
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

fn assert_golden(msg: BlockMsg, bytes: &[u8]) {
    let msg = Message::Block(msg);
    // encode matches the vector; decode gives the message back; each direction
    // round-trips.
    assert_eq!(encode(&msg), bytes, "{msg:?}");
    assert_eq!(decode(bytes), Ok(msg.clone()), "{msg:?}");
    assert_eq!(encode(&decode(bytes).unwrap()), bytes, "{msg:?}");
    assert_eq!(decode(&encode(&msg)), Ok(msg));
}

#[test]
fn the_protocol_is_block_version_0() {
    assert_eq!(
        block_v0::PROTOCOL,
        ProtocolId {
            name: "block",
            version: 0
        }
    );
    assert_eq!(BLOCK_SIZE, 512);
    for other in [mem::PROTOCOL, mem_v1::PROTOCOL, irq_v0::PROTOCOL] {
        assert_ne!(block_v0::PROTOCOL, other);
    }
    for (msg, _) in goldens() {
        assert_eq!(Message::from(msg).protocol(), block_v0::PROTOCOL);
    }
}

#[test]
fn every_variant_matches_hand_written_golden_bytes() {
    for (msg, payload) in goldens() {
        assert_golden(msg, &framed(payload));
    }
}

#[test]
fn full_blocks_match_hand_written_golden_bytes() {
    #[rustfmt::skip]
    let write_head: &[u8] = &[
        0x01,                              // WriteBlock
        0x22, 0, 0, 0, 0, 0, 0, 0,         //   txn
        0x03, 0, 0, 0, 0, 0, 0, 0,         //   lba 3
        0x00, 0x02, 0, 0,                  //   data length 512
    ];
    #[rustfmt::skip]
    let read_head: &[u8] = &[
        0x02,                              // ReadResult
        0x21, 0, 0, 0, 0, 0, 0, 0,         //   txn
        0x00,                              //   outcome: Data
        0x00, 0x02, 0, 0,                  //     data length 512
    ];
    let block = pattern_block();
    assert_eq!(block.len(), BLOCK_SIZE);
    assert_eq!(
        (block[0], block[255], block[256], block[511]),
        (0, 255, 0, 255)
    );
    assert_golden(
        BlockMsg::WriteBlock {
            txn: TxnId(0x22),
            lba: 3,
            data: block.clone(),
        },
        &framed(&[write_head, &block].concat()),
    );
    assert_golden(
        BlockMsg::ReadResult {
            txn: TxnId(0x21),
            outcome: BlockReadOutcome::Data {
                data: block.clone(),
            },
        },
        &framed(&[read_head, &block].concat()),
    );
}

#[test]
fn variant_tags_are_pinned() {
    // Message tags: the first payload byte.
    let tags: Vec<u8> = goldens().iter().map(|(_, p)| p[0]).collect();
    assert_eq!(tags, [0, 1, 2, 2, 2, 3, 3, 3]);
    // Outcome tags follow the 8-byte txn; the error tag follows the outcome tag.
    assert_eq!(READ_RESULT_DATA[9], 0, "BlockReadOutcome::Data");
    assert_eq!(READ_RESULT_OUT_OF_RANGE[9..], [1, 0], "Error, OutOfRange");
    assert_eq!(READ_RESULT_BAD_BLOCK[9..], [1, 1], "Error, BadBlock");
    assert_eq!(WRITE_RESULT_DONE[9..], [0], "BlockWriteOutcome::Done");
    assert_eq!(WRITE_RESULT_OUT_OF_RANGE[9..], [1, 0], "Error, OutOfRange");
    assert_eq!(WRITE_RESULT_BAD_BLOCK[9..], [1, 1], "Error, BadBlock");
}

#[test]
fn boundary_values_round_trip() {
    let cases = [
        BlockMsg::ReadBlock {
            txn: TxnId(0),
            lba: 0,
        },
        BlockMsg::ReadBlock {
            txn: TxnId(u64::MAX),
            lba: u64::from(u32::MAX),
        },
        BlockMsg::ReadBlock {
            txn: TxnId(1),
            lba: u64::from(u32::MAX) + 1,
        },
        BlockMsg::WriteBlock {
            txn: TxnId(u64::MAX),
            lba: u64::MAX,
            data: pattern_block(),
        },
        BlockMsg::ReadResult {
            txn: TxnId(0),
            outcome: BlockReadOutcome::Error {
                error: MediaError::BadBlock,
            },
        },
        BlockMsg::WriteResult {
            txn: TxnId(u64::MAX),
            outcome: BlockWriteOutcome::Done,
        },
    ];
    for msg in cases {
        let msg = Message::Block(msg);
        assert_eq!(decode(&encode(&msg)), Ok(msg));
    }
    #[rustfmt::skip]
    let max_read: &[u8] = &[
        0x00,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    assert_golden(
        BlockMsg::ReadBlock {
            txn: TxnId(u64::MAX),
            lba: u64::MAX,
        },
        &framed(max_read),
    );
}

/// Decoding checks bytes, not meaning: `data` of any length, including the wrong block
/// size, is representable and decodes. Only the receiver rejects a length other than
/// `BLOCK_SIZE` (`docs/m2-design.md` §8.1), so the decoder must never grow that check.
#[test]
fn any_data_length_decodes_and_is_left_to_the_receiver() {
    for len in [0, 1, BLOCK_SIZE - 1, BLOCK_SIZE, BLOCK_SIZE + 1] {
        let data = vec![0x5A; len];
        let write = BlockMsg::WriteBlock {
            txn: TxnId(4),
            lba: 9,
            data: data.clone(),
        };
        let read = BlockMsg::ReadResult {
            txn: TxnId(4),
            outcome: BlockReadOutcome::Data { data },
        };
        for msg in [write, read] {
            let msg = Message::Block(msg);
            assert_eq!(decode(&encode(&msg)), Ok(msg), "length {len}");
        }
    }
    // The same, from hand-written bytes: 511 (0x1FF) and 513 (0x201) bytes of data.
    #[rustfmt::skip]
    let write_head = |len: [u8; 4]| -> Vec<u8> {
        [&[
            0x01,
            0x04, 0, 0, 0, 0, 0, 0, 0,
            0x09, 0, 0, 0, 0, 0, 0, 0,
        ][..], &len].concat()
    };
    #[rustfmt::skip]
    let read_head = |len: [u8; 4]| -> Vec<u8> {
        [&[
            0x02,
            0x04, 0, 0, 0, 0, 0, 0, 0,
            0x00,
        ][..], &len].concat()
    };
    for (len, prefix) in [(511, [0xFF, 0x01, 0, 0]), (513, [0x01, 0x02, 0, 0])] {
        let data = vec![0x5A; len];
        let write = framed(&[write_head(prefix), data.clone()].concat());
        let read = framed(&[read_head(prefix), data.clone()].concat());
        assert_eq!(
            decode(&write),
            Ok(Message::Block(BlockMsg::WriteBlock {
                txn: TxnId(4),
                lba: 9,
                data: data.clone(),
            }))
        );
        assert_eq!(
            decode(&read),
            Ok(Message::Block(BlockMsg::ReadResult {
                txn: TxnId(4),
                outcome: BlockReadOutcome::Data { data },
            }))
        );
    }
}

#[test]
fn unknown_versions_and_names_are_rejected() {
    for version in [1u16, 2, u16::MAX] {
        let mut bytes = framed(READ_BLOCK);
        bytes[9..11].copy_from_slice(&version.to_le_bytes());
        assert_eq!(decode(&bytes), Err(DecodeError::UnknownProtocol));
    }
    #[rustfmt::skip]
    let names: [&[u8]; 3] = [
        &[0x05, 0, 0, 0, b'B', b'l', b'o', b'c', b'k', 0x00, 0],
        &[0x06, 0, 0, 0, b'b', b'l', b'o', b'c', b'k', b's', 0x00, 0],
        &[0x04, 0, 0, 0, b'd', b'i', b's', b'k', 0x00, 0],
    ];
    for name in names {
        let bytes = [name, READ_BLOCK].concat();
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
    assert_eq!(patched(READ_BLOCK, 0, 4), tag("block.v0", 4));
    assert_eq!(patched(READ_BLOCK, 0, 0xFF), tag("block.v0", 0xFF));
    assert_eq!(
        patched(READ_RESULT_OUT_OF_RANGE, 9, 2),
        tag("block.v0 read outcome", 2)
    );
    assert_eq!(
        patched(READ_RESULT_DATA, 9, 0xFF),
        tag("block.v0 read outcome", 0xFF)
    );
    assert_eq!(
        patched(WRITE_RESULT_DONE, 9, 2),
        tag("block.v0 write outcome", 2)
    );
    assert_eq!(
        patched(READ_RESULT_BAD_BLOCK, 10, 2),
        tag("block.v0 media error", 2)
    );
    assert_eq!(
        patched(WRITE_RESULT_BAD_BLOCK, 10, 0xFF),
        tag("block.v0 media error", 0xFF)
    );
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
    assert_eq!(with_len(WRITE_BLOCK, 17, 3), Err(DecodeError::Truncated));
    assert_eq!(
        with_len(WRITE_BLOCK, 17, u32::MAX),
        Err(DecodeError::Truncated)
    );
    assert_eq!(
        with_len(READ_RESULT_DATA, 10, 5),
        Err(DecodeError::Truncated)
    );
    // A shorter one leaves bytes over.
    assert_eq!(
        with_len(WRITE_BLOCK, 17, 1),
        Err(DecodeError::TrailingBytes)
    );
    assert_eq!(
        with_len(READ_RESULT_DATA, 10, 0),
        Err(DecodeError::TrailingBytes)
    );
}

fn any_error() -> impl Strategy<Value = MediaError> {
    prop_oneof![Just(MediaError::OutOfRange), Just(MediaError::BadBlock)]
}

fn any_lba() -> impl Strategy<Value = u64> {
    prop_oneof![
        any::<u64>(),
        Just(0),
        Just(u64::from(u32::MAX)),
        Just(u64::from(u32::MAX) + 1),
        Just(u64::MAX),
    ]
}

fn any_msg() -> impl Strategy<Value = BlockMsg> {
    let data = || proptest::collection::vec(any::<u8>(), 0..=BLOCK_SIZE + 1);
    prop_oneof![
        (any::<u64>(), any_lba()).prop_map(|(t, lba)| BlockMsg::ReadBlock { txn: TxnId(t), lba }),
        (any::<u64>(), any_lba(), data()).prop_map(|(t, lba, data)| BlockMsg::WriteBlock {
            txn: TxnId(t),
            lba,
            data
        }),
        (any::<u64>(), data()).prop_map(|(t, data)| BlockMsg::ReadResult {
            txn: TxnId(t),
            outcome: BlockReadOutcome::Data { data }
        }),
        (any::<u64>(), any_error()).prop_map(|(t, error)| BlockMsg::ReadResult {
            txn: TxnId(t),
            outcome: BlockReadOutcome::Error { error }
        }),
        any::<u64>().prop_map(|t| BlockMsg::WriteResult {
            txn: TxnId(t),
            outcome: BlockWriteOutcome::Done
        }),
        (any::<u64>(), any_error()).prop_map(|(t, error)| BlockMsg::WriteResult {
            txn: TxnId(t),
            outcome: BlockWriteOutcome::Error { error }
        }),
    ]
}

proptest! {
    #[test]
    fn every_message_round_trips(msg in any_msg()) {
        let msg = Message::Block(msg);
        let bytes = encode(&msg);
        prop_assert_eq!(decode(&bytes), Ok(msg));
        prop_assert_eq!(encode(&decode(&bytes).unwrap()), bytes);
    }
}
