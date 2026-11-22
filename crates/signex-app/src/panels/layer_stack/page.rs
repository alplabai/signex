//! The tab's frame (mockup row 105): the shared document-tab shell with the
//! toolbar, the sub-tabs and a footer, around the Stack page (the table on
//! the left, the cross-section and preview on the right) or the Via Types /
//! Drill Pairs page.

use iced::widget::{column, container, row, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::doc_tab_shell::{DocTabShell, DocTabShellStyle, Footer, SubTab, SubTabs};
use signex_widgets::text;

use super::{
    LayerStackFacts, LayerStackMsg, SP, StackSubTab, TS, drills, msg, section, table, toolbar,
};
use crate::panels::{PanelContext, PanelMsg, empty_state_card};
use crate::styles::{ti, tokens::resolve};

/// The Stack page's share of the width for the table, and for the drawings.
const TABLE_SHARE: u16 = 3;
const DRAWING_SHARE: u16 = 2;

/// The tab's content, or its empty state when the board cannot be read.
pub fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let Some(facts) = ctx.layer_stack.as_ref() else {
        return empty_state_card(
            &ctx.tokens,
            t!(ctx.i18n, ids::PCB_STACK_NO_BOARD_TITLE),
            t!(ctx.i18n, ids::PCB_STACK_NO_BOARD),
            None,
        );
    };
    let body = match facts.sub_tab {
        StackSubTab::Stack => stack_page(ctx, facts),
        StackSubTab::Drills => drills::view(ctx, facts),
    };
    DocTabShell::new(body, DocTabShellStyle::from_tokens(&ctx.tokens, TS.body))
        .toolbar(toolbar::view(ctx, facts))
        .sub_tabs(sub_tabs(ctx, facts))
        .footer(footer(ctx, facts))
        .view()
}

fn sub_tabs<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> SubTabs<'a, PanelMsg> {
    let tabs = StackSubTab::ALL
        .into_iter()
        .map(|tab| SubTab::new(t!(ctx.i18n, tab.message())))
        .collect();
    let selected = StackSubTab::ALL
        .iter()
        .position(|tab| *tab == facts.sub_tab)
        .unwrap_or_default();
    let board = facts.board.clone();
    SubTabs::new(tabs, selected).on_select(move |index| {
        msg(LayerStackMsg::SubTabPicked {
            board: board.clone(),
            tab: StackSubTab::ALL
                .get(index)
                .copied()
                .unwrap_or(StackSubTab::Stack),
        })
    })
}

fn footer<'a>(ctx: &'a PanelContext, facts: &LayerStackFacts) -> Footer<'a> {
    let i18n = &*ctx.i18n;
    let copper = t!(i18n, ids::PCB_STACK_COPPER_COUNT, "count" => facts.stack.copper_count());
    let rows = t!(i18n, ids::PCB_STACK_ROW_COUNT, "count" => facts.stack.layers.len());
    let status = t!(
        i18n,
        ids::PCB_STACK_FOOTER,
        "copper" => copper.into_string(),
        "rows" => rows.into_string()
    );
    Footer::new(status).note(t!(i18n, ids::PCB_STACK_FOOTER_NOTE))
}

/// The table with its notes, beside the two drawings.
fn stack_page<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let left = scrollable(
        column![table::view(ctx, facts), notes(ctx, facts)]
            .spacing(SP.s)
            .padding([SP.s, 0.0])
            .width(Length::Fill),
    )
    .width(Length::FillPortion(TABLE_SHARE))
    .height(Length::Fill);
    let right = column![
        heading(ctx, ids::PCB_STACK_SECTION, ids::PCB_STACK_SECTION_HINT),
        section::cross_section(ctx, facts),
        heading(ctx, ids::PCB_STACK_PREVIEW, ids::PCB_STACK_PREVIEW_HINT),
        section::preview(ctx, facts),
    ]
    .spacing(SP.s)
    .padding(SP.m)
    .width(Length::FillPortion(DRAWING_SHARE))
    .height(Length::Fill);
    row![left, right].height(Length::Fill).boxed()
}

/// A drawing's title and its caption.
pub(super) fn heading<'a>(
    ctx: &'a PanelContext,
    title: MessageId,
    hint: MessageId,
) -> Element<'a, PanelMsg> {
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    row![
        text(t!(ctx.i18n, title))
            .size(TS.body)
            .font(semibold)
            .color(ti(ctx.tokens.text)),
        text(t!(ctx.i18n, hint))
            .size(TS.caption)
            .color(resolve(&ctx.tokens).ink3),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// Why nothing can be edited, that the stack is not stored yet, and the
/// Enter hint while a cell holds a draft.
fn notes<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let note = |id| text(t!(ctx.i18n, id)).size(TS.dense).color(muted);
    let mut col = column![].spacing(SP.xs);
    if facts.read_only {
        col = col.push(note(ids::PCB_STACK_READ_ONLY));
    } else if !facts.stored {
        col = col.push(note(ids::PCB_STACK_DERIVED));
    }
    if !facts.drafts.is_empty() {
        col = col.push(note(ids::PCB_STACK_ENTER_HINT));
    }
    container(col)
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
}
