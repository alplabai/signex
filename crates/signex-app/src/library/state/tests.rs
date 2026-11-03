//! Tests for library UI state.
use super::*;

use crate::library::test_adapters::{FailingListingAdapter, cached_summary};

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
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
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
pub(super) fn fixture_row(pn: &str) -> ComponentRow {
    use signex_library::{
        ComponentClass, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart, ParamMap,
        PinPadOverride, PlmReserved,
    };
    let _ = (PinPadOverride::new("1", "1"),); // module touch
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new(pn),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
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
        server_revision: None,
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
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        internal_pn: InternalPn::new("R0805_10k"),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
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
        server_revision: None,
    };
    let address = EditorAddress::new(
        PathBuf::from("/tmp/x.snxlib"),
        "resistors".to_string(),
        RowId::from_uuid(row.row_id),
    );
    let modal = EditRowModalState::new(address, row);
    assert_eq!(modal.tags_buf, "low-noise, RoHS");
}

// -- A failed primitive listing must not blank the picker (#599) ----------

/// An `OpenLibrary` whose three primitive caches already hold one
/// entry each — the state a mounted library is in before any refresh.
fn library_with_cached_primitives(root: &Path, library_id: Uuid) -> OpenLibrary {
    OpenLibrary {
        root: root.to_path_buf(),
        display_name: "FailingLib".into(),
        library_id,
        tables: HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: vec![cached_summary("R_0805")],
        cached_footprints: vec![cached_summary("SOIC-8")],
        cached_sims: vec![cached_summary("BC547.lib")],
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    }
}

/// `reload_primitives` used to assign `Vec::new()` on every `Err` arm,
/// so one failed listing reported a mounted library as having no
/// symbols at all and the user re-created a primitive already on disk.
#[test]
fn a_failed_listing_keeps_the_primitive_caches_it_could_not_refresh() {
    let library_id = Uuid::now_v7();
    let mut lib = library_with_cached_primitives(Path::new("/tmp/failing.snxlib"), library_id);
    let adapter = FailingListingAdapter::new(library_id);

    lib.reload_primitives(&adapter);

    assert_eq!(
        lib.cached_symbols.len(),
        1,
        "a failed symbol listing must keep the entries the picker was already showing"
    );
    assert_eq!(lib.cached_symbols[0].name, "R_0805");
    assert_eq!(lib.cached_footprints.len(), 1, "same for footprints");
    assert_eq!(lib.cached_sims.len(), 1, "same for sims");
}

/// `refresh_components` swallowed the same three calls with
/// `unwrap_or_default()` and no log at all — the empty vecs landed in
/// the caches the primitive picker renders.
#[test]
fn refresh_components_keeps_the_caches_when_the_listings_fail() {
    let root = PathBuf::from("/tmp/failing-refresh.snxlib");
    let library_id = Uuid::now_v7();
    let mut state = LibraryState::default();
    state
        .set
        .mount(Box::new(FailingListingAdapter::new(library_id)))
        .expect("mount stub adapter");
    state
        .open_libraries
        .push(library_with_cached_primitives(&root, library_id));

    state
        .refresh_components(&root)
        .expect("refresh_components succeeds — only the primitive listings fail");

    let lib = state.library_at(&root).expect("library still open");
    assert_eq!(
        lib.cached_symbols.len(),
        1,
        "a mounted library whose listing failed is not an empty library"
    );
    assert_eq!(lib.cached_footprints.len(), 1);
    assert_eq!(lib.cached_sims.len(), 1);
}

/// The keep-the-cache behaviour is only half the fix: silently keeping
/// a stale cache is still a swallowed error, so the failure has to
/// reach the Messages panel.
#[test]
fn a_failed_listing_reaches_the_messages_panel() {
    let _ = crate::diagnostics::init_logging();
    let library_id = Uuid::now_v7();
    let mut lib = library_with_cached_primitives(Path::new("/tmp/failing-log.snxlib"), library_id);
    // Unique display name — the diagnostics ring is global and every
    // other test in the binary writes to it.
    let marker = format!("FailingLib-{}", Uuid::now_v7().simple());
    lib.display_name = marker.clone();
    let adapter = FailingListingAdapter::new(library_id);

    lib.reload_primitives(&adapter);

    let records: Vec<_> = crate::diagnostics::recent_entries()
        .into_iter()
        .filter(|entry| entry.message.contains(&marker))
        .collect();
    assert_eq!(
        records.len(),
        3,
        "one record per failed listing: symbols, footprints, sims"
    );
    assert!(
        records
            .iter()
            .all(|entry| entry.level == crate::diagnostics::DiagnosticLevel::Warning),
        "a kept cache is a degraded path that continued, not a lost edit"
    );
}

