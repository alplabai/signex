//! Keymap chord resolution.
//!
//! The keyboard subscription forwards each raw keystroke as
//! [`UiMsg::KeymapStroke`]. Resolution happens here, in `update`, where
//! `&mut self` is available — so the multi-stroke chord buffer lives in
//! [`UiState::keymap_pending_sequence`] instead of a process-global
//! static (sound across multiple windows, MVU-clean).
//!
//! A resolved command id is run through [`Signex::dispatch_command`] —
//! the Command Registry's dispatch entry point (#278) — so the
//! keyboard is a plain consumer of the registry rather than owning
//! bridge logic itself.

use iced::Task;

use super::super::*;
use crate::app::dispatch::input::InputTarget;
use crate::keymap::{KeyStroke, ShortcutContext};

impl Signex {
    /// Resolve one forwarded keystroke against the active keymap,
    /// accumulating multi-stroke chords in `keymap_pending_sequence`.
    ///
    /// A resolved command is dispatched through the normal
    /// [`Signex::dispatch_update`] path, so it behaves exactly as if the
    /// mapped message had been sent directly. A partial chord keeps the
    /// buffer and waits; a definite miss clears it (with a single-stroke
    /// restart retry so a stale prefix can't wedge later keys).
    pub(super) fn resolve_keymap_stroke(
        &mut self,
        window: Option<iced::window::Id>,
        stroke: KeyStroke,
    ) -> Task<Message> {
        let target = self.input_target(window);
        // #103 - a blocking modal owns the screen: a bare key must not reach
        // the canvas behind it. Dropped before it can start a chord, so no
        // prefix is left over to complete once the modal closes.
        //
        // #1378 - and a chord held from before the modal opened must never
        // fire behind it, whatever the stroke: it is dropped, not resolved.
        if target.paints_overlay_stack() && self.has_blocking_modal() {
            self.cancel_pending_chord();
            if is_single_key(&stroke) {
                return Task::none();
            }
        }
        self.begin_or_continue_chord(target);
        let contexts = self.shortcut_contexts(window);
        self.ui_state.keymap_pending_sequence.push(stroke.clone());
        self.ui_state.keymap_chord_wait = None;

        if let Some(task) = self.take_keymap_match(&contexts, window) {
            return task;
        }

        // Definite miss on the accumulated sequence. Restart from the
        // latest stroke alone so a stale prefix (e.g. an abandoned `P`)
        // can't swallow the next real shortcut.
        let mut tasks = Vec::new();
        if self.ui_state.keymap_pending_sequence.len() > 1 {
            let mut held = std::mem::take(&mut self.ui_state.keymap_pending_sequence);
            held.pop();
            // #1378: `held` may be a complete shortcut that was waiting
            // because a longer chord starts with it (`T A` / `T A E`).
            // A key that does not continue the longer chord fires it
            // now; `chord.rs` covers the other ways the wait ends
            // (timeout, Esc, pointer press, modal).
            let waiting = self.ui_state.active_keymap.lookup(&held, &contexts).command;
            if let Some(command) = waiting {
                tasks.push(self.dispatch_command(
                    &command,
                    crate::app::command::CommandArgs::none(),
                    window,
                ));
            }
            // The released command may itself have opened a blocking modal
            // (print preview): the restart stroke must not run behind it,
            // same rule as at the top of this function.
            if target.paints_overlay_stack() && self.has_blocking_modal() && is_single_key(&stroke)
            {
                self.cancel_pending_chord();
                return Task::batch(tasks);
            }
            self.ui_state.keymap_pending_sequence.push(stroke);
            if let Some(task) = self.take_keymap_match(&contexts, window) {
                tasks.push(task);
                return Task::batch(tasks);
            }
        }

        self.cancel_pending_chord();
        Task::batch(tasks)
    }

    /// Look the current pending sequence up in the active keymap.
    ///
    /// Returns `Some(task)` when the sequence is consumed — either it
    /// resolved to a command (dispatched), matched a binding with no
    /// dispatch arm yet (no-op), or is a live prefix of a longer chord
    /// (buffer kept, no-op). Returns `None` on a definite miss so the
    /// caller can apply its restart retry / fall through.
    fn take_keymap_match(
        &mut self,
        contexts: &[ShortcutContext],
        window: Option<iced::window::Id>,
    ) -> Option<Task<Message>> {
        let lookup = self
            .ui_state
            .active_keymap
            .lookup(&self.ui_state.keymap_pending_sequence, contexts);

        if lookup.command.is_some() && lookup.pending {
            // #1378: complete shortcut that is also the prefix of a longer
            // chord. Wait for the next key (or `CHORD_TIMEOUT`) instead of
            // firing now, or the longer chord could never be typed.
            self.arm_chord_wait(window);
            return Some(Task::none());
        }
        if let Some(command) = lookup.command.as_ref() {
            self.cancel_pending_chord();
            return Some(self.dispatch_command(
                command,
                crate::app::command::CommandArgs::none(),
                window,
            ));
        }
        if lookup.matched {
            self.cancel_pending_chord();
            return Some(Task::none());
        }
        if lookup.pending {
            // Prefix of a longer chord — keep the buffer and wait.
            return Some(Task::none());
        }
        None
    }

