//! Canonical binary encoding: the primitive rules of `docs/m0-design.md` §4.5.
//!
//! Integers are fixed-width little-endian. Byte strings and strings carry a `u32` length;
//! sequences carry a `u32` element count. Nothing depends on the host or a serializer.
//! Decoding is strict: [`Decoder`] accepts exactly the bytes [`Encoder`] can produce.

use core::fmt;

use crate::component::{ComponentId, Delivered, PortId};
use crate::event::{EventKey, Phase};
use crate::protocol::Message;
use crate::protocol::mem::{self, MemMsg, TxnId};
use crate::protocol::mem_v1::{self, MemFault, ReadOutcome, WriteOutcome};
use crate::time::Tick;

/// Appends canonical encodings to a byte buffer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    /// Creates an empty encoder.
    pub fn new() -> Encoder {
        Encoder::default()
    }

    /// The bytes written so far.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Consumes the encoder, returning its bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Writes raw bytes with no length prefix. For fixed-size fields such as magics.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Writes a `u8`.
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Writes a `u16`, little-endian.
    pub fn u16(&mut self, v: u16) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes a `u32`, little-endian.
    pub fn u32(&mut self, v: u32) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes a `u64`, little-endian.
    pub fn u64(&mut self, v: u64) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes a `u128`, little-endian.
    pub fn u128(&mut self, v: u128) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes an `i64`, little-endian two's complement.
    pub fn i64(&mut self, v: i64) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes a `bool` as `0` or `1`.
    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }

    /// Writes a `u32` length, then the bytes.
    ///
    /// # Panics
    ///
    /// If `bytes` is 4 GiB or longer, which no canonical value may be.
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.raw(bytes);
    }

    /// Writes a string as its UTF-8 bytes, length-prefixed.
    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    /// Writes a sequence's `u32` element count. The caller then writes each element.
    ///
    /// # Panics
    ///
    /// If `n` does not fit in a `u32`.
    pub fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("canonical lengths are below 2^32"));
    }
}

/// Why canonical bytes could not be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input ended inside a value.
    Truncated,
    /// Bytes remained after the last value.
    TrailingBytes,
    /// An enum tag, phase, or `bool` had no meaning.
    InvalidTag {
        /// What was being decoded.
        what: &'static str,
        /// The byte found.
        tag: u8,
    },
    /// A string was not UTF-8.
    InvalidUtf8,
    /// A message named a protocol, or protocol version, this build does not know.
    UnknownProtocol,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Truncated => f.write_str("input ends inside a value"),
            DecodeError::TrailingBytes => f.write_str("bytes remain after the last value"),
            DecodeError::InvalidTag { what, tag } => write!(f, "invalid {what} tag {tag}"),
            DecodeError::InvalidUtf8 => f.write_str("string is not UTF-8"),
            DecodeError::UnknownProtocol => f.write_str("unknown protocol or version"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Reads canonical encodings from a byte slice.
#[derive(Clone, Debug)]
pub struct Decoder<'a> {
    buf: &'a [u8],
}

impl<'a> Decoder<'a> {
    /// Starts reading `bytes`.
    pub fn new(bytes: &'a [u8]) -> Decoder<'a> {
        Decoder { buf: bytes }
    }

    /// Number of unread bytes.
    pub fn remaining(&self) -> usize {
        self.buf.len()
    }

    /// Succeeds only if every byte was read.
    pub fn finish(self) -> Result<(), DecodeError> {
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(DecodeError::TrailingBytes)
        }
    }

    /// Reads `n` raw bytes.
    pub fn raw(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.buf.len() < n {
            return Err(DecodeError::Truncated);
        }
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }

