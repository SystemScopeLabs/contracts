//! Property tests for the time model (`docs/m0-design.md` §3 and §9, supporting tests).

use proptest::prelude::*;
use systemscope_contracts::time::{
    ClockDomain, ClockDomainId, Duration, Frequency, Rounding, SimulationClock, Tick,
};

const FS_PER_SECOND: u128 = 1_000_000_000_000_000;

/// Ranges are bounded so the exact reference computations below fit in u128.
#[derive(Debug, Clone)]
struct Setup {
    clock: SimulationClock,
    domain: ClockDomain,
    num: u64,
    den: u64,
    offset: u64,
    rounding: Rounding,
}

fn rounding() -> impl Strategy<Value = Rounding> {
    prop_oneof![Just(Rounding::Floor), Just(Rounding::Ceil)]
}

fn setup() -> impl Strategy<Value = Setup> {
    (
        1u64..=1 << 44,
        1u64..=1 << 40,
        1u64..=1 << 20,
        0u64..=1 << 40,
        rounding(),
    )
        .prop_filter_map(
            "clock faster than tick resolution",
            |(tps, num, den, offset, rounding)| {
                let clock = SimulationClock::new(tps).unwrap();
                let freq = Frequency::new(num, den).unwrap();
                let domain =
                    ClockDomain::new(&clock, ClockDomainId(0), freq, Tick(offset), rounding)
                        .ok()?;
                Some(Setup {
                    clock,
                    domain,
                    num,
                    den,
                    offset,
                    rounding,
                })
            },
        )
}

/// Exact edge position from the definition, without the split used by the implementation.
fn reference_edge(s: &Setup, n: u64) -> u128 {
    let exact_num = u128::from(n) * u128::from(s.clock.ticks_per_second()) * u128::from(s.den);
    let q = u128::from(s.num);
    let rel = match s.rounding {
        Rounding::Floor => exact_num / q,
        Rounding::Ceil => exact_num.div_ceil(q),
    };
    u128::from(s.offset) + rel
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn edge_matches_definition(s in setup(), n in 0u64..=1 << 40) {
        match s.domain.edge(n) {
            Ok(tick) => prop_assert_eq!(u128::from(tick.0), reference_edge(&s, n)),
            Err(_) => prop_assert!(reference_edge(&s, n) > u128::from(u64::MAX)),
        }
    }

    #[test]
    fn edges_are_strictly_increasing(s in setup(), n in 0u64..=1 << 32) {
        let (Ok(a), Ok(b)) = (s.domain.edge(n), s.domain.edge(n + 1)) else {
            return Ok(());
        };
        prop_assert!(a < b);
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
    fn floor_and_ceil_differ_by_at_most_one(s in setup(), n in 0u64..=1 << 32) {
        let freq = s.domain.frequency();
        let other = match s.rounding {
            Rounding::Floor => Rounding::Ceil,
            Rounding::Ceil => Rounding::Floor,
        };
        let twin = ClockDomain::new(&s.clock, ClockDomainId(1), freq, Tick(s.offset), other)
            .unwrap();
        let (Ok(a), Ok(b)) = (s.domain.edge(n), twin.edge(n)) else {
            return Ok(());
        };
        prop_assert!(a.0.abs_diff(b.0) <= 1);
    }

    #[test]
    fn next_edge_index_is_minimal(s in setup(), t in 0u64..=1 << 50) {
        let n = s.domain.next_edge_index(Tick(t)).unwrap();
        let Ok(at) = s.domain.edge(n) else { return Ok(()); };
        prop_assert!(at >= Tick(t));
        if n > 0 {
            prop_assert!(s.domain.edge(n - 1).unwrap() < Tick(t));
        }
    }

    #[test]
    fn next_edge_index_is_minimal_near_edges(
        s in setup(),
        m in 0u64..=1 << 32,
        delta in -2i64..=2,
    ) {
        // Uniform `t` almost never lands on an edge; probe the boundaries directly.
        let Ok(edge) = s.domain.edge(m) else { return Ok(()); };
        let Some(t) = edge.0.checked_add_signed(delta) else { return Ok(()); };
        let n = s.domain.next_edge_index(Tick(t)).unwrap();
        let Ok(at) = s.domain.edge(n) else { return Ok(()); };
        prop_assert!(at >= Tick(t));
        if n > 0 {
            prop_assert!(s.domain.edge(n - 1).unwrap() < Tick(t));
        }
    }

    #[test]
    fn cycles_after_is_edge_based(s in setup(), now in 0u64..=1 << 50, k in 0u64..=1 << 16) {
        let Ok(target) = s.domain.cycles_after(Tick(now), k) else { return Ok(()); };
        let base = s.domain.next_edge_index(Tick(now)).unwrap();
        prop_assert_eq!(target, s.domain.edge(base + k).unwrap());
        prop_assert!(target >= Tick(now));
    }

    #[test]
    fn duration_to_ticks_is_minimal_ceiling(tps in 1u64..=1 << 44, fs in 0u128..=1 << 70) {
        let clock = SimulationClock::new(tps).unwrap();
        let exact = fs * u128::from(tps);
        match clock.ticks_for(Duration::from_fs(fs)) {
            Ok(ticks) => {
                let ticks = u128::from(ticks);
                prop_assert!(ticks * FS_PER_SECOND >= exact);
                if ticks > 0 {
                    prop_assert!((ticks - 1) * FS_PER_SECOND < exact);
                }
            }
            Err(_) => prop_assert!(exact.div_ceil(FS_PER_SECOND) > u128::from(u64::MAX)),
        }
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
