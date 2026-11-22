//! Rows m32 (Variants), m33 (Libraries), m34 (ERC and annotation) and m35
//! (Version control) of the Project tab. Base is neither renamed nor deleted:
//! the 2026-11-19 Variant Manager decision, over the mockup's caption.

use std::path::PathBuf;

use signex_app::app::{
    EnableVersionControlMsg, EnableVersionControlOutcome, Message, ProjectSection, ProjectTabMsg,
    VersionControlScope,
};
use signex_types::project::{
    AnnotateUndo, BASE_VARIANT, LibraryEntry, LibraryEntryKind, ListMove, VariantProblem,
};

use super::support::{Bench, bench};

const ONE_SHEET: &[(&str, &str)] = &[("root.snxsch", "")];

fn on(section: ProjectSection) -> Bench {
    let mut b = bench(ONE_SHEET);
    b.open_section(section);
    b
}

/// New variant, then Enter on `name`.
fn add_variant(b: &mut Bench, name: &str) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::NewVariantRequested { project_id });
    let from = b
        .tab_state()
        .variant_rename
        .as_ref()
        .expect("the new name is open")
        .from
        .clone();
    rename_variant(b, &from, name);
}

fn rename_variant(b: &mut Bench, from: &str, text: &str) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::VariantRenameEdited {
        project_id,
        from: from.to_string(),
        text: text.to_string(),
    });
    b.tab(ProjectTabMsg::VariantRenameSubmitted { project_id });
}

#[test]
fn m32_new_variant_adds_one_and_opens_its_name() {
    let mut b = on(ProjectSection::Variants);
    let project_id = b.id;
    b.tab(ProjectTabMsg::NewVariantRequested { project_id });
    assert_eq!(b.data().variant_definitions.len(), 1);
    let name = b.data().variant_definitions[0].clone();
    let draft = b.tab_state().variant_rename.as_ref().expect("name open");
    assert_eq!(draft.from, name);
    assert_eq!(
        b.tab_state().variant_selected.as_deref(),
        Some(name.as_str())
    );
    assert!(b.dirty());
}

