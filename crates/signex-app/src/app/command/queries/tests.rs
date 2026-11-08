//! #2172 — the query commands answer with what a person sees in the UI,
//! read from the documents alone, and change nothing.

use std::path::PathBuf;

use serde_json::Value;
use signex_types::schematic::{
    ChildSheet, HAlign, Junction, Label, LabelType, Point, SchematicSheet, Symbol, TextNote,
    VAlign, Wire,
};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::interaction_snapshot_tests::interaction_snapshot;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::AppCommandId;

pub(in crate::app) fn uid(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

pub(in crate::app) const SHEET: u128 = 100;
pub(in crate::app) const WIRE_A: u128 = 1;
const WIRE_B: u128 = 2;
const WIRE_FAR: u128 = 3;
const JUNCTION: u128 = 4;
const LABEL_GND: u128 = 5;
const LABEL_VCC: u128 = 6;
const U3: u128 = 7;
const U3_PIN_1: u128 = 8;
const NOTE: u128 = 9;
const CHILD: u128 = 10;
const POWER_PORT: u128 = 11;

fn wire(n: u128, from: (f64, f64), to: (f64, f64)) -> Wire {
    Wire {
        uuid: uid(n),
        start: Point::new(from.0, from.1),
        end: Point::new(to.0, to.1),
        ..Default::default()
    }
}

fn label(n: u128, text: &str, at: (f64, f64)) -> Label {
    Label {
        uuid: uid(n),
        ..Label::new(text, Point::new(at.0, at.1), LabelType::Net)
    }
}

/// A sheet with one of everything a person reads on it: two wires on the net
/// GND (joined at a junction, named by a label), a far wire on VCC, a symbol
/// U3 with a pin, a text note, a sheet entry and a power port.
pub(in crate::app) fn sheet(id: u128) -> SchematicSheet {
    let mut u3 = Symbol {
        uuid: uid(U3),
        lib_id: "Amp:LM358".into(),
        reference: "U3".into(),
        value: "LM358".into(),
        footprint: "SOIC-8".into(),
        position: Point::new(20.0, 20.0),
        ..Default::default()
    };
    u3.pin_uuids.insert("1".into(), uid(U3_PIN_1));
    let port = Symbol {
        uuid: uid(POWER_PORT),
        lib_id: "power:VCC".into(),
        value: "VCC".into(),
        is_power: true,
        position: Point::new(40.0, 0.0),
        ..Default::default()
    };
    SchematicSheet {
        uuid: uid(id),
        paper_size: "A4".into(),
        symbols: vec![u3, port],
        wires: vec![
            wire(WIRE_A, (0.0, 0.0), (10.0, 0.0)),
            wire(WIRE_B, (10.0, 0.0), (10.0, 10.0)),
            wire(WIRE_FAR, (30.0, 0.0), (40.0, 0.0)),
        ],
        junctions: vec![Junction {
            uuid: uid(JUNCTION),
            position: Point::new(10.0, 0.0),
            ..Default::default()
        }],
        labels: vec![
            label(LABEL_GND, "GND", (0.0, 0.0)),
            label(LABEL_VCC, "VCC", (30.0, 0.0)),
        ],
        text_notes: vec![TextNote {
            uuid: uid(NOTE),
            text: "check the bypass cap".into(),
            position: Point::new(5.0, 30.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked: false,
        }],
        child_sheets: vec![ChildSheet {
            uuid: uid(CHILD),
            name: "Power".into(),
            filename: "power.snxsch".into(),
            position: Point::new(50.0, 50.0),
            size: (20.0, 10.0),
            stroke_width: 0.0,
            fill: Default::default(),
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }],
        ..Default::default()
    }
}

/// A path in a real temp dir, so it is absolute on every host.
pub(in crate::app) fn path(name: &str) -> PathBuf {
    std::env::temp_dir().join("snx-2172").join(name)
}

/// An app with one clean sheet open per `(file name, sheet id)`; the last one
/// opened is the active one.
pub(in crate::app) fn app_with(sheets: &[(&str, u128)]) -> Signex {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    for (name, id) in sheets {
        fixture::add_sheet_with(&mut app, &path(name), sheet(*id));
        app.document_state.mark_document_clean(&path(name));
    }
    app
}

fn command(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn run(app: &mut Signex, name: &str, args: &[&str]) -> CommandOutcome {
    let command = command(name);
    // No operands is the call a palette or keybinding makes; parsing would
    // refuse it for a command that needs one, before the query could.
    let args = if args.is_empty() {
        CommandArgs::none()
    } else {
        CommandArgs::parse_operands(&command, args).expect("operands parse")
    };
    let (outcome, _task) = app.run_query(&command, &args).expect("a query");
    outcome
}

pub(in crate::app) fn answer(app: &mut Signex, name: &str, args: &[&str]) -> Value {
    match run(app, name, args) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{name} {args:?} was not answered: {other:?}"),
    }
}

fn refusal(app: &mut Signex, name: &str, args: &[&str]) -> String {
    match run(app, name, args) {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("{name} {args:?} was not refused: {other:?}"),
    }
}

fn documents(value: &Value) -> &Vec<Value> {
    value["documents"].as_array().expect("documents")
}

fn red() -> signex_types::theme::Color {
    signex_types::theme::Color::new(1, 2, 3, 255)
}

fn paint_wire_a(app: &mut Signex) {
    app.execute_on_sheet(
        &path("a.snxsch"),
        signex_engine::Command::SetNetColors {
            colors: [(uid(WIRE_A), red())].into_iter().collect(),
        },
    )
    .expect("the sheet takes a colour");
}

#[test]
fn a_loaded_sheet_is_listed_with_its_own_uuid_as_id() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let answer = answer(&mut app, "list_documents", &[]);
    let docs = documents(&answer);
    assert_eq!(docs.len(), 1);
    let doc = &docs[0];
    assert_eq!(doc["id"], uid(SHEET).to_string());
    assert_eq!(doc["kind"], "schematic");
    assert_eq!(doc["name"], "a");
    assert_eq!(doc["path"], path("a.snxsch").display().to_string());
    assert_eq!(doc["loaded"], true);
    assert_eq!(doc["dirty"], false);
    assert_eq!(doc["read_only"], false);
    assert_eq!(doc["saving"], false);
    assert_eq!(doc["open_in_window"], false);
    assert!(doc["project"].is_null());
}

#[test]
fn the_filters_of_list_documents_each_narrow_the_list() {
    let mut app = app_with(&[("a.snxsch", SHEET), ("power.snxsch", SHEET + 1)]);
    app.document_state
        .mark_document_dirty(&path("power.snxsch"));
    let names = |app: &mut Signex, args: &[&str]| -> Vec<String> {
        let a = answer(app, "list_documents", args);
        documents(&a)
            .iter()
            .map(|d| d["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names(&mut app, &[]), ["a", "power"]);
    assert_eq!(names(&mut app, &["kind=schematic"]), ["a", "power"]);
    assert!(names(&mut app, &["kind=pcb"]).is_empty());
    assert_eq!(names(&mut app, &["dirty=true"]), ["power"]);
    assert_eq!(names(&mut app, &["dirty=false"]), ["a"]);
    assert_eq!(names(&mut app, &["name=POW"]), ["power"]);
    assert!(refusal(&mut app, "list_documents", &["kind=sheet"]).contains("not a document kind"));
}

#[test]
fn a_query_without_its_required_operand_is_refused() {
    let mut app = app_with(&[]);
    for name in ["document_info", "resolve", "read_document"] {
        let reason = refusal(&mut app, name, &[]);
        assert!(reason.contains("needs the operand"), "{name}: {reason}");
    }
}

#[test]
fn document_info_names_the_document_by_id_or_by_path() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let by_id = answer(
        &mut app,
        "document_info",
        &[&format!("document={}", uid(SHEET))],
    );
    let by_path = answer(
        &mut app,
        "document_info",
        &[&format!("document={}", path("a.snxsch").display())],
    );
    assert_eq!(by_id, by_path);
    assert_eq!(by_id["id"], uid(SHEET).to_string());
    assert_eq!(by_id["sheet_number"], "1");
    assert_eq!(by_id["history"]["can_undo"], false);
    assert_eq!(by_id["history"]["can_redo"], false);
    assert!(by_id["placements"].as_array().unwrap().is_empty());
}

#[test]
fn a_document_that_is_not_open_is_refused_with_a_reason() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let gone = format!("document={}", path("nope.snxsch").display());
    assert!(refusal(&mut app, "document_info", &[&gone]).contains("not an open or referenced"));
    let unknown = format!("document={}", uid(555));
    assert!(refusal(&mut app, "document_info", &[&unknown]).contains("no document has the id"));
}

#[test]
fn document_info_reports_history_after_an_edit() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    paint_wire_a(&mut app);
    let info = answer(
        &mut app,
        "document_info",
        &[&format!("document={}", uid(SHEET))],
    );
    assert_eq!(info["history"]["can_undo"], true);
    assert_eq!(info["dirty"], true);
}

fn element(found: &Value) -> &Value {
    assert_eq!(found["found"], true, "{found}");
    assert_eq!(found["what"], "element");
    &found["element"]
}

#[test]
fn resolve_a_document_id_gives_its_metadata() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let found = answer(&mut app, "resolve", &[&format!("id={}", uid(SHEET))]);
    assert_eq!(found["what"], "document");
    assert_eq!(found["document"]["name"], "a");
}

