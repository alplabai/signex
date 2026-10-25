//! The load-time Messages note for per-placement designators (#829).

use crate::app::Signex;

/// How many designators the note names before it elides the rest.
const NAMED_LIMIT: usize = 6;

/// The one-line note, reason first, or `None` when no symbol takes its
/// designator from a sheet-placement entry. `designators` has one item per
/// symbol per placement; the count is of those, the names are de-duplicated.
pub(super) fn instance_designator_note(designators: &[String]) -> Option<String> {
    if designators.is_empty() {
        return None;
    }
    let mut names: Vec<&str> = Vec::new();
    for designator in designators {
        if !names.contains(&designator.as_str()) {
            names.push(designator);
        }
    }
    let shown = names.iter().take(NAMED_LIMIT).copied().collect::<Vec<_>>();
    let more = if names.len() > NAMED_LIMIT { "…" } else { "" };
    let count = designators.len();
    let noun = if count == 1 {
        "symbol placement takes its"
    } else {
        "symbol placements take their"
    };
    Some(format!(
        "{count} {noun} designator from a sheet-placement entry: {}{more}",
        shown.join(", ")
    ))
}

/// Everything the note's walk needs, owned, so it can run off the UI thread.
pub(crate) struct NoteScope {
    pages: Vec<std::path::PathBuf>,
    root_path: std::path::PathBuf,
    base_dir: Option<std::path::PathBuf>,
    /// The project's case/separator rules, as the export uses them.
    fold: crate::app::state::scope::CaseFold,
    /// Snapshots of the open documents, so unsaved edits count.
    open: crate::app::project_sheets::OpenSheets,
}

impl NoteScope {
    /// The note for this scope, or `None` when no symbol takes a different
    /// designator from a placement entry. Walks the same roots the netlist
    /// export does (the root plus declared pages outside the hierarchy).
    /// Reads sheets that are not open from disk, so call it off the UI thread.
    pub(crate) fn note(&self) -> Option<String> {
        use crate::app::project_sheets as ps;
        let set =
            ps::assemble_project_sheets_from(&self.open, &self.pages, &self.root_path, self.fold);
        let base = self.base_dir.as_deref();
        let graph = ps::project_graph_with(&set.sheets, base, Some(&self.root_path), self.fold);
        let root_key = ps::sheet_key_with(&self.root_path, base, self.fold);
        if !graph.sheets.contains_key(&root_key) {
            return None;
        }
        let roots = ps::project_roots(root_key, &set, &graph, base, self.fold);
        let designators = signex_net::instance_entry_designators(&signex_net::ProjectGraph {
            sheets: &graph.sheets,
            resolved: &graph.resolved,
            roots: &roots,
        });
        instance_designator_note(&designators)
    }
}

impl Signex {
    /// Start the once-per-project note: a `Task` that computes it off the UI
    /// thread and hands the text back as `FileMsg::InstanceNoteReady`.
    pub(crate) fn begin_instance_note(&mut self) -> iced::Task<crate::app::Message> {
        let Some(scope) = self.take_instance_note_scope() else {
            return iced::Task::none();
        };
        iced::Task::perform(
            async move {
                tokio::task::spawn_blocking(move || scope.note())
                    .await
                    .unwrap_or_else(|error| {
                        crate::diagnostics::log_warning(format!(
                            "Sheet-placement designator note failed: {error}"
                        ));
                        None
                    })
            },
            |note| crate::app::Message::File(crate::app::FileMsg::InstanceNoteReady(note)),
        )
    }

    /// The scope to compute the note over, the first time it is asked for the
    /// active document's project (or, for a loose document, its root sheet);
    /// `None` afterwards. The once-guard is set here, before the walk, so a
    /// walk that finds nothing is not retried on every opened sheet.
    pub(crate) fn take_instance_note_scope(&mut self) -> Option<NoteScope> {
        let root_path = crate::app::project_sheets::resolve_netlist_root(&self.document_state)?;
        let (guard_key, pages, base_dir, fold) = {
            let project = self.document_state.active_document_project();
            (
                project.map_or_else(|| root_path.clone(), |p| p.path.clone()),
                project
                    .map(|p| {
                        p.data
                            .sheets
                            .iter()
                            .map(|s| p.dir().join(&s.filename))
                            .collect()
                    })
                    .unwrap_or_default(),
                project
                    .map(|p| p.dir().to_path_buf())
                    .or_else(|| root_path.parent().map(std::path::PathBuf::from)),
                crate::app::state::project_case::fold_of(project),
            )
        };
        if !self.document_state.instance_note_scopes.insert(guard_key) {
            return None;
        }
        let open = crate::app::project_sheets::open_sheets_snapshot(&self.document_state);
        Some(NoteScope {
            pages,
            root_path,
            base_dir,
            fold,
            open,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_entries_no_note() {
        assert_eq!(instance_designator_note(&[]), None);
    }

    #[test]
    fn note_leads_with_the_count_and_names_the_designators() {
        let note = instance_designator_note(&names(&["R5", "R6", "C2"])).unwrap();
        assert_eq!(
            note,
            "3 symbol placements take their designator from a sheet-placement entry: R5, R6, C2"
        );
    }

    #[test]
    fn a_long_list_is_elided_and_one_symbol_is_singular() {
        let many = names(&["R1", "R2", "R3", "R4", "R5", "R6", "R7"]);
        assert!(instance_designator_note(&many).unwrap().ends_with("R6…"));
        assert!(
            instance_designator_note(&names(&["R1"]))
                .unwrap()
                .starts_with("1 symbol placement takes its")
        );
    }
}
