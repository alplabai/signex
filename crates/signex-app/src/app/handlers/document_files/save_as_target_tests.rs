//! #1823: Save As refuses a target that is a project file or whose extension
//! does not match the document, and writes nothing.

use std::path::{Path, PathBuf};

use signex_types::format::SnxSchematic;

use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
use crate::app::{FootprintDocument, Signex, SymbolDocument, TabInfo, TabKind};

const PROJECT_BYTES: &[u8] = b"{\"project\":\"bytes that must survive\"}";

/// A project in `dir` whose `.snxprj` holds [`PROJECT_BYTES`] and whose one
/// sheet `a.snxsch` is open and active.
fn schematic_fixture(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let sheet = dir.join("a.snxsch");
    let text = SnxSchematic::new(sheet_with_net("R1", "N", &[]))
        .write_string()
        .expect("encode");
    std::fs::write(&sheet, text).expect("write sheet");
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    let project = app.document_state.projects[0].path.clone();
    std::fs::write(&project, PROJECT_BYTES).expect("write project");
    app.open_or_focus_sheet_path(sheet.clone());
    (app, sheet, project)
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn save_as_onto_the_projects_snxprj_is_refused_and_leaves_it_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet, project) = schematic_fixture(dir.path());
    let error = app
        .save_active_document_as(&sheet, project.clone())
        .expect_err("a project file is not a Save As target");
    assert!(error.to_string().contains("project file"), "{error}");
    assert_eq!(std::fs::read(&project).unwrap(), PROJECT_BYTES);
    assert_eq!(app.active_tab_path().as_deref(), Some(sheet.as_path()));
}

#[test]
fn save_as_onto_a_differently_cased_snxprj_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet, project) = schematic_fixture(dir.path());
    let shouted = dir.path().join("PROJECTA.SNXPRJ");
    assert!(
        app.save_active_document_as(&sheet, shouted).is_err(),
        "{project:?}"
    );
    assert_eq!(std::fs::read(&project).unwrap(), PROJECT_BYTES);
}

#[test]
fn save_as_to_a_foreign_extension_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet, _project) = schematic_fixture(dir.path());
    let before = entries(dir.path());
    let error = app
        .save_active_document_as(&sheet, dir.path().join("copy.txt"))
        .expect_err("not a .snxsch");
    assert!(error.to_string().contains(".snxsch"), "{error}");
    assert_eq!(entries(dir.path()), before);
}

#[test]
fn save_as_without_an_extension_is_saved_as_snxsch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet, _project) = schematic_fixture(dir.path());
    let task = app
        .save_active_document_as(&sheet, dir.path().join("copy"))
        .expect("starts");
    crate::app::path_ops::settle_task(&mut app, task);
    assert!(dir.path().join("copy.snxsch").exists());
    assert!(!dir.path().join("copy").exists());
}

#[test]
fn an_extensionless_save_as_never_overwrites_an_existing_snxsch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, sheet, _project) = schematic_fixture(dir.path());
    let before = std::fs::read(&sheet).unwrap();
    let task = app
        .save_active_document_as(&sheet, dir.path().join("a"))
        .expect("refusal is a message, not an error");
    crate::app::path_ops::settle_task(&mut app, task);
    assert_eq!(std::fs::read(&sheet).unwrap(), before);
}

/// Symbol / footprint editor `from` in `dir`, beside a project whose
/// `.snxprj` holds [`PROJECT_BYTES`].
fn primitive_fixture(dir: &Path, ext: &str) -> (Signex, PathBuf, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let project = app.document_state.projects[0].path.clone();
    std::fs::write(&project, PROJECT_BYTES).unwrap();
    let from = dir.join(format!("new.{ext}"));
    let kind = if ext == "snxsym" {
        let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
        app.document_state
            .symbol_editors
            .insert(from.clone(), SymbolDocument::new(from.clone(), file));
        TabKind::SymbolEditor(from.clone())
    } else {
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
        app.document_state
            .footprint_editors
            .insert(from.clone(), FootprintDocument::new(from.clone(), file));
        TabKind::FootprintEditor(from.clone())
    };
    app.document_state.tabs.push(TabInfo {
        title: "new".to_string(),
        path: from.clone(),
        cached_document: None,
        project_id: None,
        kind,
    });
    app.document_state.active_tab = 0;
    (app, from, project)
}

#[test]
fn symbol_and_footprint_save_as_refuse_the_snxprj_and_a_foreign_extension() {
    for ext in ["snxsym", "snxfpt"] {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, from, project) = primitive_fixture(dir.path(), ext);
        assert!(app.handle_save_primitive_as(&from, &project).is_err());
        assert_eq!(std::fs::read(&project).unwrap(), PROJECT_BYTES, "{ext}");
        let wrong = dir.path().join("copy.snxsch");
        assert!(app.handle_save_primitive_as(&from, &wrong).is_err());
        assert!(!wrong.exists(), "{ext}");
        assert!(
            app.document_state.tabs.iter().any(|t| t.path == from),
            "{ext}: the editor stays where it was"
        );
    }
}
