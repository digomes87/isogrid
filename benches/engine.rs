//! Benchmarks for the hot paths.
//!
//! Three things run often enough to matter: projecting tiles to build a frame,
//! stepping the clock, and repathing a crowd. Everything else is noise by
//! comparison.

use core::num::NonZeroU32;
use core::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use isogrid::camera::{Camera, Viewport};
use isogrid::grid::Grid;
use isogrid::iso::{GridPoint, TilePos, TileSize};
use isogrid::path::{walkable, PathFinder};
use isogrid::rng::Rng;
use isogrid::time::Clock;
use std::hint::black_box;

/// A map with scattered obstacles, the shape a park path network roughly takes.
fn maze(size: u32) -> Grid<bool> {
    Grid::from_fn(size, size, |tile| {
        // Regular pillars with gaps, rather than noise: a search has to route
        // around them, and the layout is identical between runs.
        !(tile.x % 4 == 2 && tile.y % 7 != 3)
    })
    .expect("the map is not empty")
}

fn projection(c: &mut Criterion) {
    let mut group = c.benchmark_group("projection");
    let tiles = TileSize::CLASSIC;

    group.bench_function("grid_to_screen", |b| {
        b.iter(|| tiles.grid_to_screen(black_box(GridPoint::ground(123.5, 456.5))));
    });

    group.bench_function("screen_to_grid", |b| {
        b.iter(|| {
            tiles.screen_to_grid(black_box(isogrid::iso::ScreenPoint::new(640.0, 360.0)), 0.0)
        });
    });

    group.finish();
}

fn frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame");
    let camera = Camera::new(TileSize::CLASSIC, Viewport::new(1920.0, 1080.0).unwrap());
    let world: Grid<u8> = Grid::filled(512, 512, 0).unwrap();

    group.bench_function("cull_and_project_1080p", |b| {
        b.iter(|| {
            let visible = camera.visible_tiles(2);
            let mut drawn = 0u32;
            for tile in world.draw_order_within(visible) {
                black_box(camera.world_to_screen(tile.centre()));
                drawn += 1;
            }
            drawn
        });
    });

    group.bench_function("draw_order_whole_512_map", |b| {
        b.iter(|| world.draw_order().count());
    });

    group.finish();
}

fn simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("simulation");

    group.bench_function("clock_advance", |b| {
        let mut clock = Clock::new(40).unwrap();
        b.iter(|| {
            clock
                .advance(black_box(Duration::from_micros(16_667)))
                .count()
        });
    });

    group.bench_function("rng_next_u64", |b| {
        let mut rng = Rng::from_seed(1);
        b.iter(|| rng.next_u64());
    });

    group.bench_function("rng_below_6", |b| {
        let mut rng = Rng::from_seed(1);
        b.iter(|| rng.below(black_box(6)));
    });

    group.finish();
}

fn pathfinding(c: &mut Criterion) {
    let mut group = c.benchmark_group("pathfinding");

    for size in [32u32, 128] {
        let map = maze(size);
        let far = TilePos::new(
            i32::try_from(size).unwrap() - 1,
            i32::try_from(size).unwrap() - 1,
        );

        group.bench_function(format!("corner_to_corner_{size}x{size}"), |b| {
            let mut finder = PathFinder::new();
            b.iter(|| {
                finder
                    .find(
                        &walkable(&map, |open| *open),
                        black_box(TilePos::ORIGIN),
                        black_box(far),
                    )
                    .map(|path| path.cost())
            });
        });

        group.bench_function(format!("cold_finder_{size}x{size}"), |b| {
            // The allocation cost a game pays if it forgets to keep the finder.
            b.iter_batched(
                PathFinder::new,
                |mut finder| {
                    finder
                        .find(&walkable(&map, |open| *open), TilePos::ORIGIN, far)
                        .map(|path| path.cost())
                },
                BatchSize::SmallInput,
            );
        });
    }

    // A crowd repathing at once: the shape of a real frame's worst case.
    let map = maze(128);
    group.bench_function("crowd_of_200_repathing", |b| {
        let mut finder = PathFinder::new();
        let mut rng = Rng::from_seed(9);
        let agents: Vec<(TilePos, TilePos)> = (0..200)
            .map(|_| {
                (
                    TilePos::new(rng.range(0, 127), rng.range(0, 127)),
                    TilePos::new(rng.range(0, 127), rng.range(0, 127)),
                )
            })
            .collect();

        b.iter(|| {
            let mut total = 0u64;
            for (start, goal) in &agents {
                if let Some(path) = finder.find(&walkable(&map, |open| *open), *start, *goal) {
                    total += u64::from(path.cost());
                }
            }
            total
        });
    });

    let terrain = Grid::from_fn(128, 128, |tile| {
        1 + (tile.x * 7 + tile.y * 13).unsigned_abs() % 9
    })
    .unwrap();
    group.bench_function("weighted_terrain_128x128", |b| {
        let mut finder = PathFinder::new();
        let map = isogrid::path::with_cost(&terrain, |effort| NonZeroU32::new(*effort));
        b.iter(|| {
            finder
                .find(&map, TilePos::ORIGIN, TilePos::new(127, 127))
                .map(|path| path.cost())
        });
    });

    group.finish();
}

criterion_group!(benches, projection, frame, simulation, pathfinding);
criterion_main!(benches);
