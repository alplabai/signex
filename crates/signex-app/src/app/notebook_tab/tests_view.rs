//! The notebook tab's view, laid out (mockup row 195): the empty state, the
//! toolbar, the side panel, the editor with its Problems strip, the preview
//! and the footer, in each view, and what a press on them sends.

use iced::Size;
use signex_notebook::Template;

use super::tests::{NOTEBOOK, app_in, app_with_notebook, open, send};
use super::view::{Mark, marks};
use super::{NotebookMsg, SidePanel, ViewMode};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::layout_probe::{lay_out_texts, messages_on_click};

/// A notebook whose line 5 does not compile.
fn broken() -> String {
    NOTEBOOK.replace("= Power stage", "#(1 + \n= Power stage")
}

fn window() -> Size {
    Size::new(1600.0, 1000.0)
}

fn shown(app: &Signex, id: ProjectId) -> Vec<String> {
    lay_out_texts(app.view_notebook_tab(id), window())
        .into_iter()
        .map(|t| t.text)
        .collect()
}

fn has(texts: &[String], part: &str) -> bool {
    texts.iter().any(|text| text.contains(part))
}

fn pick_view(app: &mut Signex, id: ProjectId, view: ViewMode) {
    send(
        app,
        NotebookMsg::ViewPicked {
            project_id: id,
            view,
        },
    );
}

fn pick_panel(app: &mut Signex, id: ProjectId, panel: SidePanel) {
    send(
        app,
        NotebookMsg::PanelPicked {
            project_id: id,
            panel,
        },
    );
}

fn clicked(app: &Signex, id: ProjectId, label: &str) -> Vec<NotebookMsg> {
    messages_on_click(app.view_notebook_tab(id), window(), label)
        .into_iter()
        .filter_map(|message| match message {
            Message::Project(ProjectMsg::Notebook(msg)) => Some(msg),
            _ => None,
        })
        .collect()
}

#[test]
fn the_empty_state_shows_both_templates_and_the_three_ways_in() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_in(dir.path());
    open(&mut app, id);
    let texts = shown(&app, id);
    for part in [
        "This project has no Design Notebook yet",
        "Design report",
        "Lab notebook",
        "Create from template",
        "Generate from project…",
        "Import .typ files…",
        "Creates notebook/design.snxnbk on Save",
        "No notebook",
        "Export PDF…",
    ] {
        assert!(has(&texts, part), "{part} in {texts:?}");
    }
    assert!(!has(&texts, "Insert reference"), "nothing to insert into");
}

#[test]
fn the_empty_state_creates_the_picked_template() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_in(dir.path());
    open(&mut app, id);
    let picked = clicked(&app, id, "Lab notebook");
    assert!(
        matches!(
            picked.as_slice(),
            [NotebookMsg::TemplateSelected {
                template: Template::LabNotebook,
                ..
            }]
        ),
        "{picked:?}"
    );
    send(&mut app, picked.into_iter().next().unwrap());
    let create = clicked(&app, id, "Create from template");
    assert!(
        matches!(
            create.as_slice(),
            [NotebookMsg::TemplatePicked {
                template: Template::LabNotebook,
                ..
            }]
        ),
        "{create:?}"
    );
    let import = clicked(&app, id, "Import .typ files…");
    assert!(
        matches!(import.as_slice(), [NotebookMsg::ImportPressed { .. }]),
        "{import:?}"
    );
}

