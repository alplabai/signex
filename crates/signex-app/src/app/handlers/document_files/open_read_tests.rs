//! #859 — opening a schematic, PCB or primitive reads the file on a worker;
//! the tab is installed only by a completion that is still current.

use std::path::{Path, PathBuf};

use super::restore_commit_tests::{collect_messages, run_task};
use super::save_async_tests::empty_board;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectTreeAction, Signex, TabKind};

fn write_schematic(path: &Path) {
    let text = signex_types::format::SnxSchematic::new(signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        ..Default::default()
    })
    .write_string()
    .expect("encode");
    std::fs::write(path, text).expect("write schematic");
}

fn write_pcb(path: &Path) {
    let text = signex_types::format::SnxPcb::new(empty_board())
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write pcb");
}

fn write_symbol(path: &Path) {
    use signex_library::{Symbol, SymbolFile};
    let text = SymbolFile::from_symbol(Symbol::empty("s"))
        .to_toml_string()
        .expect("encode");
    std::fs::write(path, text).expect("write symbol");
}

fn workspace(dir: &Path) -> Signex {
    let _ = crate::diagnostics::init_logging();
    app_workspace(&dir.to_string_lossy(), &[])
}

/// Double-click `name` in the project tree: the task it returns, unrun.
fn open_from_tree(app: &mut Signex, name: &str) -> iced::Task<Message> {
    app.open_project_tree_document(&[0], name.to_string())
        .expect("accepted")
}

fn apply(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        let next = app.update(message);
        run_task(app, next);
    }
}

fn tabs_for(app: &Signex, path: &Path) -> usize {
    app.document_state
        .tabs
        .iter()
        .filter(|t| t.path == path)
        .count()
}

fn logged(app: &Signex, needle: &str) -> bool {
    app.document_state
        .panel_ctx
        .diagnostics
        .iter()
        .any(|entry| entry.message.contains(needle))
}

#[test]
fn a_tree_schematic_opens_only_when_its_read_completes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    write_schematic(&path);
    let mut app = workspace(dir.path());
    let task = open_from_tree(&mut app, "a.snxsch");
    assert!(app.document_state.tabs.is_empty(), "update() did not read");
    assert!(app.document_state.pending_opens.contains(&path));
    run_task(&mut app, task);
    assert_eq!(tabs_for(&app, &path), 1);
    let tab = &app.document_state.tabs[app.document_state.active_tab];
    assert_eq!(tab.title, "a");
    assert!(matches!(tab.kind, TabKind::Schematic));
    assert!(!tab.dirty);
    assert!(app.document_state.engines.contains_key(&path));
    assert!(!app.document_state.pending_opens.contains(&path));
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn a_tree_pcb_opens_only_when_its_read_completes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("b.snxpcb");
    write_pcb(&path);
    let mut app = workspace(dir.path());
    let task = open_from_tree(&mut app, "b.snxpcb");
    assert!(app.document_state.tabs.is_empty());
    run_task(&mut app, task);
    assert_eq!(tabs_for(&app, &path), 1);
    assert!(matches!(app.document_state.tabs[0].kind, TabKind::Pcb));
    assert_eq!(app.document_state.tabs[0].title, "b");
}

#[test]
fn a_corrupt_tree_schematic_logs_the_reason_before_the_path_and_opens_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bad.snxsch");
    std::fs::write(&path, "not a schematic").expect("write");
    let mut app = workspace(dir.path());
    let task = open_from_tree(&mut app, "bad.snxsch");
    run_task(&mut app, task);
    assert!(app.document_state.tabs.is_empty(), "no half-opened tab");
    assert!(!app.document_state.pending_opens.contains(&path));
    assert!(
        app.document_state.error_notice.is_none(),
        "log only, as before"
    );
    assert!(logged(&app, &format!("parse schematic {}", path.display())));
    assert!(logged(&app, "Failed to open project tree document"));
}

#[test]
fn a_missing_tree_file_says_so_and_a_retry_is_possible() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("gone.snxpcb");
    let mut app = workspace(dir.path());
    let task = open_from_tree(&mut app, "gone.snxpcb");
    run_task(&mut app, task);
    assert!(app.document_state.tabs.is_empty());
    assert!(logged(
        &app,
        &format!("project tree file does not exist: {}", path.display())
    ));
    write_pcb(&path);
    let task = open_from_tree(&mut app, "gone.snxpcb");
    run_task(&mut app, task);
    assert_eq!(tabs_for(&app, &path), 1, "not stuck loading");
}

#[test]
fn opening_the_same_tree_file_twice_concurrently_makes_one_tab() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    write_schematic(&path);
    let mut app = workspace(dir.path());
    let first = open_from_tree(&mut app, "a.snxsch");
    let second = open_from_tree(&mut app, "a.snxsch");
    assert_eq!(first.units(), 1);
    assert_eq!(second.units(), 0, "the second click spawns no read");
    run_task(&mut app, first);
    run_task(&mut app, second);
    assert_eq!(tabs_for(&app, &path), 1);
}

