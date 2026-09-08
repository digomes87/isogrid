# Architecture

This document explains how `isogrid` is put together and why. For the decisions
behind individual choices, see [`docs/adr/`](docs/adr/).

## The one rule

**The engine does not know what game is being played.**

`isogrid` has tiles, heights, cameras, ticks, paths and pixels. It has no rides,
no visitors, no money and no opinion about what a tile means. Everything
specific to a park simulator lives in [openpark](https://github.com/digomes87/openpark),
which depends on this crate. The dependency never points the other way.

The rule is worth stating because it is easy to break by accident. The first
time the engine grows a `is_walkable` flag "just for paths", it has started
learning the game, and the next game built on it inherits assumptions it does
not want. When a piece of engine code needs to know something about the world,
it takes it as a parameter or a trait — see [`path::Traversable`](src/path.rs),
which asks the game what a step costs rather than deciding for itself.

## The modules

```text
iso     projection: grid space <-> screen space, draw-order depth
 └─ grid    rectangular tile storage, iteration orders, regions, culling
     └─ camera   pan, zoom, viewport, "what can I see"
 path    A* over anything that answers "what does this step cost"
 time    fixed-timestep clock: real time in, whole ticks out
 rng     seeded, reproducible random numbers
 error   the one error type
```

`iso` is the foundation and depends on nothing. `grid` builds on it, `camera`
builds on both. `time`, `rng` and `path` are independent of the rendering side
entirely — a headless simulation uses them and never touches a pixel.

## The two clocks

The engine keeps simulation time and real time strictly apart.

- **Simulation time** advances in whole, equal [`Tick`](src/time.rs)s. Game
  logic only ever sees ticks. Nothing in a simulation should ask what time it is
  in the real world.
- **Real time** is whatever the machine delivers between frames. It is fed into
  [`Clock::advance`](src/time.rs), which converts it into a whole number of
  ticks and keeps the remainder.

A frame therefore looks like this:

```text
loop {
    let frame = time_since_last_frame();

    for tick in clock.advance(frame) {   // zero, one, or several
        world.update(tick);              // fixed step, deterministic
    }

    render(&world, clock.alpha());       // alpha blends between the last two ticks
}
```

`alpha` is what keeps motion smooth: it is how far the renderer is between the
last tick and the next, so a guest walking at one tile per tick is drawn part of
the way along, even at 144 frames per second on a 40 Hz simulation.

## Determinism

The engine promises that **the same seed, stepped the same number of times,
produces the same world** — on any platform, in any build. Saves, replays and
any future networking all rest on it.

Three things make it hold, and each is easy to break:

1. **The clock's accumulator is integer arithmetic**, scaled by the tick rate.
   A float accumulator drifts; this cannot. A rate that does not divide a second
   evenly still produces exactly the right number of ticks over any interval.
2. **The generator's algorithm is pinned.** [`Rng`](src/rng.rs) is
   `xoshiro256**` with `SplitMix64` seeding, and a golden-value test — checked
   against an independent implementation — fails if the sequence ever changes.
3. **Tie-breaks are total.** A\*'s open set orders by cost, then by heuristic,
   then by tile position, so two equally short routes are never chosen by heap
   internals.

`tests/determinism.rs` exercises all three at once against a toy simulation, and
an `insta` snapshot pins the actual numbers so that a refactor that changes them
fails loudly rather than silently invalidating every save file.

## Rendering

Nothing in the engine draws anything yet. When a backend lands it goes behind a
trait, and the current plan is `macroquad` first with the boundary kept clean
enough to move to `wgpu` later — the simulation must not be able to tell the
difference.

Culling and ordering already live on the simulation side, because they are
geometry rather than graphics: [`Camera::visible_tiles`](src/camera.rs) names
the region on screen, [`Grid::draw_order_within`](src/grid.rs) clips it to the
map and yields the tiles back to front. A renderer's job is then only to draw
what it is handed, in the order it is handed it.

## Performance notes

Measured on an M-series laptop with `cargo bench`; the numbers are indicative,
not a contract.

| Operation | Cost |
| --- | --- |
| One projection | ~0.6 ns |
| Cull and project a 1080p frame | ~1.6 µs |
| One clock tick | ~8 ns |
| A\* across a 128×128 map | ~33 µs |
| A\* across 128×128 weighted terrain | ~0.9 ms |

The last two are the ones to watch. **Repathing 200 agents in a single tick
costs about 29 ms**, which does not fit in a 40 Hz tick's 25 ms budget. That is
a real constraint on the game, not a bug in the engine: a crowd should have its
repaths staggered across ticks rather than recomputed all at once.
[`Tick::is_multiple_of`](src/time.rs) exists for exactly that, and
[`PathFinder`](src/path.rs) reuses its buffers between searches so that
staggering costs nothing extra.

## Testing strategy

Four layers, each catching what the one above cannot:

- **Doc tests** on every public item. They are the examples users copy, so they
  are compiled and run.
- **Unit tests** beside the code, for hand-checked values and edge cases.
- **Property tests** (`tests/*_properties.rs`) for invariants across the whole
  input range: the projection round-trips, culling never hides a visible tile,
  splitting a frame never changes the tick count, and A\* agrees with a
  deliberately dumb breadth-first search that is too simple to be wrong in the
  same way.
- **Determinism tests** (`tests/determinism.rs`) for the promise above.

Benchmarks (`benches/engine.rs`) cover the three things that run often enough to
matter: projecting a frame, stepping the clock, and repathing a crowd.
