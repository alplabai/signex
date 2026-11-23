//! #2843: the footprint and sketch Active Bar fits and centres on the canvas
//! between the docks, in the main window and in an undocked one.

use std::path::PathBuf;

use iced::{Point, Size};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar::Metrics;

use super::*;
use crate::app::state::WindowKind;
use crate::app::{FootprintDocument, TabInfo, TabKind};
use crate::dock::DOCK_HANDLE;
use crate::library::editor::footprint::unified_active_bar::{
    BarSpan, PlacedBar, bar_items, place_bar, scrolling_slots,
};
use crate::library::editor::footprint::{EditorMode, FpActiveBarMenu};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

const WINDOW: Size = Size::new(1366.0, 768.0);
const LEFT: f32 = 240.0;
const RIGHT: f32 = 300.0;
const PATH: &str = "bar-span.snxfpt";

fn app_in_sketch() -> (Signex, iced::window::Id) {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.theme_id = ThemeId::GraphiteLight;
    app.ui_state.preferences_draft_theme = ThemeId::GraphiteLight;
    app.sync_panel_theme();
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    app.document_state.dock = crate::dock::graphite_first_run();
    app.ui_state.left_width = LEFT;
    app.ui_state.right_width = RIGHT;
    let path = PathBuf::from(PATH);
    app.document_state.tabs.push(TabInfo {
        title: "f".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    let mut editor = FootprintDocument::new(path.clone(), file);
    editor.state.mode = EditorMode::Sketch;
    app.document_state.footprint_editors.insert(path, editor);
    (app, main)
}

/// The sketch bar as placed in `span`, anchored on its Snap ▾ trigger.
fn placed(app: &Signex, span: BarSpan) -> PlacedBar {
    let editor = &app.document_state.footprint_editors[&PathBuf::from(PATH)];
    let items = bar_items(
        editor,
        app.ui_state.effective_theme_id(),
        &app.document_state.panel_ctx.tokens,
        &app.ui_state.i18n,
    );
    place_bar(&items, FpActiveBarMenu::Snap, scrolling_slots(editor), span)
}

fn assert_inside(bar: PlacedBar, span: BarSpan) {
    assert!(bar.left >= span.x, "{bar:?} starts left of {span:?}");
    assert!(
        bar.left + bar.width <= span.x + span.width,
        "{bar:?} ends right of {span:?}"
    );
    let snap = bar.trigger_x.expect("Snap trigger");
    assert!(
        snap + Metrics::GRAPHITE.btn <= bar.left + bar.width,
        "Snap at {snap} is outside {bar:?}"
    );
}

/// The review's case: the 966 px bar in a 1366 px window used to span
/// x = 200..1166, over the left dock and Properties.
#[test]
fn at_1366_px_with_default_docks_the_bar_stays_on_the_canvas() {
    let (app, main) = app_in_sketch();
    let span = app.footprint_bar_span(main);
    assert_eq!(span.x, LEFT + DOCK_HANDLE);
    assert_eq!(span.width, WINDOW.width - 2.0 * DOCK_HANDLE - LEFT - RIGHT);
    let bar = placed(&app, span);
    assert!(bar.width < 966.0, "the strip scrolls: {bar:?}");
    assert_inside(bar, span);
}

/// The drawn bar and the dropdown anchor agree: a click where the anchor
/// says Snap ▾ is, in the real overlay, presses Snap ▾.
#[test]
fn a_click_at_the_anchored_snap_trigger_presses_it() {
    let (app, main) = app_in_sketch();
    let bar = placed(&app, app.footprint_bar_span(main));
    let m = Metrics::GRAPHITE;
    let at = Point::new(
        bar.trigger_x.expect("Snap trigger") + m.btn / 2.0,
        app.canvas_bounds().y
            + crate::active_bar::bar_top_margin()
            + crate::active_bar::bar_height_for(&m) / 2.0,
    );
    let stack = iced::widget::Stack::with_children(app.footprint_active_bar_overlay(main)).boxed();
    let messages = crate::layout_probe::messages_on_click_at(stack, WINDOW, None, at);
    assert!(
        messages.iter().any(|message| matches!(
            message,
            Message::Library(LibraryMessage::PrimitiveEditorEvent {
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleActiveBarMenu(
                    FpActiveBarMenu::Snap
                )),
                ..
            })
        )),
        "{messages:?}"
    );
}

/// Collapsed docks keep only their rails, so the whole bar fits and centres
/// between them.
#[test]
fn with_collapsed_docks_the_whole_bar_centres_between_the_rails() {
    let (mut app, main) = app_in_sketch();
    for side in [
        crate::dock::PanelPosition::Left,
        crate::dock::PanelPosition::Right,
    ] {
        app.document_state
            .dock
            .update(crate::dock::DockMessage::ToggleCollapse(side));
    }
    let span = app.footprint_bar_span(main);
    let rail = span.x;
    assert!(rail < LEFT, "the left dock is a rail");
    assert_eq!(span.width, WINDOW.width - 2.0 * rail);
    let bar = placed(&app, span);
    assert_inside(bar, span);
    let centre = bar.left + bar.width / 2.0;
    assert!((centre - WINDOW.width / 2.0).abs() < 0.01, "{bar:?}");
}

/// An undocked tab window lays out the docks at its own width.
#[test]
fn an_undocked_window_fits_the_bar_to_its_own_canvas() {
    let (mut app, _main) = app_in_sketch();
    let undocked = iced::window::Id::unique();
    app.ui_state.windows.insert(
        undocked,
        WindowKind::UndockedTab {
            path: PathBuf::from(PATH),
            title: "f".to_string(),
        },
    );
    const OWN_WIDTH: f32 = 1100.0;
    app.ui_state
        .set_undocked_window_size(undocked, (OWN_WIDTH, WINDOW.height));
    let span = app.footprint_bar_span(undocked);
    assert_eq!(span.x, LEFT + DOCK_HANDLE);
    assert_eq!(span.width, OWN_WIDTH - 2.0 * DOCK_HANDLE - LEFT - RIGHT);
    assert_inside(placed(&app, span), span);
}
