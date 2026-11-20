//! Mockup row 126: the role sub-form edits with no flat `FpEditor*` message: the
//! keepout layer and a sketch pad's paste and mask expansions. Each writes
//! the sketch attribute, dirties the tab and is one undo step.

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::{FpSubformMsg, PanelMsg};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::attr::{KeepoutAttr, KeepoutKinds, PadAttr};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::pcb_layer::{BoardSide, CopperLayer, PcbLayer};
use std::path::PathBuf;

fn open_editor(stem: &str, footprint: Footprint) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(footprint));
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

fn subform(app: &mut Signex, msg: FpSubformMsg) {
    drop(app.update(Message::Dock(DockMessage::Panel(PanelMsg::FpSubform(msg)))));
}

fn editor<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintDocument {
    app.document_state.footprint_editors.get(path).unwrap()
}

fn entity<'a>(app: &'a Signex, path: &PathBuf, id: SketchEntityId) -> &'a Entity {
    editor(app, path)
        .primitive()
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .find(|e| e.id == id)
        .unwrap()
}

/// A footprint whose sketch holds one entity of `kind` built by `tag`.
fn footprint_with(kind: EntityKind, tag: impl FnOnce(&mut Entity)) -> (Footprint, SketchEntityId) {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let id = SketchEntityId::new();
    let mut entity = Entity::new(id, plane, kind);
    tag(&mut entity);
    sketch.entities.push(entity);
    let mut footprint = Footprint::empty("roles");
    footprint.sketch = Some(sketch);
    (footprint, id)
}

/// A keepout on a line between two points.
fn keepout_footprint() -> (Footprint, SketchEntityId) {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let (a, b, line) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    sketch
        .entities
        .push(Entity::new(a, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
    sketch
        .entities
        .push(Entity::new(b, plane, EntityKind::Point { x: 2.0, y: 0.0 }));
    let mut edge = Entity::new(line, plane, EntityKind::Line { start: a, end: b });
    edge.keepout = Some(KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::NO_ROUTING,
    });
    sketch.entities.push(edge);
    let mut footprint = Footprint::empty("keepout");
    footprint.sketch = Some(sketch);
    (footprint, line)
}

#[test]
fn the_keepout_layer_is_written_dirties_the_tab_and_undoes() {
    let (footprint, id) = keepout_footprint();
    let (mut app, path) = open_editor("keepout-layer", footprint);
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);

    subform(
        &mut app,
        FpSubformMsg::KeepoutLayerPicked { id, layer: bottom },
    );

    let keepout = entity(&app, &path, id).keepout.as_ref().unwrap();
    assert_eq!(keepout.layer, bottom);
    assert_eq!(keepout.kinds, KeepoutKinds::NO_ROUTING, "the kinds stay");
    assert!(editor(&app, &path).is_dirty());
    assert_eq!(editor(&app, &path).history.undo_len(), 1, "one undo step");

    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .undo();
    let keepout = entity(&app, &path, id).keepout.as_ref().unwrap();
    assert_eq!(keepout.layer, PcbLayer::Copper(CopperLayer::Top));
}

#[test]
fn picking_the_keepout_layer_it_has_records_no_step() {
    let (footprint, id) = keepout_footprint();
    let (mut app, path) = open_editor("keepout-same-layer", footprint);
    subform(
        &mut app,
        FpSubformMsg::KeepoutLayerPicked {
            id,
            layer: PcbLayer::Copper(CopperLayer::Top),
        },
    );
    assert_eq!(editor(&app, &path).history.undo_len(), 0);
}

fn pad_footprint() -> (Footprint, SketchEntityId) {
    footprint_with(EntityKind::Point { x: 0.0, y: 0.0 }, |point| {
        point.pad = Some(PadAttr {
            number: "1".into(),
            ..PadAttr::default()
        });
    })
}

fn stack<'a>(
    app: &'a Signex,
    path: &PathBuf,
    id: SketchEntityId,
) -> &'a signex_sketch::attr::PadStackOverrides {
    &entity(app, path, id).pad.as_ref().unwrap().stack
}

#[test]
fn a_manual_mask_expansion_is_stored_in_mm_and_rule_clears_it() {
    let (footprint, id) = pad_footprint();
    let (mut app, path) = open_editor("mask-expansion", footprint);

    subform(
        &mut app,
        FpSubformMsg::SketchPadMaskExpansionEdited {
            id,
            side: BoardSide::Top,
            value: "0.05".into(),
        },
    );
    assert_eq!(
        stack(&app, &path, id).mask_top_expr.as_deref(),
        Some("0.05mm")
    );
    assert_eq!(
        stack(&app, &path, id).mask_bottom_expr,
        None,
        "one side only"
    );
    assert!(editor(&app, &path).is_dirty());

    // Rule sends empty text: the design rule applies again.
    subform(
        &mut app,
        FpSubformMsg::SketchPadMaskExpansionEdited {
            id,
            side: BoardSide::Top,
            value: String::new(),
        },
    );
    assert_eq!(stack(&app, &path, id).mask_top_expr, None);
}

