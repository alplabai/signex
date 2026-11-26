//! What the dialog and the command-line forms share before and after the
//! worker's reads: the request taken on the update thread, and the report
//! built from the reads once they land (the shared-sheet check needs the
//! open projects).

use std::path::{Path, PathBuf};

use signex_engine::pcb::eco::{CompareReport, Compared, compare};
use signex_i18n::{ids, t};

use super::read::{CompareRead, CompareRequest};
use crate::app::Signex;
use crate::app::back_annotate::read::BackRequest;
use crate::app::project_sheets::{plan_project_sheets_for, project_root_sheet_path};
use crate::app::state::ProjectId;
use crate::app::update_pcb::design::DesignRequest;

impl Signex {
    /// Everything the worker reads to compare the open board at `board`
    /// with the schematic of project `project`, or why it cannot.
    pub(crate) fn compare_request(
        &self,
        project: ProjectId,
        board: &Path,
    ) -> Result<CompareRequest, String> {
        let i18n = &self.ui_state.i18n;
        let Some(project) = self.document_state.project_by_id(project) else {
            return Err(t!(i18n, ids::COMPARE_BOARD_PROJECT_CLOSED).into_string());
        };
        let Some(root) = project_root_sheet_path(project) else {
            return Err(t!(i18n, ids::COMPARE_BOARD_NO_SCHEMATIC).into_string());
        };
        let project_dir = project.dir().to_path_buf();
        let Some(engine) = self.document_state.board_engine(board) else {
            return Err(self.compare_board_missing(board));
        };
        let (board, revision) = (engine.board().clone(), engine.revision());
        let libraries = self.library_rows();
        let open_revisions: Vec<(PathBuf, u64)> = self
            .document_state
            .engines
            .iter()
            .map(|(path, engine)| (path.clone(), engine.revision()))
            .collect();
        let naming = crate::app::saved_channel_naming::SavedChannelNaming::of(&project.data);
        Ok(CompareRequest {
            design: DesignRequest {
                naming: naming.clone(),
                plan: plan_project_sheets_for(&self.document_state, &root),
                project_dir: project_dir.clone(),
                board: board.clone(),
                revision,
                libraries: libraries.clone(),
                open_revisions: open_revisions.clone(),
                previous: None,
            },
            back: BackRequest {
                naming,
                plan: plan_project_sheets_for(&self.document_state, &root),
                project_dir,
                board,
                revision,
                libraries,
                open_revisions,
                previous: None,
            },
        })
    }

    /// The report of both reads for project `owner`, with the default
    /// directions, before its net rows are computed
    /// ([`CompareReport::refresh_nets`]).
    pub(crate) fn compare_report_of(&self, owner: ProjectId, read: &CompareRead) -> CompareReport {
        let mut back = read.back.list.clone();
        self.block_values_shared_elsewhere(owner, &read.back, &mut back);
        let mut report = compare(&read.pcb.list, &back);
        let snapshot = &read.pcb.snapshot;
        report.compared = Compared {
            parts: snapshot.parts.len(),
            nets: snapshot.netlist.nets.len(),
            footprints: snapshot.board.footprints.len(),
        };
        report
    }

    /// The report no longer describes `board` or the design: the board
    /// moved past the report's revision, or a sheet either read used was
    /// edited, closed or opened since.
    pub(crate) fn compare_read_stale(
        &self,
        board: &Path,
        report: &CompareReport,
        read: &CompareRead,
    ) -> bool {
        let board_moved = self
            .document_state
            .board_engine(board)
            .is_none_or(|engine| engine.revision() != report.revision());
        let edited = |revisions: &[(PathBuf, Option<u64>)]| {
            revisions.iter().any(|(path, then)| {
                let now = self.document_state.engines.get(path).map(|e| e.revision());
                now != *then
            })
        };
        board_moved
            || edited(read.pcb.snapshot.sheet_revisions.as_slice())
            || edited(read.back.sheet_revisions.as_slice())
    }
}
