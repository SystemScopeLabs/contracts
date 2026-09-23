//! Contracts shared by the SystemScope runtime and every component backend.
//!
//! See `docs/m0-design.md` in `SystemScope/systemscope` for the specification.

pub mod canonical;
pub mod component;
pub mod error;
pub mod event;
pub mod protocol;
pub mod rng;
pub mod snapshot;
pub mod time;
pub mod topology;
pub mod trace;
