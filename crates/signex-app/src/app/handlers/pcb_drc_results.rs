//! The DRC results panel's actions (#2399), one path for the panel, the
//! command palette, the CLI and the agent socket: show a violation (select
//! its objects, centre the view), filter the list, and waive or unwaive a
//! violation. A waiver is board data (`BoardOptions::drc_waivers`), set as
//! one `SetBoardOptions` undo step; the filter is view state.

use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::BoardCommand;
use signex_i18n::{ids, t};
use signex_types::format::pcb_layer_token;
use signex_types::pcb_layer::PcbLayer;
use signex_types::pcb_options::DrcWaiver;

use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};
use crate::panels::{DrcFilter, DrcFilterChoice, DrcPanelMsg, DrcRow, DrcWaiveDraft};

impl Signex {
    /// A DRC results panel event, on the board in view.
    pub(crate) fn handle_drc_panel(&mut self, msg: DrcPanelMsg) -> Task<Message> {
        let Some(path) = self.active_tab_path() else {
            return Task::none();
        };
        let refused = match msg {
            DrcPanelMsg::RowPicked(key) => self.show_drc_violation(&path, &key).err(),
            DrcPanelMsg::FilterPicked(choice) => {
                let filter = &mut self.interaction_state.pcb_drc.filter;
                match choice {
                    DrcFilterChoice::Rule(rule) => filter.rule = rule,
                    DrcFilterChoice::Layer(layer) => filter.layer = layer,
                    DrcFilterChoice::Severity(severity) => filter.severity = severity,
                }
                None
            }
            DrcPanelMsg::WaivedShownToggled(on) => {
                self.interaction_state.pcb_drc.filter.show_waived = on;
                None
            }
            DrcPanelMsg::WaiveStarted(key) => {
                self.interaction_state.pcb_drc.waive_draft = Some(DrcWaiveDraft {
                    key,
                    note: String::new(),
                });
                None
            }
            DrcPanelMsg::NoteEdited(note) => {
                if let Some(draft) = self.interaction_state.pcb_drc.waive_draft.as_mut() {
                    draft.note = note;
                }
                None
            }
            DrcPanelMsg::WaiveConfirmed => {
                match self.interaction_state.pcb_drc.waive_draft.take() {
                    Some(draft) => self
                        .set_drc_waiver(&path, &draft.key, Some(draft.note))
                        .err(),
                    None => None,
                }
            }
            DrcPanelMsg::WaiveCancelled => {
                self.interaction_state.pcb_drc.waive_draft = None;
                None
            }
            DrcPanelMsg::Unwaived(key) => self.set_drc_waiver(&path, &key, None).err(),
        };
        if let Some(why) = refused {
            let line = t!(self.ui_state.i18n, ids::DRC_PANEL_REFUSED, "why" => why);
            crate::diagnostics::log_warning(line.into_string());
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Why `board` cannot be acted on from the DRC results: it must be the
    /// board in view, whose last run the panel lists.
    fn drc_results_board(&self, board: &Path) -> Result<(), String> {
        if self.active_tab_path().as_deref() == Some(board) && self.active_pcb().is_some() {
            Ok(())
        } else {
            Err(format!(
                "{} is not the board in view; the DRC results are the board in view's",
                board.display()
            ))
        }
    }

    /// Select the objects of the last run's violation `key` on `board` and
    /// centre the view on it. The answer names what was selected.
    pub(crate) fn show_drc_violation(&mut self, board: &Path, key: &str) -> Result<Value, String> {
        self.drc_results_board(board)?;
        let done = self
            .interaction_state
            .pcb_drc
            .done_for(board)
            .ok_or("the board has no DRC run yet; run `run_drc` first")?;
        let violation = done
            .report
            .violations
            .iter()
            .find(|v| signex_drc::waiver_key(v) == key)
            .ok_or_else(|| format!("the last DRC run has no violation `{key}`"))?;
        let items = violation.items.clone();
        let rule = signex_drc::rule_id(violation).to_string();
        let (x, y) = (
            pcb_geom::nm_to_mm(violation.at.x),
            pcb_geom::nm_to_mm(violation.at.y),
        );
        self.select_pcb_items(&items);
        let canvas = &mut self.interaction_state.pcb_canvas;
        // A queued fit would land afterwards and undo the centring.
        canvas.pending_fit.set(None);
        canvas.center_on_world(x as f32, y as f32);
        canvas.clear_bg_cache();
        canvas.clear_content_cache();
        Ok(json!({
            "board": board.display().to_string(),
            "id": key,
            "rule": rule,
            "items": items.iter().map(signex_drc::findings::item_id).collect::<Vec<_>>(),
            "at_mm": [x, y],
        }))
    }

    /// Waive the last run's violation `key` on `board` with `note`, or take
    /// its waiver back (`None`): one board undo step. The report stays
    /// current, since a waiver changes nothing the check reads.
    pub(crate) fn set_drc_waiver(
        &mut self,
        board: &Path,
        key: &str,
        note: Option<String>,
    ) -> Result<Value, String> {
        let rule = self
            .interaction_state
            .pcb_drc
            .done_for(board)
            .and_then(|done| {
                done.report
                    .violations
                    .iter()
                    .find(|v| signex_drc::waiver_key(v) == key)
            })
            .map(|v| signex_drc::rule_id(v).to_string());
        let engine = self
            .document_state
            .board_engine(board)
            .ok_or_else(|| format!("{} is not open", board.display()))?;
        let expected_revision = engine.revision();
        let mut options = engine.board().options.clone();
        let had = options.drc_waivers.len();
        options.drc_waivers.retain(|w| w.key != key);
        let note = note.map(|n| n.trim().to_string());
        match &note {
            Some(note) => {
                let rule = rule.ok_or_else(|| {
                    format!("the last DRC run of this board has no violation `{key}`")
                })?;
                options.drc_waivers.push(DrcWaiver {
                    key: key.to_string(),
                    rule,
                    note: note.clone(),
                });
            }
            None if options.drc_waivers.len() == had => {
                return Err(format!("violation `{key}` is not waived"));
            }
            None => {}
        }
        let label = match note {
            Some(_) => t!(self.ui_state.i18n, ids::DRC_PANEL_WAIVE_UNDO),
            None => t!(self.ui_state.i18n, ids::DRC_PANEL_UNWAIVE_UNDO),
        }
        .into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision,
        };
        let from = self.document_state.pcb_revisions.get(board).copied();
        let command = BoardCommand::SetBoardOptions {
            options: Box::new(options),
        };
        self.execute_board_commands(board, vec![command], Some(step))
            .map_err(|e| e.to_string())?;
        let to = self.document_state.pcb_revisions.get(board).copied();
        if let (Some(from), Some(to)) = (from, to) {
            self.interaction_state
                .pcb_drc
                .carry_revision(board, from, to);
        }
        self.refresh_panel_ctx();
        Ok(json!({
            "board": board.display().to_string(),
            "id": key,
            "waived": note.is_some(),
            "note": note,
        }))
    }

    /// Set what the DRC results show of the board in view and answer with
    /// the rows shown.
    pub(crate) fn filter_drc_results(
        &mut self,
        board: &Path,
        filter: DrcFilter,
    ) -> Result<Value, String> {
        self.drc_results_board(board)?;
        self.interaction_state.pcb_drc.filter = filter;
        self.refresh_panel_ctx();
        Ok(self.drc_results_json())
    }

    /// The board in view's DRC results as the panel shows them, filtered.
    pub(crate) fn drc_results_json(&self) -> Value {
        let Some(panel) = self.pcb_drc_panel() else {
            return json!({ "ran": false });
        };
        let rows: Vec<Value> = panel
            .rows
            .iter()
            .filter(|row| panel.filter.shows(row))
            .map(row_json)
            .collect();
        json!({
            "ran": panel.ran,
            "running": panel.running,
            "stale": panel.stale,
            "violations": panel.rows.len(),
            "waived": panel.rows.iter().filter(|r| r.waiver.is_some()).count(),
            "rows": rows,
        })
    }
}

/// One shown row: its key, rule, severity, nets, place, layers and waiver.
fn row_json(row: &DrcRow) -> Value {
    let layers: Vec<String> = row
        .layers
        .iter()
        .map(|layer| pcb_layer_token(PcbLayer::Copper(*layer)))
        .collect();
    json!({
        "id": row.key,
        "rule": row.rule,
        "severity": row.severity,
        "nets": [row.a, row.b],
        "measured_mm": row.measured_mm,
        "required_mm": row.required_mm,
        "at_mm": [row.x_mm, row.y_mm],
        "layers": layers,
        "objects": row.objects,
        "waiver": row.waiver,
    })
}
