//! The result of a calculation: the expression and the status pill, the
//! Target / Nominal / Minimum / Maximum strip with each delta under its
//! value, then the network drawing beside the part cards.

use iced::alignment::Vertical;
use iced::widget::{Column, Row, container, space};
use iced::{Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;

use super::view::PageLook;
use super::{
    CalculatorControl, CalculatorMessage, OffTargetTone, TARGET_UNAVAILABLE, format_difference,
    format_target, is_unbounded_difference, off_target,
};
use crate::passive_calculator::component_card::{ComponentCard, ComponentCardMessage};
use crate::passive_calculator::names::boundary_name;
use crate::passive_calculator::network::{Network, format_value};
use crate::passive_calculator::network_drawing::{network_illustration, part_field};
use crate::passive_calculator::widgets::{
    RADII, SP, TS, data, medium, metric_cell, metric_strip, metric_value, pill,
};
use crate::property_illustration::property_illustration;
use crate::text;
use crate::theme_ext::HAIRLINE;

/// Width of the drawing box beside the cards (the mockup's 200 px column),
/// in em of the body size.
const DRAWING_BOX_EM: f32 = 200.0 / 12.0;
/// Part cards to a line beside the drawing.
const CARDS_PER_LINE: usize = 2;

impl CalculatorControl {
    pub(super) fn result_view<'a>(
        &'a self,
        result: &'a Network,
        i18n: &'a I18n,
        tokens: &'a ThemeTokens,
        look: PageLook,
        is_narrow: bool,
    ) -> Element<'a, CalculatorMessage> {
        let PageLook { palette, mono } = look;
        // GH #599: never substitute 0.0 for a target that cannot be read;
        // the target-dependent metrics read TARGET_UNAVAILABLE instead.
        let target = self.target_value().ok();
        let kind = self.kind;
        let nominal = result.nominal(kind);
        let minimum = result.minimum(kind);
        let maximum = result.maximum(kind);

        let mut heading = Row::new()
            .push(
                text(t!(i18n, ids::PASSIVE_CALCULATOR_RESULT))
                    .size(TS.label)
                    .font(medium(crate::ui_font::regular()))
                    .color(palette.ink)
                    .boxed(),
            )
            .push(self.expression(result, i18n, look))
            .push(space::horizontal().boxed())
            .spacing(SP.m + SP.xs)
            .align_y(Vertical::Center)
            .width(Length::Fill);
        if let Some((percent, tone)) = off_target(nominal, target, self.active_state().series) {
            let (ink, fill) = match tone {
                OffTargetTone::Ok => (palette.ok, palette.ok_soft),
                OffTargetTone::Warn => (palette.warn, palette.warn_soft),
            };
            let label = t!(
                i18n,
                ids::PASSIVE_CALCULATOR_OFF_TARGET,
                "percent" => format!("{percent:.2}")
            );
            heading = heading.push(pill(label, ink, fill));
        }

        let delta = |value: f64, ink: iced::Color| -> Element<'a, CalculatorMessage> {
            let line = match target {
                None => data(TARGET_UNAVAILABLE),
                Some(target) if is_unbounded_difference(value, target) => {
                    t!(i18n, ids::PASSIVE_CALCULATOR_DELTA_UNBOUNDED).into()
                }
                Some(target) => t!(
                    i18n,
                    ids::PASSIVE_CALCULATOR_DELTA,
                    "difference" => format_difference(value, target)
                )
                .into(),
            };
            text(line).font(mono).size(TS.caption).color(ink).boxed()
        };
        let cells = vec![
            (
                1,
                metric_cell(
                    t!(i18n, ids::PASSIVE_CALCULATOR_TARGET),
                    metric_value(format_target(target, kind), palette, mono),
                    None,
                    palette,
                ),
            ),
            (
                1,
                metric_cell(
                    t!(i18n, ids::PASSIVE_CALCULATOR_NOMINAL),
                    metric_value(format_value(nominal, kind), palette, mono),
                    Some(delta(nominal, palette.ok)),
                    palette,
                ),
            ),
            (
                1,
                metric_cell(
                    t!(i18n, ids::PASSIVE_CALCULATOR_MINIMUM),
                    metric_value(format_value(minimum, kind), palette, mono),
                    Some(delta(minimum, palette.ink3)),
                    palette,
                ),
            ),
            (
                1,
                metric_cell(
                    t!(i18n, ids::PASSIVE_CALCULATOR_MAXIMUM),
                    metric_value(format_value(maximum, kind), palette, mono),
                    Some(delta(maximum, palette.ink3)),
                    palette,
                ),
            ),
        ];
        let strip = metric_strip(cells, if is_narrow { 2 } else { 4 }, palette);

        Column::new()
            .push(heading.boxed())
            .push(strip)
            .push(self.parts_view(result, i18n, tokens, look, is_narrow))
            .spacing(SP.m)
            .width(Length::Fill)
            .boxed()
    }

    /// `R1 + R2 = 4.52 kΩ`, or the name of a wire bridge or open circuit.
    fn expression<'a>(
        &self,
        result: &Network,
        i18n: &I18n,
        look: PageLook,
    ) -> Element<'a, CalculatorMessage> {
        let PageLook { palette, mono } = look;
        if let Network::Boundary { condition } = result {
            return text(boundary_name(i18n, *condition))
                .size(TS.label)
                .color(palette.ink)
                .boxed();
        }
        let expression = result.plain_expression(self.kind);
        // A connection is written as "(left op right)"; the outer pair
        // encloses the whole line, so the line drops it.
        let shown = match result {
            Network::Connected { .. } => expression
                .strip_prefix('(')
                .and_then(|inner| inner.strip_suffix(')'))
                .unwrap_or(&expression),
            _ => &expression,
        };
        let line = format!(
            "{shown} = {}",
            format_value(result.nominal(self.kind), self.kind)
        );
        text(data(line))
            .font(mono)
            .size(TS.label)
            .color(palette.ink)
            .boxed()
    }

    /// The drawing beside the part cards; stacked when narrow.
    fn parts_view<'a>(
        &'a self,
        result: &'a Network,
        i18n: &'a I18n,
        tokens: &'a ThemeTokens,
        look: PageLook,
        is_narrow: bool,
    ) -> Element<'a, CalculatorMessage> {
        let PageLook { palette, mono } = look;
        let components = result.components();
        let mut cards: Column<Element<'a, CalculatorMessage>> =
            Column::new().spacing(SP.m + SP.xs).width(Length::Fill);
        if components.is_empty() {
            cards = cards.push(
                text(t!(i18n, ids::PASSIVE_CALCULATOR_NO_PARTS))
                    .size(TS.body)
                    .color(palette.ink3)
                    .boxed(),
            );
        }
        let per_line = if is_narrow { 1 } else { CARDS_PER_LINE };
        let card_line = || Row::new().spacing(SP.m + SP.xs).width(Length::Fill);
        let mut line: Row<Element<'a, CalculatorMessage>> = card_line();
        let mut in_line = 0;
        let count = components.len();
        for (index, (component, tolerance)) in components.into_iter().enumerate() {
            let is_highlighted = self.drawing_hover.hovered() == Some(&part_field(index));
            let card = ComponentCard::new(index, self.kind, component, tolerance)
                .highlighted(is_highlighted)
                .view(i18n, palette, mono)
                .map(CalculatorMessage::from);
            line = line.push(container(card).width(Length::FillPortion(1)).boxed());
            in_line += 1;
            if in_line == per_line || index + 1 == count {
                // An odd last card keeps its share; a spacer takes the rest.
                for _ in in_line..per_line {
                    line = line.push(space::horizontal().boxed());
                }
                cards = cards.push(line.boxed());
                line = card_line();
                in_line = 0;
            }
        }

        let box_width = TS.body * DRAWING_BOX_EM;
        let drawing = property_illustration(
            network_illustration(result, self.kind, box_width - 2.0 * SP.l, TS.caption),
            &self.drawing_hover,
            tokens,
            TS.caption,
        )
        .map(CalculatorMessage::DrawingHovered);
        let drawing_box = container(drawing)
            .padding(SP.l)
            .width(if is_narrow {
                Length::Fill
            } else {
                Length::Fixed(box_width)
            })
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(palette.sunken)),
                border: Border {
                    width: HAIRLINE,
                    radius: RADII.button.into(),
                    color: palette.line_soft,
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed();

        if is_narrow {
            return Column::new()
                .push(drawing_box)
                .push(cards.boxed())
                .spacing(SP.l)
                .width(Length::Fill)
                .boxed();
        }
        Row::new()
            .push(drawing_box)
            .push(cards.boxed())
            .spacing(SP.l)
            .width(Length::Fill)
            .boxed()
    }
}

impl From<ComponentCardMessage> for CalculatorMessage {
    fn from(message: ComponentCardMessage) -> Self {
        match message {
            ComponentCardMessage::ToleranceChanged { index, tolerance } => {
                Self::ToleranceChanged(index, tolerance)
            }
        }
    }
}
