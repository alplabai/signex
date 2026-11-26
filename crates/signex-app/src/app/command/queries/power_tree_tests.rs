//! #1313: `query_power_tree` answers the project's power tree off the UI
//! thread from a closed sheet's part parameters: each rail with its source,
//! loads, total, limit and budget level, the converters, the roots and the
//! budget per sheet. A variant leaves out the parts it does not fit; a
//! variant the project does not have is refused; nothing is changed.

use std::path::PathBuf;

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_types::format::SnxSchematic;
use signex_types::schematic::Symbol;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::project_name_tests::{child, project_app};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

const CHILD_ID: Uuid = Uuid::from_u128(0xc41d);

fn project_path(app: &Signex) -> PathBuf {
    app.document_state.projects[0].path.clone()
}

fn part(reference: &str, fields: &[(&str, &str)]) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:U".into(),
        reference: reference.into(),
        value: reference.into(),
        unit: 1,
        fields: fields
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        ..Symbol::default()
    }
}

/// The closed child sheet: J1 supplies VIN_12V (1.5 A), U1 draws 250 mA
/// from it and supplies +3V3 (3 A), U2 draws 640 mA from +3V3 and is not
/// fitted in Lite.
pub(crate) fn write_child_with_rails(dir: &tempfile::TempDir) {
    let mut sheet = child();
    sheet.uuid = CHILD_ID;
    sheet
        .symbols
        .push(part("J1", &[("Rail Supply VIN_12V", "1.5A")]));
    sheet.symbols.push(part(
        "U1",
        &[("Rail Load VIN_12V", "250mA"), ("Rail Supply +3V3", "3A")],
    ));
    sheet.symbols.push(part(
        "U2",
        &[("Rail Load +3V3", "640mA"), ("DNP@Lite", "yes")],
    ));
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.path().join("child.snxsch"), text).expect("write sheet");
}

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("query_power_tree").expect("a catalog id");
    let document = format!("document={}", project_path(app).display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    match app.run_query(&command, &args).expect("a query") {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    }
}

fn answer(app: &mut Signex, operands: &[&str]) -> Value {
    match run(app, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("query_power_tree did not answer: {other:?}"),
    }
}

fn rail<'a>(answer: &'a Value, net: &str) -> &'a Value {
    answer["rails"]
        .as_array()
        .expect("rails")
        .iter()
        .find(|rail| rail["net"] == net)
        .unwrap_or_else(|| panic!("no rail {net}: {answer}"))
}

fn close(value: &Value, expected: f64) -> bool {
    value.as_f64().is_some_and(|v| (v - expected).abs() < 1e-9)
}

#[test]
fn the_power_tree_is_answered_from_a_closed_sheet_and_nothing_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    write_child_with_rails(&dir);
    let path = project_path(&app);
    let on_disk = std::fs::read(&path).ok();
    let tree = answer(&mut app, &[]);

    // The child's VCC port is a rail of its own with nothing on it.
    let roots = tree["roots"].as_array().expect("roots");
    assert!(roots.contains(&json!("VIN_12V")), "{tree}");
    assert!(!roots.contains(&json!("+3V3")), "U1 feeds +3V3: {tree}");
    assert_eq!(tree["variant"], Value::Null);
    let out = rail(&tree, "+3V3");
    assert_eq!(out["voltage"], json!(3.3));
    assert_eq!(out["voltage_from"], "net_name");
    assert_eq!(out["sources"][0]["part"]["designator"], "U1");
    assert_eq!(out["sources"][0]["converter"], true);
    assert_eq!(out["sources"][0]["part"]["sheet"], CHILD_ID.to_string());
    assert_eq!(out["loads"][0]["part"]["designator"], "U2");
    assert_eq!(out["loads"][0]["current_from"], "parameter");
    assert!(close(&out["total_a"], 0.64), "{out}");
    assert!(close(&out["headroom_a"], 2.36), "{out}");
    assert_eq!(out["limit_a"], json!(3.0));
    assert_eq!(out["level"], "fine");
    assert_eq!(out["fed_from"], json!(["VIN_12V"]));
    assert_eq!(out["problems"], json!([]));

    let input = rail(&tree, "VIN_12V");
    assert_eq!(input["sources"][0]["part"]["designator"], "J1");
    assert_eq!(input["loads"][0]["part"]["designator"], "U1");
    assert!(close(&input["total_a"], 0.25), "{input}");
    assert_eq!(input["level"], "fine");

    let converter = &tree["converters"][0];
    assert_eq!(converter["part"]["designator"], "U1");
    assert_eq!(converter["inputs"], json!(["VIN_12V"]));
    assert_eq!(converter["outputs"], json!(["+3V3"]));
    assert_eq!(converter["efficiency"], Value::Null, "no Efficiency given");

    let sheets = tree["sheet_budgets"].as_array().expect("sheet_budgets");
    assert_eq!(sheets.len(), 1, "{tree}");
    assert_eq!(sheets[0]["sheet"], CHILD_ID.to_string());
    assert_eq!(sheets[0]["sheet_path"], "child.snxsch");
    let draws: Vec<&str> = sheets[0]["draws"]
        .as_array()
        .unwrap()
        .iter()
        .map(|draw| draw["net"].as_str().unwrap())
        .collect();
    assert_eq!(draws, ["+3V3", "VIN_12V"]);

    assert!(!app.document_state.is_document_dirty(&path));
    assert_eq!(std::fs::read(&path).ok(), on_disk, "nothing is written");
}

#[test]
fn a_variant_leaves_out_what_it_does_not_fit_and_an_unknown_one_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    write_child_with_rails(&dir);
    app.document_state.projects[0]
        .data
        .variant_definitions
        .push("Lite".into());

    let lite = answer(&mut app, &["variant=Lite"]);
    assert_eq!(lite["variant"], "Lite");
    let out = rail(&lite, "+3V3");
    assert_eq!(out["loads"], json!([]), "U2 is not fitted in Lite: {out}");
    assert!(close(&out["total_a"], 0.0));

    let base = answer(&mut app, &["variant=Base"]);
    assert_eq!(base["variant"], Value::Null);
    assert_eq!(rail(&base, "+3V3")["loads"][0]["part"]["designator"], "U2");

    let refused = match run(&mut app, &["variant=Full"]) {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    };
    assert!(refused.contains("no variant `Full`"), "{refused}");
}
