//! The `prefs.json` writes a committed Save Preferences hands to background
//! workers (#1351).

use iced::Task;

use super::super::super::*;

impl Signex {
    /// One background write per preference the Save arm commits, each
    /// snapshotting its value from `ui_state` now, at dispatch time. Call
    /// after the draft has been committed into `ui_state`.
    pub(super) fn committed_pref_writes(&self) -> Vec<Task<Message>> {
        let ui = &self.ui_state;
        let font = ui.ui_font_name.clone();
        let classes = ui.component_classes.clone();
        let drag_direction = ui.drag_direction_select;
        let (grid_style, annotate_undo, keeps_wires, placement, autosave, gpu, theme) = (
            ui.grid_style,
            ui.annotate_undo,
            ui.move_keeps_wires,
            ui.window_controls_placement,
            ui.autosave_enabled,
            ui.pcb_gpu_render,
            ui.theme_id,
        );
        let (sym_size, sym_style, sym_pins, zoom, scale) = (
            ui.symbol_grid_size_mm,
            ui.symbol_grid_style,
            ui.symbol_pin_selection,
            ui.zoom_step_percent,
            ui.ui_scale,
        );
        fn write(key: &'static str, job: impl FnOnce() + Send + 'static) -> Task<Message> {
            crate::fonts::background_pref_write(key, job)
        }
        vec![
            write("ui_font", move || crate::fonts::write_ui_font_pref(&font)),
            write("grid_style", move || {
                crate::fonts::write_grid_style_pref(grid_style)
            }),
            write("annotate_undo", move || {
                crate::fonts::write_annotate_undo_pref(annotate_undo)
            }),
            write("move_keeps_wires", move || {
                crate::fonts::write_move_keeps_wires_pref(keeps_wires)
            }),
            write("drag_direction_select", move || {
                crate::fonts::write_drag_direction_select_pref(drag_direction)
            }),
            write("window_controls_placement", move || {
                crate::fonts::write_window_controls_placement_pref(placement)
            }),
            write("autosave_enabled", move || {
                crate::fonts::write_autosave_enabled_pref(autosave)
            }),
            write("pcb_gpu_render", move || {
                crate::fonts::write_pcb_gpu_render_pref(gpu)
            }),
            write("theme", move || crate::fonts::write_theme_pref(theme)),
            write("symbol_grid_size_mm", move || {
                crate::fonts::write_symbol_grid_size_mm_pref(sym_size)
            }),
            write("symbol_grid_style", move || {
                crate::fonts::write_symbol_grid_style_pref(sym_style)
            }),
            write("symbol_pin_selection", move || {
                crate::fonts::write_symbol_pin_selection_pref(sym_pins)
            }),
            write("zoom_step_percent", move || {
                crate::fonts::write_zoom_step_percent_pref(zoom)
            }),
            write("ui_scale", move || crate::fonts::write_ui_scale_pref(scale)),
            write("component_classes", move || {
                crate::fonts::write_component_classes_pref(&classes)
            }),
        ]
    }
}
