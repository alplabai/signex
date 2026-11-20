//! The Start screen's navigation, Home and their layout at small window
//! sizes, and the fixture the page tests (`page_tests.rs`) share.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use iced::Widget as _;
use signex_widgets::{raw, text};

use super::*;
use crate::layout_probe::{find, lay_out_texts};
use crate::project_thumbnail::ProjectThumbnails;

/// Base text size every other measure on this screen is derived from.
pub(super) const BASE: f32 = crate::styles::TypeScale::DEFAULT.body;
/// A small but plausible window; the centre region is narrower still.
/// The navigation is the icon strip at this width (#2346).
const SMALL_WINDOW: iced::Size = iced::Size::new(640.0, 360.0);
/// The narrowest start screen that keeps the full navigation (#2346),
/// as short as `SMALL_WINDOW`.
const NARROWEST_FULL_NAV: iced::Size =
    iced::Size::new(graphite::NAV_W + graphite::MAIN_MIN_W, SMALL_WINDOW.height);
/// A window as wide as the mock (1440 x 900).
pub(super) const MOCK_WINDOW: iced::Size = iced::Size::new(1440.0, 900.0);
/// #331: the app sets no `min_size` on the main window, so the screen is
/// verified at 800 x 500 (floor) and 640 x 360 (stress).
const MIN_SENSIBLE_WINDOW: iced::Size = iced::Size::new(800.0, 500.0);

/// Everything the Start screen reads, owned by the test.
pub(super) struct Fixture {
    pub recent: RecentProjects,
    pub pinned: PinnedProjects,
    pub libraries: Vec<LoadedLibrary>,
    thumbnails: ProjectThumbnails,
    pub state: StartScreenState,
    open_shortcut: String,
    shortcuts_shortcut: String,
    tokens: ThemeTokens,
    i18n: Arc<I18n>,
}

impl Fixture {
    pub(super) fn new(recent: &[PathBuf]) -> Self {
        Self {
            recent: RecentProjects::from_paths(recent.iter().cloned()),
            pinned: PinnedProjects::default(),
            libraries: Vec::new(),
            thumbnails: ProjectThumbnails::default(),
            state: StartScreenState::default(),
            open_shortcut: "Ctrl+O".to_owned(),
            shortcuts_shortcut: "F1".to_owned(),
            tokens: signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark),
            i18n: I18n::en_us(),
        }
    }

    pub(super) fn on(mut self, page: StartPage) -> Self {
        self.state.page = page;
        self
    }

    pub(super) fn view(&self) -> Element<'_, Message> {
        view(
            &self.tokens,
            &self.i18n,
            StartScreenData {
                open_shortcut: self.open_shortcut.clone(),
                shortcuts_shortcut: self.shortcuts_shortcut.clone(),
                recent: &self.recent,
                pinned: &self.pinned,
                libraries: self.libraries.clone(),
                thumbnails: &self.thumbnails,
                state: &self.state,
                now: Utc::now(),
            },
        )
    }

    pub(super) fn texts(&self, window: iced::Size) -> Vec<crate::layout_probe::LaidOutText> {
        lay_out_texts(self.view(), window)
    }

    pub(super) fn click(&self, window: iced::Size, label: &str) -> Vec<Message> {
        crate::layout_probe::messages_on_click(self.view(), window, label)
    }

    /// The catalog text of `id` with `args` as a laid-out text reads it,
    /// Fluent's isolation marks included.
    pub(super) fn says(
        &self,
        id: signex_i18n::MessageId,
        args: &[(&str, signex_i18n::Arg)],
    ) -> String {
        self.i18n.message(id, args).into_string()
    }
}

pub(super) fn paths(n: usize) -> Vec<PathBuf> {
    (0..n)
        .map(|i| PathBuf::from(format!("proj{i}.snxprj")))
        .collect()
}

/// `text` with Fluent's isolation marks removed.
pub(super) fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

