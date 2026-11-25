//! Mockup row 32: the Variant Manager's settings. A part's fit and values
//! per variant and the default variant are set and cleared, follow a rename
//! and go with a removal, a duplicate copies them, and the optional
//! `variant_settings` key round-trips (absent when empty, kept verbatim when
//! unreadable, kept by an older build). Set, it writes `snxprj/1.1`.

use super::*;
use serde_json::{Value, json};

fn bare() -> Value {
    json!({"name": "Proj", "dir": "", "schematic_root": null, "pcb_file": null})
}

fn project() -> ProjectData {
    let mut data: ProjectData = serde_json::from_value(bare()).unwrap();
    data.add_variant("Lite").unwrap();
    data.add_variant("Pro").unwrap();
    data
}

fn write_raw(dir: &Path, body: &Value) -> PathBuf {
    let path = dir.join("Proj.snxprj");
    std::fs::write(&path, body.to_string()).unwrap();
    path
}

fn saved_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn sheet_fit(designator: &str, fitted: bool) -> SheetFit {
    SheetFit {
        designator: designator.into(),
        part: None,
        fitted,
    }
}

/// The placed part made of `unit` on the root sheet.
fn link(unit: Uuid) -> crate::pcb::SymbolLink {
    crate::pcb::SymbolLink::new(crate::schematic::InstancePath::root(), [unit])
}

#[test]
fn a_fit_and_a_value_are_set_per_variant_and_cleared_back_to_the_sheets() {
    let mut data = project();
    data.set_variant_part_fit("lite", "J3", Some(false))
        .unwrap();
    data.set_variant_part_parameter("Lite", "R5", VALUE_PARAMETER, Some("47k"))
        .unwrap();
    let settings = &data.variant_settings;
    assert_eq!(
        settings.part(Some("LITE"), "J3").unwrap().fitted,
        Some(false)
    );
    assert_eq!(settings.part(Some("Lite"), "J3").unwrap().variant, "Lite");
    assert_eq!(
        settings.part(Some("Lite"), "R5").unwrap().parameters[VALUE_PARAMETER],
        "47k"
    );
    assert!(settings.part(Some("Pro"), "J3").is_none());
    assert!(settings.part(None, "J3").is_none(), "Base changes nothing");

    // Clearing both leaves no entry behind.
    data.set_variant_part_fit("Lite", "J3", None).unwrap();
    data.set_variant_part_parameter("Lite", "R5", VALUE_PARAMETER, None)
        .unwrap();
    assert!(data.variant_settings.is_empty());
}

#[test]
fn base_and_an_unknown_variant_are_refused_and_change_nothing() {
    let mut data = project();
    assert_eq!(
        data.set_variant_part_fit("Base", "R1", Some(false)),
        Err(VariantProblem::Reserved)
    );
    assert_eq!(
        data.set_variant_part_fit("Max", "R1", Some(false)),
        Err(VariantProblem::Unknown("Max".into()))
    );
    assert_eq!(
        data.set_default_variant(Some("Max")),
        Err(VariantProblem::Unknown("Max".into()))
    );
    assert!(data.variant_settings.is_empty());
}

#[test]
fn the_default_variant_is_set_and_follows_a_rename_and_a_removal() {
    let mut data = project();
    assert_eq!(data.default_variant(), None, "Base is the default");
    data.set_default_variant(Some("lite")).unwrap();
    assert_eq!(data.default_variant(), Some("Lite"));
    assert_eq!(
        data.active_variant, None,
        "the default is not the active one"
    );

    data.set_variant_part_fit("Lite", "U3", Some(false))
        .unwrap();
    data.rename_variant("Lite", "Mini").unwrap();
    assert_eq!(data.default_variant(), Some("Mini"));
    assert_eq!(
        data.variant_settings
            .part(Some("Mini"), "U3")
            .unwrap()
            .fitted,
        Some(false)
    );

    data.remove_variant("Mini").unwrap();
    assert_eq!(data.default_variant(), None);
    assert!(data.variant_settings.is_empty(), "its settings go with it");

    data.set_default_variant(Some("Pro")).unwrap();
    data.set_default_variant(Some("Base")).unwrap();
    assert_eq!(data.variant_settings.default, None);
}

