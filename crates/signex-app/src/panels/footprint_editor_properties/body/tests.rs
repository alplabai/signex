//! The footprint Properties body (#124): every section in one scroll with
//! nothing selected and the pad stack check above them, the snap fields
//! greyed while snapping is Off, the sketch solve statistics, and a role
//! sub-form above the Entity section.

use std::path::PathBuf;

use iced::Widget as _;

use super::{BodyState, body_state, view_body};
use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use crate::layout_probe::{LaidOutText, lay_out_texts, messages_on_click_at};
use crate::library::editor::footprint::SnappingMode;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::panels::{
    FootprintEditorPanelContext, FootprintModeKind, FootprintSketchEntitySummary,
    FootprintSolveSummary, KeepoutSummary, PanelMsg, PanelPalette, SnapOptionFlag,
};
use signex_library::{Footprint, FootprintFile, Pad};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

const PATH: &str = "body-tests.snxfpt";
const DOCK: iced::Size = iced::Size::new(300.0, 4000.0);

/// An app with a footprint editor tab open on eight pads, nothing selected.
fn app_with_eight_pads() -> Signex {
    let path = PathBuf::from(PATH);
    let mut footprint = Footprint::empty("SOIC-8");
    footprint.pads = (1..=8)
        .map(|n| Pad {
            number: n.to_string(),
            size: [1.0, 0.6],
            position: [f64::from(n), 0.0],
            ..Pad::default()
        })
        .collect();
    let file = FootprintFile::from_footprint(footprint);
    let editor = FootprintDocument::new(path.clone(), file);
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "SOIC-8".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    // The Properties context is the Model's mirror, rebuilt in `update`; the
    // tab was pushed directly, so build it as `update` would.
    app.refresh_panel_ctx();
    app
}

fn panel_ctx(app: &Signex) -> FootprintEditorPanelContext {
    app.document_state
        .panel_ctx
        .footprint_editor
        .clone()
        .expect("the footprint editor panel context is built")
}

/// The body of `fp` as one element, at the dock width.
fn body<'a>(app: &'a Signex, fp: &'a FootprintEditorPanelContext) -> iced::Element<'a, PanelMsg> {
    let ctx = &app.document_state.panel_ctx;
    let i18n = signex_i18n::I18n::en_us();
    view_body(
        fp,
        PanelPalette::from_tokens(&ctx.tokens),
        &ctx.tokens,
        &ctx.collapsed_sections,
        signex_types::coord::Unit::Mm,
        &i18n,
    )
    .boxed()
}

/// The laid-out texts of `fp`'s body, without Fluent's isolation marks.
fn texts(app: &Signex, fp: &FootprintEditorPanelContext) -> Vec<LaidOutText> {
    lay_out_texts(body(app, fp), DOCK)
        .into_iter()
        .map(|mut t| {
            t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
            t
        })
        .collect()
}

fn has(texts: &[LaidOutText], label: &str) -> bool {
    texts.iter().any(|t| t.text == label)
}

fn top_of(texts: &[LaidOutText], label: &str) -> f32 {
    texts
        .iter()
        .find(|t| t.text == label)
        .unwrap_or_else(|| panic!("{label:?} missing: {texts:#?}"))
        .bounds
        .y
}

#[test]
fn nothing_selected_is_the_one_scroll_body() {
    let app = app_with_eight_pads();
    let fp = panel_ctx(&app);
    assert!(matches!(body_state(&fp), BodyState::NothingSelected));
}