/// Whether `messages` holds the Start screen event `msg`.
pub(super) fn sent(messages: &[Message], msg: &StartScreenMsg) -> bool {
    messages
        .iter()
        .any(|m| matches!(m, Message::Project(crate::app::ProjectMsg::StartScreen(s)) if s == msg))
}

#[test]
fn the_list_is_capped_and_keeps_order() {
    let all = paths(MAX_START_RECENT + 4);
    assert_eq!(visible_recent(&all), &all[..MAX_START_RECENT]);
    assert_eq!(visible_recent(&all[..2]), &all[..2]);
    assert!(visible_recent(&[]).is_empty());
}

/// #2305: a pinned project leaves the Recent list, which still fills up
/// to its cap from the older entries; the total counts the capped-off ones.
#[test]
fn a_pinned_project_is_listed_only_under_pinned() {
    let all = paths(MAX_START_RECENT + 2);
    let mut pinned = PinnedProjects::default();
    pinned.pin(&all[3]);
    pinned.pin(&all[0]);
    let sections = project_sections(&all, &pinned);
    assert_eq!(sections.pinned, [all[3].clone(), all[0].clone()]);
    let expected: Vec<&PathBuf> = all
        .iter()
        .filter(|p| **p != all[0] && **p != all[3])
        .take(MAX_START_RECENT)
        .collect();
    assert_eq!(sections.recent, expected);
    assert_eq!(sections.recent_total, MAX_START_RECENT);
    assert!(!sections.has_more_recent());

    let no_pins = PinnedProjects::default();
    let none = project_sections(&all, &no_pins);
    assert!(none.pinned.is_empty());
    assert_eq!(none.recent.len(), MAX_START_RECENT);
    assert_eq!(none.recent_total, MAX_START_RECENT + 2);
    assert!(none.has_more_recent());
    assert!(none.show_recent);
    assert!(sections.show_recent);
    // No recent projects at all: Recent shows, with its empty card.
    assert!(project_sections(&[], &pinned).show_recent);
}

/// #2305: when every recent project is pinned, Recent Projects is
/// hidden instead of claiming there are none.
#[test]
fn recent_is_hidden_when_every_recent_project_is_pinned() {
    let recent = paths(2);
    let mut fx = Fixture::new(&recent);
    for path in &recent {
        fx.pinned.pin(path);
    }
    let sections = project_sections(fx.recent.entries(), &fx.pinned);
    assert!(sections.recent.is_empty());
    assert!(!sections.show_recent);

    let texts = fx.texts(MOCK_WINDOW);
    assert!(texts.iter().any(|t| t.text == "PINNED PROJECTS"));
    assert!(texts.iter().all(|t| t.text != "RECENT PROJECTS"));
    assert!(texts.iter().all(|t| t.text != "No recent projects"));

    let empty = Fixture::new(&[]).texts(MOCK_WINDOW);
    assert!(empty.iter().any(|t| t.text == "RECENT PROJECTS"));
    assert!(empty.iter().any(|t| t.text == "No recent projects"));
}

