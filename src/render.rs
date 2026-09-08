//! The drawing boundary.
//!
//! The engine does not draw anything. It describes what to draw, in screen
//! coordinates, in the right order, and hands that to a [`Renderer`] — a trait
//! small enough that a backend is a few hundred lines and a test double is
//! twenty.
//!
//! That is the point. Culling, ordering and projection are geometry and belong
//! to the simulation side, where they can be tested without a window. Only the
//! last step — turning a rhombus and a colour into pixels — needs a graphics
//! library, and swapping that library must not be visible to anything above it.
//!
//! ```
//! use isogrid::camera::{Camera, Viewport};
//! use isogrid::grid::Grid;
//! use isogrid::iso::TileSize;
//! use isogrid::render::{draw_tiles, Color, Recorder};
//!
//! let camera = Camera::new(TileSize::CLASSIC, Viewport::new(320.0, 240.0)?);
//! let park = Grid::filled(8, 8, Color::rgb(90, 140, 70))?;
//!
//! // A recorder stands in for a real backend: same trait, no window.
//! let mut canvas = Recorder::new();
//! draw_tiles(&mut canvas, &camera, &park, |_, colour| Some(*colour));
//!
//! assert!(!canvas.commands().is_empty());
//! # Ok::<(), isogrid::Error>(())
//! ```

use crate::camera::Camera;
use crate::grid::Grid;
use crate::iso::{ScreenPoint, TilePos};

/// A colour, in sRGB with straight alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Color {
    /// Red, from 0 to 255.
    pub r: u8,
    /// Green, from 0 to 255.
    pub g: u8,
    /// Blue, from 0 to 255.
    pub b: u8,
    /// Opacity, where 0 is invisible and 255 is solid.
    pub a: u8,
}

impl Color {
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);
    /// Solid black.
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    /// Solid white.
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    /// An opaque colour.
    ///
    /// ```
    /// # use isogrid::render::Color;
    /// assert_eq!(Color::rgb(10, 20, 30).a, 255);
    /// ```
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    /// A colour with explicit opacity.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// An opaque colour from a `0xRRGGBB` literal.
    ///
    /// ```
    /// # use isogrid::render::Color;
    /// assert_eq!(Color::hex(0x4C_8C_46), Color::rgb(76, 140, 70));
    /// ```
    #[allow(clippy::cast_possible_truncation)] // Each shift keeps one byte.
    pub const fn hex(rgb: u32) -> Self {
        Self::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    }

    /// The same colour at a different opacity.
    ///
    /// ```
    /// # use isogrid::render::Color;
    /// assert_eq!(Color::WHITE.with_alpha(128).a, 128);
    /// ```
    #[must_use]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// The colour scaled toward black, for cheap shading.
    ///
    /// `factor` is clamped to `0.0..=1.0`; alpha is untouched.
    ///
    /// ```
    /// # use isogrid::render::Color;
    /// assert_eq!(Color::rgb(200, 100, 50).shaded(0.5), Color::rgb(100, 50, 25));
    /// assert_eq!(Color::WHITE.shaded(2.0), Color::WHITE, "the factor is clamped");
    /// ```
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_lossless
    )]
    pub fn shaded(self, factor: f32) -> Self {
        let factor = if factor.is_nan() {
            0.0
        } else {
            factor.clamp(0.0, 1.0)
        };
        let scale = |channel: u8| (f32::from(channel) * factor) as u8;
        Self {
            r: scale(self.r),
            g: scale(self.g),
            b: scale(self.b),
            a: self.a,
        }
    }
}

/// The rhombus that one tile occupies on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileShape {
    /// The middle of the rhombus.
    pub centre: ScreenPoint,
    /// The full width, corner to corner.
    pub width: f32,
    /// The full height, corner to corner.
    pub height: f32,
}

