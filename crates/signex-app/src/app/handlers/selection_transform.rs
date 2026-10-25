//! Rotate / mirror the selection (Space / Shift+Space, R / Shift+R, X / Y,
//! Active Bar). A lone symbol turns in place about its own origin; any other
//! selection turns about its centre, snapped to the grid, as one engine
//! command (`Command::TransformSelection`).

use signex_engine::{Command, MirrorAxis, SelectionTransform};

use crate::app::Signex;

impl Signex {
    /// Rotate the selection 90 degrees on screen. The engine's
    /// `RotateSelection` delta is counter-clockwise on screen for every
    /// symbol (it compensates for mirroring), so clockwise is -90.
    pub(crate) fn handle_selection_rotate_requested(&mut self, clockwise: bool) {
        let selected = self.interaction_state.active_canvas().selected.clone();
        if self.selection_is_single_symbol(&selected) {
            self.apply_engine_command(
                Command::RotateSelection {
                    items: selected,
                    angle_degrees: if clockwise { -90.0 } else { 90.0 },
                },
                true,
                true,
            );
            return;
        }
        let transform = if clockwise {
            SelectionTransform::RotateClockwise
        } else {
            SelectionTransform::RotateCounterclockwise
        };
        self.transform_selection_about_centre(selected, transform);
    }

    pub(crate) fn handle_selection_mirror_x_requested(&mut self) {
        self.mirror_selection(MirrorAxis::Vertical);
    }

    pub(crate) fn handle_selection_mirror_y_requested(&mut self) {
        self.mirror_selection(MirrorAxis::Horizontal);
    }

    fn mirror_selection(&mut self, axis: MirrorAxis) {
        let selected = self.interaction_state.active_canvas().selected.clone();
        if self.selection_is_single_symbol(&selected) {
            self.apply_engine_command(
                Command::MirrorSelection {
                    items: selected,
                    axis,
                },
                true,
                true,
            );
            return;
        }
        self.transform_selection_about_centre(selected, SelectionTransform::Mirror(axis));
    }

    fn selection_is_single_symbol(
        &self,
        selected: &[signex_types::schematic::SelectedItem],
    ) -> bool {
        self.document_state
            .active_engine()
            .is_some_and(|engine| engine.selection_is_single_symbol(selected))
    }

    /// Turn `selected` about its bounding-box centre, rounded to the grid so
    /// pins, wire ends and labels that were on the grid stay on it. An
    /// unsupported item refuses the whole action; the engine's reason is
    /// logged to the Messages panel and nothing changes.
    fn transform_selection_about_centre(
        &mut self,
        selected: Vec<signex_types::schematic::SelectedItem>,
        transform: SelectionTransform,
    ) {
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let Some(centre) = engine.selection_centre(&selected) else {
            // Nothing transformable is selected; let the engine name a
            // sheet symbol / pin / field if that is what blocked it.
            if !selected.is_empty() {
                self.apply_engine_command(
                    Command::TransformSelection {
                        items: selected,
                        centre: signex_types::schematic::Point::new(0.0, 0.0),
                        transform,
                    },
                    true,
                    true,
                );
            }
            return;
        };
        let grid = f64::from(self.ui_state.grid_size_mm);
        let centre = if grid > 0.0 {
            signex_types::schematic::Point::new(
                (centre.x / grid).round() * grid,
                (centre.y / grid).round() * grid,
            )
        } else {
            centre
        };
        self.apply_engine_command(
            Command::TransformSelection {
                items: selected,
                centre,
                transform,
            },
            true,
            true,
        );
    }
}
