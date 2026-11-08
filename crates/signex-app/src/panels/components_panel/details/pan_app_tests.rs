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
const THEMES: [ThemeId; 2] = [ThemeId::GraphiteLight, ThemeId::GraphiteDark];

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

/// Dock layouts these tests produce go nowhere: the per-process prefs file
/// they would land in is read back by every other test's `Signex::new`.
struct DiscardLayouts;

impl crate::dock::DockLayoutStore for DiscardLayouts {
    fn write(&self, _layout: &crate::dock::DockLayoutSnapshot) -> Result<(), String> {
        Ok(())
    }
}

fn send(app: &mut Signex, message: Message) {
    TaskDriver::new()
        .send(app, message)
        .expect("the message settled");
}

/// The app in `theme` with the sample sheet open (a bare Start screen has no
/// docks), the stock library mounted, the Components
/// panel showing in the `side` dock and its first stock row selected with
/// its details loaded. Returns the selected row too.
fn app_with_details(
    theme: ThemeId,
    side: PanelPosition,
) -> (Signex, tempfile::TempDir, ComponentsPanelRowKey) {
    let (mut app, _boot) = Signex::new();
    app.document_state.dock_layout_sync = crate::dock::DockLayoutSync::new(
        std::sync::Arc::new(DiscardLayouts),
        crate::dock::DockLayoutSnapshot::capture(&app.document_state.dock),
    );
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
    // The sheet alone: opening the project would record it in Open Recent,
    // which this process's other tests read back from the shared prefs file.
    let recent = app.ui_state.recent_projects.entries().to_vec();
    send(
        &mut app,
        Message::File(crate::app::FileMsg::Opened(Some(
            sample.join("linear-regulator.snxsch"),
        ))),
    );
    assert!(!app.document_state.tabs.is_empty(), "the sheet is open");
    assert_eq!(
        app.ui_state.recent_projects.entries(),
        recent.as_slice(),
        "nothing was added to Open Recent"
    );
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

/// Every text of the live tree with its window bounds: a scrolled pane's
/// texts are moved by its scroll offset.
#[derive(Default)]
struct WindowTexts {
    texts: Vec<(String, Rectangle)>,
    /// Scroll offset of the scrollables around the widget being visited.
    offset: iced::Vector,
    /// Offset of a scrollable just met, applied to its content.
    pending: iced::Vector,
}

impl iced::advanced::widget::Operation for WindowTexts {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation)) {
        let saved = self.offset;
        self.offset += std::mem::take(&mut self.pending);
        operate(self);
        self.offset = saved;
    }

    fn scrollable(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        _bounds: Rectangle,
        _content: Size,
        translation: iced::Vector,
        _state: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        self.pending = translation;
    }

    fn text(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        bounds: Rectangle,
        text: &mut dyn iced::advanced::widget::operation::Text,
    ) {
        self.texts
            .push((text.text().into_owned(), bounds - self.offset));
    }
}

/// The last text reading `title` (any case: Graphite sets section labels in
/// capitals) whose left edge lies in `area`, where the live tree draws it.
fn label_in(live: &mut Live, title: &str, area: Rectangle) -> Rectangle {
    let window = live.app.ui_state.main_window_id.expect("main window");
    let mut renderer = crate::layout_probe::headless_renderer();
    let cache = std::mem::take(&mut live.cache);
    let mut ui = UserInterface::build(live.app.view(window), WINDOW, cache, &mut renderer);
    let mut found = WindowTexts::default();
    ui.operate(&renderer, &mut found);
    live.cache = ui.into_cache();
    found
        .texts
        .iter()
        .filter(|(text, bounds)| {
            text.eq_ignore_ascii_case(title) && bounds.x >= area.x && bounds.x < area.x + area.width
        })
        .max_by(|a, b| a.1.y.total_cmp(&b.1.y))
        .unwrap_or_else(|| {
            let lower = title.to_lowercase();
            let near: Vec<&(String, Rectangle)> = found
                .texts
                .iter()
                .filter(|(text, _)| text.to_lowercase().contains(&lower))
                .collect();
            panic!("no {title:?} text in {area:?}; texts like it: {near:?}")
        })
        .1
}

