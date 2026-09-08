# 0003. Bundle a generator instead of depending on `rand`

**Status:** Accepted
**Date:** 2026-09-08

## Context

A deterministic simulation needs random numbers whose sequence is fixed forever.
`rand` is the obvious dependency and is an excellent crate, but it optimises for
a different property: it reserves the right to change its algorithms, and its
`StdRng` is explicitly not reproducible across major versions. Its reproducible
generators live in separate crates (`rand_xoshiro`, `rand_pcg`), so the
"just use `rand`" answer is really "use `rand` plus a pinned generator crate
plus the discipline never to reach for the convenient one by accident".

For an engine whose central promise is reproducibility, the sequence of random
numbers is part of the save file format. It cannot be an implementation detail
of a dependency.

## Decision

Bundle a small `xoshiro256**` generator with `SplitMix64` seed expansion, in
`src/rng.rs`, about 200 lines including documentation. Its output is pinned by a
golden-value test cross-checked against an independent implementation, and the
module documents that changing the sequence is a breaking change.

## Consequences

**Easier.** The sequence is owned, tested and versioned with the engine. A
dependency update cannot change what a saved park does. There is one obvious
generator to reach for, so nobody accidentally uses a non-reproducible one. It
also keeps the dependency tree small, which matters for a crate meant to be
cheap to adopt.

**Harder.** It is code that must be maintained and, more importantly, must be
*correct*: a subtly wrong xoshiro still looks random. That is why the golden
values were checked against an outside implementation rather than captured from
this code's own output — capturing your own output only pins the bug.

**Scope.** This generator is not cryptographically secure and the docs say so.
If the project ever needs secrets, that is a different tool.

## Alternatives considered

**`rand` + `rand_xoshiro`.** Two dependencies, a well-tested implementation, and
a trait ecosystem that comes free. The deciding factor was `rand`'s own
`StdRng`, sitting in scope, non-reproducible, one autocomplete away — plus the
`Rng` trait's convenience methods, which are the ones people actually call and
whose *implementations* are free to change. Reintroducing `rand` behind a
feature flag later remains possible.

**`fastrand`.** Tiny and pleasant, but seeds itself from the environment by
default and does not guarantee sequence stability, which is the one thing
required here.

**Passing generation in from the game.** The engine would take a trait and let
the game supply the numbers. Cleanest boundary, and it pushes the hardest part
of determinism onto every consumer. The engine should make the right thing easy.
