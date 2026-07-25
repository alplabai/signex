use chrono::{Datelike, Local};
use signex_widgets::passive_calculator::{
    ProductionDateCode, ProductionDateCycle, ProductionMonth, RkmEncoder,
};

#[test]
fn twenty_year_cycle_matches_the_documented_examples_and_month_letters() {
    for (year, month, expected) in [
        (2017, ProductionMonth::August, "J8"),
        (2026, ProductionMonth::December, "UD"),
        (2030, ProductionMonth::October, "AO"),
        (1997, ProductionMonth::August, "J8"),
    ] {
        assert_eq!(
            ProductionDateCode::new(ProductionDateCycle::TwentyYear, year, month).to_string(),
            expected
        );
    }
}

#[test]
fn ten_year_cycle_uses_the_year_digit_and_distinct_last_quarter_letters() {
    for (year, month, expected) in [
        (2017, ProductionMonth::August, "78"),
        (2026, ProductionMonth::October, "6X"),
        (2026, ProductionMonth::November, "6Y"),
        (2026, ProductionMonth::December, "6Z"),
    ] {
        assert_eq!(
            ProductionDateCode::new(ProductionDateCycle::TenYear, year, month).to_string(),
            expected
        );
    }
}

#[test]
fn four_year_cycle_encodes_year_and_month_in_one_case_sensitive_character() {
    for (year, month, expected) in [
        (2021, ProductionMonth::January, "A"),
        (2022, ProductionMonth::October, "X"),
        (2023, ProductionMonth::December, "m"),
        (2024, ProductionMonth::January, "n"),
        (2025, ProductionMonth::January, "A"),
    ] {
        assert_eq!(
            ProductionDateCode::new(ProductionDateCycle::FourYear, year, month).to_string(),
            expected
        );
    }
}

#[test]
fn production_date_code_preserves_its_structured_inputs() {
    let code =
        ProductionDateCode::new(ProductionDateCycle::TwentyYear, 2026, ProductionMonth::July);
    assert_eq!(code.cycle(), ProductionDateCycle::TwentyYear);
    assert_eq!(code.year(), 2026);
    assert_eq!(code.month(), ProductionMonth::July);
}

#[test]
fn production_year_dropdown_starts_with_the_current_year() {
    let encoder = RkmEncoder::default();
    let current_year =
        u16::try_from(Local::now().year()).expect("the current calendar year fits into a u16");
    assert_eq!(
        encoder.production_year_options().first(),
        Some(&current_year)
    );
    assert!(
        encoder
            .production_year_options()
            .windows(2)
            .all(|years| years[0] == years[1] + 1)
    );
}
