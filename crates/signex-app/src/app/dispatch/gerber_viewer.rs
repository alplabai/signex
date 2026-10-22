use iced::Task;

use super::super::*;
use signex_widgets::gerber_viewer::{
    GerberDockPanel, GerberDocumentId, GerberShortcutResolver, GerberViewerMessage,
    GerberViewerState,
};
use signex_widgets::grid_editor::{
    GerberGridEditorMessage, GerberGridEditorOutcome, GerberGridEditorState,
};

const GERBER_RS274X_EXTENSIONS: &[&str] = &[
    "gbr", "ger", "pho", "art", "gbx", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gtp", "gbp",
    "gm1", "gm2", "gko", "gvc", "gsp",
];
const EXCELLON_DRILL_EXTENSIONS: &[&str] = &["drl", "drd"];
const GERBER_JOB_EXTENSIONS: &[&str] = &["gbrjob"];
const ZIP_ARCHIVE_EXTENSIONS: &[&str] = &["zip"];
const ALL_SUPPORTED_FABRICATION_EXTENSIONS: &[&str] = &[
    "gbr", "ger", "pho", "art", "gbx", "gtl", "gbl", "gto", "gbo", "gts", "gbs", "gtp", "gbp",
    "gm1", "gm2", "gko", "gvc", "gsp", "drl", "drd", "gbrjob", "zip",
];
const FABRICATION_FILE_FILTERS: [(&str, &[&str]); 6] = [
    (
        "All supported file formats",
        ALL_SUPPORTED_FABRICATION_EXTENSIONS,
    ),
    ("Gerber RS-274X", GERBER_RS274X_EXTENSIONS),
    ("Excellon Drill", EXCELLON_DRILL_EXTENSIONS),
    ("Gerber job", GERBER_JOB_EXTENSIONS),
    ("ZIP archive", ZIP_ARCHIVE_EXTENSIONS),
    ("All files", &["*"]),
];

impl GerberShortcutResolver for crate::keymap::CompiledKeymap {
    fn resolve_gerber_shortcut(
        &self,
        key: &iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    ) -> Option<GerberViewerMessage> {
        let stroke = crate::keymap::KeyStroke::from_iced(key, modifiers)?;
        let lookup = self.lookup(
            &[stroke],
            &[
                crate::keymap::ShortcutContext::Global,
                crate::keymap::ShortcutContext::Gerber,
            ],
        );
        match lookup.command?.as_str() {
            "gerber_next_layer" => Some(GerberViewerMessage::NextLayer),
            "gerber_previous_layer" => Some(GerberViewerMessage::PreviousLayer),
            "gerber_move_layer_up" => Some(GerberViewerMessage::MoveLayerUp),
            "gerber_move_layer_down" => Some(GerberViewerMessage::MoveLayerDown),
            "gerber_sketch_flashes" => Some(GerberViewerMessage::ToggleSketchFlashes),
            "gerber_sketch_lines" => Some(GerberViewerMessage::ToggleSketchLines),
            "gerber_sketch_polygons" => Some(GerberViewerMessage::ToggleSketchPolygons),
            "gerber_show_d_codes" => Some(GerberViewerMessage::ToggleDCodeLabels),
            "gerber_compare_layers" => Some(GerberViewerMessage::ToggleCompareMode),
            "gerber_forced_opacity" => Some(GerberViewerMessage::ToggleForcedOpacityMode),
            "gerber_dim_inactive_layers" => Some(GerberViewerMessage::ToggleDimInactiveLayers),
            "gerber_flip_view" => Some(GerberViewerMessage::ToggleMirrored),
            "gerber_clear_highlight" => Some(GerberViewerMessage::ClearHighlight),
            "gerber_export_native_pcb" => Some(GerberViewerMessage::ExportNativePcb),
            "gerber_print" => Some(GerberViewerMessage::PrintVisibleLayers),
            "gerber_quit" => Some(GerberViewerMessage::CloseRequested),
            _ => None,
        }
    }
}

