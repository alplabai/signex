//! Mockup row 207, laid out: a pane's tab strip presses its tabs and opens
//! its ⋯ menu, a splitter takes the press that starts its drag, and the
//! pane menu runs its rows and closes on a press outside.

use iced::{Point, Size};

use crate::app::handlers::document_panes_tests::{app, drop_tab};
use crate::app::{Message, PaneMsg, Signex, WindowMsg};
use crate::layout_probe::{messages_on_click, messages_on_click_at};

const WINDOW: Size = Size::new(1600.0, 1000.0);

/// Three sheets, `power` dropped on the right edge: the left pane holds
/// `mcu` and `io` and draws `io`, the right one `power` with the focus.
fn split_app() -> Signex {
    let (mut app, main) = app();
    drop_tab(&mut app, main, 0, (0.95, 0.5));
    app
}

fn pane_msgs(messages: Vec<Message>) -> Vec<PaneMsg> {
    messages
        .into_iter()
        .filter_map(|message| match message {
            Message::Window(WindowMsg::Pane(msg)) => Some(msg),
            _ => None,
        })
        .collect()
}

#[test]
fn a_pane_strip_presses_its_tabs_and_opens_its_menu() {
    let app = split_app();
    let strip = app.pane_strip(0, &[1, 2], 2, false);
    let pressed = pane_msgs(messages_on_click(
        strip,
        Size::new(600.0, 26.0),
        "mcu.snxsch",
    ));
    assert!(
        pressed
            .iter()
            .any(|msg| matches!(msg, PaneMsg::TabPressed { pane: 0, tab: 1 })),
        "{pressed:?}"
    );
    let strip = app.pane_strip(0, &[1, 2], 2, false);
    let menu = pane_msgs(messages_on_click(strip, Size::new(600.0, 26.0), "⋯"));
    assert!(
        menu.iter()
            .any(|msg| matches!(msg, PaneMsg::MenuToggled(0))),
        "{menu:?}"
    );
}

#[test]
fn a_splitter_takes_the_press_on_the_line_between_the_panes() {
    let app = split_app();
    let view = app.tiled_pane_view().expect("split");
    let area = app.document_area_in(None);
    let size = (area.width, area.height);
    let line = (area.width / 2.0).floor();
    let layer = app.pane_splitters(&view, size);
    let on_line = pane_msgs(messages_on_click_at(
        layer,
        Size::new(size.0, size.1),
        None,
        Point::new(line, size.1 / 2.0),
    ));
    assert!(
        on_line
            .iter()
            .any(|msg| matches!(msg, PaneMsg::SplitterPressed(0))),
        "{on_line:?}"
    );
    let layer = app.pane_splitters(&view, size);
    let beside = pane_msgs(messages_on_click_at(
        layer,
        Size::new(size.0, size.1),
        None,
        Point::new(line - 40.0, size.1 / 2.0),
    ));
    assert!(beside.is_empty(), "{beside:?}");
}

#[test]
fn the_pane_menu_runs_its_rows_and_closes_outside() {
    let mut app = split_app();
    assert!(app.pane_menu_overlay().is_none(), "closed");
    app.ui_state.workspace_layouts.panes.menu = Some(0);
    let menu = app.pane_menu_overlay().expect("open");
    let picked = pane_msgs(messages_on_click(menu, WINDOW, "Move Tab to Other Pane"));
    assert!(
        picked.iter().any(|msg| matches!(
            msg,
            PaneMsg::MenuCommandPicked { pane: 0, command } if command.as_str() == "move_tab_to_other_pane"
        )),
        "{picked:?}"
    );
    let menu = app.pane_menu_overlay().expect("open");
    let outside = pane_msgs(messages_on_click_at(
        menu,
        WINDOW,
        None,
        Point::new(5.0, 990.0),
    ));
    assert!(
        outside
            .iter()
            .any(|msg| matches!(msg, PaneMsg::MenuDismissed)),
        "{outside:?}"
    );
}
