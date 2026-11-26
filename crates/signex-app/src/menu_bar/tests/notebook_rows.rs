//! What the Design Notebook rows do (mockup rows 195, 197, 201 and 202):
//! Project ▸ Design Notebook, File ▸ Export ▸ Design Notebook PDF… and
//! View ▸ Notebook Badges.

use super::*;

const PROJECT_ROWS: [&str; 3] = [
    "Project/Design Notebook/Open Design Notebook",
    "Project/Design Notebook/Generate from Project…",
    "File/Export/Design Notebook PDF…",
];

/// How many lines a click said in Messages on this thread. Other tests log
/// the same notebook path at once, so the shared ring is not counted.
fn said_by_click(app: &mut Signex, path: &str) -> u64 {
    let before = crate::diagnostics::said_on_this_thread();
    let _ = click(app, path);
    crate::diagnostics::said_on_this_thread() - before
}

#[test]
fn the_project_notebook_rows_are_greyed_without_a_project() {
    let app = bare_app();
    for path in PROJECT_ROWS {
        assert!(!row_is_live(&app, path), "{path}");
    }
}

#[test]
fn the_project_notebook_rows_act_on_the_open_project() {
    for path in PROJECT_ROWS {
        let mut app = bare_app();
        open_project(&mut app, "P.snxprj");
        assert!(row_is_live(&app, path), "{path}");
        assert!(said_by_click(&mut app, path) >= 1, "{path} said nothing");
    }
}

#[test]
fn notebook_badges_need_a_sheet_in_view() {
    assert!(!row_is_live(&bare_app(), "View/Notebook Badges"));
    let mut app = one_symbol_selected();
    assert!(row_is_live(&app, "View/Notebook Badges"));
    assert!(said_by_click(&mut app, "View/Notebook Badges") >= 1);
}
