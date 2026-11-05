//! `schematic_defaults` in the `.snxprj` (#2026): optional, built-in when
//! absent, round-trips, keeps unknown keys, and never stops a project from
//! opening.

use super::*;
use crate::schematic::SchematicDefaults;

const OLD_FILE: &str = r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#;

fn project_file(json: &str) -> (std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    std::fs::write(&path, json).unwrap();
    (path, dir)
}

#[test]
fn a_file_without_the_key_loads_as_built_in_and_resaves_without_it() {
    let (path, _dir) = project_file(OLD_FILE);
    let data = parse_project(&path).unwrap();
    assert!(data.schematic_defaults.is_empty());
    assert_eq!(
        data.schematic_defaults.resolve(),
        SchematicDefaults::BUILT_IN
    );
    assert_eq!(data.schematic_defaults.font_family(), "Iosevka");
    write_project(&path, &data).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("schematic_defaults"), "{saved}");
}

#[test]
fn the_defaults_keys_round_trip_and_keep_unknown_keys() {
    let (path, _dir) = project_file(
        r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,
            "schematic_defaults":{"wire_width_mm":0.3,"bus_width_mm":0.8,
                "font_family":"Roboto","line_style":"dashed"},
            "future_key":1}"#,
    );
    let data = parse_project(&path).unwrap();
    let resolved = data.schematic_defaults.resolve();
    assert_eq!(resolved.wire_width_mm, 0.3);
    assert_eq!(resolved.bus_width_mm, 0.8);
    assert_eq!(resolved.symbol_line_width_mm, 0.3, "pins follow the wire");
    assert_eq!(resolved.text_size_pt, 10.0, "missing key: built-in");
    assert_eq!(data.schematic_defaults.font_family(), "Roboto");
    write_project(&path, &data).unwrap();
    let back = parse_project(&path).unwrap();
    assert_eq!(back.schematic_defaults, data.schematic_defaults);
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(raw["schematic_defaults"]["line_style"], "dashed");
    assert_eq!(raw["future_key"], 1);
}

#[test]
fn an_unreadable_section_opens_the_project_and_is_kept_until_replaced() {
    let (path, _dir) = project_file(
        r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,
            "schematic_defaults":{"wire_width_mm":"thick"}}"#,
    );
    let mut data = parse_project(&path).expect("the project opens");
    assert_eq!(
        data.schematic_defaults.resolve(),
        SchematicDefaults::BUILT_IN
    );
    write_project(&path, &data).unwrap();
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(raw["schematic_defaults"]["wire_width_mm"], "thick");

    data.schematic_defaults = data
        .schematic_defaults
        .with(SchematicDefaultField::WireWidth, Some(0.5));
    write_project(&path, &data).unwrap();
    let back = parse_project(&path).unwrap();
    assert_eq!(back.schematic_defaults.wire_width_mm, Some(0.5));
}

#[test]
fn unusable_values_resolve_as_unset() {
    let overrides = SchematicDefaultOverrides {
        wire_width_mm: Some(0.0),
        bus_width_mm: Some(-2.0),
        text_size_pt: Some(f64::NAN),
        font_family: Some("  ".into()),
        ..Default::default()
    };
    assert_eq!(overrides.resolve(), SchematicDefaults::BUILT_IN);
    assert_eq!(overrides.font_family(), "Iosevka");
}

#[test]
fn line_style_shape_fill_and_font_round_trip_and_resolve() {
    use crate::net_class::NetLineStyle;
    use crate::schematic::FillType;
    let (path, _dir) = project_file(
        r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,
            "schematic_defaults":{"wire_line_style":"dashed","shape_fill":"background","font_family":"Roboto"}}"#,
    );
    let data = parse_project(&path).unwrap();
    let resolved = data.schematic_defaults.resolve();
    assert_eq!(resolved.wire_line_style, NetLineStyle::Dashed);
    assert_eq!(resolved.shape_fill, FillType::Background);
    assert_eq!(
        data.schematic_defaults.placed_font_family().as_deref(),
        Some("Roboto")
    );
    write_project(&path, &data).unwrap();
    assert_eq!(
        parse_project(&path).unwrap().schematic_defaults,
        data.schematic_defaults
    );
    assert_eq!(
        SchematicDefaultOverrides::default().placed_font_family(),
        None
    );
}
