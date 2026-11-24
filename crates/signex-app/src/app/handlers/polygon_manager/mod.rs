//! The Polygon Manager's events (mockup row 178), from its tab and from the
//! PCB panel's Polygons mode. View state goes to `PcbPanelUi::polygons`;
//! every edit is the zone command the pour's Properties runs for the same
//! field (`zone_edit_command`, `shelve_all_pours`), one board undo step
//! with the same label, so it shows in Properties at once (DECISIONS
//! 2026-11-18). Repour is the Repour Selected / Repour All path: the fill is
//! computed, not stored, so it makes no undo step.

mod facts;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::Path;

use iced::Task;
use signex_engine::pcb::pour_edit::shelve_all_pours;
use signex_engine::pcb::primitive_edit::{ZoneEdit, zone_edit_command};
use signex_i18n::{MessageId, ids, t};
use uuid::Uuid;

use crate::app::{Message, Signex, TabKind};
use pcb_geom::ItemRef;

use crate::panels::PolygonFacts;
use crate::panels::polygons::{PolygonMsg, PourField};

/// The zone edit a typed `value` of `field` stands for.
fn field_edit(field: PourField, value: f64) -> ZoneEdit {
    match field {
        PourField::Clearance => ZoneEdit::Clearance(value),
        PourField::MinWidth => ZoneEdit::MinThickness(value),
        PourField::IslandsBelow => ZoneEdit::MinIslandArea(value),
    }
}

impl Signex {
    pub(crate) fn handle_polygons(&mut self, msg: PolygonMsg) -> Task<Message> {
        if msg == PolygonMsg::ManagerOpened {
            self.open_polygon_manager();
            return Task::none();
        }
        self.document_state.panel_ctx.pcb_panel.polygons.apply(&msg);
        self.act_on_polygons(&msg);
        Task::none()
    }

    /// Open or focus the Polygon Manager of the board in view, with the
    /// pours selected on the board ticked, or else the pour under the
    /// pointer (a pour's right-click).
    fn open_polygon_manager(&mut self) {
        let picked = self
            .page_board()
            .map(|board| self.pours_to_tick(&board))
            .unwrap_or_default();
        let Some(board) = self.open_board_page_tab(TabKind::PolygonManager) else {
            return;
        };
        if !picked.is_empty() {
            let ui = &mut self.document_state.panel_ctx.pcb_panel.polygons;
            ui.follow(&board);
            ui.current = picked.iter().next().copied();
            ui.selected = picked;
        }
        self.refresh_polygon_facts();
    }

    /// The pours selected on `board`; without one, the pour under the
    /// pointer, which a right-click on a pour leaves there.
    fn pours_to_tick(&self, board: &Path) -> BTreeSet<Uuid> {
        let selected = self.selected_pours(board);
        if !selected.is_empty() {
            return selected;
        }
        let state = &self.interaction_state.pcb_selection;
        let Some(pcb) = self
            .document_state
            .pcb_board(board)
            .filter(|_| state.document() == Some(board))
        else {
            return selected;
        };
        match state.hover() {
            Some(ItemRef::Zone { zone, .. })
                if pcb.zones.iter().any(|z| z.uuid == zone && z.is_pour()) =>
            {
                BTreeSet::from([zone])
            }
            _ => selected,
        }
    }

    /// The board part of `msg`; its view state is already applied.
    fn act_on_polygons(&mut self, msg: &PolygonMsg) {
        use PolygonMsg as M;
        const DEAD: MessageId = ids::PCB_PRIM_REMOVE_DEAD_ISLANDS;
        match msg {
            M::FieldSubmitted { board, zone, field } => self.submit_field(board, *zone, *field),
            M::BulkSubmitted { board, field } => self.submit_bulk(board, *field),
            M::DeadCopperToggled { board, zone, on } => {
                self.edit_pours(board, vec![*zone], ZoneEdit::RemoveDeadIslands(*on), DEAD);
            }
            M::BulkDeadCopperToggled { board, on } => {
                self.edit_ticked(board, ZoneEdit::RemoveDeadIslands(*on), DEAD);
            }
            M::RepourAllPressed { board } => self.repour_board(board, false),
            M::RepourSelectedPressed { board } => {
                let zones: BTreeSet<Uuid> = self.ticked_pours(board).into_iter().collect();
                self.repour_pours(board, &zones);
            }
            M::RepourPressed { board, zone } => self.repour_pours(board, &BTreeSet::from([*zone])),
            M::ShelveAllPressed { board } => self.shelve_every_pour(board, true),
            M::UnshelveAllPressed { board } => self.shelve_every_pour(board, false),
            M::ShelveSelectedPressed { board } => {
                self.edit_ticked(board, ZoneEdit::Shelved(true), ids::PCB_PRIM_POUR_SHELVE);
            }
            M::ShelveToggled {
                board,
                zone,
                shelved,
            } => self.shelve_one(board, *zone, *shelved),
            M::ManagerOpened
            | M::SubTabPicked { .. }
            | M::LayerFilterPicked { .. }
            | M::NetFilterPicked { .. }
            | M::StateFilterPicked { .. }
            | M::AttentionPicked { .. }
            | M::RowClicked { .. }
            | M::RowTicked { .. }
            | M::ShownTicked { .. }
            | M::FieldTyped { .. }
            | M::BulkTyped { .. } => return,
        }
        // Repour refreshed the fills; an edit refreshed the rows with the
        // board. Either way the rows read the fills as they are now.
        self.refresh_polygon_facts();
    }

