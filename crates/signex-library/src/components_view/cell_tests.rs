use signex_types::quantity::{Quantity, RangeEnd, Unit, ValueStyle};

use super::test_rows::{interface_ic, interface_rows, m, row, text};
use super::*;
use crate::range_filter::{FilterBound, RangeFilter, RangeOn, RangeOp, RangeVerdict};
use crate::templates::{ColumnKey, ParameterTemplate};

fn cell(row_index: usize, column: &ColumnKey) -> CellValue {
    let rows = interface_rows();
    column_value(
        &rows[row_index],
        &RowFacts::default(),
        Some(&interface_ic()),
        column,
    )
}

fn shown(value: &CellValue) -> CellText {
    value.display(ValueStyle::Engineering)
}

/// components-cx decision 8: a min-to-max value is two stored
/// measurements shown as one cell.
#[test]
fn a_range_column_pairs_its_two_parameters() {
    let supply = cell(1, &ColumnKey::param("supply"));
    assert_eq!(
        shown(&supply),
        CellText::Range {
            low: "3".into(),
            high: "3.6 V".into()
        }
    );
    let CellValue::Range { range, sort_by } = &supply else {
        panic!("{supply:?}");
    };
    assert_eq!((range.low().base(), *sort_by), (3.0, RangeEnd::Min));
    let temp = cell(1, &ColumnKey::param("temperature"));
    assert_eq!(
        shown(&temp),
        CellText::Range {
            low: "\u{2212}40".into(),
            high: "85 °C".into()
        }
    );
    assert!(matches!(
        temp,
        CellValue::Range {
            sort_by: RangeEnd::Max,
            ..
        }
    ));
    // A row without both ends has no range.
    assert_eq!(cell(3, &ColumnKey::param("supply")), CellValue::Missing);
    // The two ends stay readable as their own parameters.
    assert!(matches!(
        cell(1, &ColumnKey::param("supply_min")),
        CellValue::Quantity { .. }
    ));
}

/// components-cx decisions 2 and 7: a measurement cell shows its stored
/// base value in its display prefix; 20 kbps shows as 20 kbps.
#[test]
fn measurement_cells_show_the_stored_value_in_its_prefix() {
    assert_eq!(
        shown(&cell(2, &ColumnKey::param("rate"))),
        CellText::Plain("20 kbps".into())
    );
    assert_eq!(
        shown(&cell(0, &ColumnKey::param("rate"))),
        CellText::Plain("5 Mbps".into())
    );
    assert_eq!(
        shown(&cell(1, &ColumnKey::param("rate"))),
        CellText::Plain("20 Mbps".into())
    );
    let tol = column_value(
        &row("T", &[("tolerance", m(1.0, "%"))]),
        &RowFacts::default(),
        Some(&interface_ic()),
        &ColumnKey::param("tolerance"),
    );
    assert_eq!(shown(&tol), CellText::Plain("±1 %".into()));
    // Text parameters stay text.
    assert_eq!(
        cell(0, &ColumnKey::param("protocol")),
        CellValue::Text("CAN FD".into())
    );
}

#[test]
fn row_fields_and_facts_fill_their_columns() {
    let mut r = row("C1", &[]);
    r.description = "100 nF 50 V X7R 0603".into();
    let facts = RowFacts {
        library_ref: Some("CAP_NP".into()),
        footprint: Some("CAPC1608X90N".into()),
        stock: Some(4000),
        price: Some(0.012),
    };
    let at = |c: ColumnKey| column_value(&r, &facts, None, &c);
    assert_eq!(at(ColumnKey::PartNumber), CellValue::Text("C1".into()));
    assert_eq!(
        at(ColumnKey::Description),
        CellValue::Text(r.description.clone())
    );
    assert_eq!(
        at(ColumnKey::Manufacturer),
        CellValue::Text("Vishay".into())
    );
    assert_eq!(at(ColumnKey::LibraryRef), CellValue::Text("CAP_NP".into()));
    assert_eq!(
        at(ColumnKey::Footprint),
        CellValue::Text("CAPC1608X90N".into())
    );
    assert_eq!(at(ColumnKey::Stock), CellValue::Number(4000.0));
    assert_eq!(at(ColumnKey::Price), CellValue::Number(0.012));
    let none = RowFacts::default();
    for c in [
        ColumnKey::LibraryRef,
        ColumnKey::Footprint,
        ColumnKey::Stock,
        ColumnKey::Price,
    ] {
        assert_eq!(column_value(&r, &none, None, &c), CellValue::Missing, "{c}");
    }
}

