//! #2209: a `.snxsch` written before versioned library symbol bodies keys
//! `lib_symbols` by plain `lib_id` and names no `body_ref`. The fixture is
//! the unchanged output of the old writer (origin/trunk before #2209).
//! Loading it re-keys the body and points the instance at it, so its pins
//! still resolve, and the next save is `snxsch/1.1` in the new form.

use signex_types::format::SnxSchematic;

const PRE_2209: &str = include_str!("../legacy/lib-symbols-cache-pre2209.snxsch");

#[test]
fn a_pre_2209_file_upgrades_and_its_pins_resolve() {
    assert!(PRE_2209.starts_with("format = \"snxsch/1\""));
    assert!(PRE_2209.contains("[extras.sheet.lib_symbols.opa2]"));

    let loaded = SnxSchematic::parse(PRE_2209).unwrap();
    assert!(!loaded.is_from_newer_minor());
    let sheet = &loaded.sheet;
    let part = &sheet.symbols[0];
    assert_eq!(part.lib_id, "opa2");
    assert!(part.body_ref.starts_with("opa2#"), "{}", part.body_ref);
    assert!(!sheet.lib_symbols.contains_key("opa2"));
    let body = sheet.body_of(part).expect("the instance resolves its body");
    assert_eq!(body.body_ref(), part.body_ref);
    assert!(!body.pins.is_empty(), "the pins survive the upgrade");

    let saved = loaded.write_string().unwrap();
    assert!(saved.starts_with("format = \"snxsch/1.1\""));
    assert!(saved.contains(&format!("[extras.sheet.lib_symbols.\"{}\"]", part.body_ref)));
    let again = SnxSchematic::parse(&saved).unwrap();
    assert_eq!(again.sheet.symbols[0].body_ref, part.body_ref);
    assert_eq!(again.write_string().unwrap(), saved);
}
