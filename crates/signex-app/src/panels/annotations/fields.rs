//! A row's review record (mockup row 198): the fields of the block's kind
//! and the blocks that cite it, built in `update` from the scan
//! ([`record_fields`]) and drawn as a label and value grid above an
//! expanded row's source; the verdict pill of a Calculation or
//! Measurement.

use iced::widget::{Column, Row, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_notebook::{Annotation, BlockId, Citation, Record, Severity, Verdict};
use signex_widgets::text;

use super::{PAD_TIGHT, TS};
use crate::panels::PanelContext;
use crate::panels::PanelMsg;
use crate::styles::{PillTone, ti, tokens::resolve};

/// The label column of the field grid: the mock-up's 84 px at the 12 px
/// body.
const LABEL_WIDTH: f32 = TS.body * 84.0 / 12.0;
/// The verdict pill's side padding and height, as the status pill's.
const PILL_PAD: f32 = TS.body * 6.0 / 12.0;
const PILL_H: f32 = TS.body * 17.0 / 12.0;

/// What a field of the grid is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldLabel {
    Author,
    Owner,
    Review,
    Requirement,
    Severity,
    Due,
    From,
    /// The Issues raised `from:` this block.
    Opens,
    Supersedes,
    SupersededBy,
    Chosen,
    Instrument,
    Conditions,
    Operator,
    /// A Measurement's verdict counts.
    Result,
    Inputs,
    /// The Calculations that read this block.
    ReadBy,
}

/// The value of a field.
#[derive(Debug, Clone, PartialEq)]
pub enum RowValue {
    /// Text as written in the notebook.
    Text(String),
    /// Block ids, in mono.
    Ids(Vec<String>),
    Severity(Severity),
    /// How many rows pass, warn and fail; zero counts are not shown.
    Verdicts([(Verdict, usize); 3]),
}

/// One field of a row's record.
#[derive(Debug, Clone, PartialEq)]
pub struct RowField {
    pub label: FieldLabel,
    pub value: RowValue,
}

/// The fields `annotation` shows, in the order the printed record lists
/// them, then the blocks that cite it (`cited`, from `signex_notebook::cited_by`).
pub(crate) fn record_fields(
    annotation: &Annotation,
    cited: &[(Citation, BlockId)],
) -> Vec<RowField> {
    let mut fields = Vec::new();
    let mut written = |label: FieldLabel, value: &Option<String>| {
        if let Some(value) = value {
            fields.push(RowField {
                label,
                value: RowValue::Text(value.clone()),
            });
        }
    };
    let ids = |list: &mut Vec<RowField>, label: FieldLabel, ids: Vec<String>| {
        if !ids.is_empty() {
            list.push(RowField {
                label,
                value: RowValue::Ids(ids),
            });
        }
    };
    let joined = |list: &[String]| (!list.is_empty()).then(|| list.join(", "));
    match &annotation.record {
        Record::Note(note) => written(FieldLabel::Author, &note.author),
        Record::Calculation(calculation) => {
            written(FieldLabel::Requirement, &joined(&calculation.requirements));
        }
        Record::Measurement(measurement) => {
            written(FieldLabel::Instrument, &measurement.instrument);
            written(FieldLabel::Conditions, &measurement.conditions);
            written(FieldLabel::Operator, &measurement.operator);
            written(FieldLabel::Requirement, &joined(&measurement.requirements));
        }
        Record::Issue(issue) => {
            written(FieldLabel::Owner, &issue.owner);
            written(FieldLabel::Due, &issue.due);
            written(FieldLabel::Requirement, &joined(&issue.requirements));
        }
        Record::Decision(decision) => {
            written(FieldLabel::Owner, &decision.owner);
            written(FieldLabel::Review, &decision.review);
            written(FieldLabel::Chosen, &decision.chosen);
            written(FieldLabel::Requirement, &joined(&decision.requirements));
        }
    }
    match &annotation.record {
        Record::Issue(issue) => {
            if let Some(severity) = issue.severity {
                fields.push(RowField {
                    label: FieldLabel::Severity,
                    value: RowValue::Severity(severity),
                });
            }
            ids(
                &mut fields,
                FieldLabel::From,
                issue.from.iter().map(BlockId::to_string).collect(),
            );
        }
        Record::Decision(decision) => {
            let named = decision.supersedes.iter().map(BlockId::to_string).collect();
            ids(&mut fields, FieldLabel::Supersedes, named);
        }
        Record::Calculation(calculation) => {
            let named = calculation.inputs.iter().map(BlockId::to_string).collect();
            ids(&mut fields, FieldLabel::Inputs, named);
        }
        Record::Measurement(measurement) if measurement.rows.len() > 1 => {
            fields.push(RowField {
                label: FieldLabel::Result,
                value: RowValue::Verdicts(measurement.counts()),
            });
        }
        Record::Measurement(_) | Record::Note(_) => {}
    }
    let citing = |citation: Citation| -> Vec<String> {
        cited
            .iter()
            .filter(|(how, _)| *how == citation)
            .map(|(_, by)| by.to_string())
            .collect()
    };
    ids(&mut fields, FieldLabel::Opens, citing(Citation::From));
    ids(
        &mut fields,
        FieldLabel::SupersededBy,
        citing(Citation::Supersedes),
    );
    ids(&mut fields, FieldLabel::ReadBy, citing(Citation::Input));
    fields
}

