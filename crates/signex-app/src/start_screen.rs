//! Start screen — what the centre region shows when no document is on
//! screen. Two distinct states (#331):
//!
//! * nothing loaded at all → [`view`], an actionable start screen
//!   (New Project, Open Project…, Open Sample Project, a short recent-
//!   projects list, Keyboard Shortcuts);
//! * a project is loaded but no document is picked → [`hint_view`], a
//!   quieter pointer at the project tree.
//!
//! Every button dispatches an existing message: the actions reuse the
//! menu messages, and a recent entry reuses File ▸ Open Recent's
//! `OpenRecentProject`, so a vanished project is dropped and reported by
//! the same handler. Pinning, the Pinned Projects rows and the row menu send
//! [`StartScreenMsg`] (#2305).

use std::path::{Path, PathBuf};

use iced::widget::{column, container};
use iced::{Element, Length, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use crate::app::Message;
use crate::menu_bar::MenuMessage;
use crate::pinned_projects::PinnedProjects;
use crate::styles;

mod graphite;
mod nav_strip;
mod projects;

/// Base text size every other measure on this screen is derived from.
const BASE: f32 = crate::styles::TypeScale::DEFAULT.body;

/// How many recent projects the start screen lists (File ▸ Open Recent
/// keeps more).
pub const MAX_START_RECENT: usize = 5;

/// Which centre-region placeholder applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Nothing loaded: show the start screen.
    Start,
    /// Project loaded, no document selected: show the quiet hint.
    ProjectHint,
}

impl Surface {
    pub fn select(has_active_project: bool) -> Self {
        if has_active_project {
            Self::ProjectHint
        } else {
            Self::Start
        }
    }
}

/// One clickable entry on the start screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewProject,
    OpenProject,
    OpenSampleProject,
    KeyboardShortcuts,
}

impl Action {
    pub const ALL: [Action; 4] = [
        Action::NewProject,
        Action::OpenProject,
        Action::OpenSampleProject,
        Action::KeyboardShortcuts,
    ];

    /// The existing, working message this action dispatches.
    pub fn message(self) -> Message {
        Message::Menu(match self {
            Action::NewProject => MenuMessage::NewProject,
            Action::OpenProject => MenuMessage::OpenProject,
            Action::OpenSampleProject => MenuMessage::OpenSampleProject,
            Action::KeyboardShortcuts => MenuMessage::OpenKeyboardShortcuts,
        })
    }
}

/// The recent projects the start screen shows: newest first, capped.
pub fn visible_recent(recent: &[PathBuf]) -> &[PathBuf] {
    &recent[..recent.len().min(MAX_START_RECENT)]
}

/// The two project lists the Start screen shows (#2305).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSections<'a> {
    /// Pinned Projects, in the user's order; the section is hidden when
    /// this is empty.
    pub pinned: &'a [PathBuf],
    /// Recent Projects without the pinned ones, newest first, capped.
    pub recent: Vec<&'a PathBuf>,
    /// Whether Recent Projects shows: hidden when every recent project is
    /// pinned, shown (with its empty card) when there are none at all.
    pub show_recent: bool,
}

/// Split `recent` and `pinned` into the Start screen's sections: a pinned
/// project is listed under Pinned Projects only.
pub fn project_sections<'a>(
    recent: &'a [PathBuf],
    pinned: &'a PinnedProjects,
) -> ProjectSections<'a> {
    let unpinned: Vec<&PathBuf> = recent
        .iter()
        .filter(|path| !pinned.contains(path))
        .take(MAX_START_RECENT)
        .collect();
    ProjectSections {
        pinned: pinned.entries(),
        show_recent: recent.is_empty() || !unpinned.is_empty(),
        recent: unpinned,
    }
}

/// The existing File ▸ Open Recent message for one entry.
pub fn recent_message(path: &Path) -> Message {
    Message::Menu(MenuMessage::OpenRecentProject(path.to_path_buf()))
}

/// Wrap a Start screen event in its namespace.
pub fn message(msg: StartScreenMsg) -> Message {
    Message::Project(crate::app::ProjectMsg::StartScreen(msg))
}

