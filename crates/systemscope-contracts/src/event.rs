//! Event ordering and scheduling requests (`docs/m0-design.md` §4).

use core::cmp::Ordering;
use core::fmt;

use crate::error::SimError;
use crate::time::{ClockDomain, ClockDomainId, Duration, SimulationClock, Tick};

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

/// When a scheduled event should run, relative to the current time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum When {
    /// At the current tick.
    Now,
    /// A number of ticks from now.
    Ticks(u64),
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

impl When {
    /// Resolves this request to an absolute tick.
    ///
    /// `domain` looks up clock domains by id and returns `None` for unknown ids.
    pub fn resolve<'a>(
        self,
        now: Tick,
        clock: &SimulationClock,
        domain: impl FnOnce(ClockDomainId) -> Option<&'a ClockDomain>,
    ) -> Result<Tick, SimError> {
        let tick = match self {
            When::Now => now,
            When::Ticks(n) => now.checked_add(n)?,
            When::After(d) => clock.after(now, d)?,
            When::Cycles { domain: id, k } => domain(id)
                .ok_or(SimError::UnknownClockDomain(id))?
                .cycles_after(now, k)?,
        };
        Ok(tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{Frequency, Rounding};

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

    #[test]
    fn when_resolves_each_variant() {
        let clock = SimulationClock::default();
        let cpu = ClockDomain::new(
            &clock,
            ClockDomainId(7),
            Frequency::from_hz(3_000_000_000).unwrap(),
            Tick::ZERO,
            Rounding::Floor,
        )
        .unwrap();
        let lookup = |id: ClockDomainId| (id == cpu.id()).then_some(&cpu);
        let now = Tick(400);

        assert_eq!(When::Now.resolve(now, &clock, lookup), Ok(Tick(400)));
        assert_eq!(When::Ticks(10).resolve(now, &clock, lookup), Ok(Tick(410)));
        assert_eq!(
            When::After(Duration::from_ns(1)).resolve(now, &clock, lookup),
            Ok(Tick(1_400))
        );
        let cycles = When::Cycles {
            domain: ClockDomainId(7),
            k: 1,
        };
        assert_eq!(cycles.resolve(now, &clock, lookup), Ok(Tick(1_000)));

        let unknown = When::Cycles {
            domain: ClockDomainId(8),
            k: 1,
        };
        assert_eq!(
            unknown.resolve(now, &clock, lookup),
            Err(SimError::UnknownClockDomain(ClockDomainId(8)))
        );
        assert!(matches!(
            When::Ticks(u64::MAX).resolve(now, &clock, lookup),
            Err(SimError::Time(_))
        ));
    }
}
