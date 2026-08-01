//! Esc resolution: which OS window the key was typed in, and what Esc
//! means there.
//!
//! Split out of the `Message::EscapePressed` arm in `dispatch/mod.rs`
//! (#554), which had grown into ~110 inline lines that read the MAIN
//! window's state no matter where the key came from.
//!
//! # Windows are not interchangeable
//!
//! [`EscapeSource`] is derived from `WindowKind` by an exhaustive match,
//! so a new window kind is a compile error here rather than silently
//! inheriting the main window's Esc — which is how #547 and this issue
//! both happened.
//!
//! The four kinds split by **what they paint**, not by what they are:
//!
//! * an undocked tab renders a full duplicate of the main view, overlay
//!   stack included (`view/mod.rs` maps `UndockedTab { .. } =>
//!   view_main_for(window_id)`, and `view_main_for` pushes
//!   `collect_overlays()` unconditionally), so the Esc ladder is
//!   *correct* there — a modal it dismisses really is on screen in that
//!   window;
//! * a detached modal paints one modal and no stack (#547);
//! * a detached panel and a component-editor window paint neither, so
//!   the ladder must not run for them at all.

use super::*;

/// Where an Esc came from, as far as resolving it cares (#554).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EscapeSource {
    /// The main window — or a synthesised Esc naming no window at all
    /// (`cancel_current_tool` from the command palette or a keymap
    /// binding, `app/command/bridge.rs`), which resolves the same way.
    Main,
    /// A modal showing in its own OS window (#547).
    DetachedModal(crate::app::state::ModalId),
    /// An undocked document tab. Paints the whole main view, so the Esc
    /// ladder applies; only the editor/tool reset behind it has to be
    /// re-aimed.
    UndockedTab,
    /// A detached dock panel. No canvas, no tool, no overlay stack.
    DetachedPanel,
    /// A detached Component Preview. Its state is addressable through
    /// `library.editors`, but nothing in it answers Esc today — and
    /// neither does the inline tab it mirrors.
    ComponentEditor,
}

impl Signex {
    /// Classify the window an Esc was typed in.
    ///
    /// The main window is deliberately absent from `ui_state.windows`
    /// (only `ui_state.main_window_id` names it), so a miss maps to
    /// [`EscapeSource::Main`] — as does a window already dropped from the
    /// map on a close frame, which is the answer the pre-#547 code gave.
    pub(crate) fn escape_source(&self, window: Option<iced::window::Id>) -> EscapeSource {
        use crate::app::state::WindowKind;

        let Some(window) = window else {
            return EscapeSource::Main;
        };
        match self.ui_state.windows.get(&window) {
            None => EscapeSource::Main,
            Some(WindowKind::DetachedModal(modal)) => EscapeSource::DetachedModal(*modal),
            Some(WindowKind::UndockedTab { .. }) => EscapeSource::UndockedTab,
            Some(WindowKind::DetachedPanel(_)) => EscapeSource::DetachedPanel,
            Some(WindowKind::ComponentEditor { .. }) => EscapeSource::ComponentEditor,
        }
    }

