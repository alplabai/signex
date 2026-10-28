//! Keeping each tool or confirm dialog in step with its own OS window.
//! Design and the lock policy: docs/DETACHED_MODALS.md.

use iced::Task;

use crate::app::Message;
use crate::app::state::{ModalId, Signex, WindowKind};

/// Dialogs whose window follows their open state, whatever code flips it.
const FOLLOWED: [ModalId; 11] = [
    ModalId::FindReplace,
    ModalId::RenameDialog,
    ModalId::RemoveDialog,
    ModalId::EnableVersionControl,
    ModalId::GridProperties,
    ModalId::SelectionFilterCustom,
    ModalId::About,
    ModalId::KeyboardShortcuts,
    ModalId::PassiveCalculator,
    ModalId::FirstRunTour,
    ModalId::CreateChildSheet,
];

/// Confirm-class dialogs among the detached ones: while one is open the
/// main window takes no input.
const LOCKING: [ModalId; 4] = [
    ModalId::RenameDialog,
    ModalId::RemoveDialog,
    ModalId::AnnotateResetConfirm,
    ModalId::CreateChildSheet,
];

impl Signex {
    /// Whether a followed dialog's state says it is open; `None` for a
    /// dialog that manages its own window.
    fn followed_modal_open(&self, modal: ModalId) -> Option<bool> {
        let ui = &self.ui_state;
        Some(match modal {
            ModalId::FindReplace => ui.find_replace.open,
            ModalId::RenameDialog => ui.rename_dialog.is_some(),
            ModalId::RemoveDialog => ui.remove_dialog.is_some(),
            ModalId::EnableVersionControl => ui.enable_version_control.is_some(),
            ModalId::GridProperties => ui.grid_properties.is_some(),
            ModalId::SelectionFilterCustom => ui.selection_filter_custom.is_some(),
            ModalId::About => ui.about_open,
            ModalId::KeyboardShortcuts => ui.keyboard_shortcuts_open,
            ModalId::PassiveCalculator => ui.passive_calculator_open,
            ModalId::FirstRunTour => ui.first_run_tour_open,
            ModalId::CreateChildSheet => ui.create_child_sheet_confirm.is_some(),
            ModalId::AnnotateDialog
            | ModalId::AnnotateResetConfirm
            | ModalId::ErcDialog
            | ModalId::MoveSelection
            | ModalId::NetColorPalette
            | ModalId::ParameterManager
            | ModalId::Preferences
            | ModalId::PrintPreview
            | ModalId::BomPreview => return None,
        })
    }

    /// Open the window of every followed dialog that has just opened and
    /// close the window of every one that has just closed. Runs after each
    /// `update`, so the many places that flip these flags need no wiring.
    pub(crate) fn sync_detached_modals(&mut self) -> Option<Task<Message>> {
        if !self.ui_state.main_window_opened {
            return None;
        }
        let mut tasks = Vec::new();
        for modal in FOLLOWED {
            let Some(open) = self.followed_modal_open(modal) else {
                continue;
            };
            match (open, self.modal_detached(modal)) {
                (true, false) => tasks.push(self.handle_detach_modal(modal)),
                (false, true) => tasks.push(self.close_detached_modal(modal)),
                (true, true) | (false, false) => {}
            }
        }
        (!tasks.is_empty()).then(|| Task::batch(tasks))
    }

