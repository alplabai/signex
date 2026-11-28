//! The Enhanced Output exit criterion "a variant BOM export of a fixture
//! project matches its golden BOM" (docs/ROADMAP.md): a four-channel project
//! with two variants, Full and Lite, exported through the BOM dialog once
//! per variant, Base included, each file compared with its golden in
//! `testdata/variant_bom/`. The goldens are the written CSV with its UTF-8
//! byte order mark dropped and its CRLF line ends read as LF. Regenerate
//! them with `UPDATE_GOLDEN=1 cargo test -p signex-app variant_bom`; a diff
//! is a behaviour change of the BOM.
//!
//! The fixture: `top` holds the regulator U1 and the header J1 (not placed,
//! fitted in Full only) and places `channel` as `Repeat(CH, 1, 4)`; each
//! channel holds R5 and C7. Lite leaves C7 out of channels 3 and 4 and puts
//! a 1.8 V regulator in place of the 3.3 V one, set in the project's variant
//! settings.

use std::path::{Path, PathBuf};

use signex_types::project::{VALUE_PARAMETER, VariantPart};
use signex_types::schematic::{SchematicSheet, Symbol};

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::Signex;
use crate::app::handlers::menu::export::settle;

const DIR: &str = "/w/variant-bom";
const GOLDEN_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/app/handlers/menu/export/tests/testdata/variant_bom"
);

/// What a part shows in the BOM: value, footprint, description and part
/// number.
type Shown<'a> = (&'a str, &'a str, &'a str, &'a str);

/// Give `symbol` what it shows in the BOM.
fn fill(symbol: &mut Symbol, (value, footprint, description, part_number): Shown<'_>) {
    symbol.value = value.to_string();
    symbol.footprint = footprint.to_string();
    symbol
        .fields
        .insert("Description".to_string(), description.to_string());
    symbol
        .fields
        .insert("Part Number".to_string(), part_number.to_string());
}

/// One more part placed on `sheet`, with the lib symbol of `lib_id` (a copy
/// of the sheet's resistor body: the BOM reads none of it).
fn add_part(sheet: &mut SchematicSheet, reference: &str, lib_id: &str, shown: Shown<'_>) {
    if !sheet.lib_symbols.contains_key(lib_id) {
        let mut body = sheet
            .lib_symbols
            .get("Device:R")
            .cloned()
            .expect("the sheet's resistor body");
        body.id = lib_id.to_string();
        sheet.lib_symbols.insert(lib_id.to_string(), body);
    }
    let mut symbol = Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: lib_id.to_string(),
        reference: reference.to_string(),
        ..Default::default()
    };
    fill(&mut symbol, shown);
    sheet.symbols.push(symbol);
}

/// Give the part `sheet_with_net` placed what it shows in the BOM.
fn fill_first(sheet: &mut SchematicSheet, shown: Shown<'_>) {
    fill(sheet.symbols.first_mut().expect("a placed part"), shown);
}

fn not_fitted_in_lite(designator: &str) -> VariantPart {
    VariantPart {
        variant: "Lite".to_string(),
        designator: designator.to_string(),
        fitted: Some(false),
        ..Default::default()
    }
}