/// #1356 -- a primitive ref resolves through its own `library_id`, not the
/// row's library; an unopened library yields no path.
#[test]
fn primitive_file_resolves_through_the_ref_library_id() {
    use signex_library::PrimitiveRef;
    let (lib_a, lib_b) = (Uuid::now_v7(), Uuid::now_v7());
    let mut state = LibraryState::default();
    for (id, root) in [(lib_a, "/tmp/a/A.snxlib"), (lib_b, "/tmp/b/B.snxlib")] {
        state.open_libraries.push(OpenLibrary {
            root: std::path::PathBuf::from(root),
            display_name: String::new(),
            library_id: id,
            tables: Default::default(),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            pricing: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
        });
    }
    let prim = Uuid::now_v7();
    let path = state
        .primitive_file(&PrimitiveRef::new(lib_b, prim), "symbols", "snxsym")
        .unwrap();
    assert_eq!(
        path,
        std::path::Path::new("/tmp/b/symbols").join(format!("{prim}.snxsym"))
    );
    assert!(
        state
            .primitive_file(
                &PrimitiveRef::new(Uuid::now_v7(), prim),
                "symbols",
                "snxsym"
            )
            .is_none()
    );
}

/// `reload_tables` (the open/mount path) left `pricing` empty, so a
/// freshly mounted library showed no cached prices until a later
/// `refresh_components`.
#[test]
fn reload_tables_loads_cached_pricing() {
    let library_id = Uuid::now_v7();
    let adapter = crate::library::test_adapters::PricedAdapter::new(library_id);
    let mut lib = library_with_cached_primitives(Path::new("/tmp/priced.snxlib"), library_id);
    assert!(lib.pricing.is_empty());

    lib.reload_tables(&adapter).expect("reload");

    assert!(lib.pricing.contains_key(&adapter.priced_row));
}

/// The row's description reaches the picker summary (#1474).
#[test]
fn summarize_row_carries_the_row_description() {
    let mut row = fixture_row("C1");
    row.description = "100 nF 50 V X7R".into();
    assert_eq!(
        super::methods::summarize_row(&row).description,
        "100 nF 50 V X7R"
    );
    assert_eq!(
        super::methods::summarize_row(&fixture_row("C2")).description,
        ""
    );
}

/// The off-thread refresh read built every summary with an empty
/// description, so the picker's description column and filter went dead
/// after the first refresh.
#[test]
fn refresh_snapshot_carries_row_descriptions() {
    let mut row = fixture_row("C1");
    row.description = "100 nF 50 V X7R".into();
    let adapter =
        crate::library::test_adapters::PricedAdapter::new(Uuid::now_v7()).with_rows(vec![row]);
    let snapshot = LibrarySnapshot::read(&adapter, Path::new("/tmp/desc.snxlib")).expect("read");

    assert_eq!(snapshot.summaries()[0].description, "100 nF 50 V X7R");
}