#[test]
fn an_open_notebook_shows_the_toolbar_outline_editor_preview_and_status() {
    let dir = tempfile::tempdir().unwrap();
    let (app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let texts = shown(&app, id);
    for part in [
        "Source",
        "Split",
        "Preview",
        "Insert reference",
        "Calculation",
        "Measurement",
        "Figure",
        "Block",
        "Save",
        "Outline",
        "Pages",
        "References",
        "Sections",
        "Power stage",
        "· Typst",
        "Recompiles as you type",
        "+++ header (TOML)",
        "Problems",
        "No problems.",
        "Compiled in",
        "Ln ",
    ] {
        assert!(has(&texts, part), "{part} in {texts:?}");
    }
}

#[test]
fn a_compile_error_shows_its_mark_its_peek_its_row_and_the_banner() {
    let dir = tempfile::tempdir().unwrap();
    let (app, id) = app_with_notebook(dir.path(), &broken());
    let (doc, tab) = app.notebook_parts(id).unwrap();
    let held = marks(doc, tab);
    assert!(held.contains(&(1, Mark::Error)), "{held:?}");
    let texts = shown(&app, id);
    let parts = [
        "Go to error",
        "Compile failed at line",
        "Compile failed ·",
        "error",
    ];
    for part in parts {
        assert!(has(&texts, part), "{part} in {texts:?}");
    }
    let go = clicked(&app, id, "Go to error");
    assert!(
        matches!(go.first(), Some(NotebookMsg::LinePicked { line: 5, .. })),
        "{go:?}"
    );
}

#[test]
fn preview_hides_the_editor_and_source_hides_the_preview() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    pick_view(&mut app, id, ViewMode::Preview);
    let preview = shown(&app, id);
    assert!(!has(&preview, "Recompiles as you type"), "{preview:?}");
    assert!(has(&preview, "Next page"), "{preview:?}");
    pick_view(&mut app, id, ViewMode::Source);
    let source = shown(&app, id);
    assert!(has(&source, "Recompiles as you type"), "{source:?}");
    assert!(!has(&source, "Next page"), "{source:?}");
}

#[test]
fn an_outline_row_moves_the_cursor_to_its_line() {
    let dir = tempfile::tempdir().unwrap();
    let (app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let picked = clicked(&app, id, "Power stage");
    assert!(
        picked
            .iter()
            .any(|msg| matches!(msg, NotebookMsg::LinePicked { line: 5, .. })),
        "{picked:?}"
    );
}

#[test]
fn the_references_panel_lists_each_reference_once() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    pick_panel(&mut app, id, SidePanel::References);
    let texts = shown(&app, id);
    assert!(texts.iter().any(|t| t == "R7"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "+3V3"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "part") && texts.iter().any(|t| t == "net"));
}

#[test]
fn the_pages_panel_shows_a_thumbnail_per_page() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    pick_panel(&mut app, id, SidePanel::Pages);
    let tab = &app.ui_state.notebook_tabs[&id];
    let pages = tab.compile.last_good.as_ref().unwrap().status.pages.len();
    assert_eq!(tab.compile.thumbnails.len(), pages);
    assert!(has(&shown(&app, id), "Page "));
}

#[test]
fn the_block_button_opens_its_menu_and_a_row_inserts_its_block() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let toggled = clicked(&app, id, "Block");
    assert!(
        matches!(toggled.as_slice(), [NotebookMsg::BlockMenuToggled { .. }]),
        "{toggled:?}"
    );
    send(&mut app, NotebookMsg::BlockMenuToggled { project_id: id });
    let texts = shown(&app, id);
    for row in ["Note", "Issue", "Decision"] {
        assert!(texts.iter().any(|t| t == row), "{row} in {texts:?}");
    }
    let inserted = clicked(&app, id, "Decision");
    assert!(
        inserted
            .iter()
            .any(|msg| matches!(msg, NotebookMsg::InsertPressed { .. })),
        "{inserted:?}"
    );
}

#[test]
fn save_is_greyed_until_the_text_changes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    assert!(clicked(&app, id, "Save").is_empty(), "nothing to save");
    let typed = NotebookMsg::TextActed {
        project_id: id,
        action: super::edit::typed('x'),
    };
    send(&mut app, typed);
    let save = clicked(&app, id, "Save");
    assert!(
        matches!(save.as_slice(), [NotebookMsg::SavePressed { .. }]),
        "{save:?}"
    );
    assert!(has(&shown(&app, id), "Unsaved, written on Save"));
}