#[test]
fn closing_all_documents_makes_an_in_flight_open_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    write_schematic(&path);
    let mut app = workspace(dir.path());
    let messages = collect_messages(open_from_tree(&mut app, "a.snxsch"));
    let close = app.handle_project_tree_action(ProjectTreeAction::CloseAllDocuments);
    run_task(&mut app, close);
    apply(&mut app, messages);
    assert!(
        app.document_state.tabs.is_empty(),
        "the stale read is dropped"
    );
    assert!(!app.document_state.engines.contains_key(&path));
    let task = open_from_tree(&mut app, "a.snxsch");
    run_task(&mut app, task);
    assert_eq!(tabs_for(&app, &path), 1, "a fresh open still works");
}

#[test]
fn closing_the_project_makes_an_in_flight_open_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    write_schematic(&path);
    let mut app = workspace(dir.path());
    let messages = collect_messages(open_from_tree(&mut app, "a.snxsch"));
    let close = app.handle_project_tree_action(ProjectTreeAction::CloseProject(vec![0]));
    run_task(&mut app, close);
    assert!(app.document_state.projects.is_empty(), "project closed");
    apply(&mut app, messages);
    assert!(app.document_state.tabs.is_empty());
}

#[test]
fn a_reopen_after_a_cancel_is_not_overwritten_by_the_old_completion() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    write_schematic(&path);
    let mut app = workspace(dir.path());
    let old = collect_messages(open_from_tree(&mut app, "a.snxsch"));
    app.cancel_in_flight_reads();
    let new = open_from_tree(&mut app, "a.snxsch");
    apply(&mut app, old);
    assert!(app.document_state.tabs.is_empty(), "old generation dropped");
    assert!(
        app.document_state.pending_opens.contains(&path),
        "the new claim survives the old completion"
    );
    run_task(&mut app, new);
    assert_eq!(tabs_for(&app, &path), 1);
}

#[test]
fn a_tab_opened_by_another_route_meanwhile_is_not_duplicated() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    write_schematic(&path);
    let mut app = workspace(dir.path());
    let messages = collect_messages(open_from_tree(&mut app, "a.snxsch"));
    app.open_schematic_tab(path.clone(), "a".into(), Default::default());
    apply(&mut app, messages);
    assert_eq!(tabs_for(&app, &path), 1);
}

#[test]
fn a_tree_symbol_opens_on_completion_and_a_corrupt_one_raises_the_card() {
    let dir = tempfile::tempdir().expect("tempdir");
    let good = dir.path().join("g.snxsym");
    let bad = dir.path().join("b.snxsym");
    write_symbol(&good);
    std::fs::write(&bad, "not a symbol").expect("write");
    let mut app = workspace(dir.path());

    let task = open_from_tree(&mut app, "g.snxsym");
    assert!(app.document_state.tabs.is_empty());
    run_task(&mut app, task);
    assert_eq!(tabs_for(&app, &good), 1);
    assert!(app.document_state.symbol_editors.contains_key(&good));
    assert_eq!(app.document_state.active_path, None);

    let task = open_from_tree(&mut app, "b.snxsym");
    run_task(&mut app, task);
    assert_eq!(tabs_for(&app, &bad), 0, "no half-opened tab");
    assert!(!app.document_state.symbol_editors.contains_key(&bad));
    let card = app.document_state.error_notice.as_ref().expect("card");
    assert!(
        card.detail
            .starts_with(&format!("could not parse {}", bad.display())),
        "{}",
        card.detail
    );
    assert!(logged(&app, "Failed to open primitive"));
}

#[test]
fn a_primitive_open_is_deduplicated_and_dropped_when_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("g.snxsym");
    write_symbol(&path);
    let mut app = workspace(dir.path());
    let first = app.handle_open_primitive(path.clone());
    let second = app.handle_open_primitive(path.clone());
    assert_eq!((first.units(), second.units()), (1, 0));
    let messages = collect_messages(first);
    app.cancel_in_flight_reads();
    apply(&mut app, messages);
    assert!(app.document_state.tabs.is_empty());
    assert!(app.document_state.symbol_editors.is_empty());
}

#[test]
fn a_tree_footprint_opens_on_completion() {
    use signex_library::{Footprint, FootprintFile};
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("f.snxfpt");
    let text = FootprintFile::from_footprint(Footprint::empty("f"))
        .to_toml_string()
        .expect("encode");
    std::fs::write(&path, text).expect("write");
    let mut app = workspace(dir.path());
    let task = open_from_tree(&mut app, "f.snxfpt");
    assert!(app.document_state.tabs.is_empty());
    run_task(&mut app, task);
    assert!(app.document_state.footprint_editors.contains_key(&path));
    assert!(matches!(
        app.document_state.tabs[0].kind,
        TabKind::FootprintEditor(_)
    ));
}
