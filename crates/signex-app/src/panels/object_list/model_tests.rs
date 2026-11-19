//! #1259: the Object List's row builder, filter, counts and selection marks.

use std::path::{Path, PathBuf};

use signex_types::schematic::{
    ChildSheet, Junction, Label, LabelType, Point, SchDirective, SchematicSheet, SelectedItem,
    SelectedKind, SheetPin, Symbol, TextNote, Wire,
};
use signex_widgets::chips::FilterSet;
use uuid::Uuid;

use super::model::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn part(n: u128, reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: id(n),
        reference: reference.into(),
        value: value.into(),
        position: Point::new(n as f64, 0.0),
        ..Symbol::default()
    }
}

/// A sheet with two parts, a power port, a net label, a port, a sheet
/// symbol with one entry, a wire, a text note, a junction and a directive.
pub(super) fn power_sheet() -> SchematicSheet {
    let mut s = SchematicSheet::default();
    s.symbols.push(part(1, "U3", "INA226"));
    s.symbols.push(part(2, "R3", "100k"));
    s.symbols.push(Symbol {
        is_power: true,
        ..part(3, "#PWR1", "GND")
    });
    s.labels.push(Label {
        uuid: id(4),
        ..Label::new("VOUT_3V3", Point::new(1.0, 1.0), LabelType::Net)
    });
    s.labels.push(Label {
        uuid: id(5),
        ..Label::new("SDA", Point::new(2.0, 1.0), LabelType::Global)
    });
    s.child_sheets.push(ChildSheet {
        uuid: id(6),
        name: "Input".into(),
        filename: "input.snxsch".into(),
        position: Point::new(10.0, 10.0),
        size: (20.0, 10.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: id(7),
            name: "VIN".into(),
            direction: String::new(),
            position: Point::new(10.0, 12.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    s.wires.push(Wire {
        uuid: id(8),
        start: Point::new(0.0, 0.0),
        end: Point::new(12.7, 0.0),
        ..Default::default()
    });
    s.text_notes.push(TextNote {
        uuid: id(9),
        text: "Place C4 within 3 mm of U1".into(),
        position: Point::new(5.0, 5.0),
        rotation: 0.0,
        font_size: 0.0,
        font_family: None,
        justify_h: Default::default(),
        justify_v: Default::default(),
        locked: false,
    });
    s.junctions.push(Junction {
        uuid: id(10),
        position: Point::new(0.0, 0.0),
        ..Default::default()
    });
    s.directives.push(SchDirective::DiffPair {
        uuid: id(11),
        position: Point::new(3.0, 3.0),
    });
    s
}

fn power_path() -> PathBuf {
    PathBuf::from("proj").join("Power.snxsch")
}

fn rows_with_nets() -> Vec<ObjectRow> {
    sheet_rows(&power_sheet(), &power_path(), &|wire: Uuid| {
        (wire == id(8)).then(|| "SW".to_string())
    })
}

fn row(rows: &[ObjectRow], uuid: u128) -> &ObjectRow {
    rows.iter()
        .find(|r| r.target.item.uuid == id(uuid))
        .expect("the object is listed")
}

#[test]
fn every_object_gets_its_kind_name_value_and_sheet() {
    let rows = rows_with_nets();
    assert_eq!(rows.len(), 11);
    assert!(rows.iter().all(|r| r.sheet == "Power"));
    assert!(rows.iter().all(|r| r.target.sheet == power_path()));

    let u3 = row(&rows, 1);
    assert_eq!(u3.kind, ObjectKind::Part);
    assert_eq!(u3.name, RowName::Text("U3".into()));
    assert_eq!(u3.value, RowValue::Text("INA226".into()));
    assert_eq!(
        u3.target.item,
        SelectedItem::new(id(1), SelectedKind::Symbol)
    );

    let gnd = row(&rows, 3);
    assert_eq!(gnd.kind, ObjectKind::PowerPort);
    assert_eq!(gnd.name, RowName::Text("GND".into()));
    assert_eq!(gnd.value, RowValue::Word(ValueWord::PowerPort));

    let label = row(&rows, 4);
    assert_eq!(label.kind, ObjectKind::Label);
    assert_eq!(label.value, RowValue::Word(ValueWord::NetLabel));
    assert_eq!(label.target.item.kind, SelectedKind::Label);
    assert_eq!(row(&rows, 5).kind, ObjectKind::Port);

    let sheet = row(&rows, 6);
    assert_eq!(sheet.kind, ObjectKind::SheetSymbol);
    assert_eq!(sheet.name, RowName::Text("Input".into()));
    assert_eq!(sheet.value, RowValue::Text("input.snxsch".into()));
    assert_eq!(sheet.target.item.kind, SelectedKind::ChildSheet);
    let entry = row(&rows, 7);
    assert_eq!(entry.kind, ObjectKind::SheetEntry);
    assert_eq!(entry.target.item.kind, SelectedKind::SheetPin);

    let text = row(&rows, 9);
    assert_eq!(text.name, RowName::Noun(ObjectNoun::Text));
    assert_eq!(
        text.value,
        RowValue::Text("Place C4 within 3 mm of U1".into())
    );
    assert_eq!(row(&rows, 10).kind, ObjectKind::Junction);
    assert_eq!(row(&rows, 11).name, RowName::Noun(ObjectNoun::DiffPair));
}

#[test]
fn a_wire_reads_its_net_from_the_index_and_its_length() {
    let rows = rows_with_nets();
    let wire = row(&rows, 8);
    assert_eq!(wire.kind, ObjectKind::Wire);
    assert_eq!(wire.name, RowName::Noun(ObjectNoun::Wire));
    let RowValue::Length { net, mm } = &wire.value else {
        panic!("a wire shows its length: {:?}", wire.value);
    };
    assert_eq!(net.as_deref(), Some("SW"));
    assert!((mm - 12.7).abs() < 1e-9);
    assert_eq!(wire.target.at, Point::new(6.35, 0.0));

    // No index: the net is left out, not derived here.
    let bare = sheet_rows(&power_sheet(), &power_path(), &|_: Uuid| None);
    assert!(matches!(
        row(&bare, 8).value,
        RowValue::Length { net: None, .. }
    ));
}

#[test]
fn kind_counts_follow_the_filter_but_not_the_chips() {
    let rows = rows_with_nets();
    let counts = kind_counts(&rows, "");
    assert_eq!(counts.total(), 11);
    assert_eq!(counts.of(ObjectKind::Part), 2);
    assert_eq!(counts.of(ObjectKind::Label), 1);
    assert_eq!(counts.of(ObjectKind::Wire), 1);
    assert_eq!(counts.more(), 7);

    let counts = kind_counts(&rows, "u3");
    assert_eq!(counts.of(ObjectKind::Part), 1);
    assert_eq!(counts.total(), 1);
}

#[test]
fn the_filter_matches_name_value_text_and_net_without_case() {
    let rows = rows_with_nets();
    let all = FilterSet::default();
    let names = |filter: &str| -> Vec<Uuid> {
        filtered(&rows, filter, &all)
            .iter()
            .map(|r| r.target.item.uuid)
            .collect()
    };
    assert_eq!(names("r3"), vec![id(2)], "a designator");
    assert_eq!(names("ina"), vec![id(1)], "a value");
    assert_eq!(names("WITHIN 3"), vec![id(9)], "a text note");
    assert_eq!(names("sw"), vec![id(8)], "a wire's net");
    assert_eq!(names("  vout  "), vec![id(4)], "trimmed");
    assert_eq!(filtered(&rows, "", &all).len(), 11);
}

#[test]
fn kind_chips_keep_their_kinds_and_none_on_is_all() {
    let rows = rows_with_nets();
    let parts = FilterSet::new([ObjectKind::Part]);
    assert!(
        filtered(&rows, "", &parts)
            .iter()
            .all(|r| r.kind == ObjectKind::Part)
    );
    assert_eq!(filtered(&rows, "", &parts).len(), 2);
    let two = FilterSet::new([ObjectKind::Part, ObjectKind::Wire]);
    assert_eq!(filtered(&rows, "", &two).len(), 3);
}

fn state_with(rows: Vec<ObjectRow>) -> ObjectListState {
    let mut state = ObjectListState::default();
    state.source.rows = std::sync::Arc::new(rows);
    state
}

#[test]
fn the_view_marks_the_rows_selected_on_the_sheet_in_view() {
    let state = state_with(rows_with_nets());
    let selected = [
        SelectedItem::new(id(1), SelectedKind::Symbol),
        SelectedItem::new(id(2), SelectedKind::SymbolRefField),
    ];
    let view = ObjectListView::build(
        &state,
        ObjectListStatus::Ready,
        Some(power_path()),
        &selected,
    );
    assert!(view.is_selected(row(&view.rows, 1)));
    assert!(
        view.is_selected(row(&view.rows, 2)),
        "a field selects its part"
    );
    assert!(!view.is_selected(row(&view.rows, 3)));
    assert_eq!(view.selected_count, 2);

    // The same uuids on another sheet are not this sheet's selection.
    let elsewhere = ObjectListView::build(
        &state,
        ObjectListStatus::Ready,
        Some(Path::new("proj").join("Input.snxsch")),
        &selected,
    );
    assert_eq!(elsewhere.selected_count, 0);
}

#[test]
fn the_view_draws_a_page_of_rows_and_counts_before_the_chips() {
    let mut state = state_with(rows_with_nets());
    state.kinds = FilterSet::new([ObjectKind::Wire]);
    let view = ObjectListView::build(&state, ObjectListStatus::Ready, Some(power_path()), &[]);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(
        view.counts.total(),
        11,
        "the chips do not narrow the counts"
    );
    assert_eq!(view.row_limit, ROW_PAGE);
}
