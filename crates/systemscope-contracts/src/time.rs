//! Logical time: ticks, physical durations, and clock domains (`docs/m0-design.md` §3).
//!
//! [`Tick`] is the only authority for simulated time. Nothing in this module reads host
//! wall-clock time or uses floating point.

use core::fmt;

/// Femtoseconds per second, the fixed precision of [`Duration`].
const FS_PER_SECOND: u128 = 1_000_000_000_000_000;

/// Errors produced by time computations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TimeError {
    /// A computed time does not fit in a `u64` tick. Simulated time never wraps.
    TimeOverflow,
    /// `ticks_per_second` was zero.
    ZeroResolution,
    /// A frequency numerator or denominator was zero.
    ZeroFrequency,
    /// Adjacent edges of a clock cannot be distinguished at the session resolution,
    /// because its period is shorter than one tick.
    FrequencyAboveResolution,
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            TimeError::TimeOverflow => "simulated time overflowed u64 ticks",
            TimeError::ZeroResolution => "ticks_per_second must be non-zero",
            TimeError::ZeroFrequency => "frequency numerator and denominator must be non-zero",
            TimeError::FrequencyAboveResolution => {
                "adjacent clock edges cannot be distinguished at this tick resolution"
            }
        };
        f.write_str(msg)
    }
}

impl std::error::Error for TimeError {}

/// A point in simulated time, counted in ticks since the start of the session.
///
/// The physical length of a tick is fixed per session by [`SimulationClock`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub u64);

impl Tick {
    /// The start of simulated time.
    pub const ZERO: Tick = Tick(0);

    /// Returns `self + ticks`, or [`TimeError::TimeOverflow`] if the result does not fit.
    pub fn checked_add(self, ticks: u64) -> Result<Tick, TimeError> {
        self.0
            .checked_add(ticks)
            .map(Tick)
            .ok_or(TimeError::TimeOverflow)
    }
}

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A physical length of time, independent of the session's tick resolution.
///
/// Stored as an exact number of femtoseconds. Convert to ticks with
/// [`SimulationClock::ticks_for`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration {
    femtoseconds: u128,
}

impl Duration {
    /// A zero-length duration.
    pub const ZERO: Duration = Duration { femtoseconds: 0 };

    /// Creates a duration of `fs` femtoseconds.
    pub const fn from_fs(fs: u128) -> Duration {
        Duration { femtoseconds: fs }
    }

    /// Creates a duration of `ps` picoseconds.
    pub const fn from_ps(ps: u64) -> Duration {
        Duration::from_fs(ps as u128 * 1_000)
    }

    /// Creates a duration of `ns` nanoseconds.
    pub const fn from_ns(ns: u64) -> Duration {
        Duration::from_fs(ns as u128 * 1_000_000)
    }

    /// Creates a duration of `us` microseconds.
    pub const fn from_us(us: u64) -> Duration {
        Duration::from_fs(us as u128 * 1_000_000_000)
    }

    /// Creates a duration of `ms` milliseconds.
    pub const fn from_ms(ms: u64) -> Duration {
        Duration::from_fs(ms as u128 * 1_000_000_000_000)
    }

    /// Creates a duration of `s` seconds.
    pub const fn from_s(s: u64) -> Duration {
        Duration::from_fs(s as u128 * FS_PER_SECOND)
    }

    /// Returns the exact length in femtoseconds.
    pub const fn as_femtoseconds(self) -> u128 {
        self.femtoseconds
    }

    /// Returns `self + other`, or `None` on overflow.
    pub fn checked_add(self, other: Duration) -> Option<Duration> {
        self.femtoseconds
            .checked_add(other.femtoseconds)
            .map(Duration::from_fs)
    }
}

/// The tick resolution of a simulation session.
///
/// Chosen once when a session starts and fixed for its lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SimulationClock {
    ticks_per_second: u64,
}

impl SimulationClock {
    /// Default resolution: one tick per picosecond.
    pub const DEFAULT_TICKS_PER_SECOND: u64 = 1_000_000_000_000;

    /// Creates a clock with the given resolution.
    pub fn new(ticks_per_second: u64) -> Result<SimulationClock, TimeError> {
        if ticks_per_second == 0 {
            return Err(TimeError::ZeroResolution);
        }
        Ok(SimulationClock { ticks_per_second })
    }

