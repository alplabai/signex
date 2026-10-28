//! Keeping each tool or confirm dialog in step with its own OS window.
//! Design and the lock policy: docs/DETACHED_MODALS.md.

use iced::Task;

use crate::app::Message;
use crate::app::state::{ModalId, Signex, WindowKind};

/// Dialogs whose window follows their open state, whatever code flips it.
const FOLLOWED: [ModalId; 15] = [
    ModalId::FindReplace,
    ModalId::RenameDialog,
    ModalId::RemoveDialog,
    ModalId::EnableVersionControl,
    ModalId::GridProperties,
    ModalId::SelectionFilterCustom,
    ModalId::About,
    ModalId::KeyboardShortcuts,
    ModalId::PassiveCalculator,
    ModalId::CreateChildSheet,
    ModalId::ErrorNotice,
    ModalId::NetlistIncompletePrompt,
    ModalId::AutosaveRecovery,
    ModalId::ProjectCloseConfirm,
    ModalId::AppQuitConfirm,
];

/// Confirm-class dialogs among the detached ones: while one is open the
/// main window takes no input.
const LOCKING: [ModalId; 9] = [
    ModalId::RenameDialog,
    ModalId::RemoveDialog,
    ModalId::AnnotateResetConfirm,
    ModalId::CreateChildSheet,
    ModalId::ErrorNotice,
    ModalId::NetlistIncompletePrompt,
    ModalId::AutosaveRecovery,
    ModalId::ProjectCloseConfirm,
    ModalId::AppQuitConfirm,
];

/// Take keyboard focus away from whatever text input holds it, in every window.
fn unfocus_text_inputs() -> Task<Message> {
    iced_runtime::task::effect(iced_runtime::Action::widget(
        iced::advanced::widget::operation::focusable::unfocus::<()>(),
    ))
}

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
            ModalId::ErrorNotice => self.document_state.error_notice.is_some(),
            ModalId::NetlistIncompletePrompt => {
                self.document_state.netlist_incomplete_prompt.is_some()
            }
            ModalId::AutosaveRecovery => self.document_state.autosave.current_offer().is_some(),
            ModalId::ProjectCloseConfirm => ui.project_close_confirm.is_some(),
            ModalId::AppQuitConfirm => ui.app_quit_confirm.is_some(),
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
        // A text field that had keyboard focus keeps taking typing under the
        // lock, so release it once when a confirm dialog locks the windows.
        let locked = self.locking_confirm_window().is_some();
        if locked && !self.ui_state.lock_unfocused {
            tasks.push(unfocus_text_inputs());
        }
        self.ui_state.lock_unfocused = locked;
        (!tasks.is_empty()).then(|| Task::batch(tasks))
    }

    /// The window `modal` lives in, if it is detached.
    pub(crate) fn detached_window_of(&self, modal: ModalId) -> Option<iced::window::Id> {
        self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, WindowKind::DetachedModal(m) if *m == modal).then_some(*id)
        })
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
            ModalId::ErrorNotice => {
                app.document_state.error_notice =
                    Some(crate::app::state::ErrorNotice::create("it failed"))
            }
            ModalId::AutosaveRecovery => {
                app.document_state
                    .autosave
                    .offer(crate::autosave::RecoveryOffer {
                        path: "p.snxsch".into(),
                        age_secs: 90,
                        older_than_file: false,
                    })
            }
            ModalId::ProjectCloseConfirm => {
                let project_id = app.document_state.mint_project_id();
                app.ui_state.project_close_confirm = Some(crate::app::ProjectCloseConfirmState {
                    project_id,
                    project_name: "p".into(),
                    dirty_paths: vec!["/p/a.snxsch".into()],
                    dirty_rows: Vec::new(),
                    kept_paths: Vec::new(),
                    sheet_refs: Default::default(),
                })
            }
            ModalId::AppQuitConfirm => {
                app.ui_state.app_quit_confirm = Some(crate::app::AppQuitConfirmState {
                    dirty_paths: vec!["/p/a.snxsch".into()],
                    dirty_rows: Vec::new(),
                })
            }
            ModalId::NetlistIncompletePrompt => panic!("needs an export context"),
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

    /// Every followed dialog the fixtures can open (the netlist prompt needs a
    /// whole export context and has its own test).
    fn testable() -> impl Iterator<Item = ModalId> {
        FOLLOWED
            .into_iter()
            .filter(|m| *m != ModalId::NetlistIncompletePrompt)
    }

    fn window_of(app: &Signex, modal: ModalId) -> Option<iced::window::Id> {
        app.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, WindowKind::DetachedModal(m) if *m == modal).then_some(*id)
        })
    }

    #[test]
    fn opening_each_followed_dialog_gives_it_a_window_and_closing_removes_it() {
        for modal in testable() {
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
        for modal in testable() {
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
        for modal in testable() {
            let (mut app, _main) = test_app();
            open(&mut app, modal);
            let _ = app.sync_detached_modals();
            let id = window_of(&app, modal).expect("window");
            let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(id)));
            // A recovery offer is only ever answered by the user: its window
            // vanishing keeps it (the sync shows it again).
            let expected = modal == ModalId::AutosaveRecovery;
            assert_eq!(app.followed_modal_open(modal), Some(expected), "{modal:?}");
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

    #[test]
    fn a_quit_confirm_stays_answerable_while_a_detached_confirm_is_open() {
        let (mut app, main) = test_app();
        open(&mut app, ModalId::RenameDialog);
        let _ = app.sync_detached_modals();
        assert!(app.locking_confirm_window().is_some());
        app.document_state.dirty_paths.insert("/p/a.snxsch".into());
        let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
        assert!(
            app.ui_state.app_quit_confirm.is_some(),
            "the quit confirm opened"
        );
        // It has its own window, so the lock layer cannot hide it...
        let window = app
            .detached_window_of(ModalId::AppQuitConfirm)
            .expect("its window");
        let texts = lay_out_texts(app.view(window), iced::Size::new(540.0, 380.0));
        assert!(
            texts.iter().any(|t| t.text.contains("unsaved item")),
            "the quit confirm shows its own window"
        );
        // ...and Esc, which the lock lets through, cancels it.
        let _ = app.update(Message::Input(InputMsg::EscapePressed {
            window: Some(main),
        }));
        assert!(app.ui_state.app_quit_confirm.is_none());
    }

    #[test]
    fn closing_enable_version_control_while_git_runs_keeps_the_dialog() {
        let (mut app, _main) = test_app();
        open(&mut app, ModalId::EnableVersionControl);
        app.ui_state
            .enable_version_control
            .as_mut()
            .expect("open")
            .working = true;
        let _ = app.sync_detached_modals();
        let id = window_of(&app, ModalId::EnableVersionControl).expect("window");
        let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(id)));
        assert!(
            app.ui_state.enable_version_control.is_some(),
            "state is not dropped"
        );
        assert!(
            window_of(&app, ModalId::EnableVersionControl).is_some(),
            "window stays"
        );
    }

    #[test]
    fn the_lock_asks_the_runtime_to_release_text_focus_once() {
        let (mut app, _main) = test_app();
        open(&mut app, ModalId::RenameDialog);
        assert!(!app.ui_state.lock_unfocused);
        let first = app.sync_detached_modals().expect("tasks");
        assert!(app.ui_state.lock_unfocused);
        assert!(first.units() >= 2, "window open + unfocus");
        let second = app.sync_detached_modals();
        assert!(second.is_none(), "not repeated while the lock holds");
    }
}
