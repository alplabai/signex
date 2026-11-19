//! Sketch pad sub-form (#126): a pad whose copper is a sketch profile. The
//! header names the pad and jumps to it in Pads mode; Pad Attributes, the
//! top side's paste and mask as enable sections with Rule / Manual
//! expansions, the bottom side and hole only for a drilled pad, and the
//! pad features in a closed section.

use iced::widget::{Row, container};
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_sketch::attr::{ElectricalType, PadFeature, PadKind};
use signex_sketch::id::SketchEntityId;
use signex_types::pcb_layer::{BoardSide, CopperLayer, PcbLayer};
use signex_types::theme::ThemeTokens;
use signex_widgets::property_illustration::FieldId;
use signex_widgets::{button, text};

use super::super::super::{
    CollapsedSections, FootprintEditorPanelContext, FpSubformMsg, PanelMsg, PanelPalette,
    SketchPadAttrSummary, expansion_buffer_key,
};
use super::super::pad::rule_or_manual_row;
use super::expansion::literal_mm;
use super::helpers::{
    SP, TS, caption, check, check_grid, enable_section, example_input, field_row, form, header,
    illustration, input, is_open, pair, pick, section_header, value_box,
};
use super::sketch_pad_drawing::{MASK_BOTTOM, MASK_TOP, PASTE_BOTTOM, PASTE_TOP, drawing};
use crate::pcb_canvas::layer_style::layer_name;
use crate::styles::tokens::resolve;

const ATTRIBUTES: &str = "fp_role_sketch_pad_attributes";
const TOP_SIDE: &str = "fp_role_sketch_pad_top";
const BOTTOM_SIDE: &str = "fp_role_sketch_pad_bottom";
const HOLE: &str = "fp_role_sketch_pad_hole";
/// Closed by default: the key is in the collapsed set while it is open.
const FEATURES: &str = "fp_role_sketch_pad_features";

/// The catalog name of a sketch pad kind.
fn kind_message(kind: PadKind) -> MessageId {
    match kind {
        PadKind::Smd => ids::EDITORS_PAD_KIND_SMD,
        PadKind::Tht => ids::EDITORS_PAD_KIND_THROUGH_HOLE,
        PadKind::NptHole => ids::EDITORS_PAD_KIND_NPT_HOLE,
        PadKind::ConnectorPad => ids::EDITORS_PAD_KIND_CONNECTOR,
        PadKind::Fiducial => ids::EDITORS_PAD_KIND_FIDUCIAL,
        PadKind::Castellated => ids::EDITORS_PAD_KIND_CASTELLATED,
    }
}

/// The bottom side's paste and mask and the hole details apply to a
/// drilled pad only; a surface pad shows its own layer.
pub(super) fn shows_bottom_side(pad: &SketchPadAttrSummary) -> bool {
    pad.has_drill
}

/// The state word beside a solder mask header.
pub(super) fn mask_state(is_open: bool) -> MessageId {
    if is_open {
        ids::EDITORS_ROLE_SKETCH_PAD_OPENING
    } else {
        ids::EDITORS_ROLE_SKETCH_PAD_TENTED
    }
}

