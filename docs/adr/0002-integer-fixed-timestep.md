# 0002. Fixed timestep with an integer accumulator

**Status:** Accepted
**Date:** 2026-09-08

## Context

The simulation must be deterministic: the same inputs must produce the same
world, on every machine, so that saves and replays mean anything.

The usual fixed-timestep loop keeps a floating-point accumulator of leftover
frame time. It is the shape everyone writes, and it has two problems. Floating
point addition is not associative, so a machine delivering 16.6 ms frames and
one delivering 8.3 ms frames accumulate slightly different totals and eventually
disagree about which frame a tick falls on. And a tick rate that does not divide
a second evenly — 3 ticks per second is 333.333… ms — has no exact
representation at all, so the error compounds forever.

## Decision

Keep the accumulator in integers, scaled by the tick rate.

Adding a frame does `accumulated += frame.as_nanos() * rate`. A tick is due for
every whole `1_000_000_000` in the accumulator, and consuming one subtracts
exactly that. No division happens until the value is handed out for display.

`advance` also caps how many ticks it will run in one call, defaulting to eight.

## Consequences

**Easier.** Frame pacing provably cannot change the outcome: 40 frames of 25 ms
and one frame of 1000 ms produce the same tick count from the same clock, and
`tests/clock_properties.rs` asserts it over arbitrary frame sequences. Awkward
tick rates work exactly. The clock serialises to a save file and resumes without
a rounding difference.

**Harder.** The arithmetic needs `u128` to avoid overflow, and the accumulator's
unit — nanosecond-ticks — is not the obvious thing to read in a debugger. It
needs the comment it has.

**The catch-up cap is a trade-off, not a free win.** Beyond eight ticks in one
call the clock abandons the rest: simulated time still jumps forward, so the
world does not rewind, but that work never happens. Without the cap, a machine
that cannot simulate a tick within a tick asks for more every frame and spirals
until it stops responding. Dropping time is the lesser evil, and
`TickRun::is_behind` reports it so a game can say so rather than quietly running
slow.

## Alternatives considered

**A float accumulator.** The common shape, and wrong for the stated goal. It
would probably survive single-player, and fail the first time two people
compared a replay.

**`Duration` arithmetic directly.** `Duration` is already integer nanoseconds,
which fixes the drift, but the leftover after subtracting a tick that is not a
whole number of nanoseconds still rounds. Scaling by the rate avoids the
division entirely.

**No catch-up cap.** Simpler, and turns a slow frame into an unrecoverable
hang. Not worth it.
