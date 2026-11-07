//! The Components panel previews pan inside the running app: a left drag on
//! the Symbol and Footprint preview goes through the whole window tree (docks,
//! overlays, the update loop rebuilding the view after every message) and the
//! canvas keeps its grab for the whole drag.

use std::path::PathBuf;

use iced::advanced::shell::{Bus, Waker};
use iced::{Event, Point, Size, mouse};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::{Cache, State};
use signex_types::theme::ThemeId;

use crate::app::{Message, PreferencesMsg, Signex};
use crate::dock::{DockMessage, PanelPosition};
use crate::library::messages::LibraryMessage;
use crate::library::state::ComponentsPanelRowKey;
use crate::panels::PanelKind;
use crate::task_driver::TaskDriver;

const WINDOW: Size = Size::new(1440.0, 1400.0);

fn stock_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

/// The app in `theme` with the stock library mounted, the Components panel
/// showing and its first row selected with its details loaded.
fn app_with_details(theme: ThemeId) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    app.ui_state.theme_id = theme;
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        crate::preferences::PrefMsg::DraftTheme(theme),
    )));
    let stock = stock_root();
    app.library.pending_mounts.clear();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount the stock library");
    show_components(&mut app);
    let (_, row) = app
        .library
        .all_components()
        .into_iter()
        .next()
        .expect("stock lists a component");
    let table = app
        .library
        .table_of_row(&stock, row.row_id)
        .expect("table of the row");
    let key = ComponentsPanelRowKey {
        library_path: stock,
        table,
        row_id: signex_library::RowId::from_uuid(row.row_id),
    };
    TaskDriver::new()
        .send(
            &mut app,
            Message::Library(LibraryMessage::ComponentsPanelRowPressed(key)),
        )
        .expect("the details read settled");
    // The press armed a drag-to-place; its release ends it.
    app.library.components_panel.drag = None;
    let details = &app.library.components_panel.details;
    let loaded = details.loaded.as_deref().expect("details loaded");
    assert!(loaded.symbol.is_some(), "the stock row has a symbol");
    assert!(loaded.footprint.is_some(), "the stock row has a footprint");
    app
}

fn show_components(app: &mut Signex) {
    for position in [
        PanelPosition::Left,
        PanelPosition::Right,
        PanelPosition::Bottom,
    ] {
        if let Some(i) = app
            .document_state
            .dock
            .panel_kinds(position)
            .iter()
            .position(|k| *k == PanelKind::Components)
        {
            let _ = app.update(Message::Dock(DockMessage::SelectTab(position, i)));
            return;
        }
    }
    let _ = app.update(Message::Overlay(crate::app::OverlayMsg::OpenPanel(
        PanelKind::Components,
    )));
}

/// Centre of the preview under the details section title `title`.
fn preview_centre(app: &Signex, title: &str) -> Point {
    let window = app.ui_state.main_window_id.expect("main window");
    let texts = crate::layout_probe::lay_out_texts(app.view(window), WINDOW);
    let dock_right = app.ui_state.left_width;
    let label = texts
        .iter()
        .filter(|t| t.text == title && t.bounds.x < dock_right)
        .max_by(|a, b| a.bounds.y.total_cmp(&b.bounds.y))
        .unwrap_or_else(|| panic!("no {title:?} section in the left dock"))
        .bounds;
    // Title, its bottom padding, then a preview box; a pick list (about 30 px
    // and the column gap) sits above the footprint preview when the row
    // offers several footprints.
    let picker = app
        .library
        .components_panel
        .details
        .loaded
        .as_deref()
        .filter(|d| title == "Footprint" && d.footprint_choices().len() >= 2)
        .map_or(0.0, |_| 30.0 + super::SP.s);
    Point::new(
        label.x + 60.0,
        label.y + label.height + super::SP.s + picker + super::PREVIEW_HEIGHT / 2.0,
    )
}

/// Feed `events` one per frame, the way the runtime does: the view is rebuilt
/// from the app after each one and every published message goes through
/// `update`. Returns the mouse interaction after each event.
fn drive(app: &mut Signex, events: &[(Event, Point)]) -> Vec<mouse::Interaction> {
    let window = app.ui_state.main_window_id.expect("main window");
    let mut renderer = crate::layout_probe::headless_renderer();
    let mut cache = Cache::default();
    let mut out = Vec::new();
    for (event, at) in events {
        let cursor = mouse::Cursor::Available(*at);
        let mut published = Bus::new();
        {
            let mut ui = UserInterface::build(app.view(window), WINDOW, cache, &mut renderer);
            let _ = ui.update(
                &iced::window::Headless,
                &Waker::noop(),
                std::slice::from_ref(event),
                cursor,
                &mut renderer,
                &mut published,
            );
            cache = ui.into_cache();
        }
        let mut driver = TaskDriver::new();
        for message in published {
            driver.send(app, message).expect("message settled");
        }
        // Read the interaction off a fresh build, as the next frame sees it.
        let mut ui = UserInterface::build(app.view(window), WINDOW, cache, &mut renderer);
        let (state, _) = ui.update(
            &iced::window::Headless,
            &Waker::noop(),
            &[],
            cursor,
            &mut renderer,
            &mut Bus::new(),
        );
        cache = ui.into_cache();
        out.push(match state {
            State::Updated {
                mouse_interaction, ..
            } => mouse_interaction,
            State::Outdated => mouse::Interaction::None,
        });
    }
    out
}

fn drag_through(app: &mut Signex, from: Point) -> Vec<mouse::Interaction> {
    let to = Point::new(from.x + 30.0, from.y + 12.0);
    let further = Point::new(from.x + 45.0, from.y + 20.0);
    drive(
        app,
        &[
            (
                Event::Mouse(mouse::Event::CursorMoved { position: from }),
                from,
            ),
            (
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                from,
            ),
            (Event::Mouse(mouse::Event::CursorMoved { position: to }), to),
            (
                Event::Mouse(mouse::Event::CursorMoved { position: further }),
                further,
            ),
            (
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                further,
            ),
        ],
    )
}

fn assert_pans(theme: ThemeId, title: &str) {
    let mut app = app_with_details(theme);
    let at = preview_centre(&app, title);
    let seen = drag_through(&mut app, at);
    use mouse::Interaction::{Grab, Grabbing};
    assert_eq!(
        seen,
        vec![Grab, Grabbing, Grabbing, Grabbing, Grab],
        "{theme:?} {title} preview at {at:?}: hover grabs, the drag holds the grab, release lets go"
    );
}

#[test]
fn the_symbol_preview_pans_in_the_app_on_every_theme() {
    for theme in [ThemeId::Signex, ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        assert_pans(theme, "Symbol");
    }
}

#[test]
fn the_footprint_preview_pans_in_the_app_on_every_theme() {
    for theme in [ThemeId::Signex, ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        assert_pans(theme, "Footprint");
    }
}
