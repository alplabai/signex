use super::*;

#[derive(Debug, Clone)]
pub struct ViewerLayer {
    pub layer: LoadedLayer,
    pub visible: bool,
    pub color: Color,
}

#[derive(Debug)]
pub struct GerberViewerState {
    pub layers: Vec<ViewerLayer>,
    pub active_layer: Option<usize>,
    pub loading: bool,
    pub status: String,
    pub redraw_generation: u64,
    pub(super) selected_item: Option<GerberItemSelection>,
    pub(super) region_selection: Vec<GerberItemSelection>,
    pub zoom: f32,
    pub pan: iced::Vector,
    pub layer_manager_visible: bool,
    pub(super) highlight_panel_visible: bool,
    pub(super) grid_panel_visible: bool,
    pub(super) layer_information_visible: bool,
    pub(super) d_code_list_visible: bool,
    pub(super) source_view_visible: bool,
    pub(crate) grid_catalog: Vec<GridSizePreset>,
    pub(crate) active_grid_index: usize,
    pub(super) grid_visible: bool,
    pub(super) grid_style: GerberGridStyle,
    pub(super) grid_color: Color,
    pub(super) display_unit: GerberDisplayUnit,
    pub(super) cursor_world_position: Option<signex_gerber::Point>,
    pub(super) measurement_active: bool,
    pub(super) measurement: Option<GerberMeasurement>,
    pub(super) sketch_flashes: bool,
    pub(super) sketch_lines: bool,
    pub(super) sketch_polygons: bool,
    pub(super) ghost_negative_objects: bool,
    pub(super) negative_ghost_color: Color,
    pub(super) show_d_code_labels: bool,
    pub(super) d_code_color: Color,
    pub(super) compare_mode: bool,
    pub(super) compare_palette: Vec<Color>,
    pub(super) forced_opacity_mode: bool,
    pub(super) forced_opacity: f32,
    pub(super) dim_inactive_layers: bool,
    pub(super) inactive_layer_opacity: f32,
    pub(super) mirrored: bool,
    pub(super) crosshair_mode: GerberCrosshairMode,
    pub(super) page_size: GerberPageSize,
    pub(super) zoom_selection_active: bool,
    pub(super) highlighted_component: Option<String>,
    pub(super) highlighted_net: Option<String>,
    pub(super) highlighted_attribute: Option<GerberAttributeValue>,
    pub(super) highlighted_d_code: Option<i32>,
    pub(crate) decimal_separator: String,
    pub(super) grid_editor_open: bool,
    pub(super) new_grid_name: String,
    pub(super) new_grid_x: String,
    pub(super) new_grid_y: String,
    pub(super) new_grid_unit: GridUnit,
    pub(super) edit_grid_name: String,
    pub(super) edit_grid_x: String,
    pub(super) edit_grid_y: String,
    pub(super) edit_grid_unit: GridUnit,
    pub(super) grid_editor_error: Option<String>,
    pub(super) layer_palette: Vec<Color>,
    pub(super) palette: Vec<GerberMaterialColor>,
    pub(super) open_color_picker: Option<GerberColorTarget>,
}

