//! The facts of the area panel (#2393): fills, solid regions, polygon pours
//! and keepouts, read from the zones. The kind is named the way the pick
//! filter names it (`pcb_geom::PickKind::of_zone`). A pour also shows its
//! priority, island rules and the copper its fill holds (#2216), and its
//! Copper card repours and shelves (#2523): Shelve is one board undo step,
//! Repour refreshes the computed fill and changes nothing in the document.

use std::path::Path;

use pcb_geom::{ItemRef, PickKind, ZoneFills};
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::pour_edit::shelve_all_pours;
use signex_engine::pcb::primitive_edit::{PrimitiveEditRefusal, ZoneEdit, zone_edit_command};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Zone};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::zone_keepout::KEEPOUT_FILL;

use super::PrimitiveEdit;
use crate::app::Signex;
use crate::app::state::pcb_edit::CheckedStep;
use crate::panels::pcb_primitive_properties::{
    AreaLayerChoice, PourMsg, PrimitiveCommon, PrimitiveField, PrimitiveTarget, ZoneFacts,
    ZoneKind, shared,
};
use crate::pcb_canvas::layer_style;

impl Signex {
    /// An event of a pour's Copper card (#2523).
    pub(crate) fn handle_pour_msg(&mut self, msg: &PourMsg) {
        let panel = &mut self.interaction_state.pcb_primitive;
        match msg {
            PourMsg::MenuToggled => {
                panel.pour_menu_open = !panel.pour_menu_open;
                return;
            }
            PourMsg::RepourPressed => self.repour_board_in_view(true),
            PourMsg::ShelveToggled { target, shelved } => self.shelve_pours(target, *shelved),
            PourMsg::RepourAllPressed => self.repour_board_in_view(false),
            PourMsg::ShelveAllPressed => self.shelve_all_pours_in_view(true),
            PourMsg::UnshelveAllPressed => self.shelve_all_pours_in_view(false),
        }
        // A pick closes the menu.
        self.interaction_state.pcb_primitive.pour_menu_open = false;
    }

    /// Shelve or unshelve the selected pours as one undo step.
    fn shelve_pours(&mut self, target: &PrimitiveTarget, shelved: bool) {
        if target.is_placement() {
            return;
        }
        let zones: Vec<_> = target
            .items
            .iter()
            .filter_map(|i| match i {
                ItemRef::Zone { zone, .. } => Some(*zone),
                _ => None,
            })
            .collect();
        let what = if shelved {
            ids::PCB_PRIM_POUR_SHELVE
        } else {
            ids::PCB_PRIM_POUR_UNSHELVE
        };
        self.run_pour_command(&target.board, what, |board| {
            zone_edit_command(board, &zones, ZoneEdit::Shelved(shelved))
        });
    }

    /// Shelve or unshelve every pour of the board in view as one undo step.
    fn shelve_all_pours_in_view(&mut self, shelved: bool) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        let what = if shelved {
            ids::PCB_PRIM_POUR_SHELVE_ALL
        } else {
            ids::PCB_PRIM_POUR_UNSHELVE_ALL
        };
        self.run_pour_command(&path, what, |board| Ok(shelve_all_pours(board, shelved)));
    }

    /// Run the command `command` builds on the board at `path` as one
    /// labelled undo step, or say in Messages why not. Nothing to change is
    /// no step.
    fn run_pour_command(
        &mut self,
        path: &Path,
        what: MessageId,
        command: impl FnOnce(&PcbBoard) -> Result<Option<BoardCommand>, PrimitiveEditRefusal>,
    ) {
        let Some(engine) = self.document_state.board_engine(path) else {
            let name = t!(self.ui_state.i18n, what).into_string();
            let line = t!(self.ui_state.i18n, ids::PCB_PRIM_BOARD_GONE, "what" => name);
            crate::diagnostics::log_warning(line.into_string());
            return;
        };
        if engine.board().unknown.newer_minor_source.is_some() {
            let why = t!(self.ui_state.i18n, ids::PCB_PRIM_READ_ONLY).into_string();
            self.refuse_primitive_edit(what, why);
            return;
        }
        let revision = engine.revision();
        let command = match command(engine.board()) {
            Ok(Some(command)) => command,
            Ok(None) => return,
            Err(refusal) => {
                self.refuse_primitive_edit(what, refusal.to_string());
                return;
            }
        };
        let label = t!(self.ui_state.i18n, what).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: revision,
        };
        if let Err(error) = self.execute_board_commands(path, vec![command], Some(step)) {
            self.refuse_primitive_edit(what, error.to_string());
        }
    }
}

/// What kind of area `zone` is; `None` for a zone none of the tools makes.
fn zone_kind(zone: &Zone) -> Option<ZoneKind> {
    if zone.is_pour() {
        return Some(ZoneKind::Pour);
    }
    match PickKind::of_zone(zone)? {
        PickKind::Keepouts => Some(ZoneKind::Keepout),
        PickKind::Fills => Some(ZoneKind::Fill),
        PickKind::Regions => Some(ZoneKind::Region),
        _ => None,
    }
}

