//! What the user is pointing at, and which project net it sits on.

use std::path::PathBuf;

use signex_net::SheetKey;
use signex_types::schematic::{InstancePath, Point, SchematicSheet, SelectedItem, SelectedKind};

use crate::app::state::net_nav::ProjectNetData;

use super::super::super::*;

/// How close to a pin's tip the cursor must be to mean that pin, in mm: half
/// the default grid.
pub(super) const PIN_TOLERANCE_MM: f64 = 1.27;

/// An object on a sheet, and the point the user meant on it.
#[derive(Debug, Clone)]
pub(super) struct NetTarget {
    pub sheet: PathBuf,
    pub item: SelectedItem,
    pub at: Point,
    /// The placement the sheet's tab was entered through; `None` when it was
    /// opened directly, which reads the first placement.
    pub occurrence: Option<InstancePath>,
}

/// The kinds of object that sit on a net.
pub(super) fn is_net_kind(kind: SelectedKind) -> bool {
    matches!(
        kind,
        SelectedKind::Wire
            | SelectedKind::Junction
            | SelectedKind::Label
            | SelectedKind::SheetPin
            | SelectedKind::Symbol
    )
}

/// The net of `item` as the stitched index knows it. A placed symbol has one
/// net per pin, so it answers only for the pin at `at`.
pub(super) fn net_of_target(
    data: &ProjectNetData,
    sheet: &SheetKey,
    item: &SelectedItem,
    at: Point,
    occurrence: Option<&InstancePath>,
) -> Option<usize> {
    let nets = data.index.nets_in(sheet, item.uuid, occurrence);
    match item.kind {
        SelectedKind::Symbol => {
            let by_pin = data
                .index
                .pin_near(sheet, at, PIN_TOLERANCE_MM, occurrence)
                .map(|(net, _)| net);
            let only = match nets.as_slice() {
                [only] => Some(*only),
                _ => None,
            };
            by_pin.or(only)
        }
        kind if is_net_kind(kind) => nets.first().copied(),
        _ => None,
    }
}

/// Where `item` sits on `sheet`: a point on it to centre the view on.
pub(super) fn item_position(sheet: &SchematicSheet, item: &SelectedItem) -> Option<Point> {
    let id = item.uuid;
    match item.kind {
        SelectedKind::Symbol => sheet
            .symbols
            .iter()
            .find(|s| s.uuid == id)
            .map(|s| s.position),
        SelectedKind::Label => sheet
            .labels
            .iter()
            .find(|l| l.uuid == id)
            .map(|l| l.position),
        SelectedKind::Junction => sheet
            .junctions
            .iter()
            .find(|j| j.uuid == id)
            .map(|j| j.position),
        SelectedKind::Wire => sheet
            .wires
            .iter()
            .find(|w| w.uuid == id)
            .map(|w| Point::new((w.start.x + w.end.x) / 2.0, (w.start.y + w.end.y) / 2.0)),
        SelectedKind::SheetPin => sheet
            .child_sheets
            .iter()
            .flat_map(|cs| &cs.pins)
            .find(|p| p.uuid == id)
            .map(|p| p.position),
        _ => None,
    }
}

impl Signex {
    /// The object under `(x, y)` on the focused sheet.
    pub(crate) fn item_hit_at(&self, x: f64, y: f64) -> Option<SelectedItem> {
        let canvas = self.interaction_state.active_canvas();
        let snapshot = canvas.active_snapshot()?;
        crate::schematic_runtime::hit_test::hit_test_styled(
            snapshot,
            &canvas.designators,
            x,
            y,
            self.ui_state.net_class_styles.styles(),
        )
    }

    /// The object the cursor rests on, on the focused sheet.
    pub(super) fn target_under_cursor(&self) -> Option<NetTarget> {
        let (x, y) = self.cursor_world_now()?;
        let item = self.item_hit_at(x, y).filter(|i| is_net_kind(i.kind))?;
        let sheet = self.document_state.active_path.clone()?;
        Some(NetTarget {
            occurrence: self.placement_of(&sheet),
            sheet,
            item,
            at: Point::new(x, y),
        })
    }

    /// The one selected object of the focused sheet, when it sits on a net.
    pub(super) fn target_from_selection(&self) -> Option<NetTarget> {
        let canvas = self.interaction_state.active_canvas();
        let [item] = canvas.selected.as_slice() else {
            return None;
        };
        if !is_net_kind(item.kind) {
            return None;
        }
        let at = item_position(canvas.active_snapshot()?, item)?;
        let sheet = self.document_state.active_path.clone()?;
        Some(NetTarget {
            occurrence: self.placement_of(&sheet),
            sheet,
            item: *item,
            at,
        })
    }

    /// The placement the tab of `sheet` was entered through (#1133).
    pub(super) fn placement_of(&self, sheet: &std::path::Path) -> Option<InstancePath> {
        self.document_state.tab_placements.get(sheet).cloned()
    }

    /// The project that owns the sheet at `path`.
    pub(super) fn project_of_sheet(
        &self,
        path: &std::path::Path,
    ) -> Option<crate::app::state::ProjectId> {
        self.document_state.document_project(path).map(|p| p.id)
    }
}