    /// Resolve one Esc against the window it was typed in.
    pub(crate) fn handle_escape_pressed(
        &mut self,
        window: Option<iced::window::Id>,
    ) -> Task<Message> {
        match self.escape_source(window) {
            // #547 — an Esc inside a detached modal's own window
            // addresses THAT modal and nothing else. The ladder is not
            // walked: it ranks overlays by where they paint in the main
            // window, and none of that stack covers a separate OS window,
            // so walking it would let a dialog elsewhere outrank the one
            // the user is typing into.
            //
            // `None` means the modal has no Esc at all (the
            // `every_modal_claims_escape` gap — Move Selection, the
            // net-colour palette, the parameter manager). Its in-window
            // card ignores Esc too, so swallowing the key keeps the two
            // spellings of one gesture agreeing.
            EscapeSource::DetachedModal(modal) => match self.detached_modal_escape_message(modal) {
                Some(msg) => self.update(msg),
                None => Task::none(),
            },

            // #554 — neither of these paints an overlay stack
            // (`view/mod.rs` renders a panel body / an editor surface and
            // nothing else), and neither owns a canvas or a tool. Running
            // the ladder here would dismiss a card the user cannot see
            // from this window, and the tool reset would cancel a
            // placement session in a window they are not typing into.
            //
            // Deliberately NOT "Esc closes the window": closing a
            // detached panel re-docks it
            // (`WindowMsg::SecondaryWindowClosed`), so a reflexive Esc
            // would destroy a layout the user arranged on purpose; and
            // the inline Component Preview tab has no Esc, so giving only
            // the detached form a dismissal is the asymmetry
            // `detached_modal_escape_message` argues against.
            EscapeSource::DetachedPanel | EscapeSource::ComponentEditor => Task::none(),

            // An undocked tab paints the same overlay stack as the main
            // window, so the ladder is right — but the editor branch
            // behind it is not. `view_center` gates the footprint /
            // symbol / component-editor surfaces on `is_main`, so an
            // undocked window only ever hosts a schematic (or PCB)
            // canvas; consulting the MAIN window's active tab from here
            // could only ever fire `FootprintToolEscape` at an editor
            // living in another window.
            EscapeSource::UndockedTab => {
                if let Some(overlay_msg) = self.escape_overlay_message() {
                    return self.update(overlay_msg);
                }
                self.escape_tool_reset()
            }

            EscapeSource::Main => {
                // The modal Esc ladder resolves FIRST, and here rather
                // than in the keyboard subscription (#535). Two things
                // follow from resolving it against live state:
                //
                // - a rung can carry owned data, which is what finally
                //   gives the per-browser delete-confirm modal a rung at
                //   all (its Cancel is addressed to one library path);
                // - there is no one-update staleness. The subscription
                //   snapshot was rebuilt only after `update` returned, so
                //   an Esc arriving in the same frame as a click-Close was
                //   resolved against the pre-click world.
                //
                // A behaviour change falls out of the second point, and it
                // is deliberate: when a click closes the LAST open modal
                // and an Esc is queued behind it, that Esc now falls
                // through to the tool reset below instead of being
                // swallowed as a Cancel for an already-closed modal. No
                // modal is open at that point, so the reset is what the
                // user is looking at.
                if let Some(overlay_msg) = self.escape_overlay_message() {
                    return self.update(overlay_msg);
                }
                self.escape_editor_or_tool_reset()
            }
        }
    }

