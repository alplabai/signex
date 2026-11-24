use super::*;

fn opamp() -> Symbol {
    let mut sym = Symbol::empty("OPA2333");
    for (i, (number, name)) in [("1", "OUTA"), ("2", "INA"), ("3", "VCC")]
        .into_iter()
        .enumerate()
    {
        let mut pin = SymbolPin::new(number, name);
        pin.position = [0.0, -2.54 * i as f64];
        sym.pins.push(pin);
    }
    sym
}

fn rows(of: &[usize]) -> BTreeSet<usize> {
    of.iter().copied().collect()
}

#[test]
fn a_table_pin_takes_the_next_number_below_the_lowest_pin() {
    let mut sym = opamp();
    let idx = add_table_pin(&mut sym, PartView::normal(2));
    let pin = &sym.pins[idx];
    assert_eq!(pin.number, "4");
    assert_eq!(pin.part_number, 2);
    assert_eq!(pin.position[0], 0.0);
    assert!((pin.position[1] + 7.62).abs() < 1e-9, "{:?}", pin.position);
}

#[test]
fn delete_selected_removes_exactly_the_ticked_pins() {
    let mut sym = opamp();
    assert!(remove_pins(&mut sym, &rows(&[0, 2])));
    let numbers: Vec<&str> = sym.pins.iter().map(|p| p.number.as_str()).collect();
    assert_eq!(numbers, ["2"]);
    assert!(!remove_pins(&mut sym, &rows(&[5])));
}

#[test]
fn apply_to_selected_writes_the_set_fields_and_keeps_the_rest() {
    let mut sym = opamp();
    let bulk = BulkEdit {
        electrical: Some(PinDirection::Input),
        part: "2".into(),
        length: String::new(),
        orientation: None,
        hidden: Some(true),
    };
    assert!(apply_bulk(&mut sym, &rows(&[1, 2]), &bulk, Some(5.08)));
    assert_eq!(sym.pins[0].electrical, PinDirection::Unclassified);
    assert!(!sym.pins[0].hidden);
    for pin in &sym.pins[1..] {
        assert_eq!(pin.electrical, PinDirection::Input);
        assert_eq!(pin.part_number, 2);
        assert_eq!(pin.length, 5.08);
        assert!(pin.hidden);
        assert_eq!(pin.orientation, PinOrientation::Right, "kept");
    }
}

#[test]
fn a_part_that_is_not_a_whole_number_keeps_each_part() {
    let mut sym = opamp();
    let bulk = BulkEdit {
        part: "B".into(),
        ..BulkEdit::default()
    };
    apply_bulk(&mut sym, &rows(&[0]), &bulk, None);
    assert_eq!(sym.pins[0].part_number, 1);
}

fn pasted(number: &str, name: &str) -> PastedRow {
    PastedRow {
        number: Some(number.into()),
        name: Some(name.into()),
        electrical: Some(PinDirection::Output),
        ..PastedRow::default()
    }
}

#[test]
fn update_by_number_updates_a_matching_pin_and_adds_the_rest() {
    let mut sym = opamp();
    let rows = [pasted("2", "\u{2212}INA"), pasted("8", "V+")];
    assert!(apply_paste(
        &mut sym,
        &rows,
        PasteMode::UpdateByNumber,
        PartView::normal(1),
        |_| true
    ));
    assert_eq!(sym.pins.len(), 4);
    assert_eq!(sym.pins[1].name, "\u{2212}INA");
    assert_eq!(sym.pins[1].electrical, PinDirection::Output);
    let added = &sym.pins[3];
    assert_eq!(added.number, "8");
    assert_eq!(added.name, "V+");
    assert_eq!(added.orientation, PinOrientation::Left);
}

#[test]
fn append_adds_every_row_and_replace_all_drops_the_old_pins() {
    let mut sym = opamp();
    let rows = [pasted("1", "A")];
    apply_paste(
        &mut sym,
        &rows,
        PasteMode::Append,
        PartView::normal(1),
        |_| true,
    );
    assert_eq!(sym.pins.len(), 4);
    apply_paste(
        &mut sym,
        &rows,
        PasteMode::ReplaceAll,
        PartView::normal(1),
        |_| true,
    );
    assert_eq!(sym.pins.len(), 1);
    assert_eq!(sym.pins[0].name, "A");
}

#[test]
fn a_refused_pasted_length_keeps_the_pin_length() {
    let mut sym = opamp();
    let row = PastedRow {
        number: Some("1".into()),
        length: Some(-1.0),
        ..PastedRow::default()
    };
    apply_paste(
        &mut sym,
        &[row],
        PasteMode::UpdateByNumber,
        PartView::normal(1),
        |l| l > 0.0,
    );
    assert_eq!(sym.pins[0].length, 2.54);
}

#[test]
fn nothing_pasted_changes_nothing() {
    let mut sym = opamp();
    assert!(!apply_paste(
        &mut sym,
        &[],
        PasteMode::ReplaceAll,
        PartView::normal(1),
        |_| true
    ));
    assert_eq!(sym.pins.len(), 3);
}
