//! A [`Renderer`] and window loop built on [`macroquad`].
//!
//! Enable the `macroquad-backend` feature to use it. See
//! [ADR 0004](https://github.com/digomes87/isogrid/blob/main/docs/adr/0004-macroquad-first.md)
//! for why macroquad, and for the security advisory that comes with it.
//!
//! The backend does three things and no more: it turns engine draw calls into
//! macroquad calls, it fills in an [`Input`] each frame, and it runs the
//! fixed-timestep loop. Everything else is above it and knows nothing about it.
//!
//! ```no_run
//! use isogrid::backend::macroquad::{run, App};
//! use isogrid::input::Input;
//! use isogrid::render::{Color, Renderer};
//! use isogrid::time::{Tick, TickRate};
//!
//! struct EmptyPark;
//!
//! impl App for EmptyPark {
//!     fn tick(&mut self, _tick: Tick, _input: &Input) {}
//!
//!     fn draw(&mut self, canvas: &mut dyn Renderer, _alpha: f32) {
//!         canvas.clear(Color::hex(0x1E_2A_38));
//!     }
//! }
//!
//! # async fn example() {
//! run(EmptyPark, TickRate::CLASSIC).await;
//! # }
//! ```

use ::macroquad::prelude as mq;

use crate::input::{Button, Input, Key};
use crate::iso::ScreenPoint;
use crate::render::{Color, Renderer, TileShape};
use crate::time::{Clock, Tick, TickRate};

/// A game the backend can run.
///
/// [`App::tick`] advances the simulation by exactly one fixed step and is where
/// all state changes belong. [`App::draw`] must not change simulation state:
/// it runs a variable number of times per tick, so anything it changed would
/// depend on the frame rate.
pub trait App {
    /// Advances the simulation by one tick.
    fn tick(&mut self, tick: Tick, input: &Input);

    /// Draws the current state.
    ///
    /// `alpha` is how far the frame is between the last tick and the next, from
    /// `0.0` to just under `1.0`. Use it to interpolate moving things so that
    /// motion is smooth at frame rates above the tick rate.
    fn draw(&mut self, canvas: &mut dyn Renderer, alpha: f32);

    /// Called when the window size changes.
    ///
    /// The default does nothing; a game holding a camera should resize it.
    fn resize(&mut self, width: f32, height: f32) {
        let _ = (width, height);
    }

    /// Whether the loop should stop.
    ///
    /// Checked once per frame. The default runs until the window closes.
    fn should_quit(&self) -> bool {
        false
    }
}

/// Draws through a macroquad window.
///
/// Positions arrive already projected, culled and ordered, so this type does no
/// geometry beyond splitting a rhombus into two triangles.
#[derive(Debug, Clone, Copy, Default)]
pub struct MacroquadRenderer {
    _private: (),
}

impl MacroquadRenderer {
    /// Builds a renderer for the current macroquad window.
    pub const fn new() -> Self {
        Self { _private: () }
    }

    /// The size of the window right now.
    pub fn viewport(self) -> (f32, f32) {
        (mq::screen_width(), mq::screen_height())
    }
}

fn to_mq(colour: Color) -> mq::Color {
    mq::Color::from_rgba(colour.r, colour.g, colour.b, colour.a)
}

fn to_vec(point: ScreenPoint) -> mq::Vec2 {
    mq::vec2(point.x, point.y)
}

impl Renderer for MacroquadRenderer {
    fn clear(&mut self, colour: Color) {
        mq::clear_background(to_mq(colour));
    }

    fn fill_tile(&mut self, shape: TileShape, colour: Color) {
        let [top, right, bottom, left] = shape.corners().map(to_vec);
        let colour = to_mq(colour);
        // A rhombus is two triangles sharing the top-to-bottom diagonal.
        mq::draw_triangle(top, right, bottom, colour);
        mq::draw_triangle(top, bottom, left, colour);
    }

    fn stroke_tile(&mut self, shape: TileShape, thickness: f32, colour: Color) {
        let corners = shape.corners();
        for i in 0..4 {
            self.line(corners[i], corners[(i + 1) % 4], thickness, colour);
        }
    }

    fn line(&mut self, from: ScreenPoint, to: ScreenPoint, thickness: f32, colour: Color) {
        mq::draw_line(from.x, from.y, to.x, to.y, thickness, to_mq(colour));
    }

    fn text(&mut self, text: &str, at: ScreenPoint, size: f32, colour: Color) {
        mq::draw_text(text, at.x, at.y, size, to_mq(colour));
    }
}

/// Reads macroquad's input into an engine [`Input`] for this frame.
fn read_input(input: &mut Input) {
    let (x, y) = mq::mouse_position();
    input.begin_frame(ScreenPoint::new(x, y));

    for (button, mq_button) in [
        (Button::Left, mq::MouseButton::Left),
        (Button::Right, mq::MouseButton::Right),
        (Button::Middle, mq::MouseButton::Middle),
    ] {
        if mq::is_mouse_button_pressed(mq_button) {
            input.press_button(button);
        } else if !mq::is_mouse_button_down(mq_button) {
            input.release_button(button);
        }
    }

    for (key, mq_key) in [
        (Key::Escape, mq::KeyCode::Escape),
        (Key::Space, mq::KeyCode::Space),
        (Key::Up, mq::KeyCode::Up),
        (Key::Down, mq::KeyCode::Down),
        (Key::Left, mq::KeyCode::Left),
        (Key::Right, mq::KeyCode::Right),
        (Key::Plus, mq::KeyCode::Equal),
        (Key::Minus, mq::KeyCode::Minus),
    ] {
        if mq::is_key_pressed(mq_key) {
            input.press_key(key);
        } else if !mq::is_key_down(mq_key) {
            input.release_key(key);
        }
    }

    let (_, wheel) = mq::mouse_wheel();
    if wheel != 0.0 {
        input.scroll_by(if wheel > 0.0 { 1 } else { -1 });
    }
}

/// Runs `app` until it asks to stop or the window closes.
///
/// The loop is the one from `ARCHITECTURE.md`: real frame time in,
/// [`App::tick`] called once per whole tick owed, then [`App::draw`] once with
/// the leftover fraction.
///
/// Call it from a `#[macroquad::main]` entry point, which is what creates the
/// window.
pub async fn run(mut app: impl App, rate: TickRate) {
    let mut clock = Clock::at_rate(rate);
    let mut canvas = MacroquadRenderer::new();
    let mut input = Input::default();
    let mut size = canvas.viewport();

    // The first frame's delta includes process start-up, which would otherwise
    // be charged to the simulation as a stall.
    clock.reset_accumulator();
    let _ = mq::get_frame_time();

    loop {
        let current = canvas.viewport();
        if current != size {
            size = current;
            app.resize(current.0, current.1);
        }

        read_input(&mut input);

        let frame = core::time::Duration::from_secs_f32(mq::get_frame_time().max(0.0));
        for tick in clock.advance(frame) {
            app.tick(tick, &input);
        }

        app.draw(&mut canvas, clock.alpha());

        if app.should_quit() {
            return;
        }
        mq::next_frame().await;
    }
}
