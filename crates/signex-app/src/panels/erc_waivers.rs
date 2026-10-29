//! Per-finding ERC waivers in the ERC panel (#1285): the Waive chip, the
//! inline reason editor it opens, and the list of stored waivers.

use super::*;
use crate::styles::{Spacing, TypeScale};
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
#[derive(Debug, Clone)]
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
) -> Element<'a, PanelMsg> {
    let Some(key) = entry.waiver_key.clone() else {
        return Space::new().width(0).height(0).into();
    };
    row![
        Space::new().width(6).height(Length::Shrink),
        iced::widget::button(text("Waive").size(9).color(theme_ext::text_primary(tokens)))
            .padding([SP.xs, SP.m])
            .on_press(PanelMsg::ErcWaiver(ErcWaiverMsg::Started {
                key,
                message: entry.message.clone(),
                sheet: entry.sheet_path.clone(),
            }))
            .style(crate::styles::menu_item(tokens)),
    ]
    .into()
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
    let input = |hint: &'a str, value: &str, on: fn(String) -> ErcWaiverMsg| {
        iced::widget::text_input(hint, value)
            .on_input(move |s| wrap(on(s)))
            .on_submit(wrap(ErcWaiverMsg::Confirmed))
            .size(TS.body)
            .padding(SP.s)
            .width(Length::Fill)
    };
    let button = |label: &'a str, msg: ErcWaiverMsg| {
        iced::widget::button(
            text(label)
                .size(9)
                .color(theme_ext::text_primary(&ctx.tokens)),
        )
        .padding([SP.xs, SP.m])
        .on_press(wrap(msg))
        .style(crate::styles::menu_item(&ctx.tokens))
    };
    let mut col: Column<'a, PanelMsg> = Column::new()
        .spacing(SP.xs)
        .padding([SP.xs, 18.0])
        .width(Length::Fill);
    col = col.push(input(
        "Reason (required)",
        &draft.reason,
        ErcWaiverMsg::ReasonChanged,
    ));
    col = col.push(input(
        "Author (required)",
        &draft.author,
        ErcWaiverMsg::AuthorChanged,
    ));
    if let Some(error) = &draft.error {
        col = col.push(
            text(error.clone())
                .size(9)
                .color(theme_ext::error_color(&ctx.tokens)),
        );
    }
    col = col.push(row![
        button("Waive", ErcWaiverMsg::Confirmed),
        Space::new().width(6).height(Length::Shrink),
        button("Cancel", ErcWaiverMsg::Cancelled),
    ]);
    Some(col.into())
}

/// The project's per-finding waivers, dimmed, each with its reason, author,
/// status and a Restore button. Shown even when every finding is waived.
pub(super) fn violation_waivers_section<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = theme_ext::text_secondary(&ctx.tokens);
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(SP.xs).width(Length::Fill);
    if ctx.erc_violation_waivers.is_empty() {
        return col.into();
    }
    col = col.push(separator(&ctx.tokens));
    col = col.push(
        text(format!(
            "{} findings waived",
            ctx.erc_violation_waivers.len()
        ))
        .size(TS.caption)
        .color(muted),
    );
    for w in &ctx.erc_violation_waivers {
        let status = match w.status {
            ErcWaiverStatus::Waiving(1) => "waived".to_string(),
            ErcWaiverStatus::Waiving(n) => format!("waives {n} findings"),
            ErcWaiverStatus::Lapsed => "LAPSED: an involved object changed".to_string(),
            ErcWaiverStatus::Unmatched => "matches no finding".to_string(),
            ErcWaiverStatus::Incomplete => "ignored: no reason or author".to_string(),
        };
        let color = match w.status {
            ErcWaiverStatus::Lapsed | ErcWaiverStatus::Incomplete => {
                theme_ext::warning_color(&ctx.tokens)
            }
            _ => muted,
        };
        let detail = format!(
            "{}  {}  ({status})\n  Reason: {}  by {} {}",
            w.rule_id, w.message, w.reason, w.author, w.date
        );
        col = col.push(
            row![
                text(detail).size(9).color(color).width(Length::Fill),
                iced::widget::button(
                    text("Restore")
                        .size(9)
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
            .width(Length::Fill),
        );
    }
    col.into()
}
