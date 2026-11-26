//! `board_status document=<board>` (#2460): the numbers the board's status
//! bar shows, read from the same state (the connection counts cached with
//! the ratsnest for the board's revision, the last DRC run), with the parts
//! not placed yet, the waived DRC findings, the routed length and the board
//! against its schematic. The last is the report `check_board_against_schematic`
//! answers, so both agree; it is read off the UI thread, and the answer
//! comes when it lands. A board in no project answers at once with
//! `"schematic": null`. Shape: `docs/CLI.md`, "Board status".

use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_types::pcb::PcbBoard;

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::compare_board::JobMode;
use crate::app::document::DocumentKey;
use crate::app::project_sheets::project_root_sheet_path;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Answer the counters of the board `args` names; with a schematic to
    /// compare, once the comparison lands.
    pub(super) fn board_status_query(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let (path, status) = match self.board_status_now(args) {
            Ok(found) => found,
            Err(why) => {
                let reason =
                    signex_i18n::Translatable::new(signex_i18n::ids::BOARD_STATUS_COMMAND_REFUSED)
                        .arg("why", why);
                return (CommandOutcome::Disabled { reason }, Task::none());
            }
        };
        let project = self
            .document_state
            .board_project(&path, None)
            .filter(|project| project_root_sheet_path(project).is_some())
            .map(|project| project.id);
        let Some(project) = project else {
            let mut status = status;
            status["schematic"] = Value::Null;
            return (CommandOutcome::Answered(status), Task::none());
        };
        self.start_compare_job(command, args, (project, path), JobMode::Status(status))
    }

    /// The loaded board `args` names and its counters, the board against
    /// the schematic left for the comparison.
    fn board_status_now(&self, args: &CommandArgs) -> Result<(std::path::PathBuf, Value), String> {
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return Err("`board_status` needs the operand `document`".into());
        };
        let entry = self.resolve_document_target(target)?;
        let Some(DocumentKey::Pcb(path)) = &entry.key else {
            return Err(format!("{} is not an open board", entry.name));
        };
        let board = self
            .document_state
            .pcb_board(path)
            .ok_or_else(|| format!("{} is not loaded", entry.name))?;
        Ok((path.clone(), self.board_status_json(path, board)))
    }

    fn board_status_json(&self, path: &Path, board: &PcbBoard) -> Value {
        let revision = self.board_revision(path);
        // The status bar's counts when they were computed for this
        // revision; else the same functions run now.
        let counts = match self
            .interaction_state
            .pcb_ratsnest
            .counts_for(path, revision)
        {
            Some(counts) => counts.clone(),
            None => {
                let fills = pcb_geom::zone_fills(board);
                pcb_geom::connection_counts(board, &pcb_geom::ratsnest_with_fills(board, &fills))
            }
        };
        let unplaced = pcb_geom::unplaced_footprints(board).map(|ids| {
            let ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
            json!({ "count": ids.len(), "ids": ids })
        });
        let drc = match self.interaction_state.pcb_drc.done_for(path) {
            None => {
                json!({ "ran": false, "violations": 0, "waived": 0, "edits_since": Value::Null })
            }
            Some(done) => {
                let rows = self.pcb_drc_panel_for(path, board).rows;
                let waived = rows.iter().filter(|row| row.waiver.is_some()).count();
                json!({
                    "ran": true,
                    "violations": done.report.violations.len(),
                    "waived": waived,
                    "edits_since": done.edits_since(revision),
                })
            }
        };
        json!({
            "board": path.display().to_string(),
            "revision": revision,
            "parts": { "total": board.footprints.len(), "unplaced": unplaced },
            "connections": {
                "total": counts.total,
                "routed": counts.routed(),
                "unrouted": counts.unrouted,
            },
            "unrouted_nets": counts.per_net.len(),
            "drc": drc,
            "routed_length_mm": pcb_geom::total_routed_length_mm(board),
        })
    }
}
