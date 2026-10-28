//! A project's volume probe landing (#694).

use iced::Task;

use crate::app::state::ProjectId;
use crate::app::state::project_case::CaseProbeOutcome;
use crate::app::{Message, Signex};

impl Signex {
    /// Store the probe answer. When it CHANGES the project's key rules,
    /// everything derived under the old ones is stale: the net-class styling
    /// job re-stitches under the new fold (the fold is part of its gate and
    /// its `ProjectKey`), the Navigator hierarchy follows through its
    /// fingerprint (which includes the fold), and any ERC results on screen
    /// are flagged as predating the check.
    pub(crate) fn handle_case_probed(
        &mut self,
        project_id: ProjectId,
        outcome: CaseProbeOutcome,
    ) -> Task<Message> {
        if !self.document_state.apply_case_probe(project_id, outcome) {
            return Task::none();
        }
        // ERC runs on the active document's project; another project's probe
        // landing says nothing about those results.
        let erc_project_changed = self
            .document_state
            .active_document_project()
            .is_some_and(|p| p.id == project_id);
        let erc_on_screen = !self.ui_state.erc_violations_by_path.is_empty()
            || !self.ui_state.erc_violations.is_empty();
        if erc_project_changed && erc_on_screen {
            crate::diagnostics::log_info(
                "ERC results predate the volume check of this project; run ERC again.",
            );
        }
        self.refresh_panel_ctx();
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::app::ProjectMsg;
    use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
    use crate::app::state::scope::CaseFold;

    /// A root that references one child under two spellings, `Sub.snxsch`
    /// and `sub.snxsch`, both open: one file on a case-insensitive volume,
    /// two sheets on any other rule.
    fn app_with_two_spellings(fold: CaseFold) -> Signex {
        let mut app = app_workspace("/w/pc-694", &["Top.snxsch"]);
        app.document_state.projects[0].case_fold = fold;
        let top = PathBuf::from("/w/pc-694/Top.snxsch");
        let root = sheet_with_net("R0", "N0", &["Sub.snxsch", "sub.snxsch"]);
        open_with(&mut app.document_state, &top, root);
        let child = sheet_with_net("R1", "N1", &[]);
        for name in ["Sub.snxsch", "sub.snxsch"] {
            let path = PathBuf::from("/w/pc-694").join(name);
            open_with(&mut app.document_state, &path, child.clone());
        }
        app.document_state.active_path = Some(top);
        app
    }

    fn has_duplicate_uuid(app: &Signex) -> bool {
        crate::app::stitched_netlist::stitched_for_active_project(&app.document_state)
            .warnings
            .iter()
            .any(|w| w.contains("share a UUID"))
    }

    fn deliver(app: &mut Signex, project_id: ProjectId, outcome: CaseProbeOutcome) {
        let _ = app.update(Message::Project(ProjectMsg::CaseProbed {
            project_id,
            outcome,
        }));
    }

    fn navigator_fold(app: &Signex) -> Option<CaseFold> {
        app.document_state
            .panel_ctx
            .navigator
            .hierarchy
            .as_ref()
            .map(|h| h.fold)
    }

    #[test]
    fn a_probe_that_changes_the_fold_restitches_the_project_and_the_navigator() {
        let mut app = app_with_two_spellings(CaseFold::from_probe(false));
        let id = app.document_state.projects[0].id;
        app.refresh_panel_ctx();
        app.settle_navigator();
        assert!(
            has_duplicate_uuid(&app),
            "precondition: two spellings are two sheets under a case-sensitive rule"
        );
        assert_eq!(navigator_fold(&app), Some(CaseFold::from_probe(false)));

        deliver(&mut app, id, CaseProbeOutcome::Decided(true));

        assert!(
            !has_duplicate_uuid(&app),
            "stitched under the probed fold, one file is one sheet"
        );
        assert_eq!(
            navigator_fold(&app),
            Some(CaseFold::from_probe(true)),
            "the Navigator hierarchy follows the new fold"
        );
    }

    #[test]
    fn a_probe_for_a_closed_project_or_a_repeated_answer_changes_nothing() {
        let mut app = app_with_two_spellings(CaseFold::from_probe(false));
        let id = app.document_state.projects[0].id;
        let closed = app.document_state.mint_project_id();

        deliver(&mut app, closed, CaseProbeOutcome::Decided(true));
        assert_eq!(
            app.document_state.projects[0].case_fold,
            CaseFold::from_probe(false)
        );

        deliver(&mut app, id, CaseProbeOutcome::Decided(true));
        deliver(&mut app, id, CaseProbeOutcome::Decided(false));
        assert!(app.document_state.projects[0].case_fold.fold_case);
    }

    #[test]
    fn erc_results_on_screen_are_flagged_as_predating_the_volume_check() {
        let _ = crate::diagnostics::init_logging();
        let mut app = app_with_two_spellings(CaseFold::from_probe(false));
        let id = app.document_state.projects[0].id;
        app.ui_state
            .erc_violations_by_path
            .insert(PathBuf::from("/w/pc-694/Top.snxsch"), Vec::new());
        let count = || {
            crate::diagnostics::recent_entries()
                .iter()
                .filter(|e| e.message.contains("ERC results predate"))
                .count()
        };
        let before = count();

        deliver(&mut app, id, CaseProbeOutcome::Decided(true));

        assert_eq!(count(), before + 1);
    }
}
