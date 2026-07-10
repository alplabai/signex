use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::color_code::ComponentColorCode;
use crate::domain::{ComponentKind, ESeries, SiPrefix, Tolerance};
use crate::network::{Network, format_number, format_value};
use crate::solver::{MAX_PARTS, SolveOptions, solve};

const MAX_COMPONENT_OPTIONS: [usize; MAX_PARTS] = [1, 2, 3, 4];

#[derive(Debug, Clone)]
pub enum CalculatorMessage {
    KindChanged(ComponentKind),
    TargetChanged(String),
    PrefixChanged(SiPrefix),
    SeriesChanged(ESeries),
    MaxComponentsChanged(usize),
    Calculate,
    ToleranceChanged(usize, Tolerance),
}

#[derive(Debug, Clone)]
pub struct CalculatorControl {
    pub kind: ComponentKind,
    pub modes: [CalculatorModeState; 3],
}

#[derive(Debug, Clone)]
pub struct CalculatorModeState {
    pub target_input: String,
    pub prefix: SiPrefix,
    pub series: ESeries,
    pub max_components: usize,
    pub result: Option<Network>,
    pub validation_error: Option<String>,
}

impl Default for CalculatorControl {
    fn default() -> Self {
        Self {
            kind: ComponentKind::Resistor,
            modes: [
                CalculatorModeState::new(ComponentKind::Resistor),
                CalculatorModeState::new(ComponentKind::Capacitor),
                CalculatorModeState::new(ComponentKind::Inductor),
            ],
        }
    }
}

impl CalculatorModeState {
    fn new(kind: ComponentKind) -> Self {
        Self {
            target_input: "220".to_string(),
            prefix: match kind {
                ComponentKind::Resistor => SiPrefix::None,
                ComponentKind::Capacitor => SiPrefix::Micro,
                ComponentKind::Inductor => SiPrefix::Milli,
            },
            series: ESeries::E24,
            max_components: 3,
            result: None,
            validation_error: None,
        }
    }
}

impl CalculatorControl {
    pub fn state(&self, kind: ComponentKind) -> &CalculatorModeState {
        &self.modes[kind.index()]
    }

    pub fn state_mut(&mut self, kind: ComponentKind) -> &mut CalculatorModeState {
        &mut self.modes[kind.index()]
    }

    pub fn active_state(&self) -> &CalculatorModeState {
        self.state(self.kind)
    }

    pub fn active_state_mut(&mut self) -> &mut CalculatorModeState {
        self.state_mut(self.kind)
    }

    pub fn update(&mut self, message: CalculatorMessage) {
        match message {
            CalculatorMessage::KindChanged(kind) => {
                self.kind = kind;
            }
            CalculatorMessage::TargetChanged(value) => {
                let state = self.active_state_mut();
                state.target_input = value;
                state.validation_error = None;
            }
            CalculatorMessage::PrefixChanged(prefix) => {
                let state = self.active_state_mut();
                state.prefix = prefix;
                state.result = None;
            }
            CalculatorMessage::SeriesChanged(series) => {
                let state = self.active_state_mut();
                state.series = series;
                state.result = None;
            }
            CalculatorMessage::MaxComponentsChanged(max_components) => {
                let state = self.active_state_mut();
                state.max_components = max_components.clamp(1, MAX_PARTS);
                state.result = None;
            }
            CalculatorMessage::Calculate => self.calculate(),
            CalculatorMessage::ToleranceChanged(index, tolerance) => {
                if let Some(result) = &mut self.active_state_mut().result {
                    result.set_tolerance(index, tolerance);
                }
            }
        }
    }

