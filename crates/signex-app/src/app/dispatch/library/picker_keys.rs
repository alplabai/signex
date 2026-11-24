//! ↑ and ↓ in the Place Component picker (mockup row 129): the footer reads
//! "↑↓ to move · Enter to place", so the arrows move the list's selection
//! while the picker's list shows, in the window that shows it.

use iced::keyboard;

use super::super::input::InputTarget;
use crate::app::Message;
use crate::app::state::ModalId;
use crate::library::messages::{LibraryMessage, PickerMsg};
use crate::library::state::PickerMode;

impl crate::app::Signex {
    /// The message ↑ or ↓ makes in `target` while the picker lists parts;
    /// `None` for any other key, window or stage.
    pub(in crate::app::dispatch) fn picker_arrow_message(
        &self,
        target: InputTarget,
        event: &keyboard::Event,
    ) -> Option<Message> {
        let picker = self.library.picker.as_ref()?;
        if !matches!(
            picker.mode,
            PickerMode::Place | PickerMode::ChangeComponent(_)
        ) {
            return None;
        }
        // Its own window, or the main window while it has none.
        let in_own_window = target == InputTarget::DetachedModal(ModalId::LibraryPicker);
        let in_main_fallback = target.paints_overlay_stack()
            && self.detached_window_of(ModalId::LibraryPicker).is_none();
        if !in_own_window && !in_main_fallback {
            return None;
        }
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        if !modifiers.is_empty() {
            return None;
        }
        let step = match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => 1,
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => -1,
            _ => return None,
        };
        Some(Message::Library(LibraryMessage::Picker(
            PickerMsg::SelectionStepped(step),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::state::PickerState;

    fn arrow(named: keyboard::key::Named) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(named),
            modified_key: keyboard::Key::Named(named),
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        }
    }

    fn step_of(message: Option<Message>) -> Option<isize> {
        match message? {
            Message::Library(LibraryMessage::Picker(PickerMsg::SelectionStepped(step))) => {
                Some(step)
            }
            _ => None,
        }
    }

    /// With the picker listing parts in the main window, ↓ steps one row on
    /// and ↑ one row back; a detached panel and a closed picker get nothing.
    #[test]
    fn the_arrows_step_the_picker_only_where_its_list_shows() {
        use keyboard::key::Named::{ArrowDown, ArrowLeft, ArrowUp};
        let (mut app, _boot) = crate::app::Signex::new();
        let down = arrow(ArrowDown);
        assert_eq!(
            step_of(app.picker_arrow_message(InputTarget::Main, &down)),
            None
        );

        app.library.picker = Some(PickerState::default());
        assert_eq!(
            step_of(app.picker_arrow_message(InputTarget::Main, &down)),
            Some(1)
        );
        assert_eq!(
            step_of(app.picker_arrow_message(InputTarget::Main, &arrow(ArrowUp))),
            Some(-1)
        );
        assert_eq!(
            step_of(app.picker_arrow_message(InputTarget::Main, &arrow(ArrowLeft))),
            None
        );
        assert_eq!(
            step_of(app.picker_arrow_message(InputTarget::DetachedPanel, &down)),
            None
        );
    }

    fn summary(pn: &str) -> signex_library::ComponentSummary {
        signex_library::ComponentSummary {
            row_id: uuid::Uuid::now_v7(),
            internal_pn: signex_library::InternalPn::new(pn),
            mpn: String::new(),
            state: signex_library::LifecycleState::Released,
            description: String::new(),
        }
    }

    /// A step through `update` selects the listed row it lands on: ↓ from
    /// nothing takes the first row, ↓ again the next, and ↓ on the last row
    /// keeps it.
    #[test]
    fn a_step_selects_the_listed_row_it_lands_on() {
        let (mut app, _boot) = crate::app::Signex::new();
        app.library.open_libraries.clear();
        app.library
            .open_libraries
            .push(crate::library::state::OpenLibrary {
                root: "/tmp/picker-keys/Lib.snxlib".into(),
                display_name: "Lib".into(),
                library_id: uuid::Uuid::now_v7(),
                tables: Default::default(),
                cached_components: vec![summary("R-1K"), summary("C-1U")],
                cached_symbols: Vec::new(),
                cached_footprints: Vec::new(),
                cached_sims: Vec::new(),
                cached_classes: Vec::new(),
                observed_stamp: None,
                own_edit_stamp: None,
                display: Default::default(),
                dangling: Default::default(),
                pricing: Default::default(),
            });
        app.library.touch_data();
        app.library.picker = Some(PickerState::default());
        app.library.sync_view_caches_in(&app.ui_state.i18n);

        let selected_pn = |app: &crate::app::Signex| {
            let picker = app.library.picker.as_ref().expect("picker open");
            picker
                .selected
                .as_ref()
                .map(|(_, s)| s.internal_pn.as_str().to_string())
        };
        let down = || Message::Library(LibraryMessage::Picker(PickerMsg::SelectionStepped(1)));
        let _ = app.update(down());
        assert_eq!(selected_pn(&app).as_deref(), Some("R-1K"));
        let _ = app.update(down());
        assert_eq!(selected_pn(&app).as_deref(), Some("C-1U"));
        let _ = app.update(down());
        assert_eq!(selected_pn(&app).as_deref(), Some("C-1U"));
    }
}
