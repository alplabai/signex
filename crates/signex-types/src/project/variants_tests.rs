//! #1690: variant names are checked, a rename carries the active variant and
//! a removal of the active one falls back to Base; the fields round-trip.

use super::*;

const EMPTY_PROJECT: &str = r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#;

fn project() -> ProjectData {
    serde_json::from_str::<ProjectData>(EMPTY_PROJECT).expect("an empty project")
}

#[test]
fn a_variant_is_added_trimmed_and_a_bad_name_is_refused() {
    let mut data = project();
    data.add_variant("  Lite ").unwrap();
    assert_eq!(data.variant_definitions, ["Lite"]);
    assert_eq!(data.add_variant(" "), Err(VariantProblem::Empty));
    assert_eq!(data.add_variant("base"), Err(VariantProblem::Reserved));
    assert_eq!(
        data.add_variant("LITE"),
        Err(VariantProblem::Taken("Lite".into()))
    );
    assert_eq!(data.add_variant("a\nb"), Err(VariantProblem::Invalid));
    let long = "x".repeat(MAX_VARIANT_NAME_CHARS + 1);
    assert_eq!(data.add_variant(&long), Err(VariantProblem::TooLong));
    assert_eq!(data.variant_definitions, ["Lite"]);
}

#[test]
fn a_rename_carries_the_active_variant_and_may_change_only_case() {
    let mut data = project();
    data.add_variant("Lite").unwrap();
    data.add_variant("Pro").unwrap();
    data.set_active_variant(Some("lite")).unwrap();
    assert_eq!(data.active_variant.as_deref(), Some("Lite"));

    data.rename_variant("Lite", "LITE").unwrap();
    assert_eq!(data.active_variant.as_deref(), Some("LITE"));
    assert_eq!(
        data.rename_variant("LITE", "pro"),
        Err(VariantProblem::Taken("Pro".into()))
    );
    assert_eq!(
        data.rename_variant("Max", "Mini"),
        Err(VariantProblem::Unknown("Max".into()))
    );
    assert_eq!(data.variant_definitions, ["LITE", "Pro"]);
}

#[test]
fn removing_the_active_variant_makes_base_active() {
    let mut data = project();
    data.add_variant("Lite").unwrap();
    data.set_active_variant(Some("Lite")).unwrap();
    data.remove_variant("lite").unwrap();
    assert!(data.variant_definitions.is_empty());
    assert_eq!(data.active_variant, None);
    assert_eq!(data.active_variant_name(), BASE_VARIANT);
    assert_eq!(
        data.set_active_variant(Some("Lite")),
        Err(VariantProblem::Unknown("Lite".into()))
    );
    data.set_active_variant(Some("Base")).unwrap();
    assert_eq!(data.active_variant, None);
}

#[test]
fn variants_round_trip_through_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    let mut data = project();
    data.add_variant("Lite").unwrap();
    data.set_active_variant(Some("Lite")).unwrap();
    write_project(&path, &data).unwrap();
    let read = parse_project(&path).unwrap();
    assert_eq!(read.variant_definitions, ["Lite"]);
    assert_eq!(read.active_variant.as_deref(), Some("Lite"));
}