impl Default for GerberViewerState {
    fn default() -> Self {
        let grid_catalog = load_grid_catalog();
        let active_grid_index = DEFAULT_GRID_INDEX.min(grid_catalog.len() - 1);
        let decimal_separator = system_decimal_separator();
        let active_grid = &grid_catalog[active_grid_index];
        let edit_grid_name = active_grid.name.clone().unwrap_or_default();
        let edit_grid_x = format_distance_input(active_grid.x, &decimal_separator);
        let edit_grid_y = format_distance_input(active_grid.y, &decimal_separator);
        let edit_grid_unit = active_grid.unit;
        let grid_color = material_grid_color();
        let negative_ghost_color = material_negative_ghost_color();
        let d_code_color = material_d_code_color();
        let layer_palette = material_layer_palette();
        let palette = material_color_palette();
        Self {
            layers: Vec::new(),
            active_layer: None,
            loading: false,
            status: "Open one or more Gerber files to begin.".into(),
            redraw_generation: 0,
            selected_item: None,
            region_selection: Vec::new(),
            zoom: 1.0,
            pan: iced::Vector::default(),
            layer_manager_visible: true,
            highlight_panel_visible: true,
            grid_panel_visible: true,
            layer_information_visible: true,
            d_code_list_visible: true,
            source_view_visible: true,
            grid_catalog,
            active_grid_index,
            grid_visible: true,
            grid_style: load_grid_style(),
            grid_color,
            display_unit: GerberDisplayUnit::Millimetres,
            cursor_world_position: None,
            measurement_active: false,
            measurement: None,
            sketch_flashes: false,
            sketch_lines: false,
            sketch_polygons: false,
            ghost_negative_objects: false,
            negative_ghost_color,
            show_d_code_labels: false,
            d_code_color,
            compare_mode: false,
            compare_palette: material_compare_palette(),
            forced_opacity_mode: false,
            forced_opacity: default_forced_opacity(),
            dim_inactive_layers: false,
            inactive_layer_opacity: default_inactive_layer_opacity(),
            mirrored: false,
            crosshair_mode: GerberCrosshairMode::default(),
            page_size: load_page_size(),
            zoom_selection_active: false,
            highlighted_component: None,
            highlighted_net: None,
            highlighted_attribute: None,
            highlighted_d_code: None,
            decimal_separator: decimal_separator.clone(),
            grid_editor_open: false,
            new_grid_name: String::new(),
            new_grid_x: String::new(),
            new_grid_y: String::new(),
            new_grid_unit: GridUnit::Mil,
            edit_grid_name,
            edit_grid_x,
            edit_grid_y,
            edit_grid_unit,
            grid_editor_error: None,
            layer_palette,
            palette,
            open_color_picker: None,
        }
    }
}
impl GerberViewerState {
    pub fn begin_loading(&mut self) {
        self.loading = true;
        self.status = "Loading fabrication files…".into();
    }