/// Start screen project-row events (#2305). Routed to
/// `Signex::dispatch_start_screen_message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartScreenMsg {
    /// Pin was pressed on a Recent Projects row (hover button or menu).
    ProjectPinned(PathBuf),
    /// Open was picked in a Recent Projects row's menu: the menu closes and
    /// the project opens as File ▸ Open Recent opens it.
    RecentProjectClicked(PathBuf),
    /// Unpin, or Remove on a pinned project missing on disk, was pressed.
    ProjectUnpinned(PathBuf),
    /// A Pinned Projects row (or its menu's Open) was clicked.
    PinnedProjectClicked(PathBuf),
    /// A project row was right-clicked: open its menu at the pointer.
    RowRightClicked(PathBuf),
    /// The row menu was dismissed without a pick.
    RowMenuDismissed,
    /// The start-up check of the pins finished off the UI thread: the pins
    /// whose file is not on disk, or why the check could not run.
    PinsChecked(Result<Vec<PathBuf>, String>),
}

/// The open right-click menu of a Start screen project row.
#[derive(Debug, Clone, PartialEq)]
pub struct StartRowMenu {
    pub path: PathBuf,
    pub x: f32,
    pub y: f32,
}

fn recent_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// A loaded library for the Graphite start screen: file name and how many
/// components it holds.
pub type LoadedLibrary = (String, usize);

pub fn view<'a>(
    tokens: &'a ThemeTokens,
    i18n: &'a signex_i18n::I18n,
    open_shortcut: &str,
    recent: &'a [PathBuf],
    pinned: &'a PinnedProjects,
    libraries: &[LoadedLibrary],
) -> Element<'a, Message> {
    graphite::view(tokens, i18n, open_shortcut, recent, pinned, libraries)
}

pub fn hint_view<'a>(tokens: &'a ThemeTokens) -> Element<'a, Message> {
    let text_secondary = styles::ti(tokens.text_secondary);
    let content = column![
        text(raw("No document selected"))
            .size(BASE * 1.15)
            .color(text_secondary),
        text(raw("Choose a schematic or PCB from the project tree"))
            .size(BASE * 0.9)
            .color(text_secondary),
    ]
    .spacing(BASE * 0.65)
    .align_x(iced::Alignment::Center);
    region(tokens, content.boxed())
}

fn region<'a>(tokens: &'a ThemeTokens, content: Element<'a, Message>) -> Element<'a, Message> {
    container(content)
        .center(Length::Fill)
        .style(styles::panel_region(tokens))
        .boxed()
}

