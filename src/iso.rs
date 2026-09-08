//! The isometric projection: converting between grid space and screen space.
//!
//! Grid space is the world as the simulation thinks of it — `x` runs south-east
//! across the screen, `y` runs south-west, and `z` is height above the ground
//! plane. Screen space is pixels, `y` growing downward, as every 2D backend
//! expects.
//!
//! A tile is a rhombus. In the classic 2:1 projection it is twice as wide as it
//! is tall, which makes the maths cheap and the pixels line up:
//!
//! ```text
//!            (0,0)
//!          .        .          x
//!        .            .      /
//!      .      tile      .  /
//!        .            .
//!          .        .    \
//!            .    .        \
//!                            y
//! ```
//!
//! ```
//! use isogrid::iso::{GridPoint, TileSize};
//!
//! let tiles = TileSize::CLASSIC;
//! let origin = tiles.grid_to_screen(GridPoint::new(0.0, 0.0, 0.0));
//! assert_eq!((origin.x, origin.y), (0.0, 0.0));
//!
//! // One step along +x moves half a tile right and half a tile down.
//! let east = tiles.grid_to_screen(GridPoint::new(1.0, 0.0, 0.0));
//! assert_eq!((east.x, east.y), (32.0, 16.0));
//! ```

use crate::error::{Error, Result};

/// A point in continuous grid space.
///
/// Whole numbers land on tile corners; `(0.5, 0.5, 0.0)` is the centre of the
/// tile at the origin. `z` is measured in elevation steps, not pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GridPoint {
    /// Distance along the south-east axis, in tiles.
    pub x: f32,
    /// Distance along the south-west axis, in tiles.
    pub y: f32,
    /// Height above the ground plane, in elevation steps.
    pub z: f32,
}

impl GridPoint {
    /// The origin of grid space.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Builds a grid point.
    ///
    /// ```
    /// # use isogrid::iso::GridPoint;
    /// let p = GridPoint::new(2.0, 3.0, 1.0);
    /// assert_eq!(p.y, 3.0);
    /// ```
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Builds a grid point on the ground plane.
    ///
    /// ```
    /// # use isogrid::iso::GridPoint;
    /// assert_eq!(GridPoint::ground(2.0, 3.0), GridPoint::new(2.0, 3.0, 0.0));
    /// ```
    pub const fn ground(x: f32, y: f32) -> Self {
        Self::new(x, y, 0.0)
    }

    /// The tile containing this point.
    ///
    /// Rounds toward negative infinity, so the tile to the north-west of the
    /// origin is `(-1, -1)` rather than `(0, 0)`.
    ///
    /// ```
    /// # use isogrid::iso::{GridPoint, TilePos};
    /// assert_eq!(GridPoint::ground(2.7, 3.1).tile(), TilePos::new(2, 3));
    /// assert_eq!(GridPoint::ground(-0.5, -0.5).tile(), TilePos::new(-1, -1));
    /// ```
    #[allow(clippy::cast_possible_truncation)]
    pub fn tile(self) -> TilePos {
        TilePos::new(self.x.floor() as i32, self.y.floor() as i32)
    }
}

impl From<GridPoint> for glam::Vec3 {
    fn from(p: GridPoint) -> Self {
        Self::new(p.x, p.y, p.z)
    }
}

impl From<glam::Vec3> for GridPoint {
    fn from(v: glam::Vec3) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

/// A point in screen space, in pixels, with `y` growing downward.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScreenPoint {
    /// Horizontal offset in pixels, growing to the right.
    pub x: f32,
    /// Vertical offset in pixels, growing downward.
    pub y: f32,
}

impl ScreenPoint {
    /// The origin of screen space.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Builds a screen point.
    ///
    /// ```
    /// # use isogrid::iso::ScreenPoint;
    /// let p = ScreenPoint::new(10.0, -4.0);
    /// assert_eq!(p.x, 10.0);
    /// ```
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<ScreenPoint> for glam::Vec2 {
    fn from(p: ScreenPoint) -> Self {
        Self::new(p.x, p.y)
    }
}

impl From<glam::Vec2> for ScreenPoint {
    fn from(v: glam::Vec2) -> Self {
        Self::new(v.x, v.y)
    }
}

/// A discrete tile on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TilePos {
    /// Tile index along the south-east axis.
    pub x: i32,
    /// Tile index along the south-west axis.
    pub y: i32,
}

