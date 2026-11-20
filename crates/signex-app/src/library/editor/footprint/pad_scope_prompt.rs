//! The "This pad / All pads" card (mockup row 192, item D1, decision 9, Caner 2026-11-20):
//! shown over the footprint canvas while an edit of a patterned pad waits
//! for where it goes. Pure view; the answer is
//! `FootprintEditorMsg::PadScopeAnswered`.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::pad_scope::PadScope;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::{DialogButtonKind, dialog_button};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The card at the canvas's top centre; an empty layer when no edit waits.
pub fn overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let Some(prompt) = editor.state.pad_scope.as_deref() else {
        return Space::new().boxed();
    };
    let answer = |scope: PadScope| {
        Some(LibraryMessage::PrimitiveEditorEvent {
            path: editor.path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::PadScopeAnswered(scope)),
        })
    };
    let question = text(t!(
        i18n,
        ids::EDITORS_PAD_SCOPE_QUESTION,
        "designator" => prompt.after.number.as_str()
    ))
    .size(TS.body)
    .color(theme_ext::text_primary(tokens));
    let buttons = row![
        dialog_button(
            t!(i18n, ids::EDITORS_PAD_SCOPE_THIS_PAD),
            DialogButtonKind::Primary,
            answer(PadScope::ThisPad),
            tokens,
        ),
        dialog_button(
            t!(i18n, ids::EDITORS_PAD_SCOPE_ALL_PADS),
            DialogButtonKind::Secondary,
            answer(PadScope::AllPads),
            tokens,
        ),
        dialog_button(
            t!(i18n, ids::EDITORS_PAD_SCOPE_CANCEL),
            DialogButtonKind::Secondary,
            answer(PadScope::Cancel),
            tokens,
        ),
    ]
    .spacing(SP.s);
    let card = container(column![question, buttons].spacing(SP.m))
        .padding([SP.m, SP.l])
        .style(crate::styles::modal_card(tokens));
    container(card)
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Top)
        .boxed()
}
