//! Event ordering and scheduling requests (`docs/m0-design.md` §4).

use core::cmp::Ordering;
use core::fmt;

use crate::time::{ClockDomainId, Duration, Tick};

/// The fixed, runtime-defined order of work within a single tick.
///
/// Phases run in declaration order. Components may schedule into every phase except
/// [`Phase::Observe`], which belongs to the runtime and read-only observers.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Initiators start transactions.
    Request = 0,
    /// Links and interconnects move and arbitrate messages.
    Transfer = 1,
    /// Targets finish transactions and send responses.
    Complete = 2,
    /// Architecturally visible state updates, such as retirement.
    Commit = 3,
    /// Runtime only: observers read state; nothing may be mutated.
    Observe = 4,
}

impl Phase {
    /// All phases in execution order.
    pub const ALL: [Phase; 5] = [
        Phase::Request,
        Phase::Transfer,
        Phase::Complete,
        Phase::Commit,
        Phase::Observe,
    ];
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Phase::Request => "REQUEST",
            Phase::Transfer => "TRANSFER",
            Phase::Complete => "COMPLETE",
            Phase::Commit => "COMMIT",
            Phase::Observe => "OBSERVE",
        };
        f.write_str(name)
    }
}

/// The total order of events: by tick, then phase, then runtime-assigned sequence.
///
/// `sequence` is unique within a session, so no two scheduled events compare equal and
/// the dispatch order never depends on how a queue breaks ties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventKey {
    /// When the event runs.
    pub tick: Tick,
    /// Where in the tick the event runs.
    pub phase: Phase,
    /// Global scheduling order, assigned by the runtime and never by components.
    pub sequence: u64,
}

impl Ord for EventKey {
    fn cmp(&self, other: &EventKey) -> Ordering {
        self.tick
            .cmp(&other.tick)
            .then(self.phase.cmp(&other.phase))
            .then(self.sequence.cmp(&other.sequence))
    }
}

impl PartialOrd for EventKey {
    fn partial_cmp(&self, other: &EventKey) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// When a component wants an event to run, relative to the current time.
///
/// There is deliberately no tick variant: components never handle ticks, so their
/// behavior does not change with the session resolution. Only the runtime resolves this
/// to an absolute [`Tick`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScheduleWhen {
    /// At the current tick (tick 0 during `init`).
    Now,
    /// A physical duration from now, rounded up to whole ticks.
    After(Duration),
    /// `k` cycles after the first edge of `domain` at or after now.
    Cycles {
        /// The clock domain to count cycles in.
        domain: ClockDomainId,
        /// Number of cycles; `0` aligns to the next edge.
        k: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(tick: u64, phase: Phase, sequence: u64) -> EventKey {
        EventKey {
            tick: Tick(tick),
            phase,
            sequence,
        }
    }

    #[test]
    fn phases_are_ordered_by_declaration() {
        assert!(Phase::ALL.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn tick_dominates_phase_and_sequence() {
        assert!(key(1, Phase::Observe, 99) < key(2, Phase::Request, 0));
    }

    #[test]
    fn phase_dominates_sequence() {
        assert!(key(5, Phase::Request, 99) < key(5, Phase::Transfer, 0));
    }

    #[test]
    fn sequence_breaks_remaining_ties_ascending() {
        assert!(key(5, Phase::Commit, 1) < key(5, Phase::Commit, 2));
        assert_eq!(
            key(5, Phase::Commit, 1).cmp(&key(5, Phase::Commit, 1)),
            Ordering::Equal
        );
    }
}
