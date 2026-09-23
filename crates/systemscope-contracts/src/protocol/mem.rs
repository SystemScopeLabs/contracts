//! `mem.v0`: memory read/write transactions.

use super::ProtocolId;

/// The `mem.v0` protocol.
pub const PROTOCOL: ProtocolId = ProtocolId {
    name: "mem",
    version: 0,
};

/// Identifies a transaction. Allocated by the initiator from its own counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TxnId(pub u64);

/// A `mem.v0` message. Variant and field order are part of the canonical encoding.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MemMsg {
    /// Read `len` bytes at `addr`.
    ReadReq {
        /// Transaction id.
        txn: TxnId,
        /// Byte address.
        addr: u64,
        /// Number of bytes.
        len: u32,
    },
    /// Data returned for a read.
    ReadResp {
        /// Transaction id of the request.
        txn: TxnId,
        /// The bytes read.
        data: Vec<u8>,
    },
    /// Write `data` at `addr`.
    WriteReq {
        /// Transaction id.
        txn: TxnId,
        /// Byte address.
        addr: u64,
        /// The bytes to write.
        data: Vec<u8>,
    },
    /// Acknowledges a write.
    WriteResp {
        /// Transaction id of the request.
        txn: TxnId,
    },
}
