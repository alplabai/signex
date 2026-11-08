use super::*;

use crate::app::ContextMenuState;
use crate::layout_probe::{LaidOutText, find, lay_out_texts, messages_on_click_at};
use iced::Widget as _;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::active_bar_dropdown::DropdownItem;

const WINDOW: (f32, f32) = (1000.0, 800.0);
const MENU: (f32, f32) = (200.0, 100.0);

/// `layers` stacked over an empty window, as `view` stacks overlays.
fn stacked(layers: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    let mut stack: iced::widget::Stack<Element<'_, Message>> = iced::widget::Stack::new()
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .push(
            iced::widget::Space::new()
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .boxed(),
        );
    for layer in layers {
        stack = stack.push(layer);
    }
    stack.boxed()
}

/// Every label of `layers` laid out in a window of `window`.
pub(in crate::app::view::context_menu) fn lay_out_layers(
    layers: Vec<Element<'_, Message>>,
    window: iced::Size,
) -> Vec<LaidOutText> {
    lay_out_texts(stacked(layers), window)
}

#[test]
fn an_anchor_with_room_is_kept() {
    assert_eq!(clamp_menu_origin((10.0, 20.0), MENU, WINDOW), (10.0, 20.0));
}

#[test]
fn a_menu_crossing_the_right_edge_shifts_left_of_it() {
    let (x, y) = clamp_menu_origin((950.0, 20.0), MENU, WINDOW);
    assert_eq!(x, WINDOW.0 - MENU.0 - EDGE_MARGIN);
    assert_eq!(y, 20.0);
}

/// The card moves up only as far as the bottom edge needs; it no longer
/// flips a whole card height above the cursor (the maintainer asked for
/// the origin at the cursor, clamped only at the window edge).
#[test]
fn a_menu_crossing_the_bottom_edge_moves_up_only_to_the_edge() {
    let (x, y) = clamp_menu_origin((10.0, 780.0), MENU, WINDOW);
    assert_eq!(x, 10.0);
    assert_eq!(y, WINDOW.1 - MENU.1 - EDGE_MARGIN);
}

#[test]
fn a_menu_larger_than_the_window_stays_on_screen() {
    assert_eq!(
        clamp_menu_origin((5.0, 5.0), (2000.0, 2000.0), WINDOW),
        (0.0, 0.0)
    );
}

#[test]
fn a_flyout_opens_right_of_its_parent_at_the_given_offset() {
    let (x, y) = submenu_origin((100.0, 50.0), 200.0, (200.0, 300.0), 64.0, WINDOW);
    assert_eq!((x, y), (300.0, 114.0));
}

#[test]
fn a_flyout_that_would_cross_the_right_edge_opens_left() {
    let (x, _) = submenu_origin((700.0, 0.0), 200.0, (200.0, 100.0), 0.0, WINDOW);
    assert_eq!(x, 500.0);
}

#[test]
fn a_flyout_that_would_cross_the_bottom_edge_moves_up_to_it() {
    let (_, y) = submenu_origin((100.0, 600.0), 200.0, (200.0, 300.0), 100.0, WINDOW);
    assert_eq!(y, WINDOW.1 - 300.0 - EDGE_MARGIN);
}

fn plain_card(tokens: &signex_types::theme::ThemeTokens) -> Element<'_, Message> {
    let entries = ["Alpha", "Bravo", "Charlie"]
        .into_iter()
        .map(|label| DropdownEntry::Item(DropdownItem::new(label, Message::Ui(UiMsg::Noop))))
        .collect();
    menu_card(entries, tokens, "").element
}

