//! One part of a calculated network: Part / Value / RKM code / Tolerance as
//! key-value rows, the tolerance still editable, and the part's colour
//! code under them (`.ccard`).

use iced::alignment::Vertical;
use iced::widget::{Column, Row, container, pick_list};
use iced::{Background, Border, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, ids, t};

use super::color_code::ComponentColorCode;
use super::color_code_view::color_code_representations;
use super::domain::{ComponentKind, PreferredComponent, Tolerance};
use super::look::CalculatorPalette;
use super::network::format_value;
use super::rkm_code::RkmCode;
use super::widgets::{RADII, SP, TS, data};
use crate::dropdown_style::GraphiteDropdown;
use crate::text;
use crate::theme_ext::HAIRLINE;

/// Width of the key column, in em of the body size.
const KEY_EM: f32 = 5.5;

#[derive(Debug, Clone)]
pub enum ComponentCardMessage {
    ToleranceChanged { index: usize, tolerance: Tolerance },
}

pub struct ComponentCard {
    index: usize,
    kind: ComponentKind,
    component: PreferredComponent,
    tolerance: Tolerance,
    is_highlighted: bool,
}

impl ComponentCard {
    pub fn new(
        index: usize,
        kind: ComponentKind,
        component: PreferredComponent,
        tolerance: Tolerance,
    ) -> Self {
        Self {
            index,
            kind,
            component,
            tolerance,
            is_highlighted: false,
        }
    }

    /// Outlines the card in the accent while its symbol in the network
    /// drawing is under the pointer.
    pub fn highlighted(mut self, is_highlighted: bool) -> Self {
        self.is_highlighted = is_highlighted;
        self
    }

    pub fn view<'a>(
        self,
        i18n: &I18n,
        palette: CalculatorPalette,
        mono: Font,
    ) -> Element<'a, ComponentCardMessage> {
        let Self {
            index,
            kind,
            component,
            tolerance,
            is_highlighted,
        } = self;
        let value = |shown: String| -> Element<'a, ComponentCardMessage> {
            text(data(shown))
                .font(mono)
                .size(TS.body)
                .color(palette.ink)
                .boxed()
        };
        let tolerance_pick = pick_list(Some(tolerance), Tolerance::ALL, Tolerance::to_string)
            .graphite()
            .on_select(move |tolerance| ComponentCardMessage::ToleranceChanged { index, tolerance })
            .font(mono)
            .text_size(TS.body)
            .padding(Padding::from([0.0, SP.s]))
            .boxed();
        let rows = [
            (
                t!(i18n, ids::PASSIVE_CALCULATOR_PART),
                value(format!("{}{}", kind.symbol(), index + 1)),
            ),
            (
                t!(i18n, ids::PASSIVE_CALCULATOR_VALUE),
                value(format_value(component.value(), kind)),
            ),
            (
                t!(i18n, ids::PASSIVE_CALCULATOR_RKM_CODE),
                value(RkmCode::for_component(kind, component, tolerance).to_string()),
            ),
            (t!(i18n, ids::PASSIVE_CALCULATOR_TOLERANCE), tolerance_pick),
        ];
        let mut details: Column<Element<'a, ComponentCardMessage>> =
            Column::new().spacing(SP.xs).width(Length::Fill);
        for (key, shown) in rows {
            details = details.push(kv_row(key, shown, palette));
        }

        let color_codes = color_code_representations(
            i18n,
            ComponentColorCode::representations_for_kind(kind, component, tolerance),
            kind,
            palette,
        );
        let border = if is_highlighted {
            palette.accent
        } else {
            palette.border
        };
        container(
            Column::new()
                .push(details.boxed())
                .push(color_codes)
                .spacing(SP.m),
        )
        .padding(Padding::from([SP.m + SP.xs, SP.l]))
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(palette.panel)),
            border: Border {
                width: HAIRLINE,
                radius: RADII.card.into(),
                color: border,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
    }
}

/// A muted key beside its value (`.kv`).
fn kv_row<'a>(
    key: Localized,
    value: Element<'a, ComponentCardMessage>,
    palette: CalculatorPalette,
) -> Element<'a, ComponentCardMessage> {
    Row::new()
        .push(
            text(key)
                .size(TS.body)
                .color(palette.ink4)
                .width(TS.body * KEY_EM)
                .boxed(),
        )
        .push(value)
        .spacing(SP.l)
        .align_y(Vertical::Center)
        .boxed()
}
