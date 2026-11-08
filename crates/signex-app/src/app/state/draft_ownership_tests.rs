//! #1900 — which prompt owns a symbol / footprint draft: ties between
//! libraries in one folder, a project nested in another, and the case rule.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::project::{LibraryEntry, LibraryEntryKind};

use super::scope::{CaseFold, owning_libraries, owning_library};
use super::{LoadedProject, ProjectId};
use crate::app::{Signex, SymbolDocument};

fn project(id: u32, dir: &str, locals: &[&str]) -> LoadedProject {
    LoadedProject {
        id: ProjectId(id),
        path: Path::new(dir).join("p.snxprj"),
        data: signex_types::project::ProjectData {
            document_id: None,
            description: None,
            name: format!("p{id}"),
            dir: dir.to_string(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: locals
                .iter()
                .map(|path| LibraryEntry {
                    path: PathBuf::from(path),
                    kind: LibraryEntryKind::ProjectLocal,
                    library_id: None,
                    unknown: Default::default(),
                })
                .collect(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    }
}

fn with_dirty_draft(projects: Vec<LoadedProject>, draft: &str) -> Signex {
    let (mut app, _task) = Signex::new();
    let docs = &mut app.document_state;
    docs.projects = projects;
    let path = PathBuf::from(draft);
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("S"));
    docs.symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    docs.mark_document_dirty(&path);
    app
}

fn mine(
    app: &Signex,
    closing: usize,
    libraries: &[&str],
    folds: &HashMap<PathBuf, CaseFold>,
) -> Vec<PathBuf> {
    let docs = &app.document_state;
    let refs = docs.sheet_ref_inputs().resolve(|_| false);
    let libraries: Vec<PathBuf> = libraries.iter().map(PathBuf::from).collect();
    docs.close_scan(&refs, closing, &[], &libraries, folds).mine
}

#[test]
fn libraries_in_one_folder_tie_for_a_draft() {
    let libraries = vec![PathBuf::from("/d/a.snxlib"), PathBuf::from("/d/b.snxlib")];
    let draft = Path::new("/d/symbols/s.snxsym");
    let fold = CaseFold::from_probe(false);
    assert_eq!(owning_libraries(draft, &libraries, fold).len(), 2);
    // No single library claims a tied draft: Close Project keeps it.
    assert_eq!(owning_library(draft, &libraries, fold), None);
}

#[test]
fn a_deeper_library_wins_the_draft_alone() {
    let libraries = vec![PathBuf::from("/d/a.snxlib"), PathBuf::from("/d/v/v.snxlib")];
    let draft = Path::new("/d/v/symbols/s.snxsym");
    let owners = owning_libraries(draft, &libraries, CaseFold::from_probe(false));
    assert_eq!(owners, vec![&libraries[1]]);
}

#[test]
fn an_inner_project_claims_its_own_draft_inside_an_outer_project() {
    let outer = project(1, "nonexistent-1900/A", &[]);
    let inner = project(2, "nonexistent-1900/A/sub", &["lib/B.snxlib"]);
    let draft = "nonexistent-1900/A/sub/lib/symbols/x.snxsym";
    let app = with_dirty_draft(vec![outer, inner], draft);
    let libraries = ["nonexistent-1900/A/sub/lib/B.snxlib"];

    assert_eq!(
        mine(&app, 1, &libraries, &HashMap::new()),
        vec![PathBuf::from(draft)],
        "closing the inner project asks about its own draft"
    );
    assert!(
        mine(&app, 0, &libraries, &HashMap::new()).is_empty(),
        "closing the outer project leaves the inner project's draft alone"
    );
}

#[test]
fn a_tied_draft_is_not_the_projects_when_the_other_library_outlives_it() {
    let closing = project(1, "nonexistent-1900/P", &["lib/b.snxlib"]);
    let draft = "nonexistent-1900/P/lib/symbols/s.snxsym";
    let app = with_dirty_draft(vec![closing], draft);
    let libraries = [
        "nonexistent-1900/P/lib/a.snxlib",
        "nonexistent-1900/P/lib/b.snxlib",
    ];

    assert!(
        mine(&app, 0, &libraries, &HashMap::new()).is_empty(),
        "a.snxlib is not the project's and stays mounted, so the draft is kept"
    );
}

#[test]
fn the_close_project_prompt_decides_a_draft_by_the_close_library_case_rule() {
    let spelled = "nonexistent-1900/P/LIB/symbols/x.snxsym";
    let libraries = ["nonexistent-1900/P/lib/B.snxlib"];
    let mut closing = project(1, "nonexistent-1900/P", &["lib/B.snxlib"]);
    closing.case_fold = CaseFold::from_probe(false);
    let app = with_dirty_draft(vec![closing], spelled);
    let draft = PathBuf::from(spelled);

    let folds = HashMap::from([(draft.clone(), CaseFold::from_probe(true))]);
    assert_eq!(
        mine(&app, 0, &libraries, &folds),
        vec![draft.clone()],
        "the draft's folder ignores case, whatever the project's rule says"
    );
    let folds = HashMap::from([(draft, CaseFold::from_probe(false))]);
    assert!(mine(&app, 0, &libraries, &folds).is_empty());
}
