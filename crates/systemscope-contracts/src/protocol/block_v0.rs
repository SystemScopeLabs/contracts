//! `block.v0`: single-block reads and writes to a block device (`docs/m2-design.md` §8.1).
//!
//! Every message moves one block of [`BLOCK_SIZE`] bytes, addressed by its logical block
//! address (LBA). A request is answered by exactly one result. Commands that span several
//! blocks are orchestrated by the initiator, one block at a time; `block.v0` has no
//! multi-block operation, no queue, and no atomicity across blocks.
//!
//! # Transactions
//!
//! `txn` is allocated by the initiator from its own counter, as in [`mem.v1`](super::mem_v1),
//! and is the same [`TxnId`] type. An initiator never has two requests with one `txn`
//! outstanding. The target echoes the request's `txn` in its result.
//!
//! # Block length is checked by the receiver
//!
//! A block is 512 bytes, so [`BlockMsg::WriteBlock`] and [`BlockReadOutcome::Data`] carry
//! exactly [`BLOCK_SIZE`] bytes. The wire format still encodes `data` as ordinary
//! length-prefixed bytes, and decoding accepts any length: it checks bytes, not meaning.
//! A receiver that gets `data` of any other length must fault the session instead of
//! answering or using it, exactly as a `mem.v1` initiator checks the length of a read's
//! data.
//!
//! # Errors
//!
//! A request answered with an `Error` outcome has not read or written anything. The
//! protocol gives no other meaning to an error: what the initiator does next is its own
//! rule.
//!
//! # Versioning
//!
//! Variant order, field order, and every nested enum are part of the canonical encoding.
//! Any change to them, including a new [`MediaError`] variant, is a new protocol version,
//! never an edit to this one.

use super::ProtocolId;
pub use super::mem::TxnId;

/// The `block.v0` protocol.
pub const PROTOCOL: ProtocolId = ProtocolId {
    name: "block",
    version: 0,
};

/// The number of bytes in one block. The decoder does not enforce it; receivers do.
pub const BLOCK_SIZE: usize = 512;

/// A `block.v0` message. Variant and field order are part of the canonical encoding.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlockMsg {
    /// Read the block at `lba`.
    ReadBlock {
        /// Transaction id, allocated by the initiator.
        txn: TxnId,
        /// Logical block address.
        lba: u64,
    },
    /// Write `data` to the block at `lba`.
    WriteBlock {
        /// Transaction id, allocated by the initiator.
        txn: TxnId,
        /// Logical block address.
        lba: u64,
        /// The block's new contents: exactly [`BLOCK_SIZE`] bytes, which the receiver
        /// checks.
        data: Vec<u8>,
    },
    /// The result of a [`BlockMsg::ReadBlock`].
    ReadResult {
        /// Transaction id of the request.
        txn: TxnId,
        /// The block read, or why the read failed.
        outcome: BlockReadOutcome,
    },
    /// The result of a [`BlockMsg::WriteBlock`].
    WriteResult {
        /// Transaction id of the request.
        txn: TxnId,
        /// Whether the write happened.
        outcome: BlockWriteOutcome,
    },
}

/// What a block read produced.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlockReadOutcome {
    /// The read succeeded.
    Data {
        /// The block's contents: exactly [`BLOCK_SIZE`] bytes, which the receiver checks.
        data: Vec<u8>,
    },
    /// The read failed and returned nothing.
    Error {
        /// Why it failed.
        error: MediaError,
    },
}

/// What a block write did.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BlockWriteOutcome {
    /// The whole block was written.
    Done,
    /// The write failed and changed nothing.
    Error {
        /// Why it failed.
        error: MediaError,
    },
}

/// Why a block request failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MediaError {
    /// The LBA is not below the media's capacity.
    OutOfRange,
    /// The block exists but cannot be read or written.
    BadBlock,
}
