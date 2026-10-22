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
                // GH #599 — a result belongs to the target it was solved
                // for. Leaving it standing while the target changes
                // rendered the whole summary against `target = 0.0`:
                // "Target" read 0 and all three deltas were computed
                // against 0 while Nominal / Minimum / Maximum still
                // showed the real network — with no error text either,
                // because this arm has just cleared `validation_error`.
                // Every sibling edit arm (PrefixChanged, SeriesChanged,
                // MaxComponentsChanged) already drops the stale result.
                state.result = None;
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
                text("RKM Encoder").size(16).color(token_color(tokens.text)),
                text("Encode component specifications independently of network synthesis.")
                    .size(11)
                    .color(token_color(tokens.text_secondary)),
                tabs,
                self.rkm_encoder
                    .view(tokens)
                    .map(CalculatorMessage::RkmEncoder),
            ]
            .spacing(14)
            .padding(12);
            return scrollable(content).height(Length::Fill).into();
        }

        let state = self.active_state();

        let target_row = row![
            labeled(
                "Target",
                text_input("220", &state.target_input)
                    .on_input(CalculatorMessage::TargetChanged)
                    .size(11)
                    .padding([4, 6])
                    .width(110),
            ),
            labeled(
                "Unit",
                pick_list(
                    SiPrefix::for_kind(self.kind),
                    Some(state.prefix),
                    CalculatorMessage::PrefixChanged,
                )
                .text_size(11)
                .padding([4, 6])
                .width(80),
            ),
            text(state.prefix.unit(self.kind))
                .size(11)
                .color(token_color(tokens.text_secondary)),
            labeled(
                "Preferred series",
                pick_list(
                    ESeries::ALL,
                    Some(state.series),
                    CalculatorMessage::SeriesChanged,
                )
                .text_size(11)
                .padding([4, 6])
                .width(85),
            ),
            labeled(
                "Maximum parts",
                pick_list(
                    MAX_COMPONENT_OPTIONS,
                    Some(state.max_components),
                    CalculatorMessage::MaxComponentsChanged,
                )
                .text_size(11)
                .padding([4, 6])
                .width(70),
            ),
            button(text("Calculate").size(11))
                .padding([4, 10])
                .on_press(CalculatorMessage::Calculate),
        ]
        .spacing(12)
        .align_y(Alignment::End);

        let mut content = column![
            text("Passive Network Calculator")
                .size(16)
                .color(token_color(tokens.text)),
            text("Find a bounded preferred-value series/parallel realization.")
                .size(11)
                .color(token_color(tokens.text_secondary)),
            tabs,
            target_row,
        ]
        .spacing(14)
        .padding(12);

        if let Some(error) = &state.validation_error {
            content = content.push(
                text(error)
                    .size(11)
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
                .padding(12)
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
        // GH #599 — never substitute 0.0 for a target that cannot be
        // read. `unwrap_or_default()` here silently turned "Target" into
        // 0 and computed all three deltas against 0, next to Nominal /
        // Minimum / Maximum still showing the real network. Clearing
        // `result` on every target edit makes that unreachable; keeping
        // the `Err` explicit here means it can never come back silently.
        let target = self.target_value().ok();
        let nominal = result.nominal(self.kind);
        let minimum = result.minimum(self.kind);
        let maximum = result.maximum(self.kind);

        let summary = column![
            text(result.expression(self.kind))
                .size(14)
                .color(token_color(tokens.text)),
            row![
                metric("Target", format_target(target, self.kind), tokens),
                metric("Nominal", format_value(nominal, self.kind), tokens),
                metric("Minimum", format_value(minimum, self.kind), tokens),
                metric("Maximum", format_value(maximum, self.kind), tokens),
                metric(
                    "Nominal Δ",
                    format_optional_difference(nominal, target),
                    tokens
                ),
                metric(
                    "Lower Δ",
                    format_optional_difference(minimum, target),
                    tokens
                ),
                metric(
                    "Upper Δ",
                    format_optional_difference(maximum, target),
                    tokens
                ),
            ]
            .spacing(18),
        ]
        .spacing(9);

        let components = result.components();
        let mut component_cards = column![].spacing(10);
        if components.is_empty() {
            component_cards = component_cards.push(
                text("No discrete component is required.")
                    .size(11)
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
            .padding(12)
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
    button(text(label).size(11))
        .padding([4, 10])
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
    column![text(label).size(11), control.into()]
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
            .size(10)
            .color(token_color(tokens.text_secondary)),
        text(value).size(11).color(token_color(tokens.text)),
    ]
    .spacing(2)
    .into()
}

/// GH #599 — what a target-dependent metric reads when the target
/// itself cannot be read. A dash says "not available"; a `0` would have
/// read as a real, satisfied value.
pub const TARGET_UNAVAILABLE: &str = "—";

/// The "Target" metric, or [`TARGET_UNAVAILABLE`] when the target input
/// is not a number.
pub fn format_target(target: Option<f64>, kind: ComponentKind) -> String {
    target.map_or_else(
        || TARGET_UNAVAILABLE.to_string(),
        |target| format_value(target, kind),
    )
}

/// A delta against the target, or [`TARGET_UNAVAILABLE`] when there is
/// no readable target to measure against.
pub fn format_optional_difference(value: f64, target: Option<f64>) -> String {
    target.map_or_else(
        || TARGET_UNAVAILABLE.to_string(),
        |target| format_difference(value, target),
    )
}

pub fn format_difference(value: f64, target: f64) -> String {
    if value == target {
        return "0 (0%)".to_string();
    }
    if target == 0.0 || target == f64::INFINITY {
        return "unbounded".to_string();
    }
    let difference = value - target;
    let percentage = difference / target * 100.0;
    let percentage_sign = if percentage > 0.0 { "+" } else { "" };
    format!(
        "{} ({percentage_sign}{}%)",
        format_number(difference.abs()),
        format_number(percentage),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A control that has just produced a network for target `100`.
    fn calculated() -> CalculatorControl {
        let mut control = CalculatorControl::default();
        control.update(CalculatorMessage::TargetChanged("100".into()));
        control.update(CalculatorMessage::Calculate);
        assert!(
            control.active_state().result.is_some(),
            "setup: Calculate must produce a network for target 100"
        );
        control
    }

    // GH #599 — the result used to survive an edit that left the target
    // unreadable, and the summary then rendered against `target = 0.0`:
    // "Target" read 0 and all three deltas were measured against 0 while
    // Nominal / Minimum / Maximum still showed the real network. No
    // error text either, because TargetChanged clears validation_error.
    #[test]
    fn an_unreadable_target_edit_drops_the_result_it_was_solved_for() {
        let mut control = calculated();
        control.update(CalculatorMessage::TargetChanged("22o".into()));
        assert!(
            control.active_state().result.is_none(),
            "a result outlived the target it was solved for"
        );
    }

    #[test]
    fn clearing_the_target_drops_the_result() {
        let mut control = calculated();
        control.update(CalculatorMessage::TargetChanged(String::new()));
        assert!(control.active_state().result.is_none());
    }

    // Mid-typing states are the same shape: `2.` is not yet a number.
    #[test]
    fn a_partial_target_drops_the_result() {
        let mut control = calculated();
        control.update(CalculatorMessage::TargetChanged("2.".into()));
        assert!(control.active_state().result.is_none());
    }

    // A readable edit must still clear the stale network — the siblings
    // (PrefixChanged / SeriesChanged / MaxComponentsChanged) all do.
    #[test]
    fn a_readable_target_edit_also_drops_the_stale_result() {
        let mut control = calculated();
        control.update(CalculatorMessage::TargetChanged("220".into()));
        assert!(control.active_state().result.is_none());
        control.update(CalculatorMessage::Calculate);
        assert!(control.active_state().result.is_some());
    }

    // GH #599 — a missing target must never render as a real `0`.
    #[test]
    fn target_dependent_metrics_read_unavailable_without_a_target() {
        assert_eq!(
            format_target(None, ComponentKind::Resistor),
            TARGET_UNAVAILABLE
        );
        assert_eq!(format_optional_difference(120.0, None), TARGET_UNAVAILABLE);
        assert_eq!(
            format_optional_difference(100.0, Some(100.0)),
            format_difference(100.0, 100.0)
        );
    }
}