pub(super) fn view<'a>(
    fp: &'a FootprintEditorPanelContext,
    pad: &'a SketchPadAttrSummary,
    palette: PanelPalette,
    tokens: &'a ThemeTokens,
    collapsed: &'a CollapsedSections,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let mut parts = vec![
        header(
            t!(i18n, ids::EDITORS_ROLE_SKETCH_PAD_TITLE),
            None,
            header_caption(pad, tokens, i18n),
            tokens,
        ),
        illustration(drawing(pad, i18n), fp, tokens),
    ];
    let open = is_open(ATTRIBUTES, collapsed, true);
    parts.push(section_header(
        ATTRIBUTES,
        t!(i18n, ids::EDITORS_ROLE_SKETCH_PAD_ATTRIBUTES),
        open,
        tokens,
        i18n,
    ));
    if open {
        parts.extend(attribute_rows(pad, tokens, i18n));
    }
    let top = PcbLayer::Copper(CopperLayer::Top);
    let open = is_open(TOP_SIDE, collapsed, true);
    parts.push(section_header(
        TOP_SIDE,
        side_title(i18n, top),
        open,
        tokens,
        i18n,
    ));
    if open {
        parts.extend(side_rows(fp, pad, BoardSide::Top, palette, tokens, i18n));
        parts.push(field_row(
            t!(i18n, ids::EDITORS_PAD_CORNER_RADIUS),
            example_input(
                "25",
                pad.corner_radius_pct
                    .map(|v| format!("{v:.0}"))
                    .unwrap_or_default(),
                move |value| PanelMsg::FpEditorSetSketchPadCornerRadiusPct { id: pad.id, value },
            ),
            tokens,
        ));
        let id = pad.id;
        parts.push(field_row(
            UserText::new(String::new()),
            check(
                t!(i18n, ids::EDITORS_PAD_THERMAL_RELIEF),
                pad.thermal_relief,
                move |value| PanelMsg::FpEditorToggleSketchPadThermalRelief { id, value },
                tokens,
            ),
            tokens,
        ));
    }
    if shows_bottom_side(pad) {
        let bottom = PcbLayer::Copper(CopperLayer::Bottom);
        let open = is_open(BOTTOM_SIDE, collapsed, true);
        parts.push(section_header(
            BOTTOM_SIDE,
            side_title(i18n, bottom),
            open,
            tokens,
            i18n,
        ));
        if open {
            parts.extend(side_rows(fp, pad, BoardSide::Bottom, palette, tokens, i18n));
        }
        let open = is_open(HOLE, collapsed, true);
        parts.push(section_header(
            HOLE,
            t!(i18n, ids::EDITORS_PAD_HOLE_DETAILS),
            open,
            tokens,
            i18n,
        ));
        if open {
            parts.extend(hole_rows(pad, tokens, i18n));
        }
    }
    let open = is_open(FEATURES, collapsed, false);
    parts.push(section_header(
        FEATURES,
        t!(i18n, ids::EDITORS_ROLE_SKETCH_PAD_FEATURES),
        open,
        tokens,
        i18n,
    ));
    if open {
        parts.extend(feature_rows(pad, tokens, i18n));
    }
    form(parts)
}

fn side_title(i18n: &I18n, layer: PcbLayer) -> signex_i18n::Localized {
    t!(
        i18n,
        ids::EDITORS_ROLE_SKETCH_PAD_SIDE_SECTION,
        "layer" => layer_name(i18n, layer).into_string()
    )
}

/// "Pad 3 · SMD · from sketch profile ·" and the jump to the pad in Pads
/// mode.
fn header_caption<'a>(
    pad: &SketchPadAttrSummary,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let line = t!(
        i18n,
        ids::EDITORS_ROLE_SKETCH_PAD_CAPTION,
        "number" => pad.number.as_str(),
        "kind" => t!(i18n, kind_message(pad.kind)).into_string()
    );
    let accent = resolve(tokens).accent_text;
    let link = button(
        text(t!(i18n, ids::EDITORS_ROLE_SKETCH_PAD_EDIT_IN_PADS))
            .size(TS.caption)
            .color(accent),
    )
    .padding([0.0, SP.xs])
    .on_press(PanelMsg::FpEditorEditSketchPadInPads { id: pad.id })
    .style(|_: &Theme, _| button::Style::default());
    Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .push(caption(line, tokens))
        .push(link.boxed())
        .boxed()
}

fn attribute_rows<'a>(
    pad: &'a SketchPadAttrSummary,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, PanelMsg>> {
    let id = pad.id;
    vec![
        field_row(
            t!(i18n, ids::EDITORS_PAD_DESIGNATOR),
            pair(
                value_box(UserText::new(pad.number.as_str())),
                check(
                    t!(i18n, ids::EDITORS_PAD_LOCKED),
                    pad.locked,
                    move |value| PanelMsg::FpEditorToggleSketchPadLocked { id, value },
                    tokens,
                ),
            ),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_NET),
            input(
                t!(i18n, ids::EDITORS_ROLE_POUR_NO_NET),
                pad.net.clone(),
                move |value| PanelMsg::FpEditorSetSketchPadNet { id, value },
            ),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_TEMPLATE),
            input(
                UserText::new(String::new()),
                pad.template.clone(),
                move |value| PanelMsg::FpEditorSetSketchPadTemplate { id, value },
            ),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_TEMPLATE_LIBRARY),
            input(
                UserText::new(String::new()),
                pad.template_library.clone(),
                move |value| PanelMsg::FpEditorSetSketchPadTemplateLibrary { id, value },
            ),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_ELECTRICAL_TYPE),
            pick(ElectricalType::ALL, pad.electrical_type, move |value| {
                PanelMsg::FpEditorSetSketchPadElectricalType { id, value }
            }),
            tokens,
        ),
    ]
}

