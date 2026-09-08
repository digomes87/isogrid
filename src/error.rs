//! The error type returned by fallible engine operations.

/// A specialised `Result` for engine operations.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Anything the engine can refuse to do.
///
/// The engine fails on impossible *configuration* — a tile with no width, a
/// grid with no tiles — and returns `Option` for the ordinary absences, such as
/// asking for a tile outside the map.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A tile size was zero, negative or not a number.
    #[error(
        "invalid tile size {width}x{height} with elevation {elevation}: \
         width and height must be finite and positive, elevation finite and non-negative"
    )]
    InvalidTileSize {
        /// The rejected tile width, in pixels.
        width: f32,
        /// The rejected tile height, in pixels.
        height: f32,
        /// The rejected elevation step, in pixels.
        elevation: f32,
    },

    /// A grid was asked for with a zero dimension, or one too large to address.
    #[error(
        "invalid grid size {width}x{height}: both dimensions must be non-zero \
         and their product must not exceed Grid::MAX_TILES"
    )]
    InvalidGridSize {
        /// The requested width in tiles.
        width: u32,
        /// The requested height in tiles.
        height: u32,
    },

    /// A zoom range was empty, inverted or not a number.
    #[error("invalid zoom range {min}..={max}: both bounds must be finite and positive, with min <= max")]
    InvalidZoomRange {
        /// The requested lower bound.
        min: f32,
        /// The requested upper bound.
        max: f32,
    },

    /// A simulation was configured with a tick rate of zero.
    #[error("invalid tick rate: a simulation must run at least one tick per second")]
    InvalidTickRate,
}
