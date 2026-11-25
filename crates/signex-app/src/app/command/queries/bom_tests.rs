//! #2938, #1297: `query_bom` prices the project's BOM off the UI thread
//! from the cached prices (a priced line, a line with no price, a line
//! served by an alternate, a variant), and `search_parts` finds library
//! rows by the panel's words and by their parameters, with their cached
//! offers, never fetching one.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use chrono::{Duration, Utc};
use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_library::distributor::PriceBreak;
use signex_library::{
    CachedPricing, ComponentRow, DistributorListing, InternalPn, LifecycleState, ParamValue,
    RowPricing,
};
use signex_types::format::SnxSchematic;
use signex_types::project::BomLineAlternates;
use signex_types::schematic::Symbol;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::project_name_tests::{child, project_app};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;
use crate::library::state::{LibraryDisplaySettings, OpenLibrary};

/// The rows of the test library, by part number.
struct Rows {
    library: Uuid,
    resistor: Uuid,
    capacitor: Uuid,
    regulator: Uuid,
    alternate: Uuid,
}

fn row(pn: &str, sku: &str, parameters: &[(&str, ParamValue)]) -> ComponentRow {
    let mut row = crate::library::component_preview::test_row();
    row.internal_pn = InternalPn::new(pn);
    row.primary_mpn = signex_library::ManufacturerPart::draft("Acme", format!("{pn}-MPN"));
    row.supply = vec![DistributorListing::new("Distributor A", sku)];
    row.state = LifecycleState::Released;
    row.parameters = parameters
        .iter()
        .map(|(name, value)| ((*name).to_string(), value.clone()))
        .collect();
    row
}

fn ohms(value: f64) -> ParamValue {
    ParamValue::Measurement {
        value,
        unit: "ohm".into(),
    }
}

fn priced(sku: &str, stock: u32, breaks: &[(u32, f64)]) -> RowPricing {
    RowPricing::from([(
        "Distributor A".to_string(),
        BTreeMap::from([(
            sku.to_string(),
            CachedPricing {
                sku: sku.into(),
                currency: "USD".into(),
                stock: Some(stock),
                breaks: breaks
                    .iter()
                    .map(|(qty, unit_price)| PriceBreak {
                        qty: *qty,
                        unit_price: *unit_price,
                    })
                    .collect(),
                fetched_at: Utc::now() - Duration::days(2),
            },
        )]),
    )])
}

/// A library open in memory: a 10k resistor priced and in stock, a
/// capacitor with no cached price, a regulator priced but out of stock, and
/// its alternate, priced and in stock.
fn mount_library(app: &mut Signex) -> Rows {
    let resistor = row(
        "R-10K",
        "DK-R",
        &[
            ("value", ohms(10_000.0)),
            ("package", ParamValue::Text("0603".into())),
            (
                "tolerance",
                ParamValue::Measurement {
                    value: 1.0,
                    unit: "%".into(),
                },
            ),
            (
                "power",
                ParamValue::Measurement {
                    value: 0.1,
                    unit: "W".into(),
                },
            ),
        ],
    );
    let capacitor = row("C-100N", "DK-C", &[]);
    let regulator = row("U-LDO", "DK-U", &[]);
    let alternate = row("U-LDO-ALT", "DK-A", &[]);
    let rows = Rows {
        library: Uuid::now_v7(),
        resistor: resistor.row_id,
        capacitor: capacitor.row_id,
        regulator: regulator.row_id,
        alternate: alternate.row_id,
    };
    let pricing = HashMap::from([
        (
            rows.resistor,
            priced("DK-R", 1000, &[(1, 0.10), (10, 0.05)]),
        ),
        (rows.regulator, priced("DK-U", 0, &[(1, 2.00)])),
        (rows.alternate, priced("DK-A", 50, &[(1, 2.50)])),
    ]);
    // Only this library is open, so the counts below are its own.
    app.library.open_libraries.clear();
    app.library.open_libraries.push(OpenLibrary {
        root: PathBuf::from("/tmp/bom-query/P.snxlib"),
        display_name: "P".into(),
        library_id: rows.library,
        tables: HashMap::from([(
            "Parts".to_string(),
            vec![resistor, capacitor, regulator, alternate],
        )]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing,
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    rows
}

fn part(reference: &str, value: &str, library: Uuid, row: Uuid) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".into(),
        reference: reference.into(),
        value: value.into(),
        unit: 1,
        library_id: Some(library),
        row_id: Some(row),
        ..Symbol::default()
    }
}

/// The closed child sheet with R1 and R2 (the resistor), C1 (the
/// capacitor, not fitted in variant `Lite`) and U1 (the regulator).
fn write_child(dir: &tempfile::TempDir, rows: &Rows) {
    let mut sheet = child();
    sheet
        .symbols
        .push(part("R1", "10k", rows.library, rows.resistor));
    sheet
        .symbols
        .push(part("R2", "10k", rows.library, rows.resistor));
    let mut c1 = part("C1", "100n", rows.library, rows.capacitor);
    c1.fields.insert("dnp@Lite".into(), "true".into());
    sheet.symbols.push(c1);
    sheet
        .symbols
        .push(part("U1", "LDO", rows.library, rows.regulator));
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.path().join("child.snxsch"), text).expect("write sheet");
}

