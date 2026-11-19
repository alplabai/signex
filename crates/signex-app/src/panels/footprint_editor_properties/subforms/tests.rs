//! The role sub-forms (#126): which form a selection gets, and the rules
//! the forms follow (SMD pads show one side, BGA-only rows grey, the
//! pocket and mask switches send the inverse of the stored flag).

use std::path::PathBuf;

use signex_library::{Footprint, FootprintFile};
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{BoardCutoutAttr, DrillSpec, KeepoutAttr, KeepoutKinds, PadAttr};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::pcb_layer::{BoardSide, CopperLayer, InnerCopper, PcbLayer};
use signex_types::theme::{ThemeId, theme_tokens};

use super::pattern_layout::{Layout, layout};
use super::{cutout, keepout, pattern, sketch_pad, view_role_subform};
use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::panels::{
    ArrayKindSummary, CollapsedSections, FootprintEditorPanelContext, PanelMsg, PanelPalette,
};

const PATH: &str = "role-subforms.snxfpt";

/// A sketch with one plane and the entities `build` adds.
fn sketch(build: impl FnOnce(PlaneId, &mut SketchData)) -> SketchData {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    build(plane, &mut sketch);
    sketch
}

/// Two points and the line between them; returns the line.
fn line(plane: PlaneId, sketch: &mut SketchData) -> SketchEntityId {
    let a = SketchEntityId::new();
    let b = SketchEntityId::new();
    sketch
        .entities
        .push(Entity::new(a, plane, EntityKind::Point { x: 0.0, y: 0.0 }));
    sketch
        .entities
        .push(Entity::new(b, plane, EntityKind::Point { x: 2.0, y: 0.0 }));
    let id = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        id,
        plane,
        EntityKind::Line { start: a, end: b },
    ));
    id
}

/// A point carrying a pad; `drill` makes it a through-hole pad.
fn pad_point(plane: PlaneId, sketch: &mut SketchData, drill: Option<DrillSpec>) -> SketchEntityId {
    let id = SketchEntityId::new();
    let mut point = Entity::new(id, plane, EntityKind::Point { x: 0.0, y: 0.0 });
    point.pad = Some(PadAttr {
        number: "3".into(),
        drill,
        ..PadAttr::default()
    });
    sketch.entities.push(point);
    id
}

/// The panel context of an editor on `sketch` with `selected` selected.
fn ctx_with(sketch: SketchData, selected: SketchEntityId) -> FootprintEditorPanelContext {
    let path = PathBuf::from(PATH);
    let mut footprint = Footprint::empty("roles");
    footprint.sketch = Some(sketch);
    let editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(footprint));
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "roles".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path,
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchSelect {
            id: Some(selected),
            shift: false,
        }),
    }));
    app.document_state
        .panel_ctx
        .footprint_editor
        .clone()
        .expect("the footprint editor panel context is built")
}

fn has_form(ctx: &FootprintEditorPanelContext, id: SketchEntityId) -> bool {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let palette = PanelPalette::from_tokens(&tokens);
    let collapsed = CollapsedSections::default();
    let i18n = signex_i18n::I18n::en_us();
    view_role_subform(ctx, id, palette, &tokens, &collapsed, &i18n).is_some()
}

#[test]
fn a_plain_line_has_no_role_subform() {
    let mut id = None;
    let sketch = sketch(|plane, s| id = Some(line(plane, s)));
    let id = id.unwrap();
    let ctx = ctx_with(sketch, id);
    assert!(ctx.selected_keepout.is_none() && ctx.selected_sketch_pad.is_none());
    assert!(!has_form(&ctx, id));
}

#[test]
fn a_keepout_line_gets_the_keepout_form_with_its_layer() {
    let mut id = None;
    let sketch = sketch(|plane, s| {
        let line = line(plane, s);
        s.entities
            .iter_mut()
            .find(|e| e.id == line)
            .unwrap()
            .keepout = Some(KeepoutAttr {
            layer: PcbLayer::Copper(CopperLayer::Bottom),
            kinds: KeepoutKinds::NO_ROUTING,
        });
        id = Some(line);
    });
    let id = id.unwrap();
    let ctx = ctx_with(sketch, id);
    let keepout = ctx.selected_keepout.as_ref().expect("a keepout summary");
    assert_eq!(keepout.layer, PcbLayer::Copper(CopperLayer::Bottom));
    assert!(has_form(&ctx, id));
}

