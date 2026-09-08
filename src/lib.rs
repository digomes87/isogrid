//! `isogrid` is a small, domain-agnostic isometric engine.
//!
//! It knows about tiles, heights, cameras, ticks and paths. It deliberately
//! knows nothing about the game built on top of it: there are no rides, no
//! visitors and no money in this crate.
//!
//! # Coordinate spaces
//!
//! Two spaces, and one conversion between them:
//!
//! - **Grid space** ([`iso::GridPoint`], [`iso::TilePos`]) is where the
//!   simulation lives. `x` runs south-east, `y` south-west, `z` upward in
//!   elevation steps.
//! - **Screen space** ([`iso::ScreenPoint`]) is pixels, `y` downward.
//!
//! [`iso::TileSize`] converts between them and nothing else in the crate needs
//! to know how.
//!
//! ```
//! use isogrid::iso::{GridPoint, TilePos, TileSize};
//!
//! let tiles = TileSize::CLASSIC;
//! let screen = tiles.grid_to_screen(TilePos::new(3, 1).centre());
//! let picked = tiles.pick_tile(screen, 0.0);
//! assert_eq!(picked, TilePos::new(3, 1));
//! ```

#![doc(html_root_url = "https://docs.rs/isogrid")]

pub mod error;
pub mod iso;

pub use error::{Error, Result};
