use super::*;
use crate::test_support::test_sheet;
use crate::{Engine, SymbolFlag};
use signex_types::schematic::{Label, LabelType, Point, Wire};

fn symbol(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: Uuid::new_v4(),
        reference: reference.into(),
        value: value.into(),
        in_bom: true,
        on_board: true,
        ..Default::default()
    }
}

fn label(text: &str, font_size: f64) -> Label {
    Label {
        uuid: Uuid::new_v4(),
        text: text.into(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size,
        justify: HAlign::Left,
        justify_v: Default::default(),
        font_family: None,
        unknown_columns: Default::default(),
    }
}

fn wire(width: f64) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: Point::new(0.0, 0.0),
        end: Point::new(5.0, 0.0),
        stroke_width: width,
        ..Default::default()
    }
}

fn pick(sheet: &SchematicSheet) -> Vec<SelectedItem> {
    let mut items = Vec::new();
    items.extend(
        sheet
            .symbols
            .iter()
            .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol)),
    );
    items.extend(
        sheet
            .labels
            .iter()
            .map(|l| SelectedItem::new(l.uuid, SelectedKind::Label)),
    );
    items.extend(
        sheet
            .wires
            .iter()
            .map(|w| SelectedItem::new(w.uuid, SelectedKind::Wire)),
    );
    items
}

#[test]
fn agreeing_fields_are_shared_and_differing_ones_mixed() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![
        symbol("R1", "10k"),
        symbol("R2", "10k"),
        symbol("R3", "22k"),
    ];
    sheet.symbols[1].dnp = true;
    let groups = selection_groups(&sheet, &pick(&sheet));
    let symbols = groups.symbols.expect("symbol group");
    assert_eq!(symbols.count, 3);
    assert_eq!(symbols.value, Shared::Mixed);
    assert_eq!(symbols.dnp, Shared::Mixed);
    assert_eq!(symbols.in_bom, Shared::Same(true));
    assert!(groups.labels.is_none() && groups.wires.is_none());
}

#[test]
fn parameters_are_the_union_and_a_missing_one_is_mixed() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![symbol("R1", ""), symbol("R2", "")];
    sheet.symbols[0].fields.insert("MPN".into(), "X".into());
    sheet.symbols[1].fields.insert("MPN".into(), "X".into());
    sheet.symbols[0].fields.insert("Tol".into(), "1%".into());
    let group = selection_groups(&sheet, &pick(&sheet)).symbols.unwrap();
    assert_eq!(
        group.parameters,
        vec![
            ("MPN".into(), Shared::Same("X".into())),
            ("Tol".into(), Shared::Mixed),
        ]
    );
}

#[test]
fn power_ports_and_field_items_are_left_out() {
    let mut sheet = test_sheet();
    let mut power = symbol("#PWR1", "GND");
    power.is_power = true;
    sheet.symbols = vec![power, symbol("R1", "1k")];
    let mut items = pick(&sheet);
    items.push(SelectedItem::new(
        sheet.symbols[1].uuid,
        SelectedKind::SymbolRefField,
    ));
    let group = selection_groups(&sheet, &items).symbols.unwrap();
    assert_eq!(group.count, 1);
    assert_eq!(group.value, Shared::Same("1k".into()));
}

#[test]
fn label_and_wire_groups_compare_lengths_with_a_tolerance() {
    let mut sheet = test_sheet();
    sheet.labels = vec![label("A", 1.27), label("B", 1.27 + 1e-9)];
    sheet.wires = vec![wire(0.0), wire(0.25)];
    let groups = selection_groups(&sheet, &pick(&sheet));
    let labels = groups.labels.unwrap();
    assert_eq!(labels.font_size_mm, Shared::Same(1.27));
    assert_eq!(labels.justify, Shared::Same(HAlign::Left));
    assert_eq!(groups.wires.unwrap().width_mm, Shared::Mixed);
}

#[test]
fn one_edit_changes_every_part_in_one_undo_step() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![symbol("R1", "10k"), symbol("R2", "22k"), symbol("R3", "1k")];
    sheet.labels = vec![label("A", 1.27)];
    let items = pick(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    let commands = edit_commands(
        engine.document(),
        &items,
        &MultiEdit::SymbolValue("4k7".into()),
    );
    assert_eq!(commands.len(), 3, "the label is not touched");
    engine.execute_batch(commands).unwrap();
    assert!(engine.document().symbols.iter().all(|s| s.value == "4k7"));
    assert!(engine.undo().unwrap().is_some());
    let values: Vec<_> = engine
        .document()
        .symbols
        .iter()
        .map(|s| s.value.as_str())
        .collect();
    assert_eq!(values, ["10k", "22k", "1k"]);
    assert!(!engine.can_undo(), "the whole edit was one history entry");
}

