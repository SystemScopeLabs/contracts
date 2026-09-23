//! Simulation errors shared by the runtime and components.

use core::fmt;

use crate::event::Phase;
use crate::time::{ClockDomainId, Tick, TimeError};

/// A fatal simulation error. The run stops and reports it with its diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SimError {
    /// A time computation failed.
    Time(TimeError),
    /// S1: an event was scheduled before the current tick.
    PastTick {
        /// The current tick.
        now: Tick,
        /// The tick that was requested.
        requested: Tick,
    },
    /// S2/S3: an event was scheduled into an earlier phase of the current tick, or into
    /// [`Phase::Observe`].
    PhaseViolation {
        /// The current tick.
        now: Tick,
        /// The phase currently running.
        current: Phase,
        /// The requested tick.
        tick: Tick,
        /// The requested phase.
        requested: Phase,
    },
    /// S5: more than the allowed number of events ran in one `(tick, phase)`.
    SameTickLivelock {
        /// The tick that did not make progress.
        tick: Tick,
        /// The phase that did not make progress.
        phase: Phase,
        /// The configured limit.
        limit: u64,
    },
    /// S6: the global event sequence counter is exhausted.
    SequenceOverflow,
    /// A clock domain id did not name a domain in this session.
    UnknownClockDomain(ClockDomainId),
}

impl From<TimeError> for SimError {
    fn from(e: TimeError) -> SimError {
        SimError::Time(e)
    }
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimError::Time(e) => write!(f, "time error: {e}"),
            SimError::PastTick { now, requested } => {
                write!(f, "S1: scheduled at tick {requested}, before now ({now})")
            }
            SimError::PhaseViolation {
                now,
                current,
                tick,
                requested,
            } => write!(
                f,
                "phase violation: scheduled {requested} at tick {tick} while running \
                 {current} at tick {now}"
            ),
            SimError::SameTickLivelock { tick, phase, limit } => {
                write!(f, "S5: more than {limit} events in {phase} at tick {tick}")
            }
            SimError::SequenceOverflow => f.write_str("S6: event sequence counter exhausted"),
            SimError::UnknownClockDomain(id) => write!(f, "unknown clock domain {}", id.0),
        }
    }
}

impl std::error::Error for SimError {}
