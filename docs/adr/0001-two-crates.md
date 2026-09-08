# 0001. Split the engine and the game into two crates

**Status:** Accepted
**Date:** 2026-09-08

## Context

The goal is a RollerCoaster Tycoon–flavoured park simulator, written for the
pleasure of writing it. Almost all of the interesting work — an isometric
projection, a height-mapped grid, a camera, a deterministic tick, pathfinding —
is not specific to parks at all. It is the substrate every isometric tile game
re-implements.

Left in one crate, that substrate and the game logic grow into each other. The
projection learns that a tile can hold a ride; the pathfinder learns that guests
prefer paths; the grid grows a `is_queue` flag. Each step is individually
reasonable and the result is a codebase where nothing can be reasoned about, or
reused, in isolation.

## Decision

Two public repositories with a strictly one-directional dependency:

- **`isogrid`** — the engine. Tiles, projection, camera, ticks, paths, pixels.
  Published to crates.io.
- **`openpark`** — the game. Rides, guests, economy, scenery, saves, UI. Depends
  on `isogrid`; in development via a `git` dependency, at release via crates.io.

The engine never depends on the game, and never learns a single fact about it.

## Consequences

**Easier.** The engine can be tested headlessly and exhaustively, because it has
no game state to set up. Its API has to be honest: anything the engine needs to
know about the world arrives as a parameter or a trait implementation, which
makes the seams obvious. Someone else can build an unrelated isometric game on
it.

**Harder.** Every piece of shared vocabulary has to be designed, not just
written — `path::Traversable` exists because the pathfinder cannot simply look
at a park. Changes that span the boundary need two pull requests in two
repositories and a version bump between them. Some of that friction is the point;
some of it is just friction.

**What has to stay true.** The moment a type in the engine mentions a ride, a
guest or money, this decision has quietly been reversed. That is the thing to
watch for in review.

## Alternatives considered

**One crate, two modules.** Cheaper day to day, and nothing enforces the
boundary but discipline. Discipline is exactly what erodes at 1 a.m. on a hobby
project.

**A Cargo workspace with two crates.** Keeps the compiler enforcing the
boundary while sharing one repository, one CI run and one version. It is a
genuinely good option and was close. Two repositories won because the engine is
meant to be usable by someone who does not want a park simulator, and a crate
that lives inside a game's repository never quite reads as an independent
library. The cost is real: two CI pipelines, two release processes.

**An existing engine (Bevy, macroquad alone, ggez).** The point of this project
is to write the isometric machinery, not to configure someone else's. A
rendering backend is a different question — see the renderer decision when it
lands.
