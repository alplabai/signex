//! Shared, theme-token-driven chrome for the Library modals: the header
//! close button and the footer secondary / primary action buttons.
//!
//! Every Library modal (picker, primitive picker, recovery, updates,
//! close-library prompt, edit-row) used to carry its own copy of these
//! with hard-coded `from_rgba` colours and pixel sizes. They now share
//! one definition built on [`crate::styles`] tokens, so a theme change
//! reaches all of them.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{Label, raw, text};

use super::messages::LibraryMessage;
use crate::styles::{
    DialogButtonKind, DialogIcon, LINE_HEIGHT, PillTone, Radii, Spacing, TypeScale, dialog_button,
    dialog_footer_split, pill_colors, ti, tokens,
};

/// Fixed widths of the Library modals that size themselves (the others
/// take the picker / dialog default). One home so a resize is one edit.
pub(crate) const CLOSE_PROMPT_W: f32 = 500.0;
pub(crate) const CREATE_OPTIONS_W: f32 = 560.0;
pub(crate) const DOCUMENT_OPTIONS_W: f32 = 460.0;
pub(crate) const EDIT_ROW_W: f32 = 860.0;
pub(crate) const EDIT_ROW_H: f32 = 640.0;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Padding of the muted note line a windowed dialog shows above its body.
const NOTE_LINE_PAD_Y: f32 = 6.0;
const NOTE_LINE_PAD_X: f32 = 14.0;

/// Height of the one-line note a windowed [`header_with_note_closing`]
/// dialog shows above its body (the window sizes count it).
pub(crate) const NOTE_LINE_H: f32 = TS.body * LINE_HEIGHT + 2.0 * NOTE_LINE_PAD_Y;

/// [`notice_banner`]: padding, gap between glyph and text, glyph size.
pub(crate) const BANNER_PAD_Y: f32 = 7.0;
pub(crate) const BANNER_PAD_X: f32 = 10.0;
pub(crate) const BANNER_GAP: f32 = 8.0;
pub(crate) const BANNER_GLYPH: f32 = 14.0;

/// [`key_value_box`]: inner padding, key column width, row and column gaps.
pub(crate) const KV_PAD: f32 = 10.0;
pub(crate) const KV_KEY_W: f32 = 96.0;
pub(crate) const KV_ROW_GAP: f32 = 5.0;
const KV_COL_GAP: f32 = 10.0;

/// What sits at the right end of a dialog header.
enum HeaderEnd {
    Nothing,
    Close(LibraryMessage),
    Note(String),
    /// A muted note, then the close button.
    NoteAndClose(String, LibraryMessage),
}

fn header_strip<'a>(
    title: &str,
    end: HeaderEnd,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    // In its own OS window the window's title bar carries the title and the
    // close: the slot stays empty (docs/DETACHED_MODALS.md).
    if windowed {
        // A note (the library or file the dialog acts on) is not in the
        // window title, so it stays, as a muted line above the body.
        let note_line = |note: String| {
            text(raw(note))
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
        };
        return match end {
            HeaderEnd::Note(note) => container(note_line(note))
                .padding([NOTE_LINE_PAD_Y, NOTE_LINE_PAD_X])
                .boxed(),
            // One line, cut at the end: the window height counts one line.
            HeaderEnd::NoteAndClose(note, _) => container(
                note_line(note)
                    .wrapping(Wrapping::None)
                    .ellipsis(Ellipsis::End)
                    .width(Length::Fill),
            )
            .padding([NOTE_LINE_PAD_Y, NOTE_LINE_PAD_X])
            .width(Length::Fill)
            .boxed(),
            HeaderEnd::Nothing | HeaderEnd::Close(_) => Space::new().height(0).boxed(),
        };
    }
    let (subtitle, close) = match end {
        HeaderEnd::Nothing => (None, None),
        HeaderEnd::Close(message) => (None, Some(message)),
        HeaderEnd::Note(note) => (Some(note), None),
        HeaderEnd::NoteAndClose(note, message) => (Some(note), Some(message)),
    };
    crate::app::view::dialogs::graphite_header_closing(
        title.to_string(),
        subtitle,
        None,
        close,
        crate::app::view::dialogs::DIALOG_HEADER_H,
        tokens,
    )
}