impl TileShape {
    /// The four corners, clockwise from the top.
    ///
    /// ```
    /// # use isogrid::iso::ScreenPoint;
    /// # use isogrid::render::TileShape;
    /// let shape = TileShape { centre: ScreenPoint::ZERO, width: 64.0, height: 32.0 };
    /// let [top, right, bottom, left] = shape.corners();
    /// assert_eq!((top.x, top.y), (0.0, -16.0));
    /// assert_eq!((right.x, right.y), (32.0, 0.0));
    /// assert_eq!(left.x, -right.x);
    /// assert_eq!(bottom.y, -top.y);
    /// ```
    pub fn corners(self) -> [ScreenPoint; 4] {
        let (dx, dy) = (self.width / 2.0, self.height / 2.0);
        [
            ScreenPoint::new(self.centre.x, self.centre.y - dy),
            ScreenPoint::new(self.centre.x + dx, self.centre.y),
            ScreenPoint::new(self.centre.x, self.centre.y + dy),
            ScreenPoint::new(self.centre.x - dx, self.centre.y),
        ]
    }
}

/// Somewhere pixels can be put.
///
/// Implement this over a graphics library to give the engine a backend, or use
/// [`Recorder`] to test drawing code without one. Every position is already in
/// screen coordinates: a renderer never projects, culls or sorts.
pub trait Renderer {
    /// Paints the whole surface a single colour.
    fn clear(&mut self, colour: Color);

    /// Fills a tile-shaped rhombus.
    fn fill_tile(&mut self, shape: TileShape, colour: Color);

    /// Outlines a tile-shaped rhombus.
    fn stroke_tile(&mut self, shape: TileShape, thickness: f32, colour: Color);

    /// Draws a straight line.
    fn line(&mut self, from: ScreenPoint, to: ScreenPoint, thickness: f32, colour: Color);

    /// Draws a string with its left baseline at `at`.
    fn text(&mut self, text: &str, at: ScreenPoint, size: f32, colour: Color);
}

/// Draws every visible tile of `grid`, back to front.
///
/// `colour_of` decides what a tile looks like, and returns `None` to skip it —
/// which is how a game leaves holes for water, or for tiles it will draw itself
/// with something more interesting.
///
/// This is the whole ground-drawing pass: cull to the camera, walk in draw
/// order, project, fill.
///
/// ```
/// # use isogrid::camera::{Camera, Viewport};
/// # use isogrid::grid::Grid;
/// # use isogrid::iso::TileSize;
/// # use isogrid::render::{draw_tiles, Color, Command, Recorder};
/// let camera = Camera::new(TileSize::CLASSIC, Viewport::new(200.0, 200.0)?);
/// let park = Grid::filled(4, 4, ())?;
///
/// let mut canvas = Recorder::new();
/// // Skip every other tile, checkerboard style.
/// draw_tiles(&mut canvas, &camera, &park, |tile, ()| {
///     ((tile.x + tile.y) % 2 == 0).then_some(Color::WHITE)
/// });
///
/// assert_eq!(canvas.commands().len(), 8, "half of sixteen tiles");
/// # Ok::<(), isogrid::Error>(())
/// ```
pub fn draw_tiles<T>(
    renderer: &mut impl Renderer,
    camera: &Camera,
    grid: &Grid<T>,
    mut colour_of: impl FnMut(TilePos, &T) -> Option<Color>,
) {
    let tiles = camera.tiles();
    let zoom = camera.zoom();

    for tile in grid.draw_order_within(camera.visible_tiles(1)) {
        let Some(colour) = colour_of(tile, &grid[tile]) else {
            continue;
        };
        renderer.fill_tile(
            TileShape {
                centre: camera.world_to_screen(tile.centre()),
                width: tiles.width() * zoom,
                height: tiles.height() * zoom,
            },
            colour,
        );
    }
}