#[test]
fn a_duplicate_copies_the_settings_and_the_sheet_fits_it_does_not_set() {
    let mut data = project();
    data.set_variant_part_fit("Lite", "U3", Some(true)).unwrap();
    data.set_variant_part_parameter("Lite", "R5", VALUE_PARAMETER, Some("47k"))
        .unwrap();
    let sheet_fits = [sheet_fit("U3", false), sheet_fit("D3", false)];
    data.duplicate_variant("Lite", "Lite copy", &sheet_fits)
        .unwrap();
    assert_eq!(data.variant_definitions, ["Lite", "Pro", "Lite copy"]);
    let copy = |designator: &str| {
        data.variant_settings
            .part(Some("Lite copy"), designator)
            .cloned()
    };
    assert_eq!(
        copy("U3").unwrap().fitted,
        Some(true),
        "its own setting wins"
    );
    assert_eq!(copy("D3").unwrap().fitted, Some(false));
    assert_eq!(copy("R5").unwrap().parameters[VALUE_PARAMETER], "47k");
    // The source keeps its own settings.
    assert!(data.variant_settings.part(Some("Lite"), "D3").is_none());

    assert_eq!(
        data.duplicate_variant("Lite", "pro", &[]),
        Err(VariantProblem::Taken("Pro".into()))
    );
    data.duplicate_variant("Base", "Plain", &[]).unwrap();
    assert!(
        data.variant_settings
            .parts
            .iter()
            .all(|part| part.variant != "Plain")
    );
}

#[test]
fn no_settings_write_no_key_and_re_save_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let data = parse_project(&path).unwrap();
    assert!(data.variant_settings.is_empty());
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();
    assert!(saved_json(&path).get("variant_settings").is_none());
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first);
}

#[test]
fn the_settings_round_trip_through_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_raw(dir.path(), &bare());
    let mut data = parse_project(&path).unwrap();
    data.add_variant("Lite").unwrap();
    data.set_default_variant(Some("Lite")).unwrap();
    data.set_variant_part_fit("Lite", "J3", Some(false))
        .unwrap();
    data.set_variant_part_parameter("Lite", "TP1", FOOTPRINT_PARAMETER, Some("TP_1.5"))
        .unwrap();
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();

    let back = parse_project(&path).unwrap();
    assert_eq!(back.variant_settings, data.variant_settings);
    write_project(&path, &back).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first, "byte-identical");

    let saved = saved_json(&path);
    assert_eq!(
        saved["format"],
        json!(SNXPRJ_FORMAT_V1_1),
        "variant settings need 1.1"
    );
    assert_eq!(
        saved["variant_settings"],
        json!({
            "default": "Lite",
            "parts": [
                {"variant": "Lite", "designator": "J3", "fitted": false},
                {"variant": "Lite", "designator": "TP1", "parameters": {"Footprint": "TP_1.5"}}
            ]
        })
    );
}

#[test]
fn an_unreadable_section_opens_the_project_is_kept_and_refuses_edits() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [
        json!("yes"),
        json!({"parts": [{"designator": "R1"}]}),
        json!({"parts": [{"variant": "Lite", "designator": "R1", "fitted": "no"}]}),
    ] {
        let mut body = bare();
        body["variant_definitions"] = json!(["Lite"]);
        body["variant_settings"] = bad.clone();
        let path = write_raw(dir.path(), &body);
        let mut data = parse_project(&path).unwrap();
        assert!(data.has_unreadable_variant_settings(), "{bad}");
        assert!(data.variant_settings.is_empty());
        assert_eq!(
            data.set_variant_part_fit("Lite", "R1", Some(false)),
            Err(VariantProblem::SettingsUnreadable)
        );
        assert_eq!(
            data.set_default_variant(Some("Lite")),
            Err(VariantProblem::SettingsUnreadable)
        );
        // Even settings that reach the model never replace the kept section.
        data.variant_settings.default = Some("Lite".into());
        write_project(&path, &data).unwrap();
        assert_eq!(saved_json(&path)["variant_settings"], bad);
    }
}

/// A build from before the key existed reads `variant_settings` into its
/// unknown-key bag and writes it back unchanged.
#[test]
fn a_build_without_the_key_keeps_it_through_its_unknown_bag() {
    #[derive(Serialize, Deserialize)]
    struct BeforeVariantSettings {
        name: String,
        #[serde(default, flatten)]
        unknown: UnknownJson,
    }
    let mut data = project();
    data.set_variant_part_fit("Lite", "J3", Some(false))
        .unwrap();
    let written: Value = serde_json::from_slice(&encode_project(&data).unwrap()).unwrap();
    let old: BeforeVariantSettings = serde_json::from_value(written.clone()).unwrap();
    let rewritten = serde_json::to_value(&old).unwrap();
    assert_eq!(rewritten["variant_settings"], written["variant_settings"]);
}