    /// Returns the number of ticks in one simulated second.
    pub const fn ticks_per_second(&self) -> u64 {
        self.ticks_per_second
    }

    /// Converts a duration to ticks, rounding up so a latency is never shortened.
    pub fn ticks_for(&self, duration: Duration) -> Result<u64, TimeError> {
        let tps = u128::from(self.ticks_per_second);
        let fs = duration.as_femtoseconds();
        // ceil(fs × tps / 10^15) = (fs / 10^15) × tps + ceil((fs % 10^15) × tps / 10^15).
        // The second product is below 10^15 × 2^64 and cannot overflow; the first can, and
        // then the result would not fit in a tick anyway.
        let whole = (fs / FS_PER_SECOND).checked_mul(tps);
        let frac = (fs % FS_PER_SECOND)
            .checked_mul(tps)
            .map(|x| x.div_ceil(FS_PER_SECOND));
        let ticks = whole
            .zip(frac)
            .and_then(|(w, f)| w.checked_add(f))
            .ok_or(TimeError::TimeOverflow)?;
        u64::try_from(ticks).map_err(|_| TimeError::TimeOverflow)
    }

    /// Returns the tick `duration` after `now`, rounding the duration up.
    pub fn after(&self, now: Tick, duration: Duration) -> Result<Tick, TimeError> {
        now.checked_add(self.ticks_for(duration)?)
    }
}

impl Default for SimulationClock {
    fn default() -> SimulationClock {
        SimulationClock {
            ticks_per_second: SimulationClock::DEFAULT_TICKS_PER_SECOND,
        }
    }
}

/// An exact frequency in hertz, expressed as the ratio `num / den`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frequency {
    num: u64,
    den: u64,
}

impl Frequency {
    /// Creates the frequency `num / den` Hz.
    pub fn new(num: u64, den: u64) -> Result<Frequency, TimeError> {
        if num == 0 || den == 0 {
            return Err(TimeError::ZeroFrequency);
        }
        Ok(Frequency { num, den })
    }

    /// Creates a whole-number frequency in hertz.
    pub fn from_hz(hz: u64) -> Result<Frequency, TimeError> {
        Frequency::new(hz, 1)
    }

    /// Returns the numerator in hertz.
    pub const fn num(&self) -> u64 {
        self.num
    }

    /// Returns the denominator.
    pub const fn den(&self) -> u64 {
        self.den
    }
}

/// How a clock edge that falls between two ticks is placed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Rounding {
    /// Place the edge on the tick at or before its exact time.
    #[default]
    Floor,
    /// Place the edge on the tick at or after its exact time.
    Ceil,
}

/// Identifies a clock domain within a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockDomainId(pub u32);

/// A clock with an exact rational frequency, projected onto session ticks.
///
/// Edge `n` is computed absolutely as
/// `offset + round(n × ticks_per_second × den / num)`, never by accumulating a rounded
/// period, so long runs do not drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClockDomain {
    id: ClockDomainId,
    frequency: Frequency,
    offset: Tick,
    edge_rounding: Rounding,
    /// Period in ticks as the reduced fraction `period_num / period_den`.
    ///
    /// `period_num = ticks_per_second × den / g` can need up to 128 bits.
    /// `period_den = num / g` always fits in 64 bits because `g ≥ 1`.
    period_num: u128,
    period_den: u64,
}

impl ClockDomain {
    /// Creates a clock domain whose edge 0 is at `offset`.
    ///
    /// Fails with [`TimeError::FrequencyAboveResolution`] if adjacent edges cannot be
    /// distinguished at `clock`'s resolution, i.e. the period is shorter than one tick.
    pub fn new(
        clock: &SimulationClock,
        id: ClockDomainId,
        frequency: Frequency,
        offset: Tick,
        edge_rounding: Rounding,
    ) -> Result<ClockDomain, TimeError> {
        // period = ticks_per_second / (num / den) = ticks_per_second × den / num.
        // A product of two u64 values always fits in u128.
        let num = u128::from(clock.ticks_per_second()) * u128::from(frequency.den());
        let den = u128::from(frequency.num());
        let g = gcd(num, den);
        let period_num = num / g;
        let period_den = u64::try_from(den / g).expect("num / g <= num fits in u64");
        if period_num < u128::from(period_den) {
            return Err(TimeError::FrequencyAboveResolution);
        }
        Ok(ClockDomain {
            id,
            frequency,
            offset,
            edge_rounding,
            period_num,
            period_den,
        })
    }

