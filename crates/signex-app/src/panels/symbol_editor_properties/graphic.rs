//! Graphic properties: the header, the illustration, Geometry (the shape's
//! own coordinates; a rectangle adds Width / Height, written as its End
//! corner) and Style (stroke, and the fill colour of a closed shape).

use iced::{Color, Element};
use signex_i18n::{MessageId, UserText, ids, t};

use super::super::{
    ColorFieldProps, GraphicFieldId, GraphicKindSummary, GraphicSummary, NumericFieldKey, PanelMsg,
    SymbolEditorPanelContext, color_field,
};
use super::illustration::{self, field};
use super::object_context;
use super::rows::Form;
use crate::library::editor::symbol::rect_extent;

const GEOMETRY: &str = "symprops.geometry";
const STYLE: &str = "symprops.style";

pub(super) fn view<'a>(
    sym: &'a SymbolEditorPanelContext,
    g: &'a GraphicSummary,
    form: &Form<'_>,
    border_c: Color,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let mut col = vec![form.header(
        t!(i18n, kind_title(&g.kind)).into(),
        None,
        Some((object_context(sym, sym.active_part, i18n).into(), false)),
    )];
    if let Some(ill) = illustration::graphic(g, &sym.part_drawing.shapes, i18n) {
        col.push(illustration::view(ill, form));
    }
    col.push(form.section(
        GEOMETRY,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_GEOMETRY),
        true,
        None,
        || geometry_rows(g, form),
    ));
    col.push(form.section(
        STYLE,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_STYLE),
        true,
        None,
        || style_rows(sym, g, form, border_c),
    ));
    col
}

fn kind_title(kind: &GraphicKindSummary) -> MessageId {
    match kind {
        GraphicKindSummary::Rectangle { .. } => ids::EDITORS_SYMPROPS_RECTANGLE,
        GraphicKindSummary::Line { .. } => ids::EDITORS_SYMPROPS_LINE,
        GraphicKindSummary::Circle { .. } => ids::EDITORS_SYMPROPS_CIRCLE,
        GraphicKindSummary::Arc { .. } => ids::EDITORS_SYMPROPS_ARC,
        GraphicKindSummary::Text { .. } => ids::EDITORS_SYMPROPS_TEXT,
        GraphicKindSummary::Polygon { .. } => ids::EDITORS_SYMPROPS_POLYGON,
    }
}

