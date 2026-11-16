//! Per-finding ERC waivers in the ERC panel (#1285): the Waive chip, the
//! inline reason editor it opens, and the list of stored waivers.

use super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_erc::waivers::violation::WaiverKey;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Messages of the per-finding waiver UI, nested under `PanelMsg`.
#[derive(Debug, Clone)]
pub enum ErcWaiverMsg {
    /// The Waive chip was pressed on the finding keyed `key`. Carries the key,
    /// not the row index, so a run that lands between render and click cannot
    /// retarget it, and the sheet, so Confirm re-keys it on that sheet only.
    Started {
        key: WaiverKey,
        message: String,
        sheet: std::path::PathBuf,
    },
    ReasonChanged(String),
    AuthorChanged(String),
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
    pub reason: String,
    pub author: String,
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
    /// No reason or author (hand-edited file): never honoured.
    Incomplete,
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

/// "Waive" chip for a finding that names objects; nothing otherwise.
pub(super) fn waive_chip<'a>(
    entry: &ErcDiagnosticEntry,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    let Some(key) = entry.waiver_key.clone() else {
        return Space::new().width(0).height(0).boxed();
    };
    row![
        Space::new().width(6).height(Length::Shrink),
        signex_widgets::button(
            text(signex_i18n::t!(i18n, signex_i18n::ids::PANELS_ERC_WAIVE))
                .size(TS.caption)
                .color(theme_ext::text_primary(tokens))
        )
        .padding([SP.xs, SP.m])
        .on_press(PanelMsg::ErcWaiver(ErcWaiverMsg::Started {
            key,
            message: entry.message.clone(),
            sheet: entry.sheet_path.clone(),
        }))
        .style(crate::styles::menu_item(tokens)),
    ]
    .boxed()
}

/// The reason / author editor under the row being waived, if it is this one.
pub(super) fn waive_editor<'a>(
    entry: &ErcDiagnosticEntry,
    ctx: &'a PanelContext,
) -> Option<Element<'a, PanelMsg>> {
    let draft = ctx.erc_waive_draft.as_ref()?;
    if entry.waiver_key.as_ref() != Some(&draft.key) {
        return None;
    }
    let wrap = PanelMsg::ErcWaiver;
    let input = |hint: signex_i18n::MessageId, value: &'a str, on: fn(String) -> ErcWaiverMsg| {
        signex_widgets::text_input(signex_i18n::t!(ctx.i18n, hint), value)
            .on_input(move |s| wrap(on(s)))
            .on_submit(wrap(ErcWaiverMsg::Confirmed))
            .size(TS.body)
            .padding(SP.s)
            .width(Length::Fill)
    };
    let button = |label: signex_i18n::MessageId, msg: ErcWaiverMsg| {
        signex_widgets::button(
            text(signex_i18n::t!(ctx.i18n, label))
                .size(TS.caption)
                .color(theme_ext::text_primary(&ctx.tokens)),
        )
        .padding([SP.xs, SP.m])
        .on_press(wrap(msg))
        .style(crate::styles::menu_item(&ctx.tokens))
    };
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.xs)
        .padding([SP.xs, 18.0])
        .width(Length::Fill);
    col = col.push(
        input(
            signex_i18n::ids::PLACEHOLDER_ERC_WAIVER_REASON,
            &draft.reason,
            ErcWaiverMsg::ReasonChanged,
        )
        .boxed(),
    );
    col = col.push(
        input(
            signex_i18n::ids::PLACEHOLDER_ERC_WAIVER_AUTHOR,
            &draft.author,
            ErcWaiverMsg::AuthorChanged,
        )
        .boxed(),
    );
    if let Some(error) = &draft.error {
        col = col.push(
            // raw: the refusal is worded by the waiver handler in English (#2061).
            text(raw(error.clone()))
                .size(TS.caption)
                .color(theme_ext::error_color(&ctx.tokens))
                .boxed(),
        );
    }
    col = col.push(
        row![
            button(signex_i18n::ids::PANELS_ERC_WAIVE, ErcWaiverMsg::Confirmed),
            Space::new().width(6).height(Length::Shrink),
            button(
                signex_i18n::ids::PANELS_ERC_WAIVER_CANCEL,
                ErcWaiverMsg::Cancelled
            ),
        ]
        .boxed(),
    );
    Some(col.boxed())
}

