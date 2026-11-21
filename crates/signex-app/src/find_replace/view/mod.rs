//! The Find and Replace dialog (#67, #2525): a header with the Find /
//! Replace switch, the Find field (match pill and the Match case / Whole
//! word / Regex toggles), the Replace field, the scope switch, the results
//! grouped by kind and the footer. Pure: it reads the state, nothing else.

mod footer;
mod form;
mod results;

#[cfg(test)]
mod tests;

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, SegmentedStyle, segmented};

use super::{FindReplaceMsg, FindReplaceState};
use crate::styles::{Spacing, TypeScale, chrome_px, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Card width: a result line and its sheet fit on one row.
const DIALOG_W: f32 = chrome_px(500.0 * 11.0 / 12.0);
/// Card height of the in-window dialog; the results list takes the rest.
const DIALOG_H: f32 = chrome_px(480.0 * 11.0 / 12.0);
/// Width of the form's label column (Find, Replace, Scope).
const LABEL_W: f32 = chrome_px(72.0 * 11.0 / 12.0);

/// The dialog painted as a centred card on a scrim in the main window.
pub fn view<'a>(
    state: &'a FindReplaceState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    build(state, tokens, i18n, false)
}

/// The dialog as the content of its own OS window: no header (the native
/// title bar has the title and the close), filling the window. The Find /
/// Replace switch moves to the top of the form.
pub fn window_view<'a>(
    state: &'a FindReplaceState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    build(state, tokens, i18n, true)
}

/// The header's Find / Replace switch: it changes the mode in place.
fn mode_switch<'a>(
    state: &FindReplaceState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let options: [(bool, Label<'a>); 2] = [
        (false, t!(i18n, ids::FIND_MODE_FIND).into()),
        (true, t!(i18n, ids::FIND_MODE_REPLACE).into()),
    ];
    segmented(
        options,
        state.replace_mode,
        segmented_style(tokens, TS.caption),
        FindReplaceMsg::ModeChanged,
    )
}

/// The Graphite segmented look, from the tokens.
fn segmented_style(theme_tokens: &ThemeTokens, text_size: f32) -> SegmentedStyle {
    let r = tokens::resolve(theme_tokens);
    r.segmented(text_size)
}

fn build<'a>(
    state: &'a FindReplaceState,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, FindReplaceMsg> {
    let title = t!(
        i18n,
        if state.replace_mode {
            ids::DIALOG_TITLE_FIND_REPLACE
        } else {
            ids::FIND_TITLE
        }
    );
    let mut top = column![].spacing(SP.m);
    if windowed {
        top = top.push(mode_switch(state, theme_tokens, i18n));
    }
    let top = top.push(form::form(state, theme_tokens, i18n));

    let list = results::results(state, theme_tokens, i18n);
    let mut body = column![list].spacing(SP.s).height(Length::Fill);
    if let Some(note) = &state.skipped_note {
        body = body.push(results::skipped_line(note, theme_tokens, i18n));
    }

    let header: Element<'a, FindReplaceMsg> = if windowed {
        Space::new().height(0).boxed()
    } else {
        crate::app::view::dialogs::graphite_header(
            title.into_string(),
            None,
            Some(mode_switch(state, theme_tokens, i18n)),
            FindReplaceMsg::Close,
            crate::app::view::dialogs::DIALOG_HEADER_H,
            theme_tokens,
        )
    };
    let (dialog_w, dialog_h) = if windowed {
        (Length::Fill, Length::Fill)
    } else {
        (Length::Fixed(DIALOG_W), Length::Fixed(DIALOG_H))
    };
    let dialog = container(
        column![
            header,
            container(top).padding([SP.l, SP.xl]),
            container(body).padding([0.0, SP.xl]).height(Length::Fill),
            footer::footer(state, theme_tokens, i18n),
        ]
        .width(dialog_w)
        .height(dialog_h),
    )
    .style(crate::styles::modal_card(theme_tokens));
    if windowed {
        return dialog.boxed();
    }

    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                dialog,
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::app::view::dialogs::scrim_style(theme_tokens))
    .boxed()
}
