//! Empty states of the Components panel: no library mounted (a card with
//! Add Library… and Open Project…, #2535), a source chip with nothing in it,
//! and a filter nothing matches.

use iced::widget::{column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::{HINT_TEXT_SIZE, SP, look};
use crate::library::messages::LibraryMessage;
use crate::library::state::ComponentsMountSource;
use crate::panels::empty_state::EmptyStateAction;
use crate::styles::tokens::resolve;

/// What the workspace says about project libraries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProjectLibraries {
    NoProject,
    /// A project is open and declares no library.
    NoneDeclared,
    /// A project declares libraries, none of them mounted (pending, missing).
    DeclaredNotMounted,
}

impl ProjectLibraries {
    /// `declared` is the library count each open project declares.
    pub(super) fn of(declared: &[usize]) -> Self {
        if declared.is_empty() {
            Self::NoProject
        } else if declared.iter().all(|&n| n == 0) {
            Self::NoneDeclared
        } else {
            Self::DeclaredNotMounted
        }
    }
}

/// What an empty source says. Project tells apart no project,
/// a project without libraries and a project whose libraries are not mounted.
pub(super) fn empty_source_text(
    src: ComponentsMountSource,
    project: ProjectLibraries,
) -> MessageId {
    match src {
        ComponentsMountSource::Project => match project {
            ProjectLibraries::NoProject => ids::PANELS_COMPONENTS_EMPTY_NO_PROJECT,
            ProjectLibraries::NoneDeclared => ids::PANELS_COMPONENTS_EMPTY_NONE_DECLARED,
            ProjectLibraries::DeclaredNotMounted => ids::PANELS_COMPONENTS_EMPTY_NOT_MOUNTED,
        },
        ComponentsMountSource::Installed => ids::PANELS_COMPONENTS_EMPTY_INSTALLED,
        ComponentsMountSource::Global => ids::PANELS_COMPONENTS_EMPTY_GLOBAL,
        ComponentsMountSource::Stock => ids::PANELS_COMPONENTS_EMPTY_STOCK,
    }
}

/// The two actions of the "No libraries mounted" card: Add Library… opens
/// the "+ Add" menu, Open Project… runs File > Open Project.
pub(super) fn nothing_mounted_actions(i18n: &I18n) -> Vec<EmptyStateAction<LibraryMessage>> {
    vec![
        EmptyStateAction {
            label: t!(i18n, ids::PANELS_COMPONENTS_ADD_LIBRARY),
            on_press: LibraryMessage::ComponentsPanelAddMenuToggled,
            primary: true,
        },
        EmptyStateAction {
            label: t!(i18n, ids::PANELS_COMPONENTS_OPEN_PROJECT),
            on_press: LibraryMessage::ComponentsPanelOpenProjectClicked,
            primary: false,
        },
    ]
}

/// "No libraries mounted" as a card with its two buttons.
pub(super) fn nothing_mounted<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    container(crate::panels::empty_state::empty_state_card_with_actions(
        tokens,
        t!(i18n, ids::PANELS_COMPONENTS_NOTHING_MOUNTED),
        t!(i18n, ids::PANELS_COMPONENTS_NOTHING_MOUNTED_BODY),
        None,
        nothing_mounted_actions(i18n),
    ))
    .padding([SP.l, look::EDGE])
    .width(Length::Fill)
    .boxed()
}

/// No part matches the filter: say so, with a Clear that empties it.
pub(super) fn no_match<'a>(
    filter: &str,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    container(
        column![
            text(t!(i18n, ids::PANELS_COMPONENTS_NO_MATCH, "filter" => filter.trim().to_owned()))
                .size(HINT_TEXT_SIZE)
                .color(r.ink3),
            button(
                text(t!(i18n, ids::PANELS_COMPONENTS_CLEAR_FILTER))
                    .size(HINT_TEXT_SIZE)
                    .color(crate::styles::ti(tokens.text)),
            )
            .padding([SP.xs, SP.m])
            .on_press(LibraryMessage::ComponentsPanelSetFilter(String::new()))
            .style(crate::styles::outline_button(tokens)),
        ]
        .spacing(SP.s),
    )
    .padding([SP.m, look::LIBRARY_INSET])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_project_section_tells_its_three_states_apart() {
        let en = I18n::en_us();
        let project =
            |state| t!(en, empty_source_text(ComponentsMountSource::Project, state)).into_string();
        let none = project(ProjectLibraries::NoProject);
        let empty = project(ProjectLibraries::NoneDeclared);
        let unmounted = project(ProjectLibraries::DeclaredNotMounted);
        assert!(none.contains("Open a project"), "{none}");
        assert!(empty.contains("no libraries yet"), "{empty}");
        assert!(
            empty.contains("Add New to Project > Component Library"),
            "{empty}"
        );
        assert!(unmounted.contains("not mounted"), "{unmounted}");
        assert!(!unmounted.contains("no libraries yet"), "{unmounted}");
    }

    #[test]
    fn the_state_comes_from_the_declared_libraries() {
        assert_eq!(ProjectLibraries::of(&[]), ProjectLibraries::NoProject);
        assert_eq!(ProjectLibraries::of(&[0]), ProjectLibraries::NoneDeclared);
        assert_eq!(
            ProjectLibraries::of(&[0, 2]),
            ProjectLibraries::DeclaredNotMounted
        );
    }

    #[test]
    fn other_sections_ignore_the_project_state() {
        for src in [
            ComponentsMountSource::Installed,
            ComponentsMountSource::Global,
            ComponentsMountSource::Stock,
        ] {
            assert_eq!(
                empty_source_text(src, ProjectLibraries::NoneDeclared),
                empty_source_text(src, ProjectLibraries::NoProject)
            );
        }
    }

    /// The empty card offers Add Library… (primary, the "+ Add" menu) and
    /// Open Project… (File > Open Project).
    #[test]
    fn the_nothing_mounted_card_offers_add_library_and_open_project() {
        let en = I18n::en_us();
        let actions = nothing_mounted_actions(&en);
        let labels: Vec<String> = actions
            .iter()
            .map(|a| a.label.clone().into_string())
            .collect();
        assert_eq!(labels, ["Add Library…", "Open Project…"]);
        assert!(actions[0].primary && !actions[1].primary);
        assert!(matches!(
            actions[0].on_press,
            LibraryMessage::ComponentsPanelAddMenuToggled
        ));
        assert!(matches!(
            actions[1].on_press,
            LibraryMessage::ComponentsPanelOpenProjectClicked
        ));
    }

    #[test]
    fn no_match_names_the_filter() {
        let en = I18n::en_us();
        let line =
            t!(en, ids::PANELS_COMPONENTS_NO_MATCH, "filter" => "xyz".to_owned()).into_string();
        assert!(line.contains("No components match"), "{line}");
        assert!(line.contains("xyz"), "{line}");
    }
}
