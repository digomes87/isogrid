//! Grid pathfinding with A\*.
//!
//! [`PathFinder`] searches any map that can answer two questions: what area it
//! covers, and what it costs to step from one tile to the next. That is the
//! whole interface — the engine has no idea whether a tile is blocked by a
//! wall, a fence or a lake.
//!
//! ```
//! use isogrid::grid::Grid;
//! use isogrid::iso::TilePos;
//! use isogrid::path::{PathFinder, walkable};
//!
//! // A wall across the middle of a small map, with a gap at the end.
//! let mut map = Grid::filled(5, 5, true)?;
//! for x in 0..4 {
//!     map[TilePos::new(x, 2)] = false;
//! }
//!
//! let mut finder = PathFinder::new();
//! let path = finder
//!     .find(&walkable(&map, |open| *open), TilePos::ORIGIN, TilePos::new(0, 4))
//!     .expect("the gap is open");
//!
//! assert_eq!(path.start(), TilePos::ORIGIN);
//! assert_eq!(path.goal(), TilePos::new(0, 4));
//! assert!(path.tiles().contains(&TilePos::new(4, 2)), "it must go round through the gap");
//! # Ok::<(), isogrid::Error>(())
//! ```

use core::cmp::{Ordering, Reverse};
use core::num::NonZeroU32;
use std::collections::BinaryHeap;

use crate::grid::{Grid, TileBounds};
use crate::iso::TilePos;

/// A map that [`PathFinder`] can search.
///
/// Implement it on whatever your game calls a map. The engine only needs the
/// area covered and the cost of a single step.
pub trait Traversable {
    /// The region the map covers. Tiles outside it are never entered.
    fn bounds(&self) -> TileBounds;

    /// What it costs to step from `from` to an adjacent `to`, or `None` if that
    /// step is not allowed.
    ///
    /// Costs are non-zero so that a search cannot stall by circling for free.
    /// The step is always between tiles sharing an edge.
    fn step_cost(&self, from: TilePos, to: TilePos) -> Option<NonZeroU32>;

    /// The cheapest any single step can be.
    ///
    /// The heuristic is scaled by this. Returning something larger than the
    /// true minimum makes the search faster and the result possibly
    /// sub-optimal; the default of `1` is always safe.
    fn min_step_cost(&self) -> NonZeroU32 {
        NonZeroU32::MIN
    }
}

/// A [`Traversable`] built from a grid and a closure.
///
/// Returned by [`walkable`] and [`with_cost`]; you rarely need to name it.
#[derive(Debug, Clone, Copy)]
pub struct GridMap<'a, T, F> {
    grid: &'a Grid<T>,
    cost: F,
}

impl<T, F> Traversable for GridMap<'_, T, F>
where
    F: Fn(&T) -> Option<NonZeroU32>,
{
    fn bounds(&self) -> TileBounds {
        self.grid.bounds()
    }

    fn step_cost(&self, _from: TilePos, to: TilePos) -> Option<NonZeroU32> {
        (self.cost)(self.grid.get(to)?)
    }
}

/// Treats a grid as a map where each tile is either open or blocked.
///
/// Every allowed step costs the same, so the shortest path is the one with the
/// fewest tiles.
///
/// ```
/// # use isogrid::grid::Grid;
/// # use isogrid::iso::TilePos;
/// # use isogrid::path::{PathFinder, walkable};
/// let map = Grid::filled(4, 4, true)?;
/// let path = PathFinder::new()
///     .find(&walkable(&map, |open| *open), TilePos::ORIGIN, TilePos::new(3, 3))
///     .expect("an empty map is all path");
/// assert_eq!(path.cost(), 6); // six steps across the diagonal
/// # Ok::<(), isogrid::Error>(())
/// ```
pub fn walkable<T>(
    grid: &Grid<T>,
    open: impl Fn(&T) -> bool,
) -> GridMap<'_, T, impl Fn(&T) -> Option<NonZeroU32>> {
    GridMap {
        grid,
        cost: move |tile: &T| open(tile).then_some(NonZeroU32::MIN),
    }
}

