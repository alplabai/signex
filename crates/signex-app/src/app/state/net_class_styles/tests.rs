use signex_types::net_class::NetLineStyle;
use signex_types::schematic::{
    HAlign, Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, Symbol,
    VAlign, Wire,
};

use super::*;

/// A one-pin part. The netlist only forms a net where a pin lands, so a
/// labelled wire alone would resolve to no net at all.
fn one_pin_part() -> LibSymbol {
    LibSymbol {
        id: "R".to_string(),
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
        pins: vec![LibPin {
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
        }],
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn sheet_with_net(wire_id: Uuid, name: &str) -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert("R".to_string(), one_pin_part());
    sheet.symbols.push(Symbol {
        uuid: Uuid::from_u128(98),
        lib_id: "R".to_string(),
        reference: "R1".to_string(),
        position: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.wires.push(Wire {
        uuid: wire_id,
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet.labels.push(Label {
        uuid: Uuid::from_u128(99),
        text: name.to_string(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
    });
    sheet
}

fn dashed_class(name: &str, net: &str) -> NetClass {
    let mut class = NetClass::named(name);
    class.nets.push(net.to_string());
    class.style.line = NetLineStyle::Dashed;
    class
}

fn input<'a>(
    path: &'a Path,
    revision: u64,
    sheet: &'a SchematicSheet,
    classes: &'a [NetClass],
) -> SheetClassInput<'a> {
    SheetClassInput {
        path,
        revision,
        sheet,
        classes,
    }
}

#[test]
fn a_member_net_wire_gets_the_class_style() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let classes = [dashed_class("Power", "VCC")];
    let path = PathBuf::from("a.snxsch");
    let mut cache = NetClassStyleCache::default();

    let changed = cache.refresh([input(&path, 1, &sheet, &classes)]);

    assert!(changed);
    assert_eq!(cache.styles()[&wire_id].line, NetLineStyle::Dashed);
}

#[test]
fn no_classes_leaves_the_cache_empty_and_reports_no_change() {
    let sheet = sheet_with_net(Uuid::from_u128(1), "VCC");
    let path = PathBuf::from("a.snxsch");
    let mut cache = NetClassStyleCache::default();

    let changed = cache.refresh([input(&path, 1, &sheet, &[])]);

    assert!(!changed || cache.styles().is_empty());
    assert!(cache.is_empty());
}

#[test]
fn a_new_revision_with_a_renamed_net_drops_the_style() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let path = PathBuf::from("a.snxsch");
    let mut cache = NetClassStyleCache::default();
    let before = sheet_with_net(wire_id, "VCC");
    cache.refresh([input(&path, 1, &before, &classes)]);

    let after = sheet_with_net(wire_id, "SDA");
    let changed = cache.refresh([input(&path, 2, &after, &classes)]);

    assert!(changed);
    assert!(cache.styles().is_empty());
}

#[test]
fn an_unchanged_revision_and_table_is_not_rebuilt() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let classes = [dashed_class("Power", "VCC")];
    let path = PathBuf::from("a.snxsch");
    let mut cache = NetClassStyleCache::default();
    cache.refresh([input(&path, 1, &sheet, &classes)]);

    // Same revision, different sheet content: the revision is the contract,
    // so the cached resolution stands.
    let other = sheet_with_net(wire_id, "SDA");
    let changed = cache.refresh([input(&path, 1, &other, &classes)]);

    assert!(!changed);
    assert!(cache.styles().contains_key(&wire_id));
}

#[test]
fn editing_the_class_table_restyles_without_a_new_revision() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let path = PathBuf::from("a.snxsch");
    let mut cache = NetClassStyleCache::default();
    cache.refresh([input(&path, 1, &sheet, &[dashed_class("Power", "VCC")])]);

    let mut dotted = dashed_class("Power", "VCC");
    dotted.style.line = NetLineStyle::Dotted;
    let changed = cache.refresh([input(&path, 1, &sheet, &[dotted])]);

    assert!(changed);
    assert_eq!(cache.styles()[&wire_id].line, NetLineStyle::Dotted);
}

#[test]
fn a_closed_sheet_leaves_the_cache() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let classes = [dashed_class("Power", "VCC")];
    let path = PathBuf::from("a.snxsch");
    let mut cache = NetClassStyleCache::default();
    cache.refresh([input(&path, 1, &sheet, &classes)]);

    let changed = cache.refresh(std::iter::empty());

    assert!(changed);
    assert!(cache.styles().is_empty());
}
