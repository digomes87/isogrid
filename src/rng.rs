//! A small, deterministic random number generator.
//!
//! A simulation that wants to be reproducible cannot reach for the operating
//! system's entropy, and it cannot use a generator whose algorithm might change
//! under it. [`Rng`] is `xoshiro256**` with a fixed, documented algorithm: the
//! same seed produces the same sequence, on every platform, in every version of
//! this crate that keeps the same major version.
//!
//! It is **not** cryptographically secure, and nothing here should be used to
//! generate secrets.
//!
//! ```
//! use isogrid::rng::Rng;
//!
//! let mut a = Rng::from_seed(42);
//! let mut b = Rng::from_seed(42);
//! assert_eq!(a.next_u64(), b.next_u64());
//!
//! let mut different = Rng::from_seed(43);
//! assert_ne!(Rng::from_seed(42).next_u64(), different.next_u64());
//! ```

/// A seeded `xoshiro256**` generator.
///
/// Cheap to copy and to store in a save file. Give each independent system its
/// own generator — with [`Rng::fork`] — rather than sharing one, so that adding
/// a system does not change what every other system rolls.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    /// Builds a generator from a seed.
    ///
    /// The seed is expanded with `SplitMix64`, so neighbouring seeds — `1` and
    /// `2`, or a pair of adjacent tile numbers — still produce unrelated
    /// streams.
    ///
    /// ```
    /// # use isogrid::rng::Rng;
    /// let mut from_one = Rng::from_seed(1);
    /// let mut from_two = Rng::from_seed(2);
    /// assert_ne!(from_one.next_u64(), from_two.next_u64());
    /// ```
    pub const fn from_seed(seed: u64) -> Self {
        let mut splitmix = seed;
        let mut state = [0u64; 4];
        let mut i = 0;
        while i < 4 {
            splitmix = splitmix.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = splitmix;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            state[i] = z ^ (z >> 31);
            i += 1;
        }
        Self { state }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.state[1] << 17;

        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);

        result
    }

    /// The next 32 random bits.
    #[allow(clippy::cast_possible_truncation)] // Taking the top 32 bits is the point.
    pub fn next_u32(&mut self) -> u32 {
        // The high bits of xoshiro256** are the better-mixed ones.
        (self.next_u64() >> 32) as u32
    }

    /// A number in `0..bound`, or `None` if `bound` is zero.
    ///
    /// Unbiased: the naive modulo makes the low values fractionally more likely,
    /// which is invisible in a test and visible in a histogram.
    ///
    /// ```
    /// # use isogrid::rng::Rng;
    /// let mut rng = Rng::from_seed(7);
    /// let roll = rng.below(6).expect("six is not zero");
    /// assert!(roll < 6);
    /// assert_eq!(rng.below(0), None);
    /// ```
    // Every cast here keeps a deliberate half of a 64-bit product.
    #[allow(clippy::cast_possible_truncation)]
    pub fn below(&mut self, bound: u32) -> Option<u32> {
        if bound == 0 {
            return None;
        }
        // Lemire's multiply-shift, with the rejection step that removes the bias.
        let mut product = u64::from(self.next_u32()) * u64::from(bound);
        let mut low = product as u32;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                product = u64::from(self.next_u32()) * u64::from(bound);
                low = product as u32;
            }
        }
        Some((product >> 32) as u32)
    }

    /// A number in `low..=high`, inclusive, in whichever order the bounds come.
    ///
    /// ```
    /// # use isogrid::rng::Rng;
    /// let mut rng = Rng::from_seed(11);
    /// let roll = rng.range(1, 6);
    /// assert!((1..=6).contains(&roll));
    /// assert_eq!(rng.range(4, 4), 4);
    /// ```
    #[allow(clippy::cast_possible_wrap)] // A full-width span wants every bit pattern.
    pub fn range(&mut self, low: i32, high: i32) -> i32 {
        let (low, high) = (low.min(high), low.max(high));
        let span = high.abs_diff(low);
        if span == u32::MAX {
            return self.next_u32() as i32;
        }
        let offset = self.below(span + 1).unwrap_or(0);
        low.saturating_add_unsigned(offset)
    }

    /// A number in `0.0..1.0`.
    ///
    /// ```
    /// # use isogrid::rng::Rng;
    /// let mut rng = Rng::from_seed(3);
    /// let value = rng.next_f32();
    /// assert!((0.0..1.0).contains(&value));
    /// ```
    #[allow(clippy::cast_precision_loss)] // 24 bits into an f32 mantissa is exact.
    pub fn next_f32(&mut self) -> f32 {
        // 24 bits is exactly what an f32 mantissa holds, so every value in the
        // range is equally likely and none of them round up to 1.0.
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Whether an event with the given probability happens.
    ///
    /// Probabilities at or below zero never happen; at or above one, always.
    ///
    /// ```
    /// # use isogrid::rng::Rng;
    /// let mut rng = Rng::from_seed(5);
    /// assert!(!rng.chance(0.0));
    /// assert!(rng.chance(1.0));
    /// assert!(!rng.chance(f32::NAN)); // never, rather than a panic
    /// ```
    pub fn chance(&mut self, probability: f32) -> bool {
        if probability <= 0.0 || probability.is_nan() {
            return false;
        }
        if probability >= 1.0 {
            return true;
        }
        self.next_f32() < probability
    }

    /// One item of `items`, or `None` if it is empty.
    ///
    /// ```
    /// # use isogrid::rng::Rng;
    /// let mut rng = Rng::from_seed(9);
    /// let flavours = ["vanilla", "chocolate", "pistachio"];
    /// assert!(flavours.contains(rng.choose(&flavours).unwrap()));
    /// assert_eq!(rng.choose::<u8>(&[]), None);
    /// ```
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        let index = self.below(u32::try_from(items.len()).unwrap_or(u32::MAX))?;
        items.get(index as usize)
    }

    /// Shuffles `items` in place, with every ordering equally likely.
    ///
    /// ```
    /// # use isogrid::rng::Rng;
    /// let mut rng = Rng::from_seed(13);
    /// let mut queue = [1, 2, 3, 4, 5];
    /// rng.shuffle(&mut queue);
    /// queue.sort_unstable();
    /// assert_eq!(queue, [1, 2, 3, 4, 5]); // a permutation, nothing lost
    /// ```
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        // Fisher-Yates, back to front.
        for i in (1..items.len()).rev() {
            let bound = u32::try_from(i + 1).unwrap_or(u32::MAX);
            if let Some(j) = self.below(bound) {
                items.swap(i, j as usize);
            }
        }
    }

    /// Splits off an independent generator.
    ///
    /// Give each system its own stream. Sharing one generator means adding a
    /// system changes what every other system rolls, which turns an unrelated
    /// feature into a change in every existing replay.
    ///
    /// ```
    /// # use isogrid::rng::Rng;
    /// let mut world = Rng::from_seed(1);
    /// let mut weather = world.fork();
    /// let mut crowds = world.fork();
    /// assert_ne!(weather.next_u64(), crowds.next_u64());
    /// ```
    #[must_use]
    pub fn fork(&mut self) -> Self {
        Self::from_seed(self.next_u64())
    }
}

