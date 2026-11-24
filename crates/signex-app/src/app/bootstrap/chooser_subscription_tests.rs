//! Mockup row 71: window events reach the net colour chooser through the
//! listeners `Signex::subscription` really returns, run on their own streams.

use futures::executor::block_on;
use futures::{StreamExt, stream};
use iced::advanced::subscription::{Event as RuntimeEvent, Hasher, Recipe, into_recipes};
use iced::event::Status;
use iced::window::{self, Id};
use iced::{Event, Point, mouse, touch};

use super::window_subscriptions::{net_color_chooser_presses, window_focus};
use crate::app::state::{ModalId, WindowKind};
use crate::app::{Message, NetColorMsg, Signex};

type Listener = Box<dyn Recipe<Output = Message>>;

fn identity(recipe: &dyn Recipe<Output = Message>) -> u64 {
    let mut hasher = Hasher::default();
    recipe.hash(&mut hasher);
    std::hash::Hasher::finish(&hasher)
}

fn identities(subscription: iced::Subscription<Message>) -> Vec<u64> {
    into_recipes(subscription)
        .iter()
        .map(|recipe| identity(recipe.as_ref()))
        .collect()
}

/// The focus and chooser press listeners among the app's live subscriptions.
fn live_listeners(app: &Signex) -> Vec<Listener> {
    let mut wanted = identities(window_focus());
    wanted.extend(identities(net_color_chooser_presses(true)));
    into_recipes(app.subscription())
        .into_iter()
        .filter(|recipe| wanted.contains(&identity(recipe.as_ref())))
        .collect()
}

fn has_press_listener(app: &Signex) -> bool {
    let presses = identities(net_color_chooser_presses(true));
    identities(app.subscription())
        .iter()
        .any(|id| presses.contains(id))
}

/// Runs `event` through the live listeners and applies what they produce.
fn deliver(app: &mut Signex, window: Id, event: Event, status: Status) {
    let input = RuntimeEvent::Interaction {
        window,
        event,
        status,
    };
    let mut messages = Vec::new();
    for listener in live_listeners(app) {
        let output = listener.stream(stream::iter([input.clone()]).boxed());
        messages.extend(block_on(output.collect::<Vec<_>>()));
    }
    for message in messages {
        let _ = app.update(message);
    }
}

/// An app with the palette in its own window and the chooser open on a row.
fn app_with_open_chooser() -> (Signex, Id) {
    let (mut app, _task) = Signex::new();
    let palette = Id::unique();
    app.ui_state
        .windows
        .insert(palette, WindowKind::DetachedModal(ModalId::NetColorPalette));
    let _ = app.update(Message::NetColor(NetColorMsg::RowOpened("A".into())));
    (app, palette)
}

fn chooser(app: &Signex) -> Option<&str> {
    app.ui_state.net_color_palette.chooser.as_deref()
}

fn presses() -> [Event; 3] {
    [
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        Event::Touch(touch::Event::FingerPressed {
            id: touch::Finger(0),
            position: Point::ORIGIN,
        }),
    ]
}

#[test]
fn the_press_listener_runs_only_while_a_chooser_is_open() {
    let (mut app, _palette) = app_with_open_chooser();
    assert!(has_press_listener(&app));
    let _ = app.update(Message::NetColor(NetColorMsg::ChooserDismissed));
    assert!(!has_press_listener(&app));
}

#[test]
fn a_press_in_another_window_closes_the_chooser_through_the_subscription() {
    for status in [Status::Ignored, Status::Captured] {
        for press in presses() {
            let (mut app, _palette) = app_with_open_chooser();
            deliver(&mut app, Id::unique(), press, status);
            assert_eq!(chooser(&app), None);
        }
    }
}

#[test]
fn a_press_in_the_palette_window_leaves_the_chooser_to_the_palette() {
    for status in [Status::Ignored, Status::Captured] {
        for press in presses() {
            let (mut app, palette) = app_with_open_chooser();
            deliver(&mut app, palette, press, status);
            assert_eq!(chooser(&app), Some("A"));
        }
    }
}

/// A press in another application or on the OS title bar reaches Signex only
/// as focus changing hands.
#[test]
fn the_palette_window_losing_focus_leaves_the_chooser_open() {
    let (mut app, palette) = app_with_open_chooser();
    let _ = app.update(Message::Window(crate::app::WindowMsg::WindowFocused(
        palette,
    )));
    let unfocused = Event::Window(window::Event::Unfocused);
    deliver(&mut app, palette, unfocused, Status::Ignored);
    assert_eq!(app.ui_state.focused_window, None, "the event was delivered");
    assert_eq!(chooser(&app), Some("A"));

    let main = Id::unique();
    let focused = Event::Window(window::Event::Focused);
    deliver(&mut app, main, focused, Status::Ignored);
    assert_eq!(app.ui_state.focused_window, Some(main));
    assert_eq!(chooser(&app), Some("A"));
}
