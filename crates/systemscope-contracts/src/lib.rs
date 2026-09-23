//! Contracts shared by the SystemScope runtime and every component backend.
//!
//! See `docs/m0-design.md` in `SystemScopeLabs/systemscope` for the specification.

pub mod canonical;
pub mod component;
pub mod error;
pub mod event;
pub mod observe;
pub mod protocol;
pub mod rng;
pub mod snapshot;
pub mod time;
pub mod topology;
pub mod trace;

/// Identifies which snapshots and traces can be restored or resumed together.
///
/// It is written as the `contracts_version` field of every snapshot's session info and
/// every trace header, so it feeds `StateDigest` and `TraceDigest`. It is deliberately
/// not the crate's Cargo version: the crate follows SemVer and may change freely, while
/// this id changes only when snapshots or traces made under one value can no longer be
/// restored or resumed under the other, such as when an existing encoding changes. Adding
/// a protocol, as `mem.v1` did, does not change it. Changing it re-blesses every golden
/// file (`docs/m1-design.md` §4.5).
///
/// The value looks like a version but is compared only for equality.
pub const COMPATIBILITY_ID: &str = "0.0.0";
