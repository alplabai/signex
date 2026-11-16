//! The text view: the same rules as rule text, edited as text and applied
//! as one undo step, with what the compiler says about the text as typed.

use iced::widget::{Column, column, row, text_editor};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::{button, text};

use super::{Frame, SP, TS, up};
use crate::app::Message;
use crate::app::rules_tab::{RulesTabMsg, RulesText};
use crate::styles::tokens;

pub(super) fn view<'a>(frame: &Frame<'a>, rules_text: &'a RulesText) -> Element<'a, Message> {
    let (i18n, tokens, id) = (frame.i18n, frame.tokens, frame.id);
    let editor = text_editor(&rules_text.content)
        .on_action(move |action| {
            up(RulesTabMsg::TextActed {
                project_id: id,
                action,
            })
        })
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .size(TS.body)
        .padding(SP.m)
        .height(Length::Fill);
    let apply = button(text(t!(i18n, ids::RULES_TEXT_APPLY)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::accent_button(tokens))
        .on_press_maybe(
            rules_text
                .edited
                .then(|| up(RulesTabMsg::TextApplied { project_id: id })),
        );
    let discard = button(text(t!(i18n, ids::RULES_TEXT_DISCARD)).size(TS.label))
        .padding([SP.xs, SP.m])
        .style(crate::styles::subtle_button(tokens))
        .on_press_maybe(
            rules_text
                .edited
                .then(|| up(RulesTabMsg::TextDiscarded { project_id: id })),
        );
    // What the compiler says about the text as typed, not as stored; read
    // in `update` after each edit.
    let typed = rules_text.content.text();
    let table = &rules_text.checked;
    let said = table
        .syntax
        .iter()
        .chain(table.rows.iter().flat_map(|r| r.problems.iter()));
    let mut problems: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    let mut any = false;
    for d in said {
        any = true;
        let (line, column) = d.line_col(&typed);
        problems = problems.push(
            text(t!(
                i18n,
                ids::RULES_TEXT_PROBLEM,
                "line" => line,
                "column" => column,
                "message" => d.message.as_str()
            ))
            .size(TS.body)
            .color(tokens::danger(tokens))
            .boxed(),
        );
    }
    if !any {
        problems = problems.push(
            text(t!(i18n, ids::RULES_TEXT_CLEAN))
                .size(TS.body)
                .color(tokens::muted_text(tokens))
                .boxed(),
        );
    }
    column![
        editor,
        row![problems.width(Length::Fill), discard, apply]
            .spacing(SP.s)
            .align_y(Alignment::Start),
    ]
    .spacing(SP.m)
    .height(Length::Fill)
    .boxed()
}
