//! The result area of the dialog: two layer panels and one status row.
//!
//! The shape is the same in every state. An invalid input does not remove
//! the panels, it blanks their values, and failures and chart warnings
//! share one status row of fixed height, so the modal never changes
//! height while the user types (a centred card would otherwise jump).

use iced::widget::{Space, column, container, row, scrollable, text};
use iced::{Alignment, Background, Border, Color, Element, Length};
use signex_types::theme::ThemeTokens;

use super::control::{TraceCalculatorError, TraceCalculatorMessage, format_number, unit_picker};
use super::domain::{LayerKind, LayerResult, TraceCalculation, WidthUnit};
use crate::theme_ext;

/// What a value reads while the inputs cannot be calculated.
pub const UNAVAILABLE: &str = "\u{2014}";

const BODY_SIZE: f32 = 11.0;
const ROW_SPACING: f32 = 8.0;
const PANEL_PADDING: u16 = 12;
const PANEL_RADIUS: f32 = 6.0;
const OUTPUT_VALUE_WIDTH: f32 = 84.0;
const OUTPUT_UNIT_WIDTH: f32 = 40.0;
/// Room for about four lines of status text; longer text scrolls.
const STATUS_HEIGHT: f32 = 84.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRow {
    pub label: &'static str,
    pub value: String,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelModel {
    pub title: &'static str,
    pub layer: LayerKind,
    pub width_unit: WidthUnit,
    pub rows: [ValueRow; 5],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Clear,
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusModel {
    pub kind: StatusKind,
    pub text: String,
}

/// Everything the result area shows. Panels are ordered inner, outer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultsModel {
    pub panels: [PanelModel; 2],
    pub status: StatusModel,
}

pub fn results_model(
    outcome: &Result<TraceCalculation, TraceCalculatorError>,
    inner_unit: WidthUnit,
    outer_unit: WidthUnit,
) -> ResultsModel {
    let calculation = outcome.as_ref().ok();
    ResultsModel {
        panels: [
            panel(
                "Inner layer",
                LayerKind::Inner,
                calculation.map(|c| c.inner),
                inner_unit,
            ),
            panel(
                "Outer layer",
                LayerKind::Outer,
                calculation.map(|c| c.outer),
                outer_unit,
            ),
        ],
        status: status(outcome),
    }
}

fn panel(
    title: &'static str,
    layer: LayerKind,
    result: Option<LayerResult>,
    width_unit: WidthUnit,
) -> PanelModel {
    let value = |read: fn(LayerResult) -> f64| {
        result.map_or_else(|| UNAVAILABLE.to_string(), |r| format_number(read(r)))
    };
    let row = |label, value: String, unit: &str| ValueRow {
        label,
        value,
        unit: unit.to_string(),
    };
    PanelModel {
        title,
        layer,
        width_unit,
        rows: [
            row(
                "Required width",
                result.map_or_else(
                    || UNAVAILABLE.to_string(),
                    |r| format_number(r.width(width_unit)),
                ),
                &width_unit.to_string(),
            ),
            row("Area", value(|r| r.area_mil2), "mil\u{b2}"),
            row("Resistance", value(|r| r.resistance_ohm), "\u{3a9}"),
            row("Voltage drop", value(|r| r.voltage_drop_v), "V"),
            row("Power loss", value(|r| r.power_loss_w), "W"),
        ],
    }
}

fn status(outcome: &Result<TraceCalculation, TraceCalculatorError>) -> StatusModel {
    match outcome {
        Err(error) => StatusModel {
            kind: StatusKind::Error,
            text: error.to_string(),
        },
        Ok(calculation) if calculation.warnings.is_empty() => StatusModel {
            kind: StatusKind::Clear,
            text: String::new(),
        },
        Ok(calculation) => {
            let warnings = calculation
                .warnings
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            StatusModel {
                kind: StatusKind::Warning,
                text: format!("{warnings} The result is an extrapolation."),
            }
        }
    }
}

/// `alpha` (0..=1) fades the values and the status text; the structure
/// and sizes do not depend on it.
pub fn view<'a>(
    model: ResultsModel,
    alpha: f32,
    tokens: &'a ThemeTokens,
) -> Element<'a, TraceCalculatorMessage> {
    let [inner, outer] = model.panels;
    let panels = row![
        panel_view(inner, alpha, tokens),
        panel_view(outer, alpha, tokens)
    ]
    .spacing(ROW_SPACING);
    column![panels, status_view(model.status, alpha, tokens)]
        .spacing(ROW_SPACING)
        .into()
}