/// Header of a Library dialog: title and a close button, or the empty slot
/// when `windowed` (the OS window's title bar carries both). Every Library
/// dialog builds its header here.
pub(crate) fn header<'a>(
    title: &str,
    on_close: LibraryMessage,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    header_strip(title, HeaderEnd::Close(on_close), windowed, tokens)
}

/// Header with a muted note at the right end instead of a close button.
pub(crate) fn header_with_note<'a>(
    title: &str,
    note: String,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    header_strip(title, HeaderEnd::Note(note), windowed, tokens)
}

/// Header with an optional muted note (the library or project the dialog
/// is about) and the close button. In its own window the note stays as one
/// line above the body, as [`header_with_note`] keeps it.
pub(crate) fn header_with_note_closing<'a>(
    title: &str,
    note: Option<String>,
    on_close: LibraryMessage,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let end = match note {
        Some(note) => HeaderEnd::NoteAndClose(note, on_close),
        None => HeaderEnd::Close(on_close),
    };
    header_strip(title, end, windowed, tokens)
}

/// Header with the title alone (no close button).
pub(crate) fn header_title_only<'a>(
    title: &str,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    header_strip(title, HeaderEnd::Nothing, windowed, tokens)
}

/// Bordered secondary footer action (Cancel, Skip, ...).
pub(crate) fn secondary_btn<'a>(
    label: impl Into<Label<'a>>,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(label, DialogButtonKind::Secondary, Some(message), tokens)
}

/// The dismissing footer action (Cancel): Ghost on Graphite, bordered
/// elsewhere. It goes first in the right-aligned button row.
pub(crate) fn cancel_btn<'a>(
    label: impl Into<Label<'a>>,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(label, DialogButtonKind::Cancel, Some(message), tokens)
}

/// Accent-filled primary footer action.
pub(crate) fn primary_btn<'a>(
    label: impl Into<Label<'a>>,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    primary_btn_if(label, message, true, tokens)
}

/// Primary footer action that is inert (and drawn disabled) while
/// `enabled` is false.
pub(crate) fn primary_btn_if<'a>(
    label: impl Into<Label<'a>>,
    message: LibraryMessage,
    enabled: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(
        label,
        DialogButtonKind::Primary,
        enabled.then_some(message),
        tokens,
    )
}

/// Footer of a Library dialog on the flat footer strip: `extras` at the
/// left end, `buttons` right-aligned with the standard gaps.
pub(crate) fn footer_strip<'a>(
    extras: Vec<Element<'a, LibraryMessage>>,
    buttons: Vec<Element<'a, LibraryMessage>>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(dialog_footer_split(extras, buttons))
        .style(crate::styles::modal_footer_strip(tokens))
        .boxed()
}

/// Destructive footer action (Delete, Discard): error-coloured outline.
pub(crate) fn danger_btn<'a>(
    label: impl Into<Label<'a>>,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(label, DialogButtonKind::Danger, Some(message), tokens)
}

/// A notice inside a dialog body: `body` on the soft fill of `tone`, in the
/// tone's ink, after a status glyph; `actions` go in a row under the body.
/// The caller decides whether `body` wraps.
pub(crate) fn notice_banner<'a>(
    tone: PillTone,
    body: Element<'a, LibraryMessage>,
    actions: Vec<Element<'a, LibraryMessage>>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let (fill, ink) = pill_colors(tokens, tone);
    let glyph = match tone {
        PillTone::Err => DialogIcon::Warn,
        _ => DialogIcon::Info,
    };
    let has_actions = !actions.is_empty();
    let mut lines: Column<Element<'a, LibraryMessage>> =
        column![container(body).width(Length::Fill)]
            .spacing(SP.s)
            .width(Length::Fill);
    if has_actions {
        lines = lines.push(
            row(actions)
                .spacing(SP.s)
                .align_y(Alignment::Center)
                .boxed(),
        );
    }
    let line = row![glyph.glyph_element(BANNER_GLYPH, ink), lines.boxed()]
        .spacing(BANNER_GAP)
        .align_y(if has_actions {
            Alignment::Start
        } else {
            Alignment::Center
        });
    container(line)
        .padding([BANNER_PAD_Y, BANNER_PAD_X])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            text_color: Some(ink),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The facts of a dialog as key / value rows in a bordered box: each key
