//! Property tests for A\*.
//!
//! A\* is easy to write and easy to get subtly wrong: an inadmissible heuristic
//! or a mishandled closed set gives paths that are valid but not shortest, and
//! nothing about the output looks suspicious. So the properties here check the
//! result against a deliberately dumb reference — a breadth-first search for
//! uniform costs, Dijkstra for weighted ones — which is too simple to be wrong
//! in the same way.

use core::num::NonZeroU32;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use isogrid::grid::Grid;
use isogrid::iso::TilePos;
use isogrid::path::{walkable, with_cost, PathFinder};
use proptest::prelude::*;

/// Maps small enough to search exhaustively, varied enough to have dead ends.
fn map() -> impl Strategy<Value = Grid<bool>> {
    (2u32..12, 2u32..12, 0u32..100).prop_map(|(w, h, fill)| {
        // A cheap deterministic pattern rather than random noise, so a shrunk
        // counter-example is reproducible from its parameters alone.
        Grid::from_fn(w, h, |tile| {
            let hash = (tile.x * 73_856_093) ^ (tile.y * 19_349_663);
            hash.unsigned_abs() % 100 >= fill
        })
        .expect("the map is not empty")
    })
}

fn costed_map() -> impl Strategy<Value = Grid<u32>> {
    (2u32..10, 2u32..10, 1u32..9).prop_map(|(w, h, spread)| {
        Grid::from_fn(w, h, |tile| {
            let hash = (tile.x * 73_856_093) ^ (tile.y * 19_349_663);
            // Zero means blocked, so this leaves some tiles impassable.
            hash.unsigned_abs() % (spread + 1)
        })
        .expect("the map is not empty")
    })
}

fn tile_in<T>(grid: &Grid<T>, x: u32, y: u32) -> TilePos {
    TilePos::new(
        i32::try_from(x % grid.width()).expect("small"),
        i32::try_from(y % grid.height()).expect("small"),
    )
}

/// The first tile at or after `(x, y)` that `open` accepts.
///
/// Scanning is how these tests avoid `prop_assume!`: on a map that is mostly
/// walls, assuming an open start rejects nearly every case and starves the
/// strategy instead of testing it.
fn open_tile_from<T>(grid: &Grid<T>, x: u32, y: u32, open: impl Fn(&T) -> bool) -> Option<TilePos> {
    let all: Vec<TilePos> = grid.positions().collect();
    let offset = (y % grid.height()) as usize * grid.width() as usize + (x % grid.width()) as usize;
    (0..all.len())
        .map(|step| all[(offset + step) % all.len()])
        .find(|tile| open(&grid[*tile]))
}

/// The shortest number of steps, found the obvious way.
fn bfs_steps(map: &Grid<bool>, start: TilePos, goal: TilePos) -> Option<u32> {
    if start == goal {
        return Some(0);
    }
    let mut seen = HashMap::new();
    let mut queue = VecDeque::from([(start, 0u32)]);
    seen.insert(start, 0u32);

    while let Some((tile, steps)) = queue.pop_front() {
        for next in map.neighbours(tile) {
            if !map[next] || seen.contains_key(&next) {
                continue;
            }
            if next == goal {
                return Some(steps + 1);
            }
            seen.insert(next, steps + 1);
            queue.push_back((next, steps + 1));
        }
    }
    None
}

/// The cheapest route, found the obvious way.
fn dijkstra_cost(map: &Grid<u32>, start: TilePos, goal: TilePos) -> Option<u32> {
    if start == goal {
        return Some(0);
    }
    let mut best: HashMap<TilePos, u32> = HashMap::from([(start, 0)]);
    let mut queue = BinaryHeap::from([(std::cmp::Reverse(0u32), start)]);

    while let Some((std::cmp::Reverse(cost), tile)) = queue.pop() {
        if tile == goal {
            return Some(cost);
        }
        if cost > *best.get(&tile).unwrap_or(&u32::MAX) {
            continue;
        }
        for next in map.neighbours(tile) {
            let step = map[next];
            if step == 0 {
                continue; // blocked
            }
            let next_cost = cost + step;
            if next_cost < *best.get(&next).unwrap_or(&u32::MAX) {
                best.insert(next, next_cost);
                queue.push((std::cmp::Reverse(next_cost), next));
            }
        }
    }
    None
}