#[test]
fn nothing_selected_shows_every_section_in_one_scroll() {
    let app = app_with_eight_pads();
    let mut fp = panel_ctx(&app);
    fp.mode_kind = FootprintModeKind::Pads;
    fp.snapping_mode = SnappingMode::AllLayers;
    // Expansion sits under the Courtyard switch; switched off it is greyed
    // and inert, which hides its text from the layout probe.
    fp.auto_fit_courtyard = true;
    let texts = texts(&app, &fp);
    for label in [
        "Footprint",
        "8 pads",
        "Name",
        "Description",
        "Designator",
        "default",
        "Type",
        "Height (mm)",
        "Version",
        "Units",
        "COURTYARD & SILKSCREEN",
        "Expansion",
        "0.25 mm",
        "SNAP OPTIONS",
        "GRID MANAGER",
        "SKETCH SOLVE",
    ] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
    // No section switch, and the empty Settings header is dropped.
    for label in ["Snap & Grid", "Settings"] {
        assert!(!has(&texts, label), "{label:?} shown: {texts:#?}");
    }
    assert!(top_of(&texts, "8 pads") < top_of(&texts, "Name"));
    assert!(top_of(&texts, "Name") < top_of(&texts, "COURTYARD & SILKSCREEN"));
    assert!(top_of(&texts, "COURTYARD & SILKSCREEN") < top_of(&texts, "SNAP OPTIONS"));
    assert!(top_of(&texts, "SNAP OPTIONS") < top_of(&texts, "GRID MANAGER"));
    assert!(top_of(&texts, "GRID MANAGER") < top_of(&texts, "SKETCH SOLVE"));
}

#[test]
fn the_pad_stack_check_sits_above_every_section() {
    let app = app_with_eight_pads();
    let mut fp = panel_ctx(&app);
    fp.legacy_pad_stack_offer = Some(3);
    fp.paste_disabled_smd_pads = vec!["1".into(), "2".into()];
    let texts = texts(&app, &fp);
    let card = top_of(&texts, "Pad stack check");
    assert!(top_of(&texts, "8 pads") < card);
    assert!(card < top_of(&texts, "Name"), "{texts:#?}");
    assert!(has(&texts, "Keep as is"));
}

#[test]
fn nothing_selected_shows_the_snap_fields_and_the_grid_manager() {
    let app = app_with_eight_pads();
    let mut fp = panel_ctx(&app);
    fp.snapping_mode = SnappingMode::AllLayers;
    // Axis Snap Range is greyed and inert (hidden from the layout probe)
    // while snapping to axes is off.
    fp.snap_options.snap_to_axes = true;
    let texts = texts(&app, &fp);
    for label in [
        "SNAP OPTIONS",
        "All Layers",
        "Current Layer",
        "Off",
        "Grids",
        "Guides",
        "Axes",
        "Pad Centers",
        "Footprint Origins",
        "Snap Distance",
        "Axis Snap Range",
        "GRID MANAGER",
        "+ Add",
        "Step",
        "Display",
        "Properties…",
    ] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
}

#[test]
fn snapping_off_greys_the_snap_fields_and_they_take_no_clicks() {
    let app = app_with_eight_pads();
    let mut fp = panel_ctx(&app);
    fp.snapping_mode = SnappingMode::AllLayers;
    let grids = lay_out_texts(body(&app, &fp), DOCK)
        .into_iter()
        .find(|t| t.text == "Grids")
        .expect("the Grids target")
        .bounds
        .center();
    let on = messages_on_click_at(body(&app, &fp), DOCK, None, grids);
    assert!(
        on.iter().any(|m| matches!(
            m,
            PanelMsg::FpEditorToggleSnapOption(SnapOptionFlag::SnapToGrids)
        )),
        "with snapping on, Grids toggles: {on:?}"
    );

    fp.snapping_mode = SnappingMode::Off;
    let off = messages_on_click_at(body(&app, &fp), DOCK, None, grids);
    assert!(
        !off.iter()
            .any(|m| matches!(m, PanelMsg::FpEditorToggleSnapOption(_))),
        "with snapping Off, the targets take no clicks: {off:?}"
    );
    // The scope switch itself stays live.
    let texts = texts(&app, &fp);
    assert!(has(&texts, "All Layers"));
}

