//! Bench readings: `#measurement(...)` (mockup row 199). One call records
//! one reading of a part, pin or net against its limit; a call with a body
//! is a block that holds several readings, which take the block's id and
//! whatever of its instrument, conditions, operator, time, revision and
//! board they do not set. The block prints as a verdict table (Target,
//! Tol., Measured, Error, Result); readings on the same target form a
//! timeline ([`timeline`]). The rules are in `docs/NOTEBOOK_MEASUREMENTS.md`.

pub(crate) mod read;
mod saved;
mod timeline;

pub use read::{MeasurementProblem, MeasurementProblemKind, readings};
pub use saved::{ReadingKey, changed_saved_readings};
pub use timeline::{CSV_HEADER, Timeline, TimelineEntry, quantities_of, timeline};

use crate::id::BlockId;
use crate::reference::Reference;
use crate::si::{MINUS, THIN_SPACE, decimals_for, fixed};
use crate::timestamp::Timestamp;

/// A reading inside its tolerance but closer than this share of the
/// reference value to a limit is a warning ("Warn = inside tolerance,
/// under 1 % from the limit", mockup row 60).
pub const WARN_MARGIN: f64 = 0.01;
/// The significant digits of the Error column.
pub const ERROR_DIGITS: u32 = 2;

/// What a reading is held against.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Limit {
    /// `limit: (max: 30)`: at most.
    Max { max: f64 },
    /// `limit: (min: 90)`: at least.
    Min { min: f64 },
    /// `limit: (min: 2.9, max: 3.4)`: between.
    Range { min: f64, max: f64 },
    /// `nominal: 3.3, tolerance: 2%` (or an absolute tolerance in the
    /// reading's unit, `tolerance: 0.05`).
    Nominal { nominal: f64, tolerance: Tolerance },
}

/// The tolerance around a nominal value.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Tolerance {
    /// Percent of the nominal value: `2%` is `Percent(2.0)`.
    Percent(f64),
    /// In the reading's unit.
    Absolute(f64),
}

/// The outcome of one reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    /// Inside the limit, within [`WARN_MARGIN`] of it.
    Warn,
    Fail,
}

impl Verdict {
    /// The word the table prints: `Pass`, `Warn`, `Fail`.
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Pass => "Pass",
            Verdict::Warn => "Warn",
            Verdict::Fail => "Fail",
        }
    }
}

/// The bounds a limit sets and the value its error is taken against.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bounds {
    low: Option<f64>,
    high: Option<f64>,
    reference: f64,
}

impl Limit {
    fn bounds(self) -> Bounds {
        match self {
            Limit::Max { max } => Bounds {
                low: None,
                high: Some(max),
                reference: max,
            },
            Limit::Min { min } => Bounds {
                low: Some(min),
                high: None,
                reference: min,
            },
            Limit::Range { min, max } => Bounds {
                low: Some(min),
                high: Some(max),
                reference: (min + max) / 2.0,
            },
            Limit::Nominal { nominal, tolerance } => {
                let band = match tolerance {
                    Tolerance::Percent(percent) => nominal * percent / 100.0,
                    Tolerance::Absolute(band) => band,
                }
                .abs();
                Bounds {
                    low: Some(nominal - band),
                    high: Some(nominal + band),
                    reference: nominal,
                }
            }
        }
    }

    /// The Target column: `3.3 V`, `< 30 mV`, `≥ 90 %`, `2.9–3.4 V`.
    pub fn target_text(self, unit: &str) -> String {
        let with_unit = |value: f64| quantity_text(&plain(value), unit);
        match self {
            Limit::Max { max } => format!("< {}", with_unit(max)),
            Limit::Min { min } => format!("≥ {}", with_unit(min)),
            Limit::Range { min, max } => format!("{}–{}", plain(min), with_unit(max)),
            Limit::Nominal { nominal, .. } => with_unit(nominal),
        }
    }

    /// The Tol. column: `±2 %`, `±0.05 V`, `max`, `min`, `range`.
    pub fn tolerance_text(self, unit: &str) -> String {
        match self {
            Limit::Max { .. } => "max".to_string(),
            Limit::Min { .. } => "min".to_string(),
            Limit::Range { .. } => "range".to_string(),
            Limit::Nominal {
                tolerance: Tolerance::Percent(percent),
                ..
            } => format!("±{}", quantity_text(&plain(percent), "%")),
            Limit::Nominal {
                tolerance: Tolerance::Absolute(band),
                ..
            } => format!("±{}", quantity_text(&plain(band), unit)),
        }
    }
}

