//! An applied range filter folds into a compact removable chip (mockup row
//! 118): "Resistance 1k – 100k Ω ×". The chip opens the editor again; the
//! × removes the filter.

use std::collections::BTreeMap;

use iced::{Element, Size};
use signex_i18n::I18n;
use signex_library::RangeOp;
use signex_types::quantity::{SiPrefix, Unit};
use signex_types::theme::{ThemeId, theme_tokens};

use super::RangeFilters;
use super::view::{FilterColumn, chip_label, range_filter_bar};
use crate::layout_probe::{lay_out_inputs, lay_out_texts, messages_on_click};
use crate::library::messages::{RangeBound, RangeFilterMsg};

const WINDOW: Size = Size::new(1400.0, 900.0);
const KEY: &str = "value";

fn plain(s: impl ToString) -> String {
    s.to_string().replace(['\u{2068}', '\u{2069}'], "")
}

fn edit(filters: &mut RangeFilters, bound: RangeBound, text: &str, prefix: SiPrefix) {
    filters.apply(RangeFilterMsg::NumberEdited {
        key: KEY.into(),
        bound,
        text: text.into(),
    });
    filters.apply(RangeFilterMsg::PrefixPicked {
        key: KEY.into(),
        bound,
        prefix,
    });
}

/// "Value between 1k and 100k Ω", typed the way the widgets send it.
fn between_1k_and_100k() -> RangeFilters {
    let mut filters = RangeFilters::default();
    filters.apply(RangeFilterMsg::ColumnPicked {
        key: KEY.into(),
        unit: Unit::Ohm,
    });
    filters.apply(RangeFilterMsg::OperatorPicked {
        key: KEY.into(),
        op: RangeOp::Between,
    });
    edit(&mut filters, RangeBound::Low, "1", SiPrefix::Kilo);
    edit(&mut filters, RangeBound::High, "100", SiPrefix::Kilo);
    filters
}

fn finish(filters: &mut RangeFilters) {
    filters.apply(RangeFilterMsg::EditingFinished { key: KEY.into() });
}

fn columns() -> Vec<FilterColumn> {
    vec![
        FilterColumn {
            key: KEY.into(),
            label: "value".into(),
            unit: Unit::Ohm,
        },
        FilterColumn {
            key: "power".into(),
            label: "power".into(),
            unit: Unit::Watt,
        },
    ]
}

fn bar<'a>(
    filters: &'a RangeFilters,
    columns: &[FilterColumn],
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, RangeFilterMsg> {
    range_filter_bar(filters, columns, &BTreeMap::new(), tokens, i18n, |m| m).expect("a bar")
}

#[test]
fn a_new_filter_opens_in_its_editor_and_done_folds_it_into_a_chip() {
    let mut filters = between_1k_and_100k();
    assert!(
        !filters.shows_as_chip(KEY),
        "the filter being typed stays an editor"
    );

    finish(&mut filters);
    assert!(filters.shows_as_chip(KEY));
    // Folding into a chip does not change what is filtered.
    assert_eq!(filters.active().len(), 1);
}

#[test]
fn pressing_the_chip_opens_the_editor_again() {
    let mut filters = between_1k_and_100k();
    finish(&mut filters);

    filters.apply(RangeFilterMsg::ChipPressed { key: KEY.into() });
    assert!(!filters.shows_as_chip(KEY));
    assert_eq!(
        filters.active().len(),
        1,
        "editing keeps the filter applied"
    );
}

#[test]
fn an_unfinished_filter_keeps_its_editor_when_done_is_sent() {
    let mut filters = RangeFilters::default();
    filters.apply(RangeFilterMsg::ColumnPicked {
        key: KEY.into(),
        unit: Unit::Ohm,
    });
    edit(&mut filters, RangeBound::Low, "-", SiPrefix::One);
    finish(&mut filters);
    assert!(!filters.shows_as_chip(KEY));

    // Finishing the number later does not fold it mid-typing.
    edit(&mut filters, RangeBound::Low, "5", SiPrefix::One);
    assert!(!filters.shows_as_chip(KEY));
}

#[test]
fn editing_another_filter_folds_the_finished_one_into_its_chip() {
    let mut filters = between_1k_and_100k();
    filters.apply(RangeFilterMsg::ColumnPicked {
        key: "power".into(),
        unit: Unit::Watt,
    });
    assert!(filters.shows_as_chip(KEY));
    assert!(!filters.shows_as_chip("power"));
}

#[test]
fn removing_a_filter_from_its_chip_drops_it() {
    let mut filters = between_1k_and_100k();
    finish(&mut filters);
    filters.apply(RangeFilterMsg::Cleared { key: KEY.into() });
    assert!(filters.is_empty());
    assert!(!filters.shows_as_chip(KEY));
}

