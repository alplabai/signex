//! The Bezier, dimension and table tools, the table edits and the bound
//! table refresh (#1267, #1300), driven through `Signex::update`.

use signex_types::schematic::{
    DimensionDirection, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, PinSort, Point,
    SchDrawing, SchTable, SchematicSheet, Symbol, TableSource,
};

use super::*;
use crate::app::handlers::canvas::placement_tests::{app_with_tool, click, doc};
use crate::app::{EditMsg, Tool};

fn undo(app: &mut Signex) {
    let _ = app.update(Message::Edit(EditMsg::Undo));
}

fn only_drawing(app: &Signex) -> &SchDrawing {
    let drawings = &doc(app).drawings;
    assert_eq!(drawings.len(), 1, "{drawings:?}");
    &drawings[0]
}

#[test]
fn four_bezier_clicks_place_one_curve_in_the_sheet_colour() {
    let mut app = app_with_tool(Tool::Bezier);
    for (i, p) in [(0.0, 0.0), (2.54, 10.16), (12.7, 10.16), (15.24, 0.0)]
        .into_iter()
        .enumerate()
    {
        click(&mut app, p);
        let placed = doc(&app).drawings.len();
        assert_eq!(placed, usize::from(i == 3), "after click {}", i + 1);
    }
    let SchDrawing::Bezier(b) = only_drawing(&app) else {
        panic!("a bezier");
    };
    assert_eq!(b.start, Point::new(0.0, 0.0));
    assert!((b.end.x - 15.24).abs() < 1e-6 && b.end.y.abs() < 1e-6);
    assert!((b.c1.y - 10.16).abs() < 1e-6);
    assert_eq!(
        b.stroke_color, None,
        "red only by an explicit pick (SCH-14)"
    );
    assert!(app.interaction_state.annotation_tool.points.is_empty());
    assert_eq!(
        app.interaction_state.current_tool,
        Tool::Bezier,
        "the tool stays armed"
    );
}

#[test]
fn a_linear_dimension_takes_two_points_then_its_line() {
    let mut app = app_with_tool(Tool::LinearDimension);
    let _ = app.update(Message::Tool(crate::app::ToolMessage::SelectTool(
        Tool::LinearDimension,
    )));
    click(&mut app, (0.0, 0.0));
    let draft = app
        .interaction_state
        .active_canvas()
        .annotation_draft
        .clone()
        .expect("the hint bar shows while placing");
    assert_eq!(draft.points, vec![Point::new(0.0, 0.0)]);
    assert!(!draft.hint.is_empty() && !draft.title.is_empty());
    click(&mut app, (12.7, 2.54));
    click(&mut app, (5.08, -5.08));
    let SchDrawing::Dimension(d) = only_drawing(&app) else {
        panic!("a dimension");
    };
    assert_eq!(
        d.direction,
        DimensionDirection::Horizontal,
        "dragged mostly sideways"
    );
    assert!((d.measured_mm() - 12.7).abs() < 1e-6);
    assert!(
        (d.line()[0].y + 5.08).abs() < 1e-6,
        "the line runs through the third click"
    );
    assert_eq!(d.label(), "12.70 mm");
}

#[test]
fn a_live_tag_shows_the_value_the_cursor_would_place() {
    let mut app = app_with_tool(Tool::AlignedDimension);
    let _ = app.update(Message::Tool(crate::app::ToolMessage::SelectTool(
        Tool::AlignedDimension,
    )));
    click(&mut app, (0.0, 0.0));
    let draft = app
        .interaction_state
        .active_canvas()
        .annotation_draft
        .clone()
        .expect("draft");
    let live = draft
        .dimension_at(Point::new(3.0, 4.0))
        .expect("a dimension");
    assert_eq!(live.direction, DimensionDirection::Aligned);
    assert_eq!(live.label(), "5.00 mm");
}

#[test]
fn escape_drops_the_half_placed_curve() {
    let mut app = app_with_tool(Tool::Bezier);
    click(&mut app, (0.0, 0.0));
    click(&mut app, (2.54, 2.54));
    app.clear_transient_schematic_tool_state();
    assert!(app.interaction_state.annotation_tool.points.is_empty());
    assert!(
        app.interaction_state
            .active_canvas()
            .annotation_draft
            .is_none()
    );
    assert!(doc(&app).drawings.is_empty());
}

#[test]
fn one_click_places_a_typed_three_by_three_table() {
    let mut app = app_with_tool(Tool::Table);
    click(&mut app, (25.4, 25.4));
    let SchDrawing::Table(t) = only_drawing(&app) else {
        panic!("a table");
    };
    assert_eq!(t.source, TableSource::Typed);
    assert_eq!(t.size(), (3, 3));
    assert!((t.position.x - 25.4).abs() < 1e-6 && (t.position.y - 25.4).abs() < 1e-6);
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}

fn table_id(app: &Signex) -> uuid::Uuid {
    only_drawing(app).uuid()
}

#[test]
fn a_cell_edit_and_a_column_toggle_are_one_undo_step_each() {
    let mut app = app_with_tool(Tool::Table);
    click(&mut app, (0.0, 0.0));
    let id = table_id(&app);
    let _ = app.update(Message::from(TablesMsg::CellEdited {
        table: id,
        row: 1,
        column: 2,
        text: "0x05".into(),
    }));
    let SchDrawing::Table(t) = only_drawing(&app) else {
        panic!("a table");
    };
    assert_eq!(t.cell(1, 2), "0x05");
    assert!(
        app.apply_annotation_edit_to(id, &AnnotationEdit::TableColumnShown("c1".into(), false))
    );
    let SchDrawing::Table(t) = only_drawing(&app) else {
        panic!("a table");
    };
    assert_eq!(t.size(), (3, 2), "the unchecked column is hidden");
    undo(&mut app);
    let SchDrawing::Table(t) = only_drawing(&app) else {
        panic!("a table");
    };
    assert_eq!(t.size(), (3, 3));
    assert_eq!(t.cell(1, 2), "0x05", "the cell edit was its own step");
    assert!(
        !app.apply_annotation_edit_to(id, &AnnotationEdit::TableColumnShown("c1".into(), true)),
        "showing a shown column changes nothing"
    );
}