/// The layers an area can be on: the board's copper layers, and for
/// keepouts the two keep-out layers too.
fn area_layers(board: &PcbBoard, keepout: bool, i18n: &I18n) -> Vec<AreaLayerChoice> {
    let copper = signex_renderer::pcb::board_copper_layers(&board.layers)
        .into_iter()
        .map(PcbLayer::Copper);
    let keep_out = [TechnicalLayer::KeepoutTop, TechnicalLayer::KeepoutBottom]
        .into_iter()
        .map(PcbLayer::Technical)
        .filter(|_| keepout);
    copper
        .chain(keep_out)
        .map(|layer| AreaLayerChoice {
            layer,
            label: layer_style::layer_name(i18n, layer).into_string(),
        })
        .collect()
}

/// The copper the fills of the pours in `zones` hold: area in mm² and
/// pieces. `None` without fills, or when a zone is not a filled pour.
fn pour_copper(
    board: &PcbBoard,
    zones: &[&Zone],
    fills: Option<&ZoneFills>,
) -> Option<(f64, usize)> {
    let fills = fills?;
    let mut total = (0.0, 0);
    for zone in zones {
        let index = board.zones.iter().position(|z| z.uuid == zone.uuid)?;
        let fill = fills.of(index).filter(|f| f.zone == zone.uuid)?;
        total.0 += fill.area_mm2();
        total.1 += fill.pieces.len();
    }
    Some(total)
}

/// The facts of the area panel for `zones`; `fills` are the board's pour
/// fills when they are computed.
pub(super) fn zone_facts(
    board: &PcbBoard,
    common: PrimitiveCommon,
    zones: &[&Zone],
    fills: Option<&ZoneFills>,
    i18n: &I18n,
) -> ZoneFacts {
    let kind = shared(zones.iter().map(|z| zone_kind(z))).flatten();
    let all_keepouts = zones.iter().all(|z| z.fill_type == KEEPOUT_FILL);
    let layer = shared(zones.iter().map(|z| z.layer.as_str()))
        .and_then(parse_pcb_layer)
        .map(|layer| AreaLayerChoice {
            layer,
            label: layer_style::layer_name(i18n, layer).into_string(),
        });
    ZoneFacts {
        kind,
        is_copper: zones.iter().all(|z| z.fill_type != KEEPOUT_FILL),
        is_pour: zones.iter().all(|z| z.is_pour()),
        layer,
        layers: area_layers(board, all_keepouts, i18n),
        clearance: shared(zones.iter().map(|z| z.clearance)),
        min_thickness: shared(zones.iter().map(|z| z.min_thickness)),
        priority: shared(zones.iter().map(|z| z.priority)),
        min_island_area: shared(zones.iter().map(|z| z.min_island_area)),
        remove_dead_islands: shared(zones.iter().map(|z| z.remove_dead_islands)),
        copper: pour_copper(board, zones, fills),
        shelved: zones
            .iter()
            .all(|z| z.is_pour())
            .then(|| shared(zones.iter().map(|z| z.shelved)))
            .flatten(),
        thermal: shared(
            zones
                .iter()
                .map(|z| (z.thermal_relief, z.thermal_width, z.thermal_gap)),
        ),
        menu_open: false,
        common,
    }
}

/// The area edit `edit` stands for; `None` for a field areas do not have.
pub(crate) fn zone_edit(edit: &PrimitiveEdit) -> Option<ZoneEdit> {
    use PrimitiveField as F;
    Some(match *edit {
        PrimitiveEdit::AreaLayer(layer) => ZoneEdit::Layer(layer),
        PrimitiveEdit::Layer(layer) => ZoneEdit::Layer(PcbLayer::Copper(layer)),
        PrimitiveEdit::Net(net) => ZoneEdit::Net(net),
        PrimitiveEdit::Number(F::Clearance, v) => ZoneEdit::Clearance(v),
        PrimitiveEdit::Number(F::MinThickness, v) => ZoneEdit::MinThickness(v),
        PrimitiveEdit::Number(F::Priority, v) => ZoneEdit::Priority(v),
        PrimitiveEdit::Number(F::MinIslandArea, v) => ZoneEdit::MinIslandArea(v),
        PrimitiveEdit::DeadIslands(on) => ZoneEdit::RemoveDeadIslands(on),
        _ => return None,
    })
}

/// The command for `edit` on the selected areas `items`.
pub(super) fn zone_command(
    board: &PcbBoard,
    items: &[ItemRef],
    edit: &PrimitiveEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let ids: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            ItemRef::Zone { zone, .. } => Some(*zone),
            _ => None,
        })
        .collect();
    match zone_edit(edit) {
        Some(e) => zone_edit_command(board, &ids, e),
        None => Ok(None),
    }
}
