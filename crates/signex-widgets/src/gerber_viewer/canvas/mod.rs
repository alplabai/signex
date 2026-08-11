use super::*;

mod behavior;
mod formatting;
mod geometry;
mod grid_view;
mod layer_view;
mod palette;
mod state;
mod view;

pub(super) use formatting::{
    format_bounds_in_unit, format_cartesian_coordinate_in_unit, format_polar_coordinate_in_unit,
};
pub(super) use geometry::{
    fit_transform, normalized_screen_rectangle, page_bounds, screen_to_world_point, visible_bounds,
    world_to_screen_point, zoom_transform_for_selection,
};
pub(super) use palette::{
    material_color_palette, material_compare_palette, material_d_code_color, material_grid_color,
    material_layer_palette, material_negative_ghost_color,
};
pub(super) use state::GerberCanvasState;

#[cfg(test)]
pub(super) use grid_view::{
    crosshair_segments, full_window_crosshair_segments, grid_render_color, visible_grid_spacing,
};

#[cfg(test)]
pub(super) use layer_view::{
    DCodeLabel, FlashRenderMode, LineRenderMode, PolygonRenderMode, compare_layer_color,
    composite_compare_colors, d_code_label, d_code_labels_visible, flash_render_mode,
    forced_opacity_color, inactive_layer_color, line_render_mode, line_stroke_widths,
    polygon_render_mode, primitive_polarity_color,
};

pub(super) struct GerberCanvas<'a> {
    pub(super) layers: &'a [ViewerLayer],
    pub(super) background: Color,
    pub(super) grid: Color,
    pub(super) grid_visible: bool,
    pub(super) grid_style: GerberGridStyle,
    pub(super) crosshair_mode: GerberCrosshairMode,
    pub(super) page_size: GerberPageSize,
    pub(super) zoom_selection_active: bool,
    pub(super) selection_active: bool,
    pub(super) measurement_active: bool,
    pub(super) measurement: Option<GerberMeasurement>,
    pub(super) measurement_annotation: Option<String>,
    pub(super) sketch_flashes: bool,
    pub(super) sketch_lines: bool,
    pub(super) sketch_polygons: bool,
    pub(super) ghost_negative_objects: bool,
    pub(super) negative_ghost_color: Color,
    pub(super) show_d_code_labels: bool,
    pub(super) d_code_color: Color,
    pub(super) compare_mode: bool,
    pub(super) compare_palette: &'a [Color],
    pub(super) forced_opacity_mode: bool,
    pub(super) forced_opacity: f32,
    pub(super) dim_inactive_layers: bool,
    pub(super) inactive_layer_opacity: f32,
    pub(super) mirrored: bool,
    pub(super) active_layer: Option<usize>,
    pub(super) highlighted_component: Option<&'a str>,
    pub(super) highlighted_net: Option<&'a str>,
    pub(super) highlighted_attribute: Option<&'a GerberAttributeValue>,
    pub(super) highlighted_d_code: Option<i32>,
    pub(super) selected_item: Option<GerberItemSelection>,
    pub(super) selected_items: &'a [GerberItemSelection],
    pub(super) grid_size: &'a GridSizePreset,
    pub(super) shortcut_resolver: &'a dyn GerberShortcutResolver,
    pub(super) redraw_generation: u64,
    pub(super) zoom: f32,
    pub(super) pan: iced::Vector,
}