/// Bounds and scroll offset of every scrollable of the live tree.
fn scrollables(live: &mut Live) -> Vec<(Rectangle, iced::Vector)> {
    #[derive(Default)]
    struct Found(Vec<(Rectangle, iced::Vector)>);
    impl iced::advanced::widget::Operation for Found {
        fn traverse(
            &mut self,
            operate: &mut dyn FnMut(&mut dyn iced::advanced::widget::Operation),
        ) {
            operate(self);
        }

        fn scrollable(
            &mut self,
            _id: Option<&iced::advanced::widget::Id>,
            bounds: Rectangle,
            _content: Size,
            translation: iced::Vector,
            _state: &mut dyn iced::advanced::widget::operation::Scrollable,
        ) {
            self.0.push((bounds, translation));
        }
    }
    let window = live.app.ui_state.main_window_id.expect("main window");
    let mut renderer = crate::layout_probe::headless_renderer();
    let cache = std::mem::take(&mut live.cache);
    let mut ui = UserInterface::build(live.app.view(window), WINDOW, cache, &mut renderer);
    let mut found = Found::default();
    ui.operate(&renderer, &mut found);
    live.cache = ui.into_cache();
    found.0
}

fn drive(app: &mut Live, events: &[(Event, Point)]) -> Vec<mouse::Interaction> {
    drive_with(app, events, false).0
}

/// The app and its widget tree, kept across frames as the runtime keeps
/// them, so the details scroll offset and the canvas cameras persist.
struct Live {
    app: Signex,
    cache: Cache,
    /// Whether the window asked to be redrawn after each event of the last
    /// `drive_with`: without that the runtime shows no change.
    redraws: Vec<bool>,
    _tmp: tempfile::TempDir,
}

fn start(theme: ThemeId, side: PanelPosition) -> (Live, ComponentsPanelRowKey) {
    let (app, tmp, key) = app_with_details(theme, side);
    (
        Live {
            app,
            cache: Cache::default(),
            redraws: Vec::new(),
            _tmp: tmp,
        },
        key,
    )
}

/// Feed `events` one per frame, the way the runtime does: the view is rebuilt
/// from the app after each one and every published message goes through
/// `update`. Returns the mouse interaction after each event and, with
/// `snap`, the window's pixels after each event.
fn drive_with(
    live: &mut Live,
    events: &[(Event, Point)],
    snap: bool,
) -> (Vec<mouse::Interaction>, Vec<Vec<u8>>) {
    use iced::advanced::renderer::Headless as _;
    let window = live.app.ui_state.main_window_id.expect("main window");
    let mut renderer = crate::layout_probe::headless_renderer();
    let mut cache = std::mem::take(&mut live.cache);
    let mut out = Vec::new();
    let mut frames = Vec::new();
    live.redraws.clear();
    for (event, at) in events {
        let cursor = mouse::Cursor::Available(*at);
        let mut published = Bus::new();
        {
            let mut ui = UserInterface::build(live.app.view(window), WINDOW, cache, &mut renderer);
            let (event_state, _) = ui.update(
                &iced::window::Headless,
                &Waker::noop(),
                std::slice::from_ref(event),
                cursor,
                &mut renderer,
                &mut published,
            );
            // An outdated interface is rebuilt and so redrawn.
            live.redraws.push(match event_state {
                State::Updated { redraw_request, .. } => {
                    !matches!(redraw_request, iced::window::RedrawRequest::Wait)
                }
                State::Outdated => true,
            });
            cache = ui.into_cache();
        }
        for message in published {
            send(&mut live.app, message);
        }
        // Read the interaction off a fresh build, as the next frame sees it.
        let mut ui = UserInterface::build(live.app.view(window), WINDOW, cache, &mut renderer);
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
            let theme = live.app.theme(window).expect("theme");
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
    live.cache = cache;
    (out, frames)
}

/// Pixels inside `area` of `frame` that differ from `before` shifted by
/// `shift` px, and the pixel count of `area`. A zero shift counts what
/// changed; the drag's shift counts what did not follow the pointer.
fn mismatched_pixels(
    before: &[u8],
    frame: &[u8],
    area: Rectangle,
    shift: (usize, usize),
) -> (usize, usize) {
    let width = WINDOW.width as usize;
    let (x0, y0) = (area.x as usize, area.y as usize);
    let (x1, y1) = (x0 + area.width as usize, y0 + area.height as usize);
    let at = |x: usize, y: usize| (y * width + x) * 4;
    let mut differ = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let (i, j) = (at(x, y), at(x - shift.0, y - shift.1));
            if frame[i..i + 3] != before[j..j + 3] {
                differ += 1;
            }
        }
    }
    (differ, (x1 - x0) * (y1 - y0))
}

