//! Property tests for the isometric projection.
//!
//! The unit tests next to the implementation pin down specific, hand-checked
//! values. These check the invariants that must hold for *every* input, which
//! is where sign errors and off-by-half-a-tile bugs actually hide.

// `z` is carried through the projection untouched, so exact equality is the
// assertion we want for it; every other comparison here uses a tolerance.
#![allow(clippy::float_cmp)]

use isogrid::iso::{GridPoint, ScreenPoint, TilePos, TileSize};
use proptest::prelude::*;

/// Grid coordinates in a range wide enough to cover any plausible map, and
/// small enough that `f32` still has precision to spare.
fn coordinate() -> impl Strategy<Value = f32> {
    -4096.0f32..4096.0
}

fn tile_size() -> impl Strategy<Value = TileSize> {
    (1.0f32..256.0, 1.0f32..256.0, 0.0f32..64.0)
        .prop_map(|(w, h, e)| TileSize::new(w, h, e).expect("bounds keep this valid"))
}

proptest! {
    // Integration tests live outside `src`, where proptest cannot find a crate
    // root to persist regression files against. Shrunken counter-examples are
    // still printed on failure; they are simply not written to disk.
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    /// Unprojecting a projected point returns the point it started from.
    #[test]
    fn screen_to_grid_inverts_grid_to_screen(
        tiles in tile_size(),
        x in coordinate(),
        y in coordinate(),
        z in -64.0f32..64.0,
    ) {
        let point = GridPoint::new(x, y, z);
        let round_trip = tiles.screen_to_grid(tiles.grid_to_screen(point), z);

        // Tolerance scales with magnitude: f32 has ~7 significant digits, and
        // the projection sums coordinates before dividing them apart again.
        let tolerance = 1e-3 * (1.0 + x.abs().max(y.abs()) / 1024.0);
        prop_assert!((round_trip.x - point.x).abs() < tolerance, "x drifted: {round_trip:?} vs {point:?}");
        prop_assert!((round_trip.y - point.y).abs() < tolerance, "y drifted: {round_trip:?} vs {point:?}");
        prop_assert_eq!(round_trip.z, point.z);
    }

    /// The centre of a tile always picks that same tile back.
    #[test]
    fn a_tile_centre_picks_its_own_tile(
        tiles in tile_size(),
        x in -2048i32..2048,
        y in -2048i32..2048,
    ) {
        let tile = TilePos::new(x, y);
        prop_assert_eq!(tiles.pick_tile(tiles.grid_to_screen(tile.centre()), 0.0), tile);
    }

    /// The projection is linear: translating in grid space translates in screen
    /// space, by an amount that does not depend on where you started.
    #[test]
    fn translation_in_grid_space_is_translation_on_screen(
        tiles in tile_size(),
        x in coordinate(),
        y in coordinate(),
        dx in -64.0f32..64.0,
        dy in -64.0f32..64.0,
    ) {
        let origin = tiles.grid_to_screen(GridPoint::ground(0.0, 0.0));
        let step = tiles.grid_to_screen(GridPoint::ground(dx, dy));
        let expected = ScreenPoint::new(step.x - origin.x, step.y - origin.y);

        let from = tiles.grid_to_screen(GridPoint::ground(x, y));
        let to = tiles.grid_to_screen(GridPoint::ground(x + dx, y + dy));

        let tolerance = 1e-2 * (1.0 + x.abs().max(y.abs()) / 128.0);
        prop_assert!(((to.x - from.x) - expected.x).abs() < tolerance);
        prop_assert!(((to.y - from.y) - expected.y).abs() < tolerance);
    }

    /// Moving away from the camera never decreases the draw-order key.
    #[test]
    fn depth_grows_with_distance_from_the_camera(
        tiles in tile_size(),
        x in coordinate(),
        y in coordinate(),
        step in 0.0f32..64.0,
    ) {
        let near = tiles.depth(GridPoint::ground(x + step, y + step));
        let far = tiles.depth(GridPoint::ground(x, y));
        prop_assert!(near >= far);
    }

    /// Every neighbour of a tile is exactly one step away, and distinct.
    #[test]
    fn neighbours_are_distinct_and_adjacent(x in -2048i32..2048, y in -2048i32..2048) {
        let tile = TilePos::new(x, y);
        let neighbours = tile.neighbours();
        for (i, a) in neighbours.iter().enumerate() {
            prop_assert_eq!(tile.manhattan_distance(*a), 1);
            for b in &neighbours[i + 1..] {
                prop_assert_ne!(a, b);
            }
        }
    }

    /// Rejecting a tile size never panics, whatever it is handed.
    #[test]
    fn tile_size_validation_is_total(w in any::<f32>(), h in any::<f32>(), e in any::<f32>()) {
        let built = TileSize::new(w, h, e);
        prop_assert_eq!(
            built.is_ok(),
            w.is_finite() && w > 0.0 && h.is_finite() && h > 0.0 && e.is_finite() && e >= 0.0
        );
    }
}
