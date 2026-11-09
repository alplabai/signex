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

use iced::widget::{Column, Row, Space, Widget, column, container, pick_list, row, scrollable};
use iced::{Color, Element, Length};
use signex_library::{DatasheetDoc, DatasheetRef};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::theme_ext;
use signex_widgets::{raw, text, text_input};

use super::super::messages::{DatasheetMsg, EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress};
use super::inert::unless_read_only_veiled as veiled;
use super::kit::{self, SP, TS};
use crate::panels::{PROPERTY_CONTROL_PORTION, PROPERTY_LABEL_PORTION};

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
/// On a read-only library the editing controls take `veil` and ignore
/// input; each row's Open button stays live.
pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
    read_only: bool,
    veil: Color,
) -> Element<'a, LibraryMessage> {
    let docs = state.row.documents();
    let count = docs.len();

    let add = veiled(
        read_only,
        veil,
        kit::secondary_button(
            "Add document",
            Some(event(&address, DatasheetMsg::Add)),
            tokens,
        ),
    );
    let header = kit::section_header("Datasheets", Some(add), tokens);
    let intro = text(raw(INTRO))
        .size(TS.caption)
        .color(theme_ext::text_secondary(tokens))
        .boxed();

    let mut list: Column<Element<'a, LibraryMessage>> =
        Column::new().spacing(SP.m).width(Length::Fill);
    if count == 0 {
        list = list.push(kit::card(
            kit::empty_state(EMPTY_MESSAGE, None, tokens),
            tokens,
        ));
    }
    for (index, doc) in docs.into_iter().enumerate() {
        let error = state.datasheet_errors.get(&index).map(String::as_str);
        let stored = !is_staged_only(state, &doc.source);
        list = list.push(view_document(
            index,
            count,
            doc,
            error,
            tokens,
            address.clone(),
            Gate {
                read_only,
                veil,
                stored,
            },
        ));
    }

    scrollable(
        column![header, intro, list.boxed()]
            .spacing(SP.m)
            .width(Length::Fill)
            .boxed(),
    )
    .width(Length::Fill)
    .height(Length::Fill)
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

/// A card's form row: a muted label and its control, split by the same
/// portions as the Properties panel's `form_input_row` so the columns match.
fn field_row<'a>(
    label: &'a str,
    field: Element<'a, LibraryMessage>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    row![
        container(
            text(raw(label))
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
        )
        .width(Length::FillPortion(PROPERTY_LABEL_PORTION))
        .clip(true)
        .boxed(),
        container(field)
            .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
            .boxed(),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// Whether `source` pins a PDF that is only staged on this editor: picked,
/// but not written to the library until the row is saved.
pub(crate) fn is_staged_only(state: &ComponentPreviewState, source: &DatasheetRef) -> bool {
    matches!(source, DatasheetRef::HashPinned { hash, .. } if state.staged_datasheets.contains_key(hash))
}

/// What the read-only library and the save state allow on a document card.
struct Gate {
    read_only: bool,
    veil: Color,
    /// False while the pinned PDF is staged and not yet saved.
    stored: bool,
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

/// A role / source badge: the accent fill when `accent`, a hover overlay
/// otherwise.
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
    kit::pill(label.to_string(), fill, fg)
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
    gate: Gate,
) -> Element<'a, LibraryMessage> {
    let mode = DatasheetMode::from_ref(Some(&doc.source));
    let Gate {
        read_only,
        veil,
        stored,
    } = gate;
    let can_open = !doc.source.is_empty() && stored;

    let mode_address = address.clone();
    let mode_picker = pick_list(Some(mode), DatasheetMode::ALL, ToString::to_string)
        .graphite()
        .on_select(move |m| event(&mode_address, DatasheetMsg::SetMode(index, m)))
        .padding([SP.s, SP.m])
        .boxed();

    let label_address = address.clone();
    let label_input = text_input(raw("Datasheet"), doc.label.clone())
        .on_input(move |s| event(&label_address, DatasheetMsg::SetLabel(index, s)))
        .padding([SP.s, SP.m])
        .size(TS.body)
        .width(Length::Fill)
        .boxed();

    let mut actions: Row<Element<'a, LibraryMessage>> =
        row![].spacing(SP.s).align_y(iced::Alignment::Center);
    if index > 0 {
        let to_top = DatasheetMsg::Move { from: index, to: 0 };
        actions = actions.push(kit::secondary_button(
            "Make primary",
            Some(event(&address, to_top)),
            tokens,
        ));
    }
    // Row 1's "up" would repeat "Make primary", so it is disabled there.
    let move_to = |to: usize| Some(event(&address, DatasheetMsg::Move { from: index, to }));
    actions = actions.push(kit::reorder_remove(
        (index > 1).then(|| move_to(index - 1)).flatten(),
        (index + 1 < count).then(|| move_to(index + 1)).flatten(),
        event(&address, DatasheetMsg::Remove(index)),
        tokens,
    ));

    // Open only reads, so it must stay live. On a read-only library the
    // whole card is veiled like the other tabs, so Open moves out of it.
    let open = kit::secondary_button(
        "Open",
        can_open.then(|| event(&address, DatasheetMsg::Open(index))),
        tokens,
    );
    let (header_open, open_below) = if read_only {
        (None, Some(open))
    } else {
        (Some(open), None)
    };
    let mut header = row![
        pill(role_label(index), index == 0, tokens),
        pill(mode_label(mode), false, tokens),
        Space::new().width(Length::Fill).boxed(),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if let Some(open) = header_open {
        header = header.push(open);
    }
    header = header.push(actions.boxed());

    let (value_label, value_row): (&str, Element<'a, LibraryMessage>) = match mode {
        DatasheetMode::Url => ("URL", view_url_input(index, &doc.source, address.clone())),
        DatasheetMode::PinnedPdf => (
            "File",
            view_pinned_input(index, &doc.source, tokens, address.clone()),
        ),
    };

    let fields = column![
        field_row("Label", label_input, tokens),
        field_row("Source", mode_picker, tokens),
        field_row(value_label, value_row, tokens),
    ]
    .spacing(SP.m)
    .boxed();
    let mut body: Column<Element<'a, LibraryMessage>> =
        column![header.boxed(), fields].spacing(SP.m);
    if let Some(reason) = error {
        body = body.push(
            text(raw(reason))
                .size(TS.body)
                .color(theme_ext::error_color(tokens))
                .boxed(),
        );
    }

    // The veil covers the whole card, as the other tabs veil their cards,
    // so no part of the card's surface is darker than the rest.
    let card = veiled(read_only, veil, kit::card(body.boxed(), tokens));
    match open_below {
        Some(open) => column![
            card,
            row![Space::new().width(Length::Fill).boxed(), open].align_y(iced::Alignment::Center)
        ]
        .spacing(SP.s)
        .boxed(),
        None => card,
    }
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
    text_input(raw("https://example.com/datasheet.pdf"), url_value)
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
                text(raw(filename.clone()))
                    .size(TS.body)
                    .color(text_c)
                    .boxed(),
                Space::new().width(Length::Fill).boxed(),
                text(raw(short_hash)).size(TS.caption).color(muted).boxed(),
            ]
            .align_y(iced::Alignment::Center)
            .boxed()
        }
        _ => text(raw("No PDF attached yet."))
            .size(TS.body)
            .color(muted)
            .boxed(),
    };

    row![container(summary).width(Length::Fill).boxed(), upload_btn]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .boxed()
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
