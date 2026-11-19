use signex_i18n::{I18n, UserText};

use super::*;

fn sample() -> ClassMatrix {
    ClassMatrix::new(
        [("PWR", 0.3), ("HV", 0.8), ("USB", 0.15)]
            .into_iter()
            .map(|(name, mm)| MatrixClass {
                name: name.into(),
                clearance_mm: mm,
            })
            .collect(),
    )
}

#[test]
fn a_pair_is_the_same_in_either_order_and_lives_in_the_lower_half() {
    assert_eq!(ClassPair::new(0, 2), ClassPair::new(2, 0));
    assert_eq!(ClassPair::new(0, 2), ClassPair { row: 2, col: 0 });
    assert!(ClassPair::is_live(2, 0));
    assert!(ClassPair::is_live(1, 1));
    assert!(!ClassPair::is_live(0, 2));
}

#[test]
fn an_empty_cell_falls_back_to_the_larger_class_clearance() {
    let matrix = sample();
    assert_eq!(
        matrix.clearance(ClassPair::new(1, 2)),
        Some(PairClearance::Fallback(0.8))
    );
    assert_eq!(
        matrix.clearance(ClassPair::new(2, 2)),
        Some(PairClearance::Fallback(0.15))
    );
    assert_eq!(matrix.clearance(ClassPair::new(5, 0)), None);
}

#[test]
fn set_refuses_unknown_classes_and_bad_values() {
    let mut matrix = sample();
    assert!(matrix.set(ClassPair::new(0, 1), 1.0));
    assert_eq!(
        matrix.clearance(ClassPair::new(1, 0)),
        Some(PairClearance::Written(1.0))
    );
    assert!(!matrix.set(ClassPair::new(3, 0), 1.0));
    assert!(!matrix.set(ClassPair::new(0, 0), -1.0));
    assert!(!matrix.set(ClassPair::new(0, 0), f64::INFINITY));
}

#[test]
fn pressing_a_cell_pins_it_and_submit_writes_it() {
    let mut matrix = sample();
    let pair = ClassPair::new(1, 0);
    assert_eq!(matrix.update(MatrixMessage::CellPressed(pair)), None);
    assert_eq!(matrix.pinned(), Some(pair));
    assert_eq!(matrix.edit().map(|e| e.text.as_str()), Some(""));
    matrix.update(MatrixMessage::EditTextChanged("1.0".into()));
    assert_eq!(
        matrix.update(MatrixMessage::EditSubmitted),
        Some(MatrixChange::ClearanceSet { pair, mm: 1.0 })
    );
    assert!(matrix.edit().is_none());
    assert_eq!(matrix.pinned(), Some(pair), "the cell stays pinned");

    matrix.update(MatrixMessage::CellPressed(pair));
    assert_eq!(matrix.edit().map(|e| e.text.as_str()), Some("1"));
    assert_eq!(
        matrix.update(MatrixMessage::EditSubmitted),
        None,
        "same value, no event"
    );
}

#[test]
fn a_bad_entry_stays_open_and_an_empty_one_clears() {
    let mut matrix = sample();
    let pair = ClassPair::new(2, 1);
    matrix.set(pair, 0.5);
    matrix.update(MatrixMessage::CellPressed(pair));
    matrix.update(MatrixMessage::EditTextChanged("-3".into()));
    assert_eq!(matrix.update(MatrixMessage::EditSubmitted), None);
    assert!(matrix.edit().is_some_and(|e| e.is_invalid));
    matrix.update(MatrixMessage::EditTextChanged(" ".into()));
    assert_eq!(
        matrix.update(MatrixMessage::EditSubmitted),
        Some(MatrixChange::ClearanceCleared { pair })
    );
    assert_eq!(matrix.clearance(pair), Some(PairClearance::Fallback(0.8)));
}

#[test]
fn clear_drops_a_written_value_once() {
    let mut matrix = sample();
    let pair = ClassPair::new(0, 0);
    matrix.set(pair, 0.4);
    assert_eq!(
        matrix.update(MatrixMessage::CellCleared(pair)),
        Some(MatrixChange::ClearanceCleared { pair })
    );
    assert_eq!(matrix.update(MatrixMessage::CellCleared(pair)), None);
    assert_eq!(
        matrix.update(MatrixMessage::CellPressed(ClassPair::new(9, 9))),
        None
    );
    assert_eq!(matrix.pinned(), None, "an unknown pair is not pinned");
}

#[test]
fn the_matrix_view_builds_with_an_open_editor() {
    use signex_types::theme::{ThemeId, theme_tokens};
    let i18n = I18n::en_us_ref();
    let mut matrix = sample();
    matrix.set(ClassPair::new(1, 0), 1.0);
    let words = MatrixText {
        clear: UserText::new("Clear").into(),
        fallback: UserText::new("Fallback").into(),
    };
    let style = GridStyle::from_tokens(&theme_tokens(ThemeId::DEFAULT), 12.0);
    let _ = matrix_view(&matrix, i18n, &words, style);
    matrix.update(MatrixMessage::CellPressed(ClassPair::new(2, 1)));
    let _ = matrix_view(&matrix, i18n, &words, style);
}
