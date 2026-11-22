//! Per-finding ERC waivers in the ERC panel (#1285): the shared waiver form
//! under the row being waived (AV-13), and the stored waivers' rows in the
//! Waived group with their status pills.

use iced::widget::text_editor;
use signex_erc::waivers::violation::WaiverKey;
use signex_i18n::{I18n, Localized, ids, t};

use super::validation_kit::{FindingRow, finding_row, severity_badge, status_pill};
use super::waiver_form::{ReasonFormEvents, ReasonFormView, WaiverReason, reason_form};
use super::*;
use crate::styles::{PillTone, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Messages of the per-finding waiver UI, nested under `PanelMsg`.
#[derive(Debug, Clone)]
pub enum ErcWaiverMsg {
    /// The Waive action was pressed on the finding keyed `key`. Carries the
    /// key, not the row index, so a run that lands between render and click
    /// cannot retarget it, and the sheet, so Confirm re-keys it on that
    /// sheet only.
    Started {
        key: WaiverKey,
        message: String,
        sheet: std::path::PathBuf,
    },
    /// The Reason field was typed in, or its cursor moved.
    ReasonActed(text_editor::Action),
    Confirmed,
    Cancelled,
    /// Restore the finding a stored waiver hides.
    Removed {
        rule_id: String,
        objects: Vec<String>,
    },
}

/// The waiver being written, held in the Model until confirmed.
#[derive(Debug, Clone, PartialEq)]
pub struct ErcWaiveDraft {
    pub key: WaiverKey,
    /// The sheet the finding is filed under; Confirm re-keys it there only.
    pub sheet: std::path::PathBuf,
    pub message: String,
    pub reason: WaiverReason,
    /// The git identity, empty when git has none (mockup row 88).
    pub author: String,
    /// The git identity is still being read: Waive waits for it.
    pub awaits_identity: bool,
    /// Why the last Confirm was refused.
    pub error: Option<String>,
}

/// What the newest ERC run made of a stored waiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcWaiverStatus {
    /// It hides this many findings.
    Waiving(usize),
    /// An involved object changed: the finding is back.
    Lapsed,
    /// No finding of this rule on these objects (fixed, or not run yet).
    Unmatched,
    /// No reason (hand-edited file): never honoured.
    Incomplete,
}

impl ErcWaiverStatus {
    /// The status of a net waiver that hid `hidden` findings.
    pub(super) fn of_net(hidden: usize) -> Self {
        match hidden {
            0 => Self::Unmatched,
            n => Self::Waiving(n),
        }
    }

    /// The pill's words and tone.
    pub(super) fn pill(self, i18n: &I18n) -> (Localized, PillTone) {
        match self {
            Self::Waiving(1) => (t!(i18n, ids::PANELS_ERC_WAIVER_STATUS_ONE), PillTone::Ok),
            Self::Waiving(n) => (
                // As `format!` writes it, no grouping (#2059).
                t!(i18n, ids::PANELS_ERC_WAIVER_STATUS_MANY, "count" => n.to_string()),
                PillTone::Ok,
            ),
            Self::Lapsed => (
                t!(i18n, ids::PANELS_ERC_WAIVER_STATUS_LAPSED),
                PillTone::Err,
            ),
            Self::Unmatched => (
                t!(i18n, ids::PANELS_ERC_WAIVER_STATUS_UNMATCHED),
                PillTone::Neutral,
            ),
            Self::Incomplete => (
                t!(i18n, ids::PANELS_ERC_WAIVER_STATUS_INCOMPLETE),
                PillTone::Warn,
            ),
        }
    }
}

/// One stored per-finding waiver for the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct ErcViolationWaiverEntry {
    pub rule_id: String,
    pub objects: Vec<String>,
    pub message: String,
    pub reason: String,
    pub author: String,
    pub date: String,
    pub status: ErcWaiverStatus,
    /// Severity of the finding it hides in the newest run, if any.
    pub severity: Option<ErcSeverityLite>,
}

/// The "Waive" action of a finding that names objects; `None` otherwise.
pub(super) fn waive_action<'a>(
    entry: &ErcDiagnosticEntry,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, PanelMsg>> {
    let key = entry.waiver_key.clone()?;
    Some(super::validation_kit::row_action(
        t!(i18n, ids::PANELS_ERC_WAIVE),
        PanelMsg::ErcWaiver(ErcWaiverMsg::Started {
            key,
            message: entry.message.clone(),
            sheet: entry.sheet_path.clone(),
        }),
        tokens,
    ))
}

/// The shared waiver form under the row being waived, if it is this one.
pub(super) fn waive_editor<'a>(
    entry: &ErcDiagnosticEntry,
    ctx: &'a PanelContext,
) -> Option<Element<'a, PanelMsg>> {
    let draft = ctx.erc_waive_draft.as_ref()?;
    if entry.waiver_key.as_ref() != Some(&draft.key) {
        return None;
    }
    let mut error = None;
    if let Some(why) = draft.error.as_deref() {
        // raw: the refusal is worded by the waiver handler in English (#2061).
        error = Some(raw(why));
    }
    let form = reason_form(
        ReasonFormView {
            reason: &draft.reason,
            placeholder: t!(ctx.i18n, ids::PLACEHOLDER_ERC_WAIVER_REASON),
            error,
        },
        ReasonFormEvents {
            reason: |action| PanelMsg::ErcWaiver(ErcWaiverMsg::ReasonActed(action)),
            confirm: (!draft.awaits_identity).then_some(PanelMsg::ErcWaiver(ErcWaiverMsg::Confirmed)),
            cancel: PanelMsg::ErcWaiver(ErcWaiverMsg::Cancelled),
        },
        &ctx.i18n,
        &ctx.tokens,
    );
    Some(container(form).padding([SP.xs, SP.m]).boxed())
}

