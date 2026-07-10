use iced::widget::{column, container, pick_list, row, text};
use iced::{Alignment, Background, Border, Element, Length};
use signex_types::theme::ThemeTokens;

use super::color_code::ComponentColorCode;
use super::color_code_view::color_code_representations;
use super::domain::{ComponentKind, PreferredComponent, Tolerance};
use super::network::format_value;
use super::rkm_code::RkmCode;

#[derive(Debug, Clone)]
pub enum ComponentCardMessage {
    ToleranceChanged { index: usize, tolerance: Tolerance },
}

pub struct ComponentCard<'a> {
    index: usize,
    kind: ComponentKind,
    component: PreferredComponent,
    tolerance: Tolerance,
    tokens: &'a ThemeTokens,
}

impl<'a> ComponentCard<'a> {
    pub fn new(
        index: usize,
        kind: ComponentKind,
        component: PreferredComponent,
        tolerance: Tolerance,
        tokens: &'a ThemeTokens,
    ) -> Self {
        Self {
            index,
            kind,
            component,
            tolerance,
            tokens,
        }
    }

    pub fn view(self) -> Element<'a, ComponentCardMessage> {
        let Self {
            index,
            kind,
            component,
            tolerance,
            tokens,
        } = self;

        let details = row![
            metric(
                "Part",
                format!("{}{}", kind.symbol(), subscript(index + 1)),
                tokens
            ),
            metric("Value", format_value(component.value(), kind), tokens),
            metric(
                "RKM code",
                RkmCode::for_component(kind, component, tolerance).to_string(),
                tokens
            ),
            labeled(
                "Tolerance",
                pick_list(Tolerance::ALL, Some(tolerance), move |tolerance| {
                    ComponentCardMessage::ToleranceChanged { index, tolerance }
                })
                .width(125),
            ),
        ]
        .spacing(28)
        .align_y(Alignment::End);

        let color_codes = color_code_representations(
            ComponentColorCode::representations_for_kind(kind, component, tolerance),
            kind,
            tokens,
        );

        container(column![details, color_codes].spacing(12))
            .padding(12)
            .width(Length::Fill)
            .style(move |_theme| card_style(tokens))
            .into()
    }
}

fn labeled<'a, M: 'a>(label: &'a str, control: impl Into<Element<'a, M>>) -> Element<'a, M> {
    column![text(label).size(12), control.into()]
        .spacing(4)
        .into()
}

fn metric<'a>(
    label: &'a str,
    value: String,
    tokens: &'a ThemeTokens,
) -> Element<'a, ComponentCardMessage> {
    column![
        text(label)
            .size(11)
            .color(token_color(tokens.text_secondary)),
        text(value).size(13).color(token_color(tokens.text)),
    ]
    .spacing(2)
    .into()
}

fn card_style(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(Background::Color(token_color(tokens.panel_bg))),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: token_color(tokens.border),
        },
        ..container::Style::default()
    }
}

fn subscript(value: usize) -> String {
    value
        .to_string()
        .chars()
        .map(|character| match character {
            '0' => '₀',
            '1' => '₁',
            '2' => '₂',
            '3' => '₃',
            '4' => '₄',
            '5' => '₅',
            '6' => '₆',
            '7' => '₇',
            '8' => '₈',
            '9' => '₉',
            _ => character,
        })
        .collect()
}

fn token_color(color: signex_types::theme::Color) -> iced::Color {
    iced::Color::from_rgba8(color.r, color.g, color.b, f32::from(color.a) / 255.0)
}
