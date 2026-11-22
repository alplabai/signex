//! Graphite header of the component editor (mockup row 128): the internal
//! PN in mono, a `v<version>` pill, the accent "• unsaved" pill while dirty
//! and the "N fields do not parse" pill, a button that shows the first bad
//! field; the library / table caption under them; the close button at
//! the right. The save actions live in the footer (`footer.rs`).

use iced::widget::{Space, Widget, column, container, row};
use iced::{Element, Length, Theme};
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::{button, text, theme_ext, tooltip};

use super::kit::{self, SP, TS};
use crate::library::messages::{EditorMsg, LibraryMessage};
use crate::library::state::{ComponentPreviewState, EditorAddress};
use crate::styles::PillTone;

/// Shown in place of a blank internal part number.
const NO_PART_NUMBER: &str = "(no part number)";

/// Height of the header band: two field heights (52 px at the default size).
pub(super) fn header_height() -> f32 {
    2.0 * crate::styles::ChromeMetrics::DEFAULT.field
}

/// The library's display name: its folder name without `.snxlib`.
fn library_name(state: &ComponentPreviewState) -> String {
    state
        .library_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn event(address: &EditorAddress, msg: EditorMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    }
}

pub(super) fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &I18n,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let location = text(t!(
        i18n,
        ids::EDITORS_COMPONENT_HEADER_LOCATION,
        "library" => library_name(state),
        "table" => state.table.as_str()
    ))
    .size(TS.caption)
    .color(crate::styles::tokens::resolve(tokens).ink4);
    let identity = column![
        identity_line(state, tokens, i18n, &address),
        location.boxed()
    ]
    .spacing(SP.xs)
    .width(Length::Shrink);
    let line: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        identity.boxed(),
        Space::new().width(Length::Fill).boxed(),
        close_button(tokens, i18n, &address),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    container(line.boxed())
        .padding(iced::Padding {
            top: 0.0,
            right: SP.m,
            bottom: 0.0,
            left: SP.xl,
        })
        .width(Length::Fill)
        .height(Length::Fixed(header_height()))
        .center_y(Length::Fixed(header_height()))
        .style(crate::styles::modal_header_strip(tokens))
        .boxed()
}

/// Internal PN, the version pill, the unsaved pill and the parse pill.
fn identity_line<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let pn = state.row.internal_pn.as_str();
    let pn = if pn.trim().is_empty() {
        NO_PART_NUMBER
    } else {
        pn
    };
    let mono = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
    };
    let mut line: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        text(UserText::new(pn))
            .size(TS.title)
            .font(mono)
            .color(theme_ext::text_primary(tokens))
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let version = state.row.version.trim();
    if !version.is_empty() {
        line = line.push(kit::toned_pill(
            t!(i18n, ids::EDITORS_COMPONENT_HEADER_VERSION, "version" => version).to_string(),
            PillTone::Neutral,
            tokens,
        ));
    }
    if state.dirty {
        line = line.push(kit::toned_pill(
            t!(i18n, ids::EDITORS_COMPONENT_UNSAVED_MARKER).to_string(),
            PillTone::Accent,
            tokens,
        ));
    }
    // Measurements typed on the Parameters tab that did not parse. A press
    // shows the first one, outlined; Save stays disabled meanwhile.
    let unparsed = state.params_error.len();
    if unparsed > 0 {
        let pill = kit::toned_pill(
            t!(i18n, ids::EDITORS_COMPONENT_HEADER_UNPARSED, "count" => unparsed).to_string(),
            PillTone::Err,
            tokens,
        );
        let hover = crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover);
        line = line.push(
            button(pill)
                .padding(0)
                .on_press(event(address, EditorMsg::UnparsedFieldsPillPressed))
                .style(move |_: &Theme, status| iced::widget::button::Style {
                    background: matches!(status, iced::widget::button::Status::Hovered)
                        .then_some(iced::Background::Color(hover)),
                    border: iced::Border {
                        radius: crate::styles::Radii::GRAPHITE.pill.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                })
                .boxed(),
        );
    }
    line.boxed()
}

/// The × at the right: the editor's close path, which asks about unsaved
/// edits.
fn close_button<'a>(
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let ink = crate::styles::tokens::resolve(tokens).ink3;
    let size = crate::styles::ChromeMetrics::DEFAULT.button;
    let icon = iced::widget::svg(crate::icons::icon_chrome_window_close(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(TS.label)
    .height(TS.label)
    .style(move |_: &Theme, _| iced::widget::svg::Style {
        color: Some(ink),
        ..Default::default()
    });
    let close = button(
        container(icon)
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size)),
    )
    .padding(0)
    .on_press(event(address, EditorMsg::CloseEditor))
    .style(crate::styles::ghost_button(tokens));
    tooltip(
        close,
        container(text(t!(i18n, ids::EDITORS_COMPONENT_CLOSE)).size(TS.caption))
            .padding([SP.xs, SP.s])
            .style(crate::styles::context_menu(tokens)),
        iced::widget::tooltip::Position::Bottom,
    )
    .boxed()
}
