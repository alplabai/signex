//! Repour Selected and Repour All (#2216). A pour's fill is computed from
//! the board and never stored; it is refreshed whenever the board changes.
//! Repour computes the fills of the board in view again at once and says
//! what they came to: in Messages for a click, key or palette row, and as
//! JSON for an agent or the CLI (`repour_all document=<board>`). Fills
//! depend on each other (priority, pour-to-pour clearance), so the whole
//! board is filled; Repour Selected reports the selected pours. Nothing in
//! the document changes, so there is no undo step, unless a pour it names
//! is unfilled (#2506): Repour first clears that flag as one undo step.
//! Unfill Selected and the Copper card's Unfill set it, one step too.

use std::collections::BTreeSet;
use std::path::Path;

use iced::Task;
use pcb_geom::{ItemRef, ZoneFills};
use serde_json::{Value, json};
use signex_engine::pcb::pour_edit;
use signex_engine::pcb::primitive_edit::{ZoneEdit, zone_edit_command};
use signex_i18n::{ids, t};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::{Message, Signex};

/// What each pour of `board` (each in `only`, when given) holds under
/// `fills`, in board order: its identity, net, layer and priority, whether
/// it is shelved (#2523) or unfilled (#2506; either way not filled), the
/// pieces and area of its copper, and the islands its rules dropped.
pub(crate) fn pour_report(
    board: &PcbBoard,
    fills: &ZoneFills,
    only: Option<&BTreeSet<Uuid>>,
) -> Vec<Value> {
    board
        .zones
        .iter()
        .enumerate()
        .filter(|(_, zone)| zone.is_pour())
        .filter(|(_, zone)| only.is_none_or(|wanted| wanted.contains(&zone.uuid)))
        .map(|(index, zone)| {
            let fill = fills.of(index).filter(|f| f.zone == zone.uuid);
            json!({
                "zone": zone.uuid.to_string(),
                "net": zone.net,
                "net_name": zone.net_name,
                "layer": zone.layer,
                "priority": zone.priority,
                "shelved": zone.shelved,
                "unfilled": zone.is_unfilled_pour(),
                "filled": fill.is_some(),
                "pieces": fill.map_or(0, |f| f.pieces.len()),
                "area_mm2": fill.map_or(0.0, |f| f.area_mm2()),
                "islands_removed": fill.map_or(0, |f| f.islands_removed),
            })
        })
        .collect()
}

/// The sum of `key` over the pours of a report.
fn total(report: &[Value], key: &str) -> u64 {
    report.iter().filter_map(|pour| pour[key].as_u64()).sum()
}

