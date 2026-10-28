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
//! │     ESP32-WROOM-32.snxsym       (uuid abc12345…)  │
//! │     LM7805.snxsym               (uuid bd45ee01…)  │
//! │ ▼ Loratis-SN-lib-3.snxlib                          │
//! │     74HC595.snxsym                                 │
//! │                                                    │
//! │ [ Browse filesystem… ]    [ Cancel ]   [ Pick ]    │
//! └────────────────────────────────────────────────────┘
//! ```

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::{PrimitiveKind, PrimitiveRef, PrimitiveSummary};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::browser::{OVERSCAN_ROWS, visible_range};
use super::messages::{LibraryMessage, PrimitivePickerMsg};
use super::modal_chrome::close_x;
use super::state::{LibraryState, PrimitivePickerState};

const PICKER_W: f32 = 640.0;
const PICKER_H: f32 = 520.0;
/// Height every list entry (library header or primitive row) is pinned to,
/// so the window maths needs only the scroll offset.
const ROW_H: f32 = 24.0;

/// One entry of the flattened list: a library header or a primitive.
enum ListItem {
    Header { text: String },
    Row(String, uuid::Uuid, PrimitiveRef),
}

/// Flatten the filtered per-library cache into header + row entries. Cheap
/// (no widgets), so it can run on every view.
fn list_items(state: &LibraryState, kind: PrimitiveKind, needle: &str) -> Vec<ListItem> {
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
    items
}

/// Entries to build for `count` list entries at `scroll_y`. The viewport is
/// taken as the whole picker height, which only ever over-builds.
fn list_window(scroll_y: f32, count: usize) -> std::ops::Range<usize> {
    visible_range(scroll_y, PICKER_H, ROW_H, count, OVERSCAN_ROWS)
}

fn row_button<'a>(
    name: &str,
    uuid: uuid::Uuid,
    primitive_ref: PrimitiveRef,
    kind: PrimitiveKind,
    text_c: iced::Color,
    muted: iced::Color,
) -> Element<'a, LibraryMessage> {
    let short_uuid = {
        let s = uuid.simple().to_string();
        if s.len() >= 8 { s[..8].to_string() } else { s }
    };
    let ext = match kind {
        PrimitiveKind::Symbol => "snxsym",
        PrimitiveKind::Footprint => "snxfpt",
        PrimitiveKind::Sim => "snxsim",
        _ => "snx",
    };
    button(
        row![
            Space::new().width(20),
            text(format!("{name}.{ext}"))
                .size(11)
                .color(text_c)
                .width(Length::FillPortion(3)),
            text(format!("(uuid {}…)", short_uuid))
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
        ]
        .spacing(0)
        .padding([3, 8])
        .height(Length::Fill)
        .align_y(iced::Alignment::Center),
    )
    .padding(0)
    .width(Length::Fill)
    .height(ROW_H)
    .on_press(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Pick(
        primitive_ref,
    )))
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06),
            )),
            _ => None,
        };
        iced::widget::button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

pub fn view<'a>(
    state: &'a LibraryState,
    picker: &'a PrimitivePickerState,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let title = match picker.kind {
        PrimitiveKind::Symbol => "Pick Symbol",
        PrimitiveKind::Footprint => "Pick Footprint",
        PrimitiveKind::Sim => "Pick Sim Model",
        _ => "Pick Primitive",
    };

    let header = container(
        row![
            text(title).size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Cancel),
                tokens
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    let search = text_input("Filter…", &picker.filter)
        .on_input(|s| LibraryMessage::PrimitivePicker(PrimitivePickerMsg::SetFilter(s)))
        .padding(6)
        .size(12);

    let needle = picker.filter.trim().to_lowercase();

    let items = list_items(state, picker.kind, &needle);
    let total_visible = items
        .iter()
        .filter(|i| matches!(i, ListItem::Row(..)))
        .count();
    let window = list_window(picker.scroll_y, items.len());

    // Only the rows in view (plus overscan) become widgets; two spacers of
    // the skipped height keep the scrollbar and total height unchanged.
    let mut list_col = column![].spacing(0);
    if window.start > 0 {
        list_col = list_col.push(Space::new().height(window.start as f32 * ROW_H));
    }
    for item in &items[window.clone()] {
        list_col = list_col.push(match item {
            ListItem::Header { text: label } => {
                container(text(label.clone()).size(11).color(muted))
                    .padding([6, 8])
                    .height(ROW_H)
                    .into()
            }
            ListItem::Row(summary_name, uuid, primitive_ref) => row_button(
                summary_name,
                *uuid,
                *primitive_ref,
                picker.kind,
                text_c,
                muted,
            ),
        });
    }
    if window.end < items.len() {
        list_col = list_col.push(Space::new().height((items.len() - window.end) as f32 * ROW_H));
    }

    if total_visible == 0 {
        list_col = list_col.push(
            container(
                text(if state.open_libraries.is_empty() {
                    "No libraries open. Use Browse filesystem… to pick a primitive directly."
                } else {
                    "No primitives match. Open more libraries or use Browse filesystem…"
                })
                .size(11)
                .color(muted),
            )
            .padding([14, 14]),
        );
    }

    let body = container(
        scrollable(list_col)
            .on_scroll(|viewport| {
                LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Scrolled(
                    viewport.absolute_offset().y,
                ))
            })
            .height(Length::Fill)
            .width(Length::Fill),
    )
    .padding([6, 0])
    .height(Length::Fill);

    let error_row: Element<'_, LibraryMessage> = if let Some(err) = picker.error.as_ref() {
        container(
            text(err.clone())
                .size(11)
                .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
        )
        .padding([4, 14])
        .into()
    } else {
        Space::new().height(0).into()
    };

    let browse_btn =
        button(container(text("Browse filesystem…").size(11).color(text_c)).padding([4, 14]))
            .on_press(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Browse))
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            });

    let footer = container(
        row![
            browse_btn,
            Space::new().width(Length::Fill),
            button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
                .on_press(LibraryMessage::PrimitivePicker(PrimitivePickerMsg::Cancel))
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04
                    ))),
                    text_color: text_c,
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..iced::widget::button::Style::default()
                }),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    let body_pad =
        container(column![container(search).padding([6, 12]), body, error_row].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill);

    container(
        column![header, body_pad, footer]
            .width(super::card_len(windowed, PICKER_W))
            .height(super::card_len(windowed, PICKER_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
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
