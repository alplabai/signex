use iced::widget::{column, container, pick_list, row};
use iced::{Alignment, Background, Border, Element, Length, Widget};
use signex_types::theme::ThemeTokens;

use super::color_code::ComponentColorCode;
use super::color_code_view::color_code_representations;
use super::domain::{ComponentKind, PreferredComponent, Tolerance};
use super::network::format_value;
use super::rkm_code::RkmCode;
use super::widgets::{COMPACT_METRIC, labeled, metric, token_color};

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
                COMPACT_METRIC,
                "Part",
                format!("{}{}", kind.symbol(), subscript(index + 1)),
                tokens
            ),
            metric(
                COMPACT_METRIC,
                "Value",
                format_value(component.value(), kind),
                tokens
            ),
            metric(
                COMPACT_METRIC,
                "RKM code",
                RkmCode::for_component(kind, component, tolerance).to_string(),
                tokens
            ),
            labeled(
                11.0,
                "Tolerance",
                pick_list(Some(tolerance), Tolerance::ALL, Tolerance::to_string)
                    .on_select(move |tolerance| ComponentCardMessage::ToleranceChanged {
                        index,
                        tolerance,
                    })
                    .text_size(11)
                    .padding([4, 6])
                    .width(105),
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
            .boxed()
    }
}

fn card_style(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(Background::Color(token_color(tokens.panel_bg))),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: token_color(tokens.border),
            ..Border::default()
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
