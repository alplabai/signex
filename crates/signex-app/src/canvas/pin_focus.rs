//! When a clicked pin stays focused: the one decision the runtime (what the
//! slot keeps) and the canvas (what it draws) share (#2086).

use signex_types::schematic::{SelectedItem, SelectedKind};
use uuid::Uuid;

/// Whether `symbol` is the whole selection: exactly one item, the part.
pub(crate) fn is_sole_selected_symbol(selected: &[SelectedItem], symbol: Uuid) -> bool {
    matches!(
        selected,
        [item] if item.kind == SelectedKind::Symbol && item.uuid == symbol
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_part_must_be_the_only_item_selected() {
        let part = Uuid::from_u128(1);
        let part_item = SelectedItem::new(part, SelectedKind::Symbol);
        let wire = SelectedItem::new(Uuid::from_u128(2), SelectedKind::Wire);
        assert!(is_sole_selected_symbol(&[part_item], part));
        assert!(!is_sole_selected_symbol(&[part_item, wire], part));
        assert!(!is_sole_selected_symbol(&[], part));
        assert!(!is_sole_selected_symbol(&[wire], part));
        assert!(!is_sole_selected_symbol(
            &[SelectedItem::new(part, SelectedKind::SymbolRefField)],
            part
        ));
        assert!(!is_sole_selected_symbol(&[part_item], Uuid::from_u128(3)));
    }
}
