//! Deserialisation must not be a way round the constructors.
//!
//! Every type here refuses bad input in `new`. A derived `Deserialize` writes
//! the fields directly and skips that check, so a save file that was truncated,
//! hand-edited or written by a newer version could build a value the rest of
//! the crate assumes is impossible: a tick rate of zero that divides by zero, a
//! grid whose tile count disagrees with its dimensions. These tests feed each
//! type the input its constructor would reject and expect an error, not a value.

use isogrid::camera::{Camera, Viewport, ZoomRange};
use isogrid::grid::{Grid, TileBounds};
use isogrid::iso::{TilePos, TileSize};
use isogrid::path::Path;
use isogrid::time::{Clock, TickRate};
use serde_json::{from_str, from_value, json, to_string, to_value};

#[test]
fn a_zero_tick_rate_is_refused() {
    assert!(from_str::<TickRate>("0").is_err());
    assert_eq!(from_str::<TickRate>("40").unwrap(), TickRate::CLASSIC);
}

#[test]
fn a_clock_with_a_zero_rate_is_refused() {
    let mut clock = to_value(Clock::new(40).unwrap()).unwrap();
    clock["rate"] = json!(0);
    assert!(from_value::<Clock>(clock).is_err());
}

#[test]
fn a_clock_holding_more_than_a_tick_of_leftover_time_is_refused() {
    // `advance` always leaves less than one tick in the accumulator. More than
    // that is not a state the clock can reach, and an enormous value would
    // overflow the next addition.
    let mut clock = to_value(Clock::new(40).unwrap()).unwrap();
    clock["accumulated"] = json!(1_000_000_000u64);
    assert!(from_value::<Clock>(clock).is_err());
}

#[test]
fn a_clock_that_may_never_tick_is_refused() {
    let mut clock = to_value(Clock::new(40).unwrap()).unwrap();
    clock["max_catch_up"] = json!(0);
    assert!(from_value::<Clock>(clock).is_err());
}

#[test]
fn degenerate_tile_sizes_are_refused() {
    let bad = json!({ "width": 0.0, "height": 32.0, "elevation": 8.0 });
    assert!(from_value::<TileSize>(bad).is_err());
    let bad = json!({ "width": 64.0, "height": 32.0, "elevation": -1.0 });
    assert!(from_value::<TileSize>(bad).is_err());
}

#[test]
fn degenerate_viewports_are_refused() {
    assert!(from_value::<Viewport>(json!({ "width": 800.0, "height": 0.0 })).is_err());
    assert!(from_value::<Viewport>(json!({ "width": -1.0, "height": 600.0 })).is_err());
}

#[test]
fn inverted_zoom_ranges_are_refused() {
    assert!(from_value::<ZoomRange>(json!({ "min": 4.0, "max": 0.5 })).is_err());
    assert!(from_value::<ZoomRange>(json!({ "min": 0.0, "max": 4.0 })).is_err());
}

#[test]
fn a_camera_zoomed_outside_its_range_is_refused() {
    let camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0).unwrap());
    let mut raw = to_value(camera).unwrap();
    raw["zoom"] = json!(0.0); // every unprojection divides by this
    assert!(from_value::<Camera>(raw).is_err());
}

#[test]
fn a_grid_whose_tiles_do_not_match_its_dimensions_is_refused() {
    let short = json!({ "width": 2, "height": 2, "tiles": [1, 2, 3] });
    assert!(from_value::<Grid<u8>>(short).is_err());
    let empty = json!({ "width": 0, "height": 0, "tiles": [] });
    assert!(from_value::<Grid<u8>>(empty).is_err());
}

#[test]
fn bounds_given_backwards_are_normalised_as_the_constructor_does() {
    let raw = json!({ "min": { "x": 5, "y": 5 }, "max": { "x": 1, "y": 2 } });
    let bounds: TileBounds = from_value(raw).unwrap();
    assert_eq!(
        bounds,
        TileBounds::new(TilePos::new(1, 2), TilePos::new(5, 5))
    );
}

#[test]
fn a_path_with_no_tiles_is_refused() {
    // `Path::start` and `Path::goal` index unconditionally.
    assert!(from_value::<Path>(json!({ "tiles": [], "cost": 0 })).is_err());
}

#[test]
fn valid_values_survive_a_round_trip_unchanged() {
    fn round_trip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        from_str(&to_string(value).unwrap()).unwrap()
    }

    let mut clock = Clock::new(60).unwrap();
    clock.advance(core::time::Duration::from_millis(40));
    assert_eq!(round_trip(&clock), clock);

    let grid = Grid::from_fn(3, 2, |tile| tile.x * 10 + tile.y).unwrap();
    assert_eq!(round_trip(&grid), grid);

    let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(640.0, 480.0).unwrap());
    camera.set_zoom(2.0);
    assert_eq!(round_trip(&camera), camera);

    let bounds = TileBounds::new(TilePos::new(-2, 1), TilePos::new(4, 9));
    assert_eq!(round_trip(&bounds), bounds);

    let range = ZoomRange::new(0.5, 4.0).unwrap();
    assert_eq!(round_trip(&range), range);
}