fn key(rows: &Rows, row: Uuid) -> String {
    format!("{}/{row}", rows.library)
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands parse");
    match app.run_query(&id, &args).expect("a query") {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    }
}

fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

fn refusal(app: &mut Signex, id: &str, operands: &[&str]) -> String {
    match run(app, id, operands) {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("{id} was not refused: {other:?}"),
    }
}

/// The line of `bom` whose designators are `designators`.
fn line<'a>(bom: &'a Value, designators: &[&str]) -> &'a Value {
    bom["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .find(|line| line["designators"] == json!(designators))
        .unwrap_or_else(|| panic!("no line {designators:?} in {bom}"))
}

fn bom_app(dir: &tempfile::TempDir) -> (Signex, Rows, String) {
    let mut app = project_app(dir);
    let rows = mount_library(&mut app);
    write_child(dir, &rows);
    let data = &mut app.document_state.projects[0].data;
    data.variant_definitions.push("Lite".into());
    data.bom_alternates = vec![BomLineAlternates {
        line: key(&rows, rows.regulator),
        rows: vec![key(&rows, rows.alternate)],
    }];
    let document = format!("document={}", app.document_state.projects[0].path.display());
    (app, rows, document)
}

#[test]
fn query_bom_prices_each_line_for_the_build_and_reports_an_unpriced_one() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, rows, document) = bom_app(&dir);
    let bom = answer(
        &mut app,
        "query_bom",
        &[&document, "quantity=5", "currency=usd"],
    );
    assert_eq!(bom["source"], "cache");
    assert_eq!(
        (bom["quantity"].clone(), bom["currency"].clone()),
        (json!(5), json!("USD"))
    );

    let resistors = line(&bom, &["R1", "R2"]);
    assert_eq!(resistors["key"], json!(key(&rows, rows.resistor)));
    assert_eq!(resistors["value"], "10k");
    assert_eq!(
        (
            resistors["quantity"].clone(),
            resistors["build_quantity"].clone()
        ),
        (json!(2), json!(10))
    );
    assert_eq!(
        resistors["price"]["unit_price"], 0.05,
        "10 parts at the 10 break"
    );
    assert_eq!(resistors["price"]["extended_price"], 0.5);
    assert_eq!(resistors["unpriced"], Value::Null);

    let capacitor = line(&bom, &["C1"]);
    assert_eq!(capacitor["price"], Value::Null);
    assert_eq!(capacitor["unpriced"], "no_price");

    let regulator = line(&bom, &["U1"]);
    assert_eq!(regulator["unpriced"], Value::Null, "priced, so counted");
    assert_eq!(
        regulator["alternates"][0]["row"],
        json!(key(&rows, rows.alternate))
    );
    assert_eq!(
        regulator["served_by"]["row"],
        json!(key(&rows, rows.alternate))
    );
    assert_eq!(regulator["served_by"]["reason"], "no_stock");
    assert_eq!(regulator["served_by"]["price"]["extended_price"], 12.5);

    assert_eq!(bom["unpriced"], json!(["C1"]));
    assert_eq!(bom["total"], Value::Null, "an unpriced line: no total");
    assert_eq!(bom["known_total"], 10.5);
}

