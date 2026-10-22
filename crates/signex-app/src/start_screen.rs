//! Start screen — what the centre region shows when no document is on
//! screen. Two distinct states (#331):
//!
//! * nothing loaded at all → [`view`], an actionable start screen
//!   (New Project, Open Project…, Keyboard Shortcuts);
//! * a project is loaded but no document is picked → [`hint_view`], a
//!   quieter pointer at the project tree.
//!
//! Open Sample Project and a recent-projects list are deliberately
//! absent: neither feature exists yet, and a dead button is worse than
//! no button. Add them here once the commands land.

use iced::widget::{Space, button, column, container, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;

use crate::app::Message;
use crate::menu_bar::MenuMessage;
use crate::styles;

/// Base text size every other measure on this screen is derived from.
const BASE: f32 = 12.0;

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
    KeyboardShortcuts,
}

impl Action {
    pub const ALL: [Action; 3] = [
        Action::NewProject,
        Action::OpenProject,
        Action::KeyboardShortcuts,
    ];

    fn label(self, open_shortcut: &str) -> String {
        match self {
            Action::NewProject => "New Project".to_string(),
            Action::OpenProject => format!("Open Project…  ({open_shortcut})"),
            Action::KeyboardShortcuts => "Keyboard Shortcuts (F1)".to_string(),
        }
    }

    /// The existing, working message this action dispatches.
    pub fn message(self) -> Message {
        Message::Menu(match self {
            Action::NewProject => MenuMessage::NewProject,
            Action::OpenProject => MenuMessage::OpenProject,
            Action::KeyboardShortcuts => MenuMessage::OpenKeyboardShortcuts,
        })
    }
}

pub fn view<'a>(tokens: &'a ThemeTokens, open_shortcut: &str) -> Element<'a, Message> {
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

    let content = column![
        text("Signex").size(BASE * 2.5).color(text_primary),
        text(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .size(BASE * 0.9)
            .color(text_secondary),
        Space::new().height(BASE * 1.5),
        actions,
    ]
    .spacing(BASE * 0.25)
    .align_x(iced::Alignment::Center);

    region(tokens, content)
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
