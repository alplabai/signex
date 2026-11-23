//! Cutout role sub-form (mockup row 126): header, the hole in the board with its edge
//! radius, the Edge radius field and the "Limit depth (pocket)" switch.

use iced::widget::Column;
use iced::{Element, Point, Rectangle, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_sketch::id::SketchEntityId;
use signex_types::theme::ThemeTokens;
use signex_widgets::property_illustration::{
    Anchor, FieldId, Illustration, Ink, Leader, Paint, Part, Primitive, Style, TextRole, Tint,
};

use super::super::super::{
    CollapsedSections, CutoutSummary, FootprintEditorPanelContext, PanelMsg,
};
use super::drawing::{canvas, centred, hatch, label};
use super::helpers::{
    caption, enable_section, field_row, form, header, illustration, input, is_open, section_header,
};

pub(super) const EDGE_RADIUS: FieldId = FieldId::new("cutout.edge_radius");
pub(super) const POCKET: FieldId = FieldId::new("cutout.pocket");

const SECTION: &str = "fp_role_cutout";

/// The message the pocket switch sends: a pocket is a cutout that is not
/// through the board.
pub(super) fn pocket_toggled(id: SketchEntityId, is_pocket: bool) -> PanelMsg {
    PanelMsg::FpEditorSetCutoutThrough {
        id,
        value: !is_pocket,
    }
}

/// The edge radius in mm when the expression is a plain length.
fn radius_mm(expr: &str) -> Option<f64> {
    super::expansion::literal_mm(expr).filter(|mm| *mm >= 0.0)
}

pub(super) fn view<'a>(
    fp: &'a FootprintEditorPanelContext,
    cutout: &'a CutoutSummary,
    id: SketchEntityId,
    tokens: &'a ThemeTokens,
    collapsed: &'a CollapsedSections,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let mut parts = vec![
        header(
            t!(i18n, ids::EDITORS_ROLE_CUTOUT_TITLE),
            None,
            caption(t!(i18n, ids::EDITORS_ROLE_CUTOUT_CAPTION), tokens),
            tokens,
        ),
        illustration(drawing(cutout, i18n), fp, tokens),
    ];
    let open = is_open(SECTION, collapsed, true);
    parts.push(section_header(
        SECTION,
        t!(i18n, ids::EDITORS_ROLE_CUTOUT_SECTION),
        open,
        tokens,
        i18n,
    ));
    if !open {
        return form(parts);
    }
    parts.push(field_row(
        t!(i18n, ids::EDITORS_ROLE_CUTOUT_EDGE_RADIUS),
        input(
            t!(i18n, ids::EDITORS_ROLE_CUTOUT_SHARP_PLACEHOLDER),
            cutout.edge_radius_expr.clone().unwrap_or_default(),
            move |value| PanelMsg::FpEditorSetCutoutEdgeRadius { id, value },
        ),
        tokens,
    ));
    let is_pocket = !cutout.through;
    parts.push(enable_section(
        t!(i18n, ids::EDITORS_ROLE_CUTOUT_POCKET),
        is_pocket,
        move |on| pocket_toggled(id, on),
        (!is_pocket).then(|| t!(i18n, ids::EDITORS_ROLE_CUTOUT_THROUGH)),
        Column::<Element<'a, PanelMsg>>::new().boxed(),
        tokens,
    ));
    form(parts)
}

/// The board with the cutout in it: knocked out when it goes through, a
/// hatched pocket otherwise; a leader names the edge radius.
pub(super) fn drawing(cutout: &CutoutSummary, i18n: &I18n) -> Illustration {
    const SCALE: f32 = 30.0;
    let board = Rectangle {
        x: 14.0,
        y: 26.0,
        width: 236.0,
        height: 106.0,
    };
    let center = Point::new(124.0, 78.0);
    let hole = centred(center, 108.0, 72.0);
    let expr = cutout
        .edge_radius_expr
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty());
    let radius = expr
        .and_then(radius_mm)
        .map_or(0.0, |mm| (mm as f32 * SCALE).min(hole.height / 2.0));
    let hole_style = if cutout.through {
        Style::edge().with_fill(Paint::Ink(Ink::Panel), Tint::Solid)
    } else {
        Style::edge()
    };
    let mut drawing = canvas()
        .part(Part::new(
            Primitive::RoundedRect {
                rect: board,
                radius: 3.0,
            },
            Style::edge().with_fill(Paint::Ink(Ink::Secondary), Tint::Wash),
        ))
        .part(label(
            Point::new(board.x + 6.0, board.y + 12.0),
            t!(i18n, ids::EDITORS_ROLE_CUTOUT_BOARD),
            Anchor::Start,
            TextRole::Note,
        ))
        .part(
            Part::new(Primitive::RoundedRect { rect: hole, radius }, hole_style)
                .linked_all([EDGE_RADIUS, POCKET]),
        );
    if !cutout.through {
        drawing = drawing.parts(
            hatch(hole, 5.0, Paint::Ink(Ink::Tertiary))
                .into_iter()
                .map(|part| part.linked(POCKET)),
        );
    }
    let inside = if cutout.through {
        t!(i18n, ids::EDITORS_ROLE_CUTOUT_THROUGH)
    } else {
        t!(i18n, ids::EDITORS_ROLE_CUTOUT_POCKET_SHORT)
    };
    let radius_text = match expr {
        Some(expr) => t!(
            i18n,
            ids::EDITORS_ROLE_CUTOUT_RADIUS_CALLOUT,
            "radius" => expr
        ),
        None => t!(i18n, ids::EDITORS_ROLE_CUTOUT_RADIUS_SHARP),
    };
    // The leader starts on the rounded corner, top right.
    let inset = radius * (1.0 - std::f32::consts::FRAC_1_SQRT_2);
    let corner = Point::new(hole.x + hole.width - inset, hole.y + inset);
    drawing
        .part(label(center, inside, Anchor::Middle, TextRole::Note).linked(POCKET))
        .part(
            Part::leader(Leader {
                at: corner,
                label_at: Point::new(260.0, 12.0),
                label: radius_text.into(),
                anchor: Anchor::End,
            })
            .linked(EDGE_RADIUS),
        )
}