/// A newer build's keys inside the section and inside a part entry survive
/// this build's save byte-identical (#788), and an edit that clears what
/// this build knows of an entry keeps the entry for them.
#[test]
fn a_newer_builds_keys_in_the_settings_survive_a_save() {
    let dir = tempfile::tempdir().unwrap();
    let section = json!({
        "default": "Lite",
        "layout": {"columns": 3},
        "parts": [
            {"variant": "Lite", "designator": "R1", "fitted": false, "alternate": {"lib_id": "Device:R_Small"}}
        ]
    });
    let mut body = bare();
    body["variant_definitions"] = json!(["Lite"]);
    body["variant_settings"] = section.clone();
    let path = write_raw(dir.path(), &body);
    let mut data = parse_project(&path).unwrap();
    assert!(!data.has_unreadable_variant_settings());
    write_project(&path, &data).unwrap();
    let first = std::fs::read(&path).unwrap();
    assert_eq!(saved_json(&path)["variant_settings"], section);
    write_project(&path, &parse_project(&path).unwrap()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first, "byte-identical");

    data.set_variant_part_fit("Lite", "R1", None).unwrap();
    let entry = data.variant_settings.part(Some("Lite"), "R1").unwrap();
    assert_eq!(entry.fitted, None);
    assert!(entry.unknown.get("alternate").is_some(), "kept for its key");
}

/// An entry set on a placed part stays on it when a re-annotation gives it
/// another designator, and never lands on the part that takes its old one.
#[test]
fn an_entry_stays_on_its_part_through_a_new_designator() {
    let mut data = project();
    let r5 = link(Uuid::now_v7());
    let other = link(Uuid::now_v7());
    data.set_variant_part_fit("Lite", PlacedPart::new("R5", Some(&r5)), Some(false))
        .unwrap();

    // Re-annotated: the part is R7 now and another part took R5.
    let settings = &data.variant_settings;
    let moved = settings.part(Some("Lite"), PlacedPart::new("R7", Some(&r5)));
    assert_eq!(moved.unwrap().fitted, Some(false));
    assert!(
        settings
            .part(Some("Lite"), PlacedPart::new("R5", Some(&other)))
            .is_none()
    );

    // An edit under the new designator changes the same entry and renames it.
    data.set_variant_part_parameter(
        "Lite",
        PlacedPart::new("R7", Some(&r5)),
        VALUE_PARAMETER,
        Some("47k"),
    )
    .unwrap();
    let parts = &data.variant_settings.parts;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].designator, "R7");
    assert_eq!(parts[0].fitted, Some(false));
    assert_eq!(parts[0].part.as_ref(), Some(&r5));

    // The link round-trips through the file's JSON.
    let json = serde_json::to_value(&data.variant_settings).unwrap();
    assert_eq!(
        json["parts"][0]["part"],
        json!({"sheet": [], "units": [r5.units[0].to_string()]})
    );
    let back: VariantSettings = serde_json::from_value(json).unwrap();
    assert_eq!(back, data.variant_settings);
}

/// While the section is unreadable a rename or a removal is refused too:
/// the kept section would still name the old variant.
#[test]
fn an_unreadable_section_refuses_a_rename_and_a_removal() {
    let dir = tempfile::tempdir().unwrap();
    let mut body = bare();
    body["variant_definitions"] = json!(["Lite"]);
    body["variant_settings"] = json!("yes");
    let path = write_raw(dir.path(), &body);
    let mut data = parse_project(&path).unwrap();
    assert_eq!(
        data.rename_variant("Lite", "Lite2"),
        Err(VariantProblem::SettingsUnreadable)
    );
    assert_eq!(
        data.remove_variant("Lite"),
        Err(VariantProblem::SettingsUnreadable)
    );
    assert_eq!(
        data.duplicate_variant("Lite", "Lite copy", &[]),
        Err(VariantProblem::SettingsUnreadable)
    );
    assert_eq!(data.variant_definitions, ["Lite"]);
}
