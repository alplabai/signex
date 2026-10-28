//! #1351 - a preference toggled from `update` is written by the `Task` it
//! returns, never inline: the file is untouched until that task runs.

use signex_types::coord::Unit;

use super::*;

/// Run `task` the way the iced runtime would and hand every message it
/// yields to `on_message` as it arrives, so a test can look at the file at
/// that exact moment.
fn run_task_observing(task: iced::Task<Message>, mut on_message: impl FnMut(&Message)) {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                on_message(&message);
            }
        }
    });
}

fn run_task(task: iced::Task<Message>) {
    run_task_observing(task, |_| {});
}

/// The Save tests all drive `autosave_enabled` in the one process-wide
/// prefs file, so they take turns.
static AUTOSAVE_KEY: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn autosave_key() -> std::sync::MutexGuard<'static, ()> {
    AUTOSAVE_KEY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn save_message() -> Message {
    Message::Preferences(PreferencesMsg::Inner(crate::preferences::PrefMsg::Save))
}

fn is_save_writes_finished(message: &Message) -> bool {
    matches!(
        message,
        Message::Preferences(PreferencesMsg::Inner(
            crate::preferences::PrefMsg::SaveWritesFinished
        ))
    )
}

#[test]
fn cycling_the_unit_leaves_prefs_json_to_the_returned_task() {
    let (mut app, _task) = Signex::new();
    crate::fonts::write_unit_pref(Unit::Mm);
    app.ui_state.unit = Unit::Mm;

    let task = app.dispatch_update(Message::Ui(UiMsg::UnitCycled));

    assert_eq!(app.ui_state.unit, Unit::Mil, "the model changes at once");
    assert_eq!(
        crate::fonts::read_unit_pref(),
        Unit::Mm,
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1, "the write is handed back as a Task");

    run_task(task);
    assert_eq!(
        crate::fonts::read_unit_pref(),
        Unit::Mil,
        "the returned task writes the new unit"
    );
}

#[test]
fn setting_the_grid_size_leaves_prefs_json_to_the_returned_task() {
    let (mut app, _task) = Signex::new();
    crate::fonts::write_grid_size_mm_pref(1.27);

    let task = app.dispatch_update(Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::SetGridSize(5.08),
    )));

    assert_eq!(app.ui_state.grid_size_mm, 5.08);
    assert_eq!(
        crate::fonts::read_grid_size_mm_pref(),
        Some(1.27),
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1);

    run_task(task);
    assert_eq!(
        crate::fonts::read_grid_size_mm_pref(),
        Some(5.08),
        "the returned task writes the new grid size"
    );
}

#[test]
fn saving_preferences_leaves_prefs_json_to_the_returned_tasks() {
    let _turn = autosave_key();
    let (mut app, _task) = Signex::new();
    crate::fonts::write_autosave_enabled_pref(false);
    app.ui_state.autosave_enabled = false;
    app.ui_state.preferences_draft_autosave_enabled = true;

    let task = app.dispatch_update(Message::Preferences(PreferencesMsg::Inner(
        crate::preferences::PrefMsg::Save,
    )));

    assert!(app.ui_state.autosave_enabled, "the model changes at once");
    assert!(
        !crate::fonts::read_autosave_enabled_pref(),
        "Save must not write prefs.json inline"
    );
    assert!(task.units() >= 1, "the writes are handed back as Tasks");

    run_task(task);
    assert!(
        crate::fonts::read_autosave_enabled_pref(),
        "the Save tasks write the committed value"
    );
}

#[test]
fn clearing_recent_projects_leaves_prefs_json_to_the_returned_task() {
    let (mut app, _task) = Signex::new();
    let seeded = vec![std::path::PathBuf::from("/signex-1351/a.snxprj")];
    crate::fonts::write_recent_projects(&seeded);
    app.ui_state.recent_projects =
        crate::recent_projects::RecentProjects::from_paths(seeded.clone());

    let task = app.handle_clear_recent_projects();

    assert!(app.ui_state.recent_projects.entries().is_empty());
    assert_eq!(
        crate::fonts::read_recent_projects(),
        seeded,
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1);

    run_task(task);
    assert!(
        crate::fonts::read_recent_projects().is_empty(),
        "the returned task writes the cleared list"
    );
}

