//! `Signex::subscription` — keyboard / window / tick event wiring.
//! Split from `app/bootstrap.rs` as pure code motion.

use super::super::*;

use crate::keymap::KeyStroke;
use iced::Subscription;

impl Signex {
    pub fn subscription(&self) -> Subscription<Message> {
        use iced::keyboard;

        let kbd = keyboard::listen()
            .with((
                self.ui_state.find_replace.open,
                self.ui_state.command_palette.open,
                self.ui_state.keyboard_shortcuts_open,
                self.ui_state.first_run_tour_open,
                self.ui_state.preferences_open,
                self.ui_state.annotate_dialog_open,
                self.ui_state.erc_dialog_open,
                self.ui_state.rename_dialog.is_some(),
                self.ui_state.remove_dialog.is_some(),
                self.ui_state.enable_version_control.is_some(),
                self.library.create_options.is_some(),
                self.ui_state.preferences_keymap_recorder.is_some(),
            ))
            .map(
                |(
                    (
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
                    ),
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
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if find_replace_open =>
                            {
                                Message::FindReplaceMsg(crate::find_replace::FindReplaceMsg::Close)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if kbd_shortcuts_open =>
                            {
                                Message::Overlay(OverlayMsg::CloseKeyboardShortcuts)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if first_run_tour_open =>
                            {
                                Message::Overlay(OverlayMsg::DismissFirstRunTour)
                            }
                            // Esc closes the deepest open modal first (UX §1.3).
                            // The order here goes "user-facing top → bottom":
                            // ERC, then Annotate, then Preferences. Once those
                            // are closed, Esc falls through to the tool reset.
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _) if erc_open => {
                                Message::Erc(ErcMsg::CloseDialog)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if annotate_open =>
                            {
                                Message::Annotate(AnnotateMsg::CloseDialog)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if prefs_open =>
                            {
                                Message::Preferences(PreferencesMsg::Close)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if rename_open =>
                            {
                                Message::Rename(RenameMsg::Close)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if remove_open =>
                            {
                                Message::Remove(RemoveMsg::Close)
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if enable_vc_open =>
                            {
                                Message::EnableVersionControl(EnableVersionControlMsg::Close)
                            }
                            // F12 — Library Options modal Esc gap. Without
                            // this, Esc fell through to `Tool::Select`
                            // reset; users hit Create Library out of
                            // frustration thinking that was the only way
                            // out, which actually wrote the .snxlib to disk
                            // (violating the "no disk writes without user
                            // save" invariant when the user hadn't intended
                            // to confirm).
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _)
                                if library_create_options_open =>
                            {
                                Message::Library(
                                    crate::library::messages::LibraryMessage::LibraryCreateOptionsCancel,
                                )
                            }
                            (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                                // v0.15 — route through the
                                // dispatcher so Esc resets the
                                // footprint editor's tool state when
                                // a `.snxfpt` tab is active, and
                                // falls back to the schematic
                                // Tool::Select reset otherwise.
                                Message::EscapePressed
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
                                if m.command() && !m.alt() && super::selection_slot_from_key(c).is_some() =>
                            {
                                match super::selection_slot_from_key(c) {
                                    Some(slot) => Message::Selection(
                                        selection_request::SelectionRequest::StoreSlot { slot },
                                    ),
                                    _ => Message::Noop,
                                }
                            }
                            (keyboard::Key::Character(c), m)
                                if m.alt() && !m.command() && super::selection_slot_from_key(c).is_some() =>
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
