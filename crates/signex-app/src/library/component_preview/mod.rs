//! The Component Preview surface: inline editing of a library component
//! row (datasheet, pin map, supply, parameters, simulation) without
//! opening the standalone primitive editors.
//!
//! The update logic that applies these inline edits lives in [`updates`],
//! split by field-group concern.

pub(crate) mod updates;

pub(crate) use updates::apply_inline_edit;

/// A draft row with no datasheet, for tests.
#[cfg(test)]
pub(crate) fn test_row() -> signex_library::ComponentRow {
    use signex_library::{
        ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
        ParamMap, PlmReserved, PrimitiveRef,
    };
    use uuid::Uuid;
    ComponentRow {
        row_id: Uuid::new_v4(),
        internal_pn: InternalPn::new("U1"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(Uuid::new_v4(), Uuid::new_v4()),
        alternate_symbols: Vec::new(),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: ParamMap::new(),
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
        content_hash: [0u8; 32],
        server_revision: None,
    }
}
