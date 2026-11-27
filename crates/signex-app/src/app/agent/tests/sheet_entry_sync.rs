//! #2501: `sync_sheet_entries direction=to_sheet` on the agent channel. The
//! child sheet the call does not name joins the agent's step: one undo takes
//! back the parent and the child, and a list that fails after the sync leaves
//! both as they were.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet};

use super::{app_in, call_settled};
use crate::app::Signex;

/// Write `file` in `dir` with the hierarchical labels `ports`.
fn write_sheet(dir: &Path, file: &str, ports: &[&str]) -> PathBuf {
    let path = dir.join(file);
    let labels = (ports.iter().enumerate())
        .map(|(i, name)| {
            let at = Point::new(20.0, 20.0 + 5.08 * i as f64);
            Label::new(*name, at, LabelType::Hierarchical)
        })
        .collect();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        labels,
        ..Default::default()
    };
    signex_engine::Engine::new_with_path(sheet, Some(path.clone()))
        .expect("engine")
        .save()
        .expect("write sheet");
    path
}

/// The fixture's project in `dir`, with `power.snxsch` (the child its sheets
/// place, carrying the port `VIN`) and the empty `charger.snxsch` on disk.
fn project(dir: &Path) -> Signex {
    write_sheet(dir, "power.snxsch", &["VIN"]);
    write_sheet(dir, "charger.snxsch", &[]);
    app_in(dir)
}

/// The ports the child at `path` exposes, from its engine; `None` when no
/// engine holds it.
fn ports(app: &Signex, path: &Path) -> Option<Vec<String>> {
    let engine = app.document_state.engines.get(path)?;
    let exposed = signex_engine::exposed_sheet_ports(engine.document());
    Some(exposed.into_iter().map(|port| port.name).collect())
}

fn symbols_on(app: &Signex, path: &Path) -> usize {
    app.document_state.engines[path]
        .document()
        .child_sheets
        .len()
}

/// Charger on `b`, its entry `VIN`, and the entry pushed into the child.
fn charger_edits() -> Vec<Value> {
    vec![
        json!({"id": "place_sheet_symbol", "args": {
            "start": "100,100", "end": "120,120", "name": "Charger", "file": "charger.snxsch"}}),
        json!({"id": "place_sheet_entry", "args": {"sheet": "Charger", "at": "100,105", "name": "VIN"}}),
        json!({"id": "sync_sheet_entries", "args": {"symbol": "Charger", "direction": "to_sheet"}}),
    ]
}

fn apply(app: &mut Signex, b: &Path, edits: Vec<Value>) -> Value {
    let revision = app.document_state.engines[b].revision();
    let args =
        json!({"document": b.display().to_string(), "expect_revision": revision, "edits": edits});
    call_settled(app, "apply_edits", args)
}

#[test]
fn to_sheet_in_an_edit_list_joins_the_step_and_one_undo_takes_both_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project(dir.path());
    let (b, charger) = (
        dir.path().join("b.snxsch"),
        dir.path().join("charger.snxsch"),
    );
    let on_disk = std::fs::read_to_string(&charger).unwrap();
    let before = symbols_on(&app, &b);

    let reply = apply(&mut app, &b, charger_edits());

    assert_eq!(reply["status"], "answered", "{reply}");
    let data = &reply["data"];
    assert_eq!(data["changed"], true, "{reply}");
    assert_eq!(data["results"][2]["added"], json!(["VIN"]), "{reply}");
    // The child is listed: a step that landed there is taken back by naming it.
    let sheets = data["sheets"]
        .as_array()
        .expect("the sheets the step changed");
    let child = (sheets.iter())
        .find(|sheet| sheet["document"] == "charger.snxsch")
        .expect("the child is listed");
    assert_eq!(child["changed"], true, "{reply}");
    assert_eq!(symbols_on(&app, &b), before + 1);
    assert_eq!(ports(&app, &charger), Some(vec!["VIN".to_string()]));
    assert_eq!(
        std::fs::read_to_string(&charger).unwrap(),
        on_disk,
        "the child is left unsaved"
    );

    // The person's Ctrl+Z on `b` takes back the parent and the child at once.
    assert!(app.apply_engine_undo(false));
    assert_eq!(symbols_on(&app, &b), before);
    let child_ports = ports(&app, &charger).unwrap_or_default();
    assert!(child_ports.is_empty(), "{child_ports:?}");
}

#[test]
fn a_list_that_fails_after_the_sync_leaves_the_parent_and_the_child_as_they_were() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project(dir.path());
    let (b, charger) = (
        dir.path().join("b.snxsch"),
        dir.path().join("charger.snxsch"),
    );
    let revision = app.document_state.engines[&b].revision();
    let mut edits = charger_edits();
    // A second `VIN` on the same symbol is refused.
    edits.push(json!({"id": "place_sheet_entry", "args": {"sheet": "Charger", "at": "100,110", "name": "VIN"}}));

    let reply = apply(&mut app, &b, edits);

    assert_eq!(reply["status"], "refused", "{reply}");
    assert_eq!(app.document_state.engines[&b].revision(), revision);
    assert!(
        !app.document_state.engines.contains_key(&charger),
        "the child loaded for the step is dropped again"
    );
    assert!(app.document_state.agent_edits.joined.is_empty());
}

#[test]
fn to_sheet_alone_in_a_list_is_one_labelled_step_of_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project(dir.path());
    let (b, power) = (dir.path().join("b.snxsch"), dir.path().join("power.snxsch"));
    let revision = app.document_state.engines[&b].revision();

    // The fixture's `Power` symbol has no entries: to_sheet takes `VIN` away.
    // Its `document` is optional, so an agent reaches it only in a list.
    let edit =
        json!({"id": "sync_sheet_entries", "args": {"symbol": "Power", "direction": "to_sheet"}});
    let reply = apply(&mut app, &b, vec![edit]);

    assert_eq!(reply["status"], "answered", "{reply}");
    assert_eq!(
        reply["data"]["results"][0]["removed"],
        json!(["VIN"]),
        "{reply}"
    );
    assert_eq!(ports(&app, &power), Some(Vec::new()));
    assert_eq!(app.document_state.engines[&b].revision(), revision);
    let group = (app.document_state.engines[&power].next_undo_group())
        .expect("the child's step carries the agent's group");
    let edits = &app.document_state.agent_edits;
    assert_eq!(
        edits.labels.get(&group).map(String::as_str),
        Some("Agent: sync_sheet_entries")
    );
    assert!(edits.joined.is_empty(), "the step let go of the child");
}