    pub fn target_value(&self) -> Result<f64, &'static str> {
        let state = self.active_state();
        let normalized = state.target_input.trim().replace(',', ".");
        let value = normalized
            .parse::<f64>()
            .map_err(|_| "Enter a valid positive number")?;
        if !value.is_finite() || value <= 0.0 {
            return Err("Enter a value greater than zero");
        }
        Ok(value * state.prefix.multiplier())
    }

    pub fn calculate(&mut self) {
        let target = match self.target_value() {
            Ok(value) => value,
            Err(error) => {
                let state = self.active_state_mut();
                state.result = None;
                state.validation_error = Some(error.to_string());
                return;
            }
        };

        let kind = self.kind;
        let state = self.active_state();
        let series = state.series;
        let max_components = state.max_components;
        let result = solve(SolveOptions {
            kind: self.kind,
            target,
            series,
            max_parts: max_components,
            default_tolerance: default_tolerance(series),
            result_limit: 1,
        })
        .into_iter()
        .next();
        let state = self.state_mut(kind);
        state.validation_error = result
            .is_none()
            .then(|| "No result was found for this configuration".to_string());
        state.result = result;
    }

    pub fn view<'a>(&'a self, tokens: &'a ThemeTokens) -> Element<'a, CalculatorMessage> {
        let state = self.active_state();
        let tabs = row(ComponentKind::ALL.map(|kind| {
            let active = kind == self.kind;
            button(text(kind.component_name()).size(14))
                .padding([8, 18])
                .on_press(CalculatorMessage::KindChanged(kind))
                .style(move |_theme: &Theme, status| {
                    let background = if active {
                        token_color(tokens.accent)
                    } else if status == button::Status::Hovered {
                        token_color(tokens.hover)
                    } else {
                        token_color(tokens.panel_bg)
                    };
                    button::Style {
                        background: Some(Background::Color(background)),
                        text_color: if active {
                            iced::Color::WHITE
                        } else {
                            token_color(tokens.text)
                        },
                        border: Border {
                            width: 1.0,
                            radius: 4.0.into(),
                            color: token_color(tokens.border),
                        },
                        ..button::Style::default()
                    }
                })
                .into()
        }))
        .spacing(6);

        let target_row = row![
            labeled(
                "Target",
                text_input("220", &state.target_input)
                    .on_input(CalculatorMessage::TargetChanged)
                    .padding(8)
                    .width(150),
            ),
            labeled(
                "Unit",
                pick_list(
                    SiPrefix::for_kind(self.kind),
                    Some(state.prefix),
                    CalculatorMessage::PrefixChanged,
                )
                .width(95),
            ),
            text(state.prefix.unit(self.kind))
                .size(14)
                .color(token_color(tokens.text_secondary)),
            labeled(
                "Preferred series",
                pick_list(
                    ESeries::ALL,
                    Some(state.series),
                    CalculatorMessage::SeriesChanged,
                )
                .width(100),
            ),
            labeled(
                "Maximum parts",
                pick_list(
                    MAX_COMPONENT_OPTIONS,
                    Some(state.max_components),
                    CalculatorMessage::MaxComponentsChanged,
                )
                .width(90),
            ),
            button(text("Calculate"))
                .padding([9, 18])
                .on_press(CalculatorMessage::Calculate),
        ]
        .spacing(12)
        .align_y(Alignment::End);

        let mut content = column![
            text("Passive Network Calculator")
                .size(24)
                .color(token_color(tokens.text)),
            text("Find a bounded preferred-value series/parallel realization.")
                .size(13)
                .color(token_color(tokens.text_secondary)),
            tabs,
            target_row,
        ]
        .spacing(14)
        .padding(20);

        if let Some(error) = &state.validation_error {
            content = content.push(
                text(error)
                    .size(13)
                    .color(iced::Color::from_rgb8(210, 70, 70)),
            );
        }

        if let Some(result) = &state.result {
            content = content.push(self.result_view(result, tokens));
        } else {
            content = content.push(
                container(
                    text("Enter a target and select Calculate to generate a network.")
                        .color(token_color(tokens.text_secondary)),
                )
                .padding(24)
                .width(Length::Fill)
                .style(move |_theme| panel_style(tokens)),
            );
        }

        scrollable(content).height(Length::Fill).into()
    }

    fn result_view<'a>(
        &'a self,
        result: &'a Network,
        tokens: &'a ThemeTokens,
    ) -> Element<'a, CalculatorMessage> {
        let target = self.target_value().unwrap_or_default();
        let nominal = result.nominal(self.kind);
        let minimum = result.minimum(self.kind);
        let maximum = result.maximum(self.kind);

        let summary = column![
            text(result.expression(self.kind))
                .size(20)
                .color(token_color(tokens.text)),
            row![
                metric("Target", format_value(target, self.kind), tokens),
                metric("Nominal", format_value(nominal, self.kind), tokens),
                metric("Minimum", format_value(minimum, self.kind), tokens),
                metric("Maximum", format_value(maximum, self.kind), tokens),
                metric("Nominal Δ", format_difference(nominal, target), tokens),
                metric("Lower Δ", format_difference(minimum, target), tokens),
                metric("Upper Δ", format_difference(maximum, target), tokens),
            ]
            .spacing(18),
        ]
        .spacing(9);

        let header = row![
            table_cell("Part", 90.0, tokens),
            table_cell("Value", 125.0, tokens),
            table_cell("Tolerance", 125.0, tokens),
            table_cell("Color code", 420.0, tokens),
        ]
        .spacing(8);

        let mut table = column![header].spacing(6);
        for (index, (component, tolerance)) in result.components().into_iter().enumerate() {
            let designator = format!("{}{}", self.kind.symbol(), subscript(index + 1));
            let color_code: Element<'a, CalculatorMessage> =
                match ComponentColorCode::for_kind(self.kind, component, tolerance) {
                    Some(code) => {
                        let mut bands = row![].spacing(5).align_y(Alignment::Center);
                        for band in code.bands.iter().copied() {
                            bands = bands.push(container(text(" ")).width(20).height(30).style(
                                move |_theme| container::Style {
                                    background: Some(Background::Color(band.color())),
                                    border: Border {
                                        width: 1.0,
                                        radius: 2.0.into(),
                                        color: iced::Color::BLACK,
                                    },
                                    ..container::Style::default()
                                },
                            ));
                        }
                        bands
                            .push(
                                text(code.accessible_label())
                                    .size(12)
                                    .color(token_color(tokens.text)),
                            )
                            .into()
                    }
                    None => text(color_code_unavailable_label(self.kind))
                        .size(12)
                        .color(token_color(tokens.text_secondary))
                        .into(),
                };

            table = table.push(
                row![
                    container(text(designator)).width(90),
                    container(text(format_value(component.value(), self.kind))).width(125),
                    container(pick_list(Tolerance::ALL, Some(tolerance), move |value| {
                        CalculatorMessage::ToleranceChanged(index, value)
                    },))
                    .width(125),
                    container(color_code).width(420),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }

        container(column![summary, table].spacing(16))
            .padding(16)
            .width(Length::Fill)
            .style(move |_theme| panel_style(tokens))
            .into()
    }
}

