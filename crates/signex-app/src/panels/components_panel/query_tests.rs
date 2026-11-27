//! Value-aware search of the Components panel (mockup row 52,
//! components-cx decisions 5, 6 and 9) and the header that holds it.

use std::path::Path;

use iced::Widget;
use signex_i18n::I18n;
use signex_library::{ComponentRow, ParamValue};
use signex_types::quantity::{Quantity, Unit, ValueStyle, format_quantity};

use super::*;
use crate::library::messages::LibraryMessage;
use crate::library::state::LibraryState;
use crate::panels::components_panel::look;
use crate::panels::components_panel::tests::{fixture_row, library_with};

fn i18n() -> std::sync::Arc<I18n> {
    I18n::en_us()
}

fn kind(word: &str) -> WordKind {
    let query = PanelQuery::parse(&i18n(), word);
    assert_eq!(query.words().len(), 1, "{word:?} is one word");
    query.words()[0].kind.clone()
}

fn value(word: &str) -> ValueWord {
    match kind(word) {
        WordKind::Value(v) => v,
        other => panic!("{word:?} should be a value word, got {other:?}"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

/// A part with `params` as measurements `(name, base value, unit token)`.
fn part(pn: &str, params: &[(&str, f64, &str)]) -> ComponentRow {
    let mut row = fixture_row(pn, "", "");
    for (name, base, unit) in params {
        row.parameters.insert(
            (*name).to_owned(),
            ParamValue::Measurement {
                value: *base,
                unit: (*unit).to_owned(),
            },
        );
    }
    row
}

fn matches(row: &ComponentRow, query: &str) -> bool {
    let i18n = i18n();
    let keys = SearchKeys::of(&i18n, row);
    PanelQuery::parse(&i18n, query)
        .words()
        .iter()
        .all(|w| keys.matches(w))
}

#[test]
fn numbers_with_a_prefix_or_unit_read_as_base_values() {
    for (word, base, unit) in [
        ("100n", 100e-9, None),
        ("0.1u", 0.1e-6, None),
        ("0.1µ", 0.1e-6, None),
        ("100nF", 100e-9, Some(Unit::Farad)),
        ("4k7", 4700.0, None),
        ("4R7", 4.7, Some(Unit::Ohm)),
        ("16V", 16.0, Some(Unit::Volt)),
        ("10%", 10.0, Some(Unit::Percent)),
        ("1M", 1e6, None),
        ("1m", 1e-3, None),
    ] {
        let v = value(word);
        assert!(close(v.base, base), "{word:?}: {} != {base}", v.base);
        assert_eq!(v.unit, unit, "{word:?}");
    }
}

#[test]
fn a_number_and_a_unit_word_join_into_one_value() {
    let query = PanelQuery::parse(&i18n(), "0.1 µF X7R");
    let words = query.words();
    assert_eq!(words.len(), 2);
    assert_eq!(words[0].text, "0.1µF");
    match &words[0].kind {
        WordKind::Value(v) => {
            assert!(close(v.base, 0.1e-6));
            assert_eq!(v.unit, Some(Unit::Farad));
        }
        other => panic!("0.1 µF should be a value word, got {other:?}"),
    }
    assert_eq!(words[1].kind, WordKind::Text);
}

#[test]
fn bare_numbers_and_names_keep_their_kinds() {
    assert_eq!(kind("100"), WordKind::Bare(100.0));
    assert_eq!(kind("0603"), WordKind::Bare(603.0));
    for word in ["ERA", "R-10K", "2N2222", "C-100N", "1.2.3", "-"] {
        assert_eq!(kind(word), WordKind::Text, "{word:?}");
    }
}

#[test]
fn a_value_word_finds_the_same_base_value_in_any_prefix() {
    let cap = part(
        "C1",
        &[("capacitance", 100e-9, "F"), ("voltage", 50.0, "V")],
    );
    for query in [
        "100n", "0.1u", "0.1 µF", "100nF", "100.4n", "50V", "100n 50V",
    ] {
        assert!(matches(&cap, query), "{query:?} should find 100 nF 50 V");
    }
    for query in ["101n", "100nV", "50F", "100p"] {
        assert!(
            !matches(&cap, query),
            "{query:?} should not find 100 nF 50 V"
        );
    }
}

#[test]
fn prefixes_are_case_sensitive() {
    let res = part("R1", &[("resistance", 1e6, "ohm")]);
    assert!(matches(&res, "1M"));
    assert!(matches(&res, "1MΩ"));
    assert!(!matches(&res, "1m"), "1m is milli, not mega");
}

#[test]
fn a_bare_number_matches_every_prefix_and_the_names() {
    let pico = part("C-A", &[("capacitance", 100e-12, "F")]);
    let nano = part("C-B", &[("capacitance", 100e-9, "F")]);
    let micro = part("C-C", &[("capacitance", 10e-6, "F")]);
    let named = part("CAP-0100", &[]);
    assert!(matches(&pico, "100"));
    assert!(matches(&nano, "100"));
    assert!(!matches(&micro, "100"), "10 µF does not show 100");
    assert!(matches(&named, "100"), "the part number holds 100");
}

#[test]
fn text_words_must_all_match() {
    let mut row = fixture_row("C-100N", "GRM21", "Murata");
    row.description = "Ceramic capacitor".into();
    assert!(matches(&row, "murata grm"));
    assert!(matches(&row, "ceramic"), "the description is searched");
    // "tdk" is in no field ("era" was, inside "Ceramic").
    assert!(!matches(&row, "murata tdk"));
}

fn shown(state: &LibraryState) -> usize {
    state
        .components_panel
        .rows
        .visible_rows(Path::new("/tmp/one.snxlib"), usize::MAX)
        .count()
}

fn set_filter(state: &mut LibraryState, filter: &str) {
    state.components_panel.filter = filter.into();
    state.sync_view_caches();
}

fn three_caps() -> LibraryState {
    library_with(vec![
        part("C-A", &[("capacitance", 100e-12, "F")]),
        part("C-B", &[("capacitance", 100e-9, "F")]),
        part("C-C", &[("capacitance", 10e-6, "F")]),
        part("CAP-0100", &[]),
    ])
}

#[test]
fn the_panel_list_follows_the_value_aware_search() {
    let mut state = three_caps();
    set_filter(&mut state, "100n");
    assert_eq!(shown(&state), 1);
    set_filter(&mut state, "100");
    assert_eq!(shown(&state), 3);
    set_filter(&mut state, "10u");
    assert_eq!(shown(&state), 1);
    set_filter(&mut state, "one");
    assert_eq!(shown(&state), 4, "the library name lists every part");
}

/// The cache keys on the query as typed: `1m` and `1M` fold alike but are
/// different values.
#[test]
fn a_prefix_case_change_refilters() {
    let mut state = library_with(vec![part("R1", &[("resistance", 1e6, "ohm")])]);
    set_filter(&mut state, "1m");
    assert_eq!(shown(&state), 0);
    set_filter(&mut state, "1M");
    assert_eq!(shown(&state), 1);
}

fn label(base: f64) -> String {
    let q = Quantity::new(base, Unit::Farad).expect("finite");
    format_quantity(&q, ValueStyle::Engineering, false)
}

#[test]
fn the_summary_counts_what_a_bare_number_matched() {
    let mut state = three_caps();
    set_filter(&mut state, "100");
    let summary = state.components_panel.rows.match_summary();
    assert_eq!(
        summary,
        &[MatchLine {
            word: "100".into(),
            values: vec![(label(100e-12), 1), (label(100e-9), 1)],
            by_name: 1,
        }]
    );
    set_filter(&mut state, "100n");
    assert!(
        state.components_panel.rows.match_summary().is_empty(),
        "a value word needs no summary"
    );
}

#[test]
fn the_summary_line_lists_values_and_name_matches() {
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
    let i18n = i18n();
    let lines = [MatchLine {
        word: "100".into(),
        values: vec![("100 pF".into(), 8), ("100 nF".into(), 3)],
        by_name: 2,
    }];
    let element = look::match_summary(&lines, &tokens, &i18n).expect("a summary");
    let texts = crate::layout_probe::lay_out_texts(element, iced::Size::new(400.0, 400.0));
    for shown in [
        "100",
        "\u{2068}100 pF\u{2069} \u{2068}8\u{2069}",
        "\u{2068}100 nF\u{2069} \u{2068}3\u{2069}",
        "name or text \u{2068}2\u{2069}",
    ] {
        crate::layout_probe::find(&texts, shown);
    }
    let empty = [MatchLine {
        word: "7".into(),
        values: Vec::new(),
        by_name: 0,
    }];
    assert!(look::match_summary(&empty, &tokens, &i18n).is_none());
}

/// #2828: the search field spans the dock under the tool line, so its
/// placeholder reads in full even in a narrow dock.
#[test]
fn the_search_field_spans_the_dock_and_shows_its_placeholder() {
    let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
    let i18n = i18n();
    let dock = 300.0;
    let header = look::filter_field("", &tokens, &i18n);
    let inputs =
        crate::layout_probe::lay_out_inputs::<LibraryMessage>(header, iced::Size::new(dock, 400.0));
    let [field] = inputs.as_slice() else {
        panic!("one search field: {inputs:#?}");
    };
    let placeholder = field.text.clone();
    assert_eq!(placeholder, "Search, try 100n or 0.1u");
    let natural = crate::layout_probe::natural::<LibraryMessage>(
        signex_widgets::text(signex_i18n::UserText::new(placeholder.clone()))
            .size(crate::styles::TypeScale::DEFAULT.body)
            .boxed(),
        &placeholder,
    );
    assert!(
        field.bounds.width >= natural.width,
        "the placeholder fits: {} < {}",
        field.bounds.width,
        natural.width
    );
    assert!(
        field.bounds.width >= dock * 0.75,
        "the field takes the dock width: {}",
        field.bounds.width
    );
}

/// A bare number compares with the number a value displays, through the
/// domain's `Quantity::displayed_number` and `approx_equal` (the search
/// used its own copies before, #2785): the sign is part of it and the
/// 0.5 % tolerance applies, so `-40` finds −40 °C, `40` does not, and
/// `100.4` still finds 100 nF.
#[test]
fn a_bare_number_reads_the_displayed_number_with_its_sign() {
    let row = part(
        "IF-1",
        &[
            ("temp_min", -40.0, "degC"),
            ("temp_max", 125.0, "degC"),
            ("capacitance", 100e-9, "F"),
        ],
    );
    assert!(matches(&row, "-40"));
    assert!(!matches(&row, "40"));
    assert!(matches(&row, "125"));
    assert!(matches(&row, "100.4"));
    assert!(!matches(&row, "101"));
}
