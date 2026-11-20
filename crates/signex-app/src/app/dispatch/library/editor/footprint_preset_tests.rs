//! #1639 — capturing a footprint filter preset must not touch `prefs.json`
//! on the UI thread. The write rides the ordered background writer, so the
//! handler returns a `Task`, the disk changes only once that Task runs, and
//! two captures landing out of order still leave the later one on disk.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use iced::Task;

use crate::active_bar::FootprintFilterPreset;
use crate::app::{FootprintDocument, Message, Signex};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// `prefs.json` is one per-process file shared by every test in the binary;
/// these tests assert on its contents, so they take turns.
static PREFS_TURN: Mutex<()> = Mutex::new(());

fn take_turn() -> MutexGuard<'static, ()> {
    PREFS_TURN.lock().unwrap_or_else(|p| p.into_inner())
}

fn app_with_footprint_tab() -> (Signex, PathBuf) {
    let (mut app, _task) = Signex::new();
    let path = PathBuf::from("/w/part.snxfpt");
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.interaction_state.footprint_filter_presets = Vec::new();
    (app, path)
}

fn capture(app: &mut Signex, path: &Path) -> Task<Message> {
    app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::CaptureFilterPreset),
    }))
}

fn on_disk() -> Vec<FootprintFilterPreset> {
    crate::fonts::read_footprint_filter_presets_from(crate::fonts::load_prefs_json().as_ref())
}

fn names(presets: &[FootprintFilterPreset]) -> Vec<String> {
    presets.iter().map(|p| p.name.clone()).collect()
}

fn run(task: Task<Message>) {
    use futures::StreamExt;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async { while stream.next().await.is_some() {} });
}

#[test]
fn capturing_a_preset_returns_a_task_and_leaves_prefs_json_alone_until_it_runs() {
    let _turn = take_turn();
    let (mut app, path) = app_with_footprint_tab();
    let before = names(&on_disk());

    let task = capture(&mut app, &path);

    assert!(task.units() >= 1, "the write must be a Task, not inline IO");
    assert_eq!(
        names(&on_disk()),
        before,
        "update must not write prefs.json"
    );
    assert_eq!(
        names(&app.interaction_state.footprint_filter_presets),
        ["Filter 1"],
        "the chip shows from in-memory state before the write lands"
    );

    run(task);

    assert_eq!(names(&on_disk()), ["Filter 1"]);
}

#[test]
fn the_preset_list_comes_from_memory_not_from_prefs_json() {
    let _turn = take_turn();
    let (mut app, path) = app_with_footprint_tab();
    // Disk holds one stale preset; memory holds none. A disk read would
    // name the capture "Filter 2".
    crate::fonts::write_footprint_filter_presets(&[FootprintFilterPreset {
        name: "stale".into(),
        kinds: Vec::new(),
    }]);

    run(capture(&mut app, &path));

    assert_eq!(names(&on_disk()), ["Filter 1"]);
}

#[test]
fn back_to_back_captures_leave_the_later_one_on_disk_whatever_the_run_order() {
    let _turn = take_turn();
    let (mut app, path) = app_with_footprint_tab();
    crate::fonts::write_footprint_filter_presets(&[]);

    let first = capture(&mut app, &path);
    let second = capture(&mut app, &path);
    assert!(first.units() >= 1 && second.units() >= 1);
    // The later write reaches its worker first; the older one must not
    // then overwrite it.
    run(second);
    run(first);

    assert_eq!(names(&on_disk()), ["Filter 1", "Filter 2"]);
}

/// `app_with_footprint_tab` with the footprint's tab active.
fn app_with_active_footprint_tab() -> Signex {
    let (mut app, path) = app_with_footprint_tab();
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "part".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app
}

/// The Custom Selection Filter dialog's Apply, with `name` typed and only
/// Pads ticked when `pads_only`.
fn apply_custom(app: &mut Signex, name: &str, pads_only: bool) {
    use crate::app::SelectionFilterMsg;
    let _ = app.update(Message::SelectionFilter(SelectionFilterMsg::OpenCustom));
    let draft = app
        .ui_state
        .selection_filter_custom
        .as_mut()
        .expect("the dialog opens on the footprint editor");
    draft.preset_name = name.to_string();
    if pads_only {
        draft.set_all_shown(false);
        draft.pads = true;
    }
    let _ = app.update(Message::SelectionFilter(SelectionFilterMsg::ApplyCustom));
}

/// Messages lines holding `needle`; installs the logger first, since each
/// test may run in its own process.
fn logged(needle: &str) -> usize {
    crate::diagnostics::logged_count(needle)
}

#[test]
fn applying_the_same_filter_again_adds_no_preset() {
    let mut app = app_with_active_footprint_tab();
    apply_custom(&mut app, "", false);
    apply_custom(&mut app, "", false);
    apply_custom(&mut app, "", false);
    assert_eq!(
        names(&app.interaction_state.footprint_filter_presets),
        ["Filter 1"],
        "no endless Filter N"
    );
    apply_custom(&mut app, "", true);
    assert_eq!(
        names(&app.interaction_state.footprint_filter_presets),
        ["Filter 1", "Filter 2"],
        "other kinds are a new preset"
    );
}

#[test]
fn a_typed_name_at_the_preset_limit_is_said_in_messages() {
    let mut app = app_with_active_footprint_tab();
    app.interaction_state.footprint_filter_presets = (0
        ..crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT)
        .map(|n| FootprintFilterPreset {
            name: format!("Full {n}"),
            kinds: Vec::new(),
        })
        .collect();
    let before = logged("Dropped at the limit");
    apply_custom(&mut app, "Dropped at the limit", true);
    assert_eq!(
        app.interaction_state.footprint_filter_presets.len(),
        crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT
    );
    assert_eq!(
        logged("Dropped at the limit"),
        before + 1,
        "the dropped name is named"
    );
}
