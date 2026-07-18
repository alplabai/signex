//! Supply-chain edits for a Component Preview row: the primary
//! manufacturer part, alternate parts, and distributor listings.

use crate::library::ComponentPreviewState;
use crate::library::editor::supply::distributor_source_to_string;
use signex_library::DistributorSource;

// ── Primary manufacturer part ───────────────────────────────────────

/// Set the primary part's manufacturer name.
pub(super) fn set_primary_manufacturer(state: &mut ComponentPreviewState, value: String) {
    state.row.primary_mpn.manufacturer = value;
    state.dirty = true;
}

/// Set the primary part's manufacturer part number.
pub(super) fn set_primary_mpn(state: &mut ComponentPreviewState, value: String) {
    state.row.primary_mpn.mpn = value;
    state.dirty = true;
}

/// Set the primary part's lifecycle status.
pub(super) fn set_primary_status(
    state: &mut ComponentPreviewState,
    value: signex_library::AlternateStatus,
) {
    state.row.primary_mpn.status = value;
    state.dirty = true;
}

/// Set the primary part's free-text notes, storing `None` when blank.
pub(super) fn set_primary_notes(state: &mut ComponentPreviewState, value: String) {
    state.row.primary_mpn.notes = if value.trim().is_empty() {
        None
    } else {
        Some(value)
    };
    state.dirty = true;
}

// ── Alternate parts ─────────────────────────────────────────────────

/// Append a new, approved alternate part draft.
pub(super) fn add_alternate(state: &mut ComponentPreviewState) {
    let mut alt = signex_library::ManufacturerPart::draft("", "");
    alt.status = signex_library::AlternateStatus::Approved;
    state.row.alternates.push(alt);
    state.dirty = true;
}

/// Set alternate `idx`'s manufacturer name.
pub(super) fn set_alternate_manufacturer(
    state: &mut ComponentPreviewState,
    idx: usize,
    value: String,
) {
    if let Some(alt) = state.row.alternates.get_mut(idx) {
        alt.manufacturer = value;
        state.dirty = true;
    }
}

/// Set alternate `idx`'s manufacturer part number.
pub(super) fn set_alternate_mpn(state: &mut ComponentPreviewState, idx: usize, value: String) {
    if let Some(alt) = state.row.alternates.get_mut(idx) {
        alt.mpn = value;
        state.dirty = true;
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
        state.dirty = true;
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
        state.dirty = true;
    }
}

/// Remove alternate `idx` when it is in range.
pub(super) fn remove_alternate(state: &mut ComponentPreviewState, idx: usize) {
    if idx < state.row.alternates.len() {
        state.row.alternates.remove(idx);
        state.dirty = true;
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
    state.dirty = true;
}

/// Set listing `idx`'s distributor from a picked source.
pub(super) fn set_listing_distributor(
    state: &mut ComponentPreviewState,
    idx: usize,
    value: DistributorSource,
) {
    if let Some(listing) = state.row.supply.get_mut(idx) {
        listing.distributor = distributor_source_to_string(value);
        state.dirty = true;
    }
}

/// Set listing `idx`'s distributor SKU.
pub(super) fn set_listing_sku(state: &mut ComponentPreviewState, idx: usize, value: String) {
    if let Some(listing) = state.row.supply.get_mut(idx) {
        listing.sku = value;
        state.dirty = true;
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
        state.dirty = true;
    }
}

/// Remove listing `idx` when it is in range.
pub(super) fn remove_listing(state: &mut ComponentPreviewState, idx: usize) {
    if idx < state.row.supply.len() {
        state.row.supply.remove(idx);
        state.dirty = true;
    }
}
