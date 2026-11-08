use super::*;

fn body(id: &str) -> LibSymbol {
    serde_json::from_str(&format!(r#"{{"id":"{id}"}}"#)).unwrap()
}

fn part(lib_id: &str, body_ref: &str) -> Symbol {
    Symbol {
        lib_id: lib_id.to_owned(),
        body_ref: body_ref.to_owned(),
        ..Default::default()
    }
}

#[test]
fn a_difference_below_the_hash_precision_is_the_same_version() {
    let a = body("lib:r");
    let mut b = a.clone();
    b.pin_name_offset += 1e-12;
    assert_ne!(a, b);
    assert!(a.same_version(&b));
    assert_eq!(a.body_ref(), b.body_ref());
    let mut sheet = SchematicSheet::default();
    sheet.embed_body(a).unwrap();
    assert!(sheet.embed_body(b).is_ok(), "same hash, same version");
}

#[test]
fn a_body_with_nan_is_the_same_version_as_itself() {
    let mut a = body("lib:r");
    a.pin_name_offset = f64::NAN;
    assert_ne!(a, a.clone(), "NaN breaks ==");
    assert!(a.same_version(&a.clone()));
    let mut sheet = SchematicSheet::default();
    sheet.embed_body(a.clone()).unwrap();
    assert!(sheet.embed_body(a).is_ok());
}

/// Two different bodies with one body ref: a birthday search over the 32-bit
/// hash (about 80k tries), deterministic.
fn colliding_pair() -> (LibSymbol, LibSymbol) {
    static PAIR: std::sync::OnceLock<(LibSymbol, LibSymbol)> = std::sync::OnceLock::new();
    PAIR.get_or_init(search_collision).clone()
}

fn search_collision() -> (LibSymbol, LibSymbol) {
    let mut seen = std::collections::HashMap::new();
    for n in 0.. {
        let mut b = body("lib:r");
        b.description = format!("variant {n}");
        if let Some(prev) = seen.insert(b.body_ref(), b.clone()) {
            return (prev, b);
        }
    }
    unreachable!()
}

#[test]
fn a_real_difference_under_one_ref_is_a_collision_and_changes_nothing() {
    let (a, other) = colliding_pair();
    assert!(!a.same_version(&other));
    let key = a.body_ref();
    assert_eq!(key, other.body_ref());
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert(key.clone(), other.clone());
    assert_eq!(
        sheet.check_body(&a),
        Err(BodyRefCollision {
            body_ref: key.clone()
        })
    );
    assert!(sheet.embed_body(a).is_err());
    assert_eq!(sheet.lib_symbols[&key], other);
}

#[test]
fn only_an_eight_hex_suffix_is_a_hash() {
    assert_eq!(lib_id_of_body_ref("lib:r#0123abcd"), "lib:r");
    assert_eq!(lib_id_of_body_ref("lib:r"), "lib:r");
    assert_eq!(lib_id_of_body_ref("lib:r#x"), "lib:r#x");
    assert_eq!(lib_id_of_body_ref("lib:r#0123abcg"), "lib:r#0123abcg");
    assert_eq!(lib_id_of_body_ref("a#b:r"), "a#b:r");
    assert_eq!(lib_id_of_body_ref("a#b:r#0123abcd"), "a#b:r");
}

#[test]
fn a_stored_key_that_no_longer_matches_its_body_is_re_keyed() {
    let b = body("lib:r");
    let mut sheet = SchematicSheet::default();
    sheet.lib_symbols.insert("lib:r#deadbeef".into(), b.clone());
    sheet
        .lib_symbols
        .insert("lib:bare".into(), body("lib:bare"));
    sheet.symbols.push(part("lib:r", "lib:r#deadbeef"));
    sheet.symbols.push(part("lib:bare", ""));
    sheet.upgrade_unversioned_bodies();
    assert_eq!(sheet.symbols[0].body_ref, b.body_ref());
    assert_eq!(sheet.symbols[1].body_ref, body("lib:bare").body_ref());
    for sym in &sheet.symbols {
        assert!(sheet.body_of(sym).is_some());
    }
    assert_eq!(sheet.lib_symbols.len(), 2);
}

#[test]
fn an_upgrade_never_overwrites_a_different_body_on_the_new_key() {
    let (b, squatter) = colliding_pair();
    let mut sheet = SchematicSheet::default();
    sheet
        .lib_symbols
        .insert(squatter.body_ref(), squatter.clone());
    sheet.lib_symbols.insert("lib:r".into(), b.clone());
    sheet.symbols.push(part("lib:r", ""));
    sheet.upgrade_unversioned_bodies();
    assert_eq!(
        sheet.lib_symbols[&b.body_ref()],
        squatter,
        "not overwritten"
    );
    assert_eq!(sheet.lib_symbols["lib:r"], b, "old key kept");
    assert_eq!(sheet.symbols[0].body_ref, "");
}

#[test]
fn a_part_whose_units_use_two_versions_is_reported_once() {
    let mut sheet = SchematicSheet::default();
    let (v1, v2) = ("lib:r#00000001", "lib:r#00000002");
    for (reference, unit, body) in [("U1", 1, v1), ("U1", 2, v2), ("U2", 1, v1), ("U2", 2, v1)] {
        let mut sym = part("lib:r", body);
        sym.reference = reference.into();
        sym.unit = unit;
        sheet.symbols.push(sym);
    }
    assert_eq!(sheet.mixed_version_parts(), ["U1"]);
}
