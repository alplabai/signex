//! The project's Schematic Refinement settings: one optional
//! `schematic_design` key, absent at the defaults (so an untouched project
//! re-saves byte-identical), read back, kept verbatim when unreadable, and
//! kept by an older build through its unknown-key bag. No format token
//! changes.

use super::*;
use crate::diff_pair_class::DiffPairClass;
use crate::schematic::{HarnessDefinition, HarnessDefinitionEntry};
use serde_json::{Value, json};

fn write_raw(dir: &Path, body: &Value) -> PathBuf {
    let path = dir.join("Proj.snxprj");
    std::fs::write(&path, body.to_string()).unwrap();
    path
}

fn saved_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn bare() -> Value {
    json!({"name": "Proj", "dir": "", "schematic_root": null, "pcb_file": null})
}

fn signal(name: &str) -> HarnessDefinitionEntry {
    HarnessDefinitionEntry {
        name: name.into(),
        harness: None,
    }
}

/// Every setting away from its default.
fn settings() -> SchematicDesignSettings {
    let mut usb = DiffPairClass::named("USB");
    usb.impedance_ohm = Some(90.0);
    usb.impedance_tolerance_pct = Some(10.0);
    usb.width_mm = Some(0.265);
    usb.gap_mm = Some(0.175);
    let mut lvds = DiffPairClass::named("LVDS_100R");
    lvds.impedance_ohm = Some(100.0);
    lvds.pair_by_suffix = false;
    SchematicDesignSettings {
        channel_naming: ChannelNaming {
            designator_format: "${Designator}${Channel}".into(),
            net_format: "${Channel}_${Net}".into(),
        },
        harness_definitions: vec![
            HarnessDefinition {
                name: "UART".into(),
                description: "Console".into(),
                entries: vec![signal("TX"), signal("RX")],
            },
            HarnessDefinition {
                name: "DEBUG".into(),
                description: String::new(),
                entries: vec![
                    signal("SWDIO"),
                    HarnessDefinitionEntry {
                        name: "CONSOLE".into(),
                        harness: Some("UART".into()),
                    },
                ],
            },
        ],
        diff_pair_classes: vec![usb, lvds],
    }
}

#[test]
fn the_defaults_write_no_key_and_an_untouched_project_resaves_identical() {
    assert!(SchematicDesignSettings::default().is_default());
    assert!(ChannelNaming::default().is_default());
    assert_eq!(
        ChannelNaming::default().designator_format,
        "${Channel}_${Designator}"
    );
    assert_eq!(ChannelNaming::default().net_format, "${Net}_${Channel}");

    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let data = parse_project(&path).unwrap();
    assert!(data.schematic_design.is_default());
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();
    let saved = saved_json(&path);
    assert!(saved.get("schematic_design").is_none(), "{saved}");
    assert_eq!(saved["format"], json!(SNXPRJ_FORMAT_V1));
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first);
}

#[test]
fn the_settings_round_trip_through_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let mut data = parse_project(&path).unwrap();
    data.schematic_design = settings();
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();

    let back = parse_project(&path).unwrap();
    assert_eq!(back.schematic_design, settings());
    write_project(&path, &back).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first, "byte-identical");

    let saved = saved_json(&path);
    assert_eq!(saved["format"], json!(SNXPRJ_FORMAT_V1), "no token change");
    let design = &saved["schematic_design"];
    assert_eq!(
        design["channel_naming"],
        json!({"designator_format": "${Designator}${Channel}", "net_format": "${Channel}_${Net}"})
    );
    assert_eq!(
        design["diff_pair_classes"][0],
        json!({"name": "USB", "impedance_ohm": 90.0, "impedance_tolerance_pct": 10.0,
               "width_mm": 0.265, "gap_mm": 0.175})
    );
    // `pair_by_suffix` is written only when switched off.
    assert_eq!(
        design["diff_pair_classes"][1]["pair_by_suffix"],
        json!(false)
    );
    assert_eq!(
        design["harness_definitions"][1]["entries"][1],
        json!({"name": "CONSOLE", "harness": "UART"})
    );
    // An empty description is not written.
    assert!(
        design["harness_definitions"][1]
            .get("description")
            .is_none()
    );
}

