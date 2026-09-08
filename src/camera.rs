//! The camera: what part of the world is on screen, and how big it looks.
//!
//! A [`Camera`] sits over the world at a grid position, at some zoom, looking
//! through a viewport of a given pixel size. It converts between world and
//! screen coordinates with the pan and zoom folded in, and it can name the
//! tiles it might be able to see so the renderer can skip the rest.
//!
//! ```
//! use isogrid::camera::{Camera, Viewport};
//! use isogrid::iso::{GridPoint, TileSize};
//!
//! let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0)?);
//! camera.look_at(GridPoint::ground(10.0, 10.0));
//!
//! // Whatever the camera is looking at lands in the middle of the viewport.
//! let centre = camera.world_to_screen(GridPoint::ground(10.0, 10.0));
//! assert_eq!((centre.x, centre.y), (400.0, 300.0));
//! # Ok::<(), isogrid::Error>(())
//! ```

use crate::error::{Error, Result};
use crate::grid::TileBounds;
use crate::iso::{GridPoint, ScreenPoint, TilePos, TileSize};

/// The pixel size of the window, or of the region being drawn into.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Viewport {
    width: f32,
    height: f32,
}

impl Viewport {
    /// Builds a viewport.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidViewport`] if either dimension is not finite and
    /// positive.
    ///
    /// ```
    /// # use isogrid::camera::Viewport;
    /// assert!(Viewport::new(1280.0, 720.0).is_ok());
    /// assert!(Viewport::new(0.0, 720.0).is_err());
    /// ```
    pub fn new(width: f32, height: f32) -> Result<Self> {
        let positive = |v: f32| v.is_finite() && v > 0.0;
        if !positive(width) || !positive(height) {
            return Err(Error::InvalidViewport { width, height });
        }
        Ok(Self { width, height })
    }

    /// The width of the viewport in pixels.
    pub const fn width(self) -> f32 {
        self.width
    }

    /// The height of the viewport in pixels.
    pub const fn height(self) -> f32 {
        self.height
    }

    /// The middle of the viewport, in screen coordinates.
    ///
    /// ```
    /// # use isogrid::camera::Viewport;
    /// let centre = Viewport::new(800.0, 600.0)?.centre();
    /// assert_eq!((centre.x, centre.y), (400.0, 300.0));
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn centre(self) -> ScreenPoint {
        ScreenPoint::new(self.width / 2.0, self.height / 2.0)
    }
}

/// The range a camera's zoom is allowed to move within.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ZoomRange {
    min: f32,
    max: f32,
}

impl ZoomRange {
    /// A quarter-size to eight-times range, which suits most tile sizes.
    pub const DEFAULT: Self = Self {
        min: 0.25,
        max: 8.0,
    };

    /// Builds a zoom range.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidZoomRange`] unless both bounds are finite and
    /// strictly positive, with `min` no greater than `max`. Zero is excluded:
    /// a camera zoomed to nothing has no invertible projection.
    ///
    /// ```
    /// # use isogrid::camera::ZoomRange;
    /// assert!(ZoomRange::new(0.5, 4.0).is_ok());
    /// assert!(ZoomRange::new(4.0, 0.5).is_err());
    /// assert!(ZoomRange::new(0.0, 4.0).is_err());
    /// ```
    pub fn new(min: f32, max: f32) -> Result<Self> {
        let positive = |v: f32| v.is_finite() && v > 0.0;
        if !positive(min) || !positive(max) || min > max {
            return Err(Error::InvalidZoomRange { min, max });
        }
        Ok(Self { min, max })
    }

    /// The closest the camera may pull back.
    pub const fn min(self) -> f32 {
        self.min
    }

    /// The furthest the camera may push in.
    pub const fn max(self) -> f32 {
        self.max
    }

    /// `zoom` brought inside the range.
    ///
    /// A `NaN` zoom clamps to the minimum rather than propagating.
    ///
    /// ```
    /// # use isogrid::camera::ZoomRange;
    /// let range = ZoomRange::new(0.5, 4.0)?;
    /// assert_eq!(range.clamp(10.0), 4.0);
    /// assert_eq!(range.clamp(f32::NAN), 0.5);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn clamp(self, zoom: f32) -> f32 {
        if zoom.is_nan() {
            return self.min;
        }
        zoom.clamp(self.min, self.max)
    }
}

impl Default for ZoomRange {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A view onto the world: where it is looking, how far in, and how big the
/// window is.
///
/// The camera owns the [`TileSize`] because the projection and the zoom are
/// applied together — asking the camera to convert a point is the only
/// conversion a renderer should need.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Camera {
    tiles: TileSize,
    viewport: Viewport,
    focus: GridPoint,
    zoom: f32,
    zoom_range: ZoomRange,
}

