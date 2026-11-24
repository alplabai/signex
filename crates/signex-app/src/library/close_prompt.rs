//! "Close Library" confirmation modal for a library with unsaved drafts.
//!
//! Mirrors the dock subsystem's `ProjectCloseConfirm` flow (see
//! `app/view/dialogs.rs::view_project_close_confirm_body`) — when
//! the user closes a library that still has at least one editor
//! window with `dirty = true`, the dispatcher diverts to
//! [`crate::library::messages::LibraryMessage::ConfirmCloseLibrary`]
//! and shows this modal listing every dirty draft so the user can
//! Save All / Discard All / Cancel.
//!
//! The header names the library; each draft row says whether it is a table
//! row or a file. The intro, list and footer come from the shared
//! unsaved-prompt chrome (`modal_chrome::unsaved_body` and
//! `unsaved_footer_parts`), as the Close Project and Exit confirms do.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Space, column, container, row};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use crate::styles::{HAIRLINE, Spacing, TypeScale, dialog_list, ti, tokens};

use super::messages::{CloseLibraryChoice, LibraryMessage};
use super::modal_chrome::{
    UnsavedActions, UnsavedBody, footer_strip, header_with_note_closing, unsaved_body,
    unsaved_footer_parts,
};
use super::state::{CloseLibraryConfirmState, LibraryState};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of one draft row.
pub(crate) const ROW_H: f32 = 28.0;
/// The most the draft list grows to before it scrolls, border included.
pub(crate) const LIST_MAX_H: f32 = 140.0;
/// Padding and border of the list box (`dialog_list`).
const LIST_CHROME: f32 = 2.0 * (SP.s + HAIRLINE);
/// Inner horizontal padding of a row and the gap before its kind.
const ROW_PAD_X: f32 = 8.0;
const ROW_GAP: f32 = 8.0;

/// How a confirm modal names an unsaved file: relative to its library's
/// folder when it has one (`vendor/symbols/s.snxsym`, so same-named drafts of
/// different folders differ), else its file name.
pub(crate) fn dirty_path_label(
    path: &std::path::Path,
    library_dir: Option<&std::path::Path>,
) -> String {
    let relative = library_dir.and_then(|dir| path.strip_prefix(dir).ok());
    match relative {
        Some(rest) => rest
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        None => path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("(unnamed)")
            .to_string(),
    }
}

/// The intro's title and secondary text; the window height follows their
/// wrapped line counts (`app/handlers/modal_sizing.rs`).
pub(crate) fn intro_texts(i18n: &I18n, library_name: &str, drafts: usize) -> (String, String) {
    (
        t!(i18n, ids::LIBRARY_CLOSE_INTRO_TITLE).into_string(),
        t!(
            i18n,
            ids::LIBRARY_CLOSE_INTRO_TEXT,
            "library" => library_name.to_owned(),
            "count" => drafts
        )
        .into_string(),
    )
}

/// Height of the draft list box for `rows` drafts, border included: as tall
/// as its rows, up to [`LIST_MAX_H`], then it scrolls.
pub(crate) fn list_height(rows: usize) -> f32 {
    (rows as f32 * ROW_H + LIST_CHROME).min(LIST_MAX_H)
}