#[test]
fn resolve_an_unknown_uuid_is_answered_as_unknown() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let found = answer(&mut app, "resolve", &[&format!("id={}", uid(999))]);
    assert_eq!(found["found"], false);
    assert_eq!(found["what"], "unknown");
    assert!(refusal(&mut app, "resolve", &["id=not-a-uuid"]).contains("not an id"));
}

#[test]
fn resolve_each_kind_of_element_gives_its_document_and_human_fields() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let mut resolve = |n: u128| answer(&mut app, "resolve", &[&format!("id={}", uid(n))]);

    let symbol = resolve(U3);
    let e = element(&symbol);
    assert_eq!(e["kind"], "symbol");
    assert_eq!(e["designator"], "U3");
    assert_eq!(e["value"], "LM358");
    assert_eq!(e["footprint"], "SOIC-8");
    assert_eq!(e["position"], serde_json::json!([20.0, 20.0]));
    assert_eq!(e["document"]["id"], uid(SHEET).to_string());
    assert_eq!(e["document"]["name"], "a");

    let pin = resolve(U3_PIN_1);
    assert_eq!(element(&pin)["kind"], "pin");
    assert_eq!(element(&pin)["number"], "1");
    assert_eq!(element(&pin)["symbol_designator"], "U3");

    let wire = resolve(WIRE_B);
    assert_eq!(element(&wire)["kind"], "wire");
    assert_eq!(
        element(&wire)["net"],
        "GND",
        "a wire is on the net its label names"
    );

    assert_eq!(element(&resolve(WIRE_FAR))["net"], "VCC");
    assert_eq!(element(&resolve(JUNCTION))["kind"], "junction");
    let label = resolve(LABEL_GND);
    assert_eq!(element(&label)["kind"], "label");
    assert_eq!(element(&label)["text"], "GND");
    assert_eq!(element(&label)["net"], "GND");
    let port = resolve(POWER_PORT);
    assert_eq!(element(&port)["kind"], "power_port");
    assert_eq!(element(&port)["net"], "VCC");
    let note = resolve(NOTE);
    assert_eq!(element(&note)["kind"], "text");
    assert_eq!(element(&note)["text"], "check the bypass cap");
    let child = resolve(CHILD);
    assert_eq!(element(&child)["kind"], "sheet_entry");
    assert_eq!(element(&child)["filename"], "power.snxsch");
}

