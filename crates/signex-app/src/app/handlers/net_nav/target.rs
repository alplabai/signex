//! What the user is pointing at, and which project net it sits on.

use std::path::PathBuf;

use signex_net::{NetMemberKind, SheetKey};
use signex_types::schematic::{InstancePath, Point, SchematicSheet, SelectedItem, SelectedKind};

use crate::app::state::net_nav::ProjectNetData;

use super::super::super::*;

/// An object on a sheet, and the point the user meant on it.
#[derive(Debug, Clone)]
pub(super) struct NetTarget {
    pub sheet: PathBuf,
    pub item: SelectedItem,
    pub at: Point,
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

/// The net of the pin `key` of the placed symbol `symbol` in the placement
/// `occurrence` (every placement when `None`).
pub(crate) fn pin_net(
    data: &ProjectNetData,
    sheet: &SheetKey,
    symbol: uuid::Uuid,
    key: &str,
    occurrence: Option<&InstancePath>,
) -> Option<usize> {
    data.index
        .nets_in(sheet, symbol, occurrence)
        .into_iter()
        .find(|&net| {
            data.index.net(net).is_some_and(|n| {
                n.members.iter().any(|m| {
                    m.id == symbol
                        && occurrence.is_none_or(|o| &m.occurrence == o)
                        && matches!(&m.kind, NetMemberKind::Pin { pin, .. } if pin == key)
                })
            })
        })
}

/// The net of `item` as the stitched index knows it. A placed symbol has one
/// net per pin, so it answers for the picked `pin` (see
/// [`Signex::pin_under`]), else for its only net.
pub(super) fn net_of_target(
    data: &ProjectNetData,
    sheet: &SheetKey,
    item: &SelectedItem,
    pin: Option<&str>,
    occurrence: Option<&InstancePath>,
) -> Option<usize> {
    let nets = data.index.nets_in(sheet, item.uuid, occurrence);
    match item.kind {
        SelectedKind::Symbol => {
            let by_pin = pin.and_then(|k| pin_net(data, sheet, item.uuid, k, occurrence));
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

/// The net a hover card is for. Unlike [`net_of_target`], a placed symbol
/// answers only for a picked pin; its body belongs to the part, whose own
/// card shows there. A power port is the exception: it is a net name, so
/// the card shows over the whole port, as the highlight does.
pub(super) fn net_of_hover(
    data: &ProjectNetData,
    sheet: &SheetKey,
    item: &SelectedItem,
    pin: Option<&str>,
    is_power: bool,
    occurrence: Option<&InstancePath>,
) -> Option<usize> {
    if item.kind == SelectedKind::Symbol && !is_power {
        return pin.and_then(|k| pin_net(data, sheet, item.uuid, k, occurrence));
    }
    net_of_target(data, sheet, item, pin, occurrence)
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
    /// Pixels per mm of the focused schematic canvas.
    pub(crate) fn schematic_px_per_mm(&self) -> f64 {
        self.interaction_state.active_canvas().zoom_percent() / 100.0
            * f64::from(signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE)
    }

    /// Whether `item` is a power port on the sheet at `sheet`.
    pub(crate) fn is_power_port(&self, sheet: &std::path::Path, item: &SelectedItem) -> bool {
        item.kind == SelectedKind::Symbol
            && self.document_state.engines.get(sheet).is_some_and(|e| {
                e.document()
                    .symbols
                    .iter()
                    .any(|s| s.uuid == item.uuid && s.is_power)
            })
    }

    /// The pin of the placed symbol `item` that the point `at` picks on the
    /// sheet at `sheet`: the one rule for hover, click and the net highlight
    /// (`signex_types::schematic::symbol_pin_at`).
    pub(crate) fn pin_under(
        &self,
        sheet: &std::path::Path,
        item: &SelectedItem,
        at: Point,
    ) -> Option<String> {
        if item.kind != SelectedKind::Symbol {
            return None;
        }
        let doc = self.document_state.engines.get(sheet)?.document();
        let tolerance = signex_types::schematic::pin_pick_tolerance_mm(self.schematic_px_per_mm());
        signex_types::schematic::symbol_pin_at(doc, item.uuid, at, tolerance).map(|p| p.key)
    }

    /// The net name of each of `pins` (keys) of the placed symbol `symbol` on
    /// the sheet at `sheet`, from the project net index in the placement the
    /// sheet shows (the hover card reads the same). Empty while the index is
    /// not built: this never starts a build.
    pub(crate) fn pin_net_names(
        &self,
        sheet: &std::path::Path,
        symbol: uuid::Uuid,
        pins: &[String],
    ) -> Vec<(String, String)> {
        let Some(project) = self.project_of_sheet(sheet) else {
            return Vec::new();
        };
        let Some(data) = self.ready_net_data(project) else {
            return Vec::new();
        };
        let Some(key) = data.key_of(sheet) else {
            return Vec::new();
        };
        let occurrence = self.net_occurrence(&data, sheet);
        pins.iter()
            .filter_map(|pin| {
                let net = pin_net(&data, &key, symbol, pin, occurrence.as_ref())?;
                Some((pin.clone(), data.index.net(net)?.name.clone()))
            })
            .collect()
    }

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
            sheet,
            item: *item,
            at,
        })
    }

    /// The placement the net index is read through for the sheet at `path`:
    /// exactly the one the designators show (`shown_placement`), so a window
    /// never reads one placement's nets while drawing another's designators.
    /// A placement the index no longer reaches reads no nets (everything
    /// dims). Only when the designators show none does the default placement
    /// (the smallest the walk reaches) stand in.
    pub(super) fn net_occurrence(
        &self,
        data: &ProjectNetData,
        path: &std::path::Path,
    ) -> Option<InstancePath> {
        if let Some(shown) = self.shown_placement(path) {
            return Some(shown);
        }
        let live = data.index.placements(&data.key_of(path)?);
        crate::app::state::default_placement(live.iter().cloned())
    }

    /// The project that owns the sheet at `path`.
    pub(super) fn project_of_sheet(
        &self,
        path: &std::path::Path,
    ) -> Option<crate::app::state::ProjectId> {
        self.document_state.document_project(path).map(|p| p.id)
    }
}
