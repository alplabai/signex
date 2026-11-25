//! The "Copy to Library" dialog. A pure read of [`StockCopyState`] and the
//! open libraries' names; every colour comes from theme tokens.
//!
//! Select, then confirm (mockup row 134): the writable libraries are radio
//! rows, one preselected when the dialog opens (see
//! [`LibraryState::stock_copy_choice`]), and the footer's primary "Copy to
//! <library>" copies into the selected one. New Library… is a quiet action
//! at the footer's left. With no writable library a card says so, and New
//! Library… is the primary.

use std::path::{Path, PathBuf};

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use super::{StockCopyMsg, StockCopyState};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{footer_strip, header, notice_banner};
use crate::library::state::LibraryState;
use crate::styles::{
    DialogButtonKind, DialogIcon, HAIRLINE, PillTone, Radii, Spacing, TypeScale, dialog_button,
    dialog_icon_button, dialog_intro, ti, tokens,
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
/// size of its radio and library glyph.
const ROW_PAD_X: f32 = 8.0;
const ROW_GAP: f32 = 8.0;
const GLYPH: f32 = 14.0;
/// The radio's selected dot inside its ring.
const RADIO_DOT: f32 = GLYPH * 4.0 / 7.0;

/// What a footer button does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FooterAction {
    /// Ask for a new library to create and copy into.
    NewLibrary,
    Cancel,
    /// Copy into the library at this `.snxlib` path, called by this name.
    CopyTo(PathBuf, String),
}

/// The footer's buttons: `left` at the left end, `right` right-aligned,
/// each with the role it is drawn with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FooterPlan {
    pub left: Vec<(FooterAction, DialogButtonKind)>,
    pub right: Vec<(FooterAction, DialogButtonKind)>,
}

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

/// The message a click on the row of `root` sends: it selects that
/// library; nothing is copied until the footer's primary is pressed.
pub(crate) fn row_message(root: &Path) -> LibraryMessage {
    LibraryMessage::StockCopy(StockCopyMsg::TargetSelected(root.to_path_buf()))
}

/// The footer for the selected library `choice`: Cancel and one primary
/// (mockup row 134, DECISIONS 2026-11-08). With a library to copy into, the
/// primary copies there and New Library… is quiet at the left; with none,
/// New Library… is the primary.
pub(crate) fn footer_plan(choice: Option<(&Path, &str)>) -> FooterPlan {
    let cancel = (FooterAction::Cancel, DialogButtonKind::Cancel);
    match choice {
        Some((root, name)) => FooterPlan {
            left: vec![(FooterAction::NewLibrary, DialogButtonKind::Ghost)],
            right: vec![
                cancel,
                (
                    FooterAction::CopyTo(root.to_path_buf(), name.to_owned()),
                    DialogButtonKind::Primary,
                ),
            ],
        },
        None => FooterPlan {
            left: Vec::new(),
            right: vec![
                cancel,
                (FooterAction::NewLibrary, DialogButtonKind::Primary),
            ],
        },
    }
}

/// The message a footer action sends.
pub(crate) fn footer_message(action: &FooterAction) -> LibraryMessage {
    LibraryMessage::StockCopy(match action {
        FooterAction::NewLibrary => StockCopyMsg::NewLibraryRequested,
        FooterAction::Cancel => StockCopyMsg::Cancelled,
        FooterAction::CopyTo(root, _) => StockCopyMsg::TargetPicked(root.clone()),
    })
}

/// The label of a footer action.
pub(crate) fn footer_label(action: &FooterAction, i18n: &I18n) -> Localized {
    match action {
        FooterAction::NewLibrary => t!(i18n, ids::LIBRARY_STOCK_COPY_NEW_LIBRARY),
        FooterAction::Cancel => t!(i18n, ids::DIALOG_BUTTON_CANCEL),
        FooterAction::CopyTo(_, name) => {
            t!(i18n, ids::LIBRARY_STOCK_COPY_COPY_TO, "library" => shown_library_name(name))
        }
    }
}

/// The longest library name "Copy to <library>" shows. The dialog buttons
/// are as wide as their labels, so a longer name would push the primary
/// past the card; at this length New Library…, Cancel and the primary fit
/// [`CARD_W`] (pinned by the long-name layout test).
pub(crate) const COPY_TO_NAME_MAX_CHARS: usize = 20;

