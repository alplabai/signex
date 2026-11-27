//! #431 — the netlist-incomplete prompt: "Export anyway (incomplete)?".
//!
//! Raised when a netlist export would leave pages out. It names how many,
//! lists them with the reason, and offers two actions: Cancel (the primary,
//! default action; Esc and a press outside the card cancel too) writes
//! nothing, and "Export anyway (incomplete)" on the left, in the soft danger
//! style, writes the partial `.net` and reports the omitted pages in the
//! Messages panel.

use std::sync::LazyLock;

use iced::widget::{Space, column, container, svg};
use iced::{Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_widgets::text;

use super::*;
use crate::app::state::{ModalId, OmittedPage, OmittedReason, UnreadableCause};
use crate::styles::{
    DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_footer_split, dialog_head_row,
    dialog_intro_with, dialog_row, dialog_rows,
};

/// Side of the glyph at the start of an omitted-page row.
const ROW_GLYPH: f32 = 16.0;

const SVG_WARN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l10 18H2z"/><path d="M12 10v5M12 18h.01"/></svg>"##;

/// The row glyph, parsed once.
static GLYPH_WARN: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(SVG_WARN));

impl Signex {
    /// The prompt in its own OS window: the card filling the window.
    pub(in crate::app::view) fn view_netlist_incomplete_prompt(&self) -> Element<'_, Message> {
        self.netlist_incomplete_prompt_card()
            .unwrap_or_else(|| Space::new().boxed())
    }

    /// The prompt's card: in the main window at its design width with its
    /// header (the overlay centres it and cancels on a press outside it); in
    /// its own window filling the window, the native title bar holding the
    /// title.
    pub(in crate::app::view) fn netlist_incomplete_prompt_card(
        &self,
    ) -> Option<Element<'_, Message>> {
        let prompt = self.document_view.netlist_incomplete_prompt.as_ref()?;
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let ink = crate::styles::tokens::resolve(tokens);
        let detached = self.modal_detached(ModalId::NetlistIncompletePrompt);
        let cancel = Message::Export(ExportMsg::NetlistCancelIncomplete);

        let header: Element<'_, Message> = if detached {
            super::os_title_bar_slot()
        } else {
            super::modal_title_header(
                t!(i18n, ids::DIALOG_NETLIST_INCOMPLETE_TITLE).into_string(),
                cancel.clone(),
                tokens,
            )
        };
        let detail = text(t!(i18n, ids::DIALOG_NETLIST_INCOMPLETE_DETAIL))
            .size(TS.body)
            .color(ink.ink3)
            .boxed();
        let intro = dialog_intro_with(
            DialogIcon::Warn,
            t!(
                i18n,
                ids::DIALOG_NETLIST_INCOMPLETE_COUNT,
                "count" => prompt.omitted.len()
            )
            .into_string(),
            Some(detail),
            tokens,
        );
        let head = dialog_head_row(
            vec![
                Space::new().width(Length::Fixed(ROW_GLYPH)).boxed(),
                text(t!(i18n, ids::DIALOG_NETLIST_OMITTED_PAGE))
                    .size(TS.section)
                    .width(Length::FillPortion(2))
                    .boxed(),
                text(t!(i18n, ids::DIALOG_NETLIST_OMITTED_WHY))
                    .size(TS.section)
                    .width(Length::FillPortion(3))
                    .boxed(),
            ],
            tokens,
        );
        let rows = prompt
            .omitted
            .iter()
            .map(|page| omitted_row(page, i18n, tokens))
            .collect();
        let list = dialog_rows(
            Some(head),
            rows,
            Some(crate::app::handlers::modal_sizing::OMITTED_LIST_MAX),
            tokens,
        );
        let note = text(t!(i18n, ids::DIALOG_NETLIST_INCOMPLETE_ANYWAY_NOTE))
            .size(TS.body)
            .color(ink.ink4)
            .boxed();

        // Cancel is the default and the primary; exporting an incomplete
        // netlist is a deliberate, risky choice, so it sits apart on the left
        // in the danger style.
        let footer = dialog_footer_split(
            vec![dialog_button(
                t!(i18n, ids::DIALOG_NETLIST_EXPORT_ANYWAY),
                DialogButtonKind::Danger,
                Some(Message::Export(ExportMsg::NetlistExportAnyway)),
                tokens,
            )],
            vec![dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Primary,
                Some(cancel),
                tokens,
            )],
        );

        let width = self.dialog_width(
            ModalId::NetlistIncompletePrompt,
            super::sizes::NETLIST_INCOMPLETE,
        );
        let height = if detached {
            Length::Fill
        } else {
            Length::Shrink
        };
        let card = container(
            column![
                header,
                dialog_body(column![intro, list, note].spacing(SP.m)),
                footer,
            ]
            .width(Length::Fill)
            .height(height),
        )
        .width(width)
        .height(height)
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        Some(card.boxed())
    }
}

/// One omitted page: the glyph (error ink for a missing file, warn ink
/// otherwise), the file in the mono face, and why it is left out.
fn omitted_row<'a>(
    page: &OmittedPage,
    i18n: &I18n,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    let glyph_ink = match page.reason {
        OmittedReason::FileMissing { .. } => crate::styles::ti(tokens.error),
        OmittedReason::Unreadable { .. }
        | OmittedReason::SheetCycle { .. }
        | OmittedReason::KeyCollision { .. } => crate::styles::ti(tokens.warning),
    };
    let glyph = svg(GLYPH_WARN.clone())
        .width(Length::Fixed(ROW_GLYPH))
        .height(Length::Fixed(ROW_GLYPH))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(glyph_ink),
            ..Default::default()
        })
        .boxed();
    dialog_row(vec![
        glyph,
        text(UserText::new(page.file.clone()))
            .font(super::graphite::mono())
            .size(TS.body)
            .color(crate::styles::ti(tokens.text))
            .width(Length::FillPortion(2))
            .boxed(),
        text(reason_label(&page.reason, i18n))
            .size(TS.caption)
            .color(crate::styles::tokens::resolve(tokens).ink4)
            .width(Length::FillPortion(3))
            .boxed(),
    ])
}