/// A menu opens with its corner at the cursor wherever it has room, in any
/// window: the clamp reads the window it is laid out in, so an undocked
/// window clamps to its own size.
#[test]
fn a_menu_opens_at_the_cursor_and_clamps_only_at_its_own_windows_edge() {
    for id in [ThemeId::GraphiteLight] {
        let tokens = theme_tokens(id);
        let metrics = MenuMetrics::of(&tokens);
        let card_h = 3.0 * metrics.item_height(false) + 2.0 * metrics.panel_pad;
        for window in [(1400.0_f32, 1000.0_f32), (500.0, 300.0)] {
            let size = iced::Size::new(window.0, window.1);
            let at = (120.0, 90.0);
            let texts = lay_out_texts(
                stacked(Signex::menu_overlay(
                    Message::Ui(UiMsg::Noop),
                    plain_card(&tokens),
                    at,
                )),
                size,
            );
            let first = find(&texts, "Alpha").bounds;
            let row_top = first.y + first.height / 2.0 - metrics.item_height(false) / 2.0;
            assert!(
                (row_top - (at.1 + metrics.panel_pad)).abs() < 0.6,
                "{id:?} {window:?}: first row at {row_top}, cursor at {}",
                at.1
            );
            assert!(first.x > at.0 && first.x < at.0 + 40.0, "{id:?}: {first:?}");

            // At the far corner the card stays inside that window.
            let corner = (window.0 - 5.0, window.1 - 5.0);
            let texts = lay_out_texts(
                stacked(Signex::menu_overlay(
                    Message::Ui(UiMsg::Noop),
                    plain_card(&tokens),
                    corner,
                )),
                size,
            );
            let last = find(&texts, "Charlie").bounds;
            let card_bottom =
                last.y + last.height / 2.0 + metrics.item_height(false) / 2.0 + metrics.panel_pad;
            assert!(
                (card_bottom - (window.1 - EDGE_MARGIN)).abs() < 1.0,
                "{id:?} {window:?}: card bottom {card_bottom}, height {card_h}"
            );
            assert!(
                last.x + last.width < window.0,
                "{id:?} {window:?}: {last:?}"
            );
        }
    }
}

/// The app as the maintainer ran it: Graphite, a tall window.
fn graphite_app() -> (Signex, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.document_state.panel_ctx.tokens = theme_tokens(ThemeId::GraphiteLight);
    let main = app
        .ui_state
        .main_window_id
        .expect("the app opens its main window");
    (app, main)
}

fn open_canvas_menu(app: &mut Signex, at: (f32, f32)) {
    app.interaction_state.context_menu = Some(ContextMenuState {
        x: at.0,
        y: at.1,
        world: None,
        window: None,
    });
}

/// A right-click in the middle of the canvas opens the menu at the cursor.
/// It used to open over the menu bar: the placement assumed 22 icon rows,
/// taller than the room below the click, so it flipped the card above the
/// cursor and clamped it to the top of the window.
#[test]
fn a_canvas_right_click_opens_the_menu_at_the_cursor() {
    let (mut app, main) = graphite_app();
    let window = iced::Size::new(1400.0, 1000.0);
    let at = (600.0, 450.0);
    open_canvas_menu(&mut app, at);
    let metrics = MenuMetrics::of(&app.document_state.panel_ctx.tokens);
    let texts = lay_out_layers(app.context_menu_overlay(main), window);
    let first = find(
        &texts,
        &super::super::items::command_text("find_similar_objects"),
    )
    .bounds;
    let row_top = first.y + first.height / 2.0 - metrics.item_height(true) / 2.0;
    assert!(
        (row_top - (at.1 + metrics.panel_pad)).abs() < 0.6,
        "first row at {row_top}, cursor at {}",
        at.1
    );
    assert!(first.x > at.0 && first.x < at.0 + 60.0, "{first:?}");
}

/// Graphite menu rows are 28 px apart, and the card is fitted between 200
/// and 320 px.
#[test]
fn the_canvas_menu_has_28_px_rows_and_a_fitted_width() {
    let (mut app, main) = graphite_app();
    open_canvas_menu(&mut app, (100.0, 100.0));
    let width = crate::layout_probe::root_size(
        app.view_context_menu(main).element,
        iced::Size::new(1400.0, 1000.0),
    )
    .width;
    assert!((200.0..=320.0).contains(&width), "{width}");
    let texts = lay_out_layers(
        app.context_menu_overlay(main),
        iced::Size::new(1400.0, 1000.0),
    );
    let y = |id: &str| {
        let b = find(&texts, &super::super::items::command_text(id)).bounds;
        b.y + b.height / 2.0
    };
    let pitch = y("smart_paste") - y("paste");
    assert!((pitch - 28.0).abs() < 0.6, "rows {pitch} px apart");
}

