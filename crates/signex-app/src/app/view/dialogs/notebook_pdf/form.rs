//! The settings form of Export Design Notebook PDF (mockup row 202): the
//! stale banner, Pages, Paper and Variant, the Include ticks, the
//! annotation kinds, the stale rule and Save to, then Add as Output Job
//! output… on the left of the footer and Cancel and Export PDF (Export
//! anyway once a reference is stale) on the right.

use iced::widget::{Column, Row, container, pick_list, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{Arg, I18n, Localized, MessageId, ids, t};
use signex_notebook::export::{NotebookInspection, Paper, PaperName, PaperSize, StaleRule};
use signex_notebook::{AnnotationKind, DiagnosticSeverity};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, segmented, text, text_input, tooltip};

use super::super::board_output::kit;
use super::{Parts, SP, TS, out, subtitle};
use crate::app::Message;
use crate::app::NotebookPdfMsg as M;
use crate::app::notebook_pdf::{IncludeFlag, NotebookPdfDialog, PageChoice, Stage};
use crate::panels::form_look::text_input_style;
use crate::styles::{DialogButtonKind, PillTone, dialog_button, dialog_footer_split};

/// The catalog name of an annotation kind, as the Annotations panel shows
/// it.
pub(super) fn kind_label(kind: AnnotationKind) -> MessageId {
    match kind {
        AnnotationKind::Note => ids::ANNOTATIONS_KIND_NOTE,
        AnnotationKind::Calculation => ids::ANNOTATIONS_KIND_CALCULATION,
        AnnotationKind::Measurement => ids::ANNOTATIONS_KIND_MEASUREMENT,
        AnnotationKind::Issue => ids::ANNOTATIONS_KIND_ISSUE,
        AnnotationKind::Decision => ids::ANNOTATIONS_KIND_DECISION,
    }
}

fn include_label(flag: IncludeFlag) -> (MessageId, Option<MessageId>) {
    match flag {
        IncludeFlag::TitleBlock => (ids::NOTEBOOK_PDF_TITLE_BLOCK, None),
        IncludeFlag::Contents => (ids::NOTEBOOK_PDF_CONTENTS, None),
        IncludeFlag::Bookmarks => (
            ids::NOTEBOOK_PDF_BOOKMARKS,
            Some(ids::NOTEBOOK_PDF_BOOKMARKS_DETAIL),
        ),
        IncludeFlag::PythonSource => (
            ids::NOTEBOOK_PDF_PYTHON_SOURCE,
            Some(ids::NOTEBOOK_PDF_PYTHON_SOURCE_DETAIL),
        ),
    }
}

/// A paper by name: `A4 portrait`.
pub(super) fn paper_words(paper: &PaperSize, i18n: &I18n) -> String {
    let name = match paper.name {
        PaperName::A4 => t!(i18n, ids::NOTEBOOK_PDF_PAPER_A4),
        PaperName::Letter => t!(i18n, ids::NOTEBOOK_PDF_PAPER_LETTER),
        PaperName::Other => t!(i18n, ids::NOTEBOOK_PDF_PAPER_OTHER),
    }
    .into_string();
    let id = if paper.landscape {
        ids::NOTEBOOK_PDF_LANDSCAPE
    } else {
        ids::NOTEBOOK_PDF_PORTRAIT
    };
    t!(i18n, id, "paper" => name).into_string()
}

/// The Paper list: the document's paper (`From the document (A4
/// portrait)`), A4 and Letter. A pick list draws plain text, so the labels
/// carry no bidi isolation marks.
pub(super) fn paper_labels(document: Option<&PaperSize>, i18n: &I18n) -> Vec<(Paper, String)> {
    let document = document.map_or_else(
        || t!(i18n, ids::NOTEBOOK_PDF_PAPER_OTHER).into_string(),
        |paper| paper_words(paper, i18n),
    );
    Paper::ALL
        .into_iter()
        .map(|paper| {
            let label = match paper {
                Paper::FromDocument => i18n.message_plain(
                    ids::NOTEBOOK_PDF_PAPER_DOCUMENT,
                    &[("paper", Arg::from(document.as_str()))],
                ),
                Paper::A4 => i18n.message_plain(ids::NOTEBOOK_PDF_PAPER_A4, &[]),
                Paper::Letter => i18n.message_plain(ids::NOTEBOOK_PDF_PAPER_LETTER, &[]),
            };
            (paper, label)
        })
        .collect()
}

