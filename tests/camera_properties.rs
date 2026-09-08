//! Property tests for the camera.
//!
//! Two invariants carry most of the weight: the projection stays invertible at
//! every pan and zoom, and culling never hides a tile that is actually on
//! screen. A false negative in culling is a hole in the world.

// Pan is exact, so `z` and the focus compare exactly; screen round-trips use a
// tolerance because zoom divides.
#![allow(clippy::float_cmp)]

use isogrid::camera::{Camera, Viewport, ZoomRange};
use isogrid::iso::{GridPoint, ScreenPoint, TileSize};
use proptest::prelude::*;

fn camera() -> impl Strategy<Value = Camera> {
    (
        320.0f32..1920.0,
        240.0f32..1080.0,
        -512.0f32..512.0,
        -512.0f32..512.0,
        0.25f32..8.0,
    )
        .prop_map(|(w, h, fx, fy, zoom)| {
            let viewport = Viewport::new(w, h).expect("bounds keep this valid");
            let mut camera = Camera::new(TileSize::CLASSIC, viewport);
            camera.look_at(GridPoint::ground(fx, fy));
            camera.set_zoom(zoom);
            camera
        })
}

proptest! {
    /// Whatever the pan and zoom, the projection stays invertible.
    #[test]
    fn screen_and_world_round_trip(camera in camera(), x in -256.0f32..256.0, y in -256.0f32..256.0) {
        let point = GridPoint::ground(x, y);
        let back = camera.screen_to_world(camera.world_to_screen(point), 0.0);

        // Zooming out divides the screen delta back up, so the tolerance has to
        // scale with how far out the camera is.
        let tolerance = 1e-2 / camera.zoom().min(1.0);
        prop_assert!((back.x - point.x).abs() < tolerance, "{back:?} vs {point:?}");
        prop_assert!((back.y - point.y).abs() < tolerance, "{back:?} vs {point:?}");
    }

    /// The camera's focus is always at the centre of the viewport.
    #[test]
    fn the_focus_is_always_centred(camera in camera()) {
        prop_assert_eq!(camera.world_to_screen(camera.focus()), camera.viewport().centre());
    }

    /// Culling never drops a tile that is on screen.
    ///
    /// This is the invariant that matters: an over-estimate costs a few wasted
    /// draw calls, an under-estimate leaves holes in the world.
    #[test]
    fn visible_tiles_covers_every_pixel_of_the_viewport(
        camera in camera(),
        u in 0.0f32..1.0,
        v in 0.0f32..1.0,
    ) {
        let pixel = ScreenPoint::new(
            u * camera.viewport().width(),
            v * camera.viewport().height(),
        );
        let tile = camera.pick_tile(pixel, 0.0);
        prop_assert!(
            camera.visible_tiles(0).contains(tile),
            "{tile:?} is on screen at {pixel:?} but was culled"
        );
    }

    /// A margin only ever grows the visible region.
    #[test]
    fn a_margin_never_shrinks_the_visible_region(camera in camera(), margin in 0u32..16) {
        let tight = camera.visible_tiles(0);
        let loose = camera.visible_tiles(margin);
        prop_assert!(loose.len() >= tight.len());
        prop_assert!(loose.contains(tight.min()) && loose.contains(tight.max()));
    }

    /// Panning by a delta and back returns the camera exactly where it started.
    #[test]
    fn panning_is_reversible(camera in camera(), dx in -64.0f32..64.0, dy in -64.0f32..64.0) {
        let mut moved = camera;
        moved.pan(dx, dy);
        moved.pan(-dx, -dy);
        prop_assert!((moved.focus().x - camera.focus().x).abs() < 1e-3);
        prop_assert!((moved.focus().y - camera.focus().y).abs() < 1e-3);
    }

    /// Zooming toward a point leaves whatever is under that point where it was.
    #[test]
    fn zooming_towards_a_point_keeps_it_still(
        camera in camera(),
        u in 0.0f32..1.0,
        v in 0.0f32..1.0,
        factor in 0.5f32..2.0,
    ) {
        let mut camera = camera;
        let anchor = ScreenPoint::new(
            u * camera.viewport().width(),
            v * camera.viewport().height(),
        );

        let before = camera.screen_to_world(anchor, 0.0);
        camera.zoom_towards(anchor, factor);
        let after = camera.screen_to_world(anchor, 0.0);

        let tolerance = 1e-2 / camera.zoom().min(1.0);
        prop_assert!((after.x - before.x).abs() < tolerance, "{after:?} vs {before:?}");
        prop_assert!((after.y - before.y).abs() < tolerance, "{after:?} vs {before:?}");
    }

    /// Zoom stays inside its range no matter what it is handed.
    #[test]
    fn zoom_never_escapes_its_range(camera in camera(), zoom in any::<f32>()) {
        let mut camera = camera;
        camera.set_zoom(zoom);
        let range = ZoomRange::DEFAULT;
        prop_assert!(camera.zoom() >= range.min() && camera.zoom() <= range.max());
    }
}
