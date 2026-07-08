use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_document_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FileOpened(path) => {
                self.handle_document_file_opened(path);
                self.finish_update()
            }
            Message::NewProjectFile(path) => {
                self.handle_new_project_file(path);
                self.finish_update()
            }
            Message::DeleteSelected => {
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
                if let Some(path) = footprint_path {
                    let _ = self.update(Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path,
                            msg: crate::library::messages::PrimitiveEdit::Footprint(
                                crate::library::messages::FootprintEditorMsg::DeleteSelected,
                            ),
                        },
                    ));
                } else {
                    self.handle_selection_delete_requested();
                }
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
                let task = self.handle_active_document_save_requested();
                iced::Task::batch([task, self.finish_update()])
            }
            Message::SaveFileAs(path) => {
                self.handle_active_document_save_as_requested(path);
                self.finish_update()
            }
            Message::SavePrimitiveAs { from_path, to_path } => {
                self.handle_save_primitive_as(&from_path, &to_path);
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

    /// Print-preview modal message handler (namespaced family, ADR-0001 D3).
    pub(crate) fn dispatch_print_preview_message(&mut self, msg: PrintPreviewMsg) -> Task<Message> {
        match msg {
            PrintPreviewMsg::Requested => {
                let task = self.handle_print_preview_requested();
                iced::Task::batch([task, self.finish_update()])
            }
            PrintPreviewMsg::SelectPage(idx) => {
                self.handle_print_preview_select_page(idx);
                self.finish_update()
            }
            PrintPreviewMsg::SetColourMode(mode) => {
                self.handle_print_preview_set_colour_mode(mode);
                self.finish_update()
            }
            PrintPreviewMsg::SetPageRangeAll => {
                self.handle_print_preview_set_page_range_all();
                self.finish_update()
            }
            PrintPreviewMsg::SetPageRangeCurrent => {
                self.handle_print_preview_set_page_range_current();
                self.finish_update()
            }
            PrintPreviewMsg::SetPageRangeSpecific => {
                self.handle_print_preview_set_page_range_specific();
                self.finish_update()
            }
            PrintPreviewMsg::SetSpecificPageInput(value) => {
                self.handle_print_preview_set_specific_page_input(value);
                self.finish_update()
            }
            PrintPreviewMsg::SetFitToPage(fit) => {
                self.handle_print_preview_set_fit_to_page(fit);
                self.finish_update()
            }
            PrintPreviewMsg::SetIncludeTitleBlock(include) => {
                self.handle_print_preview_set_include_title_block(include);
                self.finish_update()
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
            PrintPreviewMsg::ToggleFile(path) => {
                self.handle_print_preview_toggle_file(path);
                self.finish_update()
            }
            PrintPreviewMsg::SelectAllFiles => {
                self.handle_print_preview_select_all_files();
                self.finish_update()
            }
            PrintPreviewMsg::ClearAllFiles => {
                self.handle_print_preview_clear_all_files();
                self.finish_update()
            }
            PrintPreviewMsg::SetVariant(v) => {
                self.handle_print_preview_set_variant(v);
                self.finish_update()
            }
            // Visual toggles (No-ERC Markers, Notes) — affect what
            // the SVG renderer emits, so we mutate `pdf_options`
            // directly and trigger a rerasterize so the Preview tab
            // reflects the change immediately.
            PrintPreviewMsg::SetIncludeNoErcMarkers(v) => {
                if let Some(p) = self.document_state.preview.as_mut() {
                    p.pdf_options.include_no_erc_markers = v;
                }
                self.handle_print_preview_rerender();
                self.finish_update()
            }
            PrintPreviewMsg::SetIncludeNotes(v) => {
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
                self.handle_print_preview_rerender();
                self.finish_update()
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
