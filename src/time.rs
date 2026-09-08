//! The fixed-timestep clock that drives the simulation.
//!
//! Simulation time and real time are different things. The simulation advances
//! in equal, whole ticks; the machine delivers frames whenever it manages to.
//! [`Clock`] absorbs that difference: you hand it the real time that passed,
//! and it tells you how many ticks are owed.
//!
//! Everything here is integer arithmetic. A clock fed the same sequence of
//! durations produces the same sequence of ticks on every machine, which is the
//! whole point — a replay, a save file and a network peer all depend on it.
//!
//! ```
//! use std::time::Duration;
//! use isogrid::time::Clock;
//!
//! let mut clock = Clock::new(40)?; // 40 ticks per second: 25 ms each
//! let run = clock.advance(Duration::from_millis(60));
//!
//! assert_eq!(run.count(), 2);                     // two whole ticks fit
//! assert!((clock.alpha() - 0.4).abs() < 1e-6);    // 10 ms of the next one
//! # Ok::<(), isogrid::Error>(())
//! ```

use core::time::Duration;

use crate::error::{Error, Result};

/// One nanosecond-second, the fixed point the accumulator counts in.
const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// A tick number, counted from the start of the simulation.
///
/// Ticks are the simulation's only clock. Nothing in a deterministic
/// simulation should ever ask what time it is in the real world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tick(u64);

impl Tick {
    /// The tick before the simulation has run at all.
    pub const ZERO: Self = Self(0);

    /// Builds a tick from its number.
    ///
    /// ```
    /// # use isogrid::time::Tick;
    /// assert_eq!(Tick::new(7).get(), 7);
    /// ```
    pub const fn new(tick: u64) -> Self {
        Self(tick)
    }

    /// The tick number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The tick `count` steps later, saturating at the end of `u64`.
    ///
    /// ```
    /// # use isogrid::time::Tick;
    /// assert_eq!(Tick::new(3).after(4), Tick::new(7));
    /// ```
    #[must_use]
    pub const fn after(self, count: u64) -> Self {
        Self(self.0.saturating_add(count))
    }

    /// Whether this tick falls on a period of `every` ticks.
    ///
    /// The idiom for work that runs less often than every tick — repathing,
    /// economy updates, autosaves.
    ///
    /// ```
    /// # use isogrid::time::Tick;
    /// assert!(Tick::new(40).is_multiple_of(40));
    /// assert!(!Tick::new(41).is_multiple_of(40));
    /// assert!(!Tick::new(40).is_multiple_of(0)); // never, rather than a panic
    /// ```
    pub const fn is_multiple_of(self, every: u64) -> bool {
        every != 0 && self.0 % every == 0
    }
}

/// How many ticks the simulation runs per second of simulated time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TickRate(u32);

impl TickRate {
    /// Forty ticks a second, the rate the original park sims ran at.
    pub const CLASSIC: Self = Self(40);

    /// Builds a tick rate.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidTickRate`] if `per_second` is zero.
    ///
    /// ```
    /// # use isogrid::time::TickRate;
    /// assert!(TickRate::new(60).is_ok());
    /// assert!(TickRate::new(0).is_err());
    /// ```
    pub const fn new(per_second: u32) -> Result<Self> {
        if per_second == 0 {
            return Err(Error::InvalidTickRate);
        }
        Ok(Self(per_second))
    }

    /// Ticks per second.
    pub const fn per_second(self) -> u32 {
        self.0
    }

    /// How long one tick lasts in simulated time.
    ///
    /// Rounded down to the nanosecond for display. The clock itself never uses
    /// this value; it works in exact integer fractions so that rates which do
    /// not divide a second evenly still do not drift.
    ///
    /// ```
    /// # use isogrid::time::TickRate;
    /// # use core::time::Duration;
    /// assert_eq!(TickRate::new(40)?.step(), Duration::from_millis(25));
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    #[allow(clippy::cast_possible_truncation)] // At worst one second of nanoseconds.
    pub const fn step(self) -> Duration {
        Duration::from_nanos((NANOS_PER_SECOND / self.0 as u128) as u64)
    }
}

