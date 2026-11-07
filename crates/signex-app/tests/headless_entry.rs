//! #1933: the public headless entry. Boot without a window under a
//! redirected config root, with an explicit ERC rule profile, run commands by
//! id and read what happened. The override is process-wide, so each scenario
//! runs in a child process of this binary ([`probe`]).

#![allow(
    clippy::disallowed_methods,
    reason = "the test re-runs its own binary as a child process"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_app::app::Signex;
use signex_app::config_root::{default_config_root, set_config_root_override};
use signex_app::fonts::catalog::{FontSet, install_before_boot};
use signex_app::headless::{CommandArgs, CommandOutcome, HeadlessBootError};
use signex_app::keymap::AppCommandId;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ErcRules, RuleProfile};

const PROBE_ENV: &str = "SIGNEX_HEADLESS_ENTRY_PROBE";

fn run_probe(scenario: &str) {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, scenario)
        .output()
        .expect("run the probe");
    assert!(
        output.status.success(),
        "scenario {scenario} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_headless_boot_runs_commands_by_id_and_leaves_the_real_config_alone() {
    run_probe("run");
}

#[test]
fn a_headless_boot_is_refused_without_a_config_root_override() {
    run_probe("no_override");
}

#[test]
fn probe() {
    let Ok(scenario) = std::env::var(PROBE_ENV) else {
        return;
    };
    match scenario.as_str() {
        "run" => run(),
        "no_override" => no_override(),
        "undo_round_trip" => undo_round_trip(),
        "undo_depth" => undo_depth(),
        "undo_depth_dispatch" => undo_depth_dispatch(),
        "net_colors" => net_colors(),
        "net_class_assign" => net_class_assign(),
        other => panic!("unknown scenario {other}"),
    }
}

#[test]
fn net_colours_run_undo_and_save_through_command_ids_alone() {
    run_probe("net_colors");
}

/// Colour a net by operands, undo, redo, clear, save: no window, and the
/// sheet's dirty state follows the history (#661).
fn net_colors() {
    use signex_types::schematic::{Point, SchematicSheet, Wire};

    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, boot).expect("boot settled");

    let path = isolated.path().join("colours.snxsch");
    let wire = Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    let wire_id = wire.uuid;
    let sheet = SchematicSheet {
        wires: vec![wire],
        ..Default::default()
    };
    let mut seed = signex_engine::Engine::new_with_path(sheet, Some(path.clone())).expect("engine");
    seed.save().expect("seed file");
    app.headless_answers()
        .expect("headless answers")
        .replace_all([("path".to_string(), path.display().to_string())]);
    let (outcome, task) = app.dispatch_command_headless(&id("open_document"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    driver.settle(&mut app, task).expect("open settled");

    let painted = |app: &Signex| {
        app.document_state.engines[&path]
            .document()
            .net_colors
            .get(&wire_id)
            .copied()
    };
    let mut run = |app: &mut Signex, name: &str, operands: &[&str]| {
        let command = id(name);
        let args = CommandArgs::parse_operands(&command, operands).expect("operands");
        let (outcome, task) = app.dispatch_command_headless(&command, args);
        driver.settle(app, task).expect("command settled");
        outcome
    };
    assert!(!app.document_state.is_document_dirty(&path));

    let ran = run(&mut app, "color_net", &["at=5,0", "color=#336699"]);
    assert_eq!(ran, CommandOutcome::Ran);
    assert_eq!(
        painted(&app),
        Some(signex_types::theme::Color::new(0x33, 0x66, 0x99, 255)),
        "the operands changed the sheet"
    );
    assert!(app.document_state.is_document_dirty(&path), "an edit");

    assert_eq!(run(&mut app, "undo", &[]), CommandOutcome::Ran);
    assert_eq!(painted(&app), None);
    assert!(
        !app.document_state.is_document_dirty(&path),
        "undo back to the saved colours is clean"
    );
    assert_eq!(run(&mut app, "redo", &[]), CommandOutcome::Ran);
    assert!(painted(&app).is_some());
    assert!(app.document_state.is_document_dirty(&path));

    assert_eq!(run(&mut app, "save_document", &[]), CommandOutcome::Ran);
    assert!(!app.document_state.is_document_dirty(&path), "saved");
    let text = std::fs::read_to_string(&path).expect("saved file");
    let saved = signex_types::format::SnxSchematic::parse(&text)
        .expect("reopens")
        .sheet;
    assert!(
        saved.net_colors.contains_key(&wire_id),
        "the file holds the colour"
    );

    let cleared = run(&mut app, "clear_all_net_colors", &[]);
    assert_eq!(cleared, CommandOutcome::Ran);
    assert_eq!(painted(&app), None);
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(run(&mut app, "undo", &[]), CommandOutcome::Ran);
    assert!(painted(&app).is_some());
    assert!(
        !app.document_state.is_document_dirty(&path),
        "back at the save"
    );

    // Without its operands `color_net` would only open the colour picker:
    // refused with the reason, nothing coloured, never `Ran`.
    let before = app.document_state.engines[&path]
        .document()
        .net_colors
        .clone();
    let (outcome, _task) = app.dispatch_command_headless(&id("color_net"), CommandArgs::none());
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("needs the operand")),
        "{outcome:?}"
    );
    assert_eq!(
        app.document_state.engines[&path].document().net_colors,
        before
    );
    assert!(!app.ui_state.net_color_custom.show, "no picker opened");

    // A command that declares operands and has no handler is still refused.
    let refused = run(&mut app, "place_net_label", &["at=1,1", "text=A"]);
    assert!(
        matches!(&refused, CommandOutcome::Disabled { reason } if reason.contains("not run yet")),
        "{refused:?}"
    );
}

