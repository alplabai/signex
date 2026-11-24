use std::collections::BTreeMap;

use signex_i18n::I18n;
use signex_types::coord::Unit;

use super::*;
use crate::LengthFormat;

fn mm() -> LengthFormat<'static> {
    LengthFormat::new(I18n::en_us_ref(), Unit::Mm)
}

/// The caller's clearances: one per class, plus the written pairs. The
/// matrix only reads them; a test applies each reported change.
struct Clearances {
    classes: Vec<f64>,
    cells: BTreeMap<ClassPair, f64>,
}

impl Clearances {
    fn apply(&mut self, change: Option<MatrixChange>) {
        match change {
            Some(MatrixChange::ClearanceSet { pair, mm }) => {
                self.cells.insert(pair, mm);
            }
            Some(MatrixChange::ClearanceCleared { pair }) => {
                self.cells.remove(&pair);
            }
            None => {}
        }
    }
}

/// The caller's rule: an empty cell falls back to the larger of the two
/// classes' own clearances.
impl PairValues for Clearances {
    fn resolve(&self, pair: ClassPair) -> Option<PairClearance> {
        let fallback = self.fallback(pair)?;
        Some(match self.cells.get(&pair) {
            Some(mm) => PairClearance::Written(*mm),
            None => PairClearance::Fallback(fallback),
        })
    }

    fn fallback(&self, pair: ClassPair) -> Option<f64> {
        let a = self.classes.get(pair.row)?;
        let b = self.classes.get(pair.col)?;
        Some(f64::max(*a, *b))
    }
}

fn sample() -> (ClassMatrix, Clearances) {
    let classes = [("PWR", 0.3), ("HV", 0.8), ("USB", 0.15)];
    let matrix = ClassMatrix::new(
        classes
            .iter()
            .map(|(name, _)| MatrixClass {
                name: (*name).into(),
            })
            .collect(),
    );
    let values = Clearances {
        classes: classes.iter().map(|(_, mm)| *mm).collect(),
        cells: BTreeMap::new(),
    };
    (matrix, values)
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
    let (matrix, values) = sample();
    assert_eq!(
        matrix.clearance(&values, ClassPair::new(1, 2)),
        Some(PairClearance::Fallback(0.8))
    );
    assert_eq!(
        matrix.clearance(&values, ClassPair::new(2, 2)),
        Some(PairClearance::Fallback(0.15))
    );
    assert_eq!(matrix.clearance(&values, ClassPair::new(5, 0)), None);
}

#[test]
fn a_written_value_wins_and_unknown_classes_or_bad_values_are_refused() {
    let (mut matrix, mut values) = sample();
    values.cells.insert(ClassPair::new(0, 1), 1.0);
    assert_eq!(
        matrix.clearance(&values, ClassPair::new(1, 0)),
        Some(PairClearance::Written(1.0))
    );
    assert_eq!(matrix.clearance(&values, ClassPair::new(3, 0)), None);
    let pair = ClassPair::new(0, 0);
    matrix.update(MatrixMessage::CellPressed(pair), &values, mm());
    for typed in ["-1", "inf", "wide"] {
        matrix.update(MatrixMessage::EditTextChanged(typed.into()), &values, mm());
        let change = matrix.update(MatrixMessage::EditSubmitted, &values, mm());
        assert_eq!(change, None, "{typed}");
        assert!(matrix.edit().is_some_and(|e| e.is_invalid), "{typed}");
        values.apply(change);
    }
    assert_eq!(values.written(pair), None);
}

#[test]
fn pressing_a_cell_pins_it_and_submit_writes_it() {
    let (mut matrix, mut values) = sample();
    let pair = ClassPair::new(1, 0);
    assert_eq!(
        matrix.update(MatrixMessage::CellPressed(pair), &values, mm()),
        None
    );
    assert_eq!(matrix.pinned(), Some(pair));
    assert_eq!(matrix.edit().map(|e| e.text.as_str()), Some(""));
    matrix.update(MatrixMessage::EditTextChanged("1.0".into()), &values, mm());
    let change = matrix.update(MatrixMessage::EditSubmitted, &values, mm());
    assert_eq!(change, Some(MatrixChange::ClearanceSet { pair, mm: 1.0 }));
    assert!(matrix.edit().is_none());
    assert_eq!(matrix.pinned(), Some(pair), "the cell stays pinned");
    assert_eq!(values.written(pair), None, "the matrix stores nothing");
    values.apply(change);

    matrix.update(MatrixMessage::CellPressed(pair), &values, mm());
    assert_eq!(matrix.edit().map(|e| e.text.as_str()), Some("1.000"));
    assert_eq!(
        matrix.update(MatrixMessage::EditSubmitted, &values, mm()),
        None,
        "same value, no event"
    );
}

