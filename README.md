# isogrid

[![CI](https://github.com/digomes87/isogrid/actions/workflows/ci.yml/badge.svg)](https://github.com/digomes87/isogrid/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/digomes87/isogrid/branch/main/graph/badge.svg)](https://codecov.io/gh/digomes87/isogrid)
[![crates.io](https://img.shields.io/crates/v/isogrid.svg)](https://crates.io/crates/isogrid)
[![docs.rs](https://img.shields.io/docsrs/isogrid)](https://docs.rs/isogrid)
[![MSRV](https://img.shields.io/badge/MSRV-1.85.0-blue)](https://blog.rust-lang.org/)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

A small, domain-agnostic **isometric engine** written in Rust.

`isogrid` gives you the parts every tile-based isometric game re-invents — a 2:1
projection, a height-mapped tile grid, a pan/zoom camera, a deterministic
fixed-timestep loop, grid pathfinding and a thin renderer abstraction — and
nothing else. It has no opinion about your game.

It is the engine half of [openpark](https://github.com/digomes87/openpark), a
RollerCoaster Tycoon–flavoured park simulator. The split is strict and
one-directional: **the engine never learns about the game.**

## Status

Early and honest about it. The public API will move before `1.0`.

## Quickstart

```toml
[dependencies]
isogrid = "0.1"
```

## Design goals

- **Deterministic.** The same seed and the same inputs produce the same world
  state after the same number of ticks, on every platform.
- **Testable.** The maths is pure and property-tested; nothing important
  requires a window to verify.
- **Replaceable rendering.** The drawing backend sits behind a trait, so the
  current `macroquad` implementation can be swapped for `wgpu` without the
  simulation noticing.
- **Small.** Features earn their place. Anything that only one game needs
  belongs in that game.

## Feature flags

| Feature | Default | Description |
| --- | --- | --- |
| `serde` | no | Derive `Serialize`/`Deserialize` for engine data types. |
| `macroquad-backend` | no | A renderer and window loop built on `macroquad`. |

## Minimum supported Rust version

`1.85.0`. Raising the MSRV is a minor version bump and is tested in CI.

## Documentation

- API reference: [docs.rs/isogrid](https://docs.rs/isogrid)
- Design overview: [`ARCHITECTURE.md`](ARCHITECTURE.md)
- Decision records: [`docs/adr/`](docs/adr/)

## Contributing

Bug reports, ideas and pull requests are welcome — see
[`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
