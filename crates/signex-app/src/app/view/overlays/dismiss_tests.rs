//! #2377 — the error notice, the netlist-incomplete prompt and the custom
//! net-colour picker close on a press outside their card only. They used to
//! reach the dismiss helper already placed in a full-window container, so a
//! press on the card's padding fell through to the click-outside layer.

use super::*;
use crate::app::state::{ErrorNotice, NetlistIncompletePrompt};
use crate::layout_probe::{lay_out_texts, messages_on_click_at};

const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

/// A point in the window corner, outside every centred or anchored card.
const OUTSIDE: iced::Point = iced::Point::new(4.0, WINDOW.height - 4.0);

fn app() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    app
}

/// The one overlay layer, inside the scroll guard `collect_overlays` puts
/// around every layer.
fn guarded(mut layers: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    assert_eq!(layers.len(), 1, "one layer: backdrop and card together");
    super::scroll_guard(layers.pop().expect("one layer"))
}

/// A point on the card just inside its padding: `gap` px left of and above
/// the laid-out text `label`, which sits inside the card's body padding.
fn on_padding_near(layers: Vec<Element<'_, Message>>, label: &str, gap: f32) -> iced::Point {
    let texts = lay_out_texts(guarded(layers), WINDOW);
    let text = texts
        .iter()
        .find(|t| t.text == label)
        .unwrap_or_else(|| panic!("{label:?} not laid out: {texts:#?}"));
    iced::Point::new(text.bounds.x - gap, text.bounds.y - gap)
}

fn error_app() -> Signex {
    let mut app = app();
    app.document_state.error_notice = Some(ErrorNotice::save(crate::app::state::verbatim(
        "disk full while saving",
    )));
    app
}

fn is_error_dismiss(m: &Message) -> bool {
    matches!(m, Message::Overlay(OverlayMsg::DismissErrorNotice))
}

#[test]
fn a_press_on_the_error_notice_padding_keeps_it_open() {
    let app = error_app();
    let at = on_padding_near(app.error_notice_overlay(), "Save Failed", 4.0);
    let messages = messages_on_click_at(guarded(app.error_notice_overlay()), WINDOW, None, at);
    assert!(
        !messages.iter().any(is_error_dismiss),
        "a press on the card dismissed it: {messages:?}"
    );
}

#[test]
fn a_press_outside_the_error_notice_dismisses_it() {
    let app = error_app();
    let messages = messages_on_click_at(guarded(app.error_notice_overlay()), WINDOW, None, OUTSIDE);
    assert!(messages.iter().any(is_error_dismiss), "{messages:?}");
}

fn prompt_app() -> Signex {
    let mut app = app();
    app.document_state.netlist_incomplete_prompt = Some(NetlistIncompletePrompt {
        save_path: "out.net".into(),
        messages: vec!["sheet two is not in the netlist".to_owned()],
        ctx: signex_output::ExportContext {
            sheets: Vec::new(),
            metadata: signex_output::ProjectMetadata::default(),
            netlist: None,
            netlist_sheets: Vec::new(),
            current_sheet: None,
            netlist_placements: Vec::new(),
        },
        omitted: vec![crate::app::state::OmittedPage {
            file: "two.snxsch".to_owned(),
            reason: crate::app::state::OmittedReason::FileMissing { placed_by: None },
        }],
    });
    app
}

fn is_prompt_cancel(m: &Message) -> bool {
    matches!(m, Message::Export(ExportMsg::NetlistCancelIncomplete))
}

#[test]
fn a_press_on_the_netlist_prompt_padding_keeps_it_open() {
    let app = prompt_app();
    let at = on_padding_near(
        app.netlist_incomplete_prompt_overlay(),
        "Netlist is incomplete",
        4.0,
    );
    let messages = messages_on_click_at(
        guarded(app.netlist_incomplete_prompt_overlay()),
        WINDOW,
        None,
        at,
    );
    assert!(
        !messages.iter().any(is_prompt_cancel),
        "a press on the card cancelled it: {messages:?}"
    );
}

#[test]
fn a_press_outside_the_netlist_prompt_cancels_it() {
    let app = prompt_app();
    let messages = messages_on_click_at(
        guarded(app.netlist_incomplete_prompt_overlay()),
        WINDOW,
        None,
        OUTSIDE,
    );
    assert!(messages.iter().any(is_prompt_cancel), "{messages:?}");
}

fn picker_app() -> Signex {
    let mut app = app();
    app.ui_state.net_color_custom.show = true;
    app
}

fn is_picker_close(m: &Message) -> bool {
    matches!(m, Message::NetColor(NetColorMsg::CustomShow(false)))
}

#[test]
fn a_press_on_the_net_colour_picker_padding_keeps_it_open() {
    let app = picker_app();
    let at = on_padding_near(app.net_color_custom_overlay(), "Quick Pick", 4.0);
    let messages = messages_on_click_at(guarded(app.net_color_custom_overlay()), WINDOW, None, at);
    assert!(
        !messages.iter().any(is_picker_close),
        "a press on the card closed it: {messages:?}"
    );
}

#[test]
fn a_press_outside_the_net_colour_picker_closes_it() {
    let app = picker_app();
    let messages = messages_on_click_at(
        guarded(app.net_color_custom_overlay()),
        WINDOW,
        None,
        OUTSIDE,
    );
    assert!(messages.iter().any(is_picker_close), "{messages:?}");
}
