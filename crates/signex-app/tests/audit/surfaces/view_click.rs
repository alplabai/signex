//! A pointer click on the drawn main window. Some messages carry a type the
//! crate does not export, so only the widget that shows them can build one.

use crate::render::{self, Crop};
use crate::settle_support;
use iced::advanced::shell::{Bus, Waker};
use iced::mouse::{self, Button, Cursor};
use iced::{Event, Point, Size};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::Cache;
use signex_app::app::{Message, Signex};

/// Scan steps in pixels, finer than the smallest toolbar button.
const STEP_X: usize = 12;
const STEP_Y: usize = 8;

/// Click across `area` until a widget publishes a message `want` accepts,
/// then send that one message. Every other message is dropped unapplied.
pub fn click_view(
    app: &mut Signex,
    r: &mut iced::Renderer,
    area: Crop,
    button: Button,
    want: impl Fn(&Message) -> bool,
) -> Result<(), String> {
    click_view_stepped(app, r, area, button, (STEP_X, STEP_Y), want)
}

/// [`click_view`] with its own scan steps, for wide targets such as a
/// section heading.
pub fn click_view_stepped(
    app: &mut Signex,
    r: &mut iced::Renderer,
    area: Crop,
    button: Button,
    step: (usize, usize),
    want: impl Fn(&Message) -> bool,
) -> Result<(), String> {
    let message = find_message(app, r, area, button, step, want)
        .ok_or_else(|| format!("no widget in {area:?} published the wanted message"))?;
    settle_support::settle(app, message);
    Ok(())
}

/// Whether a click somewhere in `area` would publish a message `want`
/// accepts. Nothing is sent, so the app does not change.
pub fn view_offers(
    app: &Signex,
    r: &mut iced::Renderer,
    area: Crop,
    step: (usize, usize),
    want: impl Fn(&Message) -> bool,
) -> bool {
    find_message(app, r, area, Button::Left, step, want).is_some()
}

fn find_message(
    app: &Signex,
    r: &mut iced::Renderer,
    area: Crop,
    button: Button,
    (step_x, step_y): (usize, usize),
    want: impl Fn(&Message) -> bool,
) -> Option<Message> {
    let window = app.ui_state.main_window_id?;
    let size = Size::new(render::WIDTH as f32, render::HEIGHT as f32);
    let mut ui = UserInterface::build(app.view(window), size, Cache::default(), r);
    let waker = Waker::noop();
    for y in (area.y..area.y + area.h).step_by(step_y) {
        for x in (area.x..area.x + area.w).step_by(step_x) {
            let at = Point::new(x as f32, y as f32);
            let events = [
                Event::Mouse(mouse::Event::CursorMoved { position: at }),
                Event::Mouse(mouse::Event::ButtonPressed(button)),
                Event::Mouse(mouse::Event::ButtonReleased(button)),
            ];
            let mut bus = Bus::new();
            let _ = ui.update(
                &iced::window::Headless,
                &waker,
                &events,
                Cursor::Available(at),
                r,
                &mut bus,
            );
            for (message, _receipt) in bus.drain() {
                if want(&message) {
                    return Some(message);
                }
            }
        }
    }
    None
}