/// Where the middle of the collapsed navigation strip is, given the left
/// edge of the laid-out hero title (#2346).
#[cfg(test)]
pub(crate) fn strip_centre_from_hero(hero_left: f32) -> f32 {
    hero_left - graphite::MAIN_PAD_X - nav_strip::STRIP_W / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{find, lay_out_texts};

    /// A small but plausible window; the centre region is narrower still.
    /// The navigation is the icon strip at this width (#2346).
    const SMALL_WINDOW: iced::Size = iced::Size::new(640.0, 360.0);
    /// The narrowest start screen that keeps the full navigation (#2346),
    /// as short as `SMALL_WINDOW`.
    const NARROWEST_FULL_NAV: iced::Size =
        iced::Size::new(graphite::NAV_W + graphite::MAIN_MIN_W, SMALL_WINDOW.height);

    fn paths(n: usize) -> Vec<PathBuf> {
        (0..n)
            .map(|i| PathBuf::from(format!("proj{i}.snxprj")))
            .collect()
    }

    static NO_PINS: PinnedProjects = PinnedProjects::new();

    #[test]
    fn the_list_is_capped_and_keeps_order() {
        let all = paths(MAX_START_RECENT + 4);
        assert_eq!(visible_recent(&all), &all[..MAX_START_RECENT]);
        assert_eq!(visible_recent(&all[..2]), &all[..2]);
        assert!(visible_recent(&[]).is_empty());
    }

    /// #2305: a pinned project leaves the Recent list, which still fills up
    /// to its cap from the older entries.
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

        let none = project_sections(&all, &NO_PINS);
        assert!(none.pinned.is_empty());
        assert_eq!(none.recent.len(), MAX_START_RECENT);
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
        let mut pinned = PinnedProjects::default();
        for path in &recent {
            pinned.pin(path);
        }
        let sections = project_sections(&recent, &pinned);
        assert!(sections.recent.is_empty());
        assert!(!sections.show_recent);

        let tokens = graphite();
        let texts = lay_out_texts(
            view(
                &tokens,
                &signex_i18n::I18n::en_us(),
                "Ctrl+O",
                &recent,
                &pinned,
                &[],
            ),
            MOCK_WINDOW,
        );
        assert!(texts.iter().any(|t| t.text == "PINNED PROJECTS"));
        assert!(texts.iter().all(|t| t.text != "RECENT PROJECTS"));
        assert!(texts.iter().all(|t| t.text != "No recent projects"));

        let empty = lay_out_texts(
            view(
                &tokens,
                &signex_i18n::I18n::en_us(),
                "Ctrl+O",
                &[],
                &NO_PINS,
                &[],
            ),
            MOCK_WINDOW,
        );
        assert!(empty.iter().any(|t| t.text == "RECENT PROJECTS"));
        assert!(empty.iter().any(|t| t.text == "No recent projects"));
    }

    /// #2305: Pinned Projects sits above Recent Projects, only when a
    /// project is pinned; a pinned project missing on disk says so and
    /// offers Remove.
    #[test]
    fn the_pinned_section_shows_above_recent_only_when_something_is_pinned() {
        let tokens = graphite();
        let i18n = signex_i18n::I18n::en_us();
        let recent = paths(3);
        let texts = lay_out_texts(
            view(&tokens, &i18n, "Ctrl+O", &recent, &NO_PINS, &[]),
            MOCK_WINDOW,
        );
        assert!(texts.iter().all(|t| t.text != "PINNED PROJECTS"));

        let mut pinned = PinnedProjects::default();
        pinned.pin(&recent[1]);
        pinned.pin(&PathBuf::from("gone.snxprj"));
        pinned.set_missing(&PathBuf::from("gone.snxprj"), true);
        let texts = lay_out_texts(
            view(&tokens, &i18n, "Ctrl+O", &recent, &pinned, &[]),
            MOCK_WINDOW,
        );
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
        // The mark sits on the gone row: under its path line, above the
        // next section's header.
        let gone_lines: Vec<_> = texts.iter().filter(|t| t.text == "gone.snxprj").collect();
        assert_eq!(gone_lines.len(), 2, "name and path: {gone_lines:#?}");
        let path_line = gone_lines[1].bounds;
        let mark = find(&texts, "Missing on disk").bounds;
        assert!(
            mark.y + 0.5 >= path_line.y + path_line.height,
            "mark {mark:?}, path {path_line:?}"
        );
        assert!(
            mark.y + mark.height <= top("RECENT PROJECTS"),
            "mark {mark:?} spills past the row"
        );
        // Remove has its own column: the path never runs under it.
        let remove_label = find(&texts, "Remove").bounds;
        assert!(
            path_line.x + path_line.width <= remove_label.x,
            "path {path_line:?} runs under Remove {remove_label:?}"
        );
        assert!(remove_label.y < path_line.y + path_line.height + BASE);
        assert!(remove_label.y + remove_label.height > gone_lines[0].bounds.y);
        let remove = crate::layout_probe::messages_on_click(
            view(&tokens, &i18n, "Ctrl+O", &recent, &pinned, &[]),
            MOCK_WINDOW,
            "Remove",
        );
        assert!(
            remove.iter().any(|m| matches!(
                m,
                Message::Project(crate::app::ProjectMsg::StartScreen(
                    StartScreenMsg::ProjectUnpinned(p)
                )) if p == &PathBuf::from("gone.snxprj")
            )),
            "{remove:?}"
        );
    }

    #[test]
    fn recent_section_is_absent_without_entries_and_present_with_them() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
        let empty = lay_out_texts(
            view(
                &tokens,
                &signex_i18n::I18n::en_us(),
                "Ctrl+O",
                &[],
                &NO_PINS,
                &[],
            ),
            SMALL_WINDOW,
        );
        assert!(empty.iter().all(|t| !t.text.starts_with("proj")));

        let some = paths(2);
        let texts = lay_out_texts(
            view(
                &tokens,
                &signex_i18n::I18n::en_us(),
                "Ctrl+O",
                &some,
                &NO_PINS,
                &[],
            ),
            SMALL_WINDOW,
        );
        // The Graphite screen names an entry in its list and on its card.
        let mut order: Vec<_> = texts
            .iter()
            .filter(|t| t.text.starts_with("proj"))
            .map(|t| t.text.as_str())
            .collect();
        order.dedup();
        assert_eq!(order, ["proj0.snxprj", "proj1.snxprj"]);
        assert!(texts.iter().any(|t| t.text == "RECENT PROJECTS"));
    }

    fn graphite() -> ThemeTokens {
        signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark)
    }

    /// The keycap is drawn by the full navigation, so it is checked at the
    /// narrowest window that keeps it (#2346); at `SMALL_WINDOW` the strip
    /// draws no keycap and still no blank text.
    #[test]
    fn an_unbound_open_shortcut_leaves_no_empty_keycap() {
        let tokens = graphite();
        let bound = lay_out_texts(
            view(
                &tokens,
                &signex_i18n::I18n::en_us(),
                "F2",
                &[],
                &NO_PINS,
                &[],
            ),
            NARROWEST_FULL_NAV,
        );
        assert!(bound.iter().any(|t| t.text == "F2"));
        for window in [NARROWEST_FULL_NAV, SMALL_WINDOW] {
            let unbound = lay_out_texts(
                view(&tokens, &signex_i18n::I18n::en_us(), "", &[], &NO_PINS, &[]),
                window,
            );
            if window == NARROWEST_FULL_NAV {
                assert!(unbound.iter().any(|t| t.text == "Open\u{2026}"));
            }
            assert!(
                unbound.iter().all(|t| !t.text.trim().is_empty()),
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
        let tokens = graphite();
        let recent = paths(MAX_START_RECENT + 2);
        let checks: [(iced::Size, &[&str]); 2] = [
            (
                NARROWEST_FULL_NAV,
                &["New project", "Open Sample Project", "proj0.snxprj"],
            ),
            (SMALL_WINDOW, &["proj0.snxprj"]),
        ];
        for (window, labels) in checks {
            assert_labels_fit(&tokens, &recent, window, labels);
        }
    }

    fn assert_labels_fit(
        tokens: &ThemeTokens,
        recent: &[PathBuf],
        window: iced::Size,
        labels: &[&str],
    ) {
        let texts = lay_out_texts(
            view(
                tokens,
                &signex_i18n::I18n::en_us(),
                "Ctrl+O",
                recent,
                &NO_PINS,
                &[],
            ),
            window,
        );
        let one_line = BASE * 2.0;
        for &label in labels {
            let bounds = find(&texts, label).bounds;
            // The primary button's label fills the button's height, so its
            // box is the button's, not a line's; wrapping shows as a
            // narrower-than-natural box, which the width check below catches.
            let natural = crate::layout_probe::natural::<()>(
                text(raw(label.to_string())).size(label_size(label)).boxed(),
                label,
            );
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

    /// A window as wide as the mock (1440 x 900).
    const MOCK_WINDOW: iced::Size = iced::Size::new(1440.0, 900.0);

    /// #2262: the hero title is semibold at the hero size, the line under it
    /// one step up the type scale.
    #[test]
    fn hero_lines_use_the_mock_sizes() {
        let tokens = graphite();
        let i18n = signex_i18n::I18n::en_us();
        let texts = lay_out_texts(
            view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]),
            MOCK_WINDOW,
        );
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
        let body = i18n
            .message(signex_i18n::ids::START_HERO_BODY, &[])
            .into_string();
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

    /// #2262: the app-only rows sit under a divider after the mock's page
    /// rows and still dispatch their commands.
    #[test]
    fn app_only_nav_rows_sit_under_the_divider_and_still_work() {
        let tokens = graphite();
        let i18n = signex_i18n::I18n::en_us();
        let texts = lay_out_texts(
            view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]),
            MOCK_WINDOW,
        );
        let top = |label: &str| find(&texts, label).bounds.y;
        // Projects, Libraries, Parts search: two row steps.
        let row_step = (top("Parts search") - top("Projects")) / 2.0;
        let gap = top("Open Sample Project") - top("Parts search");
        assert!(
            gap > row_step + 2.0 * crate::styles::Spacing::DEFAULT.m - 0.5,
            "no divider room between the page rows and the app rows: gap {gap}, row {row_step}"
        );
        assert!(top("Keyboard Shortcuts (F1)") > top("Open Sample Project"));

        let sample = crate::layout_probe::messages_on_click(
            view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]),
            MOCK_WINDOW,
            "Open Sample Project",
        );
        assert!(
            sample
                .iter()
                .any(|m| matches!(m, Message::Menu(MenuMessage::OpenSampleProject))),
            "{sample:?}"
        );
        let keys = crate::layout_probe::messages_on_click(
            view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]),
            MOCK_WINDOW,
            "Keyboard Shortcuts (F1)",
        );
        assert!(
            keys.iter()
                .any(|m| matches!(m, Message::Menu(MenuMessage::OpenKeyboardShortcuts))),
            "{keys:?}"
        );
    }

    /// #331: the app sets no `min_size` on the main window, so the screen is
    /// verified at 800 x 500 (floor) and 640 x 360 (stress).
    const MIN_SENSIBLE_WINDOW: iced::Size = iced::Size::new(800.0, 500.0);

    /// Every label at each window size: at least its natural width, one
    /// line high, horizontally inside the window, with the full recent list
    /// present. Vertically the list may exceed a short window only because it
    /// sits in a `scrollable`. Where the navigation is the icon strip
    /// (#2346) its labels are tooltips, so only the project list is laid out.
    #[test]
    fn every_button_label_fits_at_small_window_sizes() {
        let tokens = graphite();
        let ts = crate::styles::TypeScale::DEFAULT;
        let recent = paths(MAX_START_RECENT + 2);
        let nav_labels: Vec<(String, f32)> = vec![
            ("New project".into(), ts.dense),
            ("Open Sample Project".into(), ts.label),
            ("Keyboard Shortcuts (F1)".into(), ts.label),
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
            let texts = lay_out_texts(
                view(
                    &tokens,
                    &signex_i18n::I18n::en_us(),
                    "Ctrl+O",
                    &recent,
                    &NO_PINS,
                    &[],
                ),
                window,
            );
            for (label, size) in &labels {
                let bounds = find(&texts, label).bounds;
                let natural = crate::layout_probe::natural::<()>(
                    text(raw(label.clone())).size(*size).boxed(),
                    label,
                );
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
    const NAV_TEXTS: [&str; 8] = [
        "New project",
        "Open\u{2026}",
        "Home",
        "Projects",
        "Libraries",
        "Parts search",
        "Open Sample Project",
        "Keyboard Shortcuts (F1)",
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
        assert!(nav_strip::STRIP_W < graphite::NAV_W / 3.0);

        let tokens = graphite();
        let i18n = signex_i18n::I18n::en_us();
        let lay_out =
            |window| lay_out_texts(view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]), window);
        let full = lay_out(NARROWEST_FULL_NAV);
        for label in NAV_TEXTS {
            assert!(
                full.iter().any(|t| t.text == label),
                "{label:?} missing from the full navigation"
            );
        }
        let strip = lay_out(SMALL_WINDOW);
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
        let wide = lay_out(MOCK_WINDOW);
        assert!(wide.iter().any(|t| t.text == "Parts search"));
    }

    /// #2346: every strip row sends the message of its full row; the
    /// current page and the planned Parts search send none.
    #[test]
    fn the_icon_strip_rows_send_their_full_rows_messages() {
        let tokens = graphite();
        let i18n = signex_i18n::I18n::en_us();
        let step = crate::styles::Spacing::DEFAULT.m;
        let sent: Vec<String> = (0..)
            .map(|i| i as f32 * step)
            .take_while(|y| *y < SMALL_WINDOW.height)
            .flat_map(|y| {
                crate::layout_probe::messages_on_click_at(
                    view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]),
                    SMALL_WINDOW,
                    None,
                    iced::Point::new(nav_strip::STRIP_W / 2.0, y),
                )
            })
            .map(|m| format!("{m:?}"))
            .collect();
        let actions: Vec<String> = Action::ALL
            .iter()
            .map(|a| format!("{:?}", a.message()))
            .collect();
        for action in &actions {
            assert!(
                sent.contains(action),
                "no strip row sends {action}: {sent:?}"
            );
        }
        assert!(
            sent.iter().all(|m| actions.contains(m)),
            "a strip row sends something no full row sends: {sent:?}"
        );
    }

    /// Whether a click at (`x`, `y`) in a `window` start screen opens
    /// Keyboard Shortcuts.
    fn click_opens_shortcuts(window: iced::Size, x: f32, y: f32) -> bool {
        let tokens = graphite();
        let i18n = signex_i18n::I18n::en_us();
        let shortcuts = format!("{:?}", Action::KeyboardShortcuts.message());
        crate::layout_probe::messages_on_click_at(
            view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]),
            window,
            None,
            iced::Point::new(x, y),
        )
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
        let tokens = graphite();
        let i18n = signex_i18n::I18n::en_us();
        let row_h = graphite::NAV_ROW_H;
        let full_texts = lay_out_texts(
            view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]),
            NARROWEST_FULL_NAV,
        );
        let label = find(&full_texts, "Keyboard Shortcuts (F1)").bounds;
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
        let tokens = graphite();
        let i18n = signex_i18n::I18n::en_us();
        let shortcuts = format!("{:?}", Action::KeyboardShortcuts.message());
        let scrolled = crate::layout_probe::messages_for_events(
            view(&tokens, &i18n, "Ctrl+O", &[], &NO_PINS, &[]),
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
}
