//! `Default` for the test-fixture types must equal what serde yields for a
//! manifest holding only the required keys.

use super::*;
use serde::de::DeserializeOwned;
use serde_json::json;

fn assert_default_matches<T>(minimal: serde_json::Value)
where
    T: Default + Serialize + DeserializeOwned,
{
    let from_serde: T = serde_json::from_value(minimal).expect("minimal manifest deserializes");
    assert_eq!(
        serde_json::to_value(T::default()).expect("default serializes"),
        serde_json::to_value(from_serde).expect("deserialized value serializes"),
    );
}

#[test]
fn sheet_default_equals_deserialized_empty_manifest() {
    assert_default_matches::<SchematicSheet>(json!({ "uuid": Uuid::nil() }));
}

#[test]
fn symbol_default_equals_deserialized_minimal_manifest() {
    assert_default_matches::<Symbol>(json!({
        "uuid": Uuid::nil(),
        "lib_id": "",
        "position": { "x": 0.0, "y": 0.0 },
    }));
}

#[test]
fn wire_default_equals_deserialized_minimal_manifest() {
    let origin = json!({ "x": 0.0, "y": 0.0 });
    assert_default_matches::<Wire>(json!({
        "uuid": Uuid::nil(), "start": origin, "end": origin,
    }));
}

#[test]
fn junction_default_equals_deserialized_minimal_manifest() {
    assert_default_matches::<Junction>(json!({
        "uuid": Uuid::nil(),
        "position": { "x": 0.0, "y": 0.0 },
    }));
}

// Serialization-level equality is blind to `skip_serializing*` fields (`unknown`,
// `unknown_columns`). These destructures have no `..`, so adding a field to a
// type fails to compile here until the tests above are revisited.
#[test]
fn every_field_is_accounted_for() {
    let SchematicSheet {
        uuid: _,
        version: _,
        generator: _,
        generator_version: _,
        paper_size: _,
        root_sheet_page: _,
        symbols: _,
        wires: _,
        junctions: _,
        labels: _,
        child_sheets: _,
        no_connects: _,
        text_notes: _,
        buses: _,
        bus_entries: _,
        drawings: _,
        no_erc_directives: _,
        title_block: _,
        lib_symbols: _,
        unknown: _,
    } = SchematicSheet::default();
    let Symbol {
        uuid: _,
        lib_id: _,
        reference: _,
        value: _,
        footprint: _,
        datasheet: _,
        position: _,
        rotation: _,
        mirror_x: _,
        mirror_y: _,
        unit: _,
        is_power: _,
        ref_text: _,
        val_text: _,
        fields_autoplaced: _,
        fields_user_placed: _,
        dnp: _,
        in_bom: _,
        on_board: _,
        exclude_from_sim: _,
        locked: _,
        fields: _,
        custom_properties: _,
        pin_uuids: _,
        instances: _,
        library_id: _,
        row_id: _,
        library_version: _,
        unknown: _,
        unknown_columns: _,
    } = Symbol::default();
    let Wire {
        uuid: _,
        start: _,
        end: _,
        stroke_width: _,
        unknown_columns: _,
    } = Wire::default();
    let Junction {
        uuid: _,
        position: _,
        diameter: _,
        minted: _,
        unknown_columns: _,
    } = Junction::default();
}
