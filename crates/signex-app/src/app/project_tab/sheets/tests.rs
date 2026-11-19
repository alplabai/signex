//! #31 (mockup m31): the Sheets rows' root and missing marks, Add New ▸
//! Schematic from the empty state, and the New sheets preview's fit.

use iced::Size;
use signex_types::project::SheetDefaults;
use signex_types::schematic::{SheetOrientation, SheetSize};

use super::preview::{PREVIEW_H, fit_sheet, mm_text, sheet_of};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, SetupKind, Signex};

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-sheets-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(
        &dir.to_string_lossy(),
        &["a.snxsch", "b.snxsch", "c.snxsch"],
    );
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn send(app: &mut Signex, msg: ProjectTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Tab(msg)));
}

/// The declared root is marked `root`; a file the last probe found absent
/// is marked `missing`; a file the probe has not answered for is not.
#[test]
fn sheet_rows_mark_the_root_and_a_missing_file() {
    let (mut app, _) = app();
    let dir = app.document_state.projects[0].dir().to_path_buf();
    app.document_state
        .project_fs
        .facts
        .set_exists(dir.join("b.snxsch"), false);
    app.document_state
        .project_fs
        .facts
        .set_exists(dir.join("c.snxsch"), true);
    let rows = app.project_sheet_rows(&app.document_state.projects[0]);
    let marks: Vec<(&str, bool, bool)> = rows
        .iter()
        .map(|row| (row.filename.as_str(), row.is_root, row.is_missing))
        .collect();
    assert_eq!(
        marks,
        [
            ("a.snxsch", true, false),
            ("b.snxsch", false, true),
            ("c.snxsch", false, false),
        ]
    );

    // Another root: the mark follows the project's `schematic_root`.
    app.document_state.projects[0].data.schematic_root = Some("c.snxsch".into());
    let rows = app.project_sheet_rows(&app.document_state.projects[0]);
    let roots: Vec<bool> = rows.iter().map(|row| row.is_root).collect();
    assert_eq!(roots, [false, false, true]);

    // No declared root: no row is marked.
    app.document_state.projects[0].data.schematic_root = None;
    let rows = app.project_sheet_rows(&app.document_state.projects[0]);
    assert!(rows.iter().all(|row| !row.is_root));
}

/// The Sheets pill and the project tree's `(missing)` share one rule: an
/// absent file held by an open tab or an unsaved document is not missing.
#[test]
fn an_absent_file_held_open_or_unsaved_is_not_missing() {
    let mut facts = crate::app::state::project_fs::ProjectFsFacts::default();
    let gone = std::path::PathBuf::from("/p/gone.snxsch");
    facts.set_exists(gone.clone(), false);
    assert!(facts.is_listed_file_missing(&gone, false, false));
    assert!(!facts.is_listed_file_missing(&gone, true, false));
    assert!(!facts.is_listed_file_missing(&gone, false, true));
    let unprobed = std::path::Path::new("/p/new.snxsch");
    assert!(!facts.is_listed_file_missing(unprobed, false, false));
}

/// Add New ▸ Schematic in the empty state opens the New Schematic setup for
/// the project, as the project tree's Add New ▸ Schematic does.
#[test]
fn add_schematic_opens_the_new_schematic_setup_for_the_project() {
    let (mut app, id) = app();
    assert!(app.ui_state.document_setup.is_none());
    send(
        &mut app,
        ProjectTabMsg::AddSchematicRequested { project_id: id },
    );
    let setup = app
        .ui_state
        .document_setup
        .as_ref()
        .expect("the New Schematic setup is open");
    assert_eq!(setup.kind, SetupKind::Sheet);
    assert_eq!(setup.project, id);
}

/// A late Add New ▸ Schematic for a project closed meanwhile opens nothing
/// and leaves the other projects' state alone.
#[test]
fn add_schematic_for_a_closed_project_does_nothing() {
    let (mut app, id) = app();
    app.document_state.projects.clear();
    send(
        &mut app,
        ProjectTabMsg::AddSchematicRequested { project_id: id },
    );
    assert!(app.ui_state.document_setup.is_none());
    assert!(app.document_state.projects.is_empty());
}

/// A4 landscape and portrait both fit inside the drawing with the label band
/// clear and centred across; the portrait sheet is drawn smaller, since the
/// drawing is wider than tall.
#[test]
fn the_preview_fits_a4_landscape_and_portrait() {
    let area = Size::new(PREVIEW_H * 1.8, PREVIEW_H);
    let a4 = SheetSize::IsoA4;
    let mut scales = Vec::new();
    for orientation in [SheetOrientation::Landscape, SheetOrientation::Portrait] {
        let (w, h) = a4.dimensions_mm(orientation);
        let fit = fit_sheet(w, h, area);
        let (drawn_w, drawn_h) = (w as f32 * fit.scale, h as f32 * fit.scale);
        assert!(fit.scale > 0.0, "{orientation:?}");
        assert!(
            fit.origin.x >= 0.0 && fit.origin.y >= 0.0,
            "{orientation:?}"
        );
        assert!(fit.origin.x + drawn_w <= area.width, "{orientation:?}");
        // The band under the sheet stays free for the size label.
        assert!(
            fit.origin.y + drawn_h < area.height * 0.9,
            "{orientation:?}"
        );
        let centre = fit.origin.x + drawn_w / 2.0;
        assert!((centre - area.width / 2.0).abs() < 0.01, "{orientation:?}");
        scales.push((fit.scale, drawn_w, drawn_h));
    }
    let (landscape, portrait) = (scales[0], scales[1]);
    assert!(
        portrait.0 < landscape.0,
        "portrait is drawn at a smaller scale"
    );
    assert!(portrait.1 < landscape.1, "portrait is narrower");
    // Both are limited by the height: the long and the short edge fill it.
    assert!((landscape.2 - portrait.2).abs() < 0.01);
}

/// The preview reads size and orientation from the stored token, and labels
/// lengths in whole or tenth millimetres.
#[test]
fn the_preview_reads_the_sheet_token_and_labels_millimetres() {
    let defaults = SheetDefaults {
        paper_size: "A4 portrait".into(),
        show_title_block: true,
    };
    assert_eq!(
        sheet_of(&defaults),
        (SheetSize::IsoA4, SheetOrientation::Portrait)
    );
    assert_eq!(mm_text(297.0), "297");
    assert_eq!(mm_text(215.9), "215.9");
    assert_eq!(mm_text(279.39999), "279.4");
}