/// #2305: Pinned Projects sits above Recent Projects, only when a
/// project is pinned; a pinned project missing on disk says so beside its
/// name (m1) and offers Remove in its own column.
#[test]
fn the_pinned_section_shows_above_recent_only_when_something_is_pinned() {
    let recent = paths(3);
    let texts = Fixture::new(&recent).texts(MOCK_WINDOW);
    assert!(texts.iter().all(|t| t.text != "PINNED PROJECTS"));

    let mut fx = Fixture::new(&recent);
    fx.pinned.pin(&recent[1]);
    fx.pinned.pin(&PathBuf::from("gone.snxprj"));
    fx.pinned.set_missing(&PathBuf::from("gone.snxprj"), true);
    let texts = fx.texts(MOCK_WINDOW);
    let top = |label: &str| find(&texts, label).bounds.y;
    assert!(top("PINNED PROJECTS") < top("proj1.snxprj"));
    assert!(top("proj1.snxprj") < top("gone.snxprj"));
    assert!(top("gone.snxprj") < top("RECENT PROJECTS"));
    assert!(top("RECENT PROJECTS") < top("proj0.snxprj"));
    // proj1 is pinned, so Recent does not list it again.
    assert!(
        texts
            .iter()
            .filter(|t| t.text == "proj1.snxprj")
            .all(|t| t.bounds.y < top("RECENT PROJECTS"))
    );
    // The mark sits beside the name, above the path line, in a wide row.
    let gone_lines: Vec<_> = texts.iter().filter(|t| t.text == "gone.snxprj").collect();
    assert_eq!(gone_lines.len(), 2, "name and path: {gone_lines:#?}");
    let (name_line, path_line) = (gone_lines[0].bounds, gone_lines[1].bounds);
    let mark = find(&texts, "Missing on disk").bounds;
    assert!(
        mark.x + 0.5 >= name_line.x + name_line.width,
        "mark {mark:?} is not right of the name {name_line:?}"
    );
    assert!(
        mark.y + mark.height <= path_line.y + 0.5,
        "mark {mark:?} is not above the path {path_line:?}"
    );
    // Remove has its own column: the path never runs under it.
    let remove_label = find(&texts, "Remove").bounds;
    assert!(
        path_line.x + path_line.width <= remove_label.x,
        "path {path_line:?} runs under Remove {remove_label:?}"
    );
    assert!(remove_label.y < path_line.y + path_line.height + BASE);
    assert!(remove_label.y + remove_label.height > name_line.y);
    let remove = fx.click(MOCK_WINDOW, "Remove");
    assert!(
        sent(
            &remove,
            &StartScreenMsg::ProjectUnpinned(PathBuf::from("gone.snxprj"))
        ),
        "{remove:?}"
    );
}

#[test]
fn recent_section_is_absent_without_entries_and_present_with_them() {
    let empty = Fixture::new(&[]).texts(SMALL_WINDOW);
    assert!(empty.iter().all(|t| !t.text.starts_with("proj")));

    let texts = Fixture::new(&paths(2)).texts(SMALL_WINDOW);
    // A row names its project and shows its path.
    let mut order: Vec<_> = texts
        .iter()
        .filter(|t| t.text.starts_with("proj"))
        .map(|t| t.text.as_str())
        .collect();
    order.dedup();
    assert_eq!(order, ["proj0.snxprj", "proj1.snxprj"]);
    assert!(texts.iter().any(|t| t.text == "RECENT PROJECTS"));
}

/// The keycap is drawn by the full navigation, so it is checked at the
/// narrowest window that keeps it (#2346); at `SMALL_WINDOW` the strip
/// draws no keycap and still no blank text. The same holds for the
/// Keyboard Shortcuts key.
#[test]
fn an_unbound_open_shortcut_leaves_no_empty_keycap() {
    let mut fx = Fixture::new(&[]);
    fx.open_shortcut = "F2".to_owned();
    let bound = fx.texts(NARROWEST_FULL_NAV);
    assert!(bound.iter().any(|t| t.text == "F2"));
    fx.open_shortcut.clear();
    fx.shortcuts_shortcut.clear();
    for window in [NARROWEST_FULL_NAV, SMALL_WINDOW] {
        let unbound = fx.texts(window);
        if window == NARROWEST_FULL_NAV {
            assert!(unbound.iter().any(|t| t.text == "Open\u{2026}"));
        }
        assert!(
            unbound.iter().all(|t| !plain(&t.text).trim().is_empty()),
            "an unbound shortcut leaves a blank keycap at {window:?}: {unbound:#?}"
        );
    }
}

/// Font size the Graphite screen draws `label` at.
fn label_size(label: &str) -> f32 {
    let ts = crate::styles::TypeScale::DEFAULT;
    if label == "New project" {
        ts.dense
    } else {
        ts.label
    }
}

