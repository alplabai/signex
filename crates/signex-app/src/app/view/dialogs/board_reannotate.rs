//! The Positional Re-Annotate dialog (#1312): the order and bottom-side
//! options, the renames they give, the parts kept, and Execute. State and
//! rules: `app/board_reannotate/`.

use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_engine::pcb::designators::{BottomSideOrder, Kept, KeptReason, ReannotateOrder};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::sizes;
use crate::app::{BoardReannotateMsg, BoardReannotateState, Message, ReannotateProblem, Signex};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of each designator column.
const COLUMN_W: f32 = TS.body * 10.0;

fn ra(msg: BoardReannotateMsg) -> Message {
    Message::from(msg)
}

fn s(i18n: &I18n, id: MessageId) -> String {
    t!(i18n, id).into_string()
}

impl Signex {
    pub(in crate::app::view) fn view_board_reannotate_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_board_reannotate_card(), sizes::BOARD_REANNOTATE)
    }

    fn view_board_reannotate_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.board_reannotate.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(ra(BoardReannotateMsg::CancelRequested));
        let header = super::header::graphite_header_closing(
            s(i18n, ids::PCB_REANNOTATE_TITLE),
            Some(
                t!(i18n, ids::PCB_REANNOTATE_SUBTITLE, "board" => state.board_name.as_str())
                    .into_string(),
            ),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let body = scrollable(container(body(state, i18n, tokens)).padding([SP.l, SP.xl]))
            .height(Length::Fill);
        let footer = dialog_footer_split(
            Vec::new(),
            vec![
                dialog_button(
                    t!(i18n, ids::PCB_REANNOTATE_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::PCB_REANNOTATE_EXECUTE),
                    super::graphite::call_to_action(tokens),
                    state
                        .can_execute()
                        .then(|| ra(BoardReannotateMsg::ExecuteRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::BOARD_REANNOTATE.0),
        )))
        .height(Length::Fixed(sizes::BOARD_REANNOTATE.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The intro, the options, the problem if any, the summary, the renames and
/// the parts kept.
fn body<'a>(
    state: &'a BoardReannotateState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let ink = ti(tokens.text);
    let options = state.options;
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l);
    col = col.push(
        text(t!(i18n, ids::PCB_REANNOTATE_INTRO))
            .size(TS.body)
            .color(muted)
            .boxed(),
    );
    col = col.push(
        text(t!(i18n, ids::PCB_DESIGNATOR_UPDATE_PCB_NOTE))
            .size(TS.body)
            .color(muted)
            .boxed(),
    );
    col = col.push(choices(
        t!(i18n, ids::PCB_REANNOTATE_ORDER),
        [
            (
                t!(i18n, ids::PCB_REANNOTATE_ORDER_COLUMNS),
                options.order == ReannotateOrder::XThenY,
                BoardReannotateMsg::OrderPicked(ReannotateOrder::XThenY),
            ),
            (
                t!(i18n, ids::PCB_REANNOTATE_ORDER_ROWS),
                options.order == ReannotateOrder::YThenX,
                BoardReannotateMsg::OrderPicked(ReannotateOrder::YThenX),
            ),
        ],
        tokens,
    ));
    col = col.push(choices(
        t!(i18n, ids::PCB_REANNOTATE_BOTTOM),
        [
            (
                t!(i18n, ids::PCB_REANNOTATE_BOTTOM_AFTER),
                options.bottom == BottomSideOrder::AfterTop,
                BoardReannotateMsg::BottomPicked(BottomSideOrder::AfterTop),
            ),
            (
                t!(i18n, ids::PCB_REANNOTATE_BOTTOM_WITH),
                options.bottom == BottomSideOrder::WithTop,
                BoardReannotateMsg::BottomPicked(BottomSideOrder::WithTop),
            ),
        ],
        tokens,
    ));
    if let Some(problem) = state.problem.as_ref() {
        col = col.push(
            text(problem_text(problem, i18n))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    let plan = &state.plan;
    let summary = if plan.renames.is_empty() {
        t!(i18n, ids::PCB_REANNOTATE_NOTHING)
    } else {
        t!(
            i18n,
            ids::PCB_REANNOTATE_SUMMARY,
            "renamed" => plan.renames.len(),
            "unchanged" => plan.unchanged,
            "kept" => plan.kept.len()
        )
    };
    col = col.push(text(summary).size(TS.body).color(ink).boxed());
    if !plan.renames.is_empty() {
        col = col.push(renames(state, i18n, tokens));
    }
    for kept in &plan.kept {
        col = col.push(
            text(kept_text(kept, i18n))
                .size(TS.body)
                .color(muted)
                .boxed(),
        );
    }
    col.boxed()
}

/// A caption over a row of toggle pills, one of them on.
fn choices<'a>(
    caption: Localized,
    options: [(Localized, bool, BoardReannotateMsg); 2],
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut pills: Row<Element<'a, Message>> = Row::new().spacing(SP.s);
    for (label, on, msg) in options {
        pills = pills.push(
            button(container(text(label).size(TS.body)).padding([SP.s, 10.0]))
                .on_press(ra(msg))
                .style(super::graphite_annotate::toggle_style(tokens, on))
                .boxed(),
        );
    }
    column![
        text(caption)
            .size(TS.label)
            .color(tokens::muted_text(tokens)),
        pills.wrap(),
    ]
    .spacing(SP.s)
    .boxed()
}

/// The renames as two columns: the designator now and the one it gets.
fn renames<'a>(
    state: &'a BoardReannotateState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let ink = ti(tokens.text);
    let cell = |label: signex_widgets::Label<'a>, color| {
        text(label)
            .size(TS.body)
            .color(color)
            .width(Length::Fixed(COLUMN_W))
    };
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.xs).push(
        row![
            cell(t!(i18n, ids::PCB_REANNOTATE_COL_FROM).into(), muted),
            cell(t!(i18n, ids::PCB_REANNOTATE_COL_TO).into(), muted),
        ]
        .boxed(),
    );
    for rename in &state.plan.renames {
        col = col.push(
            row![
                cell(UserText::new(rename.from.clone()).into(), ink),
                cell(UserText::new(rename.to.clone()).into(), ink),
            ]
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    col.boxed()
}

fn problem_text(problem: &ReannotateProblem, i18n: &I18n) -> Localized {
    match problem {
        ReannotateProblem::ReadOnly => t!(i18n, ids::PCB_REANNOTATE_READ_ONLY),
        ReannotateProblem::Stale => t!(i18n, ids::PCB_REANNOTATE_STALE),
        ReannotateProblem::Refused(why) => {
            t!(i18n, ids::PCB_REANNOTATE_REFUSED, "why" => why.as_str())
        }
    }
}

fn kept_text(kept: &Kept, i18n: &I18n) -> Localized {
    let designator = kept.reference.as_str();
    match kept.reason {
        KeptReason::Locked => {
            t!(i18n, ids::PCB_REANNOTATE_KEPT_LOCKED, "designator" => designator)
        }
        KeptReason::NoPrefix if designator.trim().is_empty() => {
            t!(i18n, ids::PCB_REANNOTATE_KEPT_EMPTY)
        }
        KeptReason::NoPrefix => {
            t!(i18n, ids::PCB_REANNOTATE_KEPT_NO_PREFIX, "designator" => designator)
        }
        KeptReason::Unlinked => {
            t!(i18n, ids::PCB_REANNOTATE_KEPT_UNLINKED, "designator" => designator)
        }
    }
}
