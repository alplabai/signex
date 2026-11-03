//! A row edit that changes what a placed part follows moves the row's
//! version (#1867), so the drift check can tell a moved row from a part the
//! user overrode.

use signex_library::ComponentRow;

/// Patch-bump `after.version` when `after` differs from the stored `before`
/// in footprint, Manufacturer or MPN. An edit that already set its own
/// version keeps it.
pub(super) fn bump_on_part_change(before: &ComponentRow, after: &mut ComponentRow) {
    let moved = before.footprint_ref != after.footprint_ref
        || before.primary_mpn.manufacturer != after.primary_mpn.manufacturer
        || before.primary_mpn.mpn != after.primary_mpn.mpn;
    if moved && after.version == before.version {
        after.version = signex_library::patch_bump(&after.version);
        after.updated = chrono::Utc::now();
    }
}