/// One call made to a [`Renderer`].
///
/// Recorded by [`Recorder`] so that drawing code can be asserted on.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// [`Renderer::clear`].
    Clear(Color),
    /// [`Renderer::fill_tile`].
    FillTile(TileShape, Color),
    /// [`Renderer::stroke_tile`].
    StrokeTile(TileShape, f32, Color),
    /// [`Renderer::line`].
    Line(ScreenPoint, ScreenPoint, f32, Color),
    /// [`Renderer::text`].
    Text(String, ScreenPoint, f32, Color),
}

/// A [`Renderer`] that records what it was asked to draw instead of drawing it.
///
/// The point of putting rendering behind a trait: drawing code can be tested
/// exactly, headlessly, in a normal `cargo test` run.
///
/// ```
/// # use isogrid::iso::ScreenPoint;
/// # use isogrid::render::{Color, Command, Recorder, Renderer};
/// let mut canvas = Recorder::new();
/// canvas.clear(Color::BLACK);
/// canvas.text("40 guests", ScreenPoint::new(8.0, 16.0), 14.0, Color::WHITE);
///
/// assert_eq!(canvas.commands().len(), 2);
/// assert!(matches!(canvas.commands()[0], Command::Clear(Color::BLACK)));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Recorder {
    commands: Vec<Command>,
}

impl Recorder {
    /// Builds an empty recorder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything recorded so far, in the order it was drawn.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Forgets everything recorded, keeping the allocation.
    pub fn clear_commands(&mut self) {
        self.commands.clear();
    }

    /// The tiles filled so far, in draw order.
    ///
    /// ```
    /// # use isogrid::iso::ScreenPoint;
    /// # use isogrid::render::{Color, Recorder, Renderer, TileShape};
    /// let mut canvas = Recorder::new();
    /// let shape = TileShape { centre: ScreenPoint::ZERO, width: 64.0, height: 32.0 };
    /// canvas.clear(Color::BLACK);
    /// canvas.fill_tile(shape, Color::WHITE);
    /// assert_eq!(canvas.filled_tiles(), [(shape, Color::WHITE)]);
    /// ```
    pub fn filled_tiles(&self) -> Vec<(TileShape, Color)> {
        self.commands
            .iter()
            .filter_map(|command| match command {
                Command::FillTile(shape, colour) => Some((*shape, *colour)),
                _ => None,
            })
            .collect()
    }
}

impl Renderer for Recorder {
    fn clear(&mut self, colour: Color) {
        self.commands.push(Command::Clear(colour));
    }

    fn fill_tile(&mut self, shape: TileShape, colour: Color) {
        self.commands.push(Command::FillTile(shape, colour));
    }

    fn stroke_tile(&mut self, shape: TileShape, thickness: f32, colour: Color) {
        self.commands
            .push(Command::StrokeTile(shape, thickness, colour));
    }

    fn line(&mut self, from: ScreenPoint, to: ScreenPoint, thickness: f32, colour: Color) {
        self.commands
            .push(Command::Line(from, to, thickness, colour));
    }