    /// Reads a fixed-size array, such as a digest.
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let bytes = self.raw(N)?;
        Ok(bytes.try_into().expect("raw returned N bytes"))
    }

    /// Reads a `u8`.
    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.raw(1)?[0])
    }

    /// Reads a little-endian `u16`.
    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        self.array().map(u16::from_le_bytes)
    }

    /// Reads a little-endian `u32`.
    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        self.array().map(u32::from_le_bytes)
    }

    /// Reads a little-endian `u64`.
    pub fn u64(&mut self) -> Result<u64, DecodeError> {
        self.array().map(u64::from_le_bytes)
    }

    /// Reads a little-endian `u128`.
    pub fn u128(&mut self) -> Result<u128, DecodeError> {
        self.array().map(u128::from_le_bytes)
    }

    /// Reads a little-endian two's-complement `i64`.
    pub fn i64(&mut self) -> Result<i64, DecodeError> {
        self.array().map(i64::from_le_bytes)
    }

    /// Reads a `bool`; only `0` and `1` are valid.
    pub fn bool(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(DecodeError::InvalidTag { what: "bool", tag }),
        }
    }

    /// Reads a sequence's `u32` element count.
    #[allow(
        clippy::len_without_is_empty,
        reason = "reads a count from the input, mirroring Encoder::len"
    )]
    pub fn len(&mut self) -> Result<usize, DecodeError> {
        self.u32().map(|n| n as usize)
    }

    /// Reads a length-prefixed byte string.
    pub fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let n = self.len()?;
        self.raw(n)
    }

    /// Reads a length-prefixed UTF-8 string.
    pub fn str(&mut self) -> Result<&'a str, DecodeError> {
        core::str::from_utf8(self.bytes()?).map_err(|_| DecodeError::InvalidUtf8)
    }
}

impl Phase {
    /// The phase with this `repr(u8)` value.
    pub fn from_u8(tag: u8) -> Result<Phase, DecodeError> {
        Phase::ALL
            .get(usize::from(tag))
            .copied()
            .ok_or(DecodeError::InvalidTag { what: "phase", tag })
    }
}

impl EventKey {
    /// Writes `tick u64 · phase u8 · sequence u64`.
    pub fn encode(&self, e: &mut Encoder) {
        e.u64(self.tick.0);
        e.u8(self.phase as u8);
        e.u64(self.sequence);
    }

    /// Reads what [`EventKey::encode`] writes.
    pub fn decode(d: &mut Decoder<'_>) -> Result<EventKey, DecodeError> {
        Ok(EventKey {
            tick: Tick(d.u64()?),
            phase: Phase::from_u8(d.u8()?)?,
            sequence: d.u64()?,
        })
    }
}

impl MemMsg {
    /// Writes the variant tag in declaration order, then the fields.
    pub fn encode(&self, e: &mut Encoder) {
        match self {
            MemMsg::ReadReq { txn, addr, len } => {
                e.u8(0);
                e.u64(txn.0);
                e.u64(*addr);
                e.u32(*len);
            }
            MemMsg::ReadResp { txn, data } => {
                e.u8(1);
                e.u64(txn.0);
                e.bytes(data);
            }
            MemMsg::WriteReq { txn, addr, data } => {
                e.u8(2);
                e.u64(txn.0);
                e.u64(*addr);
                e.bytes(data);
            }
            MemMsg::WriteResp { txn } => {
                e.u8(3);
                e.u64(txn.0);
            }
        }
    }

    /// Reads what [`MemMsg::encode`] writes.
    pub fn decode(d: &mut Decoder<'_>) -> Result<MemMsg, DecodeError> {
        let msg = match d.u8()? {
            0 => MemMsg::ReadReq {
                txn: TxnId(d.u64()?),
                addr: d.u64()?,
                len: d.u32()?,
            },
            1 => MemMsg::ReadResp {
                txn: TxnId(d.u64()?),
                data: d.bytes()?.to_vec(),
            },
            2 => MemMsg::WriteReq {
                txn: TxnId(d.u64()?),
                addr: d.u64()?,
                data: d.bytes()?.to_vec(),
            },
            3 => MemMsg::WriteResp {
                txn: TxnId(d.u64()?),
            },
            tag => {
                return Err(DecodeError::InvalidTag {
                    what: "mem.v0",
                    tag,
                });
            }
        };
        Ok(msg)
    }
}

