//! The project's Design Notebook: which notebook a request means, the
//! commands that reach it, and the Projects tree's notebook row.

use std::path::PathBuf;

use signex_widgets::tree_view::{TreeKey, TreeMsg, TreeNode};

use super::{NotebookMsg, NotebookTarget, has_notebook, is_notebook_row, notebook_file};
use crate::app::command::{CommandArgs, bridge};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectMsg, Signex};
use crate::keymap::AppCommandId;

const DIR: &str = "/w/buck-3v3";

fn notebook_in(dir: &str) -> PathBuf {
    PathBuf::from(dir).join("notebook").join("design.snxnbk")
}

/// How many lines a step said in Messages.
fn said_by(step: impl FnOnce()) -> u64 {
    let before = crate::diagnostics::said_on_this_thread();
    step();
    crate::diagnostics::said_on_this_thread() - before
}

#[test]
fn a_named_project_means_its_notebook_file() {
    let app = app_workspace(DIR, &["top.snxsch"]);
    let id = app.document_state.projects[0].id;
    assert_eq!(
        app.notebook_target(Some(id)),
        Some(NotebookTarget {
            project: id,
            path: notebook_in(DIR),
        })
    );
}

#[test]
fn no_project_named_means_the_active_project() {
    let app = app_workspace(DIR, &["top.snxsch"]);
    let id = app.document_state.projects[0].id;
    let target = app.notebook_target(None).expect("the active project");
    assert_eq!(target.project, id);
    assert_eq!(target.path, notebook_file(&app.document_state.projects[0]));
}

#[test]
fn with_no_project_open_a_request_means_nothing_and_says_so() {
    let (mut app, _boot) = Signex::new();
    assert_eq!(app.notebook_target(None), None);
    let said = said_by(|| {
        let _ = app.dispatch_notebook_message(NotebookMsg::OpenRequested { project: None });
    });
    assert_eq!(said, 1);
}

/// Until the tab, the badges, the generator and the export land, each
/// request is said in Messages, so no row or key is silent.
#[test]
fn every_request_is_said_in_messages() {
    let mut app = app_workspace(DIR, &["top.snxsch"]);
    for msg in [
        NotebookMsg::OpenRequested { project: None },
        NotebookMsg::GenerateRequested { project: None },
        NotebookMsg::ExportPdfRequested { project: None },
        NotebookMsg::BadgesToggled,
    ] {
        let said = said_by(|| {
            let _ = app.dispatch_notebook_message(msg.clone());
        });
        assert_eq!(said, 1, "{msg:?}");
    }
}

/// `update` routes the project's notebook messages to this slice.
#[test]
fn a_notebook_message_is_a_project_message_routed_here() {
    let message: Message = NotebookMsg::BadgesToggled.into();
    assert!(matches!(
        message,
        Message::Project(ProjectMsg::Notebook(NotebookMsg::BadgesToggled))
    ));
    let mut app = app_workspace(DIR, &["top.snxsch"]);
    let said = said_by(|| {
        let _ = app.update(message);
    });
    assert!(said >= 1, "the request reached no handler");
}

/// Every notebook command reaches its request, for the active document's
/// project.
#[test]
fn every_notebook_command_reaches_its_request() {
    let cases = [
        (
            "open_design_notebook",
            NotebookMsg::OpenRequested { project: None },
        ),
        (
            "generate_design_notebook",
            NotebookMsg::GenerateRequested { project: None },
        ),
        (
            "export_notebook_pdf",
            NotebookMsg::ExportPdfRequested { project: None },
        ),
        ("toggle_notebook_badges", NotebookMsg::BadgesToggled),
    ];
    for (id, want) in cases {
        let command = AppCommandId::new(id).expect("a catalog id");
        let got = bridge::core_to_message(&command, &CommandArgs::none());
        assert!(
            matches!(&got, Some(Message::Project(ProjectMsg::Notebook(msg)))
                if format!("{msg:?}") == format!("{want:?}")),
            "{id}: {got:?}"
        );
    }
}

