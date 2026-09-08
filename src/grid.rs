//! A rectangular grid of tiles, and the orders you can walk it in.
//!
//! [`Grid<T>`] is deliberately dull: a bounds-checked rectangle of `T`, with no
//! opinion about what a tile *is*. A game stores its terrain in one, its
//! ownership map in another, and the engine treats both the same way.
//!
//! ```
//! use isogrid::grid::Grid;
//! use isogrid::iso::TilePos;
//!
//! let mut heights = Grid::filled(4, 4, 0u8)?;
//! heights[TilePos::new(1, 2)] = 3;
//!
//! assert_eq!(heights.get(TilePos::new(1, 2)), Some(&3));
//! assert_eq!(heights.get(TilePos::new(9, 9)), None); // outside the grid
//! # Ok::<(), isogrid::Error>(())
//! ```

use core::ops::{Index, IndexMut};

use crate::error::{Error, Result};
use crate::iso::TilePos;

/// A rectangular grid of tiles addressed by [`TilePos`].
///
/// The grid always starts at [`TilePos::ORIGIN`] and extends toward positive
/// `x` and `y`. Negative coordinates are outside every grid, which keeps
/// bounds checks to two unsigned comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Grid<T> {
    width: u32,
    height: u32,
    tiles: Vec<T>,
}

impl<T> Grid<T> {
    /// The largest number of tiles a grid may hold.
    ///
    /// A little over 67 million — a square map 8192 tiles on a side. The cap
    /// exists so that a mistyped dimension is rejected as invalid input rather
    /// than aborting the process inside the allocator.
    pub const MAX_TILES: usize = 1 << 26;

