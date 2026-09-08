//! Property tests for [`Grid`].
//!
//! The two things that must never break: addressing agrees with iteration, and
//! draw order is a permutation of the grid that is sorted back to front.

use isogrid::grid::Grid;
use isogrid::iso::TilePos;
use proptest::prelude::*;

/// Grid dimensions large enough to have interesting shapes, small enough that
/// thousands of cases stay fast.
fn dimension() -> impl Strategy<Value = u32> {
    1u32..48
}

proptest! {
    /// Every tile the grid reports as inside can be read, and every tile it
    /// reports as outside cannot.
    #[test]
    fn contains_agrees_with_get(w in dimension(), h in dimension(), x in -8i32..64, y in -8i32..64) {
        let grid = Grid::from_fn(w, h, |tile| tile)?;
        let tile = TilePos::new(x, y);
        prop_assert_eq!(grid.contains(tile), grid.get(tile).is_some());
        if let Some(stored) = grid.get(tile) {
            prop_assert_eq!(*stored, tile, "a tile was stored under the wrong position");
        }
    }

    /// Row-major iteration visits every position exactly once.
    #[test]
    fn positions_cover_the_grid_exactly_once(w in dimension(), h in dimension()) {
        let grid = Grid::filled(w, h, ())?;
        let mut seen: Vec<_> = grid.positions().collect();
        prop_assert_eq!(seen.len(), grid.len());
        seen.sort_unstable();
        seen.dedup();
        prop_assert_eq!(seen.len(), grid.len());
    }

    /// Draw order is the same set of tiles, ordered by increasing depth.
    #[test]
    fn draw_order_is_a_back_to_front_permutation(w in dimension(), h in dimension()) {
        let grid = Grid::filled(w, h, ())?;

        let drawn: Vec<_> = grid.draw_order().collect();
        prop_assert_eq!(drawn.len(), grid.len());

        for pair in drawn.windows(2) {
            prop_assert!(
                pair[0].x + pair[0].y <= pair[1].x + pair[1].y,
                "{:?} was drawn before the shallower {:?}",
                pair[0],
                pair[1]
            );
        }

        let mut sorted_drawn = drawn;
        sorted_drawn.sort_unstable();
        let mut sorted_positions: Vec<_> = grid.positions().collect();
        sorted_positions.sort_unstable();
        prop_assert_eq!(sorted_drawn, sorted_positions);
    }

    /// Mutable iteration reports the same positions, in the same order, as the
    /// shared one.
    #[test]
    fn iter_mut_matches_iter(w in dimension(), h in dimension()) {
        let mut grid = Grid::filled(w, h, 0u8)?;
        let expected: Vec<_> = grid.positions().collect();
        let actual: Vec<_> = grid.iter_mut().map(|(pos, _)| pos).collect();
        prop_assert_eq!(actual, expected);
    }

    /// A tile is a neighbour of its neighbours, and never of itself.
    #[test]
    fn neighbourhood_is_symmetric(w in dimension(), h in dimension(), x in 0i32..48, y in 0i32..48) {
        let grid = Grid::filled(w, h, ())?;
        let tile = TilePos::new(x, y);
        prop_assume!(grid.contains(tile));

        for neighbour in grid.neighbours(tile) {
            prop_assert_ne!(neighbour, tile);
            prop_assert!(grid.neighbours(neighbour).any(|back| back == tile));
        }
    }

    /// Mapping preserves the shape of the grid and the position of every tile.
    #[test]
    fn map_preserves_shape(w in dimension(), h in dimension()) {
        let grid = Grid::from_fn(w, h, |tile| tile)?;
        let mapped = grid.map(|pos, tile| (pos, *tile));

        prop_assert_eq!(mapped.width(), grid.width());
        prop_assert_eq!(mapped.height(), grid.height());
        for pos in mapped.positions() {
            prop_assert_eq!(mapped[pos], (pos, pos));
        }
    }
}
