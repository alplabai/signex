//! The Component Preview has no footprint sketch to mirror (mockup row 191):
//! it holds no footprint editor and builds no footprint canvas, and
//! `route_inline_edit` drops every `EditorMsg::Footprint`. So Mirror cannot
//! be armed there, and no click can reach Mirror unseen by the card. In a
//! footprint editor the card shows as soon as Mirror is armed.

use std::path::PathBuf;

use signex_library::{Footprint, FootprintFile, RowId};
use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::{MirrorCardMsg, MirrorField, MirrorObject, shown};
use crate::app::{FootprintDocument, Message, Signex};
use crate::library::component_preview::test_row;
use crate::library::component_preview::updates::apply_inline_edit;
use crate::library::editor::footprint::state::EditorMode;
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

/// A footprint editor in sketch mode over a centerline at x = 0 and a Line
/// from (2, 0) to (4, 1), with both selected; the ids of the centerline and
/// the Line.
fn sketch_editor() -> (FootprintDocument, SketchEntityId, SketchEntityId) {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let mut add = |kind| {
        let id = SketchEntityId::new();
        sketch.entities.push(Entity::new(id, plane, kind));
        id
    };
    let c0 = add(EntityKind::Point { x: 0.0, y: -5.0 });
    let c1 = add(EntityKind::Point { x: 0.0, y: 5.0 });
    let centerline = add(EntityKind::Line { start: c0, end: c1 });
    let a = add(EntityKind::Point { x: 2.0, y: 0.0 });
    let b = add(EntityKind::Point { x: 4.0, y: 1.0 });
    let line = add(EntityKind::Line { start: a, end: b });
    for e in &mut sketch.entities {
        e.centerline = e.id == centerline;
    }
    let mut fp = Footprint::empty("t");
    fp.sketch = Some(sketch);
    let mut editor = FootprintDocument::new(
        PathBuf::from("/tmp/mirror-card/t.snxpcb"),
        FootprintFile::from_footprint(fp),
    );
    editor.state.mode = EditorMode::Sketch;
    editor.state.selected_sketch = Some(line);
    editor.state.selected_sketch_secondary = Some(centerline);
    (editor, centerline, line)
}

/// Mockup row 191: arming Mirror shows the card at once, before any
/// message, with the selection as its objects, the selected centerline as
/// its mirror line and Keep linked on; any other tool shows no card.
#[test]
fn arming_mirror_shows_the_card_with_the_selection_and_the_centerline() {
    let (mut editor, centerline, line) = sketch_editor();
    editor.state.active_tool = SketchTool::Select;
    assert_eq!(shown(&editor, None), None, "no card while Select is armed");

    editor.state.active_tool = SketchTool::Mirror;
    let card = shown(&editor, None).expect("the card shows while Mirror is armed");
    assert_eq!(card.objects, vec![MirrorObject::Entity(line)]);
    assert_eq!(card.line, Some(centerline));
    assert!(card.keep_linked);
    assert_eq!(card.field, MirrorField::Objects);
    let sketch = editor.primitive().sketch.as_ref().expect("sketch");
    assert!(card.request(sketch).is_some(), "OK is ready");
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