/// `unused_pin` switched off: a rule the machine profile does not change.
fn rules() -> RuleProfile {
    let mut erc = ErcRules::default();
    erc.severity.insert("unused_pin".into(), "off".into());
    RuleProfile::new("Headless", "1", erc)
}

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn no_override() {
    let refused = Signex::boot_headless(Vec::new(), &rules()).err();
    assert_eq!(refused, Some(HeadlessBootError::NoConfigRootOverride));
    // An app booted the GUI way runs nothing through the headless entry.
    let (mut app, _boot) = Signex::new();
    let grid = app.ui_state.grid_visible;
    let (outcome, _task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::NotHeadless);
    assert_eq!(app.ui_state.grid_visible, grid, "nothing ran");
    // Headless ports without the override: still refused.
    app.ui_state.io = signex_app::io_ports::IoPorts::headless(Default::default());
    let (outcome, _task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::NotHeadless);
    assert_eq!(app.ui_state.grid_visible, grid, "nothing ran");
}

fn run() {
    let real = default_config_root().expect("the test redirect resolves");
    std::fs::create_dir_all(&real).expect("real root");
    std::fs::write(real.join("prefs.json"), br#"{"grid_visible":true}"#).expect("seed");
    let before = snapshot(&real);

    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let relative = PathBuf::from("relative/demo.snxprj");
    assert_eq!(
        Signex::boot_headless(vec![relative.clone()], &rules()).err(),
        Some(HeadlessBootError::NotAbsolute(relative))
    );
    // Native dialogs: not a headless app, even with the override set.
    let (mut native, _boot) = Signex::new();
    let (outcome, _task) =
        native.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::NotHeadless);

    let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, boot).expect("boot settled");
    assert_eq!(
        app.ui_state
            .erc_severity_override
            .get(&signex_erc::RuleKind::UnusedPin),
        Some(&signex_erc::Severity::Off),
        "the explicit rule profile is in force"
    );
    assert_eq!(
        app.ui_state.io.opener,
        signex_app::io_ports::Opener::Refuse,
        "launches are refused"
    );
    // Headless dialogs but native launches: still not a headless app.
    app.ui_state.io.opener = signex_app::io_ports::Opener::Native;
    let (outcome, _task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::NotHeadless);
    app.ui_state.io.opener = signex_app::io_ports::Opener::Refuse;

    let grid_before = app.ui_state.grid_visible;
    let (outcome, task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    driver.settle(&mut app, task).expect("command settled");
    assert_eq!(app.ui_state.grid_visible, !grid_before, "the command ran");

    // One answered dialog: Open takes the `path` answer, so the open of a
    // file that is not there reports that file.
    let missing = isolated.path().join("zq-missing.snxsch");
    app.headless_answers()
        .expect("headless answers")
        .replace_all([("path".to_string(), missing.display().to_string())]);
    let (outcome, task) = app.dispatch_command_headless(&id("open_document"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    driver.settle(&mut app, task).expect("open settled");
    let notice = app
        .document_state
        .error_notice
        .take()
        .expect("the open was tried");
    assert!(
        notice.detail.contains("zq-missing.snxsch"),
        "{}",
        notice.detail
    );

    let (outcome, _task) =
        app.dispatch_command_headless(&id("zq_no_such_command"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::UnknownId);
    // ERC needs a schematic, and none is open.
    let (outcome, _task) = app.dispatch_command_headless(&id("run_erc"), CommandArgs::none());
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    // Operands parse but nothing runs them yet (#1934): refused, not a
    // silent "Ran" that only arms the tool.
    let label = id("place_net_label");
    let args = CommandArgs::parse_operands(&label, &["at=10,5", "text=VCC"]).expect("operands");
    let (outcome, _task) = app.dispatch_command_headless(&label, args);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("#1934")),
        "{outcome:?}"
    );

    let end = app.end_headless_session();
    driver.settle(&mut app, end).expect("session ended");
    assert!(driver.has_exited());
    let written = std::fs::read_to_string(isolated.path().join("prefs.json")).expect("prefs");
    assert!(written.contains("grid_visible"), "{written}");
    assert_eq!(snapshot(&real), before, "the real config root changed");
}

#[test]
fn the_snapshot_sees_a_folder_only_change() {
    let root = tempfile::tempdir().expect("root");
    let before = snapshot(root.path());
    std::fs::create_dir(root.path().join("recovery")).expect("folder");
    assert_ne!(snapshot(root.path()), before);
}

/// Every file and folder under `root`, with a file's bytes.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                out.insert(path.clone(), None);
                stack.push(path);
            } else {
                out.insert(path.clone(), Some(std::fs::read(&path).expect("read")));
            }
        }
    }
    out
}

