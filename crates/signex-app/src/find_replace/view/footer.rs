//! The footer: "Match 2 of 5" and, in Replace mode, the one-undo-step note,
//! Replace Current and the primary button that counts the included results
//! ("Replace 1 of 5", #2525).

use iced::widget::column;
use iced::{Element, Widget};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::{SP, TS};
use crate::find_replace::{FindReplaceMsg, FindReplaceState};
use crate::styles::{DialogButtonKind, dialog_button, dialog_footer_split, tokens};

/// "Match 2 of 5" for the current result; `None` with none selected.
pub(super) fn position_line(state: &FindReplaceState, i18n: &I18n) -> Option<Localized> {
    let total = state.result_count();
    let current = state.selected_index.filter(|&i| i < total)? + 1;
    Some(t!(
        i18n,
        ids::FIND_MATCH_POSITION,
        "current" => current,
        "total" => total
    ))
}

/// The primary button's label: how many results Replace acts on, of all.
pub(super) fn replace_label(state: &FindReplaceState, i18n: &I18n) -> Localized {
    t!(
        i18n,
        ids::FIND_REPLACE_INCLUDED,
        "included" => state.included_count(),
        "total" => state.result_count()
    )
}

pub(super) fn footer<'a>(
    state: &FindReplaceState,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let muted = tokens::muted_text(theme_tokens);
    let mut info: iced::widget::Column<Element<'a, FindReplaceMsg>> = column![].spacing(SP.xs);
    if let Some(line) = position_line(state, i18n) {
        info = info.push_boxed(text(line).size(TS.caption).color(muted));
    }
    if state.replace_mode {
        info = info.push_boxed(
            text(t!(i18n, ids::FIND_ONE_UNDO_STEP))
                .size(TS.caption)
                .color(muted),
        );
    }
    let mut buttons: Vec<Element<'a, FindReplaceMsg>> = Vec::new();
    if state.replace_mode {
        let idle = !state.is_busy();
        let current =
            (idle && state.selected_index.is_some()).then_some(FindReplaceMsg::ReplaceCurrent);
        let all = (idle && state.included_count() > 0).then_some(FindReplaceMsg::ReplaceAll);
        buttons.push(dialog_button(
            t!(i18n, ids::FIND_REPLACE_CURRENT),
            DialogButtonKind::Secondary,
            current,
            theme_tokens,
        ));
        buttons.push(dialog_button(
            replace_label(state, i18n),
            DialogButtonKind::Primary,
            all,
            theme_tokens,
        ));
    }
    dialog_footer_split(vec![info.boxed()], buttons)
}