fn label_text(i18n: &I18n, label: FieldLabel) -> Localized {
    match label {
        FieldLabel::Author => t!(i18n, ids::ANNOTATIONS_FIELD_AUTHOR),
        FieldLabel::Owner => t!(i18n, ids::ANNOTATIONS_FIELD_OWNER),
        FieldLabel::Review => t!(i18n, ids::ANNOTATIONS_FIELD_REVIEW),
        FieldLabel::Requirement => t!(i18n, ids::ANNOTATIONS_FIELD_REQUIREMENT),
        FieldLabel::Severity => t!(i18n, ids::ANNOTATIONS_FIELD_SEVERITY),
        FieldLabel::Due => t!(i18n, ids::ANNOTATIONS_FIELD_DUE),
        FieldLabel::From => t!(i18n, ids::ANNOTATIONS_FIELD_FROM),
        FieldLabel::Opens => t!(i18n, ids::ANNOTATIONS_FIELD_OPENS),
        FieldLabel::Supersedes => t!(i18n, ids::ANNOTATIONS_FIELD_SUPERSEDES),
        FieldLabel::SupersededBy => t!(i18n, ids::ANNOTATIONS_FIELD_SUPERSEDED_BY),
        FieldLabel::Chosen => t!(i18n, ids::ANNOTATIONS_FIELD_CHOSEN),
        FieldLabel::Instrument => t!(i18n, ids::ANNOTATIONS_FIELD_INSTRUMENT),
        FieldLabel::Conditions => t!(i18n, ids::ANNOTATIONS_FIELD_CONDITIONS),
        FieldLabel::Operator => t!(i18n, ids::ANNOTATIONS_FIELD_OPERATOR),
        FieldLabel::Result => t!(i18n, ids::ANNOTATIONS_FIELD_RESULT),
        FieldLabel::Inputs => t!(i18n, ids::ANNOTATIONS_FIELD_INPUTS),
        FieldLabel::ReadBy => t!(i18n, ids::ANNOTATIONS_FIELD_READ_BY),
    }
}

fn severity_text(i18n: &I18n, severity: Severity) -> Localized {
    match severity {
        Severity::Minor => t!(i18n, ids::ANNOTATIONS_SEVERITY_MINOR),
        Severity::Major => t!(i18n, ids::ANNOTATIONS_SEVERITY_MAJOR),
        Severity::Critical => t!(i18n, ids::ANNOTATIONS_SEVERITY_CRITICAL),
    }
}

pub(crate) fn verdict_text(i18n: &I18n, verdict: Verdict) -> Localized {
    match verdict {
        Verdict::Pass => t!(i18n, ids::ANNOTATIONS_VERDICT_PASS),
        Verdict::Warn => t!(i18n, ids::ANNOTATIONS_VERDICT_WARN),
        Verdict::Fail => t!(i18n, ids::ANNOTATIONS_VERDICT_FAIL),
    }
}

fn count_text(i18n: &I18n, verdict: Verdict, count: usize) -> Localized {
    let id = match verdict {
        Verdict::Pass => ids::ANNOTATIONS_VERDICT_COUNT_PASS,
        Verdict::Warn => ids::ANNOTATIONS_VERDICT_COUNT_WARN,
        Verdict::Fail => ids::ANNOTATIONS_VERDICT_COUNT_FAIL,
    };
    t!(i18n, id, "count" => count)
}

/// The verdict as a pill: ok for Pass, warn for Warn, error for Fail.
pub(super) fn verdict_pill<'a>(ctx: &'a PanelContext, verdict: Verdict) -> Element<'a, PanelMsg> {
    let tone = match verdict {
        Verdict::Pass => PillTone::Ok,
        Verdict::Warn => PillTone::Warn,
        Verdict::Fail => PillTone::Err,
    };
    container(text(verdict_text(&ctx.i18n, verdict)).size(TS.caption))
        .padding([0.0, PILL_PAD])
        .center_y(PILL_H)
        .style(crate::styles::pill(&ctx.tokens, tone))
        .boxed()
}

/// The fields as a grid: the label in the muted ink, the value beside it.
pub(super) fn field_grid<'a>(
    ctx: &'a PanelContext,
    fields: &'a [RowField],
) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let mut grid = Column::new().spacing(PAD_TIGHT * 0.5).width(Length::Fill);
    for field in fields {
        let label = text(label_text(&ctx.i18n, field.label))
            .size(TS.caption)
            .color(r.ink3)
            .width(LABEL_WIDTH);
        grid = grid.push_boxed(
            row![label, field_value(ctx, &field.value)]
                .spacing(PAD_TIGHT)
                .align_y(Alignment::Start),
        );
    }
    grid.boxed()
}

fn field_value<'a>(ctx: &'a PanelContext, value: &'a RowValue) -> Element<'a, PanelMsg> {
    let ink = ti(ctx.tokens.text);
    match value {
        RowValue::Text(written) => text(UserText::new(written.as_str()))
            .size(TS.caption)
            .color(ink)
            .width(Length::Fill)
            .boxed(),
        RowValue::Ids(list) => text(UserText::new(list.join(", ")))
            .size(TS.caption)
            .font(super::rows::mono())
            .color(ink)
            .width(Length::Fill)
            .boxed(),
        RowValue::Severity(severity) => text(severity_text(&ctx.i18n, *severity))
            .size(TS.caption)
            .color(ink)
            .boxed(),
        RowValue::Verdicts(counts) => {
            let mut line = Row::new().spacing(PAD_TIGHT);
            for (verdict, count) in counts.iter().filter(|(_, count)| *count > 0) {
                line = line.push_boxed(
                    text(count_text(&ctx.i18n, *verdict, *count))
                        .size(TS.caption)
                        .color(ink),
                );
            }
            line.boxed()
        }
    }
}

#[cfg(test)]
mod tests;
