//! Property tests for the fixed-timestep clock.
//!
//! The invariant that matters: how the real time is chopped up must not change
//! how many ticks come out of it. A machine running at 30 frames per second and
//! one running at 144 must simulate the same world.

use core::time::Duration;

use isogrid::time::{Clock, Tick};
use proptest::prelude::*;

fn rate() -> impl Strategy<Value = u32> {
    prop_oneof![
        Just(1u32),
        Just(3),
        Just(24),
        Just(40),
        Just(60),
        Just(144),
        1u32..1000
    ]
}

/// Frame times from a stuttering 1 ms to a miserable 200 ms.
fn frames() -> impl Strategy<Value = Vec<u64>> {
    prop::collection::vec(1u64..200, 1..64)
}

proptest! {
    // Integration tests live outside `src`, where proptest cannot find a crate
    // root to persist regression files against. Shrunken counter-examples are
    // still printed on failure; they are simply not written to disk.
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    /// Ticks are never lost or invented: the count matches the elapsed time,
    /// however the frames were split.
    #[test]
    fn ticks_account_for_all_elapsed_time(rate in rate(), frames in frames()) {
        let mut clock = Clock::new(rate)?;
        clock.set_max_catch_up(u32::MAX); // isolate accounting from the drop policy

        let mut ticks = 0u64;
        let mut elapsed_nanos = 0u128;
        for frame in &frames {
            let step = Duration::from_millis(*frame);
            elapsed_nanos += step.as_nanos();
            ticks += u64::from(clock.advance(step).count());
        }

        let expected = elapsed_nanos * u128::from(rate) / 1_000_000_000;
        prop_assert_eq!(u128::from(ticks), expected);
        prop_assert_eq!(clock.tick(), Tick::new(ticks));
    }

    /// Splitting a frame in two produces the same total as leaving it whole.
    #[test]
    fn splitting_a_frame_changes_nothing(rate in rate(), millis in 1u64..500, split in 1u64..499) {
        let split = split.min(millis - 1);

        let mut whole = Clock::new(rate)?;
        whole.set_max_catch_up(u32::MAX);
        whole.advance(Duration::from_millis(millis));

        let mut halves = Clock::new(rate)?;
        halves.set_max_catch_up(u32::MAX);
        halves.advance(Duration::from_millis(split));
        halves.advance(Duration::from_millis(millis - split));

        prop_assert_eq!(whole.tick(), halves.tick());
        prop_assert_eq!(whole, halves, "the leftover accumulators diverged");
    }

    /// The blend factor stays a usable fraction, always.
    #[test]
    fn alpha_stays_in_the_unit_interval(rate in rate(), frames in frames()) {
        let mut clock = Clock::new(rate)?;
        for frame in frames {
            clock.advance(Duration::from_millis(frame));
            let alpha = clock.alpha();
            prop_assert!((0.0..1.0).contains(&alpha), "alpha escaped: {alpha}");
        }
    }

    /// Simulated time never runs backwards, and never overtakes real time.
    #[test]
    fn simulated_time_trails_real_time(rate in rate(), frames in frames()) {
        let mut clock = Clock::new(rate)?;
        clock.set_max_catch_up(u32::MAX);

        let mut real = Duration::ZERO;
        let mut previous = Duration::ZERO;
        for frame in frames {
            let step = Duration::from_millis(frame);
            real += step;
            clock.advance(step);

            let simulated = clock.elapsed();
            prop_assert!(simulated >= previous, "simulated time went backwards");
            prop_assert!(simulated <= real, "simulated time overtook real time");
            previous = simulated;
        }
    }

    /// Catching up never runs more ticks than allowed, and never loses one
    /// without reporting it.
    #[test]
    fn the_catch_up_cap_is_respected(rate in rate(), cap in 1u32..16, millis in 1u64..5000) {
        let mut clock = Clock::new(rate)?;
        clock.set_max_catch_up(cap);

        let before = clock.tick();
        let run = clock.advance(Duration::from_millis(millis));

        prop_assert!(run.count() <= cap);
        prop_assert_eq!(run.first(), before);
        prop_assert_eq!(
            clock.tick(),
            Tick::new(before.get() + u64::from(run.count()) + run.dropped()),
            "a tick was neither run nor reported as dropped"
        );
    }
}