fn element_ids(found: &Value) -> Vec<String> {
    let mut ids: Vec<String> = found["elements"]
        .as_array()
        .expect("elements")
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

#[test]
fn find_by_designator_gives_the_uuid_an_agent_then_acts_on() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let found = answer(&mut app, "find", &["designator=u3"]);
    assert_eq!(found["count"], 1);
    assert_eq!(element_ids(&found), [uid(U3).to_string()]);
    assert_eq!(
        found["elements"][0]["document"]["id"],
        uid(SHEET).to_string()
    );
    assert_eq!(answer(&mut app, "find", &["designator=U4"])["count"], 0);
}

#[test]
fn find_by_net_gives_the_wires_junction_and_label_on_it() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let found = answer(&mut app, "find", &["net=gnd"]);
    let mut want: Vec<String> = [WIRE_A, WIRE_B, JUNCTION, LABEL_GND]
        .iter()
        .map(|n| uid(*n).to_string())
        .collect();
    want.sort();
    assert_eq!(element_ids(&found), want);
    let wires = answer(&mut app, "find", &["net=VCC", "kind=wire"]);
    assert_eq!(element_ids(&wires), [uid(WIRE_FAR).to_string()]);
}

#[test]
fn find_by_text_value_and_kind() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let text = answer(&mut app, "find", &["text=BYPASS"]);
    assert_eq!(element_ids(&text), [uid(NOTE).to_string()]);
    let value = answer(&mut app, "find", &["value=lm3"]);
    assert_eq!(element_ids(&value), [uid(U3).to_string()]);
    let labels = answer(&mut app, "find", &["kind=label"]);
    assert_eq!(labels["count"], 2);
}

