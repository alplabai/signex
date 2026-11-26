//! End to end (#1313): `query_power_tree` through the real `signex-cli`
//! binary answers the project's power tree and budget per sheet from its
//! parts' rail parameters, refuses a variant the project does not have, and
//! writes nothing.

mod support;

use std::path::Path;

use serde_json::{Value, json};
use signex_types::format::SnxSchematic;
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

/// R1 supplies +3V3 (1 A) from VIN_5V at 90 % and C1 draws 0.45 A from
/// +3V3.
fn give_rail_parameters(sheet: &Path) {
    let text = std::fs::read_to_string(sheet).expect("the sheet");
    let mut sheet_file = SnxSchematic::parse(&text).expect("the sheet reads");
    for symbol in &mut sheet_file.sheet.symbols {
        let fields: Vec<(&str, &str)> = match symbol.reference.as_str() {
            "R1" => vec![
                ("Rail Supply +3V3", "1A"),
                ("Rail Load VIN_5V", "0.33"),
                ("Efficiency", "90 %"),
            ],
            "C1" => vec![("Rail Load +3V3", "450mA")],
            _ => Vec::new(),
        };
        for (key, value) in fields {
            symbol.fields.insert(key.to_string(), value.to_string());
        }
    }
    let text = SnxSchematic::new(sheet_file.sheet)
        .write_string()
        .expect("encode");
    std::fs::write(sheet, text).expect("write sheet");
}

fn rail<'a>(tree: &'a Value, net: &str) -> &'a Value {
    tree["rails"]
        .as_array()
        .expect("rails")
        .iter()
        .find(|rail| rail["net"] == net)
        .unwrap_or_else(|| panic!("no rail {net}: {tree}"))
}

#[test]
fn the_power_tree_is_answered_headless_and_nothing_is_written() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    give_rail_parameters(&fx.sheet);
    let before = without_journals(snapshot(&root));
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        jsonl(&[
            json!({"id": "query_power_tree", "args": {"document": "demo.snxprj"}}),
            json!({"id": "query_power_tree", "args": {"document": "main.snxsch",
                   "variant": "Lite"}}),
        ]),
    );
    let first = &out.lines[0];
    assert_eq!(first["status"], "answered", "{}", out.stdout);
    let tree = &first["data"];
    assert_eq!(tree["roots"], json!(["VIN_5V"]), "{tree}");

    let out_rail = rail(tree, "+3V3");
    assert_eq!(out_rail["voltage"], json!(3.3));
    assert_eq!(out_rail["sources"][0]["part"]["designator"], "R1");
    assert_eq!(out_rail["loads"][0]["part"]["designator"], "C1");
    assert_eq!(out_rail["total_a"], json!(0.45));
    assert_eq!(out_rail["limit_a"], json!(1.0));
    assert_eq!(out_rail["level"], "fine");
    assert_eq!(out_rail["fed_from"], json!(["VIN_5V"]));

    let input = rail(tree, "VIN_5V");
    assert_eq!(input["voltage"], json!(5.0));
    assert_eq!(input["loads"][0]["current_from"], "parameter");
    assert_eq!(input["problems"], json!([{"kind": "no_source"}]));

    let converter = &tree["converters"][0];
    assert_eq!(converter["part"]["designator"], "R1");
    assert_eq!(converter["efficiency"], json!(0.9));

    let sheets = tree["sheets"].as_array().expect("sheets");
    assert_eq!(sheets.len(), 1, "{tree}");
    assert_eq!(sheets[0]["sheet_path"], "main.snxsch");

    let second = &out.lines[1];
    assert_eq!(second["status"], "refused", "{}", out.stdout);
    let reason = second["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("no variant `Lite`"), "{reason}");

    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}