    /// Attach the pending chord buffer to `target`, dropping whatever
    /// prefix belonged to a different one (#559).
    ///
    /// `keymap_pending_sequence` is app-wide, so without this a sequence
    /// could begin in one window, complete in another, and resolve
    /// against the contexts of a third.
    fn begin_or_continue_chord(&mut self, target: InputTarget) {
        if self.ui_state.keymap_pending_target == Some(target) {
            return;
        }
        self.cancel_pending_chord();
        self.ui_state.keymap_pending_target = Some(target);
    }

    /// Which shortcut contexts are live for a stroke typed in `window`
    /// (#559).
    ///
    /// These used to be derived from `document_state.active_tab` — the
    /// MAIN window's tab — no matter where the stroke came from, so a
    /// shortcut pressed in an undocked schematic window resolved against
    /// the main window's footprint / symbol / library contexts.
    pub(crate) fn shortcut_contexts(
        &self,
        window: Option<iced::window::Id>,
    ) -> Vec<ShortcutContext> {
        let mut contexts = vec![ShortcutContext::Global];
        match self.input_target(window) {
            // Unchanged: the main window's contexts follow its active tab.
            InputTarget::Main => {
                if self.has_active_schematic() {
                    contexts.push(ShortcutContext::Schematic);
                }
                if self.has_active_pcb() {
                    contexts.push(ShortcutContext::Pcb);
                }
                match self
                    .document_state
                    .tabs
                    .get(self.document_state.active_tab)
                    .map(|tab| &tab.kind)
                {
                    Some(TabKind::FootprintEditor(_)) => {
                        contexts.push(ShortcutContext::Footprint);
                        contexts.push(ShortcutContext::Library);
                    }
                    Some(TabKind::SymbolEditor(_)) | Some(TabKind::LibraryBrowser(_)) => {
                        contexts.push(ShortcutContext::Library);
                    }
                    _ => {}
                }
            }

            // An undocked tab carries its own document. `view_center`
            // gates the footprint / symbol / library-browser surfaces on
            // `is_main`, so such a window can only ever host a schematic
            // or PCB canvas — `Footprint` and `Library` are unreachable
            // there by construction, not by omission.
            InputTarget::UndockedTab => {
                let has_schematic = window
                    .and_then(|id| self.document_state.engine_for_window(id, &self.ui_state))
                    .is_some();
                if has_schematic {
                    contexts.push(ShortcutContext::Schematic);
                } else if self.has_active_pcb() {
                    contexts.push(ShortcutContext::Pcb);
                }
            }

            // No document surface, so nothing document-scoped may fire:
            // the command would act on a canvas in a window the user is
            // not looking at. Global commands still work.
            InputTarget::DetachedModal(_)
            | InputTarget::DetachedPanel
            | InputTarget::ComponentEditor => {}
        }
        contexts
    }
}

