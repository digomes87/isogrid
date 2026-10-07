//! Backend-agnostic input.
//!
//! [`Input`] is a snapshot of the pointer and keyboard for one frame. Backends
//! fill it in; game code reads it and never learns which windowing library is
//! underneath. It also means input-driven logic — camera controls, tool
//! handling — can be tested by building an [`Input`] by hand.
//!
//! ```
//! use isogrid::input::{Button, Input, Key};
//! use isogrid::iso::ScreenPoint;
//!
//! let mut frame = Input::default();
//! frame.begin_frame(ScreenPoint::new(120.0, 80.0));
//! frame.press_button(Button::Left);
//! frame.press_key(Key::Space);
//!
//! assert!(frame.button_pressed(Button::Left));
//! assert!(frame.button_down(Button::Left));
//! assert!(frame.key_pressed(Key::Space));
//! ```

use std::collections::HashSet;

use crate::iso::ScreenPoint;

/// A pointer button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    /// The primary button.
    Left,
    /// The secondary button.
    Right,
    /// The wheel button.
    Middle,
}

impl Button {
    /// Every button, for iteration.
    pub const ALL: [Self; 3] = [Self::Left, Self::Right, Self::Middle];

    const fn index(self) -> usize {
        match self {
            Self::Left => 0,
            Self::Right => 1,
            Self::Middle => 2,
        }
    }
}

/// A key the engine knows about.
///
/// Deliberately short: these are the keys an isometric engine has an opinion
/// about. Anything else is the game's business, and a game that needs more can
/// keep its own input state alongside this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Key {
    /// Cancel the current action.
    Escape,
    /// Confirm, or pause.
    Space,
    /// Scroll the view north.
    Up,
    /// Scroll the view south.
    Down,
    /// Scroll the view west.
    Left,
    /// Scroll the view east.
    Right,
    /// Zoom in.
    Plus,
    /// Zoom out.
    Minus,
}

/// The pointer and keyboard, for one frame.
///
/// "Down" means held right now; "pressed" means it went down during this frame
/// specifically. Camera panning wants the former, placing a building wants the
/// latter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
    pointer: PointerState,
    buttons_down: [bool; 3],
    buttons_pressed: [bool; 3],
    keys_down: HashSet<Key>,
    keys_pressed: HashSet<Key>,
    scroll: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PointerState {
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
}

impl Input {
    /// Starts a new frame at `pointer`, forgetting the previous one.
    ///
    /// Clears the edge state — presses, scroll and pointer movement — keeps
    /// what is still held, and works out how far the pointer moved.
    ///
    /// This is the right call when every frame is also a simulation step, and
    /// when building an [`Input`] by hand in a test. A backend driving a
    /// fixed-timestep loop must use [`Input::move_pointer`] and
    /// [`Input::end_tick`] instead: frames and ticks do not line up, and
    /// clearing once per frame loses whatever was pressed in a frame that ran
    /// no tick.
    pub fn begin_frame(&mut self, pointer: ScreenPoint) {
        self.end_tick();
        self.move_pointer(pointer);
    }

    /// Reports where the pointer is now, adding the movement to
    /// [`Input::pointer_delta`].
    ///
    /// Unlike [`Input::begin_frame`] this clears nothing, so presses, scroll
    /// and movement keep adding up until a tick reads them and
    /// [`Input::end_tick`] is called.
    ///
    /// ```
    /// # use isogrid::input::Input;
    /// # use isogrid::iso::ScreenPoint;
    /// let mut input = Input::default();
    /// input.move_pointer(ScreenPoint::new(10.0, 0.0));
    /// input.move_pointer(ScreenPoint::new(25.0, 5.0));
    /// assert_eq!(input.pointer_delta(), ScreenPoint::new(25.0, 5.0));
    /// ```
    pub fn move_pointer(&mut self, pointer: ScreenPoint) {
        #[allow(clippy::cast_possible_truncation)]
        let (x, y) = (pointer.x as i32, pointer.y as i32);

        self.pointer = PointerState {
            x,
            y,
            dx: self
                .pointer
                .dx
                .saturating_add(x.saturating_sub(self.pointer.x)),
            dy: self
                .pointer
                .dy
                .saturating_add(y.saturating_sub(self.pointer.y)),
        };
    }

