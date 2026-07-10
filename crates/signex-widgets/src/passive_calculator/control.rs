use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use super::component_card::{ComponentCard, ComponentCardMessage};
use super::domain::{ComponentKind, ESeries, SiPrefix, Tolerance};
use super::network::{Network, format_number, format_value};
use super::rkm_encoder::{RkmEncoder, RkmEncoderMessage};
use super::solver::{MAX_PARTS, SolveOptions, solve};

const MAX_COMPONENT_OPTIONS: [usize; MAX_PARTS] = [1, 2, 3, 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalculatorTab {
    Passive(ComponentKind),
    RkmEncoder,
}

#[derive(Debug, Clone)]
pub enum CalculatorMessage {
    KindChanged(ComponentKind),
    TabChanged(CalculatorTab),
    RkmEncoder(RkmEncoderMessage),
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
    pub active_tab: CalculatorTab,
    pub modes: [CalculatorModeState; 3],
    pub rkm_encoder: RkmEncoder,
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
            active_tab: CalculatorTab::Passive(ComponentKind::Resistor),
            modes: [
                CalculatorModeState::new(ComponentKind::Resistor),
                CalculatorModeState::new(ComponentKind::Capacitor),
                CalculatorModeState::new(ComponentKind::Inductor),
            ],
            rkm_encoder: RkmEncoder::default(),
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
                self.active_tab = CalculatorTab::Passive(kind);
            }
            CalculatorMessage::TabChanged(tab) => self.active_tab = tab,
            CalculatorMessage::RkmEncoder(message) => self.rkm_encoder.update(message),
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
        let value = match normalized.to_ascii_lowercase().as_str() {
            "∞" | "inf" | "infinity" => f64::INFINITY,
            _ => normalized
                .parse::<f64>()
                .map_err(|_| "Enter a valid non-negative number or infinity")?,
        };
        if value.is_nan() || value < 0.0 || value == f64::NEG_INFINITY {
            return Err("Enter a non-negative number or positive infinity");
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
        let tabs = row(ComponentKind::ALL.map(|kind| {
            tab_button(
                kind.component_name(),
                self.active_tab == CalculatorTab::Passive(kind),
                CalculatorMessage::KindChanged(kind),
                tokens,
            )
        }))
        .push(tab_button(
            "RKM Encoder",
            self.active_tab == CalculatorTab::RkmEncoder,
            CalculatorMessage::TabChanged(CalculatorTab::RkmEncoder),
            tokens,
        ))
        .spacing(6);

        if self.active_tab == CalculatorTab::RkmEncoder {
            let content = column![
                text("RKM Encoder").size(24).color(token_color(tokens.text)),
                text("Encode component specifications independently of network synthesis.")
                    .size(13)
                    .color(token_color(tokens.text_secondary)),
                tabs,
                self.rkm_encoder
                    .view(tokens)
                    .map(CalculatorMessage::RkmEncoder),
            ]
            .spacing(14)
            .padding(20);
            return scrollable(content).height(Length::Fill).into();
        }

        let state = self.active_state();

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

        let components = result.components();
        let mut component_cards = column![].spacing(10);
        if components.is_empty() {
            component_cards = component_cards.push(
                text("No discrete component is required.")
                    .size(13)
                    .color(token_color(tokens.text_secondary)),
            );
        }
        for (index, (component, tolerance)) in components.into_iter().enumerate() {
            component_cards = component_cards.push(
                ComponentCard::new(index, self.kind, component, tolerance, tokens)
                    .view()
                    .map(CalculatorMessage::from),
            );
        }

        container(column![summary, component_cards].spacing(16))
            .padding(16)
            .width(Length::Fill)
            .style(move |_theme| panel_style(tokens))
            .into()
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

fn tab_button<'a>(
    label: &'a str,
    active: bool,
    message: CalculatorMessage,
    tokens: &'a ThemeTokens,
) -> Element<'a, CalculatorMessage> {
    button(text(label).size(14))
        .padding([8, 18])
        .on_press(message)
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

fn format_difference(value: f64, target: f64) -> String {
    if value == target {
        return "0 (0%)".to_string();
    }
    if target == 0.0 || target == f64::INFINITY {
        return "unbounded".to_string();
    }
    let difference = value - target;
    let percentage = difference / target * 100.0;
    format!("{:+} ({:+.4}%)", format_number(difference), percentage)
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