impl Default for Rng {
    /// A generator seeded with zero.
    ///
    /// Deliberately fixed: an engine that seeds itself from the clock cannot be
    /// replayed, and the choice of seed belongs to the game, not to a default.
    fn default() -> Self {
        Self::from_seed(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first values from seed 1. If a refactor changes these, it has
    /// changed every existing replay and save file, and that must be a
    /// deliberate, breaking decision rather than a surprise.
    ///
    /// Cross-checked against an independent implementation of `SplitMix64` and
    /// `xoshiro256**` rather than against this code's own output.
    const SEED_ONE: [u64; 4] = [
        12_966_619_160_104_079_557,
        9_600_361_134_598_540_522,
        10_590_380_919_521_690_900,
        7_218_738_570_589_545_383,
    ];

    #[test]
    fn the_sequence_is_pinned_to_the_algorithm() {
        let mut rng = Rng::from_seed(1);
        let actual: Vec<u64> = (0..4).map(|_| rng.next_u64()).collect();
        assert_eq!(actual, SEED_ONE, "the generator's output changed");
    }

    #[test]
    fn the_same_seed_replays_exactly() {
        let mut a = Rng::from_seed(0xDEAD_BEEF);
        let mut b = Rng::from_seed(0xDEAD_BEEF);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_eq!(a, b);
    }

    #[test]
    fn neighbouring_seeds_are_unrelated() {
        let streams: Vec<u64> = (0..8).map(|seed| Rng::from_seed(seed).next_u64()).collect();
        let mut sorted = streams.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), streams.len(), "adjacent seeds collided");
    }