#[test]
fn a_net_with_two_names_is_called_by_the_smallest() {
    let mut sheet = sheet(SHEET);
    sheet.labels.push(label(99, "ALT", (10.0, 10.0)));
    assert_eq!(
        super::sheet_view::net_name_of_wire(&sheet, uid(WIRE_B)).as_deref(),
        Some("ALT")
    );
}

#[test]
fn find_without_an_element_filter_finds_documents() {
    let mut app = app_with(&[("a.snxsch", SHEET), ("power.snxsch", SHEET + 1)]);
    let found = answer(&mut app, "find", &["name=power"]);
    assert_eq!(documents(&found).len(), 1);
    assert_eq!(documents(&found)[0]["name"], "power");
}

#[test]
fn find_in_one_document_leaves_the_others_out() {
    let mut app = app_with(&[("a.snxsch", SHEET), ("b.snxsch", SHEET + 1)]);
    let both = answer(&mut app, "find", &["designator=U3"]);
    assert_eq!(both["count"], 2, "the same designator on two sheets");
    let one = answer(
        &mut app,
        "find",
        &[
            "designator=U3",
            &format!("document={}", path("b.snxsch").display()),
        ],
    );
    assert_eq!(one["count"], 1);
    assert_eq!(
        one["elements"][0]["document"]["id"],
        uid(SHEET + 1).to_string()
    );
}

#[test]
fn a_find_with_no_filter_is_still_the_find_dialog() {
    let (mut app, _task) = Signex::new();
    let command = command("find");
    assert!(app.run_query(&command, &CommandArgs::none()).is_none());
}

#[test]
fn queries_leave_the_persons_interaction_state_as_it_was() {
    let mut app = app_with(&[("a.snxsch", SHEET), ("b.snxsch", SHEET + 1)]);
    fixture::activate(&mut app, &path("a.snxsch"));
    app.interaction_state.active_canvas_mut().selected =
        vec![signex_types::schematic::SelectedItem::new(
            uid(WIRE_A),
            signex_types::schematic::SelectedKind::Wire,
        )];
    let before = interaction_snapshot(&app);
    let by_id = format!("document={}", uid(SHEET + 1));
    answer(&mut app, "list_documents", &[]);
    answer(&mut app, "document_info", &[&by_id]);
    answer(&mut app, "resolve", &[&format!("id={}", uid(U3))]);
    answer(&mut app, "find", &["designator=U3"]);
    answer(&mut app, "read_document", &[&by_id]);
    assert_eq!(interaction_snapshot(&app), before);
    assert!(!app.document_state.is_document_dirty(&path("b.snxsch")));
}