#[test]
fn flags_parameters_labels_and_wires_all_apply() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![symbol("R1", ""), symbol("R2", "")];
    sheet.labels = vec![label("A", 1.27), label("B", 2.0)];
    sheet.wires = vec![wire(0.0), wire(0.3)];
    let items = pick(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    for edit in [
        MultiEdit::SymbolDnp(true),
        MultiEdit::SymbolInBom(false),
        MultiEdit::SymbolOnBoard(false),
        MultiEdit::SymbolParameter {
            key: "MPN".into(),
            value: "Z".into(),
        },
        MultiEdit::LabelFontSizeMm(2.54),
        MultiEdit::LabelJustify(HAlign::Right),
        MultiEdit::LabelRotationDegrees(90.0),
        MultiEdit::WireWidthMm(0.5),
    ] {
        let commands = edit_commands(engine.document(), &items, &edit);
        engine.execute_batch(commands).unwrap();
    }
    let doc = engine.document();
    assert!(
        doc.symbols
            .iter()
            .all(|s| s.dnp && !s.in_bom && !s.on_board)
    );
    assert!(
        doc.symbols
            .iter()
            .all(|s| s.fields.get("MPN").map(String::as_str) == Some("Z"))
    );
    assert!(doc.labels.iter().all(|l| l.justify == HAlign::Right));
    assert!(
        doc.labels
            .iter()
            .all(|l| (l.font_size - 2.54).abs() < 1e-9 && l.rotation == 90.0)
    );
    assert!(
        doc.wires
            .iter()
            .all(|w| (w.stroke_width - 0.5).abs() < 1e-9)
    );
}

#[test]
fn a_negative_or_non_finite_wire_width_is_refused() {
    let mut sheet = test_sheet();
    sheet.wires = vec![wire(0.2)];
    let id = sheet.wires[0].uuid;
    let mut engine = Engine::new(sheet).unwrap();
    for width_mm in [-0.1, f64::NAN] {
        let refused = engine.execute(Command::SetWireWidth {
            wire_id: id,
            width_mm,
        });
        assert!(matches!(
            refused,
            Err(crate::EngineError::InvalidWireWidth(_))
        ));
    }
    assert_eq!(engine.document().wires[0].stroke_width, 0.2);
    assert!(!engine.can_undo());
}

#[test]
fn an_unchanged_flag_records_no_history() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![symbol("R1", "")];
    let id = sheet.symbols[0].uuid;
    let mut engine = Engine::new(sheet).unwrap();
    let same = engine
        .execute(Command::SetSymbolFlag {
            symbol_id: id,
            flag: SymbolFlag::InBom,
            value: true,
        })
        .unwrap();
    assert!(!same.changed && !engine.can_undo());
}

#[test]
fn an_empty_parameter_value_removes_it_from_every_part() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![symbol("R1", ""), symbol("R2", "")];
    for s in &mut sheet.symbols {
        s.fields.insert("MPN".into(), "X".into());
    }
    let items = pick(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    let edit = MultiEdit::SymbolParameter {
        key: "MPN".into(),
        value: String::new(),
    };
    let commands = edit_commands(engine.document(), &items, &edit);
    engine.execute_batch(commands).unwrap();
    assert!(
        engine
            .document()
            .symbols
            .iter()
            .all(|s| !s.fields.contains_key("MPN"))
    );
}

#[test]
fn flags_apply_to_a_locked_part_but_a_value_edit_is_refused() {
    let mut sheet = test_sheet();
    sheet.symbols = vec![symbol("R1", "1k")];
    sheet.symbols[0].locked = true;
    let id = sheet.symbols[0].uuid;
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .execute(Command::SetSymbolFlag {
            symbol_id: id,
            flag: SymbolFlag::OnBoard,
            value: false,
        })
        .unwrap();
    engine
        .execute(Command::SetSymbolDnp {
            symbol_id: id,
            dnp: true,
        })
        .unwrap();
    assert!(engine.document().symbols[0].dnp && !engine.document().symbols[0].on_board);
    let refused = engine.execute(Command::UpdateText {
        target: TextTarget::SymbolValue(id),
        value: "2k".into(),
    });
    assert!(refused.is_err());
}
