//! The Components panel previews (and the component editor's General tab
//! previews, which share their canvases) pan inside the running app: a left
//! drag goes through the whole window tree (docks, overlays, the update loop
//! rebuilding the view after every message) and the canvas keeps its grab for
//! the whole drag.

use std::path::PathBuf;

use iced::advanced::shell::{Bus, Waker};
use iced::{Event, Point, Rectangle, Size, mouse};
use iced_runtime::UserInterface;
use iced_runtime::user_interface::{Cache, State};
use signex_types::theme::ThemeId;

use crate::app::{Message, PreferencesMsg, Signex};
use crate::dock::PanelPosition;
use crate::library::messages::LibraryMessage;
use crate::library::state::ComponentsPanelRowKey;
use crate::panels::PanelKind;
use crate::task_driver::TaskDriver;

const WINDOW: Size = Size::new(1440.0, 1400.0);
const THEMES: [ThemeId; 3] = [
    ThemeId::Signex,
    ThemeId::GraphiteLight,
    ThemeId::GraphiteDark,
];

fn stock_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/stock-library/Stock.snxlib")
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir") {
        let path = entry.expect("entry").path();
        let dest = to.join(path.file_name().expect("file name"));
        if path.is_dir() {
            copy_dir(&path, &dest);
        } else {
            std::fs::copy(&path, &dest).expect("copy");
        }
    }
}

fn send(app: &mut Signex, message: Message) {
    TaskDriver::new()
        .send(app, message)
        .expect("the message settled");
}

/// The app in `theme` with the sample project and its sheet open (a bare
/// Start screen has no docks), the stock library mounted, the Components
/// panel showing in the `side` dock and its first stock row selected with
/// its details loaded. Returns the selected row too.
fn app_with_details(
    theme: ThemeId,
    side: PanelPosition,
) -> (Signex, tempfile::TempDir, ComponentsPanelRowKey) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    app.ui_state.theme_id = theme;
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
        crate::preferences::PrefMsg::DraftTheme(theme),
    )));
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let sample = tmp.path().join("sample");
    copy_dir(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/sample-project"),
        &sample,
    );
    let project = sample.join("linear-regulator.snxprj");
    for path in [project.clone(), project.with_extension("snxsch")] {
        send(
            &mut app,
            Message::File(crate::app::FileMsg::Opened(Some(path))),
        );
    }
    assert!(!app.document_state.tabs.is_empty(), "the sheet is open");
    let stock = stock_root();
    app.library.pending_mounts.clear();
    app.library.stock_library_root = Some(stock.clone());
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount the stock library");
    let dock = &mut app.document_state.dock;
    dock.close_panel(PanelKind::Components);
    dock.show_panel_at(side, PanelKind::Components);
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
    send(
        &mut app,
        Message::Library(LibraryMessage::ComponentsPanelRowPressed(key.clone())),
    );
    // The press armed a drag-to-place; its release ends it.
    app.library.components_panel.drag = None;
    let details = &app.library.components_panel.details;
    let loaded = details.loaded.as_deref().expect("details loaded");
    assert!(loaded.symbol.is_some(), "the stock row has a symbol");
    assert!(loaded.footprint.is_some(), "the stock row has a footprint");
    (app, tmp, key)
}

/// Open `key` in the component editor tab, as a double-click does.
fn open_editor(app: &mut Signex, key: &ComponentsPanelRowKey) {
    send(
        app,
        Message::Library(LibraryMessage::OpenComponentRow {
            library_path: key.library_path.clone(),
            table: key.table.clone(),
            row_id: key.row_id,
        }),
    );
}

/// The window strip a dock side occupies.
fn region(app: &Signex, side: PanelPosition) -> Rectangle {
    let ui = &app.ui_state;
    match side {
        PanelPosition::Left => {
            Rectangle::new(Point::ORIGIN, Size::new(ui.left_width, WINDOW.height))
        }
        PanelPosition::Right => Rectangle::new(
            Point::new(WINDOW.width - ui.right_width, 0.0),
            Size::new(ui.right_width, WINDOW.height),
        ),
        PanelPosition::Bottom => Rectangle::with_size(WINDOW),
    }
}

/// The last text reading `title` (any case: Graphite sets section labels in
/// capitals) whose left edge lies in `area`.
fn label_in(app: &Signex, title: &str, area: Rectangle) -> Rectangle {
    let window = app.ui_state.main_window_id.expect("main window");
    let texts = crate::layout_probe::lay_out_texts(app.view(window), WINDOW);
    texts
        .iter()
        .filter(|t| {
            t.text.eq_ignore_ascii_case(title)
                && t.bounds.x >= area.x
                && t.bounds.x < area.x + area.width
        })
        .max_by(|a, b| a.bounds.y.total_cmp(&b.bounds.y))
        .unwrap_or_else(|| panic!("no {title:?} text in {area:?}"))
        .bounds
}

fn drive(app: &mut Signex, events: &[(Event, Point)]) -> Vec<mouse::Interaction> {
    drive_with(app, events, false).0
}

