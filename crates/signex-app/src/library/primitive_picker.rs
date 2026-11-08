//! Primitive picker modal — Pick Symbol / Pick Footprint.
//!
//! Listing source: every mounted library's cached primitive summaries
//! (`OpenLibrary::cached_symbols` / `cached_footprints` / `cached_sims`),
//! so the view does no disk IO. Filtering is case-insensitive across the
//! primitive name and its UUID.
//!
//! Shape:
//!
//! ```text
//! ┌─[ Pick Symbol ]────────────────────────────────────┐
//! │ Filter: [_______________]                          │
//! │                                                    │
//! │ ▼ Loratis-SN-lib-2.snxlib                          │
//! │     ESP32-WROOM-32.snxsym       (uuid …abc12345)  │
//! │     LM7805.snxsym               (uuid …bd45ee01)  │
//! │ ▼ Loratis-SN-lib-3.snxlib                          │
//! │     74HC595.snxsym                                 │
//! │                                                    │
//! │ [ Browse filesystem… ]    [ Cancel ]   [ Pick ]    │
//! └────────────────────────────────────────────────────┘
//! ```

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Border, Element, Length, Theme, Widget};
use signex_library::{PrimitiveKind, PrimitiveRef, PrimitiveSummary};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{button, raw, text, text_input};

use super::browser::{OVERSCAN_ROWS, visible_range};
use super::loose_symbols::LooseSymbol;
use super::messages::{LibraryMessage, PrimitivePickerMsg};
use super::modal_chrome::{footer_strip, header, primary_btn, secondary_btn};
use super::state::{LibraryState, PrimitivePickerState};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const PICKER_W: f32 = 640.0;
const PICKER_H: f32 = 520.0;
/// Height every list entry (library header or primitive row) is pinned to,
/// so the window maths needs only the scroll offset.
const ROW_H: f32 = 24.0;

/// One entry of the flattened list: a library header or a primitive.
enum ListItem {
    Header {
        text: String,
    },
    Row(String, uuid::Uuid, PrimitiveRef),
    /// A symbol of a file in no library: choosing it asks for a copy.
    Loose(LooseSymbol),
}

/// Flatten the filtered per-library cache into header + row entries. Cheap
/// (no widgets), so it can run on every view.
fn list_items(
    state: &LibraryState,
    kind: PrimitiveKind,
    needle: &str,
    loose: &[LooseSymbol],
) -> Vec<ListItem> {
    let mut items = Vec::new();
    for lib in &state.open_libraries {
        // Read the per-library cache instead of calling `adapter.list_*`
        // every view tick; it is refreshed on open, post-save and after row
        // mutations, so the picker does no per-frame disk IO.
        let summaries: &[PrimitiveSummary] = match kind {
            PrimitiveKind::Symbol => &lib.cached_symbols,
            PrimitiveKind::Footprint => &lib.cached_footprints,
            PrimitiveKind::Sim => &lib.cached_sims,
            _ => &[],
        };
        let start = items.len();
        items.push(ListItem::Header {
            text: String::new(),
        });
        for s in summaries {
            let hit = needle.is_empty()
                || contains_ci(&s.name, needle)
                || s.uuid
                    .simple()
                    .encode_lower(&mut uuid::Uuid::encode_buffer())
                    .contains(needle);
            if hit {
                items.push(ListItem::Row(
                    s.name.clone(),
                    s.uuid,
                    PrimitiveRef::new(lib.library_id, s.uuid),
                ));
            }
        }
        let count = items.len() - start - 1;
        if count == 0 {
            items.truncate(start);
        } else {
            items[start] = ListItem::Header {
                text: format!("▼ {}.snxlib  ({})", lib.display_name, count),
            };
        }
    }
    push_loose(&mut items, state, loose, needle);
    items
}

