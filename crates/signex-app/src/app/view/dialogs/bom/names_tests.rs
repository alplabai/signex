//! #2060: the BOM preview's column and rule text comes from the catalog,
//! with the English the domain names give.

use signex_i18n::{I18n, LanguagePref};
use signex_output::{BomColumn, BomRule};

use super::rules::rule_label;
use super::table::column_name;

fn xa() -> I18n {
    I18n::new(&LanguagePref::Locale("en-XA".to_owned()))
}

/// Every standard column reads as its exported header; a custom-field
/// column keeps its own name.
#[test]
fn column_headers_keep_the_exported_english() {
    let en = I18n::en_us();
    let standard = [
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
    ];
    for column in standard {
        let id = column_name(&column).expect("a standard column has a catalog name");
        assert_eq!(en.message_plain(id, &[]), column.header());
        assert_ne!(xa().message_plain(id, &[]), column.header());
    }
    assert_eq!(column_name(&BomColumn::Custom("MPN".into())), None);
    // Its own message, not the Parameter Manager's "Reference" column.
    assert_eq!(
        column_name(&BomColumn::Reference),
        Some(signex_i18n::ids::BOM_COLUMN_REFERENCE)
    );
}

/// Every rule reads as `BomRule::label` did.
#[test]
fn rule_names_keep_their_english() {
    let en = I18n::en_us();
    for rule in BomRule::ALL {
        assert_eq!(en.message_plain(rule_label(rule), &[]), rule.label());
        assert_ne!(xa().message_plain(rule_label(rule), &[]), rule.label());
    }
}