#[test]
fn adding_a_custom_filter_preset_leaves_prefs_json_to_the_returned_task() {
    let (mut app, _task) = Signex::new();
    app.interaction_state.custom_filter_presets.clear();
    crate::fonts::write_custom_filter_presets(&[]);

    let task = app.dispatch_update(Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::AddCustomFilterPreset,
    )));

    assert_eq!(app.interaction_state.custom_filter_presets.len(), 1);
    assert!(
        crate::fonts::read_custom_filter_presets().is_empty(),
        "update must not write prefs.json inline"
    );
    assert!(task.units() >= 1);

    run_task(task);
    assert_eq!(
        crate::fonts::read_custom_filter_presets().len(),
        1,
        "the returned task writes the new preset"
    );
}

#[test]
fn a_save_writes_the_value_committed_at_dispatch_not_a_later_one() {
    let _turn = autosave_key();
    let (mut app, _task) = Signex::new();
    crate::fonts::write_autosave_enabled_pref(false);
    app.ui_state.autosave_enabled = false;
    app.ui_state.preferences_draft_autosave_enabled = true;

    let task = app.dispatch_update(save_message());
    // The user flips the setting again before the worker gets to run.
    app.ui_state.autosave_enabled = false;
    run_task(task);

    assert!(
        crate::fonts::read_autosave_enabled_pref(),
        "the write carries the snapshot taken when Save was dispatched"
    );
}

#[test]
fn save_writes_finished_runs_only_after_the_writes_landed() {
    let _turn = autosave_key();
    let (mut app, _task) = Signex::new();
    crate::fonts::write_autosave_enabled_pref(false);
    app.ui_state.autosave_enabled = false;
    app.ui_state.preferences_draft_autosave_enabled = true;

    let task = app.dispatch_update(save_message());
    let mut probed = false;
    run_task_observing(task, |message| {
        if is_save_writes_finished(message) {
            probed = true;
            assert!(
                crate::fonts::read_autosave_enabled_pref(),
                "the file-health probe must not run before the writes finished"
            );
        }
    });

    assert!(probed, "Save chains the probe after its writes");
}

/// Save, then quit at once: the quit path must not release `next` (which
/// ends in `iced::exit()`) until the background writes have landed.
#[test]
fn quitting_right_after_save_waits_for_the_preference_writes() {
    let _turn = autosave_key();
    let (mut app, _task) = Signex::new();
    crate::fonts::write_autosave_enabled_pref(false);
    app.ui_state.autosave_enabled = false;
    app.ui_state.preferences_draft_autosave_enabled = true;

    // The prefs file stays busy until the quit path is seen waiting for the
    // writes (or, if it never waits, until it would exit), so the writes
    // cannot land first by luck of timing.
    let busy = crate::fonts::PrefsIoHold::take();
    crate::fonts::reset_pref_wait_started();
    let exit_reached = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let releaser = {
        let exit_reached = exit_reached.clone();
        std::thread::spawn(move || {
            while !crate::fonts::pref_wait_started()
                && !exit_reached.load(std::sync::atomic::Ordering::SeqCst)
            {
                std::thread::yield_now();
            }
            busy.release();
        })
    };
    let save = app.dispatch_update(save_message());
    let exit_marker =
        Message::Preferences(PreferencesMsg::Inner(crate::preferences::PrefMsg::Noop));
    let quit = app.flush_quit_writes_then(iced::Task::done(exit_marker));
    let mut exited = false;
    run_task_observing(iced::Task::batch([save, quit]), |message| {
        if matches!(
            message,
            Message::Preferences(PreferencesMsg::Inner(crate::preferences::PrefMsg::Noop))
        ) {
            exited = true;
            exit_reached.store(true, std::sync::atomic::Ordering::SeqCst);
            assert!(
                crate::fonts::read_autosave_enabled_pref(),
                "the app would exit here: the saved value must already be on disk"
            );
        }
    });

    assert!(exited, "the quit chain reaches its exit step");
    releaser.join().expect("releaser thread");
}
