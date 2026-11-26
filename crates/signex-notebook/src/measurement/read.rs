//! Reading the `#measurement(...)` calls of one notebook file.

use crate::annotation::AnnotationKind;
use crate::id::BlockId;
use crate::literal::{FoundCall, Literal, calls_of};
use crate::measurement::{Limit, Reading, Tolerance};
use crate::syntax::typst_source;
use crate::timestamp::Timestamp;

/// The block function the readings are written with.
pub(crate) const FUNCTION: &str = "measurement";

/// Something in a `#measurement` call that could not be used. The reading
/// is still listed; only that field is left out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MeasurementProblem {
    /// The 1-based line of the call.
    pub line: u32,
    pub kind: MeasurementProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MeasurementProblemKind {
    /// `value:` is not a number as written (a computed value is not read).
    BadValue { written: String },
    /// `limit:` is not `(max: ..)`, `(min: ..)` or both, or `nominal:`
    /// came without a `tolerance:` (or the other way round).
    BadLimit { written: String },
    /// `timestamp:` (or `date:`) is not `YYYY-MM-DD` with an optional
    /// `THH:MM`.
    BadTimestamp { written: String },
    /// `id:` is not a `MEAS-` id.
    BadId { written: String },
    /// `supersedes:` is not a `MEAS-` id.
    BadSupersedes { written: String },
}

/// The readings of the notebook file text `source`, in source order, and
/// what could not be read. A block (a call with a body) gives the readings
/// inside it, each taking the block's id and the fields it does not set; a
/// block with its own `value:` is a reading too.
pub fn readings(source: &str) -> (Vec<Reading>, Vec<MeasurementProblem>) {
    let text = typst_source(source);
    let calls = calls_of(&text, FUNCTION);
    let mut problems = Vec::new();
    // Every call with what its blocks pass down; a block comes before the
    // calls inside it, so its entry is complete when they are read.
    let mut resolved: Vec<Reading> = Vec::with_capacity(calls.len());
    // The last row number used inside each block.
    let mut last_row = vec![0u32; calls.len()];
    let mut found = Vec::new();
    for (index, call) in calls.iter().enumerate() {
        let mut reading = fields(call, &mut problems);
        if let Some(block) = call.parent.and_then(|p| resolved.get(p)) {
            inherit(&mut reading, block);
        }
        if is_reading(call) {
            reading.row = match call.parent {
                Some(p) => {
                    last_row[p] += 1;
                    last_row[p]
                }
                None => 1,
            };
            // A block that is a reading itself is its own first row.
            last_row[index] = 1;
            found.push(reading.clone());
        }
        resolved.push(reading);
    }
    problems.sort_by_key(|p| p.line);
    (found, problems)
}

/// A call with a value, or one with no body, records a reading; a call
/// with a body and no value only holds readings.
fn is_reading(call: &FoundCall) -> bool {
    call.arg("value").is_some() || call.body().is_none()
}

/// The fields `call` sets, its problems pushed to `problems`.
fn fields(call: &FoundCall, problems: &mut Vec<MeasurementProblem>) -> Reading {
    let line = call.line;
    let mut problem = |kind| problems.push(MeasurementProblem { line, kind });
    let text = |name: &str| {
        call.arg(name)
            .and_then(Literal::text)
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    };
    let (value, value_text) = match call.arg("value") {
        Some(Literal::Number(value, written)) => (Some(*value), written.clone()),
        Some(other) => {
            problem(MeasurementProblemKind::BadValue {
                written: written(other),
            });
            (None, String::new())
        }
        None => (None, String::new()),
    };
    let id = call.arg("id").and_then(|literal| {
        let parsed = literal
            .text()
            .and_then(|t| BlockId::parse_for(AnnotationKind::Measurement, t.trim()));
        if parsed.is_none() {
            problem(MeasurementProblemKind::BadId {
                written: written(literal),
            });
        }
        parsed
    });
    let supersedes = call.arg("supersedes").and_then(|literal| {
        let parsed = literal
            .text()
            .and_then(|t| BlockId::parse_for(AnnotationKind::Measurement, t.trim()));
        if parsed.is_none() {
            problem(MeasurementProblemKind::BadSupersedes {
                written: written(literal),
            });
        }
        parsed
    });
    let stamp = call.arg("timestamp").or_else(|| call.arg("date"));
    let timestamp = stamp.and_then(|literal| {
        let parsed = literal.text().and_then(Timestamp::parse);
        if parsed.is_none() {
            problem(MeasurementProblemKind::BadTimestamp {
                written: written(literal),
            });
        }
        parsed
    });
    let limit = match limit_of(call) {
        Ok(limit) => limit,
        Err(written) => {
            problem(MeasurementProblemKind::BadLimit { written });
            None
        }
    };
    Reading {
        file: String::new(),
        id,
        row: 1,
        targets: call
            .arg("target")
            .map(Literal::references)
            .unwrap_or_default(),
        quantity: text("quantity"),
        value,
        value_text,
        unit: text("unit"),
        limit,
        computed: call.arg("computed").and_then(Literal::number),
        conditions: text("conditions"),
        instrument: text("instrument"),
        operator: text("operator"),
        timestamp,
        rev: text("rev"),
        board: text("board"),
        attach: text("attach"),
        supersedes,
        requirements: strings(call.arg("requirements")),
        taken_with: pairs(call.arg("taken-with")),
        line,
    }
}