/// The paste and solder mask enable sections of one side, each with its
/// Rule / Manual expansion.
fn side_rows<'a>(
    fp: &'a FootprintEditorPanelContext,
    pad: &'a SketchPadAttrSummary,
    side: BoardSide,
    palette: PanelPalette,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, PanelMsg>> {
    let id = pad.id;
    let (paste_on, mask_tented, paste_expr, mask_expr) = match side {
        BoardSide::Top => (
            pad.paste_top_enabled,
            pad.mask_top_tented,
            &pad.paste_top_expr,
            &pad.mask_top_expr,
        ),
        BoardSide::Bottom => (
            pad.paste_bottom_enabled,
            pad.mask_bottom_tented,
            &pad.paste_bottom_expr,
            &pad.mask_bottom_expr,
        ),
    };
    let (paste_label, mask_label, paste_field, mask_field) = match side {
        BoardSide::Top => (
            ids::EDITORS_ROLE_SKETCH_PAD_TOP_PASTE,
            ids::EDITORS_ROLE_SKETCH_PAD_TOP_MASK,
            PASTE_TOP,
            MASK_TOP,
        ),
        BoardSide::Bottom => (
            ids::EDITORS_ROLE_SKETCH_PAD_BOTTOM_PASTE,
            ids::EDITORS_ROLE_SKETCH_PAD_BOTTOM_MASK,
            PASTE_BOTTOM,
            MASK_BOTTOM,
        ),
    };
    let paste_row = expansion_row(
        fp,
        id,
        paste_field,
        paste_expr,
        false,
        side,
        palette,
        tokens,
        i18n,
    );
    let mask_row = expansion_row(
        fp, id, mask_field, mask_expr, true, side, palette, tokens, i18n,
    );
    let mask_open = !mask_tented;
    vec![
        enable_section(
            t!(i18n, paste_label),
            paste_on,
            move |value| paste_toggled(id, side, value),
            (!paste_on).then(|| t!(i18n, ids::EDITORS_ROLE_SKETCH_PAD_OFF)),
            paste_row,
            tokens,
        ),
        enable_section(
            t!(i18n, mask_label),
            mask_open,
            move |open| mask_toggled(id, side, open),
            Some(t!(i18n, mask_state(mask_open))),
            mask_row,
            tokens,
        ),
    ]
}

fn paste_toggled(id: SketchEntityId, side: BoardSide, value: bool) -> PanelMsg {
    match side {
        BoardSide::Top => PanelMsg::FpEditorToggleSketchPadPasteEnabledTop { id, value },
        BoardSide::Bottom => PanelMsg::FpEditorToggleSketchPadPasteEnabledBottom { id, value },
    }
}

/// The mask header is ticked while the mask opens over the pad; unticked
/// tents it.
pub(super) fn mask_toggled(id: SketchEntityId, side: BoardSide, is_open: bool) -> PanelMsg {
    let value = !is_open;
    match side {
        BoardSide::Top => PanelMsg::FpEditorToggleSketchPadMaskTentedTop { id, value },
        BoardSide::Bottom => PanelMsg::FpEditorToggleSketchPadMaskTentedBottom { id, value },
    }
}

/// One expansion as a Rule / Manual row. An expression that names a
/// parameter is Manual and shows its text.
#[allow(
    clippy::too_many_arguments,
    reason = "the row needs the panel look, the hover and the catalog"
)]
fn expansion_row<'a>(
    fp: &'a FootprintEditorPanelContext,
    id: SketchEntityId,
    field: FieldId,
    expr: &Option<String>,
    is_mask: bool,
    side: BoardSide,
    palette: PanelPalette,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let typed = fp
        .numeric_buffers
        .get(&expansion_buffer_key(id, is_mask, side))
        .cloned();
    let (value_mm, buffer) = match expr.as_deref() {
        None => (None, typed),
        Some(e) => match literal_mm(e) {
            Some(mm) => (Some(mm), typed),
            None => (Some(0.0), typed.or_else(|| Some(e.to_owned()))),
        },
    };
    let on_input = move |value: String| {
        PanelMsg::FpSubform(if is_mask {
            FpSubformMsg::SketchPadMaskExpansionEdited { id, side, value }
        } else {
            FpSubformMsg::SketchPadPasteExpansionEdited { id, side, value }
        })
    };
    rule_or_manual_row(
        t!(i18n, ids::EDITORS_ROLE_SKETCH_PAD_EXPANSION),
        field,
        value_mm,
        buffer,
        on_input,
        palette,
        tokens,
        &fp.illustration_hover,
        i18n,
    )
}

