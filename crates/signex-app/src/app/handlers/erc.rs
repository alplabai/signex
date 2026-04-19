use iced::Task;

use super::super::*;

impl Signex {
    /// Run ERC across every sheet in the project — open tabs (live
    /// engine or cached session) plus every sheet in `project_data`
    /// that isn't open yet (parsed on-the-fly). Results are cached
    /// per-path in `erc_violations_by_path`; the visible list
    /// `erc_violations` repoints at the active sheet's entry so the
    /// Messages panel + canvas markers stay consistent with what's
    /// on screen.
    pub(crate) fn handle_run_erc(&mut self) -> Task<Message> {
        use crate::app::documents::TabDocument;
        let overrides = self.ui_state.erc_severity_override.clone();
        let apply_overrides = |mut violations: Vec<signex_erc::Violation>|
         -> Vec<signex_erc::Violation> {
            for v in &mut violations {
                if let Some(&sev) = overrides.get(&v.rule) {
                    v.severity = sev;
                }
            }
            violations.retain(|v| v.severity != signex_erc::Severity::Off);
            violations.sort_by_key(|v| {
                let bucket = match v.severity {
                    signex_erc::Severity::Error => 0,
                    signex_erc::Severity::Warning => 1,
                    signex_erc::Severity::Info => 2,
                    signex_erc::Severity::Off => 3,
                };
                (bucket, format!("{:?}", v.rule))
            });
            violations
        };

        let mut by_path: std::collections::HashMap<
            std::path::PathBuf,
            Vec<signex_erc::Violation>,
        > = std::collections::HashMap::new();

        // 1. Active tab — use the live engine's snapshot.
        if let Some(tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && let Some(snapshot) = self.active_render_snapshot()
        {
            let violations = apply_overrides(signex_erc::run(snapshot));
            by_path.insert(tab.path.clone(), violations);
        }
        // 2. Cached (non-active) tabs — build a snapshot from their
        // cached SchematicSheet and run ERC against it.
        for (idx, tab) in self.document_state.tabs.iter().enumerate() {
            if idx == self.document_state.active_tab {
                continue;
            }
            if let Some(TabDocument::Schematic(session)) = tab.cached_document.as_ref() {
                let snapshot =
                    signex_render::schematic::SchematicRenderSnapshot::from_sheet(
                        session.document(),
                    );
                let violations = apply_overrides(signex_erc::run(&snapshot));
                by_path.insert(tab.path.clone(), violations);
            }
        }
        // 3. Project sheets not opened as tabs — parse from disk,
        // run ERC, store. Gives a true project-wide picture.
        let open_paths: std::collections::HashSet<std::path::PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect();
        let project_root = self
            .document_state
            .project_path
            .as_ref()
            .and_then(|p| p.parent().map(std::path::PathBuf::from));
        if let Some(pd) = self.document_state.project_data.as_ref() {
            for sheet in &pd.sheets {
                let path = match project_root.as_ref() {
                    Some(root) => root.join(&sheet.filename),
                    None => std::path::PathBuf::from(&sheet.filename),
                };
                if open_paths.contains(&path) || by_path.contains_key(&path) {
                    continue;
                }
                let Ok(parsed) = kicad_parser::parse_schematic_file(&path) else {
                    continue;
                };
                let snapshot =
                    signex_render::schematic::SchematicRenderSnapshot::from_sheet(
                        &parsed,
                    );
                let violations = apply_overrides(signex_erc::run(&snapshot));
                by_path.insert(path, violations);
            }
        }

        let total: usize = by_path.values().map(|v| v.len()).sum();
        crate::diagnostics::log_info(&format!(
            "ERC: {} total violations across {} sheet(s)",
            total,
            by_path.len(),
        ));

        // Repoint the visible list + canvas markers at the active
        // sheet's entry. Updates on tab switch via `sync_active_tab`.
        let active_path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .map(|t| t.path.clone());
        self.ui_state.erc_violations_by_path = by_path;
        self.refresh_active_erc_from_cache(active_path.as_ref());

        // Surface the Messages panel so the user can see the results.
        self.document_state.dock.add_panel(
            crate::dock::PanelPosition::Right,
            crate::panels::PanelKind::Messages,
        );
        Task::none()
    }

    /// Repoint `erc_violations` + canvas markers at whatever the
    /// per-sheet cache holds for `active_path`. Empty vec when the
    /// sheet has never had ERC run, which is the right behaviour
    /// pre-Run-ERC.
    pub(crate) fn refresh_active_erc_from_cache(
        &mut self,
        active_path: Option<&std::path::PathBuf>,
    ) {
        let violations = active_path
            .and_then(|p| self.ui_state.erc_violations_by_path.get(p))
            .cloned()
            .unwrap_or_default();
        self.interaction_state.canvas.erc_markers = violations
            .iter()
            .map(|v| crate::canvas::ErcMarker {
                x: v.location.x,
                y: v.location.y,
                severity: match v.severity {
                    signex_erc::Severity::Error => {
                        crate::canvas::ErcMarkerSeverity::Error
                    }
                    signex_erc::Severity::Warning => {
                        crate::canvas::ErcMarkerSeverity::Warning
                    }
                    _ => crate::canvas::ErcMarkerSeverity::Info,
                },
                primary_uuid: v.primary.as_ref().map(|s| s.uuid),
            })
            .collect();
        self.interaction_state.canvas.clear_overlay_cache();
        self.ui_state.erc_violations = violations;
    }

    /// Move the viewport to center on a world-space point and optionally
    /// replace the current selection. Used by the Messages panel's
    /// click-to-zoom and by future Find/Replace result navigation.
    pub(crate) fn handle_focus_at(
        &mut self,
        world_x: f64,
        world_y: f64,
        select: Option<signex_types::schematic::SelectedItem>,
    ) -> Task<Message> {
        if let Some(item) = select {
            self.interaction_state.canvas.selected = vec![item];
            self.update_selection_info();
            self.interaction_state.canvas.clear_overlay_cache();
        }
        // Stage a fit target around the violation point so the canvas's
        // next draw centers on it.
        let half = 20.0_f32;
        self.interaction_state
            .canvas
            .pending_fit
            .set(Some(iced::Rectangle {
                x: world_x as f32 - half,
                y: world_y as f32 - half,
                width: half * 2.0,
                height: half * 2.0,
            }));
        Task::none()
    }

    pub(crate) fn handle_toggle_auto_focus(&mut self) -> Task<Message> {
        self.ui_state.auto_focus = !self.ui_state.auto_focus;
        // Mirror the flag onto the canvas so the renderer can compute
        // the focus uuid set locally without reaching into app state.
        self.interaction_state.canvas.auto_focus = self.ui_state.auto_focus;
        self.interaction_state.canvas.clear_content_cache();
        self.interaction_state.canvas.clear_overlay_cache();
        Task::none()
    }

    pub(crate) fn handle_annotate(
        &mut self,
        mode: signex_engine::AnnotateMode,
    ) -> Task<Message> {
        // Share one per-prefix counter across every open sheet so designators
        // don't collide across sheets of the same project.
        let mut next_by_prefix: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        let tab_count = self.document_state.tabs.len();

        // Pass A: seed the shared counter from every sheet's already-
        // annotated symbols (cached + active). This happens inside
        // annotate_with_seed's phase 2, but running a separate seed pass
        // first ensures order-independence — without this, sheet B could
        // reuse numbers it considers free that sheet A actually claims.
        let mut all_existing: Vec<String> = Vec::new();
        if let Some(eng) = self.document_state.engine.as_ref() {
            for sym in &eng.document().symbols {
                if !sym.is_power && !sym.reference.starts_with('#') {
                    all_existing.push(sym.reference.clone());
                }
            }
        }
        for tab in &self.document_state.tabs {
            if let Some(TabDocument::Schematic(session)) = tab.cached_document.as_ref() {
                for sym in &session.document().symbols {
                    if !sym.is_power && !sym.reference.starts_with('#') {
                        all_existing.push(sym.reference.clone());
                    }
                }
            }
        }
        for refstr in &all_existing {
            let prefix: String = refstr
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            if prefix.is_empty() {
                continue;
            }
            if let Ok(n) = refstr[prefix.len()..].parse::<u32>() {
                let e = next_by_prefix.entry(prefix).or_insert(0);
                if n > *e {
                    *e = n;
                }
            }
        }

        // Pass B: apply to cached (non-active) tabs via the shared counter.
        let locked = self.ui_state.annotate_locked.clone();
        let mut any_cached_changed = false;
        for (idx, tab) in self.document_state.tabs.iter_mut().enumerate() {
            if idx == self.document_state.active_tab {
                continue;
            }
            if let Some(TabDocument::Schematic(session)) = tab.cached_document.as_mut() {
                if let Ok(changed) = session
                    .engine_mut()
                    .annotate_with_seed_and_locks(mode, &mut next_by_prefix, &locked)
                {
                    if changed {
                        session.set_dirty(true);
                        tab.dirty = true;
                        any_cached_changed = true;
                    }
                }
            }
        }

        // Pass B2: walk every sheet in the project that isn't currently
        // open as a tab — parse from disk, annotate with the shared
        // counter, and write back. Altium's Annotate-Across-Project
        // covers even the sheets the user hasn't opened so designators
        // stay unique project-wide.
        let open_paths: std::collections::HashSet<std::path::PathBuf> = self
            .document_state
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .collect();
        let project_root = self
            .document_state
            .project_path
            .as_ref()
            .and_then(|p| p.parent().map(std::path::PathBuf::from));
        let unopened_sheet_paths: Vec<std::path::PathBuf> = self
            .document_state
            .project_data
            .as_ref()
            .map(|pd| {
                pd.sheets
                    .iter()
                    .filter_map(|s| {
                        let path = match project_root.as_ref() {
                            Some(root) => root.join(&s.filename),
                            None => std::path::PathBuf::from(&s.filename),
                        };
                        if open_paths.contains(&path) {
                            None
                        } else {
                            Some(path)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut disk_touched = 0usize;
        for sheet_path in unopened_sheet_paths {
            let Ok(sheet) = kicad_parser::parse_schematic_file(&sheet_path) else {
                crate::diagnostics::log_info(&format!(
                    "Annotate: failed to parse unopened sheet {}",
                    sheet_path.display()
                ));
                continue;
            };
            let Ok(mut engine) = signex_engine::Engine::new(sheet) else {
                continue;
            };
            engine.set_path(Some(sheet_path.clone()));
            let Ok(changed) =
                engine.annotate_with_seed_and_locks(mode, &mut next_by_prefix, &locked)
            else {
                continue;
            };
            if !changed {
                continue;
            }
            if engine.save().is_ok() {
                disk_touched += 1;
                crate::diagnostics::log_info(&format!(
                    "Annotate: saved {}",
                    sheet_path.display()
                ));
            }
        }

        // Pass C: apply to the active engine so the canvas, Properties
        // panel, and render cache all refresh. Run through the raw engine
        // method (not Command) so it shares the same counter.
        if let Some(engine) = self.document_state.engine.as_mut() {
            let _ = engine.annotate_with_seed_and_locks(mode, &mut next_by_prefix, &locked);
        }
        if disk_touched > 0 {
            crate::diagnostics::log_info(&format!(
                "Annotate: wrote {} unopened sheet file(s) to disk",
                disk_touched,
            ));
        }
        // Force a render + panel refresh as if a command had fired.
        self.interaction_state.canvas.clear_content_cache();
        self.sync_canvas_from_visible_schematic(
            signex_render::schematic::RenderInvalidation::FULL,
        );
        self.update_selection_info();
        if any_cached_changed || self.document_state.engine.is_some() {
            self.refresh_panel_ctx();
        }

        self.ui_state.annotate_reset_confirm = false;
        crate::diagnostics::log_info(&format!(
            "Annotated symbols across {} sheet(s) ({:?})",
            tab_count.max(1),
            mode,
        ));
        Task::none()
    }

    pub(crate) fn handle_open_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = true;
        self.interaction_state.context_menu = None;
        // Altium parity: these big modals live in their own OS window
        // from the moment they open — no in-window overlay, no drag-off
        // dance. `handle_detach_modal` is idempotent, so re-opening
        // while a window already exists just no-ops.
        self.handle_detach_modal(super::super::state::ModalId::AnnotateDialog)
    }

    pub(crate) fn handle_close_annotate_dialog(&mut self) -> Task<Message> {
        self.ui_state.annotate_dialog_open = false;
        self.close_detached_modal(super::super::state::ModalId::AnnotateDialog)
    }

    pub(crate) fn handle_annotate_order_changed(
        &mut self,
        order: super::super::state::AnnotateOrder,
    ) -> Task<Message> {
        self.ui_state.annotate_order = order;
        Task::none()
    }

    pub(crate) fn handle_open_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = true;
        self.interaction_state.context_menu = None;
        self.handle_detach_modal(super::super::state::ModalId::ErcDialog)
    }

    pub(crate) fn handle_close_erc_dialog(&mut self) -> Task<Message> {
        self.ui_state.erc_dialog_open = false;
        self.close_detached_modal(super::super::state::ModalId::ErcDialog)
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
        self.handle_detach_modal(super::super::state::ModalId::AnnotateResetConfirm)
    }

    pub(crate) fn handle_close_annotate_reset_confirm(&mut self) -> Task<Message> {
        self.ui_state.annotate_reset_confirm = false;
        self.close_detached_modal(super::super::state::ModalId::AnnotateResetConfirm)
    }

    pub(crate) fn handle_modal_drag_start(
        &mut self,
        modal: super::super::state::ModalId,
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
        // Don't re-undock a tab that already has a window.
        if self.ui_state.windows.values().any(
            |k| matches!(k, super::super::state::WindowKind::UndockedTab { path: p, .. } if p == &path),
        ) {
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
        self.ui_state.windows.insert(
            id,
            super::super::state::WindowKind::UndockedTab {
                path: path.clone(),
                title,
            },
        );
        open_task.map(move |settled_id| Message::UndockedTabOpened {
            path: path.clone(),
            id: settled_id,
        })
    }

    /// Remove the floating panel at `idx` and open an OS window that
    /// renders that panel's content. Closing the OS window re-docks the
    /// panel to the right column — see `SecondaryWindowClosed` in
    /// dispatch/mod.rs.
    pub(crate) fn handle_detach_floating_panel(
        &mut self,
        idx: usize,
    ) -> Task<Message> {
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
            .insert(id, super::super::state::WindowKind::DetachedPanel(kind));
        open_task.map(move |settled_id| Message::DetachedPanelOpened {
            kind,
            id: settled_id,
        })
    }

    /// Find any OS window that currently hosts `modal` and request the
    /// OS to close it. Used by the in-body Close button so pressing Close
    /// inside a detached modal both dismisses the modal state and cleans
    /// up the popped-out window — without this, the window would stay
    /// open rendering an orphaned modal body.
    pub(crate) fn close_detached_modal(
        &mut self,
        modal: super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::state::WindowKind;
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
        modal: super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::state::ModalId;
        // Don't open a second window for the same modal — treat detach
        // on an already-detached modal as a no-op.
        if self
            .ui_state
            .windows
            .values()
            .any(|kind| matches!(kind, super::super::state::WindowKind::DetachedModal(m) if *m == modal))
        {
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
            ModalId::CloseTabConfirm => iced::Size::new(420.0, 180.0),
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
            .insert(id, super::super::state::WindowKind::DetachedModal(modal));
        // When the OS finishes opening the window, forward the id so the
        // update can double-check and clear any leftover drag state.
        open_task.map(move |settled_id| Message::DetachedModalOpened {
            modal,
            id: settled_id,
        })
    }

    pub(crate) fn handle_open_move_selection_dialog(&mut self) -> Task<Message> {
        self.ui_state.move_selection = super::super::state::MoveSelectionState {
            open: true,
            dx: "0".to_string(),
            dy: "0".to_string(),
        };
        self.handle_detach_modal(super::super::state::ModalId::MoveSelection)
    }

    pub(crate) fn handle_close_move_selection_dialog(&mut self) -> Task<Message> {
        self.ui_state.move_selection.open = false;
        Task::none()
    }

    pub(crate) fn handle_move_selection_apply(&mut self) -> Task<Message> {
        let dx = self.ui_state.move_selection.dx.trim().parse::<f64>().unwrap_or(0.0);
        let dy = self.ui_state.move_selection.dy.trim().parse::<f64>().unwrap_or(0.0);
        if dx == 0.0 && dy == 0.0 {
            self.ui_state.move_selection.open = false;
            return Task::none();
        }
        let items = self.interaction_state.canvas.selected.clone();
        if items.is_empty() {
            self.ui_state.move_selection.open = false;
            return Task::none();
        }
        if let Some(engine) = self.document_state.engine.as_mut() {
            let _ = engine.execute(signex_engine::Command::MoveSelection { items, dx, dy });
        }
        self.ui_state.move_selection.open = false;
        self.interaction_state.canvas.clear_content_cache();
        self.interaction_state.canvas.clear_overlay_cache();
        self.sync_canvas_from_visible_schematic(
            signex_render::schematic::RenderInvalidation::FULL,
        );
        self.update_selection_info();
        Task::none()
    }

    pub(crate) fn handle_parameter_manager_edit(
        &mut self,
        symbol_uuid: uuid::Uuid,
        key: String,
        value: String,
    ) -> Task<Message> {
        if let Some(engine) = self.document_state.engine.as_mut() {
            let _ = engine.execute(signex_engine::Command::SetSymbolField {
                symbol_id: symbol_uuid,
                key,
                value,
            });
            self.interaction_state.canvas.clear_content_cache();
            self.sync_canvas_from_visible_schematic(
                signex_render::schematic::RenderInvalidation::FULL,
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
        modal: super::super::state::ModalId,
    ) -> Task<Message> {
        use super::super::state::WindowKind;
        let id = self.ui_state.windows.iter().find_map(|(id, kind)| {
            if matches!(kind, WindowKind::DetachedModal(m) if *m == modal) {
                Some(*id)
            } else {
                None
            }
        });
        match id {
            Some(id) => iced::window::drag(id),
            None => Task::none(),
        }
    }
}
