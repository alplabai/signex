//! Alternate symbols section of the Component Preview tab (#1230).
//!
//! One table line per alternate drawing of the row (label field, move up /
//! down, remove) and an "Add alternate symbol" button in the section
//! header that opens the symbol picker. A refused add shows its reason
//! under the table; so do label problems that save would refuse.

use iced::widget::{Column, Widget, column};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{raw, text, text_input};

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress, PrimitivePickerTarget};
use super::kit::{self, SP, TS};

/// Shown when the row has no alternate drawings.
const EMPTY_MESSAGE: signex_i18n::MessageId = signex_i18n::ids::EDITORS_COMPONENT_ALTERNATES_EMPTY;

const COLUMNS: [Length; 3] = [
    Length::FillPortion(3),
    Length::FillPortion(2),
    Length::FillPortion(4),
];

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    i18n: &signex_i18n::I18n,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    use signex_i18n::{ids, t};
    let warning = theme_ext::warning_color(tokens);

    let add = kit::secondary_button(
        "Add alternate symbol",
        Some(LibraryMessage::OpenPrimitivePicker {
            kind: signex_library::PrimitiveKind::Symbol,
            target: PrimitivePickerTarget::PreviewAlternate(address.clone()),
        }),
        tokens,
    );
    let mut col: Column<Element<'a, LibraryMessage>> = column![kit::section_header(
        t!(i18n, ids::EDITORS_COMPONENT_ALTERNATES_HEADING),
        Some(add),
        tokens
    )]
    .spacing(SP.s)
    .width(Length::Fill);

    let count = state.row.alternate_symbols.len();
    if count == 0 {
        col = col.push(kit::empty_state(t!(i18n, EMPTY_MESSAGE), None, tokens));
    } else {
        let mut table: Column<Element<'a, LibraryMessage>> = column![kit::table_header(
            &[
                (Some(ids::EDITORS_COMPONENT_COLUMN_LABEL), COLUMNS[0]),
                (Some(ids::EDITORS_COMPONENT_COLUMN_SYMBOL), COLUMNS[1]),
                (None, COLUMNS[2])
            ],
            i18n,
            tokens,
        )]
        .width(Length::Fill);
        for (idx, alt) in state.row.alternate_symbols.iter().enumerate() {
            table = table.push(alternate_row(idx, count, alt, tokens, address));
        }
        col = col.push(kit::card(table.boxed(), tokens));
    }

    let issues: Vec<String> = signex_library::alternate_symbol_issues(&state.row)
        .iter()
        .map(ToString::to_string)
        .collect();
    for problem in state.alternate_error.iter().chain(issues.iter()) {
        col = col.push(
            text(raw(problem.clone()))
                .size(TS.body)
                .color(warning)
                .boxed(),
        );
    }
    col.boxed()
}

fn alternate_row<'a>(
    idx: usize,
    count: usize,
    alt: &'a signex_library::AlternateSymbol,
    tokens: &ThemeTokens,
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
    let label = text_input(raw("Label (e.g. IEC)"), &alt.label)
        .padding([SP.s, SP.m])
        .size(TS.label)
        .width(Length::Fill)
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
    let actions = kit::reorder_remove(
        up,
        down,
        event(EditorMsg::AlternateSymbolRemoved { idx }),
        tokens,
    );

    kit::table_row(
        vec![
            (label.boxed(), COLUMNS[0]),
            (
                text(raw(format!("symbol {short}")))
                    .size(TS.body)
                    .color(muted)
                    .boxed(),
                COLUMNS[1],
            ),
            (actions, COLUMNS[2]),
        ],
        idx,
        None,
        tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_message_is_plain_language() {
        let message = signex_i18n::I18n::en_us()
            .message(EMPTY_MESSAGE, &[])
            .to_string();
        assert_eq!(
            message,
            "No alternate drawings; the row uses its default symbol only."
        );
        assert!(!message.contains("uuid"));
    }
}
