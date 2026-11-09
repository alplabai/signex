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
//! the same handler.

use std::path::{Path, PathBuf};

use iced::widget::{column, container};
use iced::{Element, Length, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use crate::app::Message;
use crate::menu_bar::MenuMessage;
use crate::styles;

mod graphite;

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

/// The existing File ▸ Open Recent message for one entry.
pub fn recent_message(path: &Path) -> Message {
    Message::Menu(MenuMessage::OpenRecentProject(path.to_path_buf()))
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
    open_shortcut: &str,
    recent: &'a [PathBuf],
    libraries: &[LoadedLibrary],
) -> Element<'a, Message> {
    graphite::view(tokens, open_shortcut, recent, libraries)
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

    #[test]
    fn the_list_is_capped_and_keeps_order() {
        let all = paths(MAX_START_RECENT + 4);
        assert_eq!(visible_recent(&all), &all[..MAX_START_RECENT]);
        assert_eq!(visible_recent(&all[..2]), &all[..2]);
        assert!(visible_recent(&[]).is_empty());
    }

    #[test]
    fn recent_section_is_absent_without_entries_and_present_with_them() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
        let empty = lay_out_texts(view(&tokens, "Ctrl+O", &[], &[]), SMALL_WINDOW);
        assert!(empty.iter().all(|t| !t.text.starts_with("proj")));

        let some = paths(2);
        let texts = lay_out_texts(view(&tokens, "Ctrl+O", &some, &[]), SMALL_WINDOW);
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
        let bound = lay_out_texts(view(&tokens, "F2", &[], &[]), SMALL_WINDOW);
        assert!(bound.iter().any(|t| t.text == "F2"));
        let unbound = lay_out_texts(view(&tokens, "", &[], &[]), SMALL_WINDOW);
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
        let texts = lay_out_texts(view(&tokens, "Ctrl+O", &recent, &[]), SMALL_WINDOW);
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
            let texts = lay_out_texts(view(&tokens, "Ctrl+O", &recent, &[]), window);
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
