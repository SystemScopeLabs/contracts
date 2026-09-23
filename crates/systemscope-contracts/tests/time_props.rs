//! Property tests for the time model (`docs/m0-design.md` §3 and §9, supporting tests).
//!
//! Results are compared against an exact 256-bit oracle, so overflow boundaries are checked
//! precisely: an implementation must return `TimeOverflow` exactly when the true result
//! does not fit in a `u64` tick. Tests run in debug builds, where any unchecked integer
//! overflow panics.

use proptest::prelude::*;
use systemscope_contracts::time::{
    ClockDomain, ClockDomainId, Duration, Frequency, Rounding, SimulationClock, Tick, TimeError,
};

const FS_PER_SECOND: u64 = 1_000_000_000_000_000;

/// Unsigned 256-bit integer as little-endian u64 limbs, used only as a test oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct U256([u64; 4]);

impl U256 {
    fn from_u128(x: u128) -> U256 {
        U256([x as u64, (x >> 64) as u64, 0, 0])
    }

    fn mul_u64(self, m: u64) -> U256 {
        let mut out = [0u64; 4];
        let mut carry = 0u128;
        for (o, limb) in out.iter_mut().zip(self.0) {
            let x = u128::from(limb) * u128::from(m) + carry;
            *o = x as u64;
            carry = x >> 64;
        }
        assert_eq!(carry, 0, "oracle overflowed 256 bits");
        U256(out)
    }

    /// Returns `(self / d, self % d)`.
    fn div_rem_u64(self, d: u64) -> (U256, u64) {
        let mut out = [0u64; 4];
        let mut rem = 0u128;
        for i in (0..4).rev() {
            let cur = (rem << 64) | u128::from(self.0[i]);
            out[i] = (cur / u128::from(d)) as u64;
            rem = cur % u128::from(d);
        }
        (U256(out), rem as u64)
    }

    fn add_u64(self, a: u64) -> U256 {
        let mut out = self.0;
        let mut carry = u128::from(a);
        for o in out.iter_mut() {
            let x = u128::from(*o) + carry;
            *o = x as u64;
            carry = x >> 64;
        }
        assert_eq!(carry, 0, "oracle overflowed 256 bits");
        U256(out)
    }

    fn to_u64(self) -> Option<u64> {
        (self.0[1..] == [0, 0, 0]).then_some(self.0[0])
    }
}

/// Divides and rounds as `rounding` says.
fn div_round(x: U256, d: u64, rounding: Rounding) -> U256 {
    let (q, r) = x.div_rem_u64(d);
    match rounding {
        Rounding::Ceil if r > 0 => q.add_u64(1),
        _ => q,
    }
}

#[derive(Debug, Clone)]
struct Setup {
    clock: SimulationClock,
    domain: ClockDomain,
    num: u64,
    den: u64,
    offset: u64,
    rounding: Rounding,
}

/// Exact edge from the definition `offset + round(n × tps × den / num)`, or `None` if it
/// does not fit in a tick.
fn oracle_edge(s: &Setup, n: u64) -> Option<u64> {
    let exact = U256::from_u128(u128::from(n))
        .mul_u64(s.clock.ticks_per_second())
        .mul_u64(s.den);
    div_round(exact, s.num, s.rounding)
        .add_u64(s.offset)
        .to_u64()
}

/// Exact `ceil(fs × tps / 10^15)`, or `None` if it does not fit in a tick.
fn oracle_ticks_for(tps: u64, fs: u128) -> Option<u64> {
    let exact = U256::from_u128(fs).mul_u64(tps);
    div_round(exact, FS_PER_SECOND, Rounding::Ceil).to_u64()
}

fn rounding() -> impl Strategy<Value = Rounding> {
    prop_oneof![Just(Rounding::Floor), Just(Rounding::Ceil)]
}

/// Non-zero u64 biased towards small values and both ends of the range.
fn wide_nonzero() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(1u64),
        Just(u64::MAX),
        Just(u64::MAX - 1),
        1u64..=1 << 20,
        1u64..=1 << 44,
        1u64..=u64::MAX,
    ]
}

fn wide_u64() -> impl Strategy<Value = u64> {
    prop_oneof![Just(0u64), wide_nonzero()]
}

fn wide_u128() -> impl Strategy<Value = u128> {
    prop_oneof![
        Just(0u128),
        Just(u128::MAX),
        Just(u128::from(u64::MAX)),
        0u128..=1 << 70,
        any::<u128>(),
    ]
}

fn build((tps, num, den, offset, rounding): (u64, u64, u64, u64, Rounding)) -> Option<Setup> {
    let clock = SimulationClock::new(tps).unwrap();
    let freq = Frequency::new(num, den).unwrap();
    let domain = ClockDomain::new(&clock, ClockDomainId(0), freq, Tick(offset), rounding).ok()?;
    Some(Setup {
        clock,
        domain,
        num,
        den,
        offset,
        rounding,
    })
}

/// Realistic clocks, where most edges are representable.
fn setup() -> impl Strategy<Value = Setup> {
    (
        1u64..=1 << 44,
        1u64..=1 << 40,
        1u64..=1 << 20,
        0u64..=1 << 40,
        rounding(),
    )
        .prop_filter_map("clock faster than tick resolution", build)
}

/// Extreme parameters across the whole u64 range.
fn wide_setup() -> impl Strategy<Value = Setup> {
    (
        wide_nonzero(),
        wide_nonzero(),
        wide_nonzero(),
        wide_u64(),
        rounding(),
    )
        .prop_filter_map("clock faster than tick resolution", build)
}

fn any_setup() -> impl Strategy<Value = Setup> {
    prop_oneof![setup(), wide_setup()]
}