    /// Returns the domain's identifier.
    pub const fn id(&self) -> ClockDomainId {
        self.id
    }

    /// Returns the domain's exact frequency.
    pub const fn frequency(&self) -> Frequency {
        self.frequency
    }

    /// Returns the tick of edge 0.
    pub const fn offset(&self) -> Tick {
        self.offset
    }

    /// Returns how edges between ticks are placed.
    pub const fn edge_rounding(&self) -> Rounding {
        self.edge_rounding
    }

    /// Returns the tick of edge `n`.
    pub fn edge(&self, n: u64) -> Result<Tick, TimeError> {
        let q = u128::from(self.period_den);
        let (p_whole, p_rem) = (self.period_num / q, self.period_num % q);
        let n = u128::from(n);
        // n × p / q = n × (p / q) + n × (p % q) / q; only the second term needs rounding.
        // n × (p % q) < 2^64 × 2^64 fits; n × (p / q) may overflow, and then so does the
        // result.
        let whole = n.checked_mul(p_whole);
        let frac = n.checked_mul(p_rem).map(|x| match self.edge_rounding {
            Rounding::Floor => x / q,
            Rounding::Ceil => x.div_ceil(q),
        });
        let tick = whole
            .zip(frac)
            .and_then(|(w, f)| w.checked_add(f))
            .and_then(|t| t.checked_add(u128::from(self.offset.0)))
            .ok_or(TimeError::TimeOverflow)?;
        u64::try_from(tick)
            .map(Tick)
            .map_err(|_| TimeError::TimeOverflow)
    }

    /// Returns the smallest `n` such that `edge(n) >= t`.
    pub fn next_edge_index(&self, t: Tick) -> Result<u64, TimeError> {
        let d = match t.0.checked_sub(self.offset.0) {
            Some(d) if d > 0 => u128::from(d),
            _ => return Ok(0),
        };
        let p = self.period_num;
        let q = u128::from(self.period_den);
        // d × q < 2^64 × 2^64 fits; since p >= q, the result is at most d + 1.
        let n = match self.edge_rounding {
            // floor(n·p/q) >= d  ⇔  n >= d·q/p
            Rounding::Floor => d.checked_mul(q).map(|x| x.div_ceil(p)),
            // ceil(n·p/q) >= d  ⇔  n·p/q > d − 1  ⇔  n > (d − 1)·q/p
            Rounding::Ceil => (d - 1).checked_mul(q).and_then(|x| (x / p).checked_add(1)),
        };
        n.and_then(|n| u64::try_from(n).ok())
            .ok_or(TimeError::TimeOverflow)
    }

    /// Returns the tick `k` cycles after the first edge at or after `now`.
    ///
    /// `k = 0` aligns `now` to the next edge.
    pub fn cycles_after(&self, now: Tick, k: u64) -> Result<Tick, TimeError> {
        let n = self
            .next_edge_index(now)?
            .checked_add(k)
            .ok_or(TimeError::TimeOverflow)?;
        self.edge(n)
    }
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    const PS: SimulationClock = SimulationClock {
        ticks_per_second: SimulationClock::DEFAULT_TICKS_PER_SECOND,
    };

    fn domain(num: u64, den: u64, offset: u64, rounding: Rounding) -> ClockDomain {
        let freq = Frequency::new(num, den).unwrap();
        ClockDomain::new(&PS, ClockDomainId(0), freq, Tick(offset), rounding).unwrap()
    }

    fn edges(d: &ClockDomain, count: u64) -> Vec<u64> {
        (0..count).map(|n| d.edge(n).unwrap().0).collect()
    }

    #[test]
    fn three_ghz_edges_do_not_drift() {
        let d = domain(3_000_000_000, 1, 0, Rounding::Floor);
        assert_eq!(edges(&d, 7), [0, 333, 666, 1000, 1333, 1666, 2000]);
        assert_eq!(d.edge(3_000_000_000).unwrap(), Tick(1_000_000_000_000));
    }

