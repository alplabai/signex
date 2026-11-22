//! The frame of a dialog's live preview (mockup rows 66 and 69): a bordered
//! paper panel with a caption, an optional corner control and the drawing.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::styles::{Radii, Spacing, TypeScale, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// `drawing` in the preview frame, under `caption` and beside `corner`.
pub(crate) fn preview_frame<'a, M: 'a>(
    caption: Localized,
    drawing: Element<'a, M>,
    corner: Option<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let ink = tokens::resolve(tokens).ink4;
    let (paper, edge) = (ti(tokens.paper), ti(tokens.border));
    let head = row![
        text(caption)
            .size(TS.caption)
            .color(ink)
            .width(Length::Fill),
        corner.unwrap_or_else(|| Space::new().boxed()),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    container(column![head, drawing].spacing(SP.xs))
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(paper)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
