//! Pour role sub-form. Split from `subforms.rs`.

use iced::widget::{Column, Widget, container, pick_list, row, text, text_input};
use iced::{Color, Element, Length};
use signex_widgets::dropdown_style::GraphiteDropdown as _;

use super::super::super::{FootprintEditorPanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// v0.16.4 — Pour role sub-form. Renders when the entity's `pour`
/// attr is set; otherwise the column passes through unchanged.
pub(in crate::panels::footprint_editor_properties) fn render_pour_subform<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<Element<'a, PanelMsg>> {
    let Some(pour) = fp.selected_pour.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Pour properties").size(TS.caption).color(primary))
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed(),
    );

    // Net (text input — empty = unassigned)
    let net_buf = pour.net.clone().unwrap_or_default();
    col = col.push(
        container(
            row![
                text("Net")
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text_input("(none)", net_buf)
                    .size(TS.caption)
                    .padding(SP.xs)
                    .style(crate::panels::footprint_editor_properties::fp_field_style(
                        muted, primary, border_c
                    ))
                    .on_input(move |v| PanelMsg::FpEditorSetPourNet { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed(),
    );

    // Fill type (Solid / Hatched / Outline)
    let fill_picker = pick_list(
        Some(pour.fill_type),
        signex_sketch::attr::PourFillType::ALL,
        |v: &signex_sketch::attr::PourFillType| v.to_string(),
    )
    .graphite()
    .on_select(move |v| PanelMsg::FpEditorSetPourFillType { id, value: v })
    .text_size(10)
    .padding([3.0, SP.m]);
    col = col.push(
        container(
            row![
                text("Fill")
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                fill_picker,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed(),
    );

    // Priority (u32 text input)
    col = col.push(
        container(
            row![
                text("Priority")
                    .size(TS.caption)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text_input("0", pour.priority.to_string())
                    .size(TS.caption)
                    .padding(SP.xs)
                    .style(crate::panels::footprint_editor_properties::fp_field_style(
                        muted, primary, border_c
                    ))
                    .on_input(move |v| PanelMsg::FpEditorSetPourPriority { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed(),
    );

    col
}
