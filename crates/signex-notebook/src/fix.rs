//! The fix actions of a stale reference (mockup row 196) and the text
//! rewrite each makes. A renamed part offers Update to its new designator,
//! a deleted part or net Remove reference and Keep as plain text, a missing
//! pin Pick another pin. Update all lists every change before applying
//! it ([`plan_update_all`]).

use crate::file::{NotebookFile, NotebookFileError};
use crate::reference::{Reference, quote};
use crate::resolution::{ReferenceReport, ReferenceState, ResolvedReference};
use crate::syntax::{distinct_references, reference_spans, typst_source};

/// A fix the reference card offers for a stale reference.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "fix", rename_all = "snake_case")]
pub enum FixOffer {
    /// Rewrite every use to `to` ("Update to R17").
    Update { to: Reference },
    /// Delete every use from the text.
    RemoveReference,
    /// Replace every use with its name as plain text.
    KeepAsPlainText,
    /// Point every use at one of `pins`.
    PickAnotherPin { pins: Vec<String> },
}

/// A fix to apply.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "fix", rename_all = "snake_case")]
pub enum Fix {
    UpdateTo { to: Reference },
    RemoveReference,
    KeepAsPlainText,
    PickAnotherPin { pin: String },
}

impl ResolvedReference {
    /// The fixes this reference offers; none when it resolves.
    pub fn offers(&self) -> Vec<FixOffer> {
        match &self.state {
            ReferenceState::Resolved => Vec::new(),
            ReferenceState::Renamed { now } => vec![FixOffer::Update { to: now.clone() }],
            ReferenceState::Deleted => vec![FixOffer::RemoveReference, FixOffer::KeepAsPlainText],
            ReferenceState::MissingPin { pins } => {
                vec![FixOffer::PickAnotherPin { pins: pins.clone() }]
            }
        }
    }
}

/// Why a fix was not applied. Nothing is written then.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FixError {
    #[error(transparent)]
    File(#[from] NotebookFileError),
    /// The file does not use the reference.
    #[error("the notebook does not use {0}")]
    NotUsed(String),
    /// Removing a use inside code (a block's `target:`) would leave the
    /// code broken.
    #[error("line {line} uses {name} inside code; edit that line by hand")]
    InCode { name: String, line: u32 },
    /// The use is read as a value (`.value`, `.param(...)`); plain text has
    /// none.
    #[error("line {line} reads a value of {name}; edit that line by hand")]
    ReadAsValue { name: String, line: u32 },
    /// The fix does not apply to this kind of reference.
    #[error("{fix} does not apply to {name}")]
    NotOffered { fix: &'static str, name: String },
}

/// The rewritten file and how many uses changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixed {
    pub text: String,
    pub changed: usize,
}

/// Apply `fix` to every use of `reference` in `text`, the whole `.snxnbk`
/// file. The header keeps the reference's stored id under its new name and
/// drops what the text no longer uses. A use the fix cannot rewrite safely
/// refuses the whole fix.
pub fn apply_fix(text: &str, reference: &Reference, fix: &Fix) -> Result<Fixed, FixError> {
    let mut file = NotebookFile::parse(text)?;
    let blanked = typst_source(text);
    let shift = blanked.len().saturating_sub(file.body.len());
    let uses: Vec<_> = reference_spans(&blanked)
        .into_iter()
        .filter(|span| &span.reference == reference)
        .collect();
    if uses.is_empty() {
        return Err(FixError::NotUsed(reference.name()));
    }
    let target = fix_target(reference, fix)?;
    let mut body = file.body.clone();
    for span in uses.iter().rev() {
        let start = span.range.start.saturating_sub(shift);
        let end = span.range.end.saturating_sub(shift);
        let in_markup = body[..start].ends_with('#');
        let read_as_value = body[end..].starts_with('.');
        let (from, with) = match (&target, in_markup) {
            (Some(new), _) => (start, new.to_code()),
            (None, _) if read_as_value => {
                return Err(value_error(reference, span.line));
            }
            (None, true) => (start - 1, replacement_in_markup(reference, fix)),
            (None, false) if *fix == Fix::KeepAsPlainText => (start, quote(&reference.name())),
            (None, false) => {
                return Err(FixError::InCode {
                    name: reference.name(),
                    line: span.line,
                });
            }
        };
        body.replace_range(from..end, &with);
    }
    let id = file.header.id_of(reference);
    if let Some(new) = &target {
        file.header.set_id(new, id);
    }
    file.header.retain_used(&distinct_references(&body));
    file.body = body;
    Ok(Fixed {
        text: file.write()?,
        changed: uses.len(),
    })
}

/// The reference every use becomes, `None` for a fix that drops it.
fn fix_target(reference: &Reference, fix: &Fix) -> Result<Option<Reference>, FixError> {
    match fix {
        Fix::UpdateTo { to } => Ok(Some(to.clone())),
        Fix::RemoveReference | Fix::KeepAsPlainText => Ok(None),
        Fix::PickAnotherPin { pin } => match reference {
            Reference::Pin { component, .. } if !pin.trim().is_empty() => {
                Ok(Some(Reference::Pin {
                    component: component.clone(),
                    pin: pin.trim().to_string(),
                }))
            }
            _ => Err(FixError::NotOffered {
                fix: "Pick another pin",
                name: reference.name(),
            }),
        },
    }
}

fn value_error(reference: &Reference, line: u32) -> FixError {
    FixError::ReadAsValue {
        name: reference.name(),
        line,
    }
}

/// What replaces `#call(...)` in markup: nothing, or the name as text.
fn replacement_in_markup(reference: &Reference, fix: &Fix) -> String {
    match fix {
        Fix::KeepAsPlainText => markup_text(&reference.name()),
        _ => String::new(),
    }
}

/// `text` escaped so Typst markup prints it as written.
pub fn markup_text(text: &str) -> String {
    const SPECIAL: &[char] = &[
        '\\', '/', '[', ']', '{', '}', '#', '*', '_', '+', '=', '~', '`', '$', '"', '\'', '<', '>',
        '@', '-',
    ];
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if SPECIAL.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// One line of the Update all list: the reference, and the fix applied, or
/// `None` when it needs a choice (a deleted part, a missing pin).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlannedFix {
    pub reference: Reference,
    pub uses: usize,
    pub fix: Option<Fix>,
}

/// Every stale reference of `report` and what Update all does to it: a
/// renamed one is updated; the others are listed and left for a choice.
pub fn plan_update_all(report: &ReferenceReport) -> Vec<PlannedFix> {
    report
        .stale()
        .map(|entry| PlannedFix {
            reference: entry.reference.clone(),
            uses: entry.uses(),
            fix: match &entry.state {
                ReferenceState::Renamed { now } => Some(Fix::UpdateTo { to: now.clone() }),
                _ => None,
            },
        })
        .collect()
}

/// Apply the planned fixes of `plan` to `text` in order. Stops at the first
/// that fails, so nothing is half written.
pub fn apply_plan(text: &str, plan: &[PlannedFix]) -> Result<Fixed, FixError> {
    let mut fixed = Fixed {
        text: text.to_string(),
        changed: 0,
    };
    for planned in plan {
        let Some(fix) = &planned.fix else {
            continue;
        };
        let step = apply_fix(&fixed.text, &planned.reference, fix)?;
        fixed = Fixed {
            text: step.text,
            changed: fixed.changed + step.changed,
        };
    }
    Ok(fixed)
}