/// Who waived it and when; the date alone for a waiver with no author (git
/// had no identity, mockup row 88).
fn waived_by_line<'a>(
    ctx: &'a PanelContext,
    w: &'a ErcViolationWaiverEntry,
) -> Option<Element<'a, PanelMsg>> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    if w.author.trim().is_empty() {
        if w.date.is_empty() {
            return None;
        }
        return Some(
            text(signex_i18n::UserText::new(w.date.as_str()))
                .size(TS.caption)
                .color(muted)
                .boxed(),
        );
    }
    Some(
        text(t!(
            ctx.i18n, ids::PANELS_ERC_WAIVED_BY,
            "author" => w.author.as_str(), "date" => w.date.as_str()
        ))
        .size(TS.caption)
        .color(muted)
        .boxed(),
    )
}

/// A stored per-finding waiver in the Waived group: the finding, its status
/// pill and reason, who waived it and when, and Restore.
pub(super) fn violation_waiver_row<'a>(
    ctx: &'a PanelContext,
    w: &'a ErcViolationWaiverEntry,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let muted = theme_ext::text_secondary(tokens);
    let (status, tone) = w.status.pill(i18n);
    let mut reason_line = row![status_pill(status, tone, tokens)]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    if !w.reason.is_empty() {
        reason_line = reason_line.push(
            // The reason, author and date are stored text, shown as written.
            text(signex_i18n::UserText::new(w.reason.as_str()))
                .size(TS.caption)
                .color(muted)
                .boxed(),
        );
    }
    let mut extra: Vec<Element<'a, PanelMsg>> = waived_by_line(ctx, w).into_iter().collect();
    extra.push(super::validation_kit::row_action(
        t!(i18n, ids::PANELS_ERC_RESTORE),
        PanelMsg::ErcWaiver(ErcWaiverMsg::Removed {
            rule_id: w.rule_id.clone(),
            objects: w.objects.clone(),
        }),
        tokens,
    ));
    finding_row(
        FindingRow {
            badge: severity_badge(raw("–"), PillTone::Neutral, false, tokens),
            // raw: the finding as signex-erc worded it when it was waived (#2062).
            message: raw(w.message.as_str()),
            place: reason_line.wrap().vertical_spacing(SP.xs).boxed(),
            extra,
            actions: Vec::new(),
            selected: false,
            hovered: true,
            on_press: None,
            on_enter: None,
            on_exit: None,
        },
        tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waiver(status: ErcWaiverStatus) -> ErcViolationWaiverEntry {
        ErcViolationWaiverEntry {
            rule_id: "unused-pin".into(),
            objects: vec![],
            message: "Pin 3 of U1 is unconnected".into(),
            reason: "spare gate".into(),
            author: "hg".into(),
            date: "2026-11-16".into(),
            status,
            severity: None,
        }
    }

    fn texts(ctx: &PanelContext) -> Vec<String> {
        crate::layout_probe::lay_out_texts(
            super::super::view_erc(ctx),
            iced::Size::new(300.0, 100_000.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
    }

    /// The Waived group counts the stored waivers, and an open group lists
    /// each with its status pill, reason, author and date (#85).
    #[test]
    fn the_waived_group_counts_and_lists_the_stored_waivers() {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.erc_violation_waivers = vec![waiver(ErcWaiverStatus::Lapsed); 12];
        ctx.validation.erc.waived_open = Some(true);
        let shown = texts(&ctx);
        assert!(shown.iter().any(|t| t == "Waived"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "12"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Pin 3 of U1 is unconnected"));
        assert!(
            shown
                .iter()
                .any(|t| t == "LAPSED: an involved object changed")
        );
        assert!(shown.iter().any(|t| t == "spare gate"));
        assert!(shown.iter().any(|t| t == "by hg · 2026-11-16"));
        assert!(shown.iter().any(|t| t == "Restore"));
    }

    /// Each status reads as its pill (#85): waived, waives N findings,
    /// LAPSED, matches no finding, ignored.
    #[test]
    fn every_waiver_status_has_its_pill() {
        let i18n = I18n::en_us();
        let words = |s: ErcWaiverStatus| {
            s.pill(&i18n)
                .0
                .into_string()
                .replace(['\u{2068}', '\u{2069}'], "")
        };
        assert_eq!(words(ErcWaiverStatus::Waiving(1)), "waived");
        assert_eq!(
            words(ErcWaiverStatus::Waiving(1200)),
            "waives 1200 findings"
        );
        assert_eq!(
            words(ErcWaiverStatus::Lapsed),
            "LAPSED: an involved object changed"
        );
        assert_eq!(words(ErcWaiverStatus::Unmatched), "matches no finding");
        assert_eq!(
            words(ErcWaiverStatus::Incomplete),
            "ignored: no reason"
        );
        assert_eq!(ErcWaiverStatus::of_net(0), ErcWaiverStatus::Unmatched);
        assert_eq!(ErcWaiverStatus::of_net(3), ErcWaiverStatus::Waiving(3));
    }

    /// Mockup row 88: a waiver stored with no author shows its date alone,
    /// never a dangling "by".
    #[test]
    fn a_waiver_without_an_author_shows_its_date_alone() {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        let mut stored = waiver(ErcWaiverStatus::Waiving(1));
        stored.author.clear();
        ctx.erc_violation_waivers = vec![stored];
        ctx.validation.erc.waived_open = Some(true);
        let shown = texts(&ctx);
        assert!(shown.iter().any(|t| t == "2026-11-16"), "{shown:?}");
        assert!(!shown.iter().any(|t| t.starts_with("by ")), "{shown:?}");
    }
}
