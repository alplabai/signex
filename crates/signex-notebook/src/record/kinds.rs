//! One reader per block kind: `read_note`, `read_calculation`,
//! `read_measurement`, `read_issue` and `read_decision`, over a [`Reader`]
//! that hands out each field once and reports the ones nobody took.

use super::measurement::{Limits, measurement_rows, verdict_of};
use super::value::FieldValue;
use super::{
    Alternative, CalculationRecord, DecisionRecord, FieldProblem, Fields, IssueRecord,
    MeasurementRecord, NoteRecord, Severity,
};
use crate::id::BlockId;

/// The fields of one block, each read at most once.
pub(super) struct Reader<'a> {
    fields: &'a Fields,
    taken: Vec<bool>,
    problems: &'a mut Vec<FieldProblem>,
}

impl<'a> Reader<'a> {
    pub(super) fn new(fields: &'a Fields, problems: &'a mut Vec<FieldProblem>) -> Self {
        Self {
            taken: vec![false; fields.entries.len()],
            fields,
            problems,
        }
    }

    /// The value of field `name`, marked as read.
    pub(super) fn value(&mut self, name: &str) -> Option<&'a FieldValue> {
        let fields = self.fields;
        let index = fields.entries.iter().position(|(n, _, _)| n == name)?;
        self.taken[index] = true;
        Some(&fields.entries[index].1)
    }

    /// Report field `name` as holding a value it does not take.
    pub(super) fn bad(&mut self, name: &str) {
        if let Some((_, _, raw)) = self.fields.entries.iter().find(|(n, _, _)| n == name) {
            self.problems.push(FieldProblem::Bad {
                argument: name.to_string(),
                value: raw.clone(),
            });
        }
    }

    /// A string or content field.
    pub(super) fn text(&mut self, name: &str) -> Option<String> {
        let value = self.value(name)?;
        match value.as_text() {
            Some(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
            Some(_) => None,
            None if *value == FieldValue::None => None,
            None => {
                self.bad(name);
                None
            }
        }
    }

    /// A string field, or the strings of an array: `"REQ-002"` or
    /// `("REQ-001", "REQ-006")`.
    pub(super) fn texts(&mut self, name: &str) -> Vec<String> {
        let Some(value) = self.value(name) else {
            return Vec::new();
        };
        let texts: Option<Vec<String>> = value
            .items()
            .into_iter()
            .map(|item| item.as_text().map(|t| t.trim().to_string()))
            .collect();
        texts.unwrap_or_else(|| {
            self.bad(name);
            Vec::new()
        })
    }

    /// A block id field of any kind: `from: "CALC-2.1"`.
    pub(super) fn block_id(&mut self, name: &str) -> Option<BlockId> {
        self.block_ids(name).into_iter().next()
    }

    /// Block ids, one or an array of them.
    pub(super) fn block_ids(&mut self, name: &str) -> Vec<BlockId> {
        let texts = self.texts(name);
        let ids: Option<Vec<BlockId>> = texts.iter().map(|t| BlockId::parse(t)).collect();
        ids.unwrap_or_else(|| {
            self.bad(name);
            Vec::new()
        })
    }

    /// A number field: `value: 310`.
    pub(super) fn number(&mut self, name: &str) -> Option<f64> {
        let value = self.value(name)?;
        match value.as_number() {
            Some((number, _)) => Some(number),
            None => {
                self.bad(name);
                None
            }
        }
    }

    /// A `(min: .., max: ..)` field.
    pub(super) fn limits(&mut self, name: &str) -> Option<Limits> {
        let value = self.value(name)?;
        let limits = Limits::from_dict(value);
        if limits.is_none() {
            self.bad(name);
        }
        limits
    }

    /// Report every field no reader took.
    pub(super) fn finish(self) {
        for ((name, _, _), taken) in self.fields.entries.iter().zip(self.taken) {
            if !taken {
                self.problems.push(FieldProblem::Unknown {
                    argument: name.clone(),
                });
            }
        }
    }

    /// `target:` names the parts, pins and nets the block concerns; they
    /// are read into the block's references, so the field is taken here.
    fn take_target(&mut self) -> Option<&'a FieldValue> {
        self.value("target")
    }
}