impl Default for TickRate {
    fn default() -> Self {
        Self::CLASSIC
    }
}

/// The ticks owed after one call to [`Clock::advance`].
///
/// Iterate it to run them:
///
/// ```
/// # use core::time::Duration;
/// # use isogrid::time::Clock;
/// let mut clock = Clock::new(40)?;
/// let mut simulated = 0;
/// for _tick in clock.advance(Duration::from_millis(100)) {
///     simulated += 1;
/// }
/// assert_eq!(simulated, 4);
/// # Ok::<(), isogrid::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickRun {
    first: Tick,
    count: u32,
    dropped: u64,
}

impl TickRun {
    /// The first tick of the run.
    pub const fn first(&self) -> Tick {
        self.first
    }

    /// How many ticks to run.
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// How many ticks were abandoned because the simulation could not keep up.
    ///
    /// Non-zero means the machine is behind and the clock gave up rather than
    /// spiralling. Simulated time skips forward; a game that cares should say
    /// so out loud rather than silently running slow.
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Whether the simulation fell behind during this run.
    pub const fn is_behind(&self) -> bool {
        self.dropped > 0
    }
}

impl IntoIterator for TickRun {
    type Item = Tick;
    type IntoIter = TickRunIter;

    fn into_iter(self) -> Self::IntoIter {
        TickRunIter {
            next: self.first,
            remaining: self.count,
        }
    }
}

/// The iterator over a [`TickRun`].
#[derive(Debug, Clone)]
pub struct TickRunIter {
    next: Tick,
    remaining: u32,
}

impl Iterator for TickRunIter {
    type Item = Tick;

    fn next(&mut self) -> Option<Tick> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let tick = self.next;
        self.next = tick.after(1);
        Some(tick)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for TickRunIter {}

/// A fixed-timestep clock.
///
/// Feed it real elapsed time; it hands back whole ticks and keeps the remainder
/// for next time.
///
/// ```
/// use core::time::Duration;
/// use isogrid::time::Clock;
///
/// let mut clock = Clock::new(60)?;
/// // Three uneven frames still add up to exactly the right number of ticks.
/// let mut ticks = 0;
/// for frame in [17, 16, 17] {
///     ticks += clock.advance(Duration::from_millis(frame)).count();
/// }
/// assert_eq!(ticks, 3);
/// # Ok::<(), isogrid::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Clock {
    rate: TickRate,
    /// Elapsed real time, scaled by the tick rate. A tick is due for every
    /// whole `NANOS_PER_SECOND` in here, which keeps the arithmetic exact for
    /// rates that do not divide a second evenly.
    accumulated: u128,
    ticks: u64,
    max_catch_up: u32,
}

impl Clock {
    /// How many ticks a single [`Clock::advance`] will run before giving up and
    /// dropping the rest.
    ///
    /// Without a cap, a machine that cannot simulate a tick in less than a tick
    /// asks for more ticks every frame and never recovers. Dropping simulated
    /// time is the lesser evil, and [`TickRun::is_behind`] reports it.
    pub const DEFAULT_MAX_CATCH_UP: u32 = 8;

    /// Builds a clock running at `ticks_per_second`, starting at
    /// [`Tick::ZERO`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidTickRate`] if `ticks_per_second` is zero.
    pub const fn new(ticks_per_second: u32) -> Result<Self> {
        match TickRate::new(ticks_per_second) {
            Ok(rate) => Ok(Self::at_rate(rate)),
            Err(error) => Err(error),
        }
    }

    /// Builds a clock at an already-validated rate.
    ///
    /// ```
    /// # use isogrid::time::{Clock, Tick, TickRate};
    /// let clock = Clock::at_rate(TickRate::CLASSIC);
    /// assert_eq!(clock.tick(), Tick::ZERO);
    /// ```
    pub const fn at_rate(rate: TickRate) -> Self {
        Self {
            rate,
            accumulated: 0,
            ticks: 0,
            max_catch_up: Self::DEFAULT_MAX_CATCH_UP,
        }
    }

