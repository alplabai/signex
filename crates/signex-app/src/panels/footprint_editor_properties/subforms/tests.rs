//! The role sub-forms (mockup row 126): which form a selection gets, and the rules
//! the forms follow (SMD pads show one side, BGA-only rows grey, the
//! pocket and mask switches send the inverse of the stored flag).

use std::path::PathBuf;

use signex_bake::array::ArrayPreview;
use signex_library::{Footprint, FootprintFile};
use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::attr::{
    BoardCutoutAttr, DrillSpec, KeepoutAttr, KeepoutKinds, PadAttr, PadSide,
};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_types::pcb_layer::{BoardSide, CopperLayer, InnerCopper, PcbLayer};
use signex_types::theme::{ThemeId, theme_tokens};

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
        .as_deref()
        .cloned()
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
    assert_eq!(sketch_pad::shown_sides(pad), &[BoardSide::Top]);
    assert!(has_form(&ctx, id));
}

#[test]
fn a_bottom_smd_sketch_pad_shows_only_its_bottom_paste_and_mask() {
    let mut id = None;
    let sketch = sketch(|plane, s| {
        let pad = pad_point(plane, s, None);
        let entity = s.entities.iter_mut().find(|e| e.id == pad).unwrap();
        entity.pad.as_mut().unwrap().side = PadSide::Bottom;
        id = Some(pad);
    });
    let id = id.unwrap();
    let ctx = ctx_with(sketch, id);
    let pad = ctx.selected_sketch_pad.as_ref().expect("a sketch pad");
    assert_eq!(pad.side, PadSide::Bottom);
    assert_eq!(
        sketch_pad::shown_sides(pad),
        &[BoardSide::Bottom],
        "the bake gives a bottom SMD pad the bottom paste and mask only"
    );
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
    assert_eq!(
        sketch_pad::shown_sides(pad),
        &[BoardSide::Top, BoardSide::Bottom]
    );
}

/// A 2 × 2 grid pattern on a pad point, numbered by `numbering`.
fn grid_ctx(numbering: NumberingScheme) -> FootprintEditorPanelContext {
    grid_ctx_with(numbering, "2", None)
}

