# 0004. Start with macroquad behind a renderer trait

**Status:** Accepted
**Date:** 2026-09-08

## Context

The engine needs to put pixels on a screen. The choice of graphics library is
the single most expensive dependency decision in a game project, and the one
most likely to be regretted: it dictates the shader story, the asset pipeline,
the platform support and the build times, and it tends to spread until it is
load-bearing everywhere.

The project is an isometric park simulator drawing flat, sorted rhombuses. It
needs 2D triangles, text and eventually textures. It does not need a render
graph, PBR or an ECS.

There is also a specific complication. `macroquad` carries two RustSec
advisories:

- **RUSTSEC-2025-0035** — multiple soundness issues; pervasive use of mutable
  statics makes use-after-free possible from safe code. No fixed version.
- **RUSTSEC-2026-0192** — `ttf-parser`, reached through `fontdue`, is
  unmaintained.

And its dependency tree does not build on this crate's declared MSRV, because
`fontdue` uses APIs stabilised after 1.85.

## Decision

Put all drawing behind a `render::Renderer` trait, and provide `macroquad` as
the first implementation, behind an off-by-default `macroquad-backend` feature.

Accept both advisories explicitly in `deny.toml`, with this record as the
justification, and exclude the backend from the MSRV check.

## Consequences

**Easier.** The window loop and renderer together are about 200 lines. Nothing
above the trait knows which library is underneath: culling, ordering and
projection are already on the simulation side and are tested headlessly against
`render::Recorder`, a `Renderer` that records draw calls instead of making them.
Migrating to `wgpu` means writing one file, not rewriting the engine.

**The security position, stated plainly.** The advisories are real and are not
dismissed. What makes them acceptable *here*:

- The feature is off by default. A headless simulation, a replay checker or any
  downstream crate that only wants the geometry never links macroquad at all.
- The engine's own code is `forbid(unsafe_code)`; the unsoundness is confined to
  a dependency of an optional feature.
- The threat model is a single-player game rendering its own data. There is no
  untrusted input reaching the renderer, and nothing here handles secrets.

What would change the answer: parsing untrusted content — a downloaded park, a
mod, a font from a save file — through this backend. That should wait for a
backend without an open soundness advisory.

**The MSRV split.** `cargo check` in CI runs with `--features serde` rather than
`--all-features`, so the 1.85 promise covers this crate's code. The backend
follows macroquad's MSRV, which is not ours to promise. The README says so.

**Harder.** The trait is a lowest common denominator, and it will be tempting to
widen it every time macroquad offers something convenient. Every addition should
be answerable with "how would `wgpu` implement this?" — if the answer is
awkward, the feature belongs in the game, not the trait.

## Alternatives considered

**Bevy.** A genuinely excellent engine, and it would supply the ECS, the asset
pipeline and the renderer at once. It would also *be* the architecture: this
project exists to write the isometric machinery, and adopting Bevy means
configuring someone else's instead. Its compile times are also a poor fit for a
project meant to be fun to iterate on.

**`wgpu` directly.** The right long-term answer and the wrong starting point.
Getting a triangle on screen is a day of surface configuration and pipeline
setup before anything looks like a park. The trait exists so this stays
available.

**`softbuffer` or a pure CPU rasteriser.** Genuinely appealing for a project in
the spirit of a game written in assembly, and no advisories. Rejected for now
because text rendering and scaling become the project rather than a detail of
it. A fun future backend.

**`ggez`, `sdl2`, `pixels`.** All plausible. macroquad won on the smallest
amount of code between `cargo run` and a drawn tile, and on WASM support coming
free — which matters for putting a playable build behind a link.