fn default_tolerance(series: ESeries) -> Tolerance {
    match series {
        ESeries::E3 => Tolerance::Percent20,
        ESeries::E6 => Tolerance::Percent20,
        ESeries::E12 => Tolerance::Percent10,
        ESeries::E24 => Tolerance::Percent5,
        ESeries::E48 => Tolerance::Percent2,
        ESeries::E96 => Tolerance::Percent1,
        ESeries::E192 => Tolerance::Percent0_5,
    }
}

fn color_code_unavailable_label(kind: ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Resistor => "Not representable with IEC 60062 bands",
        ComponentKind::Capacitor => "Not representable with capacitor color bands",
        ComponentKind::Inductor => "Not representable with common µH color bands",
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
) -> Element<'a, CalculatorMessage> {
    column![
        text(label)
            .size(11)
            .color(token_color(tokens.text_secondary)),
        text(value).size(13).color(token_color(tokens.text)),
    ]
    .spacing(2)
    .into()
}

fn table_cell<'a>(
    label: &'a str,
    width: f32,
    tokens: &'a ThemeTokens,
) -> Element<'a, CalculatorMessage> {
    container(
        text(label)
            .size(12)
            .color(token_color(tokens.text_secondary)),
    )
    .width(width)
    .into()
}

fn format_difference(value: f64, target: f64) -> String {
    let difference = value - target;
    let percentage = difference / target * 100.0;
    format!("{:+} ({:+.4}%)", format_number(difference), percentage)
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

fn panel_style(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(Background::Color(token_color(tokens.panel_bg))),
        border: Border {
            width: 1.0,
            radius: 6.0.into(),
            color: token_color(tokens.border),
        },
        ..container::Style::default()
    }
}
