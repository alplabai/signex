//! A capture as a scope screen: 10 x 8 divisions, a 1-2-5 scale per
//! division, the traces, the cursors and the trigger marker. The plot is
//! an SVG of lines only, drawn as vectors in the PDF; its labels (axis
//! numbers and units, the channel legend, the readout) are laid out by the
//! notebook package in the page's own fonts from [`ScopePlot`]'s fields.

use std::fmt::Write as _;

use super::{Coupling, PlotSpec, ScopeCapture};
use crate::si;

/// Divisions across and up the screen.
pub const DIVISIONS_X: u32 = 10;
pub const DIVISIONS_Y: u32 = 8;
/// SVG units per division.
const DIV: f64 = 40.0;
/// The most points one trace is drawn with: two per SVG unit across.
const MAX_POINTS: usize = 800;
/// The divisions an automatic scale leaves the trace to fill.
const FILL_DIVISIONS: f64 = 6.0;
/// Where the Y axis is labelled, in divisions from the centre.
const Y_LABEL_DIVISIONS: [f64; 3] = [3.0, 0.0, -3.0];
/// The print colours of the plot. The page is always white
/// (`docs/NOTEBOOK_MEASUREMENTS.md`, "Printing"), so they are fixed
/// rather than taken from the app theme.
const PAPER: &str = "#ffffff";
const GRID: &str = "#d4d4d4";
const AXIS: &str = "#8c8c8c";
const FRAME: &str = "#5a5a5a";
const CURSOR: &str = "#6b4fa0";
const TRIGGER: &str = "#b25d00";
pub(crate) const TRACES: [&str; 4] = ["#a87a00", "#1f6fa8", "#a8326a", "#2b7a3d"];

/// One channel as drawn.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PlotChannel {
    pub name: String,
    pub label: String,
    pub unit: String,
    pub coupling: Coupling,
    /// The channel's unit per division.
    pub per_div: f64,
    /// The value at the centre line, before AC coupling takes it away.
    #[serde(skip)]
    pub centre: f64,
    pub bandwidth: String,
    /// `CH1 +3V3 AC · 5 mV/div · 20 MHz BW`.
    pub legend: String,
    pub colour: &'static str,
    /// In divisions: x from 0 to 10, y from -4 to 4 around the centre.
    #[serde(skip)]
    pub points: Vec<(f64, f64)>,
}

/// One cursor line.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PlotCursor {
    /// `Y1`, `Y2`, `X1`, `X2`.
    pub label: String,
    /// In divisions: from the centre for Y, from the left for X.
    pub at: f64,
}

/// One axis number.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AxisLabel {
    /// In divisions: from the left for X, from the centre for Y.
    pub at: f64,
    pub text: String,
}

/// A capture laid out on the screen.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ScopePlot {
    /// The time per division, `1 µs/div`.
    pub time_per_div: String,
    pub channels: Vec<PlotChannel>,
    pub cursors: Vec<PlotCursor>,
    /// The trigger's position in divisions from the left, when on screen.
    pub trigger: Option<f64>,
    /// `ΔY = 18 mV p-p`, `f = 500 kHz`.
    pub readout: Vec<String>,
    pub x_labels: Vec<AxisLabel>,
    pub y_labels: Vec<AxisLabel>,
}