impl Signex {
    fn grid_editor_window_id(&self) -> Option<iced::window::Id> {
        self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, crate::app::state::WindowKind::GerberGridEditor { .. },).then_some(*id)
        })
    }

    fn gerber_viewer_window_id(&self) -> Option<iced::window::Id> {
        self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, crate::app::state::WindowKind::GerberViewer,).then_some(*id)
        })
    }

    fn handle_open_gerber_grid_editor(
        &mut self,
        document_id: GerberDocumentId,
        gerber_viewer: &GerberViewerState,
    ) -> Task<Message> {
        self.ui_state.gerber_grid_editor = Some(GerberGridEditorState::from_viewer(gerber_viewer));
        self.ui_state.gerber_grid_editor_document = Some(document_id);

        if let Some(id) = self.grid_editor_window_id() {
            self.ui_state.windows.insert(
                id,
                crate::app::state::WindowKind::GerberGridEditor { document_id },
            );
            return iced::window::gain_focus(id);
        }

        let (_id, open_task) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(860.0, 580.0),
            min_size: Some(iced::Size::new(720.0, 460.0)),
            icon: crate::app::bootstrap::bundled_window_icon(),
            decorations: false,
            ..Default::default()
        });
        open_task.map(move |window_id| Message::GerberGridEditorOpened {
            document_id,
            window_id,
        })
    }

    pub(super) fn dispatch_gerber_grid_editor_message(
        &mut self,
        message: GerberGridEditorMessage,
    ) -> Task<Message> {
        let outcome = match self.ui_state.gerber_grid_editor.as_mut() {
            Some(editor) => editor.update(message),
            None => return Task::none(),
        };

        match outcome {
            GerberGridEditorOutcome::None => Task::none(),
            GerberGridEditorOutcome::Cancel => self
                .grid_editor_window_id()
                .map_or_else(Task::none, iced::window::close),
            GerberGridEditorOutcome::Apply => {
                let Some(document_id) = self.ui_state.gerber_grid_editor_document else {
                    return Task::none();
                };
                let editor = self
                    .ui_state
                    .gerber_grid_editor
                    .take()
                    .expect("grid editor exists while applying");
                let result = self
                    .ui_state
                    .gerber_workspace
                    .viewer_mut(document_id)
                    .ok_or_else(|| {
                        "The Gerber document was closed while its grid editor was open.".to_owned()
                    })
                    .and_then(|viewer| editor.apply_to(viewer));
                self.ui_state.gerber_grid_editor = Some(editor);
                match result {
                    Ok(()) => self
                        .grid_editor_window_id()
                        .map_or_else(Task::none, iced::window::close),
                    Err(error) => {
                        if let Some(editor) = self.ui_state.gerber_grid_editor.as_mut() {
                            editor.set_error(error);
                        }
                        Task::none()
                    }
                }
            }
        }
    }

    pub(super) fn handle_open_gerber_viewer(&mut self) -> Task<Message> {
        if let Some(id) = self.gerber_viewer_window_id() {
            return iced::window::gain_focus(id);
        }

        let (_id, open_task) = iced::window::open(iced::window::Settings {
            size: iced::Size::new(1280.0, 800.0),
            min_size: Some(iced::Size::new(860.0, 560.0)),
            icon: crate::app::bootstrap::bundled_window_icon(),
            decorations: false,
            ..Default::default()
        });
        open_task.map(Message::GerberViewerOpened)
    }

    fn apply_gerber_load_batch(
        &mut self,
        document_id: GerberDocumentId,
        gerber_viewer: &mut GerberViewerState,
        batch: signex_gerber::GerberLoadBatch,
    ) {
        if gerber_viewer.layers.is_empty()
            && let Some(first_layer) = batch.layers.first()
        {
            let title = match batch.layers.len() {
                1 => first_layer.name.clone(),
                count => format!("{} +{}", first_layer.name, count - 1),
            };
            self.ui_state
                .gerber_workspace
                .rename_document(document_id, title);
        }

        gerber_viewer.apply_load_batch(batch);
    }

    pub(super) fn dispatch_gerber_viewer_message(
        &mut self,
        document_id: GerberDocumentId,
        message: GerberViewerMessage,
    ) -> Task<Message> {
        if matches!(message, GerberViewerMessage::CloseRequested) {
            return self
                .gerber_viewer_window_id()
                .map_or_else(Task::none, iced::window::close);
        }
        if matches!(message, GerberViewerMessage::NewDocument) {
            self.ui_state.gerber_workspace.new_document();
            return Task::none();
        }
        if let GerberViewerMessage::DockEvent(event) = &message {
            let closed_document = self.ui_state.gerber_workspace.handle_dock_event(event);
            let grid_editor_to_close = if closed_document.is_some()
                && self.ui_state.gerber_grid_editor_document == closed_document
            {
                self.ui_state.gerber_grid_editor = None;
                self.ui_state.gerber_grid_editor_document = None;
                self.grid_editor_window_id()
            } else {
                None
            };
            return grid_editor_to_close.map_or_else(Task::none, iced::window::close);
        }

        let Some(mut gerber_viewer) = self.ui_state.gerber_workspace.take_viewer(document_id)
        else {
            return Task::none();
        };
        let route = move |message| Message::GerberViewer(document_id, message);

        let task = match message {
            GerberViewerMessage::NoOp => Task::none(),
            GerberViewerMessage::DockEvent(_)
            | GerberViewerMessage::NewDocument
            | GerberViewerMessage::CloseRequested => Task::none(),
            GerberViewerMessage::OpenFiles => {
                gerber_viewer.begin_loading();
                Task::perform(
                    async {
                        FABRICATION_FILE_FILTERS
                            .iter()
                            .fold(
                                rfd::AsyncFileDialog::new().set_title("Open Fabrication File(s)"),
                                |dialog, (label, extensions)| dialog.add_filter(*label, extensions),
                            )
                            .pick_files()
                            .await
                            .map(|files| {
                                files
                                    .into_iter()
                                    .map(|file| file.path().to_path_buf())
                                    .collect::<Vec<_>>()
                            })
                    },
                    move |paths| route(GerberViewerMessage::FilesChosen(paths)),
                )
            }
            GerberViewerMessage::FilesChosen(Some(paths)) => Task::perform(
                async move { signex_gerber::load_fabrication_files(paths) },
                move |batch| route(GerberViewerMessage::FilesLoaded(batch)),
            ),
            GerberViewerMessage::FilesChosen(None) => {
                gerber_viewer.loading = false;
                gerber_viewer.status = "Open fabrication files cancelled.".into();
                Task::none()
            }
            GerberViewerMessage::FilesLoaded(batch) => {
                self.apply_gerber_load_batch(document_id, &mut gerber_viewer, batch);
                Task::none()
            }
            GerberViewerMessage::ReloadAllLayers => {
                gerber_viewer.begin_loading();
                let layers = gerber_viewer
                    .layers
                    .iter()
                    .map(|layer| layer.layer.clone())
                    .collect::<Vec<_>>();
                Task::perform(
                    async move { signex_gerber::reload_layers(layers) },
                    move |batch| route(GerberViewerMessage::LayersReloaded(batch)),
                )
            }
            GerberViewerMessage::LayersReloaded(batch) => {
                gerber_viewer.apply_reload_batch(batch);
                Task::none()
            }
            GerberViewerMessage::SelectLayer(index) => {
                gerber_viewer.select_layer(index);
                Task::none()
            }
            GerberViewerMessage::NextLayer => {
                gerber_viewer.select_next_layer();
                Task::none()
            }
            GerberViewerMessage::PreviousLayer => {
                gerber_viewer.select_previous_layer();
                Task::none()
            }
            GerberViewerMessage::MoveLayerUp => {
                gerber_viewer.move_active_layer_up();
                Task::none()
            }
            GerberViewerMessage::MoveLayerDown => {
                gerber_viewer.move_active_layer_down();
                Task::none()
            }
            GerberViewerMessage::SetLayerVisible(index, visible) => {
                gerber_viewer.set_layer_visible(index, visible);
                Task::none()
            }
            GerberViewerMessage::ToggleColorPicker(target) => {
                gerber_viewer.toggle_color_picker(target);
                Task::none()
            }
            GerberViewerMessage::CloseColorPicker => {
                gerber_viewer.close_color_picker();
                Task::none()
            }
            GerberViewerMessage::SetLayerColor(index, palette_index) => {
                gerber_viewer.set_layer_color(index, palette_index);
                Task::none()
            }
            GerberViewerMessage::SetGridColor(palette_index) => {
                gerber_viewer.set_grid_color(palette_index);
                Task::none()
            }
            GerberViewerMessage::SetDCodeColor(palette_index) => {
                gerber_viewer.set_d_code_color(palette_index);
                Task::none()
            }
            GerberViewerMessage::SetNegativeObjectColor(palette_index) => {
                gerber_viewer.set_negative_object_color(palette_index);
                Task::none()
            }
            GerberViewerMessage::ClearCurrentLayer => {
                gerber_viewer.clear_current_layer();
                Task::none()
            }
            GerberViewerMessage::ClearAllLayers => {
                gerber_viewer.clear_all_layers();
                Task::none()
            }
            GerberViewerMessage::RedrawViewport => {
                gerber_viewer.redraw_viewport();
                Task::none()
            }
            GerberViewerMessage::ToggleSketchFlashes => {
                gerber_viewer.toggle_sketch_flashes();
                Task::none()
            }
            GerberViewerMessage::ToggleSketchLines => {
                gerber_viewer.toggle_sketch_lines();
                Task::none()
            }
            GerberViewerMessage::ToggleSketchPolygons => {
                gerber_viewer.toggle_sketch_polygons();
                Task::none()
            }
            GerberViewerMessage::ToggleGhostNegativeObjects => {
                gerber_viewer.toggle_ghost_negative_objects();
                Task::none()
            }
            GerberViewerMessage::ToggleDCodeLabels => {
                gerber_viewer.toggle_d_code_labels();
                Task::none()
            }
            GerberViewerMessage::ToggleCompareMode => {
                gerber_viewer.toggle_compare_mode();
                Task::none()
            }
            GerberViewerMessage::ToggleForcedOpacityMode => {
                gerber_viewer.toggle_forced_opacity_mode();
                Task::none()
            }
            GerberViewerMessage::ToggleDimInactiveLayers => {
                gerber_viewer.toggle_dim_inactive_layers();
                Task::none()
            }
            GerberViewerMessage::ToggleMirrored => {
                gerber_viewer.toggle_mirrored();
                Task::none()
            }
            GerberViewerMessage::ZoomBy(factor) => {
                gerber_viewer.zoom_by(factor);
                Task::none()
            }
            GerberViewerMessage::PanBy(delta) => {
                gerber_viewer.pan_by(delta);
                Task::none()
            }
            GerberViewerMessage::FitPage => {
                gerber_viewer.fit_page();
                Task::none()
            }
            GerberViewerMessage::ToggleLayerManager => {
                gerber_viewer.toggle_layer_manager();
                self.ui_state.gerber_workspace.set_tool_visible(
                    GerberDockPanel::Layers,
                    gerber_viewer.is_tool_panel_visible(GerberDockPanel::Layers),
                );
                Task::none()
            }
            GerberViewerMessage::ToggleHighlightPanel => {
                gerber_viewer.toggle_highlight_panel();
                self.ui_state.gerber_workspace.set_tool_visible(
                    GerberDockPanel::Highlight,
                    gerber_viewer.is_tool_panel_visible(GerberDockPanel::Highlight),
                );
                Task::none()
            }
            GerberViewerMessage::ToggleGridPanel => {
                gerber_viewer.toggle_grid_panel();
                self.ui_state.gerber_workspace.set_tool_visible(
                    GerberDockPanel::Grid,
                    gerber_viewer.is_tool_panel_visible(GerberDockPanel::Grid),
                );
                Task::none()
            }
            GerberViewerMessage::ToggleLayerInformation => {
                gerber_viewer.toggle_layer_information();
                self.ui_state.gerber_workspace.set_tool_visible(
                    GerberDockPanel::LayerInformation,
                    gerber_viewer.is_tool_panel_visible(GerberDockPanel::LayerInformation),
                );
                Task::none()
            }
            GerberViewerMessage::ToggleDCodeList => {
                gerber_viewer.toggle_d_code_list();
                self.ui_state.gerber_workspace.set_tool_visible(
                    GerberDockPanel::DCodes,
                    gerber_viewer.is_tool_panel_visible(GerberDockPanel::DCodes),
                );
                Task::none()
            }
            GerberViewerMessage::ToggleSourceView => {
                gerber_viewer.toggle_source_view();
                self.ui_state.gerber_workspace.set_tool_visible(
                    GerberDockPanel::Source,
                    gerber_viewer.is_tool_panel_visible(GerberDockPanel::Source),
                );
                Task::none()
            }
            GerberViewerMessage::SelectGridSize(index) => {
                gerber_viewer.select_grid_size(index);
                Task::none()
            }
            GerberViewerMessage::OpenGridEditor => {
                self.handle_open_gerber_grid_editor(document_id, &gerber_viewer)
            }
            GerberViewerMessage::NewGridNameChanged(value) => {
                gerber_viewer.set_new_grid_name(value);
                Task::none()
            }
            GerberViewerMessage::NewGridXChanged(value) => {
                gerber_viewer.set_new_grid_x(value);
                Task::none()
            }
            GerberViewerMessage::NewGridYChanged(value) => {
                gerber_viewer.set_new_grid_y(value);
                Task::none()
            }
            GerberViewerMessage::SetNewGridUnitMillimetres(millimetres) => {
                gerber_viewer.set_new_grid_unit_millimetres(millimetres);
                Task::none()
            }
            GerberViewerMessage::CreateGridDefinition => {
                gerber_viewer.create_grid();
                Task::none()
            }
            GerberViewerMessage::DeleteGridDefinition => {
                gerber_viewer.delete_grid();
                Task::none()
            }
            GerberViewerMessage::MoveGridUp => {
                gerber_viewer.move_grid_up();
                Task::none()
            }
            GerberViewerMessage::MoveGridDown => {
                gerber_viewer.move_grid_down();
                Task::none()
            }
            GerberViewerMessage::EditGridNameChanged(value) => {
                gerber_viewer.set_edit_grid_name(value);
                Task::none()
            }
            GerberViewerMessage::EditGridXChanged(value) => {
                gerber_viewer.set_edit_grid_x(value);
                Task::none()
            }
            GerberViewerMessage::EditGridYChanged(value) => {
                gerber_viewer.set_edit_grid_y(value);
                Task::none()
            }
            GerberViewerMessage::SetEditGridUnitMillimetres(millimetres) => {
                gerber_viewer.set_edit_grid_unit_millimetres(millimetres);
                Task::none()
            }
            GerberViewerMessage::UpdateGridDefinition => {
                gerber_viewer.update_grid();
                Task::none()
            }
            GerberViewerMessage::ToggleGridVisibility(visible) => {
                gerber_viewer.set_grid_visible(visible);
                Task::none()
            }
            GerberViewerMessage::SetDisplayUnit(unit) => {
                gerber_viewer.set_display_unit(unit);
                Task::none()
            }
            GerberViewerMessage::CycleDisplayUnit => {
                gerber_viewer.cycle_display_unit();
                Task::none()
            }
            GerberViewerMessage::CursorWorldPositionChanged(position) => {
                gerber_viewer.set_cursor_world_position(position);
                Task::none()
            }
            GerberViewerMessage::ActivateSelectionTool => {
                gerber_viewer.activate_selection_tool();
                Task::none()
            }
            GerberViewerMessage::ActivateMeasurementTool => {
                gerber_viewer.activate_measurement_tool();
                Task::none()
            }
            GerberViewerMessage::ToggleMeasurement => {
                gerber_viewer.toggle_measurement();
                Task::none()
            }
            GerberViewerMessage::BeginMeasurement(point) => {
                gerber_viewer.begin_measurement(point);
                Task::none()
            }
            GerberViewerMessage::UpdateMeasurement(point) => {
                gerber_viewer.update_measurement(point);
                Task::none()
            }
            GerberViewerMessage::CompleteMeasurement(point) => {
                gerber_viewer.complete_measurement(point);
                Task::none()
            }
            GerberViewerMessage::CaptureMeasurementPoint(point) => {
                gerber_viewer.capture_measurement_point(point);
                Task::none()
            }
            GerberViewerMessage::ResetMeasurement => {
                gerber_viewer.reset_measurement();
                Task::none()
            }
            GerberViewerMessage::ToggleFullWindowCrosshair(full_window) => {
                gerber_viewer.set_full_window_crosshair(full_window);
                Task::none()
            }
            GerberViewerMessage::CycleCrosshairMode => {
                gerber_viewer.cycle_crosshair_mode();
                Task::none()
            }
            GerberViewerMessage::SetPageSize(page_size) => {
                gerber_viewer.set_page_size(page_size);
                Task::none()
            }
            GerberViewerMessage::PrintVisibleLayers => match gerber_viewer.print_pdf() {
                Ok(bytes) => {
                    gerber_viewer.status = "Choose where to save the Gerber print PDF.".into();
                    Task::perform(
                        async move {
                            let Some(file) = rfd::AsyncFileDialog::new()
                                .set_title("Print Visible Gerber Layers to PDF")
                                .add_filter("PDF document", &["pdf"])
                                .set_file_name("gerber-view.pdf")
                                .save_file()
                                .await
                            else {
                                return Err("Gerber PDF print cancelled.".to_owned());
                            };
                            let mut path = file.path().to_path_buf();
                            if path.extension().is_none() {
                                path.set_extension("pdf");
                            }
                            std::fs::write(&path, bytes).map_err(|error| {
                                format!("Could not write {}: {error}", path.display(),)
                            })?;
                            Ok(path)
                        },
                        move |result| route(GerberViewerMessage::GerberPrintFinished(result)),
                    )
                }
                Err(error) => {
                    gerber_viewer.status = error;
                    Task::none()
                }
            },
            GerberViewerMessage::GerberPrintFinished(result) => {
                gerber_viewer.status = match result {
                    Ok(path) => {
                        format!("Printed visible Gerber layers to {}.", path.display())
                    }
                    Err(error) => error,
                };
                Task::none()
            }
            GerberViewerMessage::ExportNativePcb => {
                gerber_viewer.status =
                    "Choose where to save the lossy native PCB conversion.".into();
                Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Export Lossy Gerber Conversion")
                            .add_filter("Signex PCB", &["snxpcb"])
                            .set_file_name("gerber-import.snxpcb")
                            .save_file()
                            .await
                            .map(|file| file.path().to_path_buf())
                    },
                    move |path| route(GerberViewerMessage::NativePcbPathChosen(path)),
                )
            }
            GerberViewerMessage::NativePcbPathChosen(Some(mut path)) => {
                if path.extension().is_none() {
                    path.set_extension("snxpcb");
                }
                let export = gerber_viewer.export_native_pcb();
                let summary = export.report.detailed_summary();
                if export.report.converted_count() == 0 {
                    gerber_viewer.status =
                        format!("No supported Gerber artwork was exported. {summary}",);
                    Task::none()
                } else {
                    match export.write_string() {
                        Ok(source) => Task::perform(
                            async move {
                                signex_types::atomic_io::atomic_write(&path, source.as_bytes())
                                    .map_err(|error| {
                                        format!("Could not write {}: {error}", path.display(),)
                                    })?;
                                Ok((path, summary))
                            },
                            move |result| {
                                route(GerberViewerMessage::NativePcbExportFinished(result))
                            },
                        ),
                        Err(error) => {
                            gerber_viewer.status = error;
                            Task::none()
                        }
                    }
                }
            }
            GerberViewerMessage::NativePcbPathChosen(None) => {
                gerber_viewer.status = "Lossy native PCB export cancelled.".into();
                Task::none()
            }
            GerberViewerMessage::NativePcbExportFinished(result) => {
                gerber_viewer.status = match result {
                    Ok((path, summary)) => {
                        format!("Exported lossy native PCB to {}. {summary}", path.display(),)
                    }
                    Err(error) => error,
                };
                Task::none()
            }
            GerberViewerMessage::ToggleZoomSelection => {
                gerber_viewer.toggle_zoom_selection();
                Task::none()
            }
            GerberViewerMessage::SetHighlightedComponent(component) => {
                gerber_viewer.set_highlighted_component(component);
                Task::none()
            }
            GerberViewerMessage::ClearComponentHighlight => {
                gerber_viewer.clear_component_highlight();
                Task::none()
            }
            GerberViewerMessage::SetHighlightedNet(net) => {
                gerber_viewer.set_highlighted_net(net);
                Task::none()
            }
            GerberViewerMessage::ClearNetHighlight => {
                gerber_viewer.clear_net_highlight();
                Task::none()
            }
            GerberViewerMessage::SetHighlightedAttribute(attribute) => {
                gerber_viewer.set_highlighted_attribute(attribute);
                Task::none()
            }
            GerberViewerMessage::ClearAttributeHighlight => {
                gerber_viewer.clear_attribute_highlight();
                Task::none()
            }
            GerberViewerMessage::SetHighlightedDCode(d_code) => {
                gerber_viewer.set_highlighted_d_code(d_code);
                Task::none()
            }
            GerberViewerMessage::ClearDCodeHighlight => {
                gerber_viewer.clear_d_code_highlight();
                Task::none()
            }
            GerberViewerMessage::ClearHighlight => {
                gerber_viewer.clear_highlight();
                Task::none()
            }
            GerberViewerMessage::SetSelectedItem(selection) => {
                gerber_viewer.set_selected_item(selection);
                Task::none()
            }
            GerberViewerMessage::SetRegionSelection(selections) => {
                gerber_viewer.set_region_selection(selections);
                Task::none()
            }
            GerberViewerMessage::ZoomToSelection { bounds, viewport } => {
                gerber_viewer.zoom_to_selection(bounds, viewport);
                Task::none()
            }
        };

        self.ui_state
            .gerber_workspace
            .restore_viewer(document_id, gerber_viewer);
        task
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn fabrication_file_filters_default_to_all_supported_and_end_with_all_files() {
        assert_eq!(
            FABRICATION_FILE_FILTERS.first(),
            Some(&(
                "All supported file formats",
                ALL_SUPPORTED_FABRICATION_EXTENSIONS,
            )),
        );
        let all_files = FABRICATION_FILE_FILTERS.last().expect("All files filter");
        assert_eq!(all_files.0, "All files");
        assert_eq!(all_files.1, ["*"]);

        let specific_extensions = FABRICATION_FILE_FILTERS[1..5]
            .iter()
            .flat_map(|(_, extensions)| extensions.iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(specific_extensions, ALL_SUPPORTED_FABRICATION_EXTENSIONS,);
    }

    fn gerber_document_ids(app: &Signex) -> Vec<GerberDocumentId> {
        app.ui_state
            .gerber_workspace
            .documents()
            .iter()
            .map(|document| document.id)
            .collect()
    }

    fn gerber_document(app: &Signex, document_id: GerberDocumentId) -> &GerberViewerState {
        app.ui_state
            .gerber_workspace
            .viewer(document_id)
            .expect("Gerber document state")
    }

    #[test]
    fn multiple_gerber_documents_keep_messages_and_state_isolated() {
        let (mut app, _task) = Signex::new();
        let _ = app.dispatch_gerber_viewer_message(
            app.ui_state.gerber_workspace.active_document_id(),
            GerberViewerMessage::NewDocument,
        );
        let document_ids = gerber_document_ids(&app);

        assert_eq!(document_ids.len(), 2);
        assert_ne!(document_ids[0], document_ids[1]);

        let _ =
            app.dispatch_gerber_viewer_message(document_ids[0], GerberViewerMessage::ZoomBy(2.0));
        let _ =
            app.dispatch_gerber_viewer_message(document_ids[1], GerberViewerMessage::ZoomBy(0.5));

        assert_eq!(gerber_document(&app, document_ids[0]).zoom, 2.0);
        assert_eq!(gerber_document(&app, document_ids[1]).zoom, 0.5);

        let _ = app.dispatch_gerber_viewer_message(
            document_ids[0],
            GerberViewerMessage::NativePcbPathChosen(Some(std::path::PathBuf::from(
                ".temp/empty.snxpcb",
            ))),
        );

        assert!(
            gerber_document(&app, document_ids[0])
                .status
                .contains("No supported Gerber artwork")
        );
        assert_eq!(gerber_document(&app, document_ids[1]).zoom, 0.5);
    }

    #[test]
    fn first_successful_load_names_its_own_document_tab() {
        let (mut app, _task) = Signex::new();
        app.ui_state.gerber_workspace.new_document();
        let document_ids = gerber_document_ids(&app);
        let layer = signex_gerber::load_gerber_reader(
            "front-copper.gbr",
            Cursor::new(b"%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000*%\nD10*\nX0Y0D03*\nM02*\n"),
        )
        .expect("test Gerber must parse");

        let _ = app.dispatch_gerber_viewer_message(
            document_ids[0],
            GerberViewerMessage::FilesLoaded(signex_gerber::GerberLoadBatch {
                layers: vec![layer],
                failures: Vec::new(),
            }),
        );

        let titles = app
            .ui_state
            .gerber_workspace
            .documents()
            .iter()
            .map(|document| document.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(titles, ["front-copper.gbr", "Gerber Viewer 2"]);
        assert_eq!(gerber_document(&app, document_ids[0]).layers.len(), 1,);
        assert!(gerber_document(&app, document_ids[1]).layers.is_empty());
    }

    #[test]
    fn closing_a_gerber_document_clears_its_grid_editor() {
        let (mut app, _task) = Signex::new();
        let document_id = app.ui_state.gerber_workspace.active_document_id();
        let editor = GerberGridEditorState::from_viewer(gerber_document(&app, document_id));
        app.ui_state.gerber_grid_editor = Some(editor);
        app.ui_state.gerber_grid_editor_document = Some(document_id);

        let _ = app.dispatch_gerber_viewer_message(
            document_id,
            GerberViewerMessage::document_closed(document_id),
        );

        assert!(app.ui_state.gerber_grid_editor.is_none());
        assert!(app.ui_state.gerber_grid_editor_document.is_none());
    }

    #[test]
    fn gerber_viewer_registers_as_a_dedicated_window() {
        let (mut app, _task) = Signex::new();
        let main_tab_count = app.document_state.tabs.len();
        let window_id = iced::window::Id::unique();

        let _ = app.dispatch_update(Message::GerberViewerOpened(window_id));

        assert!(matches!(
            app.ui_state.windows.get(&window_id),
            Some(crate::app::state::WindowKind::GerberViewer),
        ));
        assert_eq!(app.document_state.tabs.len(), main_tab_count);
    }

    #[test]
    fn built_in_profiles_bind_gerber_keys_through_widget_boundary() {
        let mut profiles = crate::keymap::ShortcutProfileSet::built_ins()
            .expect("built-in shortcut profiles must parse");
        let page_up = iced::keyboard::Key::Named(iced::keyboard::key::Named::PageUp);
        let page_down = iced::keyboard::Key::Named(iced::keyboard::key::Named::PageDown);
        let f = iced::keyboard::Key::Character("f".into());
        let l = iced::keyboard::Key::Character("l".into());
        let p = iced::keyboard::Key::Character("p".into());
        let d = iced::keyboard::Key::Character("d".into());
        let plus = iced::keyboard::Key::Character("+".into());
        let minus = iced::keyboard::Key::Character("-".into());
        let q = iced::keyboard::Key::Character("q".into());

        for profile in ["altium", "classic"] {
            profiles
                .set_active_profile(profile)
                .expect("known built-in profile");
            let keymap = profiles.compile_active();

            assert!(matches!(
                keymap.resolve_gerber_shortcut(&page_up, iced::keyboard::Modifiers::default(),),
                Some(GerberViewerMessage::PreviousLayer),
            ));
            assert!(matches!(
                keymap.resolve_gerber_shortcut(&page_down, iced::keyboard::Modifiers::default(),),
                Some(GerberViewerMessage::NextLayer),
            ));
            assert!(matches!(
                keymap.resolve_gerber_shortcut(&f, iced::keyboard::Modifiers::default(),),
                Some(GerberViewerMessage::ToggleSketchFlashes),
            ));
            assert!(matches!(
                keymap.resolve_gerber_shortcut(&l, iced::keyboard::Modifiers::default(),),
                Some(GerberViewerMessage::ToggleSketchLines),
            ));
            assert!(matches!(
                keymap.resolve_gerber_shortcut(&p, iced::keyboard::Modifiers::default(),),
                Some(GerberViewerMessage::ToggleSketchPolygons),
            ));
            assert!(matches!(
                keymap.resolve_gerber_shortcut(&d, iced::keyboard::Modifiers::default(),),
                Some(GerberViewerMessage::ToggleDCodeLabels),
            ));
            assert!(matches!(
                keymap.resolve_gerber_shortcut(&plus, iced::keyboard::Modifiers::SHIFT,),
                Some(GerberViewerMessage::MoveLayerUp),
            ));
            assert!(matches!(
                keymap.resolve_gerber_shortcut(&minus, iced::keyboard::Modifiers::default(),),
                Some(GerberViewerMessage::MoveLayerDown),
            ));
            let print = keymap.resolve_gerber_shortcut(&p, iced::keyboard::Modifiers::CTRL);
            let print_stroke =
                crate::keymap::KeyStroke::from_iced(&p, iced::keyboard::Modifiers::CTRL)
                    .expect("Ctrl+P must be a key stroke");
            let print_lookup = keymap.lookup(
                std::slice::from_ref(&print_stroke),
                &[
                    crate::keymap::ShortcutContext::Global,
                    crate::keymap::ShortcutContext::Gerber,
                ],
            );
            assert!(
                matches!(print, Some(GerberViewerMessage::PrintVisibleLayers)),
                "{profile} resolved Ctrl+P to {print:?}; stroke was {print_stroke:?}; lookup was \
                 {print_lookup:?}",
            );
            let quit = keymap.resolve_gerber_shortcut(&q, iced::keyboard::Modifiers::CTRL);
            assert!(
                matches!(quit, Some(GerberViewerMessage::CloseRequested)),
                "{profile} resolved Ctrl+Q to {quit:?}",
            );
        }
    }
}
