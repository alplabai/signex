//! The "Copy to Library" dialog. A pure read of [`StockCopyState`] and the
//! open libraries' names; every colour comes from theme tokens.
//!
//! The writable libraries are radio rows; the primary button copies into the
//! selected one. With no writable library a card says so and New Library…
//! is the primary.

use std::path::{Path, PathBuf};

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, radio, raw, text};

use super::{StockCopyMsg, StockCopyState};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{cancel_btn, footer_strip, header, notice_banner, primary_btn};
use crate::library::state::LibraryState;
use crate::styles::{
    DialogButtonKind, DialogIcon, HAIRLINE, PillTone, Radii, Spacing, TypeScale, dialog_button,
    dialog_intro, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the dialog card in the main window.
pub(crate) const CARD_W: f32 = 520.0;

/// Height of one library row.
pub(crate) const ROW_H: f32 = 28.0;
/// Rows shown before the list scrolls.
pub(crate) const LIST_MAX_ROWS: usize = 6;
/// Padding inside the bordered list box.
pub(crate) const LIST_PAD: f32 = 4.0;
/// Vertical padding of the no-writable-library card.
pub(crate) const EMPTY_CARD_PAD_Y: f32 = 18.0;
const EMPTY_CARD_PAD_X: f32 = 12.0;
/// Inner horizontal padding of a row, the gap between its parts, and the
/// sizes of its radio and library glyph.
const ROW_PAD_X: f32 = 8.0;
const ROW_GAP: f32 = 8.0;
const RADIO: f32 = 14.0;
const GLYPH: f32 = 14.0;

/// The intro's title and secondary text; the window height follows their
/// wrapped line counts (`app/handlers/modal_sizing.rs`).
pub(crate) fn intro_texts(i18n: &I18n, label: &str) -> (String, String) {
    (
        t!(i18n, ids::LIBRARY_STOCK_COPY_INTRO_TITLE, "label" => label.to_owned()).into_string(),
        t!(i18n, ids::LIBRARY_STOCK_COPY_INTRO_TEXT).into_string(),
    )
}

/// The note shown when the library used before can no longer take the copy.
pub(crate) fn previous_note(i18n: &I18n, previous: &Path) -> Localized {
    t!(
        i18n,
        ids::LIBRARY_STOCK_COPY_PREVIOUS_GONE,
        "path" => previous.display().to_string()
    )
}

/// Height of the library list box for `rows` rows, border included: as
/// tall as its rows, up to [`LIST_MAX_ROWS`], then it scrolls.
pub(crate) fn list_height(rows: usize) -> f32 {
    rows.min(LIST_MAX_ROWS) as f32 * ROW_H + 2.0 * (LIST_PAD + HAIRLINE)
}

/// The selected library, if it is still one of the writable ones.
fn selected_library<'l>(
    writable: &[(&'l Path, &'l str)],
    selected: Option<&Path>,
) -> Option<(usize, &'l Path, &'l str)> {
    let selected = selected?;
    writable
        .iter()
        .enumerate()
        .find(|(_, (root, _))| *root == selected)
        .map(|(index, (root, name))| (index, *root, *name))
}

