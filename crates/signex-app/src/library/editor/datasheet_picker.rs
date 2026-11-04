//! Datasheet tab — one card per document on the row (#1231), the
//! primary first, each with a label, a mode pick_list and a value
//! control. Standalone Component-Preview tab.
//!
//! Two modes per document:
//!  * **URL** — plain text input. Sets `DatasheetRef::Url`.
//!  * **Pinned PDF** — file picker. Hashes the bytes and sets
//!    `DatasheetRef::HashPinned { hash, filename }`.
//!
//! The mode is derived from the document's `DatasheetRef` variant.

use iced::widget::{
    Column, Space, Widget, button, column, container, pick_list, row, text, text_input,
};
use iced::{Border, Element, Length, Theme};
use signex_library::{DatasheetDoc, DatasheetRef};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{DatasheetMsg, EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

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

/// Render the Datasheet tab: a short explanation, one card per document
/// in `state.row.documents()` (the primary first), then "Add document".
pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let docs = state.row.documents();
    let count = docs.len();

    let intro = text(
        "The first document is the primary: it is the one the schematic opens. \
         A pinned PDF is stored beside the library file, named by its hash, when \
         you save the row, and a git-backed library commits it with the row. A \
         library without a local folder (a server library) cannot store PDFs: \
         use a URL there.",
    )
    .size(TS.caption)
    .color(muted)
    .width(Length::Fill);

    let mut list: Column<Element<'a, LibraryMessage>> =
        Column::new().spacing(SP.m).width(Length::Fill);
    if count == 0 {
        list = list.push(
            container(
                text("No datasheet on this component yet.")
                    .size(TS.body)
                    .color(muted),
            )
            .padding(SP.l)
            .width(Length::Fill)
            .style(crate::styles::modal_card(tokens))
            .boxed(),
        );
    }
    for (index, doc) in docs.into_iter().enumerate() {
        let error = state.datasheet_errors.get(&index).map(String::as_str);
        list = list.push(view_document(
            index,
            count,
            doc,
            error,
            tokens,
            address.clone(),
        ));
    }

    let add = small_button("+ Add document", tokens).on_press(event(&address, DatasheetMsg::Add));

    column![intro, list, add]
        .spacing(SP.m)
        .width(Length::Fill)
        .boxed()
}

fn event(address: &EditorAddress, msg: DatasheetMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg: EditorMsg::Datasheet(msg),
    }
}

fn small_button<'a>(
    label: &'a str,
    tokens: &'a ThemeTokens,
) -> iced::widget::Button<'a, LibraryMessage> {
    button(text(label).size(TS.caption).boxed())
        .padding([SP.xs, SP.m])
        .style(crate::styles::outline_button(tokens))
}

/// The left column of a card's form rows: a muted, fixed-share label so
/// the fields of every row line up.
fn field_row<'a>(
    label: &'a str,
    field: Element<'a, LibraryMessage>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    row![
        container(
            text(label)
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
        )
        .width(Length::FillPortion(1)),
        container(field).width(Length::FillPortion(6)),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The role badge in a card header: the primary gets the accent fill.
fn role_badge<'a>(is_primary: bool, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let (label, fill, fg) = if is_primary {
        (
            "Primary",
            crate::styles::tokens::accent_fill(tokens),
            crate::styles::tokens::on_accent(tokens),
        )
    } else {
        (
            "Document",
            crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover),
            theme_ext::text_secondary(tokens),
        )
    };
    container(text(label).size(TS.caption).color(fg))
        .padding([SP.xs / 2.0, SP.s])
        .style(move |_: &Theme| container::Style {
            background: Some(fill.into()),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// One document card: a header with its role and reorder / remove
/// actions, then the label, source type and value as aligned rows.
fn view_document<'a>(
    index: usize,
    count: usize,
    doc: DatasheetDoc,
    error: Option<&'a str>,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let mode = DatasheetMode::from_ref(Some(&doc.source));

    let mode_address = address.clone();
    let mode_picker = pick_list(Some(mode), DatasheetMode::ALL, ToString::to_string)
        .on_select(move |m| event(&mode_address, DatasheetMsg::SetMode(index, m)))
        .text_size(TS.body)
        .padding([SP.s, SP.m]);

    let label_address = address.clone();
    let label_input = text_input("Datasheet", doc.label.clone())
        .on_input(move |s| event(&label_address, DatasheetMsg::SetLabel(index, s)))
        .padding([SP.s, SP.m])
        .size(TS.body)
        .width(Length::Fill);

    let mut actions = row![].spacing(SP.s).align_y(iced::Alignment::Center);
    if index > 0 {
        actions = actions.push(
            small_button("Make primary", tokens)
                .on_press(event(&address, DatasheetMsg::Move { from: index, to: 0 })),
        );
        actions = actions.push(small_button("Up", tokens).on_press(event(
            &address,
            DatasheetMsg::Move {
                from: index,
                to: index - 1,
            },
        )));
    }
    if index + 1 < count {
        actions = actions.push(small_button("Down", tokens).on_press(event(
            &address,
            DatasheetMsg::Move {
                from: index,
                to: index + 1,
            },
        )));
    }
    actions = actions.push(
        small_button("Remove", tokens).on_press(event(&address, DatasheetMsg::Remove(index))),
    );

    let header = row![
        role_badge(index == 0, tokens),
        Space::new().width(Length::Fill),
        actions,
    ]
    .align_y(iced::Alignment::Center);

    let (value_label, value_row): (&str, Element<'a, LibraryMessage>) = match mode {
        DatasheetMode::Url => ("URL", view_url_input(index, &doc.source, address.clone())),
        DatasheetMode::PinnedPdf => (
            "File",
            view_pinned_input(index, &doc.source, tokens, address.clone()),
        ),
    };

    let mut body = column![
        header,
        field_row("Label", label_input.boxed(), tokens),
        field_row("Source", mode_picker.boxed(), tokens),
        field_row(value_label, value_row, tokens),
    ]
    .spacing(SP.m);
    if let Some(reason) = error {
        body = body.push(
            text(reason)
                .size(TS.body)
                .color(theme_ext::error_color(tokens))
                .boxed(),
        );
    }

    container(body)
        .padding(SP.l)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .boxed()
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
    text_input("https://example.com/datasheet.pdf", url_value)
        .on_input(move |s| event(&address, DatasheetMsg::SetUrl(index, s)))
        .padding([SP.s, SP.m])
        .size(TS.body)
        .boxed()
}

fn view_pinned_input<'a>(
    index: usize,
    source: &DatasheetRef,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let upload_btn = button(container(text("Upload PDF…").size(TS.body)).padding([SP.s, SP.l]))
        .on_press(event(&address, DatasheetMsg::UploadDialog(index)))
        .style(crate::styles::accent_button(tokens));

    let summary: Element<'a, LibraryMessage> = match source {
        DatasheetRef::HashPinned { hash, filename } if !hash.is_empty() => {
            let short_hash = if hash.len() > 12 {
                format!("{}…", crate::text_trunc::prefix_within_bytes(hash, 12))
            } else {
                hash.clone()
            };
            row![
                text(filename.clone()).size(TS.body).color(text_c),
                Space::new().width(Length::Fill),
                text(short_hash).size(TS.caption).color(muted),
            ]
            .align_y(iced::Alignment::Center)
            .boxed()
        }
        _ => text("No PDF attached yet.")
            .size(TS.body)
            .color(muted)
            .boxed(),
    };

    row![container(summary).width(Length::Fill), upload_btn]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .boxed()
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
