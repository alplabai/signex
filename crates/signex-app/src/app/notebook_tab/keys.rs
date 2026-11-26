//! The notebook tab's keys (mockup row 195, "Keyboard"): Ctrl+S saves the
//! notebook, Ctrl+\ cycles Source, Split and Preview, F8 goes to the next
//! problem, Ctrl+Shift+M inserts a measurement; while the reference picker
//! is open Up and Down move its highlight and Esc closes it (Esc closes the
//! Block menu too). Typing `@` in the editor opens the picker
//! (`edit::act`). Each is claimed only while the tab is the one shown.

use iced::keyboard::{self, Key, Modifiers, key::Named};
use signex_notebook::{AnnotationKind, Insertion};

use super::NotebookMsg;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// What a key on the tab does, before it names its project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TabKey {
    Save,
    CycleView,
    NextProblem,
    InsertMeasurement,
    PickerUp,
    PickerDown,
    Escape,
}

/// The tab's key for `key` with `modifiers`; `picker` and `menu` say what
/// is open over the editor.
pub(super) fn tab_key(key: &Key, modifiers: Modifiers, picker: bool, menu: bool) -> Option<TabKey> {
    let command = modifiers.command() && !modifiers.alt();
    match key.as_ref() {
        Key::Named(Named::Escape) if modifiers.is_empty() && (picker || menu) => {
            Some(TabKey::Escape)
        }
        Key::Named(Named::ArrowUp) if picker && modifiers.is_empty() => Some(TabKey::PickerUp),
        Key::Named(Named::ArrowDown) if picker && modifiers.is_empty() => Some(TabKey::PickerDown),
        Key::Named(Named::F8) if modifiers.is_empty() => Some(TabKey::NextProblem),
        Key::Character(c) if command && !modifiers.shift() && c.eq_ignore_ascii_case("s") => {
            Some(TabKey::Save)
        }
        Key::Character("\\") if command && !modifiers.shift() => Some(TabKey::CycleView),
        Key::Character(c) if command && modifiers.shift() && c.eq_ignore_ascii_case("m") => {
            Some(TabKey::InsertMeasurement)
        }
        _ => None,
    }
}

impl TabKey {
    fn message(self, project_id: ProjectId) -> NotebookMsg {
        match self {
            TabKey::Save => NotebookMsg::SavePressed { project_id },
            TabKey::CycleView => NotebookMsg::ViewCycled { project_id },
            TabKey::NextProblem => NotebookMsg::NextProblemRequested { project_id },
            TabKey::InsertMeasurement => NotebookMsg::InsertPressed {
                project_id,
                insertion: Insertion::Block(AnnotationKind::Measurement),
            },
            TabKey::PickerUp => NotebookMsg::PickerMoved {
                project_id,
                down: false,
            },
            TabKey::PickerDown => NotebookMsg::PickerMoved {
                project_id,
                down: true,
            },
            TabKey::Escape => NotebookMsg::EscapePressed { project_id },
        }
    }
}

impl Signex {
    /// The notebook tab's key for `event` in `window`, when the window
    /// shows a notebook tab and no overlay over it takes the key first.
    pub(crate) fn notebook_key(
        &self,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Option<Message> {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        let shown = self
            .document_state
            .tab_shown_in_window(window, &self.ui_state)?;
        let id = shown.kind.as_notebook()?;
        let tab = self.ui_state.notebook_tabs.get(&id)?;
        let found = tab_key(key, *modifiers, tab.picker.is_some(), tab.block_menu)?;
        if found == TabKey::Escape && self.escape_overlay_message().is_some() {
            return None;
        }
        Some(Message::from(found.message(id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(c: &str) -> Key {
        Key::Character(c.into())
    }

    #[test]
    fn the_drawn_keys_are_read() {
        let ctrl = Modifiers::COMMAND;
        let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        assert_eq!(tab_key(&ch("s"), ctrl, false, false), Some(TabKey::Save));
        assert_eq!(
            tab_key(&ch("\\"), ctrl, false, false),
            Some(TabKey::CycleView)
        );
        assert_eq!(
            tab_key(&ch("M"), ctrl_shift, false, false),
            Some(TabKey::InsertMeasurement)
        );
        assert_eq!(
            tab_key(&Key::Named(Named::F8), Modifiers::empty(), false, false),
            Some(TabKey::NextProblem)
        );
        assert_eq!(tab_key(&ch("s"), Modifiers::empty(), false, false), None);
    }

    #[test]
    fn arrows_and_escape_are_the_pickers_only_while_it_is_open() {
        let none = Modifiers::empty();
        let up = Key::Named(Named::ArrowUp);
        let esc = Key::Named(Named::Escape);
        assert_eq!(tab_key(&up, none, true, false), Some(TabKey::PickerUp));
        assert_eq!(tab_key(&up, none, false, false), None, "the editor's own");
        assert_eq!(tab_key(&esc, none, false, true), Some(TabKey::Escape));
        assert_eq!(tab_key(&esc, none, false, false), None);
    }
}
