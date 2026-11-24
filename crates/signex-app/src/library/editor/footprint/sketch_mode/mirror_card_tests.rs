//! The Component Preview has no footprint sketch to mirror (mockup row 191):
//! it holds no footprint editor and builds no footprint canvas, and
//! `route_inline_edit` drops every `EditorMsg::Footprint`. So Mirror cannot
//! be armed there, and no click can reach Mirror unseen by the card.

use std::path::PathBuf;

use signex_library::RowId;

use super::MirrorCardMsg;
use crate::app::{Message, Signex};
use crate::library::component_preview::test_row;
use crate::library::component_preview::updates::apply_inline_edit;
use crate::library::editor::footprint::state::SketchTool;
use crate::library::messages::{EditorMsg, FootprintEditorMsg, LibraryMessage};
use crate::library::state::{ComponentPreviewState, EditorAddress};

fn mirror_messages() -> [FootprintEditorMsg; 3] {
    [
        FootprintEditorMsg::SketchSetTool(SketchTool::Mirror),
        FootprintEditorMsg::SketchToolClick {
            x_mm: 1.0,
            y_mm: 1.0,
            snap_id: None,
        },
        FootprintEditorMsg::SketchMirrorCard(MirrorCardMsg::Confirmed),
    ]
}

#[test]
fn the_component_preview_reducer_drops_footprint_messages() {
    let library_path = PathBuf::from("/tmp/mirror-preview/Lib.snxlib");
    let mut preview = ComponentPreviewState::from_row(library_path, "R".into(), test_row());

    for msg in mirror_messages() {
        apply_inline_edit(&mut preview, EditorMsg::Footprint(msg));
    }

    assert!(!preview.dirty, "nothing was edited");
    assert!(!preview.can_undo_step(), "no undo step");
}

#[test]
fn mirror_sent_to_a_component_preview_arms_no_card_and_opens_no_editor() {
    let (mut app, _task) = Signex::new();
    let row = test_row();
    let library_path = PathBuf::from("/tmp/mirror-preview/Lib.snxlib");
    let address = EditorAddress::new(
        library_path.clone(),
        "R".into(),
        RowId::from_uuid(row.row_id),
    );
    let preview = ComponentPreviewState::from_row(library_path, "R".into(), row);
    app.library.editors.insert(address.clone(), preview);

    for msg in mirror_messages() {
        drop(app.update(Message::Library(LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg: EditorMsg::Footprint(msg),
        })));
    }

    assert!(app.library.mirror_cards.is_empty());
    assert!(app.document_state.footprint_editors.is_empty());
    assert!(!app.library.editors[&address].dirty);
}