/// Treats a grid as a map where entering each tile has its own cost.
///
/// Return `None` for tiles that cannot be entered at all.
///
/// ```
/// # use isogrid::grid::Grid;
/// # use core::num::NonZeroU32;
/// # use isogrid::iso::TilePos;
/// # use isogrid::path::{PathFinder, with_cost};
/// // Mud down the middle: passable, but four times the effort.
/// let mut terrain = Grid::filled(3, 3, 1u32)?;
/// terrain[TilePos::new(1, 1)] = 4;
///
/// let map = with_cost(&terrain, |effort| NonZeroU32::new(*effort));
/// let path = PathFinder::new()
///     .find(&map, TilePos::ORIGIN, TilePos::new(2, 2))
///     .expect("the mud is passable");
/// assert!(!path.tiles().contains(&TilePos::new(1, 1)), "it should walk around the mud");
/// # Ok::<(), isogrid::Error>(())
/// ```
pub fn with_cost<T, F>(grid: &Grid<T>, cost: F) -> GridMap<'_, T, F>
where
    F: Fn(&T) -> Option<NonZeroU32>,
{
    GridMap { grid, cost }
}

/// A route from one tile to another.
///
/// Always contains at least the starting tile, and the tiles are consecutive
/// and adjacent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Path {
    tiles: Vec<TilePos>,
    cost: u32,
}

impl Path {
    /// The tiles walked, starting at the start and ending at the goal.
    pub fn tiles(&self) -> &[TilePos] {
        &self.tiles
    }

    /// The total cost of the route.
    ///
    /// Zero for a path that goes nowhere.
    pub const fn cost(&self) -> u32 {
        self.cost
    }

    /// The number of tiles on the route, including both ends.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Always `false`; a path always contains at least its starting tile.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The tile the route starts on.
    ///
    /// # Panics
    ///
    /// Never: a path is never built empty.
    pub fn start(&self) -> TilePos {
        self.tiles[0]
    }

    /// The tile the route ends on.
    ///
    /// # Panics
    ///
    /// Never: a path is never built empty.
    pub fn goal(&self) -> TilePos {
        self.tiles[self.tiles.len() - 1]
    }

    /// The tiles after the start — the steps still to take.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// # use isogrid::path::{PathFinder, walkable};
    /// let map = Grid::filled(3, 1, true)?;
    /// let path = PathFinder::new()
    ///     .find(&walkable(&map, |open| *open), TilePos::ORIGIN, TilePos::new(2, 0))
    ///     .expect("a clear line");
    /// assert_eq!(path.steps(), [TilePos::new(1, 0), TilePos::new(2, 0)]);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn steps(&self) -> &[TilePos] {
        &self.tiles[1..]
    }
}

/// The state A\* keeps for one tile.
#[derive(Debug, Clone, Copy)]
struct Node {
    /// Which search wrote this entry. Lets the buffer be reused without being
    /// cleared, which matters when hundreds of agents repath every tick.
    generation: u32,
    cost: u32,
    came_from: TilePos,
    closed: bool,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            generation: 0,
            cost: u32::MAX,
            came_from: TilePos::ORIGIN,
            closed: false,
        }
    }
}

/// An entry in the open set, ordered by estimated total cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Candidate {
    estimate: u32,
    remaining: u32,
    tile: TilePos,
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Ties are broken by the heuristic first — preferring tiles nearer the
        // goal is a real speed-up — and then by position, so that the search is
        // fully deterministic rather than depending on heap internals.
        self.estimate
            .cmp(&other.estimate)
            .then_with(|| self.remaining.cmp(&other.remaining))
            .then_with(|| self.tile.cmp(&other.tile))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A reusable A\* search.
