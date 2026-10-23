use signex_types::schematic::{
    ChildSheet, FillType, HAlign, Label, LabelType, Point, SchematicSheet, SheetPin, VAlign,
};

use super::{exposed_sheet_ports, ports_for_pin_sync};
use crate::test_support::test_sheet;

fn label(text: &str, label_type: LabelType, shape: &str) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        text: text.to_string(),
        position: Point::new(10.0, 10.0),
        rotation: 0.0,
        label_type,
        shape: shape.to_string(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    }
}

fn pin(name: &str, direction: &str) -> SheetPin {
    SheetPin {
        uuid: uuid::Uuid::new_v4(),
        name: name.to_string(),
        direction: direction.to_string(),
        position: Point::new(100.0, 102.0),
        rotation: 0.0,
        auto_generated: true,
        user_moved: false,
    }
}

fn symbol(filename: &str, pins: Vec<SheetPin>) -> ChildSheet {
    ChildSheet {
        uuid: uuid::Uuid::new_v4(),
        name: "Child".to_string(),
        filename: filename.to_string(),
        position: Point::new(100.0, 100.0),
        size: (30.0, 30.0),
        stroke_width: 0.12,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins,
        instances: Vec::new(),
    }
}

fn child_with(labels: Vec<Label>) -> SchematicSheet {
    let mut child = test_sheet();
    child.labels = labels;
    child
}

fn parent_with(symbols: Vec<ChildSheet>) -> SchematicSheet {
    let mut parent = test_sheet();
    parent.child_sheets = symbols;
    parent
}

fn direction_of<'a>(ports: &'a [crate::command::SheetPort], name: &str) -> Option<&'a str> {
    ports
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.direction.as_str())
}

#[test]
fn existing_pin_keeps_its_direction() {
    let child = child_with(vec![label("SDA", LabelType::Hierarchical, "input")]);
    let parent = parent_with(vec![symbol("c.snxsch", vec![pin("SDA", "output")])]);

    let ports = ports_for_pin_sync(&child, &parent, "c.snxsch");

    assert_eq!(direction_of(&ports, "SDA"), Some("output"));
}

#[test]
fn new_pin_takes_the_label_shape() {
    let child = child_with(vec![
        label("IN", LabelType::Hierarchical, "input"),
        label("OUT", LabelType::Global, "output"),
        label("TRI", LabelType::Hierarchical, "tri_state"),
    ]);
    let parent = parent_with(vec![symbol("c.snxsch", Vec::new())]);

    let ports = ports_for_pin_sync(&child, &parent, "c.snxsch");

    assert_eq!(direction_of(&ports, "IN"), Some("input"));
    assert_eq!(direction_of(&ports, "OUT"), Some("output"));
    assert_eq!(direction_of(&ports, "TRI"), Some("tri_state"));
}

#[test]
fn shapeless_or_unknown_shape_falls_back_to_bidirectional() {
    let child = child_with(vec![
        label("A", LabelType::Hierarchical, ""),
        label("B", LabelType::Hierarchical, "dot"),
    ]);
    let parent = parent_with(vec![symbol("c.snxsch", Vec::new())]);

    let ports = ports_for_pin_sync(&child, &parent, "c.snxsch");

    assert_eq!(direction_of(&ports, "A"), Some("bidirectional"));
    assert_eq!(direction_of(&ports, "B"), Some("bidirectional"));
}

#[test]
fn pin_direction_is_taken_from_any_symbol_of_the_same_child() {
    let child = child_with(vec![
        label("X", LabelType::Hierarchical, "input"),
        label("Y", LabelType::Hierarchical, "input"),
    ]);
    let parent = parent_with(vec![
        symbol("c.snxsch", vec![pin("X", "output")]),
        symbol("c.snxsch", vec![pin("Y", "passive")]),
        symbol("other.snxsch", vec![pin("X", "tri_state")]),
    ]);

    let ports = ports_for_pin_sync(&child, &parent, "c.snxsch");

    assert_eq!(direction_of(&ports, "X"), Some("output"));
    assert_eq!(direction_of(&ports, "Y"), Some("passive"));
}

#[test]
fn hierarchical_label_wins_over_global_of_the_same_name() {
    let child = child_with(vec![
        label("D", LabelType::Global, "output"),
        label("D", LabelType::Hierarchical, "input"),
    ]);

    let ports = exposed_sheet_ports(&child);

    assert_eq!(ports.len(), 1);
    assert_eq!(direction_of(&ports, "D"), Some("input"));
}

#[test]
fn padded_label_names_are_trimmed_and_blank_ones_dropped() {
    let child = child_with(vec![
        label(" CLK ", LabelType::Hierarchical, ""),
        label("   ", LabelType::Hierarchical, ""),
        label("LOCAL", LabelType::Net, ""),
    ]);
    let parent = parent_with(vec![symbol("c.snxsch", vec![pin("CLK", "input")])]);

    let ports = ports_for_pin_sync(&child, &parent, "c.snxsch");

    assert_eq!(ports.len(), 1);
    assert_eq!(direction_of(&ports, "CLK"), Some("input"));
}