    #[test]
    fn fractional_frequency_uses_denominator() {
        // 1.5 GHz: period = 2000/3 ps
        let floor = domain(3_000_000_000, 2, 0, Rounding::Floor);
        assert_eq!(edges(&floor, 4), [0, 666, 1333, 2000]);
        let ceil = domain(3_000_000_000, 2, 0, Rounding::Ceil);
        assert_eq!(edges(&ceil, 4), [0, 667, 1334, 2000]);
    }

    #[test]
    fn offset_shifts_every_edge() {
        let d = domain(1_000_000_000, 1, 250, Rounding::Floor);
        assert_eq!(edges(&d, 3), [250, 1250, 2250]);
        assert_eq!(d.next_edge_index(Tick(0)).unwrap(), 0);
        assert_eq!(d.next_edge_index(Tick(250)).unwrap(), 0);
        assert_eq!(d.next_edge_index(Tick(251)).unwrap(), 1);
    }

    #[test]
    fn next_edge_index_at_and_around_edges() {
        for rounding in [Rounding::Floor, Rounding::Ceil] {
            let d = domain(3_000_000_000, 1, 0, rounding);
            for n in 1..100 {
                let e = d.edge(n).unwrap().0;
                assert_eq!(d.next_edge_index(Tick(e)).unwrap(), n);
                assert_eq!(d.next_edge_index(Tick(e - 1)).unwrap(), n);
                assert_eq!(d.next_edge_index(Tick(e + 1)).unwrap(), n + 1);
            }
        }
    }

    #[test]
    fn cycles_after_counts_from_next_edge() {
        let d = domain(3_000_000_000, 1, 0, Rounding::Floor);
        assert_eq!(d.cycles_after(Tick(333), 0).unwrap(), Tick(333));
        assert_eq!(d.cycles_after(Tick(333), 1).unwrap(), Tick(666));
        assert_eq!(d.cycles_after(Tick(400), 0).unwrap(), Tick(666));
        assert_eq!(d.cycles_after(Tick(400), 3).unwrap(), Tick(1666));
    }

    #[test]
    fn frequency_above_resolution_is_rejected() {
        let ns = SimulationClock::new(1_000_000_000).unwrap();
        let freq = Frequency::from_hz(3_000_000_000).unwrap();
        let err = ClockDomain::new(&ns, ClockDomainId(0), freq, Tick::ZERO, Rounding::Floor);
        assert_eq!(err, Err(TimeError::FrequencyAboveResolution));
        // Exactly one tick per cycle is allowed.
        let freq = Frequency::from_hz(1_000_000_000).unwrap();
        assert!(ClockDomain::new(&ns, ClockDomainId(0), freq, Tick::ZERO, Rounding::Floor).is_ok());
    }

    #[test]
    fn invalid_parameters_are_rejected() {
        assert_eq!(SimulationClock::new(0), Err(TimeError::ZeroResolution));
        assert_eq!(Frequency::new(0, 1), Err(TimeError::ZeroFrequency));
        assert_eq!(Frequency::new(1, 0), Err(TimeError::ZeroFrequency));
    }

    #[test]
    fn edge_overflow_is_an_error() {
        let d = domain(1_000_000_000, 1, 0, Rounding::Floor);
        assert_eq!(d.edge(u64::MAX), Err(TimeError::TimeOverflow));
        assert_eq!(
            d.cycles_after(Tick(u64::MAX - 10), 1),
            Err(TimeError::TimeOverflow)
        );
    }

    #[test]
    fn period_numerator_wider_than_u64_is_accepted() {
        // ~8.6 GHz at 1 ps: period ≈ 116 ticks, but period_num = 10^12 × 2^30 > u64::MAX.
        let num = (1u64 << 63) - 25;
        let freq = Frequency::new(num, 1 << 30).unwrap();
        let d = ClockDomain::new(&PS, ClockDomainId(0), freq, Tick::ZERO, Rounding::Floor).unwrap();
        let p = 1_000_000_000_000u128 << 30;
        assert!(p > u128::from(u64::MAX));
        for n in [1u64, 2, 3, 1_000, 1_000_000] {
            let expected = u64::try_from(u128::from(n) * p / u128::from(num)).unwrap();
            assert_eq!(d.edge(n).unwrap(), Tick(expected));
        }
    }

