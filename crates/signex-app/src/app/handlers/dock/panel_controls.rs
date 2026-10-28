use iced::Task;
use signex_types::schematic::SheetSize;

use super::super::super::*;

impl Signex {
    /// Push the effective paper dimensions from PanelContext into the canvas so
    /// the background / grid track Page Options changes immediately. Also
    /// called from the document-load path so an opened sheet's stored paper
    /// size drives the drawn sheet, not the previous tab's leftovers.
    pub(crate) fn apply_page_dimensions_to_canvas(&mut self) {
        let ctx = &self.document_state.panel_ctx;
        let (w, h) = crate::panels::paper_dimensions(&ctx.paper_size);
        let sheet_frame = self.sheet_frame_for_active(w, h);
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.paper_width_mm = w;
        canvas.paper_height_mm = h;
        canvas.sheet_frame = sheet_frame;
        canvas.clear_bg_cache();
    }

    /// Store `token` as the active sheet's `paper_size`. The write is an
    /// engine command, so it persists with the document and undoes like any
    /// other edit; an unchanged token records nothing. Undo and redo redraw
    /// the page through the mutation gateway.
    fn set_sheet_paper_token(&mut self, token: String) {
        self.document_state.panel_ctx.paper_size = token.clone();
        self.apply_page_dimensions_to_canvas();
        self.apply_engine_command(
            signex_engine::Command::SetPaperSize { paper_size: token },
            false,
            false,
        );
    }

    /// Write the active sheet's draft for `field` as one undoable edit
    /// (#1080). Other fields' drafts, and other sheets', stay as they are;
    /// surrounding whitespace is trimmed, and an empty value clears the field.
    pub(crate) fn commit_title_block_field(&mut self, field: crate::panels::TitleBlockField) {
        let Some(sheet) = self.document_state.active_path.clone() else {
            return;
        };
        let drafts = &mut self.document_state.panel_ctx.title_block_drafts;
        let Some(at) = drafts
            .iter()
            .position(|d| d.field == field && d.sheet == sheet)
        else {
            return;
        };
        let draft = drafts.remove(at);
        self.apply_engine_command(
            signex_engine::Command::SetTitleBlockField {
                key: field.key().to_string(),
                value: draft.text.trim().to_string(),
            },
            false,
            false,
        );
    }

    /// Write the active sheet's custom width/height draft as one undoable
    /// `SetPaperSize`. Text that is not a number keeps the current edge.
    pub(crate) fn commit_custom_paper_edge(&mut self, edge: crate::panels::PaperEdge) {
        let Some(sheet) = self.document_state.active_path.clone() else {
            return;
        };
        let drafts = &mut self.document_state.panel_ctx.paper_edge_drafts;
        let Some(at) = drafts
            .iter()
            .position(|d| d.edge == edge && d.sheet == sheet)
        else {
            return;
        };
        let draft = drafts.remove(at);
        let (w, h) = crate::panels::paper_dimensions(&self.document_state.panel_ctx.paper_size);
        let current = match edge {
            crate::panels::PaperEdge::Width => w,
            crate::panels::PaperEdge::Height => h,
        };
        let mm = f64::from(crate::panels::typed_mm(&draft.text, current));
        let (width, height) = match edge {
            crate::panels::PaperEdge::Width => (Some(mm), None),
            crate::panels::PaperEdge::Height => (None, Some(mm)),
        };
        let token =
            SheetSize::resized_token(&self.document_state.panel_ctx.paper_size, width, height);
        self.set_sheet_paper_token(token);
    }

