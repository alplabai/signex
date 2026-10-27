//! #1447 — open and Ctrl+S decide "one file or two" from the directory's
//! probed case rule, folding until it answers.

use std::path::{Path, PathBuf};

use crate::app::state::project_case::CaseProbeOutcome;
use crate::app::{FileMsg, Message, Signex};

const DIR: &str = "/w/loose-1447";

fn sheet() -> signex_types::schematic::SchematicSheet {
    signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
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
        dirty: false,
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
        app.document_state.dirty_paths.insert(path.clone());
        for tab in app
            .document_state
            .tabs
            .iter_mut()
            .filter(|t| t.path == *path)
        {
            tab.dirty = true;
        }
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
    app.document_state
        .tabs
        .iter()
        .find(|t| t.path == path)
        .is_some_and(|t| t.dirty)
}

#[test]
fn ctrl_s_refuses_when_another_spelling_holds_different_edits() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, upper, lower) = app_with_dirty_variants(dir.path(), true);
    let sink = count_writes(&mut app);
    let _ = probe_dir(&mut app, dir.path(), CaseProbeOutcome::Decided(true));
    let pair = format!("{} and {}", upper.display(), lower.display());
    let named_by_probe = crate::diagnostics::logged_count(&pair);

    let task = app.handle_active_document_save_requested();
    feed(&mut app, task);

    assert!(sink.0.lock().unwrap().is_empty(), "nothing was written");
    assert!(tab_dirty(&app, &upper) && tab_dirty(&app, &lower));
    assert!(app.document_state.dirty_paths.contains(&upper));
    assert!(app.document_state.dirty_paths.contains(&lower));
    // Both spellings sit in one message; the temp dir makes the needle unique.
    // The Messages panel clips an entry, so the pair is the needle; the
    // probe's own warning names it too, hence before and after.
    assert_eq!(
        crate::diagnostics::logged_count(&pair),
        named_by_probe + 1,
        "the refusal names both spellings"
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

    let both = format!("{} and {}", upper.display(), lower.display());
    assert_eq!(crate::diagnostics::logged_count(&both), 1, "warned once");
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
