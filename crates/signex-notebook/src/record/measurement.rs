//! A Measurement's rows and the verdict of a value against its limits:
//! Pass inside them, Warn inside but under 1 % from a limit, Fail outside
//! (mockup row 60b, settled 2026-11-25).

use super::kinds::{Reader, band_of};
use super::value::FieldValue;
use crate::reference::Reference;

/// How close to a limit, as a share of the reference value, a value inside
/// it warns.
pub const VERDICT_WARN_MARGIN: f64 = 0.01;

/// The verdict of a value against its limits; orders Pass, Warn, Fail, so
/// the worst of several is their maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
}

impl Verdict {
    pub const ALL: [Verdict; 3] = [Verdict::Pass, Verdict::Warn, Verdict::Fail];

    /// The token a JSON result and the printed badge use.
    pub fn token(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Warn => "warn",
            Verdict::Fail => "fail",
        }
    }
}

/// The lowest and highest value allowed; either may be open.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct Limits {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Limits {
    /// The band `nominal` ± `tolerance`, a percentage of it when
    /// `percent`. `None` for a negative tolerance.
    pub fn around(nominal: f64, tolerance: f64, percent: bool) -> Option<Self> {
        let half = if percent {
            nominal.abs() * tolerance / 100.0
        } else {
            tolerance
        };
        (half >= 0.0).then_some(Self {
            min: Some(nominal - half),
            max: Some(nominal + half),
        })
    }

    /// `(min: 1.0, max: 2.5)`, either key optional, at least one given.
    pub(crate) fn from_dict(value: &FieldValue) -> Option<Self> {
        let FieldValue::Dict(entries) = value else {
            return None;
        };
        let mut limits = Limits::default();
        for (key, value) in entries {
            let (number, _) = value.as_number()?;
            match key.as_str() {
                "min" => limits.min = Some(number),
                "max" => limits.max = Some(number),
                _ => return None,
            }
        }
        (limits.min.is_some() || limits.max.is_some()).then_some(limits)
    }

    /// The one limit of an open band (`max: 250`), for the error of a
    /// value against it.
    fn single(self) -> Option<f64> {
        match (self.min, self.max) {
            (Some(limit), None) | (None, Some(limit)) => Some(limit),
            _ => None,
        }
    }
}

/// The verdict of `value` against `limits`: inside them but closer to one
/// than [`VERDICT_WARN_MARGIN`] of the reference value (the middle of a
/// band, else its one limit) warns, as the timeline and the PDF judge it
/// (DECISIONS row 776, #3021). A reference value of zero has no warning
/// band.
pub fn verdict_of(value: f64, limits: Limits) -> Verdict {
    let below = limits.min.is_some_and(|min| value < min);
    let above = limits.max.is_some_and(|max| value > max);
    if below || above {
        return Verdict::Fail;
    }
    let reference = match (limits.min, limits.max) {
        (Some(min), Some(max)) => (min + max) / 2.0,
        (Some(limit), None) | (None, Some(limit)) => limit,
        (None, None) => return Verdict::Pass,
    };
    let scale = reference.abs();
    let margin = [
        limits.min.map(|min| value - min),
        limits.max.map(|max| max - value),
    ]
    .into_iter()
    .flatten()
    .fold(f64::INFINITY, f64::min);
    if scale > 0.0 && margin < VERDICT_WARN_MARGIN * scale {
        Verdict::Warn
    } else {
        Verdict::Pass
    }
}

/// One measured quantity.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MeasurementRow {
    /// What it names when it is not a part, pin or net: `"VIN ripple"`.
    pub quantity: Option<String>,
    pub target: Option<Reference>,
    pub nominal: Option<f64>,
    pub limits: Option<Limits>,
    pub measured: f64,
    /// The error against the nominal, else against the one limit, in
    /// percent.
    pub error_percent: Option<f64>,
    pub verdict: Option<Verdict>,
}

impl MeasurementRow {
    fn new(
        what: Option<&FieldValue>,
        nominal: Option<f64>,
        limits: Option<Limits>,
        measured: f64,
    ) -> Self {
        let base = nominal.or_else(|| limits.and_then(Limits::single));
        let error_percent = base
            .filter(|base| *base != 0.0)
            .map(|base| (measured - base) / base.abs() * 100.0);
        Self {
            quantity: what.and_then(FieldValue::as_text).map(str::to_string),
            target: match what {
                Some(FieldValue::Reference(reference)) => Some(reference.clone()),
                _ => None,
            },
            nominal,
            limits,
            measured,
            error_percent,
            verdict: limits.map(|limits| verdict_of(measured, limits)),
        }
    }
}

/// The rows of a Measurement: `rows: ((net("VBUS"), 5.00, 5%, 4.78), ...)`
/// (what, nominal, tolerance, measured; or what, `(max: ..)`, measured), or
/// the one row of a reading: `target:`, `quantity:` and `value:`, judged
/// against `limit:` or against `nominal:` and `tolerance:` (the reading
/// form of mockup row 199).
pub(super) fn measurement_rows(reader: &mut Reader) -> Vec<MeasurementRow> {
    let target = reader.value("target");
    let quantity = reader.text("quantity");
    let nominal = reader.value("nominal");
    let tolerance = reader.value("tolerance");
    let mut rows = Vec::new();
    if let Some(measured) = reader.number("value") {
        let (row_nominal, band) = match (nominal, tolerance) {
            (Some(nominal), Some(tolerance)) => {
                let pair = FieldValue::Array(vec![nominal.clone(), tolerance.clone()]);
                match band_of(&pair) {
                    Some((nominal, limits)) => (Some(nominal), Some(limits)),
                    None => {
                        reader.bad("tolerance");
                        (None, None)
                    }
                }
            }
            (Some(nominal), None) => match nominal.as_number() {
                Some((nominal, _)) => (Some(nominal), None),
                None => {
                    reader.bad("nominal");
                    (None, None)
                }
            },
            _ => (None, None),
        };
        let limits = reader.limits("limit").or(band);
        let mut row = MeasurementRow::new(target, row_nominal, limits, measured);
        if quantity.is_some() {
            row.quantity = quantity;
        }
        rows.push(row);
    }
    let Some(value) = reader.value("rows") else {
        return rows;
    };
    let read: Option<Vec<MeasurementRow>> = value.items().into_iter().map(row_of).collect();
    match read {
        Some(read) => rows.extend(read),
        None => reader.bad("rows"),
    }
    rows
}

fn row_of(item: &FieldValue) -> Option<MeasurementRow> {
    let FieldValue::Array(cells) = item else {
        return None;
    };
    match cells.as_slice() {
        [what, nominal, tolerance, measured] => {
            let pair = FieldValue::Array(vec![nominal.clone(), tolerance.clone()]);
            let (nominal, limits) = band_of(&pair)?;
            let (measured, _) = measured.as_number()?;
            Some(MeasurementRow::new(
                Some(what),
                Some(nominal),
                Some(limits),
                measured,
            ))
        }
        [what, limits, measured] => {
            let limits = Limits::from_dict(limits)?;
            let (measured, _) = measured.as_number()?;
            Some(MeasurementRow::new(
                Some(what),
                None,
                Some(limits),
                measured,
            ))
        }
        _ => None,
    }
}