/// The fixture project, with `top` in view. The per-variant PDF goldens
/// (`variant_pdf.rs`) export the same project.
pub(super) fn fixture() -> Signex {
    let mut app = app_workspace(DIR, &["top.snxsch", "channel.snxsch"]);
    let top_path = PathBuf::from(DIR).join("top.snxsch");

    let mut top = sheet_with_net("U1", "VIN", &["channel.snxsch"]);
    top.child_sheets[0].name = "Repeat(CH, 1, 4)".to_string();
    fill_first(
        &mut top,
        ("TLV75533", "SOT-23-5", "LDO regulator", "TLV75533PDBVR"),
    );
    add_part(
        &mut top,
        "J1",
        "Connector:Conn_01x04",
        (
            "CONN_01x04",
            "PinHeader_1x04",
            "Header 1x4",
            "TSW-104-07-G-S",
        ),
    );
    let header = top.symbols.last_mut().expect("J1");
    header.dnp = true;
    header
        .fields
        .insert("Fitted@Full".to_string(), "yes".to_string());
    open_with(&mut app.document_state, &top_path, top);

    let mut channel = sheet_with_net("R5", "FB", &[]);
    fill_first(
        &mut channel,
        ("10k", "R_0603", "Resistor", "RC0603FR-0710KL"),
    );
    add_part(
        &mut channel,
        "C7",
        "Device:C",
        ("100n", "C_0603", "Capacitor", "GRM188R71C104KA01"),
    );
    open_with(
        &mut app.document_state,
        &PathBuf::from(DIR).join("channel.snxsch"),
        channel,
    );

    let data = &mut app.document_state.projects[0].data;
    data.variant_definitions = vec!["Full".to_string(), "Lite".to_string()];
    let mut regulator = VariantPart {
        variant: "Lite".to_string(),
        designator: "U1".to_string(),
        ..Default::default()
    };
    regulator
        .parameters
        .insert(VALUE_PARAMETER.to_string(), "TLV75518".to_string());
    regulator
        .parameters
        .insert("Part Number".to_string(), "TLV75518PDBVR".to_string());
    data.variant_settings.parts = vec![
        not_fitted_in_lite("CH3_C7"),
        not_fitted_in_lite("CH4_C7"),
        regulator,
    ];
    app.document_state.active_path = Some(top_path);
    app
}

/// The BOM the dialog writes with `variant` picked (`None` is Base), as
/// the golden spells it.
fn exported(app: &mut Signex, variant: Option<&str>) -> String {
    let _ = app.handle_bom_preview_open();
    app.handle_bom_preview_set_variant(variant.map(str::to_string));
    let preview = app.document_view.bom_preview.as_ref().expect("dialog open");
    let (source, options) = (preview.source.clone(), preview.options.clone());
    let dir = tempfile::tempdir().expect("a temp dir");
    let out = dir.path().join("bom.csv");
    let task = app.handle_export_bom_finished(source, options, Ok(out.clone()));
    settle(app, task);
    let bytes = std::fs::read(&out).expect("the BOM was written");
    let text = String::from_utf8(bytes).expect("UTF-8");
    let text = text
        .strip_prefix('\u{feff}')
        .expect("the CSV starts with a byte order mark");
    assert!(text.ends_with("\r\n"), "CRLF line ends");
    text.replace("\r\n", "\n")
}

fn golden(name: &str) -> PathBuf {
    Path::new(GOLDEN_DIR).join(format!("{name}.csv"))
}

fn assert_golden(name: &str, actual: &str) {
    let path = golden(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(GOLDEN_DIR).expect("the golden folder");
        std::fs::write(&path, actual).expect("write the golden");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|why| panic!("read {}: {why}", path.display()))
        .replace("\r\n", "\n");
    assert_eq!(
        actual,
        expected,
        "the {name} BOM differs from {}; a diff is a behaviour change",
        path.display()
    );
}

#[test]
fn each_variant_exports_its_golden_bom() {
    for (name, variant) in [
        ("base", None),
        ("full", Some("Full")),
        ("lite", Some("Lite")),
    ] {
        let mut app = fixture();
        let bom = exported(&mut app, variant);
        assert_golden(name, &bom);
    }
}

/// The goldens say what the variants are for, so a regenerated golden that
/// lost a variant's difference does not pass unseen.
#[test]
fn the_goldens_differ_where_the_variants_do() {
    let read = |name| std::fs::read_to_string(golden(name)).expect("a golden");
    let (base, full, lite) = (read("base"), read("full"), read("lite"));
    assert!(!base.contains("J1") && full.contains("J1") && !lite.contains("J1"));
    assert!(base.contains("CH4_C7") && !lite.contains("CH3_C7") && !lite.contains("CH4_C7"));
    assert!(base.contains("TLV75533") && lite.contains("TLV75518"));
    for bom in [&base, &full, &lite] {
        assert!(bom.contains("\"CH1_R5, CH2_R5, CH3_R5, CH4_R5\""), "{bom}");
    }
}
