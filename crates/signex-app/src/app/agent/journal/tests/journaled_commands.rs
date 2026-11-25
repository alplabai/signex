//! Which agent commands the edit journal records (#2253): those that change a
//! document, or what is on disk or open.

use super::super::record::changes_documents;
use crate::keymap::AppCommandId;

fn journaled(id: &str) -> bool {
    changes_documents(&AppCommandId::new(id).unwrap())
}

/// #2506: Repour All clears `unfilled` on the board's pours as an undo step,
/// so an agent's run of it is journaled like any other board edit.
#[test]
fn repour_all_is_journaled_as_a_board_edit() {
    assert!(journaled("repour_all"));
    assert!(journaled("reset_board_origin"));
    assert!(!journaled("read_document"));
}
