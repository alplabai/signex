//! m192, Pads and Sketch: Edit in Sketch, Back to pad, Jump to pad and the
//! "This pad / All pads" question on a patterned pad (decision 9).

use std::collections::HashMap;

use signex_app::library::editor::footprint::{EditorMode, PadLinkMsg, PadLinkOffer, PadScope};
use signex_app::library::messages::FootprintEditorMsg;
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, Pad};
use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::solver::Solver;

use super::support::{FpRig, SketchBuilder, open_footprint};

/// Pad 6 driven by a point inside a rectangle, in Footprint mode.
fn linked_pad() -> (FpRig, SketchEntityId) {
    let mut s = SketchBuilder::new();
    let corners = [
        s.point(-1.0, -0.5),
        s.point(1.0, -0.5),
        s.point(1.0, 0.5),
        s.point(-1.0, 0.5),
    ];
    for (i, &corner) in corners.iter().enumerate() {
        s.line(corner, corners[(i + 1) % corners.len()]);
    }
    let centre = s.point(0.0, 0.0);
    s.entity_mut(centre).pad = Some(PadAttr {
        number: "6".into(),
        ..PadAttr::default()
    });
    let mut footprint = Footprint::empty("link");
    footprint.sketch = Some(s.data);
    footprint.pads.push(Pad {
        number: "6".into(),
        ..Pad::default()
    });
    let mut rig = open_footprint(footprint);
    let editor = rig.editor_mut();
    editor.state.mode = EditorMode::Normal;
    editor.state.pads[0].sketch_entity_id = Some(centre);
    (rig, centre)
}

/// A 4-pad row: source pad "1" and a linear pattern of it at 1 mm, baked.
fn patterned_row() -> FpRig {
    let plane = PlaneId::new();
    let source = SketchEntityId::new();
    let mut point = Entity::new(source, plane, EntityKind::Point { x: 0.0, y: 0.0 });
    point.pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        ..PadAttr::default()
    });
    let sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![point],
        arrays: vec![Array {
            id: ArrayId::new(),
            kind: ArrayKind::Linear {
                source,
                count_expr: "4".into(),
                dx_expr: "1mm".into(),
                dy_expr: "0mm".into(),
            },
            numbering: NumberingScheme::default(),
        }],
        ..SketchData::default()
    };
    let solved = Solver::default()
        .solve(&sketch, &HashMap::new())
        .expect("the row solves");
    let (mut pads, mut warnings) = (Vec::new(), Vec::new());
    signex_bake::bake_pads(&sketch, &solved, &HashMap::new(), &mut pads, &mut warnings)
        .expect("pads bake");
    signex_bake::bake_arrays(&sketch, &solved, &HashMap::new(), &mut pads, &mut warnings)
        .expect("the pattern bakes");
    assert_eq!(pads.len(), 4, "{warnings:?}");
    let mut footprint = Footprint::empty("row");
    footprint.pads = pads;
    footprint.sketch = Some(sketch);
    open_footprint(footprint)
}

fn edit_in_sketch(rig: &mut FpRig, pad_idx: usize) {
    rig.panel(PanelMsg::FpEditorEditPadInSketch { pad_idx });
}

fn set_net(rig: &mut FpRig, number: &str, net: &str) {
    let idx = rig
        .editor()
        .state
        .pads
        .iter()
        .position(|p| p.number == number)
        .expect("the pad");
    rig.panel(PanelMsg::FpEditorSetSelectedPadNet {
        idx,
        value: net.into(),
    });
}

/// `(number, net)` of every baked pad with a net.
fn nets(rig: &FpRig) -> Vec<(String, String)> {
    let mut out: Vec<_> = rig
        .editor()
        .primitive()
        .pads
        .iter()
        .filter(|p| !p.net.is_empty())
        .map(|p| (p.number.clone(), p.net.clone()))
        .collect();
    out.sort();
    out
}