fn panel_view<'a>(
    model: PanelModel,
    alpha: f32,
    tokens: &'a ThemeTokens,
) -> Element<'a, TraceCalculatorMessage> {
    let on_unit = match model.layer {
        LayerKind::Inner => TraceCalculatorMessage::InnerWidthUnitChanged,
        LayerKind::Outer => TraceCalculatorMessage::OuterWidthUnitChanged,
    };
    let mut body = column![
        row![
            text(model.title)
                .size(BODY_SIZE)
                .color(theme_ext::text_primary(tokens)),
            Space::new().width(Length::Fill),
            unit_picker(WidthUnit::ALL, model.width_unit, on_unit),
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(ROW_SPACING);
    for value_row in model.rows {
        body = body.push(output_row(value_row, alpha, tokens));
    }
    container(body)
        .padding(PANEL_PADDING)
        .width(Length::Fill)
        .style(move |_theme| panel_style(tokens))
        .into()
}

fn output_row<'a>(
    value_row: ValueRow,
    alpha: f32,
    tokens: &'a ThemeTokens,
) -> Element<'a, TraceCalculatorMessage> {
    let primary = theme_ext::text_primary(tokens);
    row![
        text(value_row.label)
            .size(BODY_SIZE)
            .color(theme_ext::text_secondary(tokens))
            .width(Length::Fill),
        text(value_row.value)
            .size(BODY_SIZE)
            .color(faded(primary, alpha))
            .width(OUTPUT_VALUE_WIDTH),
        text(value_row.unit)
            .size(BODY_SIZE)
            .color(primary)
            .width(OUTPUT_UNIT_WIDTH),
    ]
    .align_y(Alignment::Center)
    .spacing(ROW_SPACING)
    .into()
}

/// Always the same height; bordered only while it says something.
fn status_view<'a>(
    status: StatusModel,
    alpha: f32,
    tokens: &'a ThemeTokens,
) -> Element<'a, TraceCalculatorMessage> {
    let color = match status.kind {
        StatusKind::Error => theme_ext::error_color(tokens),
        StatusKind::Warning | StatusKind::Clear => theme_ext::warning_color(tokens),
    };
    let visible = status.kind != StatusKind::Clear;
    container(scrollable(
        text(status.text)
            .size(BODY_SIZE)
            .color(faded(color, alpha))
            .width(Length::Fill),
    ))
    .padding(PANEL_PADDING)
    .width(Length::Fill)
    .height(STATUS_HEIGHT)
    .style(move |_theme| {
        if visible {
            panel_style(tokens)
        } else {
            iced::widget::container::Style::default()
        }
    })
    .into()
}

fn faded(color: Color, alpha: f32) -> Color {
    Color {
        a: color.a * alpha.clamp(0.0, 1.0),
        ..color
    }
}

fn panel_style(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme_ext::to_color(&tokens.panel_bg))),
        border: Border {
            width: 1.0,
            radius: PANEL_RADIUS.into(),
            color: theme_ext::border_color(tokens),
        },
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_calculator::control::TraceCalculatorControl;
    use crate::trace_calculator::domain::{CalculationError, InputField};

    fn model_for(current: &str) -> ResultsModel {
        let mut control = TraceCalculatorControl::default();
        control.update(TraceCalculatorMessage::CurrentChanged(current.into()));
        results_model(
            &control.calculation(),
            control.inner_width_unit,
            control.outer_width_unit,
        )
    }

    #[test]
    fn valid_and_invalid_input_have_the_same_panels_and_rows() {
        let valid = model_for("5");
        let invalid = model_for("five");

        for (v, i) in valid.panels.iter().zip(&invalid.panels) {
            assert_eq!(
                (v.title, v.layer, v.width_unit),
                (i.title, i.layer, i.width_unit)
            );
            let labels = |p: &PanelModel| {
                p.rows
                    .iter()
                    .map(|r| (r.label, r.unit.clone()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(labels(v), labels(i), "rows and units must not change");
        }
    }

    #[test]
    fn invalid_input_blanks_every_value() {
        let invalid = model_for("five");
        for panel in &invalid.panels {
            for row in &panel.rows {
                assert_eq!(row.value, UNAVAILABLE, "{} / {}", panel.title, row.label);
            }
        }
        let valid = model_for("5");
        assert!(
            valid
                .panels
                .iter()
                .flat_map(|p| &p.rows)
                .all(|r| r.value != UNAVAILABLE)
        );
    }

    #[test]
    fn the_status_row_carries_the_failure_and_is_clear_when_valid() {
        let invalid = model_for("five");
        assert_eq!(invalid.status.kind, StatusKind::Error);
        assert_eq!(invalid.status.text, "Enter a numeric value for current.");
        let valid = model_for("5");
        assert_eq!(valid.status.kind, StatusKind::Clear);
        assert!(valid.status.text.is_empty());
    }

    #[test]
    fn chart_warnings_use_the_same_status_row() {
        let warned = model_for("40");
        assert_eq!(warned.status.kind, StatusKind::Warning);
        assert!(warned.status.text.contains("stops at 17.5 A"));
        assert!(
            warned
                .status
                .text
                .ends_with("The result is an extrapolation.")
        );
        assert_eq!(
            warned.panels[0].rows.len(),
            model_for("5").panels[0].rows.len()
        );
    }

    #[test]
    fn a_domain_rejection_also_blanks_the_values() {
        let outcome = Err(TraceCalculatorError::Calculation(
            CalculationError::NonPositive(InputField::Current),
        ));
        let model = results_model(&outcome, WidthUnit::Millimeter, WidthUnit::Mil);
        assert_eq!(model.panels[1].width_unit, WidthUnit::Mil);
        assert!(
            model
                .panels
                .iter()
                .flat_map(|p| &p.rows)
                .all(|r| r.value == UNAVAILABLE)
        );
        assert_eq!(model.status.kind, StatusKind::Error);
    }
}