#[test]
fn a_csv_import_replaces_columns_and_rows_in_one_step() {
    let mut app = app_with_tool(Tool::Table);
    click(&mut app, (0.0, 0.0));
    let id = table_id(&app);
    let read = Ok((
        vec!["Addr".to_string(), "Register".into()],
        vec![vec!["0x00".to_string(), "Configuration".into()]],
    ));
    let _ = app.update(Message::from(TablesMsg::CsvImportRead {
        table: id,
        picked: Some(("regs.csv".into(), read)),
    }));
    let SchDrawing::Table(t) = only_drawing(&app) else {
        panic!("a table");
    };
    assert_eq!(t.size(), (1, 2));
    assert_eq!(t.columns[1].title, "Register");
    undo(&mut app);
    let SchDrawing::Table(t) = only_drawing(&app) else {
        panic!("a table");
    };
    assert_eq!(t.size(), (3, 3), "one undo restores the typed table");
}

fn pin(number: &str, name: &str, direction: PinDirection) -> LibPin {
    LibPin {
        unit: 1,
        body_style: 1,
        pin: Pin {
            direction,
            shape_style: PinShapeStyle::Plain,
            position: Point::ZERO,
            rotation: 0.0,
            length: 2.54,
            name: name.into(),
            number: number.into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

/// A sheet with part U1 (pins 1 BOOT, 2 VIN) and a pin table for it whose
/// rows are stale.
fn sheet_with_pin_table() -> SchematicSheet {
    let mut body: LibSymbol = serde_json::from_str(r#"{"id":"U"}"#).expect("body");
    body.pins = vec![
        pin("2", "VIN", PinDirection::PowerInput),
        pin("1", "BOOT", PinDirection::Passive),
    ];
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert("U".to_string(), body);
    sheet.symbols.push(Symbol {
        uuid: uuid::Uuid::from_u128(7),
        lib_id: "U".into(),
        reference: "U1".into(),
        unit: 1,
        ..Default::default()
    });
    let mut table = SchTable::new_typed(uuid::Uuid::from_u128(8), Point::new(50.0, 50.0));
    table.source = TableSource::ComponentPins {
        designator: "U1".into(),
        sort: PinSort::Number,
    };
    table.columns = SchTable::columns_for(&table.source);
    table.rows = vec![vec!["1".into(), "OLD".into()]];
    sheet.drawings.push(SchDrawing::Table(table));
    sheet
}

fn app_with_sheet(sheet: SchematicSheet) -> Signex {
    let mut app = app_with_tool(Tool::Select);
    let path = app.document_state.active_path.clone().expect("active");
    app.document_state
        .engines
        .insert(path, signex_engine::Engine::new(sheet).expect("engine"));
    app
}

fn pin_table(app: &Signex) -> SchTable {
    doc(app)
        .drawings
        .iter()
        .find_map(|d| match d {
            SchDrawing::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("a table")
}

#[test]
fn the_refresh_rebuilds_a_pin_table_as_one_undo_step_and_only_when_stale() {
    let mut app = app_with_sheet(sheet_with_pin_table());
    app.refresh_bound_tables();
    let t = pin_table(&app);
    assert_eq!(t.cell(0, 0), "1", "sorted by pin number");
    assert_eq!(t.cell(0, 1), "BOOT");
    assert_eq!(t.cell(1, 1), "VIN");
    assert_eq!(t.cell(1, 3), "Power input");
    // Nothing changed since: no second step.
    app.refresh_bound_tables();
    undo(&mut app);
    assert_eq!(
        pin_table(&app).cell(0, 1),
        "OLD",
        "one undo undoes the refresh"
    );
}

#[test]
fn a_table_whose_part_is_gone_keeps_its_rows() {
    let mut sheet = sheet_with_pin_table();
    sheet.symbols.clear();
    let mut app = app_with_sheet(sheet);
    app.refresh_bound_tables();
    assert_eq!(pin_table(&app).cell(0, 1), "OLD");
    assert!(super::source_is_missing(doc(&app), &pin_table(&app)));
}

#[test]
fn contents_pages_follow_the_channel_choice() {
    let page = |name: &str, file: &str, channel: bool, repeat: bool| refresh::CompiledPage {
        name: name.into(),
        file: file.into(),
        is_channel: channel,
        is_repeat: repeat,
        title: format!("{name} title"),
        revision: "B".into(),
        date: String::new(),
    };
    let pages = refresh::CompiledPages {
        sheets: vec![
            page("top", "top.snxsch", false, false),
            page("U_BUCK", "buck_channel.snxsch", false, true),
            page("CH1", "buck_channel.snxsch", true, false),
            page("CH2", "buck_channel.snxsch", true, false),
        ],
    };
    let per_channel = pages.pages(true);
    assert_eq!(
        per_channel
            .iter()
            .map(|e| e.sheet.as_str())
            .collect::<Vec<_>>(),
        vec!["top", "buck_channel · CH1", "buck_channel · CH2"]
    );
    assert_eq!(per_channel[2].page, 3);
    let per_sheet = pages.pages(false);
    assert_eq!(per_sheet.len(), 2);
    assert_eq!(per_sheet[1].file, "buck_channel.snxsch");
}