impl TilePos {
    /// The tile at the grid origin.
    pub const ORIGIN: Self = Self { x: 0, y: 0 };

    /// The four tiles sharing an edge with any tile, as offsets.
    pub const NEIGHBOURS: [Self; 4] = [
        Self { x: 1, y: 0 },
        Self { x: 0, y: 1 },
        Self { x: -1, y: 0 },
        Self { x: 0, y: -1 },
    ];

    /// Builds a tile position.
    ///
    /// ```
    /// # use isogrid::iso::TilePos;
    /// assert_eq!(TilePos::new(1, 2).x, 1);
    /// ```
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// The centre of this tile, on the ground plane.
    ///
    /// ```
    /// # use isogrid::iso::{GridPoint, TilePos};
    /// assert_eq!(TilePos::new(1, 1).centre(), GridPoint::ground(1.5, 1.5));
    /// ```
    #[allow(clippy::cast_precision_loss)]
    pub fn centre(self) -> GridPoint {
        GridPoint::ground(self.x as f32 + 0.5, self.y as f32 + 0.5)
    }

    /// The corner of this tile closest to the grid origin, on the ground plane.
    ///
    /// ```
    /// # use isogrid::iso::{GridPoint, TilePos};
    /// assert_eq!(TilePos::new(1, 1).corner(), GridPoint::ground(1.0, 1.0));
    /// ```
    #[allow(clippy::cast_precision_loss)]
    pub fn corner(self) -> GridPoint {
        GridPoint::ground(self.x as f32, self.y as f32)
    }

    /// This tile offset by `dx` and `dy`.
    ///
    /// Saturates rather than wrapping at the edges of `i32`.
    ///
    /// ```
    /// # use isogrid::iso::TilePos;
    /// assert_eq!(TilePos::new(1, 1).offset(2, -1), TilePos::new(3, 0));
    /// ```
    #[must_use]
    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Self::new(self.x.saturating_add(dx), self.y.saturating_add(dy))
    }

    /// The four tiles sharing an edge with this one.
    ///
    /// ```
    /// # use isogrid::iso::TilePos;
    /// let around = TilePos::ORIGIN.neighbours();
    /// assert!(around.contains(&TilePos::new(0, -1)));
    /// assert_eq!(around.len(), 4);
    /// ```
    pub fn neighbours(self) -> [Self; 4] {
        Self::NEIGHBOURS.map(|d| self.offset(d.x, d.y))
    }

    /// The number of edge-to-edge steps between two tiles.
    ///
    /// ```
    /// # use isogrid::iso::TilePos;
    /// assert_eq!(TilePos::ORIGIN.manhattan_distance(TilePos::new(2, -3)), 5);
    /// ```
    pub fn manhattan_distance(self, other: Self) -> u32 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
    }
}

/// The pixel dimensions of one tile, and of one step of elevation.
///
/// This is the whole projection: every conversion between grid and screen space
/// is derived from these three numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TileSize {
    width: f32,
    height: f32,
    elevation: f32,
}

impl TileSize {
    /// The classic 2:1 tile: 64 pixels wide, 32 tall, 8 pixels per height step.
    pub const CLASSIC: Self = Self {
        width: 64.0,
        height: 32.0,
        elevation: 8.0,
    };

    /// Builds a tile size.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidTileSize`] unless `width` and `height` are
    /// finite and strictly positive and `elevation` is finite and not negative.
    /// A zero `elevation` is allowed: it flattens the world onto the ground
    /// plane, which is occasionally what you want.
    ///
    /// ```
    /// # use isogrid::iso::TileSize;
    /// let tiles = TileSize::new(32.0, 16.0, 4.0)?;
    /// assert_eq!(tiles.width(), 32.0);
    /// assert!(TileSize::new(0.0, 16.0, 4.0).is_err());
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn new(width: f32, height: f32, elevation: f32) -> Result<Self> {
        let positive = |v: f32| v.is_finite() && v > 0.0;
        if !positive(width) || !positive(height) || !elevation.is_finite() || elevation < 0.0 {
            return Err(Error::InvalidTileSize {
                width,
                height,
                elevation,
            });
        }
        Ok(Self {
            width,
            height,
            elevation,
        })
    }

