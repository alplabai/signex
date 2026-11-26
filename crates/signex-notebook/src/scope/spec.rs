//! How an `#attachment` asks for its scope capture to be drawn: per
//! channel coupling, scale and bandwidth limit as the scope was set, the
//! time scale, the cursors and the trigger. Every value is optional; what
//! is not given is chosen from the samples.

use crate::literal::{FoundCall, Literal};
use crate::si;

/// How a channel was coupled; AC is drawn around its mean.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub enum Coupling {
    #[default]
    #[serde(rename = "DC")]
    Dc,
    #[serde(rename = "AC")]
    Ac,
}

impl Coupling {
    pub fn label(self) -> &'static str {
        match self {
            Coupling::Dc => "DC",
            Coupling::Ac => "AC",
        }
    }
}

/// One channel as the scope was set.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChannelSpec {
    /// The CSV column: `CH1`. Matched without case; an unnamed spec takes
    /// the column in its place.
    pub name: String,
    /// What it probed: `+3V3`.
    pub label: String,
    pub coupling: Coupling,
    /// Volts (or the channel's unit) per division.
    pub scale: Option<f64>,
    /// The bandwidth limit as written: `20 MHz`.
    pub bandwidth: String,
}

/// The cursor positions: Y in the channel's unit (as drawn, so around
/// zero for AC), X in the capture's time.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cursors {
    pub y1: Option<f64>,
    pub y2: Option<f64>,
    pub x1: Option<f64>,
    pub x2: Option<f64>,
}

/// Where the scope triggered, in the capture's time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trigger {
    pub time: f64,
}

/// Everything an attachment says about drawing its capture.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlotSpec {
    pub channels: Vec<ChannelSpec>,
    /// Seconds (or the time unit) per division.
    pub time_scale: Option<f64>,
    pub cursors: Cursors,
    pub trigger: Option<Trigger>,
}

impl PlotSpec {
    /// The spec of an `#attachment` call: `channels:`, `time-scale:`,
    /// `cursors:` and `trigger:`. A value is a number in the base unit or a
    /// string with an SI prefix (`"5 mV"`, `"1 µs"`); one that is neither
    /// is left to the samples.
    pub(crate) fn from_call(call: &FoundCall) -> Self {
        let channels = match call.arg("channels") {
            Some(Literal::Array(items)) => items.iter().map(channel).collect(),
            Some(one @ Literal::Dict(_)) => vec![channel(one)],
            _ => Vec::new(),
        };
        let cursors = call.arg("cursors");
        let at = |key: &str| cursors.and_then(|c| c.get(key)).and_then(amount);
        Self {
            channels,
            time_scale: call.arg("time-scale").and_then(amount),
            cursors: Cursors {
                y1: at("y1"),
                y2: at("y2"),
                x1: at("x1"),
                x2: at("x2"),
            },
            trigger: call
                .arg("trigger")
                .and_then(|t| t.get("time"))
                .and_then(amount)
                .map(|time| Trigger { time }),
        }
    }
}

fn channel(literal: &Literal) -> ChannelSpec {
    let text = |key: &str| {
        literal
            .get(key)
            .and_then(Literal::text)
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    };
    let coupling = match text("coupling").to_ascii_uppercase().as_str() {
        "AC" => Coupling::Ac,
        _ => Coupling::Dc,
    };
    ChannelSpec {
        name: text("name"),
        label: text("label"),
        coupling,
        scale: literal.get("scale").and_then(amount).filter(|s| *s > 0.0),
        bandwidth: text("bandwidth"),
    }
}

/// A number, or a string with an SI prefix and unit (`/div` dropped).
fn amount(literal: &Literal) -> Option<f64> {
    match literal {
        Literal::Number(value, _) => Some(*value),
        Literal::Str(text) => si::parse(text.trim_end_matches("/div")).map(|(value, _)| value),
        _ => None,
    }
}
