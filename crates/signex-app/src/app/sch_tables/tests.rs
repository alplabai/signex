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
        "red only by an explicit pick (DECISIONS 2026-11-19)"
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
    let _ = app.update(Message::Tool(crate::toolbar::ToolMessage::SelectTool(
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
    let _ = app.update(Message::Tool(crate::toolbar::ToolMessage::SelectTool(
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
fn a_table_refuses_a_size_or_padding_it_cannot_store() {
    let mut app = app_with_tool(Tool::Table);
    click(&mut app, (0.0, 0.0));
    let id = table_id(&app);
    let before = only_drawing(&app).clone();
    for edit in [
        AnnotationEdit::TableTextSize(0.0),
        AnnotationEdit::TableTextSize(-3.0),
        AnnotationEdit::TableTextSize(0.49),
        AnnotationEdit::TableCellPadding(-1.0),
        AnnotationEdit::TableCellPadding(10.01),
    ] {
        assert!(!app.apply_annotation_edit_to(id, &edit), "{edit:?}");
        assert_eq!(*only_drawing(&app), before, "{edit:?}");
    }
    assert!(app.apply_annotation_edit_to(id, &AnnotationEdit::TableTextSize(0.5)));
    assert!(app.apply_annotation_edit_to(id, &AnnotationEdit::TableCellPadding(10.0)));
    let SchDrawing::Table(t) = only_drawing(&app) else {
        panic!("a table");
    };
    assert_eq!((t.style.text_size, t.style.cell_padding), (0.5, 10.0));
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
    let sheet = app.document_state.active_path.clone().expect("active");
    let _ = app.update(Message::from(TablesMsg::CsvImportRead {
        sheet,
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

/// The file dialog is async: an import answered after a tab switch lands on
/// the sheet Import… was pressed on, not on the sheet in view.
#[test]
fn a_csv_import_lands_on_its_own_sheet_after_a_tab_switch() {
    let mut app = app_with_tool(Tool::Table);
    click(&mut app, (0.0, 0.0));
    let id = table_id(&app);
    let sheet = app.document_state.active_path.clone().expect("active");
    let other = std::path::PathBuf::from("other.snxsch");
    app.document_state.engines.insert(
        other.clone(),
        signex_engine::Engine::new(SchematicSheet::default()).expect("engine"),
    );
    app.document_state.active_path = Some(other.clone());
    let read = Ok((vec!["Addr".to_string()], vec![vec!["0x10".to_string()]]));
    let _ = app.update(Message::from(TablesMsg::CsvImportRead {
        sheet: sheet.clone(),
        table: id,
        picked: Some(("regs.csv".into(), read)),
    }));
    let table = app.document_state.engines[&sheet]
        .document()
        .drawings
        .iter()
        .find_map(|d| match d {
            SchDrawing::Table(t) if t.uuid == id => Some(t.clone()),
            _ => None,
        })
        .expect("the table");
    assert_eq!(table.size(), (1, 1));
    assert_eq!(table.cell(0, 0), "0x10");
    assert!(
        app.document_state.engines[&other]
            .document()
            .drawings
            .is_empty(),
        "the sheet in view is untouched"
    );
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
    app.refresh_bound_tables(None);
    let t = pin_table(&app);
    assert_eq!(t.cell(0, 0), "1", "sorted by pin number");
    assert_eq!(t.cell(0, 1), "BOOT");
    assert_eq!(t.cell(1, 1), "VIN");
    assert_eq!(t.cell(1, 3), "Power input");
    // Nothing changed since: no second step.
    app.refresh_bound_tables(None);
    undo(&mut app);
    assert_eq!(
        pin_table(&app).cell(0, 1),
        "OLD",
        "one undo undoes the refresh"
    );
}

/// The refresh is an edit through the gateway: a sheet showing a channel tab
/// (a compiled view) and a read-only sheet refuse it.
#[test]
fn a_channel_view_or_a_read_only_sheet_refuses_the_refresh() {
    let _ = crate::diagnostics::init_logging();
    // The refusal says so in Messages, with a reason after the colon.
    let refresh_says_why = |app: &mut Signex, case: &str| {
        let before = crate::diagnostics::newest_entry_id();
        let said = crate::diagnostics::said_on_this_thread();
        app.refresh_bound_tables(None);
        assert!(
            crate::diagnostics::said_on_this_thread() > said,
            "{case}: nothing in Messages"
        );
        let lines: Vec<String> = crate::diagnostics::entries_since(before)
            .entries
            .into_iter()
            .map(|e| e.message.replace(['\u{2068}', '\u{2069}'], ""))
            .filter(|m| m.contains("Table refresh failed on"))
            .collect();
        assert!(
            lines.iter().any(|m| m
                .rsplit_once(": ")
                .is_some_and(|(_, why)| !why.trim().is_empty())),
            "{case}: {lines:?}"
        );
    };

    let mut app = app_with_sheet(sheet_with_pin_table());
    let path = app.document_state.active_path.clone().expect("active");
    app.ui_state.compiled_design.channel_tabs.insert(path, 1);
    refresh_says_why(&mut app, "a channel tab");
    assert_eq!(pin_table(&app).cell(0, 1), "OLD", "a channel tab");

    let mut sheet = sheet_with_pin_table();
    sheet.unknown.newer_minor_source = Some("snxsch/1.9".into());
    let mut app = app_with_sheet(sheet);
    refresh_says_why(&mut app, "a read-only sheet");
    assert_eq!(pin_table(&app).cell(0, 1), "OLD", "a read-only sheet");
}

#[test]
fn a_table_whose_part_is_gone_keeps_its_rows() {
    let mut sheet = sheet_with_pin_table();
    sheet.symbols.clear();
    let mut app = app_with_sheet(sheet);
    app.refresh_bound_tables(None);
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

fn contents_entry(page: usize, name: &str, title: &str, rev: &str) -> rows::ContentsEntry {
    rows::ContentsEntry {
        page,
        sheet: name.into(),
        title: title.into(),
        revision: rev.into(),
        last_changed: String::new(),
        file: format!("{name}.snxsch"),
    }
}

/// A contents table listing `top`, `bad` (Title Buck, Rev B) and `leaf`.
fn contents_table() -> SchTable {
    let mut table = SchTable::new_typed(uuid::Uuid::from_u128(9), Point::new(0.0, 0.0));
    table.source = TableSource::ProjectSheets { per_channel: true };
    table.columns = SchTable::columns_for(&table.source);
    let entries = [
        contents_entry(1, "top", "Top", "A"),
        contents_entry(2, "bad", "Buck", "B"),
        contents_entry(3, "leaf", "Leaf", "C"),
    ];
    (table.rows, table.links) = rows::contents_table_rows(&entries, &table.columns);
    table
}

/// What a compile that could not read `bad` lists: `leaf` moves up a page.
fn pages_without_bad(leaf_title: &str) -> refresh::Contents {
    vec![
        contents_entry(1, "top", "Top", "A"),
        contents_entry(2, "leaf", leaf_title, "C"),
    ]
}

fn title_cells(table: &SchTable) -> Vec<String> {
    let title = table
        .columns
        .iter()
        .position(|c| c.key == signex_types::schematic::sheet_columns::TITLE)
        .expect("a Title column");
    table.rows.iter().map(|r| r[title].clone()).collect()
}

/// The refresh after a compile that left a sheet unread.
fn rebuilt_unread(
    sheet: &SchematicSheet,
    table: &SchTable,
    pages: &dyn Fn(bool) -> Option<refresh::Contents>,
) -> Option<SchTable> {
    let unread = rows::UnreadSheets::default();
    refresh::rebuilt_table(sheet, table, pages, Some(&unread))
}

/// #2742: with a sheet unread nothing changed, so there is no edit at all.
#[test]
fn an_unread_sheet_keeps_its_row_and_the_refresh_changes_nothing() {
    let table = contents_table();
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(pages_without_bad("Leaf"));
    assert_eq!(
        rebuilt_unread(&sheet, &table, &pages),
        None,
        "same rows, same page numbers"
    );
}

/// #2742: the readable rows refresh around the kept one, which stays in its
/// place with its Title and Rev.
#[test]
fn readable_rows_refresh_around_an_unread_sheets_row() {
    let table = contents_table();
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(pages_without_bad("Leaf v2"));
    let next = rebuilt_unread(&sheet, &table, &pages).expect("leaf changed");
    assert_eq!(title_cells(&next), ["Top", "Buck", "Leaf v2"]);
    assert_eq!(next.rows[1], table.rows[1], "the kept row is unchanged");
    assert_eq!(next.links, table.links);
    assert_eq!(
        next.cell(2, 0),
        "3",
        "leaf keeps its page after the kept row"
    );
}

/// #2742: with every sheet read, a sheet no longer in the project leaves.
#[test]
fn a_removed_sheet_leaves_the_table_when_every_sheet_was_read() {
    let table = contents_table();
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(pages_without_bad("Leaf"));
    let next = refresh::rebuilt_table(&sheet, &table, &pages, None).expect("a row left");
    assert_eq!(title_cells(&next), ["Top", "Leaf"]);
    assert_eq!(next.links, ["top.snxsch", "leaf.snxsch"]);
}

/// #2742: an old bare link naming a sheet the compile reached is that sheet,
/// not a second row, while another sheet is unread.
#[test]
fn a_bare_link_to_a_reached_sheet_is_not_kept_as_a_second_row() {
    let mut table = contents_table();
    table.links[2] = "leaf.snxsch".into();
    let mut fresh = pages_without_bad("Leaf");
    fresh[1].file = "power/leaf.snxsch".into();
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(fresh.clone());
    // The compile found no `leaf.snxsch` beside the table: the link is legacy.
    let missing = [std::path::PathBuf::from("/p/leaf.snxsch")];
    let unread = rows::UnreadSheets {
        dir: Some(std::path::Path::new("/p")),
        missing_beside: &missing,
        ..Default::default()
    };
    let next = refresh::rebuilt_table(&sheet, &table, &pages, Some(&unread))
        .expect("the link is rewritten");
    assert_eq!(title_cells(&next), ["Top", "Buck", "Leaf"]);
    assert_eq!(next.links[2], "power/leaf.snxsch");
}

fn table_of(entries: &[rows::ContentsEntry]) -> SchTable {
    let mut table = SchTable::new_typed(uuid::Uuid::from_u128(9), Point::new(0.0, 0.0));
    table.source = TableSource::ProjectSheets { per_channel: true };
    table.columns = SchTable::columns_for(&table.source);
    (table.rows, table.links) = rows::contents_table_rows(entries, &table.columns);
    table
}

fn entry_of(page: usize, name: &str, file: &str, title: &str) -> rows::ContentsEntry {
    rows::ContentsEntry {
        file: file.into(),
        ..contents_entry(page, name, title, "A")
    }
}

/// #2742: two old rows store the same bare link for different sheets. The
/// compile reaches only the second, so the first keeps its row.
#[test]
fn a_bare_link_takes_a_fresh_row_only_for_the_same_sheet_name() {
    let table = table_of(&[
        entry_of(1, "top", "top.snxsch", "Top"),
        entry_of(2, "A leaf", "leaf.snxsch", "Leaf A"),
        entry_of(3, "B leaf", "leaf.snxsch", "Leaf B"),
    ]);
    let fresh = vec![
        entry_of(1, "top", "top.snxsch", "Top"),
        entry_of(2, "B leaf", "b/leaf.snxsch", "Leaf B"),
    ];
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(fresh.clone());
    let missing = [std::path::PathBuf::from("/p/leaf.snxsch")];
    let unread = rows::UnreadSheets {
        dir: Some(std::path::Path::new("/p")),
        missing_beside: &missing,
        ..Default::default()
    };
    let next = refresh::rebuilt_table(&sheet, &table, &pages, Some(&unread))
        .expect("the reached row takes its new link");
    assert_eq!(title_cells(&next), ["Top", "Leaf A", "Leaf B"]);
    assert_eq!(next.links[1], "leaf.snxsch");
    assert_eq!(next.links[2], "b/leaf.snxsch");
}

/// #2742: `top` places readable `A` and unread `B`, both place `common`. The
/// compile lists `common` once, which is one of the two old rows.
#[test]
fn a_reused_file_keeps_the_row_of_its_unread_placement() {
    let table = table_of(&[
        entry_of(1, "top", "top.snxsch", "Top"),
        entry_of(2, "A", "a.snxsch", "Sheet A"),
        entry_of(3, "common", "common.snxsch", "Common in A"),
        entry_of(4, "B", "b.snxsch", "Sheet B"),
        entry_of(5, "common", "common.snxsch", "Common in B"),
    ]);
    let fresh = vec![
        entry_of(1, "top", "top.snxsch", "Top"),
        entry_of(2, "A", "a.snxsch", "Sheet A"),
        entry_of(3, "common", "common.snxsch", "Common in A"),
    ];
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(fresh.clone());

    assert_eq!(
        rebuilt_unread(&sheet, &table, &pages),
        None,
        "every row and page number stays"
    );
}

/// #2742: the channels of a repeat under an unread parent keep their rows when
/// the same file's channels are reached through a readable parent.
#[test]
fn channel_rows_under_an_unread_parent_are_kept_one_for_one() {
    let table = table_of(&[
        entry_of(1, "top", "top.snxsch", "Top"),
        entry_of(2, "buck · CH1", "buck.snxsch", "Read CH1"),
        entry_of(3, "buck · CH2", "buck.snxsch", "Read CH2"),
        entry_of(4, "B", "b.snxsch", "Sheet B"),
        entry_of(5, "buck · CH1", "buck.snxsch", "Unread CH1"),
        entry_of(6, "buck · CH2", "buck.snxsch", "Unread CH2"),
    ]);
    let fresh = vec![
        entry_of(1, "top", "top.snxsch", "Top"),
        entry_of(2, "buck · CH1", "buck.snxsch", "Read CH1"),
        entry_of(3, "buck · CH2", "buck.snxsch", "Read CH2"),
    ];
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(fresh.clone());

    assert_eq!(rebuilt_unread(&sheet, &table, &pages), None);
}

/// `/p/bad.snxsch`, beside the table, is the sheet the compile could not read.
fn root_bad_unread<'a>(paths: &'a [std::path::PathBuf]) -> rows::UnreadSheets<'a> {
    rows::UnreadSheets {
        dir: Some(std::path::Path::new("/p")),
        paths,
        missing_beside: &[],
        fold: Default::default(),
    }
}

/// #2742: `bad.snxsch` exists beside the table, below a parent the compile
/// could not read, so the compile neither lists it nor knows it as unread. A
/// same-named sheet newly reached in another folder gets its own row.
#[test]
fn a_bare_link_to_a_file_beside_the_table_stays_below_an_unread_parent() {
    let table = table_of(&[entry_of(1, "bad", "bad.snxsch", "Root title")]);
    let fresh = vec![entry_of(1, "bad", "power/bad.snxsch", "Power title")];
    let paths = [std::path::PathBuf::from("/p/parent.snxsch")];
    let unread = root_bad_unread(&paths);
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(fresh.clone());

    let next = refresh::rebuilt_table(&sheet, &table, &pages, Some(&unread)).expect("a new row");

    assert_eq!(title_cells(&next), ["Root title", "Power title"]);
    assert_eq!(next.links, ["bad.snxsch", "power/bad.snxsch"]);
}

/// #2742: a table in the root lists `bad.snxsch` and `power/bad.snxsch`; the
/// root file is unread and only the other is reached. The bare link is the
/// sheet beside the table, so its row stays.
#[test]
fn a_bare_link_beside_the_table_is_not_a_same_named_sheet_in_another_folder() {
    let table = table_of(&[
        entry_of(1, "root bad", "bad.snxsch", "Root title"),
        entry_of(2, "power bad", "power/bad.snxsch", "Power title"),
    ]);
    let fresh = vec![entry_of(1, "power bad", "power/bad.snxsch", "Power title")];
    let paths = [std::path::PathBuf::from("/p/bad.snxsch")];
    let unread = root_bad_unread(&paths);
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(fresh.clone());

    assert_eq!(
        refresh::rebuilt_table(&sheet, &table, &pages, Some(&unread)),
        None
    );
}

/// #2742: the same when the table has no row yet for the sheet in the other
/// folder, so no exact link takes the fresh row first.
#[test]
fn a_bare_link_to_an_unread_sheet_does_not_take_a_new_sheets_row() {
    let table = table_of(&[entry_of(1, "bad", "bad.snxsch", "Root title")]);
    let fresh = vec![entry_of(1, "bad", "power/bad.snxsch", "Power title")];
    let paths = [std::path::PathBuf::from("/p/bad.snxsch")];
    let unread = root_bad_unread(&paths);
    let sheet = SchematicSheet::default();
    let pages = |_: bool| Some(fresh.clone());

    let next = refresh::rebuilt_table(&sheet, &table, &pages, Some(&unread)).expect("a new row");

    assert_eq!(title_cells(&next), ["Root title", "Power title"]);
    assert_eq!(next.links, ["bad.snxsch", "power/bad.snxsch"]);
}

/// #2742: through the real refresh, a compile with an unread sheet pushes no
/// undo step and leaves the table as it was.
#[test]
fn the_refresh_after_a_compile_with_an_unread_sheet_pushes_no_undo_step() {
    use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
    let mut app = app_workspace("/w/toc-unread", &["contents.snxsch"]);
    let path = std::path::PathBuf::from("/w/toc-unread/contents.snxsch");
    let table = contents_table();
    let mut sheet = SchematicSheet::default();
    sheet.drawings.push(SchDrawing::Table(table.clone()));
    open_with(&mut app.document_state, &path, sheet);
    let project = app.document_state.projects[0].id;
    app.document_state.compiled_design.results.insert(
        project,
        crate::app::core::compiled_design::CompiledResult {
            design: std::sync::Arc::new(signex_net::CompiledDesign::default()),
            paths: Default::default(),
            compiled_at: "12:41".to_string(),
            stale: Default::default(),
            has_unread_sheets: true,
            unread_paths: Vec::new(),
            bare_links_missing: Vec::new(),
        },
    );
    let steps = app.document_state.engines[&path].undo_position();

    app.refresh_bound_tables(Some(project));

    let engine = &app.document_state.engines[&path];
    assert_eq!(engine.undo_position(), steps);
    assert_eq!(engine.document().drawings[0], SchDrawing::Table(table));
}