proptest! {
    // Integration tests live outside `src`, where proptest cannot find a crate
    // root to persist regression files against. Shrunken counter-examples are
    // still printed on failure; they are simply not written to disk.
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    /// Any path returned is actually walkable: adjacent steps, open tiles, and
    /// the endpoints it was asked for.
    #[test]
    fn a_returned_path_is_walkable(map in map(), sx in 0u32.., sy in 0u32.., gx in 0u32.., gy in 0u32..) {
        let (start, goal) = (tile_in(&map, sx, sy), tile_in(&map, gx, gy));
        let Some(path) = PathFinder::new().find(&walkable(&map, |open| *open), start, goal) else {
            return Ok(());
        };

        prop_assert_eq!(path.start(), start);
        prop_assert_eq!(path.goal(), goal);
        prop_assert_eq!(path.cost() as usize, path.len() - 1);

        for pair in path.tiles().windows(2) {
            prop_assert_eq!(pair[0].manhattan_distance(pair[1]), 1, "the path teleported");
        }
        // Every tile except possibly the start must be enterable; a guest can
        // legitimately be standing on a tile that was just built over.
        for tile in path.steps() {
            prop_assert!(map[*tile], "the path walked through {tile:?}, which is blocked");
        }
    }

    /// A\* finds a path exactly when one exists, and it is as short as the
    /// dumbest possible search can find.
    #[test]
    fn it_matches_breadth_first_search(map in map(), sx in 0u32.., sy in 0u32.., gx in 0u32.., gy in 0u32..) {
        // BFS below assumes it can leave the start, so scan to an open tile
        // rather than rejecting the blocked-start case that `find` allows.
        let (Some(start), Some(goal)) = (
            open_tile_from(&map, sx, sy, |open| *open),
            open_tile_from(&map, gx, gy, |open| *open),
        ) else {
            return Ok(()); // a map with no open tiles at all
        };

        let found = PathFinder::new().find(&walkable(&map, |open| *open), start, goal);
        let expected = bfs_steps(&map, start, goal);

        match (found, expected) {
            (Some(path), Some(steps)) => prop_assert_eq!(path.cost(), steps, "not the shortest route"),
            (None, None) => {}
            (found, expected) => prop_assert!(
                false,
                "A* and BFS disagree: {:?} vs {:?}",
                found.map(|p| p.cost()),
                expected
            ),
        }
    }

    /// With weighted terrain, A\* agrees with Dijkstra on the cost.
    #[test]
    fn it_matches_dijkstra_on_weighted_terrain(
        terrain in costed_map(),
        sx in 0u32..,
        sy in 0u32..,
        gx in 0u32..,
        gy in 0u32..,
    ) {
        let (Some(start), Some(goal)) = (
            open_tile_from(&terrain, sx, sy, |effort| *effort > 0),
            open_tile_from(&terrain, gx, gy, |effort| *effort > 0),
        ) else {
            return Ok(()); // a map with no passable tiles at all
        };

        let map = with_cost(&terrain, |effort| NonZeroU32::new(*effort));
        let found = PathFinder::new().find(&map, start, goal);
        let expected = dijkstra_cost(&terrain, start, goal);

        prop_assert_eq!(found.as_ref().map(isogrid::path::Path::cost), expected);

        // And the reported cost is really the sum of the steps taken.
        if let Some(path) = found {
            let walked: u32 = path.steps().iter().map(|t| terrain[*t]).sum();
            prop_assert_eq!(walked, path.cost());
        }
    }

    /// The same question always gets the same answer, including which of
    /// several equally short routes comes back.
    #[test]
    fn searches_are_deterministic(map in map(), sx in 0u32.., sy in 0u32.., gx in 0u32.., gy in 0u32..) {
        let (start, goal) = (tile_in(&map, sx, sy), tile_in(&map, gx, gy));
        let once = PathFinder::new().find(&walkable(&map, |open| *open), start, goal);

        // Also through a reused finder, which keeps its buffers between calls.
        let mut finder = PathFinder::new();
        finder.find(&walkable(&map, |open| *open), goal, start);
        let again = finder.find(&walkable(&map, |open| *open), start, goal);

        prop_assert_eq!(once, again);
    }

    /// Reversing the endpoints costs the same, because entry costs here depend
    /// only on the tile being entered.
    #[test]
    fn paths_cost_the_same_in_both_directions(map in map(), sx in 0u32.., sy in 0u32.., gx in 0u32.., gy in 0u32..) {
        let (Some(start), Some(goal)) = (
            open_tile_from(&map, sx, sy, |open| *open),
            open_tile_from(&map, gx, gy, |open| *open),
        ) else {
            return Ok(()); // a map with no open tiles at all
        };

        let mut finder = PathFinder::new();
        let there = finder.find(&walkable(&map, |open| *open), start, goal).map(|p| p.cost());
        let back = finder.find(&walkable(&map, |open| *open), goal, start).map(|p| p.cost());
        prop_assert_eq!(there, back);
    }

    /// A tile is always reachable from itself, even when it is walled in.
    #[test]
    fn every_tile_reaches_itself(map in map(), x in 0u32.., y in 0u32..) {
        let tile = tile_in(&map, x, y);
        let path = PathFinder::new()
            .find(&walkable(&map, |open| *open), tile, tile)
            .expect("a tile always reaches itself");
        prop_assert_eq!(path.len(), 1);
        prop_assert_eq!(path.cost(), 0);
    }
}