fn moved(to: Point) -> (Event, Point) {
    (Event::Mouse(mouse::Event::CursorMoved { position: to }), to)
}

/// A point well inside the preview under `label`: the first point below
/// the label that offers a grab, plus a margin. Panics with the hover scan
/// when nothing below the label offers one.
fn preview_point(app: &mut Live, label: Rectangle) -> Point {
    // Far enough in that the drag's source pixels stay inside the canvas.
    let x = label.x + 80.0;
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
    Point::new(x, points[first].y + 40.0)
}

/// Hover, press, two moves, release at `from`: the hover grabs, the drag
/// holds the grab, the release lets go, and the drawn preview has moved.
fn assert_drag_pans(app: &mut Live, from: Point, what: &str) {
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
    assert!(
        app.redraws[2] && app.redraws[3],
        "{what}: a drag move asks for a redraw ({:?})",
        app.redraws
    );
    // Inside the preview: from the first grab row down, beside the press.
    // The drawing has moved, and moved exactly with the pointer: the frame
    // after the drag is the frame before it shifted by the drag.
    let area = Rectangle::new(
        Point::new(from.x + 2.0, from.y - 7.0),
        Size::new(100.0, 70.0),
    );
    let (changed, total) = mismatched_pixels(&frames[0], &frames[3], area, (0, 0));
    assert!(
        changed > 20,
        "{what}: the drawn preview did not move ({changed} of {total} pixels changed in {area:?})"
    );
    let (off, total) = mismatched_pixels(&frames[0], &frames[3], area, (37, 23));
    assert!(
        off <= total / 50,
        "{what}: the drawing did not follow the pointer 1:1 ({off} of {total} pixels off in {area:?})"
    );
}

fn panel_previews_pan(side: PanelPosition) {
    for theme in THEMES {
        for title in ["Symbol", "Footprint"] {
            let (mut live, _) = start(theme, side);
            let area = region(&live.app, side);
            let label = label_in(&mut live, title, area);
            let at = preview_point(&mut live, label);
            assert_drag_pans(&mut live, at, &format!("{theme:?} {side:?} {title}"));
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
            let (mut live, key) = start(theme, PanelPosition::Right);
            open_editor(&mut live.app, &key);
            let right = region(&live.app, PanelPosition::Right);
            let label = label_in(&mut live, title, right);
            let at = preview_point(&mut live, label);
            assert_drag_pans(&mut live, at, &format!("{theme:?} panel {title}"));

            // Everything left of the right dock: an emptied left dock hides,
            // and the editor then starts at the window's edge.
            let centre = Rectangle::new(Point::ORIGIN, Size::new(right.x, WINDOW.height));
            let label = label_in(&mut live, title, centre);
            let at = preview_point(&mut live, label);
            assert_drag_pans(&mut live, at, &format!("{theme:?} editor {title}"));
        }
    }
}

