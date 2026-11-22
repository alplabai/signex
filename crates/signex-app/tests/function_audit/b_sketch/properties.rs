//! m124 (footprint Properties with nothing selected), m125 (a pad) and m126
//! (the role sub-forms).

use signex_app::library::editor::footprint::state::SnappingMode;
use signex_app::library::messages::FootprintEditorMsg;
use signex_app::panels::{FpSubformMsg, PanelMsg};
use signex_library::Footprint;
use signex_sketch::attr::{KeepoutAttr, KeepoutKinds, PadAttr};
use signex_sketch::id::SketchEntityId;
use signex_types::pcb_layer::{BoardSide, CopperLayer, PcbLayer};

use super::support::{FpRig, SketchBuilder, near, open_footprint, pads_rig};

fn keepout_rig() -> (FpRig, SketchEntityId) {
    let mut s = SketchBuilder::new();
    let (a, b) = (s.point(0.0, 0.0), s.point(2.0, 0.0));
    let edge = s.line(a, b);
    s.entity_mut(edge).keepout = Some(KeepoutAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        kinds: KeepoutKinds::NO_ROUTING,
    });
    let mut footprint = Footprint::empty("keepout");
    footprint.sketch = Some(s.data);
    (open_footprint(footprint), edge)
}

fn sketch_pad_rig() -> (FpRig, SketchEntityId) {
    let mut s = SketchBuilder::new();
    let pad = s.point(0.0, 0.0);
    s.entity_mut(pad).pad = Some(PadAttr {
        number: "1".into(),
        ..PadAttr::default()
    });
    let mut footprint = Footprint::empty("sketch-pad");
    footprint.sketch = Some(s.data);
    (open_footprint(footprint), pad)
}

fn mask_top(rig: &FpRig, id: SketchEntityId) -> Option<String> {
    rig.sketch()
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.pad.as_ref())
        .and_then(|pad| pad.stack.mask_top_expr.clone())
}

#[test]
fn m124_editing_the_name_changes_the_footprint_and_undo_restores_it() {
    let mut rig = open_footprint(Footprint::empty("before"));

    rig.panel(PanelMsg::FpEditorSetFootprintName("QFN-16".into()));
    assert_eq!(rig.editor().primitive().name, "QFN-16");
    assert!(rig.dirty());

    rig.undo();
    assert_eq!(rig.editor().primitive().name, "before");
    assert!(!rig.dirty(), "back at the save point");
}

#[test]
fn m124_editing_the_description_and_designator_writes_both() {
    let mut rig = open_footprint(Footprint::empty("fields"));

    rig.panel(PanelMsg::FpEditorSetFootprintDescription(
        "Quad flat, 16 pads".into(),
    ));
    rig.panel(PanelMsg::FpEditorSetFootprintDefaultDesignator("U".into()));

    let footprint = rig.editor().primitive();
    assert_eq!(footprint.description, "Quad flat, 16 pads");
    assert_eq!(footprint.default_designator, "U");
}

#[test]
fn m124_a_height_writes_the_footprint() {
    let mut rig = open_footprint(Footprint::empty("height"));

    rig.panel(PanelMsg::FpEditorSetFootprintHeight("0.9".into()));

    assert_eq!(rig.editor().primitive().height_mm, Some(0.9));
    assert!(rig.dirty());
}

/// The rule is the audit's "a bad value is refused"; no row or decision
/// names this field, so the expectation is an inference.
#[test]
fn m124_a_height_that_is_not_a_number_keeps_the_stored_height() {
    let mut rig = open_footprint(Footprint::empty("height-bad"));
    rig.panel(PanelMsg::FpEditorSetFootprintHeight("0.9".into()));

    rig.panel(PanelMsg::FpEditorSetFootprintHeight("abc".into()));

    assert_eq!(
        rig.editor().primitive().height_mm,
        Some(0.9),
        "text that is not a number must not erase the height"
    );
}

#[test]
fn m124_the_snap_grid_step_is_written_and_a_bad_one_is_refused() {
    let mut rig = open_footprint(Footprint::empty("snap"));

    rig.panel(PanelMsg::FpEditorSetSnapGridStep("0.25".into()));
    assert!(near(rig.editor().state.snap_options.grid_step_mm, 0.25));

    rig.panel(PanelMsg::FpEditorSetSnapGridStep("inf".into()));
    assert!(
        near(rig.editor().state.snap_options.grid_step_mm, 0.25),
        "the stored step is kept"
    );
}

