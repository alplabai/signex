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

fn any_length(_: f64) -> bool {
    true
}

fn rows(text: &str) -> Vec<PinRow> {
    parse_pin_rows(text, any_length).expect("rows parse")
}

fn numbers(sym: &Symbol) -> Vec<&str> {
    sym.pins.iter().map(|p| p.number.as_str()).collect()
}

#[test]
fn a_row_reads_every_field_by_its_id() {
    let read = rows(
        r#"{"2": {"number": "2A", "name": "-INA", "electrical": "input", "part": 2,
            "length": 5.08, "orientation": "up", "hidden": true, "description": "Inverting"}}"#,
    );
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].number, "2");
    let PinRowChange::Set(fields) = &read[0].change else {
        panic!("a set row");
    };
    assert_eq!(fields.number.as_deref(), Some("2A"));
    assert_eq!(fields.name.as_deref(), Some("-INA"), "kept as given");
    assert_eq!(fields.electrical, Some(PinDirection::Input));
    assert_eq!(fields.part, Some(2));
    assert_eq!(fields.length, Some(5.08));
    assert_eq!(fields.orientation, Some(PinOrientation::Up));
    assert_eq!(fields.hidden, Some(true));
    assert_eq!(fields.description.as_deref(), Some("Inverting"));
}

#[test]
fn null_removes_the_pin() {
    assert_eq!(
        rows(r#"{"3": null}"#),
        [PinRow {
            number: "3".into(),
            change: PinRowChange::Remove,
        }]
    );
}

#[test]
fn a_malformed_row_is_refused_with_its_pin_and_field() {
    for (text, needle) in [
        ("[1, 2]", "JSON object"),
        ("{}", "names no pin"),
        (r#"{"1": 5}"#, "pin `1`"),
        (r#"{"1": {"colour": "red"}}"#, "`colour` is not a pin field"),
        (r#"{"1": {"electrical": "sideways"}}"#, "power_input"),
        (
            r#"{"1": {"orientation": "north"}}"#,
            "left, right, up, down",
        ),
        (r#"{"1": {"part": 300}}"#, "0 to 255"),
        (r#"{"1": {"part": -1}}"#, "0 to 255"),
        (r#"{"1": {"hidden": "yes"}}"#, "true or false"),
        (r#"{"1": {"name": 7}}"#, "`name` is text"),
        (r#"{"1": {"number": " "}}"#, "must not be empty"),
    ] {
        let why = parse_pin_rows(text, any_length).expect_err(text);
        assert!(why.contains(needle), "{text}: {why}");
    }
}

#[test]
fn a_length_the_check_refuses_refuses_the_rows() {
    let why = parse_pin_rows(r#"{"1": {"length": 0.5}}"#, |l| l >= 1.0).expect_err("refused");
    assert!(why.contains("pin `1`") && why.contains("length"), "{why}");
}

#[test]
fn rows_set_their_pins_and_leave_the_rest() {
    let mut sym = opamp();
    let applied = apply_rows(
        &mut sym,
        &rows(r#"{"1": {"electrical": "output", "hidden": true}, "3": {"name": "V+"}}"#),
    )
    .expect("applied");
    assert_eq!(applied.updated, 2);
    assert!(applied.removed.is_empty());
    assert_eq!(sym.pins[0].electrical, PinDirection::Output);
    assert!(sym.pins[0].hidden);
    assert_eq!(sym.pins[0].name, "OUTA", "not named, kept");
    assert_eq!(sym.pins[1], opamp().pins[1], "no row, untouched");
    assert_eq!(sym.pins[2].name, "V+");
}

#[test]
fn renumbered_pins_are_found_by_their_old_numbers() {
    let mut sym = opamp();
    apply_rows(
        &mut sym,
        &rows(r#"{"1": {"number": "2"}, "2": {"number": "1"}}"#),
    )
    .expect("a swap");
    assert_eq!(numbers(&sym), ["2", "1", "3"]);
    assert_eq!(sym.pins[0].name, "OUTA");
}

#[test]
fn removed_pins_go_and_are_named_by_their_old_index() {
    let mut sym = opamp();
    let applied = apply_rows(&mut sym, &rows(r#"{"1": null, "3": null}"#)).expect("applied");
    assert_eq!(numbers(&sym), ["2"]);
    assert_eq!(applied.removed, BTreeSet::from([0, 2]));
}

#[test]
fn an_unknown_or_shared_number_refuses_everything() {
    let mut sym = opamp();
    let before = sym.clone();
    let why = apply_rows(&mut sym, &rows(r#"{"1": {"name": "X"}, "9": null}"#)).expect_err("9");
    assert!(why.contains("`9`"), "{why}");
    assert_eq!(sym, before, "nothing was written");

    sym.pins[2].number = "1".into();
    let shared = sym.clone();
    let why = apply_rows(&mut sym, &rows(r#"{"1": {"name": "X"}}"#)).expect_err("shared");
    assert!(why.contains("more than one pin"), "{why}");
    assert_eq!(sym, shared);
}

#[test]
fn paste_columns_and_modes_read_by_id() {
    assert_eq!(
        parse_paste_fields("number, name,ignore,length").expect("fields"),
        [
            PasteField::Number,
            PasteField::Name,
            PasteField::Ignore,
            PasteField::Length
        ]
    );
    assert!(
        parse_paste_fields("number,type")
            .expect_err("type")
            .contains("`type`")
    );
    assert_eq!(
        parse_paste_mode("replace_all").expect("mode"),
        PasteMode::ReplaceAll
    );
    assert_eq!(
        parse_paste_mode(" append ").expect("mode"),
        PasteMode::Append
    );
    assert!(
        parse_paste_mode("merge")
            .expect_err("merge")
            .contains("update_by_number")
    );
}

#[test]
fn escapes_stand_for_the_control_characters_an_operand_cannot_carry() {
    assert_eq!(unescape_paste(r"1\tOUTA\n2\tINA"), "1\tOUTA\n2\tINA");
    assert_eq!(unescape_paste(r"A\\B"), r"A\B");
    assert_eq!(
        unescape_paste(r"I/O \x"),
        r"I/O \x",
        "another escape is kept"
    );
    assert_eq!(unescape_paste("end\\"), "end\\");
}

#[test]
fn ids_round_trip_for_every_type_and_side() {
    for electrical in PinDirection::ALL {
        let id = electrical_id(electrical);
        assert_eq!(electrical_of(&id), Some(electrical), "{id}");
    }
    for (orientation, id) in ORIENTATIONS {
        assert_eq!(orientation_id(orientation), id);
        assert_eq!(orientation_of(id), Some(orientation));
    }
    assert_eq!(electrical_id(PinDirection::PowerInput), "power_input");
}
