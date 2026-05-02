//! Datasheet tab — small pick_list + value control bound to
//! `state.row.datasheet`. Standalone Component-Preview tab per
//! `v0.9-refactor-2-plan.md` §11.5.
//!
//! Two modes:
//!  * **URL** — plain text input. Sets `DatasheetRef::Url`.
//!  * **Pinned PDF** — file picker. Hashes the bytes and sets
//!    `DatasheetRef::HashPinned { hash, filename }`.
//!
//! The mode is derived from the row's `DatasheetRef` variant.

use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::DatasheetRef;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress};

/// Mode picker entries — drives both the visible pick_list and the
/// inline message routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasheetMode {
    Url,
    PinnedPdf,
}

impl DatasheetMode {
    pub const ALL: [DatasheetMode; 2] = [DatasheetMode::Url, DatasheetMode::PinnedPdf];

    /// Derive the display mode from the current `DatasheetRef`. None
    /// of the options is "no datasheet" — that's the URL mode with an
    /// empty buffer.
    pub fn from_ref(r: Option<&DatasheetRef>) -> Self {
        match r {
            Some(DatasheetRef::HashPinned { .. }) => DatasheetMode::PinnedPdf,
            _ => DatasheetMode::Url,
        }
    }
}

impl std::fmt::Display for DatasheetMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DatasheetMode::Url => "URL",
            DatasheetMode::PinnedPdf => "Pinned PDF",
        })
    }
}

/// Render the Datasheet tab. Reads / writes `state.row.datasheet`.
pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    let datasheet = Some(&state.row.datasheet);
    let mode = DatasheetMode::from_ref(datasheet);
    let lib_path_mode = address.library_path.clone();
    let table_mode = address.table.clone();
    let row_id = address.row_id;
    let mode_picker = pick_list(DatasheetMode::ALL, Some(mode), move |m| {
        LibraryMessage::EditorEvent {
            library_path: lib_path_mode.clone(),
            table: table_mode.clone(),
            row_id,
            msg: EditorMsg::DatasheetSetMode(m),
        }
    })
    .text_size(11)
    .padding([4, 8]);

    let mode_row = row![
        text("Datasheet").size(10).color(muted),
        Space::new().width(Length::Fill),
        mode_picker,
    ]
    .align_y(iced::Alignment::Center);

    let value_row: Element<'a, LibraryMessage> = match mode {
        DatasheetMode::Url => view_url_input(datasheet, address.clone()),
        DatasheetMode::PinnedPdf => view_pinned_input(datasheet, tokens, address.clone()),
    };

    column![
        mode_row,
        Space::new().height(4),
        value_row,
        Space::new().height(2),
        text(match mode {
            DatasheetMode::Url => "Tip: paste a public datasheet URL.",
            DatasheetMode::PinnedPdf => {
                "Pinned PDFs are content-addressed — the hash travels with the part."
            }
        })
        .size(10)
        .color(muted),
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

// ─────────────────────────────────────────────────────────────────────
// Per-mode rows
// ─────────────────────────────────────────────────────────────────────

fn view_url_input<'a>(
    datasheet: Option<&'a DatasheetRef>,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let url_value = match datasheet {
        Some(DatasheetRef::Url { url }) => url.clone(),
        _ => String::new(),
    };
    let lib_path = address.library_path;
    let table = address.table;
    let row_id = address.row_id;
    text_input("https://example.com/datasheet.pdf", &url_value)
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path.clone(),
            table: table.clone(),
            row_id,
            msg: EditorMsg::DatasheetSetUrl(s),
        })
        .padding([4, 8])
        .size(12)
        .into()
}

fn view_pinned_input<'a>(
    datasheet: Option<&'a DatasheetRef>,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let upload_btn =
        button(container(text("Upload PDF…").size(11).color(iced::Color::WHITE)).padding([4, 14]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: address.library_path.clone(),
                table: address.table.clone(),
                row_id: address.row_id,
                msg: EditorMsg::DatasheetUploadDialog,
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.00, 0.47, 0.84,
                ))),
                text_color: iced::Color::WHITE,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            });

    let summary: Element<'a, LibraryMessage> = match datasheet {
        Some(DatasheetRef::HashPinned { hash, filename }) => {
            let short_hash = if hash.len() > 12 {
                format!("{}…", &hash[..12])
            } else {
                hash.clone()
            };
            container(
                column![
                    row![
                        text(filename.clone()).size(11).color(text_c),
                        Space::new().width(Length::Fill),
                        text(short_hash).size(10).color(muted),
                    ]
                    .align_y(iced::Alignment::Center),
                ]
                .spacing(0),
            )
            .padding(8)
            .style(crate::styles::modal_card(tokens))
            .width(Length::Fill)
            .into()
        }
        _ => container(
            text("No PDF attached yet — click \"Upload PDF…\".")
                .size(11)
                .color(muted),
        )
        .padding(8)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .into(),
    };

    row![summary, Space::new().width(8), upload_btn]
        .align_y(iced::Alignment::Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_from_ref_url_variant() {
        let r = DatasheetRef::url("https://x.test/d.pdf");
        assert_eq!(DatasheetMode::from_ref(Some(&r)), DatasheetMode::Url);
    }

    #[test]
    fn mode_from_ref_hash_pinned_variant() {
        let r = DatasheetRef::HashPinned {
            hash: "abc".into(),
            filename: "ds.pdf".into(),
        };
        assert_eq!(DatasheetMode::from_ref(Some(&r)), DatasheetMode::PinnedPdf);
    }

    #[test]
    fn mode_from_ref_none_falls_back_to_url() {
        assert_eq!(DatasheetMode::from_ref(None), DatasheetMode::Url);
    }

    #[test]
    fn mode_display_strings_are_stable() {
        assert_eq!(DatasheetMode::Url.to_string(), "URL");
        assert_eq!(DatasheetMode::PinnedPdf.to_string(), "Pinned PDF");
    }

    #[test]
    fn datasheet_ref_round_trips_via_json_url() {
        let r = DatasheetRef::url("https://example.com/d.pdf");
        let json = serde_json::to_string(&r).unwrap();
        let back: DatasheetRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn datasheet_ref_round_trips_via_json_hash_pinned() {
        let r = DatasheetRef::HashPinned {
            hash: "0".repeat(64),
            filename: "TLP281.pdf".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: DatasheetRef = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