#[test]
fn only_the_settings_that_differ_are_written() {
    let mut data: ProjectData = serde_json::from_value(bare()).unwrap();
    data.schematic_design
        .diff_pair_classes
        .push(DiffPairClass::named("USB"));
    let saved: Value = serde_json::from_slice(&encode_project(&data).unwrap()).unwrap();
    assert_eq!(
        saved["schematic_design"],
        json!({"diff_pair_classes": [{"name": "USB"}]})
    );
}

#[test]
fn keys_from_a_newer_build_inside_a_class_survive_a_save() {
    let dir = tempfile::tempdir().unwrap();
    let mut body = bare();
    body["schematic_design"] = json!({
        "diff_pair_classes": [{"name": "USB", "max_skew_mm": 0.1}]
    });
    let path = write_raw(dir.path(), &body);
    let data = parse_project(&path).unwrap();
    assert_eq!(data.schematic_design.diff_pair_classes[0].name, "USB");
    write_project(&path, &data).unwrap();
    assert_eq!(
        saved_json(&path)["schematic_design"]["diff_pair_classes"][0]["max_skew_mm"],
        json!(0.1)
    );
}

#[test]
fn an_unreadable_section_opens_the_project_and_is_written_back_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [
        json!("yes"),
        json!([1]),
        json!({"harness_definitions": [{"entries": []}]}),
        json!({"diff_pair_classes": [{"name": "USB", "width_mm": "wide"}]}),
    ] {
        let mut body = bare();
        body["schematic_design"] = bad.clone();
        let path = write_raw(dir.path(), &body);
        let data = parse_project(&path).unwrap();
        assert!(data.schematic_design.is_default(), "{bad}");
        write_project(&path, &data).unwrap();
        assert_eq!(saved_json(&path)["schematic_design"], bad);
    }
}

/// DECISIONS 2026-11-23 (#2750), superseding replace-on-Set: even settings
/// that reach the model never replace a section this build cannot read.
#[test]
fn settings_made_now_never_replace_an_unreadable_section() {
    let dir = tempfile::tempdir().unwrap();
    let mut body = bare();
    body["schematic_design"] = json!("garbled");
    let path = write_raw(dir.path(), &body);
    let mut data = parse_project(&path).unwrap();
    assert!(data.has_unreadable_schematic_design());
    data.schematic_design = settings();
    write_project(&path, &data).unwrap();
    assert_eq!(saved_json(&path)["schematic_design"], json!("garbled"));
    let back = parse_project(&path).unwrap();
    assert!(back.has_unreadable_schematic_design());
    assert!(back.schematic_design.is_default());
}

/// #2750: one missing naming field takes its default, so the harness
/// definitions beside it stay readable.
#[test]
fn a_missing_channel_naming_field_takes_its_default() {
    let dir = tempfile::tempdir().unwrap();
    let mut body = bare();
    body["schematic_design"] = json!({
        "channel_naming": { "net_format": "${Channel}_${Net}" },
        "harness_definitions": [{ "name": "UART", "entries": [{ "name": "TX" }] }]
    });
    let path = write_raw(dir.path(), &body);
    let data = parse_project(&path).unwrap();
    assert!(!data.has_unreadable_schematic_design());
    let naming = &data.schematic_design.channel_naming;
    assert_eq!(naming.designator_format, DEFAULT_CHANNEL_DESIGNATOR_FORMAT);
    assert_eq!(naming.net_format, "${Channel}_${Net}");
    assert_eq!(data.schematic_design.harness_definitions[0].name, "UART");
}

/// A build from before the key existed reads `schematic_design` into its
/// unknown-key bag and writes it back unchanged. Modelled on today's shape:
/// the known keys plus the flattened bag `ProjectData` carries.
#[test]
fn a_build_without_the_key_keeps_it_through_its_unknown_bag() {
    #[derive(Serialize, Deserialize)]
    struct BeforeRefinement {
        name: String,
        dir: String,
        schematic_root: Option<String>,
        pcb_file: Option<String>,
        #[serde(default, flatten)]
        unknown: UnknownJson,
    }
    let mut data: ProjectData = serde_json::from_value(bare()).unwrap();
    data.schematic_design = settings();
    let written: Value = serde_json::from_slice(&encode_project(&data).unwrap()).unwrap();

    let old: BeforeRefinement = serde_json::from_value(written.clone()).unwrap();
    assert!(old.unknown.get("schematic_design").is_some());
    let rewritten = serde_json::to_value(&old).unwrap();
    assert_eq!(rewritten["schematic_design"], written["schematic_design"]);
    assert_eq!(rewritten, written, "nothing lost or changed");
}
