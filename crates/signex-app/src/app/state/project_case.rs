//! Per-project path-key rules, probed once per loaded project (#694).
//!
//! The probe is disk IO, so it runs on a worker thread started where the
//! project is loaded and reports back with `ProjectMsg::CaseProbed`; `update`
//! only stores the answer on `LoadedProject::case_fold`. Until it arrives (and
//! when it cannot decide) the project keys paths by the host default.
//!
//! One fold per project: whoever picks the project reads `case_fold` once and
//! passes it down; nothing downstream re-derives it from a path.

use std::path::{Path, PathBuf};

use iced::Task;

use super::scope::CaseFold;
use super::volume_case::probe_directory_case;
use super::{DocumentState, LoadedProject, ProjectId};
use crate::app::{Message, ProjectMsg};

/// What the off-thread volume check reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseProbeOutcome {
    /// The volume answered: `true` when it ignores case.
    Decided(bool),
    /// The probe ran but could not decide (empty, read-only or unreadable
    /// volume).
    Undecided,
    /// The worker itself failed; carries the reason.
    Failed(String),
}

/// Probe `dir`'s volume off the update thread.
pub(crate) fn spawn_case_probe(project_id: ProjectId, dir: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            match tokio::task::spawn_blocking(move || probe_directory_case(&dir)).await {
                Ok(Some(ignores_case)) => CaseProbeOutcome::Decided(ignores_case),
                Ok(None) => CaseProbeOutcome::Undecided,
                Err(error) => CaseProbeOutcome::Failed(error.to_string()),
            }
        },
        move |outcome| {
            Message::Project(ProjectMsg::CaseProbed {
                project_id,
                outcome,
            })
        },
    )
}

/// The Messages-panel line for a project whose volume could not be probed:
/// the reason first, the path last, since the panel truncates near 157 bytes.
pub(crate) fn undecided_message(
    outcome: &CaseProbeOutcome,
    project_dir: &Path,
    assumed: CaseFold,
) -> String {
    let assumed = if assumed.fold_case {
        "case-insensitive"
    } else {
        "case-sensitive"
    };
    let reason = match outcome {
        CaseProbeOutcome::Failed(error) => format!("Volume check failed ({error})"),
        _ => "Could not tell if this volume ignores case".to_string(),
    };
    format!("{reason}; assuming {assumed}: {}", project_dir.display())
}

/// The key rules of `project`, or the host default for no project (a loose
/// document). The one place a caller that already picked its project reads
/// the fold.
pub(crate) fn fold_of(project: Option<&LoadedProject>) -> CaseFold {
    project.map_or_else(CaseFold::host_default, |p| p.case_fold)
}

impl DocumentState {
    /// Store a probe answer on project `id`; true when it CHANGED the fold, so
    /// the caller knows cached netlist and navigator state is stale.
    ///
    /// The first answer for a project wins, which is also what makes the
    /// fallback warning fire at most once per project. An undecided probe
    /// keeps the host default and says so in the Messages panel. A project
    /// closed while the worker ran is ignored.
    pub(crate) fn apply_case_probe(&mut self, id: ProjectId, outcome: CaseProbeOutcome) -> bool {
        if !self.probed_projects.insert(id) {
            return false;
        }
        let Some(project) = self.projects.iter_mut().find(|p| p.id == id) else {
            return false;
        };
        let before = project.case_fold;
        match outcome {
            CaseProbeOutcome::Decided(ignores) => project.case_fold = CaseFold::from_probe(ignores),
            undecided => {
                project.case_fold = CaseFold::host_default();
                crate::diagnostics::log_warning(undecided_message(
                    &undecided,
                    project.dir(),
                    project.case_fold,
                ));
            }
        }
        project.case_fold != before
    }

    /// The key rules for project `id`; the host default when it is not loaded.
    pub(crate) fn fold_for_project(&self, id: ProjectId) -> CaseFold {
        fold_of(self.project_by_id(id))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use signex_types::project::{ProjectData, SheetEntry};

    use super::*;
    use crate::app::state::LoadedProject;
    use crate::app::state::scope::project_owning_sheet;

    fn state_with_project(dir: &str, listed: &str) -> (DocumentState, ProjectId) {
        let (mut app, _task) = crate::app::Signex::new();
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(LoadedProject {
            id,
            path: PathBuf::from(dir).join("P.snxprj"),
            data: ProjectData {
                name: "P".to_string(),
                dir: dir.to_string(),
                schematic_root: Some(listed.to_string()),
                pcb_file: None,
                sheets: vec![SheetEntry {
                    name: listed.to_string(),
                    filename: listed.to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                }],
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                unknown: Default::default(),
            },
            pending_libraries: HashMap::new(),
            case_fold: CaseFold::default(),
        });
        (app.document_state, id)
    }

    fn warnings_naming(marker: &str) -> usize {
        let _ = crate::diagnostics::init_logging();
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|e| e.message.contains(marker))
            .count()
    }

