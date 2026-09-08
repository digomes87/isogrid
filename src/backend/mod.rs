//! Rendering backends.
//!
//! A backend is the only part of the engine that talks to a graphics library.
//! Everything above it works in [`ScreenPoint`](crate::iso::ScreenPoint)s and
//! [`Command`](crate::render::Command)s, so replacing one backend with another
//! is invisible to the simulation and to the game.
//!
//! Each backend is behind a feature flag, because a headless simulation — a
//! test, a server, a replay checker — should not have to link a window system.

#[cfg(feature = "macroquad-backend")]
pub mod macroquad;
