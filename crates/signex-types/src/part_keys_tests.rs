use super::*;

fn part(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        ..Default::default()
    }
}

fn renumber(symbols: &mut [Symbol], at: usize, reference: &str) {
    let before = symbols.to_vec();
    symbols[at].reference = reference.to_string();
    follow_part_keys(symbols, &before);
}

#[test]
fn renaming_follows_a_renumbered_part() {
    let renames = HashMap::from([
        ("R1".to_string(), "R5".to_string()),
        ("R5".to_string(), "R1".to_string()),
    ]);
    assert_eq!(
        rename_part_references("= 2 * R1.Value + R5.Value", &renames),
        Some("= 2 * R5.Value + R1.Value".to_string())
    );
    // A unit after a number, a bare name and a literal are left alone.
    assert_eq!(rename_part_references("= 10R1 + R1", &renames), None);
    assert_eq!(rename_part_references("R1.Value", &renames), None);
}

/// Reset Designators then Annotate: the reference follows the part through
/// its uuid, not whichever part is numbered R5 afterwards.
#[test]
fn a_reference_follows_its_part_through_a_reset() {
    let mut symbols = vec![
        part("R5", "1kΩ"),
        part("R7", "2kΩ"),
        part("R9", "= R5.Value * 2"),
    ];
    let target = symbols[0].uuid;
    // Reset: every designator back to `R?`.
    let before = symbols.clone();
    for symbol in &mut symbols {
        symbol.reference = "R?".to_string();
    }
    follow_part_keys(&mut symbols, &before);
    assert_eq!(
        symbols[2].value,
        format!("= {}.Value * 2", uuid_key(target))
    );
    // Annotate numbers the other part R5 and ours R6.
    let before = symbols.clone();
    symbols[1].reference = "R5".to_string();
    symbols[0].reference = "R6".to_string();
    symbols[2].reference = "R7".to_string();
    follow_part_keys(&mut symbols, &before);
    assert_eq!(symbols[2].value, "= R6.Value * 2");
}

/// A designator typed by hand, one keystroke at a time: a duplicate on the
/// way names the part by its uuid, so the reference never joins the other
/// part that already has the designator.
#[test]
fn a_reference_follows_a_designator_typed_by_hand() {
    let mut symbols = vec![
        part("R12", "1kΩ"),
        part("R1", "2kΩ"),
        part("R3", "= R12.Value"),
    ];
    renumber(&mut symbols, 0, "R1");
    assert_eq!(
        symbols[2].value,
        format!("= {}.Value", uuid_key(symbols[0].uuid))
    );
    renumber(&mut symbols, 0, "R13");
    assert_eq!(symbols[2].value, "= R13.Value");
    // A swap of two designators keeps each reference on its part.
    symbols[2].value = "= R13.Value + R1.Value".to_string();
    let before = symbols.clone();
    symbols[0].reference = "R1".to_string();
    symbols[1].reference = "R13".to_string();
    follow_part_keys(&mut symbols, &before);
    assert_eq!(symbols[2].value, "= R1.Value + R13.Value");
}

#[test]
fn the_units_of_one_part_share_its_designator_key() {
    let mut unit_a = part("U1", "LM358");
    unit_a.unit = 1;
    let mut unit_b = part("U1", "LM358");
    unit_b.unit = 2;
    let symbols = vec![unit_a, unit_b];
    assert_eq!(part_key(&symbols[0], &symbols), "U1");
    assert_eq!(part_named(&symbols, "U1").map(|s| s.unit), Some(1));
}