fn geometry_rows<'a>(g: &GraphicSummary, form: &Form<'_>) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let idx = g.idx;
    let num = |field: GraphicFieldId, value: f64| {
        form.number(NumericFieldKey::Graphic { idx, field }, value, None)
    };
    let pair = |label: MessageId,
                (a, av): (GraphicFieldId, f64),
                (b, bv): (GraphicFieldId, f64),
                id: &'static str| {
        form.field(t!(i18n, label), form.pair(num(a, av), num(b, bv)), Some(id))
    };
    let single = |label: MessageId, (f, v): (GraphicFieldId, f64), id: &'static str| {
        form.field(t!(i18n, label), num(f, v), Some(id))
    };
    use GraphicFieldId as F;
    match &g.kind {
        GraphicKindSummary::Rectangle { from, to } => vec![
            pair(
                ids::EDITORS_SYMPROPS_START_X_Y,
                (F::FromX, from[0]),
                (F::FromY, from[1]),
                field::G_START,
            ),
            pair(
                ids::EDITORS_SYMPROPS_END_X_Y,
                (F::ToX, to[0]),
                (F::ToY, to[1]),
                field::G_END,
            ),
            pair(
                ids::EDITORS_SYMPROPS_WIDTH_HEIGHT,
                (F::Width, rect_extent(from[0], to[0])),
                (F::Height, rect_extent(from[1], to[1])),
                field::G_SIZE,
            ),
        ],
        GraphicKindSummary::Line { from, to } => vec![
            pair(
                ids::EDITORS_SYMPROPS_START_X_Y,
                (F::FromX, from[0]),
                (F::FromY, from[1]),
                field::G_START,
            ),
            pair(
                ids::EDITORS_SYMPROPS_END_X_Y,
                (F::ToX, to[0]),
                (F::ToY, to[1]),
                field::G_END,
            ),
        ],
        GraphicKindSummary::Circle { center, radius } => vec![
            pair(
                ids::EDITORS_SYMPROPS_CENTER_X_Y,
                (F::CenterX, center[0]),
                (F::CenterY, center[1]),
                field::G_CENTER,
            ),
            single(
                ids::EDITORS_SYMPROPS_RADIUS,
                (F::Radius, *radius),
                field::G_RADIUS,
            ),
        ],
        GraphicKindSummary::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => vec![
            pair(
                ids::EDITORS_SYMPROPS_CENTER_X_Y,
                (F::CenterX, center[0]),
                (F::CenterY, center[1]),
                field::G_CENTER,
            ),
            single(
                ids::EDITORS_SYMPROPS_RADIUS,
                (F::Radius, *radius),
                field::G_RADIUS,
            ),
            single(
                ids::EDITORS_SYMPROPS_START_ANGLE,
                (F::StartDeg, *start_deg),
                field::G_START_ANGLE,
            ),
            single(
                ids::EDITORS_SYMPROPS_END_ANGLE,
                (F::EndDeg, *end_deg),
                field::G_END_ANGLE,
            ),
        ],
        GraphicKindSummary::Text {
            position,
            content,
            size,
        } => vec![
            pair(
                ids::EDITORS_SYMPROPS_X_Y,
                (F::PositionX, position[0]),
                (F::PositionY, position[1]),
                field::G_POSITION,
            ),
            single(
                ids::EDITORS_SYMPROPS_SIZE,
                (F::TextSize, *size),
                field::G_TEXT_SIZE,
            ),
            form.field(
                t!(i18n, ids::EDITORS_SYMPROPS_CONTENT),
                form.input(
                    t!(i18n, ids::EDITORS_SYMPROPS_CONTENT_PLACEHOLDER),
                    content.clone(),
                    false,
                    move |value| PanelMsg::SymEditorSetGraphicText { idx, value },
                ),
                Some(field::G_CONTENT),
            ),
        ],
        // Read-only: the canvas's vertex handles edit a polygon.
        GraphicKindSummary::Polygon { vertex_count } => vec![form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_VERTICES),
            form.read_only(UserText::new(i18n.count(*vertex_count as i64)).into(), true),
            None,
        )],
    }
}

fn style_rows<'a>(
    sym: &SymbolEditorPanelContext,
    g: &GraphicSummary,
    form: &Form<'_>,
    border_c: Color,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let mut rows = vec![form.field(
        t!(i18n, ids::EDITORS_SYMPROPS_STROKE),
        form.number(
            NumericFieldKey::Graphic {
                idx: g.idx,
                field: GraphicFieldId::StrokeWidth,
            },
            g.stroke_width,
            None,
        ),
        Some(field::G_STROKE),
    )];
    let closed = matches!(
        g.kind,
        GraphicKindSummary::Rectangle { .. }
            | GraphicKindSummary::Circle { .. }
            | GraphicKindSummary::Polygon { .. }
    );
    if closed {
        rows.push(form.linked(
            fill_field(sym, g, form, border_c),
            Some(field::G_FILL),
            [0.0, 0.0],
        ));
    }
    rows
}

/// The fill colour of a closed shape on the shared colour field: a swatch,
/// or "None" for an outline. The palette opens while the editor's fill
/// picker targets this graphic.
fn fill_field<'a>(
    sym: &SymbolEditorPanelContext,
    g: &GraphicSummary,
    form: &Form<'_>,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    use std::rc::Rc;
    let i18n = form.i18n;
    let idx = g.idx;
    let this = sym.graphic_fill_picker.filter(|p| p.idx == idx);
    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> =
        Rc::new(move |rgba| PanelMsg::SymEditorSetGraphicFill { idx, color: rgba });
    color_field(
        ColorFieldProps {
            label: t!(i18n, ids::EDITORS_SYMPROPS_FILL).into(),
            current: g.fill,
            none_label: t!(i18n, ids::EDITORS_SYMPROPS_FILL_NONE).into(),
            show_palette: this.is_some(),
            show_advanced: this.is_some_and(|p| p.advanced),
            muted: form.look.ink3,
            border_c,
            on_toggle: PanelMsg::SymEditorToggleGraphicFillPicker { idx },
            on_advanced: PanelMsg::SymEditorOpenGraphicFillAdvanced { idx },
            on_cancel: PanelMsg::SymEditorCancelGraphicFillPicker,
            on_pick,
            on_clear: g
                .fill
                .is_some()
                .then_some(PanelMsg::SymEditorClearGraphicFill { idx }),
        },
        i18n,
    )
}