/// The click lands on the row that is drawn there: a press in the gap
/// between a label and its shortcut, one pixel inside the row's top edge,
/// runs that row and does not close the menu.
#[test]
fn a_click_anywhere_on_a_drawn_row_runs_that_row() {
    let (mut app, main) = graphite_app();
    open_canvas_menu(&mut app, (300.0, 200.0));
    let window = iced::Size::new(1400.0, 1000.0);
    let metrics = MenuMetrics::of(&app.document_state.panel_ctx.tokens);
    let texts = lay_out_layers(app.context_menu_overlay(main), window);
    let label = find(&texts, &super::super::items::command_text("paste")).bounds;
    let hint = app.keymap_shortcut_label("paste");
    let gap_x = if hint.is_empty() {
        label.x + label.width + 20.0
    } else {
        find(&texts, &hint).bounds.x - 4.0
    };
    let row_top = label.y + label.height / 2.0 - metrics.item_height(true) / 2.0;
    let point = iced::Point::new(gap_x, row_top + 1.0);
    let messages =
        messages_on_click_at(stacked(app.context_menu_overlay(main)), window, None, point);
    assert!(
        messages.iter().any(|m| matches!(
            m,
            Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command { id, .. }))
                if id.as_str() == "paste"
        )),
        "a press at {point:?} sent {messages:?}"
    );
    assert!(
        !messages
            .iter()
            .any(|m| matches!(m, Message::ContextMenu(ContextMenuMsg::Close))),
        "the press fell through to the dismiss layer: {messages:?}"
    );
}

/// The open submenu's first row sits level with its launcher, right of the
/// card (#2136), while the window has room below; in a short window the
/// flyout moves up only as far as its bottom edge needs.
#[test]
fn the_place_flyout_lines_up_with_its_launcher() {
    let (mut app, main) = graphite_app();
    open_canvas_menu(&mut app, (200.0, 150.0));
    app.interaction_state.context_submenu = Some(ContextSubmenu::Place);
    let mid = |b: iced::Rectangle| b.y + b.height / 2.0;

    let tall = lay_out_layers(
        app.context_menu_overlay(main),
        iced::Size::new(1400.0, 1600.0),
    );
    let launcher = find(&tall, "Place").bounds;
    let first = find(&tall, "Wire").bounds;
    assert!(
        (mid(first) - mid(launcher)).abs() < 0.6,
        "flyout row at {first:?}, launcher at {launcher:?}"
    );
    assert!(
        first.x > launcher.x + 150.0,
        "the flyout opens right of the card"
    );

    let short = lay_out_layers(
        app.context_menu_overlay(main),
        iced::Size::new(1400.0, 700.0),
    );
    let first = find(&short, "Wire").bounds;
    let metrics = MenuMetrics::of(&app.document_state.panel_ctx.tokens);
    let flyout_top = mid(first) - metrics.item_height(true) / 2.0 - metrics.panel_pad;
    assert!(
        flyout_top >= 0.0,
        "the flyout runs above the window: {first:?}"
    );
    assert!(
        flyout_top < mid(launcher),
        "a short window lifts the flyout to keep it inside: {first:?}"
    );
}

/// The row height the placement assumes is the one the dropdown really lays
/// out, with and without icons, in both looks; a submenu launcher is exactly
/// one item row tall (#2136).
#[test]
fn rows_and_launchers_lay_out_at_the_metrics_row_height() {
    let icon = iced::widget::svg::Handle::from_memory(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"/>"#.as_slice(),
    );
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        let metrics = MenuMetrics::of(&tokens);
        for has_icons in [false, true] {
            let with_icon = |item: DropdownItem<Message>| {
                DropdownEntry::Item(if has_icons {
                    item.icon(icon.clone())
                } else {
                    item
                })
            };
            let entries = vec![
                with_icon(DropdownItem::new("Alpha", Message::Ui(UiMsg::Noop))),
                super::super::items::submenu_launcher(
                    has_icons.then(|| icon.clone()),
                    "Launcher",
                    ContextSubmenu::Align,
                    false,
                ),
                with_icon(DropdownItem::new("Charlie", Message::Ui(UiMsg::Noop))),
            ];
            let menu = signex_widgets::active_bar_dropdown::view(entries, &tokens, Some(240.0));
            let texts = lay_out_texts(menu, iced::Size::new(400.0, 400.0));
            let mid = |label: &str| {
                let b = find(&texts, label).bounds;
                b.y + b.height / 2.0
            };
            let row = metrics.item_height(has_icons);
            for (a, b) in [("Alpha", "Launcher"), ("Launcher", "Charlie")] {
                let pitch = mid(b) - mid(a);
                assert!(
                    (pitch - row).abs() < 0.6,
                    "{id:?} icons {has_icons}: {a} to {b} is {pitch}, rows are {row}"
                );
            }
        }
    }
}
