//! #1988: a BOM template's columns, to the exporter's columns and back.

use signex_types::bom_template::{
    BomTemplate, TemplateColumn, encode_bom_template, parse_bom_template,
};

use super::*;

fn columns() -> Vec<BomColumn> {
    vec![
        BomColumn::Designator,
        BomColumn::Qty,
        BomColumn::Custom("MPN".into()),
        BomColumn::UnitPrice,
        BomColumn::Computed(ComputedColumn::new("Cost", "= Quantity * UnitPrice").unwrap()),
    ]
}

#[test]
fn columns_round_trip_through_a_template_file_byte_identical() {
    let text = encode_bom_template(&template_of_columns(&columns())).unwrap();
    let read = parse_bom_template(&text).unwrap();
    assert_eq!(columns_of_template(&read).unwrap(), columns());
    let again = encode_bom_template(&template_of_columns(&columns_of_template(&read).unwrap()));
    assert_eq!(again.unwrap(), text);
}

#[test]
fn every_standard_column_has_a_key_and_maps_back() {
    let standard = [
        BomColumn::Name,
        BomColumn::Designator,
        BomColumn::LibRef,
        BomColumn::Reference,
        BomColumn::Qty,
        BomColumn::Value,
        BomColumn::Footprint,
        BomColumn::Description,
    ]
    .into_iter()
    .chain(BomColumn::PRICES);
    for column in standard {
        let key = column.template_key().expect("a standard column has a key");
        let template = BomTemplate {
            columns: vec![TemplateColumn::Standard(key.into())],
        };
        assert_eq!(columns_of_template(&template).unwrap(), [column]);
    }
    assert_eq!(BomColumn::Custom("MPN".into()).template_key(), None);
}

#[test]
fn an_unknown_standard_key_is_refused_naming_the_column() {
    let template = BomTemplate {
        columns: vec![
            TemplateColumn::Standard("qty".into()),
            TemplateColumn::Standard("weight".into()),
        ],
    };
    let why = columns_of_template(&template).unwrap_err();
    assert!(why.starts_with("column 2 of the BOM template"), "{why}");
    assert!(why.contains("`weight`"), "{why}");
}

/// Every standard column, Alternates included, goes through a template file
/// and back. The match is the list's witness: a new `BomColumn` fails to
/// compile here until it is added below and given a template key.
#[test]
fn every_standard_column_round_trips_through_a_template_file() {
    let witness = |column: &BomColumn| match column {
        BomColumn::Name
        | BomColumn::Designator
        | BomColumn::LibRef
        | BomColumn::Reference
        | BomColumn::Qty
        | BomColumn::Value
        | BomColumn::Footprint
        | BomColumn::Description
        | BomColumn::UnitPrice
        | BomColumn::BuyQty
        | BomColumn::ExtendedPrice
        | BomColumn::Currency
        | BomColumn::Stock
        | BomColumn::Distributor
        | BomColumn::PriceAge
        | BomColumn::Alternates => true,
        BomColumn::Custom(_) | BomColumn::Computed(_) => false,
    };
    let every = vec![
        BomColumn::Name,
        BomColumn::Designator,
        BomColumn::LibRef,
        BomColumn::Reference,
        BomColumn::Qty,
        BomColumn::Value,
        BomColumn::Footprint,
        BomColumn::Description,
        BomColumn::UnitPrice,
        BomColumn::BuyQty,
        BomColumn::ExtendedPrice,
        BomColumn::Currency,
        BomColumn::Stock,
        BomColumn::Distributor,
        BomColumn::PriceAge,
        BomColumn::Alternates,
    ];
    assert!(every.iter().all(witness));
    let template = template_of_columns(&every);
    assert_eq!(template.columns.len(), every.len(), "no column is left out");
    assert!(
        template
            .columns
            .iter()
            .all(|c| !matches!(c, TemplateColumn::Standard(key) if key.is_empty())),
        "no empty key"
    );
    let text = encode_bom_template(&template).unwrap();
    let read = parse_bom_template(&text).unwrap();
    assert_eq!(columns_of_template(&read).unwrap(), every);
}