    #[test]
    fn edge_reaches_exactly_u64_max() {
        // Period of exactly one tick: edge(n) = offset + n.
        let d = domain(1_000_000_000_000, 1, 0, Rounding::Floor);
        assert_eq!(d.edge(u64::MAX), Ok(Tick(u64::MAX)));
        assert_eq!(d.next_edge_index(Tick(u64::MAX)), Ok(u64::MAX));

        let shifted = domain(1_000_000_000_000, 1, 1, Rounding::Ceil);
        assert_eq!(shifted.edge(u64::MAX - 1), Ok(Tick(u64::MAX)));
        assert_eq!(shifted.edge(u64::MAX), Err(TimeError::TimeOverflow));
        assert_eq!(
            shifted.cycles_after(Tick(u64::MAX), 1),
            Err(TimeError::TimeOverflow)
        );
    }

    #[test]
    fn extreme_parameters_do_not_panic() {
        let max = SimulationClock::new(u64::MAX).unwrap();
        for (num, den) in [(1, u64::MAX), (u64::MAX, u64::MAX), (u64::MAX, 1), (1, 1)] {
            let freq = Frequency::new(num, den).unwrap();
            for rounding in [Rounding::Floor, Rounding::Ceil] {
                let d = ClockDomain::new(&max, ClockDomainId(0), freq, Tick(u64::MAX), rounding)
                    .unwrap();
                assert_eq!(d.edge(0), Ok(Tick(u64::MAX)));
                let _ = d.edge(u64::MAX);
                let _ = d.next_edge_index(Tick(u64::MAX));
                let _ = d.cycles_after(Tick(u64::MAX), u64::MAX);
            }
        }
        let _ = max.ticks_for(Duration::from_fs(u128::MAX));
    }

    #[test]
    fn duration_reaches_exactly_u64_max() {
        // 1 tick = 1 fs, so ticks equal femtoseconds.
        let fs = SimulationClock::new(1_000_000_000_000_000).unwrap();
        let max = u128::from(u64::MAX);
        assert_eq!(fs.ticks_for(Duration::from_fs(max)), Ok(u64::MAX));
        assert_eq!(
            fs.ticks_for(Duration::from_fs(max + 1)),
            Err(TimeError::TimeOverflow)
        );
    }

    #[test]
    fn duration_rounds_up_to_ticks() {
        assert_eq!(PS.ticks_for(Duration::ZERO).unwrap(), 0);
        assert_eq!(PS.ticks_for(Duration::from_ns(50)).unwrap(), 50_000);
        assert_eq!(PS.ticks_for(Duration::from_us(100)).unwrap(), 100_000_000);
        assert_eq!(PS.ticks_for(Duration::from_fs(1)).unwrap(), 1);
        assert_eq!(PS.ticks_for(Duration::from_fs(1_001)).unwrap(), 2);

        let ns = SimulationClock::new(1_000_000_000).unwrap();
        assert_eq!(ns.ticks_for(Duration::from_ps(1)).unwrap(), 1);
        assert_eq!(ns.ticks_for(Duration::from_ps(1_000)).unwrap(), 1);
        assert_eq!(ns.ticks_for(Duration::from_ps(1_001)).unwrap(), 2);
    }

    #[test]
    fn duration_overflow_is_an_error() {
        // u64::MAX ps is ~213 days; one more second does not fit.
        let too_long = Duration::from_s(213 * 24 * 3600 + 86_400);
        assert_eq!(PS.ticks_for(too_long), Err(TimeError::TimeOverflow));
        assert_eq!(
            PS.after(Tick(u64::MAX), Duration::from_fs(1)),
            Err(TimeError::TimeOverflow)
        );
        assert_eq!(
            PS.ticks_for(Duration::from_fs(u128::MAX)),
            Err(TimeError::TimeOverflow)
        );
    }

    #[test]
    fn tick_add_never_wraps() {
        assert_eq!(Tick(5).checked_add(7), Ok(Tick(12)));
        assert_eq!(Tick(u64::MAX).checked_add(1), Err(TimeError::TimeOverflow));
    }
}
