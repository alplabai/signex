//! #2952: `list_diff_pairs` answers the project's differential pairs, by
//! directive or by name, as the CLI and an agent read them.

use std::path::PathBuf;

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_types::diff_pair_class::DiffPairClass;
use signex_types::schematic::{
    Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchDirective,
    SchematicSheet, Symbol, Wire,
};
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::keymap::AppCommandId;

const DIRECTIVE: u128 = 90;

/// A one-pin part, so a wire with a test point at each end is a net.
fn test_point_symbol() -> LibSymbol {
    let pin = LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            length: 0.0,
            name: String::new(),
            number: "1".to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    };
    LibSymbol {
        id: "TP".to_string(),
        reference: String::new(),
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
        pins: vec![pin],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// One named net per entry: a wire at y = 10·i, its label at the left end
/// and a test point at each end.
fn nets(names: &[&str]) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet
        .lib_symbols
        .insert("TP".to_string(), test_point_symbol());
    for (i, name) in names.iter().enumerate() {
        let y = 10.0 * i as f64;
        sheet.wires.push(Wire {
            uuid: Uuid::now_v7(),
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            ..Default::default()
        });
        sheet
            .labels
            .push(Label::new(*name, Point::new(0.0, y), LabelType::Net));
        for x in [0.0, 10.0] {
            sheet.symbols.push(Symbol {
                uuid: Uuid::now_v7(),
                lib_id: "TP".to_string(),
                reference: format!("TP{}", sheet.symbols.len() + 1),
                position: Point::new(x, y),
                ..Default::default()
            });
        }
    }
    sheet
}

/// Project `a.snxsch` + `b.snxsch`, the class `USB90`; `a` is open with a
/// USB pair its directive marks and a CLK pair found by name; `b` is not.
fn app(dir: &std::path::Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch", "b.snxsch"]);
    app.document_state.projects[0]
        .data
        .schematic_design
        .diff_pair_classes = vec![DiffPairClass::named("USB90")];
    let mut a = nets(&["USB_D_P", "USB_D_N", "CLK+", "CLK-"]);
    a.directives.push(SchDirective::DiffPair {
        uuid: Uuid::from_u128(DIRECTIVE),
        position: Point::new(5.0, 0.0),
        class: Some("USB90".to_string()),
    });
    open_with(&mut app.document_state, &dir.join("a.snxsch"), a);
    app
}

fn run(app: &mut Signex, document: &str) -> CommandOutcome {
    let command = AppCommandId::new("list_diff_pairs").expect("a catalog id");
    let operand = format!("document={document}");
    let args = CommandArgs::parse_operands(&command, &[operand.as_str()]).expect("operands");
    app.run_query(&command, &args).expect("a query").0
}

fn answer(app: &mut Signex, document: &str) -> Value {
    match run(app, document) {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

#[test]
fn the_pairs_of_the_project_are_listed_with_class_and_how_they_were_found() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app(dir.path());
    let project = app.document_state.projects[0].path.display().to_string();
    let value = answer(&mut app, &project);
    assert_eq!(value["count"], 2, "{value}");
    assert_eq!(value["unread_sheets"], true, "b.snxsch is not open");
    assert_eq!(
        value["pairs"],
        json!([
            {
                "name": "USB_D",
                "positive": "USB_D_P",
                "negative": "USB_D_N",
                "class": "USB90",
                "found_by": "directive",
                "directive": Uuid::from_u128(DIRECTIVE).to_string(),
                "sheet": "a.snxsch",
            },
            {
                "name": "CLK",
                "positive": "CLK+",
                "negative": "CLK-",
                "class": null,
                "found_by": "suffix",
                "directive": null,
                "sheet": "a.snxsch",
            },
        ])
    );
}

/// A sheet of the project names it as well as the project file does, and
/// the list is what `signex_net::listed_pairs` finds on the open sheets.
#[test]
fn a_sheet_of_the_project_names_the_same_list() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app(dir.path());
    let project = app.document_state.projects[0].path.display().to_string();
    let sheet: PathBuf = dir.path().join("a.snxsch");
    let by_project = answer(&mut app, &project);
    let by_sheet = answer(&mut app, &sheet.display().to_string());
    assert_eq!(by_project, by_sheet);
    let id = app.document_state.projects[0].id;
    let classes = vec![DiffPairClass::named("USB90")];
    let listed = signex_net::listed_pairs(&app.project_open_sheets(id), &classes);
    assert_eq!(by_project["count"], listed.len());
}

#[test]
fn a_document_outside_any_project_is_refused_with_the_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app(dir.path());
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let loose = elsewhere.path().join("loose.snxsch");
    let outcome = run(&mut app, &loose.display().to_string());
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(!reason.render_en_us().is_empty());
}