/// muted in a fixed column, its value beside it.
pub(crate) fn key_value_box<'a>(
    rows: Vec<(signex_i18n::Localized, Element<'a, LibraryMessage>)>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let key_ink = tokens::resolve(tokens).ink4;
    let line = ti(tokens.border);
    let mut grid: Column<Element<'a, LibraryMessage>> =
        Column::new().spacing(KV_ROW_GAP).width(Length::Fill);
    for (key, value) in rows {
        let key = container(
            text(key)
                .size(TS.caption)
                .color(key_ink)
                .wrapping(Wrapping::None),
        )
        .width(Length::Fixed(KV_KEY_W))
        .clip(true);
        grid = grid.push(
            row![key.boxed(), container(value).width(Length::Fill).boxed()]
                .spacing(KV_COL_GAP)
                .align_y(Alignment::Center)
                .boxed(),
        );
    }
    container(grid)
        .padding(KV_PAD)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: crate::styles::HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A path or an id in the mono face on one line, cut in the middle when it
/// does not fit; hovering shows it whole.
pub(crate) fn mono_value<'a>(value: String, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let shown: Element<'a, LibraryMessage> = text(signex_i18n::UserText::new(value.clone()))
        .size(TS.body)
        .font(mono)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::Middle)
        .width(Length::Fill)
        .boxed();
    let whole = container(
        text(signex_i18n::UserText::new(value))
            .size(TS.body)
            .font(mono),
    )
    .padding(SP.s)
    .style(crate::styles::context_menu(tokens));
    signex_widgets::tooltip(shown, whole, signex_widgets::tooltip::Position::Top).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::{ids, t};
    use signex_types::theme::ThemeId;

    /// The chrome builds from tokens alone for every built-in theme; no
    /// hard-coded colour path remains to diverge per theme.
    #[test]
    fn chrome_builds_for_every_theme() {
        let i18n = signex_i18n::I18n::en_us_ref();
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            for windowed in [false, true] {
                let _ = header("Title", LibraryMessage::ClosePicker, windowed, &tokens);
                let _ = header_with_note("Title", "note".into(), windowed, &tokens);
                let _ = header_title_only("Title", windowed, &tokens);
                for note in [None, Some("note".to_owned())] {
                    let _ = header_with_note_closing(
                        "Title",
                        note,
                        LibraryMessage::ClosePicker,
                        windowed,
                        &tokens,
                    );
                }
            }
            for tone in [PillTone::Warn, PillTone::Err, PillTone::Info] {
                let _ = notice_banner(
                    tone,
                    text(t!(i18n, ids::DIALOG_BUTTON_OK)).boxed(),
                    Vec::new(),
                    &tokens,
                );
                let _ = notice_banner(
                    tone,
                    text(t!(i18n, ids::DIALOG_BUTTON_OK)).boxed(),
                    vec![secondary_btn(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        LibraryMessage::ClosePicker,
                        &tokens,
                    )],
                    &tokens,
                );
            }
            let _ = key_value_box(Vec::new(), &tokens);
            let _ = key_value_box(
                vec![(
                    t!(i18n, ids::DIALOG_BUTTON_OK),
                    text(t!(i18n, ids::DIALOG_BUTTON_CANCEL)).boxed(),
                )],
                &tokens,
            );
            let _ = mono_value("/libs/a/a.snxlib".to_owned(), &tokens);
            let _ = secondary_btn(
                t!(i18n, ids::LIBRARY_UPDATES_SKIP_ALL),
                LibraryMessage::ClosePicker,
                &tokens,
            );
            let _ = cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::ClosePicker,
                &tokens,
            );
            let _ = primary_btn(
                t!(i18n, ids::DIALOG_BUTTON_OK),
                LibraryMessage::ClosePicker,
                &tokens,
            );
            let _ = primary_btn_if(
                t!(i18n, ids::DIALOG_BUTTON_OK),
                LibraryMessage::ClosePicker,
                false,
                &tokens,
            );
            let _ = danger_btn(
                t!(i18n, ids::DIALOG_BUTTON_DELETE),
                LibraryMessage::ClosePicker,
                &tokens,
            );
        }
    }
}