    /// The window of the open confirm dialog that locks the main window, if
    /// it lives in its own window.
    pub(crate) fn locking_confirm_window(&self) -> Option<iced::window::Id> {
        self.ui_state
            .windows
            .iter()
            .find_map(|(id, kind)| match kind {
                WindowKind::DetachedModal(modal) if LOCKING.contains(modal) => Some(*id),
                _ => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        EnableVersionControlState, GridPropertiesState, InputMsg, RemoveDialogState,
        RenameDialogState, SelectionFilterCustomState, VersionControlScope, WindowMsg,
    };
    use crate::layout_probe::{LaidOutText, lay_out_texts};

    fn test_app() -> (Signex, iced::window::Id) {
        let (mut app, _task) = Signex::new();
        let main = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(main);
        app.ui_state.main_window_opened = true;
        app.ui_state.first_run_tour_open = false;
        (app, main)
    }

    /// Put `modal` in its open state, the way its open site does.
    fn open(app: &mut Signex, modal: ModalId) {
        let ui = &mut app.ui_state;
        match modal {
            ModalId::FindReplace => ui.find_replace.open = true,
            ModalId::RenameDialog => {
                ui.rename_dialog = Some(RenameDialogState {
                    target_path: "p.snxsch".into(),
                    tree_path: vec![0],
                    project_id: None,
                    buffer: "p".into(),
                    error: None,
                    is_project_rename: false,
                })
            }
            ModalId::RemoveDialog => {
                ui.remove_dialog = Some(RemoveDialogState {
                    target_path: "p.snxsch".into(),
                    tree_path: vec![0],
                    project_id: None,
                    display_name: "p".into(),
                    is_outside_project: false,
                    unsaved_prompt: false,
                })
            }
            ModalId::EnableVersionControl => {
                ui.enable_version_control = Some(EnableVersionControlState {
                    scope: VersionControlScope::Project,
                    project_path: "p.snxprj".into(),
                    project_dir: ".".into(),
                    project_name: "p".into(),
                    items: Vec::new(),
                    use_lfs: false,
                    intro_text: String::new(),
                    error: None,
                    working: false,
                })
            }
            ModalId::GridProperties => {
                ui.grid_properties = Some(GridPropertiesState {
                    step_x_mm: "1".into(),
                    step_y_mm: "1".into(),
                    link_xy: true,
                    fine_display: Default::default(),
                    coarse_display: Default::default(),
                    multiplier: 5,
                })
            }
            ModalId::SelectionFilterCustom => {
                ui.selection_filter_custom = Some(SelectionFilterCustomState {
                    pads: true,
                    tracks: true,
                    arcs: true,
                    pours: true,
                    bodies_3d: true,
                    keepouts: true,
                    cutouts: true,
                    texts: true,
                    vias: true,
                    regions: true,
                    fills: true,
                    other: true,
                })
            }
            ModalId::About => ui.about_open = true,
            ModalId::KeyboardShortcuts => ui.keyboard_shortcuts_open = true,
            ModalId::PassiveCalculator => ui.passive_calculator_open = true,
            ModalId::FirstRunTour => ui.first_run_tour_open = true,
            ModalId::CreateChildSheet => {
                ui.create_child_sheet_confirm =
                    Some(crate::app::contracts::CreateChildSheetConfirmState {
                        symbol: uuid::Uuid::nil(),
                        name: "s".into(),
                        parent: "p.snxsch".into(),
                        project_dir: ".".into(),
                        path_text: "s.snxsch".into(),
                        error: None,
                        exists: false,
                    })
            }
            other => panic!("{other:?} is not followed"),
        }
    }

    fn window_of(app: &Signex, modal: ModalId) -> Option<iced::window::Id> {
        app.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, WindowKind::DetachedModal(m) if *m == modal).then_some(*id)
        })
    }

    #[test]
    fn opening_each_followed_dialog_gives_it_a_window_and_closing_removes_it() {
        for modal in FOLLOWED {
            let (mut app, _main) = test_app();
            open(&mut app, modal);
            assert!(
                window_of(&app, modal).is_none(),
                "{modal:?} before the sync"
            );
            let _ = app.sync_detached_modals();
            assert!(window_of(&app, modal).is_some(), "{modal:?} gets a window");
            // Nothing new on a second pass.
            let _ = app.sync_detached_modals();
            assert_eq!(app.ui_state.windows.len(), 1, "{modal:?} opened twice");

            // The dialog's own close message (what the OS close button and
            // the in-dialog close send) clears the state and the window.
            let close = Signex::detached_modal_close_message(modal).expect("a close message");
            let _ = app.update(close);
            assert_eq!(app.followed_modal_open(modal), Some(false), "{modal:?}");
            assert!(window_of(&app, modal).is_none(), "{modal:?} window stays");
        }
    }

    #[test]
    fn the_os_close_request_and_esc_close_each_followed_dialog() {
        for modal in FOLLOWED {
            for via_esc in [false, true] {
                let (mut app, _main) = test_app();
                open(&mut app, modal);
                let _ = app.sync_detached_modals();
                let id = window_of(&app, modal).expect("window");
                let message = if via_esc {
                    Message::Input(InputMsg::EscapePressed { window: Some(id) })
                } else {
                    Message::Window(WindowMsg::WindowCloseRequested(id))
                };
                let _ = app.update(message);
                assert_eq!(
                    app.followed_modal_open(modal),
                    Some(false),
                    "{modal:?} (esc: {via_esc})"
                );
                assert!(
                    window_of(&app, modal).is_none(),
                    "{modal:?} (esc: {via_esc})"
                );
            }
        }
    }

    #[test]
    fn a_window_that_closes_underneath_clears_the_state() {
        for modal in FOLLOWED {
            let (mut app, _main) = test_app();
            open(&mut app, modal);
            let _ = app.sync_detached_modals();
            let id = window_of(&app, modal).expect("window");
            let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));
            assert_eq!(app.followed_modal_open(modal), Some(false), "{modal:?}");
        }
    }

    fn main_texts(app: &Signex, main: iced::window::Id) -> Vec<LaidOutText> {
        lay_out_texts(app.view(main), iced::Size::new(1400.0, 900.0))
    }

    #[test]
    fn a_detached_dialog_does_not_paint_in_the_main_window() {
        // (dialog, a text only its body draws)
        let probes = [
            (ModalId::About, "Copy version info"),
            (
                ModalId::FindReplace,
                "Targets: labels, notes, designators, values",
            ),
            (ModalId::FirstRunTour, "F1 for keyboard shortcuts"),
        ];
        for (modal, probe) in probes {
            let (mut app, main) = test_app();
            open(&mut app, modal);
            assert!(
                main_texts(&app, main).iter().any(|t| t.text == probe),
                "{modal:?} paints in the window before it is detached"
            );
            let _ = app.sync_detached_modals();
            assert!(
                !main_texts(&app, main).iter().any(|t| t.text == probe),
                "{modal:?} still paints in the main window"
            );
            // Its own window has the body.
            let id = window_of(&app, modal).expect("window");
            let texts = lay_out_texts(app.view(id), iced::Size::new(800.0, 700.0));
            assert!(texts.iter().any(|t| t.text == probe), "{modal:?} body");
        }
    }

    #[test]
    fn f1_still_toggles_the_shortcuts_sheet_from_either_window() {
        let (mut app, main) = test_app();
        let f1 = iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::F1),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        };
        let _ = app.handle_key_input(main, f1.clone());
        assert!(app.ui_state.keyboard_shortcuts_open);
        let _ = app.sync_detached_modals();
        let sheet = window_of(&app, ModalId::KeyboardShortcuts).expect("window");
        // F1 typed in the sheet's own window closes it.
        let _ = app.handle_key_input(sheet, f1.clone());
        assert!(!app.ui_state.keyboard_shortcuts_open);
        // And from the main window it opens again.
        let _ = app.handle_key_input(main, f1);
        assert!(app.ui_state.keyboard_shortcuts_open);
    }

    fn key(c: &str) -> iced::keyboard::Event {
        iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c.into()),
            modified_key: iced::keyboard::Key::Character(c.into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: Some(c.into()),
            repeat: false,
        }
    }

    #[test]
    fn a_detached_confirm_locks_the_main_window_and_a_tool_window_does_not() {
        // A tool window: keys still reach the keymap in the main window.
        let (mut app, main) = test_app();
        open(&mut app, ModalId::FindReplace);
        let _ = app.sync_detached_modals();
        assert!(app.locking_confirm_window().is_none());
        assert!(!app.has_locking_confirm_modal());
        assert!(
            app.route_key_for_test(main, &key("w")).is_some(),
            "tool window"
        );

        // A confirm: the main window takes no key, Esc still resolves.
        let (mut app, main) = test_app();
        open(&mut app, ModalId::RenameDialog);
        let _ = app.sync_detached_modals();
        let confirm = window_of(&app, ModalId::RenameDialog).expect("window");
        assert_eq!(app.locking_confirm_window(), Some(confirm));
        assert!(app.has_locking_confirm_modal(), "the native menu locks too");
        assert!(
            app.route_key_for_test(main, &key("w")).is_none(),
            "main is locked"
        );
        let esc = iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        };
        assert!(
            app.route_key_for_test(main, &esc).is_some(),
            "Esc dismisses"
        );
        // The dialog's own window keeps working.
        assert!(app.route_key_for_test(confirm, &key("w")).is_some());

        // Remove locks the same way.
        let (mut app, _main) = test_app();
        open(&mut app, ModalId::RemoveDialog);
        let _ = app.sync_detached_modals();
        assert!(app.locking_confirm_window().is_some());
    }

    #[test]
    fn clicking_the_locked_window_raises_the_confirm() {
        let (mut app, _main) = test_app();
        open(&mut app, ModalId::RenameDialog);
        let _ = app.sync_detached_modals();
        // A task, not Task::none(): it asks the OS to focus the dialog.
        let task = app.update(Message::Window(WindowMsg::LockedWindowClicked));
        assert!(task.units() > 0, "no gain_focus task");
    }
}