impl mem_v1::MemMsg {
    /// Writes the variant tag in declaration order, then the fields; nested outcomes and
    /// faults are enums under the same rule.
    pub fn encode(&self, e: &mut Encoder) {
        match self {
            mem_v1::MemMsg::ReadReq { txn, addr, len } => {
                e.u8(0);
                e.u64(txn.0);
                e.u64(*addr);
                e.u32(*len);
            }
            mem_v1::MemMsg::ReadResp { txn, outcome } => {
                e.u8(1);
                e.u64(txn.0);
                match outcome {
                    ReadOutcome::Data { data } => {
                        e.u8(0);
                        e.bytes(data);
                    }
                    ReadOutcome::Fault { fault } => {
                        e.u8(1);
                        fault.encode(e);
                    }
                }
            }
            mem_v1::MemMsg::WriteReq { txn, addr, data } => {
                e.u8(2);
                e.u64(txn.0);
                e.u64(*addr);
                e.bytes(data);
            }
            mem_v1::MemMsg::WriteResp { txn, outcome } => {
                e.u8(3);
                e.u64(txn.0);
                match outcome {
                    WriteOutcome::Done => e.u8(0),
                    WriteOutcome::Fault { fault } => {
                        e.u8(1);
                        fault.encode(e);
                    }
                }
            }
        }
    }

    /// Reads what [`mem_v1::MemMsg::encode`] writes. It checks the bytes only: a
    /// zero-length request decodes, and a read's data length cannot be checked without
    /// its request (see [`mem_v1`]).
    pub fn decode(d: &mut Decoder<'_>) -> Result<mem_v1::MemMsg, DecodeError> {
        let msg = match d.u8()? {
            0 => mem_v1::MemMsg::ReadReq {
                txn: TxnId(d.u64()?),
                addr: d.u64()?,
                len: d.u32()?,
            },
            1 => mem_v1::MemMsg::ReadResp {
                txn: TxnId(d.u64()?),
                outcome: match d.u8()? {
                    0 => ReadOutcome::Data {
                        data: d.bytes()?.to_vec(),
                    },
                    1 => ReadOutcome::Fault {
                        fault: MemFault::decode(d)?,
                    },
                    tag => {
                        return Err(DecodeError::InvalidTag {
                            what: "mem.v1 read outcome",
                            tag,
                        });
                    }
                },
            },
            2 => mem_v1::MemMsg::WriteReq {
                txn: TxnId(d.u64()?),
                addr: d.u64()?,
                data: d.bytes()?.to_vec(),
            },
            3 => mem_v1::MemMsg::WriteResp {
                txn: TxnId(d.u64()?),
                outcome: match d.u8()? {
                    0 => WriteOutcome::Done,
                    1 => WriteOutcome::Fault {
                        fault: MemFault::decode(d)?,
                    },
                    tag => {
                        return Err(DecodeError::InvalidTag {
                            what: "mem.v1 write outcome",
                            tag,
                        });
                    }
                },
            },
            tag => {
                return Err(DecodeError::InvalidTag {
                    what: "mem.v1",
                    tag,
                });
            }
        };
        Ok(msg)
    }
}

impl MemFault {
    /// Writes the variant tag in declaration order.
    pub fn encode(&self, e: &mut Encoder) {
        match self {
            MemFault::AccessFault => e.u8(0),
        }
    }

    /// Reads what [`MemFault::encode`] writes.
    pub fn decode(d: &mut Decoder<'_>) -> Result<MemFault, DecodeError> {
        match d.u8()? {
            0 => Ok(MemFault::AccessFault),
            tag => Err(DecodeError::InvalidTag {
                what: "mem.v1 fault",
                tag,
            }),
        }
    }
}