/// The project's per-finding waivers, dimmed, each with its reason, author,
/// status and a Restore button. Shown even when every finding is waived.
pub(super) fn violation_waivers_section<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    use signex_i18n::{ids, t};
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let i18n = &ctx.i18n;
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs).width(Length::Fill);
    if ctx.erc_violation_waivers.is_empty() {
        return col.boxed();
    }
    col = col.push(separator(&ctx.tokens).boxed());
    col = col.push(
        text(t!(
            i18n,
            ids::PANELS_ERC_FINDINGS_WAIVED,
            "count" => ctx.erc_violation_waivers.len().to_string()
        ))
        .size(TS.caption)
        .color(muted)
        .boxed(),
    );
    for w in &ctx.erc_violation_waivers {
        let color = match w.status {
            ErcWaiverStatus::Lapsed | ErcWaiverStatus::Incomplete => {
                theme_ext::warning_color(&ctx.tokens)
            }
            _ => muted,
        };
        col = col.push(
            row![
                text(violation_waiver_detail(w, i18n))
                    .size(TS.caption)
                    .color(color)
                    .width(Length::Fill),
                signex_widgets::button(
                    text(t!(i18n, ids::PANELS_ERC_RESTORE))
                        .size(TS.caption)
                        .color(theme_ext::text_primary(&ctx.tokens)),
                )
                .padding([SP.xs, SP.m])
                .on_press(PanelMsg::ErcWaiver(ErcWaiverMsg::Removed {
                    rule_id: w.rule_id.clone(),
                    objects: w.objects.clone(),
                }))
                .style(crate::styles::menu_item(&ctx.tokens)),
            ]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
            .boxed(),
        );
    }
    col.boxed()
}

/// A stored waiver's two lines: rule, finding and status, then why and who.
/// The rule id, message, reason, author and date are stored text, shown as
/// they are.
fn violation_waiver_detail(
    w: &ErcViolationWaiverEntry,
    i18n: &signex_i18n::I18n,
) -> signex_i18n::Localized {
    use signex_i18n::ids;
    let status = match w.status {
        ErcWaiverStatus::Waiving(1) => i18n.message_plain(ids::PANELS_ERC_WAIVER_STATUS_ONE, &[]),
        ErcWaiverStatus::Waiving(n) => i18n.message_plain(
            ids::PANELS_ERC_WAIVER_STATUS_MANY,
            &[("count", n.to_string().into())],
        ),
        ErcWaiverStatus::Lapsed => i18n.message_plain(ids::PANELS_ERC_WAIVER_STATUS_LAPSED, &[]),
        ErcWaiverStatus::Unmatched => {
            i18n.message_plain(ids::PANELS_ERC_WAIVER_STATUS_UNMATCHED, &[])
        }
        ErcWaiverStatus::Incomplete => {
            i18n.message_plain(ids::PANELS_ERC_WAIVER_STATUS_INCOMPLETE, &[])
        }
    };
    signex_i18n::t!(
        i18n,
        ids::PANELS_ERC_VIOLATION_WAIVER_ROW,
        "rule" => w.rule_id.as_str(),
        "message" => w.message.as_str(),
        "status" => status,
        "reason" => w.reason.as_str(),
        "author" => w.author.as_str(),
        "date" => w.date.as_str()
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

    /// The rendered section heads the list with the count as `format!` wrote
    /// it, no grouping, and shows each waiver's line (#2059).
    #[test]
    fn the_view_counts_the_stored_waivers() {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.erc_violation_waivers = vec![waiver(ErcWaiverStatus::Lapsed); 1200];
        let texts: Vec<String> = crate::layout_probe::lay_out_texts(
            violation_waivers_section(&ctx),
            iced::Size::new(600.0, 100_000.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
        assert!(
            texts.iter().any(|t| t == "1200 findings waived"),
            "{:?}",
            &texts[..texts.len().min(4)]
        );
        assert!(texts.iter().any(|t| t
            == "unused-pin  Pin 3 of U1 is unconnected  (LAPSED: an involved object changed)\n  Reason: spare gate  by hg 2026-11-16"));
    }

    /// The stored-waiver line reads as the `format!` it replaced (#2059).
    #[test]
    fn a_stored_waiver_reads_as_before() {
        let i18n = signex_i18n::I18n::en_us();
        let mut w = ErcViolationWaiverEntry {
            rule_id: "unused-pin".into(),
            objects: vec![],
            message: "Pin 3 of U1 is unconnected".into(),
            reason: "spare gate".into(),
            author: "hg".into(),
            date: "2026-11-16".into(),
            status: ErcWaiverStatus::Waiving(1),
            severity: None,
        };
        let shown = |w: &ErcViolationWaiverEntry| {
            violation_waiver_detail(w, &i18n)
                .into_string()
                .replace(['\u{2068}', '\u{2069}'], "")
        };
        let expected = |status: &str| {
            format!(
                "unused-pin  Pin 3 of U1 is unconnected  ({status})\n  Reason: spare gate  by hg 2026-11-16"
            )
        };
        assert_eq!(shown(&w), expected("waived"));
        for (status, text) in [
            (ErcWaiverStatus::Waiving(1200), "waives 1200 findings"),
            (
                ErcWaiverStatus::Lapsed,
                "LAPSED: an involved object changed",
            ),
            (ErcWaiverStatus::Unmatched, "matches no finding"),
            (ErcWaiverStatus::Incomplete, "ignored: no reason or author"),
        ] {
            w.status = status;
            assert_eq!(shown(&w), expected(text));
        }
    }
}
