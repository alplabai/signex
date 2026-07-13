//! ERC/annotate dialogs, modal drag, undock/detach handlers. Split from `handlers/erc.rs`.

use iced::Task;

use super::super::super::*;

impl Signex {
    pub(crate) fn handle_open_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = true;
        self.interaction_state.context_menu = None;
        self.handle_detach_modal(super::super::super::state::ModalId::ErcDialog)
    }

    pub(crate) fn handle_close_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = false;
        self.close_detached_modal(super::super::super::state::ModalId::ErcDialog)
    }

    pub(crate) fn handle_erc_severity_changed(
        &mut self,
        rule: signex_erc::RuleKind,
        severity: signex_erc::Severity,
    ) -> Task<Message> {
        if severity == rule.default_severity() {
            // Match default → remove override so the map stays minimal.
            self.ui_state.erc_severity_override.remove(&rule);
        } else {
            self.ui_state.erc_severity_override.insert(rule, severity);
        }
        // Persist so the override survives restart. Silent on I/O errors —
        // this is a preference, not critical state.
        crate::fonts::write_erc_severity_overrides(&self.ui_state.erc_severity_override);
        Task::none()
    }

    pub(crate) fn handle_open_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = true;
        self.handle_detach_modal(super::super::super::state::ModalId::AnnotateResetConfirm)
    }

    pub(crate) fn handle_close_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = false;
        self.close_detached_modal(super::super::super::state::ModalId::AnnotateResetConfirm)
    }

    pub(crate) fn handle_modal_drag_start(
        &mut self,
        modal: super::super::super::state::ModalId,
        x: f32,
        y: f32,
    ) -> Task<Message> {
        self.ui_state.modal_dragging = Some((modal, x, y));
        Task::none()
    }

    pub(crate) fn handle_modal_drag_end(&mut self) -> Task<Message> {
        self.ui_state.modal_dragging = None;
        self.ui_state.tab_dragging = None;
        Task::none()
    }

    /// Pop tab `idx` into its own OS window. The tab stays in
    /// `document_state.tabs` so reattach is a pure UI flip — closing the
    /// popped-out window via `SecondaryWindowClosed` just drops the entry
    /// from `ui_state.windows` and the tab re-appears in the tab bar.
    pub(crate) fn handle_undock_tab(&mut self, idx: usize) -> Task<Message> {
        let Some(tab) = self.document_state.tabs.get(idx) else {
            return Task::none();
        };
        let path = tab.path.clone();
        // Component Preview tabs undock to a window with
        // `WindowKind::ComponentEditor` so the editor view dispatch
        // picks it up. Schematic / PCB tabs use
        // `WindowKind::UndockedTab` as before.
        let component_editor = tab.kind.as_component_editor().cloned();

        // Don't re-undock a tab that already has a window.
        let already_undocked = match component_editor.as_ref() {
            Some(ce) => self.ui_state.windows.values().any(|k| {
                matches!(
                    k,
                    super::super::super::state::WindowKind::ComponentEditor {
                        library_path,
                        table,
                        row_id,
                    } if library_path == &ce.library_path
                        && table == &ce.table
                        && row_id == &ce.row_id
                )
            }),
            None => self.ui_state.windows.values().any(
                |k| matches!(k, super::super::super::state::WindowKind::UndockedTab { path: p, .. } if p == &path),
            ),
        };
        if already_undocked {
            return Task::none();
        }
        let title = tab.title.clone();

        // Make the tab active so the duplicated view in the new window
        // lands on that tab's content. Main window's active_tab is
        // shared — if the user wants to keep editing a different tab in
        // main, they can switch after the window opens.
        if idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }

        let size = iced::Size::new(1400.0, 900.0);
        let (id, open_task) = iced::window::open(iced::window::Settings {
            size,
            resizable: true,
            decorations: true,
            ..Default::default()
        });
        // Stash immediately so the first frame in the new window has a
        // target; `UndockedTabOpened` refreshes the title afterwards.
        if let Some(ce) = component_editor {
            self.ui_state.windows.insert(
                id,
                super::super::super::state::WindowKind::ComponentEditor {
                    library_path: ce.library_path.clone(),
                    table: ce.table.clone(),
                    row_id: ce.row_id,
                },
            );
            // Component Editor windows don't need the
            // `UndockedTabOpened` follow-up (no per-window canvas to
            // wire); the editor view picks the entry up directly off
            // `library.editors` via the address it already has.
            return open_task.discard();
        }
        self.ui_state.windows.insert(
            id,
            super::super::super::state::WindowKind::UndockedTab {
                path: path.clone(),
                title,
            },
        );
        open_task.map(move |settled_id| {
            Message::Window(WindowMsg::UndockedTabOpened {
                path: path.clone(),
                id: settled_id,
            })
        })
    }

    /// Remove the floating panel at `idx` and open an OS window that
    /// renders that panel's content. Closing the OS window re-docks the
    /// panel to the right column — see `SecondaryWindowClosed` in
    /// dispatch/mod.rs.
    pub(crate) fn handle_detach_floating_panel(&mut self, idx: usize) -> Task<Message> {
        let Some(fp) = self.document_state.dock.floating.get(idx) else {
            return Task::none();
        };
        let kind = fp.kind;
        let size = iced::Size::new(fp.width.max(420.0), fp.height.max(360.0));
        self.document_state.dock.floating.remove(idx);

        let (id, open_task) = iced::window::open(iced::window::Settings {
            size,
            resizable: true,
            decorations: true,
            ..Default::default()
        });
        self.ui_state
            .windows
            .insert(id, super::super::super::state::WindowKind::DetachedPanel(kind));
        open_task.map(move |settled_id| {
            Message::Window(WindowMsg::DetachedPanelOpened {
                kind,
                id: settled_id,
            })
        })
    }

    /// Find any OS window that currently hosts `modal` and request the
    /// OS to close it. Used by the in-body Close button so pressing Close
    /// inside a detached modal both dismisses the modal state and cleans
    /// up the popped-out window — without this, the window would stay
    /// open rendering an orphaned modal body.
    pub(crate) fn close_detached_modal(
        &mut self,
        modal: super::super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::super::state::WindowKind;
        let maybe_id = self.ui_state.windows.iter().find_map(|(id, kind)| {
            if matches!(kind, WindowKind::DetachedModal(m) if *m == modal) {
                Some(*id)
            } else {
                None
            }
        });
        if let Some(id) = maybe_id {
            self.ui_state.windows.remove(&id);
            iced::window::close(id)
        } else {
            Task::none()
        }
    }

    /// Pop `modal` out of the main window into its own OS window. The
    /// window's initial size matches the modal's in-app dimensions so the
    /// user sees continuity; position falls back to default (centered on
    /// the OS) since we don't know where to anchor absent monitor query.
    pub(crate) fn handle_detach_modal(
        &mut self,
        modal: super::super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::super::state::ModalId;
        // Don't open a second window for the same modal — treat detach
        // on an already-detached modal as a no-op.
        if self.ui_state.windows.values().any(
            |kind| matches!(kind, super::super::super::state::WindowKind::DetachedModal(m) if *m == modal),
        ) {
            return Task::none();
        }

        let size = match modal {
            ModalId::AnnotateDialog => iced::Size::new(1100.0, 760.0),
            ModalId::ErcDialog => iced::Size::new(1000.0, 600.0),
            ModalId::AnnotateResetConfirm => iced::Size::new(420.0, 180.0),
            ModalId::MoveSelection => iced::Size::new(420.0, 240.0),
            ModalId::NetColorPalette => iced::Size::new(520.0, 480.0),
            ModalId::ParameterManager => iced::Size::new(900.0, 560.0),
            ModalId::Preferences => iced::Size::new(900.0, 620.0),
            ModalId::FindReplace => iced::Size::new(420.0, 180.0),
            ModalId::RenameDialog => iced::Size::new(420.0, 200.0),
            ModalId::RemoveDialog => iced::Size::new(560.0, 260.0),
            ModalId::PrintPreview => iced::Size::new(1100.0, 780.0),
            ModalId::BomPreview => iced::Size::new(1180.0, 760.0),
            ModalId::ProjectOptions => iced::Size::new(520.0, 360.0),
            ModalId::EnableVersionControl => iced::Size::new(560.0, 480.0),
            ModalId::GridProperties => iced::Size::new(480.0, 280.0),
            ModalId::SelectionFilterCustom => iced::Size::new(440.0, 380.0),
        };

        let (id, open_task) = iced::window::open(iced::window::Settings {
            size,
            resizable: true,
            // No OS chrome — the modal body supplies its own header with
            // an X close button and a click-to-drag region.
            decorations: false,
            ..Default::default()
        });
        // Stash the mapping right away — view(id) for the new window
        // fires before open_task resolves on some platforms, and without
        // the entry the detached window would render empty.
        self.ui_state
            .windows
            .insert(id, super::super::super::state::WindowKind::DetachedModal(modal));
        // When the OS finishes opening the window, forward the id so the
        // update can double-check and clear any leftover drag state.
        open_task.map(move |settled_id| {
            Message::Window(WindowMsg::DetachedModalOpened {
                modal,
                id: settled_id,
            })
        })
    }

    pub(crate) fn handle_open_move_selection_dialog(&mut self) -> Task<Message> {
        self.ui_state.move_selection = super::super::super::state::MoveSelectionState {
            open: true,
            dx: "0".to_string(),
            dy: "0".to_string(),
        };
        self.handle_detach_modal(super::super::super::state::ModalId::MoveSelection)
    }

    pub(crate) fn handle_close_move_selection_dialog(&mut self) -> Task<Message> {
        self.ui_state.move_selection.open = false;
        Task::none()
    }

    pub(crate) fn handle_move_selection_apply(&mut self) -> Task<Message> {
        let dx = self
            .ui_state
            .move_selection
            .dx
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0);
        let dy = self
            .ui_state
            .move_selection
            .dy
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0);
        if dx == 0.0 && dy == 0.0 {
            self.ui_state.move_selection.open = false;
            return Task::none();
        }
        let items = self.interaction_state.active_canvas_mut().selected.clone();
        if items.is_empty() {
            self.ui_state.move_selection.open = false;
            return Task::none();
        }
        if let Some(engine) = self.document_state.active_engine_mut() {
            let _ = engine.execute(signex_engine::Command::MoveSelection { items, dx, dy });
        }
        self.ui_state.move_selection.open = false;
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
        self.sync_canvas_from_visible_schematic(crate::schematic_runtime::RenderInvalidation::FULL);
        self.update_selection_info();
        Task::none()
    }

    pub(crate) fn handle_parameter_manager_edit(
        &mut self,
        symbol_uuid: uuid::Uuid,
        key: String,
        value: String,
    ) -> Task<Message> {
        if let Some(engine) = self.document_state.active_engine_mut() {
            let _ = engine.execute(signex_engine::Command::SetSymbolField {
                symbol_id: symbol_uuid,
                key,
                value,
            });
            self.interaction_state
                .active_canvas_mut()
                .clear_content_cache();
            self.sync_canvas_from_visible_schematic(
                crate::schematic_runtime::RenderInvalidation::FULL,
            );
            self.refresh_panel_ctx();
        }
        Task::none()
    }

    /// Ask the OS to start a borderless-window drag for whichever window
    /// currently hosts `modal`. Wired to the decorations:false detached
    /// modal header so the user can move the window without an OS
    /// title bar.
    pub(crate) fn handle_start_detached_window_drag(
        &mut self,
        modal: super::super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::super::state::WindowKind;
        let id = self.ui_state.windows.iter().find_map(|(id, kind)| {
            if matches!(kind, WindowKind::DetachedModal(m) if *m == modal) {
                Some(*id)
            } else {
                None
            }
        });
        match id {
            Some(id) => crate::chrome::start_window_drag(id),
            None => Task::none(),
        }
    }
}