#[test]
fn the_layer_dropdown_offers_top_and_bottom_and_keeps_another_stored_layer() {
    let top = PcbLayer::Copper(CopperLayer::Top);
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);
    assert_eq!(keepout::layer_options(top), vec![top, bottom]);
    let inner = PcbLayer::Copper(CopperLayer::Inner(InnerCopper::new(2).unwrap()));
    assert_eq!(keepout::layer_options(inner), vec![top, bottom, inner]);
}

#[test]
fn the_pocket_switch_sends_the_inverse_of_through() {
    let id = SketchEntityId::new();
    let PanelMsg::FpEditorSetCutoutThrough { value, .. } = cutout::pocket_toggled(id, true) else {
        panic!("the pocket switch edits the through flag");
    };
    assert!(!value, "a pocket is not through the board");
    let PanelMsg::FpEditorSetCutoutThrough { value, .. } = cutout::pocket_toggled(id, false) else {
        panic!("the pocket switch edits the through flag");
    };
    assert!(value, "no pocket means through the board");
}

#[test]
fn a_cutout_line_gets_the_cutout_form() {
    let mut id = None;
    let sketch = sketch(|plane, s| {
        let line = line(plane, s);
        s.entities
            .iter_mut()
            .find(|e| e.id == line)
            .unwrap()
            .board_cutout = Some(BoardCutoutAttr {
            edge_radius_expr: None,
            through: true,
        });
        id = Some(line);
    });
    let id = id.unwrap();
    let ctx = ctx_with(sketch, id);
    assert!(ctx.selected_cutout.as_ref().is_some_and(|c| c.through));
    assert!(has_form(&ctx, id));
}

#[test]
fn the_mask_switch_is_ticked_while_the_mask_opens() {
    let id = SketchEntityId::new();
    let PanelMsg::FpEditorToggleSketchPadMaskTentedTop { value, .. } =
        sketch_pad::mask_toggled(id, BoardSide::Top, false)
    else {
        panic!("the top mask switch edits the top tented flag");
    };
    assert!(value, "unticking the mask tents the pad");
    let PanelMsg::FpEditorToggleSketchPadMaskTentedBottom { value, .. } =
        sketch_pad::mask_toggled(id, BoardSide::Bottom, true)
    else {
        panic!("the bottom mask switch edits the bottom tented flag");
    };
    assert!(!value, "ticking the mask opens it");
}

#[test]
fn an_smd_sketch_pad_hides_the_bottom_side_and_the_hole() {
    let mut id = None;
    let sketch = sketch(|plane, s| id = Some(pad_point(plane, s, None)));
    let id = id.unwrap();
    let ctx = ctx_with(sketch, id);
    let pad = ctx.selected_sketch_pad.as_ref().expect("a sketch pad");
    assert_eq!(pad.number, "3");
    assert!(!sketch_pad::shows_bottom_side(pad));
    assert!(has_form(&ctx, id));
}

#[test]
fn a_drilled_sketch_pad_shows_the_bottom_side_and_the_hole() {
    let mut id = None;
    let sketch = sketch(|plane, s| {
        let drill = DrillSpec {
            diameter_expr: "1mm".into(),
            slot_length_expr: None,
            plated: true,
        };
        id = Some(pad_point(plane, s, Some(drill)));
    });
    let ctx = ctx_with(sketch, id.unwrap());
    let pad = ctx.selected_sketch_pad.as_ref().expect("a sketch pad");
    assert!(sketch_pad::shows_bottom_side(pad));
}

/// A 2 × 2 grid pattern on a pad point, numbered by `numbering`.
fn grid_ctx(numbering: NumberingScheme) -> FootprintEditorPanelContext {
    let mut id = None;
    let sketch = sketch(|plane, s| {
        let source = pad_point(plane, s, None);
        s.arrays.push(Array {
            id: ArrayId::new(),
            kind: ArrayKind::Grid {
                source,
                nx_expr: "2".into(),
                ny_expr: "2".into(),
                dx_expr: "1.27mm".into(),
                dy_expr: "2mm".into(),
                depopulation: None,
            },
            numbering,
        });
        id = Some(source);
    });
    ctx_with(sketch, id.unwrap())
}