    /// Returns `None` when the message isn't a panel-control message
    /// (so the caller can fall through to the next dock handler), or
    /// `Some(task)` when handled — the task carries any follow-up work
    /// from a re-entrant `self.update(...)` so it isn't dropped.
    pub(super) fn handle_dock_panel_control_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> Option<Task<Message>> {
        let mut follow = Task::none();
        match panel_msg {
            crate::panels::PanelMsg::SetUnit(unit) => {
                self.ui_state.unit = *unit;
            }
            crate::panels::PanelMsg::RunErc => {
                follow = self.handle_run_erc();
                self.refresh_panel_ctx();
            }
            crate::panels::PanelMsg::ClearErc => {
                // Drop any in-flight run's result too.
                self.ui_state.erc_run_generation += 1;
                self.ui_state.erc_running = false;
                self.ui_state.erc_violations.clear();
                self.ui_state.erc_violations_by_path.clear();
                self.ui_state.erc_focus_global_index = None;
                self.interaction_state
                    .active_canvas_mut()
                    .erc_markers
                    .clear();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
                self.refresh_panel_ctx();
            }
            crate::panels::PanelMsg::FocusErcViolation(idx) => {
                follow = self.handle_focus_erc_diagnostic_index(*idx);
            }
            crate::panels::PanelMsg::ErcQuickFix(idx) => {
                follow = self.handle_erc_quick_fix(*idx);
            }
            crate::panels::PanelMsg::ErcNetWaiverSet {
                rule_id,
                net,
                waive,
            } => {
                follow = self.handle_erc_net_waiver_set(rule_id, net, *waive);
            }
            crate::panels::PanelMsg::FocusPrevErcDiagnostic => {
                follow = self.handle_focus_erc_diagnostic_offset(-1);
            }
            crate::panels::PanelMsg::FocusNextErcDiagnostic => {
                follow = self.handle_focus_erc_diagnostic_offset(1);
            }
            crate::panels::PanelMsg::ToggleGrid => {
                self.ui_state.grid_visible = !self.ui_state.grid_visible;
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
            }
            crate::panels::PanelMsg::ToggleSnap => {
                self.ui_state.snap_enabled = !self.ui_state.snap_enabled;
            }
            crate::panels::PanelMsg::NetClass(msg) => {
                self.handle_net_class_message(msg);
            }
            crate::panels::PanelMsg::PropertiesTab(index) => {
                self.document_state.panel_ctx.properties_tab = *index;
            }
            crate::panels::PanelMsg::ComponentFilter(filter) => {
                self.document_state.panel_ctx.component_filter = filter.clone();
                // Persisted debounced and off-thread so the next session
                // opens the panel with the same filter (#859).
                follow = self.persist_component_filter_soon(filter);
            }
            crate::panels::PanelMsg::ToggleSection(key) => {
                let key = key.clone();
                if !self
                    .document_state
                    .panel_ctx
                    .collapsed_sections
                    .remove(&key)
                {
                    self.document_state.panel_ctx.collapsed_sections.insert(key);
                }
            }
            crate::panels::PanelMsg::SetPrePlacementText(text) => {
                if let Some(pre_placement) = &mut self.document_state.panel_ctx.pre_placement {
                    pre_placement.label_text = text.clone();
                }
                // Mirror the edit to whichever ghost is armed so the live
                // preview reflects the user's typed net/port/text name.
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_label {
                    g.text = text.clone();
                }
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_symbol {
                    g.value = text.clone();
                }
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_text {
                    g.text = text.clone();
                }
            }
            crate::panels::PanelMsg::SetPrePlacementDesignator(text) => {
                if let Some(pre_placement) = &mut self.document_state.panel_ctx.pre_placement {
                    pre_placement.designator = text.clone();
                }
            }
            crate::panels::PanelMsg::SetPrePlacementRotation(rotation) => {
                if let Some(pre_placement) = &mut self.document_state.panel_ctx.pre_placement {
                    pre_placement.rotation = *rotation;
                }
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_label {
                    g.rotation = *rotation;
                }
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_symbol {
                    g.rotation = *rotation;
                }
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_text {
                    g.rotation = *rotation;
                }
            }
            crate::panels::PanelMsg::SetPrePlacementFont(font) => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.font = font.clone();
                }
            }
            crate::panels::PanelMsg::SetPrePlacementFontSize(pt) => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.font_size_pt = *pt;
                }
                let fs_mm = *pt as f64 * signex_types::schematic::SCHEMATIC_PT_TO_MM;
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_label {
                    g.font_size = fs_mm;
                }
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_text {
                    g.font_size = fs_mm;
                }
            }
            crate::panels::PanelMsg::SetPrePlacementJustifyH(h) => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.justify_h = *h;
                }
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_label {
                    g.justify = *h;
                }
                if let Some(g) = &mut self.interaction_state.active_canvas_mut().ghost_text {
                    g.justify_h = *h;
                }
            }
            crate::panels::PanelMsg::SetPrePlacementJustifyV(v) => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.justify_v = *v;
                }
            }
            crate::panels::PanelMsg::TogglePrePlacementBold => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.bold = !pp.bold;
                }
            }
            crate::panels::PanelMsg::TogglePrePlacementItalic => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.italic = !pp.italic;
                }
            }
            crate::panels::PanelMsg::TogglePrePlacementUnderline => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.underline = !pp.underline;
                }
            }
            crate::panels::PanelMsg::SetPrePlacementShapeWidth(w) => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.shape_width_mm = (*w).max(0.0);
                }
            }
            crate::panels::PanelMsg::NumericFieldTyped(key, text) => {
                // Text that is mid-edit (`-`, `1.`, empty) or refused leaves
                // the stored value alone; the draft keeps what was typed.
                if let Some(value) = crate::numeric_field::parse_field_f64(text) {
                    follow = self.update(Message::Dock(crate::dock::DockMessage::Panel(
                        key.committing(value),
                    )));
                }
                // The draft is bound after the commit, to the value the
                // model holds now: a clamped setter (pin length `max(0.1)`,
                // arc angle wrap) must not read as an outside change.
                self.document_state.panel_ctx.numeric_draft =
                    self.numeric_field_state(*key).map(|(owner, applied)| {
                        crate::panels::NumericDraft {
                            key: *key,
                            owner,
                            text: text.clone(),
                            applied,
                        }
                    });
            }
            crate::panels::PanelMsg::NumericFieldSubmitted(key) => {
                let owns_draft = self
                    .document_state
                    .panel_ctx
                    .numeric_draft
                    .as_ref()
                    .is_some_and(|d| d.key == *key);
                if owns_draft {
                    self.document_state.panel_ctx.numeric_draft = None;
                }
            }
            crate::panels::PanelMsg::SetPrePlacementShapeFill(f) => {
                if let Some(pp) = &mut self.document_state.panel_ctx.pre_placement {
                    pp.shape_fill = *f;
                }
            }
            crate::panels::PanelMsg::DrawingFieldTyping(field_id, text) => {
                // Rebind the buffer ownership when selection changes
                // so a prior drawing's half-typed text doesn't bleed
                // into the newly selected one.
                let current_uuid = self.document_state.panel_ctx.selected_uuid;
                if self.document_state.panel_ctx.drawing_edit_buf_for != current_uuid {
                    self.document_state.panel_ctx.drawing_edit_buf.clear();
                    self.document_state.panel_ctx.drawing_edit_buf_for = current_uuid;
                }
                self.document_state
                    .panel_ctx
                    .drawing_edit_buf
                    .insert(*field_id, text.clone());
                // Emit the engine update only when the string parses
                // cleanly — empty / partial input stays in the buffer
                // so the user can keep typing.
                if let Some(v) = crate::numeric_field::parse_field_f64(text) {
                    use crate::app::contracts::DrawingFieldEdit as E;
                    use crate::panels::DrawingFieldId as F;
                    let edit = match field_id {
                        F::LineStartX => E::LineStartX(v),
                        F::LineStartY => E::LineStartY(v),
                        F::LineEndX => E::LineEndX(v),
                        F::LineEndY => E::LineEndY(v),
                        F::LineWidth => E::Width(v),
                        F::RectStartX => E::RectStartX(v),
                        F::RectStartY => E::RectStartY(v),
                        F::RectWidth => E::RectWidthMm(v),
                        F::RectHeight => E::RectHeightMm(v),
                        F::RectBorder => E::Width(v),
                        F::CircleCenterX => E::CircleCenterX(v),
                        F::CircleCenterY => E::CircleCenterY(v),
                        F::CircleRadius => E::CircleRadius(v),
                        F::CircleBorder => E::Width(v),
                        F::ArcCenterX => E::ArcCenterX(v),
                        F::ArcCenterY => E::ArcCenterY(v),
                        F::ArcRadius => E::ArcRadius(v),
                        F::ArcStartAngle => E::ArcStartAngle(v),
                        F::ArcEndAngle => E::ArcEndAngle(v),
                        F::ArcWidth => E::Width(v),
                        F::PolyBorder => E::Width(v),
                    };
                    if let Some(uuid) = current_uuid.filter(|_| {
                        matches!(
                            self.document_state.panel_ctx.selected_kind,
                            Some(signex_types::schematic::SelectedKind::Drawing)
                        )
                    }) {
                        follow = self.update(crate::app::Message::Edit(
                            EditMsg::UpdateDrawingField(uuid, edit),
                        ));
                    }
                }
            }
            crate::panels::PanelMsg::UpdateDrawingEdit(edit) => {
                // Fire the engine-side update only if exactly one
                // drawing is selected — the panel shows post-placement
                // fields only in that case.
                if let Some(uuid) = self.document_state.panel_ctx.selected_uuid.filter(|_| {
                    matches!(
                        self.document_state.panel_ctx.selected_kind,
                        Some(signex_types::schematic::SelectedKind::Drawing)
                    )
                }) {
                    follow = self.update(crate::app::Message::Edit(EditMsg::UpdateDrawingField(
                        uuid, *edit,
                    )));
                }
            }
            crate::panels::PanelMsg::ConfirmPrePlacement => {
                // OK button: resume placement but keep pre_placement so the
                // next click uses the values the user just edited.
                self.interaction_state.active_canvas_mut().placement_paused = false;
            }
            crate::panels::PanelMsg::SetGridSize(size) => {
                self.ui_state.grid_size_mm = *size;
                self.document_state.panel_ctx.grid_size_mm = *size;
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                let size = *size;
                follow = crate::fonts::background_pref_write("grid_size_mm", move || {
                    crate::fonts::write_grid_size_mm_pref(size)
                });
            }
            crate::panels::PanelMsg::SetVisibleGridSize(size) => {
                self.ui_state.visible_grid_mm = *size;
                self.document_state.panel_ctx.visible_grid_mm = *size;
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
            }
            crate::panels::PanelMsg::ToggleSnapHotspots => {
                self.ui_state.snap_hotspots = !self.ui_state.snap_hotspots;
                self.document_state.panel_ctx.snap_hotspots = self.ui_state.snap_hotspots;
            }
            crate::panels::PanelMsg::SetUiFont(name) => {
                self.ui_state.ui_font_name = name.clone();
                self.document_state.panel_ctx.ui_font_name = name.clone();
                let name = name.clone();
                follow = crate::fonts::background_pref_write("ui_font", move || {
                    crate::fonts::write_ui_font_pref(&name)
                });
            }
            crate::panels::PanelMsg::SetCanvasFont(name) => {
                self.ui_state.canvas_font_name = name.clone();
                self.document_state.panel_ctx.canvas_font_name = name.clone();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
            }
            crate::panels::PanelMsg::SetCanvasFontSize(size) => {
                self.ui_state.canvas_font_size = *size;
                self.document_state.panel_ctx.canvas_font_size = *size;
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
            }
            crate::panels::PanelMsg::SetCanvasFontBold(is_bold) => {
                self.ui_state.canvas_font_bold = *is_bold;
                self.document_state.panel_ctx.canvas_font_bold = *is_bold;
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
            }
            crate::panels::PanelMsg::SetCanvasFontItalic(is_italic) => {
                self.ui_state.canvas_font_italic = *is_italic;
                self.document_state.panel_ctx.canvas_font_italic = *is_italic;
                self.interaction_state
                    .active_canvas_mut()
                    .clear_content_cache();
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
            }
            crate::panels::PanelMsg::OpenCanvasFontPopup => {
                self.document_state.panel_ctx.canvas_font_popup_open = true;
            }
            crate::panels::PanelMsg::CloseCanvasFontPopup => {
                self.document_state.panel_ctx.canvas_font_popup_open = false;
            }
            crate::panels::PanelMsg::EditSheetFrame(edit) => {
                // Applied to the engine's settings, not the panel's copy, so
                // two edits in one frame both land.
                if let Some(engine) = self.document_state.active_engine() {
                    let settings = edit.apply(engine.document().frame);
                    self.apply_engine_command(
                        signex_engine::Command::SetSheetFrame { settings },
                        false,
                        false,
                    );
                }
            }
            crate::panels::PanelMsg::SetPageFormatMode(mode) => {
                self.document_state.panel_ctx.page_format_mode = *mode;
                self.apply_page_dimensions_to_canvas();
            }
            crate::panels::PanelMsg::SetPaperSize(size) => {
                let token =
                    SheetSize::picked_token(&self.document_state.panel_ctx.paper_size, size);
                self.set_sheet_paper_token(token);
            }
            crate::panels::PanelMsg::SetSheetOrientation(orientation) => {
                let token = SheetSize::reoriented_token(
                    &self.document_state.panel_ctx.paper_size,
                    *orientation,
                );
                self.set_sheet_paper_token(token);
            }
            crate::panels::PanelMsg::TitleBlockTyping(field, text) => {
                if let Some(sheet) = self.document_state.active_path.clone() {
                    let drafts = &mut self.document_state.panel_ctx.title_block_drafts;
                    drafts.retain(|d| !(d.field == *field && d.sheet == sheet));
                    drafts.push(crate::panels::TitleBlockDraft {
                        sheet,
                        field: *field,
                        text: text.clone(),
                    });
                }
            }
            crate::panels::PanelMsg::CommitTitleBlockField(field) => {
                self.commit_title_block_field(*field);
            }
            crate::panels::PanelMsg::SetPageOrigin(origin) => {
                self.document_state.panel_ctx.page_origin = *origin;
            }
            crate::panels::PanelMsg::CustomPaperEdgeTyping(edge, text) => {
                if let Some(sheet) = self.document_state.active_path.clone() {
                    let drafts = &mut self.document_state.panel_ctx.paper_edge_drafts;
                    drafts.retain(|d| !(d.edge == *edge && d.sheet == sheet));
                    drafts.push(crate::panels::PaperEdgeDraft {
                        sheet,
                        edge: *edge,
                        text: text.clone(),
                    });
                }
            }
            crate::panels::PanelMsg::CommitCustomPaperEdge(edge) => {
                self.commit_custom_paper_edge(*edge);
            }
            crate::panels::PanelMsg::SetSheetColor(color) => {
                self.document_state.panel_ctx.sheet_color = *color;
                self.interaction_state.active_canvas_mut().clear_bg_cache();
            }
            crate::panels::PanelMsg::DragComponentsSplit => {
                self.interaction_state.dragging = Some(DragTarget::ComponentsSplit);
                self.interaction_state.drag_start_pos = None;
                self.interaction_state.drag_start_size =
                    self.document_state.panel_ctx.components_split;
            }
            crate::panels::PanelMsg::ToggleSelectionFilter(filter) => {
                follow = self.handle_active_bar_filter_toggle(*filter);
            }
            crate::panels::PanelMsg::ToggleAllSelectionFilters => {
                follow = self.handle_active_bar_all_filters_toggle();
            }
            crate::panels::PanelMsg::AddCustomFilterPreset => {
                self.handle_add_custom_filter_preset();
            }
            crate::panels::PanelMsg::RemoveCustomFilterPreset(idx) => {
                self.handle_remove_custom_filter_preset(*idx);
            }
            crate::panels::PanelMsg::RenameCustomFilterPreset(idx, name) => {
                self.handle_rename_custom_filter_preset(*idx, name.clone());
            }
            crate::panels::PanelMsg::ToggleCustomFilterPresetMember(idx, filter) => {
                self.handle_toggle_custom_filter_preset_member(*idx, *filter);
            }
            crate::panels::PanelMsg::CaptureCustomFilterPreset(idx) => {
                self.handle_capture_custom_filter_preset(*idx);
            }
            crate::panels::PanelMsg::SelectCustomFilterTab(idx) => {
                self.handle_select_custom_filter_tab(*idx);
            }
            _ => return None,
        }

        Some(follow)
    }
}