///
/// Hold on to one and call [`PathFinder::find`] repeatedly: the working buffers
/// are reused between searches, so repathing a crowd does not allocate once per
/// agent.
///
/// Searches are deterministic. The same map and the same endpoints always
/// produce the same path, including which of several equally short routes is
/// chosen.
#[derive(Debug, Clone, Default)]
pub struct PathFinder {
    open: BinaryHeap<Reverse<Candidate>>,
    nodes: Vec<Node>,
    bounds: Option<TileBounds>,
    generation: u32,
}

impl PathFinder {
    /// Builds a path finder with no buffers allocated yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Finds the cheapest route from `start` to `goal`, or `None` if there is
    /// none.
    ///
    /// Returns `None` if either endpoint is outside the map or cannot be
    /// entered. A start that equals the goal yields a path of one tile and zero
    /// cost.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// # use isogrid::path::{PathFinder, walkable};
    /// let map = Grid::filled(4, 4, true)?;
    /// let mut finder = PathFinder::new();
    ///
    /// let here = finder
    ///     .find(&walkable(&map, |open| *open), TilePos::ORIGIN, TilePos::ORIGIN)
    ///     .expect("you are already there");
    /// assert_eq!(here.len(), 1);
    /// assert_eq!(here.cost(), 0);
    ///
    /// assert!(finder
    ///     .find(&walkable(&map, |open| *open), TilePos::ORIGIN, TilePos::new(99, 99))
    ///     .is_none());
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn find(&mut self, map: &impl Traversable, start: TilePos, goal: TilePos) -> Option<Path> {
        let bounds = map.bounds();
        if !bounds.contains(start) || !bounds.contains(goal) {
            return None;
        }

        self.prepare(bounds);
        let step = map.min_step_cost().get();

        if start == goal {
            // A tile can be stood on even when it could not be entered, so this
            // deliberately does not consult `step_cost`.
            return Some(Path {
                tiles: vec![start],
                cost: 0,
            });
        }

        self.write(start, 0, start);
        self.open.push(Reverse(Candidate {
            estimate: Self::heuristic(start, goal, step),
            remaining: Self::heuristic(start, goal, step),
            tile: start,
        }));

        while let Some(Reverse(candidate)) = self.open.pop() {
            if candidate.tile == goal {
                return Some(self.rebuild(start, goal));
            }

            let index = self.index(candidate.tile);
            if self.nodes[index].closed {
                continue; // A cheaper route to this tile was already expanded.
            }
            self.nodes[index].closed = true;
            let cost_so_far = self.nodes[index].cost;

            for neighbour in candidate.tile.neighbours() {
                if !bounds.contains(neighbour) {
                    continue;
                }
                let Some(step_cost) = map.step_cost(candidate.tile, neighbour) else {
                    continue;
                };

                let cost = cost_so_far.saturating_add(step_cost.get());
                let node = self.nodes[self.index(neighbour)];
                if node.generation == self.generation && cost >= node.cost {
                    continue;
                }

                self.write(neighbour, cost, candidate.tile);
                let remaining = Self::heuristic(neighbour, goal, step);
                self.open.push(Reverse(Candidate {
                    estimate: cost.saturating_add(remaining),
                    remaining,
                    tile: neighbour,
                }));
            }
        }

        None
    }

    /// The admissible heuristic: the fewest steps possible, at the cheapest a
    /// step can be.
    fn heuristic(from: TilePos, to: TilePos, min_step_cost: u32) -> u32 {
        from.manhattan_distance(to).saturating_mul(min_step_cost)
    }

    /// Resizes the buffers for `bounds` and starts a new generation.
    fn prepare(&mut self, bounds: TileBounds) {
        self.open.clear();
        self.generation = self.generation.wrapping_add(1);

        let area = usize::try_from(bounds.len()).unwrap_or(usize::MAX);
        if self.bounds != Some(bounds) || self.nodes.len() != area {
            self.bounds = Some(bounds);
            self.nodes.clear();
            self.nodes.resize(area, Node::default());
            // A fresh buffer is all generation zero, so start above it.
            self.generation = 1;
        }
    }