/// One entry of the Paper and Variant lists.
#[derive(Debug, Clone, PartialEq)]
struct Choice<V> {
    value: V,
    label: String,
}

/// Header, the form, and the footer.
pub(super) fn parts<'a>(
    dialog: &'a NotebookPdfDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Parts<'a> {
    let close = Some(out(M::Closed));
    let header = super::super::header::graphite_header_closing(
        t!(i18n, ids::NOTEBOOK_PDF_TITLE).into_string(),
        Some(subtitle(dialog, i18n)),
        None,
        close.clone(),
        super::super::header::DIALOG_HEADER_H,
        tokens,
    );
    let body = scrollable(container(body(dialog, i18n, tokens)).padding(SP.l + SP.xs))
        .direction(crate::styles::thin_vertical())
        .height(Length::Fill)
        .boxed();
    (header, body, footer(dialog, i18n, tokens))
}

fn body<'a>(
    dialog: &'a NotebookPdfDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l);
    let inspection = match &dialog.stage {
        Stage::Reading => {
            return kit::caption(t!(i18n, ids::NOTEBOOK_PDF_READING), tokens);
        }
        Stage::Unreadable(why) => {
            let words = text(t!(i18n, ids::NOTEBOOK_PDF_UNREADABLE, "why" => why.as_str()))
                .size(TS.body)
                .boxed();
            return kit::banner(PillTone::Err, words, None, tokens);
        }
        Stage::Ready(inspection) | Stage::Exporting(inspection) => inspection,
        Stage::Done(_) => return container(iced::widget::Space::new()).boxed(),
    };
    for banner in banners(inspection, i18n, tokens) {
        col = col.push(banner);
    }
    col = col
        .push(settings_grid(dialog, inspection, i18n, tokens))
        .push(include_group(dialog, i18n, tokens))
        .push(kinds_group(dialog, i18n, tokens))
        .push(rule_and_path(dialog, i18n, tokens))
        .push(kit::caption(
            t!(i18n, ids::NOTEBOOK_PDF_REPRODUCIBLE),
            tokens,
        ));
    col.boxed()
}

/// The compile-error banner and the stale banner, when they apply.
fn banners<'a>(
    inspection: &NotebookInspection,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Vec<Element<'a, Message>> {
    let mut banners = Vec::new();
    let errors: Vec<_> = inspection
        .diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();
    if let Some(first) = errors.first() {
        let first = format!("{}:{}: {}", first.file, first.line, first.message);
        let words = t!(
            i18n,
            ids::NOTEBOOK_PDF_COMPILE_ERRORS,
            "count" => errors.len(),
            "first" => first
        );
        banners.push(kit::banner(
            PillTone::Err,
            text(words).size(TS.body).boxed(),
            None,
            tokens,
        ));
    }
    let stale = &inspection.stale.stale;
    if !stale.is_empty() {
        let names: Vec<String> = stale.iter().map(|r| r.name()).collect();
        let words = t!(
            i18n,
            ids::NOTEBOOK_PDF_STALE_BANNER,
            "count" => stale.len(),
            "names" => names.join(", ")
        );
        let review = dialog_button(
            t!(i18n, ids::NOTEBOOK_PDF_REVIEW),
            DialogButtonKind::Secondary,
            Some(out(M::ReviewRequested)),
            tokens,
        );
        banners.push(kit::banner(
            PillTone::Warn,
            text(words).size(TS.body).boxed(),
            Some(review),
            tokens,
        ));
    }
    banners
}

/// A labelled line of the form: the label, then the control and its
/// caption.
fn line<'a>(
    label: Localized,
    control: Element<'a, Message>,
    caption: Option<Element<'a, Message>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = crate::styles::tokens::resolve(tokens).ink3;
    let mut controls: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(control);
    if let Some(caption) = caption {
        controls = controls.push(caption);
    }
    Row::new()
        .spacing(SP.l)
        .align_y(Alignment::Center)
        .push(
            container(text(label).size(TS.body).color(ink))
                .width(Length::FillPortion(1))
                .boxed(),
        )
        .push(container(controls).width(Length::FillPortion(4)).boxed())
        .boxed()
}

