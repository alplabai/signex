//! Datasheet tab — one card per document on the row (#1231), the
//! primary first, each with a label, a mode pick_list and a value
//! control. Standalone Component-Preview tab per
//! `v0.9-refactor-2-plan.md` §11.5.
//!
//! Two modes per document:
//!  * **URL** — plain text input. Sets `DatasheetRef::Url`.
//!  * **Pinned PDF** — file picker. Hashes the bytes and sets
//!    `DatasheetRef::HashPinned { hash, filename }`.
//!
//! The mode is derived from the document's `DatasheetRef` variant.

use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::{DatasheetDoc, DatasheetRef};
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

/// Render the Datasheet tab: one card per document in
/// `state.row.documents()`, the primary first, then "Add document".
pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let docs = state.row.documents();
    let count = docs.len();

    let mut list = column![].spacing(8).width(Length::Fill);
    for (index, doc) in docs.into_iter().enumerate() {
        list = list.push(view_document(index, count, doc, tokens, address.clone()));
    }

    let add =
        small_button("Add document", tokens).on_press(event(&address, EditorMsg::DatasheetAdd));

    column![
        list,
        Space::new().height(6),
        row![
            text(
                "The first document is the primary: it is the one the schematic opens. \
                 Pinned PDFs are content-addressed, so the hash travels with the part."
            )
            .size(10)
            .color(muted)
            .width(Length::Fill),
            Space::new().width(8),
            add,
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

fn event(address: &EditorAddress, msg: EditorMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    }
}

fn small_button<'a>(
    label: &'a str,
    tokens: &'a ThemeTokens,
) -> iced::widget::Button<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    button(text(label).size(10).color(text_c))
        .padding([2, 8])
        .style(crate::styles::outline_button(tokens))
}

/// One document card: its role and label, the mode picker, the value
/// control, and the reorder / remove buttons.
fn view_document<'a>(
    index: usize,
    count: usize,
    doc: DatasheetDoc,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let mode = DatasheetMode::from_ref(Some(&doc.source));

    let mode_address = address.clone();
    let mode_picker = pick_list(DatasheetMode::ALL, Some(mode), move |m| {
        event(&mode_address, EditorMsg::DatasheetSetMode(index, m))
    })
    .text_size(11)
    .padding([4, 8]);

    let label_address = address.clone();
    let label_input = text_input("Datasheet", &doc.label)
        .on_input(move |s| event(&label_address, EditorMsg::DatasheetSetLabel(index, s)))
        .padding([4, 8])
        .size(11)
        .width(Length::Fixed(160.0));

    let mut actions = row![].spacing(4).align_y(iced::Alignment::Center);
    if index > 0 {
        actions = actions.push(small_button("Make primary", tokens).on_press(event(
            &address,
            EditorMsg::DatasheetMove { from: index, to: 0 },
        )));
        actions = actions.push(small_button("Up", tokens).on_press(event(
            &address,
            EditorMsg::DatasheetMove {
                from: index,
                to: index - 1,
            },
        )));
    }
    if index + 1 < count {
        actions = actions.push(small_button("Down", tokens).on_press(event(
            &address,
            EditorMsg::DatasheetMove {
                from: index,
                to: index + 1,
            },
        )));
    }
    actions = actions.push(
        small_button("Remove", tokens).on_press(event(&address, EditorMsg::DatasheetRemove(index))),
    );

    let header = row![
        text(if index == 0 { "Primary" } else { "Document" })
            .size(10)
            .color(muted),
        Space::new().width(8),
        label_input,
        Space::new().width(Length::Fill),
        mode_picker,
    ]
    .align_y(iced::Alignment::Center);

    let value_row: Element<'a, LibraryMessage> = match mode {
        DatasheetMode::Url => view_url_input(index, &doc.source, address.clone()),
        DatasheetMode::PinnedPdf => view_pinned_input(index, &doc.source, tokens, address.clone()),
    };

    container(
        column![
            header,
            Space::new().height(4),
            value_row,
            Space::new().height(4),
            row![Space::new().width(Length::Fill), actions],
        ]
        .spacing(0),
    )
    .padding(8)
    .style(crate::styles::modal_card(tokens))
    .width(Length::Fill)
    .into()
}

// ─────────────────────────────────────────────────────────────────────
// Per-mode rows
// ─────────────────────────────────────────────────────────────────────

fn view_url_input<'a>(
    index: usize,
    source: &DatasheetRef,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let url_value = match source {
        DatasheetRef::Url { url } => url.clone(),
        _ => String::new(),
    };
    text_input("https://example.com/datasheet.pdf", &url_value)
        .on_input(move |s| event(&address, EditorMsg::DatasheetSetUrl(index, s)))
        .padding([4, 8])
        .size(12)
        .into()
}

fn view_pinned_input<'a>(
    index: usize,
    source: &DatasheetRef,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let upload_btn =
        button(container(text("Upload PDF…").size(11).color(iced::Color::WHITE)).padding([4, 14]))
            .on_press(event(&address, EditorMsg::DatasheetUploadDialog(index)))
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

    let summary: Element<'a, LibraryMessage> = match source {
        DatasheetRef::HashPinned { hash, filename } if !hash.is_empty() => {
            let short_hash = if hash.len() > 12 {
                format!("{}…", crate::text_trunc::prefix_within_bytes(hash, 12))
            } else {
                hash.clone()
            };
            row![
                text(filename.clone()).size(11).color(text_c),
                Space::new().width(Length::Fill),
                text(short_hash).size(10).color(muted),
            ]
            .align_y(iced::Alignment::Center)
            .into()
        }
        _ => text("No PDF attached yet — click \"Upload PDF…\".")
            .size(11)
            .color(muted)
            .into(),
    };

    row![
        container(summary).width(Length::Fill),
        Space::new().width(8),
        upload_btn
    ]
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
