//! Datasheet tab — one card per document on the row (#1231), the
//! primary first, each with a label, a mode pick_list and a value
//! control. Standalone Component-Preview tab, built from the dialog kit.
//!
//! Two modes per document:
//!  * **URL** — plain text input. Sets `DatasheetRef::Url`.
//!  * **Pinned PDF** — file picker. Hashes the bytes and sets
//!    `DatasheetRef::HashPinned { hash, filename }`.
//!
//! The mode is derived from the document's `DatasheetRef` variant.

use iced::widget::{Space, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::{DatasheetDoc, DatasheetRef};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{DatasheetMsg, EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress};
use super::kit::{self, SP, TS};

/// One-line hint under the section title.
const INTRO: &str = "The first document is the primary; the schematic opens it.";
/// Shown when the row has no document.
const EMPTY_MESSAGE: &str = "No datasheet on this component yet.";

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

/// Render the Datasheet tab: a titled section with "Add document", then one
/// card per document in `state.row.documents()` (the primary first). The
/// whole tab scrolls so a long list never pushes the action off-screen.
pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let docs = state.row.documents();
    let count = docs.len();

    let add = kit::secondary_button(
        "Add document",
        Some(event(&address, DatasheetMsg::Add)),
        tokens,
    );
    let header = kit::section_header("Datasheets", Some(add), tokens);
    let intro = text(INTRO)
        .size(TS.caption)
        .color(theme_ext::text_secondary(tokens));

    let mut list = column![].spacing(SP.m).width(Length::Fill);
    if count == 0 {
        list = list.push(kit::card(
            kit::empty_state(EMPTY_MESSAGE, None, tokens),
            tokens,
        ));
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

    scrollable(
        column![header, intro, list]
            .spacing(SP.m)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn event(address: &EditorAddress, msg: DatasheetMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg: EditorMsg::Datasheet(msg),
    }
}

/// The left column of a card's form rows: a muted, fixed-share label so
/// the fields of every row line up.
fn field_row<'a>(
    label: &'a str,
    field: impl Into<Element<'a, LibraryMessage>>,
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
    .into()
}

/// Role label of the document at `index`: the first one is the primary.
fn role_label(index: usize) -> &'static str {
    if index == 0 { "Primary" } else { "Document" }
}

/// Source label shown as a badge next to the role.
fn mode_label(mode: DatasheetMode) -> &'static str {
    match mode {
        DatasheetMode::Url => "Link",
        DatasheetMode::PinnedPdf => "PDF",
    }
}

/// A small pill: the accent fill when `accent`, a hover overlay otherwise.
fn pill<'a>(
    label: &'static str,
    accent: bool,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let (fill, fg) = if accent {
        (
            crate::styles::tokens::accent_fill(tokens),
            crate::styles::tokens::on_accent(tokens),
        )
    } else {
        (
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
        .into()
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
    let mode_picker = pick_list(DatasheetMode::ALL, Some(mode), move |m| {
        event(&mode_address, DatasheetMsg::SetMode(index, m))
    })
    .text_size(TS.body)
    .padding([SP.s, SP.m]);

    let label_address = address.clone();
    let label_input = text_input("Datasheet", &doc.label)
        .on_input(move |s| event(&label_address, DatasheetMsg::SetLabel(index, s)))
        .padding([SP.s, SP.m])
        .size(TS.body)
        .width(Length::Fill);

    let mut actions = row![].spacing(SP.s).align_y(iced::Alignment::Center);
    if index > 0 {
        let to_top = DatasheetMsg::Move { from: index, to: 0 };
        let up = DatasheetMsg::Move {
            from: index,
            to: index - 1,
        };
        actions = actions
            .push(kit::secondary_button(
                "Make primary",
                Some(event(&address, to_top)),
                tokens,
            ))
            .push(kit::secondary_button(
                "Up",
                Some(event(&address, up)),
                tokens,
            ));
    }
    if index + 1 < count {
        let down = DatasheetMsg::Move {
            from: index,
            to: index + 1,
        };
        actions = actions.push(kit::secondary_button(
            "Down",
            Some(event(&address, down)),
            tokens,
        ));
    }
    actions = actions.push(kit::danger_button(
        "Remove",
        Some(event(&address, DatasheetMsg::Remove(index))),
        tokens,
    ));

    let header = row![
        pill(role_label(index), index == 0, tokens),
        pill(mode_label(mode), false, tokens),
        Space::new().width(Length::Fill),
        actions,
    ]
    .spacing(SP.s)
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
        field_row("Label", label_input, tokens),
        field_row("Source", mode_picker, tokens),
        field_row(value_label, value_row, tokens),
    ]
    .spacing(SP.m);
    if let Some(reason) = error {
        body = body.push(
            text(reason)
                .size(TS.body)
                .color(theme_ext::error_color(tokens)),
        );
    }

    kit::card(body, tokens)
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
        .on_input(move |s| event(&address, DatasheetMsg::SetUrl(index, s)))
        .padding([SP.s, SP.m])
        .size(TS.body)
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

    let upload_btn = kit::primary_button(
        "Upload PDF…",
        Some(event(&address, DatasheetMsg::UploadDialog(index))),
        tokens,
    );

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
            .into()
        }
        _ => text("No PDF attached yet.")
            .size(TS.body)
            .color(muted)
            .into(),
    };

    row![container(summary).width(Length::Fill), upload_btn]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_document_is_primary_the_rest_are_plain() {
        assert_eq!(role_label(0), "Primary");
        assert_eq!(role_label(1), "Document");
        assert_eq!(role_label(7), "Document");
    }

    #[test]
    fn source_badge_names_link_and_pdf() {
        assert_eq!(mode_label(DatasheetMode::Url), "Link");
        assert_eq!(mode_label(DatasheetMode::PinnedPdf), "PDF");
    }

    #[test]
    fn intro_is_a_short_single_line_hint() {
        assert!(INTRO.len() < 80 && !INTRO.contains('\n'));
        assert!(!EMPTY_MESSAGE.is_empty());
    }

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