    /// Builds a grid by calling `tile` once for every position, in row-major
    /// order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidGridSize`] if either dimension is zero, or if
    /// the two multiplied together would not fit in a `usize`.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let checkerboard = Grid::from_fn(8, 8, |tile| (tile.x + tile.y) % 2 == 0)?;
    /// assert_eq!(checkerboard[TilePos::ORIGIN], true);
    /// assert_eq!(checkerboard[TilePos::new(1, 0)], false);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn from_fn(width: u32, height: u32, mut tile: impl FnMut(TilePos) -> T) -> Result<Self> {
        let area = Self::area(width, height)?;
        let mut tiles = Vec::with_capacity(area);
        for y in 0..height {
            for x in 0..width {
                tiles.push(tile(Self::to_tile_pos(x, y)));
            }
        }
        Ok(Self {
            width,
            height,
            tiles,
        })
    }

    /// Builds a grid where every tile starts as a clone of `value`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidGridSize`] under the same conditions as
    /// [`Grid::from_fn`].
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// let empty = Grid::filled(64, 64, None::<u32>)?;
    /// assert_eq!(empty.len(), 4096);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn filled(width: u32, height: u32, value: T) -> Result<Self>
    where
        T: Clone,
    {
        let area = Self::area(width, height)?;
        Ok(Self {
            width,
            height,
            tiles: vec![value; area],
        })
    }

    /// Builds a grid where every tile starts at its [`Default`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidGridSize`] under the same conditions as
    /// [`Grid::from_fn`].
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let heights: Grid<i16> = Grid::new(16, 16)?;
    /// assert_eq!(heights[TilePos::ORIGIN], 0);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn new(width: u32, height: u32) -> Result<Self>
    where
        T: Default,
    {
        Self::from_fn(width, height, |_| T::default())
    }

    /// The width of the grid in tiles.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// The height of the grid in tiles.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The number of tiles in the grid.
    ///
    /// Never zero: a grid with no tiles cannot be built.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// assert_eq!(Grid::filled(3, 4, ())?.len(), 12);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Always `false`; kept so the type reads like the collections it mimics.
    ///
    /// A grid must have at least one tile, so there is no empty grid to report.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Whether `tile` is inside the grid.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let grid = Grid::filled(4, 4, ())?;
    /// assert!(grid.contains(TilePos::new(3, 3)));
    /// assert!(!grid.contains(TilePos::new(4, 0)));
    /// assert!(!grid.contains(TilePos::new(-1, 0)));
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    #[allow(clippy::cast_sign_loss)] // Both casts are guarded by the sign checks in front of them.
    pub const fn contains(&self, tile: TilePos) -> bool {
        tile.x >= 0 && tile.y >= 0 && (tile.x as u32) < self.width && (tile.y as u32) < self.height
    }

    /// The tile at `tile`, or `None` if it is outside the grid.
    pub fn get(&self, tile: TilePos) -> Option<&T> {
        self.index_of(tile).map(|i| &self.tiles[i])
    }

    /// A mutable reference to the tile at `tile`, or `None` if it is outside
    /// the grid.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let mut grid = Grid::filled(2, 2, 0u8)?;
    /// if let Some(tile) = grid.get_mut(TilePos::new(1, 1)) {
    ///     *tile = 7;
    /// }
    /// assert_eq!(grid.get(TilePos::new(1, 1)), Some(&7));
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn get_mut(&mut self, tile: TilePos) -> Option<&mut T> {
        self.index_of(tile).map(|i| &mut self.tiles[i])
    }

    /// Replaces the tile at `tile`, returning what was there before.
    ///
    /// Returns `None` and changes nothing if `tile` is outside the grid.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let mut grid = Grid::filled(2, 2, 1u8)?;
    /// assert_eq!(grid.replace(TilePos::ORIGIN, 9), Some(1));
    /// assert_eq!(grid.replace(TilePos::new(5, 5), 9), None);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn replace(&mut self, tile: TilePos, value: T) -> Option<T> {
        self.get_mut(tile)
            .map(|slot| core::mem::replace(slot, value))
    }

    /// Every position in the grid, in row-major order.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let grid = Grid::filled(2, 2, ())?;
    /// let visited: Vec<_> = grid.positions().collect();
    /// assert_eq!(visited[0], TilePos::ORIGIN);
    /// assert_eq!(visited[3], TilePos::new(1, 1));
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn positions(&self) -> impl Iterator<Item = TilePos> + '_ {
        let width = self.width;
        (0..self.height).flat_map(move |y| (0..width).map(move |x| Self::to_tile_pos(x, y)))
    }

    /// Every position and tile, in row-major order.
    pub fn iter(&self) -> impl Iterator<Item = (TilePos, &T)> {
        self.positions().zip(self.tiles.iter())
    }

    /// Every position and tile, mutably, in row-major order.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// let mut grid = Grid::filled(3, 3, 0i32)?;
    /// for (pos, tile) in grid.iter_mut() {
    ///     *tile = pos.x + pos.y;
    /// }
    /// assert_eq!(grid.iter().map(|(_, t)| *t).sum::<i32>(), 18);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (TilePos, &mut T)> {
        let width = self.width;
        self.tiles.iter_mut().enumerate().map(move |(i, tile)| {
            #[allow(clippy::cast_possible_truncation)]
            let i = i as u32;
            (Self::to_tile_pos(i % width, i / width), tile)
        })
    }

    /// Every position in the grid, back to front for the isometric camera.
    ///
    /// Walks anti-diagonals of constant `x + y`, which is the order
    /// [`TileSize::depth`](crate::iso::TileSize::depth) sorts by. Row-major
    /// order is fine for flat ground; this is the order you need once tiles
    /// have height or hold anything tall enough to overlap its neighbour.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let grid = Grid::filled(2, 2, ())?;
    /// let order: Vec<_> = grid.draw_order().collect();
    /// assert_eq!(order[0], TilePos::ORIGIN);
    /// assert_eq!(order[3], TilePos::new(1, 1));
    /// // The two middle tiles share a diagonal and never overlap each other.
    /// assert_eq!(order[1].x + order[1].y, order[2].x + order[2].y);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn draw_order(&self) -> impl Iterator<Item = TilePos> + '_ {
        let (width, height) = (self.width, self.height);
        (0..width + height - 1).flat_map(move |diagonal| {
            let first_x = diagonal.saturating_sub(height - 1);
            let last_x = diagonal.min(width - 1);
            (first_x..=last_x).map(move |x| Self::to_tile_pos(x, diagonal - x))
        })
    }

    /// The in-bounds tiles sharing an edge with `tile`.
    ///
    /// Positions outside the grid are dropped, so a corner tile yields two
    /// neighbours rather than four `None`s.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let grid = Grid::filled(4, 4, ())?;
    /// assert_eq!(grid.neighbours(TilePos::ORIGIN).count(), 2);
    /// assert_eq!(grid.neighbours(TilePos::new(1, 1)).count(), 4);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn neighbours(&self, tile: TilePos) -> impl Iterator<Item = TilePos> + '_ {
        tile.neighbours().into_iter().filter(|t| self.contains(*t))
    }

    /// A new grid of the same shape, with every tile passed through `f`.
    ///
    /// ```
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let heights = Grid::from_fn(3, 3, |tile| tile.x)?;
    /// let doubled = heights.map(|_, h| h * 2);
    /// assert_eq!(doubled[TilePos::new(2, 0)], 4);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn map<U>(&self, mut f: impl FnMut(TilePos, &T) -> U) -> Grid<U> {
        Grid {
            width: self.width,
            height: self.height,
            tiles: self.iter().map(|(pos, tile)| f(pos, tile)).collect(),
        }
    }

    /// The tiles in row-major order, without their positions.
    pub fn as_slice(&self) -> &[T] {
        &self.tiles
    }

    fn area(width: u32, height: u32) -> Result<usize> {
        let invalid = || Error::InvalidGridSize { width, height };
        if width == 0 || height == 0 {
            return Err(invalid());
        }
        let area = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
            .ok_or_else(invalid)?;
        if area > Self::MAX_TILES {
            return Err(invalid());
        }
        Ok(area)
    }

    fn index_of(&self, tile: TilePos) -> Option<usize> {
        if !self.contains(tile) {
            return None;
        }
        #[allow(clippy::cast_sign_loss)]
        let (x, y) = (tile.x as usize, tile.y as usize);
        Some(y * self.width as usize + x)
    }

    #[allow(clippy::cast_possible_wrap)]
    const fn to_tile_pos(x: u32, y: u32) -> TilePos {
        // Grids are capped well below `i32::MAX` by the allocation itself, so
        // this cast cannot wrap for any grid that exists.
        TilePos::new(x as i32, y as i32)
    }
}

