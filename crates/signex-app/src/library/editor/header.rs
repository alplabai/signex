//! Graphite document header of the component editor: an identity line
//! (internal PN, lifecycle, class, version, library / table) over the
//! row's description and manufacturer part, then the parse-error chip, the
//! unsaved marker and Discard / Release... / Save & Close / Save.

use iced::widget::{Space, Widget, column, container, row};
use iced::{Element, Length};
use signex_i18n::{I18n, UserText, ids, t};
use signex_widgets::{text, theme_ext};

use super::kit::{self, SP, TS};
use crate::library::messages::{EditorMsg, LibraryMessage};
use crate::library::state::{ComponentPreviewState, EditorAddress};
use crate::styles::PillTone;

/// Shown in place of a blank internal part number.
const NO_PART_NUMBER: &str = "(no part number)";

/// The library's display name: its folder name without `.snxlib`.
fn library_name(state: &ComponentPreviewState) -> String {
    state
        .library_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The manufacturer and MPN of the primary part, blank parts left out.
fn manufacturer_part(state: &ComponentPreviewState) -> Option<String> {
    let part = &state.row.primary_mpn;
    let words: Vec<&str> = [part.manufacturer.trim(), part.mpn.trim()]
        .into_iter()
        .filter(|w| !w.is_empty())
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

pub(super) fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &I18n,
    address: EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let identity = column![identity_line(state, tokens, i18n)]
        .spacing(SP.xs)
        .width(Length::Fill);
    let identity = match summary_line(state, tokens) {
        Some(summary) => identity.push(summary),
        None => identity,
    };
    let mut line: iced::widget::Row<Element<'a, LibraryMessage>> = row![identity.boxed()]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);
    // Measurements typed on the Parameters tab that did not parse; Save is
    // blocked while any is pending.
    let unparsed = state.params_error.len();
    if unparsed > 0 {
        line = line.push(kit::toned_pill(
            t!(
                i18n,
                ids::EDITORS_COMPONENT_HEADER_UNPARSED,
                "count" => unparsed
            )
            .to_string(),
            PillTone::Err,
            tokens,
        ));
    }
    if state.dirty {
        line = line.push(
            text(t!(i18n, ids::EDITORS_COMPONENT_UNSAVED))
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
                .boxed(),
        );
    }
    let event = |msg: EditorMsg| {
        Some(LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg,
        })
    };
    let editable = !read_only;
    // Discard drops the working copy for the stored row (asks first when
    // dirty); Release has no backend yet, so it is a disabled stub.
    line = line
        .push(kit::secondary_button(
            "Discard",
            event(EditorMsg::ReloadRequested).filter(|_| editable && state.dirty),
            tokens,
        ))
        .push(kit::secondary_button("Release...", None, tokens))
        .push(kit::secondary_button(
            "Save & Close",
            event(EditorMsg::Commit).filter(|_| editable),
            tokens,
        ))
        .push(kit::primary_button(
            "Save",
            event(EditorMsg::SaveDraft).filter(|_| editable),
            tokens,
        ));
    // No close button: the document tab's own cross closes the editor.
    container(line.boxed())
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(crate::styles::modal_header_strip(tokens))
        .boxed()
}

/// Internal PN, lifecycle and class pills, version and library / table,
/// in the secondary mono face.
fn identity_line<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let pn = state.row.internal_pn.as_str();
    let pn = if pn.trim().is_empty() {
        NO_PART_NUMBER
    } else {
        pn
    };
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let muted = theme_ext::text_secondary(tokens);
    let mut line: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        text(UserText::new(pn))
            .size(TS.dense)
            .font(mono)
            .color(muted)
            .boxed(),
        kit::toned_pill(
            kit::lifecycle_label(state.row.state).to_string(),
            kit::lifecycle_tone(state.row.state),
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let class = state.row.class.as_str().trim();
    if !class.is_empty() {
        line = line.push(kit::toned_pill(
            class.to_string(),
            PillTone::Neutral,
            tokens,
        ));
    }
    let version = state.row.version.trim();
    if !version.is_empty() {
        line = line.push(
            text(t!(
                i18n,
                ids::EDITORS_COMPONENT_HEADER_VERSION,
                "version" => version
            ))
            .size(TS.dense)
            .font(mono)
            .color(muted)
            .boxed(),
        );
    }
    line.push(
        text(t!(
            i18n,
            ids::EDITORS_COMPONENT_HEADER_LOCATION,
            "library" => library_name(state),
            "table" => state.table.as_str()
        ))
        .size(TS.dense)
        .font(mono)
        .color(muted)
        .boxed(),
    )
    .boxed()
}

/// The description as the heading, with the manufacturer part beside it;
/// `None` when the row has neither.
fn summary_line<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a signex_types::theme::ThemeTokens,
) -> Option<Element<'a, LibraryMessage>> {
    let description = state.row.description.trim();
    let part = manufacturer_part(state);
    if description.is_empty() && part.is_none() {
        return None;
    }
    let title_font = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let mut line: iced::widget::Row<Element<'a, LibraryMessage>> = iced::widget::Row::new()
        .spacing(SP.l)
        .align_y(iced::Alignment::Center);
    if !description.is_empty() {
        line = line.push(
            text(UserText::new(description))
                .size(TS.heading)
                .font(title_font)
                .color(theme_ext::text_primary(tokens))
                .boxed(),
        );
    }
    if let Some(part) = part {
        line = line.push(
            text(UserText::new(part))
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
                .boxed(),
        );
    }
    line = line.push(Space::new().width(Length::Fill).boxed());
    Some(line.boxed())
}
