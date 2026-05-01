use iced::Task;

use super::*;

mod command_palette;
mod document;
pub(crate) mod library;
mod overlay;
mod routed;
mod text_edit;
mod tool;
mod ui;

impl Signex {
    pub(crate) fn dispatch_update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Menu(_) | Message::Tab { .. } | Message::Dock(_) | Message::Selection(_) => {
                self.dispatch_routed_message(message)
            }
            Message::ThemeChanged(_)
            | Message::UnitCycled
            | Message::GridToggle
            | Message::CanvasEvent(_)
            | Message::CanvasEventInWindow { .. }
            | Message::DragStart(_)
            | Message::DragMove(_, _)
            | Message::WindowResized(_, _)
            | Message::DragEnd
            | Message::GridCycle
            | Message::StatusBar(_) => self.dispatch_ui_message(message),
            Message::TextEditChanged(_) | Message::TextEditSubmit => {
                self.dispatch_text_edit_message(message)
            }
            Message::PrePlacementTab
            | Message::ResumePlacement
            | Message::CycleDrawMode
            | Message::CancelDrawing
            | Message::Tool(_) => self.dispatch_tool_message(message),
            Message::FileOpened(_)
            | Message::NewProjectFile(_)
            | Message::DeleteSelected
            | Message::Undo
            | Message::Redo
            | Message::RotateSelected
            | Message::MirrorSelectedX
            | Message::MirrorSelectedY
            | Message::Cut
            | Message::Copy
            | Message::Paste
            | Message::SmartPaste
            | Message::Duplicate
            | Message::SaveFile
            | Message::SaveFileAs(_)
            | Message::SavePrimitiveAs { .. }
            | Message::SchematicLoaded(_)
            | Message::ExportPdfOpenDialog
            | Message::ExportPdfFinished(_)
            | Message::ExportNetlistFinished(_)
            | Message::ExportBomRequested
            | Message::ExportBomFinished(_)
            | Message::BomPreviewSetGrouping(_)
            | Message::BomPreviewSetFormat(_)
            | Message::BomPreviewSetIncludeDnp(_)
            | Message::BomPreviewSetIncludeNotFitted(_)
            | Message::BomPreviewToggleColumn(_)
            | Message::BomPreviewSetVariant(_)
            | Message::BomPreviewSortColumn(_)
            | Message::BomPreviewColumnDragStart(_)
            | Message::BomPreviewColumnDragDrop(_)
            | Message::BomPreviewColumnHoverEnter(_)
            | Message::BomPreviewColumnHoverExit(_)
            | Message::BomPreviewColumnResizeStart(_)
            | Message::BomPreviewColumnResizeEnd
            | Message::BomPreviewSetSidebarTab(_)
            | Message::BomPreviewExport
            | Message::BomPreviewClose
            | Message::PrintPreviewRequested
            | Message::PrintPreviewSelectPage(_)
            | Message::PrintPreviewSetColourMode(_)
            | Message::PrintPreviewSetPageRangeAll
            | Message::PrintPreviewSetPageRangeCurrent
            | Message::PrintPreviewSetPageRangeSpecific
            | Message::PrintPreviewSetSpecificPageInput(_)
            | Message::PrintPreviewSetFitToPage(_)
            | Message::PrintPreviewSetIncludeTitleBlock(_)
            | Message::PrintPreviewZoom(_)
            | Message::PrintPreviewExport
            | Message::PrintPreviewClose
            | Message::PrintPreviewSetTab(_)
            | Message::PrintPreviewPanStart
            | Message::PrintPreviewPanFinished
            | Message::PrintPreviewToggleFile(_)
            | Message::PrintPreviewSelectAllFiles
            | Message::PrintPreviewClearAllFiles
            | Message::PrintPreviewSetVariant(_)
            | Message::PrintPreviewSetUsePhysicalStructure(_)
            | Message::PrintPreviewSetPhysicalDesignators(_)
            | Message::PrintPreviewSetPhysicalNetLabels(_)
            | Message::PrintPreviewSetPhysicalPorts(_)
            | Message::PrintPreviewSetPhysicalSheetNumber(_)
            | Message::PrintPreviewSetPhysicalDocumentNumber(_)
            | Message::PrintPreviewSetIncludeNoErcMarkers(_)
            | Message::PrintPreviewSetIncludeParameterSets(_)
            | Message::PrintPreviewSetIncludeProbes(_)
            | Message::PrintPreviewSetIncludeBlankets(_)
            | Message::PrintPreviewSetIncludeNotes(_)
            | Message::PrintPreviewSetIncludeCollapsedNotes(_)
            | Message::PrintPreviewSetQuality(_)
            | Message::PrintPreviewSetBookmarkZoom(_)
            | Message::PrintPreviewSetGenerateNetsInfo(_)
            | Message::PrintPreviewSetBookmarkPins(_)
            | Message::PrintPreviewSetBookmarkNetLabels(_)
            | Message::PrintPreviewSetBookmarkPorts(_)
            | Message::PrintPreviewSetIncludeComponentParameters(_)
            | Message::PrintPreviewSetGlobalBookmarks(_)
            | Message::PrintPreviewSetPcbColourMode(_)
            | Message::DismissExportError => self.dispatch_document_message(message),
            Message::TogglePanelList
            | Message::OpenPanel(_)
            | Message::OpenFind
            | Message::OpenReplace
            | Message::OpenPreferences
            | Message::ClosePreferences
            | Message::CloseKeyboardShortcuts
            | Message::DismissFirstRunTour
            | Message::PreferencesNav(_)
            | Message::PreferencesMsg(_)
            | Message::FindReplaceMsg(_)
            | Message::ActiveBar(_)
            | Message::ShowContextMenu(_, _)
            | Message::CloseContextMenu
            | Message::ShowProjectTreeContextMenu(_)
            | Message::CloseProjectTreeContextMenu
            | Message::ProjectTreeAction(_)
            | Message::ShowTabContextMenu(_)
            | Message::CloseTabContextMenu
            | Message::TabContextAction(_)
            | Message::ProjectCloseConfirm(_)
            | Message::RenameBufferChanged(_)
            | Message::RenameSubmit
            | Message::CloseRenameDialog
            | Message::RemoveConfirm(_)
            | Message::CloseRemoveDialog
            | Message::AddExistingFilePicked { .. }
            | Message::AddNewSchematicPicked { .. }
            | Message::CloseProjectOptions
            | Message::EnableVersionControlToggleLfs
            | Message::EnableVersionControlToggleItem(_)
            | Message::EnableVersionControlConfirm
            | Message::CloseEnableVersionControl
            | Message::OpenContextSubmenu(_)
            | Message::HoverContextSubmenu(_)
            | Message::LeaveContextSubmenu
            | Message::EnterContextSubmenuPanel
            | Message::LeaveContextSubmenuPanel
            | Message::TickContextSubmenuHover
            | Message::ContextAction(_)
            | Message::RunErc
            | Message::Annotate(_)
            | Message::OpenAnnotateDialog
            | Message::CloseAnnotateDialog
            | Message::AnnotateOrderChanged(_)
            | Message::OpenErcDialog
            | Message::CloseErcDialog
            | Message::ErcSeverityChanged(_, _)
            | Message::OpenAnnotateResetConfirm
            | Message::CloseAnnotateResetConfirm
            | Message::ModalDragStart { .. }
            | Message::ModalDragEnd
            | Message::FocusAt { .. }
            | Message::ToggleAutoFocus => self.dispatch_overlay_message(message),
            Message::WindowResizedFor(id, w, h) => {
                // Only main-window resizes drive layout math. Detached
                // modal + undocked-tab windows have their own sizes
                // that would otherwise clobber the main-window state.
                if self.ui_state.main_window_id == Some(id) {
                    self.ui_state.window_size = (w, h);
                    // Windows fires a resize event whenever DWM moves
                    // the window to a monitor with a different DPI, so
                    // re-querying the scale factor here keeps the
                    // wordmark-PNG tier picker in sync after a
                    // cross-monitor drag.
                    return iced::window::scale_factor(id).map(Message::MainWindowScaleChanged);
                }
                Task::none()
            }
            Message::MainWindowScaleChanged(scale) => {
                self.ui_state.main_window_scale = scale;
                Task::none()
            }
            Message::MainWindowOpened(id) => {
                self.ui_state.main_window_id = Some(id);
                // Pull the real initial size from winit — opening the
                // window at Settings.size doesn't always land at
                // exactly that size (OS DPI scaling, display clamps).
                // Without this, Active-Bar dropdown positions are off
                // until the user physically resizes the window.
                let size_task = iced::window::size(id)
                    .map(move |size| Message::WindowResizedFor(id, size.width, size.height));
                // Stash the scale factor so the wordmark PNG picker
                // can render at native device-pixel count. Re-queried
                // on every resize to track monitor moves.
                let scale_task =
                    iced::window::scale_factor(id).map(Message::MainWindowScaleChanged);
                // Re-add Windows 11 DWM rounded corners + drop shadow
                // (silently no-ops on Windows 10 and non-Windows). Has
                // to run after the HWND is alive, hence here rather than
                // in bootstrap.
                let corners_task = crate::chrome::apply_rounded_corners::<Message>(id);
                iced::Task::batch([size_task, scale_task, corners_task])
            }
            Message::SecondaryWindowClosed(id) => {
                // Main window closed → terminate the process.
                if self.ui_state.main_window_id == Some(id) {
                    return iced::exit();
                }
                // Drop the entry and dismiss the backing modal state so
                // closing the OS window fully exits the modal instead of
                // reattaching a phantom copy to the main window on the
                // next view frame. Phase 3 will add undocked-tab cleanup
                // here too.
                if let Some(kind) = self.ui_state.windows.remove(&id) {
                    use super::state::{ModalId, WindowKind};
                    match kind {
                        WindowKind::DetachedModal(modal) => match modal {
                            ModalId::AnnotateDialog => self.ui_state.annotate_dialog_open = false,
                            ModalId::AnnotateResetConfirm => {
                                self.ui_state.annotate_reset_confirm = false
                            }
                            ModalId::ErcDialog => self.ui_state.erc_dialog_open = false,
                            ModalId::Preferences => self.ui_state.preferences_open = false,
                            ModalId::FindReplace => self.ui_state.find_replace.open = false,
                            ModalId::MoveSelection => self.ui_state.move_selection.open = false,
                            ModalId::NetColorPalette => {
                                self.ui_state.net_color_palette_open = false
                            }
                            ModalId::ParameterManager => {
                                self.ui_state.parameter_manager_open = false
                            }
                            ModalId::RenameDialog => self.ui_state.rename_dialog = None,
                            ModalId::RemoveDialog => self.ui_state.remove_dialog = None,
                            ModalId::PrintPreview => self.document_state.preview = None,
                            ModalId::BomPreview => self.document_state.bom_preview = None,
                            ModalId::ProjectOptions => {
                                self.ui_state.project_options = None;
                            }
                            ModalId::EnableVersionControl => {
                                self.ui_state.enable_version_control = None;
                            }
                        },
                        // Closing an undocked-tab window is the reattach
                        // gesture — the tab itself stays in
                        // document_state.tabs. Drop the per-window
                        // canvas so we don't leak caches for a window
                        // that's gone.
                        WindowKind::UndockedTab { .. } => {
                            self.interaction_state.canvases.remove(&id);
                        }
                        // Closing a detached panel reattaches it as a
                        // docked panel in the right column so the user
                        // doesn't lose access to the panel kind.
                        WindowKind::DetachedPanel(kind) => {
                            self.document_state
                                .dock
                                .add_panel(crate::dock::PanelPosition::Right, kind);
                        }
                        // The Component Preview lives as a tab in the
                        // main window; its state outlasts the
                        // detached OS window. Closing the OS window
                        // re-docks the editor to the main-window tab
                        // bar — `library.editors` keeps the in-flight
                        // edits keyed by `(library_path, table,
                        // row_id)`, and the main-window tab
                        // already exists, so there's nothing to do
                        // here beyond letting the window-id mapping
                        // drop above.
                        WindowKind::ComponentEditor { .. } => {}
                    }
                }
                Task::none()
            }
            Message::DetachModal(modal) => self.handle_detach_modal(modal),
            Message::DetachedModalOpened { modal, id } => {
                self.ui_state
                    .windows
                    .insert(id, super::state::WindowKind::DetachedModal(modal));
                // Any lingering drag state belongs to the main window —
                // once the modal is popped out, the OS handles window
                // drags directly.
                self.ui_state.modal_dragging = None;
                // Win11 DWM rounded corners on the detached window so
                // its edges visually match the modal_card's 8 px
                // radius. Silent no-op on Win10 / non-Windows.
                // Without this the OS paints the window with hard
                // corners and the modal_card's rounded border is
                // hidden inside a square OS frame.
                crate::chrome::apply_rounded_corners::<Message>(id)
            }
            Message::UndockTab(idx) => self.handle_undock_tab(idx),
            Message::UndockedTabOpened { path, id } => {
                let title = self
                    .document_state
                    .tabs
                    .iter()
                    .find(|t| t.path == path)
                    .map(|t| t.title.clone())
                    .unwrap_or_default();
                self.ui_state.windows.insert(
                    id,
                    super::state::WindowKind::UndockedTab {
                        path: path.clone(),
                        title,
                    },
                );
                // Spin up a fresh canvas for this window, seeded from
                // the engine that the tab points at so the new window
                // renders the correct schematic from its first frame.
                // Pan/zoom/selection start at SchematicCanvas::new
                // defaults — independent of the main canvas.
                let mut per_window = crate::canvas::SchematicCanvas::new();
                if let Some(engine) = self.document_state.engines.get(&path) {
                    per_window.set_render_cache(Some(
                        signex_render::schematic::SchematicRenderCache::from_sheet(
                            engine.document(),
                        ),
                    ));
                }
                // Mirror the main canvas's theme / snap / grid / paper
                // settings so the new window doesn't flash with the
                // defaults before any sync happens.
                per_window.theme_bg = self.interaction_state.canvas.theme_bg;
                per_window.theme_grid = self.interaction_state.canvas.theme_grid;
                per_window.theme_paper = self.interaction_state.canvas.theme_paper;
                per_window.canvas_colors = self.interaction_state.canvas.canvas_colors;
                per_window.snap_enabled = self.interaction_state.canvas.snap_enabled;
                per_window.snap_grid_mm = self.interaction_state.canvas.snap_grid_mm;
                per_window.visible_grid_mm = self.interaction_state.canvas.visible_grid_mm;
                per_window.grid_visible = self.interaction_state.canvas.grid_visible;
                per_window.paper_width_mm = self.interaction_state.canvas.paper_width_mm;
                per_window.paper_height_mm = self.interaction_state.canvas.paper_height_mm;
                per_window.fit_to_paper();
                self.interaction_state.canvases.insert(id, per_window);
                Task::none()
            }
            Message::ReattachTab(id) => {
                // Pre-remove so the tab bar shows the reattached tab on
                // the next view frame even before the OS-level close
                // fires `SecondaryWindowClosed`. Clear the per-window
                // canvas here too — otherwise `SecondaryWindowClosed`
                // short-circuits on `windows.remove -> None` and the
                // canvas cache + selection leak.
                self.ui_state.windows.remove(&id);
                self.interaction_state.canvases.remove(&id);
                iced::window::close(id)
            }
            Message::DetachFloatingPanel(idx) => self.handle_detach_floating_panel(idx),
            Message::DetachedPanelOpened { kind, id } => {
                self.ui_state
                    .windows
                    .insert(id, super::state::WindowKind::DetachedPanel(kind));
                Task::none()
            }
            Message::StartDetachedWindowDrag(modal) => {
                self.handle_start_detached_window_drag(modal)
            }
            Message::StartMainWindowDrag => match self.ui_state.main_window_id {
                Some(id) => crate::chrome::start_window_drag(id),
                None => Task::none(),
            },
            Message::StartMainWindowResize(direction) => match self.ui_state.main_window_id {
                Some(id) => crate::chrome::start_window_resize(id, direction),
                None => Task::none(),
            },
            Message::StartDetachedModalResize { modal, direction } => {
                // Find the OS window id hosting this modal, then ask
                // the OS to start a resize drag in the requested
                // direction. Same pattern as the main window —
                // detached modals have `decorations: false`, so
                // there's no OS frame to grab; the 6 px overlay
                // strips are how we expose resize. Routed through
                // `crate::chrome::start_window_resize` so the Win32
                // SC_SIZE fallback applies here too — winit's own
                // path silently no-ops on borderless windows after
                // the first attempt.
                let id = self.ui_state.windows.iter().find_map(|(id, kind)| {
                    if let super::state::WindowKind::DetachedModal(m) = kind {
                        if *m == modal {
                            return Some(*id);
                        }
                    }
                    None
                });
                match id {
                    Some(id) => crate::chrome::start_window_resize(id, direction),
                    None => Task::none(),
                }
            }
            Message::MinimizeMainWindow => match self.ui_state.main_window_id {
                Some(id) => iced::window::minimize(id, true),
                None => Task::none(),
            },
            Message::ToggleMaximizeMainWindow => match self.ui_state.main_window_id {
                Some(id) => iced::window::toggle_maximize(id),
                None => Task::none(),
            },
            Message::CloseMainWindow => match self.ui_state.main_window_id {
                Some(id) => iced::window::close(id),
                None => Task::none(),
            },
            Message::OpenMoveSelectionDialog => self.handle_open_move_selection_dialog(),
            Message::CloseMoveSelectionDialog => {
                let _ = self.handle_close_move_selection_dialog();
                self.close_detached_modal(super::state::ModalId::MoveSelection)
            }
            Message::MoveSelectionDxChanged(s) => {
                self.ui_state.move_selection.dx = s;
                Task::none()
            }
            Message::MoveSelectionDyChanged(s) => {
                self.ui_state.move_selection.dy = s;
                Task::none()
            }
            Message::MoveSelectionApply => self.handle_move_selection_apply(),
            Message::OpenNetColorPalette => {
                self.ui_state.net_color_palette_open = true;
                self.handle_detach_modal(super::state::ModalId::NetColorPalette)
            }
            Message::CloseNetColorPalette => {
                self.ui_state.net_color_palette_open = false;
                self.close_detached_modal(super::state::ModalId::NetColorPalette)
            }
            Message::NetColorSet { net, color } => {
                if let Some(c) = color {
                    self.ui_state.net_colors.insert(net, c);
                } else {
                    self.ui_state.net_colors.remove(&net);
                }
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                Task::none()
            }
            Message::OpenParameterManager => {
                self.ui_state.parameter_manager_open = true;
                self.handle_detach_modal(super::state::ModalId::ParameterManager)
            }
            Message::CloseParameterManager => {
                self.ui_state.parameter_manager_open = false;
                self.close_detached_modal(super::state::ModalId::ParameterManager)
            }
            Message::ParameterManagerEdit {
                symbol_uuid,
                key,
                value,
            } => self.handle_parameter_manager_edit(symbol_uuid, key, value),
            Message::AnnotateToggleLock(uuid) => {
                if self.ui_state.annotate_locked.contains(&uuid) {
                    self.ui_state.annotate_locked.remove(&uuid);
                } else {
                    self.ui_state.annotate_locked.insert(uuid);
                }
                Task::none()
            }
            Message::NetColorCustomShow(show) => {
                self.ui_state.net_color_custom.show = show;
                Task::none()
            }
            Message::NetColorCustomDraft(c) => {
                self.ui_state.net_color_custom.draft = c;
                Task::none()
            }
            Message::NetColorCustomSubmit(c) => {
                self.ui_state.net_color_custom.show = false;
                self.ui_state.net_color_custom.draft = c;
                let color = signex_types::theme::Color {
                    r: (c.r * 255.0).round() as u8,
                    g: (c.g * 255.0).round() as u8,
                    b: (c.b * 255.0).round() as u8,
                    a: 255,
                };
                self.ui_state.pending_net_color = Some(color);
                self.interaction_state.active_canvas_mut().pending_net_color = Some(color);
                Task::none()
            }
            Message::NetColorCustomChannel(chan, s) => {
                // Parse as u8; silently ignore invalid input so the
                // text_input doesn't reject intermediate values like
                // the empty string while the user types.
                let parsed = s.trim().parse::<u16>().unwrap_or(0).min(255) as u8;
                let draft = &mut self.ui_state.net_color_custom.draft;
                let v = parsed as f32 / 255.0;
                match chan {
                    super::contracts::Channel::R => draft.r = v,
                    super::contracts::Channel::G => draft.g = v,
                    super::contracts::Channel::B => draft.b = v,
                }
                Task::none()
            }
            Message::LassoCommit => {
                // Altium-style single terminator — Enter commits
                // whichever multi-click buffer is currently armed:
                //   - Lasso: selects inside the polygon.
                //   - Polyline (Tool::Polyline): writes a SchDrawing.
                //   - Arc (Tool::Arc): arms need 3 clicks regardless.
                if self.interaction_state.current_tool == Tool::Polyline
                    && self.interaction_state.polyline_points.len() >= 2
                {
                    let pp_w = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.shape_width_mm)
                        .unwrap_or(0.0);
                    let pp_fill = self
                        .document_state
                        .panel_ctx
                        .pre_placement
                        .as_ref()
                        .map(|pp| pp.shape_fill)
                        .unwrap_or(signex_types::schematic::FillType::None);
                    let pts = std::mem::take(&mut self.interaction_state.polyline_points);
                    let drawing = signex_types::schematic::SchDrawing::Polyline {
                        uuid: uuid::Uuid::new_v4(),
                        points: pts,
                        width: pp_w,
                        fill: pp_fill,
                        stroke_color: None,
                    };
                    self.apply_engine_command(
                        signex_engine::Command::PlaceSchDrawing { drawing },
                        false,
                        false,
                    );
                    self.interaction_state
                        .active_canvas_mut()
                        .polyline_points
                        .clear();
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_overlay_cache();
                    return Task::none();
                }
                if let Some(pts) = self.ui_state.lasso_polygon.take()
                    && pts.len() >= 3
                    && let Some(snapshot) = self.active_render_snapshot()
                {
                    let poly: Vec<(f64, f64)> = pts.iter().map(|p| (p.x, p.y)).collect();
                    let filters = self.interaction_state.selection_filters.clone();
                    self.interaction_state.active_canvas_mut().selected =
                        signex_render::schematic::hit_test::hit_test_polygon(snapshot, &poly)
                            .into_iter()
                            .filter(|h| {
                                super::handlers::selection_workflow::passes_filter(
                                    h, snapshot, &filters,
                                )
                            })
                            .collect();
                    self.update_selection_info();
                }
                self.sync_lasso_polygon_to_canvas();
                if self.open_selected_child_sheet() {
                    return Task::none();
                }
                Task::none()
            }
            Message::CycleSelectionMode => {
                use signex_render::schematic::hit_test::SelectionMode;
                self.ui_state.selection_mode = match self.ui_state.selection_mode {
                    SelectionMode::Inside => SelectionMode::Touching,
                    SelectionMode::Touching => SelectionMode::Inside,
                    SelectionMode::Single => SelectionMode::Inside,
                    _ => SelectionMode::Inside,
                };
                crate::diagnostics::log_info(format!(
                    "Selection mode: {:?}",
                    self.ui_state.selection_mode
                ));
                Task::none()
            }
            Message::PinMatrixCellCycled { row, col } => {
                use signex_erc::Severity;
                // Baseline defaults must match the `MATRIX` constant in
                // `pin_matrix_view` so "clearing" an override drops back
                // to the same severity the user sees in the UI.
                const BASELINE: [[Severity; 6]; 6] = [
                    [
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                    ],
                    [
                        Severity::Off,
                        Severity::Error,
                        Severity::Off,
                        Severity::Off,
                        Severity::Error,
                        Severity::Error,
                    ],
                    [
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Warning,
                    ],
                    [
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Off,
                        Severity::Error,
                    ],
                    [
                        Severity::Off,
                        Severity::Error,
                        Severity::Off,
                        Severity::Off,
                        Severity::Error,
                        Severity::Error,
                    ],
                    [
                        Severity::Off,
                        Severity::Error,
                        Severity::Warning,
                        Severity::Error,
                        Severity::Error,
                        Severity::Off,
                    ],
                ];
                let key = (row, col);
                let baseline = BASELINE
                    .get(row as usize)
                    .and_then(|r| r.get(col as usize))
                    .copied()
                    .unwrap_or(Severity::Off);
                let current = self
                    .ui_state
                    .pin_matrix_overrides
                    .get(&key)
                    .copied()
                    .unwrap_or(baseline);
                let next = match current {
                    Severity::Error => Severity::Warning,
                    Severity::Warning => Severity::Info,
                    Severity::Info => Severity::Off,
                    Severity::Off => Severity::Error,
                };
                if next == baseline {
                    self.ui_state.pin_matrix_overrides.remove(&key);
                } else {
                    self.ui_state.pin_matrix_overrides.insert(key, next);
                }
                crate::fonts::write_pin_matrix_overrides(&self.ui_state.pin_matrix_overrides);
                Task::none()
            }
            Message::UpdateDrawingField(uuid, edit) => self.handle_update_drawing_field(uuid, edit),
            Message::Library(msg) => self.dispatch_library_message(msg),
            Message::CommandPaletteOpen
            | Message::CommandPaletteClose
            | Message::CommandPaletteQueryChanged(_)
            | Message::CommandPaletteMoveSelection(_)
            | Message::CommandPaletteSelect(_)
            | Message::CommandPaletteExecuteSelected => {
                self.dispatch_command_palette_message(message)
            }
            Message::HistoryLoaded {
                generation,
                path: _,
                result,
            } => {
                // Drop stale results from a previous tab — the
                // generation token compares cheaply and is the
                // authoritative staleness check (the path field
                // is informational only).
                if generation != self.document_state.history.generation {
                    return Task::none();
                }
                self.document_state.history.loading = false;
                self.document_state.history.mode =
                    crate::panels::history::HistoryRenderMode::Ready;
                self.document_state.history.entries = match result {
                    Ok(entries) => entries,
                    Err(_) => Vec::new(),
                };
                self.document_state.panel_ctx.history =
                    self.document_state.history.clone();
                Task::none()
            }
            Message::Noop => Task::none(),
        }
    }
}