impl<T> Index<TilePos> for Grid<T> {
    type Output = T;

    /// # Panics
    ///
    /// Panics if `tile` is outside the grid. Use [`Grid::get`] when the
    /// position might not be there.
    fn index(&self, tile: TilePos) -> &T {
        self.get(tile).unwrap_or_else(|| {
            panic!(
                "tile {:?} is outside a {}x{} grid",
                (tile.x, tile.y),
                self.width,
                self.height
            )
        })
    }
}

impl<T> IndexMut<TilePos> for Grid<T> {
    /// # Panics
    ///
    /// Panics if `tile` is outside the grid. Use [`Grid::get_mut`] when the
    /// position might not be there.
    fn index_mut(&mut self, tile: TilePos) -> &mut T {
        let (width, height) = (self.width, self.height);
        self.get_mut(tile).unwrap_or_else(|| {
            panic!(
                "tile {:?} is outside a {width}x{height} grid",
                (tile.x, tile.y)
            )
        })
    }
}

impl<'a, T> IntoIterator for &'a Grid<T> {
    type Item = (TilePos, &'a T);
    type IntoIter = Box<dyn Iterator<Item = (TilePos, &'a T)> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_grids_with_no_tiles() {
        assert!(Grid::filled(0, 4, ()).is_err());
        assert!(Grid::filled(4, 0, ()).is_err());
    }

    #[test]
    fn rejects_grids_too_large_to_address() {
        assert!(Grid::filled(u32::MAX, u32::MAX, ()).is_err());
        // Zero-sized tiles allocate nothing, so this only exercises the cap.
        assert!(Grid::filled(1 << 13, 1 << 13, ()).is_ok());
        assert!(Grid::filled(1 << 13, (1 << 13) + 1, ()).is_err());
    }

    #[test]
    fn negative_positions_are_never_inside() {
        let grid = Grid::filled(4, 4, ()).unwrap();
        assert!(!grid.contains(TilePos::new(-1, 0)));
        assert!(!grid.contains(TilePos::new(0, -1)));
        assert_eq!(grid.get(TilePos::new(-1, -1)), None);
    }

    #[test]
    fn from_fn_visits_every_position_once() {
        let grid = Grid::from_fn(3, 5, |tile| tile).unwrap();
        for pos in grid.positions() {
            assert_eq!(grid[pos], pos);
        }
        assert_eq!(grid.len(), 15);
    }

    #[test]
    fn draw_order_never_moves_backwards() {
        let grid = Grid::filled(5, 7, ()).unwrap();
        let mut previous = i32::MIN;
        let mut seen = Vec::new();
        for tile in grid.draw_order() {
            assert!(
                tile.x + tile.y >= previous,
                "{tile:?} came after a deeper tile"
            );
            previous = tile.x + tile.y;
            seen.push(tile);
        }
        assert_eq!(seen.len(), grid.len());
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), grid.len(), "draw order repeated a tile");
    }

    #[test]
    fn draw_order_handles_a_single_tile() {
        let grid = Grid::filled(1, 1, ()).unwrap();
        assert_eq!(grid.draw_order().collect::<Vec<_>>(), vec![TilePos::ORIGIN]);
    }

    #[test]
    fn corner_tiles_have_two_neighbours() {
        let grid = Grid::filled(3, 3, ()).unwrap();
        assert_eq!(grid.neighbours(TilePos::ORIGIN).count(), 2);
        assert_eq!(grid.neighbours(TilePos::new(2, 2)).count(), 2);
        assert_eq!(grid.neighbours(TilePos::new(1, 0)).count(), 3);
        assert_eq!(grid.neighbours(TilePos::new(1, 1)).count(), 4);
    }

    #[test]
    fn replace_returns_the_previous_tile() {
        let mut grid = Grid::filled(2, 2, 1u8).unwrap();
        assert_eq!(grid.replace(TilePos::ORIGIN, 5), Some(1));
        assert_eq!(grid[TilePos::ORIGIN], 5);
        assert_eq!(grid.replace(TilePos::new(9, 9), 5), None);
    }

    #[test]
    fn iter_mut_reports_the_same_positions_as_iter() {
        let mut grid = Grid::filled(4, 3, 0u8).unwrap();
        let expected: Vec<_> = grid.positions().collect();
        let actual: Vec<_> = grid.iter_mut().map(|(pos, _)| pos).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    #[should_panic(expected = "outside a 2x2 grid")]
    fn indexing_out_of_bounds_panics() {
        let grid = Grid::filled(2, 2, 0u8).unwrap();
        let _unreachable = grid[TilePos::new(5, 5)];
    }
}