/// The navigation labels at the narrowest window that keeps the full
/// navigation, the project list also at `SMALL_WINDOW`, where the
/// navigation is an icon strip (#2346).
#[test]
fn labels_stay_on_one_line_and_inside_a_small_window() {
    let fx = Fixture::new(&paths(MAX_START_RECENT + 2));
    let checks: [(iced::Size, &[&str]); 2] = [
        (
            NARROWEST_FULL_NAV,
            &["New project", "Keyboard Shortcuts", "proj0.snxprj"],
        ),
        (SMALL_WINDOW, &["proj0.snxprj"]),
    ];
    for (window, labels) in checks {
        assert_labels_fit(&fx, window, labels);
    }
}

fn assert_labels_fit(fx: &Fixture, window: iced::Size, labels: &[&str]) {
    let texts = fx.texts(window);
    let one_line = BASE * 2.0;
    for &label in labels {
        let bounds = find(&texts, label).bounds;
        // The primary button's label fills the button's height, so its
        // box is the button's, not a line's; wrapping shows as a
        // narrower-than-natural box, which the width check below catches.
        // Project names are drawn in Inter Medium (mockup row 1): measure them
        // in the font they are drawn in.
        let mut probe = text(raw(label.to_string())).size(label_size(label));
        if label.ends_with(".snxprj") {
            probe = probe.font(super::kit::weighted(iced::font::Weight::Medium));
        }
        let natural = crate::layout_probe::natural::<()>(probe.boxed(), label);
        if label != "New project" {
            assert!(
                bounds.height < one_line,
                "{label:?} wrapped at {window:?}: {bounds:?}"
            );
        }
        assert!(
            bounds.width + 0.5 >= natural.width,
            "{label:?} is narrower than its text at {window:?}: {bounds:?}, natural {natural:?}"
        );
        assert!(
            bounds.x >= 0.0 && bounds.x + bounds.width <= window.width,
            "{label:?} overflows {window:?}: {bounds:?}"
        );
    }
}

/// #2262: the hero title is semibold at the hero size, the line under it
/// one step up the type scale.
#[test]
fn hero_lines_use_the_mock_sizes() {
    let fx = Fixture::new(&[]);
    let texts = fx.texts(MOCK_WINDOW);
    let title = "What are you building?";
    let laid = find(&texts, title).bounds;
    let natural = crate::layout_probe::natural::<()>(
        text(raw(title.to_string()))
            .size(graphite::HERO_TITLE_SIZE)
            .font(graphite::semibold())
            .boxed(),
        title,
    );
    assert!(
        (laid.height - natural.height).abs() < 0.5,
        "hero title {laid:?}, one semibold line is {natural:?}"
    );
    let body = fx.says(signex_i18n::ids::START_HERO_BODY, &[]);
    let laid = find(&texts, &body).bounds;
    let one_line = crate::layout_probe::natural::<()>(
        text(raw(body.clone()))
            .size(graphite::HERO_BODY_SIZE)
            .boxed(),
        &body,
    );
    assert!(
        laid.height + 0.5 >= one_line.height,
        "hero body is drawn below the heading size: {laid:?}, one line is {one_line:?}"
    );
}