/// The symbols of files in no library, one group per file (#1841).
fn push_loose(
    items: &mut Vec<ListItem>,
    state: &LibraryState,
    loose: &[LooseSymbol],
    needle: &str,
) {
    // A library opened since the list was read lists its own symbols.
    let loose: Vec<&LooseSymbol> = loose
        .iter()
        .filter(|s| {
            s.library
                .as_deref()
                .is_none_or(|l| state.library_at(l).is_none())
        })
        .collect();
    let mut files: Vec<&std::path::Path> = Vec::new();
    for symbol in &loose {
        if !files.contains(&symbol.file.as_path()) {
            files.push(symbol.file.as_path());
        }
    }
    for file in files {
        let start = items.len();
        items.push(ListItem::Header {
            text: String::new(),
        });
        items.extend(
            loose
                .iter()
                .filter(|s| s.file == file && (needle.is_empty() || contains_ci(&s.name, needle)))
                .map(|s| ListItem::Loose((*s).clone())),
        );
        let count = items.len() - start - 1;
        if count == 0 {
            items.truncate(start);
            continue;
        }
        let name = file.file_name().map_or_else(
            || file.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let owner = loose
            .iter()
            .find(|s| s.file == file)
            .and_then(|s| s.library.as_deref());
        items[start] = ListItem::Header {
            text: match owner {
                Some(lib) => format!("In library {} (not open): {name}  ({count})", lib_name(lib)),
                None => format!("Not in a library: {name}  ({count})"),
            },
        };
    }
}

fn lib_name(snxlib: &std::path::Path) -> String {
    snxlib.file_stem().map_or_else(
        || snxlib.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Entries to build for `count` list entries at `scroll_y`. The viewport is
/// taken as the whole picker height, which only ever over-builds.
fn list_window(scroll_y: f32, count: usize) -> std::ops::Range<usize> {
    visible_range(scroll_y, PICKER_H, ROW_H, count, OVERSCAN_ROWS)
}

fn row_button<'a>(
    name: &str,
    uuid: uuid::Uuid,
    on_press: LibraryMessage,
    kind: PrimitiveKind,
    text_c: iced::Color,
    muted: iced::Color,
    hover: iced::Color,
) -> Element<'a, LibraryMessage> {
    let short_uuid = signex_types::object_id::short_id(&uuid);
    let ext = match kind {
        PrimitiveKind::Symbol => "snxsym",
        PrimitiveKind::Footprint => "snxfpt",
        PrimitiveKind::Sim => "snxsim",
        _ => "snx",
    };
    button(
        row![
            Space::new().width(20),
            text(raw(format!("{name}.{ext}")))
                .size(TS.body)
                .color(text_c)
                .width(Length::FillPortion(3)),
            text(raw(format!("(uuid …{})", short_uuid)))
                .size(TS.caption)
                .color(muted)
                .width(Length::FillPortion(2)),
        ]
        .spacing(0)
        .padding([3.0, SP.m])
        .height(Length::Fill)
        .align_y(iced::Alignment::Center),
    )
    .padding(0)
    .width(Length::Fill)
    .height(ROW_H)
    .on_press(on_press)
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover)),
            _ => None,
        };
        iced::widget::button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..iced::widget::button::Style::default()
        }
    })
    .boxed()
}

/// Title of the picker for `kind`; the native window title of a detached
/// picker reads the same.
pub(crate) fn title(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "Pick Symbol",
        PrimitiveKind::Footprint => "Pick Footprint",
        PrimitiveKind::Sim => "Pick Sim Model",
        _ => "Pick Primitive",
    }
}