impl Camera {
    /// Builds a camera looking at the grid origin at 1× zoom.
    ///
    /// ```
    /// # use isogrid::camera::{Camera, Viewport};
    /// # use isogrid::iso::TileSize;
    /// let camera = Camera::new(TileSize::CLASSIC, Viewport::new(640.0, 480.0)?);
    /// assert_eq!(camera.zoom(), 1.0);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn new(tiles: TileSize, viewport: Viewport) -> Self {
        Self {
            tiles,
            viewport,
            focus: GridPoint::ZERO,
            zoom: 1.0,
            zoom_range: ZoomRange::DEFAULT,
        }
    }

    /// The tile size this camera projects with.
    pub const fn tiles(&self) -> TileSize {
        self.tiles
    }

    /// The viewport this camera draws into.
    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// The point the camera is centred on.
    pub const fn focus(&self) -> GridPoint {
        self.focus
    }

    /// The current zoom factor, where `1.0` draws tiles at their natural size.
    pub const fn zoom(&self) -> f32 {
        self.zoom
    }

    /// The range the zoom is clamped to.
    pub const fn zoom_range(&self) -> ZoomRange {
        self.zoom_range
    }

    /// Points the camera at `focus`.
    pub fn look_at(&mut self, focus: GridPoint) {
        self.focus = focus;
    }

    /// Moves the camera by a distance in grid space.
    ///
    /// ```
    /// # use isogrid::camera::{Camera, Viewport};
    /// # use isogrid::iso::{GridPoint, TileSize};
    /// let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(640.0, 480.0)?);
    /// camera.pan(1.0, 2.0);
    /// assert_eq!(camera.focus(), GridPoint::ground(1.0, 2.0));
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.focus = GridPoint::new(self.focus.x + dx, self.focus.y + dy, self.focus.z);
    }

    /// Moves the camera by a distance in pixels, as a mouse drag would.
    ///
    /// Dragging the world right moves the camera left, so the point under the
    /// cursor stays under the cursor.
    ///
    /// ```
    /// # use isogrid::camera::{Camera, Viewport};
    /// # use isogrid::iso::{GridPoint, ScreenPoint, TileSize};
    /// let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(640.0, 480.0)?);
    /// let anchor = GridPoint::ground(3.0, 4.0);
    /// let before = camera.world_to_screen(anchor);
    ///
    /// camera.drag(ScreenPoint::new(40.0, -20.0));
    ///
    /// let after = camera.world_to_screen(anchor);
    /// assert!((after.x - (before.x + 40.0)).abs() < 1e-3);
    /// assert!((after.y - (before.y - 20.0)).abs() < 1e-3);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn drag(&mut self, delta: ScreenPoint) {
        let unzoomed = ScreenPoint::new(-delta.x / self.zoom, -delta.y / self.zoom);
        let origin = self.tiles.screen_to_grid(ScreenPoint::ZERO, 0.0);
        let moved = self.tiles.screen_to_grid(unzoomed, 0.0);
        self.pan(moved.x - origin.x, moved.y - origin.y);
    }

    /// Replaces the zoom range, clamping the current zoom into it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidZoomRange`] if the range is not valid.
    ///
    /// ```
    /// # use isogrid::camera::{Camera, Viewport};
    /// # use isogrid::iso::TileSize;
    /// let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(640.0, 480.0)?);
    /// camera.set_zoom(8.0);
    /// camera.set_zoom_range(0.5, 2.0)?;
    /// assert_eq!(camera.zoom(), 2.0); // pulled back into the new range
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn set_zoom_range(&mut self, min: f32, max: f32) -> Result<()> {
        self.zoom_range = ZoomRange::new(min, max)?;
        self.zoom = self.zoom_range.clamp(self.zoom);
        Ok(())
    }

    /// Sets the zoom, clamped to the camera's range.
    ///
    /// ```
    /// # use isogrid::camera::{Camera, Viewport};
    /// # use isogrid::iso::TileSize;
    /// let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(640.0, 480.0)?);
    /// camera.set_zoom(1000.0);
    /// assert_eq!(camera.zoom(), camera.zoom_range().max());
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = self.zoom_range.clamp(zoom);
    }

    /// Multiplies the zoom by `factor`, clamped to the camera's range.
    pub fn zoom_by(&mut self, factor: f32) {
        self.set_zoom(self.zoom * factor);
    }

    /// Zooms while keeping whatever is under `anchor` under `anchor`.
    ///
    /// This is what a scroll wheel should do: the world grows or shrinks around
    /// the cursor rather than around the middle of the window.
    ///
    /// ```
    /// # use isogrid::camera::{Camera, Viewport};
    /// # use isogrid::iso::{ScreenPoint, TileSize};
    /// let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0)?);
    /// let cursor = ScreenPoint::new(700.0, 120.0);
    /// let under_cursor = camera.screen_to_world(cursor, 0.0);
    ///
    /// camera.zoom_towards(cursor, 2.0);
    ///
    /// let still_there = camera.screen_to_world(cursor, 0.0);
    /// assert!((still_there.x - under_cursor.x).abs() < 1e-3);
    /// assert!((still_there.y - under_cursor.y).abs() < 1e-3);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn zoom_towards(&mut self, anchor: ScreenPoint, factor: f32) {
        let before = self.screen_to_world(anchor, self.focus.z);
        self.zoom_by(factor);
        let after = self.screen_to_world(anchor, self.focus.z);
        self.pan(before.x - after.x, before.y - after.y);
    }

    /// Projects a world point to a pixel in the viewport.
    pub fn world_to_screen(&self, point: GridPoint) -> ScreenPoint {
        let point = self.tiles.grid_to_screen(point);
        let focus = self.tiles.grid_to_screen(self.focus);
        let centre = self.viewport.centre();
        ScreenPoint::new(
            (point.x - focus.x).mul_add(self.zoom, centre.x),
            (point.y - focus.y).mul_add(self.zoom, centre.y),
        )
    }

    /// Unprojects a pixel in the viewport onto the plane at height `z`.
    ///
    /// ```
    /// # use isogrid::camera::{Camera, Viewport};
    /// # use isogrid::iso::{GridPoint, TileSize};
    /// let camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0)?);
    /// let point = GridPoint::ground(4.0, -7.0);
    /// let round_trip = camera.screen_to_world(camera.world_to_screen(point), 0.0);
    /// assert!((round_trip.x - point.x).abs() < 1e-3);
    /// assert!((round_trip.y - point.y).abs() < 1e-3);
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn screen_to_world(&self, point: ScreenPoint, z: f32) -> GridPoint {
        let centre = self.viewport.centre();
        let focus = self.tiles.grid_to_screen(self.focus);
        let unzoomed = ScreenPoint::new(
            (point.x - centre.x) / self.zoom + focus.x,
            (point.y - centre.y) / self.zoom + focus.y,
        );
        self.tiles.screen_to_grid(unzoomed, z)
    }

    /// The tile under a pixel, assuming flat ground at height `z`.
    pub fn pick_tile(&self, point: ScreenPoint, z: f32) -> TilePos {
        self.screen_to_world(point, z).tile()
    }

    /// The tiles that could appear in the viewport.
    ///
    /// A conservative over-estimate: unprojecting the four corners of the
    /// viewport gives a diamond in grid space, and this returns its bounding
    /// box. `height_margin` grows it by that many tiles in every direction, to
    /// account for tall terrain or objects whose base sits off screen but whose
    /// top does not.
    ///
    /// ```
    /// # use isogrid::camera::{Camera, Viewport};
    /// # use isogrid::iso::TileSize;
    /// let camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0)?);
    /// let visible = camera.visible_tiles(0);
    /// assert!(visible.contains(camera.focus().tile()));
    /// # Ok::<(), isogrid::Error>(())
    /// ```
    pub fn visible_tiles(&self, height_margin: u32) -> TileBounds {
        let corners = [
            ScreenPoint::ZERO,
            ScreenPoint::new(self.viewport.width, 0.0),
            ScreenPoint::new(0.0, self.viewport.height),
            ScreenPoint::new(self.viewport.width, self.viewport.height),
        ]
        .map(|corner| self.screen_to_world(corner, 0.0).tile());

        let bounds = corners.iter().skip(1).fold(
            TileBounds::new(corners[0], corners[0]),
            |bounds, corner| {
                TileBounds::new(
                    TilePos::new(bounds.min().x.min(corner.x), bounds.min().y.min(corner.y)),
                    TilePos::new(bounds.max().x.max(corner.x), bounds.max().y.max(corner.y)),
                )
            },
        );

        // One extra tile absorbs the rounding done by `TilePos`, on top of
        // whatever the caller asked for.
        bounds.expanded(height_margin.saturating_add(1))
    }

    /// Resizes the viewport, keeping the camera pointed where it was.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidViewport`] if either dimension is not finite and
    /// positive.
    pub fn resize(&mut self, width: f32, height: f32) -> Result<()> {
        self.viewport = Viewport::new(width, height)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Pan and projection are exact for the values used here; the tests that
    // involve zoom use explicit tolerances.
    #![allow(clippy::float_cmp)]

    use super::*;

    fn camera() -> Camera {
        Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0).unwrap())
    }

    #[test]
    fn rejects_degenerate_viewports() {
        assert!(Viewport::new(0.0, 600.0).is_err());
        assert!(Viewport::new(800.0, -1.0).is_err());
        assert!(Viewport::new(f32::NAN, 600.0).is_err());
    }

    #[test]
    fn the_focus_lands_in_the_middle_of_the_viewport() {
        let mut camera = camera();
        camera.look_at(GridPoint::ground(12.0, -5.0));
        let centre = camera.world_to_screen(camera.focus());
        assert_eq!(centre, camera.viewport().centre());
    }

    #[test]
    fn zoom_is_clamped_to_the_range() {
        let mut camera = camera();
        camera.set_zoom(1000.0);
        assert_eq!(camera.zoom(), ZoomRange::DEFAULT.max());
        camera.set_zoom(0.0);
        assert_eq!(camera.zoom(), ZoomRange::DEFAULT.min());
        camera.set_zoom(f32::NAN);
        assert_eq!(camera.zoom(), ZoomRange::DEFAULT.min());
    }

    #[test]
    fn narrowing_the_range_pulls_the_zoom_in_with_it() {
        let mut camera = camera();
        camera.set_zoom(8.0);
        camera.set_zoom_range(0.5, 2.0).unwrap();
        assert_eq!(camera.zoom(), 2.0);
    }

    #[test]
    fn rejects_inverted_zoom_ranges() {
        let mut camera = camera();
        assert!(camera.set_zoom_range(4.0, 1.0).is_err());
        assert!(camera.set_zoom_range(0.0, 1.0).is_err());
        assert_eq!(
            camera.zoom_range(),
            ZoomRange::DEFAULT,
            "a rejected range changes nothing"
        );
    }

    #[test]
    fn zoom_does_not_move_the_focus() {
        let mut camera = camera();
        camera.look_at(GridPoint::ground(3.0, 3.0));
        camera.set_zoom(4.0);
        assert_eq!(
            camera.world_to_screen(camera.focus()),
            camera.viewport().centre()
        );
    }

    #[test]
    fn zooming_towards_a_corner_keeps_that_corner_still() {
        let mut camera = camera();
        let corner = ScreenPoint::new(0.0, 0.0);
        let before = camera.screen_to_world(corner, 0.0);
        camera.zoom_towards(corner, 0.5);
        let after = camera.screen_to_world(corner, 0.0);
        assert!((after.x - before.x).abs() < 1e-3);
        assert!((after.y - before.y).abs() < 1e-3);
    }

    #[test]
    fn zooming_towards_a_clamped_limit_does_not_drift() {
        let mut camera = camera();
        camera.set_zoom(ZoomRange::DEFAULT.max());
        let cursor = ScreenPoint::new(650.0, 90.0);
        let before = camera.screen_to_world(cursor, 0.0);
        camera.zoom_towards(cursor, 4.0); // already at the limit: a no-op
        let after = camera.screen_to_world(cursor, 0.0);
        assert!((after.x - before.x).abs() < 1e-3);
        assert!((after.y - before.y).abs() < 1e-3);
    }

    #[test]
    fn dragging_moves_the_world_with_the_cursor() {
        let mut camera = camera();
        camera.set_zoom(2.0);
        let anchor = GridPoint::ground(2.0, 6.0);
        let before = camera.world_to_screen(anchor);
        camera.drag(ScreenPoint::new(-30.0, 15.0));
        let after = camera.world_to_screen(anchor);
        assert!((after.x - (before.x - 30.0)).abs() < 1e-3);
        assert!((after.y - (before.y + 15.0)).abs() < 1e-3);
    }

    #[test]
    fn the_visible_region_covers_every_corner_of_the_viewport() {
        let mut camera = camera();
        camera.look_at(GridPoint::ground(20.0, 20.0));
        let visible = camera.visible_tiles(0);
        for corner in [
            ScreenPoint::ZERO,
            ScreenPoint::new(800.0, 0.0),
            ScreenPoint::new(0.0, 600.0),
            ScreenPoint::new(800.0, 600.0),
            ScreenPoint::new(400.0, 300.0),
        ] {
            assert!(
                visible.contains(camera.pick_tile(corner, 0.0)),
                "{corner:?} was culled"
            );
        }
    }

    #[test]
    fn zooming_in_narrows_the_visible_region() {
        let mut camera = camera();
        let wide = camera.visible_tiles(0).len();
        camera.set_zoom(4.0);
        assert!(camera.visible_tiles(0).len() < wide);
    }

    #[test]
    fn resizing_keeps_the_focus_centred() {
        let mut camera = camera();
        camera.look_at(GridPoint::ground(7.0, 7.0));
        camera.resize(1024.0, 768.0).unwrap();
        assert_eq!(
            camera.world_to_screen(camera.focus()),
            camera.viewport().centre()
        );
        assert!(camera.resize(0.0, 768.0).is_err());
    }
}