/// #2262, GG-10: Keyboard Shortcuts sits under a divider after the page
/// rows, without "(F1)" in its label and with the bound key at its right,
/// and still opens the shortcut list. The navigation has no Open Sample
/// Project row any more; the reference card on Home opens the sample.
#[test]
fn keyboard_shortcuts_sits_under_the_divider_and_the_sample_opens_from_its_card() {
    let fx = Fixture::new(&[]);
    let texts = fx.texts(MOCK_WINDOW);
    let top = |label: &str| find(&texts, label).bounds.y;
    // Projects, Libraries, Parts search: two row steps.
    let row_step = (top("Parts search") - top("Projects")) / 2.0;
    let gap = top("Keyboard Shortcuts") - top("Parts search");
    assert!(
        gap > row_step + 2.0 * crate::styles::Spacing::DEFAULT.m - 0.5,
        "no divider room between the page rows and Keyboard Shortcuts: gap {gap}, row {row_step}"
    );
    assert!(texts.iter().all(|t| t.text != "Open Sample Project"));
    assert!(texts.iter().all(|t| t.text != "Keyboard Shortcuts (F1)"));
    let key = texts
        .iter()
        .find(|t| plain(&t.text) == "F1")
        .expect("the F1 key at the row's right");
    let label = find(&texts, "Keyboard Shortcuts").bounds;
    assert!((key.bounds.center_y() - label.center_y()).abs() < BASE);
    assert!(key.bounds.x > label.x + label.width);

    let keys = fx.click(MOCK_WINDOW, "Keyboard Shortcuts");
    assert!(
        keys.iter()
            .any(|m| matches!(m, Message::Menu(MenuMessage::OpenKeyboardShortcuts))),
        "{keys:?}"
    );
    for label in ["Sample project", "Open"] {
        let sample = fx.click(MOCK_WINDOW, label);
        assert!(
            sample
                .iter()
                .any(|m| matches!(m, Message::Menu(MenuMessage::OpenSampleProject))),
            "{label}: {sample:?}"
        );
    }
}

/// Every label at each window size: at least its natural width, one
/// line high, horizontally inside the window, with the full recent list
/// present. Vertically the list may exceed a short window only because it
/// sits in a `scrollable`. Where the navigation is the icon strip
/// (#2346) its labels are tooltips, so only the project list is laid out.
#[test]
fn every_button_label_fits_at_small_window_sizes() {
    let ts = crate::styles::TypeScale::DEFAULT;
    let recent = paths(MAX_START_RECENT + 2);
    let fx = Fixture::new(&recent);
    let nav_labels: [(String, f32); 2] = [
        ("New project".into(), ts.dense),
        ("Keyboard Shortcuts".into(), ts.label),
    ];
    let list_labels: Vec<(String, f32)> = visible_recent(&recent)
        .iter()
        .map(|p| (recent_label(p), ts.label))
        .collect();

    for window in [MIN_SENSIBLE_WINDOW, NARROWEST_FULL_NAV, SMALL_WINDOW] {
        let collapsed = graphite::nav_collapsed(window.width);
        let labels: Vec<&(String, f32)> = if collapsed {
            list_labels.iter().collect()
        } else {
            nav_labels.iter().chain(&list_labels).collect()
        };
        let texts = fx.texts(window);
        for (label, size) in &labels {
            let bounds = find(&texts, label).bounds;
            // The project list is drawn in Inter Medium (mockup row 1).
            let mut probe = text(raw(label.clone())).size(*size);
            if list_labels.iter().any(|(listed, _)| listed == label) {
                probe = probe.font(super::kit::weighted(iced::font::Weight::Medium));
            }
            let natural = crate::layout_probe::natural::<()>(probe.boxed(), label);
            // "New project" fills its button's height (see above).
            if label != "New project" {
                assert!(
                    bounds.height <= natural.height + 0.5,
                    "{label:?} wraps at {window:?}: {bounds:?}, one line is {natural:?}"
                );
            }
            assert!(
                bounds.width + 0.5 >= natural.width,
                "{label:?} is narrower than its text at {window:?}: {bounds:?}, natural {natural:?}"
            );
            assert!(
                bounds.x >= 0.0 && bounds.x + bounds.width <= window.width,
                "{label:?} overflows {window:?}: {bounds:?}"
            );
        }
        let mut names: Vec<_> = texts
            .iter()
            .filter(|t| t.text.starts_with("proj"))
            .map(|t| t.text.as_str())
            .collect();
        names.dedup();
        assert_eq!(
            names.len(),
            MAX_START_RECENT,
            "the recent list is complete at {window:?}"
        );
    }
}

