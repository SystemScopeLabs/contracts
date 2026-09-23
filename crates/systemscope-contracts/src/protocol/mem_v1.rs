//! `mem.v1`: memory read/write transactions that can fail with an access fault
//! (`docs/m1-design.md` §4).
//!
//! Requests are the same as in [`mem.v0`](super::mem). Responses carry an outcome, so a
//! target or interconnect can report that nothing answers at an address instead of
//! faulting the session. `mem.v0` is frozen for M0 and is never changed; this is a
//! separate protocol with its own [`PROTOCOL`] id.
//!
//! # Versioning
//!
//! Variant order, field order, and every nested enum are part of the canonical encoding.
//! Any change to them, including a new [`MemFault`] variant, is a new protocol version,
//! never an edit to this one.
//!
//! # Requests are never empty
//!
//! A `ReadReq` has `len > 0`, and a `WriteReq` has non-empty `data`. A zero-length request
//! is not a memory access at all: it is a bug in the initiator, not an access fault, and a
//! target that receives one faults the session rather than answering. The wire format can
//! still represent one, so decoding accepts it; [`MemMsg::access`] classifies it.
//!
//! # Responses are checked by the initiator
//!
//! A response is decoded without its request, so decoding cannot know how many bytes a
//! read should return. The initiator checks that a [`ReadOutcome::Data`] response carries
//! exactly the `len` bytes it asked for, and faults the session otherwise.

use super::ProtocolId;
pub use super::mem::TxnId;

/// The `mem.v1` protocol.
pub const PROTOCOL: ProtocolId = ProtocolId {
    name: "mem",
    version: 1,
};

/// A `mem.v1` message. Variant and field order are part of the canonical encoding.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MemMsg {
    /// Read `len` bytes starting at `addr`. `len` is never 0.
    ReadReq {
        /// Transaction id, allocated by the initiator.
        txn: TxnId,
        /// Address of the first byte.
        addr: u64,
        /// Number of bytes; at least 1.
        len: u32,
    },
    /// The result of a read.
    ReadResp {
        /// Transaction id of the request.
        txn: TxnId,
        /// The bytes read, or why the read failed.
        outcome: ReadOutcome,
    },
    /// Write `data` starting at `addr`. `data` is never empty.
    WriteReq {
        /// Transaction id, allocated by the initiator.
        txn: TxnId,
        /// Address of the first byte.
        addr: u64,
        /// The bytes to write; at least one.
        data: Vec<u8>,
    },
    /// The result of a write.
    WriteResp {
        /// Transaction id of the request.
        txn: TxnId,
        /// Whether the write happened.
        outcome: WriteOutcome,
    },
}

/// What a read produced.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReadOutcome {
    /// The read succeeded. `data` must be exactly the requested `len` bytes, which the
    /// initiator checks.
    Data {
        /// The bytes read, in address order.
        data: Vec<u8>,
    },
    /// The read failed and returned nothing.
    Fault {
        /// Why it failed.
        fault: MemFault,
    },
}

/// What a write did.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WriteOutcome {
    /// Every byte was written.
    Done,
    /// The write failed and changed nothing at the target.
    Fault {
        /// Why it failed.
        fault: MemFault,
    },
}

/// Why an access failed. Deliberately generic: the target reports only that the access
/// failed, and the initiator decides what that means, such as which trap a CPU raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemFault {
    /// Nothing accepts this access: no target is mapped at some byte of the range, the
    /// range crosses from one target into another, the range runs past the end of the
    /// `u64` address space, or the target does not support this kind of access there.
    AccessFault,
}

/// The bytes a request touches, as an interconnect or target must classify them
/// (`docs/m1-design.md` §4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Bytes `first..=last`, with `first <= last`.
    Bytes {
        /// Address of the first byte.
        first: u64,
        /// Address of the last byte.
        last: u64,
    },
    /// A well-formed request whose last byte would lie past `u64::MAX`. It is answered
    /// with [`MemFault::AccessFault`].
    OutOfRange,
    /// A zero-length request. It is not an access: the initiator broke the protocol, and
    /// the receiver faults the session instead of answering.
    Empty,
}

impl MemMsg {
    /// For a request, the bytes it touches; `None` for a response.
    ///
    /// The last byte is `addr + len - 1`, computed with checked arithmetic, so a request
    /// may end exactly at `u64::MAX` but never wrap around.
    pub fn access(&self) -> Option<Access> {
        let (addr, len) = match self {
            MemMsg::ReadReq { addr, len, .. } => (*addr, u64::from(*len)),
            MemMsg::WriteReq { addr, data, .. } => (*addr, data.len() as u64),
            MemMsg::ReadResp { .. } | MemMsg::WriteResp { .. } => return None,
        };
        Some(match len.checked_sub(1) {
            None => Access::Empty,
            Some(extra) => match addr.checked_add(extra) {
                Some(last) => Access::Bytes { first: addr, last },
                None => Access::OutOfRange,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(addr: u64, len: u32) -> MemMsg {
        MemMsg::ReadReq {
            txn: TxnId(0),
            addr,
            len,
        }
    }

    fn write(addr: u64, len: usize) -> MemMsg {
        MemMsg::WriteReq {
            txn: TxnId(0),
            addr,
            data: vec![0xAB; len],
        }
    }

    #[test]
    fn zero_length_requests_are_empty_not_faults() {
        assert_eq!(read(0x1000, 0).access(), Some(Access::Empty));
        assert_eq!(write(0x1000, 0).access(), Some(Access::Empty));
        assert_eq!(read(u64::MAX, 0).access(), Some(Access::Empty));
    }

    #[test]
    fn ranges_use_checked_arithmetic() {
        let bytes = |first, last| Some(Access::Bytes { first, last });
        assert_eq!(read(0, 1).access(), bytes(0, 0));
        assert_eq!(
            read(0x8000_0000, 4).access(),
            bytes(0x8000_0000, 0x8000_0003)
        );
        assert_eq!(write(0x10, 2).access(), bytes(0x10, 0x11));
        // Ending exactly at the top of the address space is in range.
        assert_eq!(read(u64::MAX, 1).access(), bytes(u64::MAX, u64::MAX));
        assert_eq!(
            read(u64::MAX - 3, 4).access(),
            bytes(u64::MAX - 3, u64::MAX)
        );
        // One byte further wraps, which is an access fault, not a wrap-around.
        assert_eq!(read(u64::MAX, 2).access(), Some(Access::OutOfRange));
        assert_eq!(read(u64::MAX - 2, 4).access(), Some(Access::OutOfRange));
        assert_eq!(read(u64::MAX, u32::MAX).access(), Some(Access::OutOfRange));
        assert_eq!(write(u64::MAX, 2).access(), Some(Access::OutOfRange));
    }

    #[test]
    fn responses_have_no_access() {
        let data = MemMsg::ReadResp {
            txn: TxnId(0),
            outcome: ReadOutcome::Data { data: vec![1] },
        };
        let done = MemMsg::WriteResp {
            txn: TxnId(0),
            outcome: WriteOutcome::Done,
        };
        assert_eq!(data.access(), None);
        assert_eq!(done.access(), None);
    }
}
