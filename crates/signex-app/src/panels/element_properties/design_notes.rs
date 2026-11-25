//! Properties ▸ Design notes of a selected part (mockup row 197): the
//! notebook blocks about the part or its pins, each with its kind, title,
//! status and date, and Open in notebook. A row opens the notebook at its
//! block; the section is left out when the notebook does not write about
//! the part. The blocks are the part's Notebook badge
//! (`signex_notebook::badges`).

use iced::widget::{Column, Row, container};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_notebook::{Badge, BadgeTarget, BlockMention, badge_of};
use signex_widgets::theme_ext::annotation_kind_colour;
use signex_widgets::{button, text};

use crate::app::notebook_references::NotebookReferenceMsg;
use crate::panels::annotations::{kind_label, status_label, tone};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Collapse key of the Design notes section.
pub(super) const DESIGN_NOTES_KEY: &str = "sel_design_notes";

fn msg(m: NotebookReferenceMsg) -> PanelMsg {
    PanelMsg::NotebookReferences(m)
}

/// The blocks the notebook writes about `designator`, notebook order.
pub(crate) fn design_notes<'b>(badges: &'b [Badge], designator: &str) -> &'b [BlockMention] {
    badge_of(badges, &BadgeTarget::Part(designator.to_string()))
        .map_or(&[][..], |badge| badge.blocks.as_slice())
}

/// The message a block row sends: open the notebook at it.
pub(crate) fn opened(block: &BlockMention) -> PanelMsg {
    msg(NotebookReferenceMsg::BlockOpened {
        file: block.file.clone(),
        line: block.line,
    })
}

/// Append Design notes for the part shown as `designator`, when the
/// notebook writes about it.
pub(super) fn push_design_notes_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    designator: &str,
) -> Column<Element<'a, PanelMsg>> {
    let blocks = design_notes(&ctx.notebook_badges, designator);
    if blocks.is_empty() {
        return col;
    }
    col.push(
        ctx.collapsible(
            DESIGN_NOTES_KEY,
            ids::PROPERTIES_SECTION_DESIGN_NOTES,
            &ctx.collapsed_sections,
            move || notes_column(ctx, blocks),
        )
        .boxed(),
    )
}

fn notes_column<'a>(
    ctx: &'a PanelContext,
    blocks: &'a [BlockMention],
) -> Column<Element<'a, PanelMsg>> {
    let mut column: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.xs)
        .padding([SP.xs, SP.m])
        .width(Length::Fill);
    for block in blocks {
        column = column.push(note_row(ctx, block));
    }
    let open = dialog_button(
        t!(ctx.i18n, ids::NOTEBOOK_DESIGN_NOTES_OPEN),
        DialogButtonKind::Secondary,
        blocks.first().map(opened),
        &ctx.tokens,
    );
    column.push(open)
}

/// One block: the kind in its colour and the title, then the status and
/// the date. A click opens the notebook at the block.
fn note_row<'a>(ctx: &'a PanelContext, block: &'a BlockMention) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let kind_ink = annotation_kind_colour(tone(block.kind), &ctx.tokens);
    let head: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(
            text(kind_label(&ctx.i18n, block.kind))
                .size(TS.caption)
                .color(kind_ink)
                .boxed(),
        )
        .push(
            text(UserText::new(block.title.as_str()))
                .size(TS.body)
                .color(ti(ctx.tokens.text))
                .width(Length::Fill)
                .boxed(),
        );
    let mut meta: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.s);
    if let Some(status) = block.status {
        meta = meta.push(
            text(status_label(&ctx.i18n, status))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    if let Some(date) = block.date {
        meta = meta.push(
            text(UserText::new(date.to_string()))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    let lines: Column<Element<'a, PanelMsg>> = Column::new().push(head.boxed()).push(meta.boxed());
    button(container(lines).width(Length::Fill))
        .on_press(opened(block))
        .padding(0)
        .style(button::text)
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::layout_probe::{find, lay_out_texts, messages_on_click};

    const WINDOW: iced::Size = iced::Size::new(320.0, 400.0);

    fn badges() -> Vec<Badge> {
        let (blocks, _) = signex_notebook::scan(
            "#annotation(kind: \"calculation\", status: \"current\", title: \"Output voltage\")[#component(\"R7\")]\n\
             #annotation(kind: \"issue\", status: \"open\", date: \"2026-11-24\", title: \"Worst-case Vout\")[#pin(\"R7\", \"1\")]\n",
        );
        signex_notebook::badges([("design.snxnbk", blocks.as_slice())])
    }

    fn ctx() -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.notebook_badges = Arc::new(badges());
        ctx
    }

    fn section<'a>(ctx: &'a PanelContext, designator: &str) -> Element<'a, PanelMsg> {
        push_design_notes_section(Column::new(), ctx, designator).boxed()
    }

    #[test]
    fn the_blocks_about_a_part_and_its_pins_are_its_design_notes() {
        let badges = badges();
        let notes = design_notes(&badges, "R7");
        let titles: Vec<_> = notes.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, ["Output voltage", "Worst-case Vout"]);
        assert!(design_notes(&badges, "C9").is_empty());
    }

    #[test]
    fn each_note_shows_its_kind_title_status_and_date() {
        let ctx = ctx();
        let texts = lay_out_texts(section(&ctx, "R7"), WINDOW);
        find(&texts, "Design notes");
        find(&texts, "Calculation");
        find(&texts, "Output voltage");
        find(&texts, "Up to date");
        find(&texts, "Issue");
        find(&texts, "Open");
        find(&texts, "2026-11-24");
        find(&texts, "Open in notebook");
    }

    #[test]
    fn a_part_the_notebook_does_not_write_about_has_no_section() {
        let ctx = ctx();
        let texts = lay_out_texts(section(&ctx, "C9"), WINDOW);
        assert!(texts.iter().all(|t| t.text != "Design notes"), "{texts:?}");
    }

    #[test]
    fn a_row_and_open_in_notebook_open_the_notebook_at_the_block() {
        let ctx = ctx();
        let opened_at = |messages: &[PanelMsg]| match messages {
            [PanelMsg::NotebookReferences(NotebookReferenceMsg::BlockOpened { file, line })] => {
                Some((file.clone(), *line))
            }
            _ => None,
        };
        let row = messages_on_click(section(&ctx, "R7"), WINDOW, "Worst-case Vout");
        assert_eq!(opened_at(&row), Some(("design.snxnbk".into(), 2)));
        let open = messages_on_click(section(&ctx, "R7"), WINDOW, "Open in notebook");
        assert_eq!(opened_at(&open), Some(("design.snxnbk".into(), 1)));
    }
}
