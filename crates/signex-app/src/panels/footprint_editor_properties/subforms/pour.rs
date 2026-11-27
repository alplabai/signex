//! Pour role sub-form (mockup row 126): header, the pour illustration (region, fill
//! style, net, priority) and the Pour properties section, with Corners and
//! Corner size, the board pour's rows, in both sketch hosts (#2598).

use iced::{Element, Point};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_sketch::attr::PourFillType;
use signex_sketch::id::SketchEntityId;
use signex_types::theme::ThemeTokens;
use signex_types::zone_pour_shape::PourCorners;
use signex_widgets::property_illustration::{
    Anchor, FieldId, Illustration, Paint, Part, Primitive, Style, TextRole, Tint,
};

use super::super::super::{
    CollapsedSections, FootprintEditorPanelContext, FpSubformMsg, PanelMsg, PourSummary,
    pour_corner_size_buffer_key,
};
use super::super::pad::{Shown, number_input, with_refusals};
use super::drawing::{canvas, centred, corner_note, hatch, label};
use super::helpers::{
    Segment, caption, example_input, field_row, form, header, illustration, input, is_open,
    section_header, segmented, value_box,
};
use crate::pcb_canvas::layer_style::layer_name;
use crate::styles::board_palette::BoardPalette;

pub(super) const NET: FieldId = FieldId::new("pour.net");
pub(super) const PRIORITY: FieldId = FieldId::new("pour.priority");
pub(super) const FILL: FieldId = FieldId::new("pour.fill");

const SECTION: &str = "fp_role_pour";

/// The catalog name of a fill type.
pub(super) fn fill_label(i18n: &I18n, fill: PourFillType) -> Localized {
    match fill {
        PourFillType::Solid => t!(i18n, ids::EDITORS_ROLE_POUR_FILL_SOLID),
        PourFillType::Hatched => t!(i18n, ids::EDITORS_ROLE_POUR_FILL_HATCHED),
        PourFillType::Outline => t!(i18n, ids::EDITORS_ROLE_POUR_FILL_OUTLINE),
    }
}

pub(super) fn view<'a>(
    fp: &'a FootprintEditorPanelContext,
    pour: &'a PourSummary,
    id: SketchEntityId,
    tokens: &'a ThemeTokens,
    collapsed: &'a CollapsedSections,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let layer = layer_name(i18n, pour.layer);
    let region_caption = t!(
        i18n,
        ids::EDITORS_ROLE_REGION_CAPTION,
        "layer" => layer.clone().into_string()
    );
    let mut parts = vec![
        header(
            t!(i18n, ids::EDITORS_ROLE_POUR_TITLE),
            None,
            caption(region_caption, tokens),
            tokens,
        ),
        illustration(drawing(pour, layer, i18n), fp, tokens),
    ];
    let open = is_open(SECTION, collapsed, true);
    parts.push(section_header(
        SECTION,
        t!(i18n, ids::EDITORS_ROLE_POUR_SECTION),
        open,
        tokens,
        i18n,
    ));
    if !open {
        return form(parts);
    }
    parts.push(field_row(
        t!(i18n, ids::EDITORS_PAD_NET),
        input(
            t!(i18n, ids::EDITORS_ROLE_POUR_NO_NET),
            pour.net.clone().unwrap_or_default(),
            move |value| PanelMsg::FpEditorSetPourNet { id, value },
        ),
        tokens,
    ));
    parts.push(field_row(
        t!(i18n, ids::EDITORS_ROLE_POUR_PRIORITY),
        example_input("0", pour.priority.to_string(), move |value| {
            PanelMsg::FpEditorSetPourPriority { id, value }
        }),
        tokens,
    ));
    let fills = PourFillType::ALL
        .iter()
        .map(|&fill| Segment {
            label: fill_label(i18n, fill),
            active: fill == pour.fill_type,
            msg: Some(PanelMsg::FpEditorSetPourFillType { id, value: fill }),
        })
        .collect();
    parts.push(field_row(
        t!(i18n, ids::EDITORS_ROLE_POUR_FILL),
        segmented(fills),
        tokens,
    ));
    parts.extend(corner_rows(fp, pour, id, tokens, i18n));
    form(parts)
}