#[test]
fn m32_undo_removes_the_new_variant() {
    let mut b = on(ProjectSection::Variants);
    let project_id = b.id;
    b.tab(ProjectTabMsg::NewVariantRequested { project_id });
    b.undo();
    assert!(b.data().variant_definitions.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m32_rename_changes_the_name_and_the_active_variant_follows() {
    let mut b = on(ProjectSection::Variants);
    add_variant(&mut b, "Lite");
    let project_id = b.id;
    b.tab(ProjectTabMsg::ActiveVariantPicked {
        project_id,
        name: Some("Lite".into()),
    });
    rename_variant(&mut b, "Lite", "Pro");
    assert_eq!(b.data().variant_definitions, ["Pro"]);
    assert_eq!(b.data().active_variant.as_deref(), Some("Pro"));
}

#[test]
fn m32_a_taken_name_is_refused_and_stays_in_its_box() {
    let mut b = on(ProjectSection::Variants);
    add_variant(&mut b, "Lite");
    add_variant(&mut b, "Pro");
    rename_variant(&mut b, "Pro", "lite");
    assert_eq!(b.data().variant_definitions, ["Lite", "Pro"]);
    assert_eq!(
        b.tab_state().variant_problem,
        Some(VariantProblem::Taken("Lite".to_string()))
    );
    let draft = b.tab_state().variant_rename.as_ref().expect("still open");
    assert_eq!(draft.text, "lite");
}

#[test]
fn m32_a_variant_cannot_take_the_name_base() {
    let mut b = on(ProjectSection::Variants);
    add_variant(&mut b, "Lite");
    rename_variant(&mut b, "Lite", BASE_VARIANT);
    assert_eq!(b.data().variant_definitions, ["Lite"]);
    assert_eq!(
        b.tab_state().variant_problem,
        Some(VariantProblem::Reserved)
    );
}

#[test]
fn m32_base_cannot_be_renamed() {
    let mut b = on(ProjectSection::Variants);
    add_variant(&mut b, "Lite");
    rename_variant(&mut b, BASE_VARIANT, "Reference");
    assert_eq!(b.data().variant_definitions, ["Lite"]);
    assert!(b.tab_state().variant_problem.is_some());
}

#[test]
fn m32_base_cannot_be_deleted() {
    let mut b = on(ProjectSection::Variants);
    add_variant(&mut b, "Lite");
    let project_id = b.id;
    b.tab(ProjectTabMsg::VariantRemoved {
        project_id,
        name: BASE_VARIANT.to_string(),
    });
    assert_eq!(b.data().variant_definitions, ["Lite"]);
    assert!(b.tab_state().variant_problem.is_some());
}

#[test]
fn m32_delete_of_the_active_variant_falls_back_to_base_and_undo_restores_it() {
    let mut b = on(ProjectSection::Variants);
    add_variant(&mut b, "Lite");
    let project_id = b.id;
    b.tab(ProjectTabMsg::ActiveVariantPicked {
        project_id,
        name: Some("Lite".into()),
    });
    b.tab(ProjectTabMsg::VariantRemoved {
        project_id,
        name: "Lite".into(),
    });
    assert!(b.data().variant_definitions.is_empty());
    assert_eq!(b.data().active_variant_name(), BASE_VARIANT);
    b.undo();
    assert_eq!(b.data().variant_definitions, ["Lite"]);
    assert_eq!(b.data().active_variant.as_deref(), Some("Lite"));
}

#[test]
fn m32_picking_active_is_stored_and_undone() {
    let mut b = on(ProjectSection::Variants);
    add_variant(&mut b, "Lite");
    let project_id = b.id;
    b.tab(ProjectTabMsg::ActiveVariantPicked {
        project_id,
        name: Some("Lite".into()),
    });
    assert_eq!(b.data().active_variant_name(), "Lite");
    b.undo();
    assert_eq!(b.data().active_variant_name(), BASE_VARIANT);
}

#[test]
fn m32_picking_an_unknown_active_variant_is_refused() {
    let mut b = on(ProjectSection::Variants);
    let project_id = b.id;
    b.tab(ProjectTabMsg::ActiveVariantPicked {
        project_id,
        name: Some("Ghost".into()),
    });
    assert_eq!(b.data().active_variant, None);
    assert_eq!(
        b.tab_state().variant_problem,
        Some(VariantProblem::Unknown("Ghost".to_string()))
    );
    assert!(!b.dirty());
}

#[test]
fn m32_selecting_a_variant_is_not_picking_it_active() {
    let mut b = on(ProjectSection::Variants);
    add_variant(&mut b, "Lite");
    let dirty = b.dirty();
    let project_id = b.id;
    b.tab(ProjectTabMsg::VariantSelected {
        project_id,
        name: Some("Lite".into()),
    });
    assert_eq!(b.tab_state().variant_selected.as_deref(), Some("Lite"));
    assert_eq!(b.data().active_variant, None);
    assert_eq!(b.dirty(), dirty);
}

#[test]
fn m32_the_filter_and_only_differ_are_view_state() {
    let mut b = on(ProjectSection::Variants);
    let project_id = b.id;
    b.tab(ProjectTabMsg::VariantFilterEdited {
        project_id,
        text: "R*".into(),
    });
    b.tab(ProjectTabMsg::VariantDifferingToggled { project_id });
    assert_eq!(b.tab_state().variant_filter, "R*");
    assert!(b.tab_state().variants_only_differ);
    assert!(!b.dirty());
}

#[test]
fn m32_read_again_fills_the_parts_matrix() {
    let mut b = on(ProjectSection::Variants);
    let project_id = b.id;
    b.tab(ProjectTabMsg::VariantPartsReadRequested { project_id });
    assert!(!b.tab_state().variant_parts_reading);
    assert!(matches!(
        b.tab_state().variant_parts.as_deref(),
        Some(Ok(_))
    ));
}

fn library(name: &str) -> LibraryEntry {
    LibraryEntry {
        path: PathBuf::from(name),
        kind: LibraryEntryKind::ProjectLocal,
        library_id: None,
        unknown: Default::default(),
    }
}

fn with_libraries() -> Bench {
    let mut b = on(ProjectSection::Libraries);
    b.app.document_state.projects[0].data.libraries =
        vec![library("a.snxlib"), library("b.snxlib")];
    b
}

fn library_order(b: &Bench) -> Vec<PathBuf> {
    b.data().libraries.iter().map(|e| e.path.clone()).collect()
}

fn move_library(b: &mut Bench, name: &str, step: ListMove) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::LibraryMoved {
        project_id,
        path: PathBuf::from(name),
        step,
    });
}

#[test]
fn m33_move_reorders_the_libraries_and_undo_puts_them_back() {
    let mut b = with_libraries();
    move_library(&mut b, "b.snxlib", ListMove::Up);
    assert_eq!(
        library_order(&b),
        [PathBuf::from("b.snxlib"), PathBuf::from("a.snxlib")]
    );
    assert!(b.dirty());
    b.undo();
    assert_eq!(
        library_order(&b),
        [PathBuf::from("a.snxlib"), PathBuf::from("b.snxlib")]
    );
    assert!(!b.dirty());
}

