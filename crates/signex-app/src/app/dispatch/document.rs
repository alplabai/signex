use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_document_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FileOpened(path) => {
                self.handle_document_file_opened(path);
                self.finish_update()
            }
            Message::DeleteSelected => {
                // Component Editor footprint tab has its own
                // selection model and preempts Delete when a pad is
                // currently selected. We check every open editor
                // because the global keyboard listener doesn't carry
                // a window-id; the first match (if any) wins. Falls
                // through to the schematic delete when nothing
                // matches.
                let footprint_target = self
                    .library
                    .open_editors
                    .iter()
                    .find(|(_id, ed)| {
                        ed.active_tab == crate::library::state::EditorTab::Footprint
                            && ed
                                .footprint_state
                                .as_ref()
                                .and_then(|s| s.selected_pad)
                                .is_some()
                    })
                    .map(|(id, _)| *id);
                if let Some(id) = footprint_target {
                    return self.dispatch_library_message(
                        crate::library::messages::LibraryMessage::EditorEvent {
                            window_id: id,
                            msg: crate::library::messages::EditorMsg::FootprintDeleteSelected,
                        },
                    );
                }
                self.handle_selection_delete_requested();
                self.finish_update()
            }
            Message::Undo => {
                self.handle_undo_requested();
                self.finish_update()
            }
            Message::Redo => {
                self.handle_redo_requested();
                self.finish_update()
            }
            Message::RotateSelected => {
                self.handle_selection_rotate_requested();
                self.finish_update()
            }
            Message::MirrorSelectedX => {
                self.handle_selection_mirror_x_requested();
                self.finish_update()
            }
            Message::MirrorSelectedY => {
                self.handle_selection_mirror_y_requested();
                self.finish_update()
            }
            Message::Cut => self.handle_selection_cut_requested(),
            Message::Copy => {
                self.handle_selection_copy_requested();
                self.finish_update()
            }
            Message::Paste => {
                self.handle_clipboard_paste_requested();
                self.finish_update()
            }
            Message::SmartPaste => {
                self.handle_clipboard_smart_paste_requested();
                self.finish_update()
            }
            Message::Duplicate => {
                self.handle_selection_duplicate_requested();
                self.finish_update()
            }
            Message::SaveFile => {
                self.handle_active_document_save_requested();
                self.finish_update()
            }
            Message::SaveFileAs(path) => {
                self.handle_active_document_save_as_requested(path);
                self.finish_update()
            }
            Message::SchematicLoaded(sheet) => {
                self.load_schematic_into_active_tab(*sheet);
                self.finish_update()
            }
            Message::ExportPdfFinished(result) => {
                let task = self.handle_export_pdf_finished(result);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::ExportNetlistFinished(result) => {
                let task = self.handle_export_netlist_finished(result);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::PrintPreviewRequested => {
                let task = self.handle_print_preview_requested();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::PrintPreviewSelectPage(idx) => {
                self.handle_print_preview_select_page(idx);
                self.finish_update()
            }
            Message::PrintPreviewSetColourMode(mode) => {
                self.handle_print_preview_set_colour_mode(mode);
                self.finish_update()
            }
            Message::PrintPreviewSetPageRangeAll => {
                self.handle_print_preview_set_page_range_all();
                self.finish_update()
            }
            Message::PrintPreviewSetPageRangeCurrent => {
                self.handle_print_preview_set_page_range_current();
                self.finish_update()
            }
            Message::PrintPreviewSetPageRangeSpecific => {
                self.handle_print_preview_set_page_range_specific();
                self.finish_update()
            }
            Message::PrintPreviewSetSpecificPageInput(value) => {
                self.handle_print_preview_set_specific_page_input(value);
                self.finish_update()
            }
            Message::PrintPreviewSetFitToPage(fit) => {
                self.handle_print_preview_set_fit_to_page(fit);
                self.finish_update()
            }
            Message::PrintPreviewSetIncludeTitleBlock(include) => {
                self.handle_print_preview_set_include_title_block(include);
                self.finish_update()
            }
            Message::PrintPreviewZoom(delta) => {
                self.handle_print_preview_zoom(delta);
                self.finish_update()
            }
            Message::PrintPreviewExport => {
                let task = self
                    .handle_print_preview_export()
                    .unwrap_or_else(iced::Task::none);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::PrintPreviewClose => {
                let task = self.handle_print_preview_close();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::PrintPreviewSetTab(tab) => {
                self.handle_print_preview_set_tab(tab);
                self.finish_update()
            }
            Message::PrintPreviewPanStart => {
                self.handle_print_preview_pan_start();
                self.finish_update()
            }
            Message::PrintPreviewPanFinished => {
                self.handle_print_preview_pan_finished();
                self.finish_update()
            }
            Message::PrintPreviewToggleFile(path) => {
                self.handle_print_preview_toggle_file(path);
                self.finish_update()
            }
            Message::PrintPreviewSelectAllFiles => {
                self.handle_print_preview_select_all_files();
                self.finish_update()
            }
            Message::PrintPreviewClearAllFiles => {
                self.handle_print_preview_clear_all_files();
                self.finish_update()
            }
            Message::PrintPreviewSetVariant(v) => {
                self.handle_print_preview_set_variant(v);
                self.finish_update()
            }
            // Visual toggles (No-ERC Markers, Notes) — affect what
            // the SVG renderer emits, so we mutate `pdf_options`
            // directly and trigger a rerasterize so the Preview tab
            // reflects the change immediately.
            Message::PrintPreviewSetIncludeNoErcMarkers(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_no_erc_markers = v;
                }
                self.handle_print_preview_rerender();
                self.finish_update()
            }
            Message::PrintPreviewSetIncludeNotes(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_notes = v;
                }
                self.handle_print_preview_rerender();
                self.finish_update()
            }
            // Bookkeeping toggles — stored on `pdf_options` for the
            // exporter to honour later, but no render hookup yet so
            // skip the rerasterize. Adding render support is a
            // one-line move into the visual-toggle group above.
            Message::PrintPreviewSetUsePhysicalStructure(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.use_physical_structure = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetPhysicalDesignators(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_designators = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetPhysicalNetLabels(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_net_labels = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetPhysicalPorts(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_ports = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetPhysicalSheetNumber(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_sheet_number = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetPhysicalDocumentNumber(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.physical_document_number = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetIncludeParameterSets(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_parameter_sets = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetIncludeProbes(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_probes = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetIncludeBlankets(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_blankets = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetIncludeCollapsedNotes(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_collapsed_notes = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetQuality(q) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.quality = q;
                }
                // Visual toggle — preview rasterises at the new
                // DPI so the user sees the picker reflected
                // immediately rather than only at next export.
                self.handle_print_preview_rerender();
                self.finish_update()
            }
            Message::PrintPreviewSetBookmarkZoom(z) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.bookmark_zoom = z.clamp(0.0, 1.0);
                }
                self.finish_update()
            }
            Message::PrintPreviewSetGenerateNetsInfo(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.generate_nets_info = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetBookmarkPins(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.bookmark_pins = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetBookmarkNetLabels(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.bookmark_net_labels = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetBookmarkPorts(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.bookmark_ports = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetIncludeComponentParameters(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_component_parameters = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetGlobalBookmarks(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.global_bookmarks = v;
                }
                self.finish_update()
            }
            Message::PrintPreviewSetPcbColourMode(m) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.pcb_colour_mode = m;
                }
                self.finish_update()
            }
            Message::ExportPdfOpenDialog => {
                let task = self.handle_export_pdf_open_dialog();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::DismissExportError => {
                self.handle_dismiss_export_error();
                self.finish_update()
            }
            Message::ExportBomRequested => {
                let task = self.handle_bom_preview_open();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::ExportBomFinished(result) => {
                let task = self.handle_export_bom_finished(result);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::BomPreviewSetGrouping(g) => {
                self.handle_bom_preview_set_grouping(g);
                self.finish_update()
            }
            Message::BomPreviewSetFormat(f) => {
                self.handle_bom_preview_set_format(f);
                self.finish_update()
            }
            Message::BomPreviewSetIncludeDnp(b) => {
                self.handle_bom_preview_set_include_dnp(b);
                self.finish_update()
            }
            Message::BomPreviewSetIncludeNotFitted(b) => {
                self.handle_bom_preview_set_include_not_fitted(b);
                self.finish_update()
            }
            Message::BomPreviewToggleColumn(col) => {
                self.handle_bom_preview_toggle_column(col);
                self.finish_update()
            }
            Message::BomPreviewSetVariant(v) => {
                self.handle_bom_preview_set_variant(v);
                self.finish_update()
            }
            Message::BomPreviewSortColumn(idx) => {
                self.handle_bom_preview_sort_column(idx);
                self.finish_update()
            }
            Message::BomPreviewColumnDragStart(idx) => {
                self.handle_bom_preview_column_drag_start(idx);
                self.finish_update()
            }
            Message::BomPreviewColumnDragDrop(idx) => {
                self.handle_bom_preview_column_drag_drop(idx);
                self.finish_update()
            }
            Message::BomPreviewColumnHoverEnter(idx) => {
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    p.column_hover = Some(idx);
                }
                self.finish_update()
            }
            Message::BomPreviewColumnHoverExit(idx) => {
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    if p.column_hover == Some(idx) {
                        p.column_hover = None;
                    }
                }
                self.finish_update()
            }
            Message::BomPreviewColumnResizeStart(idx) => {
                let cursor_x = self.interaction_state.last_mouse_pos.0;
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    let start_width = p.column_widths.get(&idx).copied().unwrap_or_else(|| {
                        // Fall back to the per-BomColumn default
                        // table the view function uses.
                        use signex_output::BomColumn;
                        match p.options.columns.get(idx) {
                            Some(BomColumn::Name) => 140.0,
                            Some(BomColumn::Description) => 220.0,
                            Some(BomColumn::Designator) | Some(BomColumn::Reference) => 220.0,
                            Some(BomColumn::Value) => 110.0,
                            Some(BomColumn::Footprint) => 140.0,
                            Some(BomColumn::LibRef) => 160.0,
                            Some(BomColumn::Qty) => 50.0,
                            Some(BomColumn::Custom(_)) => 120.0,
                            None => 120.0,
                        }
                    });
                    p.column_resize = Some(crate::app::state::ColumnResizeState {
                        idx,
                        start_x: cursor_x,
                        start_width,
                    });
                }
                self.finish_update()
            }
            Message::BomPreviewColumnResizeEnd => {
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    p.column_resize = None;
                }
                self.finish_update()
            }
            Message::BomPreviewSetSidebarTab(tab) => {
                if let Some(p) = self.document_state.bom_preview.as_mut() {
                    p.sidebar_tab = tab;
                }
                self.finish_update()
            }
            Message::BomPreviewExport => {
                let task = self
                    .handle_bom_preview_export()
                    .unwrap_or_else(iced::Task::none);
                iced::Task::batch([task, self.finish_update()])
            }
            Message::BomPreviewClose => {
                let task = self.handle_bom_preview_close();
                iced::Task::batch([task, self.finish_update()])
            }
            _ => unreachable!("dispatch_document_message received non-document message"),
        }
    }
}
