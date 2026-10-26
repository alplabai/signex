use std::path::PathBuf;

use signex_types::net_class::NetLineStyle;
use signex_types::schematic::{
    HAlign, Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
    SchematicSheet, Symbol, VAlign, Wire,
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

const PROJECT: ProjectId = ProjectId(1);

/// An input whose project netlist is the sheet's own (one-sheet project).
fn input<'a>(
    revision: u64,
    sheet: &'a SchematicSheet,
    classes: &'a [NetClass],
) -> ProjectClassInput<'a> {
    ProjectClassInput {
        project: PROJECT,
        revision,
        classes,
        netlist: Box::new(move || Some(signex_net::build_netlist(sheet))),
    }
}

#[test]
fn a_member_net_wire_gets_the_class_style() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();

    let changed = cache.refresh([input(1, &sheet, &classes)]);

    assert!(changed);
    assert_eq!(cache.styles()[&wire_id].line, NetLineStyle::Dashed);
}

#[test]
fn no_classes_leaves_the_cache_empty_and_reports_no_change() {
    let sheet = sheet_with_net(Uuid::from_u128(1), "VCC");
    let mut cache = NetClassStyleCache::default();

    let changed = cache.refresh([input(1, &sheet, &[])]);

    assert!(!changed);
    assert!(cache.is_empty());
}

#[test]
fn a_new_revision_with_a_renamed_net_drops_the_style() {
    let wire_id = Uuid::from_u128(1);
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    let before = sheet_with_net(wire_id, "VCC");
    cache.refresh([input(1, &before, &classes)]);

    let after = sheet_with_net(wire_id, "SDA");
    let changed = cache.refresh([input(2, &after, &classes)]);

    assert!(changed);
    assert!(cache.styles().is_empty());
}

#[test]
fn an_unchanged_revision_and_table_is_not_rebuilt() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    cache.refresh([input(1, &sheet, &classes)]);

    // Same revision, different sheet content: the revision is the contract,
    // so the cached resolution stands and the netlist is never asked for.
    let other = sheet_with_net(wire_id, "SDA");
    let changed = cache.refresh([input(1, &other, &classes)]);

    assert!(!changed);
    assert!(cache.styles().contains_key(&wire_id));
}

#[test]
fn editing_the_class_table_restyles_without_a_new_revision() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let mut cache = NetClassStyleCache::default();
    cache.refresh([input(1, &sheet, &[dashed_class("Power", "VCC")])]);

    let mut dotted = dashed_class("Power", "VCC");
    dotted.style.line = NetLineStyle::Dotted;
    let changed = cache.refresh([input(1, &sheet, &[dotted])]);

    assert!(changed);
    assert_eq!(cache.styles()[&wire_id].line, NetLineStyle::Dotted);
}

#[test]
fn a_project_that_is_no_longer_an_input_leaves_the_cache() {
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VCC");
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();
    cache.refresh([input(1, &sheet, &classes)]);

    let changed = cache.refresh(std::iter::empty());

    assert!(changed);
    assert!(cache.styles().is_empty());
}

#[test]
fn the_class_table_joins_the_project_netlist_not_one_sheet() {
    // The element is named `VIN` on its own sheet; the project netlist says
    // the net is `VCC`. Only the project netlist decides.
    let wire_id = Uuid::from_u128(1);
    let sheet = sheet_with_net(wire_id, "VIN");
    let mut project_netlist = signex_net::build_netlist(&sheet);
    project_netlist.nets[0].name = "VCC".to_string();
    let classes = [dashed_class("Power", "VCC")];
    let mut cache = NetClassStyleCache::default();

    cache.refresh([ProjectClassInput {
        project: PROJECT,
        revision: 1,
        classes: &classes,
        netlist: Box::new(move || Some(project_netlist.clone())),
    }]);

    assert_eq!(cache.styles()[&wire_id].line, NetLineStyle::Dashed);
}

#[test]
fn the_gate_is_open_only_when_a_sheet_or_a_project_changed() {
    use signex_types::project::ProjectData;
    use std::collections::HashMap;

    let mut project = LoadedProject {
        id: PROJECT,
        path: PathBuf::from("P.snxprj"),
        data: ProjectData {
            name: "P".to_string(),
            dir: String::new(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: vec![dashed_class("Power", "VCC")],
            rules: None,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    };
    let mut cache = NetClassStyleCache::default();

    assert!(cache.needs_refresh(7, std::slice::from_ref(&project)));
    assert!(
        !cache.needs_refresh(7, std::slice::from_ref(&project)),
        "nothing changed, as on a pointer move"
    );
    assert!(
        cache.needs_refresh(8, std::slice::from_ref(&project)),
        "a sheet was edited"
    );
    project.data.net_classes[0].style.line = NetLineStyle::Dotted;
    assert!(
        cache.needs_refresh(8, std::slice::from_ref(&project)),
        "the class table was edited"
    );
    assert!(!cache.needs_refresh(8, std::slice::from_ref(&project)));
    assert!(cache.needs_refresh(8, &[]), "the project was closed");
}