/// A stroke with no Ctrl / Alt / Cmd held. Shift alone still types a
/// character, so it stays a single key.
fn is_single_key(stroke: &KeyStroke) -> bool {
    let m = &stroke.modifiers;
    !(m.ctrl || m.alt || m.command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::{ModalId, WindowKind};
    use crate::app::{Signex, TabInfo, TabKind};

    fn app_with_footprint_tab() -> Signex {
        let (mut app, _boot) = Signex::new();
        let path = std::path::PathBuf::from("/tmp/part.snxfpt");
        app.document_state.tabs.push(TabInfo {
            title: "part".to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: None,
            kind: TabKind::FootprintEditor(path),
        });
        app.document_state.active_tab = 0;
        app
    }

    fn open_window(app: &mut Signex, kind: WindowKind) -> iced::window::Id {
        let id = iced::window::Id::unique();
        app.ui_state.windows.insert(id, kind);
        id
    }

    fn undocked(app: &mut Signex) -> iced::window::Id {
        open_window(
            app,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("/tmp/sheet.snxsch"),
                title: "sheet".to_string(),
            },
        )
    }

    /// The #559 bug, stated as a contrast: the SAME app state resolves to
    /// different contexts depending on which window typed the stroke.
    /// Before this, both answers were the main window's.
    #[test]
    fn an_undocked_window_does_not_inherit_the_main_windows_editor_contexts() {
        let mut app = app_with_footprint_tab();
        let tab = undocked(&mut app);

        let main = app.shortcut_contexts(app.ui_state.main_window_id);
        assert!(
            main.contains(&ShortcutContext::Footprint) && main.contains(&ShortcutContext::Library),
            "the main window really is showing a footprint editor: {main:?}"
        );

        let elsewhere = app.shortcut_contexts(Some(tab));
        assert!(
            !elsewhere.contains(&ShortcutContext::Footprint)
                && !elsewhere.contains(&ShortcutContext::Library),
            "an undocked window cannot host those surfaces (`view_center` gates \
             them on `is_main`), so their contexts must not be live: {elsewhere:?}"
        );
    }

    #[test]
    fn windows_that_paint_no_document_get_global_only() {
        let mut app = app_with_footprint_tab();
        let modal = open_window(&mut app, WindowKind::DetachedModal(ModalId::ErcDialog));
        let panel = open_window(
            &mut app,
            WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
        );
        let editor = open_window(
            &mut app,
            WindowKind::ComponentEditor {
                library_path: std::path::PathBuf::from("/tmp/parts.snxlib"),
                table: "Resistors".to_string(),
                row_id: signex_library::RowId::new(),
            },
        );

        for window in [modal, panel, editor] {
            assert_eq!(
                app.shortcut_contexts(Some(window)),
                vec![ShortcutContext::Global],
                "a document-scoped command here would act on a canvas in \
                 another window"
            );
        }
    }

    #[test]
    fn a_synthesised_stroke_resolves_as_the_main_window() {
        let app = app_with_footprint_tab();
        assert_eq!(
            app.shortcut_contexts(None),
            app.shortcut_contexts(app.ui_state.main_window_id),
            "`cancel_current_tool` and friends name no window and must not \
             lose the main window's contexts"
        );
    }

    /// The chord buffer is app-wide, so a prefix begun in one window must
    /// not be completed in another.
    ///
    /// Tested against `begin_or_continue_chord` rather than through
    /// `resolve_keymap_stroke`: the resolver clears the buffer again on a
    /// definite miss, which would leave it empty either way and make the
    /// assertion vacuous.
    #[test]
    fn a_chord_prefix_does_not_survive_a_change_of_window() {
        let mut app = app_with_footprint_tab();
        let stroke = crate::keymap::KeyStroke::from_iced(
            &iced::keyboard::Key::Character("p".into()),
            iced::keyboard::Modifiers::default(),
        )
        .expect("`p` is expressible");

        app.ui_state.keymap_pending_sequence = vec![stroke.clone()];
        app.ui_state.keymap_pending_target = Some(InputTarget::Main);

        app.begin_or_continue_chord(InputTarget::Main);
        assert_eq!(
            app.ui_state.keymap_pending_sequence.len(),
            1,
            "another stroke from the same window continues the chord"
        );

        app.begin_or_continue_chord(InputTarget::UndockedTab);
        assert!(
            app.ui_state.keymap_pending_sequence.is_empty(),
            "a stroke from a different window drops the old prefix"
        );
        assert_eq!(
            app.ui_state.keymap_pending_target,
            Some(InputTarget::UndockedTab),
            "and the buffer now belongs to the window that typed into it"
        );
    }

    fn f9() -> crate::keymap::KeyStroke {
        crate::keymap::KeyStroke::from_iced(
            &iced::keyboard::Key::Named(iced::keyboard::key::Named::F9),
            iced::keyboard::Modifiers::default(),
        )
        .expect("F9 is expressible")
    }

    fn ctrl_q() -> crate::keymap::KeyStroke {
        crate::keymap::KeyStroke::from_iced(
            &iced::keyboard::Key::Character("q".into()),
            iced::keyboard::Modifiers::CTRL,
        )
        .expect("Ctrl+Q is expressible")
    }

    /// Control for the gate below: with no modal open an unmodified
    /// stroke runs its command (F9 is `toggle_auto_focus`).
    #[test]
    fn a_plain_stroke_fires_when_no_modal_is_open() {
        let mut app = app_with_footprint_tab();
        let before = app.ui_state.auto_focus;

        let _ = app.resolve_keymap_stroke(None, f9());

        assert_ne!(app.ui_state.auto_focus, before);
    }

    /// #103 - a single-key stroke must not reach the canvas behind a
    /// blocking modal. The stroke is dropped, and does not leave a chord
    /// prefix behind to complete after the modal closes.
    #[test]
    fn a_plain_stroke_is_dropped_while_a_blocking_modal_is_open() {
        let mut app = app_with_footprint_tab();
        app.document_state.error_notice = Some(crate::app::state::ErrorNotice::export("disk full"));
        let before = app.ui_state.auto_focus;

        let _ = app.resolve_keymap_stroke(None, f9());

        assert_eq!(app.ui_state.auto_focus, before);
        assert!(app.ui_state.keymap_pending_sequence.is_empty());
    }

    /// The gate is for single keys only: a Ctrl / Alt / Cmd chord is
    /// never held back by it, as before.
    #[test]
    fn only_unmodified_strokes_count_as_single_keys() {
        assert!(is_single_key(&f9()));
        assert!(!is_single_key(&ctrl_q()));
        let alt = crate::keymap::KeyStroke::from_iced(
            &iced::keyboard::Key::Character("1".into()),
            iced::keyboard::Modifiers::ALT,
        )
        .expect("Alt+1 is expressible");
        assert!(!is_single_key(&alt));
    }
}
