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

use iced::widget::{Space, button, column, container, scrollable, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use crate::app::Message;
use crate::menu_bar::MenuMessage;
use crate::styles;

/// Base text size every other measure on this screen is derived from.
const BASE: f32 = 12.0;

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

    fn label(self, open_shortcut: &str) -> String {
        match self {
            Action::NewProject => "New Project".to_string(),
            Action::OpenProject if open_shortcut.is_empty() => "Open Project…".to_string(),
            Action::OpenProject => format!("Open Project…  ({open_shortcut})"),
            Action::OpenSampleProject => "Open Sample Project".to_string(),
            Action::KeyboardShortcuts => "Keyboard Shortcuts (F1)".to_string(),
        }
    }

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

/// The "Recent" heading and one button per entry; nothing when empty.
fn recent_section<'a>(
    tokens: &'a ThemeTokens,
    recent: &'a [PathBuf],
    button_w: Length,
) -> Option<Element<'a, Message>> {
    let shown = visible_recent(recent);
    if shown.is_empty() {
        return None;
    }
    let mut list = column![
        text("Recent")
            .size(BASE * 0.9)
            .color(styles::ti(tokens.text_secondary))
    ]
    .spacing(BASE * 0.5)
    .align_x(iced::Alignment::Center);
    for path in shown {
        list = list.push(
            button(
                text(recent_label(path))
                    .size(BASE)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .width(button_w)
            .padding([BASE * 0.5, BASE])
            .on_press(recent_message(path))
            .style(styles::start_action(tokens, false)),
        );
    }
    Some(list.into())
}

pub fn view<'a>(
    tokens: &'a ThemeTokens,
    open_shortcut: &str,
    recent: &'a [PathBuf],
) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);
    let button_w = Length::Fixed(BASE * 20.0);

    let mut actions = column![]
        .spacing(BASE * 0.5)
        .align_x(iced::Alignment::Center);
    for action in Action::ALL {
        let primary = action == Action::NewProject;
        if action == Action::KeyboardShortcuts {
            actions = actions.push(Space::new().height(BASE));
        }
        actions = actions.push(
            button(
                text(action.label(open_shortcut))
                    .size(BASE)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .width(button_w)
            .padding([BASE * 0.5, BASE])
            .on_press(action.message())
            .style(styles::start_action(tokens, primary)),
        );
    }

    let mut content = column![
        text("Signex").size(BASE * 2.5).color(text_primary),
        text(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .size(BASE * 0.9)
            .color(text_secondary),
        Space::new().height(BASE * 1.5),
        actions,
    ]
    .spacing(BASE * 0.25)
    .align_x(iced::Alignment::Center);
    if let Some(section) = recent_section(tokens, recent, button_w) {
        content = content.push(Space::new().height(BASE * 1.5)).push(section);
    }

    // Scrolls rather than clips when the window is too short for it all.
    region(
        tokens,
        scrollable(container(content).center_x(Length::Fill)),
    )
}

pub fn hint_view<'a>(tokens: &'a ThemeTokens) -> Element<'a, Message> {
    let text_secondary = styles::ti(tokens.text_secondary);
    let content = column![
        text("No document selected")
            .size(BASE * 1.15)
            .color(text_secondary),
        text("Choose a schematic or PCB from the project tree")
            .size(BASE * 0.9)
            .color(text_secondary),
    ]
    .spacing(BASE * 0.65)
    .align_x(iced::Alignment::Center);
    region(tokens, content)
}

fn region<'a>(
    tokens: &'a ThemeTokens,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .center(Length::Fill)
        .style(styles::panel_region(tokens))
        .into()
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
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::Signex);
        let empty = lay_out_texts(view(&tokens, "Ctrl+O", &[]), SMALL_WINDOW);
        assert!(empty.iter().all(|t| t.text != "Recent"));

        let some = paths(2);
        let texts = lay_out_texts(view(&tokens, "Ctrl+O", &some), SMALL_WINDOW);
        let order: Vec<_> = texts
            .iter()
            .filter(|t| t.text.starts_with("proj"))
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(order, ["proj0.snxprj", "proj1.snxprj"]);
        assert!(texts.iter().any(|t| t.text == "Recent"));
    }

    #[test]
    fn labels_stay_on_one_line_and_inside_a_small_window() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::Signex);
        let recent = paths(MAX_START_RECENT + 2);
        let texts = lay_out_texts(view(&tokens, "Ctrl+O", &recent), SMALL_WINDOW);
        let one_line = BASE * 2.0;
        for label in ["New Project", "Open Sample Project", "proj0.snxprj"] {
            let bounds = find(&texts, label).bounds;
            assert!(bounds.height < one_line, "{label:?} wrapped: {bounds:?}");
            assert!(
                bounds.x >= 0.0 && bounds.x + bounds.width <= SMALL_WINDOW.width,
                "{label:?} overflows the window: {bounds:?}"
            );
        }
    }

    /// #331: the smallest window we lay the start screen out in. The app
    /// sets no `min_size` on the main window (`bootstrap/new.rs`), so the
    /// user can drag it smaller than this; 800 x 500 is the floor the
    /// screen is verified at, 640 x 360 (above) the stress case.
    const MIN_SENSIBLE_WINDOW: iced::Size = iced::Size::new(800.0, 500.0);

    /// Every button label, at both window sizes: at least its natural
    /// width, one line high, and horizontally inside the window, with
    /// the full recent list present. Vertically the list may exceed a
    /// short window only because it sits in a `scrollable`.
    #[test]
    fn every_button_label_fits_at_small_window_sizes() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::Signex);
        let recent = paths(MAX_START_RECENT + 2);
        let mut labels: Vec<String> = Action::ALL
            .iter()
            .map(|action| action.label("Ctrl+O"))
            .collect();
        labels.extend(visible_recent(&recent).iter().map(|p| recent_label(p)));

        for window in [MIN_SENSIBLE_WINDOW, SMALL_WINDOW] {
            let texts = lay_out_texts(view(&tokens, "Ctrl+O", &recent), window);
            for label in &labels {
                let bounds = find(&texts, label).bounds;
                let natural = crate::layout_probe::natural::<()>(
                    text(label.clone()).size(BASE).into(),
                    label,
                );
                assert!(
                    bounds.height <= natural.height + 0.5,
                    "{label:?} wraps at {window:?}: {bounds:?}, one line is {natural:?}"
                );
                assert!(
                    bounds.width + 0.5 >= natural.width,
                    "{label:?} is narrower than its text at {window:?}: {bounds:?}, natural {natural:?}"
                );
                assert!(
                    bounds.x >= 0.0 && bounds.x + bounds.width <= window.width,
                    "{label:?} overflows {window:?}: {bounds:?}"
                );
            }
            assert_eq!(
                texts.iter().filter(|t| t.text.starts_with("proj")).count(),
                MAX_START_RECENT,
                "the recent list is complete at {window:?}"
            );
        }
    }
}