#[test]
fn m33_moving_past_the_end_changes_nothing() {
    let mut b = with_libraries();
    move_library(&mut b, "a.snxlib", ListMove::Up);
    assert_eq!(library_order(&b)[0], PathBuf::from("a.snxlib"));
    assert!(!b.dirty());
}

#[test]
fn m34_the_annotate_undo_pick_is_stored_and_undone() {
    let mut b = on(ProjectSection::ErcAnnotation);
    let project_id = b.id;
    b.tab(ProjectTabMsg::AnnotateUndoPicked {
        project_id,
        choice: Some(AnnotateUndo::PerSheet),
    });
    assert_eq!(b.data().annotate_undo(), Some(AnnotateUndo::PerSheet));
    assert!(b.dirty());
    b.undo();
    assert_eq!(b.data().annotate_undo(), None);
    assert!(!b.dirty());
}

#[test]
fn m34_preferences_default_clears_the_projects_choice() {
    let mut b = on(ProjectSection::ErcAnnotation);
    let project_id = b.id;
    b.tab(ProjectTabMsg::AnnotateUndoPicked {
        project_id,
        choice: Some(AnnotateUndo::PerSheet),
    });
    b.tab(ProjectTabMsg::AnnotateUndoPicked {
        project_id,
        choice: None,
    });
    assert_eq!(b.data().annotate_undo(), None);
}

#[test]
fn m34_store_erc_rules_copies_them_into_the_project() {
    let mut b = on(ProjectSection::ErcAnnotation);
    assert!(b.data().rules.is_none());
    let project_id = b.id;
    b.tab(ProjectTabMsg::ErcRulesStoreRequested { project_id });
    assert!(b.data().rules.is_some());
    assert!(b.dirty());
}

#[test]
fn m34_a_second_store_leaves_the_stored_rules_as_they_are() {
    let mut b = on(ProjectSection::ErcAnnotation);
    let project_id = b.id;
    b.tab(ProjectTabMsg::ErcRulesStoreRequested { project_id });
    let once = b.data().rules.clone();
    b.tab(ProjectTabMsg::ErcRulesStoreRequested { project_id });
    assert!(b.data().rules == once);
}

/// Ctrl+Z in the tab where Store was pressed takes the store back, as
/// `undo document=<.snxprj>` does (one command id, one behaviour, 2026-11-02).
#[test]
fn m34_undo_in_the_tab_takes_back_store_erc_rules() {
    let mut b = on(ProjectSection::ErcAnnotation);
    let project_id = b.id;
    b.tab(ProjectTabMsg::ErcRulesStoreRequested { project_id });
    b.undo();
    assert!(b.data().rules.is_none(), "the store is undone");
    assert!(!b.dirty());
}

#[test]
fn m34_open_electrical_rules_check_opens_the_dialog() {
    let mut b = on(ProjectSection::ErcAnnotation);
    let project_id = b.id;
    b.tab(ProjectTabMsg::ErcDialogRequested { project_id });
    assert!(b.app.ui_state.erc_dialog_open);
}

/// The repository exists, as a finished Enable Version Control leaves it.
fn with_repository() -> Bench {
    let mut b = on(ProjectSection::VersionControl);
    let project_dir = PathBuf::from(&b.data().dir);
    // The finish message re-probes the folder, so the repository must exist.
    git2::Repository::init(&b.dir).expect("init the repository");
    b.send(Message::EnableVersionControl(
        EnableVersionControlMsg::Finished {
            scope: VersionControlScope::Project,
            project_dir,
            outcome: EnableVersionControlOutcome::Created {
                adapter_error: None,
                commit_warning: None,
            },
        },
    ));
    assert!(b.data().enable_git);
    b
}

fn toggle(b: &mut Bench, on: bool) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::VersionControlToggled { project_id, on });
}

#[test]
fn m35_commit_every_save_cannot_be_turned_on_without_a_repository() {
    let mut b = on(ProjectSection::VersionControl);
    toggle(&mut b, true);
    assert!(!b.data().enable_git);
    assert!(!b.dirty());
}

#[test]
fn m35_enable_version_control_opens_the_dialog() {
    let mut b = on(ProjectSection::VersionControl);
    let project_id = b.id;
    b.tab(ProjectTabMsg::EnableVersionControlRequested { project_id });
    assert!(b.app.ui_state.enable_version_control.is_some());
}

#[test]
fn m35_turning_commit_every_save_off_keeps_the_repository_and_undo_turns_it_on() {
    let mut b = with_repository();
    toggle(&mut b, false);
    assert!(!b.data().enable_git);
    b.undo();
    assert!(b.data().enable_git);
}

#[test]
fn m35_with_a_repository_commit_every_save_turns_back_on() {
    let mut b = with_repository();
    toggle(&mut b, false);
    toggle(&mut b, true);
    assert!(b.data().enable_git);
}
