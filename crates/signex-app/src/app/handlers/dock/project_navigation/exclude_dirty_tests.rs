//! #738 / #694 — the Exclude prompt keys paths by the dialog project's probed
//! volume rule, not by the host family. Unit tests because `CaseFold` is
//! crate-private, so an integration test cannot build a folding project.

use std::path::{Path, PathBuf};

use super::exclude_dirty::DiscardBlock;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::scope::CaseFold;
use crate::app::{ExcludeDirtyChoice, Message, RemoveChoice, RemoveDialogState, RemoveMsg, Signex};

const ORIGINAL: &[u8] = b"original-bytes";

/// A project on a case-insensitive volume listing `X.snxsch`, with that sheet
/// held dirty under the spelling `x.snxsch`, and an Exclude dialog opened on
/// `X.snxsch`. The probe says "folds" on every host, so a host-family fold
/// would disagree on macOS and Linux.
fn folding_project_with_dirty_lowercase_sheet() -> (Signex, tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().expect("utf8 tempdir").to_string();
    let mut app = app_workspace(&dir, &["X.snxsch"]);
    app.document_state.projects[0].case_fold = CaseFold::from_probe(true);
    let held = tmp.path().join("x.snxsch");
    std::fs::write(&held, ORIGINAL).expect("write sheet");
    let mut engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default())
        .expect("engine");
    engine.set_path(Some(held.clone()));
    app.document_state.engines.insert(held.clone(), engine);
    app.document_state.dirty_paths.insert(held.clone());
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: tmp.path().join("X.snxsch"),
        tree_path: vec![0, 0],
        project_id: Some(app.document_state.projects[0].id),
        display_name: "X.snxsch".to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    (app, tmp, held)
}

fn exclude_then(app: &mut Signex, choice: ExcludeDirtyChoice) {
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));
    assert!(
        app.ui_state
            .remove_dialog
            .as_ref()
            .is_some_and(|d| d.unsaved_prompt),
        "a dirty sheet held under a case variant must still prompt"
    );
    let _ = app.update(Message::Remove(RemoveMsg::ExcludeDirty(choice)));
}

fn project_lists_sheet(app: &Signex) -> bool {
    app.document_state.projects[0]
        .data
        .sheets
        .iter()
        .any(|s| s.filename == "X.snxsch")
}

#[test]
fn save_finds_a_dirty_sheet_under_a_case_variant_on_a_folding_volume() {
    let (mut app, _tmp, held) = folding_project_with_dirty_lowercase_sheet();

    exclude_then(&mut app, ExcludeDirtyChoice::Save);

    assert_ne!(std::fs::read(&held).unwrap(), ORIGINAL, "Save must write");
    assert!(!app.document_state.dirty_paths.contains(&held));
    assert!(app.ui_state.remove_dialog.is_none());
    assert!(!project_lists_sheet(&app));
}

#[test]
fn discard_drops_a_dirty_sheet_under_a_case_variant_on_a_folding_volume() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _tmp, held) = folding_project_with_dirty_lowercase_sheet();

    exclude_then(&mut app, ExcludeDirtyChoice::Discard);

    assert_eq!(std::fs::read(&held).unwrap(), ORIGINAL, "must not write");
    assert!(!app.document_state.dirty_paths.contains(&held));
    assert!(!app.document_state.engines.contains_key(Path::new(&held)));
    assert!(!project_lists_sheet(&app));
}