#[test]
fn add_new_opens_the_notebook_of_the_clicked_project_only() {
    let mut app = app_workspace(DIR, &["top.snxsch"]);
    let mut open_at = |path: &[usize]| {
        said_by(|| {
            let _ = app.open_design_notebook_at(path);
        })
    };
    assert_eq!(open_at(&[0]), 1);
    // A tree path that names no project names no notebook either.
    assert_eq!(open_at(&[7]), 0);
    assert_eq!(open_at(&[]), 0);
}

#[test]
fn the_tree_lists_the_notebook_once_the_probe_found_it() {
    let mut app = app_workspace(DIR, &["top.snxsch"]);
    let project = &app.document_state.projects[0];
    assert!(!has_notebook(project, &app.document_state.project_fs.facts));
    app.refresh_panel_ctx();
    assert_eq!(notebook_row(&app), None, "not probed yet: no row");

    app.document_state
        .project_fs
        .facts
        .set_exists(notebook_in(DIR), true);
    let project = &app.document_state.projects[0];
    assert!(has_notebook(project, &app.document_state.project_fs.facts));
    app.refresh_panel_ctx();
    let key = notebook_row(&app).expect("the notebook row");
    assert_eq!(key.last().map(String::as_str), Some(NOTEBOOK_ROW_KEY));
}

#[test]
fn a_double_click_on_the_notebook_row_opens_the_notebook() {
    // A folder name no other test logs, so the shared ring can be counted.
    const OWN: &str = "/w/nb-row-double-click";
    let mut app = app_workspace(OWN, &["top.snxsch"]);
    app.document_state
        .project_fs
        .facts
        .set_exists(notebook_in(OWN), true);
    app.refresh_panel_ctx();
    let key = notebook_row(&app).expect("the notebook row");
    let select = || {
        Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::Tree(TreeMsg::Select(key.clone())),
        ))
    };
    let opened =
        || crate::diagnostics::logged_count("Design Notebook: Open /w/nb-row-double-click");
    let _ = app.update(select());
    assert_eq!(opened(), 0, "one click opens nothing");
    let _ = app.update(select());
    assert_eq!(opened(), 1, "the double-click opens the project's notebook");
}

#[test]
fn the_notebook_row_key_is_the_notebook_path_in_the_project() {
    assert_eq!(
        NOTEBOOK_ROW_KEY,
        format!(
            "{}/{}",
            signex_notebook::NOTEBOOK_DIR,
            signex_notebook::DEFAULT_NOTEBOOK_FILE
        )
    );
}

#[test]
fn a_notebook_row_is_a_snxnbk_file() {
    assert!(is_notebook_row("design.snxnbk"));
    assert!(!is_notebook_row("design.snxsch"));
    assert!(!is_notebook_row("Source Documents"));
}

/// The palette finds the notebook by "notebook" and by "wiki", the old
/// panel's name (mockup row 60b).
#[test]
fn the_palette_finds_the_notebook_by_notebook_and_by_wiki() {
    let app = app_workspace(DIR, &["top.snxsch"]);
    let catalog = crate::app::command_palette::build_catalog(&app);
    for query in ["notebook", "wiki"] {
        let hits: Vec<&str> = crate::app::command_palette::rank_results(&catalog, query)
            .into_iter()
            .map(|(idx, _)| catalog[idx].label.as_str())
            .collect();
        assert!(
            hits.contains(&"Open the Design Notebook of the project (Wiki)"),
            "{query}: {hits:?}"
        );
    }
}

const NOTEBOOK_ROW_KEY: &str = crate::panels::NOTEBOOK_ROW_KEY;

/// The key of the tree row keyed as the notebook, if the tree has one.
fn notebook_row(app: &Signex) -> Option<TreeKey> {
    fn walk(nodes: &[TreeNode], prefix: &mut TreeKey) -> Option<TreeKey> {
        for node in nodes {
            prefix.push(node.key.clone());
            if node.key == NOTEBOOK_ROW_KEY {
                return Some(prefix.clone());
            }
            if let Some(found) = walk(&node.children, prefix) {
                return Some(found);
            }
            prefix.pop();
        }
        None
    }
    walk(&app.document_state.panel_ctx.project_tree, &mut Vec::new())
}