    /// v0.15 — the main window's post-ladder Esc: if its active tab is a
    /// primitive editor, cancel that editor's state; otherwise fall back
    /// to the schematic tool reset.
    fn escape_editor_or_tool_reset(&mut self) -> Task<Message> {
        let footprint_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| t.kind.as_footprint_editor())
            .cloned();
        if let Some(path) = footprint_path {
            // #370 — when the "Align…" dialog is open, Esc just
            // dismisses it (leaving the selection intact) rather than
            // falling through to `ToolEscape`, which would also clear the
            // selected pad. Any other footprint Esc keeps the tool-reset
            // behaviour.
            let align_open = self
                .document_state
                .footprint_editors
                .get(&path)
                .is_some_and(|ed| ed.state.align_modal.is_some());
            let esc_msg = if align_open {
                crate::library::messages::FootprintEditorMsg::AlignCancel
            } else {
                crate::library::messages::FootprintEditorMsg::ToolEscape
            };
            return self.update(Message::Library(
                crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                    path,
                    msg: crate::library::messages::PrimitiveEdit::Footprint(esc_msg),
                },
            ));
        }

        let symbol_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| t.kind.as_symbol_editor())
            .cloned();
        if let Some(path) = symbol_path {
            // Symbol editor tab: Esc's one job today is closing an open
            // right-click context menu (no per-tool cancel state to reset
            // yet, unlike the footprint editor's `ToolEscape`).
            let menu_open = self
                .document_state
                .symbol_editors
                .get(&path)
                .is_some_and(|e| e.context_menu.is_some());
            if !menu_open {
                return Task::none();
            }
            return self.update(Message::Library(
                crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                    path,
                    msg: crate::library::messages::PrimitiveEdit::Symbol(
                        crate::library::messages::SymbolEditorMsg::CloseContextMenu,
                    ),
                },
            ));
        }

        self.escape_tool_reset()
    }

    /// Cancel the placement session and drop back to `Tool::Select`.
    ///
    /// There is exactly ONE placement session app-wide —
    /// `InteractionState::current_tool` and every placement buffer are
    /// single fields, and a tool picked from an undocked window's toolbar
    /// sets those same globals — so this is the same operation whichever
    /// window the Esc came from. Only the *visuals* are per-window, and
    /// `clear_transient_schematic_tool_state` sweeps all of them.
    fn escape_tool_reset(&mut self) -> Task<Message> {
        self.update(Message::Tool(crate::app::ToolMessage::SelectTool(
            crate::app::Tool::Select,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Tool;
    use crate::app::state::{ModalId, WindowKind};

    /// A freshly built app with nothing claiming Esc.
    ///
    /// `Signex::new()` opens the first-run tour, which is a legitimate
    /// rung and would answer every Esc these tests send. Closing it is
    /// the whole fixture; the assertion keeps that honest if another
    /// overlay ever starts life open.
    fn quiet_app() -> Signex {
        let (mut app, _boot) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        assert!(
            app.escape_overlay_message().is_none(),
            "fixture is not quiet — something else opens on construction and \
             would claim these Escs"
        );
        app
    }

    /// Give `kind` its own OS window and hand back that window's id.
    fn open_window(app: &mut Signex, kind: WindowKind) -> iced::window::Id {
        let id = iced::window::Id::unique();
        app.ui_state.windows.insert(id, kind);
        id
    }

    fn detach(app: &mut Signex, modal: ModalId) -> iced::window::Id {
        open_window(app, WindowKind::DetachedModal(modal))
    }

    // ── #547: a detached modal's own window ─────────────────────────
    //
    // `keyboard::listen()` dropped the window id, so an Esc typed into a
    // detached modal's own OS window was indistinguishable from one typed
    // into the main window. The ladder skips a detached modal's rung on
    // purpose (its in-window card is not painted), so nothing claimed the
    // key and it fell through to the main window's tool reset — a canvas
    // in a window the user was not even looking at.

    #[test]
    fn esc_in_a_detached_modal_window_addresses_that_modal_and_not_the_canvas() {
        let mut app = quiet_app();
        app.ui_state.preferences_open = true;
        let prefs_window = detach(&mut app, ModalId::Preferences);
        app.interaction_state.current_tool = Tool::Wire;

        // The bug, stated as the contrast: resolved as a main-window Esc
        // this state claims nothing, which is what used to reach the tool
        // reset.
        assert!(
            app.escape_overlay_message().is_none(),
            "a detached Preferences must stay unclaimed by a MAIN-window Esc — \
             its in-window card is not painted"
        );

        let _task = app.update(Message::EscapePressed {
            window: Some(prefs_window),
        });

        assert!(
            !app.ui_state.preferences_open,
            "Esc inside the detached Preferences window must close Preferences"
        );
        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Wire,
            "Esc in another window must not touch the main window's canvas tool"
        );
        assert!(
            !app.ui_state.windows.contains_key(&prefs_window),
            "closing the modal must take its OS window with it"
        );
    }

    #[test]
    fn esc_in_the_main_window_still_leaves_a_detached_modal_alone() {
        let mut app = quiet_app();
        app.ui_state.preferences_open = true;
        let _prefs_window = detach(&mut app, ModalId::Preferences);
        app.interaction_state.current_tool = Tool::Wire;

        // `main_window_id` is never inserted into `ui_state.windows`, so
        // the main window is exactly the "not in the map" case.
        let main = app.ui_state.main_window_id;
        let _task = app.update(Message::EscapePressed { window: main });

        assert!(
            app.ui_state.preferences_open,
            "an Esc in the main window must not dismiss a dialog living in \
             another window the user can still see"
        );
        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Select,
            "with nothing painted to claim it, a main-window Esc still resets \
             the tool"
        );
    }

    #[test]
    fn a_synthesised_esc_resolves_as_a_main_window_one() {
        // `cancel_current_tool` from the palette or a keymap binding names
        // no window (`app/command/bridge.rs`). It must not be mistaken for
        // a detached-window Esc and swallowed.
        let mut app = quiet_app();
        app.interaction_state.current_tool = Tool::Wire;

        let _task = app.update(Message::EscapePressed { window: None });

        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Select,
            "a windowless Esc must still reach the tool reset"
        );
    }

    #[test]
    fn a_detached_modal_with_no_rung_still_swallows_its_own_esc() {
        // The parameter manager is one of the three modals with no
        // `OverlayId` and no `OpenOverlays` field — Esc over its in-window
        // card does nothing. Its detached window must match that, NOT fall
        // through to the main window's tool reset.
        let mut app = quiet_app();
        app.ui_state.parameter_manager_open = true;
        let window = detach(&mut app, ModalId::ParameterManager);
        app.interaction_state.current_tool = Tool::Wire;

        let _task = app.update(Message::EscapePressed {
            window: Some(window),
        });

        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Wire,
            "an Esc with nothing to claim it in ITS OWN window is swallowed, \
             not redirected at the main window's canvas"
        );
    }

    // ── #554: the other three window kinds ──────────────────────────

    #[test]
    fn escape_source_classifies_every_window_kind() {
        let mut app = quiet_app();

        assert_eq!(
            app.escape_source(None),
            EscapeSource::Main,
            "a synthesised Esc names no window and resolves as the main one"
        );
        assert_eq!(
            app.escape_source(app.ui_state.main_window_id),
            EscapeSource::Main,
            "the main window is deliberately absent from `ui_state.windows`"
        );
        assert_eq!(
            app.escape_source(Some(iced::window::Id::unique())),
            EscapeSource::Main,
            "a window already dropped from the map (close frame) falls back \
             to the main answer, as it did before #547"
        );

        let modal = detach(&mut app, ModalId::ErcDialog);
        let tab = open_window(
            &mut app,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("/tmp/sheet.snxsch"),
                title: "sheet".to_string(),
            },
        );
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

        assert_eq!(
            app.escape_source(Some(modal)),
            EscapeSource::DetachedModal(ModalId::ErcDialog)
        );
        assert_eq!(app.escape_source(Some(tab)), EscapeSource::UndockedTab);
        assert_eq!(app.escape_source(Some(panel)), EscapeSource::DetachedPanel);
        assert_eq!(
            app.escape_source(Some(editor)),
            EscapeSource::ComponentEditor
        );
    }

    #[test]
    fn esc_in_an_undocked_tab_window_still_runs_the_ladder() {
        // An undocked tab renders `view_main_for`, which pushes
        // `collect_overlays()` unconditionally — the modal really is on
        // screen in that window, so the ladder claiming Esc there is
        // correct and must not be "fixed".
        let mut app = quiet_app();
        app.ui_state.keyboard_shortcuts_open = true;
        app.interaction_state.current_tool = Tool::Wire;
        let tab = open_window(
            &mut app,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("/tmp/sheet.snxsch"),
                title: "sheet".to_string(),
            },
        );

        let _task = app.update(Message::EscapePressed { window: Some(tab) });

        assert!(
            !app.ui_state.keyboard_shortcuts_open,
            "the modal is painted in this window, so its Esc must close it"
        );
        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Wire,
            "the ladder claimed the key, so the tool reset must not also run"
        );
    }

    #[test]
    fn an_undocked_tabs_esc_never_reaches_the_main_windows_footprint_editor() {
        // The headline #554 bug. The post-ladder branch read the MAIN
        // window's active tab, so with a footprint editor open there and
        // the user wiring in an undocked schematic window, Esc fired
        // `FootprintEditorMsg::ToolEscape` at an editor in another
        // window. `view_center` gates the footprint surface on `is_main`,
        // so an undocked window can never be showing that editor.
        let mut app = quiet_app();
        let footprint = std::path::PathBuf::from("/tmp/part.snxfpt");
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "part".to_string(),
            path: footprint.clone(),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind: crate::app::TabKind::FootprintEditor(footprint),
        });
        app.document_state.active_tab = 0;
        app.interaction_state.current_tool = Tool::Wire;

        // Baseline: a MAIN-window Esc takes the footprint branch, so the
        // schematic tool reset deliberately does NOT run.
        let _task = app.update(Message::EscapePressed {
            window: app.ui_state.main_window_id,
        });
        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Wire,
            "the main window's Esc belongs to its footprint editor"
        );

        // Same state, Esc typed in an undocked schematic window. That
        // editor is in another window, so the branch must be skipped and
        // the schematic reset must run instead.
        let tab = open_window(
            &mut app,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("/tmp/sheet.snxsch"),
                title: "sheet".to_string(),
            },
        );
        let _task = app.update(Message::EscapePressed { window: Some(tab) });
        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Select,
            "an undocked window's Esc must not be answered by an editor \
             living in the main window"
        );
    }

    #[test]
    fn esc_in_an_undocked_tab_window_falls_through_to_the_tool_reset() {
        let mut app = quiet_app();
        app.interaction_state.current_tool = Tool::Wire;
        let tab = open_window(
            &mut app,
            WindowKind::UndockedTab {
                path: std::path::PathBuf::from("/tmp/sheet.snxsch"),
                title: "sheet".to_string(),
            },
        );

        let _task = app.update(Message::EscapePressed { window: Some(tab) });

        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Select,
            "there is one placement session app-wide, so cancelling it from \
             the undocked window is the same operation"
        );
    }

    #[test]
    fn esc_in_a_detached_panel_window_changes_nothing() {
        // No canvas, no tool, no overlay stack — running the ladder here
        // would dismiss a card that does not render in this window, and
        // the tool reset would cancel a session in another one.
        let mut app = quiet_app();
        app.ui_state.keyboard_shortcuts_open = true;
        app.interaction_state.current_tool = Tool::Wire;
        let panel = open_window(
            &mut app,
            WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
        );

        let _task = app.update(Message::EscapePressed {
            window: Some(panel),
        });

        assert!(
            app.ui_state.keyboard_shortcuts_open,
            "a modal invisible from this window must survive its Esc"
        );
        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Wire,
            "a panel window has no tool of its own to reset"
        );
        assert!(
            app.ui_state.windows.contains_key(&panel),
            "Esc must not close the window — that would re-dock the panel"
        );
    }

    #[test]
    fn esc_in_a_component_editor_window_changes_nothing() {
        let mut app = quiet_app();
        app.ui_state.keyboard_shortcuts_open = true;
        app.interaction_state.current_tool = Tool::Wire;
        let editor = open_window(
            &mut app,
            WindowKind::ComponentEditor {
                library_path: std::path::PathBuf::from("/tmp/parts.snxlib"),
                table: "Resistors".to_string(),
                row_id: signex_library::RowId::new(),
            },
        );

        let _task = app.update(Message::EscapePressed {
            window: Some(editor),
        });

        assert!(
            app.ui_state.keyboard_shortcuts_open,
            "a modal invisible from this window must survive its Esc"
        );
        assert_eq!(
            app.interaction_state.current_tool,
            Tool::Wire,
            "the inline Component Preview tab has no Esc either"
        );
        assert!(
            app.ui_state.windows.contains_key(&editor),
            "Esc must not close the editor window"
        );
    }

    #[test]
    fn cancelling_a_placement_session_clears_the_ghost_in_every_window() {
        // The preview paint gate is two per-`SchematicCanvas` fields
        // (`drawing_mode` + a non-empty `wire_preview`), and the cancel
        // used to reach only the main slot — so the undocked window kept
        // painting a frozen ghost wire after the session was cancelled.
        let mut app = quiet_app();
        let tab_window = iced::window::Id::unique();

        let point = signex_types::schematic::Point { x: 1.0, y: 2.0 };
        app.interaction_state.wire_drawing = true;
        app.interaction_state.canvas.drawing_mode = true;
        app.interaction_state.canvas.wire_preview = vec![point];

        let mut per_window = crate::canvas::SchematicCanvas::new();
        per_window.drawing_mode = true;
        per_window.wire_preview = vec![point];
        app.interaction_state
            .canvases
            .insert(tab_window, per_window);

        // Esc typed in the MAIN window, with the wire being drawn in the
        // undocked one. Per-window Esc routing alone would not fix this.
        let _task = app.update(Message::EscapePressed {
            window: app.ui_state.main_window_id,
        });

        let undocked = &app.interaction_state.canvases[&tab_window];
        assert!(
            !undocked.drawing_mode && undocked.wire_preview.is_empty(),
            "the undocked window kept a ghost of a cancelled wire"
        );
        assert!(
            !app.interaction_state.canvas.drawing_mode
                && app.interaction_state.canvas.wire_preview.is_empty(),
            "the main canvas must still be cleared too"
        );
        assert!(
            !app.interaction_state.wire_drawing,
            "the app-wide session flag must be cleared once"
        );
    }
}