pub(super) fn read_note(reader: &mut Reader) -> NoteRecord {
    reader.take_target();
    NoteRecord {
        author: reader.text("author"),
    }
}

pub(super) fn read_calculation(reader: &mut Reader) -> CalculationRecord {
    let (nominal, band) = match reader.take_target() {
        // `target: (3.30, 2%)`: a nominal value and its tolerance.
        Some(target) if target.references().is_empty() => match band_of(target) {
            Some((nominal, limits)) => (Some(nominal), Some(limits)),
            None => {
                reader.bad("target");
                (None, None)
            }
        },
        _ => (None, None),
    };
    let limits = reader.limits("limit").or(band);
    let result = reader.number("result");
    CalculationRecord {
        verdict: result.zip(limits).map(|(r, l)| verdict_of(r, l)),
        limits,
        nominal,
        result,
        unit: reader.text("unit"),
        requirements: reader.texts("requirement"),
        inputs: reader.block_ids("inputs"),
    }
}

pub(super) fn read_measurement(reader: &mut Reader) -> MeasurementRecord {
    let rows = measurement_rows(reader);
    MeasurementRecord {
        verdict: rows.iter().filter_map(|row| row.verdict).max(),
        rows,
        unit: reader.text("unit"),
        conditions: reader.text("conditions"),
        instrument: reader.text("instrument"),
        operator: reader.text("operator"),
        requirements: reader.texts("requirement"),
    }
}

pub(super) fn read_issue(reader: &mut Reader) -> IssueRecord {
    reader.take_target();
    let severity = reader.text("severity").and_then(|token| {
        let parsed = Severity::parse(&token);
        if parsed.is_none() {
            reader.bad("severity");
        }
        parsed
    });
    IssueRecord {
        owner: reader.text("owner"),
        severity,
        due: reader.text("due"),
        from: reader.block_id("from"),
        requirements: reader.texts("requirement"),
        closes_when: reader.text("closes"),
    }
}

pub(super) fn read_decision(reader: &mut Reader) -> DecisionRecord {
    reader.take_target();
    DecisionRecord {
        owner: reader.text("owner"),
        review: reader.text("review"),
        requirements: reader.texts("requirement"),
        chosen: reader.text("chosen"),
        alternatives: alternatives(reader),
        supersedes: reader.block_id("supersedes"),
        approver: reader.text("approver"),
    }
}

/// `alternatives: (([B, 3.3 µH], [why]), [C])`: each an option, with or
/// without the reason it was dropped.
fn alternatives(reader: &mut Reader) -> Vec<Alternative> {
    let Some(value) = reader.value("alternatives") else {
        return Vec::new();
    };
    let read: Option<Vec<Alternative>> = value
        .items()
        .into_iter()
        .map(|item| match item {
            FieldValue::Array(pair) => match pair.as_slice() {
                [option] => Some(Alternative {
                    option: option.as_text()?.to_string(),
                    reason: None,
                }),
                [option, reason] => Some(Alternative {
                    option: option.as_text()?.to_string(),
                    reason: Some(reason.as_text()?.to_string()),
                }),
                _ => None,
            },
            other => Some(Alternative {
                option: other.as_text()?.to_string(),
                reason: None,
            }),
        })
        .collect();
    read.unwrap_or_else(|| {
        reader.bad("alternatives");
        Vec::new()
    })
}

/// `(3.30, 2%)` or `(3.30, 0.05)`: the nominal and the band around it.
pub(super) fn band_of(value: &FieldValue) -> Option<(f64, Limits)> {
    let FieldValue::Array(items) = value else {
        return None;
    };
    let [nominal, tolerance] = items.as_slice() else {
        return None;
    };
    let (nominal, None) = nominal.as_number()? else {
        return None;
    };
    let (tolerance, unit) = tolerance.as_number()?;
    Some((
        nominal,
        Limits::around(nominal, tolerance, unit == Some("%"))?,
    ))
}