#[test]
fn query_bom_of_a_variant_leaves_out_its_unfitted_parts() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _rows, document) = bom_app(&dir);
    let bom = answer(
        &mut app,
        "query_bom",
        &[&document, "variant=Lite", "currency=USD"],
    );
    assert_eq!(bom["variant"], "Lite");
    let designators: Vec<Value> = bom["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line["designators"].clone())
        .collect();
    assert_eq!(designators, [json!(["R1", "R2"]), json!(["U1"])]);
    // Every line is priced: a total and a unit cost, for one board.
    assert_eq!(bom["total"], 2.2);
    assert_eq!(bom["unit_cost"], 2.2);
    let reason = refusal(&mut app, "query_bom", &[&document, "variant=Pro"]);
    assert!(reason.contains("no variant `Pro`"), "{reason}");
    let reason = refusal(&mut app, "query_bom", &[&document, "quantity=0"]);
    assert!(reason.contains("whole number from 1"), "{reason}");
}

#[test]
fn search_parts_matches_words_and_parameters_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let rows = mount_library(&mut app);
    let found = answer(
        &mut app,
        "search_parts",
        &[
            "query=10k",
            "package=0603",
            "tolerance=1%",
            "rating=0.0625W",
        ],
    );
    assert_eq!(found["source"], "cache");
    assert_eq!(found["count"], 1);
    let resistor = &found["matches"][0];
    assert_eq!(resistor["part"], json!(rows.resistor.to_string()));
    assert_eq!(resistor["library"], json!(rows.library.to_string()));
    assert_eq!(resistor["internal_pn"], "R-10K");
    assert_eq!(resistor["mpn"], "R-10K-MPN");
    assert_eq!(resistor["lifecycle"], "released");
    assert_eq!(resistor["stock"], 1000);
    assert_eq!(resistor["offers"][0]["distributor"], "Distributor A");
    assert_eq!(resistor["offers"][0]["unit_price"], 0.1);
    assert_eq!(resistor["offers"][0]["age"], "2 d ago");

    let tighter = answer(&mut app, "search_parts", &["query=10k", "tolerance=0.5%"]);
    assert_eq!(tighter["count"], 0, "1% is above 0.5%");
    let stronger = answer(&mut app, "search_parts", &["query=10k", "rating=0.25W"]);
    assert_eq!(stronger["count"], 0, "0.1 W is below 0.25 W");
    let by_value = answer(&mut app, "search_parts", &["value=10k"]);
    assert_eq!(by_value["count"], 1);
    let in_stock = answer(&mut app, "search_parts", &["query=U-LDO", "min_stock=1"]);
    assert_eq!(in_stock["count"], 1, "only the alternate is in stock");
    assert_eq!(
        in_stock["matches"][0]["part"],
        json!(rows.alternate.to_string())
    );
    let all = answer(&mut app, "search_parts", &["limit=2"]);
    assert_eq!(
        (all["count"].clone(), all["truncated"].clone()),
        (json!(4), json!(true))
    );
    assert_eq!(all["matches"].as_array().unwrap().len(), 2);

    let reason = refusal(&mut app, "search_parts", &["query=10k", "refresh=true"]);
    assert!(
        reason.contains("search the cached supplier data"),
        "{reason}"
    );
    let reason = refusal(&mut app, "search_parts", &["query=10k", "rating=high"]);
    assert!(reason.contains("`rating` is a number"), "{reason}");
}