    fn text(&mut self, text: &str, at: ScreenPoint, size: f32, colour: Color) {
        self.commands
            .push(Command::Text(text.to_owned(), at, size, colour));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use crate::camera::Viewport;
    use crate::iso::TileSize;

    fn camera(width: f32, height: f32) -> Camera {
        Camera::new(TileSize::CLASSIC, Viewport::new(width, height).unwrap())
    }

    #[test]
    fn colours_round_trip_through_hex() {
        assert_eq!(Color::hex(0xFF_FF_FF), Color::WHITE);
        assert_eq!(Color::hex(0x00_00_00), Color::BLACK);
        assert_eq!(Color::hex(0x12_34_56), Color::rgb(0x12, 0x34, 0x56));
    }

    #[test]
    fn shading_is_clamped_and_leaves_alpha_alone() {
        let colour = Color::rgba(200, 100, 50, 128);
        assert_eq!(colour.shaded(1.0), colour);
        assert_eq!(colour.shaded(0.0), Color::rgba(0, 0, 0, 128));
        assert_eq!(colour.shaded(-5.0), Color::rgba(0, 0, 0, 128));
        assert_eq!(colour.shaded(5.0), colour);
        assert_eq!(colour.shaded(f32::NAN), Color::rgba(0, 0, 0, 128));
    }

    #[test]
    fn a_tile_shape_is_a_rhombus_about_its_centre() {
        let shape = TileShape {
            centre: ScreenPoint::new(10.0, 20.0),
            width: 64.0,
            height: 32.0,
        };
        let [top, right, bottom, left] = shape.corners();
        assert_eq!(top.x, shape.centre.x);
        assert_eq!(bottom.x, shape.centre.x);
        assert_eq!(left.y, shape.centre.y);
        assert_eq!(right.y, shape.centre.y);
        assert_eq!(right.x - left.x, shape.width);
        assert_eq!(bottom.y - top.y, shape.height);
    }

    #[test]
    fn drawing_a_grid_paints_it_back_to_front() {
        let grid = Grid::filled(4, 4, Color::WHITE).unwrap();
        let mut canvas = Recorder::new();
        draw_tiles(&mut canvas, &camera(640.0, 480.0), &grid, |_, colour| {
            Some(*colour)
        });

        let filled = canvas.filled_tiles();
        assert_eq!(filled.len(), grid.len());
        for pair in filled.windows(2) {
            assert!(
                pair[0].0.centre.y <= pair[1].0.centre.y,
                "a nearer tile was drawn first"
            );
        }
    }

    #[test]
    fn skipped_tiles_are_not_drawn() {
        let grid = Grid::filled(4, 4, ()).unwrap();
        let mut canvas = Recorder::new();
        draw_tiles(&mut canvas, &camera(640.0, 480.0), &grid, |_, ()| None);
        assert!(canvas.filled_tiles().is_empty());
    }

    #[test]
    fn tiles_off_screen_are_culled() {
        let big = Grid::filled(200, 200, ()).unwrap();
        let mut canvas = Recorder::new();
        // A window far too small to hold forty thousand tiles.
        draw_tiles(&mut canvas, &camera(160.0, 120.0), &big, |_, ()| {
            Some(Color::WHITE)
        });
        assert!(
            canvas.filled_tiles().len() < big.len() / 10,
            "culling did nothing"
        );
        assert!(
            !canvas.filled_tiles().is_empty(),
            "culling removed everything"
        );
    }

    #[test]
    fn zoom_scales_the_tiles_drawn() {
        let grid = Grid::filled(2, 2, ()).unwrap();
        let mut camera = camera(640.0, 480.0);

        let mut normal = Recorder::new();
        draw_tiles(&mut normal, &camera, &grid, |_, ()| Some(Color::WHITE));

        camera.set_zoom(2.0);
        let mut zoomed = Recorder::new();
        draw_tiles(&mut zoomed, &camera, &grid, |_, ()| Some(Color::WHITE));

        assert_eq!(
            zoomed.filled_tiles()[0].0.width,
            normal.filled_tiles()[0].0.width * 2.0
        );
    }

    #[test]
    fn a_recorder_keeps_every_call_in_order() {
        let mut canvas = Recorder::new();
        canvas.clear(Color::BLACK);
        canvas.line(
            ScreenPoint::ZERO,
            ScreenPoint::new(1.0, 1.0),
            2.0,
            Color::WHITE,
        );
        canvas.text("hello", ScreenPoint::ZERO, 12.0, Color::WHITE);
        canvas.stroke_tile(
            TileShape {
                centre: ScreenPoint::ZERO,
                width: 4.0,
                height: 2.0,
            },
            1.0,
            Color::WHITE,
        );

        assert_eq!(canvas.commands().len(), 4);
        assert!(matches!(canvas.commands()[2], Command::Text(ref text, ..) if text == "hello"));

        canvas.clear_commands();
        assert!(canvas.commands().is_empty());
    }
}
