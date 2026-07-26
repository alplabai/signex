//! `Signex::subscription` — keyboard / window / tick event wiring.
//! Split from `app/bootstrap.rs` as pure code motion.

use super::super::*;

use crate::keymap::KeyStroke;
use iced::Subscription;

/// Which overlays are open, as the keyboard subscription sees them.
///
/// A named struct rather than the positional tuple this used to be, for
/// two reasons. The flag list had reached twelve entries, which is where
/// std's tuple impls stop — `Subscription::with` requires
/// `Hash + Clone + Send + Sync + 'static`, and neither `Hash` nor
/// `Clone` is implemented for a 13-tuple, so a thirteenth modal could
/// not be added at all. And with twelve same-typed `bool`s, swapping two
/// of them in either the `with(...)` list or the destructuring pattern
/// silently routed Esc to the wrong modal, with nothing to catch it.
///
/// `Hash` is load-bearing, not decorative: iced re-keys the subscription
/// from it, so every flag must stay inside the hash for a modal opening
/// to be observed.
#[derive(Clone, Copy, Default, Hash)]
struct OpenOverlays {
    find_replace_open: bool,
    palette_open: bool,
    kbd_shortcuts_open: bool,
    first_run_tour_open: bool,
    prefs_open: bool,
    annotate_open: bool,
    erc_open: bool,
    rename_open: bool,
    remove_open: bool,
    enable_vc_open: bool,
    library_create_options_open: bool,
    keymap_recorder_open: bool,
    passive_calculator_open: bool,
    // #514 — the rest of the Esc ladder. Each closes exactly one gap that
    // used to fall through to the `Tool::Select` reset behind the modal.
    annotate_reset_confirm_open: bool,
    app_quit_confirm_open: bool,
    project_close_confirm_open: bool,
    project_options_open: bool,
    grid_properties_open: bool,
    selection_filter_custom_open: bool,
    library_picker_open: bool,
    library_document_options_open: bool,
    library_updates_open: bool,
    library_primitive_picker_open: bool,
    close_library_confirm_open: bool,
    /// `None` while no library-recovery dialog is open. `RecoveryDialog`
    /// (`crate::library::recovery`) itself carries a `PathBuf` (and, for
    /// the broken-binding case, a `String`/`Uuid`), so it can't be
    /// embedded directly in this `Copy + Hash` struct — this discriminant
    /// is the cheap stand-in that lets Esc still pick the right one of
    /// the three dialogs' own Cancel-choice messages.
    recovery_kind: Option<RecoveryKind>,
    export_error_open: bool,
    netlist_incomplete_prompt_open: bool,
    print_preview_open: bool,
    bom_preview_open: bool,
    net_color_custom_open: bool,
}

/// Which of the three library-recovery flows is open — see
/// `OpenOverlays::recovery_kind`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum RecoveryKind {
    LibraryMissing,
    GitMissing,
    BrokenBinding,
}