pub fn view<'a>(
    state: &'a LibraryState,
    picker: &'a PrimitivePickerState,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let hover = crate::styles::tokens::hover_overlay(tokens);

    let header = header(
        title(picker.kind),
        LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Cancel),
        windowed,
        tokens,
    );

    let search = text_input(raw("Filter…"), &picker.filter)
        .on_input(|s| LibraryMessage::PrimitivePicker(PrimitivePickerMsg::SetFilter(s)))
        .padding(6)
        .size(TS.label);

    let needle = picker.filter.trim().to_lowercase();

    let items = list_items(state, picker.kind, &needle, &picker.loose);
    let total_visible = items
        .iter()
        .filter(|i| matches!(i, ListItem::Row(..) | ListItem::Loose(..)))
        .count();
    let window = list_window(picker.scroll_y, items.len());

    // Only the rows in view (plus overscan) become widgets; two spacers of
    // the skipped height keep the scrollbar and total height unchanged.
    let mut list_col: iced::widget::Column<Element<'_, LibraryMessage>> = column![].spacing(0);
    if window.start > 0 {
        list_col = list_col.push(Space::new().height(window.start as f32 * ROW_H).boxed());
    }
    for item in &items[window.clone()] {
        list_col = list_col.push(match item {
            ListItem::Header { text: label } => {
                container(text(raw(label.clone())).size(TS.body).color(muted))
                    .padding([6.0, SP.m])
                    .height(ROW_H)
                    .boxed()
            }
            ListItem::Row(summary_name, uuid, primitive_ref) => row_button(
                summary_name,
                *uuid,
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(*primitive_ref)),
                picker.kind,
                text_c,
                muted,
                hover,
            ),
            ListItem::Loose(symbol) => row_button(
                &symbol.name,
                symbol.uuid,
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::PickLoose(symbol.clone())),
                picker.kind,
                text_c,
                muted,
                hover,
            ),
        });
    }
    if window.end < items.len() {
        list_col = list_col.push(
            Space::new()
                .height((items.len() - window.end) as f32 * ROW_H)
                .boxed(),
        );
    }

    if total_visible == 0 {
        list_col = list_col.push(
            container(
                text(raw(if state.open_libraries.is_empty() {
                    "No libraries open. Use Browse filesystem… to pick a primitive directly."
                } else {
                    "No primitives match. Only primitives saved inside an open `.snxlib` \
                     are listed: save the symbol into `<lib>.snxlib/symbols/`, or use \
                     Browse filesystem…"
                }))
                .size(TS.body)
                .color(muted),
            )
            .padding([14, 14])
            .boxed(),
        );
    }

    let body = container(
        scrollable(list_col)
            .on_scroll(|scroll| {
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Scrolled(
                    scroll.viewport.absolute_offset().y,
                ))
            })
            .height(Length::Fill)
            .width(Length::Fill),
    )
    .padding([6, 0])
    .height(Length::Fill);

    let confirm_row = copy_confirm_row(state, picker, tokens);
    let error_row: Element<'_, LibraryMessage> = if let Some(err) = picker.error.as_ref() {
        container(
            text(raw(err.clone()))
                .size(TS.body)
                .color(crate::styles::tokens::danger(tokens)),
        )
        .padding([SP.s, 14.0])
        .boxed()
    } else {
        Space::new().height(0).boxed()
    };

    let footer = footer_strip(
        vec![secondary_btn(
            "Browse filesystem…",
            LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Browse),
            tokens,
        )],
        vec![secondary_btn(
            "Cancel",
            LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Cancel),
            tokens,
        )],
        tokens,
    );

    let body_pad = container(
        column![
            container(search).padding([6.0, SP.l]),
            body,
            confirm_row,
            error_row
        ]
        .spacing(0),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    container(
        column![header, body_pad, footer]
            .width(super::card_len(windowed, PICKER_W))
            .height(super::card_len(windowed, PICKER_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// "Copy <symbol> into library <lib>?" with its two answers, while one is
/// pending (#1841).
fn copy_confirm_row<'a>(
    state: &'a LibraryState,
    picker: &'a PrimitivePickerState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let Some(symbol) = picker.copy_confirm.as_ref() else {
        return Space::new().height(0).boxed();
    };
    let library = picker
        .target
        .library_path()
        .and_then(|path| state.library_at(path))
        .map_or("this library", |lib| lib.display_name.as_str());
    let question = match symbol.library.as_deref() {
        Some(lib) => format!(
            "{} is in library {}, which is not open. Open it to pick the symbol from there.",
            symbol.name,
            lib_name(lib)
        ),
        None => format!(
            "Copy {} into library {library}? The original file stays as it is.",
            symbol.name
        ),
    };
    let answers: Element<'_, LibraryMessage> = if picker.opening_library.is_some() {
        row![
            text(raw("Opening library…")).size(TS.body),
            secondary_btn(
                "Cancel",
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::CopyDeclined),
                tokens,
            ),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
    } else if symbol.library.is_some() {
        row![
            primary_btn(
                "Open library",
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::OpenLibraryConfirmed),
                tokens,
            ),
            secondary_btn(
                "Cancel",
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::CopyDeclined),
                tokens,
            ),
        ]
        .spacing(SP.s)
        .boxed()
    } else if picker.copying {
        text(raw("Copying…")).size(TS.body).boxed()
    } else {
        row![
            primary_btn(
                "Copy",
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::CopyConfirmed),
                tokens,
            ),
            secondary_btn(
                "Don't copy",
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::CopyDeclined),
                tokens,
            ),
        ]
        .spacing(SP.s)
        .boxed()
    };
    container(
        row![
            text(raw(question))
                .size(TS.body)
                .color(theme_ext::text_primary(tokens)),
            Space::new().width(Length::Fill),
            answers,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.s, 14.0])
    .boxed()
}