/// The navigation's own texts, none of which the main column draws.
const NAV_TEXTS: [&str; 7] = [
    "New project",
    "Open\u{2026}",
    "Home",
    "Projects",
    "Libraries",
    "Parts search",
    "Keyboard Shortcuts",
];

/// #2346: the navigation is full while the area beside it is at least
/// 480 px (40 body heights), an icon strip below that, and the main
/// column takes the room the strip frees.
#[test]
fn the_navigation_collapses_to_an_icon_strip_when_the_main_area_is_narrow() {
    let body = crate::styles::TypeScale::DEFAULT.body;
    assert_eq!(graphite::MAIN_MIN_W, body * 40.0);
    assert_eq!(graphite::NAV_W, body * 19.5);
    assert!(!graphite::nav_collapsed(NARROWEST_FULL_NAV.width));
    assert!(graphite::nav_collapsed(NARROWEST_FULL_NAV.width - 1.0));
    const { assert!(nav_strip::STRIP_W < graphite::NAV_W / 3.0) };

    let fx = Fixture::new(&[]);
    let full = fx.texts(NARROWEST_FULL_NAV);
    for label in NAV_TEXTS {
        assert!(
            full.iter().any(|t| t.text == label),
            "{label:?} missing from the full navigation"
        );
    }
    let strip = fx.texts(SMALL_WINDOW);
    for label in NAV_TEXTS {
        assert!(
            strip.iter().all(|t| t.text != label),
            "{label:?} is drawn in the icon strip; it belongs in the tooltip"
        );
    }
    let hero = "What are you building?";
    let left = find(&strip, hero).bounds.x;
    let expected = nav_strip::STRIP_W + graphite::MAIN_PAD_X;
    assert!(
        (left - expected).abs() < 1.0,
        "the main column starts at {left}, not beside the strip at {expected}"
    );
    // Wider again: the full navigation is back.
    let wide = fx.texts(MOCK_WINDOW);
    assert!(wide.iter().any(|t| t.text == "Parts search"));
}

/// What the strip rows of `fx` send, top to bottom.
fn strip_messages(fx: &Fixture) -> Vec<String> {
    let step = crate::styles::Spacing::DEFAULT.m;
    (0..)
        .map(|i| i as f32 * step)
        .take_while(|y| *y < SMALL_WINDOW.height)
        .flat_map(|y| {
            crate::layout_probe::messages_on_click_at(
                fx.view(),
                SMALL_WINDOW,
                None,
                iced::Point::new(nav_strip::STRIP_W / 2.0, y),
            )
        })
        .map(|m| format!("{m:?}"))
        .collect()
}

/// #2346: every strip row sends the message of its full row; the
/// current page and the planned Parts search send none, and there is no
/// Open Sample Project row (GG-10).
#[test]
fn the_icon_strip_rows_send_their_full_rows_messages() {
    let sent = strip_messages(&Fixture::new(&[]));
    let page = |p| format!("{:?}", message(StartScreenMsg::PageSelected(p)));
    let expected: Vec<String> = [
        Action::NewProject,
        Action::OpenProject,
        Action::KeyboardShortcuts,
    ]
    .iter()
    .map(|a| format!("{:?}", a.message()))
    .chain([page(StartPage::Projects), page(StartPage::Libraries)])
    .collect();
    for message in &expected {
        assert!(
            sent.contains(message),
            "no strip row sends {message}: {sent:?}"
        );
    }
    assert!(
        sent.iter().all(|m| expected.contains(m)),
        "a strip row sends something no full row sends: {sent:?}"
    );
    let sample = format!("{:?}", Action::OpenSampleProject.message());
    assert!(!sent.contains(&sample), "the strip still opens the sample");
    // On the Projects page, Home is pressable and Projects is not.
    let on_projects = strip_messages(&Fixture::new(&[]).on(StartPage::Projects));
    assert!(on_projects.contains(&page(StartPage::Home)));
    assert!(!on_projects.contains(&page(StartPage::Projects)));
}

