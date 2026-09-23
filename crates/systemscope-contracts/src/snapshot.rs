//! Snapshot contracts (`docs/m0-design.md` §7).
//!
//! A component writes its state with the §4.5 primitive rules and reads it back strictly.
//! Restoring must reproduce exactly the state that was written: the round-trip law is
//! `encode(restore(decode(encode(s)))) == encode(s)`.

use core::fmt;

use crate::canonical::{DecodeError, Decoder, Encoder};
use crate::component::ComponentId;

/// Where a component writes its snapshot.
pub type SnapshotWriter = Encoder;

/// Where a component reads its snapshot back. Strict: see [`Decoder`].
pub type SnapshotReader<'a> = Decoder<'a>;

/// A session setting a snapshot records and restore checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionField {
    /// The session seed.
    Seed,
    /// The tick resolution.
    TicksPerSecond,
    /// The S5 limit.
    MaxEventsPerPhase,
    /// The contracts version.
    ContractsVersion,
    /// The clock domains: ids, frequencies, offsets, and rounding.
    ClockDomains,
}

/// Why a snapshot could not be restored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RestoreError {
    /// The bytes are not a strict canonical encoding.
    Decode(DecodeError),
    /// The bytes do not start with the snapshot magic.
    BadMagic,
    /// The snapshot format version is not supported.
    FormatVersion(u32),
    /// A session setting differs from the resuming session's.
    SessionMismatch(SessionField),
    /// The topology differs from the resuming session's.
    TopologyMismatch,
    /// A component's snapshot schema differs from the one it now writes.
    SchemaVersion {
        /// The component.
        component: ComponentId,
        /// The schema the component writes now.
        expected: u32,
        /// The schema recorded in the snapshot.
        found: u32,
    },
    /// The snapshot describes a state no valid run could produce.
    InvalidState(&'static str),
}

impl From<DecodeError> for RestoreError {
    fn from(e: DecodeError) -> RestoreError {
        RestoreError::Decode(e)
    }
}

impl fmt::Display for RestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RestoreError::Decode(e) => write!(f, "malformed snapshot: {e}"),
            RestoreError::BadMagic => f.write_str("not a snapshot"),
            RestoreError::FormatVersion(v) => write!(f, "unsupported snapshot format {v}"),
            RestoreError::SessionMismatch(field) => {
                write!(f, "session setting {field:?} differs from the snapshot")
            }
            RestoreError::TopologyMismatch => f.write_str("topology differs from the snapshot"),
            RestoreError::SchemaVersion {
                component,
                expected,
                found,
            } => write!(
                f,
                "component {} writes schema {expected}, snapshot has {found}",
                component.0
            ),
            RestoreError::InvalidState(what) => write!(f, "invalid snapshot state: {what}"),
        }
    }
}

impl std::error::Error for RestoreError {}