/// A grid pattern of `nx_expr` × 2 on a pad point.
fn grid_ctx_with(
    numbering: NumberingScheme,
    nx_expr: &str,
    depopulation: Option<GridDepopulation>,
) -> FootprintEditorPanelContext {
    let mut id = None;
    let sketch = sketch(|plane, s| {
        let source = pad_point(plane, s, None);
        s.arrays.push(Array {
            id: ArrayId::new(),
            kind: ArrayKind::Grid {
                source,
                nx_expr: nx_expr.into(),
                ny_expr: "2".into(),
                dx_expr: "1.27mm".into(),
                dy_expr: "2mm".into(),
                depopulation,
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

/// Mockup row 126: a grid numbered 1, 2, 3 carries its sweep, and the
/// sweep reaches the numbers the drawing shows; BGA numbering has none.
#[test]
fn a_grid_shows_its_sweep_in_the_numbers() {
    use signex_sketch::array::GridSweep;
    let ctx = grid_ctx(NumberingScheme::LinearIncrement {
        start_expr: "1".into(),
        step_expr: "1".into(),
        sweep: GridSweep::DownThenUp,
    });
    let pattern = ctx.selected_array.as_ref().expect("a pattern");
    assert_eq!(pattern.sweep, Some(GridSweep::DownThenUp));
    let ArrayPreview::Instances(all) = &pattern.preview else {
        panic!("a literal grid resolves");
    };
    let labels: Vec<&str> = all.iter().map(|c| c.number.as_str()).collect();
    assert_eq!(labels, vec!["1", "4", "2", "3"], "cells walk row by row");
    let ctx = grid_ctx(NumberingScheme::BgaRowCol {
        skip_letters: true,
        start_row: 'A',
        start_col: 1,
    });
    assert_eq!(ctx.selected_array.as_ref().unwrap().sweep, None);
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
    let ArrayPreview::Instances(all) = &pattern.preview else {
        panic!("a literal grid resolves");
    };
    let labels: Vec<&str> = all.iter().map(|c| c.number.as_str()).collect();
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
    let ArrayPreview::Instances(all) = &pattern.preview else {
        panic!("a literal grid resolves");
    };
    let labels: Vec<&str> = all.iter().map(|c| c.number.as_str()).collect();
    assert_eq!(labels, vec!["H1", "H2", "J1", "J2"], "I is skipped");
}

#[test]
fn a_masked_or_unticked_instance_is_skipped() {
    let ctx = grid_ctx_with(
        NumberingScheme::default(),
        "2",
        Some(GridDepopulation {
            mask_expr: "i != 0 || j != 0".into(),
            suppressed_instances: vec![(1, 1)],
        }),
    );
    let shown = &ctx.selected_array.as_ref().expect("a pattern").preview;
    let ArrayPreview::Instances(all) = shown else {
        panic!("a literal grid resolves");
    };
    let skipped: Vec<bool> = all.iter().map(|c| c.is_skipped).collect();
    assert_eq!(skipped, vec![true, false, false, true]);
    assert_eq!(shown.kept(), Some(2));
}

/// Mockup row 126: the Numbering Mask shows the skipped pads' numbers, keeps
/// typed text that skips the same pads, and refuses a number the pattern
/// does not have.
#[test]
fn the_number_mask_shows_the_skipped_pads_and_refuses_unknown_numbers() {
    use signex_bake::array::NumberMaskRefusal;
    let ctx = grid_ctx_with(
        NumberingScheme::default(),
        "2",
        Some(GridDepopulation {
            mask_expr: String::new(),
            suppressed_instances: vec![(1, 1)],
        }),
    );
    let pattern = ctx.selected_array.as_ref().expect("a pattern");
    assert_eq!(
        pattern::shown_number_mask(pattern, None),
        ("4".into(), None)
    );
    let typed = |text: &str| pattern::shown_number_mask(pattern, Some(&text.to_owned()));
    assert_eq!(typed("4,"), ("4,".into(), None), "typing keeps its text");
    assert_eq!(
        typed("2"),
        ("4".into(), None),
        "text an undo left gives way"
    );
    assert_eq!(
        typed("4,9"),
        (
            "4,9".into(),
            Some(NumberMaskRefusal::NotInPattern("9".into()))
        )
    );
    assert_eq!(
        typed("4,1"),
        ("4,1".into(), Some(NumberMaskRefusal::IsSource("1".into()))),
        "the source pad is not skipped"
    );
}

#[test]
fn a_refused_number_mask_says_why_under_the_field() {
    let mut source = None;
    let sketch = sketch(|plane, s| {
        let id = pad_point(plane, s, None);
        s.arrays.push(Array {
            id: ArrayId::new(),
            kind: ArrayKind::Grid {
                source: id,
                nx_expr: "2".into(),
                ny_expr: "2".into(),
                dx_expr: "1.27mm".into(),
                dy_expr: "2mm".into(),
                depopulation: None,
            },
            numbering: NumberingScheme::default(),
        });
        source = Some(id);
    });
    let source = source.unwrap();
    let mut ctx = ctx_with(sketch, source);
    let array_id = ctx.selected_array.as_ref().expect("a pattern").array_id;
    ctx.numeric_buffers.insert(
        crate::panels::number_mask_buffer_key(array_id),
        "A1,B4".into(),
    );
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let palette = PanelPalette::from_tokens(&tokens);
    let collapsed = CollapsedSections::default();
    let i18n = signex_i18n::I18n::en_us();
    let form = view_role_subform(&ctx, source, palette, &tokens, &collapsed, &i18n)
        .expect("a pattern form");
    let texts: Vec<String> =
        crate::layout_probe::lay_out_texts(form, iced::Size::new(260.0, 4000.0))
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
    assert!(
        texts.iter().any(|t| t == "No pad A1 in this pattern"),
        "{texts:#?}"
    );
}

#[test]
fn a_count_that_does_not_evaluate_draws_the_source_only() {
    let ctx = grid_ctx_with(NumberingScheme::default(), "missing_parameter", None);
    let shown = &ctx.selected_array.as_ref().expect("a pattern").preview;
    assert_eq!(shown, &ArrayPreview::Unresolved);
    assert_eq!(shown.kept(), None);
}

/// A drilled sketch pad's copper offset or hole tolerance that was refused
/// shows the reason under its row, as the pad form does.
#[test]
fn a_refused_sketch_pad_offset_shows_its_reason() {
    use crate::library::editor::footprint::{pad_field, sketch_pad_field_buffer_key};
    let mut id = None;
    let sketch = sketch(|plane, s| {
        let drill = DrillSpec {
            diameter_expr: "1mm".into(),
            slot_length_expr: None,
            plated: true,
        };
        id = Some(pad_point(plane, s, Some(drill)));
    });
    let id = id.unwrap();
    let mut ctx = ctx_with(sketch, id);
    let reasons = |ctx: &FootprintEditorPanelContext| {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let palette = PanelPalette::from_tokens(&tokens);
        let collapsed = CollapsedSections::default();
        let i18n = signex_i18n::I18n::en_us();
        let form = view_role_subform(ctx, id, palette, &tokens, &collapsed, &i18n)
            .expect("a sketch pad form");
        crate::layout_probe::lay_out_texts(form, iced::Size::new(260.0, 4000.0))
            .into_iter()
            .filter(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "Enter a number")
            .count()
    };
    assert_eq!(reasons(&ctx), 0);

    let key = sketch_pad_field_buffer_key(id, pad_field::COPPER_OFFSET_X);
    ctx.numeric_buffers.insert(key, "-".into());
    let key = sketch_pad_field_buffer_key(id, pad_field::HOLE_TOLERANCE_MINUS);
    ctx.numeric_buffers.insert(key, "0,0".into());
    assert_eq!(reasons(&ctx), 2, "each refused field says why");
}

/// A 2 × 2 hole pattern spaced by `dx` and `dy` on a 60 × 40 board sketch's
/// size parameters.
fn spaced_grid_ctx(dx: &str, dy: &str) -> FootprintEditorPanelContext {
    let mut id = None;
    let sketch = sketch(|plane, s| {
        s.parameters.insert("board_w", "60mm");
        s.parameters.insert("board_h", "40mm");
        s.parameters.insert("hole_inset", "3.5mm");
        let source = pad_point(plane, s, None);
        s.arrays.push(Array {
            id: ArrayId::new(),
            kind: ArrayKind::Grid {
                source,
                nx_expr: "2".into(),
                ny_expr: "2".into(),
                dx_expr: dx.into(),
                dy_expr: dy.into(),
                depopulation: None,
            },
            numbering: NumberingScheme::default(),
        });
        id = Some(source);
    });
    ctx_with(sketch, id.unwrap())
}

fn spacing_note(ctx: &FootprintEditorPanelContext) -> Option<String> {
    let pattern = ctx.selected_array.as_ref().expect("a pattern");
    let ArrayKindSummary::Grid {
        dx_expr, dy_expr, ..
    } = &pattern.kind
    else {
        panic!("a grid: {:?}", pattern.kind);
    };
    let i18n = signex_i18n::I18n::en_us();
    pattern::spacing_note(pattern, [dx_expr.as_str(), dy_expr.as_str()], &i18n)
        .map(|note| note.into_string().replace(['\u{2068}', '\u{2069}'], ""))
}

/// Mockup row 179e: spacing written as expressions of the size parameters
/// stays an expression and shows the spacing the bake walks under it; a
/// plain spacing needs no line.
#[test]
fn a_spacing_bound_to_parameters_shows_the_values_it_resolves_to() {
    let bound = spaced_grid_ctx("board_w - 2*hole_inset", "board_h - 2*hole_inset");
    let pattern = bound.selected_array.as_ref().expect("a pattern");
    let ArrayKindSummary::Grid { dx_expr, .. } = &pattern.kind else {
        panic!("a grid");
    };
    assert_eq!(dx_expr, "board_w - 2*hole_inset", "the expression is kept");
    assert_eq!(
        spacing_note(&bound).as_deref(),
        Some("Spacing 53.00 × 33.00 mm")
    );
    assert_eq!(spacing_note(&spaced_grid_ctx("5mm", "5mm")), None);
}