/// The error and the verdict of a reading.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Check {
    /// `(value - reference) / |reference|`; `None` when the reference is 0.
    pub error: Option<f64>,
    pub verdict: Verdict,
}

/// `value` held against `limit`. Out of the bounds fails; inside, closer
/// than [`WARN_MARGIN`] of the reference value to a bound warns.
pub fn check(value: f64, limit: Limit) -> Check {
    let bounds = limit.bounds();
    let scale = bounds.reference.abs();
    let error = (scale > 0.0).then(|| (value - bounds.reference) / scale);
    let fails =
        bounds.low.is_some_and(|low| value < low) || bounds.high.is_some_and(|high| value > high);
    let margin = [
        bounds.low.map(|low| value - low),
        bounds.high.map(|high| high - value),
    ]
    .into_iter()
    .flatten()
    .fold(f64::INFINITY, f64::min);
    let verdict = if fails {
        Verdict::Fail
    } else if scale > 0.0 && margin < WARN_MARGIN * scale {
        Verdict::Warn
    } else {
        Verdict::Pass
    };
    Check { error, verdict }
}

/// The Error column for `error` (a share): two significant digits, a sign,
/// a thin space and `%`: `+0.30 %`, `−40 %`, `+1.3 %`, `0 %`.
pub fn error_text(error: f64) -> String {
    let percent = error * 100.0;
    let magnitude = percent.abs();
    if magnitude == 0.0 || !magnitude.is_finite() {
        return format!("0{THIN_SPACE}%");
    }
    let digits = fixed(magnitude, decimals_for(magnitude, ERROR_DIGITS));
    let sign = if percent < 0.0 { MINUS } else { '+' };
    format!("{sign}{digits}{THIN_SPACE}%")
}

/// `number` and `unit` with a thin space between; the number alone with no
/// unit.
pub fn quantity_text(number: &str, unit: &str) -> String {
    if unit.is_empty() {
        number.to_string()
    } else {
        format!("{number}{THIN_SPACE}{unit}")
    }
}

/// A number with the digits it needs, a minus sign for a negative one.
fn plain(value: f64) -> String {
    let text = format!("{value}");
    match text.strip_prefix('-') {
        Some(rest) => format!("{MINUS}{rest}"),
        None => text,
    }
}

/// One bench reading.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Reading {
    /// The notebook file it is in, as a path inside `notebook/`; the
    /// reader of one source leaves it empty.
    pub file: String,
    /// Its own id, or the id of the block it sits in.
    pub id: Option<BlockId>,
    /// 1-based row in its block; 1 for a reading on its own.
    pub row: u32,
    /// What was measured: one or more parts, pins and nets.
    pub targets: Vec<Reference>,
    /// `ripple`, `DC`, `efficiency`; empty when not named.
    pub quantity: String,
    /// The value as written (`3.310` keeps its zero) and as a number.
    pub value: Option<f64>,
    pub value_text: String,
    pub unit: String,
    pub limit: Option<Limit>,
    /// The calculated value the timeline draws beside the readings.
    pub computed: Option<f64>,
    pub conditions: String,
    pub instrument: String,
    pub operator: String,
    pub timestamp: Option<Timestamp>,
    /// The design revision and the board serial it was taken on.
    pub rev: String,
    pub board: String,
    /// The attachment it was taken with, a path inside
    /// `notebook/attachments/`.
    pub attach: String,
    /// The reading it corrects: saved readings are read-only.
    pub supersedes: Option<BlockId>,
    /// The requirement ids it shows were met.
    pub requirements: Vec<String>,
    /// The part values it was taken with (designator, value).
    pub taken_with: Vec<(String, String)>,
    /// The 1-based line of the call.
    pub line: u32,
}

impl Reading {
    /// The error and verdict, when the reading has a value and a limit.
    pub fn check(&self) -> Option<Check> {
        Some(check(self.value?, self.limit?))
    }

    /// The Quantity column: the quantity, else the names of the targets.
    pub fn quantity_label(&self) -> String {
        if !self.quantity.is_empty() {
            return self.quantity.clone();
        }
        let names: Vec<String> = self.targets.iter().map(Reference::name).collect();
        names.join(", ")
    }

    /// Whether the reading is of `reference`: a net of that net, a pin of
    /// that pin, a part of that part or one of its pins.
    pub fn is_of(&self, reference: &Reference) -> bool {
        self.targets.iter().any(|target| match (reference, target) {
            (Reference::Component(part), Reference::Pin { component, .. }) => part == component,
            _ => target == reference,
        })
    }
}