    #[test]
    fn a_zero_seed_still_generates() {
        // A naive xoshiro seeded with all zeroes emits nothing but zeroes;
        // SplitMix64 expansion is what prevents that.
        let mut rng = Rng::from_seed(0);
        assert_ne!(rng.next_u64(), 0);
    }

    #[test]
    fn below_stays_in_range_and_rejects_zero() {
        let mut rng = Rng::from_seed(2);
        for bound in [1u32, 2, 6, 7, 100, u32::MAX] {
            for _ in 0..200 {
                assert!(rng.below(bound).unwrap() < bound);
            }
        }
        assert_eq!(rng.below(0), None);
    }

    #[test]
    fn below_is_close_to_uniform() {
        let mut rng = Rng::from_seed(4);
        let mut buckets = [0u32; 6];
        for _ in 0..60_000 {
            buckets[rng.below(6).unwrap() as usize] += 1;
        }
        // Ten thousand expected per bucket; five percent is a very loose bound
        // that a biased modulo would still fail on a skewed range.
        for count in buckets {
            assert!(
                (9_500..10_500).contains(&count),
                "lopsided distribution: {buckets:?}"
            );
        }
    }

    #[test]
    fn range_covers_both_ends() {
        let mut rng = Rng::from_seed(6);
        let mut seen_low = false;
        let mut seen_high = false;
        for _ in 0..1000 {
            let roll = rng.range(1, 6);
            assert!((1..=6).contains(&roll));
            seen_low |= roll == 1;
            seen_high |= roll == 6;
        }
        assert!(seen_low && seen_high, "range never produced an endpoint");
    }

    #[test]
    fn range_handles_inverted_and_extreme_bounds() {
        let mut rng = Rng::from_seed(8);
        assert_eq!(rng.range(4, 4), 4);
        assert!((-3..=3).contains(&rng.range(3, -3)));

        // A full-width span cannot be expressed as `span + 1`, so it takes a
        // separate path. Check that path actually reaches both signs.
        let (mut negative, mut positive) = (false, false);
        for _ in 0..1000 {
            let extreme = rng.range(i32::MIN, i32::MAX);
            negative |= extreme < 0;
            positive |= extreme > 0;
        }
        assert!(negative && positive, "the full-width range is lopsided");
    }

    #[test]
    fn floats_stay_inside_the_unit_interval() {
        let mut rng = Rng::from_seed(10);
        for _ in 0..10_000 {
            let value = rng.next_f32();
            assert!(
                (0.0..1.0).contains(&value),
                "{value} escaped the unit interval"
            );
        }
    }

    #[test]
    fn certain_and_impossible_chances_do_not_consume_randomness() {
        let mut rng = Rng::from_seed(12);
        let untouched = rng.clone();
        assert!(!rng.chance(0.0));
        assert!(rng.chance(1.0));
        assert!(!rng.chance(f32::NAN));
        assert_eq!(rng, untouched, "a decided outcome should not draw a number");
    }

    #[test]
    fn shuffle_permutes_without_losing_anything() {
        let mut rng = Rng::from_seed(14);
        let mut items: Vec<u32> = (0..64).collect();
        rng.shuffle(&mut items);
        assert_ne!(
            items,
            (0..64).collect::<Vec<_>>(),
            "shuffle left the order alone"
        );
        items.sort_unstable();
        assert_eq!(items, (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_handles_short_slices() {
        let mut rng = Rng::from_seed(15);
        let mut empty: [u8; 0] = [];
        rng.shuffle(&mut empty);
        let mut single = [9];
        rng.shuffle(&mut single);
        assert_eq!(single, [9]);
    }

    #[test]
    fn choose_returns_something_from_the_slice() {
        let mut rng = Rng::from_seed(16);
        let items = [10, 20, 30];
        for _ in 0..100 {
            assert!(items.contains(rng.choose(&items).unwrap()));
        }
        assert_eq!(rng.choose::<u8>(&[]), None);
    }

    #[test]
    fn forks_are_independent_streams() {
        let mut world = Rng::from_seed(1);
        let mut left = world.fork();
        let mut right = world.fork();
        let a: Vec<u64> = (0..16).map(|_| left.next_u64()).collect();
        let b: Vec<u64> = (0..16).map(|_| right.next_u64()).collect();
        assert_ne!(a, b);
    }

    #[test]
    fn forking_is_reproducible() {
        let forked = |seed| {
            let mut parent = Rng::from_seed(seed);
            let mut child = parent.fork();
            child.next_u64()
        };
        assert_eq!(forked(99), forked(99));
    }
}