impl Signex {
    /// Repour the board in view and say in Messages what its pours, or the
    /// selected ones (`selected`), came to.
    pub(crate) fn repour_board_in_view(&mut self, selected: bool) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        self.repour_board(&path, selected);
    }

    /// Repour the board at `path` (the board in view is drawn again) and
    /// say in Messages what its pours, or the selected ones, came to.
    pub(crate) fn repour_board(&mut self, path: &Path, selected: bool) {
        let only = selected.then(|| self.selected_pours(path));
        self.fill_unfilled_pours(path, only.as_ref());
        self.refill_pours();
        self.report_pours(path, only.as_ref());
    }

    /// Repour the board at `path` and say in Messages what the pours in
    /// `only` came to. Repour on a pour's Copper card names its pours, so
    /// it holds for a board the canvas selection is not on.
    pub(crate) fn repour_pours(&mut self, path: &Path, only: &BTreeSet<Uuid>) {
        self.fill_unfilled_pours(path, Some(only));
        self.refill_pours();
        self.report_pours(path, Some(only));
    }

    /// Clear `unfilled` (#2506) on the unfilled pours in `only` (every pour
    /// for `None`) of the board at `path` as one undo step. A board with
    /// none is left alone, so a Repour of filled pours stays a refresh.
    pub(crate) fn fill_unfilled_pours(&mut self, path: &Path, only: Option<&BTreeSet<Uuid>>) {
        let zones: Option<Vec<Uuid>> = only.map(|set| set.iter().copied().collect());
        let zones = zones.as_deref();
        let owed = self
            .document_state
            .pcb_board(path)
            .is_some_and(|board| pour_edit::refill_pours(board, zones).is_some());
        if !owed {
            return;
        }
        self.run_guarded_board_step(path, ids::PCB_PRIM_POUR_REPOUR, |board| {
            Ok(pour_edit::refill_pours(board, zones).into_iter().collect())
        });
    }

    /// `unfill_selected` (#2506): the selected pours of the board in view
    /// hold no copper until the next Repour.
    pub(crate) fn unfill_selected_in_view(&mut self) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return;
        };
        let zones: Vec<Uuid> = self.selected_pours(&path).into_iter().collect();
        self.unfill_pours(&path, &zones);
    }

    /// Unfill the pours `zones` of the board at `path` as one undo step: each
    /// keeps its outline and holds no copper until the next Repour. With no
    /// pour, Messages says to select one.
    pub(crate) fn unfill_pours(&mut self, path: &Path, zones: &[Uuid]) {
        if zones.is_empty() {
            let line = t!(self.ui_state.i18n, ids::PCB_POUR_UNFILL_NONE_SELECTED).into_string();
            crate::diagnostics::log_warning(line);
            return;
        }
        self.run_guarded_board_step(path, ids::PCB_PRIM_POUR_UNFILL, |board| {
            Ok(zone_edit_command(board, zones, ZoneEdit::Unfilled(true))?
                .into_iter()
                .collect())
        });
    }

    /// Drop the computed fills; the board in view is filled and drawn again.
    fn refill_pours(&mut self) {
        self.interaction_state.pcb_ratsnest.invalidate();
        self.sync_pcb_canvas_from_visible_board();
    }

    /// Say in Messages what the pours of the board at `path` (those in
    /// `only`, when given) came to.
    fn report_pours(&self, path: &Path, only: Option<&BTreeSet<Uuid>>) {
        let i18n = &self.ui_state.i18n;
        if only.is_some_and(BTreeSet::is_empty) {
            let line = t!(i18n, ids::PCB_POUR_NONE_SELECTED).into_string();
            crate::diagnostics::log_warning(line);
            return;
        }
        let Some(board) = self.document_state.pcb_board(path) else {
            return;
        };
        // A board with no revision is not cached: fill it here.
        let computed;
        let fills = match self.current_pour_fills(path) {
            Some(fills) => fills,
            None => {
                computed = pcb_geom::zone_fills(board);
                &computed
            }
        };
        let report = pour_report(board, fills, only);
        if report.is_empty() {
            crate::diagnostics::log_info(t!(i18n, ids::PCB_POUR_NONE).into_string());
            return;
        }
        let line = t!(
            i18n,
            ids::PCB_POUR_REPOURED,
            "pours" => report.len(),
            "pieces" => total(&report, "pieces"),
            "islands" => total(&report, "islands_removed")
        );
        crate::diagnostics::log_info(line.into_string());
    }

    /// The pour fills the app holds for the board at `path`, when they are
    /// of its current revision.
    pub(crate) fn current_pour_fills(&self, path: &Path) -> Option<&ZoneFills> {
        let revision = *self.document_state.pcb_revisions.get(path)?;
        self.interaction_state
            .pcb_ratsnest
            .fills_for(&(path.to_path_buf(), revision))
    }

    /// The pours selected on the board at `path`.
    pub(crate) fn selected_pours(&self, path: &Path) -> BTreeSet<Uuid> {
        let selection = &self.interaction_state.pcb_selection;
        if selection.document() != Some(path) {
            return BTreeSet::new();
        }
        let Some(board) = self.document_state.pcb_board(path) else {
            return BTreeSet::new();
        };
        selection
            .selection()
            .items()
            .filter_map(|item| match item {
                ItemRef::Zone { zone, .. } => Some(zone),
                _ => None,
            })
            .filter(|id| board.zones.iter().any(|z| z.uuid == *id && z.is_pour()))
            .collect()
    }
}

/// `repour_all document=<board>`: the named open board's pours filled
/// again, answered as JSON (`document`, `pours`). The board in view is
/// refreshed too when it is that board.
pub(crate) fn repour_all(app: &mut Signex, args: &CommandArgs) -> (CommandOutcome, Task<Message>) {
    let refused = |reason: String| (CommandOutcome::disabled(reason), Task::none());
    let Some(OperandValue::Document(named)) = args.operand("document") else {
        return refused("`repour_all` needs the operand `document`".to_string());
    };
    let entry = match app.resolve_document_target(named) {
        Ok(entry) => entry,
        Err(reason) => return refused(reason),
    };
    if entry.kind != DocumentKind::Pcb {
        let kind = entry.kind.as_str();
        return refused(format!("{} is a {kind}, not a board", entry.name));
    }
    let Some(path) = entry.path else {
        return refused(format!("{} has no file", entry.name));
    };
    // #2506: every unfilled pour is filled again, as one undo step.
    app.fill_unfilled_pours(&path, None);
    if app.active_tab_path().as_deref() == Some(path.as_path()) {
        app.interaction_state.pcb_ratsnest.invalidate();
        app.sync_pcb_canvas_from_visible_board();
    }
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused(format!("{} is not open: open the board first", entry.name));
    };
    let fills = pcb_geom::zone_fills(board);
    let pours = pour_report(board, &fills, None);
    let answer = json!({
        "document": path.display().to_string(),
        "pours": pours,
    });
    (CommandOutcome::Answered(answer), Task::none())
}