#[test]
fn bga_rows_grey_unless_the_bga_scheme_is_active() {
    let ctx = grid_ctx(NumberingScheme::default());
    let pattern = ctx.selected_array.as_ref().expect("a pattern");
    assert!(!pattern::bga_rows_enabled(pattern));
    let ctx = grid_ctx(NumberingScheme::BgaRowCol {
        skip_letters: true,
        start_row: 'A',
        start_col: 1,
    });
    let pattern = ctx.selected_array.as_ref().expect("a pattern");
    assert!(pattern::bga_rows_enabled(pattern));
}

#[test]
fn the_kind_segment_shows_the_kind_and_offers_no_other() {
    let ctx = grid_ctx(NumberingScheme::default());
    let pattern = ctx.selected_array.as_ref().expect("a pattern");
    let i18n = signex_i18n::I18n::en_us();
    let segments = pattern::kind_segments(pattern, &i18n);
    let active: Vec<bool> = segments.iter().map(|s| s.active).collect();
    assert_eq!(active, vec![false, true, false]);
    assert!(segments.iter().all(|s| s.msg.is_none()));
}

#[test]
fn a_grid_pattern_numbers_its_instances_like_the_bake() {
    let ctx = grid_ctx(NumberingScheme::default());
    let pattern = ctx.selected_array.as_ref().expect("a pattern");
    let Layout::Instances(all) = layout(pattern, &ctx.sketch_parameters) else {
        panic!("a literal grid resolves");
    };
    let labels: Vec<&str> = all.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["1", "2", "3", "4"]);
    let last = all.last().unwrap();
    assert!((last.offset_mm[0] - 1.27).abs() < 1e-9);
    assert!((last.offset_mm[1] - 2.0).abs() < 1e-9);
    assert!(ctx.selected_array.as_ref().unwrap().source_number == "3");
}

#[test]
fn a_bga_grid_names_rows_by_letter() {
    let ctx = grid_ctx(NumberingScheme::BgaRowCol {
        skip_letters: true,
        start_row: 'H',
        start_col: 1,
    });
    let pattern = ctx.selected_array.as_ref().expect("a pattern");
    let Layout::Instances(all) = layout(pattern, &ctx.sketch_parameters) else {
        panic!("a literal grid resolves");
    };
    let labels: Vec<&str> = all.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["H1", "H2", "J1", "J2"], "I is skipped");
}

#[test]
fn a_masked_or_unticked_instance_is_skipped() {
    let ctx = grid_ctx(NumberingScheme::default());
    let mut pattern = ctx.selected_array.clone().expect("a pattern");
    let ArrayKindSummary::Grid {
        mask_expr,
        suppressed_instances,
        ..
    } = &mut pattern.kind
    else {
        panic!("a grid pattern");
    };
    *mask_expr = "i != 0 || j != 0".into();
    suppressed_instances.push((1, 1));
    let shown = layout(&pattern, &ctx.sketch_parameters);
    let Layout::Instances(all) = &shown else {
        panic!("a literal grid resolves");
    };
    let skipped: Vec<bool> = all.iter().map(|c| c.is_skipped).collect();
    assert_eq!(skipped, vec![true, false, false, true]);
    assert_eq!(shown.kept(), Some(2));
}

#[test]
fn a_count_that_does_not_evaluate_draws_the_source_only() {
    let ctx = grid_ctx(NumberingScheme::default());
    let mut pattern = ctx.selected_array.clone().expect("a pattern");
    let ArrayKindSummary::Grid { nx_expr, .. } = &mut pattern.kind else {
        panic!("a grid pattern");
    };
    *nx_expr = "missing_parameter".into();
    assert_eq!(layout(&pattern, &ctx.sketch_parameters), Layout::Unresolved);
    assert_eq!(Layout::Unresolved.kept(), None);
}