/// Pages, Paper and Variant.
fn settings_grid<'a>(
    dialog: &'a NotebookPdfDialog,
    inspection: &NotebookInspection,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let style = kit::segmented_style(tokens).compact(true);
    let pages = segmented(
        [
            (
                PageChoice::All,
                Label::from(
                    t!(i18n, ids::NOTEBOOK_PDF_PAGES_ALL, "count" => inspection.page_count),
                ),
            ),
            (
                PageChoice::Current,
                Label::from(t!(i18n, ids::NOTEBOOK_PDF_PAGES_CURRENT)),
            ),
            (
                PageChoice::Range,
                Label::from(t!(i18n, ids::NOTEBOOK_PDF_PAGES_RANGE)),
            ),
        ],
        dialog.choice,
        style,
        |choice| out(M::PagesPicked(choice)),
    );
    let range = text_input(
        t!(i18n, ids::NOTEBOOK_PDF_RANGE_PLACEHOLDER),
        &dialog.range_text,
    )
    .on_input(|text| out(M::RangeEdited(text)))
    .style(text_input_style)
    .size(TS.body)
    .width(Length::FillPortion(1));
    let range = kit::dependent(dialog.choice == PageChoice::Range, range.boxed(), tokens);
    let pages = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(pages)
        .push(range)
        .boxed();

    let papers: Vec<Choice<Paper>> = paper_labels(inspection.paper.as_ref(), i18n)
        .into_iter()
        .map(|(value, label)| Choice { value, label })
        .collect();
    let picked = papers
        .iter()
        .find(|c| c.value == dialog.options.paper)
        .cloned();
    let paper = pick_list(picked, papers, |c: &Choice<Paper>| c.label.clone())
        .graphite()
        .on_select(|c: Choice<Paper>| out(M::PaperPicked(c.value)))
        .width(Length::FillPortion(1))
        .boxed();

    let variants: Vec<Choice<String>> = dialog
        .variants
        .iter()
        .map(|v| Choice {
            value: v.clone(),
            label: v.clone(),
        })
        .collect();
    let picked = variants
        .iter()
        .find(|c| c.value == dialog.options.variant)
        .cloned();
    let variant = pick_list(picked, variants, |c: &Choice<String>| c.label.clone())
        .graphite()
        .on_select(|c: Choice<String>| out(M::VariantPicked(c.value)))
        .width(Length::FillPortion(1))
        .boxed();
    let variant_caption = t!(
        i18n,
        ids::NOTEBOOK_PDF_VARIANT_CAPTION,
        "variant" => dialog.options.variant.as_str()
    );

    Column::new()
        .spacing(SP.m)
        .push(line(t!(i18n, ids::NOTEBOOK_PDF_PAGES), pages, None, tokens))
        .push(line(
            t!(i18n, ids::NOTEBOOK_PDF_PAPER),
            paper,
            Some(kit::caption(
                t!(i18n, ids::NOTEBOOK_PDF_PAPER_CAPTION),
                tokens,
            )),
            tokens,
        ))
        .push(line(
            t!(i18n, ids::NOTEBOOK_PDF_VARIANT),
            variant,
            Some(kit::caption(variant_caption, tokens)),
            tokens,
        ))
        .boxed()
}

/// The Include ticks, two to a line, each with its note.
fn include_group<'a>(
    dialog: &NotebookPdfDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut grid: Column<Element<'a, Message>> = Column::new().spacing(SP.s);
    for pair in IncludeFlag::ALL.chunks(2) {
        let mut row: Row<Element<'a, Message>> = Row::new().spacing(SP.xl);
        for flag in pair {
            let (label, detail) = include_label(*flag);
            let mut cell: Column<Element<'a, Message>> = Column::new().push(kit::tick(
                flag.get(&dialog.options),
                t!(i18n, label),
                Some(out(M::IncludeToggled(*flag))),
            ));
            if let Some(detail) = detail {
                cell = cell.push(kit::under_tick(kit::caption(t!(i18n, detail), tokens)));
            }
            row = row.push(container(cell).width(Length::FillPortion(1)).boxed());
        }
        grid = grid.push(row.boxed());
    }
    Column::new()
        .spacing(SP.s)
        .push(kit::section_head(
            t!(i18n, ids::NOTEBOOK_PDF_INCLUDE),
            None,
            i18n,
            tokens,
        ))
        .push(grid.boxed())
        .boxed()
}