#[test]
fn the_chip_reads_the_column_the_range_and_the_unit() {
    let i18n = I18n::en_us_ref();
    let filters = between_1k_and_100k();
    let draft = filters.get(KEY).unwrap();
    assert_eq!(
        plain(chip_label(i18n, "Resistance", draft).unwrap()),
        "Resistance 1k – 100k Ω"
    );

    let mut filters = RangeFilters::default();
    filters.apply(RangeFilterMsg::ColumnPicked {
        key: KEY.into(),
        unit: Unit::Ohm,
    });
    edit(&mut filters, RangeBound::Low, "4.7", SiPrefix::Kilo);
    let draft = filters.get(KEY).unwrap();
    assert_eq!(
        plain(chip_label(i18n, "Value", draft).unwrap()),
        "Value ≥ 4.7k Ω"
    );
}

#[test]
fn the_chip_shows_the_prefix_the_filter_compares_with() {
    // Farad takes no kilo: the filter compares 5 F, and the chip says so.
    let i18n = I18n::en_us_ref();
    let mut filters = RangeFilters::default();
    filters.apply(RangeFilterMsg::ColumnPicked {
        key: KEY.into(),
        unit: Unit::Farad,
    });
    filters.apply(RangeFilterMsg::OperatorPicked {
        key: KEY.into(),
        op: RangeOp::AtMost,
    });
    edit(&mut filters, RangeBound::Low, "5", SiPrefix::Kilo);
    let draft = filters.get(KEY).unwrap();
    assert_eq!(
        plain(chip_label(i18n, "Value", draft).unwrap()),
        "Value ≤ 5 F"
    );
}

#[test]
fn an_unfinished_filter_has_no_chip_text() {
    let mut filters = RangeFilters::default();
    filters.apply(RangeFilterMsg::ColumnPicked {
        key: KEY.into(),
        unit: Unit::Ohm,
    });
    assert!(chip_label(I18n::en_us_ref(), "Value", filters.get(KEY).unwrap()).is_none());
}

#[test]
fn the_bar_draws_an_applied_filter_as_a_chip_beside_add_range_filter() {
    let i18n = I18n::en_us_ref();
    let mut filters = between_1k_and_100k();
    finish(&mut filters);
    let columns = columns();
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(id);
        let texts = lay_out_texts(bar(&filters, &columns, &tokens, i18n), WINDOW);
        let chip = texts
            .iter()
            .find(|t| plain(&t.text) == "Value 1k – 100k Ω")
            .unwrap_or_else(|| panic!("no chip text in {texts:#?}"));
        let remove = texts.iter().find(|t| t.text == "✕").expect("the chip's ×");
        // One line: the × sits right of the text, on the same row.
        assert!(remove.bounds.x >= chip.bounds.x + chip.bounds.width);
        assert!((remove.bounds.center_y() - chip.bounds.center_y()).abs() < chip.bounds.height);
        // No editor: no "and", no number fields.
        assert!(texts.iter().all(|t| plain(&t.text) != "and"));
        let inputs = lay_out_inputs(bar(&filters, &columns, &tokens, i18n), WINDOW);
        assert!(inputs.is_empty(), "a chip has no number field: {inputs:#?}");
    }
}

#[test]
fn the_bar_draws_the_filter_being_edited_as_its_editor() {
    let i18n = I18n::en_us_ref();
    let filters = between_1k_and_100k();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let columns = columns();
    let texts = lay_out_texts(bar(&filters, &columns, &tokens, i18n), WINDOW);
    assert!(texts.iter().any(|t| plain(&t.text) == "and"));
    assert!(texts.iter().any(|t| plain(&t.text) == "Done"));
    assert!(texts.iter().all(|t| plain(&t.text) != "Value 1k – 100k Ω"));
    let inputs = lay_out_inputs(bar(&filters, &columns, &tokens, i18n), WINDOW);
    assert_eq!(inputs.len(), 2, "two number fields for between");
}

#[test]
fn clicking_the_chip_opens_the_editor_and_its_x_removes_the_filter() {
    let i18n = I18n::en_us_ref();
    let mut filters = between_1k_and_100k();
    finish(&mut filters);
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let columns = columns();
    let label = chip_label(i18n, "Value", filters.get(KEY).unwrap())
        .unwrap()
        .to_string();

    let on_chip = messages_on_click(bar(&filters, &columns, &tokens, i18n), WINDOW, &label);
    assert_eq!(on_chip, [RangeFilterMsg::ChipPressed { key: KEY.into() }]);

    let on_x = messages_on_click(bar(&filters, &columns, &tokens, i18n), WINDOW, "✕");
    assert_eq!(on_x, [RangeFilterMsg::Cleared { key: KEY.into() }]);
}

#[test]
fn done_in_the_editor_publishes_editing_finished() {
    let i18n = I18n::en_us_ref();
    let filters = between_1k_and_100k();
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let columns = columns();
    let done = signex_i18n::t!(i18n, signex_i18n::ids::LIBRARY_RANGE_FILTER_DONE).to_string();
    let hits = messages_on_click(bar(&filters, &columns, &tokens, i18n), WINDOW, &done);
    assert_eq!(hits, [RangeFilterMsg::EditingFinished { key: KEY.into() }]);
}
