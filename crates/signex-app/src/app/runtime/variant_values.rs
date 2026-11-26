//! The values a canvas draws for the project's active variant (#2948): a
//! part the variant gives a value of its own (Variant Manager, mockup row
//! 32) draws that value in place of its sheet's, as the PDF and SVG print
//! the printed variant's. Which value is decided by
//! `signex_output::DrawnValues`, the export's rule, over the sheet at the
//! placement the canvas shows; this file only feeds it the sheet and the
//! project. A sheet in no project, or Base, draws the sheets' values.

use std::path::Path;

use signex_output::{DrawnValueMap, DrawnValues};
use signex_types::schematic::InstancePath;

use super::super::*;

impl Signex {
    /// Re-resolve the variant values of every schematic canvas: the active
    /// one, the undocked windows and the unfocused tile panes. A canvas
    /// redraws only when its values changed.
    pub(crate) fn refresh_all_variant_values(&mut self) {
        let active = self
            .document_state
            .active_path
            .as_deref()
            .map(|path| self.variant_values_for_path(path))
            .unwrap_or_default();
        store_variant_values(self.interaction_state.active_canvas_mut(), active);
        let windows: Vec<(iced::window::Id, std::path::PathBuf)> = self
            .ui_state
            .windows
            .iter()
            .filter_map(|(id, kind)| match kind {
                crate::app::state::WindowKind::UndockedTab { path, .. } => {
                    Some((*id, path.clone()))
                }
                _ => None,
            })
            .collect();
        for (id, path) in windows {
            let values = self.variant_values_for_path(&path);
            if let Some(canvas) = self.interaction_state.canvases.get_mut(&id) {
                store_variant_values(canvas, values);
            }
        }
        let panes: Vec<std::path::PathBuf> = self
            .interaction_state
            .tile_panes
            .schematic
            .keys()
            .cloned()
            .collect();
        for path in panes {
            let values = self.variant_values_for_path(&path);
            if let Some(canvas) = self.interaction_state.tile_panes.schematic.get_mut(&path) {
                store_variant_values(canvas, values);
            }
        }
    }

    /// The values the active variant of the project owning `path` gives the
    /// parts of that sheet, at its [shown placement](Self::shown_placement)
    /// (the root chain when none is known). Empty for Base, a sheet in no
    /// project or a path with no engine.
    pub(crate) fn variant_values_for_path(&self, path: &Path) -> DrawnValueMap {
        let (Some(project), Some(engine)) = (
            self.document_state.document_project(path),
            self.document_state.engines.get(path),
        ) else {
            return DrawnValueMap::new();
        };
        let placement = self
            .shown_placement(path)
            .unwrap_or_else(InstancePath::root);
        let variant = Some(project.data.active_variant_name());
        let sheets = [(engine.document(), placement.clone())];
        DrawnValues::of(&sheets, variant, &project.data.variant_settings)
            .at(&placement)
            .cloned()
            .unwrap_or_default()
    }
}

/// Record `values` on `canvas` and redraw it when they changed.
fn store_variant_values(canvas: &mut crate::canvas::CanvasSlot, values: DrawnValueMap) {
    if canvas.variant_values != values {
        canvas.variant_values = values;
        canvas.clear_content_cache();
    }
}
