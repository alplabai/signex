//! Rebase an open Component Preview onto a row a primitive-save cascade
//! just rewrote (#1840).
//!
//! The cascade (`signex_library::cascade_after_saves`) changes only the
//! primitive pins, the row version and the update time. A preview with
//! unsaved edits keeps everything else it holds and takes those fields and
//! the stored base revision, so its own save is not refused as "changed by
//! someone else" for a change this session made. A pin whose primitive the
//! draft re-bound meanwhile is the user's and is kept.

use signex_library::ComponentRow;

/// Carry the cascade's fields from `stored` into the edited `draft`.
pub fn rebase_onto_cascade(draft: &mut ComponentRow, stored: &ComponentRow) {
    if draft.symbol_ref == stored.symbol_ref {
        draft.symbol_version.clone_from(&stored.symbol_version);
    }
    if draft.footprint_ref == stored.footprint_ref {
        draft
            .footprint_version
            .clone_from(&stored.footprint_version);
    }
    if draft.sim_ref == stored.sim_ref {
        draft.sim_version.clone_from(&stored.sim_version);
    }
    for alt in &mut draft.alternate_symbols {
        if let Some(on_disk) = stored
            .alternate_symbols
            .iter()
            .find(|s| s.symbol_ref == alt.symbol_ref)
        {
            alt.symbol_version.clone_from(&on_disk.symbol_version);
        }
    }
    for alt in &mut draft.alternate_footprints {
        if let Some(on_disk) = stored
            .alternate_footprints
            .iter()
            .find(|f| f.footprint == alt.footprint)
        {
            alt.version.clone_from(&on_disk.version);
        }
    }
    draft.version.clone_from(&stored.version);
    draft.updated = stored.updated;
    draft.server_revision = stored.server_revision;
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{AlternateSymbol, PrimitiveRef};
    use uuid::Uuid;

    fn row() -> ComponentRow {
        let mut row = crate::library::component_preview::test_row();
        row.symbol_ref = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        row
    }

    #[test]
    fn takes_the_cascade_fields_and_keeps_the_edit() {
        let mut draft = row();
        draft.description = "Mine".into();
        let mut stored = draft.clone();
        stored.description = "Old".into();
        stored.symbol_version = "0.0.3".into();
        stored.version = "0.0.3".into();
        stored.server_revision = Some(7);

        rebase_onto_cascade(&mut draft, &stored);

        assert_eq!(draft.description, "Mine");
        assert_eq!(draft.symbol_version, "0.0.3");
        assert_eq!(draft.version, "0.0.3");
        assert_eq!(draft.server_revision, Some(7));
    }

    #[test]
    fn keeps_a_pin_the_draft_re_bound() {
        let mut draft = row();
        draft.symbol_version = "9.9.9".into();
        let mut stored = row();
        stored.symbol_version = "0.0.3".into();

        rebase_onto_cascade(&mut draft, &stored);

        assert_eq!(draft.symbol_version, "9.9.9");
    }

    #[test]
    fn re_pins_an_alternate_symbol_the_stored_row_also_binds() {
        let mut draft = row();
        let target = PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7());
        draft
            .alternate_symbols
            .push(AlternateSymbol::new(target, "", "IEC"));
        let mut stored = draft.clone();
        stored.alternate_symbols[0].symbol_version = "0.0.4".into();

        rebase_onto_cascade(&mut draft, &stored);

        assert_eq!(draft.alternate_symbols[0].symbol_version, "0.0.4");
    }
}
