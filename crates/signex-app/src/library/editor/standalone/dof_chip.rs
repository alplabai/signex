//! Graphite canvas chips of the footprint editor: the sketch's constraint
//! state and the footprint name with its pad count. Both read the document.

use iced::widget::{Space, column, container, row};
use iced::{Border, Element, Length, Widget};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_sketch::solver::dof::DofColor;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;
use signex_widgets::theme_ext;

use crate::app::FootprintDocument;
use crate::library::messages::LibraryMessage;
use crate::styles::{PillTone, Radii, pill_colors};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the last solve says about the sketch, or why there is nothing to say.
fn dof_state(editor: &FootprintDocument) -> (PillTone, MessageId) {
    if editor.primitive().sketch.is_none() {
        return (PillTone::Neutral, ids::EDITORS_SKETCH_NONE);
    }
    let Some(solve) = editor.state.last_solve.as_ref() else {
        return (PillTone::Neutral, ids::EDITORS_SKETCH_NOT_SOLVED);
    };
    let over =
        !solve.over_constraints.is_empty() || solve.colours.values().any(|c| *c == DofColor::Over);
    if over {
        return (PillTone::Err, ids::EDITORS_SKETCH_OVER_CONSTRAINED);
    }
    if solve.colours.values().any(|c| *c == DofColor::Under) {
        return (PillTone::Info, ids::EDITORS_SKETCH_UNDER_CONSTRAINED);
    }
    (PillTone::Ok, ids::EDITORS_SKETCH_FULLY_CONSTRAINED)
}

fn pill<'a>(
    label: Localized,
    dot: Option<iced::Color>,
    fill: iced::Color,
    ink: iced::Color,
    line: iced::Color,
) -> Element<'a, LibraryMessage> {
    let mut content = row![].spacing(SP.m).align_y(iced::Alignment::Center);
    if let Some(color) = dot {
        let size = TS.caption * 0.6;
        content = content.push(
            container(Space::new())
                .width(Length::Fixed(size))
                .height(Length::Fixed(size))
                .style(move |_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(color)),
                    border: Border {
                        radius: 999.0.into(),
                        ..Border::default()
                    },
                    ..Default::default()
                })
                .boxed(),
        );
    }
    content = content.push(text(label).size(TS.caption).color(ink).boxed());
    container(content)
        .padding([SP.xs, SP.l])
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                width: 1.0,
                color: line,
                ..Border::default()
            },
            ..Default::default()
        })
        .boxed()
}

/// The chips floating at the canvas's top-left.
pub fn overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let line = theme_ext::border_color(tokens);
    let (tone, label) = dof_state(editor);
    let (fill, ink) = pill_colors(tokens, tone);
    let dot = (tone != PillTone::Neutral).then_some(ink);
    let dof = pill(t!(i18n, label), dot, fill, ink, line);

    let info = t!(
        i18n,
        ids::EDITORS_FOOTPRINT_NAME_PADS,
        "name" => editor.primitive().name.as_str(),
        "pads" => editor.state.pads.len()
    );
    let info = pill(
        info,
        None,
        theme_ext::to_color(&tokens.panel_bg),
        theme_ext::text_secondary(tokens),
        line,
    );
    container(column![dof, info].spacing(SP.m))
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}