/// Feed `events` one per frame, the way the runtime does: the view is rebuilt
/// from the app after each one and every published message goes through
/// `update`. Returns the mouse interaction after each event and, with
/// `snap`, the window's pixels after each event.
fn drive_with(
    app: &mut Signex,
    events: &[(Event, Point)],
    snap: bool,
) -> (Vec<mouse::Interaction>, Vec<Vec<u8>>) {
    use iced::advanced::renderer::Headless as _;
    let window = app.ui_state.main_window_id.expect("main window");
    let mut renderer = crate::layout_probe::headless_renderer();
    let mut cache = Cache::default();
    let mut out = Vec::new();
    let mut frames = Vec::new();
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
        for message in published {
            send(app, message);
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
        out.push(match state {
            State::Updated {
                mouse_interaction, ..
            } => mouse_interaction,
            State::Outdated => mouse::Interaction::None,
        });
        if snap {
            let theme = app.theme(window).expect("theme");
            ui.draw(
                &mut renderer,
                &theme,
                &iced::advanced::renderer::Style {
                    text_color: iced::Color::WHITE,
                },
                cursor,
            );
            let iced::Renderer::Secondary(software) = &mut renderer else {
                panic!("the probe renderer is the software one");
            };
            frames.push(software.screenshot(
                Size::new(WINDOW.width as u32, WINDOW.height as u32),
                1.0,
                iced::Color::BLACK,
            ));
        }
        cache = ui.into_cache();
    }
    (out, frames)
}

/// Pixels of `frame` that differ from `before` inside `area`.
fn changed_pixels(before: &[u8], frame: &[u8], area: Rectangle) -> usize {
    let width = WINDOW.width as usize;
    let rows = (area.y.max(0.0) as usize)..((area.y + area.height).min(WINDOW.height) as usize);
    let cols = (area.x.max(0.0) as usize)..((area.x + area.width).min(WINDOW.width) as usize);
    rows.flat_map(|y| cols.clone().map(move |x| (y * width + x) * 4))
        .filter(|&i| before[i..i + 3] != frame[i..i + 3])
        .count()
}

fn moved(to: Point) -> (Event, Point) {
    (Event::Mouse(mouse::Event::CursorMoved { position: to }), to)
}

/// A point well inside the preview under `label`: the first point below
/// the label that offers a grab, plus a margin. Panics with the hover scan
/// when nothing below the label offers one.
fn preview_point(app: &mut Signex, label: Rectangle) -> Point {
    let x = label.x + 40.0;
    let points: Vec<Point> = (0..60)
        .map(|i| Point::new(x, label.y + label.height + i as f32 * 6.0))
        .collect();
    let events: Vec<(Event, Point)> = points.iter().copied().map(moved).collect();
    let seen = drive(app, &events);
    let first = seen
        .iter()
        .position(|i| *i == mouse::Interaction::Grab)
        .unwrap_or_else(|| {
            let scan: Vec<(i32, mouse::Interaction)> =
                points.iter().map(|p| p.y as i32).zip(seen).collect();
            panic!("no preview offers a grab below {label:?}: {scan:?}")
        });
    Point::new(x, points[first].y + 30.0)
}

/// Hover, press, two moves, release at `from`: the hover grabs, the drag
/// holds the grab, the release lets go, and the drawn preview has moved.
fn assert_drag_pans(app: &mut Signex, from: Point, what: &str) {
    // Off any grid multiple, so the moved grid cannot land on itself.
    let to = Point::new(from.x + 25.0, from.y + 15.0);
    let further = Point::new(from.x + 37.0, from.y + 23.0);
    let (seen, frames) = drive_with(
        app,
        &[
            moved(from),
            (
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                from,
            ),
            moved(to),
            moved(further),
            (
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                further,
            ),
        ],
        true,
    );
    use mouse::Interaction::{Grab, Grabbing};
    assert_eq!(
        seen,
        vec![Grab, Grabbing, Grabbing, Grabbing, Grab],
        "{what} at {from:?}"
    );
    // Inside the preview: from the first grab row down, beside the press.
    let area = Rectangle::new(
        Point::new(from.x - 35.0, from.y - 30.0),
        Size::new(200.0, 110.0),
    );
    let moved_px = changed_pixels(&frames[0], &frames[3], area);
    assert!(
        moved_px > 200,
        "{what}: the drawn preview did not move ({moved_px} pixels changed in {area:?})"
    );
}

fn panel_previews_pan(side: PanelPosition) {
    for theme in THEMES {
        for title in ["Symbol", "Footprint"] {
            let (mut app, _tmp, _) = app_with_details(theme, side);
            let label = label_in(&app, title, region(&app, side));
            let at = preview_point(&mut app, label);
            assert_drag_pans(&mut app, at, &format!("{theme:?} {side:?} {title}"));
        }
    }
}

#[test]
fn the_panel_previews_pan_in_the_left_dock_on_every_theme() {
    panel_previews_pan(PanelPosition::Left);
}

#[test]
fn the_panel_previews_pan_in_the_right_dock_on_every_theme() {
    panel_previews_pan(PanelPosition::Right);
}

/// The maintainer's layout: a component editor tab in front and the
/// Components panel in the right dock. Both the panel previews and the
/// editor's General tab previews pan.
#[test]
fn the_previews_pan_with_a_component_editor_tab_in_front() {
    for theme in THEMES {
        for title in ["Symbol", "Footprint"] {
            let (mut app, _tmp, key) = app_with_details(theme, PanelPosition::Right);
            open_editor(&mut app, &key);
            let right = region(&app, PanelPosition::Right);
            let label = label_in(&app, title, right);
            let at = preview_point(&mut app, label);
            assert_drag_pans(&mut app, at, &format!("{theme:?} panel {title}"));

            let centre = Rectangle::new(
                Point::new(app.ui_state.left_width, 0.0),
                Size::new(right.x - app.ui_state.left_width, WINDOW.height),
            );
            let label = label_in(&app, title, centre);
            let at = preview_point(&mut app, label);
            assert_drag_pans(&mut app, at, &format!("{theme:?} editor {title}"));
        }
    }
}
