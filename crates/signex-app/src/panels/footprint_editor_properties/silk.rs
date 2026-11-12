//! Silk graphic geometry rows of the footprint Properties panel.

use iced::widget::{Column, container};
use iced::{Color, Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_widgets::text;

use super::super::{PanelMsg, SilkKindGeometry};
use super::pad::pad_input_row;
use crate::styles::{Spacing, TypeScale};

const SP: Spacing = Spacing::DEFAULT;
const TS: TypeScale = TypeScale::DEFAULT;

/// The editable rows for one silk kind's own geometry.
pub(super) fn silk_geometry_rows<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    kind: &'a SilkKindGeometry,
    muted: Color,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    match kind {
        SilkKindGeometry::Line { from_mm, to_mm } => {
            col = col.push(pad_input_row(
                t!(i18n, ids::EDITORS_SILK_FROM_X),
                "0",
                format!("{:.3}", from_mm[0]),
                PanelMsg::FpEditorSetSilkLineFromX,
                muted,
            ));
            col = col.push(pad_input_row(
                t!(i18n, ids::EDITORS_SILK_FROM_Y),
                "0",
                format!("{:.3}", from_mm[1]),
                PanelMsg::FpEditorSetSilkLineFromY,
                muted,
            ));
            col = col.push(pad_input_row(
                t!(i18n, ids::EDITORS_SILK_TO_X),
                "0",
                format!("{:.3}", to_mm[0]),
                PanelMsg::FpEditorSetSilkLineToX,
                muted,
            ));
            col = col.push(pad_input_row(
                t!(i18n, ids::EDITORS_SILK_TO_Y),
                "0",
                format!("{:.3}", to_mm[1]),
                PanelMsg::FpEditorSetSilkLineToY,
                muted,
            ));
        }
        SilkKindGeometry::Text {
            position_mm,
            content,
            size_mm,
        } => {
            col = col.push(pad_input_row(
                t!(i18n, ids::EDITORS_SILK_CONTENT),
                "TEXT",
                content.clone(),
                PanelMsg::FpEditorSetSilkText,
                muted,
            ));
            col = col.push(pad_input_row(
                t!(i18n, ids::EDITORS_SILK_POSITION_X),
                "0",
                format!("{:.3}", position_mm[0]),
                PanelMsg::FpEditorSetSilkTextPositionX,
                muted,
            ));
            col = col.push(pad_input_row(
                t!(i18n, ids::EDITORS_SILK_POSITION_Y),
                "0",
                format!("{:.3}", position_mm[1]),
                PanelMsg::FpEditorSetSilkTextPositionY,
                muted,
            ));
            col = col.push(pad_input_row(
                t!(i18n, ids::EDITORS_SILK_SIZE),
                "1.0",
                format!("{:.3}", size_mm),
                PanelMsg::FpEditorSetSilkTextSize,
                muted,
            ));
        }
        SilkKindGeometry::Other => {
            col = col.push(
                container(
                    text(t!(i18n, ids::EDITORS_SILK_USE_SKETCH))
                        .size(TS.caption)
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