/// Whether a click at (`x`, `y`) in a `window` start screen opens
/// Keyboard Shortcuts.
fn click_opens_shortcuts(window: iced::Size, x: f32, y: f32) -> bool {
    let fx = Fixture::new(&[]);
    let shortcuts = format!("{:?}", Action::KeyboardShortcuts.message());
    crate::layout_probe::messages_on_click_at(fx.view(), window, None, iced::Point::new(x, y))
        .iter()
        .any(|m| format!("{m:?}") == shortcuts)
}

/// Height of the Keyboard Shortcuts row at `x`, found by clicking
/// every quarter pixel from `from` for `reach` pixels.
fn shortcuts_row_height(window: iced::Size, x: f32, from: f32, reach: f32) -> f32 {
    const STEP: f32 = 0.25;
    let hits = (0..)
        .map(|i| from + i as f32 * STEP)
        .take_while(|y| *y < from + reach)
        .filter(|y| click_opens_shortcuts(window, x, *y))
        .count();
    hits as f32 * STEP
}

/// #2346: a strip row is as tall as a full navigation row.
#[test]
fn a_strip_row_is_as_tall_as_a_full_navigation_row() {
    let row_h = graphite::NAV_ROW_H;
    let full_texts = Fixture::new(&[]).texts(NARROWEST_FULL_NAV);
    let label = find(&full_texts, "Keyboard Shortcuts").bounds;
    let full = shortcuts_row_height(
        NARROWEST_FULL_NAV,
        label.center_x(),
        label.center_y() - row_h,
        2.0 * row_h,
    );
    let x = nav_strip::STRIP_W / 2.0;
    let step = crate::styles::Spacing::DEFAULT.m;
    let inside = (0..)
        .map(|i| i as f32 * step)
        .take_while(|y| *y < SMALL_WINDOW.height)
        .find(|y| click_opens_shortcuts(SMALL_WINDOW, x, *y))
        .expect("no strip row opens Keyboard Shortcuts");
    let strip = shortcuts_row_height(SMALL_WINDOW, x, inside - row_h, 2.0 * row_h);
    assert!(
        (full - row_h).abs() <= 1.0,
        "the full row is {full} tall, not {row_h}"
    );
    assert!(
        (strip - full).abs() <= 1.0,
        "a strip row is {strip} tall, a full row {full}"
    );
}

/// #2346: in a window too short for every strip row the strip scrolls,
/// so the last row is reached by scrolling.
#[test]
fn the_icon_strip_scrolls_in_a_short_window() {
    let short = iced::Size::new(SMALL_WINDOW.width, SMALL_WINDOW.height / 2.0);
    assert!(graphite::nav_collapsed(short.width));
    let x = nav_strip::STRIP_W / 2.0;
    // The last row's place once the strip is scrolled to its end.
    let y = short.height - crate::styles::Spacing::DEFAULT.l - graphite::NAV_ROW_H / 2.0;
    assert!(
        !click_opens_shortcuts(short, x, y),
        "the strip fits a {short:?} window; the test needs it taller"
    );
    let fx = Fixture::new(&[]);
    let shortcuts = format!("{:?}", Action::KeyboardShortcuts.message());
    let scrolled = crate::layout_probe::messages_for_events(
        fx.view(),
        short,
        iced::Point::new(x, y),
        &[
            iced::Event::Mouse(iced::mouse::Event::WheelScrolled {
                delta: iced::mouse::ScrollDelta::Pixels {
                    x: 0.0,
                    y: -SMALL_WINDOW.height,
                },
            }),
            // The scroll moves the content on the next frame.
            iced::Event::Window(iced::window::Event::RedrawRequested(
                std::time::Instant::now() + std::time::Duration::from_secs(2),
            )),
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
            iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Left,
            )),
        ],
    );
    assert!(
        scrolled.iter().any(|m| format!("{m:?}") == shortcuts),
        "scrolled to its end, the strip's last row is not under the cursor: {scrolled:?}"
    );
}