fn edge_index() -> impl Strategy<Value = u64> {
    prop_oneof![0u64..=1 << 40, wide_u64()]
}

fn tick_value() -> impl Strategy<Value = u64> {
    prop_oneof![0u64..=1 << 50, wide_u64()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn edge_matches_exact_definition(s in any_setup(), n in edge_index()) {
        prop_assert_eq!(s.domain.edge(n).ok().map(|t| t.0), oracle_edge(&s, n));
    }

    #[test]
    fn clock_domain_rejects_only_sub_tick_periods(
        tps in wide_nonzero(),
        num in wide_nonzero(),
        den in wide_nonzero(),
        offset in wide_u64(),
        rounding in rounding(),
    ) {
        let clock = SimulationClock::new(tps).unwrap();
        let freq = Frequency::new(num, den).unwrap();
        let result = ClockDomain::new(&clock, ClockDomainId(0), freq, Tick(offset), rounding);
        // Period ≥ 1 tick  ⇔  tps × den ≥ num.
        if u128::from(tps) * u128::from(den) >= u128::from(num) {
            prop_assert!(result.is_ok());
        } else {
            prop_assert_eq!(result, Err(TimeError::FrequencyAboveResolution));
        }
    }

    #[test]
    fn edges_are_strictly_increasing(s in any_setup(), n in edge_index()) {
        let (Ok(a), Ok(b)) = (s.domain.edge(n), s.domain.edge(n.saturating_add(1))) else {
            return Ok(());
        };
        prop_assert!(n == u64::MAX || a < b);
    }

    #[test]
    fn whole_seconds_land_exactly(s in setup(), secs in 0u64..=4) {
        // After num × secs cycles, exactly den × secs seconds have passed: no drift.
        let n = s.num * secs;
        let expected = u128::from(s.offset)
            + u128::from(secs) * u128::from(s.den) * u128::from(s.clock.ticks_per_second());
        match s.domain.edge(n) {
            Ok(tick) => prop_assert_eq!(u128::from(tick.0), expected),
            Err(_) => prop_assert!(expected > u128::from(u64::MAX)),
        }
    }

    #[test]
    fn floor_and_ceil_differ_by_at_most_one(s in any_setup(), n in edge_index()) {
        let other = match s.rounding {
            Rounding::Floor => Rounding::Ceil,
            Rounding::Ceil => Rounding::Floor,
        };
        let twin = ClockDomain::new(
            &s.clock,
            ClockDomainId(1),
            s.domain.frequency(),
            Tick(s.offset),
            other,
        )
        .unwrap();
        let (Ok(a), Ok(b)) = (s.domain.edge(n), twin.edge(n)) else {
            return Ok(());
        };
        prop_assert!(a.0.abs_diff(b.0) <= 1);
    }

    #[test]
    fn next_edge_index_is_minimal(s in any_setup(), t in tick_value()) {
        check_minimal(&s, t)?;
    }

    #[test]
    fn next_edge_index_is_minimal_near_edges(
        s in any_setup(),
        m in edge_index(),
        delta in -2i64..=2,
    ) {
        // Uniform `t` almost never lands on an edge; probe the boundaries directly.
        let Ok(edge) = s.domain.edge(m) else { return Ok(()); };
        let Some(t) = edge.0.checked_add_signed(delta) else { return Ok(()); };
        check_minimal(&s, t)?;
    }

    #[test]
    fn cycles_after_is_edge_based(s in any_setup(), now in tick_value(), k in wide_u64()) {
        let result = s.domain.cycles_after(Tick(now), k);
        let base = s.domain.next_edge_index(Tick(now));
        let expected = base.ok().and_then(|b| b.checked_add(k)).and_then(|n| oracle_edge(&s, n));
        prop_assert_eq!(result.ok().map(|t| t.0), expected);
        if let Ok(target) = result {
            prop_assert!(target >= Tick(now));
        }
    }

    #[test]
    fn duration_to_ticks_matches_exact_ceiling(tps in wide_nonzero(), fs in wide_u128()) {
        let clock = SimulationClock::new(tps).unwrap();
        prop_assert_eq!(clock.ticks_for(Duration::from_fs(fs)).ok(), oracle_ticks_for(tps, fs));
    }

    #[test]
    fn duration_to_ticks_is_monotonic_and_subadditive(
        tps in 1u64..=1 << 44,
        a in 0u128..=1 << 60,
        b in 0u128..=1 << 60,
    ) {
        let clock = SimulationClock::new(tps).unwrap();
        let ta = clock.ticks_for(Duration::from_fs(a)).unwrap();
        let tb = clock.ticks_for(Duration::from_fs(b)).unwrap();
        let tab = clock.ticks_for(Duration::from_fs(a + b)).unwrap();
        prop_assert!(tab >= ta.max(tb));
        prop_assert!(tab <= ta + tb);
        prop_assert!(ta + tb <= tab + 1);
    }
}

/// `next_edge_index(t)` returns the smallest `n` with `edge(n) >= t`, and fails only when
/// that `n` does not fit in a `u64`.
fn check_minimal(s: &Setup, t: u64) -> Result<(), TestCaseError> {
    match s.domain.next_edge_index(Tick(t)) {
        Ok(n) => {
            if let Some(at) = oracle_edge(s, n) {
                prop_assert!(at >= t);
            }
            if n > 0 {
                let before = oracle_edge(s, n - 1);
                prop_assert!(before.is_some_and(|b| b < t));
            }
        }
        Err(_) => {
            // Only possible when every representable index still lands before `t`.
            prop_assert!(oracle_edge(s, u64::MAX).is_some_and(|last| last < t));
        }
    }
    Ok(())
}