pub fn view<'a>(
    library: &'a LibraryState,
    state: &'a StockCopyState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let cancel = LibraryMessage::StockCopy(StockCopyMsg::Cancelled);
    let title = t!(i18n, ids::DIALOG_TITLE_COPY_TO_LIBRARY).into_string();
    let header = header(&title, cancel.clone(), windowed, tokens);

    let (intro_title, intro_text) = intro_texts(i18n, &state.label);
    let intro = dialog_intro(DialogIcon::Copy, intro_title, Some(intro_text), tokens);

    let mut body: Column<Element<'a, LibraryMessage>> = column![intro].spacing(SP.m);
    if let Some(previous) = &state.previous_unavailable {
        body = body.push(notice_banner(
            PillTone::Warn,
            text(previous_note(i18n, previous)).size(TS.body).boxed(),
            Vec::new(),
            tokens,
        ));
    }
    if let Some(error) = &state.error {
        body = body.push(notice_banner(
            PillTone::Err,
            text(raw(error.as_str())).size(TS.body).boxed(),
            Vec::new(),
            tokens,
        ));
    }

    let writable = library.writable_libraries();
    let selected = selected_library(&writable, state.selected.as_deref());
    let new_library = LibraryMessage::StockCopy(StockCopyMsg::NewLibraryRequested);
    let new_library_label = t!(i18n, ids::LIBRARY_STOCK_COPY_NEW_LIBRARY);
    let cancel_button = cancel_btn(t!(i18n, ids::DIALOG_BUTTON_CANCEL), cancel, tokens);
    let footer = if writable.is_empty() {
        // Nothing to copy into: creating a library is the one way on.
        body = body.push(empty_card(i18n, tokens));
        footer_strip(
            Vec::new(),
            vec![
                cancel_button,
                primary_btn(new_library_label, new_library, tokens),
            ],
            tokens,
        )
    } else {
        let selected_index = selected.map(|(index, _, _)| index);
        body = body.push(library_list(&writable, selected_index, tokens));
        let confirm = match selected {
            Some((_, _, name)) => {
                t!(i18n, ids::LIBRARY_STOCK_COPY_TO, "library" => name.to_owned())
            }
            None => t!(i18n, ids::LIBRARY_STOCK_COPY_COPY),
        };
        let copy = selected.map(|(_, root, _)| {
            LibraryMessage::StockCopy(StockCopyMsg::TargetPicked(root.to_path_buf()))
        });
        footer_strip(
            vec![dialog_button(
                new_library_label,
                DialogButtonKind::Secondary,
                Some(new_library),
                tokens,
            )],
            vec![
                cancel_button,
                dialog_button(confirm, DialogButtonKind::Primary, copy, tokens),
            ],
            tokens,
        )
    };

    // In the main window the card is as tall as its content; in its own
    // window it fills the window, which is sized to the content.
    let mut card = column![header, container(body).padding(SP.l)];
    if windowed {
        card = card.push(Space::new().height(Length::Fill).boxed());
    }
    let height = if windowed {
        Length::Fill
    } else {
        Length::Shrink
    };
    container(
        card.push(footer)
            .width(crate::library::card_len(windowed, CARD_W))
            .height(height),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// The card shown when no open library can take the copy.
fn empty_card<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let r = tokens::resolve(tokens);
    let (ink, line) = (r.ink3, r.line_strong);
    container(
        text(t!(i18n, ids::LIBRARY_STOCK_COPY_NO_WRITABLE))
            .size(TS.body)
            .color(ink)
            .align_x(iced::alignment::Horizontal::Center)
            .width(Length::Fill),
    )
    .padding([EMPTY_CARD_PAD_Y, EMPTY_CARD_PAD_X])
    .width(Length::Fill)
    // iced draws no dashed border: the strong line stands in for it.
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The writable libraries as radio rows in a bordered box; the selected row
/// is tinted.
fn library_list<'a>(
    writable: &[(&Path, &str)],
    selected: Option<usize>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let rows = writable.iter().enumerate().map(|(index, (root, name))| {
        library_row(index, root.to_path_buf(), name, selected, tokens)
    });
    let line = ti(tokens.border);
    let inner = list_height(writable.len()) - 2.0 * (LIST_PAD + HAIRLINE);
    container(
        scrollable(column(rows).width(Length::Fill))
            .height(Length::Fixed(inner))
            .width(Length::Fill),
    )
    .padding(LIST_PAD)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// One library: radio, library glyph, name, and its folder in muted mono.
/// A click anywhere on the row selects it; nothing is copied yet.
fn library_row<'a>(
    index: usize,
    root: PathBuf,
    name: &str,
    selected: Option<usize>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = tokens::resolve(tokens);
    let is_selected = selected == Some(index);
    let ink = if is_selected {
        r.accent_soft_ink
    } else {
        ti(tokens.text)
    };
    let folder = root
        .parent()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default();
    let pick = LibraryMessage::StockCopy(StockCopyMsg::TargetSelected(root));
    let on_radio = pick.clone();
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let line = row![
        radio(UserText::new(String::new()), index, selected, move |_| {
            on_radio
        })
        .size(RADIO)
        .boxed(),
        DialogIcon::Library.glyph_element(GLYPH, ink),
        text(UserText::new(name.to_owned()))
            .size(TS.body)
            .color(ink)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill)
            .boxed(),
        text(UserText::new(folder))
            .size(TS.caption)
            .font(mono)
            .color(r.ink4)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::Middle)
            .align_x(iced::alignment::Horizontal::Right)
            .width(Length::Fill)
            .boxed(),
    ]
    .spacing(ROW_GAP)
    .align_y(Alignment::Center);
    let (tint, hover) = (r.accent_soft, tokens::hover_overlay(tokens));
    button(line)
        .height(Length::Fixed(ROW_H))
        .width(Length::Fill)
        .padding([0.0, ROW_PAD_X])
        .on_press(pick)
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            let background = if is_selected {
                Some(Background::Color(tint))
            } else if hovered {
                Some(Background::Color(hover))
            } else {
                None
            };
            iced::widget::button::Style {
                background,
                text_color: ink,
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .boxed()
}
