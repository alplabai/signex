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

/// The middle of the window, on a centred card.
const ON_THE_CARD: iced::Point = iced::Point::new(WINDOW.width / 2.0, WINDOW.height / 2.0);

fn is_about_close(m: &Message) -> bool {
    matches!(m, Message::Overlay(OverlayMsg::CloseAbout))
}

fn is_shortcuts_close(m: &Message) -> bool {
    matches!(m, Message::Overlay(OverlayMsg::CloseKeyboardShortcuts))
}

/// #2757: About closes on a press on its dimmed layer, not on its card.
#[test]
fn a_press_outside_about_closes_it_and_one_on_the_card_does_not() {
    let mut app = app();
    app.ui_state.about_open = true;
    let outside = messages_on_click_at(guarded(app.about_overlay()), WINDOW, None, OUTSIDE);
    assert!(outside.iter().any(is_about_close), "{outside:?}");
    let on_card = messages_on_click_at(guarded(app.about_overlay()), WINDOW, None, ON_THE_CARD);
    assert!(!on_card.iter().any(is_about_close), "{on_card:?}");
}

/// #2757: Keyboard Shortcuts closes on a press on its dimmed layer, not on
/// its card.
#[test]
fn a_press_outside_the_shortcuts_closes_them_and_one_on_the_card_does_not() {
    let mut app = app();
    app.ui_state.keyboard_shortcuts_open = true;
    let outside = messages_on_click_at(
        guarded(app.keyboard_shortcuts_overlay()),
        WINDOW,
        None,
        OUTSIDE,
    );
    assert!(outside.iter().any(is_shortcuts_close), "{outside:?}");
    let on_card = messages_on_click_at(
        guarded(app.keyboard_shortcuts_overlay()),
        WINDOW,
        None,
        ON_THE_CARD,
    );
    assert!(!on_card.iter().any(is_shortcuts_close), "{on_card:?}");
}

/// A full-window fill of `color`.
fn fill<'a>(color: iced::Color) -> Element<'a, Message> {
    use iced::Widget as _;
    iced::widget::container(iced::widget::Space::new())
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(color)),
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

/// The pixel at [`OUTSIDE`] of `layers` drawn over white.
fn outside_over_white(layers: Vec<Element<'_, Message>>) -> [u8; 3] {
    use iced::Widget as _;
    let mut stack: iced::widget::Stack<Element<'_, Message>> = iced::widget::Stack::new()
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .push(fill(iced::Color::WHITE));
    for layer in layers {
        stack = stack.push(layer);
    }
    let pixels = crate::layout_probe::draw_pixels(stack.boxed(), WINDOW);
    let at = (OUTSIDE.y as usize * WINDOW.width as usize + OUTSIDE.x as usize) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

/// #2757, mockup rows 12 and 13: About and Keyboard Shortcuts dim the
/// window behind them in the theme's dialog scrim, the dim every other
/// in-window dialog draws, in both themes.
#[test]
fn about_and_the_shortcuts_dim_the_window_in_the_dialog_scrim() {
    use signex_types::theme::{ThemeId, theme_tokens};
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let mut app = app();
        app.document_state.panel_ctx.tokens = theme_tokens(id);
        let scrim = crate::app::view::dialogs::dialog_scrim(&app.document_state.panel_ctx.tokens);
        let expected = outside_over_white(vec![fill(scrim)]);
        assert!(
            expected.iter().all(|&c| c < 200),
            "{id:?}: the scrim dims white: {expected:?}"
        );
        app.ui_state.about_open = true;
        assert_eq!(
            outside_over_white(app.about_overlay()),
            expected,
            "{id:?}: About"
        );
        app.ui_state.about_open = false;
        app.ui_state.keyboard_shortcuts_open = true;
        assert_eq!(
            outside_over_white(app.keyboard_shortcuts_overlay()),
            expected,
            "{id:?}: Keyboard Shortcuts"
        );
    }
}