    /// The rate this clock runs at.
    pub const fn rate(&self) -> TickRate {
        self.rate
    }

    /// The next tick that has not run yet.
    pub const fn tick(&self) -> Tick {
        Tick(self.ticks)
    }

    /// How many ticks a single [`Clock::advance`] may run.
    pub const fn max_catch_up(&self) -> u32 {
        self.max_catch_up
    }

    /// Sets how many ticks a single [`Clock::advance`] may run.
    ///
    /// A value of zero is raised to one: a clock that can never tick is a
    /// stopped clock, which is never what the caller meant.
    pub const fn set_max_catch_up(&mut self, ticks: u32) {
        self.max_catch_up = if ticks == 0 { 1 } else { ticks };
    }

    /// How far into the next tick the clock is, from `0.0` to just under `1.0`.
    ///
    /// This is the blend factor for interpolated rendering: draw each moving
    /// thing `alpha` of the way between where it was last tick and where it is
    /// now, and motion stays smooth even though the simulation is stepping.
    ///
    /// ```
    /// # use core::time::Duration;
    /// # use isogrid::time::Clock;
    /// let mut clock = Clock::new(10)?;         // 100 ms per tick
    /// clock.advance(Duration::from_millis(50));
    /// assert!((clock.alpha() - 0.5).abs() < 1e-6);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    // The accumulator is below 1e9 here, and the result is a blend factor: f32
    // is all the precision a renderer can use.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn alpha(&self) -> f32 {
        (self.accumulated as f64 / NANOS_PER_SECOND as f64) as f32
    }

    /// Simulated time elapsed, counting whole ticks only.
    ///
    /// ```
    /// # use core::time::Duration;
    /// # use isogrid::time::Clock;
    /// let mut clock = Clock::new(40)?;
    /// clock.advance(Duration::from_millis(130));
    /// assert_eq!(clock.elapsed(), Duration::from_millis(125)); // five ticks
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn elapsed(&self) -> Duration {
        let nanos = u128::from(self.ticks) * NANOS_PER_SECOND / u128::from(self.rate.0);
        // Saturating is unreachable in practice: u64 nanoseconds is 584 years
        // of simulated time.
        Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
    }

    /// Adds real elapsed time and returns the ticks it owes.
    ///
    /// At most [`Clock::max_catch_up`] ticks are returned; any beyond that are
    /// abandoned and reported by [`TickRun::dropped`].
    ///
    /// ```
    /// # use core::time::Duration;
    /// # use isogrid::time::Clock;
    /// let mut clock = Clock::new(40)?;
    /// let stall = clock.advance(Duration::from_secs(10)); // 400 ticks owed
    /// assert_eq!(stall.count(), clock.max_catch_up());
    /// assert!(stall.is_behind());
    /// assert_eq!(clock.alpha(), 0.0, "abandoned time is discarded, not banked");
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn advance(&mut self, real: Duration) -> TickRun {
        self.accumulated += real.as_nanos() * u128::from(self.rate.0);

        let owed = self.accumulated / NANOS_PER_SECOND;
        let count = u32::try_from(owed)
            .unwrap_or(u32::MAX)
            .min(self.max_catch_up);
        let dropped = u64::try_from(owed - u128::from(count)).unwrap_or(u64::MAX);

        self.accumulated -= owed * NANOS_PER_SECOND;
        let first = Tick(self.ticks);
        self.ticks = self
            .ticks
            .saturating_add(u64::from(count))
            .saturating_add(dropped);

        TickRun {
            first,
            count,
            dropped,
        }
    }

    /// Throws away the partial tick without advancing simulated time.
    ///
    /// Call it after a pause or a long load, so the clock does not immediately
    /// try to catch up on wall-clock time the simulation was never running for.
    pub const fn reset_accumulator(&mut self) {
        self.accumulated = 0;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn rejects_a_stopped_clock() {
        assert!(Clock::new(0).is_err());
        assert!(TickRate::new(0).is_err());
    }

    #[test]
    fn a_clock_starts_at_tick_zero() {
        let clock = Clock::at_rate(TickRate::CLASSIC);
        assert_eq!(clock.tick(), Tick::ZERO);
        assert_eq!(clock.alpha(), 0.0);
        assert_eq!(clock.elapsed(), Duration::ZERO);
    }

    #[test]
    fn whole_ticks_come_out_whole() {
        let mut clock = Clock::new(40).unwrap();
        let run = clock.advance(Duration::from_millis(100));
        assert_eq!(run.count(), 4);
        assert_eq!(run.first(), Tick::ZERO);
        assert_eq!(clock.tick(), Tick::new(4));
        assert_eq!(clock.alpha(), 0.0);
    }

    #[test]
    fn a_partial_tick_is_kept_for_next_time() {
        let mut clock = Clock::new(10).unwrap();
        assert_eq!(clock.advance(Duration::from_millis(60)).count(), 0);
        assert!((clock.alpha() - 0.6).abs() < 1e-6);
        assert_eq!(clock.advance(Duration::from_millis(60)).count(), 1);
        assert!((clock.alpha() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn rates_that_do_not_divide_a_second_do_not_drift() {
        // 3 ticks per second is 333.333... ms, which no integer millisecond
        // step can represent. Over a full second it must still be exactly 3.
        let mut clock = Clock::new(3).unwrap();
        let mut ticks = 0;
        for _ in 0..1000 {
            ticks += clock.advance(Duration::from_millis(1)).count();
        }
        assert_eq!(ticks, 3);
        assert_eq!(clock.elapsed(), Duration::from_secs(1));
    }

    #[test]
    fn a_thousand_uneven_frames_land_exactly_on_the_second() {
        let mut clock = Clock::new(60).unwrap();
        let mut ticks = 0;
        // 7 ms repeated does not divide a 60 Hz tick, but the total does.
        for _ in 0..1000 {
            ticks += clock.advance(Duration::from_micros(7000)).count();
        }
        assert_eq!(ticks, 7 * 60); // 7 seconds at 60 Hz
    }

    #[test]
    fn falling_behind_drops_ticks_instead_of_spiralling() {
        let mut clock = Clock::new(40).unwrap();
        let run = clock.advance(Duration::from_secs(10));
        assert_eq!(run.count(), Clock::DEFAULT_MAX_CATCH_UP);
        assert_eq!(run.dropped(), 400 - u64::from(Clock::DEFAULT_MAX_CATCH_UP));
        assert!(run.is_behind());
        // Simulated time still jumps forward, so the world does not rewind.
        assert_eq!(clock.tick(), Tick::new(400));
        assert_eq!(clock.alpha(), 0.0);
    }

    #[test]
    fn the_catch_up_cap_is_never_zero() {
        let mut clock = Clock::new(40).unwrap();
        clock.set_max_catch_up(0);
        assert_eq!(clock.max_catch_up(), 1);
        assert_eq!(clock.advance(Duration::from_secs(1)).count(), 1);
    }

    #[test]
    fn a_run_yields_consecutive_ticks() {
        let mut clock = Clock::new(40).unwrap();
        clock.advance(Duration::from_millis(50));
        let run = clock.advance(Duration::from_millis(75));
        let ticks: Vec<_> = run.into_iter().collect();
        assert_eq!(ticks, vec![Tick::new(2), Tick::new(3), Tick::new(4)]);
    }

    #[test]
    fn resetting_the_accumulator_discards_only_the_partial_tick() {
        let mut clock = Clock::new(40).unwrap();
        clock.advance(Duration::from_millis(60));
        assert!(clock.alpha() > 0.0);
        clock.reset_accumulator();
        assert_eq!(clock.alpha(), 0.0);
        assert_eq!(clock.tick(), Tick::new(2));
    }

    #[test]
    fn periodic_work_fires_on_schedule() {
        let fires: Vec<u64> = (0..10)
            .map(Tick::new)
            .filter(|t| t.is_multiple_of(4))
            .map(Tick::get)
            .collect();
        assert_eq!(fires, vec![0, 4, 8]);
        assert!(!Tick::new(4).is_multiple_of(0));
    }
}
