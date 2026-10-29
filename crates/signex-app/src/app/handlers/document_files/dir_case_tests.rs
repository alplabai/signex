//! #1447 — open and Ctrl+S decide "one file or two" from the directory's
//! probed case rule, folding until it answers.

use std::path::{Path, PathBuf};

use crate::app::state::project_case::CaseProbeOutcome;
use crate::app::{FileMsg, Message, Signex};

const DIR: &str = "/w/loose-1447";

fn sheet() -> signex_types::schematic::SchematicSheet {
    signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    }
}

fn held() -> PathBuf {
    PathBuf::from(DIR).join("Lib.snxsch")
}

fn variant() -> PathBuf {
    PathBuf::from(DIR).join("lib.snxsch")
}

/// A loose `Lib.snxsch` open in a tab with another tab focused, no project.
fn app_holding_loose_lib() -> Signex {
    let (mut app, _task) = Signex::new();
    app.open_schematic_tab(held(), "Lib".to_string(), sheet());
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "Other".to_string(),
        path: PathBuf::from(DIR).join("Other.snxsch"),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app
}

pub(super) fn probed(app: &mut Signex, answer: CaseProbeOutcome) {
    let _task = app.update(Message::File(FileMsg::DirCaseProbed {
        dir: PathBuf::from(DIR),
        answer,
    }));
}

fn request_open(app: &mut Signex, path: PathBuf) {
    let _task: iced::Task<Message> = app.handle_document_file_opened(Some(path));
}

fn finish_open(app: &mut Signex, path: PathBuf) {
    let _task = app.update(Message::File(FileMsg::SchematicOpenFinished {
        title: "Lib".to_string(),
        path,
        result: Ok(Box::new(sheet())),
    }));
}

fn active_path(app: &Signex) -> &Path {
    &app.document_state.tabs[app.document_state.active_tab].path
}

#[test]
fn opening_a_case_variant_focuses_the_held_tab_when_the_directory_folds() {
    let mut app = app_holding_loose_lib();
    probed(&mut app, CaseProbeOutcome::Decided(true));

    request_open(&mut app, variant());

    assert_eq!(
        app.document_state.tabs.len(),
        2,
        "no second tab for one file"
    );
    assert_eq!(active_path(&app), held());
    assert!(app.document_state.pending_opens.is_empty());
}

#[test]
fn opening_a_case_variant_waits_for_an_unprobed_directory() {
    let mut app = app_holding_loose_lib();

    request_open(&mut app, variant());
    request_open(&mut app, variant());

    assert_eq!(app.document_state.tabs.len(), 2, "nothing opened yet");
    assert!(
        app.document_state.pending_opens.contains(&variant()),
        "the request is marked so a repeat is blocked"
    );
    assert!(!app.document_state.engines.contains_key(&variant()));

    // The probe lands: the directory folds, so the waiting open focuses the
    // held tab instead of opening a copy.
    probed(&mut app, CaseProbeOutcome::Decided(true));

    assert_eq!(app.document_state.tabs.len(), 2);
    assert_eq!(active_path(&app), held());
    assert!(app.document_state.pending_opens.is_empty());
}

#[test]
fn opening_a_case_variant_opens_two_tabs_when_the_directory_preserves_case() {
    let mut app = app_holding_loose_lib();
    request_open(&mut app, variant());

    probed(&mut app, CaseProbeOutcome::Decided(false));
    assert!(
        app.document_state.pending_opens.contains(&variant()),
        "the open went ahead: its read is in flight"
    );
    finish_open(&mut app, variant());

    assert_eq!(app.document_state.tabs.len(), 3, "a second tab for lib");
    assert_eq!(active_path(&app), variant());
}

/// Counts the writes a save issues, performing each for real.
#[derive(Default)]
struct CountingSink(std::sync::Mutex<Vec<PathBuf>>);

impl crate::persist::DocumentSink for CountingSink {
    fn write(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        self.0.lock().unwrap().push(path.to_path_buf());
        std::fs::write(path, bytes)
    }
}

fn count_writes(app: &mut Signex) -> std::sync::Arc<CountingSink> {
    let sink = std::sync::Arc::new(CountingSink::default());
    app.document_state.document_saves = crate::persist::DocumentSaves::new(sink.clone());
    sink
}

