//! The Filter panel: what a click, a box and the hover pick select. It
//! follows the active editor (Graphite mockup #55): on a schematic tab it
//! edits the schematic filter and its saved presets (SCH Filter is retired
//! into it, mockup rows 40 and 55), on a board the board filter. It draws
//! one toggle column (`kinds`) and edits the filters the Active Bar
//! dropdowns edit, with their messages.

mod counts;
mod kinds;
mod presets;
#[cfg(test)]
mod tests;

use iced::widget::{Column, Row, container};
use iced::{Alignment, Element, Length, Widget as _};
use pcb_geom::PickKind;
use signex_i18n::{MessageId, ids, t};
use signex_widgets::text;

pub use counts::{FilterEditor, KindCounts, SelectionFilterFacts, board_counts};

use super::{PanelContext, PanelMsg, PcbBoardMsg, PcbBoardSummary, empty_state_card};
use crate::active_bar::{SelectionFilter, editing_line};
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// What the Filter panel sends on a schematic tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchFilterMsg {
    /// A kind's toggle was clicked: turn that kind on or off.
    KindToggled(SelectionFilter),
    /// All: every kind on.
    AllPicked,
    /// None: every kind off.
    NonePicked,
    /// A saved preset (by index) was clicked: its kinds replace the set,
    /// and it is the preset the Presets section edits.
    PresetApplied(usize),
    /// Save current: a new preset holding the kinds that are on.
    PresetSaved,
    /// The edited preset's name was clicked: open it for typing.
    PresetRenameStarted(usize),
    /// Text typed into the open name field.
    PresetRenameTyped(String),
    /// Enter in the name field: the preset takes the typed name.
    PresetRenamed(usize, String),
    PresetDeleted(usize),
}

fn sch(msg: SchFilterMsg) -> PanelMsg {
    PanelMsg::SchFilter(msg)
}

fn board(msg: PcbBoardMsg) -> PanelMsg {
    PanelMsg::PcbBoard(msg)
}

/// The Filter panel (Graphite mockup #55): the active editor's kinds, with the
/// saved presets on a schematic tab (mockup rows 40 and 55), or the empty card with
/// no schematic or board tab.
pub(super) fn filter_view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let (editor, kinds, caption, editing) =
        match (ctx.selection_filter.editor, ctx.pcb_board.as_ref()) {
            (FilterEditor::Schematic, _) => (
                ids::PANELS_FILTER_EDITOR_SCHEMATIC,
                schematic_kinds(ctx),
                ids::PANELS_FILTER_COUNTS_SHEET,
                editing_line(
                    &ctx.i18n,
                    &ctx.custom_filter_presets,
                    ctx.selection_filter.editing,
                ),
            ),
            (FilterEditor::Board, Some(summary)) => (
                ids::PANELS_FILTER_EDITOR_BOARD,
                board_kinds(ctx, summary),
                ids::PANELS_FILTER_COUNTS_BOARD,
                editing_line(&ctx.i18n, &summary.presets, summary.preset_editing),
            ),
            _ => {
                return empty_state_card(
                    &ctx.tokens,
                    t!(ctx.i18n, ids::PANELS_SELECTION_FILTER_TITLE),
                    t!(ctx.i18n, ids::PANELS_SELECTION_FILTER_BODY),
                    None,
                );
            }
        };
    let muted = resolve(&ctx.tokens).ink3;
    let caption =
        container(text(t!(ctx.i18n, caption)).size(TS.caption).color(muted)).padding([SP.xs, SP.m]);
    // While a preset is selected for editing the chips change it too
    // (#2873): say which.
    let mut column = Column::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(editor_header(ctx, editor))
        .push(kinds);
    if let Some(line) = editing {
        let line = container(text(line).size(TS.caption).color(muted)).padding([SP.xs, SP.m]);
        column = column.push(line.boxed());
    }
    column = column.push(caption.boxed());
    // The schematic presets section shows on a schematic tab only.
    if ctx.selection_filter.editor == FilterEditor::Schematic {
        column = column.push(presets::section(ctx));
    }
    column.boxed()
}

/// The editor's name in semibold and that the panel follows the active
/// editor, over a hairline.
fn editor_header<'a>(ctx: &'a PanelContext, editor: MessageId) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let name = text(t!(ctx.i18n, editor))
        .size(TS.body)
        .color(ti(ctx.tokens.text))
        .font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..signex_widgets::ui_font::regular()
        });
    let follows = text(t!(ctx.i18n, ids::PANELS_FILTER_FOLLOWS))
        .size(TS.caption)
        .color(r.ink3);
    let line: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(name.boxed())
        .push(follows.boxed());
    Column::new()
        .width(Length::Fill)
        .push(container(line).padding([SP.s, SP.m]).boxed())
        .push(super::thin_sep(r.line_soft))
        .boxed()
}

/// All / None, the readout and one toggle per schematic kind, with counts
/// on a schematic tab.
fn schematic_kinds<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let on = &ctx.selection_filters;
    let facts = &ctx.selection_filter;
    let counts = (facts.editor == FilterEditor::Schematic).then_some(&facts.schematic);
    Column::new()
        .width(Length::Fill)
        .push(kinds::header(
            ctx,
            on.len(),
            SelectionFilter::ALL.len(),
            sch(SchFilterMsg::AllPicked),
            sch(SchFilterMsg::NonePicked),
        ))
        .push(kinds::toggle_column(
            ctx,
            |kind| on.contains(&kind),
            counts,
            |kind| sch(SchFilterMsg::KindToggled(kind)),
        ))
        .boxed()
}

/// The board's kinds, editing the board filter with the messages of its
/// Active Bar dropdown and Properties chips.
fn board_kinds<'a>(ctx: &'a PanelContext, summary: &'a PcbBoardSummary) -> Element<'a, PanelMsg> {
    let filter = &summary.filter;
    Column::new()
        .width(Length::Fill)
        .push(kinds::header(
            ctx,
            filter.on_kinds().count(),
            PickKind::ALL.len(),
            board(PcbBoardMsg::FilterAllSet(true)),
            board(PcbBoardMsg::FilterAllSet(false)),
        ))
        .push(kinds::toggle_column(
            ctx,
            |kind| filter.is_on(kind),
            Some(&ctx.selection_filter.board),
            |kind| board(PcbBoardMsg::FilterKindToggled(kind)),
        ))
        .boxed()
}