#[test]
fn m192_edit_in_sketch_switches_mode_and_selects_the_driving_entity() {
    let (mut rig, centre) = linked_pad();

    edit_in_sketch(&mut rig, 0);

    let state = &rig.editor().state;
    assert_eq!(state.mode, EditorMode::Sketch);
    assert_eq!(state.selected_sketch, Some(centre));
    assert_eq!(
        state.pad_link_offer(),
        Some(PadLinkOffer::BackToPad {
            pad_idx: 0,
            entity: centre
        })
    );
    assert!(!rig.dirty(), "following the link is no edit");
}

#[test]
fn m192_back_to_pad_returns_to_the_pad_without_an_edit() {
    let (mut rig, centre) = linked_pad();
    edit_in_sketch(&mut rig, 0);

    rig.send(FootprintEditorMsg::PadLink(PadLinkMsg::PadLinkPressed {
        entity: centre,
    }));

    let state = &rig.editor().state;
    assert_eq!(state.mode, EditorMode::Normal);
    assert_eq!(state.selected_pad, Some(0));
    assert!(state.pad_link.is_none());
    assert!(!rig.dirty());
}

#[test]
fn m192_selecting_a_pads_entity_in_sketch_offers_jump_to_pad() {
    let (mut rig, centre) = linked_pad();
    rig.send(FootprintEditorMsg::SetMode(EditorMode::Sketch));

    rig.send(FootprintEditorMsg::SketchSelect {
        id: Some(centre),
        shift: false,
    });

    assert_eq!(
        rig.editor().state.pad_link_offer(),
        Some(PadLinkOffer::JumpToPad { entity: centre })
    );
}

#[test]
fn m192_a_pad_with_no_sketch_entity_offers_no_link() {
    let (mut rig, _) = linked_pad();
    rig.editor_mut().state.pads[0].sketch_entity_id = None;

    edit_in_sketch(&mut rig, 0);

    let state = &rig.editor().state;
    assert!(state.pad_link.is_none());
    assert_eq!(state.pad_link_offer(), None);
    assert!(!rig.dirty());
}

#[test]
fn m192_editing_a_patterned_pad_asks_and_writes_nothing_yet() {
    let mut rig = patterned_row();
    let steps = rig.steps();

    set_net(&mut rig, "3", "GND");

    assert!(
        rig.editor().state.pad_scope.is_some(),
        "the question is open"
    );
    assert!(nets(&rig).is_empty());
    assert_eq!(rig.steps(), steps);
}

#[test]
fn m192_this_pad_changes_only_that_pad_and_undo_restores_it() {
    let mut rig = patterned_row();
    set_net(&mut rig, "3", "GND");

    rig.send(FootprintEditorMsg::PadScopeAnswered(PadScope::ThisPad));

    assert_eq!(nets(&rig), vec![("3".to_string(), "GND".to_string())]);
    assert_eq!(rig.sketch().pad_overrides.len(), 1);
    rig.undo();
    assert!(nets(&rig).is_empty());
    assert!(rig.sketch().pad_overrides.is_empty());
}

#[test]
fn m192_all_pads_changes_every_instance() {
    let mut rig = patterned_row();
    set_net(&mut rig, "3", "GND");

    rig.send(FootprintEditorMsg::PadScopeAnswered(PadScope::AllPads));

    assert_eq!(nets(&rig).len(), 4, "{:?}", nets(&rig));
}

#[test]
fn m192_cancel_on_the_question_changes_nothing() {
    let mut rig = patterned_row();
    let steps = rig.steps();
    set_net(&mut rig, "3", "GND");

    rig.send(FootprintEditorMsg::PadScopeAnswered(PadScope::Cancel));

    assert!(rig.editor().state.pad_scope.is_none());
    assert!(nets(&rig).is_empty());
    assert_eq!(rig.steps(), steps);
    assert!(!rig.dirty());
}