/// `Lib.snxsch` and `lib.snxsch` open as two dirty tabs in `dir`, `Lib`
/// focused. With `edit_second` the second spelling holds different edits.
fn app_with_dirty_variants(dir: &Path, edit_second: bool) -> (Signex, PathBuf, PathBuf) {
    let (mut app, _task) = Signex::new();
    let (upper, lower) = (dir.join("Lib.snxsch"), dir.join("lib.snxsch"));
    let second_paper = if edit_second { "A3" } else { "A4" };
    for (path, paper) in [(&upper, "A4"), (&lower, second_paper)] {
        let sheet = signex_types::schematic::SchematicSheet {
            paper_size: paper.to_string(),
            ..Default::default()
        };
        app.open_schematic_tab(path.clone(), "Lib".to_string(), sheet);
        app.document_state.mark_document_dirty(path);
    }
    app.activate_tab_for_path(&upper);
    (app, upper, lower)
}

fn probe_dir(app: &mut Signex, dir: &Path, answer: CaseProbeOutcome) -> iced::Task<Message> {
    app.update(Message::File(FileMsg::DirCaseProbed {
        dir: dir.to_path_buf(),
        answer,
    }))
}

fn feed(app: &mut Signex, task: iced::Task<Message>) {
    for message in crate::persist::run_task::outputs(task) {
        let _ = app.update(message);
    }
}

fn tab_dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.is_document_dirty(path)
}

#[test]
fn ctrl_s_refuses_when_another_spelling_holds_different_edits() {
    let _ = crate::diagnostics::init_logging();
    // A long directory name stands in for a long CI temp path: the Messages
    // panel clips an entry, so the advice must lead and survive any path.
    let dir = tempfile::Builder::new()
        .prefix(&format!("dircase-refusal-{}", "x".repeat(120)))
        .tempdir()
        .expect("tempdir");
    let (mut app, upper, lower) = app_with_dirty_variants(dir.path(), true);
    let sink = count_writes(&mut app);
    let _ = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Decided(true));
    let refusals = || {
        crate::diagnostics::recent_entries()
            .iter()
            .filter(|e| {
                e.message.contains("Failed to save active document")
                    && e.message.contains("Nothing was saved")
            })
            .count()
    };
    let before = refusals();

    let task = app.handle_active_document_save_requested();
    feed(&mut app, task);

    assert!(sink.0.lock().unwrap().is_empty(), "nothing was written");
    assert!(tab_dirty(&app, &upper) && tab_dirty(&app, &lower));
    assert!(app.document_state.is_document_dirty(&upper));
    assert!(app.document_state.is_document_dirty(&lower));
    assert_eq!(
        refusals(),
        before + 1,
        "one refusal is one Messages line, advice first"
    );
}

#[test]
fn ctrl_s_writes_once_when_case_variants_agree() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_dirty_variants(dir.path(), false);
    let sink = count_writes(&mut app);
    let _ = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Decided(true));

    let task = app.handle_active_document_save_requested();
    feed(&mut app, task);

    assert_eq!(*sink.0.lock().unwrap(), vec![upper.clone()]);
    assert!(!tab_dirty(&app, &upper));
    assert!(
        tab_dirty(&app, &lower),
        "the other tab's marker is left alone"
    );
}

#[test]
fn ctrl_s_probes_first_when_two_spellings_are_held_in_an_unprobed_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, _lower) = app_with_dirty_variants(dir.path(), true);
    let sink = count_writes(&mut app);

    let task = app.handle_active_document_save_requested();
    drop(task);
    assert!(
        sink.0.lock().unwrap().is_empty(),
        "no write before the probe"
    );

    // A volume that keeps the spellings apart: they are two files, so the
    // waiting save writes the focused one and refuses nothing.
    let resumed = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Decided(false));
    feed(&mut app, resumed);

    assert_eq!(*sink.0.lock().unwrap(), vec![upper]);
}

