//! #2596, DECISIONS 2026-11-27 decision 3: the Messages lines a kind switch
//! writes name the kind and every instance it dropped, and a switch that
//! dropped nothing writes none.

use signex_bake::array::{KindSwitchRefused, KindSwitched, PatternKind};
use signex_i18n::I18n;

use super::pattern_kind::{dropped_lines, refused_line};

#[test]
fn a_switch_names_what_it_dropped() {
    let switched = KindSwitched {
        suppressed_dropped: vec![(0, 2), (1, 2)],
        overrides_dropped: vec![(1, 3)],
    };
    let lines = dropped_lines(&I18n::en_us(), PatternKind::Polar, &switched);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("Polar"), "{}", lines[0]);
    assert!(lines[0].contains("(0, 2), (1, 2)"), "{}", lines[0]);
    assert!(lines[1].contains("(1, 3)"), "{}", lines[1]);
    assert!(lines[1].contains("override"), "{}", lines[1]);
}

#[test]
fn a_switch_that_kept_everything_writes_nothing() {
    let lines = dropped_lines(&I18n::en_us(), PatternKind::Grid, &KindSwitched::default());
    assert!(lines.is_empty());
}

/// #3232: a switch that changed nothing says why.
#[test]
fn a_refused_switch_says_why() {
    let i18n = I18n::en_us();
    let gone = refused_line(&i18n, PatternKind::Polar, KindSwitchRefused::NoSuchPattern);
    assert!(
        gone.contains("Polar") && gone.contains("no longer"),
        "{gone}"
    );
    let same = refused_line(&i18n, PatternKind::Grid, KindSwitchRefused::SameKind);
    assert!(same.contains("Grid") && same.contains("already"), "{same}");
}
