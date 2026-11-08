//! Graphite canvas chips of the footprint editor: the sketch's constraint
//! state and the footprint name with its pad count. Both read the document.

use iced::widget::{Space, column, container, row, text};
use iced::{Border, Element, Length, Widget};
use signex_sketch::solver::dof::DofColor;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::app::FootprintDocument;
use crate::library::messages::LibraryMessage;
use crate::styles::{PillTone, Radii, pill_colors};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// What the last solve says about the sketch, or why there is nothing to say.
fn dof_state(editor: &FootprintDocument) -> (PillTone, String) {
    if editor.primitive().sketch.is_none() {
        return (PillTone::Neutral, "No sketch".to_string());
    }
    let Some(solve) = editor.state.last_solve.as_ref() else {
        return (PillTone::Neutral, "Sketch not solved".to_string());
    };
    let over =
        !solve.over_constraints.is_empty() || solve.colours.values().any(|c| *c == DofColor::Over);
    if over {
        return (PillTone::Err, "Over-constrained".to_string());
    }
    if solve.colours.values().any(|c| *c == DofColor::Under) {
        return (PillTone::Info, "Under-constrained".to_string());
    }
    (PillTone::Ok, "Fully constrained \u{b7} DOF 0".to_string())
}

fn pill<'a>(
    label: String,
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
) -> Element<'a, LibraryMessage> {
    let line = theme_ext::border_color(tokens);
    let (tone, label) = dof_state(editor);
    let (fill, ink) = pill_colors(tokens, tone);
    let dot = (tone != PillTone::Neutral).then_some(ink);
    let dof = pill(label, dot, fill, ink, line);

    let pads = editor.state.pads.len();
    let info = format!(
        "{} \u{b7} {} pad{}",
        editor.primitive().name,
        pads,
        if pads == 1 { "" } else { "s" }
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