/// #1051 — a discarded sheet's net-colour overrides go with its engine.
#[test]
fn discard_forgets_the_discarded_sheets_net_colours() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _tmp, held) = folding_project_with_dirty_lowercase_sheet();
    let wire = uuid::Uuid::new_v4();
    let sheet = signex_types::schematic::SchematicSheet {
        wires: vec![signex_types::schematic::Wire {
            uuid: wire,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(held.clone()));
    app.document_state.engines.insert(held, engine);
    let red = signex_types::theme::Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    app.ui_state.wire_color_overrides.insert(wire, red);

    exclude_then(&mut app, ExcludeDirtyChoice::Discard);

    assert!(
        !app.ui_state.wire_color_overrides.contains_key(&wire),
        "a discarded sheet must not leave its net colours behind"
    );
}

/// Project A holds `Power.snxsch` dirty. Project B, on the same folding
/// volume, lists and has loaded the same file as `power.snxsch`. Both probes
/// say "folds", so the host family must not decide (#1034).
fn two_folding_projects_sharing_one_sheet() -> (Signex, tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().expect("utf8 tempdir").to_string();
    let mut app = app_workspace(&dir, &["Power.snxsch"]);
    let mut other = app_workspace(&dir, &["power.snxsch"])
        .document_state
        .projects
        .remove(0);
    other.id = app.document_state.mint_project_id();
    other.path = tmp.path().join("ProjectB.snxprj");
    app.document_state.projects.push(other);
    for project in &mut app.document_state.projects {
        project.case_fold = CaseFold::from_probe(true);
    }
    let a_sheet = tmp.path().join("Power.snxsch");
    let b_sheet = tmp.path().join("power.snxsch");
    for sheet in [&a_sheet, &b_sheet] {
        let mut engine =
            signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default())
                .expect("engine");
        engine.set_path(Some(sheet.clone()));
        app.document_state.engines.insert(sheet.clone(), engine);
    }
    app.document_state.dirty_paths.insert(a_sheet.clone());
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: a_sheet.clone(),
        tree_path: vec![0, 0],
        project_id: Some(app.document_state.projects[0].id),
        display_name: "Power.snxsch".to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    (app, tmp, a_sheet, b_sheet)
}

/// #1034 — the share check keys both spellings as one file, so Discard is
/// withheld and a Discard attempt drops neither project's engine.
#[test]
fn discard_is_withheld_when_a_folding_project_reaches_the_sheet_under_another_spelling() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _tmp, a_sheet, b_sheet) = two_folding_projects_sharing_one_sheet();
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));
    let state = app.ui_state.remove_dialog.clone().expect("prompt open");
    assert!(state.unsaved_prompt);

    assert_eq!(app.discard_block(&state), Some(DiscardBlock::Shared));

    let _ = app.update(Message::Remove(RemoveMsg::ExcludeDirty(
        ExcludeDirtyChoice::Discard,
    )));
    assert!(app.document_state.engines.contains_key(&a_sheet));
    assert!(
        app.document_state.engines.contains_key(&b_sheet),
        "the other project's engine must survive a refused Discard"
    );
    assert!(app.document_state.dirty_paths.contains(&a_sheet));
}

/// The prompt names the owner by the project's current index, so closing an
/// earlier project while the stage is open does not make the dialog's own
/// project look like "another project".
#[test]
fn prompt_follows_its_project_when_an_earlier_project_closes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().expect("utf8 tempdir").to_string();
    let mut app = app_workspace(&dir, &["Other.snxsch"]);
    let mut owner = app_workspace(&dir, &["X.snxsch"])
        .document_state
        .projects
        .remove(0);
    owner.id = app.document_state.mint_project_id();
    owner.path = tmp.path().join("ProjectB.snxprj");
    let owner_id = owner.id;
    app.document_state.projects.push(owner);
    let sheet = tmp.path().join("X.snxsch");
    let mut engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default())
        .expect("engine");
    engine.set_path(Some(sheet.clone()));
    app.document_state.engines.insert(sheet.clone(), engine);
    app.document_state.dirty_paths.insert(sheet.clone());
    let state = RemoveDialogState {
        target_path: sheet,
        tree_path: vec![1, 0],
        project_id: Some(owner_id),
        display_name: "X.snxsch".to_string(),
        is_outside_project: false,
        unsaved_prompt: true,
    };

    app.document_state.projects.remove(0);

    let (body, can_discard) = app.exclude_dirty_prompt(&state);
    assert!(can_discard, "only the dialog's own project is open: {body}");
}

/// Two engines for one file, held as `Lib`/`lib` with different edits, both
/// dirty, under `dir`.
fn hold_differing_case_variants(app: &mut Signex, dir: &Path) -> (PathBuf, PathBuf) {
    let upper = dir.join("Lib.snxsch");
    let lower = dir.join("lib.snxsch");
    for (path, paper) in [(&upper, "A4"), (&lower, "A3")] {
        let sheet = signex_types::schematic::SchematicSheet {
            paper_size: paper.to_string(),
            ..Default::default()
        };
        let mut engine = signex_engine::Engine::new(sheet).expect("engine");
        engine.set_path(Some(path.clone()));
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.dirty_paths.insert(path.clone());
    }
    (upper, lower)
}