/// Lay `capture` out as `spec` asks; a scale not given is the 1-2-5 step
/// that fits the samples.
pub fn plot(capture: &ScopeCapture, spec: &PlotSpec) -> ScopePlot {
    let start = capture.times.first().copied().unwrap_or(0.0);
    let end = capture.times.last().copied().unwrap_or(start);
    let time_per_div = spec
        .time_scale
        .unwrap_or_else(|| si::step_125((end - start) / f64::from(DIVISIONS_X)));
    let x_of = |time: f64| (time - start) / time_per_div;
    let channels: Vec<PlotChannel> = capture
        .channels
        .iter()
        .enumerate()
        .map(|(index, trace)| {
            let set = spec
                .channels
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(&trace.name))
                .or_else(|| spec.channels.get(index).filter(|c| c.name.is_empty()));
            let coupling = set.map(|c| c.coupling).unwrap_or_default();
            let (low, high) = extent(&trace.samples);
            let centre = match coupling {
                Coupling::Ac => mean(&trace.samples),
                Coupling::Dc => 0.0,
            };
            let reach = match coupling {
                Coupling::Ac => (high - low) / FILL_DIVISIONS,
                Coupling::Dc => low.abs().max(high.abs()) / (FILL_DIVISIONS / 2.0),
            };
            let per_div = set
                .and_then(|c| c.scale)
                .unwrap_or_else(|| si::step_125(reach));
            let label = set.map(|c| c.label.clone()).unwrap_or_default();
            let bandwidth = set.map(|c| c.bandwidth.clone()).unwrap_or_default();
            let points = decimate(&capture.times, &trace.samples)
                .into_iter()
                .map(|(t, v)| (x_of(t), (v - centre) / per_div))
                .collect();
            PlotChannel {
                legend: legend(
                    &trace.name,
                    &label,
                    coupling,
                    per_div,
                    &trace.unit,
                    &bandwidth,
                ),
                name: trace.name.clone(),
                label,
                unit: trace.unit.clone(),
                coupling,
                per_div,
                centre,
                bandwidth,
                colour: TRACES[index % TRACES.len()],
                points,
            }
        })
        .collect();
    let first = channels.first();
    let (per_div, unit) = first.map_or((1.0, "V"), |c| (c.per_div, c.unit.as_str()));
    let mut cursors = Vec::new();
    let mut readout = Vec::new();
    let c = spec.cursors;
    for (label, value) in [("Y1", c.y1), ("Y2", c.y2)] {
        if let Some(value) = value {
            cursors.push(PlotCursor {
                label: label.to_string(),
                at: value / per_div,
            });
        }
    }
    for (label, value) in [("X1", c.x1), ("X2", c.x2)] {
        if let Some(value) = value {
            cursors.push(PlotCursor {
                label: label.to_string(),
                at: x_of(value),
            });
        }
    }
    if let (Some(y1), Some(y2)) = (c.y1, c.y2) {
        readout.push(format!("ΔY = {} p-p", si::format((y1 - y2).abs(), unit, 3)));
    }
    if let (Some(x1), Some(x2)) = (c.x1, c.x2) {
        let span = (x2 - x1).abs();
        readout.push(format!("ΔX = {}", si::format(span, &capture.time_unit, 3)));
        if span > 0.0 {
            readout.push(format!("f = {}", si::format(1.0 / span, "Hz", 3)));
        }
    }
    let trigger_time = spec
        .trigger
        .map(|t| t.time)
        .or_else(|| (start <= 0.0 && 0.0 <= end && start < end).then_some(0.0));
    let trigger = trigger_time
        .map(x_of)
        .filter(|x| (0.0..=f64::from(DIVISIONS_X)).contains(x));
    ScopePlot {
        time_per_div: format!("{}/div", si::format(time_per_div, &capture.time_unit, 3)),
        x_labels: axis_labels(
            (0..=DIVISIONS_X / 2).map(|i| f64::from(i * 2)),
            time_per_div,
            &capture.time_unit,
        ),
        y_labels: axis_labels(Y_LABEL_DIVISIONS, per_div, unit),
        channels,
        cursors,
        trigger,
        readout,
    }
}

impl ScopePlot {
    /// The screen as SVG: paper, grid, centre lines, frame, traces clipped
    /// to the screen, dashed cursors and the trigger marker. No text, so
    /// it draws the same with any fonts; numbers are written with two
    /// decimals, so the same capture gives the same bytes.
    pub fn to_svg(&self) -> String {
        let (width, height) = (DIV * f64::from(DIVISIONS_X), DIV * f64::from(DIVISIONS_Y));
        let mut svg = String::new();
        let _ = write!(
            svg,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" \
             width=\"{width}\" height=\"{height}\">\
             <defs><clipPath id=\"screen\"><rect width=\"{width}\" height=\"{height}\"/></clipPath></defs>\
             <rect width=\"{width}\" height=\"{height}\" fill=\"{PAPER}\"/>"
        );
        let mut grid = String::new();
        for i in 1..DIVISIONS_X {
            let _ = write!(grid, "M{:.2} 0V{height}", DIV * f64::from(i));
        }
        for i in 1..DIVISIONS_Y {
            let _ = write!(grid, "M0 {:.2}H{width}", DIV * f64::from(i));
        }
        let _ = write!(
            svg,
            "<path d=\"{grid}\" stroke=\"{GRID}\" stroke-width=\"0.5\" fill=\"none\"/>\
             <path d=\"M{:.2} 0V{height}M0 {:.2}H{width}\" stroke=\"{AXIS}\" stroke-width=\"0.8\" fill=\"none\"/>",
            width / 2.0,
            height / 2.0
        );
        for channel in &self.channels {
            let points: Vec<String> = channel
                .points
                .iter()
                .map(|&(x, y)| format!("{:.2},{:.2}", x * DIV, svg_y(y)))
                .collect();
            let _ = write!(
                svg,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1\" \
                 stroke-linejoin=\"round\" clip-path=\"url(#screen)\"/>",
                points.join(" "),
                channel.colour
            );
        }
        for cursor in &self.cursors {
            let line = if cursor.label.starts_with('Y') {
                let y = svg_y(cursor.at);
                format!("M0 {y:.2}H{width}")
            } else {
                format!("M{:.2} 0V{height}", cursor.at * DIV)
            };
            let _ = write!(
                svg,
                "<path d=\"{line}\" stroke=\"{CURSOR}\" stroke-width=\"0.8\" \
                 stroke-dasharray=\"4 3\" fill=\"none\" clip-path=\"url(#screen)\"/>"
            );
        }
        if let Some(x) = self.trigger {
            let x = x * DIV;
            let _ = write!(
                svg,
                "<path d=\"M{:.2} 0L{:.2} 0L{x:.2} 7Z\" fill=\"{TRIGGER}\"/>",
                x - 5.0,
                x + 5.0
            );
        }
        let _ = write!(
            svg,
            "<rect width=\"{width}\" height=\"{height}\" fill=\"none\" stroke=\"{FRAME}\" stroke-width=\"1\"/></svg>"
        );
        svg
    }
}

