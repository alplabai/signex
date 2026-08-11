use std::{fmt, path::PathBuf};

use iced::mouse;
use iced::widget::{Space, button, canvas, checkbox, container, pick_list, row, scrollable, text};
use iced::{
    Background, Border, Color, Element, Event, Length, Point, Rectangle, Renderer, Theme, keyboard,
};
use serde::Deserialize;
use signex_gerber::{
    ApertureShape, Bounds, GerberAttributeValue, GerberLoadBatch, GerberPrimitive, LoadedLayer,
    PrimitivePolarity,
};
use signex_types::theme::ThemeTokens;

mod color_dropdown;
mod display;
mod dock;
mod gerber_viewer_state;
mod grid;
mod highlight;
mod highlight_controls;
mod item_colors;
mod layer_color;
mod layer_controls;
mod layer_order;
mod measurement;
mod menu;
mod message;
mod pcb_export;
pub(crate) mod print;
mod selection;
mod shortcuts;
pub(crate) mod styles;
mod toolbar;
mod view;
mod workspace;

#[path = "canvas/mod.rs"]
mod viewport;

pub use display::{GerberCrosshairMode, GerberDisplayUnit, GerberPageSize, GerberPrintLayout};
pub use dock::GerberDockPanel;
pub use gerber_viewer_state::{GerberViewerState, ViewerLayer};
pub use grid::GridUnit;
pub use layer_color::GerberColorTarget;
pub use measurement::GerberMeasurement;
pub use message::GerberViewerMessage;
pub use pcb_export::{GerberPcbExport, GerberPcbExportReport, GerberPcbSkippedItem};
pub use selection::GerberItemSelection;
pub use shortcuts::GerberShortcutResolver;
pub use view::view;
pub use workspace::{GerberDocumentId, GerberDocumentState, GerberWorkspaceState};

use layer_color::GerberMaterialColor;
use shortcuts::{gerber_shortcut_message, next_layer_index, previous_layer_index};

use viewport::{
    material_color_palette, material_compare_palette, material_d_code_color, material_grid_color,
    material_layer_palette, material_negative_ghost_color, page_bounds, visible_bounds,
    zoom_transform_for_selection,
};

#[cfg(test)]
use viewport::*;

use color_dropdown::color_dropdown;
pub(crate) use grid::{
    DEFAULT_GRID_INDEX, GridSizeChoice, GridSizePreset, create_grid_definition,
    default_grid_catalog, format_distance_input, grid_size_choices, persist_grid_catalog,
};
use grid::{
    GerberGridStyle, default_forced_opacity, default_inactive_layer_opacity, load_grid_catalog,
    load_grid_style, load_page_size, persist_page_size, system_decimal_separator,
};
use highlight::{
    attribute_highlight_color, component_highlight_color, d_code_highlight_color,
    net_highlight_color,
};
use layer_color::GerberLayerColorChoice;
use measurement::draw_measurement;
use selection::{
    hit_test_visible_item, hit_test_visible_items_in_bounds, selected_primitive_color,
};

const MAX_VIEWER_LAYERS: usize = 32;
const CANVAS_MARGIN: f32 = 28.0;
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 30.0;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/canvas.rs"]
mod gerber_canvas_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/toolbar.rs"]
mod gerber_toolbar_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/grid_state.rs"]
mod gerber_grid_state_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/layer_state.rs"]
mod gerber_layer_state_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/layer_order.rs"]
mod gerber_layer_order_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/layer_color.rs"]
mod gerber_layer_color_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/item_colors.rs"]
mod gerber_item_color_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/pcb_export.rs"]
mod gerber_pcb_export_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/clear_highlight.rs"]
mod gerber_clear_highlight_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/forced_opacity.rs"]
mod gerber_forced_opacity_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/flash_outline.rs"]
mod gerber_flash_outline_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/line_outline.rs"]
mod gerber_line_outline_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/polygon_outline.rs"]
mod gerber_polygon_outline_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/ghost_negatives.rs"]
mod gerber_ghost_negatives_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/d_code_labels.rs"]
mod gerber_d_code_labels_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/compare_mode.rs"]
mod gerber_compare_mode_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/dim_inactive_layers.rs"]
mod gerber_dim_inactive_layers_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/mirror_view.rs"]
mod gerber_mirror_view_test_definitions;

#[cfg(test)]
#[path = "../../tests/gerber_viewer/menu.rs"]
mod gerber_menu_test_definitions;

#[cfg(test)]
gerber_canvas_test_definitions::gerber_canvas_tests!();

#[cfg(test)]
gerber_toolbar_test_definitions::gerber_toolbar_tests!();

#[cfg(test)]
gerber_grid_state_test_definitions::gerber_grid_state_tests!();

#[cfg(test)]
gerber_layer_state_test_definitions::gerber_layer_state_tests!();

#[cfg(test)]
gerber_layer_order_test_definitions::gerber_layer_order_tests!();

#[cfg(test)]
gerber_layer_color_test_definitions::gerber_layer_color_tests!();

#[cfg(test)]
gerber_item_color_test_definitions::gerber_item_color_tests!();

#[cfg(test)]
gerber_pcb_export_test_definitions::gerber_pcb_export_tests!();

#[cfg(test)]
gerber_clear_highlight_test_definitions::gerber_clear_highlight_tests!();

#[cfg(test)]
gerber_forced_opacity_test_definitions::gerber_forced_opacity_tests!();

#[cfg(test)]
gerber_flash_outline_test_definitions::gerber_flash_outline_tests!();

#[cfg(test)]
gerber_line_outline_test_definitions::gerber_line_outline_tests!();

#[cfg(test)]
gerber_polygon_outline_test_definitions::gerber_polygon_outline_tests!();

#[cfg(test)]
gerber_ghost_negatives_test_definitions::gerber_ghost_negatives_tests!();

#[cfg(test)]
gerber_d_code_labels_test_definitions::gerber_d_code_labels_tests!();

#[cfg(test)]
gerber_compare_mode_test_definitions::gerber_compare_mode_tests!();

#[cfg(test)]
gerber_dim_inactive_layers_test_definitions::gerber_dim_inactive_layers_tests!();

#[cfg(test)]
gerber_mirror_view_test_definitions::gerber_mirror_view_tests!();
