//! Rendering backends.
//!
//! A backend is the only part of the engine that talks to a graphics library.
//! Everything above it works in [`ScreenPoint`](crate::iso::ScreenPoint)s and
//! [`Command`](crate::render::Command)s, so replacing one backend with another
//! is invisible to the simulation and to the game.
//!
//! Each backend is behind a feature flag, because a headless simulation — a
//! test, a server, a replay checker — should not have to link a window system.

//!
//! What every backend shares is [`step`]: the frame of a fixed-timestep loop,
//! with no window attached, so it can be tested like any other function.

use core::time::Duration;

use crate::input::Input;
use crate::time::{Clock, Tick, TickRun};

#[cfg(feature = "macroquad-backend")]
pub mod macroquad;

/// Runs the simulation ticks one frame owes.
///
/// Feeds `frame` to the clock, calls `tick` once per whole tick that is due,
/// and clears the input's edge state after each one. Returns the run, so a
/// caller can tell whether the simulation [fell behind](TickRun::is_behind).
///
/// Frames and ticks do not line up. At 60 frames a second a 40 Hz simulation
/// runs no tick at all in one frame out of three, and after a slow frame it
/// runs several. Clearing presses once per *frame* therefore drops the ones
/// that land in a frame with no tick and repeats the ones that land in a frame
/// with two. Clearing them once per *tick* delivers each press to exactly one
/// tick.
///
/// The backend's side of the contract is to report input with
/// [`Input::move_pointer`] and the `press_*`/`release_*` methods, and never to
/// call [`Input::begin_frame`].
///
/// ```
/// use core::time::Duration;
/// use isogrid::backend::step;
/// use isogrid::input::{Input, Key};
/// use isogrid::time::Clock;
///
/// let mut clock = Clock::new(40)?; // a tick every 25 ms
/// let mut input = Input::default();
/// let mut jumps = 0;
///
/// // The key goes down in a frame too short to owe a tick...
/// input.press_key(Key::Space);
/// step(&mut clock, &mut input, Duration::from_millis(10), |_, input| {
///     jumps += u32::from(input.key_pressed(Key::Space));
/// });
/// assert_eq!(jumps, 0);
///
/// // ...and the next tick to run still sees it, once.
/// step(&mut clock, &mut input, Duration::from_millis(90), |_, input| {
///     jumps += u32::from(input.key_pressed(Key::Space));
/// });
/// assert_eq!(jumps, 1);
/// # Ok::<(), isogrid::Error>(())
/// ```
pub fn step(
    clock: &mut Clock,
    input: &mut Input,
    frame: Duration,
    mut tick: impl FnMut(Tick, &Input),
) -> TickRun {
    let run = clock.advance(frame);
    for due in run.clone() {
        tick(due, input);
        input.end_tick();
    }
    run
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::step;
    use crate::input::{Button, Input, Key};
    use crate::iso::ScreenPoint;
    use crate::time::Clock;

    /// 40 Hz simulation: a tick every 25 ms.
    fn clock() -> Clock {
        Clock::new(40).expect("forty is not zero")
    }

    /// Runs one frame and counts the ticks that saw `key` as freshly pressed.
    fn presses_seen(clock: &mut Clock, input: &mut Input, frame_ms: u64, key: Key) -> (u32, u32) {
        let mut seen = 0;
        let run = step(clock, input, Duration::from_millis(frame_ms), |_, input| {
            seen += u32::from(input.key_pressed(key));
        });
        (run.count(), seen)
    }

    #[test]
    fn a_press_in_a_frame_with_no_tick_reaches_the_next_tick() {
        // At 60 frames a second on a 40 Hz simulation, one frame in three runs
        // no tick at all. A press reported in that frame must not be lost.
        let (mut clock, mut input) = (clock(), Input::default());

        input.move_pointer(ScreenPoint::ZERO);
        input.press_key(Key::Space);
        assert_eq!(presses_seen(&mut clock, &mut input, 16, Key::Space), (0, 0));

        input.move_pointer(ScreenPoint::ZERO);
        assert_eq!(presses_seen(&mut clock, &mut input, 16, Key::Space), (1, 1));

        input.move_pointer(ScreenPoint::ZERO);
        assert_eq!(
            presses_seen(&mut clock, &mut input, 25, Key::Space),
            (1, 0),
            "a press is an edge: once consumed it must not repeat"
        );
    }

    #[test]
    fn a_press_is_seen_by_one_tick_when_a_frame_runs_several() {
        let (mut clock, mut input) = (clock(), Input::default());
        input.move_pointer(ScreenPoint::ZERO);
        input.press_key(Key::Space);
        assert_eq!(
            presses_seen(&mut clock, &mut input, 100, Key::Space),
            (4, 1)
        );
        assert!(input.key_down(Key::Space), "the key is still held");
    }

    #[test]
    fn a_tap_between_two_ticks_is_still_a_press() {
        let (mut clock, mut input) = (clock(), Input::default());
        input.move_pointer(ScreenPoint::ZERO);
        input.press_button(Button::Left);
        step(&mut clock, &mut input, Duration::from_millis(5), |_, _| {});
        input.release_button(Button::Left);

        let mut pressed = false;
        step(
            &mut clock,
            &mut input,
            Duration::from_millis(25),
            |_, input| {
                pressed |= input.button_pressed(Button::Left);
                assert!(!input.button_down(Button::Left));
            },
        );
        assert!(pressed);
    }

    #[test]
    fn scroll_and_pointer_movement_add_up_across_frames_with_no_tick() {
        let (mut clock, mut input) = (clock(), Input::default());
        input.move_pointer(ScreenPoint::new(100.0, 100.0));
        step(&mut clock, &mut input, Duration::from_millis(25), |_, _| {});

        input.move_pointer(ScreenPoint::new(110.0, 100.0));
        input.scroll_by(1);
        step(&mut clock, &mut input, Duration::from_millis(8), |_, _| {});
        input.move_pointer(ScreenPoint::new(125.0, 95.0));
        input.scroll_by(1);

        let mut seen = None;
        step(
            &mut clock,
            &mut input,
            Duration::from_millis(17),
            |_, input| {
                seen = Some((input.pointer_delta(), input.scroll()));
            },
        );
        assert_eq!(seen, Some((ScreenPoint::new(25.0, -5.0), 2)));
        assert_eq!(input.scroll(), 0, "consumed by the tick that read it");
    }

    #[test]
    fn the_run_reports_dropped_ticks() {
        let (mut clock, mut input) = (clock(), Input::default());
        let run = step(&mut clock, &mut input, Duration::from_secs(10), |_, _| {});
        assert_eq!(run.count(), clock.max_catch_up());
        assert!(run.is_behind());
    }
}