#[test]
fn the_sketch_sections_show_the_solve_statistics_and_parameters() {
    let app = app_with_eight_pads();
    let mut fp = panel_ctx(&app);
    fp.last_solve = Some(FootprintSolveSummary {
        iterations: 6,
        elapsed_ms: 2,
        final_residual_norm: 3.1e-11,
        over_constraint_count: 0,
        over_constraints: Vec::new(),
        redundant_constraints: Vec::new(),
    });
    fp.sketch_parameters = vec![("pitch".into(), "1.27".into())];
    let texts = texts(&app, &fp);
    for label in [
        "SKETCH SOLVE",
        "ITERATIONS",
        "6",
        "RESIDUAL",
        "3.100e-11",
        "ELAPSED",
        "2 ms",
        "PARAMETERS",
        "pitch",
        "SOLVE WARNINGS",
    ] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
}

/// A sketch line selected in Sketch mode, with no role.
fn ctx_with_line_selected(app: &Signex) -> FootprintEditorPanelContext {
    let mut fp = panel_ctx(app);
    fp.mode_kind = FootprintModeKind::Sketch;
    fp.selected_sketch_entity = Some(FootprintSketchEntitySummary {
        kind_label: "Line",
        position_mm: None,
        attached_constraint_count: 4,
        construction: false,
        dof_state: None,
    });
    fp.selected_sketch_entity_id = Some(signex_sketch::id::SketchEntityId::new());
    fp
}

#[test]
fn an_entity_without_a_role_form_has_its_own_header_then_the_entity_section() {
    let app = app_with_eight_pads();
    let fp = ctx_with_line_selected(&app);
    assert!(matches!(body_state(&fp), BodyState::SketchEntity(_)));
    let texts = texts(&app, &fp);
    assert_eq!(
        texts.first().map(|t| t.text.as_str()),
        Some("Sketch entity · Line"),
        "{texts:#?}"
    );
    assert!(has(&texts, "4 constraints attached"));
    assert!(top_of(&texts, "Sketch entity · Line") < top_of(&texts, "ENTITY"));
    assert!(top_of(&texts, "ENTITY") < top_of(&texts, "Role"));
    // Snap Options stays reachable under a selection.
    assert!(top_of(&texts, "Role") < top_of(&texts, "SNAP OPTIONS"));
}

#[test]
fn a_role_with_a_sub_form_shows_the_sub_form_first_then_the_entity_section_with_role() {
    let app = app_with_eight_pads();
    let mut fp = ctx_with_line_selected(&app);
    fp.selected_keepout = Some(KeepoutSummary {
        no_routing: true,
        no_components: false,
        no_copper: false,
        no_vias: false,
        no_drilling: false,
        no_pours: false,
        layer: PcbLayer::Copper(CopperLayer::Top),
    });
    let texts = texts(&app, &fp);
    assert!(
        !has(&texts, "Sketch entity · Line"),
        "the sub-form carries the header: {texts:#?}"
    );
    let entity = top_of(&texts, "ENTITY");
    assert!(
        texts.iter().any(|t| t.bounds.y < entity),
        "the sub-form sits above the Entity section: {texts:#?}"
    );
    assert!(entity < top_of(&texts, "Role"));
}

#[test]
fn a_selected_silk_graphic_is_the_silk_body() {
    let mut app = app_with_eight_pads();
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: PathBuf::from(PATH),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SelectPad(None)),
    }));
    let mut fp = panel_ctx(&app);
    fp.selected_silk_summary = Some(crate::panels::FootprintSelectedSilkSummary {
        idx: 0,
        kind_label: "Line",
        stroke_width_mm: 0.15,
        filled: false,
        from_sketch: false,
        kind: crate::panels::SilkKindGeometry::Line {
            from_mm: [0.0, 0.0],
            to_mm: [1.0, 0.0],
        },
    });
    assert!(matches!(body_state(&fp), BodyState::Silk(_)));
    let texts = texts(&app, &fp);
    for label in [
        "Silk graphic · Line",
        "Stroke width (mm)",
        "Filled",
        "Delete",
    ] {
        assert!(has(&texts, label), "{label:?} missing: {texts:#?}");
    }
}