#[test]
fn two_documents_with_one_id_the_later_gets_a_new_one_and_a_message() {
    let mut app = app_with(&[("first.snxsch", SHEET), ("second.snxsch", SHEET)]);
    let listed = answer(&mut app, "list_documents", &[]);
    let docs = documents(&listed);
    assert_eq!(docs[0]["name"], "first");
    assert_eq!(
        docs[0]["id"],
        uid(SHEET).to_string(),
        "the first seen keeps it"
    );
    let second = docs[1]["id"].as_str().unwrap();
    assert_ne!(second, uid(SHEET).to_string());
    assert_eq!(
        app.document_state.engines[&path("second.snxsch")]
            .document()
            .uuid
            .to_string(),
        second,
        "the new id is in the document, so its next save writes it"
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("second.snxsch") && e.message.contains("same document id")),
        "Messages says so"
    );
    assert!(
        !app.document_state.is_document_dirty(&path("second.snxsch")),
        "the repair is not an edit"
    );
    // Asked again, nothing moves.
    assert_eq!(
        answer(&mut app, "list_documents", &[])["documents"][1]["id"],
        second
    );
}

#[test]
fn a_sheet_with_no_id_is_given_one_and_keeps_it() {
    let mut app = app_with(&[("a.snxsch", 0)]);
    let first = answer(&mut app, "list_documents", &[])["documents"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(first, Uuid::nil().to_string());
    let again = answer(&mut app, "list_documents", &[])["documents"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(first, again);
}

#[test]
fn read_document_carries_the_revision_and_it_moves_with_an_edit() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let by_id = format!("document={}", uid(SHEET));
    let before = answer(&mut app, "read_document", &[&by_id]);
    let revision = before["revision"].as_u64().expect("a revision");
    assert_eq!(before["total"], 10);
    assert_eq!(before["document"]["id"], uid(SHEET).to_string());
    paint_wire_a(&mut app);
    let after = answer(&mut app, "read_document", &[&by_id]);
    assert_ne!(after["revision"].as_u64().unwrap(), revision);
    assert_eq!(after["header"]["net_colors"].as_object().unwrap().len(), 1);
}

#[test]
fn read_document_narrows_by_kinds_ids_region_and_fields() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let by_id = format!("document={}", uid(SHEET));

    let wires = answer(&mut app, "read_document", &[&by_id, "kinds=wire"]);
    assert_eq!(wires["total"], 3);
    assert!(wires["elements"].get("symbols").is_none());
    assert_eq!(wires["elements"]["wires"].as_array().unwrap().len(), 3);

    let one = answer(
        &mut app,
        "read_document",
        &[&by_id, &format!("ids={}", uid(WIRE_B))],
    );
    assert_eq!(one["total"], 1);
    assert_eq!(one["elements"]["wires"][0]["uuid"], uid(WIRE_B).to_string());

    let region = answer(
        &mut app,
        "read_document",
        &[&by_id, "kinds=wire", "region=25,-5,45,5"],
    );
    assert_eq!(region["total"], 1, "only the far wire has an end inside");

    let fields = answer(
        &mut app,
        "read_document",
        &[&by_id, "kinds=symbol", "fields=reference"],
    );
    let symbol = &fields["elements"]["symbols"][0];
    assert_eq!(symbol["reference"], "U3");
    assert!(
        symbol.get("value").is_none(),
        "a field projection drops the rest"
    );
    assert!(symbol.get("uuid").is_some(), "the id always stays");
    assert!(fields["header"].get("lib_symbols").is_none());
}

#[test]
fn read_document_pages_through_a_large_document() {
    let mut app = app_with(&[("a.snxsch", SHEET)]);
    let by_id = format!("document={}", uid(SHEET));
    let first = answer(
        &mut app,
        "read_document",
        &[&by_id, "kinds=wire", "limit=2"],
    );
    assert_eq!(first["elements"]["wires"].as_array().unwrap().len(), 2);
    assert_eq!(first["next_offset"], 2);
    let last = answer(
        &mut app,
        "read_document",
        &[&by_id, "kinds=wire", "limit=2", "offset=2"],
    );
    assert_eq!(last["elements"]["wires"].as_array().unwrap().len(), 1);
    assert!(last["next_offset"].is_null());
    assert!(refusal(&mut app, "read_document", &[&by_id, "limit=0"]).contains("at least 1"));
    assert!(refusal(&mut app, "read_document", &[&by_id, "region=1,2"]).contains("not a region"));
}
