//! Tab in the in-place editor on a part's field (mockup row 65). Every other
//! Tab, and Tab with a modifier, stays the keymap's.

use iced::keyboard;
use signex_types::schematic::SelectedKind;

use super::{Claim, InputTarget};
use crate::app::{Message, Signex, TextEditMsg};

impl Signex {
    /// Claim a bare Tab while the editor is open on a part's field, in a
    /// window showing its sheet with no modal over it. A held key moves
    /// one field: its repeats are swallowed.
    pub(super) fn claim_text_edit_tab(
        &self,
        target: InputTarget,
        window: iced::window::Id,
        event: &keyboard::Event,
    ) -> Claim {
        let keyboard::Event::KeyPressed {
            key,
            modifiers,
            repeat,
            ..
        } = event
        else {
            return Claim::Pass;
        };
        let is_tab = matches!(
            key.as_ref(),
            keyboard::Key::Named(keyboard::key::Named::Tab)
        );
        if !is_tab || !modifiers.is_empty() || !self.edits_part_field_in(target, window) {
            return Claim::Pass;
        }
        if *repeat {
            return Claim::Swallow;
        }
        Claim::consume(Message::TextEdit(TextEditMsg::NextFieldRequested))
    }

    /// Whether `window` shows an open editor on a part's field.
    fn edits_part_field_in(&self, target: InputTarget, window: iced::window::Id) -> bool {
        let Some(state) = self.interaction_state.editing_text.as_ref() else {
            return false;
        };
        let is_part_field = matches!(
            state.kind,
            SelectedKind::SymbolRefField | SelectedKind::SymbolValField | SelectedKind::Symbol
        );
        let shown = self
            .document_state
            .sheet_path_for_window(window, &self.ui_state);
        is_part_field
            && target.paints_overlay_stack()
            && !self.has_blocking_modal()
            && shown.map(std::path::PathBuf::as_path) == state.sheet.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::TextEditState;

    fn app_editing(kind: SelectedKind) -> (Signex, iced::window::Id) {
        let (mut app, _boot) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        app.interaction_state.editing_text = Some(TextEditState {
            uuid: uuid::Uuid::from_u128(1),
            kind,
            text: "R1".into(),
            original_text: "R1".into(),
            world_x: 0.0,
            world_y: 0.0,
            sheet: None,
            placement: None,
        });
        let main = app.ui_state.main_window_id.expect("the main window");
        (app, main)
    }

    fn tab(modifiers: keyboard::Modifiers, repeat: bool) -> keyboard::Event {
        let key = keyboard::Key::Named(keyboard::key::Named::Tab);
        keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Tab),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat,
        }
    }

    fn moves_field(routed: Option<Message>) -> bool {
        matches!(
            routed,
            Some(Message::TextEdit(TextEditMsg::NextFieldRequested))
        )
    }

    #[test]
    fn tab_on_a_part_field_moves_the_editor() {
        for kind in [
            SelectedKind::SymbolRefField,
            SelectedKind::SymbolValField,
            SelectedKind::Symbol,
        ] {
            let (app, main) = app_editing(kind);
            let routed = app.route_key_for_test(main, &tab(keyboard::Modifiers::empty(), false));
            assert!(moves_field(routed), "{kind:?}");
        }
    }

    #[test]
    fn tab_on_a_label_and_shift_tab_stay_the_keymaps() {
        let (app, main) = app_editing(SelectedKind::Label);
        let routed = app.route_key_for_test(main, &tab(keyboard::Modifiers::empty(), false));
        assert!(!moves_field(routed));
        let (app, main) = app_editing(SelectedKind::SymbolRefField);
        let routed = app.route_key_for_test(main, &tab(keyboard::Modifiers::SHIFT, false));
        assert!(!moves_field(routed));
    }

    #[test]
    fn a_held_tab_moves_one_field() {
        let (app, main) = app_editing(SelectedKind::SymbolRefField);
        let routed = app.route_key_for_test(main, &tab(keyboard::Modifiers::empty(), true));
        assert!(routed.is_none());
    }
}
