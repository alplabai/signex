//! Rotate / mirror the selection (Space / Shift+Space, R / Shift+R, X / Y,
//! Active Bar). A lone symbol (with or without its own ref/value fields)
//! turns in place about its own origin; any other
//! selection turns about its centre, snapped to the grid, as one engine
//! command (`Command::TransformSelection`).

use signex_engine::{Command, MirrorAxis, SelectionTransform};

use signex_types::schematic::BusEntry;

use crate::app::{Signex, Tool};

impl Signex {
    /// Rotate the selection 90 degrees on screen. The engine's
    /// `RotateSelection` delta is counter-clockwise on screen for every
    /// symbol (it compensates for mirroring), so clockwise is -90.
    /// With the Bus Entry tool armed the turn applies to the entry about to
    /// be placed instead.
    pub(crate) fn handle_selection_rotate_requested(&mut self, clockwise: bool) {
        if self.interaction_state.current_tool == Tool::BusEntry {
            let quarter_turns = if clockwise { -1 } else { 1 };
            self.set_pending_bus_entry_size(|size| BusEntry::rotated_size(size, quarter_turns));
            return;
        }
        let selected = self.interaction_state.active_canvas().selected.clone();
        if let Some(item) = self
            .lone_symbol(&selected)
            .or_else(|| self.lone_bus_entry(&selected))
        {
            self.apply_engine_command(
                Command::RotateSelection {
                    items: item,
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
        if self.interaction_state.current_tool == Tool::BusEntry {
            self.set_pending_bus_entry_size(BusEntry::flipped_vertical);
            return;
        }
        self.mirror_selection(MirrorAxis::Vertical);
    }

    pub(crate) fn handle_selection_mirror_y_requested(&mut self) {
        if self.interaction_state.current_tool == Tool::BusEntry {
            self.set_pending_bus_entry_size(BusEntry::flipped_horizontal);
            return;
        }
        self.mirror_selection(MirrorAxis::Horizontal);
    }

    fn mirror_selection(&mut self, axis: MirrorAxis) {
        let selected = self.interaction_state.active_canvas().selected.clone();
        if let Some(item) = self
            .lone_symbol(&selected)
            .or_else(|| self.lone_bus_entry(&selected))
        {
            self.apply_engine_command(Command::MirrorSelection { items: item, axis }, true, true);
            return;
        }
        self.transform_selection_about_centre(selected, SelectionTransform::Mirror(axis));
    }

    /// The selection as one bus entry. An entry turns about its bus end, so
    /// it stays on its bus (`position` never moves), rather than about a
    /// grid-snapped centre.
    fn lone_bus_entry(
        &self,
        selected: &[signex_types::schematic::SelectedItem],
    ) -> Option<Vec<signex_types::schematic::SelectedItem>> {
        let engine = self.document_state.active_engine()?;
        engine
            .selection_is_single_bus_entry(selected)
            .then(|| selected.to_vec())
    }

    /// Re-orient the bus entry about to be placed and refresh its ghost.
    fn set_pending_bus_entry_size(&mut self, turn: impl FnOnce((f64, f64)) -> (f64, f64)) {
        let size = turn(self.interaction_state.bus_entry_size);
        self.interaction_state.bus_entry_size = size;
        self.interaction_state.active_canvas_mut().ghost_bus_entry = Some(size);
    }

    /// The selection as a lone symbol, if that is what it is: one symbol,
    /// optionally with its own reference / value fields (box-select and
    /// lasso pick those up with it). Such a selection turns in place about
    /// the symbol's origin rather than about a grid-snapped centre, which
    /// would shift an off-grid symbol.
    fn lone_symbol(
        &self,
        selected: &[signex_types::schematic::SelectedItem],
    ) -> Option<Vec<signex_types::schematic::SelectedItem>> {
        use signex_types::schematic::SelectedKind;
        let mut symbols = selected.iter().filter(|i| i.kind == SelectedKind::Symbol);
        let symbol = *symbols.next()?;
        if symbols.next().is_some() {
            return None;
        }
        let only_own_fields = selected.iter().all(|i| {
            i.kind == SelectedKind::Symbol
                || (matches!(
                    i.kind,
                    SelectedKind::SymbolRefField | SelectedKind::SymbolValField
                ) && i.uuid == symbol.uuid)
        });
        let engine = self.document_state.active_engine()?;
        (only_own_fields && engine.selection_is_single_symbol(&[symbol])).then(|| vec![symbol])
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
