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
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::{Label, raw, text};

use super::*;
use crate::app::state::{ModalId, OmittedPage, OmittedReason};
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
        let prompt = self.document_state.netlist_incomplete_prompt.as_ref()?;
        let tokens = &self.document_state.panel_ctx.tokens;
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
        OmittedReason::Unreadable(_) | OmittedReason::Stitch(_) => {
            crate::styles::ti(tokens.warning)
        }
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

/// Why a page is left out, as the list says it.
fn reason_label(reason: &OmittedReason, i18n: &I18n) -> Label<'static> {
    match reason {
        OmittedReason::FileMissing {
            placed_by: Some(symbol),
        } => t!(
            i18n,
            ids::DIALOG_NETLIST_OMITTED_PLACED_BY,
            "symbol" => symbol.as_str()
        )
        .into(),
        OmittedReason::FileMissing { placed_by: None } => {
            t!(i18n, ids::DIALOG_NETLIST_OMITTED_FILE_MISSING).into()
        }
        // raw: worded in English by the sheet reader or the netlist check
        // (#2061).
        OmittedReason::Unreadable(why) | OmittedReason::Stitch(why) => raw(why.clone()),
    }
}
