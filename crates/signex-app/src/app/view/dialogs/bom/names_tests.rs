//! #2060: the BOM preview's column, rule and issue-count text comes from the
//! catalog, with the English the domain names give.

use signex_i18n::{I18n, LanguagePref};
use signex_output::{
    BomColumn, BomIssueSeverity, BomRule, BomValidationIssue, BomValidationReport,
};

use super::rules::{issue_count_label, rule_label};
use super::table::column_name;

fn xa() -> I18n {
    I18n::new(&LanguagePref::Locale("en-XA".to_owned()))
}

fn plain(text: impl ToString) -> String {
    text.to_string().replace(['\u{2068}', '\u{2069}'], "")
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

fn report(errors: usize, warnings: usize, notes: usize) -> BomValidationReport {
    let issue = |severity| BomValidationIssue {
        rule: BomRule::MissingMpn,
        severity,
        message: String::new(),
        references: Vec::new(),
    };
    let issues = std::iter::repeat_n(issue(BomIssueSeverity::Error), errors)
        .chain(std::iter::repeat_n(
            issue(BomIssueSeverity::Warning),
            warnings,
        ))
        .chain(std::iter::repeat_n(issue(BomIssueSeverity::Info), notes))
        .collect();
    BomValidationReport { issues }
}

/// The footer count reads as `BomValidationReport::summary` for every mix
/// of severities and both plural forms, and follows the language.
#[test]
fn the_issue_count_keeps_the_summary_english() {
    let en = I18n::en_us();
    for errors in 0..3 {
        for warnings in 0..3 {
            for notes in 0..3 {
                let report = report(errors, warnings, notes);
                assert_eq!(
                    issue_count_label(&report, &en).map(plain),
                    report.summary(),
                    "{errors} errors, {warnings} warnings, {notes} notes"
                );
            }
        }
    }
    let mixed = report(2, 1, 2);
    assert_eq!(
        issue_count_label(&mixed, &en).map(plain).as_deref(),
        Some("2 errors, 1 warning, 2 notes")
    );
    assert_ne!(issue_count_label(&mixed, &xa()).map(plain), mixed.summary());
}

/// Large counts stay ungrouped, as `summary()` writes them.
#[test]
fn large_issue_counts_stay_ungrouped() {
    let en = I18n::en_us();
    for (errors, notes) in [(1000, 0), (12345, 1000)] {
        let report = report(errors, 0, notes);
        assert_eq!(issue_count_label(&report, &en).map(plain), report.summary());
    }
    assert_eq!(
        issue_count_label(&report(12345, 0, 0), &en)
            .map(plain)
            .as_deref(),
        Some("12345 errors")
    );
}
