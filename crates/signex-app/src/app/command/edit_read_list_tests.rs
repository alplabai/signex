//! #2199 review round 3: an edit list that places a sheet symbol on a closed
//! file reads that file with the project, so the later edits of the list are
//! judged against it as a check at that moment would judge them; the loop
//! check reads the project; a failed read is said; staleness looks at the
//! target's project only.

use std::path::Path;

use serde_json::{Value, json};
use signex_types::schematic::{ChildSheet, Label, LabelType, Point, SchematicSheet, Symbol, Wire};
use uuid::Uuid;

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;

fn save(dir: &Path, name: &str, sheet: SchematicSheet) {
    signex_engine::Engine::new_with_path(sheet, Some(dir.join(name)))
        .expect("engine")
        .save()
        .expect("write sheet");
}

/// The closed child: hierarchical `SDA` on a wire whose other end carries the
/// global label `I2C_SDA`.
fn charger() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        wires: vec![Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.16, 0.0),
            ..Default::default()
        }],
        labels: vec![
            Label::new("SDA", Point::new(0.0, 0.0), LabelType::Hierarchical),
            Label::new("I2C_SDA", Point::new(10.16, 0.0), LabelType::Global),
        ],
        ..Default::default()
    }
}

fn empty() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        ..Default::default()
    }
}

/// The project `top` (root, the target) and `other`, with `charger.snxsch`
/// on disk and referenced by no sheet. `other` is open when `all_open`.
fn project(dir: &Path, all_open: bool) -> Signex {
    let _ = crate::diagnostics::init_logging();
    save(dir, "top.snxsch", empty());
    save(dir, "other.snxsch", empty());
    save(dir, "charger.snxsch", charger());
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch", "other.snxsch"]);
    app.ui_state.snap_enabled = false;
    if all_open {
        app.open_or_focus_sheet_path(dir.join("other.snxsch"));
    }
    app.open_or_focus_sheet_path(dir.join("top.snxsch"));
    app
}

fn apply(app: &mut Signex, dir: &Path, edits: Value) -> CommandOutcome {
    let path = dir.join("top.snxsch");
    let revision = app.document_state.engines[&path].revision();
    let id = AppCommandId::new("apply_edits").unwrap();
    let operands = [
        format!("document={}", path.display()),
        format!("expect_revision={revision}"),
        format!("edits={edits}"),
    ];
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let call = CommandArgs::parse_operands(&id, &refs).unwrap();
    app.run_operands_settled(&id, &call).0
}

/// Charger from (100,100) to (120,120), its entry `SDA` at (100,105) and a
/// wire from it to (90,105).
fn charger_wired() -> Vec<Value> {
    vec![
        json!({"id": "place_sheet_symbol", "args": {
            "start": "100,100", "end": "120,120", "name": "Charger", "file": "charger.snxsch"}}),
        json!({"id": "place_sheet_entry", "args": {"sheet": "Charger", "at": "100,105", "name": "SDA"}}),
        json!({"id": "place_wire", "args": {"start": "100,105", "end": "90,105", "allow_open": true}}),
    ]
}

#[test]
fn a_child_placed_by_the_list_is_read_and_its_names_judged() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project(dir.path(), false);
    let mut edits = charger_wired();
    edits.push(json!({"id": "place_global_label", "args": {"at": "90,105", "text": "VBUS"}}));
    let outcome = apply(&mut app, dir.path(), Value::Array(edits));
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(reason.contains("allow_merge"), "{reason}");
}

#[test]
fn a_closed_child_placed_in_an_open_project_is_read_not_refused_for_a_retry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project(dir.path(), true);
    let outcome = apply(&mut app, dir.path(), Value::Array(charger_wired()));
    assert!(
        matches!(&outcome, CommandOutcome::Answered(a) if a["changed"] == true),
        "{outcome:?}"
    );
}