/// Case-insensitive substring test against an already-lowercased
/// `needle`, without allocating for ASCII needles (this runs per
/// primitive per frame).
fn contains_ci(hay: &str, needle: &str) -> bool {
    if needle.is_ascii() {
        let n = needle.as_bytes();
        if n.is_empty() {
            return true;
        }
        return hay
            .as_bytes()
            .windows(n.len())
            .any(|w| w.eq_ignore_ascii_case(n));
    }
    hay.to_lowercase().contains(needle)
}

/// The header labels the list shows for `picker`, for tests.
#[cfg(test)]
pub(crate) fn header_labels(state: &LibraryState, picker: &PrimitivePickerState) -> Vec<String> {
    list_items(state, picker.kind, "", &picker.loose)
        .into_iter()
        .filter_map(|item| match item {
            ListItem::Header { text } => Some(text),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{OVERSCAN_ROWS, PICKER_H, ROW_H, contains_ci, list_window};

    #[test]
    fn contains_ci_matches_case_insensitively() {
        assert!(contains_ci("LM317_Adj", "lm317"));
        assert!(contains_ci("Résistor", "résis"));
        assert!(contains_ci("anything", ""));
        assert!(!contains_ci("LM317", "lm7"));
        assert!(!contains_ci("ab", "abc"));
    }

    fn max_built() -> usize {
        (PICKER_H / ROW_H).ceil() as usize + 1 + 2 * OVERSCAN_ROWS
    }

    #[test]
    fn huge_list_builds_only_visible_plus_overscan() {
        let w = list_window(0.0, 10_000);
        assert_eq!(w.start, 0);
        assert!(w.len() <= max_built(), "built {}", w.len());
        let mid = list_window(5_000.0 * ROW_H, 10_000);
        assert!(mid.len() <= max_built() && mid.start > 0 && mid.end < 10_000);
    }

    #[test]
    fn stale_offset_after_filter_shrink_is_clamped() {
        // Scrolled deep into a 10k list, then a filter leaves 100 entries.
        let w = list_window(9_000.0 * ROW_H, 100);
        assert!(!w.is_empty() && w.end == 100);
        assert!(list_window(1.0e9, 0).is_empty());
    }

    #[test]
    fn short_list_is_built_whole() {
        assert_eq!(list_window(0.0, 5), 0..5);
    }
}
