use super::*;

const SHEET: &str = "1\tOUTA\tO\tOutput channel A\n\
                     2\t\u{2013}INA\tI\tInverting input channel A\n\
                     \n\
                     4\tV\u{2013}\tP\tNegative supply\n";

fn draft(text: &str) -> PasteDraft {
    PasteDraft {
        content: text_editor::Content::with_text(text),
        ..PasteDraft::default()
    }
}

#[test]
fn columns_split_on_a_tab_or_a_run_of_spaces() {
    assert_eq!(split_columns("1\tOUTA\tO"), ["1", "OUTA", "O"]);
    assert_eq!(
        split_columns("7  OUTB   O  Output channel B"),
        ["7", "OUTB", "O", "Output channel B"]
    );
    assert_eq!(split_columns("single spaced name"), ["single spaced name"]);
}

#[test]
fn type_words_map_to_electrical_types() {
    assert_eq!(type_of_word("I"), PinDirection::Input);
    assert_eq!(type_of_word(" out "), PinDirection::Output);
    assert_eq!(type_of_word("I/O"), PinDirection::Bidirectional);
    assert_eq!(type_of_word("PWR"), PinDirection::PowerInput);
    assert_eq!(type_of_word("\u{2014}"), PinDirection::PowerInput);
    assert_eq!(type_of_word("OC"), PinDirection::OpenDrainLow);
    assert_eq!(type_of_word("output"), PinDirection::Output);
    assert_eq!(type_of_word("NC"), PinDirection::Passive);
}

#[test]
fn pasted_names_get_the_minus_sign() {
    assert_eq!(typographic_minus("\u{2013}INA"), "\u{2212}INA");
    assert_eq!(typographic_minus("-INB"), "\u{2212}INB");
    assert_eq!(typographic_minus("V\u{2013}"), "V\u{2212}");
    assert_eq!(typographic_minus("A-B"), "A-B");
}

#[test]
fn rows_are_read_through_the_default_column_map() {
    let rows = draft(SHEET).rows();
    assert_eq!(rows.len(), 3, "the blank line is skipped");
    assert_eq!(rows[0].number.as_deref(), Some("1"));
    assert_eq!(rows[0].name.as_deref(), Some("OUTA"));
    assert_eq!(rows[0].electrical, Some(PinDirection::Output));
    assert_eq!(rows[0].description.as_deref(), Some("Output channel A"));
    assert_eq!(rows[1].name.as_deref(), Some("\u{2212}INA"));
    assert_eq!(rows[2].electrical, Some(PinDirection::PowerInput));
    assert_eq!(rows[2].part, None);
}

#[test]
fn a_remapped_column_is_read_as_its_new_field() {
    let mut ui = PinTableUi {
        paste: Some(draft("1\tOUTA\t2\t3,5")),
        ..PinTableUi::default()
    };
    assert!(ui.apply(&PinTableMsg::PasteColumnMapped(2, PasteField::Part)));
    assert!(ui.apply(&PinTableMsg::PasteColumnMapped(3, PasteField::Length)));
    let rows = ui.paste.as_ref().map(PasteDraft::rows).unwrap_or_default();
    assert_eq!(rows[0].part, Some(2));
    assert_eq!(rows[0].length, Some(3.5));
    assert_eq!(rows[0].electrical, None);
}

#[test]
fn the_map_offers_the_widest_line_and_at_least_four_columns() {
    assert_eq!(column_count(""), 4);
    assert_eq!(column_count("1\t2\t3\t4\t5\t6"), 6);
}

#[test]
fn the_preview_counts_updates_and_additions_by_mode() {
    let rows = draft(SHEET).rows();
    let numbers = ["1", "2", "3"];
    assert_eq!(
        paste_preview(&rows, PasteMode::UpdateByNumber, &numbers),
        PastePreview {
            rows: 3,
            updated: 2,
            added: 1,
            replaced: 0
        }
    );
    assert_eq!(paste_preview(&rows, PasteMode::Append, &numbers).added, 3);
    assert_eq!(
        paste_preview(&rows, PasteMode::ReplaceAll, &numbers).replaced,
        3
    );
}

#[test]
fn ticks_bulk_fields_and_the_paste_box_are_tab_state() {
    let mut ui = PinTableUi::default();
    assert!(ui.apply(&PinTableMsg::RowChecked(2, true)));
    assert!(ui.apply(&PinTableMsg::RowChecked(5, true)));
    assert!(ui.apply(&PinTableMsg::RowChecked(2, false)));
    assert_eq!(ui.checked.iter().copied().collect::<Vec<_>>(), [5]);
    assert!(ui.apply(&PinTableMsg::AllChecked(vec![0, 1], true)));
    assert_eq!(ui.checked.len(), 3);
    assert!(ui.apply(&PinTableMsg::AllChecked(Vec::new(), false)));
    assert!(ui.checked.is_empty());
    assert!(ui.apply(&PinTableMsg::BulkLengthChanged("5.08".into())));
    assert!(ui.apply(&PinTableMsg::BulkHiddenPicked(Some(true))));
    assert_eq!(ui.bulk.length, "5.08");
    assert_eq!(ui.bulk.hidden, Some(true));
    assert!(ui.apply(&PinTableMsg::PasteToggled));
    assert!(ui.paste.is_some());
    assert!(ui.apply(&PinTableMsg::PasteModePicked(PasteMode::Append)));
    assert_eq!(ui.paste.as_ref().map(|p| p.mode), Some(PasteMode::Append));
    assert!(ui.apply(&PinTableMsg::PasteCancelled));
    assert!(ui.paste.is_none());
    assert!(ui.apply(&PinTableMsg::IssuesOnlyToggled));
    assert!(ui.issues_only);
}

#[test]
fn symbol_edits_are_left_to_the_caller() {
    let mut ui = PinTableUi::default();
    for msg in [
        PinTableMsg::BulkApplied,
        PinTableMsg::AddPinPressed,
        PinTableMsg::DeleteSelectedPressed,
        PinTableMsg::PasteApplied,
    ] {
        assert!(!ui.apply(&msg), "{msg:?}");
    }
    assert_eq!(ui, PinTableUi::default());
}

#[test]
fn ticks_of_pins_that_are_gone_are_dropped() {
    let mut ui = PinTableUi::default();
    ui.checked.extend([1, 4, 7]);
    ui.keep_checks_below(5);
    assert_eq!(ui.checked.iter().copied().collect::<Vec<_>>(), [1, 4]);
}

#[test]
fn a_number_parses_with_a_decimal_comma() {
    assert_eq!(parse_mm(" 2,54 "), Some(2.54));
    assert_eq!(parse_mm("nan"), None);
    assert_eq!(parse_mm("x"), None);
}