/// Why a page is left out, as the list says it: every reason from the
/// catalog (mockup row 28).
fn reason_label(reason: &OmittedReason, i18n: &I18n) -> Localized {
    match reason {
        OmittedReason::FileMissing {
            placed_by: Some(symbol),
        } => t!(
            i18n,
            ids::DIALOG_NETLIST_OMITTED_PLACED_BY,
            "symbol" => symbol.as_str()
        ),
        OmittedReason::FileMissing { placed_by: None } => {
            t!(i18n, ids::DIALOG_NETLIST_OMITTED_FILE_MISSING)
        }
        OmittedReason::Unreadable { cause } => t!(i18n, unreadable_label(*cause)),
        OmittedReason::SheetCycle { parent } => t!(
            i18n,
            ids::DIALOG_NETLIST_OMITTED_SHEET_CYCLE,
            "parent" => parent.as_str()
        ),
        OmittedReason::KeyCollision { kept } => t!(
            i18n,
            ids::DIALOG_NETLIST_OMITTED_KEY_COLLISION,
            "kept" => kept.as_str()
        ),
    }
}

/// The catalog line for an unreadable page, by the sheet reader's cause.
fn unreadable_label(cause: Option<UnreadableCause>) -> signex_i18n::MessageId {
    match cause {
        None => ids::DIALOG_NETLIST_OMITTED_UNREADABLE,
        Some(UnreadableCause::NotARegularFile) => ids::DIALOG_NETLIST_OMITTED_NOT_A_FILE,
        Some(UnreadableCause::TooLarge) => ids::DIALOG_NETLIST_OMITTED_TOO_LARGE,
        Some(UnreadableCause::ReadFailed) => ids::DIALOG_NETLIST_OMITTED_READ_FAILED,
        Some(UnreadableCause::NewerFormat) => ids::DIALOG_NETLIST_OMITTED_NEWER_FORMAT,
        Some(UnreadableCause::NotASheet) => ids::DIALOG_NETLIST_OMITTED_NOT_A_SHEET,
        Some(UnreadableCause::Refused) => ids::DIALOG_NETLIST_OMITTED_REFUSED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(reason: OmittedReason) -> String {
        let i18n = I18n::en_us_ref();
        reason_label(&reason, i18n)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    }

    /// Mockup row 28: the Why column reads every reason from the catalog.
    #[test]
    fn every_reason_is_worded_from_the_catalog() {
        let cause = |c| OmittedReason::Unreadable { cause: Some(c) };
        assert_eq!(
            said(cause(UnreadableCause::NewerFormat)),
            "unreadable (newer format)"
        );
        assert_eq!(
            said(OmittedReason::Unreadable { cause: None }),
            "unreadable"
        );
        assert_eq!(
            said(OmittedReason::SheetCycle {
                parent: "U_power/U_loop".into()
            }),
            "sheet cycle · re-entered from U_power/U_loop"
        );
        assert_eq!(
            said(OmittedReason::KeyCollision {
                kept: "/p/x.snxsch".into()
            }),
            "same path as /p/x.snxsch on this file system"
        );
        let causes = [
            UnreadableCause::NotARegularFile,
            UnreadableCause::TooLarge,
            UnreadableCause::ReadFailed,
            UnreadableCause::NewerFormat,
            UnreadableCause::NotASheet,
            UnreadableCause::Refused,
        ];
        for (i, a) in causes.iter().enumerate() {
            for b in &causes[i + 1..] {
                assert_ne!(said(cause(*a)), said(cause(*b)), "{a:?} / {b:?}");
            }
        }
    }

    /// Mockup row 28: the laid-out prompt words the Why column from the
    /// catalog and keeps its two buttons ("Leave two buttons").
    #[test]
    fn the_prompt_shows_the_catalog_reasons_and_two_buttons() {
        let (mut app, _) = Signex::new();
        let row = |file: &str, reason| OmittedPage {
            file: file.to_owned(),
            reason,
        };
        app.document_view.netlist_incomplete_prompt =
            Some(crate::app::state::NetlistIncompletePrompt {
                save_path: "out.net".into(),
                messages: Vec::new(),
                ctx: signex_output::ExportContext {
                    sheets: Vec::new(),
                    metadata: signex_output::ProjectMetadata::default(),
                    netlist: None,
                    netlist_sheets: Vec::new(),
                    current_sheet: None,
                    netlist_placements: Vec::new(),
                },
                omitted: vec![
                    row(
                        "sensors.snxsch",
                        OmittedReason::Unreadable {
                            cause: Some(UnreadableCause::NewerFormat),
                        },
                    ),
                    row(
                        "loop.snxsch",
                        OmittedReason::SheetCycle {
                            parent: "top.snxsch".into(),
                        },
                    ),
                ],
            });
        let card = app.netlist_incomplete_prompt_card().expect("the card");
        let shown: Vec<String> =
            crate::layout_probe::lay_out_texts(card, iced::Size::new(1200.0, 900.0))
                .into_iter()
                .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
                .collect();
        for want in [
            "unreadable (newer format)",
            "sheet cycle · re-entered from top.snxsch",
            "Export anyway (incomplete)",
            "Cancel",
        ] {
            assert!(shown.iter().any(|s| s == want), "{want:?} in {shown:?}");
        }
    }
}
