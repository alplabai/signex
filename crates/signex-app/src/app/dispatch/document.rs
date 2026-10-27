use iced::Task;

use super::super::*;

impl Signex {
    /// Edit-command message handler (namespaced family, ADR-0001 D3).
    /// Delete / undo / redo / rotate / mirror / clipboard (copy, cut,
    /// paste, smart-paste) / duplicate. Routed from `dispatch_update`
    /// via `Message::Edit`.
    pub(crate) fn dispatch_edit_message(&mut self, msg: EditMsg) -> Task<Message> {
        match msg {
            EditMsg::UpdateDrawingField(uuid, edit) => self.handle_update_drawing_field(uuid, edit),
            EditMsg::DeleteSelected => {
                // v0.20 — if the active tab is a footprint editor,
                // route the Delete key to FootprintDeleteSelected so
                // the selected pad / silk graphic is removed via the
                // footprint dispatcher. Otherwise fall through to
                // the schematic engine's delete (Component Preview
                // is read-only, so it's a no-op there).
                let footprint_path = self
                    .document_state
                    .tabs
                    .get(self.document_state.active_tab)
                    .and_then(|t| t.kind.as_footprint_editor())
                    .cloned();
                let task = if let Some(path) = footprint_path {
                    self.update(Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path,
                            msg: crate::library::messages::PrimitiveEdit::Footprint(
                                crate::library::messages::FootprintEditorMsg::DeleteSelected,
                            ),
                        },
                    ))
                } else {
                    self.handle_selection_delete_requested();
                    Task::none()
                };
                Task::batch([task, self.finish_update()])
            }
            EditMsg::Undo => {
                self.cancel_move_pick();
                self.handle_undo_requested();
                self.finish_update()
            }
            EditMsg::Redo => {
                self.cancel_move_pick();
                self.handle_redo_requested();
                self.finish_update()
            }
            EditMsg::RotateSelectedClockwise => {
                self.handle_selection_rotate_requested(true);
                self.finish_update()
            }
            EditMsg::RotateSelectedCounterclockwise => {
                self.handle_selection_rotate_requested(false);
                self.finish_update()
            }
            EditMsg::MirrorSelectedX => {
                self.handle_selection_mirror_x_requested();
                self.finish_update()
            }
            EditMsg::MirrorSelectedY => {
                self.handle_selection_mirror_y_requested();
                self.finish_update()
            }
            EditMsg::Cut => self.handle_selection_cut_requested(),
            EditMsg::Copy => {
                self.handle_selection_copy_requested();
                self.finish_update()
            }
            EditMsg::Paste => {
                self.handle_clipboard_paste_requested();
                self.finish_update()
            }
            EditMsg::SmartPaste => {
                self.handle_clipboard_smart_paste_requested();
                self.finish_update()
            }
            EditMsg::Duplicate => {
                self.handle_selection_duplicate_requested();
                self.finish_update()
            }
        }
    }

    /// File / save message handler (namespaced family, ADR-0001 D3).
    /// Covers open / new-project / save / save-as / save-primitive-as
    /// plus the async `SchematicOpenFinished` / `PcbOpenFinished`
    /// completions.
    pub(crate) fn dispatch_file_message(&mut self, msg: FileMsg) -> Task<Message> {
        match msg {
            FileMsg::Opened(path) => {
                let task = self.handle_document_file_opened(path);
                Task::batch([task, self.finish_update()])
            }
            FileMsg::NewProject(path) => {
                let task = self.handle_new_project_file(path);
                Task::batch([task, self.finish_update()])
            }
            FileMsg::NewProjectWritten { path, result } => {
                let task = self.handle_new_project_written(path, result);
                Task::batch([task, self.finish_update()])
            }
            FileMsg::SampleProjectUnpacked(result) => {
                let task = self.handle_sample_project_unpacked(result);
                Task::batch([task, self.finish_update()])
            }
            FileMsg::AltiumImportPicked(path) => {
                let task = self.handle_altium_import_picked(path);
                Task::batch([task, self.finish_update()])
            }
            FileMsg::AltiumImportFinished(result) => {
                let task = self.handle_altium_import_finished(result);
                Task::batch([task, self.finish_update()])
            }
            FileMsg::Save => {
                let task = self.handle_active_document_save_requested();
                iced::Task::batch([task, self.finish_update()])
            }
            FileMsg::SaveAs { from_path, to_path } => {
                let task = self.handle_active_document_save_as_requested(from_path, to_path);
                iced::Task::batch([task, self.finish_update()])
            }
            FileMsg::DocumentSaveWaitFailed(reason) => {
                crate::diagnostics::log_error(
                    "Could not confirm that document writes finished before exit",
                    &anyhow::anyhow!(reason),
                );
                self.finish_update()
            }
            FileMsg::DocumentSaved(done) => {
                let task = self.handle_document_saved(done);
                iced::Task::batch([task, self.finish_update()])
            }
            FileMsg::PrimitiveSaved(done) => {
                let task = self.handle_primitive_saved(*done);
                iced::Task::batch([task, self.finish_update()])
            }
            FileMsg::SaveAllProbed(plan) => {
                let task = self.begin_save_all_planned(*plan);
                iced::Task::batch([task, self.finish_update()])
            }
            FileMsg::SaveAllSettled(settle) => {
                let task = self.handle_save_all_settled(*settle);
                iced::Task::batch([task, self.finish_update()])
            }
            FileMsg::SavePrimitiveAs { from_path, to_path } => {
                let task = match self.handle_save_primitive_as(&from_path, &to_path) {
                    Ok(task) => task,
                    Err(error) => {
                        self.report_save_failure("Save As failed", &error);
                        Task::none()
                    }
                };
                iced::Task::batch([task, self.finish_update()])
            }
            FileMsg::DocumentReadFinished { request, result } => {
                let task = self.handle_document_read_finished(request, result);
                Task::batch([task, self.finish_update()])
            }
            FileMsg::HistoryRestoreFinished {
                request,
                sha,
                edit_stamp,
                result,
            } => {
                let task = self.handle_history_restore_finished(request, &sha, edit_stamp, result);
                Task::batch([task, self.finish_update()])
            }
            FileMsg::SchematicOpenFinished {
                path,
                title,
                result,
            } => {
                // #478 review — clear the in-flight guard before acting
                // on the result so a duplicate open fired while this one
                // was in flight is free to spawn again from here on.
                self.document_state.pending_opens.remove_untracked(&path);
                if self.is_open_result_superseded(&path, &result) {
                    self.attach_parked_tab_for_superseded_open(&path, title);
                    self.activate_tab_for_path(&path);
                    return self.finish_update();
                }
                match result {
                    Ok(_) if self.has_tab_for(&path) => Self::log_duplicate_open(&path),
                    Ok(sheet) => {
                        // `update` starts the recovery check of every tab opened.
                        self.open_schematic_tab(path, title, *sheet);
                    }
                    Err(message) => self.report_open_failure(message),
                }
                self.finish_update()
            }
            FileMsg::InstanceNoteReady(note) => {
                match note {
                    Ok(Some(note)) => crate::diagnostics::log_info(note),
                    Ok(None) => {}
                    Err(error) => crate::diagnostics::log_warning(format!(
                        "Sheet-placement designator note failed: {error}"
                    )),
                }
                self.finish_update()
            }
            FileMsg::PcbOpenFinished {
                path,
                title,
                result,
            } => {
                // #478 review — same in-flight guard clear as above.
                self.document_state.pending_opens.remove_untracked(&path);
                if self.is_open_result_superseded(&path, &result) {
                    self.activate_tab_for_path(&path);
                    return self.finish_update();
                }
                match result {
                    Ok(_) if self.has_tab_for(&path) => Self::log_duplicate_open(&path),
                    Ok(board) => self.open_pcb_tab(path, title, *board),
                    Err(message) => self.report_open_failure(message),
                }
                self.finish_update()
            }
        }
    }

    /// Export subsystem message handler (namespaced family, ADR-0001 D3).
    /// PDF / netlist / BOM export lifecycle plus the export-error modal
    /// dismiss. Some arms delegate to helpers in
    /// `app/handlers/menu/export.rs`.
    pub(crate) fn dispatch_export_message(&mut self, msg: ExportMsg) -> Task<Message> {
        match msg {
            ExportMsg::PdfFinished { request, result } => {
                let task = self.handle_export_pdf_finished(request, result);
                iced::Task::batch([task, self.finish_update()])
            }
            ExportMsg::NetlistFinished { source, result } => {
                let task = self.handle_export_netlist_finished(source, result);
                iced::Task::batch([task, self.finish_update()])
            }
            ExportMsg::PdfOpenDialog => {
                let task = self.handle_export_pdf_open_dialog();
                iced::Task::batch([task, self.finish_update()])
            }
            ExportMsg::NetlistExportAnyway => {
                let task = self.handle_netlist_export_anyway();
                iced::Task::batch([task, self.finish_update()])
            }
            ExportMsg::NetlistCancelIncomplete => {
                self.handle_netlist_cancel_incomplete();
                self.finish_update()
            }
            ExportMsg::BomRequested => {
                let task = self.handle_bom_preview_open();
                iced::Task::batch([task, self.finish_update()])
            }
            ExportMsg::BomFinished {
                source,
                options,
                result,
            } => {
                let task = self.handle_export_bom_finished(source, *options, result);
                iced::Task::batch([task, self.finish_update()])
            }
            ExportMsg::ErcReportPathChosen(result) => {
                let task = self.handle_erc_report_path_chosen(result);
                iced::Task::batch([task, self.finish_update()])
            }
            ExportMsg::FileWritten(outcome) => {
                let task = self.handle_export_written(*outcome);
                iced::Task::batch([task, self.finish_update()])
            }
            ExportMsg::ErcReportWritten(result) => {
                let task = self.handle_erc_report_written(result);
                iced::Task::batch([task, self.finish_update()])
            }
        }
    }

    /// BOM-preview modal message handler (namespaced family, ADR-0001 D3).
    pub(crate) fn dispatch_bom_preview_message(&mut self, msg: BomPreviewMsg) -> Task<Message> {
        // Pure pointer/UI-state messages cannot change the row order; every
        // other message (and any future one) re-sorts after it ran.
        let reorders = !matches!(
            msg,
            BomPreviewMsg::ColumnHoverEnter(_)
                | BomPreviewMsg::ColumnHoverExit(_)
                | BomPreviewMsg::ColumnDragStart(_)
                | BomPreviewMsg::ColumnResizeStart(_)
                | BomPreviewMsg::ColumnResizeEnd
                | BomPreviewMsg::SetSidebarTab(_)
        );
        let task = self.dispatch_bom_preview_message_inner(msg);
        if reorders && let Some(p) = self.document_state.bom_preview.as_mut() {
            p.row_order = crate::app::view::dialogs::sorted_row_order(
                &p.table.rows,
                &p.options.columns,
                p.sort,
            );
        }
        task
    }

    fn dispatch_bom_preview_message_inner(&mut self, msg: BomPreviewMsg) -> Task<Message> {
        match msg {
            BomPreviewMsg::SetGrouping(g) => {
                self.handle_bom_preview_set_grouping(g);
                self.finish_update()
            }
            BomPreviewMsg::SetFormat(f) => {
                self.handle_bom_preview_set_format(f);
                self.finish_update()
            }
            BomPreviewMsg::SetIncludeDnp(b) => {
                self.handle_bom_preview_set_include_dnp(b);
                self.finish_update()
            }
            BomPreviewMsg::SetIncludeNotFitted(b) => {
                self.handle_bom_preview_set_include_not_fitted(b);
                self.finish_update()
            }
            BomPreviewMsg::SetRule(rule, enabled) => {
                self.handle_bom_preview_set_rule(rule, enabled);
                self.finish_update()
            }
            BomPreviewMsg::ToggleColumn(col) => {
                self.handle_bom_preview_toggle_column(col);
                self.finish_update()
            }
            BomPreviewMsg::SetVariant(v) => {
                self.handle_bom_preview_set_variant(v);
                self.finish_update()
            }
            BomPreviewMsg::SortColumn(idx) => {
                self.handle_bom_preview_sort_column(idx);
                self.finish_update()
            }
            BomPreviewMsg::ColumnDragStart(idx) => {
                self.handle_bom_preview_column_drag_start(idx);
                self.finish_update()
            }
            BomPreviewMsg::ColumnDragDrop(idx) => {
                self.handle_bom_preview_column_drag_drop(idx);
                self.finish_update()
            }
            BomPreviewMsg::ColumnHoverEnter(idx) => {
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    p.column_hover = Some(idx);
                }
                self.finish_update()
            }
            BomPreviewMsg::ColumnHoverExit(idx) => {
                if let Some(p) = self.document_state.bom_preview.as_mut()
                    && p.column_hover == Some(idx)
                {
                    p.column_hover = None;
                }
                self.finish_update()
            }
            BomPreviewMsg::ColumnResizeStart(idx) => {
                let cursor_x = self.interaction_state.last_mouse_pos.0;
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    let start_width = p.column_widths.get(&idx).copied().unwrap_or_else(|| {
                        // Fall back to the per-BomColumn default
                        // table the view function uses.
                        p.options
                            .columns
                            .get(idx)
                            .map_or(120.0, crate::app::view::dialogs::bom_default_column_width)
                    });
                    p.column_resize = Some(crate::app::state::ColumnResizeState {
                        idx,
                        start_x: cursor_x,
                        start_width,
                    });
                }
                self.finish_update()
            }
            BomPreviewMsg::ColumnResizeEnd => {
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    p.column_resize = None;
                }
                self.finish_update()
            }
            BomPreviewMsg::SetSidebarTab(tab) => {
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    p.sidebar_tab = tab;
                }
                self.finish_update()
            }
            BomPreviewMsg::Export => {
                let task = self
                    .handle_bom_preview_export()
                    .unwrap_or_else(iced::Task::none);
                iced::Task::batch([task, self.finish_update()])
            }
            BomPreviewMsg::Close => {
                let task = self.handle_bom_preview_close();
                iced::Task::batch([task, self.finish_update()])
            }
        }
    }

    /// Print-preview modal message handler (namespaced family, ADR-0001 D3).
    pub(crate) fn dispatch_print_preview_message(&mut self, msg: PrintPreviewMsg) -> Task<Message> {
        match msg {
            PrintPreviewMsg::Requested => {
                let task = self.handle_print_preview_requested();
                iced::Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::Rasterized {
                generation,
                outline,
                pages,
            } => {
                let task = self.handle_print_preview_rasterized(generation, &outline, pages);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::PagesRasterized { generation, pages } => {
                self.handle_print_preview_pages_rasterized(generation, pages);
                self.finish_update()
            }
            PrintPreviewMsg::RasterFailed { generation, reason } => {
                self.handle_print_preview_raster_failed(generation, &reason);
                self.finish_update()
            }
            PrintPreviewMsg::SelectPage(idx) => {
                let task = self.handle_print_preview_select_page(idx);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetColourMode(mode) => {
                let task = self.handle_print_preview_set_colour_mode(mode);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetPageRangeAll => {
                let task = self.handle_print_preview_set_page_range_all();
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetPageRangeCurrent => {
                let task = self.handle_print_preview_set_page_range_current();
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetPageRangeSpecific => {
                let task = self.handle_print_preview_set_page_range_specific();
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetSpecificPageInput(value) => {
                let task = self.handle_print_preview_set_specific_page_input(value);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetPageSize(size) => {
                let task = self.handle_print_preview_set_page_size(size);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetOrientation(orientation) => {
                let task = self.handle_print_preview_set_orientation(orientation);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetCustomWidth(text) => {
                let task = self.handle_print_preview_set_custom_dimension(true, text);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetCustomHeight(text) => {
                let task = self.handle_print_preview_set_custom_dimension(false, text);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetMargin(edge, text) => {
                let task = self.handle_print_preview_set_margin(edge, text);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetScaleMode(mode) => {
                let task = self.handle_print_preview_set_scale_mode(mode);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetScalePercent(text) => {
                let task = self.handle_print_preview_set_scale_percent(text);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetIncludeTitleBlock(include) => {
                let task = self.handle_print_preview_set_include_title_block(include);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::Zoom(delta) => {
                self.handle_print_preview_zoom(delta);
                self.finish_update()
            }
            PrintPreviewMsg::Export => {
                let task = self
                    .handle_print_preview_export()
                    .unwrap_or_else(iced::Task::none);
                iced::Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::Close => {
                let task = self.handle_print_preview_close();
                iced::Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetTab(tab) => {
                self.handle_print_preview_set_tab(tab);
                self.finish_update()
            }
            PrintPreviewMsg::PanStart => {
                self.handle_print_preview_pan_start();
                self.finish_update()
            }
            PrintPreviewMsg::PanFinished => {
                self.handle_print_preview_pan_finished();
                self.finish_update()
            }
            PrintPreviewMsg::ToggleFile(page) => {
                let task = self.handle_print_preview_toggle_file(page);
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SelectAllFiles => {
                let task = self.handle_print_preview_select_all_files();
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::ClearAllFiles => {
                let task = self.handle_print_preview_clear_all_files();
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetVariant(v) => {
                self.handle_print_preview_set_variant(v);
                self.finish_update()
            }
            // Visual toggles (No-ERC Markers, No-Connect Markers, Notes) — affect what
            // the SVG renderer emits, so we mutate `pdf_options`
            // directly and trigger a rerasterize so the Preview tab
            // reflects the change immediately.
            PrintPreviewMsg::SetIncludeNoErcMarkers(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_no_erc_markers = v;
                }
                let task = self.handle_print_preview_rerender();
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetIncludeNoConnectMarkers(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_no_connect_markers = v;
                }
                let task = self.handle_print_preview_rerender();
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetIncludeNotes(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_notes = v;
                }
                let task = self.handle_print_preview_rerender();
                Task::batch([task, self.finish_update()])
            }
            // Bookkeeping toggles — stored on `pdf_options` for the
            // exporter to honour later, but no render hookup yet so
            // skip the rerasterize. Adding render support is a
            // one-line move into the visual-toggle group above.
            PrintPreviewMsg::SetUsePhysicalStructure(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.use_physical_structure = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetPhysicalDesignators(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_designators = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetPhysicalNetLabels(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_net_labels = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetPhysicalPorts(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_ports = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetPhysicalSheetNumber(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_sheet_number = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetPhysicalDocumentNumber(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_document_number = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetIncludeParameterSets(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_parameter_sets = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetIncludeProbes(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_probes = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetIncludeBlankets(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_blankets = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetIncludeCollapsedNotes(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_collapsed_notes = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetQuality(q) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.quality = q;
                }
                // Visual toggle — preview rasterises at the new
                // DPI so the user sees the picker reflected
                // immediately rather than only at next export.
                let task = self.handle_print_preview_rerender();
                Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SetBookmarkZoom(z) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.bookmark_zoom = z.clamp(0.0, 1.0);
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetGenerateNetsInfo(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.generate_nets_info = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetBookmarkPins(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.bookmark_pins = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetBookmarkNetLabels(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.bookmark_net_labels = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetBookmarkPorts(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.bookmark_ports = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetIncludeComponentParameters(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_component_parameters = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetGlobalBookmarks(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.global_bookmarks = v;
                }
                self.finish_update()
            }
            PrintPreviewMsg::SetPcbColourMode(m) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.pcb_colour_mode = m;
                }
                self.finish_update()
            }
        }
    }
}
