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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{find, lay_out_texts};

    /// A small but plausible window; the centre region is narrower still.
    const SMALL_WINDOW: iced::Size = iced::Size::new(640.0, 360.0);

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
            SMALL_WINDOW,
        );
        assert!(bound.iter().any(|t| t.text == "F2"));
        let unbound = lay_out_texts(
            view(&tokens, &signex_i18n::I18n::en_us(), "", &[], &NO_PINS, &[]),
            SMALL_WINDOW,
        );
        assert!(unbound.iter().any(|t| t.text == "Open\u{2026}"));
        assert!(
            unbound.iter().all(|t| !t.text.trim().is_empty()),
            "an unbound shortcut leaves a blank keycap: {unbound:#?}"
        );
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

    #[test]
    fn labels_stay_on_one_line_and_inside_a_small_window() {
        let tokens = graphite();
        let recent = paths(MAX_START_RECENT + 2);
        let texts = lay_out_texts(
            view(
                &tokens,
                &signex_i18n::I18n::en_us(),
                "Ctrl+O",
                &recent,
                &NO_PINS,
                &[],
            ),
            SMALL_WINDOW,
        );
        let one_line = BASE * 2.0;
        for label in ["New project", "Open Sample Project", "proj0.snxprj"] {
            let bounds = find(&texts, label).bounds;
            // The primary button's label fills the button's height, so its
            // box is the button's, not a line's; wrapping shows as a
            // narrower-than-natural box, which the width check below catches.
            let natural = crate::layout_probe::natural::<()>(
                text(raw(label.to_string())).size(label_size(label)).boxed(),
                label,
            );
            if label != "New project" {
                assert!(bounds.height < one_line, "{label:?} wrapped: {bounds:?}");
            }
            assert!(
                bounds.width + 0.5 >= natural.width,
                "{label:?} is narrower than its text: {bounds:?}, natural {natural:?}"
            );
            assert!(
                bounds.x >= 0.0 && bounds.x + bounds.width <= SMALL_WINDOW.width,
                "{label:?} overflows the window: {bounds:?}"
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

    /// Every label at both window sizes: at least its natural width, one
    /// line high, horizontally inside the window, with the full recent list
    /// present. Vertically the list may exceed a short window only because it
    /// sits in a `scrollable`.
    #[test]
    fn every_button_label_fits_at_small_window_sizes() {
        let tokens = graphite();
        let ts = crate::styles::TypeScale::DEFAULT;
        let recent = paths(MAX_START_RECENT + 2);
        let mut labels: Vec<(String, f32)> = vec![
            ("New project".into(), ts.dense),
            ("Open Sample Project".into(), ts.label),
            ("Keyboard Shortcuts (F1)".into(), ts.label),
        ];
        labels.extend(
            visible_recent(&recent)
                .iter()
                .map(|p| (recent_label(p), ts.label)),
        );

        for window in [MIN_SENSIBLE_WINDOW, SMALL_WINDOW] {
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
}