/// A y in divisions from the centre as an SVG y, which grows downwards.
fn svg_y(divisions: f64) -> f64 {
    (f64::from(DIVISIONS_Y) / 2.0 - divisions) * DIV
}

/// `CH1 +3V3 AC · 5 mV/div · 20 MHz BW`.
fn legend(
    name: &str,
    label: &str,
    coupling: Coupling,
    per_div: f64,
    unit: &str,
    bandwidth: &str,
) -> String {
    let mut head = name.to_string();
    if !label.is_empty() {
        head = format!("{head} {label}");
    }
    let mut parts = vec![
        format!("{head} {}", coupling.label()),
        format!("{}/div", si::format(per_div, unit, 3)),
    ];
    if !bandwidth.is_empty() {
        parts.push(format!("{bandwidth} BW"));
    }
    parts.join(" · ")
}

/// Axis numbers at `divisions` for `per_div` of `unit`: the numbers in one
/// prefix, the unit after the last one only (`0 2 4 6 8 10 µs`).
fn axis_labels(
    divisions: impl IntoIterator<Item = f64>,
    per_div: f64,
    unit: &str,
) -> Vec<AxisLabel> {
    let divisions: Vec<f64> = divisions.into_iter().collect();
    let largest = divisions
        .iter()
        .map(|d| (d * per_div).abs())
        .fold(0.0, f64::max);
    let (factor, prefix) = si::prefix_for(largest);
    let last = divisions.len().saturating_sub(1);
    divisions
        .iter()
        .enumerate()
        .map(|(i, &at)| {
            let number = si::number(at * per_div / factor, 3);
            let text = if i == last {
                format!("{number}{}{prefix}{unit}", si::THIN_SPACE)
            } else {
                number
            };
            AxisLabel { at, text }
        })
        .collect()
}

fn extent(samples: &[f64]) -> (f64, f64) {
    samples
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), &v| {
            (low.min(v), high.max(v))
        })
}

fn mean(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.iter().sum::<f64>() / samples.len() as f64
}

/// At most [`MAX_POINTS`] points: the samples in order, or for a longer
/// trace the lowest and the highest of each of `MAX_POINTS / 2` runs, in
/// the order they came, so a spike is never dropped.
fn decimate(times: &[f64], samples: &[f64]) -> Vec<(f64, f64)> {
    let count = times.len().min(samples.len());
    if count <= MAX_POINTS {
        return times.iter().zip(samples).map(|(&t, &v)| (t, v)).collect();
    }
    let runs = MAX_POINTS / 2;
    let mut points = Vec::with_capacity(MAX_POINTS);
    for run in 0..runs {
        let (from, to) = (run * count / runs, (run + 1) * count / runs);
        let (low, high) = extremes(&samples[from..to]);
        let (a, b) = (from + low.min(high), from + low.max(high));
        points.push((times[a], samples[a]));
        if b != a {
            points.push((times[b], samples[b]));
        }
    }
    points
}

/// The indices of the lowest and the highest sample, first found first.
fn extremes(samples: &[f64]) -> (usize, usize) {
    samples
        .iter()
        .enumerate()
        .fold((0, 0), |(low, high), (i, &v)| {
            let low = if v < samples[low] { i } else { low };
            let high = if v > samples[high] { i } else { high };
            (low, high)
        })
}