    /// Enter in a cell: its draft, read as a number, goes to that pour.
    fn submit_field(&mut self, board: &Path, zone: Uuid, field: PourField) {
        let ui = &mut self.document_state.panel_ctx.pcb_panel.polygons;
        let Some(text) = ui.drafts.remove(&(zone, field)) else {
            return;
        };
        self.apply_typed(board, vec![zone], field, &text);
    }

    /// Enter in a bulk field: its draft goes to every ticked pour.
    fn submit_bulk(&mut self, board: &Path, field: PourField) {
        let ui = &mut self.document_state.panel_ctx.pcb_panel.polygons;
        let Some(text) = ui.bulk_drafts.remove(&field) else {
            return;
        };
        let zones = self.ticked_pours(board);
        self.apply_typed(board, zones, field, &text);
    }

    /// `text` typed for `field` on `zones`, as one step; a text that is no
    /// number changes nothing and says so in Messages.
    fn apply_typed(&mut self, board: &Path, zones: Vec<Uuid>, field: PourField, text: &str) {
        let what = field.message();
        match field.read(text) {
            Some(value) => self.edit_pours(board, zones, field_edit(field, value), what),
            None => {
                let line = t!(self.ui_state.i18n, ids::PCB_PRIM_NOT_A_NUMBER, "text" => text);
                self.refuse_primitive_edit(what, line.into_string());
            }
        }
    }

    /// `edit` on the pours `zones` of `board` as one undo step labelled
    /// `what`, or why not, as Properties runs it.
    fn edit_pours(&mut self, board: &Path, zones: Vec<Uuid>, edit: ZoneEdit, what: MessageId) {
        if zones.is_empty() {
            return;
        }
        self.run_guarded_board_step(board, what, move |b| {
            Ok(zone_edit_command(b, &zones, edit)?.into_iter().collect())
        });
    }

    /// `edit` on the ticked pours of `board`, as one undo step.
    fn edit_ticked(&mut self, board: &Path, edit: ZoneEdit, what: MessageId) {
        let zones = self.ticked_pours(board);
        self.edit_pours(board, zones, edit, what);
    }

    /// Shelve or unshelve one pour, as its Copper card does.
    fn shelve_one(&mut self, board: &Path, zone: Uuid, shelved: bool) {
        let what = if shelved {
            ids::PCB_PRIM_POUR_SHELVE
        } else {
            ids::PCB_PRIM_POUR_UNSHELVE
        };
        self.edit_pours(board, vec![zone], ZoneEdit::Shelved(shelved), what);
    }

    /// Shelve or unshelve every pour of `board` as one undo step.
    fn shelve_every_pour(&mut self, board: &Path, shelved: bool) {
        let what = if shelved {
            ids::PCB_PRIM_POUR_SHELVE_ALL
        } else {
            ids::PCB_PRIM_POUR_UNSHELVE_ALL
        };
        self.run_guarded_board_step(board, what, |b| {
            Ok(shelve_all_pours(b, shelved).into_iter().collect())
        });
    }

    /// The ticked rows that are still pours of `board`.
    fn ticked_pours(&self, board: &Path) -> Vec<Uuid> {
        let ui = &self.document_state.panel_ctx.pcb_panel.polygons;
        let Some(pcb) = self
            .document_state
            .pcb_board(board)
            .filter(|_| ui.is_for(board))
        else {
            return Vec::new();
        };
        pcb.zones
            .iter()
            .filter(|zone| zone.is_pour() && ui.selected.contains(&zone.uuid))
            .map(|zone| zone.uuid)
            .collect()
    }

    /// The rows of the board in view, on its PCB tab or its Polygon Manager
    /// tab; `None` without one.
    pub(crate) fn polygon_facts(&self) -> Option<PolygonFacts> {
        let docs = &self.document_state;
        let tab = docs.tabs.get(docs.active_tab)?;
        if !matches!(tab.kind, TabKind::Pcb | TabKind::PolygonManager) {
            return None;
        }
        let board = docs.pcb_board(&tab.path)?;
        let fills = self.current_pour_fills(&tab.path);
        Some(PolygonFacts {
            board: tab.path.clone(),
            read_only: board.unknown.newer_minor_source.is_some(),
            rows: facts::pour_rows(board, fills, &self.ui_state.i18n),
        })
    }

    /// Fill the board of the Polygon Manager tab in view for its current
    /// revision when its fills are not, then read the rows again.
    pub(crate) fn refresh_polygon_facts(&mut self) {
        self.fill_polygon_manager_board();
        self.document_state.panel_ctx.polygons = self.polygon_facts();
    }

    /// On a Polygon Manager tab no canvas fills the board, so the tab fills
    /// it itself: after a tab switch, a reload or an edit, the fills may be
    /// another board's or an older revision's. A no-op on any other tab, and
    /// when the fills are already this revision's.
    pub(crate) fn fill_polygon_manager_board(&mut self) {
        let docs = &self.document_state;
        let Some(tab) = docs.tabs.get(docs.active_tab) else {
            return;
        };
        if !tab.kind.is_polygon_manager() {
            return;
        }
        let path = tab.path.clone();
        if let Some(&revision) = docs.pcb_revisions.get(&path)
            && let Some(board) = docs.pcb_board(&path)
        {
            self.interaction_state
                .pcb_ratsnest
                .refresh(Some((path, revision)), board);
        }
    }
}