    #[test]
    fn an_undecidable_probe_falls_back_to_the_host_default_and_warns_once() {
        let dir = "/w/undecidable-694-fallback";
        let _ = crate::diagnostics::init_logging();
        let (mut ds, id) = state_with_project(dir, "Top.snxsch");
        ds.projects[0].case_fold = CaseFold::from_probe(!CaseFold::host_default().fold_case);

        let changed = ds.apply_case_probe(id, CaseProbeOutcome::Undecided);
        // A repeated answer for the same project is ignored, warning included.
        let again = ds.apply_case_probe(id, CaseProbeOutcome::Undecided);

        assert!(
            changed,
            "the stand-in fold was replaced by the host default"
        );
        assert!(!again);
        assert_eq!(ds.fold_for_project(id), CaseFold::host_default());
        assert_eq!(
            warnings_naming(dir),
            1,
            "exactly one warning for the project"
        );
        let message = undecided_message(
            &CaseProbeOutcome::Undecided,
            Path::new(dir),
            CaseFold::host_default(),
        );
        assert!(
            message.find("Could not tell").unwrap() < message.find(dir).unwrap(),
            "reason first, path last: {message}"
        );
    }

    #[test]
    fn a_failed_worker_says_so_instead_of_claiming_it_could_not_tell() {
        let message = undecided_message(
            &CaseProbeOutcome::Failed("task 7 panicked".to_string()),
            Path::new("/w/p"),
            CaseFold::host_default(),
        );

        assert!(
            message.starts_with("Volume check failed (task 7 panicked)"),
            "{message}"
        );
        assert!(!message.contains("Could not tell"), "{message}");
    }

    #[test]
    fn a_decided_probe_is_stored_once_without_a_warning() {
        let dir = "/w/decided-694-no-warning";
        let _ = crate::diagnostics::init_logging();
        let (mut ds, id) = state_with_project(dir, "Top.snxsch");

        ds.apply_case_probe(id, CaseProbeOutcome::Decided(true));
        let second = ds.apply_case_probe(id, CaseProbeOutcome::Decided(false));

        assert!(!second, "the first answer wins");
        assert!(ds.fold_for_project(id).fold_case);
        assert_eq!(warnings_naming(dir), 0);
    }

    #[test]
    fn a_probe_for_an_unknown_project_is_ignored() {
        let (mut ds, _id) = state_with_project("/w/known-694", "Top.snxsch");
        let before = ds.projects[0].case_fold;

        assert!(!ds.apply_case_probe(ProjectId(9999), CaseProbeOutcome::Decided(true)));
        assert_eq!(ds.projects[0].case_fold, before);
    }

    #[test]
    fn ownership_follows_the_projects_probed_case_rule() {
        let (mut ds, _id) = state_with_project("/w/own-694", "Top.snxsch");
        let id = ds.projects[0].id;
        let spelled_differently = PathBuf::from("/w/own-694/TOP.snxsch");

        ds.projects[0].case_fold = CaseFold::from_probe(true);
        let insensitive =
            project_owning_sheet(&ds.projects, &HashMap::new(), &spelled_differently).map(|p| p.id);
        ds.projects[0].case_fold = CaseFold::from_probe(false);
        let sensitive =
            project_owning_sheet(&ds.projects, &HashMap::new(), &spelled_differently).map(|p| p.id);

        assert_eq!(
            insensitive,
            Some(id),
            "one file on a case-insensitive volume"
        );
        assert_eq!(sensitive, None, "a different file on a case-sensitive one");
    }

    #[test]
    fn a_direct_listing_beats_a_hierarchical_claim_across_different_folds() {
        // P1 (case-insensitive) lists `a.snxsch`, which references `s.snxsch`.
        // P2 (case-sensitive) lists `s.snxsch` itself. P2 owns it.
        let (mut ds, p1) = state_with_project("/w/p1-694", "a.snxsch");
        let (other, _p2) = state_with_project("/w/p2-694", "s.snxsch");
        let mut second = other.projects[0].clone();
        second.id = ds.mint_project_id();
        second.case_fold = CaseFold::from_probe(false);
        ds.projects[0].case_fold = CaseFold::from_probe(true);
        ds.projects.push(second);
        let refs = HashMap::from([(
            PathBuf::from("/w/p1-694/a.snxsch"),
            vec!["../p2-694/s.snxsch".to_string()],
        )]);

        let owner = project_owning_sheet(&ds.projects, &refs, Path::new("/w/p2-694/s.snxsch"));

        let _ = p1;
        assert_eq!(
            owner.map(|p| p.dir().to_path_buf()),
            Some(PathBuf::from("/w/p2-694"))
        );
    }
}
