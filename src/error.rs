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

    /// A viewport was given a zero, negative or non-finite dimension.
    #[error("invalid viewport {width}x{height}: both dimensions must be finite and positive")]
    InvalidViewport {
        /// The rejected width in pixels.
        width: f32,
        /// The rejected height in pixels.
        height: f32,
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

    /// A camera's zoom was outside its own zoom range, or not a number.
    ///
    /// The setters clamp, so this is only reachable by deserialising a camera
    /// that was not written by this crate.
    #[error("invalid zoom {zoom}: it must lie within the camera's range {min}..={max}")]
    InvalidZoom {
        /// The rejected zoom factor.
        zoom: f32,
        /// The lower bound of the camera's range.
        min: f32,
        /// The upper bound of the camera's range.
        max: f32,
    },

    /// A grid's tiles did not add up to its dimensions.
    ///
    /// Only reachable by deserialising: the constructors build the tiles from
    /// the dimensions.
    #[error("a {width}x{height} grid needs exactly width x height tiles, but {tiles} were given")]
    TileCountMismatch {
        /// The stated width in tiles.
        width: u32,
        /// The stated height in tiles.
        height: u32,
        /// How many tiles were actually present.
        tiles: usize,
    },

    /// A path had no tiles in it.
    ///
    /// Only reachable by deserialising: a search always yields at least the
    /// tile it started on.
    #[error("invalid path: a path must contain at least its starting tile")]
    EmptyPath,

    /// A clock was in a state it cannot reach by running.
    ///
    /// Only reachable by deserialising. The leftover time must be less than one
    /// tick, and the clock must be allowed to run at least one tick per call.
    #[error(
        "invalid clock: leftover time {accumulated} must be below one tick, \
         and the catch-up limit {max_catch_up} must be at least one"
    )]
    InvalidClock {
        /// The rejected accumulator, in nanoseconds scaled by the tick rate.
        accumulated: u128,
        /// The rejected catch-up limit.
        max_catch_up: u32,
    },
}
