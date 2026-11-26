//! The passive calculator control: the network calculator for each
//! component kind and the RKM encoder, as plain data with a pure `update`.
//! The pages are drawn in `view.rs` (inputs) and `result_view.rs` (the
//! result: expression, status pill, metric strip, drawing and part cards).

mod result_view;
mod view;

#[cfg(test)]
mod tests;

use super::domain::{ComponentKind, ESeries, SiPrefix, Tolerance};
use super::network::{Network, format_number, format_value};
use super::rkm_encoder::{RkmEncoder, RkmEncoderMessage};
use super::solver::{MAX_PARTS, SolveOptions, solve};
use crate::property_illustration::{IllustrationHover, IllustrationMsg};

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
    /// The pointer moved onto or off a part in the network drawing.
    DrawingHovered(IllustrationMsg),
}

/// Why a target cannot be solved; the view words it from the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetError {
    /// The target is not a number, `∞` or `inf`.
    NotANumber,
    /// The target is negative, NaN or negative infinity.
    Negative,
    /// The solver found no network for these settings.
    NoResult,
}

#[derive(Debug, Clone)]
pub struct CalculatorControl {
    pub kind: ComponentKind,
    pub active_tab: CalculatorTab,
    pub modes: [CalculatorModeState; 3],
    pub rkm_encoder: RkmEncoder,
    /// The part under the pointer in the network drawing, if any.
    pub drawing_hover: IllustrationHover,
}

#[derive(Debug, Clone)]
pub struct CalculatorModeState {
    pub target_input: String,
    pub prefix: SiPrefix,
    pub series: ESeries,
    pub max_components: usize,
    pub result: Option<Network>,
    pub validation_error: Option<TargetError>,
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
            drawing_hover: IllustrationHover::default(),
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
            CalculatorMessage::TabChanged(tab) => {
                // A passive tab selects the kind whose state the edit arms
                // read through `active_state`; keep the two in step.
                if let CalculatorTab::Passive(kind) = tab {
                    self.kind = kind;
                }
                self.active_tab = tab;
            }
            CalculatorMessage::RkmEncoder(message) => self.rkm_encoder.update(message),
            CalculatorMessage::TargetChanged(value) => {
                let state = self.active_state_mut();
                state.target_input = value;
                state.validation_error = None;
                // GH #599: a result belongs to the target it was solved for;
                // a stale one rendered every target-dependent metric against
                // 0. Every sibling edit arm drops it too.
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
            CalculatorMessage::DrawingHovered(message) => self.drawing_hover.update(message),
        }
    }

    pub fn target_value(&self) -> Result<f64, TargetError> {
        let state = self.active_state();
        let normalized = state.target_input.trim().replace(',', ".");
        // Keyword parse (`inf`), not UI text: ASCII lowercase in every language.
        // ast-grep-ignore: no-case-in-view
        let value = match normalized.to_ascii_lowercase().as_str() {
            "∞" | "inf" | "infinity" => f64::INFINITY,
            _ => normalized
                .parse::<f64>()
                .map_err(|_| TargetError::NotANumber)?,
        };
        if value.is_nan() || value < 0.0 || value == f64::NEG_INFINITY {
            return Err(TargetError::Negative);
        }
        Ok(value * state.prefix.multiplier())
    }

    pub fn calculate(&mut self) {
        let target = match self.target_value() {
            Ok(value) => value,
            Err(error) => {
                let state = self.active_state_mut();
                state.result = None;
                state.validation_error = Some(error);
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
        state.validation_error = result.is_none().then_some(TargetError::NoResult);
        state.result = result;
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

/// The tone of the result's status pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffTargetTone {
    /// The nominal value is within the series tolerance of the target.
    Ok,
    /// The nominal value misses the target by more than the series
    /// tolerance.
    Warn,
}

/// How far the nominal value is off the target, in percent, and the pill
/// tone: ok while that is within the tolerance the series' parts are
/// solved at (E24: 5 %), warn past it. `None` when there is no readable
/// target, or the target is zero or infinite and the nominal value is not
/// (no percentage exists).
pub fn off_target(
    nominal: f64,
    target: Option<f64>,
    series: ESeries,
) -> Option<(f64, OffTargetTone)> {
    let target = target?;
    let percent = if nominal == target {
        0.0
    } else if target == 0.0 || target == f64::INFINITY {
        return None;
    } else {
        ((nominal - target) / target * 100.0).abs()
    };
    let limit = default_tolerance(series).fraction() * 100.0;
    let tone = if percent <= limit {
        OffTargetTone::Ok
    } else {
        OffTargetTone::Warn
    };
    Some((percent, tone))
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

/// Whether `value` has no percentage against `target`: the target is zero
/// or infinite and the value is not the target.
pub fn is_unbounded_difference(value: f64, target: f64) -> bool {
    value != target && (target == 0.0 || target == f64::INFINITY)
}

pub fn format_difference(value: f64, target: f64) -> String {
    if value == target {
        return "0 (0%)".to_string();
    }
    if is_unbounded_difference(value, target) {
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
