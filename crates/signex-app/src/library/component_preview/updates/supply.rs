//! Supply-chain edits for a Component Preview row: the primary
//! manufacturer part, alternate parts, and distributor listings.

use crate::library::ComponentPreviewState;
use crate::library::editor::supply::distributor_source_to_string;
use signex_library::DistributorSource;

// ── Primary manufacturer part ───────────────────────────────────────

/// Set the primary part's manufacturer name.
pub(super) fn set_primary_manufacturer(state: &mut ComponentPreviewState, value: String) {
    state.row.primary_mpn.manufacturer = value;
    state.mark_dirty();
}

/// Set the primary part's manufacturer part number.
pub(super) fn set_primary_mpn(state: &mut ComponentPreviewState, value: String) {
    state.row.primary_mpn.mpn = value;
    state.mark_dirty();
}

/// Set the primary part's lifecycle status.
pub(super) fn set_primary_status(
    state: &mut ComponentPreviewState,
    value: signex_library::AlternateStatus,
) {
    state.row.primary_mpn.status = value;
    state.mark_dirty();
}

/// Set the primary part's free-text notes, storing `None` when blank.
pub(super) fn set_primary_notes(state: &mut ComponentPreviewState, value: String) {
    state.row.primary_mpn.notes = if value.trim().is_empty() {
        None
    } else {
        Some(value)
    };
    state.mark_dirty();
}

// ── Alternate parts ─────────────────────────────────────────────────

/// Append a new, approved alternate part draft.
pub(super) fn add_alternate(state: &mut ComponentPreviewState) {
    let mut alt = signex_library::ManufacturerPart::draft("", "");
    alt.status = signex_library::AlternateStatus::Approved;
    state.row.alternates.push(alt);
    state.mark_dirty();
}

/// Set alternate `idx`'s manufacturer name.
pub(super) fn set_alternate_manufacturer(
    state: &mut ComponentPreviewState,
    idx: usize,
    value: String,
) {
    if let Some(alt) = state.row.alternates.get_mut(idx) {
        alt.manufacturer = value;
        state.mark_dirty();
    }
}

/// Set alternate `idx`'s manufacturer part number.
pub(super) fn set_alternate_mpn(state: &mut ComponentPreviewState, idx: usize, value: String) {
    if let Some(alt) = state.row.alternates.get_mut(idx) {
        alt.mpn = value;
        state.mark_dirty();
    }
}

/// Set alternate `idx`'s lifecycle status.
pub(super) fn set_alternate_status(
    state: &mut ComponentPreviewState,
    idx: usize,
    value: signex_library::AlternateStatus,
) {
    if let Some(alt) = state.row.alternates.get_mut(idx) {
        alt.status = value;
        state.mark_dirty();
    }
}

/// Set alternate `idx`'s free-text notes, storing `None` when blank.
pub(super) fn set_alternate_notes(state: &mut ComponentPreviewState, idx: usize, value: String) {
    if let Some(alt) = state.row.alternates.get_mut(idx) {
        alt.notes = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        state.mark_dirty();
    }
}

/// Set alternate `idx`'s hand-entered stock from the typed `text` (#3046):
/// blank clears it; text that is not a whole number of units is ignored, so
/// the field keeps its last value.
pub(super) fn set_alternate_stock(state: &mut ComponentPreviewState, idx: usize, text: &str) {
    let text = text.trim();
    let value = if text.is_empty() {
        None
    } else {
        match text.parse::<u32>() {
            Ok(units) => Some(units),
            Err(_) => return,
        }
    };
    if let Some(alt) = state.row.alternates.get_mut(idx)
        && alt.manual_stock != value
    {
        alt.manual_stock = value;
        state.mark_dirty();
    }
}

/// Set alternate `idx`'s hand-entered lifecycle; `None` uses the fetched one.
pub(super) fn set_alternate_lifecycle(
    state: &mut ComponentPreviewState,
    idx: usize,
    value: Option<signex_library::ManufacturerLifecycle>,
) {
    if let Some(alt) = state.row.alternates.get_mut(idx)
        && alt.manual_lifecycle != value
    {
        alt.manual_lifecycle = value;
        state.mark_dirty();
    }
}