/// The limit `call` sets, `None` when it sets none, `Err` with the text of
/// one it cannot read.
fn limit_of(call: &FoundCall) -> Result<Option<Limit>, String> {
    let nominal = call.arg("nominal");
    let tolerance = call.arg("tolerance");
    match (nominal, tolerance) {
        (Some(nominal), Some(tolerance)) => {
            let nominal_value = nominal.number().ok_or_else(|| written(nominal))?;
            let tolerance = match tolerance {
                Literal::Percent(percent) => Tolerance::Percent(*percent),
                Literal::Number(band, _) => Tolerance::Absolute(*band),
                other => return Err(written(other)),
            };
            return Ok(Some(Limit::Nominal {
                nominal: nominal_value,
                tolerance,
            }));
        }
        (Some(alone), None) | (None, Some(alone)) => return Err(written(alone)),
        (None, None) => {}
    }
    let Some(limit) = call.arg("limit") else {
        return Ok(None);
    };
    let min = limit
        .get("min")
        .map(|v| v.number().ok_or_else(|| written(limit)));
    let max = limit
        .get("max")
        .map(|v| v.number().ok_or_else(|| written(limit)));
    match (min.transpose()?, max.transpose()?) {
        (Some(min), Some(max)) if min <= max => Ok(Some(Limit::Range { min, max })),
        (Some(min), None) => Ok(Some(Limit::Min { min })),
        (None, Some(max)) => Ok(Some(Limit::Max { max })),
        _ => Err(written(limit)),
    }
}

/// What a block passes down to the readings inside it.
fn inherit(reading: &mut Reading, block: &Reading) {
    let take = |own: &mut String, from: &String| {
        if own.is_empty() {
            own.clone_from(from);
        }
    };
    if reading.id.is_none() {
        reading.id.clone_from(&block.id);
    }
    if reading.targets.is_empty() {
        reading.targets.clone_from(&block.targets);
    }
    take(&mut reading.conditions, &block.conditions);
    take(&mut reading.instrument, &block.instrument);
    take(&mut reading.operator, &block.operator);
    take(&mut reading.rev, &block.rev);
    take(&mut reading.board, &block.board);
    take(&mut reading.unit, &block.unit);
    if reading.timestamp.is_none() {
        reading.timestamp = block.timestamp;
    }
    if reading.requirements.is_empty() {
        reading.requirements.clone_from(&block.requirements);
    }
    if reading.taken_with.is_empty() {
        reading.taken_with.clone_from(&block.taken_with);
    }
}

/// A string, or an array of strings.
pub(crate) fn strings(literal: Option<&Literal>) -> Vec<String> {
    match literal {
        Some(Literal::Array(items)) => items
            .iter()
            .filter_map(Literal::text)
            .map(|t| t.trim().to_string())
            .collect(),
        Some(other) => other
            .text()
            .map(|t| vec![t.trim().to_string()])
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// A dictionary of strings, in the order written: `taken-with: (L1:
/// "4.7 µH", "C15": "22 µF")`.
pub(crate) fn pairs(literal: Option<&Literal>) -> Vec<(String, String)> {
    match literal {
        Some(Literal::Dict(entries)) => entries
            .iter()
            .filter_map(|(key, value)| Some((key.clone(), value.text()?.trim().to_string())))
            .collect(),
        _ => Vec::new(),
    }
}

/// A literal as its source text, for a problem.
pub(crate) fn written(literal: &Literal) -> String {
    match literal {
        Literal::Str(text) => crate::reference::quote(text),
        Literal::Number(_, text) | Literal::Other(text) => text.clone(),
        Literal::Percent(value) => format!("{value}%"),
        Literal::Content(text) => format!("[{text}]"),
        Literal::None => "none".to_string(),
        Literal::Reference(reference) => reference.to_code(),
        Literal::Array(_) | Literal::Dict(_) => "(..)".to_string(),
    }
}