impl Message {
    /// Writes `protocol name · protocol version u16 · payload`.
    pub fn encode(&self, e: &mut Encoder) {
        let protocol = self.protocol();
        e.str(protocol.name);
        e.u16(protocol.version);
        match self {
            Message::Mem(msg) => msg.encode(e),
            Message::MemV1(msg) => msg.encode(e),
        }
    }

    /// Reads what [`Message::encode`] writes. Unknown protocols are rejected.
    pub fn decode(d: &mut Decoder<'_>) -> Result<Message, DecodeError> {
        let name = d.str()?;
        let version = d.u16()?;
        match (name, version) {
            (n, v) if n == mem::PROTOCOL.name && v == mem::PROTOCOL.version => {
                MemMsg::decode(d).map(Message::Mem)
            }
            (n, v) if n == mem_v1::PROTOCOL.name && v == mem_v1::PROTOCOL.version => {
                mem_v1::MemMsg::decode(d).map(Message::MemV1)
            }
            _ => Err(DecodeError::UnknownProtocol),
        }
    }
}

impl Delivered {
    /// Writes tag `0` then `port u16` and the message, or tag `1` then `token u64`.
    pub fn encode(&self, e: &mut Encoder) {
        match self {
            Delivered::Message { port, msg } => {
                e.u8(0);
                e.u16(port.0);
                msg.encode(e);
            }
            Delivered::Wake { token } => {
                e.u8(1);
                e.u64(*token);
            }
        }
    }

    /// Reads what [`Delivered::encode`] writes.
    pub fn decode(d: &mut Decoder<'_>) -> Result<Delivered, DecodeError> {
        match d.u8()? {
            0 => Ok(Delivered::Message {
                port: PortId(d.u16()?),
                msg: Message::decode(d)?,
            }),
            1 => Ok(Delivered::Wake { token: d.u64()? }),
            tag => Err(DecodeError::InvalidTag {
                what: "delivery",
                tag,
            }),
        }
    }
}

/// One event in its canonical form, `canonical(ev)` of §4.5: what the execution digest
/// absorbs and what a snapshot's queue holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CanonicalEvent {
    /// The event's key.
    pub key: EventKey,
    /// The scheduling component, or [`ComponentId::RUNTIME`].
    pub source: ComponentId,
    /// The handling component.
    pub target: ComponentId,
    /// What is delivered.
    pub delivery: Delivered,
}

impl CanonicalEvent {
    /// Writes `key · source u32 · target u32 · delivery`.
    pub fn encode(&self, e: &mut Encoder) {
        self.key.encode(e);
        e.u32(self.source.0);
        e.u32(self.target.0);
        self.delivery.encode(e);
    }

    /// The encoding as a fresh byte vector.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut e = Encoder::new();
        self.encode(&mut e);
        e.into_bytes()
    }

    /// Reads what [`CanonicalEvent::encode`] writes.
    pub fn decode(d: &mut Decoder<'_>) -> Result<CanonicalEvent, DecodeError> {
        Ok(CanonicalEvent {
            key: EventKey::decode(d)?,
            source: ComponentId(d.u32()?),
            target: ComponentId(d.u32()?),
            delivery: Delivered::decode(d)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_are_fixed_width_little_endian() {
        let mut e = Encoder::new();
        e.u8(0xAB);
        e.u16(0x0102);
        e.u32(0x0102_0304);
        e.u64(0x0102_0304_0506_0708);
        e.i64(-2);
        e.bool(true);
        e.u128(1);
        #[rustfmt::skip]
        let expected: &[u8] = &[
            0xAB,
            0x02, 0x01,
            0x04, 0x03, 0x02, 0x01,
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
            0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0x01,
            0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(e.as_bytes(), expected);
    }

    #[test]
    fn strings_and_bytes_are_length_prefixed() {
        let mut e = Encoder::new();
        e.str("h\u{e9}");
        e.bytes(&[]);
        e.len(3);
        assert_eq!(
            e.into_bytes(),
            [3, 0, 0, 0, b'h', 0xC3, 0xA9, 0, 0, 0, 0, 3, 0, 0, 0]
        );
    }
}