#[test]
fn a_probe_revealing_two_engines_on_one_file_warns_and_keeps_both_dirty() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    // Opened while the directory's rule was unknown to the caller, so both
    // spellings got their own engine.
    let (mut app, upper, lower) = app_with_dirty_variants(dir.path(), true);

    let _ = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Decided(true));
    // A repeated answer is ignored, warning included.
    let _ = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Decided(true));

    // The phrase plus this test's own temp path: the ring is shared with
    // parallel tests, and the entry is clipped before the second path.
    let needle = format!(
        "One file open as two documents; saving is refused while they differ, close one: {}",
        upper.display()
    );
    assert_eq!(crate::diagnostics::logged_count(&needle), 1, "warned once");
    assert_eq!(app.document_state.tabs.len(), 2, "neither tab was closed");
    assert!(tab_dirty(&app, &upper) && tab_dirty(&app, &lower));
    assert!(app.document_state.engines.contains_key(&upper));
    assert!(app.document_state.engines.contains_key(&lower));
}

#[test]
fn an_undecided_directory_probe_is_reported_once_and_folds() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_dirty_variants(dir.path(), false);
    let shown = dir.path().display().to_string();

    let _ = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Undecided);
    let _ = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Undecided);

    assert_eq!(
        crate::diagnostics::logged_count(&format!("assuming case-insensitive: {shown}")),
        1,
        "reported once per directory, saying what was assumed"
    );
    assert!(app.identity_fold_for(&lower).fold_case, "assumed to fold");
    let third = dir.path().join("LIB.snxsch");
    assert_eq!(
        app.held_spelling(&third),
        Some(upper),
        "a third spelling folds"
    );
}

#[test]
fn a_loose_symbol_tab_starts_a_probe_and_a_case_variant_opens_apart_once_it_says_so() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let held = dir.path().join("R.snxsym");
    let variant = dir.path().join("r.snxsym");
    // Only `r.snxsym` exists: on a case-sensitive volume it is another file.
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("r"));
    std::fs::write(&variant, file.to_toml_string().expect("serialize")).expect("write symbol");
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "R".to_string(),
        path: held.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::SymbolEditor(held.clone()),
    });
    app.document_state.active_tab = 0;

    // Whatever attached the tab, the next update starts the directory's probe.
    let _probe = app.warm_document_dir_probes();
    assert!(app.document_state.dir_case.is_probing(dir.path()));

    // Before the answer the variant waits; it is not folded into `R`.
    request_open(&mut app, variant.clone());
    assert!(app.document_state.pending_opens.contains(&variant));
    assert_eq!(app.document_state.tabs.len(), 1);

    let resumed = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Decided(false));
    feed(&mut app, resumed);

    assert_eq!(app.document_state.tabs.len(), 2, "a second editor opened");
    assert_eq!(active_path(&app), variant);
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn an_undecided_project_probe_still_folds_for_identity_and_keeps_the_host_key_rule() {
    use crate::app::ProjectMsg;
    let mut app = crate::app::handlers::menu::export::tests::app_workspace("/w/proj-1447", &[]);
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::CaseProbed {
        project_id,
        outcome: CaseProbeOutcome::Undecided,
    }));
    let path = Path::new("/w/proj-1447/Lib.snxsch");

    assert!(app.identity_fold_for(path).fold_case, "identity folds");
    assert_eq!(
        app.fold_for_path(path),
        crate::app::state::scope::CaseFold::host_default(),
        "the key rule stays the host default"
    );
}

#[test]
fn the_advice_leads_every_line_that_names_paths() {
    let long = PathBuf::from("/a/very/long/folder/".repeat(6)).join("Lib.snxsch");
    let other = PathBuf::from("/a/very/long/folder/".repeat(6)).join("lib.snxsch");
    let lines = [
        super::dir_case::aliased_documents_message(&[&long, &other]),
        super::dir_case::save_skipped_message(&long),
        super::spelling::assumed_fold_message(&long, &other),
    ];
    for line in lines {
        let first_path = line.find("/a/very").expect("names a path");
        assert!(first_path < 100, "paths come last: {line}");
    }
}

#[test]
fn a_deferred_ctrl_s_that_lost_focus_says_nothing_was_saved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_dirty_variants(dir.path(), false);
    let sink = count_writes(&mut app);
    drop(app.handle_active_document_save_requested());

    app.activate_tab_for_path(&lower);
    let resumed = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Decided(true));
    feed(&mut app, resumed);

    assert!(sink.0.lock().unwrap().is_empty());
    assert!(
        app.document_state.error_notice.is_some(),
        "the Save Failed card says nothing was written"
    );
    assert!(tab_dirty(&app, &upper));
}