/// The details pane scrolled down (a short pane: the list takes most of the
/// dock) still pans its previews 1:1 under the pointer.
#[test]
fn the_previews_pan_one_to_one_in_a_scrolled_details_pane() {
    for theme in THEMES {
        for title in ["Symbol", "Footprint"] {
            let (mut live, _) = start(theme, PanelPosition::Right);
            let right = region(&live.app, PanelPosition::Right);
            // The details pane is the lowest scrollable of the dock; give the
            // list all but 220 px of its height so the details must scroll.
            let details = |live: &mut Live| {
                scrollables(live)
                    .into_iter()
                    // Not the dock's thin horizontal tab strip.
                    .filter(|(b, _)| b.x >= right.x && b.height > 100.0)
                    .max_by(|a, b| a.0.y.total_cmp(&b.0.y))
                    .expect("the details pane")
            };
            let (pane, _) = details(&mut live);
            live.app.library.components_panel.list_height += pane.height - 220.0;
            let (pane, _) = details(&mut live);
            let over = Point::new(pane.x + 6.0, pane.y + pane.height / 2.0);
            // Bring the section label under test to the top of the short pane,
            // so its preview lies inside it.
            let label = label_in(&mut live, title, right);
            let by = (label.y - pane.y - 4.0).max(60.0);
            let wheel = Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -by },
            });
            // Frames long after the wheel settle the smooth scroll.
            let later = |ms: u64| {
                let at = std::time::Instant::now() + std::time::Duration::from_millis(ms);
                (
                    Event::Window(iced::window::Event::RedrawRequested(at)),
                    over,
                )
            };
            drive(
                &mut live,
                &[
                    moved(over),
                    (wheel, over),
                    later(0),
                    later(2_000),
                    later(4_000),
                ],
            );
            let (_, scroll) = details(&mut live);
            if scroll.y <= 40.0 {
                let all = scrollables(&mut live);
                panic!(
                    "{theme:?}: the details pane scrolled by {scroll:?} in {pane:?} (wheel at {over:?}); scrollables: {all:?}"
                );
            }
            let label = label_in(&mut live, title, right);
            let at = preview_point(&mut live, label);
            assert!(
                at.y + 70.0 < pane.y + pane.height,
                "{theme:?} {title}: the drag at {at:?} stays inside the pane {pane:?}"
            );
            assert_drag_pans(&mut live, at, &format!("{theme:?} scrolled {title}"));
        }
    }
}

/// Software-rendered PNGs of the maintainer's layout (component editor in
/// front, Components panel in the right dock) for a visual check. Set
/// `SIGNEX_PANEL_SHOTS` to the output directory and run with `--ignored`.
#[test]
#[ignore = "writes PNGs into $SIGNEX_PANEL_SHOTS"]
fn render_components_panel_shots() {
    let Ok(dir) = std::env::var("SIGNEX_PANEL_SHOTS") else {
        return;
    };
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("out dir");
    for theme in THEMES {
        let (mut live, key) = start(theme, PanelPosition::Right);
        open_editor(&mut live.app, &key);
        let at = Point::new(WINDOW.width / 2.0, 10.0);
        let save = |live: &mut Live, name: &str| {
            let (_, frames) = drive_with(live, &[moved(at)], true);
            image::save_buffer(
                dir.join(format!("{name}-{theme:?}.png")),
                &frames[0],
                WINDOW.width as u32,
                WINDOW.height as u32,
                image::ColorType::Rgba8,
            )
            .expect("write png");
        };
        save(&mut live, "components");
        // Stock only, so its rows and the selection show.
        let panel = &mut live.app.library.components_panel;
        panel.source_filter = Some(crate::library::state::ComponentsMountSource::Stock);
        save(&mut live, "components-rows");
    }
}
