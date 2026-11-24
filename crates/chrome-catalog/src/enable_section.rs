//! Enable-header subsection sample (epic #2523, BB3): the header is the
//! enable checkbox, and the fields under a switched-off header are greyed
//! under a panel-coloured veil and take no input.
//!
//! The widget is `signex_widgets::enable_section`. This crate cannot reach
//! the app's form look, so it builds the style from the theme tokens.

use iced::widget::{column, container, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::enable_section::{EnableSection, EnableSectionStyle};
use signex_widgets::theme_ext::{HAIRLINE, Radii, Spacing, TypeScale};
use signex_widgets::{Label, checkbox, raw, text};

use crate::catalog::Message;
use crate::theme;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Sample text: fixture content, not catalog copy. The one `raw` site of
/// the page.
fn label(s: &'static str) -> Label<'static> {
    raw(s)
}

/// An on and an off subsection side by side.
pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    row![
        subsection(
            tokens,
            "Paste",
            true,
            &[("Expansion", "0.05 mm"), ("Mode", "Rule")]
        ),
        subsection(
            tokens,
            "Thermal Relief",
            false,
            &[("Spokes", "4"), ("Gap", "0.25 mm")]
        ),
    ]
    .spacing(SP.xl)
    .boxed()
}

/// The section style from the tokens: a form checkbox filled with the
/// primary colour when ticked. It approximates the app's
/// `form_look::checkbox_style` and the `ink4` fallback of its token
/// resolution, which this crate cannot reach, so it may drift from them.
pub(crate) fn style(tokens: &ThemeTokens) -> EnableSectionStyle<'static> {
    let fill = theme::color(tokens.extended.primary.unwrap_or(tokens.accent));
    let field = theme::color(tokens.extended.field.unwrap_or(tokens.panel_bg));
    let line = theme::color(tokens.extended.line_strong.unwrap_or(tokens.border));
    let ink4 = theme::color(tokens.extended.ink4.unwrap_or(tokens.text_secondary));
    let paper = theme::color(tokens.paper);
    let border = theme::color(tokens.border);
    EnableSectionStyle {
        header_ink: theme::color(tokens.text_secondary),
        disabled_ink: ink4,
        panel: theme::color(tokens.panel_bg),
        font: iced::Font::DEFAULT,
        checkbox: Box::new(move |_: &Theme, status: checkbox::Status| {
            let is_checked = match status {
                checkbox::Status::Active { is_checked }
                | checkbox::Status::Hovered { is_checked }
                | checkbox::Status::Disabled { is_checked } => is_checked,
            };
            checkbox::Style {
                background: Background::Color(if is_checked { fill } else { field }),
                icon_color: Color::WHITE,
                border: Border {
                    width: HAIRLINE,
                    radius: Radii::GRAPHITE.control.into(),
                    color: if is_checked { fill } else { line },
                    ..Border::default()
                },
                text_color: None,
            }
        }),
        tip: Box::new(move |_: &Theme| container::Style {
            background: Some(Background::Color(paper)),
            border: Border {
                width: HAIRLINE,
                color: border,
                ..Border::default()
            },
            ..container::Style::default()
        }),
    }
}

fn subsection<'a>(
    tokens: &ThemeTokens,
    name: &'static str,
    is_enabled: bool,
    fields: &[(&'static str, &'static str)],
) -> Element<'a, Message> {
    let mut body = column![].spacing(SP.s);
    for &(field, value) in fields {
        body = body.push(field_row(tokens, field, value));
    }
    let section = EnableSection::new(label(name), is_enabled)
        .on_toggle(|_| Message::TreeInteracted)
        .tip(label("Turning this off keeps the values below"))
        .view(style(tokens), body.boxed());
    container(section)
        .width(Length::Fixed(TS.body * 22.0))
        .boxed()
}

/// One dependent field: label and a value box.
fn field_row<'a>(
    tokens: &ThemeTokens,
    name: &'static str,
    value: &'static str,
) -> Element<'a, Message> {
    let ink = theme::color(tokens.text);
    let ink3 = theme::color(tokens.extended.ink3.unwrap_or(tokens.text_secondary));
    let field = theme::color(tokens.extended.field.unwrap_or(tokens.panel_bg));
    let line = theme::color(tokens.border);
    row![
        container(text(label(name)).size(TS.body).color(ink3)).width(Length::FillPortion(2)),
        container(text(label(value)).size(TS.body).color(ink))
            .padding([SP.xs, SP.s + SP.xs])
            .width(Length::FillPortion(3))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(field)),
                border: Border {
                    width: HAIRLINE,
                    radius: Radii::GRAPHITE.control.into(),
                    color: line,
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    ]
    .align_y(iced::Alignment::Center)
    .padding([0.0, SP.l])
    .boxed()
}
