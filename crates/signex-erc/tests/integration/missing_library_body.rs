//! #2781: a non-power part with no library body is reported as an error and
//! keeps the part checks that need no body. Its pins stay unknown.

use signex_erc::{ErcContext, RuleKind, Severity, Violation, project_units_not_placed, run};
use signex_types::schematic::{LibSymbol, Point, SchematicSheet, Symbol};
use uuid::Uuid;

const LIB: &str = "Device:R";

fn body() -> LibSymbol {
    LibSymbol {
        id: LIB.to_string(),
        reference: "R".to_string(),
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
        pins: Vec::new(),
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn part(reference: &str, unit: u32) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: LIB.to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        unit,
        position: Point::new(25.4, 25.4),
        ..Default::default()
    }
}

fn sheet(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    }
}

fn of(found: &[Violation], kind: RuleKind) -> Vec<Violation> {
    found
        .iter()
        .filter(|v| v.builtin_kind() == Some(kind))
        .cloned()
        .collect()
}

#[test]
fn a_part_with_no_body_is_an_error_on_that_part() {
    let symbol = part("R1", 1);
    let id = symbol.uuid;
    let found = of(&run(&sheet(vec![symbol])), RuleKind::PartWithoutLibraryBody);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].severity, Severity::Error);
    assert!(found[0].message.contains("R1"), "{}", found[0].message);
    assert_eq!(found[0].primary.map(|p| p.uuid), Some(id));
}

#[test]
fn a_part_with_its_body_is_not_reported() {
    let mut s = sheet(vec![part("R1", 1)]);
    s.lib_symbols.insert(LIB.to_string(), body());
    assert!(of(&run(&s), RuleKind::PartWithoutLibraryBody).is_empty());
}

#[test]
fn a_power_port_with_no_stored_body_is_not_reported() {
    let port = Symbol {
        lib_id: "power:+5V".to_string(),
        reference: "#PWR01".to_string(),
        value: "+5V".to_string(),
        is_power: true,
        ..part("#PWR01", 1)
    };
    assert!(of(&run(&sheet(vec![port])), RuleKind::PartWithoutLibraryBody).is_empty());
}

#[test]
fn the_part_checks_that_need_no_body_still_run() {
    let duplicates = run(&sheet(vec![part("R1", 1), part("R1", 1)]));
    assert_eq!(
        of(&duplicates, RuleKind::DuplicateRefDesignator).len(),
        2,
        "{duplicates:?}"
    );
    let unannotated = run(&sheet(vec![part("R?", 1)]));
    assert_eq!(
        of(&unannotated, RuleKind::UnannotatedDesignator).len(),
        1,
        "{unannotated:?}"
    );
}

#[test]
fn no_pins_are_invented_for_a_part_with_no_body() {
    let s = sheet(vec![part("R1", 1)]);
    let ctx = ErcContext::from_snapshot(&s);
    assert_eq!(ctx.symbols.len(), 1);
    assert!(ctx.symbols[0].pins.is_empty());
    assert!(!ctx.symbols[0].has_library_body());
    assert!(of(&run(&s), RuleKind::UnusedPin).is_empty());
}

#[test]
fn a_part_with_no_body_has_no_units_due() {
    let s = sheet(vec![part("U1", 3)]);
    assert!(of(&run(&s), RuleKind::UnitsNotPlaced).is_empty());
    assert!(project_units_not_placed(&[("a", &s)]).is_empty());
}

#[test]
fn a_unit_missing_its_body_is_reported_beside_a_unit_that_has_one() {
    let whole = part("U1", 1);
    let broken = Symbol {
        body_ref: "Device:R#gone".to_string(),
        ..part("U1", 2)
    };
    let id = broken.uuid;
    let mut s = sheet(vec![whole, broken]);
    s.lib_symbols.insert(LIB.to_string(), body());
    let found = of(&run(&s), RuleKind::PartWithoutLibraryBody);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].primary.map(|p| p.uuid), Some(id));
}