/// #1077 — Save All keys by the owning project's probed fold.
#[test]
fn save_all_refuses_differing_variants_on_a_folding_project_and_keeps_markers() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().expect("utf8 tempdir").to_string();
    let mut app = app_workspace(&dir, &["Lib.snxsch"]);
    app.document_state.projects[0].case_fold = CaseFold::from_probe(true);
    let (upper, lower) = hold_differing_case_variants(&mut app, tmp.path());
    std::fs::write(&upper, ORIGINAL).unwrap();

    let result = app.try_save_dirty_path(&upper);

    assert!(result.is_err(), "differing variants must be refused");
    assert_eq!(std::fs::read(&upper).unwrap(), ORIGINAL, "nothing written");
    assert!(app.document_state.dirty_paths.contains(&upper));
    assert!(app.document_state.dirty_paths.contains(&lower));
}

/// #1077 — a loose document has no probe, so the host default decides.
#[test]
fn save_all_uses_the_host_default_for_a_loose_document() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let (upper, lower) = hold_differing_case_variants(&mut app, tmp.path());

    let result = app.try_save_dirty_path(&upper);

    if CaseFold::host_default().fold_case {
        assert!(result.is_err(), "host folds: variants are one file");
        assert!(app.document_state.dirty_paths.contains(&lower));
    } else {
        assert!(result.is_ok(), "host keeps case: {result:?}");
        assert!(app.document_state.dirty_paths.contains(&lower));
    }
}

/// #1077 — the project's rule decides, even when it is the opposite of the
/// host family's, so this fails without the fix on every host.
#[test]
fn save_all_follows_the_projects_rule_not_the_hosts() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().expect("utf8 tempdir").to_string();
    let mut app = app_workspace(&dir, &["Lib.snxsch"]);
    let project_folds = !CaseFold::host_default().fold_case;
    app.document_state.projects[0].case_fold = CaseFold::from_probe(project_folds);
    let (upper, lower) = hold_differing_case_variants(&mut app, tmp.path());

    let result = app.try_save_dirty_path(&upper);

    if project_folds {
        assert!(result.is_err(), "folding project: variants are one file");
        assert!(app.document_state.dirty_paths.contains(&upper));
        assert!(app.document_state.dirty_paths.contains(&lower));
    } else {
        assert!(result.is_ok(), "case-preserving project: {result:?}");
        app.try_save_dirty_path(&lower)
            .expect("save second spelling");
        assert!(app.document_state.dirty_paths.is_empty());
    }
}

/// #1077 — nested project directories: the inner project's volume rule wins
/// over an earlier-loaded outer one.
#[test]
fn save_all_uses_the_innermost_projects_rule_for_nested_projects() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_str().expect("utf8 tempdir").to_string();
    let mut app = app_workspace(&dir, &["Top.snxsch"]);
    app.document_state.projects[0].case_fold = CaseFold::from_probe(false);
    let inner_dir = tmp.path().join("inner");
    std::fs::create_dir(&inner_dir).expect("inner dir");
    let mut inner = app.document_state.projects[0].clone();
    inner.id = app.document_state.mint_project_id();
    inner.path = inner_dir.join("Inner.snxprj");
    inner.case_fold = CaseFold::from_probe(true);
    app.document_state.projects.push(inner);
    let (upper, lower) = hold_differing_case_variants(&mut app, &inner_dir);

    let result = app.try_save_dirty_path(&upper);

    assert!(result.is_err(), "the inner folding volume decides");
    assert!(app.document_state.dirty_paths.contains(&upper));
    assert!(app.document_state.dirty_paths.contains(&lower));
}

/// #1077 — a project with an empty dir must not claim every file.
#[test]
fn save_all_ignores_a_project_with_an_empty_dir() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    let mut stray = crate::app::LoadedProject {
        id,
        path: PathBuf::from("Stray.snxprj"),
        data: signex_types::project::ProjectData {
            name: "s".into(),
            dir: String::new(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: CaseFold::default(),
        case_probed: false,
    };
    stray.case_fold = CaseFold::from_probe(!CaseFold::host_default().fold_case);
    app.document_state.projects.push(stray);
    let (upper, _lower) = hold_differing_case_variants(&mut app, tmp.path());

    let result = app.try_save_dirty_path(&upper);

    assert_eq!(
        result.is_err(),
        CaseFold::host_default().fold_case,
        "a loose file keys by the host default: {result:?}"
    );
}