/// The annotation kinds, as the Design Notebook PDF output of Output Jobs
/// has them.
fn kinds_group<'a>(
    dialog: &NotebookPdfDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut kinds: Row<Element<'a, Message>> = Row::new().spacing(SP.l).align_y(Alignment::Center);
    for kind in AnnotationKind::ALL {
        kinds = kinds.push(kit::tick(
            dialog.options.annotations.contains(&kind),
            t!(i18n, kind_label(kind)),
            Some(out(M::KindToggled(kind))),
        ));
    }
    Column::new()
        .spacing(SP.s)
        .push(kit::section_head(
            t!(i18n, ids::NOTEBOOK_PDF_ANNOTATIONS),
            Some(t!(i18n, ids::NOTEBOOK_PDF_ANNOTATIONS_CAPTION)),
            i18n,
            tokens,
        ))
        .push(kinds.boxed())
        .boxed()
}

/// Stale references and Save to.
fn rule_and_path<'a>(
    dialog: &'a NotebookPdfDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let style = kit::segmented_style(tokens).compact(true);
    let rule = segmented(
        [
            (
                StaleRule::Warn,
                Label::from(t!(i18n, ids::NOTEBOOK_PDF_STALE_WARN)),
            ),
            (
                StaleRule::Fail,
                Label::from(t!(i18n, ids::NOTEBOOK_PDF_STALE_FAIL)),
            ),
        ],
        dialog.options.stale,
        style,
        |rule| out(M::StaleRulePicked(rule)),
    );
    let field = text_input(t!(i18n, ids::NOTEBOOK_PDF_SAVE_TO), &dialog.path_text)
        .on_input(|text| out(M::PathEdited(text)))
        .style(text_input_style)
        .size(TS.body)
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .width(Length::Fill);
    let browse = dialog_button(
        t!(i18n, ids::NOTEBOOK_PDF_BROWSE),
        DialogButtonKind::Secondary,
        Some(out(M::BrowseRequested)),
        tokens,
    );
    let path = Row::<Element<'a, Message>>::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(field.boxed())
        .push(browse)
        .boxed();
    Column::new()
        .spacing(SP.m)
        .push(line(
            t!(i18n, ids::NOTEBOOK_PDF_STALE),
            rule,
            Some(kit::caption(
                t!(i18n, ids::NOTEBOOK_PDF_STALE_CAPTION),
                tokens,
            )),
            tokens,
        ))
        .push(line(
            t!(i18n, ids::NOTEBOOK_PDF_SAVE_TO),
            path,
            None,
            tokens,
        ))
        .boxed()
}

/// Whether Export can run now, and whether it reads Export anyway.
pub(super) fn export_state(dialog: &NotebookPdfDialog) -> (bool, bool) {
    let Stage::Ready(inspection) = &dialog.stage else {
        return (false, false);
    };
    let compiles = inspection
        .diagnostics
        .iter()
        .all(|d| d.severity != DiagnosticSeverity::Error);
    let stale = !inspection.stale.stale.is_empty();
    let allowed = compiles
        && inspection.page_count > 0
        && dialog.page_selection().is_some()
        && !(stale && dialog.options.stale == StaleRule::Fail);
    (allowed, stale)
}

fn footer<'a>(
    dialog: &NotebookPdfDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (allowed, anyway) = export_state(dialog);
    let export_label = if matches!(dialog.stage, Stage::Exporting(_)) {
        ids::NOTEBOOK_PDF_EXPORTING
    } else if anyway {
        ids::NOTEBOOK_PDF_EXPORT_ANYWAY
    } else {
        ids::NOTEBOOK_PDF_EXPORT
    };
    // Export Jobs is not a document tab yet, so there is no job to add the
    // output to; the link says so on hover.
    let add = dialog_button(
        t!(i18n, ids::NOTEBOOK_PDF_ADD_OUTPUT_JOB),
        DialogButtonKind::Link,
        None,
        tokens,
    );
    let note = container(text(t!(i18n, ids::NOTEBOOK_PDF_ADD_OUTPUT_JOB_LATER)).size(TS.body))
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(tokens));
    let add = tooltip(add, note, tooltip::Position::Top).boxed();
    dialog_footer_split(
        vec![add],
        vec![
            dialog_button(
                t!(i18n, ids::NOTEBOOK_PDF_CANCEL),
                DialogButtonKind::Cancel,
                Some(out(M::Closed)),
                tokens,
            ),
            dialog_button(
                t!(i18n, export_label),
                super::super::graphite::call_to_action(tokens),
                allowed.then(|| out(M::ExportRequested)),
                tokens,
            ),
        ],
    )
}
