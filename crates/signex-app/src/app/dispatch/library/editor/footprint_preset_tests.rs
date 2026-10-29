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