fn bare_library(root: &str) -> OpenLibrary {
    OpenLibrary {
        root: PathBuf::from(root),
        display_name: root.into(),
        library_id: Uuid::now_v7(),
        tables: HashMap::new(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    }
}

#[test]
fn containing_library_picks_the_deepest_library_in_either_mount_order() {
    let file = Path::new("/p/vendor/symbols/a.snxsym");
    for order in [
        ["/p/lib.snxlib", "/p/vendor/v.snxlib"],
        ["/p/vendor/v.snxlib", "/p/lib.snxlib"],
    ] {
        let mut state = LibraryState {
            open_libraries: order.iter().map(|r| bare_library(r)).collect(),
            ..Default::default()
        };
        assert_eq!(
            state
                .containing_library_with(file, crate::app::CaseFold::host_default())
                .map(|l| l.root.clone()),
            Some(PathBuf::from("/p/vendor/v.snxlib")),
            "mount order {order:?}"
        );
        assert_eq!(
            state
                .containing_library_mut_with(file, crate::app::CaseFold::host_default())
                .map(|l| l.root.clone()),
            Some(PathBuf::from("/p/vendor/v.snxlib")),
        );
        // A file of the outer library only still belongs to the outer one.
        assert_eq!(
            state
                .containing_library_with(
                    Path::new("/p/symbols/b.snxsym"),
                    crate::app::CaseFold::host_default()
                )
                .map(|l| l.root.clone()),
            Some(PathBuf::from("/p/lib.snxlib")),
        );
    }
}

#[test]
fn containing_library_with_follows_the_case_rule_it_is_given() {
    let state = LibraryState {
        open_libraries: vec![bare_library("/p/Lib.snxlib")],
        ..Default::default()
    };
    let folding = crate::app::CaseFold::FOLD;
    let preserving = crate::app::CaseFold::PRESERVE;
    // The library folder is `/p`; only the `P` spelling differs.
    let variant = Path::new("/P/symbols/a.snxsym");
    assert!(state.containing_library_with(variant, folding).is_some());
    assert!(state.containing_library_with(variant, preserving).is_none());
}

#[test]
fn two_libraries_in_one_folder_leave_the_owner_unknown_in_either_order() {
    let file = Path::new("/p/symbols/a.snxsym");
    for order in [
        ["/p/a.snxlib", "/p/b.snxlib"],
        ["/p/b.snxlib", "/p/a.snxlib"],
    ] {
        let state = LibraryState {
            open_libraries: order.iter().map(|r| bare_library(r)).collect(),
            ..Default::default()
        };
        assert!(
            state
                .containing_library_with(file, crate::app::CaseFold::host_default())
                .is_none(),
            "{order:?}"
        );
    }
    // A deeper library still wins over an ambiguous pair above it.
    let state = LibraryState {
        open_libraries: ["/p/a.snxlib", "/p/b.snxlib", "/p/v/c.snxlib"]
            .iter()
            .map(|r| bare_library(r))
            .collect(),
        ..Default::default()
    };
    let deep = Path::new("/p/v/symbols/a.snxsym");
    assert_eq!(
        state
            .containing_library_with(deep, crate::app::CaseFold::host_default())
            .map(|l| l.root.clone()),
        Some(PathBuf::from("/p/v/c.snxlib"))
    );
}

#[test]
fn a_tie_is_broken_like_the_write_lock_whatever_the_mount_order() {
    use crate::app::CaseFold;
    let dir = tempfile::tempdir().unwrap();
    let names = ["Passives.snxlib", "connectors.snxlib"];
    for name in names {
        std::fs::write(dir.path().join(name), "").unwrap();
    }
    let file = dir.path().join("symbols").join("a.snxsym");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let locked = signex_library::enclosing_library_file(&file).expect("a library file");
    for order in [names, [names[1], names[0]]] {
        let state = LibraryState {
            open_libraries: order
                .iter()
                .map(|n| bare_library(dir.path().join(n).to_str().unwrap()))
                .collect(),
            ..Default::default()
        };
        for fold in [CaseFold::FOLD, CaseFold::PRESERVE] {
            assert_eq!(
                state.write_target_library(&file, fold).map(|l| &l.root),
                Some(&locked),
                "{order:?} {fold:?}"
            );
        }
        // A tie names no owner, but the folder is still held by a library.
        assert!(
            state
                .containing_library_with(&file, CaseFold::FOLD)
                .is_none()
        );
        assert!(state.is_in_mounted_library(&file, CaseFold::FOLD));
    }
}

#[test]
fn library_holding_requires_the_primitive_folder_of_the_owner() {
    let state = LibraryState {
        open_libraries: vec![bare_library("/p/lib.snxlib")],
        ..Default::default()
    };
    use signex_library::PrimitiveKind::{Footprint, Symbol};
    let holds = |file: &str, kind| {
        state
            .library_holding_with(Path::new(file), kind, crate::app::CaseFold::host_default())
            .is_some()
    };
    assert!(holds("/p/symbols/a.snxsym", Symbol));
    assert!(holds("/p/footprints/a.snxfpt", Footprint));
    assert!(!holds("/p/drafts/a.snxsym", Symbol));
    assert!(!holds("/p/vendor/symbols/a.snxsym", Symbol));
    // A file in another kind's folder is not held as this kind.
    assert!(!holds("/p/footprints/a.snxsym", Symbol));
    assert!(!holds("/p/symbols/a.snxfpt", Footprint));
}