impl OpenOverlays {
    /// The Esc ladder: which modal a bare Esc dismisses. `None` means no
    /// modal claimed the key, so the caller falls through to
    /// `Message::EscapePressed` (the tool reset).
    ///
    /// This quantifies over *fields of `OpenOverlays`*, not over every
    /// modal in the app — that gap is real (see `every_modal_claims_escape`
    /// at the foot of this file) and is exactly what let #511 (the passive
    /// calculator) and #514 (~14 more modals) ship with no rung. A field
    /// that IS here and has no rung is a real bug — Esc falls through and
    /// silently resets the active canvas tool *behind* the open modal —
    /// which is why this is a plain function over a `Copy` struct instead
    /// of arms buried in the subscription closure:
    /// `every_ladder_field_claims_escape` fails if a new field is added
    /// without one.
    ///
    /// Order is **the exact reverse of paint order**: rung N outranks rung
    /// N+1 iff modal N's layer is pushed LATER than modal N+1's in
    /// `collect_overlays` (`app/view/mod.rs:750-807`, plus the intra-vec
    /// push order of `simple_dialogs_overlay` and `detachable_dialogs_overlay`
    /// in `app/view/overlays/modals.rs`) — i.e. whatever is drawn on top of
    /// everything else also wins Esc. This is a derivation, not a
    /// hand-tuned priority list: if you change the push order in either of
    /// those three places, change this ladder to match, in the same
    /// direction. Do not reorder rungs below for any other reason — every
    /// ordering bug found across #514's review rounds was exactly this
    /// rule violated in one spot, and patching pairs one at a time kept
    /// reintroducing new violations elsewhere.
    ///
    /// The "topmost painted wins" premise above only holds in the
    /// no-blocking regime, which is why the function opens with a guard
    /// instead of a plain top-to-bottom read: `export_error_open` /
    /// `netlist_incomplete_prompt_open` / `print_preview_open` /
    /// `net_color_custom_open` are the `has_blocking_modal` members
    /// (`view/overlays/bars.rs:20-25`) — when one is set,
    /// `collect_overlays` returns early at `:758-759` and nothing from
    /// `:764` onward paints. But their `bool`s are NOT exclusive with
    /// rungs 1-23's — Alt+F4 / native window close sets
    /// `app_quit_confirm` unconditionally
    /// (`handle_app_quit_requested`, no `has_blocking_modal` check
    /// anywhere on that path) and keymap strokes are not modal-gated
    /// either (`dispatch/keymap.rs`'s `shortcut_contexts` has no modal
    /// awareness), so e.g. an export failure followed by Alt+F4, or by
    /// Ctrl+G, really does leave `app_quit_confirm_open` / `grid_properties_open`
    /// set while only the export-error card paints. Without the guard,
    /// Esc would then resolve to one of rungs 1-23 — cancelling an
    /// invisible dialog while the visible blocking modal stays open. The
    /// guard forces resolution within the actually-visible five, ranked
    /// `net_color_custom_open` first, then `bom_preview_open`, then
    /// `print_preview_open`, then `netlist_incomplete_prompt_open`, then
    /// `export_error_open` last — FIVE, not four, because
    /// `bom_preview_open` also paints in that same early block (`:753`)
    /// without itself being a `has_blocking_modal` member.
    ///
    /// The command palette and the keymap chord recorder are absent on
    /// purpose: both swallow keyboard input wholesale before the Esc
    /// ladder is reached, and are handled in the closure. Also absent: the
    /// Edit Component Details modal (`edit_row_modal_overlay`), gated dead
    /// behind `EDIT_MODAL_ENABLED = false` — it needs a rung the day it's
    /// re-enabled, not before — and the per-browser delete-confirm modal
    /// (`delete_confirm_overlay`), whose Cancel message
    /// (`LibraryMessage::BrowserDeleteRowCancel`) needs the owning
    /// `library_path`, which a `Copy` struct like this one has no way to
    /// carry.
    ///
    /// One rung is a KNOWN behaviour change from trunk, not an accident:
    /// `find_replace_open` now ranks BELOW `passive_calculator_open`
    /// (`find_replace_overlay` paints at `:787`, `passive_calculator_overlay`
    /// at `:789` — later, i.e. on top). Trunk had this the other way
    /// around; that was a latent bug (find_replace won Esc despite painting
    /// underneath the passive calculator), fixed here as a side effect of
    /// deriving the whole ladder from one rule instead of hand-ordering it.
    fn escape_message(self) -> Option<Message> {
        // Guard: whenever any of the four `has_blocking_modal` members is
        // set, resolve ONLY within the actually-visible five (the four
        // plus `bom_preview_open`, which shares their early paint slot
        // without being one of them) and never fall through to rungs
        // 1-23 — see the doc comment above for why their `bool`s are
        // reachable together with a rung 1-23 flag even though only one
        // of the two ever paints.
        if self.export_error_open
            || self.netlist_incomplete_prompt_open
            || self.print_preview_open
            || self.net_color_custom_open
        {
            if self.net_color_custom_open {
                return Some(Message::NetColor(NetColorMsg::CustomShow(false)));
            }
            if self.bom_preview_open {
                return Some(Message::BomPreview(BomPreviewMsg::Close));
            }
            if self.print_preview_open {
                return Some(Message::PrintPreview(PrintPreviewMsg::Close));
            }
            if self.netlist_incomplete_prompt_open {
                // Cancel path only — the prompt's other action is "Export
                // anyway (incomplete)", which Esc must never trigger.
                return Some(Message::Export(ExportMsg::NetlistCancelIncomplete));
            }
            debug_assert!(
                self.export_error_open,
                "has_blocking_modal guard entered but none of its four \
                 members were set"
            );
            return Some(Message::Export(ExportMsg::DismissError));
        }
        if self.library_updates_open {
            return Some(Message::Library(
                crate::library::messages::LibraryMessage::LibraryUpdatesCancel,
            ));
        }
        if let Some(kind) = self.recovery_kind {
            return Some(match kind {
                RecoveryKind::LibraryMissing => Message::Library(
                    crate::library::messages::LibraryMessage::RecoveryLibraryMissing(
                        crate::library::recovery::LibraryMissingChoice::Cancel,
                    ),
                ),
                RecoveryKind::GitMissing => Message::Library(
                    crate::library::messages::LibraryMessage::RecoveryGitMissing(
                        crate::library::recovery::GitMissingChoice::Cancel,
                    ),
                ),
                RecoveryKind::BrokenBinding => Message::Library(
                    crate::library::messages::LibraryMessage::RecoveryBrokenBinding(
                        crate::library::recovery::BrokenBindingChoice::Cancel,
                    ),
                ),
            });
        }
        if self.close_library_confirm_open {
            return Some(Message::Library(
                crate::library::messages::LibraryMessage::CloseLibraryConfirm(
                    crate::library::messages::CloseLibraryChoice::Cancel,
                ),
            ));
        }
        if self.library_create_options_open {
            return Some(Message::Library(
                crate::library::messages::LibraryMessage::LibraryCreateOptionsCancel,
            ));
        }
        if self.library_document_options_open {
            return Some(Message::Library(
                crate::library::messages::LibraryMessage::DocumentOptionsCancel,
            ));
        }
        if self.library_primitive_picker_open {
            return Some(Message::Library(
                crate::library::messages::LibraryMessage::PrimitivePicker(
                    crate::library::messages::PrimitivePickerMsg::Cancel,
                ),
            ));
        }
        if self.library_picker_open {
            return Some(Message::Library(
                crate::library::messages::LibraryMessage::ClosePicker,
            ));
        }
        if self.erc_open {
            return Some(Message::Erc(ErcMsg::CloseDialog));
        }
        // Must outrank `annotate_open` below — it's a child confirm of the
        // annotate dialog, with its own Cancel message
        // (`CloseResetConfirm`, not `CloseDialog`), and it also paints
        // later than `annotate_open` inside `detachable_dialogs_overlay`
        // (`modals.rs:159` vs `:156`). Checking the parent first would
        // close the annotate dialog out from under the confirm and orphan
        // it.
        if self.annotate_reset_confirm_open {
            return Some(Message::Annotate(AnnotateMsg::CloseResetConfirm));
        }
        if self.annotate_open {
            return Some(Message::Annotate(AnnotateMsg::CloseDialog));
        }
        if self.selection_filter_custom_open {
            return Some(Message::SelectionFilter(SelectionFilterMsg::CloseCustom));
        }
        if self.grid_properties_open {
            return Some(Message::GridProperties(GridPropertiesMsg::Close));
        }
        if self.enable_vc_open {
            return Some(Message::EnableVersionControl(
                EnableVersionControlMsg::Close,
            ));
        }
        if self.project_options_open {
            return Some(Message::Project(ProjectMsg::CloseOptions));
        }
        if self.app_quit_confirm_open {
            return Some(Message::Project(ProjectMsg::AppQuitConfirm(
                ProjectCloseChoice::Cancel,
            )));
        }
        if self.project_close_confirm_open {
            return Some(Message::Project(ProjectMsg::CloseConfirm(
                ProjectCloseChoice::Cancel,
            )));
        }
        if self.remove_open {
            return Some(Message::Remove(RemoveMsg::Close));
        }
        if self.rename_open {
            return Some(Message::Rename(RenameMsg::Close));
        }
        if self.first_run_tour_open {
            return Some(Message::Overlay(OverlayMsg::DismissFirstRunTour));
        }
        if self.passive_calculator_open {
            return Some(Message::Overlay(OverlayMsg::ClosePassiveCalculator));
        }
        if self.kbd_shortcuts_open {
            return Some(Message::Overlay(OverlayMsg::CloseKeyboardShortcuts));
        }
        if self.find_replace_open {
            return Some(Message::FindReplaceMsg(
                crate::find_replace::FindReplaceMsg::Close,
            ));
        }
        if self.prefs_open {
            return Some(Message::Preferences(PreferencesMsg::Close));
        }
        // Reached only when none of the four `has_blocking_modal` members
        // are set (the guard above would have returned otherwise) —
        // `bom_preview_open` is not itself one of them, so it can still be
        // independently true here and keeps its own rung at its derived
        // position.
        if self.bom_preview_open {
            return Some(Message::BomPreview(BomPreviewMsg::Close));
        }
        None
    }
}

