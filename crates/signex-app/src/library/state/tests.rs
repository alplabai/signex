//! Tests for library UI state.
use super::*;

#[test]
fn picker_state_defaults_to_empty() {
    let s = PickerState::default();
    assert!(s.filter.is_empty());
    assert!(s.selected.is_none());
}

#[test]
fn distributor_settings_default_order() {
    let s = DistributorSettings::default();
    assert_eq!(s.preferred_order.len(), 4);
    assert_eq!(s.preferred_order[0], DistributorSource::DigiKey);
}

#[test]
fn preview_tab_order_is_five_tabs() {
    assert_eq!(PreviewTab::ORDER[0], PreviewTab::Preview);
    assert_eq!(PreviewTab::ORDER.last(), Some(&PreviewTab::Simulation));
    assert_eq!(PreviewTab::ORDER.len(), 5);
}

#[test]
fn preview_tab_labels_are_short_and_distinct() {
    let labels: std::collections::HashSet<&str> =
        PreviewTab::ORDER.iter().map(|t| t.label()).collect();
    assert_eq!(labels.len(), PreviewTab::ORDER.len());
}

#[test]
fn new_component_state_defaults_to_generic_class() {
    let nc = NewComponentState::default();
    assert!(nc.internal_pn.is_empty());
    assert!(nc.library_idx.is_none());
    // Table starts unset until the user picks one in the modal.
    assert!(nc.table.is_none());
    assert_eq!(nc.class, ComponentClass::generic());
    assert!(nc.category.is_empty());
}

#[test]
fn library_set_mount_unmount_is_symmetric() {
    let mut set = LibrarySet::new();
    assert!(set.is_empty());
    // Mounting requires a real adapter; testing only the bookkeeping
    // shape here. (Full end-to-end test lives in `commands.rs`.)
    let _ = set.unmount(Uuid::nil());
    assert!(set.is_empty());
}

/// `OpenLibrary::total_rows` sums every cached table's length —
/// feeds the panel's library-node `(N)` count.
#[test]
fn open_library_total_rows_sums_tables() {
    let mut lib = OpenLibrary {
        root: PathBuf::from("/tmp/x.snxlib"),
        display_name: "X".into(),
        library_id: Uuid::nil(),
        tables: HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        display: LibraryDisplaySettings::default(),
    };
    assert_eq!(lib.total_rows(), 0);
    lib.tables.insert("resistors".into(), Vec::new());
    assert_eq!(lib.total_rows(), 0);
    lib.tables.insert(
        "capacitors".into(),
        vec![fixture_row("C1"), fixture_row("C2")],
    );
    lib.tables
        .insert("resistors".into(), vec![fixture_row("R1")]);
    assert_eq!(lib.total_rows(), 3);
}

/// Helper — minimal `ComponentRow` for the panel-side cache tests.
/// The full row schema lives in `signex_library`'s tests.
fn fixture_row(pn: &str) -> ComponentRow {
    use signex_library::{
        DatasheetRef, InternalPn, LifecycleState, ManufacturerPart, ParamMap, PinPadOverride,
        PlmReserved,
    };
    let _ = (PinPadOverride::new("1", "1"),); // module touch
    ComponentRow {
        row_id: Uuid::new_v4(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::new_v4()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Mfr", "MPN"),
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
    }
}

/// LifecycleFilter Released-only mode keeps `Released` rows and
/// drops every other state — guards plan §6's "preferred only"
/// pivot from drifting back to "active + preferred".
#[test]
fn lifecycle_filter_preferred_only_isolates_released() {
    use signex_library::LifecycleState as L;
    let f = LifecycleFilter::PreferredOnly;
    assert!(f.allows(L::Released));
    assert!(!f.allows(L::Draft));
    assert!(!f.allows(L::InReview));
    assert!(!f.allows(L::Deprecated));
    assert!(!f.allows(L::Obsolete));
}

#[test]
fn lifecycle_filter_default_hides_obsolete_and_deprecated() {
    use signex_library::LifecycleState as L;
    let f = LifecycleFilter::default();
    assert!(matches!(f, LifecycleFilter::ActiveAndPreferred));
    assert!(f.allows(L::Released));
    assert!(f.allows(L::InReview));
    assert!(f.allows(L::Draft));
    assert!(!f.allows(L::Deprecated));
    assert!(!f.allows(L::Obsolete));
}

#[test]
fn lifecycle_filter_include_deprecated_keeps_deprecated_only() {
    use signex_library::LifecycleState as L;
    let f = LifecycleFilter::IncludeDeprecated;
    assert!(f.allows(L::Released));
    assert!(f.allows(L::Deprecated));
    assert!(!f.allows(L::Obsolete));
}

#[test]
fn lifecycle_filter_all_admits_every_state() {
    use signex_library::LifecycleState as L;
    let f = LifecycleFilter::All;
    for s in [
        L::Released,
        L::InReview,
        L::Draft,
        L::Deprecated,
        L::Obsolete,
    ] {
        assert!(f.allows(s), "All filter should allow {s:?}");
    }
}

#[test]
fn library_browser_state_defaults_lifecycle_filter() {
    let s = LibraryBrowserState::new(PathBuf::from("/tmp/x.snxlib"));
    assert!(matches!(
        s.lifecycle_filter,
        LifecycleFilter::ActiveAndPreferred
    ));
    assert!(s.search.is_empty());
}

/// `EditRowModalState::new` seeds `tags_buf` from
/// `parameters["tags"]` so the modal opens with the existing
/// tags rendered in the input — Stage 18 lifecycle/tag UX.
#[test]
fn edit_row_modal_state_seeds_tags_buffer() {
    use signex_library::{
        ComponentClass, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart, ParamMap,
        ParamValue, PinPadOverride, PlmReserved,
    };
    let _ = (PinPadOverride::new("1", "1"),);
    let mut params = ParamMap::new();
    params.insert(
        "tags".to_string(),
        ParamValue::Text("low-noise, RoHS".to_string()),
    );
    let row = ComponentRow {
        row_id: Uuid::new_v4(),
        internal_pn: InternalPn::new("R0805_10k"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        state: LifecycleState::Draft,
        symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::new_v4()),
        footprint_ref: None,
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Mfr", "MPN"),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters: params,
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: String::new(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: chrono::Utc::now(),
        updated: chrono::Utc::now(),
        content_hash: [0u8; 32],
    };
    let address = EditorAddress::new(
        PathBuf::from("/tmp/x.snxlib"),
        "resistors".to_string(),
        RowId::from_uuid(row.row_id),
    );
    let modal = EditRowModalState::new(address, row);
    assert_eq!(modal.tags_buf, "low-noise, RoHS");
}