pub fn view<'a>(
    state: &'a LibraryState,
    confirm: &'a CloseLibraryConfirmState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let cancel = LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::Cancel);
    let title = t!(i18n, ids::DIALOG_TITLE_CLOSE_LIBRARY).into_string();
    let header = header_with_note_closing(
        &title,
        Some(confirm.library_name.clone()),
        cancel.clone(),
        windowed,
        tokens,
    );

    let drafts = confirm.dirty_editors.len() + confirm.dirty_paths.len();
    let (intro_title, intro_text) = intro_texts(i18n, &confirm.library_name, drafts);

    // The drafts by internal part number, looked up in `state.editors` (keyed
    // by `EditorAddress`) so the modal stays accurate while other editors
    // open or close.
    let mut rows: Vec<Element<'_, LibraryMessage>> = Vec::new();
    for address in &confirm.dirty_editors {
        let label = state
            .editors
            .get(address)
            .map(|st| st.row.internal_pn.as_str().to_string())
            .unwrap_or_else(|| format!("({} / {})", address.table, address.row_id));
        let kind = t!(i18n, ids::LIBRARY_CLOSE_KIND_ROW, "table" => address.table.clone());
        rows.push(draft_row(label, kind, tokens));
    }
    let library_dir = confirm.library_path.parent();
    for path in &confirm.dirty_paths {
        rows.push(draft_row(
            dirty_path_label(path, library_dir),
            t!(i18n, ids::LIBRARY_CLOSE_KIND_FILE),
            tokens,
        ));
    }
    let list = container(dialog_list(rows, tokens)).height(Length::Fixed(list_height(drafts)));

    let body = unsaved_body(
        UnsavedBody {
            title: intro_title,
            detail: intro_text,
            above: Vec::new(),
            list: list.boxed(),
            below: Vec::new(),
        },
        tokens,
    );

    let (left, right) = unsaved_footer_parts(
        i18n,
        UnsavedActions {
            discard_all: Some(LibraryMessage::CloseLibraryConfirm(
                CloseLibraryChoice::DiscardAll,
            )),
            cancel,
            save_all: LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::SaveAll),
        },
        tokens,
    );
    let footer = footer_strip(left, right, tokens);

    // As tall as the content in the main window; in its own window (sized
    // to the content) the footer stays at the bottom.
    let mut card = column![header, body];
    if windowed {
        card = card.push(Space::new().height(Length::Fill).boxed());
    }
    container(
        card.push(footer)
            .width(super::card_len(
                windowed,
                super::modal_chrome::CLOSE_PROMPT_W,
            ))
            .height(if windowed {
                Length::Fill
            } else {
                Length::Shrink
            }),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// One draft: its name in mono, its kind muted on the right. The app ships
/// no edit glyph, so the row has none.
fn draft_row<'a>(
    label: String,
    kind: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    container(
        row![
            text(UserText::new(label))
                .size(TS.body)
                .font(mono)
                .color(ti(tokens.text))
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .width(Length::Fill)
                .boxed(),
            text(kind)
                .size(TS.caption)
                .color(tokens::resolve(tokens).ink4)
                .wrapping(Wrapping::None)
                .boxed(),
        ]
        .spacing(ROW_GAP)
        .align_y(Alignment::Center),
    )
    .padding([0.0, ROW_PAD_X])
    .center_y(Length::Fixed(ROW_H))
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list is as tall as its rows, then stops at the cap and scrolls.
    #[test]
    fn the_list_follows_its_rows_up_to_the_cap() {
        assert!(list_height(1) < list_height(2));
        assert!((list_height(2) - list_height(1) - ROW_H).abs() < 1e-3);
        assert_eq!(list_height(40), LIST_MAX_H);
    }

    /// The count line names the library and uses the plural the count asks
    /// for; "What would you like to do?" is gone.
    #[test]
    fn the_intro_counts_the_drafts() {
        let i18n = I18n::en_us();
        let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
        let (title, one) = intro_texts(&i18n, "passives", 1);
        assert_eq!(title, "Save the drafts before closing?");
        assert_eq!(plain(one), "'passives' has 1 unsaved draft.");
        let (_, three) = intro_texts(&i18n, "passives", 3);
        assert_eq!(plain(three), "'passives' has 3 unsaved drafts.");
    }

    fn one_file_confirm() -> CloseLibraryConfirmState {
        CloseLibraryConfirmState {
            library_path: "/libs/passives/passives.snxlib".into(),
            library_name: "passives".into(),
            dirty_editors: Vec::new(),
            dirty_paths: vec!["/libs/passives/symbols/resistor.snxsym".into()],
        }
    }

    /// On the shared unsaved-prompt footer (mockup row 136) Discard All
    /// stands at the left, Cancel and Save All at the right, and each button
    /// still answers with its own choice.
    #[test]
    fn the_footer_order_and_outcomes_hold() {
        use crate::layout_probe::{find, lay_out_texts, messages_on_click};
        let state = LibraryState::default();
        let confirm = one_file_confirm();
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::BUILTINS[0]);
        let i18n = I18n::en_us();
        let window = iced::Size::new(600.0, 500.0);
        let texts = lay_out_texts(view(&state, &confirm, &tokens, &i18n, false), window);
        let x = |label| find(&texts, label).bounds.x;
        assert!(x("Discard All") < x("Cancel") && x("Cancel") < x("Save All"));
        let choice = |label: &str| {
            let messages =
                messages_on_click(view(&state, &confirm, &tokens, &i18n, false), window, label);
            match messages.as_slice() {
                [LibraryMessage::CloseLibraryConfirm(choice)] => *choice,
                other => panic!("{label}: {other:?}"),
            }
        };
        assert_eq!(choice("Discard All"), CloseLibraryChoice::DiscardAll);
        assert_eq!(choice("Cancel"), CloseLibraryChoice::Cancel);
        assert_eq!(choice("Save All"), CloseLibraryChoice::SaveAll);
    }

    /// The prompt draws from tokens alone in every built-in theme.
    #[test]
    fn the_prompt_draws_in_every_theme() {
        let state = LibraryState::default();
        let confirm = one_file_confirm();
        let i18n = I18n::en_us();
        for id in signex_types::theme::ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            for windowed in [false, true] {
                drop(view(&state, &confirm, &tokens, &i18n, windowed));
            }
        }
    }
}