#[test]
fn a_part_after_a_closed_child_in_the_list_is_numbered_past_the_childs_parts() {
    let dir = tempfile::tempdir().unwrap();
    let power = SchematicSheet {
        uuid: Uuid::now_v7(),
        symbols: (1..=4)
            .map(|n| Symbol {
                uuid: Uuid::now_v7(),
                lib_id: "Device:R".into(),
                reference: format!("R{n}"),
                unit: 1,
                position: Point::new(25.4 * f64::from(n), 25.4),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let mut app = project(dir.path(), true);
    save(dir.path(), "power.snxsch", power);
    let stock = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/stock-library/Stock.snxlib");
    app.library
        .open_library_read_only(stock.clone())
        .expect("mount stock");
    let symbol = json!({"id": "place_sheet_symbol", "args": {
        "start": "100,100", "end": "120,120", "name": "Power", "file": "power.snxsch"}});
    let part = |extra: Option<&str>| {
        let mut args =
            json!({"library": stock.display().to_string(), "part": "STOCK-R", "at": "254,254"});
        if let Some(designator) = extra {
            args["designator"] = json!(designator);
        }
        json!({"id": "place_component", "args": args})
    };
    let outcome = apply(&mut app, dir.path(), json!([symbol.clone(), part(None)]));
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["results"][1]["designator"], "R5", "{answer}");

    let dir2 = tempfile::tempdir().unwrap();
    let mut app = project(dir2.path(), true);
    save(
        dir2.path(),
        "power.snxsch",
        SchematicSheet {
            uuid: Uuid::now_v7(),
            symbols: vec![Symbol {
                uuid: Uuid::now_v7(),
                lib_id: "Device:R".into(),
                reference: "R1".into(),
                unit: 1,
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    app.library.open_library_read_only(stock.clone()).unwrap();
    let outcome = apply(&mut app, dir2.path(), json!([symbol, part(Some("R1"))]));
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(reason.contains("is another part"), "{reason}");
}

#[test]
fn a_sheet_symbol_naming_a_closed_ancestor_is_refused_with_no_window_hierarchy() {
    let dir = tempfile::tempdir().unwrap();
    let _ = crate::diagnostics::init_logging();
    let mid = signex_engine::child_sheet_from_corners(
        Point::new(100.0, 100.0),
        Point::new(120.0, 120.0),
        "mid",
    )
    .expect("big enough");
    save(
        dir.path(),
        "top.snxsch",
        SchematicSheet {
            uuid: Uuid::now_v7(),
            child_sheets: vec![ChildSheet {
                filename: "mid.snxsch".into(),
                ..mid
            }],
            ..Default::default()
        },
    );
    save(dir.path(), "mid.snxsch", empty());
    // `mid` is a listed page, so the project owns it while `top` is closed.
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch", "mid.snxsch"]);
    app.ui_state.snap_enabled = false;
    app.open_or_focus_sheet_path(dir.path().join("mid.snxsch"));
    let id = AppCommandId::new("place_sheet_symbol").unwrap();
    let call = CommandArgs::parse_operands(
        &id,
        &["start=50,50", "end=70,70", "name=Up", "file=top.snxsch"],
    )
    .unwrap();
    let outcome = app.run_operands_settled(&id, &call).0;
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(reason.contains("is an ancestor of this sheet"), "{reason}");
}

#[test]
fn an_edit_on_another_projects_sheet_during_the_read_does_not_refuse() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = super::project_name_tests::project_app(&dir);
    let top = app.document_state.edit_path().unwrap().to_path_buf();
    let loose = dir.path().join("elsewhere").join("loose.snxsch");
    crate::app::handlers::editing_commands::net_color_driver::add_sheet_with(
        &mut app,
        &loose,
        SchematicSheet::default(),
    );
    app.open_or_focus_sheet_path(top.clone());
    let id = AppCommandId::new("place_wire").unwrap();
    let call =
        CommandArgs::parse_operands(&id, &["start=5.08,0", "end=10.16,0", "allow_merge=true"])
            .unwrap();
    let (CommandOutcome::Deferred { ticket }, task) = app.run_operands(&id, &call) else {
        panic!("not deferred");
    };
    let junction = signex_types::schematic::Junction {
        uuid: Uuid::now_v7(),
        position: Point::new(1.0, 1.0),
        ..Default::default()
    };
    app.document_state
        .engines
        .get_mut(&loose)
        .unwrap()
        .execute(signex_engine::Command::PlaceJunction { junction })
        .expect("the person edits a sheet of no project");
    let (outcome, _) = app.settle_deferred(ticket, task);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn a_reader_that_panicked_is_said_in_messages_and_refused_without_a_retry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = super::project_name_tests::project_app(&dir);
    let top = app.document_state.edit_path().unwrap().to_path_buf();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let joined = runtime.block_on(async {
        tokio::task::spawn_blocking(|| -> super::EditReads { panic!("the reader broke") }).await
    });
    let id = AppCommandId::new("place_junction").unwrap();
    let loaded = super::ReadsLoaded {
        ticket: 1,
        command: id.clone(),
        args: CommandArgs::parse_operands(&id, &["at=30,30"]).unwrap(),
        target: top.clone(),
        revisions: app.watched_revisions(&top),
        origin: crate::app::agent::Origin::default(),
        serving: false,
        reads: std::sync::Arc::new(super::edit_reads::landed(joined)),
    };
    let mark = crate::diagnostics::newest_entry_id();
    let revision = app.document_state.engines[&top].revision();
    let (outcome, _) = app.run_with_reads(loaded);
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(reason.contains("failed"), "{reason}");
    assert!(!reason.contains("retry"), "{reason}");
    assert_eq!(app.document_state.engines[&top].revision(), revision);
    let said = crate::diagnostics::entries_since(mark).entries;
    assert!(
        said.iter()
            .any(|e| e.message.contains("place_junction") && e.message.contains("failed")),
        "{said:?}"
    );
}