#[test]
fn empty_and_unreadable_values_are_not_set() {
    let r = row(
        "X",
        &[
            ("blank", text("  ")),
            ("nan", crate::param::ParamValue::Number(f64::NAN)),
            ("bad", m(f64::INFINITY, "V")),
            ("flag", crate::param::ParamValue::Bool(true)),
        ],
    );
    let at = |name: &str| column_value(&r, &RowFacts::default(), None, &ColumnKey::param(name));
    for name in ["blank", "nan", "bad", "absent"] {
        assert_eq!(at(name), CellValue::Missing, "{name}");
    }
    assert_eq!(at("flag"), CellValue::Bool(true));
    assert_eq!(shown(&at("absent")), CellText::Missing);
    let mut no_pn = row("", &[]);
    no_pn.description.clear();
    assert_eq!(
        column_value(&no_pn, &RowFacts::default(), None, &ColumnKey::PartNumber),
        CellValue::Missing
    );
}

/// components-cx decision 9: search reads every measurement, both ends
/// of a range included.
#[test]
fn quantities_lists_every_measurement_of_a_cell() {
    let supply = cell(0, &ColumnKey::param("supply"));
    let bases: Vec<f64> = supply.quantities().iter().map(|q| q.base()).collect();
    assert_eq!(bases, [4.5, 5.5]);
    assert_eq!(cell(0, &ColumnKey::param("rate")).quantities().len(), 1);
    assert!(
        cell(0, &ColumnKey::param("protocol"))
            .quantities()
            .is_empty()
    );
}

#[test]
fn a_cell_is_judged_by_a_range_filter() {
    let v = |b| Quantity::new(b, Unit::Volt).unwrap();
    let runs_at_3v3 = RangeFilter::new(RangeOp::Equals, v(3.3), None).unwrap();
    let supply = |i| cell(i, &ColumnKey::param("supply"));
    assert_eq!(
        supply(1).range_verdict(&runs_at_3v3, RangeOn::Covers),
        RangeVerdict::Matches
    );
    assert_eq!(
        supply(0).range_verdict(&runs_at_3v3, RangeOn::Covers),
        RangeVerdict::Outside
    );
    assert_eq!(
        supply(3).range_verdict(&runs_at_3v3, RangeOn::Covers),
        RangeVerdict::Excluded
    );
    let fast = RangeFilter::with_bounds(
        RangeOp::AtLeast,
        Unit::BitsPerSecond,
        FilterBound::Base(Quantity::new(1e6, Unit::BitsPerSecond).unwrap()),
        None,
    )
    .unwrap();
    let rate = |i| cell(i, &ColumnKey::param("rate"));
    assert_eq!(
        rate(0).range_verdict(&fast, RangeOn::Value),
        RangeVerdict::Matches
    );
    assert_eq!(
        rate(2).range_verdict(&fast, RangeOn::Value),
        RangeVerdict::Outside
    );
    assert_eq!(
        cell(0, &ColumnKey::param("protocol")).range_verdict(&fast, RangeOn::Value),
        RangeVerdict::Excluded
    );
}

/// Legacy prefixed and canonical rows land in one group; text groups
/// regardless of case.
#[test]
fn group_keys_follow_the_value_not_its_spelling() {
    let at = |p: crate::param::ParamValue| {
        column_value(
            &row("R", &[("v", p)]),
            &RowFacts::default(),
            None,
            &ColumnKey::param("v"),
        )
        .group_key()
    };
    assert_eq!(at(m(4.7, "kΩ")), at(m(4700.0, "ohm")));
    assert_ne!(at(m(4700.0, "ohm")), at(m(4700.0, "F")));
    assert_eq!(at(text("CAN FD")), at(text("can fd")));
    assert_eq!(
        cell(1, &ColumnKey::param("temperature")).group_key(),
        GroupKey::Range {
            low: (-40.0f64).to_bits(),
            high: 85.0f64.to_bits(),
            unit: "degC".into()
        }
    );
}

/// A row storing a min-to-max pair in the new units writes and reads back
/// unchanged: the pair is two ordinary measurements, so the library file
/// format does not change.
#[test]
fn a_range_row_round_trips_through_the_table_encoding() {
    let original = interface_rows().remove(0);
    let record = crate::tables::row_to_record(&original).expect("encodes");
    let record = csv::StringRecord::from(record);
    let back = crate::tables::record_to_row(&record).expect("decodes");
    assert_eq!(back.parameters, original.parameters);
    let template = interface_ic();
    let column = ColumnKey::param("temperature");
    assert_eq!(
        column_value(&back, &RowFacts::default(), Some(&template), &column),
        column_value(&original, &RowFacts::default(), Some(&template), &column)
    );
}

#[test]
fn without_a_template_a_range_name_is_just_a_parameter() {
    let rows = interface_rows();
    let plain = column_value(
        &rows[0],
        &RowFacts::default(),
        None,
        &ColumnKey::param("supply"),
    );
    assert_eq!(plain, CellValue::Missing);
    let empty = ParameterTemplate::default();
    let rate = column_value(
        &rows[0],
        &RowFacts::default(),
        Some(&empty),
        &ColumnKey::param("rate"),
    );
    assert!(matches!(
        rate,
        CellValue::Quantity {
            plus_minus: false,
            ..
        }
    ));
}
