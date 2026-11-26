//! Keepout role sub-form (mockup row 126): header, the hatched keepout region naming
//! each blocked kind, the Layer dropdown and the kinds as a 2×3 grid.

use iced::widget::pick_list;
use iced::{Element, Length, Point, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_sketch::id::SketchEntityId;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::property_illustration::{
    Anchor, FieldId, Illustration, Paint, Part, Primitive, Stroke, Style, TextRole, Weight,
};

use super::super::super::{
    CollapsedSections, FootprintEditorPanelContext, FpSubformMsg, KeepoutKindFlag, KeepoutSummary,
    PanelMsg,
};
use super::drawing::{canvas, centred, corner_note, hatch, label};
use super::helpers::{
    SP, caption, check, check_grid, field_row, form, header, illustration, is_open, note,
    section_header,
};
use crate::pcb_canvas::layer_style::layer_name;
use crate::styles::board_palette::{BoardPalette, BoardSwatch};

pub(super) const LAYER: FieldId = FieldId::new("keepout.layer");

const SECTION: &str = "fp_role_keepout_kinds";

/// The kinds in the order of the grid, two to a row, with their field ids.
pub(super) const KINDS: [(KeepoutKindFlag, &str, MessageId); 6] = [
    (
        KeepoutKindFlag::NoCopper,
        "keepout.copper",
        ids::EDITORS_ROLE_KEEPOUT_NO_COPPER,
    ),
    (
        KeepoutKindFlag::NoRouting,
        "keepout.routing",
        ids::EDITORS_ROLE_KEEPOUT_NO_ROUTING,
    ),
    (
        KeepoutKindFlag::NoVias,
        "keepout.vias",
        ids::EDITORS_ROLE_KEEPOUT_NO_VIAS,
    ),
    (
        KeepoutKindFlag::NoPours,
        "keepout.pours",
        ids::EDITORS_ROLE_KEEPOUT_NO_POURS,
    ),
    (
        KeepoutKindFlag::NoComponents,
        "keepout.components",
        ids::EDITORS_ROLE_KEEPOUT_NO_COMPONENTS,
    ),
    (
        KeepoutKindFlag::NoDrilling,
        "keepout.drilling",
        ids::EDITORS_ROLE_KEEPOUT_NO_DRILLING,
    ),
];

/// Whether `keepout` blocks `kind`.
pub(super) fn blocks(keepout: &KeepoutSummary, kind: KeepoutKindFlag) -> bool {
    match kind {
        KeepoutKindFlag::NoRouting => keepout.no_routing,
        KeepoutKindFlag::NoComponents => keepout.no_components,
        KeepoutKindFlag::NoCopper => keepout.no_copper,
        KeepoutKindFlag::NoVias => keepout.no_vias,
        KeepoutKindFlag::NoDrilling => keepout.no_drilling,
        KeepoutKindFlag::NoPours => keepout.no_pours,
    }
}

/// The layers the Layer dropdown offers: top and bottom copper, plus the
/// keepout's own layer when it is another one (a file can name any board
/// layer), so the dropdown always shows the stored value.
pub(super) fn layer_options(current: PcbLayer) -> Vec<PcbLayer> {
    let mut options = vec![
        PcbLayer::Copper(CopperLayer::Top),
        PcbLayer::Copper(CopperLayer::Bottom),
    ];
    if !options.contains(&current) {
        options.push(current);
    }
    options
}

pub(super) fn view<'a>(
    fp: &'a FootprintEditorPanelContext,
    keepout: &'a KeepoutSummary,
    id: SketchEntityId,
    tokens: &'a ThemeTokens,
    collapsed: &'a CollapsedSections,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let layer = layer_name(i18n, keepout.layer);
    let region_caption = t!(
        i18n,
        ids::EDITORS_ROLE_REGION_CAPTION,
        "layer" => layer.clone().into_string()
    );
    let mut parts = vec![
        header(
            t!(i18n, ids::EDITORS_ROLE_KEEPOUT_TITLE),
            None,
            caption(region_caption, tokens),
            tokens,
        ),
        illustration(drawing(keepout, i18n), fp, tokens),
    ];
    let options = layer_options(keepout.layer);
    let labels: Vec<(PcbLayer, String)> = options
        .iter()
        .map(|&option| (option, layer_name(i18n, option).into_string()))
        .collect();
    let picker = pick_list(Some(keepout.layer), options, move |option: &PcbLayer| {
        labels
            .iter()
            .find(|(layer, _)| layer == option)
            .map(|(_, name)| name.clone())
            .unwrap_or_default()
    })
    .graphite()
    .on_select(move |layer| PanelMsg::FpSubform(FpSubformMsg::KeepoutLayerPicked { id, layer }))
    .padding([SP.xs, SP.m])
    .width(Length::Fill);
    parts.push(field_row(
        t!(i18n, ids::EDITORS_PAD_LAYER),
        picker.boxed(),
        tokens,
    ));
    let open = is_open(SECTION, collapsed, true);
    parts.push(section_header(
        SECTION,
        t!(i18n, ids::EDITORS_ROLE_KEEPOUT_KINDS),
        open,
        tokens,
        i18n,
    ));
    if open {
        let cells = KINDS
            .iter()
            .map(|&(kind, _, name)| {
                check(
                    t!(i18n, name),
                    blocks(keepout, kind),
                    move |value| PanelMsg::FpEditorSetKeepoutKind { id, kind, value },
                    tokens,
                )
            })
            .collect();
        parts.push(check_grid(cells));
        parts.push(note(
            t!(i18n, ids::EDITORS_ROLE_KEEPOUT_FROM_SKETCH),
            tokens,
        ));
    }
    form(parts)
}

/// The keepout region, dashed and hatched in the keep-out colour, naming
/// each kind it blocks; the layer in the corner.
pub(super) fn drawing(keepout: &KeepoutSummary, i18n: &I18n) -> Illustration {
    let colour = Paint::Document(BoardPalette::new().swatch(BoardSwatch::KeepOut));
    let center = Point::new(132.0, 72.0);
    let region = centred(center, 176.0, 104.0);
    let edge = Style {
        stroke: Some(Stroke {
            paint: colour,
            weight: Weight::Regular,
            is_dashed: true,
        }),
        fill: None,
    };
    let mut drawing = canvas()
        .parts(hatch(region, 7.0, colour))
        .part(Part::new(Primitive::Rect { rect: region }, edge));
    let blocked: Vec<_> = KINDS
        .iter()
        .filter(|(kind, _, _)| blocks(keepout, *kind))
        .collect();
    if blocked.is_empty() {
        drawing = drawing.part(label(
            center,
            t!(i18n, ids::EDITORS_ROLE_KEEPOUT_BLOCKS_NOTHING),
            Anchor::Middle,
            TextRole::Note,
        ));
    }
    let step = 13.0;
    let top = center.y - (blocked.len().saturating_sub(1) as f32) * step / 2.0;
    for (row, &&(_, field, name)) in blocked.iter().enumerate() {
        let at = Point::new(center.x, top + row as f32 * step + 3.0);
        drawing = drawing.part(
            label(at, t!(i18n, name), Anchor::Middle, TextRole::Caption)
                .linked(FieldId::new(field)),
        );
    }
    drawing.part(corner_note(layer_name(i18n, keepout.layer)).linked(LAYER))
}