/// Corners (Sharp, Rounded, Chamfer) and Corner size, as the board pour's
/// panel shows them: a typed size from 0 while Rounded or Chamfer, greyed
/// at 0 while Sharp.
fn corner_rows<'a>(
    fp: &'a FootprintEditorPanelContext,
    pour: &'a PourSummary,
    id: SketchEntityId,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> [Element<'a, PanelMsg>; 2] {
    let cells = [
        (PourCorners::Sharp, ids::EDITORS_ROLE_POUR_CORNER_SHARP),
        (PourCorners::Rounded, ids::EDITORS_ROLE_POUR_CORNER_ROUNDED),
        (PourCorners::Chamfer, ids::EDITORS_ROLE_POUR_CORNER_CHAMFER),
    ]
    .into_iter()
    .map(|(corners, label)| Segment {
        label: t!(i18n, label),
        active: corners == pour.corners,
        msg: Some(PanelMsg::FpSubform(FpSubformMsg::PourCornersPicked { id, corners })),
    })
    .collect();
    let shapes = field_row(
        t!(i18n, ids::EDITORS_ROLE_POUR_CORNERS),
        segmented(cells),
        tokens,
    );
    let label = t!(i18n, ids::EDITORS_ROLE_POUR_CORNER_SIZE);
    if pour.corners == PourCorners::Sharp {
        // raw: a number, the same in every language.
        let zero = value_box(signex_widgets::raw("0"));
        return [shapes, field_row(label, zero, tokens)];
    }
    let buffer = fp.numeric_buffers.get(&pour_corner_size_buffer_key(id));
    let shown = Shown::non_negative(buffer, pour.corner_size.to_string());
    let on_input =
        move |value| PanelMsg::FpSubform(FpSubformMsg::PourCornerSizeEdited { id, value });
    let size = field_row(label, number_input(&shown, on_input, tokens), tokens);
    [shapes, with_refusals(size, &[shown.refusal], tokens, i18n)]
}

/// The pour region in its layer colour with its fill style, the net in the
/// middle and the fill and priority under it.
pub(super) fn drawing(pour: &PourSummary, layer: Localized, i18n: &I18n) -> Illustration {
    let colour = Paint::Document(BoardPalette::new().layer(pour.layer));
    let center = Point::new(110.0, 72.0);
    let region = centred(center, 128.0, 83.0);
    let body = match pour.fill_type {
        PourFillType::Solid => Style::edge().with_fill(colour, Tint::Body),
        PourFillType::Hatched | PourFillType::Outline => Style::edge(),
    };
    let mut drawing =
        canvas().part(Part::new(Primitive::Rect { rect: region }, body).linked_all([FILL, NET]));
    if pour.fill_type == PourFillType::Hatched {
        drawing = drawing.parts(
            hatch(region, 6.0, colour)
                .into_iter()
                .map(|part| part.linked(FILL)),
        );
    }
    let net = match pour.net.as_deref().filter(|n| !n.trim().is_empty()) {
        Some(net) => label(center, UserText::new(net), Anchor::Middle, TextRole::Mono),
        None => label(
            center,
            t!(i18n, ids::EDITORS_ROLE_POUR_NO_NET_SHORT),
            Anchor::Middle,
            TextRole::Caption,
        ),
    };
    let below = region.y + region.height + 14.0;
    let fill_callout = t!(
        i18n,
        ids::EDITORS_ROLE_POUR_FILL_CALLOUT,
        "fill" => fill_label(i18n, pour.fill_type).into_string()
    );
    let priority_callout = t!(
        i18n,
        ids::EDITORS_ROLE_POUR_PRIORITY_CALLOUT,
        "priority" => pour.priority
    );
    drawing
        .part(net.linked(NET))
        .part(
            label(
                Point::new(region.x, below),
                fill_callout,
                Anchor::Start,
                TextRole::Note,
            )
            .linked(FILL),
        )
        .part(
            label(
                Point::new(region.x + region.width, below),
                priority_callout,
                Anchor::End,
                TextRole::Note,
            )
            .linked(PRIORITY),
        )
        .part(corner_note(layer))
}