impl Signex {
    pub fn subscription(&self) -> Subscription<Message> {
        use iced::keyboard;

        // True while `modal` is showing in its own detached OS window
        // rather than as an in-window overlay. Several builders skip
        // painting the in-window card in that case (`view/overlays/bars.rs`,
        // `view/overlays/modals.rs`) — the Esc ladder must skip the
        // corresponding rung for the same reason, or Esc would dismiss a
        // dialog the user can't even see, while the real (detached) window
        // stays open untouched.
        let modal_detached = |modal: crate::app::state::ModalId| -> bool {
            self.ui_state.windows.values().any(|kind| {
                matches!(kind, crate::app::state::WindowKind::DetachedModal(m) if *m == modal)
            })
        };

        let kbd = keyboard::listen()
            .with(OpenOverlays {
                find_replace_open: self.ui_state.find_replace.open,
                palette_open: self.ui_state.command_palette.open,
                kbd_shortcuts_open: self.ui_state.keyboard_shortcuts_open,
                first_run_tour_open: self.ui_state.first_run_tour_open,
                prefs_open: self.ui_state.preferences_open
                    && !modal_detached(crate::app::state::ModalId::Preferences),
                annotate_open: self.ui_state.annotate_dialog_open
                    && !modal_detached(crate::app::state::ModalId::AnnotateDialog),
                erc_open: self.ui_state.erc_dialog_open
                    && !modal_detached(crate::app::state::ModalId::ErcDialog),
                rename_open: self.ui_state.rename_dialog.is_some(),
                remove_open: self.ui_state.remove_dialog.is_some(),
                enable_vc_open: self.ui_state.enable_version_control.is_some(),
                library_create_options_open: self.library.create_options.is_some(),
                keymap_recorder_open: self.ui_state.preferences_keymap_recorder.is_some(),
                passive_calculator_open: self.ui_state.passive_calculator_open,
                annotate_reset_confirm_open: self.ui_state.annotate_reset_confirm
                    && !modal_detached(crate::app::state::ModalId::AnnotateResetConfirm),
                app_quit_confirm_open: self.ui_state.app_quit_confirm.is_some(),
                project_close_confirm_open: self.ui_state.project_close_confirm.is_some(),
                project_options_open: self.ui_state.project_options.is_some(),
                grid_properties_open: self.ui_state.grid_properties.is_some(),
                selection_filter_custom_open: self.ui_state.selection_filter_custom.is_some(),
                library_picker_open: self.library.picker.is_some(),
                library_document_options_open: self.library.document_options.is_some(),
                library_updates_open: self.library.library_updates.is_some(),
                library_primitive_picker_open: self.library.primitive_picker.is_some(),
                close_library_confirm_open: self.library.close_library_confirm.is_some(),
                recovery_kind: self
                    .library
                    .recovery
                    .as_ref()
                    .map(|recovery| match recovery {
                        crate::library::recovery::RecoveryDialog::LibraryMissing { .. } => {
                            RecoveryKind::LibraryMissing
                        }
                        crate::library::recovery::RecoveryDialog::GitMissing { .. } => {
                            RecoveryKind::GitMissing
                        }
                        crate::library::recovery::RecoveryDialog::BrokenPrimitiveBinding {
                            ..
                        } => RecoveryKind::BrokenBinding,
                    }),
                export_error_open: self.document_state.export_error.is_some(),
                netlist_incomplete_prompt_open: self
                    .document_state
                    .netlist_incomplete_prompt
                    .is_some(),
                print_preview_open: self.document_state.preview.is_some()
                    && !modal_detached(crate::app::state::ModalId::PrintPreview),
                bom_preview_open: self.document_state.bom_preview.is_some()
                    && !modal_detached(crate::app::state::ModalId::BomPreview),
                net_color_custom_open: self.ui_state.net_color_custom.show,
            })
            .map(
                |(
                    overlays @ OpenOverlays {
                        palette_open,
                        kbd_shortcuts_open,
                        keymap_recorder_open,
                        ..
                    },
                    event,
                )| match event {
                    // Chord recorder open (Preferences ▸ Keyboard
                    // Shortcuts): held modifiers drive the live
                    // "Ctrl+…" hint before a key lands.
                    keyboard::Event::ModifiersChanged(modifiers) if keymap_recorder_open => {
                        Message::Preferences(PreferencesMsg::Inner(
                            crate::preferences::PrefMsg::KeymapRecorderModifiersChanged(
                                crate::keymap::Modifiers::from_iced(modifiers),
                            ),
                        ))
                    }
                    keyboard::Event::KeyPressed {
                        key, modifiers: m, ..
                    } => {
                        // While the recorder is open, every raw stroke
                        // is captured for the binding under edit — it
                        // must NOT reach the live keymap resolver, or
                        // recording a shortcut would also fire it. The
                        // pending chord buffer is left untouched (it is
                        // only advanced by the resolver, which we skip).
                        if keymap_recorder_open {
                            return KeyStroke::from_iced(&key, m)
                                .map(|stroke| {
                                    Message::Preferences(PreferencesMsg::Inner(
                                        crate::preferences::PrefMsg::KeymapRecorderKeyPressed(
                                            stroke,
                                        ),
                                    ))
                                })
                                .unwrap_or(Message::Noop);
                        }
                        // Command palette captures most input while open so
                        // typing into the search field doesn't fire tool
                        // shortcuts (`p`, `w`, `l`, …). Only navigation
                        // and dismiss keys leak through.
                        if palette_open {
                            return match (key.as_ref(), m) {
                                (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                                    Message::CommandPalette(CommandPaletteMsg::Close)
                                }
                                (keyboard::Key::Named(keyboard::key::Named::ArrowDown), _) => {
                                    Message::CommandPalette(CommandPaletteMsg::MoveSelection(1))
                                }
                                (keyboard::Key::Named(keyboard::key::Named::ArrowUp), _) => {
                                    Message::CommandPalette(CommandPaletteMsg::MoveSelection(-1))
                                }
                                // Toggle: Ctrl+Shift+P while open closes.
                                (keyboard::Key::Character(c), m)
                                    if c.eq_ignore_ascii_case("p") && m.command() && m.shift() =>
                                {
                                    Message::CommandPalette(CommandPaletteMsg::Close)
                                }
                                _ => Message::Noop,
                            };
                        }
                        // v0.19 keymap migration: per-key tool / command
                        // shortcuts now come from the active profile (see
                        // `dispatch::keymap`). Only keys that can't be
                        // profile-driven stay hardcoded here: the modal-close
                        // Esc ladder and F1 (they depend on which modal is
                        // open — subscription state, not the profile), and
                        // the Ctrl/Alt+1-8 selection-memory chords (they
                        // carry the digit as data the profile can't express).
                        // Everything else is forwarded to the keymap resolver
                        // in `update`.
                        match (key.as_ref(), m) {
                            // Esc closes the deepest open modal first
                            // (UX §1.3) — the ladder lives in
                            // `OpenOverlays::escape_message`, which is where
                            // a new modal's rung belongs. `None` means no
                            // modal claimed the key, so it falls through to
                            // the dispatcher (v0.15): Esc resets the
                            // footprint editor's tool state when a `.snxfpt`
                            // tab is active, and falls back to the schematic
                            // `Tool::Select` reset otherwise.
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                                overlays.escape_message().unwrap_or(Message::EscapePressed)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::F1), _) => {
                                // F1 toggles: open if closed, close if open.
                                if kbd_shortcuts_open {
                                    Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)
                                } else {
                                    Message::Menu(MenuMessage::OpenKeyboardShortcuts)
                                }
                            }
                            // Ctrl+1-8 store selection memory, Alt+1-8 recall
                            // selection memory. These carry the digit as data
                            // the profile format can't express, so they stay
                            // hardcoded. The `is_some` guard is load-bearing
                            // (#127): without it this arm matched EVERY
                            // Ctrl/Alt chord and returned Noop, which would
                            // shadow Ctrl+C/X/V/D before they reach the keymap
                            // resolver below.
                            (keyboard::Key::Character(c), m)
                                if m.command()
                                    && !m.alt()
                                    && super::selection_slot_from_key(c).is_some() =>
                            {
                                match super::selection_slot_from_key(c) {
                                    Some(slot) => Message::Selection(
                                        selection_request::SelectionRequest::StoreSlot { slot },
                                    ),
                                    _ => Message::Noop,
                                }
                            }
                            (keyboard::Key::Character(c), m)
                                if m.alt()
                                    && !m.command()
                                    && super::selection_slot_from_key(c).is_some() =>
                            {
                                match super::selection_slot_from_key(c) {
                                    Some(slot) => Message::Selection(
                                        selection_request::SelectionRequest::RecallSlot { slot },
                                    ),
                                    _ => Message::Noop,
                                }
                            }
                            // Everything else routes through the active
                            // keymap: forward the raw stroke, resolved in
                            // `update` where the multi-stroke chord buffer
                            // lives in `UiState` (sound across windows). A
                            // stroke iced can't express as a `KeyStroke`
                            // (e.g. a bare modifier press) is ignored here.
                            _ => KeyStroke::from_iced(&key, m)
                                .map(|stroke| Message::Ui(UiMsg::KeymapStroke(stroke)))
                                .unwrap_or(Message::Noop),
                        }
                    }
                    _ => Message::Noop,
                },
            );

        // Mouse events for drag-to-resize/floating-drag.
        // Subscribing to cursor move only while dragging avoids per-frame
        // app updates when idle, which noticeably hurts smoothness on macOS.
        let drag_active = self.interaction_state.dragging.is_some()
            || self.document_state.dock.tab_drag.is_some()
            || self.ui_state.modal_dragging.is_some()
            || self.ui_state.tab_dragging.is_some()
            || self
                .document_state
                .dock
                .floating
                .iter()
                .any(|fp| fp.dragging);
        let modal_drag_active = self.ui_state.modal_dragging.is_some();
        let mouse_sub = if modal_drag_active {
            // Modal drag takes priority — release ends the modal drag
            // specifically (not the generic DragEnd).
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Message::Overlay(OverlayMsg::ModalDragEnd),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::Noop,
            })
        } else if drag_active {
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Message::Ui(UiMsg::DragEnd),
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Message::ContextMenu(ContextMenuMsg::Close),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::Noop,
            })
        } else {
            // Always track the cursor so `last_mouse_pos` is fresh when the
            // user starts a modal drag — otherwise the first delta is huge
            // and the dialog jumps. DragMove is a no-op when no drag is
            // active (it just updates last_mouse_pos).
            iced::event::listen().map(|event| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Message::Ui(UiMsg::DragMove(position.x, position.y))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left,
                )) => Message::ContextMenu(ContextMenuMsg::Close),
                // Window::Resized intentionally omitted — the
                // `window::resize_events()` subscription below carries
                // the window id so we can drop non-main resizes. If
                // we also forwarded the raw event here, a detached
                // modal's resize would clobber the main window's size.
                _ => Message::Noop,
            })
        };
        // Window-close events from winit: routed so Phase 2/3 can drop
        // detached-modal / undocked-tab entries from ui_state.windows.
        let window_close = iced::window::close_events()
            .map(|id| Message::Window(WindowMsg::SecondaryWindowClosed(id)));
        // OS close requests (native close button, Alt+F4, taskbar close).
        // In daemon mode iced does NOT auto-close on these, so we route
        // them explicitly: the main window goes through the unsaved-
        // changes guard, any other window closes. Without this, an
        // Alt+F4 on a dirty main window would otherwise be silently
        // dropped (or, if iced ever auto-closed, lose unsaved edits).
        let window_close_request = iced::window::close_requests()
            .map(|id| Message::Window(WindowMsg::WindowCloseRequested(id)));
        // Window-resize subscription. `iced::event::listen()`'s
        // Window::Resized event doesn't fire on the very first frame —
        // subscribing to `window::resize_events()` directly gets the
        // initial physical size so dropdowns position correctly without
        // a manual resize.
        // Fire a WindowResizedFor for every OS resize event, carrying
        // the window id so the dispatcher can ignore resizes of
        // detached modal / undocked-tab windows. A plain WindowResized
        // without the id would clobber `ui_state.window_size` with
        // e.g. the 420x240 size of the Move dialog, which then shifts
        // the Active-Bar dropdowns on the main window.
        let window_resize = iced::window::resize_events().map(|(id, size)| {
            Message::Window(WindowMsg::WindowResizedFor(id, size.width, size.height))
        });

        // Hover-open timer for the right-click context-menu submenus.
        // Active while ANY menu that owns submenus is open — canvas
        // right-click, project-tree right-click, or document-tab
        // right-click. The dispatcher checks `pending_submenu`'s
        // elapsed time on each tick.
        let any_menu_open = self.interaction_state.context_menu.is_some()
            || self.interaction_state.project_tree_context_menu.is_some()
            || self.interaction_state.tab_context_menu.is_some();
        let hover_tick = if any_menu_open {
            iced::time::every(std::time::Duration::from_millis(50))
                .map(|_| Message::ContextMenu(ContextMenuMsg::SubmenuTickHover))
        } else {
            Subscription::none()
        };

        // Hover-tooltip wake tick. The tooltip overlay only shows
        // after the cursor has dwelled on a placed symbol for 250 ms
        // — without a periodic re-render, the view layer would never
        // notice the threshold crossing once the cursor stopped
        // moving (no mouse events → no redraw). This fires until the
        // user moves off the symbol; once the tooltip is up, normal
        // CursorMoved events keep it tracking the cursor.
        let symbol_hover_active = self.interaction_state.hover_symbol_uuid.is_some()
            && self
                .interaction_state
                .hover_started_at
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(900));
        let hover_tooltip_tick = if symbol_hover_active {
            iced::time::every(std::time::Duration::from_millis(80)).map(|_| Message::Noop)
        } else {
            Subscription::none()
        };

        Subscription::batch([
            kbd,
            mouse_sub,
            window_close,
            window_close_request,
            window_resize,
            hover_tick,
            hover_tooltip_tick,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set one flag on an otherwise-closed set and resolve the Esc ladder.
    fn only(set: impl FnOnce(&mut OpenOverlays)) -> Option<Message> {
        let mut overlays = OpenOverlays::default();
        set(&mut overlays);
        overlays.escape_message()
    }

    #[test]
    fn nothing_open_falls_through_to_the_tool_reset() {
        assert!(
            OpenOverlays::default().escape_message().is_none(),
            "with no modal open Esc must reach `Message::EscapePressed`"
        );
    }

    /// #511 — the passive calculator shipped in #490 with no rung here, so
    /// Esc left the modal open AND fell through to the tool reset, silently
    /// reverting the active canvas tool behind it.
    #[test]
    fn passive_calculator_claims_escape() {
        assert!(matches!(
            only(|o| o.passive_calculator_open = true),
            Some(Message::Overlay(OverlayMsg::ClosePassiveCalculator))
        ));
    }

    /// Every *field of `OpenOverlays`* claims Esc with its own Cancel
    /// message — NOT "every modal in the app". That gap is real and is
    /// exactly what let #511 (the passive calculator) and #514 (~14 more
    /// modals) ship with no rung: a modal only gets covered here once
    /// someone remembers to add it as a field and a rung, and nothing
    /// forces that. Documented modals that are deliberately NOT fields —
    /// so NOT covered by this test — are: the command palette and the
    /// keymap chord recorder (`palette_and_keymap_recorder_are_not_on_the_ladder`,
    /// both swallow input earlier), the Edit Component Details modal
    /// (dead behind `EDIT_MODAL_ENABLED = false`), and the per-browser
    /// delete-confirm modal (its Cancel message needs a `library_path`
    /// this `Copy` struct can't carry).
    #[test]
    fn every_modal_claims_escape() {
        assert!(matches!(
            only(|o| o.find_replace_open = true),
            Some(Message::FindReplaceMsg(
                crate::find_replace::FindReplaceMsg::Close
            ))
        ));
        assert!(matches!(
            only(|o| o.kbd_shortcuts_open = true),
            Some(Message::Overlay(OverlayMsg::CloseKeyboardShortcuts))
        ));
        assert!(matches!(
            only(|o| o.first_run_tour_open = true),
            Some(Message::Overlay(OverlayMsg::DismissFirstRunTour))
        ));
        assert!(matches!(
            only(|o| o.erc_open = true),
            Some(Message::Erc(ErcMsg::CloseDialog))
        ));
        assert!(matches!(
            only(|o| o.annotate_open = true),
            Some(Message::Annotate(AnnotateMsg::CloseDialog))
        ));
        assert!(matches!(
            only(|o| o.prefs_open = true),
            Some(Message::Preferences(PreferencesMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.rename_open = true),
            Some(Message::Rename(RenameMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.remove_open = true),
            Some(Message::Remove(RemoveMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.enable_vc_open = true),
            Some(Message::EnableVersionControl(
                EnableVersionControlMsg::Close
            ))
        ));
        assert!(matches!(
            only(|o| o.library_create_options_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::LibraryCreateOptionsCancel
            ))
        ));
        // #514 additions below.
        assert!(matches!(
            only(|o| o.annotate_reset_confirm_open = true),
            Some(Message::Annotate(AnnotateMsg::CloseResetConfirm))
        ));
        assert!(matches!(
            only(|o| o.app_quit_confirm_open = true),
            Some(Message::Project(ProjectMsg::AppQuitConfirm(
                ProjectCloseChoice::Cancel
            )))
        ));
        assert!(matches!(
            only(|o| o.project_close_confirm_open = true),
            Some(Message::Project(ProjectMsg::CloseConfirm(
                ProjectCloseChoice::Cancel
            )))
        ));
        assert!(matches!(
            only(|o| o.project_options_open = true),
            Some(Message::Project(ProjectMsg::CloseOptions))
        ));
        assert!(matches!(
            only(|o| o.grid_properties_open = true),
            Some(Message::GridProperties(GridPropertiesMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.selection_filter_custom_open = true),
            Some(Message::SelectionFilter(SelectionFilterMsg::CloseCustom))
        ));
        assert!(matches!(
            only(|o| o.library_document_options_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::DocumentOptionsCancel
            ))
        ));
        assert!(matches!(
            only(|o| o.library_primitive_picker_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::PrimitivePicker(
                    crate::library::messages::PrimitivePickerMsg::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.library_updates_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::LibraryUpdatesCancel
            ))
        ));
        assert!(matches!(
            only(|o| o.close_library_confirm_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::CloseLibraryConfirm(
                    crate::library::messages::CloseLibraryChoice::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.library_picker_open = true),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::ClosePicker
            ))
        ));
        assert!(matches!(
            only(|o| o.recovery_kind = Some(RecoveryKind::LibraryMissing)),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::RecoveryLibraryMissing(
                    crate::library::recovery::LibraryMissingChoice::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.recovery_kind = Some(RecoveryKind::GitMissing)),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::RecoveryGitMissing(
                    crate::library::recovery::GitMissingChoice::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.recovery_kind = Some(RecoveryKind::BrokenBinding)),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::RecoveryBrokenBinding(
                    crate::library::recovery::BrokenBindingChoice::Cancel
                )
            ))
        ));
        assert!(matches!(
            only(|o| o.export_error_open = true),
            Some(Message::Export(ExportMsg::DismissError))
        ));
        assert!(matches!(
            only(|o| o.netlist_incomplete_prompt_open = true),
            Some(Message::Export(ExportMsg::NetlistCancelIncomplete))
        ));
        assert!(matches!(
            only(|o| o.print_preview_open = true),
            Some(Message::PrintPreview(PrintPreviewMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.bom_preview_open = true),
            Some(Message::BomPreview(BomPreviewMsg::Close))
        ));
        assert!(matches!(
            only(|o| o.net_color_custom_open = true),
            Some(Message::NetColor(NetColorMsg::CustomShow(false)))
        ));
    }

    /// The palette and the chord recorder are absent from the ladder on
    /// purpose: both swallow keyboard input wholesale earlier in the
    /// subscription, so a rung here would be dead code that outranks a real
    /// modal.
    #[test]
    fn palette_and_keymap_recorder_are_not_on_the_ladder() {
        assert!(only(|o| o.palette_open = true).is_none());
        assert!(only(|o| o.keymap_recorder_open = true).is_none());
    }

    /// Order is load-bearing — the deepest (topmost-painted) modal wins.
    /// `passive_calculator_overlay` paints at `view/mod.rs:789`,
    /// `find_replace_overlay` at `:787` (earlier, i.e. underneath) and
    /// `preferences_overlay` at `:786` (earliest of the three) — so with
    /// all three flags set, the passive calculator must win. This is a
    /// KNOWN reversal from trunk (where `find_replace_open` used to win):
    /// that was a latent ordering bug — find_replace outranked a modal
    /// that visually painted on top of it — fixed as a side effect of
    /// deriving the whole ladder from paint order instead of hand-ordering
    /// it (see the doc comment on `escape_message`).
    #[test]
    fn the_deepest_modal_wins() {
        let overlays = OpenOverlays {
            find_replace_open: true,
            prefs_open: true,
            passive_calculator_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Overlay(OverlayMsg::ClosePassiveCalculator))
        ));
    }

    /// #514 — the reset confirm is a child of the annotate dialog with
    /// its own distinct Cancel message; getting the ordering backwards
    /// would resolve Esc to `CloseDialog` and close the parent out from
    /// under the confirm.
    #[test]
    fn annotate_reset_confirm_outranks_annotate_dialog() {
        let overlays = OpenOverlays {
            annotate_open: true,
            annotate_reset_confirm_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Annotate(AnnotateMsg::CloseResetConfirm))
        ));
    }

    /// The app-quit gate can be triggered while a shallower dialog (e.g.
    /// Preferences) is still open; the gate the user just triggered must
    /// win, not the dialog underneath it.
    #[test]
    fn quit_gate_outranks_a_dialog_layered_under_it() {
        let overlays = OpenOverlays {
            app_quit_confirm_open: true,
            prefs_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Project(ProjectMsg::AppQuitConfirm(
                ProjectCloseChoice::Cancel
            )))
        ));
    }

    /// #514 round 4 — refutes a round-3 claim. `app_quit_confirm_open`
    /// and a `has_blocking_modal` member CAN both be `true` at once: Alt+F4
    /// / native window close sets `app_quit_confirm` unconditionally
    /// (`handle_app_quit_requested`, no `has_blocking_modal` check on that
    /// path anywhere) — so an export failure followed by Alt+F4 leaves
    /// both set while only the blocking modal's card paints. The guard at
    /// the top of `escape_message` is what keeps Esc resolving to the
    /// visible card in that reachable state, not the invisible quit gate;
    /// this test exercises all four members of the group against it.
    #[test]
    fn blocking_modal_outranks_a_quit_gate_set_behind_it() {
        assert!(matches!(
            OpenOverlays {
                export_error_open: true,
                app_quit_confirm_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::Export(ExportMsg::DismissError))
        ));
        assert!(matches!(
            OpenOverlays {
                netlist_incomplete_prompt_open: true,
                app_quit_confirm_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::Export(ExportMsg::NetlistCancelIncomplete))
        ));
        assert!(matches!(
            OpenOverlays {
                print_preview_open: true,
                app_quit_confirm_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::PrintPreview(PrintPreviewMsg::Close))
        ));
        assert!(matches!(
            OpenOverlays {
                net_color_custom_open: true,
                app_quit_confirm_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::NetColor(NetColorMsg::CustomShow(false)))
        ));
    }

    /// The concrete reachable repro from round 4's review: export fails
    /// (`export_error_open` set, only the error card paints, per
    /// `has_blocking_modal`'s early return), then the user hits Alt+F4 /
    /// the OS close button with dirty documents, which sets
    /// `app_quit_confirm_open` with no regard for what's currently
    /// painted. Esc must dismiss the card the user can actually see, not
    /// silently cancel a quit gate they never saw — leaving them stuck
    /// staring at an export-error card that Esc appears to do nothing to.
    #[test]
    fn alt_f4_behind_an_export_error_card_does_not_steal_escape() {
        let overlays = OpenOverlays {
            export_error_open: true,
            app_quit_confirm_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Export(ExportMsg::DismissError))
        ));
    }

    /// Pin the `has_blocking_modal` group's own internal order, derived
    /// from `collect_overlays:750-754` reversed: net-colour > bom-preview >
    /// print-preview > netlist-incomplete-prompt > export-error.
    /// `bom_preview_open` sits inside this chain even though it isn't a
    /// `has_blocking_modal` member — it paints in the same early block, one
    /// slot below `net_color_custom_open` and one above `print_preview_open`.
    #[test]
    fn blocking_modal_group_internal_order_matches_derived_position() {
        assert!(matches!(
            OpenOverlays {
                net_color_custom_open: true,
                bom_preview_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::NetColor(NetColorMsg::CustomShow(false)))
        ));
        assert!(matches!(
            OpenOverlays {
                bom_preview_open: true,
                print_preview_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::BomPreview(BomPreviewMsg::Close))
        ));
        assert!(matches!(
            OpenOverlays {
                print_preview_open: true,
                netlist_incomplete_prompt_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::PrintPreview(PrintPreviewMsg::Close))
        ));
        assert!(matches!(
            OpenOverlays {
                netlist_incomplete_prompt_open: true,
                export_error_open: true,
                ..OpenOverlays::default()
            }
            .escape_message(),
            Some(Message::Export(ExportMsg::NetlistCancelIncomplete))
        ));
    }

    /// Round-2 regression #1: `enable_vc_open` was hand-placed above
    /// `erc_open`, but `erc_dialog_open` paints at `view/mod.rs:792`
    /// (`detachable_dialogs_overlay`, pushed last inside it), strictly
    /// later than `enable_vc_open`'s home in `simple_dialogs_overlay`
    /// (`:791`) — so ERC must outrank Enable Version Control.
    #[test]
    fn erc_dialog_outranks_enable_version_control() {
        let overlays = OpenOverlays {
            erc_open: true,
            enable_vc_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Erc(ErcMsg::CloseDialog))
        ));
    }

    /// Round-2 regression #2: `enable_vc_open` was also hand-placed above
    /// `grid_properties_open`, but inside `simple_dialogs_overlay`
    /// (`modals.rs:118-141`) `grid_properties` is pushed AFTER `enable_vc`
    /// (`:133` then `:136`) — later push = paints on top — so Grid
    /// Properties must outrank Enable Version Control, not the reverse.
    #[test]
    fn grid_properties_outranks_enable_version_control() {
        let overlays = OpenOverlays {
            grid_properties_open: true,
            enable_vc_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::GridProperties(GridPropertiesMsg::Close))
        ));
    }

    /// `bom_preview_open` paints in `collect_overlays`' earliest block
    /// (`:753`) with no exclusivity guarantee of its own, so anything
    /// pushed later — Find & Replace (`:787`) included — paints over it
    /// and must win Esc too.
    #[test]
    fn bom_preview_loses_to_a_rung_that_paints_above_it() {
        let overlays = OpenOverlays {
            bom_preview_open: true,
            find_replace_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::FindReplaceMsg(
                crate::find_replace::FindReplaceMsg::Close
            ))
        ));
    }

    /// The Document Options / Primitive Picker / Library Updates /
    /// Close-Library-confirm modals can all be reached from a Library
    /// Browser tab while the picker is still the shallower host surface
    /// behind them — the picker must not steal Esc from any of them.
    #[test]
    fn library_document_options_outranks_library_picker() {
        let overlays = OpenOverlays {
            library_picker_open: true,
            library_document_options_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Library(
                crate::library::messages::LibraryMessage::DocumentOptionsCancel
            ))
        ));
    }

    /// The gates below, layered over a shallower dialog pushed after them
    /// in `simple_dialogs_overlay`, must still lose to it — Grid
    /// Properties (Ctrl+G) opened, then an unsaved-edits app-quit request
    /// (chrome ✕ / File ▸ Exit / Alt+F4) layers behind it; Esc must close
    /// the card the user can actually see.
    #[test]
    fn a_dialog_pushed_after_the_quit_gate_outranks_it() {
        let overlays = OpenOverlays {
            app_quit_confirm_open: true,
            grid_properties_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::GridProperties(GridPropertiesMsg::Close))
        ));
    }

    /// #514-followup — `bom_preview_open` is NOT a `has_blocking_modal`
    /// member (only `export_error` / `netlist_incomplete_prompt` /
    /// `preview` / `net_color_custom.show` are), so it must not steal Esc
    /// from a quit/close gate the way the four true blocking modals do.
    #[test]
    fn quit_gate_outranks_bom_preview_because_it_is_not_actually_blocking() {
        let overlays = OpenOverlays {
            app_quit_confirm_open: true,
            bom_preview_open: true,
            ..OpenOverlays::default()
        };
        assert!(matches!(
            overlays.escape_message(),
            Some(Message::Project(ProjectMsg::AppQuitConfirm(
                ProjectCloseChoice::Cancel
            )))
        ));
    }

    /// #514 — the actual reported symptom, generalized: Esc must never
    /// reach `Message::EscapePressed` (the tool reset) while a modal
    /// represented on the ladder is open.
    ///
    /// What this test mechanically guarantees, precisely (no more, no
    /// less):
    ///
    /// 1. **Every field on `OpenOverlays`, of any type, is accounted for.**
    ///    `open_overlays_field_names!`'s identifier-list argument is
    ///    destructured against `OpenOverlays::default()` with no `..` rest
    ///    pattern, so add, remove, or rename ANY field — bool or not — and
    ///    this test fails to COMPILE ("pattern does not mention field
    ///    `…`") until the list is updated. `recovery_kind` is not
    ///    special-cased out of this list — it goes through the exact same
    ///    macro as every bool field, which is what closed the previous
    ///    hole (a hand-written non-bool tail that carried no rung
    ///    requirement).
    /// 2. **Every named field is classified as exactly one of "claims
    ///    Esc" or "documented exclusion", by NAME.** The `assert_eq!`
    ///    below is a SET-EQUALITY check of NAMES — the macro's field-name
    ///    list vs. the union of `setters`' (deduplicated) first elements
    ///    and `EXCLUDED` — not a count. This catches a name that isn't a
    ///    real field at all (a typo). It does NOT, by itself, catch a
    ///    correctly-labeled entry whose closure mutates the WRONG field:
    ///    for a genuinely new struct field `new_field_open`, the entry
    ///    `("new_field_open", |o| o.prefs_open = true)` passes this check
    ///    cleanly, because `"new_field_open"` IS a real field (the
    ///    exhaustive destructure in point 1 forces it into the macro's
    ///    list) and the label matches. Guarantee 4 below is what catches
    ///    that case.
    /// 3. **Every "claims Esc" field actually resolves to `Some(_)`.** The
    ///    first loop runs each `setters` closure through `only` and
    ///    asserts non-`None`. This alone does not catch the mis-wire in
    ///    point 2 either — `prefs_open` really does resolve to `Some(_)`.
    /// 4. **No two `setters` entries resolve to the same `Message`.** The
    ///    second loop asserts distinctness (by `Debug` string, since
    ///    `Message` isn't `PartialEq`) across every entry's resolved
    ///    message. A closure that (accidentally or via copy-paste) sets
    ///    the wrong field produces a DUPLICATE of that field's real entry
    ///    — e.g. the `new_field_open` mis-wire above would resolve to
    ///    `PreferencesMsg::Close` twice, once under each of two different
    ///    names — which this catches even though 2 and 3 both pass it.
    ///
    /// What it does NOT guarantee: which exact `Message` variant a rung
    /// returns (that's `every_modal_claims_escape`, above), or relative
    /// ordering between rungs (that's the dedicated precedence tests).
    #[test]
    fn every_ladder_field_claims_escape() {
        macro_rules! open_overlays_field_names {
            ($($field:ident),+ $(,)?) => {{
                let OpenOverlays { $($field: _),+ } = OpenOverlays::default();
                [$(stringify!($field)),+]
            }};
        }
        let field_names = open_overlays_field_names![
            find_replace_open,
            palette_open,
            kbd_shortcuts_open,
            first_run_tour_open,
            prefs_open,
            annotate_open,
            erc_open,
            rename_open,
            remove_open,
            enable_vc_open,
            library_create_options_open,
            keymap_recorder_open,
            passive_calculator_open,
            annotate_reset_confirm_open,
            app_quit_confirm_open,
            project_close_confirm_open,
            project_options_open,
            grid_properties_open,
            selection_filter_custom_open,
            library_picker_open,
            library_document_options_open,
            library_updates_open,
            library_primitive_picker_open,
            close_library_confirm_open,
            recovery_kind,
            export_error_open,
            netlist_incomplete_prompt_open,
            print_preview_open,
            bom_preview_open,
            net_color_custom_open,
        ];

        // Documented, deliberate absences from the ladder — see the doc
        // comment on `escape_message`.
        const EXCLUDED: &[&str] = &["palette_open", "keymap_recorder_open"];

        type EscapeSetter = fn(&mut OpenOverlays);
        // `recovery_kind` appears three times under the SAME field name —
        // one entry per `RecoveryKind` variant — which is exactly what the
        // name-based (not count-based) equality check below is for: three
        // entries collapse to one name, same as any other field.
        let setters: &[(&str, EscapeSetter)] = &[
            ("find_replace_open", |o| o.find_replace_open = true),
            ("kbd_shortcuts_open", |o| o.kbd_shortcuts_open = true),
            ("first_run_tour_open", |o| o.first_run_tour_open = true),
            ("erc_open", |o| o.erc_open = true),
            ("annotate_open", |o| o.annotate_open = true),
            ("prefs_open", |o| o.prefs_open = true),
            ("rename_open", |o| o.rename_open = true),
            ("remove_open", |o| o.remove_open = true),
            ("enable_vc_open", |o| o.enable_vc_open = true),
            ("library_create_options_open", |o| {
                o.library_create_options_open = true
            }),
            ("passive_calculator_open", |o| {
                o.passive_calculator_open = true
            }),
            ("annotate_reset_confirm_open", |o| {
                o.annotate_reset_confirm_open = true
            }),
            ("app_quit_confirm_open", |o| o.app_quit_confirm_open = true),
            ("project_close_confirm_open", |o| {
                o.project_close_confirm_open = true
            }),
            ("project_options_open", |o| o.project_options_open = true),
            ("grid_properties_open", |o| o.grid_properties_open = true),
            ("selection_filter_custom_open", |o| {
                o.selection_filter_custom_open = true
            }),
            ("library_document_options_open", |o| {
                o.library_document_options_open = true
            }),
            ("library_primitive_picker_open", |o| {
                o.library_primitive_picker_open = true
            }),
            ("library_updates_open", |o| o.library_updates_open = true),
            ("close_library_confirm_open", |o| {
                o.close_library_confirm_open = true
            }),
            ("library_picker_open", |o| o.library_picker_open = true),
            ("recovery_kind", |o| {
                o.recovery_kind = Some(RecoveryKind::LibraryMissing)
            }),
            ("recovery_kind", |o| {
                o.recovery_kind = Some(RecoveryKind::GitMissing)
            }),
            ("recovery_kind", |o| {
                o.recovery_kind = Some(RecoveryKind::BrokenBinding)
            }),
            ("export_error_open", |o| o.export_error_open = true),
            ("netlist_incomplete_prompt_open", |o| {
                o.netlist_incomplete_prompt_open = true
            }),
            ("print_preview_open", |o| o.print_preview_open = true),
            ("bom_preview_open", |o| o.bom_preview_open = true),
            ("net_color_custom_open", |o| o.net_color_custom_open = true),
        ];

        let macro_fields: std::collections::BTreeSet<&str> = field_names.into_iter().collect();
        let mut classified: std::collections::BTreeSet<&str> =
            setters.iter().map(|(name, _)| *name).collect();
        classified.extend(EXCLUDED.iter().copied());

        assert_eq!(
            macro_fields, classified,
            "OpenOverlays field(s) are not classified as exactly one of \
             `setters` (claims Esc) or `EXCLUDED` (a documented, \
             deliberate absence) — the two sides must name the same set \
             of fields"
        );
        for excluded in EXCLUDED {
            assert!(
                !setters.iter().any(|(name, _)| name == excluded),
                "{excluded} is in both `setters` and `EXCLUDED` — pick one"
            );
        }
        for (name, set) in setters {
            assert!(
                only(*set).is_some(),
                "{name} is on the Esc ladder but escape_message() fell \
                 through to `None` — Esc would silently reset the canvas \
                 tool behind it"
            );
        }

        // Distinctness: a name-only check (above) cannot tell a correctly
        // labeled entry from one whose closure copy-pasted a DIFFERENT
        // field's mutation — both pass the set-equality and `is_some()`
        // checks. Two entries resolving to the exact same `Message` is
        // that mis-wire's signature, so assert every resolved message is
        // unique. `Message` isn't `PartialEq`, so compare `Debug` output.
        let mut seen: std::collections::BTreeMap<String, &str> = std::collections::BTreeMap::new();
        for (name, set) in setters {
            let resolved = format!("{:?}", only(*set).expect("checked non-None above"));
            if let Some(first_name) = seen.insert(resolved.clone(), name) {
                panic!(
                    "{first_name} and {name} both resolve to {resolved} — one \
                     of them is mutating the wrong field"
                );
            }
        }
    }
}