#[test]
fn m124_the_snapping_switch_turns_snapping_off_and_is_not_a_document_edit() {
    let mut rig = open_footprint(Footprint::empty("snapping"));

    rig.send(FootprintEditorMsg::ActiveBarSetSnappingMode(
        SnappingMode::Off,
    ));

    assert_eq!(rig.editor().state.snapping_mode, SnappingMode::Off);
    assert!(!rig.dirty());
    assert_eq!(rig.steps(), 0);
}

#[test]
fn m125_a_pad_size_writes_the_pad_and_undo_restores_it() {
    let mut rig = pads_rig(&[(0.0, 0.0)]);
    rig.select_pads(vec![0]);
    let before = rig.editor().state.pads[0].size_mm;

    rig.panel(PanelMsg::FpEditorSetSelectedPadSizeX {
        idx: 0,
        value: "2.5".into(),
    });

    assert!(near(rig.editor().state.pads[0].size_mm.0, 2.5));
    assert!(near(rig.editor().primitive().pads[0].size[0], 2.5));
    assert!(rig.dirty());

    rig.undo();
    assert_eq!(rig.editor().state.pads[0].size_mm, before);
    assert!(!rig.dirty());
}

#[test]
fn m125_a_size_of_zero_or_text_is_refused_and_the_pad_keeps_its_size() {
    for bad in ["0", "-1", "abc"] {
        let mut rig = pads_rig(&[(0.0, 0.0)]);
        rig.select_pads(vec![0]);
        let before = rig.editor().state.pads[0].size_mm;

        rig.panel(PanelMsg::FpEditorSetSelectedPadSizeY {
            idx: 0,
            value: bad.into(),
        });

        assert_eq!(rig.editor().state.pads[0].size_mm, before, "typed {bad:?}");
        assert!(!rig.dirty(), "typed {bad:?}");
    }
}

#[test]
fn m125_the_pad_designator_is_written_to_the_footprint() {
    let mut rig = pads_rig(&[(0.0, 0.0)]);
    rig.select_pads(vec![0]);

    rig.panel(PanelMsg::FpEditorSetSelectedPadDesignator {
        idx: 0,
        value: "EP".into(),
    });

    assert_eq!(rig.editor().state.pads[0].number, "EP");
    assert_eq!(rig.editor().primitive().pads[0].number, "EP");
}

#[test]
fn m126_the_keepout_layer_is_written_and_undo_restores_it() {
    let (mut rig, id) = keepout_rig();
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);

    rig.panel(PanelMsg::FpSubform(FpSubformMsg::KeepoutLayerPicked {
        id,
        layer: bottom,
    }));

    let layer_of = |rig: &FpRig| {
        rig.sketch()
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.keepout.as_ref())
            .map(|k| k.layer)
    };
    assert_eq!(layer_of(&rig), Some(bottom));
    assert!(rig.dirty());
    rig.undo();
    assert_eq!(layer_of(&rig), Some(PcbLayer::Copper(CopperLayer::Top)));
    assert!(!rig.dirty());
}

#[test]
fn m126_picking_the_layer_a_keepout_already_has_is_no_edit() {
    let (mut rig, id) = keepout_rig();

    rig.panel(PanelMsg::FpSubform(FpSubformMsg::KeepoutLayerPicked {
        id,
        layer: PcbLayer::Copper(CopperLayer::Top),
    }));

    assert_eq!(rig.steps(), 0);
    assert!(!rig.dirty());
}

#[test]
fn m126_a_manual_mask_expansion_is_stored_and_rule_clears_it() {
    let (mut rig, id) = sketch_pad_rig();

    rig.panel(PanelMsg::FpSubform(
        FpSubformMsg::SketchPadMaskExpansionEdited {
            id,
            side: BoardSide::Top,
            value: "0.05".into(),
        },
    ));
    assert_eq!(mask_top(&rig, id).as_deref(), Some("0.05mm"));

    rig.panel(PanelMsg::FpSubform(
        FpSubformMsg::SketchPadMaskExpansionEdited {
            id,
            side: BoardSide::Top,
            value: String::new(),
        },
    ));
    assert_eq!(mask_top(&rig, id), None);
}

#[test]
fn m126_a_mask_expansion_that_is_not_a_length_is_refused() {
    let (mut rig, id) = sketch_pad_rig();

    rig.panel(PanelMsg::FpSubform(
        FpSubformMsg::SketchPadMaskExpansionEdited {
            id,
            side: BoardSide::Top,
            value: "abc".into(),
        },
    ));

    assert_eq!(mask_top(&rig, id), None, "the text is not written");
    assert_eq!(rig.steps(), 0);
    assert!(!rig.dirty());
}