    /// Builds a 2:1 tile `width` pixels across, with elevation steps an eighth
    /// of that.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidTileSize`] if `width` is not finite and
    /// positive.
    ///
    /// ```
    /// # use isogrid::iso::TileSize;
    /// assert_eq!(TileSize::square(64.0)?, TileSize::CLASSIC);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn square(width: f32) -> Result<Self> {
        Self::new(width, width / 2.0, width / 8.0)
    }

    /// The width of one tile in pixels.
    pub const fn width(self) -> f32 {
        self.width
    }

    /// The height of one tile in pixels.
    pub const fn height(self) -> f32 {
        self.height
    }

    /// The pixel height of one step of elevation.
    pub const fn elevation(self) -> f32 {
        self.elevation
    }

    /// Projects a grid point onto the screen.
    ///
    /// ```
    /// # use isogrid::iso::{GridPoint, TileSize};
    /// let tiles = TileSize::CLASSIC;
    /// // Moving one tile south-east and one south-west lands straight below.
    /// let south = tiles.grid_to_screen(GridPoint::ground(1.0, 1.0));
    /// assert_eq!((south.x, south.y), (0.0, 32.0));
    /// // Height lifts a point up the screen.
    /// let raised = tiles.grid_to_screen(GridPoint::new(0.0, 0.0, 2.0));
    /// assert_eq!((raised.x, raised.y), (0.0, -16.0));
    /// ```
    pub fn grid_to_screen(self, point: GridPoint) -> ScreenPoint {
        ScreenPoint::new(
            (point.x - point.y) * (self.width / 2.0),
            (point.x + point.y).mul_add(self.height / 2.0, -(point.z * self.elevation)),
        )
    }

    /// Unprojects a screen point back onto the plane at height `z`.
    ///
    /// The projection throws away a dimension, so the inverse needs it back:
    /// pass the height of the plane you are picking against, usually `0.0` for
    /// the ground or the height of the tile under the cursor.
    ///
    /// ```
    /// # use isogrid::iso::{GridPoint, ScreenPoint, TileSize};
    /// let tiles = TileSize::CLASSIC;
    /// let point = GridPoint::new(3.0, -2.0, 1.0);
    /// let back = tiles.screen_to_grid(tiles.grid_to_screen(point), 1.0);
    /// assert!((back.x - point.x).abs() < 1e-4);
    /// assert!((back.y - point.y).abs() < 1e-4);
    /// ```
    pub fn screen_to_grid(self, point: ScreenPoint, z: f32) -> GridPoint {
        let half_width = self.width / 2.0;
        let half_height = self.height / 2.0;
        let ground_y = z.mul_add(self.elevation, point.y);
        let sx = point.x / half_width;
        let sy = ground_y / half_height;
        GridPoint::new(f32::midpoint(sy, sx), f32::midpoint(sy, -sx), z)
    }

    /// The tile under a screen point, assuming flat ground at height `z`.
    ///
    /// ```
    /// # use isogrid::iso::{ScreenPoint, TilePos, TileSize};
    /// let tiles = TileSize::CLASSIC;
    /// assert_eq!(tiles.pick_tile(ScreenPoint::new(0.0, 16.0), 0.0), TilePos::ORIGIN);
    /// ```
    pub fn pick_tile(self, point: ScreenPoint, z: f32) -> TilePos {
        self.screen_to_grid(point, z).tile()
    }

    /// The painter's-algorithm sort key for a grid point.
    ///
    /// Larger values are nearer the viewer and must be drawn later. Ordering by
    /// this key draws a scene back to front.
    ///
    /// ```
    /// # use isogrid::iso::{GridPoint, TileSize};
    /// let tiles = TileSize::CLASSIC;
    /// let behind = tiles.depth(GridPoint::ground(0.0, 0.0));
    /// let in_front = tiles.depth(GridPoint::ground(1.0, 1.0));
    /// assert!(in_front > behind);
    /// ```
    pub fn depth(self, point: GridPoint) -> f32 {
        // Height breaks ties between tiles at the same ground depth: a raised
        // object on a tile is drawn after the tile itself.
        point.z.mul_add(0.001, point.x + point.y)
    }
}

