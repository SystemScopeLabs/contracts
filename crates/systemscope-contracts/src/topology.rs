//! Topology declarations shared by components and the runtime (`docs/m0-design.md` §6).

use crate::time::{ClockDomainId, Duration};

/// Extra delay a link adds to every message it carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkLatency {
    /// A physical delay, rounded up to whole ticks.
    After(Duration),
    /// `k` cycles of `domain`, counted from the first edge at or after the send tick.
    Cycles {
        /// The clock domain to count cycles in.
        domain: ClockDomainId,
        /// Number of cycles.
        k: u64,
    },
}
