//! The silk graphic form of the footprint Properties panel: its header,
//! kind and index, its own geometry, stroke width, fill and Delete. A
//! graphic baked from the sketch shows read-only (#1718).

use iced::widget::{Column, container};
use iced::{Color, Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};

use super::super::{FootprintSelectedSilkSummary, PanelMsg, SilkKindGeometry};
use super::body::{Ctx, caption, ghost, is_open, object_header, section};
use super::rows::{check_row, edit_row};
use crate::styles::Spacing;

const SP: Spacing = Spacing::DEFAULT;

const KEY: &str = "fp_silk_graphic";

/// The silk graphic header and its section.
pub(super) fn view_silk_selection<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    silk: &'a FootprintSelectedSilkSummary,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let kind = t!(i18n, ids::EDITORS_FP_PROPS_SILK_KIND, "kind" => silk.kind_label).into_string();
    col = col.push(object_header(
        t!(i18n, ids::EDITORS_FP_PROPS_SILK_HEADING, "kind" => kind.as_str()),
        Some(UserText::new(fp.footprint_name.as_str()).into()),
        tokens,
    ));
    let title = t!(i18n, ids::EDITORS_FP_PROPS_SECTION_SILK);
    col = col.push(section(KEY, &title, None, cx.collapsed, tokens, i18n));
    if !is_open(KEY, cx.collapsed) {
        return col;
    }
    let muted = cx.palette.muted;
    let input_bg = cx.palette.input_bg;
    let key = |id: signex_i18n::MessageId| t!(i18n, id).into_string();
    col = super::props_kv_row(col, muted, input_bg, &key(ids::EDITORS_FP_PROPS_KIND), kind);
    col = super::props_kv_row(
        col,
        muted,
        input_bg,
        &key(ids::EDITORS_FP_PROPS_INDEX),
        silk.idx.to_string(),
    );
    if silk.from_sketch {
        return col.push(caption(
            t!(i18n, ids::EDITORS_FOOTPRINT_SILK_FROM_SKETCH),
            tokens,
        ));
    }
    col = silk_geometry_rows(col, &silk.kind, muted, i18n);
    col = col.push(edit_row(
        &key(ids::EDITORS_SILK_STROKE_WIDTH),
        &format!("{:.3}", silk.stroke_width_mm),
        muted,
        PanelMsg::FpEditorSetSilkStrokeWidth,
    ));
    // Filled only shapes a closed graphic, but shows for every kind so it
    // can be flipped without hunting for the right tool.
    col = col.push(check_row(
        &key(ids::EDITORS_SILK_FILLED),
        silk.filled,
        PanelMsg::FpEditorToggleSilkFilled,
        muted,
    ));
    col.push(
        container(ghost(
            t!(i18n, ids::EDITORS_FP_PROPS_DELETE),
            Some(PanelMsg::FpEditorDeleteSelectedSilk),
            tokens,
        ))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed(),
    )
}

/// The editable rows for one silk kind's own geometry.
fn silk_geometry_rows<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    kind: &'a SilkKindGeometry,
    muted: Color,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    let row = |id: signex_i18n::MessageId, value: String, on_input: fn(String) -> PanelMsg| {
        edit_row(&t!(i18n, id).into_string(), &value, muted, on_input)
    };
    let mm = |v: f64| format!("{v:.3}");
    match kind {
        SilkKindGeometry::Line { from_mm, to_mm } => {
            col = col.push(row(
                ids::EDITORS_SILK_FROM_X,
                mm(from_mm[0]),
                PanelMsg::FpEditorSetSilkLineFromX,
            ));
            col = col.push(row(
                ids::EDITORS_SILK_FROM_Y,
                mm(from_mm[1]),
                PanelMsg::FpEditorSetSilkLineFromY,
            ));
            col = col.push(row(
                ids::EDITORS_SILK_TO_X,
                mm(to_mm[0]),
                PanelMsg::FpEditorSetSilkLineToX,
            ));
            col = col.push(row(
                ids::EDITORS_SILK_TO_Y,
                mm(to_mm[1]),
                PanelMsg::FpEditorSetSilkLineToY,
            ));
        }
        SilkKindGeometry::Text {
            position_mm,
            content,
            size_mm,
        } => {
            col = col.push(row(
                ids::EDITORS_SILK_CONTENT,
                content.clone(),
                PanelMsg::FpEditorSetSilkText,
            ));
            col = col.push(row(
                ids::EDITORS_SILK_POSITION_X,
                mm(position_mm[0]),
                PanelMsg::FpEditorSetSilkTextPositionX,
            ));
            col = col.push(row(
                ids::EDITORS_SILK_POSITION_Y,
                mm(position_mm[1]),
                PanelMsg::FpEditorSetSilkTextPositionY,
            ));
            col = col.push(row(
                ids::EDITORS_SILK_SIZE,
                mm(*size_mm),
                PanelMsg::FpEditorSetSilkTextSize,
            ));
        }
        SilkKindGeometry::Other => {
            col = col.push(
                container(
                    signex_widgets::text(t!(i18n, ids::EDITORS_SILK_USE_SKETCH))
                        .size(crate::styles::TypeScale::DEFAULT.caption)
                        .color(muted),
                )
                .padding([SP.s, SP.m])
                .width(Length::Fill)
                .boxed(),
            );
        }
    }
    col
}