impl Default for TileSize {
    fn default() -> Self {
        Self::CLASSIC
    }
}

#[cfg(test)]
mod tests {
    // The projection is exact for the powers of two used here, so comparing
    // floats for equality is the assertion we actually want.
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn classic_tiles_are_two_to_one() {
        let tiles = TileSize::CLASSIC;
        assert_eq!(tiles.width(), tiles.height() * 2.0);
    }

    #[test]
    fn square_matches_the_classic_tile() {
        assert_eq!(TileSize::square(64.0).unwrap(), TileSize::CLASSIC);
    }

    #[test]
    fn rejects_degenerate_tile_sizes() {
        assert!(TileSize::new(0.0, 16.0, 4.0).is_err());
        assert!(TileSize::new(32.0, -1.0, 4.0).is_err());
        assert!(TileSize::new(32.0, 16.0, -1.0).is_err());
        assert!(TileSize::new(f32::NAN, 16.0, 4.0).is_err());
        assert!(TileSize::new(f32::INFINITY, 16.0, 4.0).is_err());
    }

    #[test]
    fn allows_a_flat_world() {
        let flat = TileSize::new(64.0, 32.0, 0.0).unwrap();
        let low = flat.grid_to_screen(GridPoint::ground(0.0, 0.0));
        let high = flat.grid_to_screen(GridPoint::new(0.0, 0.0, 10.0));
        assert_eq!(low, high);
    }

    #[test]
    fn the_origin_projects_to_the_origin() {
        assert_eq!(
            TileSize::CLASSIC.grid_to_screen(GridPoint::ZERO),
            ScreenPoint::ZERO
        );
    }

    #[test]
    fn the_four_corners_of_a_tile_form_a_rhombus() {
        let tiles = TileSize::CLASSIC;
        let north = tiles.grid_to_screen(GridPoint::ground(0.0, 0.0));
        let east = tiles.grid_to_screen(GridPoint::ground(1.0, 0.0));
        let south = tiles.grid_to_screen(GridPoint::ground(1.0, 1.0));
        let west = tiles.grid_to_screen(GridPoint::ground(0.0, 1.0));

        assert_eq!(east.x - north.x, tiles.width() / 2.0);
        assert_eq!(south.y - north.y, tiles.height());
        assert_eq!(west.x, -east.x);
        assert_eq!(east.y, west.y);
    }

    #[test]
    fn tiles_floor_toward_negative_infinity() {
        assert_eq!(GridPoint::ground(-0.1, -0.1).tile(), TilePos::new(-1, -1));
        assert_eq!(GridPoint::ground(0.0, 0.0).tile(), TilePos::ORIGIN);
    }

    #[test]
    fn neighbours_are_one_step_away() {
        for neighbour in TilePos::new(4, 4).neighbours() {
            assert_eq!(TilePos::new(4, 4).manhattan_distance(neighbour), 1);
        }
    }

    #[test]
    fn offsets_saturate_at_the_edge_of_the_grid() {
        assert_eq!(TilePos::new(i32::MAX, 0).offset(1, 0).x, i32::MAX);
        assert_eq!(TilePos::new(i32::MIN, 0).offset(-1, 0).x, i32::MIN);
    }

    #[test]
    fn depth_orders_back_to_front() {
        let tiles = TileSize::CLASSIC;
        let mut points = [
            GridPoint::ground(2.0, 2.0),
            GridPoint::ground(0.0, 0.0),
            GridPoint::ground(1.0, 0.0),
        ];
        points.sort_by(|a, b| tiles.depth(*a).total_cmp(&tiles.depth(*b)));
        assert_eq!(points[0], GridPoint::ground(0.0, 0.0));
        assert_eq!(points[2], GridPoint::ground(2.0, 2.0));
    }

    #[test]
    fn raised_objects_draw_after_their_tile() {
        let tiles = TileSize::CLASSIC;
        let ground = tiles.depth(GridPoint::ground(1.0, 1.0));
        let above = tiles.depth(GridPoint::new(1.0, 1.0, 3.0));
        assert!(above > ground);
    }
}
