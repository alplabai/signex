//! Components panel test fixtures, the mount-source order and the add
//! menu the empty card opens. The table's own tests are in `table/`.

use std::path::PathBuf;

use super::*;
use crate::library::state::{ComponentsMountSource, OpenLibrary};
use signex_i18n::I18n;
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, PinPadOverride, PlmReserved,
};
use uuid::Uuid;

/// The thumbnail look the view tests draw rows with.
pub(super) fn thumb_look() -> thumbs::ThumbLook {
    thumbs::ThumbLook {
        colors: signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteLight),
        device_px_per_logical_px: 1.0,
    }
}

pub(super) fn fixture_row(pn: &str, mpn: &str, mfr: &str) -> ComponentRow {
    let _ = (PinPadOverride::new("1", "1"),);
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
        primary_mpn: ManufacturerPart::draft(mfr, mpn),
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

/// One library holding `rows`, its view caches current.
pub(super) fn library_with(rows: Vec<ComponentRow>) -> LibraryState {
    let mut state = LibraryState::default();
    state.open_libraries.push(OpenLibrary {
        root: PathBuf::from("/tmp/one.snxlib"),
        display_name: "one".into(),
        library_id: uuid::Uuid::now_v7(),
        tables: std::collections::HashMap::from([("t".to_owned(), rows)]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        alternate_supply: Default::default(),
        own_edit_stamp: None,
        display: Default::default(),
        provenance: Default::default(),
    });
    state.touch_data();
    state.sync_view_caches();
    state
}

#[test]
fn mount_sources_list_stock_first_then_project_installed_global() {
    assert_eq!(
        ComponentsMountSource::ORDER,
        &[
            ComponentsMountSource::Stock,
            ComponentsMountSource::Project,
            ComponentsMountSource::Installed,
            ComponentsMountSource::Global,
        ]
    );
}

#[test]
fn mount_source_label_and_key_distinct() {
    for src in ComponentsMountSource::ORDER {
        assert!(!src.label().is_empty());
        assert!(!src.key().is_empty());
        assert_ne!(src.label(), src.key());
    }
}

/// The "+ Add" menu (opened by the empty card's Add Library…) offers
/// Project, Installed and Global, each the add-library command for that
/// source.
#[test]
fn the_add_menu_offers_project_installed_and_global() {
    assert_eq!(
        ComponentsMountSource::ADDABLE,
        &[
            ComponentsMountSource::Project,
            ComponentsMountSource::Installed,
            ComponentsMountSource::Global,
        ]
    );
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let texts = crate::layout_probe::lay_out_texts(
        look::add_menu(&tokens, &I18n::en_us()),
        iced::Size::new(300.0, 300.0),
    );
    for source in ComponentsMountSource::ADDABLE {
        crate::layout_probe::find(&texts, source.label());
    }
}
