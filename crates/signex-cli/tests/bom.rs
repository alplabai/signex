//! End to end (#2938, #1297): the BOM alternates, `query_bom`,
//! `search_parts` and `change_component` through the real `signex-cli`
//! binary: each answers with JSON, the alternates reach the project file
//! only through a save, and a swapped part reaches the sheet the same way.

mod support;

use serde_json::json;
use support::{jsonl, pins_fixture, run_cli, temp_root};

/// The bundled stock library's id (`assets/stock-library/Stock.snxlib`).
const STOCK: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae";
const LIBRARY: &str = "019df45d-af5e-7a00-8c3b-000000000aaa";
const ROW: &str = "019df45d-af5e-7a00-8c3b-000000000bbb";

#[test]
fn alternates_and_the_priced_bom_run_headless_and_round_trip_through_save() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let project = fx.project.to_str().unwrap();
    let out = run_cli(
        &["run", project, "--save"],
        jsonl(&[
            json!({"id": "bom_add_alternate", "args": {
                "document": "demo.snxprj", "line": "mpn:bat54",
                "library": LIBRARY, "part": ROW
            }}),
            json!({"id": "query_bom", "args": {
                "document": "demo.snxprj", "quantity": "2", "currency": "USD"
            }}),
            json!({"id": "save_document", "args": {"document": "demo.snxprj"}}),
        ]),
    );
    let row = format!("{LIBRARY}/{ROW}");
    assert_eq!(out.lines[0]["status"], "answered", "{}", out.stdout);
    assert_eq!(
        out.lines[0]["data"],
        json!({"line": "mpn:BAT54", "alternates": [row], "changed": true}),
        "{}",
        out.stdout
    );
    let bom = &out.lines[1];
    assert_eq!(bom["status"], "answered", "{}", out.stdout);
    assert_eq!(bom["data"]["source"], "cache", "{}", out.stdout);
    assert_eq!(bom["data"]["quantity"], 2, "{}", out.stdout);
    assert_eq!(bom["data"]["currency"], "USD", "{}", out.stdout);
    assert!(
        !bom["data"]["lines"].as_array().expect("lines").is_empty(),
        "{}",
        out.stdout
    );
    // No part has a cached price: each line is reported, none counted.
    assert!(
        !bom["data"]["unpriced"]
            .as_array()
            .expect("unpriced")
            .is_empty(),
        "{}",
        out.stdout
    );
    assert_eq!(
        bom["data"]["total"],
        serde_json::Value::Null,
        "{}",
        out.stdout
    );
    assert_eq!(out.lines[2]["status"], "ran", "{}", out.stdout);
    let saved = signex_types::project::parse_project(&fx.project).expect("the saved project");
    assert_eq!(saved.bom_alternates_of("mpn:BAT54"), [row]);
}

#[test]
fn a_searched_part_swaps_a_placed_one_and_the_save_keeps_it() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let sheet = |fx: &support::Fixture| {
        signex_types::format::SnxSchematic::parse(&std::fs::read_to_string(&fx.sheet).unwrap())
            .expect("the sheet reads back")
            .sheet
    };
    let r1 = sheet(&fx)
        .symbols
        .iter()
        .find(|s| s.reference == "R1")
        .expect("R1")
        .uuid;
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--save"],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "main.snxsch"}}),
            json!({"id": "search_parts", "args": {"query": "STOCK-C"}}),
            json!({"id": "change_component", "args": {
                "document": "main.snxsch", "id": r1.to_string(),
                "library": STOCK, "part": "STOCK-C"
            }}),
            json!({"id": "save_document", "args": {"document": "main.snxsch"}}),
        ]),
    );
    let found = &out.lines[1];
    assert_eq!(found["status"], "answered", "{}", out.stdout);
    assert_eq!(found["data"]["source"], "cache", "{}", out.stdout);
    let matches = found["data"]["matches"].as_array().expect("matches");
    let stock_c = matches
        .iter()
        .find(|m| m["internal_pn"] == "STOCK-C")
        .unwrap_or_else(|| panic!("STOCK-C was not found: {}", out.stdout));
    assert_eq!(stock_c["library"], STOCK, "{}", out.stdout);

    let swapped = &out.lines[2];
    assert_eq!(swapped["status"], "answered", "{}", out.stdout);
    assert_eq!(swapped["data"]["designator"], "R1", "{}", out.stdout);
    assert_eq!(
        swapped["data"]["to"]["internal_pn"], "STOCK-C",
        "{}",
        out.stdout
    );
    assert_eq!(out.lines[3]["status"], "ran", "{}", out.stdout);
    let saved = sheet(&fx);
    let r1 = saved
        .symbols
        .iter()
        .find(|s| s.uuid == r1)
        .expect("R1 kept");
    assert_eq!(
        r1.library_id.map(|id| id.to_string()).as_deref(),
        Some(STOCK)
    );
    assert_eq!(
        r1.row_id.map(|id| id.to_string()),
        stock_c["part"].as_str().map(str::to_string)
    );
}
