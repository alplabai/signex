//! Alternate symbols subsection of the Component Preview tab (#1230).
//!
//! One line per alternate drawing of the row (label field, move up / down,
//! remove) and an "Add alternate symbol" button that opens the symbol
//! picker. A refused add shows its reason under the list; so do label
//! problems that save would refuse.

use iced::widget::{Space, column, row, text, text_input};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress, PrimitivePickerTarget};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let warning = theme_ext::warning_color(tokens);

    let mut col = column![
        text("Alternate symbols").size(TS.body).color(muted),
        Space::new().height(6),
    ]
    .spacing(0)
    .width(Length::Fill);

    if state.row.alternate_symbols.is_empty() {
        col = col.push(
            text("No alternate drawings; the row uses its default symbol only.")
                .size(TS.body)
                .color(muted),
        );
    }
    let count = state.row.alternate_symbols.len();
    for (idx, alt) in state.row.alternate_symbols.iter().enumerate() {
        col = col.push(alternate_row(idx, count, alt, tokens, address));
    }

    col = col.push(Space::new().height(6)).push(small_btn(
        "Add alternate symbol…",
        Some(LibraryMessage::OpenPrimitivePicker {
            kind: signex_library::PrimitiveKind::Symbol,
            target: PrimitivePickerTarget::PreviewAlternate(address.clone()),
        }),
        tokens,
    ));

    let issues: Vec<String> = signex_library::alternate_symbol_issues(&state.row)
        .iter()
        .map(ToString::to_string)
        .collect();
    let problems = state.alternate_error.iter().chain(issues.iter());
    for problem in problems {
        col = col.push(text(problem.clone()).size(TS.body).color(warning));
    }
    col.into()
}

fn alternate_row<'a>(
    idx: usize,
    count: usize,
    alt: &'a signex_library::AlternateSymbol,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let event = {
        let address = address.clone();
        move |msg: EditorMsg| LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg,
        }
    };
    let on_label = event.clone();
    let label = text_input("Label (e.g. IEC)", &alt.label)
        .padding([SP.s, SP.m])
        .size(TS.label)
        .width(Length::FillPortion(3))
        .on_input(move |label| on_label(EditorMsg::AlternateSymbolRelabeled { idx, label }));
    let short = signex_types::object_id::short_id(&alt.symbol_ref.uuid);
    let up = (idx > 0).then(|| {
        event(EditorMsg::AlternateSymbolMoved {
            from: idx,
            to: idx - 1,
        })
    });
    let down = (idx + 1 < count).then(|| {
        event(EditorMsg::AlternateSymbolMoved {
            from: idx,
            to: idx + 1,
        })
    });
    row![
        label,
        Space::new().width(8),
        text(format!("symbol {}", short))
            .size(TS.body)
            .color(muted)
            .width(Length::FillPortion(2)),
        small_btn("↑", up, tokens),
        Space::new().width(4),
        small_btn("↓", down, tokens),
        Space::new().width(4),
        small_btn(
            "×",
            Some(event(EditorMsg::AlternateSymbolRemoved { idx })),
            tokens
        ),
    ]
    .align_y(iced::Alignment::Center)
    .padding([SP.xs, 0.0])
    .into()
}

fn small_btn<'a>(
    label: &'a str,
    msg: Option<LibraryMessage>,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    super::kit::secondary_button(label, msg, tokens)
}
