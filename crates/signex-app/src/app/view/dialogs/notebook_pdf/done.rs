//! The done card of Export Design Notebook PDF (mockup row 202): the file,
//! its pages, paper, size, references and variant, the bookmarks written
//! and the checksum, and whether it matches the last export of the file;
//! Show in folder on the left of the footer, Close and Open PDF on the
//! right.

use iced::widget::{Column, Row, container};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::board_output::kit;
use super::form::paper_words;
use super::{Parts, SP, TS, file_size, out};
use crate::app::Message;
use crate::app::NotebookPdfMsg as M;
use crate::app::notebook_pdf::ExportDone;
use crate::styles::{DialogButtonKind, PillTone, dialog_button, dialog_footer_split};

/// Characters of the checksum shown at each end: `3f9c…a41e`.
const CHECKSUM_ENDS: usize = 4;

/// The checksum as the card shows it: its first and last characters.
pub(super) fn short_checksum(sha256: &str) -> String {
    let n = sha256.len();
    if n <= 2 * CHECKSUM_ENDS {
        return sha256.to_string();
    }
    format!(
        "{}\u{2026}{}",
        &sha256[..CHECKSUM_ENDS],
        &sha256[n - CHECKSUM_ENDS..]
    )
}

/// The lines of the card under the file name, in order.
pub(super) fn fact_lines(done: &ExportDone, i18n: &I18n) -> Vec<String> {
    let summary = &done.summary;
    let facts = t!(
        i18n,
        ids::NOTEBOOK_PDF_DONE_FACTS,
        "pages" => summary.pages,
        "paper" => paper_words(&summary.paper, i18n),
        "size" => file_size(summary.bytes),
        "references" => summary.references,
        "stale" => summary.stale.len(),
        "variant" => summary.variant.as_str()
    );
    let marks = &summary.bookmarks;
    let bookmarks = if marks.total() == 0 {
        t!(i18n, ids::NOTEBOOK_PDF_DONE_NO_BOOKMARKS)
    } else {
        t!(
            i18n,
            ids::NOTEBOOK_PDF_DONE_BOOKMARKS,
            "sections" => marks.sections,
            "subsections" => marks.subsections,
            "figures" => marks.figures,
            "tables" => marks.tables
        )
    };
    let sum = short_checksum(&summary.sha256);
    let checksum = match done.same_as_last {
        None => t!(i18n, ids::NOTEBOOK_PDF_DONE_CHECKSUM, "sum" => sum),
        Some(true) => t!(i18n, ids::NOTEBOOK_PDF_DONE_SAME, "sum" => sum),
        Some(false) => t!(i18n, ids::NOTEBOOK_PDF_DONE_CHANGED, "sum" => sum),
    };
    vec![
        facts.into_string(),
        bookmarks.into_string(),
        checksum.into_string(),
    ]
}

pub(super) fn parts<'a>(done: &'a ExportDone, i18n: &I18n, tokens: &ThemeTokens) -> Parts<'a> {
    let close = Some(out(M::Closed));
    let header = super::super::header::graphite_header_closing(
        t!(i18n, ids::NOTEBOOK_PDF_DONE_TITLE).into_string(),
        None,
        Some(kit::status_glyph(PillTone::Ok, tokens)),
        close.clone(),
        super::super::header::DIALOG_HEADER_H,
        tokens,
    );
    let name = done
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut lines: Column<Element<'a, Message>> = Column::new().spacing(SP.xs).push(
        text(UserText::new(name))
            .size(TS.body)
            .font(crate::fonts::iced_font_for_family(
                crate::fonts::CHROME_MONO_FONT,
            ))
            .boxed(),
    );
    for line in fact_lines(done, i18n) {
        lines = lines.push(kit::caption(UserText::new(line), tokens));
    }
    let body = container(
        Row::new()
            .spacing(SP.l)
            .align_y(Alignment::Start)
            .push(lines.width(Length::Fill)),
    )
    .padding(SP.l + SP.xs)
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed();
    let footer = dialog_footer_split(
        vec![dialog_button(
            t!(i18n, ids::NOTEBOOK_PDF_SHOW_IN_FOLDER),
            DialogButtonKind::Link,
            Some(out(M::ShowInFolderRequested)),
            tokens,
        )],
        vec![
            dialog_button(
                t!(i18n, ids::NOTEBOOK_PDF_CLOSE),
                DialogButtonKind::Cancel,
                close,
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::NOTEBOOK_PDF_OPEN_PDF),
                super::super::graphite::call_to_action(tokens),
                Some(out(M::OpenPdfRequested)),
                tokens,
            ),
        ],
    );
    (header, body, footer)
}