/// Remove alternate `idx` when it is in range.
pub(super) fn remove_alternate(state: &mut ComponentPreviewState, idx: usize) {
    if idx < state.row.alternates.len() {
        state.row.alternates.remove(idx);
        state.mark_dirty();
    }
}

// ── Distributor listings ────────────────────────────────────────────

/// Append a new, empty distributor listing.
pub(super) fn add_listing(state: &mut ComponentPreviewState) {
    state.row.supply.push(signex_library::DistributorListing {
        distributor: String::new(),
        sku: String::new(),
        url: None,
        moq: None,
    });
    state.mark_dirty();
}

/// Set listing `idx`'s distributor from a picked source.
pub(super) fn set_listing_distributor(
    state: &mut ComponentPreviewState,
    idx: usize,
    value: DistributorSource,
) {
    if let Some(listing) = state.row.supply.get_mut(idx) {
        listing.distributor = distributor_source_to_string(value);
        state.mark_dirty();
    }
}

/// Set listing `idx`'s distributor SKU.
pub(super) fn set_listing_sku(state: &mut ComponentPreviewState, idx: usize, value: String) {
    if let Some(listing) = state.row.supply.get_mut(idx) {
        listing.sku = value;
        state.mark_dirty();
    }
}

/// Set listing `idx`'s product URL, storing `None` when blank.
pub(super) fn set_listing_url(state: &mut ComponentPreviewState, idx: usize, value: String) {
    if let Some(listing) = state.row.supply.get_mut(idx) {
        listing.url = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        state.mark_dirty();
    }
}

/// Remove listing `idx` when it is in range.
pub(super) fn remove_listing(state: &mut ComponentPreviewState, idx: usize) {
    if idx < state.row.supply.len() {
        state.row.supply.remove(idx);
        state.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{
        ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState,
        ManufacturerLifecycle, ManufacturerPart, ParamMap, PlmReserved, PrimitiveRef,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn editor() -> ComponentPreviewState {
        let row = ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::now_v7(),
            internal_pn: InternalPn::new("C-22U"),
            class: ComponentClass::generic(),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Murata", "GRM21BR61A226ME44"),
            alternates: vec![ManufacturerPart::draft("TDK", "C2012X5R1A226M125AB")],
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
        };
        ComponentPreviewState::from_row(PathBuf::from("passives.snxlib"), "caps".into(), row)
    }

    /// #3046: the Supply tab's hand-entered stock takes whole units, blank
    /// clears it, and other text leaves it as it was.
    #[test]
    fn a_typed_stock_sets_clears_or_is_ignored() {
        let mut state = editor();
        set_alternate_stock(&mut state, 0, " 1200 ");
        assert_eq!(state.row.alternates[0].manual_stock, Some(1200));
        assert!(state.dirty);
        set_alternate_stock(&mut state, 0, "12x");
        assert_eq!(state.row.alternates[0].manual_stock, Some(1200));
        set_alternate_stock(&mut state, 0, "-3");
        assert_eq!(state.row.alternates[0].manual_stock, Some(1200));
        set_alternate_stock(&mut state, 0, "");
        assert_eq!(state.row.alternates[0].manual_stock, None);
        set_alternate_stock(&mut state, 7, "5");
        assert_eq!(
            state.row.alternates.len(),
            1,
            "out of range changes nothing"
        );
    }

    #[test]
    fn a_picked_lifecycle_overrides_and_none_uses_the_fetched_one() {
        let mut state = editor();
        set_alternate_lifecycle(&mut state, 0, Some(ManufacturerLifecycle::Eol));
        assert_eq!(
            state.row.alternates[0].manual_lifecycle,
            Some(ManufacturerLifecycle::Eol)
        );
        assert!(state.dirty);
        set_alternate_lifecycle(&mut state, 0, None);
        assert_eq!(state.row.alternates[0].manual_lifecycle, None);
    }

    #[test]
    fn an_unchanged_value_does_not_dirty_the_editor() {
        let mut state = editor();
        set_alternate_stock(&mut state, 0, "");
        set_alternate_lifecycle(&mut state, 0, None);
        assert!(!state.dirty);
    }
}