    fn index(&self, tile: TilePos) -> usize {
        let bounds = self.bounds.expect("prepare runs before any lookup");
        let x = tile.x.abs_diff(bounds.min().x) as usize;
        let y = tile.y.abs_diff(bounds.min().y) as usize;
        y * bounds.width() as usize + x
    }

    fn write(&mut self, tile: TilePos, cost: u32, came_from: TilePos) {
        let generation = self.generation;
        let index = self.index(tile);
        self.nodes[index] = Node {
            generation,
            cost,
            came_from,
            closed: false,
        };
    }

    fn rebuild(&self, start: TilePos, goal: TilePos) -> Path {
        let cost = self.nodes[self.index(goal)].cost;
        let mut tiles = vec![goal];
        let mut tile = goal;
        while tile != start {
            tile = self.nodes[self.index(tile)].came_from;
            tiles.push(tile);
        }
        tiles.reverse();
        Path { tiles, cost }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;

    fn open_map(width: u32, height: u32) -> Grid<bool> {
        Grid::filled(width, height, true).expect("the map is not empty")
    }

    fn find(map: &Grid<bool>, start: TilePos, goal: TilePos) -> Option<Path> {
        PathFinder::new().find(&walkable(map, |open| *open), start, goal)
    }

    #[test]
    fn a_straight_line_is_a_straight_line() {
        let map = open_map(5, 1);
        let path = find(&map, TilePos::ORIGIN, TilePos::new(4, 0)).unwrap();
        assert_eq!(path.len(), 5);
        assert_eq!(path.cost(), 4);
        assert_eq!(path.steps().len(), 4);
    }

    #[test]
    fn the_start_is_also_a_destination() {
        let map = open_map(4, 4);
        let path = find(&map, TilePos::new(2, 2), TilePos::new(2, 2)).unwrap();
        assert_eq!(path.tiles(), [TilePos::new(2, 2)]);
        assert_eq!(path.cost(), 0);
        assert!(path.steps().is_empty());
    }

    #[test]
    fn endpoints_outside_the_map_have_no_path() {
        let map = open_map(4, 4);
        assert!(find(&map, TilePos::new(-1, 0), TilePos::ORIGIN).is_none());
        assert!(find(&map, TilePos::ORIGIN, TilePos::new(4, 0)).is_none());
    }

    #[test]
    fn a_walled_off_goal_has_no_path() {
        let mut map = open_map(5, 5);
        for y in 0..5 {
            map[TilePos::new(2, y)] = false;
        }
        assert!(find(&map, TilePos::ORIGIN, TilePos::new(4, 4)).is_none());
    }

    #[test]
    fn an_unreachable_goal_tile_has_no_path() {
        let mut map = open_map(4, 4);
        map[TilePos::new(3, 3)] = false;
        assert!(find(&map, TilePos::ORIGIN, TilePos::new(3, 3)).is_none());
    }

    #[test]
    fn a_blocked_start_can_still_be_left_nowhere() {
        // Standing somewhere you could not have walked into is a real state:
        // a guest on a tile that was just built over. The search must not
        // pretend the tile does not exist.
        let mut map = open_map(3, 3);
        map[TilePos::ORIGIN] = false;
        assert_eq!(
            find(&map, TilePos::ORIGIN, TilePos::ORIGIN).unwrap().len(),
            1
        );
        assert!(find(&map, TilePos::ORIGIN, TilePos::new(2, 2)).is_some());
    }

    #[test]
    fn it_walks_around_a_wall() {
        let mut map = open_map(5, 5);
        for x in 0..4 {
            map[TilePos::new(x, 2)] = false;
        }
        let path = find(&map, TilePos::ORIGIN, TilePos::new(0, 4)).unwrap();
        assert!(path.tiles().contains(&TilePos::new(4, 2)));
        assert!(path.tiles().iter().all(|tile| map[*tile]));
    }

    #[test]
    fn it_prefers_cheap_ground_over_short_routes() -> Result<()> {
        // A wall of mud with one clear tile in it: going round is longer in
        // steps but cheaper in effort.
        let mut terrain = Grid::filled(5, 5, 1u32)?;
        for y in 0..4 {
            terrain[TilePos::new(2, y)] = 50;
        }

        let map = with_cost(&terrain, |effort| NonZeroU32::new(*effort));
        let path = PathFinder::new()
            .find(&map, TilePos::ORIGIN, TilePos::new(4, 0))
            .unwrap();

        assert!(
            path.tiles().contains(&TilePos::new(2, 4)),
            "it should detour to the gap"
        );
        assert!(path.cost() < 50);
        Ok(())
    }

    #[test]
    fn zero_cost_tiles_are_impassable_by_construction() -> Result<()> {
        // `NonZeroU32::new(0)` is `None`, so a zero-cost tile is simply blocked
        // and the search cannot loop through it for free.
        let mut terrain = Grid::filled(3, 1, 1u32)?;
        terrain[TilePos::new(1, 0)] = 0;
        let map = with_cost(&terrain, |effort| NonZeroU32::new(*effort));
        assert!(PathFinder::new()
            .find(&map, TilePos::ORIGIN, TilePos::new(2, 0))
            .is_none());
        Ok(())
    }

    #[test]
    fn the_path_is_a_chain_of_adjacent_tiles() {
        let mut map = open_map(8, 8);
        map[TilePos::new(3, 3)] = false;
        map[TilePos::new(3, 4)] = false;
        let path = find(&map, TilePos::ORIGIN, TilePos::new(7, 7)).unwrap();
        for pair in path.tiles().windows(2) {
            assert_eq!(pair[0].manhattan_distance(pair[1]), 1);
        }
    }

    #[test]
    fn reusing_a_finder_gives_the_same_answers() {
        let map = open_map(12, 12);
        let mut finder = PathFinder::new();
        let first = finder.find(
            &walkable(&map, |o| *o),
            TilePos::ORIGIN,
            TilePos::new(11, 11),
        );
        for _ in 0..50 {
            finder.find(
                &walkable(&map, |o| *o),
                TilePos::new(3, 1),
                TilePos::new(9, 8),
            );
        }
        let again = finder.find(
            &walkable(&map, |o| *o),
            TilePos::ORIGIN,
            TilePos::new(11, 11),
        );
        assert_eq!(first, again, "a reused finder drifted");
    }

    #[test]
    fn a_finder_adapts_to_a_different_map_size() {
        let mut finder = PathFinder::new();
        let small = open_map(3, 3);
        let large = open_map(20, 20);
        assert!(finder
            .find(
                &walkable(&small, |o| *o),
                TilePos::ORIGIN,
                TilePos::new(2, 2)
            )
            .is_some());
        assert!(finder
            .find(
                &walkable(&large, |o| *o),
                TilePos::ORIGIN,
                TilePos::new(19, 19)
            )
            .is_some());
        assert!(finder
            .find(
                &walkable(&small, |o| *o),
                TilePos::ORIGIN,
                TilePos::new(2, 2)
            )
            .is_some());
    }

    #[test]
    fn ties_are_broken_the_same_way_every_time() {
        // An open map has many equally short routes. Which one comes back must
        // not depend on heap internals, or two machines replaying the same save
        // will send a guest down different paths.
        let map = open_map(10, 10);
        let expected = find(&map, TilePos::ORIGIN, TilePos::new(9, 9)).unwrap();
        for _ in 0..20 {
            assert_eq!(
                find(&map, TilePos::ORIGIN, TilePos::new(9, 9)).unwrap(),
                expected
            );
        }
    }
}