#[test]
fn a_paste_expression_replaces_the_paste_percentage() {
    let (mut footprint, id) = pad_footprint();
    let attr = footprint
        .sketch
        .as_mut()
        .unwrap()
        .entities
        .iter_mut()
        .find_map(|e| e.pad.as_mut())
        .unwrap();
    attr.stack.paste_bottom_pct = Some(10.0);
    let (mut app, path) = open_editor("paste-expansion", footprint);

    subform(
        &mut app,
        FpSubformMsg::SketchPadPasteExpansionEdited {
            id,
            side: BoardSide::Bottom,
            value: "-0.04".into(),
        },
    );
    let stack = stack(&app, &path, id);
    assert_eq!(stack.paste_bottom_expr.as_deref(), Some("-0.04mm"));
    assert_eq!(
        stack.paste_bottom_pct, None,
        "expression and percentage exclude"
    );
}

#[test]
fn half_typed_expansion_text_is_kept_without_touching_the_pad() {
    let (footprint, id) = pad_footprint();
    let (mut app, path) = open_editor("expansion-typing", footprint);

    subform(
        &mut app,
        FpSubformMsg::SketchPadPasteExpansionEdited {
            id,
            side: BoardSide::Top,
            value: "-".into(),
        },
    );
    assert_eq!(stack(&app, &path, id).paste_top_expr, None);
    assert_eq!(editor(&app, &path).history.undo_len(), 0);
    let key = signex_app::panels::expansion_buffer_key(id, false, BoardSide::Top);
    assert_eq!(
        editor(&app, &path)
            .state
            .numeric_buffers
            .get(&key)
            .map(String::as_str),
        Some("-")
    );

    // Finishing the number stores it and drops the typed text.
    subform(
        &mut app,
        FpSubformMsg::SketchPadPasteExpansionEdited {
            id,
            side: BoardSide::Top,
            value: "-0.03".into(),
        },
    );
    assert_eq!(
        stack(&app, &path, id).paste_top_expr.as_deref(),
        Some("-0.03mm")
    );
    assert!(!editor(&app, &path).state.numeric_buffers.contains_key(&key));
}

fn pad_attr<'a>(app: &'a Signex, path: &PathBuf, id: SketchEntityId) -> &'a PadAttr {
    entity(app, path, id).pad.as_ref().unwrap()
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    drop(app.update(Message::Dock(DockMessage::Panel(msg))));
}

/// A sketch pad's copper offset and hole tolerance refuse half-typed text
/// as the pad form does: the pad keeps its value and the text is kept
/// under the sketch pad's own key; empty text clears the value, and the
/// keystrokes of one field that are taken make one undo step.
#[test]
fn half_typed_offset_or_tolerance_on_a_sketch_pad_keeps_the_value() {
    use signex_app::library::editor::footprint::{pad_field, sketch_pad_field_buffer_key};
    let (mut footprint, id) = pad_footprint();
    let attr = footprint
        .sketch
        .as_mut()
        .unwrap()
        .entities
        .iter_mut()
        .find_map(|e| e.pad.as_mut())
        .unwrap();
    attr.copper_offset_x_mm = Some(0.1);
    attr.hole_tolerance_plus_mm = Some(0.05);
    let (mut app, path) = open_editor("sketch-pad-offset", footprint);
    let offset_x = |value: &str| PanelMsg::FpEditorSetSketchPadCopperOffsetX {
        id,
        value: value.into(),
    };
    let tolerance_plus = |value: &str| PanelMsg::FpEditorSetSketchPadHoleTolerancePlus {
        id,
        value: value.into(),
    };
    let offset_key = sketch_pad_field_buffer_key(id, pad_field::COPPER_OFFSET_X);
    let tolerance_key = sketch_pad_field_buffer_key(id, pad_field::HOLE_TOLERANCE_PLUS);
    let buffer =
        |app: &Signex, key: &str| editor(app, &path).state.numeric_buffers.get(key).cloned();

    panel(&mut app, offset_x("-"));
    panel(&mut app, tolerance_plus("0,0"));
    let pad = pad_attr(&app, &path, id);
    assert_eq!(pad.copper_offset_x_mm, Some(0.1), "the offset is kept");
    assert_eq!(
        pad.hole_tolerance_plus_mm,
        Some(0.05),
        "the tolerance is kept"
    );
    assert_eq!(buffer(&app, &offset_key).as_deref(), Some("-"));
    assert_eq!(buffer(&app, &tolerance_key).as_deref(), Some("0,0"));
    assert_eq!(editor(&app, &path).history.undo_len(), 0, "nothing written");

    // Empty text clears the tolerance and drops its typed text.
    panel(&mut app, tolerance_plus(""));
    assert_eq!(pad_attr(&app, &path, id).hole_tolerance_plus_mm, None);
    assert_eq!(buffer(&app, &tolerance_key), None);
    assert_eq!(editor(&app, &path).history.undo_len(), 1);

    // A number typed key by key is stored in one undo step.
    panel(&mut app, offset_x("-0"));
    panel(&mut app, offset_x("-0.2"));
    assert_eq!(pad_attr(&app, &path, id).copper_offset_x_mm, Some(-0.2));
    assert_eq!(buffer(&app, &offset_key), None);
    assert!(editor(&app, &path).is_dirty());
    assert_eq!(
        editor(&app, &path).history.undo_len(),
        2,
        "one step per field"
    );

    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .undo();
    let pad = pad_attr(&app, &path, id);
    assert_eq!(
        pad.copper_offset_x_mm,
        Some(0.1),
        "undo restores the offset"
    );
    assert_eq!(pad.hole_tolerance_plus_mm, None);
}
