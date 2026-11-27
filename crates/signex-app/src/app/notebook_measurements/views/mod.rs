//! The measurement views of the notebook's margin lane (mockup row 199):
//! the timeline of a net, part or pin ([`timeline`]) and the card of a
//! stale attachment ([`stale`]), plus the counts of the status strip.
//! Pure functions of the data the commands answer with; the Notebook tab
//! mounts them and owns what their buttons do, so each takes the messages
//! it publishes.

pub(crate) mod chart;
pub(crate) mod stale;
pub(crate) mod timeline;

use signex_i18n::{I18n, Localized, ids, t};
use signex_notebook::Reference;
use signex_notebook::report::Summary;

use crate::styles::TypeScale;

pub(crate) const TS: TypeScale = TypeScale::DEFAULT;
/// The lane's padding and the gap between its rows.
pub(crate) const PAD: f32 = TS.body * 8.0 / 12.0;
pub(crate) const GAP: f32 = TS.body * 4.0 / 12.0;

/// The tab of a target: `Net +3V3`, `Part C15`, `Pin U3.3`.
pub(crate) fn target_label(i18n: &I18n, reference: &Reference) -> Localized {
    let name = reference.name();
    match reference {
        Reference::Net(_) => t!(i18n, ids::NOTEBOOK_TIMELINE_TARGET_NET, "name" => name),
        Reference::Component(_) => t!(i18n, ids::NOTEBOOK_TIMELINE_TARGET_PART, "name" => name),
        Reference::Pin { .. } => t!(i18n, ids::NOTEBOOK_TIMELINE_TARGET_PIN, "name" => name),
    }
}

/// The status strip's measurement counts: `9 measurements · 8 pass ·
/// 1 fail`, then `1 stale attachment` when one is.
pub(crate) fn strip_counts(i18n: &I18n, summary: &Summary) -> Vec<Localized> {
    let mut parts = vec![t!(
        i18n, ids::NOTEBOOK_STRIP_MEASUREMENTS,
        "readings" => summary.readings,
        "pass" => summary.pass,
        "fail" => summary.fail
    )];
    if summary.stale_attachments > 0 {
        parts.push(t!(
            i18n, ids::NOTEBOOK_STRIP_STALE_ATTACHMENTS,
            "count" => summary.stale_attachments
        ));
    }
    parts
}

#[cfg(test)]
mod tests;