#[test]
fn a_bad_entry_stays_open_and_an_empty_one_clears() {
    let (mut matrix, mut values) = sample();
    let pair = ClassPair::new(2, 1);
    values.cells.insert(pair, 0.5);
    matrix.update(MatrixMessage::CellPressed(pair), &values, mm());
    matrix.update(MatrixMessage::EditTextChanged("-3".into()), &values, mm());
    assert_eq!(
        matrix.update(MatrixMessage::EditSubmitted, &values, mm()),
        None
    );
    assert!(matrix.edit().is_some_and(|e| e.is_invalid));
    matrix.update(MatrixMessage::EditTextChanged(" ".into()), &values, mm());
    let change = matrix.update(MatrixMessage::EditSubmitted, &values, mm());
    assert_eq!(change, Some(MatrixChange::ClearanceCleared { pair }));
    values.apply(change);
    assert_eq!(
        matrix.clearance(&values, pair),
        Some(PairClearance::Fallback(0.8))
    );
}

#[test]
fn a_typed_clearance_reads_in_the_display_unit() {
    let (mut matrix, values) = sample();
    let pair = ClassPair::new(1, 1);
    let mil = LengthFormat::new(I18n::en_us_ref(), Unit::Mil);
    matrix.update(MatrixMessage::CellPressed(pair), &values, mil);
    matrix.update(MatrixMessage::EditTextChanged("10".into()), &values, mil);
    match matrix.update(MatrixMessage::EditSubmitted, &values, mil) {
        Some(MatrixChange::ClearanceSet { mm, .. }) => assert!((mm - 0.254).abs() < 1e-12),
        other => panic!("expected a clearance, got {other:?}"),
    }
}

#[test]
fn clear_drops_a_written_value_once() {
    let (mut matrix, mut values) = sample();
    let pair = ClassPair::new(0, 0);
    values.cells.insert(pair, 0.4);
    let change = matrix.update(MatrixMessage::CellCleared(pair), &values, mm());
    assert_eq!(change, Some(MatrixChange::ClearanceCleared { pair }));
    values.apply(change);
    assert_eq!(
        matrix.update(MatrixMessage::CellCleared(pair), &values, mm()),
        None
    );
    assert_eq!(
        matrix.update(
            MatrixMessage::CellPressed(ClassPair::new(9, 9)),
            &values,
            mm()
        ),
        None
    );
    assert_eq!(matrix.pinned(), None, "an unknown pair is not pinned");
}

#[test]
fn the_matrix_view_builds_with_an_open_editor() {
    use signex_types::theme::{ThemeId, theme_tokens};
    fn label(s: &'static str) -> crate::Label<'static> {
        crate::raw(s)
    }
    let (mut matrix, mut values) = sample();
    values.cells.insert(ClassPair::new(1, 0), 1.0);
    let words = MatrixText {
        clear: label("Clear"),
        fallback: label("Fallback"),
    };
    let style = GridStyle::from_tokens(&theme_tokens(ThemeId::DEFAULT), 12.0);
    let _ = matrix_view(&matrix, &values, mm(), &words, style);
    matrix.update(
        MatrixMessage::CellPressed(ClassPair::new(2, 1)),
        &values,
        mm(),
    );
    let _ = matrix_view(&matrix, &values, mm(), &words, style);
}

/// The editor opens with a rounded value (0.1016 mm opens as 0.102, 0.1 mm
/// in mil as 3.94); submitting it untouched must not write that back.
#[test]
fn an_untouched_editor_reports_no_change() {
    let (mut matrix, mut values) = sample();
    let pair = ClassPair::new(1, 0);
    values.cells.insert(pair, 0.1016);
    matrix.update(MatrixMessage::CellPressed(pair), &values, mm());
    assert_eq!(matrix.edit().map(|e| e.text.as_str()), Some("0.102"));
    assert_eq!(
        matrix.update(MatrixMessage::EditSubmitted, &values, mm()),
        None
    );
    assert!(matrix.edit().is_none());

    values.cells.insert(pair, 0.1);
    let mil = LengthFormat::new(I18n::en_us_ref(), Unit::Mil);
    matrix.update(MatrixMessage::CellPressed(pair), &values, mil);
    assert_eq!(
        matrix.update(MatrixMessage::EditSubmitted, &values, mil),
        None
    );
}
