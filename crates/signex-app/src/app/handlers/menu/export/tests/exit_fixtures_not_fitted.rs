//! Advanced Schematic exit criterion 1, the PDF half that needs batch A:
//! the `Lite` PDF of `four-channel-variants` draws its not-fitted parts
//! (C2 on every channel, R1 on CH3 and CH4) greyed and crossed out (#1294),
//! so it differs from `Full`, and from itself with that drawing off.
//!
//! Written against batch A's `PdfOptions::draw_not_fitted`, so it is not in
//! the module tree on trunk. After batch A (#2933) merges, add
//! `mod exit_fixtures_not_fitted;` beside `pub(crate) mod exit_fixtures;` in
//! `handlers/menu/export/tests.rs`, then regenerate the PDF goldens
//! (`UPDATE_GOLDEN=1 cargo test -p signex-app --lib exit_fixtures`): they
//! change by the crossed-out parts.

use super::exit_fixtures::{FOUR_CHANNEL, open_fixture, pdf, pdf_options};
use signex_output::PdfOptions;

fn without_crosses(variant: &str) -> PdfOptions {
    PdfOptions {
        draw_not_fitted: false,
        ..pdf_options(variant)
    }
}

#[test]
fn the_lite_pdf_draws_its_not_fitted_parts_and_full_has_none() {
    let app = open_fixture(FOUR_CHANNEL);
    let (full, _) = pdf(&app, &pdf_options("Full"));
    let (lite, _) = pdf(&app, &pdf_options("Lite"));
    assert_ne!(full, lite, "Lite crosses out C2 and two R1 copies");
    assert_ne!(
        lite,
        pdf(&app, &without_crosses("Lite")).0,
        "the crossed-out parts are what tells the two apart"
    );
    assert_eq!(
        full,
        pdf(&app, &without_crosses("Full")).0,
        "Full fits every part, so nothing is crossed out"
    );
}