/// `name` as the primary shows it: cut in the middle to
/// [`COPY_TO_NAME_MAX_CHARS`] when longer, keeping its start and its end.
/// The library row above shows the full name.
pub(crate) fn shown_library_name(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= COPY_TO_NAME_MAX_CHARS {
        return name.to_owned();
    }
    let tail = (COPY_TO_NAME_MAX_CHARS - 1) / 2;
    let head = COPY_TO_NAME_MAX_CHARS - 1 - tail;
    let mut shown: String = chars[..head].iter().collect();
    shown.push('…');
    shown.extend(&chars[chars.len() - tail..]);
    shown
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
    let header = header(&title, cancel, windowed, tokens);

    let writable = library.writable_libraries();
    let choice = library.stock_copy_choice(state.selected.as_deref());
    let body = body(state, &writable, choice.map(|(root, _)| root), i18n, tokens);
    let footer = footer(&footer_plan(choice), i18n, tokens);

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

/// The intro, the notes, then the library list (or the empty card).
fn body<'a>(
    state: &'a StockCopyState,
    writable: &[(&Path, &str)],
    selected: Option<&Path>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
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
    if writable.is_empty() {
        body = body.push(empty_card(i18n, tokens));
    } else {
        body = body.push(library_list(writable, selected, tokens));
    }
    body.boxed()
}

/// Cancel, then the one primary, at the right; New Library… at the left,
/// with the plus glyph, while it is not the primary (mockup row 134).
fn footer<'a>(plan: &FooterPlan, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let buttons = |side: &[(FooterAction, DialogButtonKind)],
                   glyph: bool|
     -> Vec<Element<'a, LibraryMessage>> {
        side.iter()
            .map(|(action, kind)| {
                let (label, message) = (footer_label(action, i18n), Some(footer_message(action)));
                if glyph {
                    let plus = crate::icons::icon_plus(crate::about_modal::GLYPH_THEME);
                    dialog_icon_button(plus, label, *kind, message, tokens)
                } else {
                    dialog_button(label, *kind, message, tokens)
                }
            })
            .collect()
    };
    footer_strip(
        buttons(&plan.left, true),
        buttons(&plan.right, false),
        tokens,
    )
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

/// The writable libraries as radio rows in a bordered box; `selected` is
/// the row whose radio is on.
fn library_list<'a>(
    writable: &[(&Path, &str)],
    selected: Option<&Path>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let rows = writable.iter().map(|&(root, name)| {
        let on = selected == Some(root);
        library_row(root.to_path_buf(), name, on, tokens)
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
/// A click anywhere on the row selects that library.
fn library_row<'a>(
    root: PathBuf,
    name: &str,
    selected: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let folder = root
        .parent()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default();
    let pick = row_message(&root);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let line = row![
        radio_mark(selected, tokens),
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
    let (hover, picked) = (tokens::hover_overlay(tokens), r.accent_soft);
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
            let fill = if selected {
                Some(picked)
            } else {
                hovered.then_some(hover)
            };
            iced::widget::button::Style {
                background: fill.map(Background::Color),
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

/// The radio: a ring in the strong line colour on the field fill; selected,
/// an accent ring around an accent dot.
fn radio_mark<'a>(selected: bool, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let r = tokens::resolve(tokens);
    let accent = ti(tokens.accent);
    let ring = if selected { accent } else { r.line_strong };
    let field = r.field;
    let dot = container(Space::new())
        .width(Length::Fixed(RADIO_DOT))
        .height(Length::Fixed(RADIO_DOT))
        .style(move |_: &Theme| container::Style {
            background: selected.then_some(Background::Color(accent)),
            border: Border {
                radius: (RADIO_DOT / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(dot)
        .width(Length::Fixed(GLYPH))
        .height(Length::Fixed(GLYPH))
        .center_x(Length::Fixed(GLYPH))
        .center_y(Length::Fixed(GLYPH))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(field)),
            border: Border {
                width: HAIRLINE,
                color: ring,
                radius: (GLYPH / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
