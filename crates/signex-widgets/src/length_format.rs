//! How a widget shows and reads a length: the caller's display unit, plus
//! its locale for the parts that follow the language (search folding).
//! Lengths stay millimetres in the caller's model; the unit only decides
//! the number shown and the number typed. A length is an engineering value,
//! so it keeps `.` as its decimal mark in every locale (see
//! `I18n::number`): the number a cell shows is the number its editor reads.

use signex_i18n::{I18n, UserText};
use signex_types::coord::Unit;
use signex_types::measure::{length_field_text, mm_per, parse_length_field_mm};

use crate::Label;

/// Decimals a shown length is rounded to, in the display unit.
const SHOWN_DECIMALS: usize = 4;
/// Most decimals a length is rounded to.
const MAX_DECIMALS: usize = 15;

/// The caller's locale and display unit, passed to every view and update
/// that shows or parses a length.
#[derive(Debug, Clone, Copy)]
pub struct LengthFormat<'a> {
    pub i18n: &'a I18n,
    pub unit: Unit,
}

impl<'a> LengthFormat<'a> {
    pub fn new(i18n: &'a I18n, unit: Unit) -> Self {
        Self { i18n, unit }
    }

    /// `mm` in the display unit, rounded to [`SHOWN_DECIMALS`] decimals with
    /// trailing zeros dropped, without the unit symbol.
    pub fn number(&self, mm: f64) -> String {
        self.number_rounded(mm, SHOWN_DECIMALS)
    }

    /// `mm` in the display unit, rounded to `decimals` decimals with
    /// trailing zeros dropped, `.` as the decimal mark, without the unit
    /// symbol.
    pub fn number_rounded(&self, mm: f64, decimals: usize) -> String {
        let decimals = decimals.min(MAX_DECIMALS);
        let fixed = format!("{:.*}", decimals, mm / mm_per(self.unit));
        let trimmed = if fixed.contains('.') {
            fixed.trim_end_matches('0').trim_end_matches('.')
        } else {
            fixed.as_str()
        };
        // `-0` reads as a sign on nothing.
        match trimmed {
            "-0" | "" => "0".to_owned(),
            other => other.to_owned(),
        }
    }

    /// [`Self::number`] as a widget label.
    pub fn label(&self, mm: f64) -> Label<'static> {
        UserText::new(self.number(mm)).into()
    }

    /// The text an edit field opens with for `mm`. It is rounded to the
    /// unit's field decimals, so an editor commits only when its text
    /// differs from this.
    pub fn field_text(&self, mm: f64) -> String {
        length_field_text(mm, self.unit)
    }

    /// The millimetres a typed length stands for; `None` when the text is
    /// not a finite number.
    pub fn parse(&self, text: &str) -> Option<f64> {
        parse_length_field_mm(text, self.unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::LanguagePref;

    #[test]
    fn a_length_shows_and_reads_in_the_display_unit() {
        let mm = LengthFormat::new(I18n::en_us_ref(), Unit::Mm);
        assert_eq!(mm.number(0.5), "0.5");
        assert_eq!(mm.number(0.123456), "0.1235");
        assert_eq!(mm.number(10.0), "10");
        assert_eq!(mm.number(-0.00001), "0");
        assert_eq!(mm.number(-0.5), "-0.5");
        assert_eq!(mm.parse(" 0.25 "), Some(0.25));
        assert_eq!(mm.parse("abc"), None);
        assert_eq!(mm.parse("inf"), None);

        let mil = LengthFormat::new(I18n::en_us_ref(), Unit::Mil);
        assert_eq!(mil.number(0.254), "10");
        assert_eq!(mil.field_text(0.254), "10.00");
        let typed = mil.parse("10").expect("a number");
        assert!((typed - 0.254).abs() < 1e-12);
    }

    /// In a comma-decimal locale a length still shows `.`, so the shown
    /// number is one the editor reads back.
    #[test]
    fn a_comma_locale_still_shows_lengths_with_a_point() {
        let de = I18n::new(&LanguagePref::Locale("de".to_owned()));
        let mm = LengthFormat::new(&de, Unit::Mm);
        assert_eq!(mm.number(0.25), "0.25");
        assert_eq!(mm.number(1234.5), "1234.5");
        assert_eq!(mm.parse(&mm.number(0.25)), Some(0.25));
    }
}