    /// Marks the edge state as consumed: presses, scroll and pointer movement
    /// are cleared, and whatever is held stays held.
    ///
    /// Call it after each simulation tick. A press is then seen by exactly one
    /// tick — the first to run after it happened — however many frames or
    /// ticks went by in between. [`backend::step`](crate::backend::step) does
    /// this for you.
    ///
    /// ```
    /// # use isogrid::input::{Input, Key};
    /// let mut input = Input::default();
    /// input.press_key(Key::Space);
    /// input.end_tick();
    /// assert!(!input.key_pressed(Key::Space));
    /// assert!(input.key_down(Key::Space));
    /// ```
    pub fn end_tick(&mut self) {
        self.pointer.dx = 0;
        self.pointer.dy = 0;
        self.buttons_pressed = [false; 3];
        self.keys_pressed.clear();
        self.scroll = 0;
    }

    /// Reports that a button went down.
    pub fn press_button(&mut self, button: Button) {
        self.buttons_down[button.index()] = true;
        self.buttons_pressed[button.index()] = true;
    }

    /// Reports that a button came up.
    pub fn release_button(&mut self, button: Button) {
        self.buttons_down[button.index()] = false;
    }

    /// Reports that a key went down.
    pub fn press_key(&mut self, key: Key) {
        self.keys_down.insert(key);
        self.keys_pressed.insert(key);
    }

    /// Reports that a key came up.
    pub fn release_key(&mut self, key: Key) {
        self.keys_down.remove(&key);
    }

    /// Reports wheel movement, positive away from the user.
    pub fn scroll_by(&mut self, amount: i32) {
        self.scroll = self.scroll.saturating_add(amount);
    }

    /// Where the pointer is.
    pub fn pointer(&self) -> ScreenPoint {
        #[allow(clippy::cast_precision_loss)]
        ScreenPoint::new(self.pointer.x as f32, self.pointer.y as f32)
    }

    /// How far the pointer moved since the edge state was last cleared.
    pub fn pointer_delta(&self) -> ScreenPoint {
        #[allow(clippy::cast_precision_loss)]
        ScreenPoint::new(self.pointer.dx as f32, self.pointer.dy as f32)
    }

    /// Whether a button is held right now.
    pub fn button_down(&self, button: Button) -> bool {
        self.buttons_down[button.index()]
    }

    /// Whether a button went down since the edge state was last cleared.
    pub fn button_pressed(&self, button: Button) -> bool {
        self.buttons_pressed[button.index()]
    }

    /// Whether a key is held right now.
    pub fn key_down(&self, key: Key) -> bool {
        self.keys_down.contains(&key)
    }

    /// Whether a key went down since the edge state was last cleared.
    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Wheel movement since the edge state was last cleared, positive away
    /// from the user.
    pub fn scroll(&self) -> i32 {
        self.scroll
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn a_press_lasts_one_frame_but_a_hold_persists() {
        let mut input = Input::default();
        input.begin_frame(ScreenPoint::ZERO);
        input.press_button(Button::Left);
        assert!(input.button_pressed(Button::Left) && input.button_down(Button::Left));

        input.begin_frame(ScreenPoint::ZERO);
        assert!(
            !input.button_pressed(Button::Left),
            "the press should not repeat"
        );
        assert!(input.button_down(Button::Left), "but it is still held");

        input.release_button(Button::Left);
        assert!(!input.button_down(Button::Left));
    }

    #[test]
    fn keys_behave_the_same_way() {
        let mut input = Input::default();
        input.begin_frame(ScreenPoint::ZERO);
        input.press_key(Key::Escape);
        assert!(input.key_pressed(Key::Escape) && input.key_down(Key::Escape));

        input.begin_frame(ScreenPoint::ZERO);
        assert!(!input.key_pressed(Key::Escape));
        assert!(input.key_down(Key::Escape));

        input.release_key(Key::Escape);
        assert!(!input.key_down(Key::Escape));
    }

    #[test]
    fn the_pointer_delta_is_the_movement_since_the_last_frame() {
        let mut input = Input::default();
        input.begin_frame(ScreenPoint::new(100.0, 100.0));
        input.begin_frame(ScreenPoint::new(130.0, 90.0));

        let delta = input.pointer_delta();
        assert_eq!((delta.x, delta.y), (30.0, -10.0));
        assert_eq!(input.pointer(), ScreenPoint::new(130.0, 90.0));
    }

    #[test]
    fn scroll_accumulates_within_a_frame_and_resets_between_them() {
        let mut input = Input::default();
        input.begin_frame(ScreenPoint::ZERO);
        input.scroll_by(1);
        input.scroll_by(2);
        assert_eq!(input.scroll(), 3);

        input.begin_frame(ScreenPoint::ZERO);
        assert_eq!(input.scroll(), 0);
    }

    #[test]
    fn buttons_are_independent() {
        let mut input = Input::default();
        input.begin_frame(ScreenPoint::ZERO);
        input.press_button(Button::Right);
        for button in Button::ALL {
            assert_eq!(input.button_down(button), button == Button::Right);
        }
    }
}
