use super::gerber_grid_editor_state::GerberGridEditorState;
use crate::gerber_viewer::{GridUnit, create_grid_definition, format_distance_input};

impl GerberGridEditorState {
    pub(super) fn select_grid(&mut self, index: usize) {
        if index >= self.catalog.len() || index == self.selected_index || !self.sync_selected_grid()
        {
            return;
        }
        self.selected_index = index;
        self.load_selected_settings();
    }

    pub(super) fn sync_selected_grid(&mut self) -> bool {
        let grid = match create_grid_definition(
            &self.settings_name,
            &self.settings_x,
            &self.settings_y,
            self.settings_unit,
            &self.decimal_separator,
        ) {
            Ok(grid) => grid,
            Err(error) => {
                self.error = Some(error);
                return false;
            }
        };
        self.catalog[self.selected_index] = grid;
        self.error = None;
        true
    }

    pub(super) fn set_settings_unit(&mut self, unit: GridUnit) {
        if unit == self.settings_unit {
            return;
        }
        let grid = match create_grid_definition(
            &self.settings_name,
            &self.settings_x,
            &self.settings_y,
            self.settings_unit,
            &self.decimal_separator,
        ) {
            Ok(grid) => grid.converted_to(unit),
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        self.settings_x = format_distance_input(grid.x, &self.decimal_separator);
        self.settings_y = format_distance_input(grid.y, &self.decimal_separator);
        self.settings_unit = unit;
        self.catalog[self.selected_index] = grid;
        self.error = None;
    }

    pub(super) fn delete_selected_grid(&mut self) {
        if self.catalog.len() <= 1 {
            self.error = Some("At least one grid definition is required.".to_owned());
            return;
        }
        self.catalog.remove(self.selected_index);
        self.selected_index = self.selected_index.min(self.catalog.len() - 1);
        self.load_selected_settings();
    }

    pub(super) fn move_selected_grid(&mut self, direction: isize) {
        let target = self.selected_index as isize + direction;
        if target < 0 || target >= self.catalog.len() as isize {
            return;
        }
        let target = target as usize;
        self.catalog.swap(self.selected_index, target);
        self.selected_index = target;
    }

    pub(super) fn load_selected_settings(&mut self) {
        let grid = &self.catalog[self.selected_index];
        self.settings_name = grid.name.clone().unwrap_or_default();
        self.settings_x = format_distance_input(grid.x, &self.decimal_separator);
        self.settings_y = format_distance_input(grid.y, &self.decimal_separator);
        self.settings_unit = grid.unit;
        self.error = None;
    }
}
