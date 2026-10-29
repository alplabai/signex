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
    let wait_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    crate::fonts::install_wait_probe(wait_started.clone());
    let exit_reached = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let releaser = {
        let exit_reached = exit_reached.clone();
        std::thread::spawn(move || {
            while !wait_started.load(std::sync::atomic::Ordering::SeqCst)
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

fn blank_project() -> signex_types::project::ProjectData {
    signex_types::project::ProjectData {
        description: None,
        name: "p".into(),
        dir: String::new(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// hkngln #1602 item 2: right after a Save the prefs write may not have
/// landed. A library opened, mounted or created then must still start from the
/// saved values, not from what `prefs.json` holds. The Save tasks are never
/// run here, so the file keeps its old contents for the whole test.
#[test]
fn a_library_opened_right_after_save_takes_the_saved_defaults() {
    use crate::render_config::PinSelectionMode;

    let dir = tempfile::tempdir().expect("tempdir");
    let existing = dir.path().join("existing.snxlib");
    let mounted = dir.path().join("mounted.snxlib");
    let created = dir.path().join("created.snxlib");
    let fold = crate::app::CaseFold::default();
    for path in [&existing, &mounted] {
        crate::library::commands::create_library_at(
            &mut crate::library::LibraryState::default(),
            &mut blank_project(),
            path.clone(),
            false,
            false,
            fold,
        )
        .expect("fixture library");
    }

    let (mut app, _task) = Signex::new();
    let new_size = 0.635_f32;
    let new_pins = match app.ui_state.symbol_pin_selection {
        PinSelectionMode::PinOnly => PinSelectionMode::TextAndPin,
        PinSelectionMode::TextAndPin => PinSelectionMode::PinOnly,
    };
    let new_classes = vec![crate::fonts::ComponentClassEntry {
        key: "zz_1602".into(),
        label: "Zz 1602".into(),
    }];
    app.ui_state.preferences_draft_symbol_grid_size_mm = new_size;
    app.ui_state.preferences_draft_symbol_pin_selection = new_pins;
    app.ui_state.preferences_draft_component_classes = new_classes.clone();

    let _writes_not_run = app.dispatch_update(save_message());

    assert_ne!(
        crate::fonts::read_symbol_grid_size_mm_pref(),
        new_size,
        "fixture: the prefs file has not caught up"
    );

    app.library
        .open_library(existing.clone())
        .expect("open the library");
    let opened = app.library.library_at(&existing).expect("opened");
    assert_eq!(opened.display.grid_size_mm, new_size, "open: grid size");
    assert_eq!(
        opened.display.pin_selection, new_pins,
        "open: pin selection"
    );

    let prepared = crate::library::mount::prepare_mount(&mounted).expect("prepare");
    app.library.mount_prepared(prepared).expect("mount");
    let mounted_entry = app.library.library_at(&mounted).expect("mounted");
    assert_eq!(
        mounted_entry.display.grid_size_mm, new_size,
        "mount: grid size"
    );
    assert_eq!(mounted_entry.display.pin_selection, new_pins, "mount: pins");

    let id = crate::library::commands::create_library_at(
        &mut app.library,
        &mut blank_project(),
        created.clone(),
        false,
        false,
        fold,
    )
    .expect("create");
    let adapter = app.library.set.get(id).expect("created library is mounted");
    let keys: Vec<String> = adapter
        .library_classes()
        .into_iter()
        .map(|c| c.key)
        .collect();
    assert_eq!(keys, vec!["zz_1602".to_string()], "create: classes");
}
