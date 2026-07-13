//! BoardCutout role sub-form. Split from `subforms.rs`.

use iced::widget::{Column, container, row, text, text_input};
use iced::{Color, Length, Theme};

use super::super::super::{FootprintEditorPanelContext, PanelMsg};

/// v0.16.4 — BoardCutout role sub-form. Edge-radius expression input
/// + through-vs-partial-depth toggle.
pub(in crate::panels::footprint_editor_properties) fn render_cutout_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let Some(c) = fp.selected_cutout.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Cutout properties").size(10).color(primary))
            .padding([4, 8])
            .width(Length::Fill),
    );

    let radius_buf = c.edge_radius_expr.clone().unwrap_or_default();
    col = col.push(
        container(
            row![
                text("Edge radius")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text_input("(sharp)", &radius_buf)
                    .size(10)
                    .padding(2)
                    .style(move |_: &Theme, _| iced::widget::text_input::Style {
                        background: iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.04,
                        )),
                        border: iced::Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: border_c,
                        },
                        icon: iced::Color::TRANSPARENT,
                        placeholder: muted,
                        value: primary,
                        selection: iced::Color::from_rgba(0.4, 0.6, 1.0, 0.4),
                    })
                    .on_input(move |v| PanelMsg::FpEditorSetCutoutEdgeRadius { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    col = col.push(super::super::super::form_check_row(
        "Through (full board depth)",
        c.through,
        PanelMsg::FpEditorSetCutoutThrough {
            id,
            value: !c.through,
        },
        muted,
    ));

    col
}

