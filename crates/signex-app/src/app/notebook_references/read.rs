//! The worker half of the notebook read: the project's design index and
//! each notebook file's references resolved against it. Blocking; runs on
//! the notebook scan's worker.

use std::path::{Path, PathBuf};

use signex_notebook::{
    DesignIndex, NotebookFile as NotebookText, PlacedSheet, ReferenceReport, design_index,
    references, resolve_references,
};

use crate::app::design_read::DesignGraph;
use crate::app::project_sheets::ProjectSheetSet;
use crate::panels::annotations::NotebookFile;

/// One notebook file's references and where each stands.
#[derive(Debug, Clone, PartialEq)]
pub struct FileReferences {
    pub path: PathBuf,
    /// The path inside `notebook/` (`design.snxnbk`).
    pub name: String,
    pub report: ReferenceReport,
    /// The file's header could not be read: its references resolve by name
    /// only, so a renamed part reads as deleted.
    pub header_unread: bool,
}

/// The design index of `set`, the project in `project_dir` assembled. `None`
/// when the design cannot be stitched.
pub(crate) fn read_design(set: &ProjectSheetSet, project_dir: &Path) -> Option<DesignIndex> {
    let graph = DesignGraph::read(set, project_dir).ok()?;
    let netlist = signex_net::build_project_netlist(&graph.project()).netlist;
    let placed = graph.placed();
    let names: Vec<String> = placed.iter().map(|(_, _, path)| file_name(path)).collect();
    let sheets: Vec<PlacedSheet<'_>> = placed
        .iter()
        .zip(&names)
        .map(|((placement, sheet, _), file)| PlacedSheet {
            file,
            placement,
            sheet,
        })
        .collect();
    Some(design_index(&sheets, &netlist))
}

/// The references of every file read (`texts` beside `files`, `None` for a
/// file that could not be read) resolved against `design`.
pub(crate) fn file_references(
    files: &[NotebookFile],
    texts: &[Option<String>],
    design: &DesignIndex,
) -> Vec<FileReferences> {
    files
        .iter()
        .zip(texts)
        .filter_map(|(file, text)| Some(of_text(&file.path, text.as_deref()?, design)))
        .collect()
}

/// The references of one file's `text` resolved against `design`.
pub(crate) fn of_text(path: &Path, text: &str, design: &DesignIndex) -> FileReferences {
    let parsed = NotebookText::parse(text);
    let header_unread = parsed.is_err();
    let header = parsed.map(|file| file.header).unwrap_or_default();
    FileReferences {
        path: path.to_path_buf(),
        name: file_name(path),
        report: resolve_references(&references(text), &header, design),
        header_unread,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_notebook::{DesignPart, ReferenceState};

    fn design() -> DesignIndex {
        DesignIndex::new(
            vec![DesignPart {
                designator: "R17".into(),
                id: uuid::Uuid::from_u128(7),
                value: "100k".into(),
                description: String::new(),
                footprint: String::new(),
                sheet: "power.snxsch".into(),
                fitted_in: Vec::new(),
                variant_count: 0,
                pins: Vec::new(),
            }],
            Vec::new(),
        )
    }

    const RENAMED: &str = "+++\n[[references]]\nkind = \"component\"\ncomponent = \"R7\"\nid = \"00000000-0000-0000-0000-000000000007\"\n+++\n#component(\"R7\")\n";

    #[test]
    fn a_file_resolves_with_the_ids_its_header_stored() {
        let found = of_text(Path::new("/p/notebook/design.snxnbk"), RENAMED, &design());
        assert_eq!(found.name, "design.snxnbk");
        assert!(!found.header_unread);
        assert!(matches!(
            found.report.references[0].state,
            ReferenceState::Renamed { .. }
        ));
    }

    #[test]
    fn a_file_whose_header_does_not_read_resolves_by_name() {
        let found = of_text(
            Path::new("/p/notebook/x.snxnbk"),
            "+++\nnot = [\n",
            &design(),
        );
        assert!(found.header_unread);
        assert!(found.report.references.is_empty());
    }

    #[test]
    fn an_unread_file_has_no_references() {
        let file = NotebookFile {
            path: PathBuf::from("/p/notebook/a.snxnbk"),
            label: "notebook/a.snxnbk".into(),
            annotations: Vec::new(),
            problems: Vec::new(),
            read_error: None,
        };
        let found = file_references(
            &[file.clone(), file],
            &[None, Some("#component(\"Q1\")".into())],
            &design(),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].report.references[0].state, ReferenceState::Deleted);
    }
}