#[test]
fn a_headless_undo_and_redo_of_a_sheet_edit_move_the_dirty_state() {
    run_probe("undo_round_trip");
}

/// Edit, undo, redo through command ids only, no window: the sheet and its
/// dirty state follow the history.
fn undo_round_trip() {
    use signex_types::schematic::{Point, SchematicSheet, Wire};

    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, boot).expect("boot settled");

    let path = isolated.path().join("round-trip.snxsch");
    let wire = Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    let sheet = SchematicSheet {
        wires: vec![wire],
        ..Default::default()
    };
    let mut seed = signex_engine::Engine::new_with_path(sheet, Some(path.clone())).expect("engine");
    seed.save().expect("seed file");
    app.headless_answers()
        .expect("headless answers")
        .replace_all([("path".to_string(), path.display().to_string())]);
    let (outcome, task) = app.dispatch_command_headless(&id("open_document"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    driver.settle(&mut app, task).expect("open settled");
    assert_eq!(
        app.document_state.active_path.as_deref(),
        Some(path.as_path())
    );

    let wires = |app: &Signex| app.document_state.engines[&path].document().wires.len();
    let mut run = |app: &mut Signex, name: &str| {
        let (outcome, task) = app.dispatch_command_headless(&id(name), CommandArgs::none());
        assert_eq!(outcome, CommandOutcome::Ran, "{name}");
        driver.settle(app, task).expect("command settled");
    };
    assert!(!app.document_state.is_document_dirty(&path));

    run(&mut app, "select_all");
    run(&mut app, "delete_selection");
    assert_eq!(wires(&app), 0, "the delete ran");
    assert!(
        app.document_state.is_document_dirty(&path),
        "an edit is dirty"
    );

    run(&mut app, "undo");
    assert_eq!(wires(&app), 1, "undo brought the wire back");
    assert!(
        !app.document_state.is_document_dirty(&path),
        "undo back to the saved state is clean"
    );

    run(&mut app, "redo");
    assert_eq!(wires(&app), 0);
    assert!(app.document_state.is_document_dirty(&path));
}

#[test]
fn the_undo_depth_comes_from_prefs_or_the_option_and_a_bad_value_is_reported() {
    run_probe("undo_depth");
}

/// `prefs.json` first, then the boot option over it: 1000 is taken as is,
/// 1001 is clamped, zero and text give the default, and each of the last
/// three is a warning naming the value.
fn undo_depth() {
    signex_app::diagnostics::init_logging().expect("logger");
    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let prefs = isolated.path().join("prefs.json");

    let cases: [(&str, usize, bool); 5] = [
        ("1000", 1000, false),
        ("1001", 1000, true),
        ("0", 100, true),
        ("\"many\"", 100, true),
        ("1", 1, false),
    ];
    for (stored, want, warned) in cases {
        std::fs::write(&prefs, format!(r#"{{"undo_depth":{stored}}}"#)).expect("prefs");
        let before = signex_app::diagnostics::newest_entry_id();
        let (app, _boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
        assert_eq!(app.document_state.undo_depth, want, "prefs {stored}");
        let logged = signex_app::diagnostics::entries_since(before)
            .entries
            .iter()
            .any(|e| e.message.contains("undo_depth") && e.message.contains("prefs.json"));
        assert_eq!(logged, warned, "prefs {stored}");
    }

    std::fs::write(&prefs, br#"{"undo_depth":7}"#).expect("prefs");
    let cases: [(&str, usize, bool); 5] = [
        ("1000", 1000, false),
        ("1001", 1000, true),
        ("0", 100, true),
        ("many", 100, true),
        ("3", 3, false),
    ];
    for (typed, want, warned) in cases {
        let before = signex_app::diagnostics::newest_entry_id();
        let (app, _boot) =
            Signex::boot_headless_with_undo_depth(Vec::new(), &rules(), typed).expect("boot");
        assert_eq!(app.document_state.undo_depth, want, "option {typed}");
        let logged = signex_app::diagnostics::entries_since(before)
            .entries
            .iter()
            .any(|e| e.message.contains("the undo depth option") && e.message.contains(typed));
        assert_eq!(logged, warned, "option {typed}");
    }
    let (app, _boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
    assert_eq!(
        app.document_state.undo_depth, 7,
        "no option: prefs.json wins"
    );
}

#[test]
fn a_headless_command_applies_the_undo_depth_without_an_update() {
    run_probe("undo_depth_dispatch");
}

/// `dispatch_command_headless` never calls `update`: an engine that exists
/// when a command returns still gets the configured depth.
fn undo_depth_dispatch() {
    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let (mut app, _boot) =
        Signex::boot_headless_with_undo_depth(Vec::new(), &rules(), "3").expect("boot");
    let path = isolated.path().join("depth.snxsch");
    let engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default())
        .expect("engine");
    assert_eq!(engine.undo_depth(), 100);
    app.document_state.engines.insert(path.clone(), engine);
    let (outcome, _task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(app.document_state.engines[&path].undo_depth(), 3);
}

#[test]
fn a_net_class_assignment_runs_undoes_and_dirties_the_project_through_command_ids_alone() {
    run_probe("net_class_assign");
}

/// Assign a net to a class by operands, undo, redo: no window, the pick is a
/// step of the sheet's timeline, the project (not the sheet) reads dirty, and
/// undoing it reads the project clean again (#661).
fn net_class_assign() {
    use signex_types::net_class::NetClass;
    use signex_types::project::{ProjectData, SheetEntry, write_project};
    use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Wire};

    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, boot).expect("boot settled");

    let dir = isolated.path().join("proj");
    std::fs::create_dir_all(&dir).expect("project dir");
    let (prj, sheet_path) = (dir.join("demo.snxprj"), dir.join("main.snxsch"));
    let wire = Wire {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        stroke_width: 0.0,
        unknown_columns: Default::default(),
        locked: false,
    };
    // A net exists in the netlist once a pin sits on it: one resistor pin.
    let origin = Point::new(0.0, 0.0);
    use signex_types::schematic::{LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle};
    let lib = LibSymbol {
        id: "Device:R".into(),
        reference: "R".into(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: vec![LibPin {
            unit: 0,
            body_style: 1,
            pin: Pin {
                direction: PinDirection::Passive,
                shape_style: PinShapeStyle::Plain,
                position: origin,
                rotation: 0.0,
                length: 0.0,
                name: String::new(),
                number: "1".into(),
                visible: true,
                name_visible: true,
                number_visible: true,
            },
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    };
    let sheet = SchematicSheet {
        wires: vec![wire],
        labels: vec![Label::new("GND", origin, LabelType::Global)],
        symbols: vec![signex_types::schematic::Symbol {
            uuid: uuid::Uuid::now_v7(),
            lib_id: "Device:R".into(),
            reference: "R1".into(),
            position: origin,
            ..Default::default()
        }],
        lib_symbols: [("Device:R".to_string(), lib)].into_iter().collect(),
        ..Default::default()
    };
    let mut seed =
        signex_engine::Engine::new_with_path(sheet, Some(sheet_path.clone())).expect("engine");
    seed.save().expect("seed sheet");
    let data = ProjectData {
        description: None,
        name: "demo".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("main.snxsch".into()),
        pcb_file: None,
        sheets: vec![SheetEntry {
            name: "Main".into(),
            filename: "main.snxsch".into(),
            symbols_count: 0,
            wires_count: 1,
            labels_count: 1,
            unknown: Default::default(),
        }],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: vec![NetClass::named("Power")],
        rules: None,
        annotation: None,
        schematic_defaults: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    };
    write_project(&prj, &data).expect("seed project");

    let mut run = |app: &mut Signex, name: &str, operands: &[&str]| {
        let command = id(name);
        let args = if operands.is_empty() {
            CommandArgs::none()
        } else {
            CommandArgs::parse_operands(&command, operands).expect("operands")
        };
        let (outcome, task) = app.dispatch_command_headless(&command, args);
        driver.settle(app, task).expect("command settled");
        outcome
    };
    for opened in [&prj, &sheet_path] {
        app.headless_answers()
            .expect("headless answers")
            .replace_all([("path".to_string(), opened.display().to_string())]);
        assert_eq!(run(&mut app, "open_document", &[]), CommandOutcome::Ran);
    }
    let classes = |app: &Signex| {
        app.document_state.projects[0].data.net_classes[0]
            .nets
            .clone()
    };
    assert!(!app.document_state.is_document_dirty(&prj));
    assert!(!app.document_state.is_document_dirty(&sheet_path));

    // Headless names its document: no operands, or no `path`, is refused and
    // never falls back to the active sheet; nothing changes.
    for (operands, wanted) in [
        (&[][..], "needs the operand `net_name`"),
        (
            &["net_name=GND", "class=Power"][..],
            "needs the operand `path`",
        ),
    ] {
        match run(&mut app, "assign_net_class", operands) {
            CommandOutcome::Disabled { reason } => assert!(reason.contains(wanted), "{reason}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(classes(&app).is_empty() && !app.document_state.is_document_dirty(&prj));
    }
    let given = format!("path={}", sheet_path.display());
    let ran = run(
        &mut app,
        "assign_net_class",
        &["net_name=GND", "class=Power", &given],
    );
    assert_eq!(ran, CommandOutcome::Ran, "{ran:?}");
    assert_eq!(classes(&app), ["GND"]);
    assert!(
        app.document_state.is_document_dirty(&prj),
        "the project changed"
    );
    assert!(
        !app.document_state.is_document_dirty(&sheet_path),
        "the sheet did not"
    );

    assert_eq!(run(&mut app, "undo", &[]), CommandOutcome::Ran);
    assert!(classes(&app).is_empty());
    assert!(
        !app.document_state.is_document_dirty(&prj),
        "back at the saved bytes"
    );
    assert_eq!(run(&mut app, "redo", &[]), CommandOutcome::Ran);
    assert_eq!(classes(&app), ["GND"]);
    assert!(app.document_state.is_document_dirty(&prj));
}
