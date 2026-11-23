//! Mockup row 157: the end of a held Esc reaches the app through the listener
//! `Signex::subscription` really returns, run on its own stream.

use futures::executor::block_on;
use futures::{StreamExt, stream};
use iced::advanced::subscription::{Event as RuntimeEvent, Hasher, Recipe, into_recipes};
use iced::event::Status;
use iced::keyboard::{self, Key, key::Named};
use iced::window::{self, Id};
use iced::{Event, Subscription};

use super::window_subscriptions::held_escape_end;
use crate::app::{Message, Signex};

fn identity(recipe: &dyn Recipe<Output = Message>) -> u64 {
    let mut hasher = Hasher::default();
    recipe.hash(&mut hasher);
    std::hash::Hasher::finish(&hasher)
}

fn identities(subscription: Subscription<Message>) -> Vec<u64> {
    into_recipes(subscription)
        .iter()
        .map(|recipe| identity(recipe.as_ref()))
        .collect()
}

fn has_listener(app: &Signex) -> bool {
    let wanted = identities(held_escape_end(true));
    identities(app.subscription())
        .iter()
        .any(|id| wanted.contains(id))
}

/// Runs `event` through the live listener and applies what it produces.
fn deliver(app: &mut Signex, event: Event, status: Status) {
    let wanted = identities(held_escape_end(true));
    let input = RuntimeEvent::Interaction {
        window: Id::unique(),
        event,
        status,
    };
    let mut messages = Vec::new();
    for recipe in into_recipes(app.subscription()) {
        if wanted.contains(&identity(recipe.as_ref())) {
            let output = recipe.stream(stream::iter([input.clone()]).boxed());
            messages.extend(block_on(output.collect::<Vec<_>>()));
        }
    }
    for message in messages {
        let _ = app.update(message);
    }
}

fn app_holding_esc() -> Signex {
    let (mut app, _task) = Signex::new();
    app.interaction_state.annotation_tool.is_resume_esc_held = true;
    app
}

fn is_held(app: &Signex) -> bool {
    app.interaction_state.annotation_tool.is_resume_esc_held
}

fn released(key: Key) -> Event {
    Event::Keyboard(keyboard::Event::KeyReleased {
        key: key.clone(),
        modified_key: key,
        physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
        location: keyboard::Location::Standard,
        modifiers: keyboard::Modifiers::empty(),
    })
}

#[test]
fn the_held_esc_listener_runs_only_while_the_press_is_held() {
    let mut app = app_holding_esc();
    assert!(has_listener(&app));
    deliver(
        &mut app,
        released(Key::Named(Named::Escape)),
        Status::Ignored,
    );
    assert!(!has_listener(&app));
}

#[test]
fn the_release_of_esc_ends_the_hold_even_when_a_widget_captured_it() {
    for status in [Status::Ignored, Status::Captured] {
        let mut app = app_holding_esc();
        deliver(&mut app, released(Key::Named(Named::Escape)), status);
        assert!(!is_held(&app));
    }
}

#[test]
fn the_release_of_another_key_leaves_the_hold() {
    let mut app = app_holding_esc();
    deliver(
        &mut app,
        released(Key::Named(Named::Enter)),
        Status::Ignored,
    );
    assert!(is_held(&app));
}

#[test]
fn a_window_losing_focus_ends_the_hold_whose_release_it_would_lose() {
    for event in [window::Event::Unfocused, window::Event::Closed] {
        let mut app = app_holding_esc();
        deliver(&mut app, Event::Window(event), Status::Ignored);
        assert!(!is_held(&app));
    }
}