/// The shipped hole rows of a drilled pad.
fn hole_rows<'a>(
    pad: &'a SketchPadAttrSummary,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, PanelMsg>> {
    let id = pad.id;
    let mm = |v: Option<f64>| v.map(|v| format!("{v:.3}")).unwrap_or_default();
    vec![
        field_row(
            t!(i18n, ids::EDITORS_PAD_HOLE_TOLERANCE_PLUS),
            example_input("0", mm(pad.hole_tolerance_plus_mm), move |value| {
                PanelMsg::FpEditorSetSketchPadHoleTolerancePlus { id, value }
            }),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_HOLE_TOLERANCE_MINUS),
            example_input("0", mm(pad.hole_tolerance_minus_mm), move |value| {
                PanelMsg::FpEditorSetSketchPadHoleToleranceMinus { id, value }
            }),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_HOLE_ROTATION),
            example_input(
                "0",
                pad.hole_rotation_deg
                    .map(|v| format!("{v:.1}"))
                    .unwrap_or_default(),
                move |value| PanelMsg::FpEditorSetSketchPadHoleRotation { id, value },
            ),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_COPPER_OFFSET_X),
            example_input("0", mm(pad.copper_offset_x_mm), move |value| {
                PanelMsg::FpEditorSetSketchPadCopperOffsetX { id, value }
            }),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_COPPER_OFFSET_Y),
            example_input("0", mm(pad.copper_offset_y_mm), move |value| {
                PanelMsg::FpEditorSetSketchPadCopperOffsetY { id, value }
            }),
            tokens,
        ),
    ]
}

/// The shipped pad features: the surface feature of each side and the
/// four testpoint flags.
fn feature_rows<'a>(
    pad: &'a SketchPadAttrSummary,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, PanelMsg>> {
    let id = pad.id;
    let tp = pad.testpoint;
    vec![
        field_row(
            t!(i18n, ids::EDITORS_PAD_TOP_SIDE),
            pick(PadFeature::ALL, pad.feature_top, move |value| {
                PanelMsg::FpEditorSetSketchPadFeatureTop { id, value }
            }),
            tokens,
        ),
        field_row(
            t!(i18n, ids::EDITORS_PAD_BOTTOM_SIDE),
            pick(PadFeature::ALL, pad.feature_bottom, move |value| {
                PanelMsg::FpEditorSetSketchPadFeatureBottom { id, value }
            }),
            tokens,
        ),
        container(caption(t!(i18n, ids::EDITORS_PAD_TESTPOINT), tokens))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
        check_grid(vec![
            check(
                t!(i18n, ids::EDITORS_PAD_TOP_ASSEMBLY),
                tp.top_assembly,
                move |value| PanelMsg::FpEditorToggleSketchPadTestpointTopAssembly { id, value },
                tokens,
            ),
            check(
                t!(i18n, ids::EDITORS_PAD_TOP_FAB),
                tp.top_fab,
                move |value| PanelMsg::FpEditorToggleSketchPadTestpointTopFab { id, value },
                tokens,
            ),
            check(
                t!(i18n, ids::EDITORS_PAD_BOTTOM_ASSEMBLY),
                tp.bottom_assembly,
                move |value| PanelMsg::FpEditorToggleSketchPadTestpointBottomAssembly { id, value },
                tokens,
            ),
            check(
                t!(i18n, ids::EDITORS_PAD_BOTTOM_FAB),
                tp.bottom_fab,
                move |value| PanelMsg::FpEditorToggleSketchPadTestpointBottomFab { id, value },
                tokens,
            ),
        ]),
    ]
}
