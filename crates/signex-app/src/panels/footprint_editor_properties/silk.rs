//! Silk graphic geometry rows of the footprint Properties panel.

use iced::widget::{Column, container, text};
use iced::{Color, Element, Length, Widget};

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
    primary: Color,
    border_c: Color,
) -> Column<Element<'a, PanelMsg>> {
    match kind {
        SilkKindGeometry::Line { from_mm, to_mm } => {
            col = col.push(pad_input_row(
                "From X (mm)",
                "0",
                format!("{:.3}", from_mm[0]),
                PanelMsg::FpEditorSetSilkLineFromX,
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "From Y (mm)",
                "0",
                format!("{:.3}", from_mm[1]),
                PanelMsg::FpEditorSetSilkLineFromY,
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "To X (mm)",
                "0",
                format!("{:.3}", to_mm[0]),
                PanelMsg::FpEditorSetSilkLineToX,
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "To Y (mm)",
                "0",
                format!("{:.3}", to_mm[1]),
                PanelMsg::FpEditorSetSilkLineToY,
                muted,
                primary,
                border_c,
            ));
        }
        SilkKindGeometry::Text {
            position_mm,
            content,
            size_mm,
        } => {
            col = col.push(pad_input_row(
                "Content",
                "TEXT",
                content.clone(),
                PanelMsg::FpEditorSetSilkText,
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "Position X (mm)",
                "0",
                format!("{:.3}", position_mm[0]),
                PanelMsg::FpEditorSetSilkTextPositionX,
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "Position Y (mm)",
                "0",
                format!("{:.3}", position_mm[1]),
                PanelMsg::FpEditorSetSilkTextPositionY,
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "Size (mm)",
                "1.0",
                format!("{:.3}", size_mm),
                PanelMsg::FpEditorSetSilkTextSize,
                muted,
                primary,
                border_c,
            ));
        }
        SilkKindGeometry::Other => {
            col = col.push(
                container(
                    text("Use Sketch mode for parametric editing")
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