    pub fn apply_load_batch(&mut self, batch: GerberLoadBatch) {
        self.loading = false;
        let remaining = MAX_VIEWER_LAYERS.saturating_sub(self.layers.len());
        let loaded_count = batch.layers.len().min(remaining);
        let skipped_count = batch.layers.len().saturating_sub(loaded_count);

        for layer in batch.layers.into_iter().take(loaded_count) {
            let color_index = self.layers.len() % self.layer_palette.len();
            self.layers.push(ViewerLayer {
                layer,
                visible: true,
                color: self.layer_palette[color_index],
            });
        }

        if loaded_count > 0 {
            self.active_layer = Some(self.layers.len() - 1);
            self.highlighted_d_code = None;
        }

        let mut messages = Vec::new();
        if loaded_count > 0 {
            messages.push(format!("Loaded {loaded_count} fabrication layer(s)."));
        }
        if !batch.failures.is_empty() {
            let failures = batch
                .failures
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" | ");
            messages.push(format!(
                "{} file(s) could not be loaded: {failures}",
                batch.failures.len()
            ));
        }
        if skipped_count > 0 {
            messages.push(format!(
                "{skipped_count} layer(s) were skipped because the prototype supports at most \
                 {MAX_VIEWER_LAYERS} layers."
            ));
        }
        if messages.is_empty() {
            messages.push("No files were selected.".into());
        }
        self.status = messages.join(" ");
        self.retain_available_component_highlight();
        self.retain_available_net_highlight();
        self.retain_available_attribute_highlight();
        self.retain_available_d_code_highlight();
    }

    pub fn apply_reload_batch(&mut self, batch: signex_gerber::GerberReloadBatch) {
        self.loading = false;
        let reloaded_count = batch.layers.len();
        for (index, layer) in batch.layers {
            if let Some(existing) = self.layers.get_mut(index) {
                existing.layer = layer;
            }
        }
        if reloaded_count > 0 {
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }

        let mut status = format!("Reloaded {reloaded_count} layer(s).");
        if !batch.failures.is_empty() {
            let failures = batch
                .failures
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" | ");
            status.push_str(&format!(
                " {} layer(s) could not be reloaded: {failures}",
                batch.failures.len(),
            ));
        }
        self.status = status;
        self.retain_valid_selection();
        self.retain_available_component_highlight();
        self.retain_available_net_highlight();
        self.retain_available_attribute_highlight();
        self.retain_available_d_code_highlight();
    }

    pub fn clear_current_layer(&mut self) {
        let Some(index) = self.active_layer else {
            return;
        };
        self.layers.remove(index);
        self.active_layer = if self.layers.is_empty() {
            None
        } else {
            Some(index.min(self.layers.len() - 1))
        };
        self.remove_layer_from_selection(index);
        self.retain_available_component_highlight();
        self.retain_available_net_highlight();
        self.retain_available_attribute_highlight();
        self.highlighted_d_code = None;
        self.status = "Cleared the current layer.".into();
    }

    pub fn clear_all_layers(&mut self) {
        self.layers.clear();
        self.active_layer = None;
        self.zoom = 1.0;
        self.pan = iced::Vector::default();
        self.selected_item = None;
        self.region_selection.clear();
        self.measurement_active = false;
        self.measurement = None;
        self.highlighted_component = None;
        self.highlighted_net = None;
        self.highlighted_attribute = None;
        self.highlighted_d_code = None;
        self.status = "Cleared all Gerber layers.".into();
    }

    pub fn redraw_viewport(&mut self) {
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = "Gerber viewport redrawn.".into();
    }

    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            self.status = format!("Zoom: {:.0}%", self.zoom * 100.0);
        }
    }

    pub fn toggle_zoom_selection(&mut self) {
        self.zoom_selection_active = !self.zoom_selection_active;
        if self.zoom_selection_active {
            self.clear_measurement_state();
        }
        self.status = if self.zoom_selection_active {
            "Drag a rectangle with the left mouse button to zoom.".into()
        } else {
            "Zoom-area selection cancelled.".into()
        };
    }

    pub fn zoom_to_selection(&mut self, selection: Bounds, viewport: Rectangle) {
        let Some(base_bounds) = page_bounds(visible_bounds(&self.layers), self.page_size) else {
            return;
        };
        let Some((zoom, pan)) = zoom_transform_for_selection(base_bounds, selection, viewport)
        else {
            self.status = "Zoom area is too small.".into();
            return;
        };
        self.zoom = zoom;
        self.pan = pan;
        self.zoom_selection_active = false;
        self.status = "Zoomed to selected area.".into();
    }

    pub fn pan_by(&mut self, delta: iced::Vector) {
        if delta.x.is_finite() && delta.y.is_finite() {
            self.pan += delta;
        }
    }

    pub fn fit_page(&mut self) {
        self.zoom = 1.0;
        self.pan = iced::Vector::default();
        self.status = "Fit page to viewport.".into();
    }

    pub fn set_page_size(&mut self, page_size: GerberPageSize) {
        let grid_catalog = self.grid_catalog.clone();
        self.set_page_size_with(page_size, move |value| {
            persist_page_size(value, &grid_catalog)
        });
    }

    pub(super) fn set_page_size_with(
        &mut self,
        page_size: GerberPageSize,
        persist: impl FnOnce(GerberPageSize) -> Result<(), String>,
    ) {
        if self.page_size == page_size {
            return;
        }
        if let Err(error) = persist(page_size) {
            self.status = format!("Could not save Gerber page size: {error}");
            return;
        }
        self.page_size = page_size;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = format!("Page size: {page_size}.");
    }

    pub fn print_layout(&self) -> Option<GerberPrintLayout> {
        page_bounds(visible_bounds(&self.layers), self.page_size).map(|bounds| GerberPrintLayout {
            page_size: self.page_size,
            bounds,
        })
    }

    pub fn print_pdf(&self) -> Result<Vec<u8>, String> {
        print::build_pdf(self)
    }

    pub fn toggle_layer_manager(&mut self) {
        self.layer_manager_visible = !self.layer_manager_visible;
    }

    pub fn toggle_highlight_panel(&mut self) {
        self.highlight_panel_visible = !self.highlight_panel_visible;
    }

    pub fn toggle_grid_panel(&mut self) {
        self.grid_panel_visible = !self.grid_panel_visible;
    }

    pub fn toggle_layer_information(&mut self) {
        self.layer_information_visible = !self.layer_information_visible;
    }

    pub fn toggle_d_code_list(&mut self) {
        self.d_code_list_visible = !self.d_code_list_visible;
    }

    pub fn toggle_source_view(&mut self) {
        self.source_view_visible = !self.source_view_visible;
    }

    pub(super) fn set_tool_panel_visible(
        &mut self,
        panel: super::dock::GerberDockPanel,
        visible: bool,
    ) {
        match panel {
            super::dock::GerberDockPanel::Document(_) => {}
            super::dock::GerberDockPanel::Layers => {
                self.layer_manager_visible = visible;
            }
            super::dock::GerberDockPanel::Highlight => {
                self.highlight_panel_visible = visible;
            }
            super::dock::GerberDockPanel::Grid => {
                self.grid_panel_visible = visible;
            }
            super::dock::GerberDockPanel::LayerInformation => {
                self.layer_information_visible = visible;
            }
            super::dock::GerberDockPanel::DCodes => {
                self.d_code_list_visible = visible;
            }
            super::dock::GerberDockPanel::Source => {
                self.source_view_visible = visible;
            }
        }
    }

    pub fn is_tool_panel_visible(&self, panel: super::dock::GerberDockPanel) -> bool {
        match panel {
            super::dock::GerberDockPanel::Document(_) => true,
            super::dock::GerberDockPanel::Layers => self.layer_manager_visible,
            super::dock::GerberDockPanel::Highlight => self.highlight_panel_visible,
            super::dock::GerberDockPanel::Grid => self.grid_panel_visible,
            super::dock::GerberDockPanel::LayerInformation => self.layer_information_visible,
            super::dock::GerberDockPanel::DCodes => self.d_code_list_visible,
            super::dock::GerberDockPanel::Source => self.source_view_visible,
        }
    }

    pub(super) fn active_gerber_source(&self) -> Result<(&str, &str), &'static str> {
        let index = self.active_layer.ok_or("No active layer.")?;
        let layer = self.layers.get(index).ok_or("No active layer.")?;
        Ok((&layer.layer.name, layer.layer.gerber_source()?))
    }

    pub(super) fn definition_groups(&self) -> Vec<signex_gerber::LayerDefinitionGroup> {
        self.layers
            .iter()
            .map(|layer| layer.layer.definition_group())
            .collect()
    }

    pub(super) fn active_layer_metadata(&self) -> Option<signex_gerber::LayerMetadata> {
        self.active_layer
            .and_then(|index| self.layers.get(index))
            .map(|layer| layer.layer.metadata())
    }

    pub fn set_layer_visible(&mut self, index: usize, visible: bool) {
        if let Some(layer) = self.layers.get_mut(index) {
            layer.visible = visible;
        }
    }

    pub fn select_layer(&mut self, index: usize) {
        if index < self.layers.len() {
            if self.active_layer != Some(index) {
                self.clear_highlights_for_layer_change();
            }
            self.active_layer = Some(index);
        }
    }

    pub fn select_next_layer(&mut self) {
        let Some(index) = next_layer_index(self.active_layer, self.layers.len()) else {
            return;
        };
        self.select_layer_with_status(index);
    }

    pub fn select_previous_layer(&mut self) {
        let Some(index) = previous_layer_index(self.active_layer, self.layers.len()) else {
            return;
        };
        self.select_layer_with_status(index);
    }

    pub(super) fn select_layer_with_status(&mut self, index: usize) {
        if self.active_layer != Some(index) {
            self.clear_highlights_for_layer_change();
        }
        self.active_layer = Some(index);
        self.status = format!("Active layer: {}", self.layers[index].layer.name);
    }

    fn clear_highlights_for_layer_change(&mut self) {
        if self.has_active_highlight() {
            self.highlighted_component = None;
            self.highlighted_net = None;
            self.highlighted_attribute = None;
            self.highlighted_d_code = None;
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub fn select_grid_size(&mut self, index: usize) {
        let Some(label) = self
            .grid_catalog
            .get(index)
            .map(|grid| grid.display_label(&self.decimal_separator))
        else {
            return;
        };

        self.active_grid_index = index;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = format!("Grid: {label}");
        self.load_active_grid_into_editor();
    }

    pub fn set_grid_visible(&mut self, visible: bool) {
        if self.grid_visible != visible {
            self.grid_visible = visible;
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
        }
    }

    pub fn set_display_unit(&mut self, unit: GerberDisplayUnit) {
        self.display_unit = unit;
    }

    pub fn cycle_display_unit(&mut self) {
        self.set_display_unit(self.display_unit.next());
        self.status = format!("Display units: {}.", self.display_unit);
    }

    pub fn set_cursor_world_position(&mut self, position: Option<signex_gerber::Point>) {
        self.cursor_world_position = position;
    }

    pub fn set_full_window_crosshair(&mut self, full_window: bool) {
        let mode = if full_window {
            GerberCrosshairMode::Full
        } else {
            GerberCrosshairMode::None
        };
        self.set_crosshair_mode(mode);
    }

    pub fn set_crosshair_mode(&mut self, mode: GerberCrosshairMode) {
        if self.crosshair_mode != mode {
            self.crosshair_mode = mode;
            self.redraw_generation = self.redraw_generation.wrapping_add(1);
            self.status = format!("Crosshair: {mode}.");
        }
    }

    pub fn cycle_crosshair_mode(&mut self) {
        self.set_crosshair_mode(self.crosshair_mode.next());
    }

    pub(super) fn active_grid(&self) -> &GridSizePreset {
        &self.grid_catalog[self.active_grid_index]
    }

    pub fn toggle_grid_editor(&mut self) {
        self.grid_editor_open = !self.grid_editor_open;
        if self.grid_editor_open {
            self.load_active_grid_into_editor();
        }
        self.grid_editor_error = None;
    }

    pub fn set_new_grid_name(&mut self, value: String) {
        self.new_grid_name = value;
        self.grid_editor_error = None;
    }

    pub fn set_new_grid_x(&mut self, value: String) {
        self.new_grid_x = value;
        self.grid_editor_error = None;
    }

    pub fn set_new_grid_y(&mut self, value: String) {
        self.new_grid_y = value;
        self.grid_editor_error = None;
    }

    pub fn set_new_grid_unit_millimetres(&mut self, millimetres: bool) {
        self.new_grid_unit = if millimetres {
            GridUnit::Mm
        } else {
            GridUnit::Mil
        };
        self.grid_editor_error = None;
    }

    pub fn set_edit_grid_name(&mut self, value: String) {
        self.edit_grid_name = value;
        self.grid_editor_error = None;
    }

    pub fn set_edit_grid_x(&mut self, value: String) {
        self.edit_grid_x = value;
        self.grid_editor_error = None;
    }

    pub fn set_edit_grid_y(&mut self, value: String) {
        self.edit_grid_y = value;
        self.grid_editor_error = None;
    }

    pub fn set_edit_grid_unit_millimetres(&mut self, millimetres: bool) {
        let target_unit = if millimetres {
            GridUnit::Mm
        } else {
            GridUnit::Mil
        };
        if target_unit == self.edit_grid_unit {
            return;
        }

        let draft = match create_grid_definition(
            &self.edit_grid_name,
            &self.edit_grid_x,
            &self.edit_grid_y,
            self.edit_grid_unit,
            &self.decimal_separator,
        ) {
            Ok(draft) => draft.converted_to(target_unit),
            Err(error) => {
                self.grid_editor_error = Some(error);
                return;
            }
        };
        self.edit_grid_x = format_distance_input(draft.x, &self.decimal_separator);
        self.edit_grid_y = format_distance_input(draft.y, &self.decimal_separator);
        self.edit_grid_unit = target_unit;
        self.grid_editor_error = None;
    }

    pub fn create_grid(&mut self) {
        self.create_grid_with(persist_grid_catalog);
    }

    pub fn update_grid(&mut self) {
        self.update_grid_with(persist_grid_catalog);
    }

    pub fn delete_grid(&mut self) {
        self.delete_grid_with(persist_grid_catalog);
    }

    pub fn move_grid_up(&mut self) {
        self.move_grid_up_with(persist_grid_catalog);
    }

    pub fn move_grid_down(&mut self) {
        self.move_grid_down_with(persist_grid_catalog);
    }

    pub(super) fn create_grid_with(
        &mut self,
        persist: impl FnOnce(&[GridSizePreset]) -> Result<(), String>,
    ) {
        let grid = match create_grid_definition(
            &self.new_grid_name,
            &self.new_grid_x,
            &self.new_grid_y,
            self.new_grid_unit,
            &self.decimal_separator,
        ) {
            Ok(grid) => grid,
            Err(error) => {
                self.grid_editor_error = Some(error);
                return;
            }
        };
        let mut catalog = self.grid_catalog.clone();
        catalog.push(grid);
        if let Err(error) = persist(&catalog) {
            self.grid_editor_error = Some(error);
            return;
        }

        self.grid_catalog = catalog;
        self.active_grid_index = self.grid_catalog.len() - 1;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = format!(
            "Created grid: {}",
            self.active_grid().display_label(&self.decimal_separator),
        );
        self.new_grid_name.clear();
        self.new_grid_x.clear();
        self.new_grid_y.clear();
        self.load_active_grid_into_editor();
        self.grid_editor_error = None;
    }

    pub(super) fn update_grid_with(
        &mut self,
        persist: impl FnOnce(&[GridSizePreset]) -> Result<(), String>,
    ) {
        let grid = match create_grid_definition(
            &self.edit_grid_name,
            &self.edit_grid_x,
            &self.edit_grid_y,
            self.edit_grid_unit,
            &self.decimal_separator,
        ) {
            Ok(grid) => grid,
            Err(error) => {
                self.grid_editor_error = Some(error);
                return;
            }
        };
        let mut catalog = self.grid_catalog.clone();
        catalog[self.active_grid_index] = grid;
        if let Err(error) = persist(&catalog) {
            self.grid_editor_error = Some(error);
            return;
        }

        self.grid_catalog = catalog;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = format!(
            "Updated grid: {}",
            self.active_grid().display_label(&self.decimal_separator),
        );
        self.load_active_grid_into_editor();
        self.grid_editor_error = None;
    }

    pub(super) fn delete_grid_with(
        &mut self,
        persist: impl FnOnce(&[GridSizePreset]) -> Result<(), String>,
    ) {
        if self.grid_catalog.len() <= 1 {
            self.grid_editor_error = Some("At least one grid definition must remain.".to_owned());
            return;
        }

        let removed_label = self.active_grid().display_label(&self.decimal_separator);
        let mut catalog = self.grid_catalog.clone();
        catalog.remove(self.active_grid_index);
        if let Err(error) = persist(&catalog) {
            self.grid_editor_error = Some(error);
            return;
        }

        self.grid_catalog = catalog;
        self.active_grid_index = self.active_grid_index.min(self.grid_catalog.len() - 1);
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = format!("Deleted grid: {removed_label}");
        self.load_active_grid_into_editor();
        self.grid_editor_error = None;
    }

    pub(super) fn move_grid_up_with(
        &mut self,
        persist: impl FnOnce(&[GridSizePreset]) -> Result<(), String>,
    ) {
        let Some(target_index) = self.active_grid_index.checked_sub(1) else {
            return;
        };
        self.move_grid_to_with(target_index, persist);
    }

    pub(super) fn move_grid_down_with(
        &mut self,
        persist: impl FnOnce(&[GridSizePreset]) -> Result<(), String>,
    ) {
        let target_index = self.active_grid_index + 1;
        if target_index >= self.grid_catalog.len() {
            return;
        }
        self.move_grid_to_with(target_index, persist);
    }

    pub(super) fn move_grid_to_with(
        &mut self,
        target_index: usize,
        persist: impl FnOnce(&[GridSizePreset]) -> Result<(), String>,
    ) {
        let moved_label = self.active_grid().display_label(&self.decimal_separator);
        let mut catalog = self.grid_catalog.clone();
        catalog.swap(self.active_grid_index, target_index);
        if let Err(error) = persist(&catalog) {
            self.grid_editor_error = Some(error);
            return;
        }

        self.grid_catalog = catalog;
        self.active_grid_index = target_index;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = format!("Moved grid: {moved_label}");
        self.load_active_grid_into_editor();
        self.grid_editor_error = None;
    }

    pub(super) fn load_active_grid_into_editor(&mut self) {
        let grid = self.active_grid().clone();
        self.edit_grid_name = grid.name.unwrap_or_default();
        self.edit_grid_x = format_distance_input(grid.x, &self.decimal_separator);
        self.edit_grid_y = format_distance_input(grid.y, &self.decimal_separator);
        self.edit_grid_unit = grid.unit;
    }

    pub fn toggle_sketch_flashes(&mut self) {
        self.sketch_flashes = !self.sketch_flashes;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.sketch_flashes {
            "Flashed items shown in outline mode.".into()
        } else {
            "Flashed items shown filled.".into()
        };
    }

    pub fn toggle_sketch_lines(&mut self) {
        self.sketch_lines = !self.sketch_lines;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.sketch_lines {
            "Line items shown in outline mode.".into()
        } else {
            "Line items shown filled.".into()
        };
    }

    pub fn toggle_sketch_polygons(&mut self) {
        self.sketch_polygons = !self.sketch_polygons;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.sketch_polygons {
            "Polygon items shown in outline mode.".into()
        } else {
            "Polygon items shown filled.".into()
        };
    }

    pub fn toggle_ghost_negative_objects(&mut self) {
        self.ghost_negative_objects = !self.ghost_negative_objects;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.ghost_negative_objects {
            "Negative objects revealed in ghost color.".into()
        } else {
            "Negative objects use normal compositing.".into()
        };
    }

    pub fn toggle_d_code_labels(&mut self) {
        self.show_d_code_labels = !self.show_d_code_labels;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.show_d_code_labels {
            "D-code labels shown.".into()
        } else {
            "D-code labels hidden.".into()
        };
    }

    pub fn toggle_compare_mode(&mut self) {
        self.compare_mode = !self.compare_mode;
        if self.compare_mode {
            self.dim_inactive_layers = false;
            self.forced_opacity_mode = false;
        }
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.compare_mode {
            let visible_count = self.layers.iter().filter(|layer| layer.visible).count();
            format!("Layer compare mode enabled for {visible_count} visible layer(s).")
        } else {
            "Layer compare mode disabled.".into()
        };
    }

    pub fn toggle_dim_inactive_layers(&mut self) {
        self.dim_inactive_layers = !self.dim_inactive_layers;
        if self.dim_inactive_layers {
            self.compare_mode = false;
            self.forced_opacity_mode = false;
        }
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.dim_inactive_layers {
            "Inactive layers dimmed.".into()
        } else {
            "All visible layers shown at normal contrast.".into()
        };
    }

    pub fn toggle_forced_opacity_mode(&mut self) {
        self.forced_opacity_mode = !self.forced_opacity_mode;
        if self.forced_opacity_mode {
            self.compare_mode = false;
            self.dim_inactive_layers = false;
        }
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.forced_opacity_mode {
            format!(
                "Forced-opacity layer compositing enabled at {:.0}%.",
                self.forced_opacity * 100.0,
            )
        } else {
            "Forced-opacity layer compositing disabled.".to_owned()
        };
    }

    pub fn toggle_mirrored(&mut self) {
        self.mirrored = !self.mirrored;
        self.redraw_generation = self.redraw_generation.wrapping_add(1);
        self.status = if self.mirrored {
            "Gerber view mirrored horizontally.".into()
        } else {
            "Gerber view shown in normal orientation.".into()
        };
    }
}
