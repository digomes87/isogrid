//! Determinism tests.
//!
//! The engine's central promise: the same seed, stepped the same number of
//! times, produces the same world. Saves, replays and any future networking all
//! rest on it, and it is the kind of promise that breaks quietly — so it is
//! tested against a toy simulation that uses every stateful piece of the crate
//! at once.

use core::time::Duration;

use isogrid::grid::Grid;
use isogrid::iso::TilePos;
use isogrid::rng::Rng;
use isogrid::time::{Clock, Tick};

/// A deliberately fiddly stand-in for a game: wanderers that roll for a
/// direction every tick, and a grid that counts what walked over it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Serialize, serde::Deserialize))]
struct World {
    clock: Clock,
    rng: Rng,
    wanderers: Vec<TilePos>,
    footfall: Grid<u32>,
}

impl World {
    const SIZE: u32 = 16;

    fn new(seed: u64) -> Self {
        let mut rng = Rng::from_seed(seed);
        let wanderers = (0..8)
            .map(|_| {
                let size = i32::try_from(Self::SIZE).expect("the map is small");
                TilePos::new(rng.range(0, size - 1), rng.range(0, size - 1))
            })
            .collect();

        Self {
            clock: Clock::new(40).expect("forty is not zero"),
            rng,
            wanderers,
            footfall: Grid::filled(Self::SIZE, Self::SIZE, 0).expect("the map is not empty"),
        }
    }

    /// Runs however many ticks `frame` is worth.
    fn advance(&mut self, frame: Duration) {
        let run = self.clock.advance(frame);
        for tick in run {
            self.tick(tick);
        }
    }

    fn tick(&mut self, tick: Tick) {
        for wanderer in &mut self.wanderers {
            let step = *self
                .rng
                .choose(&TilePos::NEIGHBOURS)
                .expect("there are always four neighbours");
            let next = wanderer.offset(step.x, step.y);
            if self.footfall.contains(next) {
                *wanderer = next;
            }
            if let Some(count) = self.footfall.get_mut(*wanderer) {
                *count += 1;
            }
        }

        // Something that happens on a period rather than every tick, because
        // that is where off-by-one determinism bugs like to hide.
        if tick.is_multiple_of(10) {
            self.wanderers.sort_unstable();
            self.rng.shuffle(&mut self.wanderers);
        }
    }
}

/// Uneven frame times, as a real machine would deliver them.
const FRAMES: [u64; 12] = [16, 17, 33, 8, 16, 100, 16, 4, 16, 51, 16, 16];

fn run(seed: u64, frames: usize) -> World {
    let mut world = World::new(seed);
    for i in 0..frames {
        world.advance(Duration::from_millis(FRAMES[i % FRAMES.len()]));
    }
    world
}

#[test]
fn the_same_seed_produces_the_same_world() {
    assert_eq!(run(1234, 200), run(1234, 200));
}

#[test]
fn different_seeds_diverge() {
    assert_ne!(run(1234, 200), run(1235, 200));
}

#[test]
fn dropped_ticks_still_advance_simulated_time() {
    // Past the catch-up limit the clock abandons ticks rather than spiralling.
    // The world then runs fewer steps than a steady machine would — which is
    // the documented trade-off, and is worth pinning so it cannot change by
    // accident.
    let mut stalled = World::new(77);
    stalled.advance(Duration::from_secs(1)); // 40 ticks owed, 8 allowed

    let mut steady = World::new(77);
    for _ in 0..40 {
        steady.advance(Duration::from_millis(25));
    }

    assert_eq!(
        stalled.clock.tick(),
        steady.clock.tick(),
        "simulated time must not rewind"
    );
    assert_ne!(stalled, steady, "abandoned ticks should visibly skip work");
}

#[test]
fn frame_pacing_does_not_change_the_outcome() {
    // The same total simulated time, delivered as one long frame or many short
    // ones, must land on the same state. This is what stops a slow machine
    // from playing a different game.
    let mut steady = World::new(77);
    for _ in 0..80 {
        steady.advance(Duration::from_millis(25)); // exactly one tick each
    }

    let mut lumpy = World::new(77);
    for _ in 0..10 {
        // Eight ticks at a time: the default catch-up limit exactly, so nothing
        // is dropped and the two runs must agree.
        lumpy.advance(Duration::from_millis(200));
    }

    assert_eq!(steady.clock.tick(), lumpy.clock.tick());
    assert_eq!(steady, lumpy);
}

#[test]
fn a_world_resumes_identically_from_a_save() {
    let mut saved = run(4242, 60);
    let restored: World =
        serde_json::from_str(&serde_json::to_string(&saved).expect("the world serialises"))
            .expect("and comes back");
    assert_eq!(saved, restored);

    let mut restored = restored;
    for _ in 0..60 {
        saved.advance(Duration::from_millis(31));
        restored.advance(Duration::from_millis(31));
    }
    assert_eq!(
        saved, restored,
        "a restored world drifted from the one it was saved from"
    );
}

#[test]
fn the_world_state_is_stable_across_versions() {
    // A snapshot of the actual numbers. If a refactor changes the sequence of
    // random draws or the tick schedule, this fails loudly rather than silently
    // invalidating every save file in the wild.
    let world = run(2026, 40);
    insta::assert_ron_snapshot!((
        world.clock.tick().get(),
        world.wanderers,
        world.footfall.as_slice()
    ));
}
